//! `wpg template` subcommand: lint, list, show, schema, migrate, export, import.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use wpg_core::notation::format_value;
use wpg_core::stream::Expansion;
use wpg_core::{FieldLayout, FieldType, FieldValue, ProtocolDef};
use wpg_i18n::{t, t_args, t_diag, FluentArgs, Locale};
use wpg_protocols::catalog;
use wpg_template::bundle::slug;
use wpg_template::{
    apply_overrides, compile, export_bundle, import_bundle, lint, schema_version, Compiled,
    LintIssue, LintLevel, Template, TemplateError, VarOverrides, SCHEMA_VERSION,
};

use serde_json::{json, Value};

use crate::cmd::cli_locale;
use crate::cmd::send::print_json;
use crate::{TemplateAction, TemplateArgs};

/// Default template library directory of `wpg template list`.
const DEFAULT_DIR: &str = "templates";

/// Exit status for template validation errors (docs/spec/08-cli.md).
pub(crate) const EXIT_INVALID_TEMPLATE: u8 = 3;

/// Exit status for a template that failed to load: 1 when the file cannot be
/// read, else [`EXIT_INVALID_TEMPLATE`].
fn load_error_code(err: &TemplateError) -> ExitCode {
    if matches!(err, TemplateError::File { .. }) {
        ExitCode::FAILURE
    } else {
        ExitCode::from(EXIT_INVALID_TEMPLATE)
    }
}

/// Runs `wpg template`.
pub(crate) fn run(args: TemplateArgs) -> ExitCode {
    let locale = cli_locale();
    match args.action {
        TemplateAction::Lint { files, json } => lint_files(&files, json, locale),
        TemplateAction::List { dir, json } => list(
            dir.as_deref().unwrap_or(Path::new(DEFAULT_DIR)),
            json,
            locale,
        ),
        TemplateAction::Show { file, vars, json } => show(&file, &vars, json, locale),
        TemplateAction::Schema => {
            match serde_json::to_string_pretty(&Template::json_schema()) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            }
            ExitCode::SUCCESS
        }
        TemplateAction::Export {
            templates,
            output,
            attach,
            dir,
        } => export(
            &templates,
            &output,
            &attach,
            dir.as_deref().unwrap_or(Path::new(DEFAULT_DIR)),
            locale,
        ),
        TemplateAction::Migrate {
            paths,
            check,
            no_backup,
        } => migrate(&paths, check, !no_backup, locale),
        TemplateAction::Import {
            bundle,
            output,
            force,
        } => import(&bundle, &output, force, locale),
    }
}

/// Parses repeated `--var name=value` arguments.
pub(crate) fn parse_vars(vars: &[String], locale: Locale) -> Result<VarOverrides, String> {
    let mut out = VarOverrides::new();
    for v in vars {
        let Some((k, val)) = v.split_once('=').filter(|(k, _)| !k.trim().is_empty()) else {
            let mut a = FluentArgs::new();
            a.set("value", v.as_str());
            return Err(t_args(locale, "cli-bad-var", &a));
        };
        out.insert(k.trim().to_owned(), val.to_owned());
    }
    Ok(out)
}

/// Localised text of a lint issue (Fluent by key, English fallback).
pub(crate) fn issue_text(issue: &LintIssue, locale: Locale) -> String {
    let mut args = FluentArgs::new();
    for (k, v) in &issue.args {
        args.set(k.as_str(), v.as_str());
    }
    let text = t_args(locale, &issue.key, &args);
    if text == issue.key {
        issue.message.clone()
    } else {
        text
    }
}

fn level_text(level: LintLevel, locale: Locale) -> String {
    t(
        locale,
        match level {
            LintLevel::Error => "lint-level-error",
            LintLevel::Warning => "lint-level-warning",
        },
    )
}

/// Prints the failure of a template load / compile (with all issues).
pub(crate) fn report_error(path: &Path, err: &TemplateError, locale: Locale) {
    match err {
        TemplateError::Lint(issues) => {
            for i in issues {
                eprintln!(
                    "{}: {}: {}: {}",
                    path.display(),
                    level_text(i.level, locale),
                    i.path,
                    issue_text(i, locale)
                );
            }
        }
        other => {
            let mut a = FluentArgs::new();
            a.set("value", path.display().to_string());
            a.set("error", other.to_string());
            eprintln!("{}", t_args(locale, "template-lint-load-error", &a));
        }
    }
}

