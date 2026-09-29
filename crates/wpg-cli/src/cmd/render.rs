//! `wpg render` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg render`.
pub(crate) fn run(_args: crate::RenderArgs) -> ExitCode {
    not_implemented("render")
}
