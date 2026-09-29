//! `wpg config init|show|validate` (TODO §2.8 "`wpg config init`").

use std::path::Path;
use std::process::ExitCode;

use wpg_i18n::{t, Locale};
use wpg_server::config::{ConfigIssue, IssueLevel};
use wpg_server::{Config, Overrides};

use crate::cmd::cli_locale;
use crate::cmd::send::tr;
use crate::cmd::serve::config_source;
use crate::{ConfigAction, ConfigArgs};

/// Runs `wpg config`.
#[allow(clippy::needless_pass_by_value)] // uniform `cmd::*::run(args)` dispatch
pub(crate) fn run(args: ConfigArgs) -> ExitCode {
    let locale = cli_locale();
    match args.action {
        ConfigAction::Init { path, force } => init(&path, force, locale),
        ConfigAction::Show { config } => match load(config.as_deref(), locale) {
            Ok(c) => match c.to_toml_redacted() {
                Ok(s) => {
                    print!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            Err(code) => code,
        },
        ConfigAction::Validate { config } => {
            let c = match load(config.as_deref(), locale) {
                Ok(c) => c,
                Err(code) => return code,
            };
            let source = config_source(config.as_deref(), locale);
            let issues = c.validate();
            for i in &issues {
                println!("{}", issue_line(i, locale));
            }
            let errors = issues
                .iter()
                .filter(|i| i.level == IssueLevel::Error)
                .count();
            if issues.is_empty() {
                println!(
                    "{}",
                    tr(locale, "config-validate-ok", &[("source", source)])
                );
            } else {
                println!(
                    "{}",
                    tr(
                        locale,
                        "config-validate-summary",
                        &[
                            ("errors", errors.to_string()),
                            ("warnings", (issues.len() - errors).to_string()),
                            ("source", source),
                        ],
                    )
                );
            }
            if errors > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
    }
}

fn load(file: Option<&Path>, locale: Locale) -> Result<Config, ExitCode> {
    Config::load(file, &Overrides::default()).map_err(|e| {
        eprintln!(
            "{}",
            tr(locale, "config-load-error", &[("error", e.to_string())])
        );
        ExitCode::FAILURE
    })
}

/// One localised finding.
pub(crate) fn issue_line(i: &ConfigIssue, locale: Locale) -> String {
    let level = t(
        locale,
        match i.level {
            IssueLevel::Error => "config-level-error",
            IssueLevel::Warning => "config-level-warning",
        },
    );
    let detail = if i.detail.is_empty() {
        String::new()
    } else {
        format!(" ({})", i.detail)
    };
    tr(
        locale,
        "config-issue-line",
        &[
            ("level", level),
            ("key", i.key.clone()),
            ("message", t(locale, &i.message_key)),
            ("detail", detail),
        ],
    )
}

fn init(path: &Path, force: bool, locale: Locale) -> ExitCode {
    let shown = path.display().to_string();
    if path.exists() && !force {
        eprintln!("{}", tr(locale, "config-init-exists", &[("path", shown)]));
        return ExitCode::FAILURE;
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("{}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    }
    match std::fs::write(path, Config::example_toml()) {
        Ok(()) => {
            println!("{}", tr(locale, "config-init-written", &[("path", shown)]));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{shown}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_issue_key_is_translated() {
        let mut c = Config::default();
        c.server.bind = "0.0.0.0:1".parse().unwrap();
        c.server.base_url = Some("ftp://x".into());
        c.server.tls = Some(wpg_server::config::TlsConfig {
            cert: "/nope/c".into(),
            key: "/nope/k".into(),
        });
        c.auth.password_hash = Some("plain".into());
        c.auth.password_file = Some("/nope/p".into());
        c.auth.session_ttl_secs = 0;
        c.templates.library_paths = vec!["/nope/lib".into()];
        c.tx.max_rate = Some("50%".into());
        c.tx.default_backend = Some("warp".into());
        c.log.level = "[".into();
        let issues = c.validate();
        assert!(issues.len() >= 10, "{issues:?}");
        for i in &issues {
            for locale in Locale::ALL {
                assert_ne!(t(locale, &i.message_key), i.message_key);
                assert!(
                    !issue_line(i, locale).contains("config-"),
                    "{}",
                    i.message_key
                );
            }
        }
    }
}