/// Loads and compiles a template with the built-in catalog. Errors: exit status
/// 2 for a bad `--var`, 1 for an unreadable file, 3 for an invalid template.
pub(crate) fn load_compiled(
    path: &Path,
    vars: &[String],
    locale: Locale,
) -> Result<(Template, Compiled), ExitCode> {
    let overrides = parse_vars(vars, locale).map_err(|m| {
        eprintln!("{m}");
        ExitCode::from(2)
    })?;
    let template = Template::from_path(path).map_err(|e| {
        report_error(path, &e, locale);
        load_error_code(&e)
    })?;
    match compile(&template, &overrides, catalog()) {
        Ok(c) => Ok((template, c)),
        Err(e) => {
            report_error(path, &e, locale);
            Err(load_error_code(&e))
        }
    }
}

/// JSON of a lint issue (`level`, `code`, `path`, English `message`, `args`).
fn issue_json(i: &LintIssue) -> Value {
    json!({
        "level": i.level.to_string(),
        "code": i.key,
        "path": i.path,
        "message": i.message,
        "args": i.args,
    })
}

fn lint_files(files: &[PathBuf], json: bool, locale: Locale) -> ExitCode {
    let (mut errors, mut warnings, mut unreadable) = (0usize, 0usize, false);
    let mut results = Vec::new();
    for path in files {
        let issues = match Template::from_path(path) {
            Ok(t) => lint(&t, catalog()),
            Err(TemplateError::Lint(issues)) => issues,
            Err(e) => {
                errors += 1;
                unreadable |= matches!(e, TemplateError::File { .. });
                if json {
                    results.push(json!({
                        "file": path.display().to_string(),
                        "issues": [{
                            "level": LintLevel::Error.to_string(),
                            "code": "template-load-error",
                            "path": "",
                            "message": e.to_string(),
                        }],
                    }));
                } else {
                    report_error(path, &e, locale);
                }
                continue;
            }
        };
        for i in &issues {
            match i.level {
                LintLevel::Error => errors += 1,
                LintLevel::Warning => warnings += 1,
            }
        }
        if json {
            results.push(json!({
                "file": path.display().to_string(),
                "issues": issues.iter().map(issue_json).collect::<Vec<_>>(),
            }));
            continue;
        }
        if issues.is_empty() {
            let mut a = FluentArgs::new();
            a.set("value", path.display().to_string());
            println!("{}", t_args(locale, "template-lint-ok", &a));
        }
        for i in &issues {
            println!(
                "{}: {}: {}: {}",
                path.display(),
                level_text(i.level, locale),
                i.path,
                issue_text(i, locale)
            );
        }
    }
    if json {
        let doc = json!({ "files": results, "errors": errors, "warnings": warnings });
        if !print_json(serde_json::to_string_pretty(&doc), locale) {
            return ExitCode::FAILURE;
        }
    } else {
        let mut a = FluentArgs::new();
        a.set("fail", errors as i64);
        a.set("warn", warnings as i64);
        a.set("ok", files.len() as i64);
        println!("{}", t_args(locale, "template-lint-summary", &a));
    }
    if unreadable {
        ExitCode::FAILURE
    } else if errors > 0 {
        ExitCode::from(EXIT_INVALID_TEMPLATE)
    } else {
        ExitCode::SUCCESS
    }
}

/// File name without the `.wpg.yaml` / `.wpg.json` / `.yaml` suffix.
fn template_stem(p: &Path) -> String {
    let n = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    for suffix in [
        ".wpg.yaml",
        ".wpg.yml",
        ".wpg.json",
        ".yaml",
        ".yml",
        ".json",
    ] {
        if let Some(s) = n.strip_suffix(suffix) {
            return s.to_owned();
        }
    }
    n.to_owned()
}

