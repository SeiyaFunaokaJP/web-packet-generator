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
