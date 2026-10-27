//! Pure logic of the SNMP screen (unit tested natively): request bodies from
//! the form, target profiles (without secrets) in `localStorage`, OID list /
//! varbind line parsing, counter rates, upload batching.

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::sparkline::{push_sample, SPARK_SAMPLES};

/// `localStorage` key of the saved target profiles.
pub(crate) const PROFILES_KEY: &str = "wpg.snmp.profiles";

/// Largest upload request (the server accepts 2 MiB JSON bodies).
pub(crate) const UPLOAD_BATCH_BYTES: usize = 1_500_000;

/// v3 security levels.
pub(crate) const LEVELS: [&str; 3] = ["noAuthNoPriv", "authNoPriv", "authPriv"];
/// Authentication protocols.
pub(crate) const AUTH_PROTOCOLS: [&str; 6] =
    ["MD5", "SHA", "SHA-224", "SHA-256", "SHA-384", "SHA-512"];
/// Privacy protocols.
pub(crate) const PRIV_PROTOCOLS: [&str; 4] = ["DES", "AES", "AES-192", "AES-256"];
/// Value types of Set / notification bindings: (code, i18n key).
pub(crate) const VALUE_TYPES: [(&str, &str); 13] = [
    ("auto", "ui-snmp-type-auto"),
    ("i", "ui-snmp-type-integer"),
    ("u", "ui-snmp-type-unsigned"),
    ("c", "ui-snmp-type-counter32"),
    ("C", "ui-snmp-type-counter64"),
    ("t", "ui-snmp-type-timeticks"),
    ("a", "ui-snmp-type-ipaddress"),
    ("o", "ui-snmp-type-oid"),
    ("s", "ui-snmp-type-string"),
    ("x", "ui-snmp-type-hex"),
    ("d", "ui-snmp-type-decimal"),
    ("n", "ui-snmp-type-null"),
    ("opaque", "ui-snmp-type-opaque"),
];

/// The target form's values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TargetValues {
    pub(crate) host: String,
    pub(crate) port: String,
    pub(crate) version: String,
    /// Not stored in profiles.
    #[serde(skip)]
    pub(crate) community: String,
    pub(crate) user: String,
    pub(crate) level: String,
    pub(crate) auth_protocol: String,
    /// Not stored in profiles.
    #[serde(skip)]
    pub(crate) auth_password: String,
    pub(crate) priv_protocol: String,
    /// Not stored in profiles.
    #[serde(skip)]
    pub(crate) priv_password: String,
    pub(crate) context_name: String,
    pub(crate) timeout_ms: String,
    pub(crate) retries: String,
}

impl Default for TargetValues {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: String::new(),
            version: "v2c".into(),
            community: "public".into(),
            user: String::new(),
            level: "authPriv".into(),
            auth_protocol: "SHA".into(),
            auth_password: String::new(),
            priv_protocol: "AES".into(),
            priv_password: String::new(),
            context_name: String::new(),
            timeout_ms: "2000".into(),
            retries: "1".into(),
        }
    }
}

/// Why the form cannot be sent (i18n key).
pub(crate) type Invalid = &'static str;

impl TargetValues {
    /// Field checks (`Err(i18n key)`).
    pub(crate) fn check(&self) -> Result<(), Invalid> {
        if self.host.trim().is_empty() {
            return Err("ui-snmp-need-host");
        }
        if !self.port.trim().is_empty() && self.port.trim().parse::<u16>().map_or(true, |p| p == 0)
        {
            return Err("ui-snmp-bad-port");
        }
        if self.version == "v3" {
            if self.user.trim().is_empty() {
                return Err("ui-snmp-need-user");
            }
            let need_auth = self.level != "noAuthNoPriv";
            let need_priv = self.level == "authPriv";
            if need_auth && self.auth_password.chars().count() < 8 {
                return Err("ui-snmp-short-password");
            }
            if need_priv && self.priv_password.chars().count() < 8 {
                return Err("ui-snmp-short-password");
            }
        }
        if !self.timeout_ms.trim().is_empty() && self.timeout_ms.trim().parse::<u64>().is_err() {
            return Err("ui-snmp-bad-number");
        }
        if !self.retries.trim().is_empty() && self.retries.trim().parse::<u32>().is_err() {
            return Err("ui-snmp-bad-number");
        }
        Ok(())
    }

    /// The `target` object of the API (`port_override` replaces the port,
    /// e.g. the notification receiver's).
    pub(crate) fn to_json(&self, port_override: Option<u16>) -> Value {
        let mut t = json!({
            "host": self.host.trim(),
            "version": self.version,
        });
        let port = port_override.or_else(|| self.port.trim().parse::<u16>().ok());
        if let Some(p) = port {
            t["port"] = json!(p);
        }
        if let Ok(ms) = self.timeout_ms.trim().parse::<u64>() {
            t["timeout_ms"] = json!(ms);
        }
        if let Ok(r) = self.retries.trim().parse::<u32>() {
            t["retries"] = json!(r);
        }
        if self.version == "v3" {
            let mut v3 = json!({ "user": self.user.trim() });
            if self.level != "noAuthNoPriv" {
                v3["auth_protocol"] = json!(self.auth_protocol);
                v3["auth_password"] = json!(self.auth_password);
            }
            if self.level == "authPriv" {
                v3["priv_protocol"] = json!(self.priv_protocol);
                v3["priv_password"] = json!(self.priv_password);
            }
            if !self.context_name.trim().is_empty() {
                v3["context_name"] = json!(self.context_name.trim());
            }
            t["v3"] = v3;
        } else {
            t["community"] = json!(self.community);
        }
        t
    }
}

