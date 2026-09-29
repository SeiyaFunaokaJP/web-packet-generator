//! `wpg import` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg import`.
pub(crate) fn run(_args: crate::ImportArgs) -> ExitCode {
    not_implemented("import")
}
