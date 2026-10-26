//! `wpg capture` subcommand: receive counters, the first N frames, pcapng output
//! and signature verification (spec §2.3, M9).
//!
//! Progress goes to stderr once per second; the first `--show` matching frames and
//! the summary go to stdout (or a JSON [`CaptureReport`] plus the frames with
//! `--json`). Ctrl-C stops the capture cleanly. Human-readable output is localised
//! (Fluent keys `rx-*`, `capture-*`); JSON is not.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use clap::Args;
use wpg_i18n::{t, Locale};
use wpg_protocols::catalog;
use wpg_rx::backend::{self, RxOptions};
use wpg_rx::capture::{CapturedPacket, EndReason};
use wpg_rx::{Capture, CaptureConfig, CaptureHandle, CaptureReport, CaptureState, Filter, RxError};
use wpg_tx::rate::parse_duration;

use crate::cmd::cli_locale;
use crate::cmd::send::{ctrl_c_flag, print_json, si, tr};

/// `wpg capture` arguments.
#[derive(Debug, Args)]
pub(crate) struct CaptureArgs {
    /// Interface to receive on (`any` = every interface with `afpacket`); for the
    /// `pcapfile` backend the capture file to replay.
    #[arg(short, long)]
    pub(crate) iface: Option<String>,
    /// Receive backend: afpacket (Linux), pcap (feature `pcap`), pcapfile, null.
    #[arg(long)]
    pub(crate) backend: Option<String>,
    /// tcpdump-style filter, e.g. `udp port 5001 and wpg` (default: every frame).
    #[arg(short, long, value_name = "EXPR")]
    pub(crate) filter: Option<String>,
    /// Stop after this many matching frames.
    #[arg(short, long)]
    pub(crate) count: Option<u64>,
    /// Stop after this long, e.g. `10s`.
    #[arg(long)]
    pub(crate) duration: Option<String>,
    /// Print the first N matching frames.
    #[arg(long, value_name = "N", default_value_t = 5)]
    pub(crate) show: usize,
    /// Also print a hex dump of the shown frames.
    #[arg(long)]
    pub(crate) hex: bool,
    /// Write the matching frames to this pcapng file.
    #[arg(short, long, value_name = "FILE")]
    pub(crate) write: Option<PathBuf>,
    /// Check the signatures stamped by `wpg send --signature` (loss / order /
    /// latency).
    #[arg(long)]
    pub(crate) verify: bool,
    /// Promiscuous mode.
    #[arg(long)]
    pub(crate) promisc: bool,
    /// Also count frames this host transmits.
    #[arg(long)]
    pub(crate) include_outgoing: bool,
    /// Machine-readable output.
    #[arg(long)]
    pub(crate) json: bool,
}

/// One-line description of a frame: `ethernet2 / ipv4 / udp / payload`.
fn layers(data: &[u8]) -> String {
    match wpg_core::parse(data, "ethernet2", catalog()) {
        Ok(p) => p
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect::<Vec<_>>()
            .join(" / "),
        Err(_) => "?".to_owned(),
    }
}

/// `HH:MM:SS.nnnnnnnnn` (UTC) of a timestamp in nanoseconds since the epoch.
fn clock(ts_ns: u64) -> String {
    let secs = ts_ns / 1_000_000_000;
    format!(
        "{:02}:{:02}:{:02}.{:09}",
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60,
        ts_ns % 1_000_000_000
    )
}

fn hexdump(data: &[u8]) -> String {
    let mut s = String::new();
    for (i, chunk) in data.chunks(16).enumerate() {
        let _ = write!(s, "    {:04x}  ", i * 16);
        for b in chunk {
            let _ = write!(s, "{b:02x} ");
        }
        s.push('\n');
    }
    s
}

fn print_packet(p: &CapturedPacket, hex: bool, locale: Locale) {
    let sig = p
        .signature
        .map(|s| {
            tr(
                locale,
                "capture-packet-signature",
                &[
                    ("stream", s.stream_id.to_string()),
                    ("seq", s.seq.to_string()),
                ],
            )
        })
        .unwrap_or_default();
    println!(
        "{}",
        tr(
            locale,
            "capture-packet",
            &[
                ("index", format!("{:>4}", p.index)),
                ("time", clock(p.ts_ns)),
                ("len", format!("{:>5}", p.len)),
                ("layers", layers(&p.data)),
                ("signature", sig),
            ],
        )
    );
    if hex {
        print!("{}", hexdump(&p.data));
    }
}

