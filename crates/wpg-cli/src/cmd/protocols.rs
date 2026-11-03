//! User-defined protocols (`--protocols-dir`, `[protocols] dirs`): loads the
//! extension files and installs them into the process-wide catalog before a
//! subcommand runs (see [`wpg_protocols::extension`]), and `wpg protocols`
//! (`list`, `show`, `check`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};
use serde_json::{json, Value};
use wpg_i18n::{t, t_args, FluentArgs, Locale};
use wpg_protocols::extension::{self, Extension, ExtensionError};

use crate::cmd::cli_locale;

/// Loads and installs the extensions of `dirs` (nothing to do when empty) and
/// remembers the directories for a later reload. Prints a localised message and
/// returns exit status 2 on errors.
pub(crate) fn install(dirs: &[PathBuf], locale: Locale) -> Result<(), ExitCode> {
    if dirs.is_empty() {
        return Ok(());
    }
    match extension::install_dirs(dirs) {
        Ok(exts) => {
            let count = exts.iter().map(|e| e.protocols.len()).sum::<usize>();
            let mut a = FluentArgs::new();
            a.set("count", i64::try_from(count).unwrap_or(i64::MAX));
            tracing::info!("{}", t_args(locale, "protocols-ext-loaded", &a));
            Ok(())
        }
        Err(e) => {
            let mut a = FluentArgs::new();
            a.set("file", e.source);
            a.set("error", e.reason);
            eprintln!("{}", t_args(locale, "protocols-ext-error", &a));
            Err(ExitCode::from(2))
        }
    }
}

/// Arguments of `wpg protocols`.
#[derive(Debug, Args)]
pub(crate) struct ProtocolsArgs {
    #[command(subcommand)]
    pub(crate) action: ProtocolsAction,
}