/// Resolves an export argument: an existing file, else an id (file stem or name slug)
/// in the library directory.
fn resolve_template(arg: &str, dir: &Path) -> Option<PathBuf> {
    let direct = PathBuf::from(arg);
    if direct.is_file() {
        return Some(direct);
    }
    let mut files = Vec::new();
    collect(dir, &mut files);
    files.into_iter().find(|f| {
        template_stem(f) == arg
            || Template::from_path(f).is_ok_and(|t| slug(&t.meta.name) == slug(arg))
    })
}

fn fail_with(key: &str, value: &str, error: &str, locale: Locale) -> ExitCode {
    let mut a = FluentArgs::new();
    a.set("value", value);
    a.set("error", error);
    eprintln!("{}", t_args(locale, key, &a));
    ExitCode::FAILURE
}

fn export(
    templates: &[String],
    output: &Path,
    attach: &[PathBuf],
    dir: &Path,
    locale: Locale,
) -> ExitCode {
    if templates.is_empty() {
        eprintln!("{}", t(locale, "template-export-none"));
        return ExitCode::from(2);
    }
    let mut items = Vec::new();
    for arg in templates {
        let Some(path) = resolve_template(arg, dir) else {
            let mut a = FluentArgs::new();
            a.set("value", arg.as_str());
            eprintln!("{}", t_args(locale, "template-export-not-found", &a));
            return ExitCode::FAILURE;
        };
        match Template::from_path(&path) {
            Ok(t) => items.push((template_stem(&path), t)),
            Err(e) => {
                report_error(&path, &e, locale);
                return ExitCode::FAILURE;
            }
        }
    }
    let mut attachments = Vec::new();
    for path in attach {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment")
            .to_owned();
        match std::fs::read(path) {
            Ok(b) => attachments.push((name, b)),
            Err(e) => {
                return fail_with(
                    "template-export-error",
                    &path.display().to_string(),
                    &e.to_string(),
                    locale,
                )
            }
        }
    }
    let shown = output.display().to_string();
    let file = match std::fs::File::create(output) {
        Ok(f) => f,
        Err(e) => return fail_with("template-export-error", &shown, &e.to_string(), locale),
    };
    if let Err(e) = export_bundle(&items, &attachments, std::io::BufWriter::new(file)) {
        return fail_with("template-export-error", &shown, &e.to_string(), locale);
    }
    let mut a = FluentArgs::new();
    a.set("count", items.len() as i64);
    a.set("value", shown);
    println!("{}", t_args(locale, "template-export-done", &a));
    ExitCode::SUCCESS
}

fn import(bundle: &Path, output: &Path, force: bool, locale: Locale) -> ExitCode {
    let shown = bundle.display().to_string();
    let file = match std::fs::File::open(bundle) {
        Ok(f) => std::io::BufReader::new(f),
        Err(e) => return fail_with("template-import-error", &shown, &e.to_string(), locale),
    };
    let imported = match import_bundle(file, catalog()) {
        Ok(b) => b,
        Err(e) => return fail_with("template-import-error", &shown, &e.to_string(), locale),
    };
    if let Err(e) = std::fs::create_dir_all(output) {
        return fail_with(
            "template-import-error",
            &output.display().to_string(),
            &e.to_string(),
            locale,
        );
    }
    let (mut written, mut failed) = (0usize, false);
    for item in &imported.templates {
        let file_name = item.path.rsplit('/').next().unwrap_or(&item.path);
        let target = output.join(file_name);
        for i in &item.issues {
            failed |= i.level == LintLevel::Error;
            eprintln!(
                "{}: {}: {}: {}",
                item.path,
                level_text(i.level, locale),
                i.path,
                issue_text(i, locale)
            );
        }
        if target.exists() && !force {
            let mut a = FluentArgs::new();
            a.set("value", target.display().to_string());
            eprintln!("{}", t_args(locale, "template-import-exists", &a));
            failed = true;
            continue;
        }
        match item.template.save(&target) {
            Ok(()) => written += 1,
            Err(e) => {
                return fail_with(
                    "template-import-error",
                    &target.display().to_string(),
                    &e.to_string(),
                    locale,
                )
            }
        }
    }
    for (name, data) in &imported.attachments {
        let target = output.join(name);
        if target.exists() && !force {
            continue;
        }
        if let Err(e) = std::fs::write(&target, data) {
            return fail_with(
                "template-import-error",
                &target.display().to_string(),
                &e.to_string(),
                locale,
            );
        }
    }
    let mut a = FluentArgs::new();
    a.set("count", written as i64);
    a.set("value", shown);
    a.set("detail", output.display().to_string());
    println!("{}", t_args(locale, "template-import-done", &a));
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn is_template_file(p: &Path) -> bool {
    let n = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    [".wpg.yaml", ".wpg.yml", ".wpg.json"]
        .iter()
        .any(|s| n.ends_with(s))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect(&p, out);
        } else if is_template_file(&p) {
            out.push(p);
        }
    }
}

