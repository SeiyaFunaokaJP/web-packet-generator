//! `wpg send` subcommand: compile a template and transmit it headlessly.
//!
//! Progress goes to stderr once per second; the final summary goes to stdout (or a
//! JSON [`JobReport`] with `--json`). Ctrl-C stops the job cleanly (the backend is
//! flushed and closed, the summary is still printed).

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Args;
use wpg_core::stream::{Expansion, PlanConfig, PlanHint};
use wpg_protocols::catalog;
use wpg_tx::job::{JobConfig, JobHandle, JobReport, JobState, TxJob};
use wpg_tx::rate::{link_speed_bps, parse_duration, Rate};
use wpg_tx::source::{FrameSource, PreRendered, Rendered};
use wpg_tx::{backend, TxError, TxOptions};

use crate::cmd::cli_locale;
use crate::cmd::template::load_compiled;

/// Backend tuning shared by `wpg send` and `wpg bench`.
#[derive(Debug, Clone, Args)]
pub(crate) struct TxTuningArgs {
    /// Frames per `send_batch` call.
    #[arg(long = "batch", value_name = "N")]
    pub(crate) batch: Option<usize>,
    /// TX ring depth in frames (`afpacket`).
    #[arg(long, value_name = "N")]
    pub(crate) ring_depth: Option<usize>,
    /// Pin the TX thread to this CPU core (Linux).
    #[arg(long, value_name = "CORE")]
    pub(crate) pin_core: Option<usize>,
    /// Bypass the qdisc layer (`afpacket`, `PACKET_QDISC_BYPASS`).
    #[arg(long)]
    pub(crate) qdisc_bypass: bool,
    /// Use `sendmmsg` instead of the TPACKET_V3 TX ring (`afpacket`).
    #[arg(long)]
    pub(crate) no_tx_ring: bool,
}

impl TxTuningArgs {
    pub(crate) fn options(&self, default_batch: usize) -> TxOptions {
        let d = TxOptions::default();
        TxOptions {
            batch_size: self.batch.unwrap_or(default_batch).max(1),
            ring_depth: self.ring_depth.unwrap_or(d.ring_depth),
            pin_core: self.pin_core,
            qdisc_bypass: self.qdisc_bypass,
            tx_ring: !self.no_tx_ring,
        }
    }
}

/// Formats a rate with an SI prefix: `1.23 M`.
pub(crate) fn si(v: f64) -> String {
    let (d, p) = if v >= 1e9 {
        (1e9, "G")
    } else if v >= 1e6 {
        (1e6, "M")
    } else if v >= 1e3 {
        (1e3, "k")
    } else {
        (1.0, "")
    };
    format!("{:.2} {p}", v / d)
}

/// Prints an open / job error, with the permission hint where relevant.
pub(crate) fn report_tx_error(ctx: &str, e: &TxError) {
    eprintln!("{ctx}: {e}");
    if matches!(e, TxError::Unsupported(_)) {
        let names: Vec<String> = backend::available()
            .into_iter()
            .filter(|b| b.available)
            .map(|b| b.name)
            .collect();
        eprintln!("available backends: {}", names.join(", "));
    }
}

/// Installs a Ctrl-C handler that sets the returned flag (once per process).
pub(crate) fn ctrl_c_flag() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let f = Arc::clone(&flag);
    if let Err(e) = ctrlc::set_handler(move || f.store(true, Ordering::SeqCst)) {
        eprintln!("warning: cannot install Ctrl-C handler: {e}");
    }
    flag
}

fn plan_hint(s: &str) -> Option<PlanHint> {
    match s.to_ascii_lowercase().replace(['-', '_'], "").as_str() {
        "auto" => Some(PlanHint::Auto),
        "prerender" | "pre" => Some(PlanHint::PreRender),
        "onthefly" | "fly" => Some(PlanHint::OnTheFly),
        _ => None,
    }
}

/// Everything `wpg send` needs besides the template.
struct Resolved {
    backend: String,
    iface: String,
    rate: Rate,
    count: Option<u64>,
    duration: Option<Duration>,
    burst: u32,
}