/// Actions of `wpg protocols`.
#[derive(Debug, Subcommand)]
pub(crate) enum ProtocolsAction {
    /// List the protocols: built-in ones and those of `--protocols-dir`.
    List {
        /// Output JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print a protocol definition, as an extension file to start a new protocol
    /// from (change its `id` first).
    Show {
        /// Protocol id (`udp`, `ipv4`, ...).
        id: String,
        /// Output the definition as JSON instead of YAML.
        #[arg(long)]
        json: bool,
    },
    /// Check user-defined protocol files (exit status 1 on errors).
    ///
    /// The files are checked together, with the built-in protocols and the other
    /// files of `--protocols-dir`.
    Check {
        /// Extension files (`*.yaml`) or directories holding them.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
}

/// One protocol of `wpg protocols list`.
#[derive(Debug)]
struct Listed {
    id: String,
    name: String,
    layer: String,
    /// File of a user-defined protocol; `null` for a built-in one.
    source: Option<String>,
}

/// Runs `wpg protocols`.
pub(crate) fn run(args: &ProtocolsArgs) -> ExitCode {
    let locale = cli_locale();
    match &args.action {
        ProtocolsAction::List { json } => list(locale, *json),
        ProtocolsAction::Show { id, json } => show(locale, id, *json),
        ProtocolsAction::Check { paths } => check(locale, paths),
    }
}

fn listed() -> Vec<Listed> {
    let exts = extension::extensions();
    wpg_protocols::catalog()
        .iter()
        .map(|p| Listed {
            id: p.id.clone(),
            name: p.name.clone(),
            layer: serde_json::to_value(p.layer)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            source: exts
                .iter()
                .find(|e| e.protocols.iter().any(|d| d.id == p.id))
                .map(|e| e.source.clone()),
        })
        .collect()
}

fn list(locale: Locale, json: bool) -> ExitCode {
    let rows = listed();
    if json {
        let v: Vec<Value> = rows
            .iter()
            .map(|r| json!({ "id": r.id, "name": r.name, "layer": r.layer, "source": r.source }))
            .collect();
        return print_json(&Value::Array(v));
    }
    let builtin = t(locale, "protocols-source-builtin");
    let mut table = vec![[
        "protocols-col-id",
        "protocols-col-name",
        "protocols-col-layer",
        "protocols-col-source",
    ]
    .map(|k| t(locale, k))];
    table.extend(rows.into_iter().map(|r| {
        [
            r.id,
            r.name,
            r.layer,
            r.source.unwrap_or_else(|| builtin.clone()),
        ]
    }));
    print!("{}", render_table(&table));
    ExitCode::SUCCESS
}

fn show(locale: Locale, id: &str, json: bool) -> ExitCode {
    let Some(def) = wpg_protocols::catalog().get(id) else {
        let mut a = FluentArgs::new();
        a.set("id", id.to_owned());
        eprintln!("{}", t_args(locale, "protocols-unknown", &a));
        return ExitCode::FAILURE;
    };
    if json {
        return match serde_json::to_value(def) {
            Ok(v) => print_json(&v),
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    let ext = Extension {
        protocols: vec![def.clone()],
        ..Extension::default()
    };
    match ext.to_yaml() {
        Ok(yaml) => {
            print!("{yaml}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Every extension file named by `paths` (directories expanded), or the error
/// of a path that cannot be read.
fn expand(paths: &[PathBuf]) -> Vec<Result<PathBuf, ExtensionError>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            match extension::files_in(p) {
                Ok(files) => out.extend(files.into_iter().map(Ok)),
                Err(e) => out.push(Err(e)),
            }
        } else {
            out.push(Ok(p.clone()));
        }
    }
    out
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn check(locale: Locale, paths: &[PathBuf]) -> ExitCode {
    let files = expand(paths);
    if files.is_empty() {
        let mut a = FluentArgs::new();
        a.set(
            "path",
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
        eprintln!("{}", t_args(locale, "protocols-check-none", &a));
        return ExitCode::FAILURE;
    }
    let mut errors: Vec<ExtensionError> = Vec::new();
    let mut checked: Vec<(PathBuf, Extension)> = Vec::new();
    for f in files {
        match f.and_then(|path| extension::load_file(&path).map(|e| (path, e))) {
            Ok(pair) => checked.push(pair),
            Err(e) => errors.push(e),
        }
    }
    // Merge with the installed files (`--protocols-dir`) that are not checked
    // here, so that references between files resolve.
    if errors.is_empty() {
        let mut all: Vec<Extension> = extension::extensions()
            .iter()
            .filter(|e| {
                !checked
                    .iter()
                    .any(|(p, _)| same_file(p, Path::new(&e.source)))
            })
            .cloned()
            .collect();
        all.extend(checked.iter().map(|(_, e)| e.clone()));
        if let Err(e) = extension::with_extensions(&all) {
            errors.push(e);
        }
    }
    for e in &errors {
        let mut a = FluentArgs::new();
        a.set("file", e.source.clone());
        a.set("error", e.reason.clone());
        eprintln!("{}", t_args(locale, "protocols-check-error", &a));
    }
    if !errors.is_empty() {
        return ExitCode::FAILURE;
    }
    for (_, e) in &checked {
        let mut a = FluentArgs::new();
        a.set("file", e.source.clone());
        a.set(
            "protocols",
            i64::try_from(e.protocols.len()).unwrap_or(i64::MAX),
        );
        a.set("bindings", i64::try_from(e.bind.len()).unwrap_or(i64::MAX));
        println!("{}", t_args(locale, "protocols-check-ok", &a));
    }
    ExitCode::SUCCESS
}

fn print_json(v: &Value) -> ExitCode {
    match serde_json::to_string_pretty(v) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Terminal cell width: CJK / kana count as two columns.
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| if u32::from(c) >= 0x2e80 { 2 } else { 1 })
        .sum()
}

/// Left-aligned columns separated by two spaces.
fn render_table<const N: usize>(rows: &[[String; N]]) -> String {
    let widths: Vec<usize> = (0..N)
        .map(|c| rows.iter().map(|r| display_width(&r[c])).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for r in rows {
        for (c, cell) in r.iter().enumerate() {
            out.push_str(cell);
            if c + 1 < N {
                let pad = widths[c] - display_width(cell) + 2;
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pads_columns() {
        let rows = [
            ["id".to_owned(), "name".to_owned()],
            ["ipv4".to_owned(), "IPv4".to_owned()],
        ];
        assert_eq!(render_table(&rows), "id    name\nipv4  IPv4\n");
    }

    #[test]
    fn lists_built_in_protocols() {
        let rows = listed();
        let udp = rows.iter().find(|r| r.id == "udp").unwrap();
        assert_eq!(udp.layer, "l4");
        assert!(udp.source.is_none());
    }
}
