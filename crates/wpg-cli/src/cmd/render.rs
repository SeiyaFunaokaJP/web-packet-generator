//! `wpg render` subcommand: template to pcapng without transmitting.

use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;

use wpg_i18n::{t, t_args, FluentArgs};
use wpg_protocols::catalog;
use wpg_template::render::render_pcapng;
use wpg_template::TemplateError;

use crate::cmd::cli_locale;
use crate::cmd::template::load_compiled;

/// Runs `wpg render`.
pub(crate) fn run(args: &crate::RenderArgs) -> ExitCode {
    let locale = cli_locale();
    let (_, compiled) = match load_compiled(&args.template, &args.vars, locale) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let file = match File::create(&args.output) {
        Ok(f) => BufWriter::new(f),
        Err(e) => {
            eprintln!("{}: {e}", args.output.display());
            return ExitCode::FAILURE;
        }
    };
    match render_pcapng(&compiled, catalog(), args.count, file) {
        Ok(n) => {
            let mut a = FluentArgs::new();
            a.set("len", i64::try_from(n).unwrap_or(i64::MAX));
            a.set("value", args.output.display().to_string());
            println!("{}", t_args(locale, "render-done", &a));
            ExitCode::SUCCESS
        }
        Err(TemplateError::Unbounded) => {
            eprintln!("{}", t(locale, "render-unbounded"));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
