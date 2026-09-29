//! `wpg` — Web Packet Generator command line (TODO §2.7).
//!
//! `serve` runs the HTTP server; the other subcommands live in [`cmd`]. Ones that
//! are not implemented yet exit with status 2.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

mod cmd;

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
    Serve(ServeArgs),
    /// Send a template headlessly.
    Send(SendArgs),
    /// Render a template to a pcapng file without transmitting.
    Render(RenderArgs),
    /// Convert a pcap/pcapng capture into a template.
    Import(ImportArgs),
    /// Template tools (lint, list, export, import).
    Template(TemplateArgs),
    /// List network interfaces usable for transmission.
    Ifaces(IfacesArgs),
    /// Diagnose privileges, drivers, hugepages and Npcap.
    Doctor(DoctorArgs),
    /// Measure transmit performance.
    Bench(BenchArgs),
}

/// Arguments of `wpg serve`.
#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// Address to listen on. Keep it on localhost until authentication lands.
    #[arg(long, default_value = "127.0.0.1:8080", env = "WPG_BIND")]
    pub(crate) bind: SocketAddr,
}

/// Arguments of `wpg send`.
#[derive(Debug, Args)]
pub(crate) struct SendArgs {
    /// Template file (`.wpg.yaml`).
    pub(crate) template: PathBuf,
    /// Interface to transmit on.
    #[arg(short, long)]
    pub(crate) iface: Option<String>,
    /// Transmit backend (null, pcapfile, pcap, afpacket, afxdp, dpdk).
    #[arg(long)]
    pub(crate) backend: Option<String>,
    /// Number of packets.
    #[arg(long)]
    pub(crate) count: Option<u64>,
    /// Duration limit, e.g. `10s`, `2m`.
    #[arg(long)]
    pub(crate) duration: Option<String>,
    /// Rate, e.g. `1Mpps`, `10Gbps`, `50%`, `ifg=10us`, `unlimited`.
    #[arg(long)]
    pub(crate) rate: Option<String>,
    /// Frames sent back to back per rate-control grant.
    #[arg(long)]
    pub(crate) burst: Option<u32>,
    /// Ramp the rate up linearly from 0 over this time, e.g. `5s`.
    #[arg(long, value_name = "DURATION")]
    pub(crate) ramp_up: Option<String>,
    /// Start at this time: RFC 3339 (`2026-09-29T12:00:00Z`) or a delay (`+10s`).
    #[arg(long, value_name = "TIME")]
    pub(crate) start_at: Option<String>,
    /// Expansion strategy: auto, prerender or onthefly.
    #[arg(long, default_value = "auto")]
    pub(crate) plan: String,
    /// Print the final report as JSON on stdout.
    #[arg(long)]
    pub(crate) json: bool,
    /// Template variable override (`name=value`), repeatable.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) vars: Vec<String>,
    #[command(flatten)]
    pub(crate) tuning: cmd::send::TxTuningArgs,
}

/// Arguments of `wpg render`.
#[derive(Debug, Args)]
pub(crate) struct RenderArgs {
    /// Template file.
    pub(crate) template: PathBuf,
    /// Output pcapng file.
    #[arg(short, long)]
    pub(crate) output: PathBuf,
    /// Number of packets.
    #[arg(long)]
    pub(crate) count: Option<u64>,
    /// Template variable override (`name=value`), repeatable.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) vars: Vec<String>,
}

/// Arguments of `wpg import`.
#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    /// Input capture.
    pub(crate) input: PathBuf,
    /// Output template.
    #[arg(short, long)]
    pub(crate) output: PathBuf,
}

/// Arguments of `wpg template`.
#[derive(Debug, Args)]
pub(crate) struct TemplateArgs {
    #[command(subcommand)]
    pub(crate) action: TemplateAction,
}

/// Actions of `wpg template`.
#[derive(Debug, Subcommand)]
pub(crate) enum TemplateAction {
    /// Check templates for errors and warnings (exit status 1 on errors).
    Lint {
        /// Template files.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// List the templates of a directory (default: `templates/`).
    List {
        /// Directory to scan recursively.
        dir: Option<PathBuf>,
    },
    /// Print the built packet (hex dump and layout tree) of a template.
    Show {
        /// Template file.
        file: PathBuf,
        /// Template variable override (`name=value`), repeatable.
        #[arg(long = "var", value_name = "NAME=VALUE")]
        vars: Vec<String>,
    },
    /// Print the JSON Schema of the template format.
    Schema,
    /// Export templates (not implemented yet).
    Export {
        /// Arguments.
        args: Vec<String>,
    },
    /// Import templates (not implemented yet).
    Import {
        /// Arguments.
        args: Vec<String>,
    },
}

/// Arguments of `wpg ifaces`.
#[derive(Debug, Args)]
pub(crate) struct IfacesArgs {
    /// Output JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

/// Arguments of `wpg doctor`.
#[derive(Debug, Args)]
pub(crate) struct DoctorArgs {
    /// Output JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

/// Arguments of `wpg bench`.
#[derive(Debug, Args)]
pub(crate) struct BenchArgs {
    /// Backend to benchmark.
    #[arg(long)]
    pub(crate) backend: Option<String>,
    /// Interface (for real backends).
    #[arg(short, long)]
    pub(crate) iface: Option<String>,
    /// Duration in seconds.
    #[arg(long, default_value_t = 5.0)]
    pub(crate) duration: f64,
    /// Frame size in bytes (without FCS).
    #[arg(long, default_value_t = 64)]
    pub(crate) frame_size: usize,
    /// Benchmark on-the-fly generation of this template instead of a fixed frame.
    #[arg(long, value_name = "FILE")]
    pub(crate) template: Option<PathBuf>,
    /// Template variable override (`name=value`), repeatable (with --template).
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) vars: Vec<String>,
    /// Output JSON.
    #[arg(long)]
    pub(crate) json: bool,
    #[command(flatten)]
    pub(crate) tuning: cmd::send::TxTuningArgs,
}

#[cfg(test)]
impl Command {
    fn name(&self) -> &'static str {
        match self {
            Command::Serve(_) => "serve",
            Command::Send(_) => "send",
            Command::Render(_) => "render",
            Command::Import(_) => "import",
            Command::Template(_) => "template",
            Command::Ifaces(_) => "ifaces",
            Command::Doctor(_) => "doctor",
            Command::Bench(_) => "bench",
        }
    }
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
        Command::Serve(args) => serve(args.bind),
        Command::Send(args) => cmd::send::run(args),
        Command::Render(args) => cmd::render::run(&args),
        Command::Import(args) => cmd::import::run(args),
        Command::Template(args) => cmd::template::run(args),
        Command::Ifaces(args) => cmd::ifaces::run(&args),
        Command::Doctor(args) => cmd::doctor::run(&args),
        Command::Bench(args) => cmd::bench::run(args),
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
            Command::Serve(a) => assert_eq!(a.bind, "127.0.0.1:8080".parse().unwrap()),
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
            &[
                "wpg",
                "bench",
                "--backend",
                "afpacket",
                "-i",
                "lo",
                "--batch",
                "128",
                "--no-tx-ring",
                "--qdisc-bypass",
                "--pin-core",
                "1",
            ],
            &[
                "wpg",
                "send",
                "t.wpg.yaml",
                "--backend",
                "null",
                "--burst",
                "8",
                "--plan",
                "onthefly",
                "--duration",
                "2s",
                "--json",
            ],
        ] {
            let cli = Cli::try_parse_from(argv).unwrap();
            assert_eq!(cli.command.name(), argv[1]);
        }
    }
}
