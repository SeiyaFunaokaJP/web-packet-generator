//! `wpg bench` subcommand: unlimited-rate transmission to measure throughput.
//!
//! By default a synthetic frame (EtherType 0x88b5) is replayed, measuring the
//! backend. With `--template` the template is generated on the fly (in-place
//! patching of the generated fields), measuring generation plus backend — with
//! `--backend null` (the default) generation alone. `--workers N` runs N TX
//! threads, each with its own backend instance.

use std::process::ExitCode;
use std::time::Duration;

use wpg_core::stream::Expansion;
use wpg_i18n::{t, Locale};
use wpg_protocols::catalog;
use wpg_tx::bench::{bench_result, start_bench, start_bench_sources, BenchOptions, BenchResult};
use wpg_tx::job::JobHandle;
use wpg_tx::source::{FrameSource, OnTheFly};

use crate::cmd::cli_locale;
use crate::cmd::send::{
    ctrl_c_flag, default_iface, drive, paren, print_json, report_open_error, report_tx_error, si,
    tr,
};
use crate::cmd::template::load_compiled;

fn print_table(r: &BenchResult, locale: Locale, fast: Option<bool>) {
    let line = |key: &str, args: &[(&str, String)]| println!("{}", tr(locale, key, args));
    line(
        "summary-backend",
        &[
            ("backend", r.backend.clone()),
            ("detail", paren(r.detail.as_deref())),
        ],
    );
    line("bench-iface", &[("iface", r.iface.clone())]);
    if let Some(tpl) = &r.template {
        let mode = if fast.unwrap_or(false) {
            "bench-mode-patch"
        } else {
            "bench-mode-rebuild"
        };
        line(
            "bench-template",
            &[("value", tpl.clone()), ("mode", t(locale, mode))],
        );
    }
    if r.workers > 1 {
        line("summary-workers", &[("workers", r.workers.to_string())]);
        for (i, d) in r.worker_details.iter().enumerate() {
            line(
                "bench-worker",
                &[("index", i.to_string()), ("detail", d.clone())],
            );
        }
    }
    line(
        "bench-frame",
        &[
            ("len", r.frame_size.to_string()),
            ("batch", r.batch_size.to_string()),
        ],
    );
    line(
        "summary-elapsed",
        &[("secs", format!("{:.3}", r.elapsed.as_secs_f64()))],
    );
    line("summary-packets", &[("value", r.packets.to_string())]);
    line("summary-dropped", &[("value", r.dropped.to_string())]);
    line("bench-rate", &[("pps", si(r.pps))]);
    // Continuation line, aligned under the value column of `bench-rate`.
    println!(
        "{:11}{}",
        "",
        tr(
            locale,
            "bench-bits",
            &[("bps", si(r.bps)), ("l1", si(r.l1_bps))]
        )
    );
}

/// On-the-fly sources for `--template`, the first frame's length and whether the
/// generated fields are patched in place.
struct TemplateSources {
    sources: Vec<Box<dyn FrameSource>>,
    frame_len: usize,
    fast: bool,
}

fn template_sources(
    args: &crate::BenchArgs,
    path: &std::path::Path,
    workers: usize,
    locale: Locale,
) -> Result<TemplateSources, ExitCode> {
    let (_, compiled) = load_compiled(path, &args.vars, locale)?;
    let exp = Expansion::new(&compiled.stream, None).map_err(|e| {
        eprintln!(
            "{}",
            tr(locale, "send-stream-error", &[("error", e.to_string())])
        );
        ExitCode::FAILURE
    })?;
    let render_err = |e: &dyn std::fmt::Display| {
        eprintln!(
            "{}",
            tr(locale, "send-render-error", &[("error", e.to_string())])
        );
        ExitCode::FAILURE
    };
    let mut fly = OnTheFly::new(compiled.packet, exp, catalog()).map_err(|e| render_err(&e))?;
    let mut probe = Vec::new();
    let len = fly.render(0, &mut probe).map_err(|e| render_err(&e))?;
    let fast = fly.is_fast();
    let sources = fly
        .cycling()
        .split(workers)
        .into_iter()
        .map(|f| Box::new(f) as Box<dyn FrameSource>)
        .collect();
    Ok(TemplateSources {
        sources,
        frame_len: len,
        fast,
    })
}

/// Runs `wpg bench`.
#[allow(clippy::needless_pass_by_value)] // uniform `cmd::*::run(args)` dispatch
pub(crate) fn run(args: crate::BenchArgs) -> ExitCode {
    let locale = cli_locale();
    let backend = args.backend.clone().unwrap_or_else(|| "null".to_owned());
    let iface = match default_iface(args.iface.as_ref(), &backend, locale) {
        Ok(i) => i,
        Err(m) => {
            eprintln!("{m}");
            return ExitCode::from(2);
        }
    };
    let Ok(duration) = Duration::try_from_secs_f64(args.duration) else {
        eprintln!("{}", t(locale, "bench-bad-duration"));
        return ExitCode::from(2);
    };
    let workers = match args.tuning.check_workers(&backend, locale) {
        Ok(n) => n,
        Err(code) => return code,
    };
    let opts = BenchOptions {
        tx: args.tuning.options(BenchOptions::default().tx.batch_size),
        workers,
        pin_cores: args.tuning.pin_cores_for(&iface, workers),
    };
    let mut frame_size = args.frame_size;
    let mut fast = None;
    let started: Result<JobHandle, _> = match &args.template {
        Some(path) => {
            let ts = match template_sources(&args, path, workers, locale) {
                Ok(v) => v,
                Err(code) => return code,
            };
            frame_size = ts.frame_len;
            fast = Some(ts.fast);
            start_bench_sources(&backend, &iface, duration, ts.sources, &opts)
        }
        None => start_bench(&backend, &iface, duration, frame_size, &opts),
    };
    let job = match started {
        Ok(j) => j,
        Err(e) => {
            report_open_error(locale, &backend, &iface, &e);
            return ExitCode::FAILURE;
        }
    };
    let interrupted = ctrl_c_flag(locale);
    let report = drive(job, &interrupted, !args.json, locale, None);
    let mut result = match bench_result(&report, &iface, frame_size, &opts) {
        Ok(r) => r,
        Err(e) => {
            let m = tr(locale, "bench-failed", &[("error", e.to_string())]);
            report_tx_error(locale, &m, &e);
            return ExitCode::FAILURE;
        }
    };
    result.template = args.template.as_ref().map(|p| p.display().to_string());
    if args.json {
        if !print_json(serde_json::to_string_pretty(&result), locale) {
            return ExitCode::FAILURE;
        }
    } else {
        print_table(&result, locale, fast);
    }
    ExitCode::SUCCESS
}
