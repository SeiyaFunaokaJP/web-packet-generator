//! `wpg template` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg template`.
pub(crate) fn run(_args: crate::TemplateArgs) -> ExitCode {
    not_implemented("template")
}