/// Outcome of `wpg template migrate` for one file.
#[derive(Debug, PartialEq, Eq)]
enum MigrateOutcome {
    /// Already at [`SCHEMA_VERSION`].
    Current,
    /// Older; would be migrated (`--check`).
    Outdated(u32),
    /// Rewritten from this version.
    Migrated(u32),
}

/// Migrates one file (pure apart from the file system; tested with temp files).
fn migrate_file(path: &Path, check: bool, backup: bool) -> Result<MigrateOutcome, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let found = schema_version(&text).map_err(|e| e.to_string())?;
    if found == SCHEMA_VERSION {
        return Ok(MigrateOutcome::Current);
    }
    // Validates the version range and the migrated shape before touching the file.
    let template = Template::from_path(path).map_err(|e| e.to_string())?;
    if check {
        return Ok(MigrateOutcome::Outdated(found));
    }
    if backup {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        std::fs::write(&bak, &text).map_err(|e| e.to_string())?;
    }
    template.save(path).map_err(|e| e.to_string())?;
    Ok(MigrateOutcome::Migrated(found))
}

fn migrate(paths: &[PathBuf], check: bool, backup: bool, locale: Locale) -> ExitCode {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            collect(p, &mut files);
        } else {
            files.push(p.clone());
        }
    }
    let (mut current, mut changed, mut failed) = (0i64, 0i64, 0i64);
    for f in &files {
        let mut a = FluentArgs::new();
        a.set("value", f.display().to_string());
        a.set("expected", i64::from(SCHEMA_VERSION));
        match migrate_file(f, check, backup) {
            Ok(MigrateOutcome::Current) => current += 1,
            Ok(MigrateOutcome::Outdated(v)) => {
                changed += 1;
                a.set("found", i64::from(v));
                println!("{}", t_args(locale, "template-migrate-outdated", &a));
            }
            Ok(MigrateOutcome::Migrated(v)) => {
                changed += 1;
                a.set("found", i64::from(v));
                println!("{}", t_args(locale, "template-migrate-done", &a));
            }
            Err(e) => {
                failed += 1;
                a.set("error", e);
                eprintln!("{}", t_args(locale, "template-migrate-error", &a));
            }
        }
    }
    let mut a = FluentArgs::new();
    a.set("ok", current);
    a.set("count", changed);
    a.set("fail", failed);
    a.set("expected", i64::from(SCHEMA_VERSION));
    let key = if check {
        "template-migrate-check-summary"
    } else {
        "template-migrate-summary"
    };
    println!("{}", t_args(locale, key, &a));
    if failed > 0 || (check && changed > 0) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn list(dir: &Path, json: bool, locale: Locale) -> ExitCode {
    let mut files = Vec::new();
    collect(dir, &mut files);
    if files.is_empty() && !json {
        let mut a = FluentArgs::new();
        a.set("value", dir.display().to_string());
        println!("{}", t_args(locale, "template-list-empty", &a));
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    let mut entries = Vec::new();
    for f in &files {
        match Template::from_path(f) {
            Ok(t) => {
                let stack: Vec<&str> = t.layers.iter().map(|l| l.protocol.as_str()).collect();
                if json {
                    entries.push(json!({
                        "name": t.meta.name,
                        "description": t.meta.description,
                        "tags": t.meta.tags,
                        "layers": stack,
                        "path": f.display().to_string(),
                    }));
                    continue;
                }
                let tags = if t.meta.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", t.meta.tags.join(", "))
                };
                println!(
                    "{}{}\n    {}\n    {}",
                    t.meta.name,
                    tags,
                    stack.join(" / "),
                    f.display()
                );
            }
            Err(e) => {
                failed = true;
                report_error(f, &e, locale);
            }
        }
    }
    if json && !print_json(serde_json::to_string_pretty(&entries), locale) {
        return ExitCode::FAILURE;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn show(file: &Path, vars: &[String], json: bool, locale: Locale) -> ExitCode {
    let (_, compiled) = match load_compiled(file, vars, locale) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let mut packet = compiled.packet.clone();
    match Expansion::new(&compiled.stream, None) {
        Ok(exp) => match exp.nth(0) {
            Ok(ov) => {
                if let Err(e) = apply_overrides(&mut packet, &ov) {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        },
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    }
    let built = match packet.build(catalog()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    if json {
        return if print_json(serde_json::to_string_pretty(&built_json(&built)), locale) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    let mut a = FluentArgs::new();
    a.set("len", built.bytes.len() as i64);
    println!("{}", t_args(locale, "template-show-hex", &a));
    print!("{}", hex_dump(&built.bytes));
    println!("\n{}", t(locale, "template-show-layout"));
    for (i, layer) in built.layout.iter().enumerate() {
        let def = wpg_core_lookup(&layer.protocol);
        println!(
            "[{i}] {}  bytes {}..{}",
            layer.protocol,
            layer.offset,
            layer.offset + layer.len
        );
        print_fields(&layer.fields, def, 1);
    }
    for tr in &built.trailers {
        println!(
            "({:?}  bytes {}..{})",
            tr.kind,
            tr.offset,
            tr.offset + tr.len
        );
    }
    for d in &built.diagnostics {
        println!("{}: {}", d.level, t_diag(locale, d));
    }
    ExitCode::SUCCESS
}

/// JSON of a built packet: `len`, `hex`, `layers` (with their fields),
/// `trailers` and `diagnostics`. Field values are formatted as in the text output.
fn built_json(built: &wpg_core::Built) -> Value {
    fn fields_json(fields: &[FieldLayout], def: Option<&ProtocolDef>) -> Vec<Value> {
        fields
            .iter()
            .map(|f| {
                let ty = def.and_then(|d| d.field(&f.name)).map(|d| &d.ty);
                let mut v = json!({
                    "name": f.name,
                    "value": describe(&f.value, ty),
                    "bit_offset": f.bit_offset,
                    "bit_len": f.bit_len,
                    "auto": f.auto,
                });
                if !f.children.is_empty() {
                    v["children"] = Value::from(fields_json(&f.children, None));
                }
                v
            })
            .collect()
    }
    let hex: String = built.bytes.iter().map(|b| format!("{b:02x}")).collect();
    let layers: Vec<Value> = built
        .layout
        .iter()
        .map(|l| {
            json!({
                "protocol": l.protocol,
                "offset": l.offset,
                "header_len": l.header_len,
                "len": l.len,
                "truncated": l.truncated,
                "fields": fields_json(&l.fields, wpg_core_lookup(&l.protocol)),
            })
        })
        .collect();
    let diagnostics: Vec<Value> = built
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "level": d.level.to_string(),
                "code": d.key,
                "layer": d.layer,
                "field": d.field,
                "message": d.message,
            })
        })
        .collect();
    json!({
        "len": built.bytes.len(),
        "hex": hex,
        "layers": layers,
        "trailers": built.trailers,
        "diagnostics": diagnostics,
    })
}

fn wpg_core_lookup(id: &str) -> Option<&'static ProtocolDef> {
    catalog().get(id)
}

fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "{:04x}  ", i * 16);
        for j in 0..16 {
            match chunk.get(j) {
                Some(b) => {
                    let _ = write!(out, "{b:02x} ");
                }
                None => out.push_str("   "),
            }
            if j == 7 {
                out.push(' ');
            }
        }
        out.push(' ');
        out.extend(chunk.iter().map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            }
        }));
        out.push('\n');
    }
    out
}

