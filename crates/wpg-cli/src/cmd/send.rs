//! `wpg send` subcommand: compile a template and transmit it headlessly.
//!
//! Progress goes to stderr once per second; the final summary goes to stdout (or a
//! JSON [`JobReport`] with `--json`). Ctrl-C stops the job cleanly (the backends
//! are flushed and closed, the summary is still printed). Human-readable output is
//! localised (Fluent keys `tx-*`, `send-*`, `summary-*`); JSON is not.

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use clap::Args;
use wpg_core::stream::{Expansion, PlanConfig, PlanHint};
use wpg_i18n::{t, t_args, FluentArgs, Locale};
use wpg_protocols::catalog;
use wpg_tx::job::{
    FinishReason, JobConfig, JobHandle, JobReport, JobState, SignatureConfig, TxJob,
};
use wpg_tx::rate::{link_speed_bps, parse_duration, parse_start_time, Rate, RateUnit};
use wpg_tx::rate::{PacerConfig, WaitMode};
use wpg_tx::source::{FrameSource, PreRendered, Rendered};
use wpg_tx::{backend, pin, TxError, TxOptions, XdpMode};

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
    /// Pin the TX thread to this CPU core (Linux; the first worker).
    #[arg(long, value_name = "CORE")]
    pub(crate) pin_core: Option<usize>,
    /// Pin worker i to the i-th core of this list, e.g. `1,2` (Linux).
    #[arg(long, value_name = "CORES", value_delimiter = ',')]
    pub(crate) pin_cores: Vec<usize>,
    /// TX worker threads, each with its own backend instance (afpacket: socket and
    /// ring) and share of the stream and rate.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub(crate) workers: usize,
    /// Bypass the qdisc layer (`afpacket`, `PACKET_QDISC_BYPASS`).
    #[arg(long)]
    pub(crate) qdisc_bypass: bool,
    /// Use `sendmmsg` instead of the TPACKET_V3 TX ring (`afpacket`).
    #[arg(long)]
    pub(crate) no_tx_ring: bool,
    /// Pin the workers to cores on the NIC's NUMA node, core 0 last (Linux;
    /// ignored when --pin-cores is given).
    #[arg(long)]
    pub(crate) auto_pin: bool,
    /// First hardware queue (`afxdp`, `dpdk`; worker i uses queue N+i).
    #[arg(long, value_name = "N")]
    pub(crate) queue: Option<u32>,
    /// AF_XDP bind mode: auto (zero-copy, else copy), copy or zerocopy.
    #[arg(long, value_name = "MODE", default_value = "auto")]
    pub(crate) xdp_mode: XdpMode,
    /// Busy polling: `afxdp` sockets busy-poll the driver (kernel 5.11+) and
    /// `wpg send` paces with `--wait busy-poll` unless --wait says otherwise.
    #[arg(long)]
    pub(crate) busy_poll: bool,
    /// DPDK EAL arguments (`dpdk`), e.g. "-a 0000:00:04.0 --file-prefix wpg"
    /// (split on whitespace; wpg adds `-l <first CPU>` and `--in-memory` unless given).
    #[arg(long, value_name = "ARGS", allow_hyphen_values = true)]
    pub(crate) eal_args: Option<String>,
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
            queue: self.queue,
            xdp_mode: self.xdp_mode,
            busy_poll: self.busy_poll,
            eal_args: self
                .eal_args
                .as_deref()
                .map(|a| a.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default(),
        }
    }

    /// Cores for `workers` workers: `--pin-cores`, else `--auto-pin` (NUMA-local
    /// cores of `iface`), else none.
    pub(crate) fn pin_cores_for(&self, iface: &str, workers: usize) -> Vec<usize> {
        if !self.pin_cores.is_empty() {
            self.pin_cores.clone()
        } else if self.auto_pin {
            pin::auto_cores(iface, workers)
        } else {
            Vec::new()
        }
    }

    /// Validates `--workers` against the backend; prints the localised error.
    pub(crate) fn check_workers(&self, backend: &str, locale: Locale) -> Result<usize, ExitCode> {
        if self.workers == 0 {
            eprintln!("{}", t(locale, "tx-bad-workers"));
            return Err(ExitCode::from(2));
        }
        if self.workers > 1 && !backend::supports_workers(backend) {
            eprintln!(
                "{}",
                tr(
                    locale,
                    "tx-workers-unsupported",
                    &[("backend", backend.to_owned())]
                )
            );
            return Err(ExitCode::from(2));
        }
        if !self.pin_cores.is_empty() && self.pin_cores.len() < self.workers {
            eprintln!(
                "{}",
                tr(
                    locale,
                    "tx-pin-cores-short",
                    &[
                        ("count", self.pin_cores.len().to_string()),
                        ("workers", self.workers.to_string()),
                    ]
                )
            );
        }
        Ok(self.workers)
    }
}

