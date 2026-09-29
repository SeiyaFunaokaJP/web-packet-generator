//! Subcommand implementations. Each submodule exposes `run(args) -> ExitCode`.

use std::process::ExitCode;

use wpg_i18n::{negotiate, t_args, FluentArgs, Locale};

pub(crate) mod bench;
pub(crate) mod doctor;
pub(crate) mod ifaces;
pub(crate) mod import;
pub(crate) mod render;
pub(crate) mod send;
pub(crate) mod template;

/// Exit status for subcommands that are declared but not implemented yet.
pub(crate) const EXIT_NOT_IMPLEMENTED: u8 = 2;

/// CLI locale from `LC_ALL` / `LC_MESSAGES` / `LANG` (POSIX precedence).
pub(crate) fn cli_locale() -> Locale {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .find(|v| !v.is_empty())
        .map(|v| negotiate(&v))
        .unwrap_or_default()
}

/// Prints the localised "not implemented" message and returns the exit code.
pub(crate) fn not_implemented(command: &str) -> ExitCode {
    let mut args = FluentArgs::new();
    args.set("command", format!("wpg {command}"));
    eprintln!("{}", t_args(cli_locale(), "error-not-implemented", &args));
    ExitCode::from(EXIT_NOT_IMPLEMENTED)
}
