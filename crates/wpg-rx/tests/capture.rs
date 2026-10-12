//! End-to-end: frames stamped by a TX job are received and verified, through a
//! capture file (`pcapfile`, every OS) and over `lo` (`afpacket`, Linux, needs
//! `CAP_NET_RAW`; skipped otherwise).

use std::path::{Path, PathBuf};
use std::time::Duration;

use wpg_core::signature::{self, Signature};
use wpg_core::stream::Expansion;
use wpg_protocols::catalog;
use wpg_rx::backend::{open_by_name, RxOptions};
use wpg_rx::capture::EndReason;
use wpg_rx::{Capture, CaptureConfig, CaptureState, Filter, RxError};
use wpg_template::pcapread::read_capture;
use wpg_template::{compile, Template, VarOverrides};
use wpg_tx::job::{JobConfig, SignatureConfig, TxJob};
use wpg_tx::rate::Rate;
use wpg_tx::source::{FrameSource, OnTheFly};
use wpg_tx::{Frame, PcapFileBackend, TxBackend, TxOptions};

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wpg-rx-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn udp_source() -> Box<dyn FrameSource> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/examples/eth-ipv4-udp-range.wpg.yaml");
    let t = Template::from_path(&p).unwrap();
    let c = compile(&t, &VarOverrides::new(), catalog()).unwrap();
    let exp = Expansion::new(&c.stream, None).unwrap();
    Box::new(OnTheFly::new(c.packet, exp, catalog()).unwrap())
}

fn send_to_file(path: &Path, count: u64) {
    let backend = Box::new(PcapFileBackend::create(path).unwrap());
    let r = TxJob::start(
        backend,
        udp_source(),
        JobConfig {
            count: Some(count),
            signature: Some(SignatureConfig { stream_id_base: 3 }),
            ..JobConfig::default()
        },
    )
    .unwrap()
    .wait();
    assert_eq!(r.stats.packets, count);
}

fn open_file(path: &Path) -> Box<dyn wpg_rx::RxBackend> {
    open_by_name("pcapfile", path.to_str().unwrap(), &RxOptions::default()).unwrap()
}

#[test]
fn pcapfile_roundtrip_with_verification() {
    let dir = tmpdir("file");
    let input = dir.join("tx.pcapng");
    send_to_file(&input, 300);
    let saved = dir.join("rx.pcapng");
    let report = Capture::start(
        open_file(&input),
        CaptureConfig {
            filter: Filter::parse("udp dst port 5001 and wpg").unwrap(),
            keep: 5,
            save: Some(saved.clone()),
            verify: true,
            ..CaptureConfig::default()
        },
    )
    .unwrap()
    .wait();
    assert_eq!(report.state, CaptureState::Finished(EndReason::EndOfInput));
    let s = &report.stats;
    assert_eq!((s.packets, s.matched, s.saved, s.kept), (300, 300, 300, 5));
    let v = s.verify.as_ref().unwrap();
    assert_eq!((v.signed, v.unsigned), (300, 0));
    assert_eq!(v.streams.len(), 1);
    let st = &v.streams[0];
    assert_eq!((st.stream_id, st.first_seq, st.last_seq), (3, 0, 299));
    assert_eq!((st.lost, st.out_of_order, st.duplicates), (0, 0, 0));
    // pcapfile TX timestamps are taken right after the signature: latency is tiny.
    assert!(st.latency_max_ns < 1_000_000_000, "{st:?}");
    assert_eq!(
        read_capture(&std::fs::read(&saved).unwrap()).unwrap().len(),
        300
    );

    // A filter that matches nothing; no verification.
    let r = Capture::start(
        open_file(&input),
        CaptureConfig {
            filter: Filter::parse("tcp").unwrap(),
            ..CaptureConfig::default()
        },
    )
    .unwrap()
    .wait();
    assert_eq!((r.stats.packets, r.stats.matched), (300, 0));
    assert!(r.stats.verify.is_none());

    // Count limit.
    let r = Capture::start(
        open_file(&input),
        CaptureConfig {
            count: Some(10),
            ..CaptureConfig::default()
        },
    )
    .unwrap()
    .wait();
    assert_eq!(r.state, CaptureState::Finished(EndReason::CountReached));
    assert_eq!(r.stats.matched, 10);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn detects_loss_reordering_and_duplicates() {
    let dir = tmpdir("loss");
    let path = dir.join("lossy.pcapng");
    let mut b = PcapFileBackend::create(&path).unwrap();
    // seq 0..20 of stream 1, without 5 and 6, 10 / 11 swapped, 15 twice.
    let mut order: Vec<u64> = (0..20).filter(|s| *s != 5 && *s != 6).collect();
    order.swap(8, 9);
    order.insert(14, 15);
    let frames: Vec<Frame> = order
        .iter()
        .map(|&seq| {
            let mut f = vec![0xffu8; 6];
            f.extend_from_slice(&[2, 0, 0, 0, 0, 1, 0x88, 0xb5]);
            f.resize(64, 0);
            let sig = Signature {
                stream_id: 1,
                seq,
                ts_ns: 1,
            };
            signature::stamp(&mut f, &sig).unwrap();
            Frame::new(f)
        })
        .collect();
    b.send_batch(&frames).unwrap();
    b.close().unwrap();
    let r = Capture::start(
        open_file(&path),
        CaptureConfig {
            verify: true,
            ..CaptureConfig::default()
        },
    )
    .unwrap()
    .wait();
    let st = &r.stats.verify.as_ref().unwrap().streams[0];
    assert_eq!(
        (st.received, st.lost, st.out_of_order, st.duplicates),
        (19, 2, 1, 1),
        "{st:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn loopback_afpacket_end_to_end() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let backend = match open_by_name("afpacket", "lo", &RxOptions::default()) {
        Ok(b) => b,
        Err(RxError::Permission(m)) => {
            eprintln!("skipping: {m}");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let h = Capture::start(
        backend,
        CaptureConfig {
            filter: Filter::parse("udp port 5001 and wpg").unwrap(),
            count: Some(500),
            duration: Some(Duration::from_secs(20)),
            verify: true,
            keep: 3,
            ..CaptureConfig::default()
        },
    )
    .unwrap();
    let job = TxJob::start_on(
        "afpacket",
        "lo",
        &TxOptions::default(),
        vec![udp_source()],
        JobConfig {
            count: Some(500),
            rate: Rate::parse("20000pps").unwrap(),
            signature: Some(SignatureConfig { stream_id_base: 0 }),
            ..JobConfig::default()
        },
    )
    .unwrap();
    assert_eq!(job.wait().stats.packets, 500);
    let r = h.wait();
    assert_eq!(
        r.state,
        CaptureState::Finished(EndReason::CountReached),
        "{r:?}"
    );
    let st = &r.stats.verify.as_ref().unwrap().streams[0];
    assert_eq!(
        (st.received, st.lost, st.out_of_order, st.duplicates),
        (500, 0, 0, 0)
    );
    assert!(
        st.latency_min_ns >= 0 && st.latency_avg_ns < 1_000_000_000,
        "{st:?}"
    );
    assert_eq!(r.stats.kept, 3);
}
