//! `JobConfig::signature`: every frame carries a signature with consecutive
//! sequence numbers, and the stamped frames still have valid checksums.

use std::path::PathBuf;

use wpg_core::diag::keys;
use wpg_core::signature;
use wpg_core::stream::Expansion;
use wpg_protocols::catalog;
use wpg_template::pcapread::read_capture;
use wpg_template::{compile, Template, VarOverrides};
use wpg_tx::job::{JobConfig, SignatureConfig, TxJob};
use wpg_tx::source::{FrameSource, OnTheFly};
use wpg_tx::PcapFileBackend;

fn udp_source() -> Box<dyn FrameSource> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/examples/eth-ipv4-udp-range.wpg.yaml");
    let t = Template::from_path(&p).unwrap();
    let c = compile(&t, &VarOverrides::new(), catalog()).unwrap();
    let exp = Expansion::new(&c.stream, None).unwrap();
    Box::new(OnTheFly::new(c.packet, exp, catalog()).unwrap())
}

#[test]
fn stamps_every_frame_with_valid_checksums() {
    let dir = std::env::temp_dir().join(format!("wpg-tx-sig-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sig.pcapng");
    let backend = Box::new(PcapFileBackend::create(&path).unwrap());
    let report = TxJob::start(
        backend,
        udp_source(),
        JobConfig {
            count: Some(200),
            signature: Some(SignatureConfig { stream_id_base: 5 }),
            ..JobConfig::default()
        },
    )
    .unwrap()
    .wait();
    assert_eq!(report.stats.packets, 200);
    assert_eq!(report.stats.unsigned, 0);

    let frames = read_capture(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(frames.len(), 200);
    for (i, f) in frames.iter().enumerate() {
        let sig = signature::find(&f.data).expect("signature");
        assert_eq!((sig.stream_id, sig.seq), (5, i as u64));
        assert!(sig.ts_ns > 1_600_000_000_000_000_000);
        let parsed = wpg_core::parse(&f.data, "ethernet2", catalog()).unwrap();
        assert!(
            !parsed
                .diagnostics
                .iter()
                .any(|d| d.key == keys::CHECKSUM_MISMATCH),
            "frame {i}: {:?}",
            parsed.diagnostics
        );
    }
    // Sanity check: the parser does notice a payload change without a fix-up.
    let mut broken = frames[0].data.clone();
    *broken.last_mut().unwrap() ^= 0xff;
    let parsed = wpg_core::parse(&broken, "ethernet2", catalog()).unwrap();
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.key == keys::CHECKSUM_MISMATCH));
    std::fs::remove_dir_all(&dir).ok();
}
