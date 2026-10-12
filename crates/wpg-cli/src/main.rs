//! `wpg` — Web Packet Generator command line (TODO §2.7).
//!
//! Every subcommand lives in [`cmd`] (`serve`: the HTTP server, `config`:
//! `wpg.toml` tools, ...). Ones that are not implemented yet exit with status 2.

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
    /// Template tools (lint, list, migrate, export, import).
    Template(TemplateArgs),
    /// List network interfaces usable for transmission.
    Ifaces(IfacesArgs),
    /// Diagnose privileges, drivers, hugepages and Npcap.
    Doctor(DoctorArgs),
    /// Measure transmit performance.
    Bench(BenchArgs),
    /// Configuration file tools (init, show, validate).
    Config(ConfigArgs),
}

/// Arguments of `wpg serve`.
#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// Address to listen on (default from the config: 127.0.0.1:8080). Without a
    /// password only loopback addresses are accepted (see --allow-insecure).
    #[arg(long, env = "WPG_BIND")]
    pub(crate) bind: Option<SocketAddr>,
    /// Admin password (plain text or argon2 hash). Visible in the process list:
    /// prefer --password-file, `WPG_PASSWORD` or `wpg config set-password`.
    #[arg(long, value_name = "PASSWORD")]
    pub(crate) password: Option<String>,
    /// File holding the admin password (plain text or argon2 hash).
    #[arg(long, value_name = "FILE")]
    pub(crate) password_file: Option<PathBuf>,
    /// Serve HTTPS; without `[server.tls]` a self-signed certificate is generated
    /// into `<data_dir>/tls/`.
    #[arg(long)]
    pub(crate) tls: bool,
    /// Listen on a non-loopback address even though no password is configured
    /// (anyone reaching the server can then set it on the setup screen).
    #[arg(long)]
    pub(crate) allow_insecure: bool,
    /// Configuration file (default: `wpg.toml` in the working directory, if any).
    #[arg(long, env = "WPG_CONFIG", value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
    /// Development mode: allow CORS from the Trunk dev server (:8081).
    #[arg(long)]
    pub(crate) dev: bool,
    /// Template library directory (repeatable; replaces `templates.library_paths`).
    #[arg(long = "library-path", value_name = "DIR")]
    pub(crate) library_paths: Vec<PathBuf>,
    /// Data directory (user templates, history).
    #[arg(long, value_name = "DIR")]
    pub(crate) data_dir: Option<PathBuf>,
}

/// Arguments of `wpg config`.
#[derive(Debug, Args)]
pub(crate) struct ConfigArgs {
    #[command(subcommand)]
    pub(crate) action: ConfigAction,
}

/// Actions of `wpg config`.
#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
    /// Write a fully commented example `wpg.toml`.
    Init {
        /// Output file.
        #[arg(long, default_value = "wpg.toml", value_name = "FILE")]
        path: PathBuf,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Print the effective configuration (file + environment) as TOML, secrets
    /// redacted.
    Show {
        /// Configuration file (default: `wpg.toml` if present).
        #[arg(long, env = "WPG_CONFIG", value_name = "FILE")]
        config: Option<PathBuf>,
    },
    /// Check the configuration; exit status 1 on errors.
    Validate {
        /// Configuration file (default: `wpg.toml` if present).
        #[arg(long, env = "WPG_CONFIG", value_name = "FILE")]
        config: Option<PathBuf>,
    },
    /// Prompt for the admin password and store its argon2id hash as
    /// `auth.password_hash` (the file is created from the example if missing).
    SetPassword {
        /// Configuration file to update.
        #[arg(
            long,
            env = "WPG_CONFIG",
            default_value = "wpg.toml",
            value_name = "FILE"
        )]
        config: PathBuf,
        /// Read the password from the first line of standard input (no prompt).
        #[arg(long)]
        stdin: bool,
    },
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
    /// Input: a `.pcap` / `.pcapng` capture, a hex dump (`.hex`, see `--hex`) or `-`
    /// for standard input.
    pub(crate) input: PathBuf,
    /// Output template (`-` prints YAML on standard output).
    #[arg(short, long, required_unless_present = "per_packet")]
    pub(crate) output: Option<PathBuf>,
    /// Write one template per packet into this directory instead of one merged template.
    #[arg(long, value_name = "DIR", conflicts_with = "output")]
    pub(crate) per_packet: Option<PathBuf>,
    /// Protocol the frames start with.
    #[arg(long, default_value = "ethernet2", value_name = "PROTOCOL")]
    pub(crate) first_protocol: String,
    /// Read at most this many packets.
    #[arg(long, value_name = "N")]
    pub(crate) max_packets: Option<usize>,
    /// Do not infer generators (range / list / random) from the packets.
    #[arg(long)]
    pub(crate) no_detect_generators: bool,
    /// Treat the input as pasted hex (Wireshark hex stream, spaced bytes, hexdump).
    #[arg(long)]
    pub(crate) hex: bool,
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
    /// Pack templates (files or ids) into a `.wpgbundle` archive.
    Export {
        /// Template files, or ids (file stem / name slug) looked up in `--dir`.
        #[arg(required = true)]
        templates: Vec<String>,
        /// Output bundle.
        #[arg(short, long)]
        output: PathBuf,
        /// Attach a capture (pcap / pcapng) or other file, repeatable.
        #[arg(long, value_name = "FILE")]
        attach: Vec<PathBuf>,
        /// Template library used to resolve ids (default: `templates/`).
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
    },
    /// Upgrade templates to the current schema version.
    ///
    /// Directories are scanned recursively. The original of a rewritten file is kept
    /// as `<file>.bak`; the new file loses its comments. Files already at the current
    /// version are not touched.
    Migrate {
        /// Template files or directories.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Only report outdated templates (exit status 1 if any needs migrating).
        #[arg(long)]
        check: bool,
        /// Do not keep a `.bak` copy of migrated files.
        #[arg(long)]
        no_backup: bool,
    },
    /// Extract the templates of a `.wpgbundle` archive into a directory.
    Import {
        /// The bundle.
        bundle: PathBuf,
        /// Output directory.
        #[arg(short, long)]
        output: PathBuf,
        /// Overwrite existing files.
        #[arg(long)]
        force: bool,
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
            Command::Config(_) => "config",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => cmd::serve::run(&args),
        Command::Send(args) => cmd::send::run(args),
        Command::Render(args) => cmd::render::run(&args),
        Command::Import(args) => cmd::import::run(&args),
        Command::Template(args) => cmd::template::run(args),
        Command::Ifaces(args) => cmd::ifaces::run(&args),
        Command::Doctor(args) => cmd::doctor::run(&args),
        Command::Bench(args) => cmd::bench::run(args),
        Command::Config(args) => cmd::config::run(args),
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
            Command::Serve(a) => {
                assert_eq!(a.bind, None);
                assert!(!a.dev);
            }
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
            &[
                "wpg",
                "serve",
                "--bind",
                "127.0.0.1:1",
                "--config",
                "x.toml",
                "--dev",
                "--library-path",
                "a",
                "--library-path",
                "b",
                "--data-dir",
                "d",
                "--password-file",
                "p.txt",
                "--tls",
                "--allow-insecure",
            ],
            &[
                "wpg",
                "config",
                "set-password",
                "--config",
                "w.toml",
                "--stdin",
            ],
            &["wpg", "config", "init", "--path", "w.toml", "--force"],
            &["wpg", "config", "show", "--config", "w.toml"],
            &["wpg", "config", "validate"],
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
