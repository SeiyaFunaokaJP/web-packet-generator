//! `wpg config token create|list|revoke`: API tokens in the local database
//! (`<data_dir>/wpg.sqlite`), for scripts and `wpg remote`. Works while the
//! server runs (`SQLite` WAL); revoking takes effect on the next request.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};
use wpg_i18n::{t, Locale};
use wpg_server::auth::token::{self, TokenError};
use wpg_server::db::{Db, TokenInfo};
use wpg_server::{Config, Overrides};

use crate::cmd::cli_locale;
use crate::cmd::send::tr;

/// Arguments of `wpg config token`.
#[derive(Debug, Args)]
pub(crate) struct TokenArgs {
    /// Data directory holding `wpg.sqlite` (default: `data_dir` of the configuration).
    #[arg(long, value_name = "DIR", global = true)]
    pub(crate) data_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) action: TokenAction,
}

/// Actions of `wpg config token`.
#[derive(Debug, Subcommand)]
pub(crate) enum TokenAction {
    /// Create a token and print its secret (shown only once).
    Create {
        /// Name, to recognise the token later.
        name: String,
        /// User the token acts as (with that user's role).
        #[arg(long, default_value = wpg_server::auth::ADMIN)]
        user: String,
        /// Expire after this many days (default: never).
        #[arg(long, value_name = "DAYS")]
        expires_days: Option<u32>,
        /// Print the token and its secret as JSON.
        #[arg(long)]
        json: bool,
    },
    /// List the tokens (never their secrets).
    List {
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Revoke (delete) a token by id.
    Revoke {
        /// Token id (see `wpg config token list`).
        id: i64,
    },
}

/// Runs `wpg config token`.
pub(crate) fn run(args: &TokenArgs, config: Option<&Path>) -> ExitCode {
    let locale = cli_locale();
    let db = match open(config, args.data_dir.as_deref(), locale) {
        Ok(db) => db,
        Err(code) => return code,
    };
    let fail = |e: &dyn std::fmt::Display| {
        eprintln!(
            "{}",
            tr(locale, "config-load-error", &[("error", e.to_string())])
        );
        ExitCode::FAILURE
    };
    match &args.action {
        TokenAction::Create {
            name,
            user,
            expires_days,
            json,
        } => {
            let u = match db.user_by_name(user) {
                Ok(Some(u)) => u,
                Ok(None) => {
                    eprintln!(
                        "{}",
                        tr(locale, "config-token-no-user", &[("user", user.clone())])
                    );
                    return ExitCode::FAILURE;
                }
                Err(e) => return fail(&e),
            };
            let now = wpg_server::auth::now_secs();
            let (info, secret) = match token::create(&db, &u, name, *expires_days, now) {
                Ok(v) => v,
                Err(TokenError::Name) => {
                    eprintln!(
                        "{}",
                        tr(
                            locale,
                            "auth-error-token-name",
                            &[("max", token::MAX_NAME.to_string())]
                        )
                    );
                    return ExitCode::FAILURE;
                }
                Err(TokenError::Expiry) => {
                    eprintln!(
                        "{}",
                        tr(
                            locale,
                            "auth-error-token-expiry",
                            &[("max", token::MAX_DAYS.to_string())]
                        )
                    );
                    return ExitCode::FAILURE;
                }
                Err(e) => return fail(&e),
            };
            if *json {
                let v = serde_json::json!({ "token": info, "secret": secret });
                println!("{v:#}");
            } else {
                println!("{secret}");
                eprintln!(
                    "{}",
                    tr(
                        locale,
                        "config-token-created",
                        &[("id", info.id.to_string()), ("user", info.user)]
                    )
                );
            }
            ExitCode::SUCCESS
        }
        TokenAction::List { json } => {
            let tokens = match db.tokens(None) {
                Ok(t) => t,
                Err(e) => return fail(&e),
            };
            if *json {
                println!(
                    "{:#}",
                    serde_json::to_value(&tokens).unwrap_or(serde_json::Value::Null)
                );
            } else if tokens.is_empty() {
                println!("{}", t(locale, "config-token-none"));
            } else {
                for i in &tokens {
                    println!("{}", line(i, locale));
                }
            }
            ExitCode::SUCCESS
        }
        TokenAction::Revoke { id } => match db.delete_token(*id, None) {
            Ok(true) => {
                println!(
                    "{}",
                    tr(locale, "config-token-revoked", &[("id", id.to_string())])
                );
                ExitCode::SUCCESS
            }
            Ok(false) => {
                eprintln!(
                    "{}",
                    tr(locale, "config-token-not-found", &[("id", id.to_string())])
                );
                ExitCode::FAILURE
            }
            Err(e) => fail(&e),
        },
    }
}

fn open(config: Option<&Path>, data_dir: Option<&Path>, locale: Locale) -> Result<Db, ExitCode> {
    let overrides = Overrides {
        data_dir: data_dir.map(Path::to_path_buf),
        ..Overrides::default()
    };
    let path = Config::load(config, &overrides)
        .map_err(|e| {
            eprintln!(
                "{}",
                tr(locale, "config-load-error", &[("error", e.to_string())])
            );
            ExitCode::from(2)
        })?
        .db_path();
    if !path.is_file() {
        eprintln!(
            "{}",
            tr(
                locale,
                "config-token-no-db",
                &[("path", path.display().to_string())]
            )
        );
        return Err(ExitCode::FAILURE);
    }
    Db::open(&path).map_err(|e| {
        eprintln!(
            "{}",
            tr(locale, "config-load-error", &[("error", e.to_string())])
        );
        ExitCode::FAILURE
    })
}

fn when(secs: Option<i64>, locale: Locale) -> String {
    secs.and_then(|s| u64::try_from(s).ok()).map_or_else(
        || t(locale, "config-token-never"),
        wpg_server::terminal::rfc3339,
    )
}

/// One token in `wpg config token list`.
fn line(i: &TokenInfo, locale: Locale) -> String {
    tr(
        locale,
        "config-token-line",
        &[
            ("id", i.id.to_string()),
            ("name", i.name.clone()),
            ("prefix", i.prefix.clone()),
            ("user", i.user.clone()),
            ("created", when(Some(i.created), locale)),
            ("used", when(i.last_used, locale)),
            ("expires", when(i.expires, locale)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_line() {
        let i = TokenInfo {
            id: 3,
            name: "ci".into(),
            prefix: "wpg_0123abcd".into(),
            user: "admin".into(),
            created: 0,
            last_used: None,
            expires: Some(86_400),
        };
        let s = line(&i, Locale::En);
        for part in [
            "3",
            "ci",
            "wpg_0123abcd",
            "admin",
            "1970-01-01T00:00:00Z",
            "1970-01-02T00:00:00Z",
        ] {
            assert!(s.contains(part), "{s}");
        }
        assert!(!s.contains("config-token"), "{s}");
    }
}