/// A saved target (no community, no pass phrases).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) target: TargetValues,
}

/// Profiles from their JSON text (broken text = none).
pub(crate) fn parse_profiles(text: &str) -> Vec<Profile> {
    serde_json::from_str(text).unwrap_or_default()
}

/// Profiles as JSON text (secrets are `#[serde(skip)]`).
pub(crate) fn profiles_json(p: &[Profile]) -> String {
    serde_json::to_string(p).unwrap_or_else(|_| "[]".into())
}

/// Adds or replaces the profile `name`, sorted by name.
pub(crate) fn upsert_profile(list: &mut Vec<Profile>, name: &str, target: &TargetValues) {
    let name = name.trim().to_owned();
    list.retain(|p| p.name != name);
    list.push(Profile {
        name,
        target: target.clone(),
    });
    list.sort_by(|a, b| a.name.cmp(&b.name));
}

/// Applies a profile, keeping the current secrets.
pub(crate) fn apply_profile(current: &TargetValues, p: &Profile) -> TargetValues {
    TargetValues {
        community: current.community.clone(),
        auth_password: current.auth_password.clone(),
        priv_password: current.priv_password.clone(),
        ..p.target.clone()
    }
}

/// OIDs typed in one field, separated by spaces, commas or new lines.
pub(crate) fn parse_oid_list(s: &str) -> Vec<String> {
    s.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The word being typed at the end of `s` (for autocompletion) and what
/// comes before it.
pub(crate) fn split_last_token(s: &str) -> (&str, &str) {
    let idx = s
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace() || *c == ',')
        .map_or(0, |(i, c)| i + c.len_utf8());
    (&s[..idx], &s[idx..])
}

/// A binding typed as a line: `oid value` or `oid type value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VarBindLine {
    pub(crate) oid: String,
    pub(crate) value_type: String,
    pub(crate) value: String,
}

impl VarBindLine {
    pub(crate) fn to_json(&self) -> Value {
        json!({ "oid": self.oid, "type": self.value_type, "value": self.value })
    }
}

/// Parses binding lines (`#` comments and blank lines skipped). A second
/// word that is a type code makes the line typed; the value is the rest of
/// the line (surrounding quotes removed). Errors carry the 1-based line.
pub(crate) fn parse_varbind_lines(text: &str) -> Result<Vec<VarBindLine>, usize> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let (oid, rest) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
        let rest = rest.trim_start();
        let (ty, value) = match rest.split_once(char::is_whitespace) {
            Some((t, v)) if VALUE_TYPES.iter().any(|(c, _)| *c == t) => (t, v.trim()),
            _ if VALUE_TYPES.iter().any(|(c, _)| *c == rest) && rest == "n" => ("n", ""),
            _ => ("auto", rest),
        };
        if oid.is_empty() || (value.is_empty() && ty != "n") {
            return Err(n + 1);
        }
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        out.push(VarBindLine {
            oid: oid.to_owned(),
            value_type: ty.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(out)
}

/// Per-OID counter rates between polls.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Rates {
    last: HashMap<String, (u64, f64)>,
    pub(crate) rate: HashMap<String, f64>,
    pub(crate) history: HashMap<String, VecDeque<f64>>,
}

