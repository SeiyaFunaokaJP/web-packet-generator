//! `wpg doctor`: diagnose privileges, drivers, hugepages and Npcap (TODO §2.7).

use std::process::ExitCode;

use serde_json::{json, Value};
use wpg_i18n::{t, t_args, FluentArgs, Locale};
use wpg_sys::doctor::{self, Check, Report, Status};

use crate::cmd::cli_locale;

/// Runs `wpg doctor`. Exit status 1 if any check failed.
pub(crate) fn run(args: &crate::DoctorArgs) -> ExitCode {
    let locale = cli_locale();
    let report = doctor::run();
    if args.json {
        match serde_json::to_string_pretty(&to_json(locale, &report)) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print!("{}", render(locale, &report));
    }
    if report.has_failures() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn marker(status: Status) -> &'static str {
    match status {
        Status::Ok => "✓",
        Status::Warn => "!",
        Status::Fail => "✗",
        Status::Skip => "-",
    }
}

fn fix_text(locale: Locale, check: &Check) -> Option<String> {
    let key = check.fix_key?;
    let mut args = FluentArgs::new();
    args.set("hint", check.fix_hint.clone().unwrap_or_default());
    Some(t_args(locale, key, &args))
}

fn counts(report: &Report) -> (usize, usize, usize) {
    let n = |s: Status| report.checks.iter().filter(|c| c.status == s).count();
    (n(Status::Ok), n(Status::Warn), n(Status::Fail))
}

/// Human readable report.
pub(crate) fn render(locale: Locale, report: &Report) -> String {
    let mut out = format!("{}\n\n", t(locale, "doctor-title"));
    for c in &report.checks {
        out.push_str(&format!(
            "{} {}  [{}]\n",
            marker(c.status),
            t(locale, c.summary_key),
            c.id
        ));
        if !c.detail.is_empty() {
            out.push_str(&format!("    {}\n", c.detail));
        }
        if let Some(fix) = fix_text(locale, c) {
            out.push_str(&format!("    → {fix}\n"));
        }
    }
    let (ok, warn, fail) = counts(report);
    let mut args = FluentArgs::new();
    args.set("ok", ok);
    args.set("warn", warn);
    args.set("fail", fail);
    out.push_str(&format!("\n{}\n", t_args(locale, "doctor-summary", &args)));
    out
}

/// Machine readable report: the raw checks plus localised `summary` / `fix` texts.
pub(crate) fn to_json(locale: Locale, report: &Report) -> Value {
    let checks: Vec<Value> = report
        .checks
        .iter()
        .map(|c| {
            let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
            if let Value::Object(m) = &mut v {
                m.insert("summary".into(), Value::String(t(locale, c.summary_key)));
                m.insert(
                    "fix".into(),
                    fix_text(locale, c).map_or(Value::Null, Value::String),
                );
            }
            v
        })
        .collect();
    json!({ "ok": !report.has_failures(), "locale": locale.as_str(), "checks": checks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_summary_and_fix_key_is_translated() {
        // Enumerate keys by running the real checks plus the fixed key lists that depend
        // on the host (the ftl parity test covers en/ja equality).
        let report = doctor::run();
        for c in &report.checks {
            for locale in Locale::ALL {
                assert_ne!(t(locale, c.summary_key), c.summary_key, "{}", c.summary_key);
                if let Some(k) = c.fix_key {
                    assert_ne!(t(locale, k), k, "{k}");
                }
            }
        }
    }

    #[test]
    fn all_static_keys_exist() {
        for key in [
            "doctor-caps-ok",
            "doctor-caps-missing",
            "doctor-iface-ok",
            "doctor-iface-none",
            "doctor-iface-error",
            "doctor-af-xdp-ok",
            "doctor-af-xdp-old",
            "doctor-tpacket-v3-ok",
            "doctor-tpacket-v3-old",
            "doctor-hugepages-ok",
            "doctor-hugepages-none",
            "doctor-npcap-ok",
            "doctor-npcap-missing",
            "doctor-npcap-no-dll",
            "doctor-pcaplib-ok",
            "doctor-pcaplib-missing",
            "doctor-skip",
            "doctor-fix-caps",
            "doctor-fix-iface",
            "doctor-fix-hugepages",
            "doctor-fix-npcap",
            "doctor-fix-pcaplib",
        ] {
            for locale in Locale::ALL {
                assert_ne!(t(locale, key), key, "{key} missing in {locale}");
            }
        }
    }

    #[test]
    fn render_and_json() {
        let report = doctor::run();
        let text = render(Locale::En, &report);
        assert!(text.contains("[caps.raw_socket]"));
        let j = to_json(Locale::Ja, &report);
        assert_eq!(j["locale"], "ja");
        assert!(j["checks"][0]["summary"].is_string());
    }
}
