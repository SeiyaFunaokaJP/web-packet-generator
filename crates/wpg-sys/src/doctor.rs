//! `wpg doctor`: combines the OS probes into a list of checks.
//!
//! `summary_key` and `fix_key` are Fluent message ids (`doctor-*` in `locales/*/main.ftl`);
//! `detail` and `fix_hint` are OS text (versions, paths, shell commands) and stay
//! untranslated. The fix message receives `hint` as its only argument.

use serde::Serialize;

use crate::caps::{self, RawAccess};
use crate::hugepages::{self, Hugepages};
use crate::ifaces::{self, Iface};
use crate::kernel::{self, KernelVersion};
use crate::npcap::{self, NpcapInfo};
use crate::pcaplib;

/// Result of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Fine.
    Ok,
    /// Works but degraded / optional feature unavailable.
    Warn,
    /// Sending will not work.
    Fail,
    /// Not applicable on this OS.
    Skip,
}

/// One diagnostic line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// Stable machine id, e.g. `caps.raw_socket`.
    pub id: &'static str,
    /// Outcome.
    pub status: Status,
    /// Fluent message id for the one-line summary.
    pub summary_key: &'static str,
    /// Extra facts (untranslated).
    pub detail: String,
    /// Fluent message id wrapping the hint (`{ $hint }`), when there is a fix.
    pub fix_key: Option<&'static str>,
    /// Command / URL that fixes the problem (untranslated).
    pub fix_hint: Option<String>,
}

impl Check {
    fn new(id: &'static str, status: Status, summary_key: &'static str, detail: String) -> Self {
        Self {
            id,
            status,
            summary_key,
            detail,
            fix_key: None,
            fix_hint: None,
        }
    }

    fn with_fix(mut self, key: &'static str, hint: String) -> Self {
        self.fix_key = Some(key);
        self.fix_hint = Some(hint);
        self
    }

    fn skip(id: &'static str) -> Self {
        Self::new(id, Status::Skip, "doctor-skip", String::new())
    }
}

/// All checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// In display order.
    pub checks: Vec<Check>,
}

impl Report {
    /// Any check failed.
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.checks.iter().any(|c| c.status == Status::Fail)
    }

    /// Looks a check up by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Check> {
        self.checks.iter().find(|c| c.id == id)
    }
}

/// Runs every check against the current host.
#[must_use]
pub fn run() -> Report {
    let exe = std::env::current_exe()
        .map_or_else(|_| "wpg".to_owned(), |p| p.to_string_lossy().into_owned());
    let ifaces = ifaces::list();
    let first_iface = ifaces
        .as_ref()
        .ok()
        .and_then(|v| v.iter().find(|i| i.is_usable()))
        .map(|i| i.name.clone());
    let npcap = npcap::detect();
    Report {
        checks: vec![
            check_caps(&caps::probe(), &exe, cfg!(windows), npcap.admin_only),
            check_ifaces(ifaces.as_deref().map_err(ToString::to_string)),
            check_af_xdp(kernel::release().as_deref(), cfg!(target_os = "linux")),
            check_tpacket_v3(kernel::release().as_deref(), cfg!(target_os = "linux")),
            check_hugepages(hugepages::read().as_ref(), first_iface.as_deref()),
            check_npcap(&npcap, cfg!(windows)),
            check_pcap_lib(
                pcaplib::find().as_deref().map(|p| p.display().to_string()),
                cfg!(windows),
            ),
        ],
    }
}

/// `npcap_admin_only`: the Npcap driver's administrator-only setting (Windows).
fn check_caps(a: &RawAccess, exe: &str, windows: bool, npcap_admin_only: Option<bool>) -> Check {
    let mut detail = format!(
        "euid/elevated={} CAP_NET_RAW={} CAP_NET_ADMIN={} bpf_writable={}",
        a.privileged_user,
        opt(a.cap_net_raw),
        opt(a.cap_net_admin),
        opt(a.bpf_writable)
    );
    if windows {
        detail.push_str(&format!(" npcap_admin_only={}", opt(npcap_admin_only)));
    }
    if a.allowed() {
        return Check::new("caps.raw_socket", Status::Ok, "doctor-caps-ok", detail);
    }
    // An unelevated process can use Npcap unless it was installed in administrator-only
    // mode; when that setting cannot be read this is only a warning.
    let (status, key) = match (windows, npcap_admin_only) {
        (true, Some(false)) => {
            return Check::new(
                "caps.raw_socket",
                Status::Ok,
                "doctor-caps-npcap-user",
                detail,
            );
        }
        (true, Some(true)) => (Status::Fail, "doctor-caps-npcap-admin"),
        (true, None) => (Status::Warn, "doctor-caps-missing"),
        (false, _) => (Status::Fail, "doctor-caps-missing"),
    };
    Check::new("caps.raw_socket", status, key, detail)
        .with_fix("doctor-fix-caps", caps::setcap_hint(exe))
}

fn opt(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "n/a",
    }
}

