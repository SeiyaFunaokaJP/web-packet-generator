//! `wpg import` subcommand: pcap / pcapng / hex → template(s).

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use wpg_i18n::{t_args, FluentArgs, Locale};
use wpg_protocols::catalog;
use wpg_template::{
    import_capture, import_hex, ImportError, ImportMode, ImportNote, ImportOptions, ImportResult,
};

use crate::cmd::cli_locale;

/// Runs `wpg import`.
pub(crate) fn run(args: &crate::ImportArgs) -> ExitCode {
    let locale = cli_locale();
    let label = args.input.display().to_string();
    let bytes = match read_input(&args.input) {
        Ok(b) => b,
        Err(e) => return read_failure(&label, &e.to_string(), locale),
    };
    let opts = ImportOptions {
        mode: if args.per_packet.is_some() {
            ImportMode::PerPacket
        } else {
            ImportMode::Merge
        },
        max_packets: args.max_packets,
        first_protocol: args.first_protocol.clone(),
        name_prefix: stem(&args.input),
        detect_generators: !args.no_detect_generators,
    };
    let result = if args.hex || (!is_capture(&bytes) && !looks_binary(&bytes)) {
        let text = String::from_utf8_lossy(&bytes);
        import_hex(&text, &opts.first_protocol, catalog())
    } else {
        import_capture(&bytes, &label, &opts, catalog())
    };
    let result = match result {
        Ok(r) => r,
        Err(e) => return import_failure(&label, &e, locale),
    };
    for n in &result.notes {
        eprintln!("{label}: {}", note_text(n, locale));
    }
    write_result(args, &result, locale)
}

fn write_result(args: &crate::ImportArgs, result: &ImportResult, locale: Locale) -> ExitCode {
    let mut where_ = String::new();
    if let Some(dir) = &args.per_packet {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return write_failure(dir, &e.to_string(), locale);
        }
        for t in &result.templates {
            let path = dir.join(format!("{}.wpg.yaml", t.meta.name));
            if let Err(e) = t.save(&path) {
                return write_failure(&path, &e.to_string(), locale);
            }
        }
        where_ = dir.display().to_string();
    } else if let Some(out) = &args.output {
        let t = &result.templates[0];
        if out.as_os_str() == "-" {
            match t.to_yaml_string() {
                Ok(y) => print!("{y}"),
                Err(e) => return write_failure(out, &e.to_string(), locale),
            }
            return ExitCode::SUCCESS;
        }
        if let Err(e) = t.save(out) {
            return write_failure(out, &e.to_string(), locale);
        }
        where_ = out.display().to_string();
    }
    let mut a = FluentArgs::new();
    a.set("len", i64::try_from(result.packets).unwrap_or(i64::MAX));
    a.set(
        "count",
        i64::try_from(result.templates.len()).unwrap_or(i64::MAX),
    );
    a.set("value", where_);
    println!("{}", t_args(locale, "import-done", &a));
    ExitCode::SUCCESS
}

fn read_input(path: &Path) -> std::io::Result<Vec<u8>> {
    if path.as_os_str() == "-" {
        let mut buf = Vec::new();
        std::io::stdin().lock().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        std::fs::read(path)
    }
}

fn stem(path: &Path) -> String {
    if path.as_os_str() == "-" {
        return "imported".to_owned();
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("imported");
    let stem = name.split('.').next().unwrap_or(name);
    if stem.is_empty() {
        "imported".to_owned()
    } else {
        stem.to_owned()
    }
}

/// pcap (both orders / resolutions) or pcapng magic.
fn is_capture(b: &[u8]) -> bool {
    matches!(
        b.get(..4),
        Some(
            [0xd4, 0xc3, 0xb2, 0xa1]
                | [0x4d, 0x3c, 0xb2, 0xa1]
                | [0xa1, 0xb2, 0xc3, 0xd4]
                | [0xa1, 0xb2, 0x3c, 0x4d]
                | [0x0a, 0x0d, 0x0d, 0x0a]
        )
    )
}

/// Binary garbage is reported as an unknown capture format instead of a hex error.
fn looks_binary(b: &[u8]) -> bool {
    b.iter().take(512).any(|c| *c == 0)
}

/// Localised text of a note (Fluent by key, English fallback).
fn note_text(n: &ImportNote, locale: Locale) -> String {
    let mut args = FluentArgs::new();
    for (k, v) in &n.args {
        args.set(k.as_str(), v.as_str());
    }
    let body = t_args(locale, &n.key, &args);
    let body = if body == n.key {
        n.message.clone()
    } else {
        body
    };
    match n.packet {
        Some(i) if !n.args.contains_key("index") => {
            let mut a = FluentArgs::new();
            a.set("index", i64::try_from(i + 1).unwrap_or(i64::MAX));
            a.set("value", body);
            t_args(locale, "import-note-packet", &a)
        }
        _ => body,
    }
}

fn read_failure(label: &str, error: &str, locale: Locale) -> ExitCode {
    let mut a = FluentArgs::new();
    a.set("value", label);
    a.set("error", error);
    eprintln!("{}", t_args(locale, "import-read-error", &a));
    ExitCode::FAILURE
}

fn import_failure(label: &str, e: &ImportError, locale: Locale) -> ExitCode {
    read_failure(label, &e.to_string(), locale)
}

fn write_failure(path: &Path, error: &str, locale: Locale) -> ExitCode {
    let mut a = FluentArgs::new();
    a.set("value", path.display().to_string());
    a.set("error", error);
    eprintln!("{}", t_args(locale, "import-write-error", &a));
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_and_sniffing() {
        assert_eq!(stem(Path::new("/x/v.pcapng")), "v");
        assert_eq!(stem(Path::new("cap.tar.pcap")), "cap");
        assert_eq!(stem(Path::new("-")), "imported");
        assert!(is_capture(&[0x0a, 0x0d, 0x0d, 0x0a, 0]));
        assert!(is_capture(&[0xd4, 0xc3, 0xb2, 0xa1]));
        assert!(!is_capture(b"ff ff"));
        assert!(looks_binary(&[1, 0, 2]));
    }

    #[test]
    fn notes_are_localised_with_packet_prefix() {
        let n = ImportNote {
            packet: Some(2),
            key: "import-not-reproducible".into(),
            args: Default::default(),
            message: "fallback".into(),
        };
        assert!(note_text(&n, Locale::En).starts_with("packet 3: "));
        assert!(note_text(&n, Locale::Ja).contains("パケット 3"));
    }
}