/// Translates `key` with string arguments.
pub(crate) fn tr(locale: Locale, key: &str, args: &[(&str, String)]) -> String {
    let mut a = FluentArgs::new();
    for (k, v) in args {
        a.set(*k, v.clone());
    }
    t_args(locale, key, &a)
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

/// ` (detail)` or empty.
pub(crate) fn paren(detail: Option<&str>) -> String {
    detail.map(|d| format!(" ({d})")).unwrap_or_default()
}

/// Prints an open / job error, with the list of backends where relevant.
pub(crate) fn report_tx_error(locale: Locale, message: &str, e: &TxError) {
    eprintln!("{message}");
    if matches!(e, TxError::Unsupported(_)) {
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

/// [`report_tx_error`] for a backend that could not be opened.
pub(crate) fn report_open_error(locale: Locale, backend: &str, iface: &str, e: &TxError) {
    let m = tr(
        locale,
        "tx-open-failed",
        &[
            ("backend", backend.to_owned()),
            ("iface", iface.to_owned()),
            ("error", e.to_string()),
        ],
    );
    report_tx_error(locale, &m, e);
}

/// Installs a Ctrl-C handler that sets the returned flag (once per process).
pub(crate) fn ctrl_c_flag(locale: Locale) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let f = Arc::clone(&flag);
    if let Err(e) = ctrlc::set_handler(move || f.store(true, Ordering::SeqCst)) {
        eprintln!(
            "{}",
            tr(locale, "tx-ctrlc-warning", &[("error", e.to_string())])
        );
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
    ramp_up: Option<Duration>,
    start_at: Option<SystemTime>,
}

/// Default interface for backends that need none; the localised error otherwise.
pub(crate) fn default_iface(
    iface: Option<&String>,
    backend: &str,
    locale: Locale,
) -> Result<String, String> {
    match (iface, backend) {
        (Some(i), _) => Ok(i.clone()),
        (None, "null") => Ok("null0".to_owned()),
        (None, "pcapfile") => Err(t(locale, "tx-iface-required-pcapfile")),
        (None, _) => Err(tr(
            locale,
            "tx-iface-required",
            &[("backend", backend.to_owned())],
        )),
    }
}

fn resolve(
    args: &crate::SendArgs,
    tx: &wpg_template::TxProfile,
    locale: Locale,
) -> Result<Resolved, String> {
    let err = |key: &str, e: String| tr(locale, key, &[("error", e)]);
    let backend = args
        .backend
        .clone()
        .unwrap_or_else(|| backend::default_name().to_owned());
    let iface = default_iface(args.iface.as_ref(), &backend, locale)?;
    let rate_str = args.rate.clone().or_else(|| tx.rate.clone());
    let mut rate = match rate_str {
        Some(s) => Rate::parse(&s).map_err(|e| err("send-bad-rate", e.to_string()))?,
        None => Rate::Unlimited,
    };
    if let Rate::LinkPercent { percent, .. } = rate {
        let link = link_speed_bps(&iface).ok_or_else(|| {
            tr(
                locale,
                "send-link-speed-unknown",
                &[("iface", iface.clone()), ("value", percent.to_string())],
            )
        })?;
        rate = rate.with_link_speed(link);
    }
    let duration = match &args.duration {
        Some(d) => Some(parse_duration(d).map_err(|e| err("send-bad-duration", e.to_string()))?),
        None => tx
            .duration_secs
            .map(|s| {
                Duration::try_from_secs_f64(s).map_err(|e| err("send-bad-duration", e.to_string()))
            })
            .transpose()?,
    };
    let ramp_up = args
        .ramp_up
        .as_deref()
        .map(|s| parse_duration(s).map_err(|e| err("send-bad-ramp-up", e.to_string())))
        .transpose()?;
    let start_at = args
        .start_at
        .as_deref()
        .map(|s| parse_start_time(s, SystemTime::now()).map_err(|e| err("send-bad-start-at", e)))
        .transpose()?;
    Ok(Resolved {
        backend,
        iface,
        rate,
        count: args.count.or(tx.count),
        duration,
        burst: args.burst.or(tx.burst).unwrap_or(1).max(1),
        ramp_up,
        start_at,
    })
}

fn progress_line(h: &JobHandle, locale: Locale, start_at: Option<SystemTime>) -> String {
    let s = h.stats();
    let mut line = tr(
        locale,
        "tx-progress",
        &[
            ("t", format!("{:>7.1}", s.t.as_secs_f64())),
            ("packets", format!("{:>12}", s.packets)),
            ("pps", si(s.pps)),
            ("bps", si(s.bps)),
            ("dropped", s.dropped.to_string()),
        ],
    );
    if let Some(r) = s.rate_achieved_ratio {
        line.push_str("  ");
        line.push_str(&tr(
            locale,
            "tx-progress-rate",
            &[("ratio", format!("{:.1}", r * 100.0))],
        ));
    }
    match s.state {
        JobState::Paused => {
            line.push_str("  ");
            line.push_str(&t(locale, "tx-progress-paused"));
        }
        JobState::Scheduled => {
            let secs = start_at
                .and_then(|at| at.duration_since(SystemTime::now()).ok())
                .unwrap_or_default();
            line.push_str("  ");
            line.push_str(&tr(
                locale,
                "tx-progress-scheduled",
                &[("secs", format!("{:.0}", secs.as_secs_f64().ceil()))],
            ));
        }
        _ => {}
    }
    line
}

fn state_text(state: &JobState, locale: Locale) -> String {
    match state {
        JobState::Finished(reason) => {
            let key = match reason {
                FinishReason::CountReached => "summary-reason-count-reached",
                FinishReason::DurationElapsed => "summary-reason-duration-elapsed",
                FinishReason::SourceExhausted => "summary-reason-source-exhausted",
                FinishReason::Stopped => "summary-reason-stopped",
            };
            tr(
                locale,
                "summary-state-finished",
                &[("reason", t(locale, key))],
            )
        }
        JobState::Failed(m) => tr(locale, "summary-state-failed", &[("error", m.clone())]),
        other => format!("{other:?}"),
    }
}

fn print_summary(r: &JobReport, locale: Locale) {
    let s = &r.stats;
    let line = |key: &str, args: &[(&str, String)]| println!("{}", tr(locale, key, args));
    line("summary-state", &[("value", state_text(&r.state, locale))]);
    line(
        "summary-backend",
        &[
            ("backend", r.backend_name.clone()),
            ("detail", paren(r.backend_detail.as_deref())),
        ],
    );
    if r.workers.len() > 1 {
        line(
            "summary-workers",
            &[("workers", r.workers.len().to_string())],
        );
    }
    line("summary-packets", &[("value", s.packets.to_string())]);
    line("summary-bytes", &[("value", s.bytes.to_string())]);
    line("summary-dropped", &[("value", s.dropped.to_string())]);
    if s.unsigned > 0 {
        line("summary-unsigned", &[("value", s.unsigned.to_string())]);
    }
    line(
        "summary-elapsed",
        &[("secs", format!("{:.3}", r.active.as_secs_f64()))],
    );
    line(
        "summary-average",
        &[("pps", si(s.pps)), ("bps", si(s.bps)), ("l1", si(s.l1_bps))],
    );
    if let (Some(target), Some(ratio)) = (r.accuracy.target, r.accuracy.ratio) {
        line(
            "summary-accuracy",
            &[
                ("target", si(target)),
                (
                    "unit",
                    match r.accuracy.unit {
                        RateUnit::Pps => "pps".to_owned(),
                        RateUnit::Bps => "bps (L1)".to_owned(),
                    },
                ),
                ("ratio", format!("{:.2}", ratio * 100.0)),
                ("max", format!("{:?}", r.accuracy.max_jitter)),
                ("mean", format!("{:?}", r.accuracy.mean_jitter)),
            ],
        );
    }
    if s.per_stream.len() > 1 {
        for (i, st) in s.per_stream.iter().enumerate() {
            line(
                "summary-stream",
                &[
                    ("index", format!("{i:<3}")),
                    ("packets", st.packets.to_string()),
                    ("bytes", st.bytes.to_string()),
                ],
            );
        }
    }
    if r.workers.len() > 1 {
        for w in &r.workers {
            let core = w
                .core
                .map(|c| tr(locale, "summary-worker-core", &[("core", c.to_string())]))
                .unwrap_or_default()
                + &paren(w.detail.as_deref());
            line(
                "summary-worker",
                &[
                    ("index", format!("{:<3}", w.index)),
                    ("packets", w.packets.to_string()),
                    ("bytes", w.bytes.to_string()),
                    ("core", core),
                ],
            );
        }
    }
    if let Some(e) = &r.pin_error {
        line("summary-warning", &[("error", e.clone())]);
    }
}

/// Runs a started job to completion with progress output and Ctrl-C handling.
pub(crate) fn drive(
    job: JobHandle,
    interrupted: &AtomicBool,
    progress: bool,
    locale: Locale,
    start_at: Option<SystemTime>,
) -> JobReport {
    let mut next = Instant::now() + Duration::from_secs(1);
    let mut stopping = false;
    while !job.is_finished() {
        std::thread::sleep(Duration::from_millis(50));
        if !stopping && interrupted.load(Ordering::SeqCst) {
            eprintln!("{}", t(locale, "tx-interrupted"));
            job.stop();
            stopping = true;
        }
        if progress && Instant::now() >= next {
            eprintln!("{}", progress_line(&job, locale, start_at));
            next += Duration::from_secs(1);
        }
    }
    job.wait()
}

/// Prints serialised JSON; `false` on failure (already reported).
pub(crate) fn print_json(json: serde_json::Result<String>, locale: Locale) -> bool {
    match json {
        Ok(s) => {
            println!("{s}");
            true
        }
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "tx-json-error", &[("error", e.to_string())])
            );
            false
        }
    }
}

