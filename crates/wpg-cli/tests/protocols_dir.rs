//! `--protocols-dir`: user-defined protocols (`examples/protocols.d`) are used by
//! `wpg render` / `wpg import`, and broken extension files stop the command.

use std::path::PathBuf;
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn wpg() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_wpg"));
    c.current_dir(repo())
        .env("LANG", "en")
        .env_remove("WPG_PROTOCOLS_DIR");
    c
}

#[test]
fn render_and_import_with_user_protocols() {
    let dir = tempfile::tempdir().unwrap();
    let pcap = dir.path().join("acme.pcapng");
    let out = wpg()
        .args(["render", "examples/acme-telemetry.wpg.yaml", "-o"])
        .arg(&pcap)
        .args(["--protocols-dir", "examples/protocols.d"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Without the extension the template refers to an unknown protocol.
    let out = wpg()
        .args(["render", "examples/acme-telemetry.wpg.yaml", "-o"])
        .arg(dir.path().join("x.pcapng"))
        .output()
        .unwrap();
    assert!(!out.status.success());

    let yaml = dir.path().join("acme.wpg.yaml");
    let out = wpg()
        .arg("import")
        .arg(&pcap)
        .arg("-o")
        .arg(&yaml)
        .env("WPG_PROTOCOLS_DIR", "examples/protocols.d")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&yaml).unwrap();
    assert!(text.contains("protocol: acme_tlm"), "{text}");
    assert!(text.contains("msg_type: report"), "{text}");
}

#[test]
fn broken_extension_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("bad.yaml"),
        "bind:\n  - under: udp\n    value: 4789\n    protocol: vxlan\n",
    )
    .unwrap();
    let out = wpg()
        .args(["template", "lint", "templates/examples/arp-sweep.wpg.yaml"])
        .arg("--protocols-dir")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("bad.yaml") && err.contains("already bound"),
        "{err}"
    );
}

fn run_ok(c: &mut Command) -> String {
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn protocols_list_marks_the_source() {
    let text = run_ok(wpg().args([
        "protocols",
        "list",
        "--protocols-dir",
        "examples/protocols.d",
    ]));
    let line = text.lines().find(|l| l.starts_with("udp ")).unwrap();
    assert!(line.contains("built-in"), "{line}");
    let line = text.lines().find(|l| l.starts_with("acme_tlm ")).unwrap();
    assert!(line.contains("acme-telemetry.yaml"), "{line}");

    let json = run_ok(wpg().args([
        "--protocols-dir",
        "examples/protocols.d",
        "protocols",
        "list",
        "--json",
    ]));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let rows = v.as_array().unwrap();
    let udp = rows.iter().find(|r| r["id"] == "udp").unwrap();
    assert!(udp["source"].is_null());
    assert_eq!(udp["layer"], "l4");
    let acme = rows.iter().find(|r| r["id"] == "acme_tlm").unwrap();
    assert!(acme["source"]
        .as_str()
        .unwrap()
        .ends_with("acme-telemetry.yaml"));
}

#[test]
fn protocols_show_is_a_starting_point() {
    let yaml = run_ok(wpg().args(["protocols", "show", "udp"]));
    assert!(yaml.starts_with("protocols:"), "{yaml}");
    // Renamed, the output is a valid extension file.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("my.yaml");
    std::fs::write(&file, yaml.replace("id: udp", "id: my_udp")).unwrap();
    let out = run_ok(wpg().args(["protocols", "check"]).arg(&file));
    assert!(out.contains("my.yaml") && out.contains("OK"), "{out}");

    let json = run_ok(wpg().args(["protocols", "show", "ipv4", "--json"]));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["id"], "ipv4");

    let out = wpg()
        .args(["protocols", "show", "no_such"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no_such"));
}

#[test]
fn protocols_check_reports_file_and_path() {
    let out = run_ok(wpg().args(["protocols", "check", "examples/protocols.d"]));
    assert!(out.contains("acme-telemetry.yaml"), "{out}");
    // Checking the files of --protocols-dir does not clash with themselves.
    run_ok(wpg().args([
        "--protocols-dir",
        "examples/protocols.d",
        "protocols",
        "check",
        "examples/protocols.d/acme-telemetry.yaml",
    ]));

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("bad.yaml"),
        "bind:\n  - under: udp\n    value: 4789\n    protocol: vxlan\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("typo.yaml"),
        "protocols:\n  - id: x\n    name: X\n    layer: l7\n    fields:\n      - name: a\n        type: { kind: uint, bitz: 8 }\n",
    )
    .unwrap();
    let out = wpg()
        .args(["protocols", "check"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("typo.yaml") && err.contains("protocols[0].fields[0]"),
        "{err}"
    );

    std::fs::remove_file(dir.path().join("typo.yaml")).unwrap();
    let out = wpg()
        .args(["protocols", "check"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("bad.yaml") && err.contains("bind[0]") && err.contains("already bound"),
        "{err}"
    );

    let empty = tempfile::tempdir().unwrap();
    let out = wpg()
        .args(["protocols", "check"])
        .arg(empty.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}