fn resolve(args: &crate::SendArgs, tx: &wpg_template::TxProfile) -> Result<Resolved, String> {
    let backend = args
        .backend
        .clone()
        .unwrap_or_else(|| backend::default_name().to_owned());
    let iface = match (&args.iface, backend.as_str()) {
        (Some(i), _) => i.clone(),
        (None, "null") => "null0".to_owned(),
        (None, "pcapfile") => {
            return Err("--iface <output path> is required for the pcapfile backend".to_owned())
        }
        (None, _) => return Err(format!("--iface is required for the {backend} backend")),
    };
    let rate_str = args.rate.clone().or_else(|| tx.rate.clone());
    let mut rate = match rate_str {
        Some(s) => Rate::parse(&s).map_err(|e| e.to_string())?,
        None => Rate::Unlimited,
    };
    if let Rate::LinkPercent { percent, .. } = rate {
        let link = link_speed_bps(&iface).ok_or_else(|| {
            format!("cannot determine the link speed of `{iface}` for `{percent}%`; give an absolute rate such as `1Gbps`")
        })?;
        rate = rate.with_link_speed(link);
    }
    let duration = match &args.duration {
        Some(d) => Some(parse_duration(d).map_err(|e| e.to_string())?),
        None => tx
            .duration_secs
            .map(|s| Duration::try_from_secs_f64(s).map_err(|e| format!("duration: {e}")))
            .transpose()?,
    };
    Ok(Resolved {
        backend,
        iface,
        rate,
        count: args.count.or(tx.count),
        duration,
        burst: args.burst.or(tx.burst).unwrap_or(1).max(1),
    })
}

fn progress_line(h: &JobHandle) -> String {
    let s = h.stats();
    let ratio = s
        .rate_achieved_ratio
        .map(|r| format!("  rate {:.1}%", r * 100.0))
        .unwrap_or_default();
    format!(
        "[{:>7.1}s] {:>12} pkts  {}pps  {}bps  dropped {}{}{}",
        s.t.as_secs_f64(),
        s.packets,
        si(s.pps),
        si(s.bps),
        s.dropped,
        ratio,
        if s.state == JobState::Paused {
            "  (paused)"
        } else {
            ""
        }
    )
}

fn print_summary(r: &JobReport) {
    let s = &r.stats;
    let state = match &r.state {
        JobState::Finished(reason) => format!("finished ({reason:?})"),
        JobState::Failed(m) => format!("failed: {m}"),
        other => format!("{other:?}"),
    };
    println!("state      {state}");
    println!(
        "backend    {}{}",
        r.backend_name,
        r.backend_detail
            .as_ref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default()
    );
    println!("packets    {}", s.packets);
    println!("bytes      {}", s.bytes);
    println!("dropped    {}", s.dropped);
    println!("elapsed    {:.3} s", r.active.as_secs_f64());
    println!(
        "average    {}pps  {}bps (L2)  {}bps (L1)",
        si(s.pps),
        si(s.bps),
        si(s.l1_bps)
    );
    if let (Some(target), Some(ratio)) = (r.accuracy.target, r.accuracy.ratio) {
        println!(
            "accuracy   target {}{}  achieved {:.2}%  max jitter {:?}  mean jitter {:?}",
            si(target),
            match r.accuracy.unit {
                wpg_tx::rate::RateUnit::Pps => "pps",
                wpg_tx::rate::RateUnit::Bps => "bps (L1)",
            },
            ratio * 100.0,
            r.accuracy.max_jitter,
            r.accuracy.mean_jitter
        );
    }
    if s.per_stream.len() > 1 {
        for (i, st) in s.per_stream.iter().enumerate() {
            println!("stream {i:<3} {} pkts  {} bytes", st.packets, st.bytes);
        }
    }
    if let Some(e) = &r.pin_error {
        println!("warning    {e}");
    }
}

/// Runs a started job to completion with progress output and Ctrl-C handling.
pub(crate) fn drive(job: JobHandle, interrupted: &AtomicBool, progress: bool) -> JobReport {
    let mut next = Instant::now() + Duration::from_secs(1);
    let mut stopping = false;
    while !job.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
        if !stopping && interrupted.load(Ordering::SeqCst) {
            eprintln!("interrupted, stopping...");
            job.stop();
            stopping = true;
        }
        if progress && Instant::now() >= next {
            eprintln!("{}", progress_line(&job));
            next += Duration::from_secs(1);
        }
    }
    job.wait()
}