fn progress_line(h: &CaptureHandle, locale: Locale) -> String {
    let s = h.stats();
    tr(
        locale,
        "rx-progress",
        &[
            ("t", format!("{:>7.1}", s.t.as_secs_f64())),
            ("packets", format!("{:>10}", s.packets)),
            ("pps", si(s.pps)),
            ("bps", si(s.bps)),
            ("matched", s.matched.to_string()),
            ("mpps", si(s.matched_pps)),
            ("dropped", s.dropped.to_string()),
        ],
    )
}

fn state_text(state: &CaptureState, locale: Locale) -> String {
    match state {
        CaptureState::Finished(reason) => {
            let key = match reason {
                EndReason::CountReached => "summary-reason-count-reached",
                EndReason::DurationElapsed => "summary-reason-duration-elapsed",
                EndReason::EndOfInput => "capture-reason-end-of-input",
                EndReason::Stopped => "summary-reason-stopped",
            };
            tr(
                locale,
                "summary-state-finished",
                &[("reason", t(locale, key))],
            )
        }
        CaptureState::Failed(m) => tr(locale, "summary-state-failed", &[("error", m.clone())]),
        CaptureState::Running => t(locale, "capture-running"),
    }
}

/// Nanoseconds as a short human duration (`12.3 µs`).
fn ns(v: i64) -> String {
    let a = v.unsigned_abs() as f64;
    let sign = if v < 0 { "-" } else { "" };
    if a >= 1e9 {
        format!("{sign}{:.3} s", a / 1e9)
    } else if a >= 1e6 {
        format!("{sign}{:.3} ms", a / 1e6)
    } else if a >= 1e3 {
        format!("{sign}{:.1} µs", a / 1e3)
    } else {
        format!("{sign}{a:.0} ns")
    }
}

fn print_summary(r: &CaptureReport, locale: Locale) {
    let s = &r.stats;
    let line = |key: &str, args: &[(&str, String)]| println!("{}", tr(locale, key, args));
    line("summary-state", &[("value", state_text(&r.state, locale))]);
    line(
        "summary-backend",
        &[
            ("backend", r.backend_name.clone()),
            ("detail", String::new()),
        ],
    );
    line("summary-packets", &[("value", s.packets.to_string())]);
    line("summary-bytes", &[("value", s.bytes.to_string())]);
    line(
        "capture-summary-matched",
        &[
            ("packets", s.matched.to_string()),
            ("bytes", s.matched_bytes.to_string()),
        ],
    );
    line("summary-dropped", &[("value", s.dropped.to_string())]);
    line(
        "summary-elapsed",
        &[("secs", format!("{:.3}", s.t.as_secs_f64()))],
    );
    line(
        "capture-summary-average",
        &[
            ("pps", si(s.pps)),
            ("bps", si(s.bps)),
            ("mpps", si(s.matched_pps)),
            ("mbps", si(s.matched_bps)),
        ],
    );
    if let Some(p) = &r.saved_path {
        line(
            "capture-summary-saved",
            &[
                ("count", s.saved.to_string()),
                ("path", p.display().to_string()),
            ],
        );
    }
    if let Some(v) = &s.verify {
        line(
            "capture-summary-signed",
            &[
                ("signed", v.signed.to_string()),
                ("unsigned", v.unsigned.to_string()),
            ],
        );
        for st in &v.streams {
            line(
                "capture-summary-stream",
                &[
                    ("stream", format!("{:<5}", st.stream_id)),
                    ("received", st.received.to_string()),
                    ("first", st.first_seq.to_string()),
                    ("last", st.last_seq.to_string()),
                    ("lost", st.lost.to_string()),
                    ("ratio", format!("{:.3}", st.loss_ratio() * 100.0)),
                    ("reordered", st.out_of_order.to_string()),
                    ("duplicates", st.duplicates.to_string()),
                ],
            );
            line(
                "capture-summary-latency",
                &[
                    ("min", ns(st.latency_min_ns)),
                    ("avg", ns(st.latency_avg_ns)),
                    ("max", ns(st.latency_max_ns)),
                    ("jitter", ns(st.jitter_ns)),
                ],
            );
        }
    }
}

fn open_error(locale: Locale, backend_name: &str, iface: &str, e: &RxError) {
    eprintln!(
        "{}",
        tr(
            locale,
            "tx-open-failed",
            &[
                ("backend", backend_name.to_owned()),
                ("iface", iface.to_owned()),
                ("error", e.to_string()),
            ],
        )
    );
    if matches!(e, RxError::Unsupported(_)) {
        let names: Vec<String> = backend::available()
            .into_iter()
            .filter(|b| b.available)
            .map(|b| b.name)
            .collect();
        eprintln!(
            "{}",
            tr(
                locale,
                "tx-available-backends",
                &[("value", names.join(", "))]
            )
        );
    }
}

