//! `wpg ifaces` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg ifaces`.
pub(crate) fn run(_args: crate::IfacesArgs) -> ExitCode {
    not_implemented("ifaces")
}