fn check_ifaces(list: Result<&[Iface], String>) -> Check {
    match list {
        Err(e) => Check::new("iface.any_up", Status::Fail, "doctor-iface-error", e),
        Ok(all) => {
            let usable: Vec<&str> = all
                .iter()
                .filter(|i| i.is_usable())
                .map(|i| i.name.as_str())
                .collect();
            if usable.is_empty() {
                Check::new(
                    "iface.any_up",
                    Status::Warn,
                    "doctor-iface-none",
                    format!("{} interface(s), none up with carrier", all.len()),
                )
                .with_fix("doctor-fix-iface", "wpg ifaces".to_owned())
            } else {
                Check::new(
                    "iface.any_up",
                    Status::Ok,
                    "doctor-iface-ok",
                    usable.join(", "),
                )
            }
        }
    }
}

fn check_af_xdp(release: Option<&str>, linux: bool) -> Check {
    if !linux {
        return Check::skip("kernel.af_xdp");
    }
    let ver = release.and_then(kernel::parse_release);
    version_check(
        "kernel.af_xdp",
        release,
        ver,
        kernel::AF_XDP_MIN,
        "doctor-af-xdp",
    )
}

fn check_tpacket_v3(release: Option<&str>, linux: bool) -> Check {
    if !linux {
        return Check::skip("kernel.tpacket_v3");
    }
    let ver = release.and_then(kernel::parse_release);
    version_check(
        "kernel.tpacket_v3",
        release,
        ver,
        kernel::TPACKET_V3_MIN,
        "doctor-tpacket-v3",
    )
}

fn version_check(
    id: &'static str,
    release: Option<&str>,
    ver: Option<KernelVersion>,
    min: KernelVersion,
    key_base: &'static str,
) -> Check {
    let detail = format!("kernel {} (need >= {min})", release.unwrap_or("unknown"));
    let (status, suffix) = match ver {
        Some(v) if v.at_least(min) => (Status::Ok, "ok"),
        _ => (Status::Warn, "old"),
    };
    let key = match (key_base, suffix) {
        ("doctor-af-xdp", "ok") => "doctor-af-xdp-ok",
        ("doctor-af-xdp", _) => "doctor-af-xdp-old",
        (_, "ok") => "doctor-tpacket-v3-ok",
        _ => "doctor-tpacket-v3-old",
    };
    Check::new(id, status, key, detail)
}