fn limit_text(r: &Resolved, remaining: Option<u64>, locale: Locale) -> String {
    match (r.count, r.duration) {
        (Some(c), Some(d)) => tr(
            locale,
            "send-limit-count-or-duration",
            &[("count", c.to_string()), ("duration", format!("{d:?}"))],
        ),
        (Some(c), None) => tr(locale, "send-limit-count", &[("count", c.to_string())]),
        (None, Some(d)) => tr(
            locale,
            "send-limit-duration",
            &[("duration", format!("{d:?}"))],
        ),
        (None, None) => remaining.map_or_else(
            || t(locale, "send-limit-forever"),
            |n| tr(locale, "send-limit-count", &[("count", n.to_string())]),
        ),
    }
}

/// Runs `wpg send`.
#[allow(clippy::needless_pass_by_value)] // uniform `cmd::*::run(args)` dispatch
#[allow(clippy::too_many_lines)]
pub(crate) fn run(args: crate::SendArgs) -> ExitCode {
    let locale = cli_locale();
    let (_, mut compiled) = match load_compiled(&args.template, &args.vars, locale) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let r = match resolve(&args, &compiled.tx, locale) {
        Ok(r) => r,
        Err(m) => {
            eprintln!("{m}");
            return ExitCode::from(2);
        }
    };
    let Some(hint) = plan_hint(&args.plan) else {
        eprintln!("{}", t(locale, "send-bad-plan"));
        return ExitCode::from(2);
    };
    let workers = match args.tuning.check_workers(&r.backend, locale) {
        Ok(n) => n,
        Err(code) => return code,
    };

    // An explicit count makes finite generators cycle up to that many packets; a
    // duration without a count loops the stream until the time is up.
    if let Some(c) = r.count {
        compiled.stream.count = Some(c);
    }
    let exp = match Expansion::new(&compiled.stream, None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "{}",
                tr(locale, "send-stream-error", &[("error", e.to_string())])
            );
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
            eprintln!(
                "{}",
                tr(locale, "send-render-error", &[("error", e.to_string())])
            );
            return ExitCode::FAILURE;
        }
    };
    let rendered = match (rendered, loop_stream) {
        (Rendered::Pre(p), true) => Rendered::Pre(PreRendered::new(p.into_frames(), None)),
        (Rendered::Fly(f), true) => Rendered::Fly(Box::new(f.cycling())),
        (other, false) => other,
    };
    let remaining = rendered.remaining();
    let sources: Vec<Box<dyn FrameSource>> = rendered
        .split(workers)
        .into_iter()
        .map(|s| Box::new(s) as Box<dyn FrameSource>)
        .collect();

    let opts = args.tuning.options(64);
    let backends = match backend::open_workers(&r.backend, &r.iface, &opts, sources.len()) {
        Ok(b) => b,
        Err(e) => {
            report_open_error(locale, &r.backend, &r.iface, &e);
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "{}",
        tr(
            locale,
            "send-starting",
            &[
                ("iface", r.iface.clone()),
                ("backend", r.backend.clone()),
                ("detail", paren(backends[0].detail().as_deref())),
                ("rate", r.rate.to_string()),
                ("burst", r.burst.to_string()),
                ("limit", limit_text(&r, remaining, locale)),
                ("workers", backends.len().to_string()),
                ("strategy", format!("{:?}", plan.strategy)),
                ("reason", format!("{:?}", plan.reason)),
            ],
        )
    );
    if let Some(d) = r.ramp_up {
        eprintln!(
            "{}",
            tr(
                locale,
                "send-ramp-up",
                &[("rate", r.rate.to_string()), ("duration", format!("{d:?}"))],
            )
        );
    }
    if let (Some(at), Some(text)) = (r.start_at, &args.start_at) {
        if let Ok(wait) = at.duration_since(SystemTime::now()) {
            eprintln!(
                "{}",
                tr(
                    locale,
                    "send-scheduled",
                    &[
                        ("time", text.clone()),
                        ("secs", format!("{:.1}", wait.as_secs_f64())),
                    ],
                )
            );
        }
    }
    let interrupted = ctrl_c_flag(locale);
    let job = match TxJob::start_workers(
        backends.into_iter().zip(sources).collect(),
        JobConfig {
            count: r.count,
            duration: r.duration,
            rate: r.rate,
            burst: r.burst,
            pin_core: opts.pin_core,
            pin_cores: args.tuning.pin_cores_for(&r.iface, workers),
            workers,
            batch_size: opts.batch_size,
            ramp_up: r.ramp_up,
            start_at: r.start_at,
            signature: args.signature.then_some(SignatureConfig {
                stream_id_base: args.stream_id,
            }),
            pacer: PacerConfig {
                wait: args.wait.unwrap_or(if opts.busy_poll {
                    WaitMode::BusyPoll
                } else {
                    WaitMode::default()
                }),
                clock: args.clock,
                ..PacerConfig::default()
            },
            ..JobConfig::default()
        },
    ) {
        Ok(j) => j,
        Err(e) => {
            let m = tr(locale, "tx-start-failed", &[("error", e.to_string())]);
            report_tx_error(locale, &m, &e);
            return ExitCode::FAILURE;
        }
    };
    let report = drive(job, &interrupted, true, locale, r.start_at);
    if args.json {
        if !print_json(serde_json::to_string_pretty(&report), locale) {
            return ExitCode::FAILURE;
        }
    } else {
        print_summary(&report, locale);
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

    #[test]
    fn localised_state_text() {
        let s = JobState::Finished(FinishReason::CountReached);
        assert_eq!(state_text(&s, Locale::En), "finished (count reached)");
        assert_eq!(state_text(&s, Locale::Ja), "完了 (指定数に到達)");
        assert_eq!(
            state_text(&JobState::Failed("x".into()), Locale::En),
            "failed: x"
        );
    }
}
