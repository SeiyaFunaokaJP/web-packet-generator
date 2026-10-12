//! Measures how precisely the [`Pacer`] spaces frames (results: `docs/perf.md`).
//!
//! ```sh
//! cargo run --release -p wpg-tx --example pacing -- [SECONDS] [--json]
//! ```
//!
//! For every wait mode × clock × rate it runs a pacer for `SECONDS` (default 1) on
//! a pinned thread, charging every granted frame (at most 64 per grant, like a TX
//! batch), three times; the run with the median p99 is reported. Lateness is how
//! late the pacer woke up relative to the deadline it was waiting for (its
//! [`RateAccuracy`](wpg_tx::rate::RateAccuracy) jitter fields), and the achieved
//! / target ratio is over the whole run.

use std::time::{Duration, Instant};

use wpg_tx::clock::{tsc_calibration, ClockSource};
use wpg_tx::pin;
use wpg_tx::rate::{reduce_timer_slack, Pacer, PacerConfig, Rate, RateAccuracy, WaitMode};

struct Row {
    wait: &'static str,
    clock: &'static str,
    tsc: bool,
    pps: f64,
    ratio: f64,
    mean: f64,
    p50: f64,
    p99: f64,
    max: f64,
}

fn run(wait: WaitMode, clock: ClockSource, pps: f64, secs: f64) -> RateAccuracy {
    let mut p = Pacer::new(
        Rate::Pps(pps),
        PacerConfig {
            wait,
            clock,
            ..PacerConfig::default()
        },
    )
    .expect("valid rate");
    let end = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < end {
        if let Some(g) = p.poll(Duration::from_millis(5)) {
            for _ in 0..g.min(64) {
                p.charge(60);
            }
        }
    }
    p.accuracy()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let secs: f64 = args.iter().find_map(|a| a.parse().ok()).unwrap_or(1.0);
    reduce_timer_slack();
    let waits: [(&str, WaitMode); 4] = [
        ("hybrid", WaitMode::default()),
        ("sleep", WaitMode::Sleep),
        ("spin", WaitMode::Spin),
        ("busy-poll", WaitMode::BusyPoll),
    ];
    let clocks: [(&str, ClockSource); 2] = [
        ("monotonic", ClockSource::Monotonic),
        ("tsc", ClockSource::Tsc),
    ];
    // Pin to the last allowed core (away from core 0's housekeeping).
    if let Some(&core) = pin::allowed_cpus().last() {
        let _ = pin::pin_current_thread(core);
    }
    let ns = |d: Duration| d.as_secs_f64() * 1e9;
    let mut rows = Vec::new();
    for &(wn, w) in &waits {
        for &(cn, c) in &clocks {
            for pps in [1e3, 1e4, 1e5, 1e6] {
                let mut runs: Vec<RateAccuracy> = (0..3).map(|_| run(w, c, pps, secs)).collect();
                runs.sort_by_key(|a| a.p99_jitter);
                let a = runs[1];
                rows.push(Row {
                    wait: wn,
                    clock: cn,
                    tsc: a.tsc,
                    pps,
                    ratio: a.ratio.unwrap_or(0.0),
                    mean: ns(a.mean_jitter),
                    p50: ns(a.p50_jitter),
                    p99: ns(a.p99_jitter),
                    max: ns(a.max_jitter),
                });
            }
        }
    }
    if json {
        let items: Vec<String> = rows
            .iter()
            .map(|r| {
                format!(
                    "{{\"wait\":\"{}\",\"clock\":\"{}\",\"tsc\":{},\"pps\":{},\"ratio\":{:.5},\"mean_ns\":{:.0},\"p50_ns\":{:.0},\"p99_ns\":{:.0},\"max_ns\":{:.0}}}",
                    r.wait, r.clock, r.tsc, r.pps, r.ratio, r.mean, r.p50, r.p99, r.max
                )
            })
            .collect();
        println!("[\n  {}\n]", items.join(",\n  "));
        return;
    }
    println!(
        "TSC: {}",
        tsc_calibration().map_or("unavailable".to_owned(), |c| format!(
            "{:.3} GHz",
            c.hz / 1e9
        ))
    );
    println!("| wait | clock | rate | achieved | mean late | p50 late | p99 late | max late |");
    println!("|---|---|---|---|---|---|---|---|");
    for r in &rows {
        println!(
            "| {} | {}{} | {} pps | {:.3} % | {:.2} µs | {:.2} µs | {:.2} µs | {:.1} µs |",
            r.wait,
            r.clock,
            if r.clock == "tsc" && !r.tsc {
                " (fallback)"
            } else {
                ""
            },
            r.pps,
            r.ratio * 100.0,
            r.mean / 1e3,
            r.p50 / 1e3,
            r.p99 / 1e3,
            r.max / 1e3,
        );
    }
}
