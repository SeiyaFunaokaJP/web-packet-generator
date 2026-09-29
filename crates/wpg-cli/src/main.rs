//! `wpg` — Web Packet Generator command line (TODO §2.7).
//!
//! `serve` runs the HTTP server; the other subcommands are declared with their
//! intended arguments but are not implemented yet (they exit with status 2).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use wpg_i18n::{negotiate, t_args, FluentArgs, Locale};

/// Exit status for subcommands that are declared but not implemented yet.
const EXIT_NOT_IMPLEMENTED: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "wpg",
    version,
    about = "Web Packet Generator",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start the web UI / API server.
    Serve {
        /// Address to listen on. Keep it on localhost until authentication lands.
        #[arg(long, default_value = "127.0.0.1:8080", env = "WPG_BIND")]
        bind: SocketAddr,
    },
    /// Send a template headlessly.
    Send {
        /// Template file (`.wpg.yaml`).
        template: PathBuf,
        /// Interface to transmit on.
        #[arg(short, long)]
        iface: Option<String>,
        /// Transmit backend (null, pcap, afpacket, afxdp, dpdk).
        #[arg(long)]
        backend: Option<String>,
        /// Number of packets.
        #[arg(long)]
        count: Option<u64>,
        /// Rate, e.g. `1Mpps`, `10Gbps`, `50%`.
        #[arg(long)]
        rate: Option<String>,
    },
    /// Render a template to a pcapng file without transmitting.
    Render {
        /// Template file.
        template: PathBuf,
        /// Output pcapng file.
        #[arg(short, long)]
        output: PathBuf,
        /// Number of packets.
        #[arg(long)]
        count: Option<u64>,
    },
    /// Convert a pcap/pcapng capture into a template.
    Import {
        /// Input capture.
        input: PathBuf,
        /// Output template.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Template tools (lint, list, export, import).
    Template {
        /// Action: lint | list | export | import.
        action: String,
        /// Files / arguments for the action.
        args: Vec<String>,
    },
    /// List network interfaces usable for transmission.
    Ifaces {
        /// Output JSON.
        #[arg(long)]
        json: bool,
    },
    /// Diagnose privileges, drivers, hugepages and Npcap.
    Doctor,
    /// Measure transmit performance.
    Bench {
        /// Backend to benchmark.
        #[arg(long)]
        backend: Option<String>,
    },
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Command::Serve { .. } => "serve",
            Command::Send { .. } => "send",
            Command::Render { .. } => "render",
            Command::Import { .. } => "import",
            Command::Template { .. } => "template",
            Command::Ifaces { .. } => "ifaces",
            Command::Doctor => "doctor",
            Command::Bench { .. } => "bench",
        }
    }
}

/// CLI locale from `LC_ALL` / `LC_MESSAGES` / `LANG` (POSIX precedence).
fn cli_locale() -> Locale {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .find(|v| !v.is_empty())
        .map(|v| negotiate(&v))
        .unwrap_or_default()
}

fn not_implemented(command: &str) -> ExitCode {
    let mut args = FluentArgs::new();
    args.set("command", format!("wpg {command}"));
    eprintln!("{}", t_args(cli_locale(), "error-not-implemented", &args));
    ExitCode::from(EXIT_NOT_IMPLEMENTED)
}

fn serve(bind: SocketAddr) -> ExitCode {
    wpg_server::init_tracing();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(wpg_server::run(bind)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("server error on {bind}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { bind } => serve(bind),
        // Interface enumeration lands in M2 (getifaddrs / GetAdaptersAddresses / Npcap).
        Command::Ifaces { .. } => not_implemented("ifaces"),
        other => not_implemented(other.name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn serve_default_bind() {
        let cli = Cli::try_parse_from(["wpg", "serve"]).unwrap();
        match cli.command {
            Command::Serve { bind } => assert_eq!(bind, "127.0.0.1:8080".parse().unwrap()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn all_subcommands_parse() {
        for argv in [
            &["wpg", "send", "t.wpg.yaml", "-i", "eth0", "--rate", "1Mpps"][..],
            &["wpg", "render", "t.wpg.yaml", "-o", "out.pcapng"],
            &["wpg", "import", "in.pcap", "-o", "t.wpg.yaml"],
            &["wpg", "template", "lint", "t.wpg.yaml"],
            &["wpg", "ifaces", "--json"],
            &["wpg", "doctor"],
            &["wpg", "bench", "--backend", "null"],
        ] {
            let cli = Cli::try_parse_from(argv).unwrap();
            assert_eq!(cli.command.name(), argv[1]);
        }
    }
}
