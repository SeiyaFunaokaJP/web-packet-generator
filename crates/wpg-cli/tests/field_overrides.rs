//! `--set`, `--gen` and `--vars-file` on `wpg template show` / `wpg render`.

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

const TEMPLATE: &str = "templates/examples/eth-ipv4-udp-range.wpg.yaml";

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

/// `wpg template show TEMPLATE --json <args>`.
fn show(args: &[&str]) -> Output {
    wpg()
        .args(["template", "show", TEMPLATE, "--json"])
        .args(args)
        .output()
        .unwrap()
}

/// The field `name` of the layer `protocol` in the JSON of `template show`.
fn field<'a>(v: &'a Value, protocol: &str, name: &str) -> &'a Value {
    v["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["protocol"] == protocol)
        .and_then(|l| {
            l["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["name"] == name)
        })
        .unwrap_or_else(|| panic!("no {protocol}.{name} in {v}"))
}

fn ok_json(out: &Output) -> Value {
    assert!(out.status.success(), "{}", stderr(out));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn set_and_gen_override_fields() {
    let v = ok_json(&show(&[]));
    assert_eq!(field(&v, "ipv4", "ttl")["value"], "64");
    assert_eq!(field(&v, "ipv4", "src")["value"], "192.168.1.1");
    assert_eq!(field(&v, "ethernet2", "ethertype")["auto"], true);

    let v = ok_json(&show(&[
        "--set",
        "ipv4.ttl=7",
        // replaces the template's generator
        "--set",
        "ipv4[0].src=10.9.9.9",
        // an Auto field becomes Manual
        "--set",
        "0.ethertype=0x1234",
        "--gen",
        "udp.src_port=range:2000..2009",
    ]));
    assert_eq!(field(&v, "ipv4", "ttl")["value"], "7");
    assert_eq!(field(&v, "ipv4", "src")["value"], "10.9.9.9");
    assert_eq!(field(&v, "ethernet2", "ethertype")["auto"], false);
    assert_eq!(field(&v, "udp", "src_port")["value"], "2000");
}

#[test]
fn vars_file_then_var() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("vars.yaml");
    std::fs::write(&file, "dst_ip: 10.2.2.2\ndst_port: 7000\n").unwrap();
    let file = file.to_str().unwrap();

    let v = ok_json(&show(&["--vars-file", file]));
    assert_eq!(field(&v, "ipv4", "dst")["value"], "10.2.2.2");
    assert_eq!(field(&v, "udp", "dst_port")["value"], "7000");

    // --var wins over the file
    let v = ok_json(&show(&["--vars-file", file, "--var", "dst_port=8000"]));
    assert_eq!(field(&v, "ipv4", "dst")["value"], "10.2.2.2");
    assert_eq!(field(&v, "udp", "dst_port")["value"], "8000");

    // JSON works too; an undeclared variable is a template error
    let json = dir.path().join("vars.json");
    std::fs::write(&json, r#"{"nope": 1}"#).unwrap();
    let out = show(&["--vars-file", json.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(stderr(&out).contains("nope"), "{}", stderr(&out));

    for bad in ["[1, 2]", "a: ["] {
        std::fs::write(&json, bad).unwrap();
        let out = show(&["--vars-file", json.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(2), "{bad}: {}", stderr(&out));
    }
    let out = show(&["--vars-file", "missing-vars.yaml"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
}

#[test]
fn bad_targets_name_close_matches() {
    let out = show(&["--set", "ipv5.ttl=1"]);
    assert_eq!(out.status.code(), Some(3));
    let err = stderr(&out);
    assert!(
        err.contains("ipv5") && err.contains("ethernet2, ipv4, udp, payload"),
        "{err}"
    );

    let out = show(&["--gen", "ipv4.tll=random"]);
    assert_eq!(out.status.code(), Some(3));
    let err = stderr(&out);
    assert!(err.contains("`tll`") && err.contains("ttl"), "{err}");

    // a value that does not fit the field is reported by the compiler
    let out = show(&["--set", "ipv4.ttl=300"]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    let out = show(&["--gen", "ipv4.ttl=range:1..999"]);
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));

    // malformed arguments are usage errors
    for args in [
        &["--set", "ttl=1"][..],
        &["--set", "ipv4.ttl"],
        &["--gen", "ipv4.ttl=range:1"],
    ] {
        let out = show(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
}

#[test]
fn render_and_send_accept_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let out = wpg()
        .args(["render", TEMPLATE, "--json", "--count", "3", "-o"])
        .arg(dir.path().join("o.pcapng"))
        .args(["--gen", "udp.src_port=list:1,2,3", "--set", "ipv4.ttl=1"])
        .output()
        .unwrap();
    let v = ok_json(&out);
    assert_eq!(v["packets"], 3);

    let out = wpg()
        .args(["send", TEMPLATE, "--backend", "null", "--count", "1"])
        .args(["--set", "ipv9.ttl=1"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
}
