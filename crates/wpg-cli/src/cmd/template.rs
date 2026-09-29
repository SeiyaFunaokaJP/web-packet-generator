//! `wpg template` subcommand: lint, list, show, schema, export, import.

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
    apply_overrides, compile, export_bundle, import_bundle, lint, Compiled, LintIssue, LintLevel,
    Template, TemplateError, VarOverrides,
};

use crate::cmd::cli_locale;
use crate::{TemplateAction, TemplateArgs};

/// Default template library directory of `wpg template list`.
const DEFAULT_DIR: &str = "templates";

/// Runs `wpg template`.
pub(crate) fn run(args: TemplateArgs) -> ExitCode {
    let locale = cli_locale();
    match args.action {
        TemplateAction::Lint { files } => lint_files(&files, locale),
        TemplateAction::List { dir } => {
            list(dir.as_deref().unwrap_or(Path::new(DEFAULT_DIR)), locale)
        }
        TemplateAction::Show { file, vars } => show(&file, &vars, locale),
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

/// Loads and compiles a template with the built-in catalog.
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
        ExitCode::FAILURE
    })?;
    match compile(&template, &overrides, catalog()) {
        Ok(c) => Ok((template, c)),
        Err(e) => {
            report_error(path, &e, locale);
            Err(ExitCode::FAILURE)
        }
    }
}

fn lint_files(files: &[PathBuf], locale: Locale) -> ExitCode {
    let (mut errors, mut warnings) = (0usize, 0usize);
    for path in files {
        let template = match Template::from_path(path) {
            Ok(t) => t,
            Err(e) => {
                report_error(path, &e, locale);
                errors += 1;
                continue;
            }
        };
        let issues = lint(&template, catalog());
        if issues.is_empty() {
            let mut a = FluentArgs::new();
            a.set("value", path.display().to_string());
            println!("{}", t_args(locale, "template-lint-ok", &a));
        }
        for i in &issues {
            match i.level {
                LintLevel::Error => errors += 1,
                LintLevel::Warning => warnings += 1,
            }
            println!(
                "{}: {}: {}: {}",
                path.display(),
                level_text(i.level, locale),
                i.path,
                issue_text(i, locale)
            );
        }
    }
    let mut a = FluentArgs::new();
    a.set("fail", errors as i64);
    a.set("warn", warnings as i64);
    a.set("ok", files.len() as i64);
    println!("{}", t_args(locale, "template-lint-summary", &a));
    if errors > 0 {
        ExitCode::FAILURE
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

fn list(dir: &Path, locale: Locale) -> ExitCode {
    let mut files = Vec::new();
    collect(dir, &mut files);
    if files.is_empty() {
        let mut a = FluentArgs::new();
        a.set("value", dir.display().to_string());
        println!("{}", t_args(locale, "template-list-empty", &a));
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    for f in &files {
        match Template::from_path(f) {
            Ok(t) => {
                let stack: Vec<&str> = t.layers.iter().map(|l| l.protocol.as_str()).collect();
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
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn show(file: &Path, vars: &[String], locale: Locale) -> ExitCode {
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