/// Runs `wpg send`.
#[allow(clippy::needless_pass_by_value)] // uniform `cmd::*::run(args)` dispatch
pub(crate) fn run(args: crate::SendArgs) -> ExitCode {
    let locale = cli_locale();
    let (_, mut compiled) = match load_compiled(&args.template, &args.vars, locale) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let r = match resolve(&args, &compiled.tx) {
        Ok(r) => r,
        Err(m) => {
            eprintln!("{m}");
            return ExitCode::from(2);
        }
    };
    let Some(hint) = plan_hint(&args.plan) else {
        eprintln!("--plan must be auto, prerender or onthefly");
        return ExitCode::from(2);
    };

    // An explicit count makes finite generators cycle up to that many packets; a
    // duration without a count loops the stream until the time is up.
    if let Some(c) = r.count {
        compiled.stream.count = Some(c);
    }
    let exp = match Expansion::new(&compiled.stream, None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("stream: {e}");
            return ExitCode::FAILURE;
        }
    };
    let loop_stream = r.count.is_none() && r.duration.is_some();
    let cfg = PlanConfig {
        hint,
        ..PlanConfig::default()
    };
    let (rendered, plan) = match Rendered::auto(compiled.packet, exp, catalog(), &cfg) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("render: {e}");
            return ExitCode::FAILURE;
        }
    };
    let source: Box<dyn FrameSource> = match (rendered, loop_stream) {
        (Rendered::Pre(p), true) => Box::new(PreRendered::new(p.into_frames(), None)),
        (Rendered::Fly(f), true) => Box::new(f.cycling()),
        (other, false) => Box::new(other),
    };

    let opts = args.tuning.options(64);
    let tx = match backend::open_by_name(&r.backend, &r.iface, &opts) {
        Ok(b) => b,
        Err(e) => {
            report_tx_error(&format!("cannot open {} on `{}`", r.backend, r.iface), &e);
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "sending on {} via {}{}: rate {}, burst {}, {}, plan {:?} ({:?})",
        r.iface,
        r.backend,
        tx.detail().map(|d| format!(" ({d})")).unwrap_or_default(),
        r.rate,
        r.burst,
        match (r.count, r.duration) {
            (Some(c), Some(d)) => format!("{c} packets or {d:?}"),
            (Some(c), None) => format!("{c} packets"),
            (None, Some(d)) => format!("for {d:?}"),
            (None, None) => source
                .remaining()
                .map_or_else(|| "until Ctrl-C".to_owned(), |n| format!("{n} packets")),
        },
        plan.strategy,
        plan.reason,
    );
    let interrupted = ctrl_c_flag();
    let job = match TxJob::start(
        tx,
        source,
        JobConfig {
            count: r.count,
            duration: r.duration,
            rate: r.rate,
            burst: r.burst,
            pin_core: opts.pin_core,
            batch_size: opts.batch_size,
            ..JobConfig::default()
        },
    ) {
        Ok(j) => j,
        Err(e) => {
            report_tx_error("cannot start the job", &e);
            return ExitCode::FAILURE;
        }
    };
    let report = drive(job, &interrupted, true);
    if args.json {
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("json: {e}"),
        }
    } else {
        print_summary(&report);
    }
    if matches!(report.state, JobState::Failed(_)) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn si_prefixes() {
        assert_eq!(si(999.0), "999.00 ");
        assert_eq!(si(1_500.0), "1.50 k");
        assert_eq!(si(14_880_952.0), "14.88 M");
        assert_eq!(si(1e10), "10.00 G");
    }

    #[test]
    fn plan_hints() {
        assert_eq!(plan_hint("auto"), Some(PlanHint::Auto));
        assert_eq!(plan_hint("pre-render"), Some(PlanHint::PreRender));
        assert_eq!(plan_hint("on_the_fly"), Some(PlanHint::OnTheFly));
        assert_eq!(plan_hint("x"), None);
    }
}
