//! `wpg render` subcommand: template to pcapng without transmitting.
//!
//! `-o -` writes the pcapng to standard output; the summary then goes to stderr
//! so that nothing else ends up in the stream. `--json` prints the summary as
//! JSON (`packets`, `bytes`, `path`) instead of the localised line.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use wpg_i18n::{t, t_args, FluentArgs};
use wpg_protocols::catalog;
use wpg_template::render::render_pcapng;
use wpg_template::TemplateError;

use crate::cmd::cli_locale;
use crate::cmd::send::print_json;
use crate::cmd::template::load_compiled;

/// Counts the bytes written through it.
struct Counting<W> {
    inner: W,
    bytes: u64,
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Runs `wpg render`.
pub(crate) fn run(args: &crate::RenderArgs) -> ExitCode {
    let locale = cli_locale();
    let to_stdout = args.output == Path::new("-");
    if to_stdout && args.json {
        eprintln!("{}", t(locale, "render-json-stdout"));
        return ExitCode::from(2);
    }
    let (_, compiled) = match load_compiled(&args.template, &args.inputs, locale) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let sink: Box<dyn Write> = if to_stdout {
        Box::new(io::stdout().lock())
    } else {
        match File::create(&args.output) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("{}: {e}", args.output.display());
                return ExitCode::FAILURE;
            }
        }
    };
    let mut out = Counting {
        inner: BufWriter::new(sink),
        bytes: 0,
    };
    let result = render_pcapng(&compiled, catalog(), args.count, &mut out)
        .and_then(|n| out.flush().map(|()| n).map_err(TemplateError::Write));
    match result {
        Ok(n) => {
            let path = args.output.display().to_string();
            if args.json {
                let json = serde_json::json!({
                    "packets": n,
                    "bytes": out.bytes,
                    "path": path,
                });
                if !print_json(serde_json::to_string_pretty(&json), locale) {
                    return ExitCode::FAILURE;
                }
            } else {
                let mut a = FluentArgs::new();
                a.set("len", i64::try_from(n).unwrap_or(i64::MAX));
                a.set("value", path);
                let line = t_args(locale, "render-done", &a);
                if to_stdout {
                    eprintln!("{line}");
                } else {
                    println!("{line}");
                }
            }
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
