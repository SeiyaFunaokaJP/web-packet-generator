//! `wpg render` (CLI) and `POST /api/render` (server) produce the same frames.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Packet data of every Enhanced Packet Block of a little-endian pcapng file.
fn pcapng_packets(path: &Path) -> Vec<Vec<u8>> {
    let b = std::fs::read(path).unwrap();
    let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    assert_eq!(u32_at(8), 0x1A2B_3C4D, "expected a little-endian section");
    let mut out = Vec::new();
    let mut i = 0;
    while i + 12 <= b.len() {
        let (ty, len) = (u32_at(i), u32_at(i + 4));
        if ty == 6 {
            let cap = u32_at(i + 20);
            out.push(b[i + 28..i + 28 + cap].to_vec());
        }
        i += len;
    }
    out
}

#[tokio::test]
async fn cli_render_matches_api_render() {
    let dir = tempfile::tempdir().unwrap();
    for (file, vars) in [
        ("templates/examples/arp-sweep.wpg.yaml", json!({})),
        (
            "templates/examples/eth-ipv4-udp-range.wpg.yaml",
            json!({ "dst_ip": "10.1.2.3" }),
        ),
        ("templates/examples/vlan-tcp-syn.wpg.yaml", json!({})),
    ] {
        let template = repo().join(file);
        let out = dir.path().join("out.pcapng");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_wpg"));
        cmd.args(["render", "--count", "20", "-o"])
            .arg(&out)
            .arg(&template)
            .env("LANG", "C");
        for (k, v) in vars.as_object().unwrap() {
            cmd.arg("--var").arg(format!("{k}={}", v.as_str().unwrap()));
        }
        let status = cmd.status().unwrap();
        assert!(status.success(), "wpg render {file}");
        let cli = pcapng_packets(&out);
        assert_eq!(cli.len(), 20, "{file}");

        let mut config = wpg_server::Config {
            data_dir: dir.path().join("data"),
            ..wpg_server::Config::default()
        };
        config.templates.library_paths = vec![];
        let app = wpg_server::app(wpg_server::AppState::new(config).unwrap());
        let body = json!({
            "template": std::fs::read_to_string(&template).unwrap(),
            "vars": vars,
            "count": 20,
        });
        let resp = app
            .oneshot(
                Request::post("/api/render")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let j: Value =
            serde_json::from_slice(&to_bytes(resp.into_body(), 1 << 24).await.unwrap()).unwrap();
        let api: Vec<Vec<u8>> = j["packets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let h = p["bytes_hex"].as_str().unwrap();
                (0..h.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(api, cli, "{file}");
    }
}
