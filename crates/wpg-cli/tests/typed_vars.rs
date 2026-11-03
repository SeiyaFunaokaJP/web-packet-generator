//! `--var` values are checked against the declared variable type: a bad value is a
//! template error (exit status 3) that names the variable and the expected type.

use std::path::PathBuf;
use std::process::{Command, Output};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn render(vars: &[&str]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_wpg"));
    c.current_dir(repo())
        .env("LANG", "en")
        .env_remove("WPG_PROTOCOLS_DIR")
        .env_remove("WPG_CONFIG")
        .args(["render", "--count", "1", "-o", "-"]);
    for v in vars {
        c.arg("--var").arg(v);
    }
    c.arg("templates/examples/vlan-tcp-syn.wpg.yaml");
    c.output().unwrap()
}

#[test]
fn bad_var_values_exit_3_and_name_the_variable() {
    for (var, needle) in [
        ("vlan_id=5000", "1..4094"),
        ("vlan_id=ten", "uint"),
        ("dst_ip=198.51.100.300", "ipv4"),
    ] {
        let out = render(&[var]);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(3), "{var}: {err}");
        assert!(out.stdout.is_empty(), "{var}");
        let name = var.split('=').next().unwrap();
        assert!(
            err.contains(&format!("`{name}`")) && err.contains(needle),
            "{var}: {err}"
        );
    }
    let out = render(&["vlan_id=4094", "dst_ip=198.51.100.1"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.stdout.is_empty());
}
