//! `wpg bench` subcommand.

use std::process::ExitCode;

use crate::cmd::not_implemented;

/// Runs `wpg bench`.
pub(crate) fn run(_args: crate::BenchArgs) -> ExitCode {
    not_implemented("bench")
}