fn describe(v: &FieldValue, ty: Option<&FieldType>) -> String {
    match v {
        FieldValue::Bytes(b) | FieldValue::Nsap(b) if b.is_empty() => return "-".to_owned(),
        FieldValue::Tlv(i) if i.is_empty() => return "-".to_owned(),
        FieldValue::List(i) if i.is_empty() => return "-".to_owned(),
        _ => {}
    }
    let ty = ty.unwrap_or(&FieldType::Raw);
    let base = format_value(v, ty);
    match (v, ty) {
        (FieldValue::UInt(x), FieldType::Enum { variants, .. }) => variants
            .iter()
            .find(|e| e.value == *x)
            .map_or_else(|| format!("{x:#x}"), |e| format!("{x:#x} ({})", e.name)),
        (FieldValue::UInt(x), FieldType::Flags { flags, .. }) => {
            let names: Vec<&str> = flags
                .iter()
                .filter(|f| x >> f.bit & 1 == 1)
                .map(|f| f.name.as_str())
                .collect();
            if names.is_empty() {
                base
            } else {
                format!("{base} ({})", names.join("|"))
            }
        }
        _ => base,
    }
}

fn print_fields(fields: &[FieldLayout], def: Option<&ProtocolDef>, depth: usize) {
    let pad = "  ".repeat(depth);
    for f in fields {
        let ty = def.and_then(|d| d.field(&f.name)).map(|d| &d.ty);
        let auto = if f.auto { "  (auto)" } else { "" };
        if f.children.is_empty() {
            println!("{pad}{}: {}{auto}", f.name, describe(&f.value, ty));
        } else {
            println!("{pad}{}:{auto}", f.name);
            print_fields(&f.children, None, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_reports_current_and_refuses_unknown_versions() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.wpg.yaml");
        std::fs::write(&ok, "schema: 1\nmeta: {name: ok}\n").unwrap();
        assert_eq!(
            migrate_file(&ok, false, true).unwrap(),
            MigrateOutcome::Current
        );
        assert!(!dir.path().join("ok.wpg.yaml.bak").exists());

        let newer = dir.path().join("newer.wpg.yaml");
        std::fs::write(&newer, "schema: 99\nmeta: {name: x}\n").unwrap();
        assert!(migrate_file(&newer, true, true).is_err());
        let missing = dir.path().join("missing.wpg.json");
        std::fs::write(&missing, "{\"meta\": {\"name\": \"x\"}}").unwrap();
        assert!(migrate_file(&missing, false, true).is_err());

        // Directories are scanned; errors make the exit status non-zero.
        assert_eq!(
            migrate(&[dir.path().to_path_buf()], true, true, Locale::En),
            ExitCode::FAILURE
        );
        std::fs::remove_file(&newer).unwrap();
        std::fs::remove_file(&missing).unwrap();
        assert_eq!(
            migrate(&[dir.path().to_path_buf()], true, true, Locale::En),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn migrate_keys_are_translated() {
        for key in [
            "template-migrate-outdated",
            "template-migrate-done",
            "template-migrate-error",
            "template-migrate-summary",
            "template-migrate-check-summary",
        ] {
            for locale in Locale::ALL {
                assert_ne!(t(locale, key), key, "{key} missing in {locale}");
            }
        }
    }

    #[test]
    fn parses_var_arguments() {
        let v = parse_vars(&["a=1".into(), "b=x=y".into()], Locale::En).unwrap();
        assert_eq!(v["a"], "1");
        assert_eq!(v["b"], "x=y");
        assert!(parse_vars(&["novalue".into()], Locale::En).is_err());
        assert!(parse_vars(&["=x".into()], Locale::En).is_err());
    }

    #[test]
    fn issues_are_localised_with_english_fallback() {
        let t =
            Template::from_yaml_str("schema: 1\nmeta: {name: t}\nlayers:\n  - protocol: nope\n")
                .unwrap();
        let issues = lint(&t, catalog());
        assert!(issue_text(&issues[0], Locale::En).contains("Unknown protocol `nope`"));
        assert!(issue_text(&issues[0], Locale::Ja).contains("nope"));
        let mut unknown = issues[0].clone();
        unknown.key = "no-such-key".into();
        assert_eq!(issue_text(&unknown, Locale::En), unknown.message);
    }
}