fn check_hugepages(h: Option<&Hugepages>, iface: Option<&str>) -> Check {
    let Some(h) = h else {
        return Check::skip("hugepages");
    };
    let detail = if h.pools.is_empty() {
        format!(
            "{} x {} kB, {} free",
            h.meminfo.total, h.meminfo.size_kb, h.meminfo.free
        )
    } else {
        h.pools
            .iter()
            .map(|p| format!("{} x {} kB ({} free)", p.total, p.size_kb, p.free))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if h.total_pages() > 0 {
        Check::new("hugepages", Status::Ok, "doctor-hugepages-ok", detail)
    } else {
        Check::new("hugepages", Status::Warn, "doctor-hugepages-none", detail)
            .with_fix("doctor-fix-hugepages", hugepages::dpdk_devbind_hint(iface))
    }
}

fn check_npcap(info: &NpcapInfo, windows: bool) -> Check {
    if !windows {
        return Check::skip("npcap");
    }
    let detail = format!(
        "registry={} wpcap.dll={} Packet.dll={} winpcap_compatible={}",
        info.registry_key,
        info.wpcap_dll
            .as_ref()
            .map_or_else(|| "-".to_owned(), |p| p.display().to_string()),
        info.packet_dll
            .as_ref()
            .map_or_else(|| "-".to_owned(), |p| p.display().to_string()),
        info.winpcap_compatible
    );
    if !info.installed() {
        Check::new("npcap", Status::Fail, "doctor-npcap-missing", detail)
            .with_fix("doctor-fix-npcap", "https://npcap.com/#download".to_owned())
    } else if !info.loadable() {
        // wpg adds System32\Npcap to the DLL search path itself (`npcap::load`), so the
        // WinPcap-compatible mode is not needed; only a missing wpcap.dll is a problem.
        Check::new("npcap", Status::Warn, "doctor-npcap-no-dll", detail)
            .with_fix("doctor-fix-npcap", "https://npcap.com/#download".to_owned())
    } else {
        Check::new("npcap", Status::Ok, "doctor-npcap-ok", detail)
    }
}

fn check_pcap_lib(found: Option<String>, windows: bool) -> Check {
    if windows {
        return Check::skip("pcap_lib");
    }
    match found {
        Some(p) => Check::new("pcap_lib", Status::Ok, "doctor-pcaplib-ok", p),
        None => Check::new(
            "pcap_lib",
            Status::Warn,
            "doctor-pcaplib-missing",
            String::new(),
        )
        .with_fix(
            "doctor-fix-pcaplib",
            "sudo apt install libpcap0.8   # or: brew install libpcap".to_owned(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(raw: Option<bool>, root: bool) -> RawAccess {
        RawAccess {
            privileged_user: root,
            cap_net_raw: raw,
            cap_net_admin: raw,
            bpf_writable: None,
        }
    }

    #[test]
    fn caps_check() {
        let ok = check_caps(&access(Some(true), false), "/x/wpg", false, None);
        assert_eq!(ok.status, Status::Ok);
        assert!(ok.fix_hint.is_none());
        let bad = check_caps(&access(Some(false), true), "/x/wpg", false, None);
        assert_eq!(bad.status, Status::Fail);
        assert_eq!(bad.summary_key, "doctor-caps-missing");
        assert!(bad.fix_hint.unwrap().contains("/x/wpg") || cfg!(windows));
    }

    #[test]
    fn caps_check_windows_follows_npcap_admin_only() {
        let unelevated = access(None, false);
        let user = check_caps(&unelevated, "wpg.exe", true, Some(false));
        assert_eq!(user.status, Status::Ok);
        assert_eq!(user.summary_key, "doctor-caps-npcap-user");
        assert!(user.detail.contains("npcap_admin_only=no"));
        let admin = check_caps(&unelevated, "wpg.exe", true, Some(true));
        assert_eq!(admin.status, Status::Fail);
        assert_eq!(admin.summary_key, "doctor-caps-npcap-admin");
        assert!(admin.fix_hint.is_some());
        let unknown = check_caps(&unelevated, "wpg.exe", true, None);
        assert_eq!(unknown.status, Status::Warn);
        assert_eq!(unknown.summary_key, "doctor-caps-missing");
        let elevated = check_caps(&access(None, true), "wpg.exe", true, Some(true));
        assert_eq!(elevated.status, Status::Ok);
        assert_eq!(elevated.summary_key, "doctor-caps-ok");
    }

    #[test]
    fn kernel_checks() {
        assert_eq!(
            check_af_xdp(Some("5.15.0-91-generic"), true).status,
            Status::Ok
        );
        let old = check_af_xdp(Some("4.19.0"), true);
        assert_eq!(old.status, Status::Warn);
        assert_eq!(old.summary_key, "doctor-af-xdp-old");
        assert_eq!(check_af_xdp(None, true).status, Status::Warn);
        assert_eq!(check_af_xdp(Some("5.15.0"), false).status, Status::Skip);
        assert_eq!(
            check_tpacket_v3(Some("2.6.32"), true).summary_key,
            "doctor-tpacket-v3-old"
        );
    }

    #[test]
    fn hugepage_checks() {
        let none = Hugepages::default();
        let c = check_hugepages(Some(&none), Some("eth0"));
        assert_eq!(c.status, Status::Warn);
        assert!(c.fix_hint.is_some());
        let some = Hugepages {
            meminfo: hugepages::parse_meminfo(
                "HugePages_Total: 8\nHugePages_Free: 8\nHugepagesize: 2048 kB\n",
            ),
            pools: vec![],
        };
        assert_eq!(check_hugepages(Some(&some), None).status, Status::Ok);
        assert_eq!(check_hugepages(None, None).status, Status::Skip);
    }

    #[test]
    fn npcap_checks() {
        assert_eq!(
            check_npcap(&NpcapInfo::default(), true).status,
            Status::Fail
        );
        let compat = NpcapInfo {
            registry_key: true,
            winpcap_compatible: true,
            ..NpcapInfo::default()
        };
        assert_eq!(check_npcap(&compat, true).status, Status::Ok);
        let non_compat = NpcapInfo {
            registry_key: true,
            wpcap_dll: Some("C:\\Windows\\System32\\Npcap\\wpcap.dll".into()),
            ..NpcapInfo::default()
        };
        assert_eq!(check_npcap(&non_compat, true).status, Status::Ok);
        let broken = NpcapInfo {
            registry_key: true,
            ..NpcapInfo::default()
        };
        assert_eq!(check_npcap(&broken, true).status, Status::Warn);
        assert_eq!(
            check_npcap(&NpcapInfo::default(), false).status,
            Status::Skip
        );
    }

    #[test]
    fn iface_and_pcap_lib_checks() {
        assert_eq!(check_ifaces(Ok(&[])).status, Status::Warn);
        assert_eq!(check_ifaces(Err("boom".into())).status, Status::Fail);
        assert_eq!(check_pcap_lib(None, false).status, Status::Warn);
        assert_eq!(check_pcap_lib(Some("/x".into()), false).status, Status::Ok);
        assert_eq!(check_pcap_lib(None, true).status, Status::Skip);
    }

    #[test]
    fn run_has_all_ids_and_serializes() {
        let r = run();
        for id in [
            "caps.raw_socket",
            "iface.any_up",
            "kernel.af_xdp",
            "kernel.tpacket_v3",
            "hugepages",
            "npcap",
            "pcap_lib",
        ] {
            assert!(r.get(id).is_some(), "missing {id}");
        }
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["checks"][0]["id"], "caps.raw_socket");
        assert!(matches!(
            json["checks"][0]["status"].as_str(),
            Some("ok" | "warn" | "fail")
        ));
    }
}
