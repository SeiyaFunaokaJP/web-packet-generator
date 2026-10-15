//! Subcommand implementations. Each submodule exposes `run(args) -> ExitCode`.

use wpg_i18n::{negotiate, Locale};

pub(crate) mod bench;
pub(crate) mod capture;
pub(crate) mod config;
pub(crate) mod docs;
pub(crate) mod doctor;
pub(crate) mod ifaces;
pub(crate) mod import;
pub(crate) mod protocols;
pub(crate) mod render;
pub(crate) mod send;
pub(crate) mod serve;
pub(crate) mod template;

/// CLI locale from `LC_ALL` / `LC_MESSAGES` / `LANG` (POSIX precedence).
pub(crate) fn cli_locale() -> Locale {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .find(|v| !v.is_empty())
        .map(|v| negotiate(&v))
        .unwrap_or_default()
}
