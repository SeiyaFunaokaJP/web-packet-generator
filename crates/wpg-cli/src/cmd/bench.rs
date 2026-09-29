//! `wpg bench` subcommand: unlimited-rate transmission of a synthetic frame
//! (EtherType 0x88b5) to measure backend throughput.

use std::process::ExitCode;
use std::time::Duration;

use wpg_tx::bench::{bench_result, start_bench, BenchOptions, BenchResult};

use crate::cmd::send::{ctrl_c_flag, drive, report_tx_error, si};

fn print_table(r: &BenchResult) {
    println!(
        "backend    {}{}",
        r.backend,
        r.detail
            .as_ref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default()
    );
    println!("iface      {}", r.iface);
    println!(
        "frame      {} B (+4 FCS)  batch {}",
        r.frame_size, r.batch_size
    );
    println!("elapsed    {:.3} s", r.elapsed.as_secs_f64());
    println!("packets    {}", r.packets);
    println!("dropped    {}", r.dropped);
    println!("rate       {}pps", si(r.pps));
    println!("           {}bps (L2)  {}bps (L1)", si(r.bps), si(r.l1_bps));
}

/// Runs `wpg bench`.
#[allow(clippy::needless_pass_by_value)] // uniform `cmd::*::run(args)` dispatch
pub(crate) fn run(args: crate::BenchArgs) -> ExitCode {
    let backend = args.backend.clone().unwrap_or_else(|| "null".to_owned());
    let iface = match (&args.iface, backend.as_str()) {
        (Some(i), _) => i.clone(),
        (None, "null") => "null0".to_owned(),
        (None, _) => {
            eprintln!("--iface is required for the {backend} backend");
            return ExitCode::from(2);
        }
    };
    let Ok(duration) = Duration::try_from_secs_f64(args.duration) else {
        eprintln!("--duration must be a non-negative number of seconds");
        return ExitCode::from(2);
    };
    let opts = BenchOptions {
        tx: args.tuning.options(BenchOptions::default().tx.batch_size),
    };
    let job = match start_bench(&backend, &iface, duration, args.frame_size, &opts) {
        Ok(j) => j,
        Err(e) => {
            report_tx_error(&format!("cannot open {backend} on `{iface}`"), &e);
            return ExitCode::FAILURE;
        }
    };
    let interrupted = ctrl_c_flag();
    let report = drive(job, &interrupted, !args.json);
    let result = match bench_result(&report, &iface, args.frame_size, &opts) {
        Ok(r) => r,
        Err(e) => {
            report_tx_error("benchmark failed", &e);
            return ExitCode::FAILURE;
        }
    };
    if args.json {
        match serde_json::to_string_pretty(&result) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("json: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print_table(&result);
    }
    ExitCode::SUCCESS
}
