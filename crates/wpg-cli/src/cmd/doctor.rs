//! `wpg doctor` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg doctor`.
pub(crate) fn run(_args: crate::DoctorArgs) -> ExitCode {
    not_implemented("doctor")
}
