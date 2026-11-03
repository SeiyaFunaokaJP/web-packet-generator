//! The global `--config` (`[protocols] dirs`, `[tx]` defaults) and the
//! scripting output of `wpg render` / `wpg template` (`--json`, `-o -`, exit
//! status 3 for invalid templates).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn wpg() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_wpg"));
    c.current_dir(repo())
        .env("LANG", "en")
        .env_remove("WPG_PROTOCOLS_DIR")
        .env_remove("WPG_CONFIG");
    c
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            stderr(out)
        )
    })
}

/// Writes a `wpg.toml` into `dir` with `body` and returns its path.
fn config(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("wpg.toml");
    std::fs::write(&path, body).unwrap();
    path
}

fn protocols_config(dir: &Path) -> PathBuf {
    let protos = repo().join("examples/protocols.d");
    config(
        dir,
        &format!("[protocols]\ndirs = ['{}']\n", protos.display()),
    )
}

#[test]
fn config_protocol_dirs_apply_to_render() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = protocols_config(dir.path());
    let template = repo().join("examples/acme-telemetry.wpg.yaml");

    // Explicit --config.
    let out = wpg()
        .arg("render")
        .arg(&template)
        .arg("-o")
        .arg(dir.path().join("a.pcapng"))
        .arg("--config")
        .arg(&cfg)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    // WPG_CONFIG, given before the subcommand is the same option.
    let out = wpg()
        .arg("render")
        .arg(&template)
        .arg("-o")
        .arg(dir.path().join("b.pcapng"))
        .env("WPG_CONFIG", &cfg)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    // `wpg.toml` in the working directory is read without --config.
    let out = wpg()
        .current_dir(dir.path())
        .arg("template")
        .arg("lint")
        .arg(&template)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));

    // Without a configuration the protocol is unknown: a template error.
    let out = wpg()
        .arg("render")
        .arg(&template)
        .arg("-o")
        .arg(dir.path().join("c.pcapng"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
}

#[test]
fn explicit_config_must_exist_and_be_valid() {
    let dir = tempfile::tempdir().unwrap();
    let out = wpg()
        .args(["--config"])
        .arg(dir.path().join("missing.toml"))
        .args(["template", "lint", "templates/examples/arp-sweep.wpg.yaml"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("missing.toml"), "{}", stderr(&out));

    let bad = config(dir.path(), "[tx]\nno_such_key = 1\n");
    let out = wpg()
        .args(["template", "lint", "templates/examples/arp-sweep.wpg.yaml"])
        .arg("--config")
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("no_such_key"), "{}", stderr(&out));
}

#[test]
fn send_uses_the_configured_default_backend() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), "[tx]\ndefault_backend = \"null\"\n");
    let out = wpg()
        .args([
            "send",
            "templates/examples/arp-sweep.wpg.yaml",
            "--count",
            "3",
            "--json",
        ])
        .arg("--config")
        .arg(&cfg)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let report = json(&out);
    assert_eq!(report["backend_name"], "null", "{report}");
    assert_eq!(report["stats"]["packets"], 3, "{report}");
}

#[test]
fn render_json_and_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.pcapng");
    let out = wpg()
        .args([
            "render",
            "templates/examples/arp-sweep.wpg.yaml",
            "--count",
            "4",
            "--json",
            "-o",
        ])
        .arg(&file)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["packets"], 4);
    assert_eq!(v["path"], file.display().to_string());
    assert_eq!(
        v["bytes"].as_u64().unwrap(),
        std::fs::metadata(&file).unwrap().len()
    );

    // `-o -`: only the pcapng on stdout, the summary on stderr.
    let out = wpg()
        .args([
            "render",
            "templates/examples/arp-sweep.wpg.yaml",
            "--count",
            "4",
            "-o",
            "-",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(out.stdout, std::fs::read(&file).unwrap());
    assert_eq!(&out.stdout[..4], &[0x0a, 0x0d, 0x0d, 0x0a]);
    assert!(stderr(&out).contains("Wrote 4 packets"), "{}", stderr(&out));

    let out = wpg()
        .args([
            "render",
            "templates/examples/arp-sweep.wpg.yaml",
            "-o",
            "-",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

#[test]
fn template_json_output() {
    let out = wpg()
        .args([
            "template",
            "show",
            "templates/examples/arp-sweep.wpg.yaml",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let v = json(&out);
    let hex = v["hex"].as_str().unwrap();
    assert_eq!(hex.len() as u64, 2 * v["len"].as_u64().unwrap());
    assert_eq!(v["layers"][0]["protocol"], "ethernet2", "{v}");
    assert!(v["layers"][0]["fields"]
        .as_array()
        .is_some_and(|f| !f.is_empty()));

    let out = wpg()
        .args(["template", "list", "templates/examples", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let v = json(&out);
    let items = v.as_array().unwrap();
    assert!(!items.is_empty());
    assert!(items
        .iter()
        .all(|i| i["name"].is_string() && i["path"].is_string() && i["layers"].is_array()));

    let out = wpg()
        .args([
            "template",
            "lint",
            "templates/examples/arp-sweep.wpg.yaml",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(json(&out)["errors"], 0);
}

#[test]
fn invalid_templates_exit_with_status_3() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.wpg.yaml");
    std::fs::write(
        &bad,
        "schema: 1\nmeta: {name: bad}\nlayers:\n  - protocol: nope\n",
    )
    .unwrap();

    let out = wpg()
        .args(["template", "lint", "--json"])
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let v = json(&out);
    assert_eq!(v["errors"], 1, "{v}");
    let issue = &v["files"][0]["issues"][0];
    assert_eq!(issue["level"], "error");
    assert!(issue["code"].as_str().unwrap().starts_with("lint-"), "{v}");
    assert!(issue["path"].as_str().unwrap().starts_with("layers"), "{v}");

    for args in [
        &["template", "show"][..],
        &["render", "-o", "-"],
        &["send", "--backend", "null", "--count", "1"],
    ] {
        let out = wpg().args(args).arg(&bad).output().unwrap();
        assert_eq!(out.status.code(), Some(3), "{args:?}: {}", stderr(&out));
        assert!(out.stdout.is_empty(), "{args:?}");
    }

    // An unreadable file is a general error.
    let out = wpg()
        .args(["template", "show"])
        .arg(dir.path().join("missing.wpg.yaml"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}