/// Runs `wpg capture`.
#[allow(clippy::needless_pass_by_value)] // matches the other `run(args)` entry points
pub(crate) fn run(args: CaptureArgs) -> ExitCode {
    let locale = cli_locale();
    let backend_name = args
        .backend
        .clone()
        .unwrap_or_else(|| backend::default_name().to_owned());
    let iface = match (&args.iface, backend_name.as_str()) {
        (Some(i), _) => i.clone(),
        (None, "null") => "null0".to_owned(),
        (None, "pcapfile") => {
            eprintln!("{}", t(locale, "capture-iface-required-pcapfile"));
            return ExitCode::from(2);
        }
        (None, b) => {
            eprintln!(
                "{}",
                tr(locale, "tx-iface-required", &[("backend", b.to_owned())])
            );
            return ExitCode::from(2);
        }
    };
    let filter = match Filter::parse(args.filter.as_deref().unwrap_or("")) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "capture-bad-filter", &[("error", e.to_string())])
            );
            return ExitCode::from(2);
        }
    };
    let duration = match args.duration.as_deref().map(parse_duration).transpose() {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "send-bad-duration", &[("error", e.to_string())])
            );
            return ExitCode::from(2);
        }
    };
    let opts = RxOptions {
        promisc: args.promisc,
        include_outgoing: args.include_outgoing,
        ..RxOptions::default()
    };
    let rx = match backend::open_by_name(&backend_name, &iface, &opts) {
        Ok(b) => b,
        Err(e) => {
            open_error(locale, &backend_name, &iface, &e);
            return ExitCode::FAILURE;
        }
    };
    let config = CaptureConfig {
        filter,
        count: args.count,
        duration,
        keep: args.show,
        save: args.write.clone(),
        verify: args.verify,
        ..CaptureConfig::default()
    };
    eprintln!(
        "{}",
        tr(
            locale,
            "capture-starting",
            &[
                ("iface", iface.clone()),
                ("backend", backend_name.clone()),
                (
                    "filter",
                    if config.filter.is_all() {
                        t(locale, "capture-filter-all")
                    } else {
                        config.filter.text().to_owned()
                    }
                ),
            ],
        )
    );
    let interrupted = ctrl_c_flag(locale);
    let handle = match Capture::start(rx, config) {
        Ok(h) => h,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "tx-start-failed", &[("error", e.to_string())])
            );
            return ExitCode::FAILURE;
        }
    };
    let mut next = Instant::now() + Duration::from_secs(1);
    let mut stopping = false;
    while !handle.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
        if !stopping && interrupted.load(Ordering::SeqCst) {
            eprintln!("{}", t(locale, "tx-interrupted"));
            handle.stop();
            stopping = true;
        }
        if !args.json && Instant::now() >= next {
            eprintln!("{}", progress_line(&handle, locale));
            next += Duration::from_secs(1);
        }
    }
    let packets = handle.packets();
    let report = handle.wait();
    if args.json {
        let v = serde_json::json!({ "report": report, "packets": packets });
        if !print_json(serde_json::to_string_pretty(&v), locale) {
            return ExitCode::FAILURE;
        }
    } else {
        for p in &packets {
            print_packet(p, args.hex, locale);
        }
        print_summary(&report, locale);
    }
    if matches!(report.state, CaptureState::Failed(_)) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting() {
        assert_eq!(clock(3_723_000_000_123), "01:02:03.000000123");
        assert_eq!(ns(950), "950 ns");
        assert_eq!(ns(-12_340), "-12.3 µs");
        assert_eq!(ns(5_000_000), "5.000 ms");
        assert_eq!(hexdump(&[0xab; 17]).lines().count(), 2);
        let mut f = vec![0xffu8; 12];
        f.extend_from_slice(&[0x08, 0x06]);
        f.extend_from_slice(&[0, 1, 8, 0, 6, 4, 0, 1]);
        f.resize(60, 0);
        assert!(layers(&f).starts_with("ethernet2 / arp"), "{}", layers(&f));
    }

    #[test]
    fn localised_state() {
        let s = CaptureState::Finished(EndReason::EndOfInput);
        assert_eq!(state_text(&s, Locale::En), "finished (end of input)");
        assert!(state_text(&s, Locale::Ja).starts_with("完了"));
    }
}
