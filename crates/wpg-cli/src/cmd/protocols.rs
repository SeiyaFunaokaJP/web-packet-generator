//! User-defined protocols (`--protocols-dir`, `[protocols] dirs`): loads the
//! extension files and installs them into the process-wide catalog before a
//! subcommand runs (see [`wpg_protocols::extension`]).

use std::path::PathBuf;
use std::process::ExitCode;

use wpg_i18n::{t_args, FluentArgs, Locale};
use wpg_protocols::extension;

/// Loads and installs the extensions of `dirs` (nothing to do when empty).
/// Prints a localised message and returns exit status 2 on errors.
pub(crate) fn install(dirs: &[PathBuf], locale: Locale) -> Result<(), ExitCode> {
    if dirs.is_empty() {
        return Ok(());
    }
    let result = extension::load_dirs(dirs).and_then(|exts| {
        let count = exts.iter().map(|e| e.protocols.len()).sum::<usize>();
        extension::install(exts).map(|_| count)
    });
    match result {
        Ok(count) => {
            let mut a = FluentArgs::new();
            a.set("count", i64::try_from(count).unwrap_or(i64::MAX));
            tracing::info!("{}", t_args(locale, "protocols-ext-loaded", &a));
            Ok(())
        }
        Err(e) => {
            let mut a = FluentArgs::new();
            a.set("file", e.source);
            a.set("error", e.reason);
            eprintln!("{}", t_args(locale, "protocols-ext-error", &a));
            Err(ExitCode::from(2))
        }
    }
}
