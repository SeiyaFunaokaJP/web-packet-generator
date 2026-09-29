//! `wpg send` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg send`.
pub(crate) fn run(_args: crate::SendArgs) -> ExitCode {
    not_implemented("send")
}