impl Rates {
    /// Records `value` of counter `key` at `t_ms`; returns the rate per second
    /// since the previous sample (a Counter32 wrap is accounted for; a
    /// Counter64 going backwards — a reset — starts over).
    pub(crate) fn update(
        &mut self,
        key: &str,
        value: u64,
        t_ms: f64,
        counter32: bool,
    ) -> Option<f64> {
        let prev = self.last.insert(key.to_owned(), (value, t_ms));
        let (pv, pt) = prev?;
        let dt = (t_ms - pt) / 1000.0;
        if dt <= 0.0 {
            return None;
        }
        let delta = if value >= pv {
            value - pv
        } else if counter32 && pv <= u64::from(u32::MAX) {
            value + (1u64 << 32) - pv
        } else {
            self.rate.remove(key);
            return None;
        };
        #[allow(clippy::cast_precision_loss)] // display only
        let r = delta as f64 / dt;
        self.rate.insert(key.to_owned(), r);
        push_sample(
            self.history.entry(key.to_owned()).or_default(),
            r,
            SPARK_SAMPLES,
        );
        Some(r)
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Splits files into upload requests of at most [`UPLOAD_BATCH_BYTES`]
/// (a larger single file still goes alone; the server judges it).
pub(crate) fn batches<T: Clone>(files: &[(T, usize)]) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = Vec::new();
    let mut size = 0;
    for (f, len) in files {
        if out.is_empty() || size + len > UPLOAD_BATCH_BYTES && size > 0 {
            out.push(Vec::new());
            size = 0;
        }
        size += len;
        if let Some(last) = out.last_mut() {
            last.push(f.clone());
        }
    }
    out
}

/// A rate per second in short form (`1.25 k/s`).
pub(crate) fn rate_text(r: f64) -> String {
    crate::fmt::si(r, "/s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_json_and_checks() {
        let mut t = TargetValues {
            host: " 192.0.2.1 ".into(),
            ..TargetValues::default()
        };
        assert_eq!(t.check(), Ok(()));
        let j = t.to_json(None);
        assert_eq!(j["host"], "192.0.2.1");
        assert_eq!(j["community"], "public");
        assert!(j.get("port").is_none());
        assert_eq!(t.to_json(Some(162))["port"], 162);
        t.version = "v3".into();
        assert_eq!(t.check(), Err("ui-snmp-need-user"));
        t.user = "ops".into();
        assert_eq!(t.check(), Err("ui-snmp-short-password"));
        t.auth_password = "authpass1".into();
        t.priv_password = "privpass1".into();
        assert_eq!(t.check(), Ok(()));
        let j = t.to_json(None);
        assert_eq!(j["v3"]["priv_protocol"], "AES");
        assert!(j.get("community").is_none());
        t.level = "authNoPriv".into();
        assert!(t.to_json(None)["v3"].get("priv_password").is_none());
        t.port = "70000".into();
        assert_eq!(t.check(), Err("ui-snmp-bad-port"));
        t.host = String::new();
        assert_eq!(t.check(), Err("ui-snmp-need-host"));
    }

    #[test]
    fn profiles_have_no_secrets() {
        let t = TargetValues {
            host: "rtr1".into(),
            community: "s3cret".into(),
            auth_password: "authpass1".into(),
            priv_password: "privpass1".into(),
            version: "v3".into(),
            user: "ops".into(),
            ..TargetValues::default()
        };
        let mut list = Vec::new();
        upsert_profile(&mut list, "core", &t);
        upsert_profile(&mut list, "core", &t);
        upsert_profile(&mut list, "access", &t);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "access");
        let text = profiles_json(&list);
        assert!(
            !text.contains("s3cret") && !text.contains("authpass1") && !text.contains("privpass1"),
            "{text}"
        );
        let back = parse_profiles(&text);
        assert_eq!(back[1].target.host, "rtr1");
        assert_eq!(
            back[1].target.community, "public",
            "default, not the secret"
        );
        let current = TargetValues {
            community: "mine".into(),
            ..TargetValues::default()
        };
        let applied = apply_profile(&current, &back[1]);
        assert_eq!(applied.community, "mine");
        assert_eq!(applied.user, "ops");
        assert!(parse_profiles("garbage").is_empty());
    }

    #[test]
    fn oid_lists_and_lines() {
        assert_eq!(
            parse_oid_list("sysDescr.0, sysUpTime.0\nifDescr"),
            ["sysDescr.0", "sysUpTime.0", "ifDescr"]
        );
        assert_eq!(split_last_token("sysDescr.0 ifDe"), ("sysDescr.0 ", "ifDe"));
        assert_eq!(split_last_token("ifDe"), ("", "ifDe"));
        let l = parse_varbind_lines("# comment\nifAdminStatus.2 down\nsysName.0 s \"core router\"\n1.3.6.1.4.1.1.0 u 5\nx.0 n\n").unwrap();
        assert_eq!(l.len(), 4);
        assert_eq!(l[0].value_type, "auto");
        assert_eq!(l[0].value, "down");
        assert_eq!(l[1].value_type, "s");
        assert_eq!(l[1].value, "core router");
        assert_eq!(l[2].value_type, "u");
        assert_eq!(l[3].value_type, "n");
        assert_eq!(parse_varbind_lines("ok.0 1\nbroken\n"), Err(2));
    }

    #[test]
    fn counter_rates() {
        let mut r = Rates::default();
        assert_eq!(r.update("a", 100, 0.0, true), None);
        assert_eq!(r.update("a", 300, 2000.0, true), Some(100.0));
        // Counter32 wrap.
        assert_eq!(
            r.update("a", 10, 3000.0, true),
            Some(((1u64 << 32) - 290) as f64)
        );
        assert_eq!(r.history["a"].len(), 2);
        // Counter64 reset.
        assert_eq!(r.update("b", 50, 0.0, false), None);
        assert_eq!(r.update("b", 10, 1000.0, false), None);
        assert_eq!(r.update("b", 20, 2000.0, false), Some(10.0));
        r.clear();
        assert!(r.rate.is_empty());
    }

    #[test]
    fn upload_batches() {
        let b = batches(&[
            ("a", 1_000_000),
            ("b", 400_000),
            ("c", 200_000),
            ("d", 3_000_000),
            ("e", 1),
        ]);
        assert_eq!(b, vec![vec!["a", "b"], vec!["c"], vec!["d"], vec!["e"]]);
        assert!(batches::<&str>(&[]).is_empty());
    }
}
