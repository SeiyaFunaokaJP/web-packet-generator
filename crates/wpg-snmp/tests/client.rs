//! The client and the receiver against the in-process agent on 127.0.0.1:
//! v1 / v2c / v3 (every security level) Get, GetNext, GetBulk, Walk, Set,
//! traps and informs, and the pcap recording of it all (decoded back with the
//! wpg protocol parser).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wpg_snmp::agent::{Agent, AgentConfig};
use wpg_snmp::client::Error;
use wpg_snmp::pcap::{PcapFormat, PcapRecorder, Tap};
use wpg_snmp::pdu::TrapV1;
use wpg_snmp::receiver::{Notification, Receiver, ReceiverConfig};
use wpg_snmp::usm::{AuthProtocol, PrivProtocol, UsmUser};
use wpg_snmp::{Client, ClientConfig, Oid, PduType, Value, VarBind, Version};

fn oid(s: &str) -> Oid {
    s.parse().unwrap()
}

fn data() -> BTreeMap<Oid, Value> {
    let mut d = BTreeMap::new();
    d.insert(
        oid("1.3.6.1.2.1.1.1.0"),
        Value::OctetString(b"wpg test agent".to_vec()),
    );
    d.insert(oid("1.3.6.1.2.1.1.3.0"), Value::TimeTicks(4242));
    d.insert(
        oid("1.3.6.1.2.1.1.5.0"),
        Value::OctetString(b"agent1".to_vec()),
    );
    for i in 1..=3u32 {
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.1.{i}")),
            Value::Integer(i64::from(i)),
        );
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.2.{i}")),
            Value::OctetString(format!("eth{i}").into_bytes()),
        );
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.10.{i}")),
            Value::Counter32(1000 * i),
        );
    }
    d.insert(oid("1.3.6.1.2.1.4.1.0"), Value::Integer(1));
    d
}

fn users() -> Vec<UsmUser> {
    vec![
        UsmUser {
            name: "noauth".into(),
            ..UsmUser::default()
        },
        UsmUser {
            name: "md5des".into(),
            auth: Some((AuthProtocol::Md5, "authpass1".into())),
            privacy: Some((PrivProtocol::Des, "privpass1".into())),
        },
        UsmUser {
            name: "shaaes".into(),
            auth: Some((AuthProtocol::Sha1, "authpass2".into())),
            privacy: Some((PrivProtocol::Aes128, "privpass2".into())),
        },
        UsmUser {
            name: "sha256".into(),
            auth: Some((AuthProtocol::Sha256, "authpass3".into())),
            privacy: None,
        },
        UsmUser {
            name: "sha512aes256".into(),
            auth: Some((AuthProtocol::Sha512, "authpass4".into())),
            privacy: Some((PrivProtocol::Aes256, "privpass4".into())),
        },
    ]
}

async fn agent() -> Agent {
    Agent::start(
        AgentConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            community: "public".into(),
            write_community: "private".into(),
            users: users(),
            engine_id: b"\x80\x00\x00\x00\x04wpg-test-agent".to_vec(),
        },
        data(),
    )
    .await
    .unwrap()
}

fn cfg(a: &Agent, version: Version, community: &str) -> ClientConfig {
    let mut c = ClientConfig::v2c(a.local_addr(), community);
    c.version = version;
    c.timeout = Duration::from_millis(500);
    c
}

#[tokio::test]
async fn v2c_operations() {
    let a = agent().await;
    let mut c = Client::connect(cfg(&a, Version::V2c, "public"), None)
        .await
        .unwrap();
    let r = c
        .get(&[oid("1.3.6.1.2.1.1.1.0"), oid("1.3.6.1.2.1.1.9.0")])
        .await
        .unwrap();
    assert_eq!(r.ty, PduType::Response);
    assert_eq!(
        r.varbinds[0].value,
        Value::OctetString(b"wpg test agent".to_vec())
    );
    assert_eq!(r.varbinds[1].value, Value::NoSuchObject);
    let r = c.get_next(&[oid("1.3.6.1.2.1.1.1.0")]).await.unwrap();
    assert_eq!(r.varbinds[0].oid, oid("1.3.6.1.2.1.1.3.0"));
    let r = c
        .get_bulk(1, 2, &[oid("1.3.6.1.2.1.1"), oid("1.3.6.1.2.1.2.2.1.2")])
        .await
        .unwrap();
    assert_eq!(r.varbinds.len(), 3);
    assert_eq!(r.varbinds[2].oid, oid("1.3.6.1.2.1.2.2.1.2.2"));
    // Walk with small bulks crosses several requests and stops at the subtree end.
    let w = c.walk(&oid("1.3.6.1.2.1.2.2"), 2, 1000).await.unwrap();
    assert_eq!(w.varbinds.len(), 9);
    assert!(w.requests >= 5, "{}", w.requests);
    assert!(!w.truncated);
    let w = c.walk(&oid("1.3.6.1.2.1.2.2"), 5, 4).await.unwrap();
    assert!(w.truncated);
    assert_eq!(w.varbinds.len(), 4);
    // A walk of a scalar instance falls back to Get.
    let w = c.walk(&oid("1.3.6.1.2.1.1.5.0"), 10, 100).await.unwrap();
    assert_eq!(w.varbinds.len(), 1);
    // End of the MIB.
    let w = c.walk(&oid("1.3.6.1.2.1.4"), 10, 100).await.unwrap();
    assert_eq!(w.varbinds.len(), 1);
    // Set: read-only community refused, read-write accepted, wrong type refused.
    let r = c
        .set(vec![VarBind::new(
            oid("1.3.6.1.2.1.1.5.0"),
            Value::OctetString(b"x".to_vec()),
        )])
        .await
        .unwrap();
    assert_eq!(r.error_status, 6, "noAccess");
    let mut w = Client::connect(cfg(&a, Version::V2c, "private"), None)
        .await
        .unwrap();
    let r = w
        .set(vec![VarBind::new(
            oid("1.3.6.1.2.1.1.5.0"),
            Value::OctetString(b"renamed".to_vec()),
        )])
        .await
        .unwrap();
    assert_eq!(r.error_status, 0);
    assert_eq!(
        a.store().lock().unwrap()[&oid("1.3.6.1.2.1.1.5.0")],
        Value::OctetString(b"renamed".to_vec())
    );
    let r = w
        .set(vec![VarBind::new(
            oid("1.3.6.1.2.1.1.5.0"),
            Value::Integer(1),
        )])
        .await
        .unwrap();
    assert_eq!((r.error_status, r.error_index), (7, 1), "wrongType");
    // Wrong community: silence, then a timeout after the retry.
    let mut bad = Client::connect(cfg(&a, Version::V2c, "nope"), None)
        .await
        .unwrap();
    let e = bad.get(&[oid("1.3.6.1.2.1.1.1.0")]).await.unwrap_err();
    assert!(matches!(e, Error::Timeout { attempts: 2, .. }), "{e}");
}

#[tokio::test]
async fn v1_operations() {
    let a = agent().await;
    let mut c = Client::connect(cfg(&a, Version::V1, "public"), None)
        .await
        .unwrap();
    let r = c.get(&[oid("1.3.6.1.2.1.1.9.0")]).await.unwrap();
    assert_eq!((r.error_status, r.error_index), (2, 1), "noSuchName");
    let w = c.walk(&oid("1.3.6.1.2.1.2.2.1.2"), 10, 100).await.unwrap();
    assert_eq!(w.varbinds.len(), 3);
    let w = c.walk(&oid("1.3.6.1.2.1.4"), 10, 100).await.unwrap();
    assert_eq!(w.varbinds.len(), 1, "noSuchName ends a v1 walk");
    assert!(matches!(
        c.get_bulk(0, 5, &[oid("1.3")]).await,
        Err(Error::Invalid(_))
    ));
}

#[tokio::test]
async fn v3_every_level() {
    let a = agent().await;
    for u in users() {
        let mut c = cfg(&a, Version::V3, "");
        c.user = Some(u.clone());
        let mut c = Client::connect(c, None).await.unwrap();
        let r = c.get(&[oid("1.3.6.1.2.1.1.1.0")]).await.unwrap();
        assert_eq!(
            r.varbinds[0].value,
            Value::OctetString(b"wpg test agent".to_vec()),
            "{}",
            u.name
        );
        assert_eq!(
            c.engine_id().unwrap(),
            b"\x80\x00\x00\x00\x04wpg-test-agent"
        );
        let w = c.walk(&oid("1.3.6.1.2.1.2.2.1.10"), 10, 100).await.unwrap();
        assert_eq!(w.varbinds.len(), 3, "{}", u.name);
        let r = c
            .set(vec![VarBind::new(
                oid("1.3.6.1.2.1.1.5.0"),
                Value::OctetString(u.name.clone().into_bytes()),
            )])
            .await
            .unwrap();
        assert_eq!(r.error_status, 0, "{}", u.name);
    }
    // Wrong pass phrase → usmStatsWrongDigests report.
    let mut c = cfg(&a, Version::V3, "");
    c.user = Some(UsmUser {
        name: "md5des".into(),
        auth: Some((AuthProtocol::Md5, "wrongpass".into())),
        privacy: Some((PrivProtocol::Des, "privpass1".into())),
    });
    let mut c = Client::connect(c, None).await.unwrap();
    match c.get(&[oid("1.3.6.1.2.1.1.1.0")]).await {
        Err(Error::Report { name, .. }) => assert_eq!(name, "usmStatsWrongDigests"),
        other => panic!("{other:?}"),
    }
    // Unknown user.
    let mut c = cfg(&a, Version::V3, "");
    c.user = Some(UsmUser {
        name: "ghost".into(),
        ..UsmUser::default()
    });
    let mut c = Client::connect(c, None).await.unwrap();
    match c.get(&[oid("1.3.6.1.2.1.1.1.0")]).await {
        Err(Error::Report { name, .. }) => assert_eq!(name, "usmStatsUnknownUserNames"),
        other => panic!("{other:?}"),
    }
    // Short pass phrases fail before anything is sent.
    let mut c = cfg(&a, Version::V3, "");
    c.user = Some(UsmUser {
        name: "x".into(),
        auth: Some((AuthProtocol::Sha1, "short".into())),
        privacy: None,
    });
    assert!(matches!(Client::connect(c, None).await, Err(Error::Usm(_))));
}

#[tokio::test]
async fn notifications_and_pcap() {
    let got: Arc<Mutex<Vec<Notification>>> = Arc::default();
    let sink = {
        let got = Arc::clone(&got);
        Arc::new(move |n: Notification| got.lock().unwrap().push(n))
    };
    let rec = Arc::new(PcapRecorder::default());
    rec.set_enabled(true);
    let tap = Tap::new(Arc::clone(&rec));
    let rx = Receiver::start(
        ReceiverConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            communities: vec!["public".into()],
            users: users(),
            engine_id: b"\x80\x00\x00\x00\x04wpg-receiver".to_vec(),
        },
        Some(tap.clone()),
        sink,
    )
    .await
    .unwrap();
    let target = rx.local_addr();
    let mk = |version: Version, community: &str, user: Option<UsmUser>| {
        let mut c = ClientConfig::v2c(target, community);
        c.version = version;
        c.user = user;
        c.timeout = Duration::from_millis(500);
        c
    };
    let link_down = oid("1.3.6.1.6.3.1.1.5.3");
    let vbs = [VarBind::new(
        oid("1.3.6.1.2.1.2.2.1.1.2"),
        Value::Integer(2),
    )];

    // v1 Trap-PDU, v1 via conversion, v2c trap, v2c inform, bad community.
    let mut c = Client::connect(mk(Version::V1, "public", None), Some(tap.clone()))
        .await
        .unwrap();
    c.trap_v1(TrapV1 {
        enterprise: oid("1.3.6.1.4.1.99999"),
        agent_addr: [127, 0, 0, 1],
        generic: 6,
        specific: 5,
        timestamp: 77,
        varbinds: vbs.to_vec(),
    })
    .await
    .unwrap();
    c.notify(&link_down, 100, &vbs, false).await.unwrap();
    let mut c = Client::connect(mk(Version::V2c, "public", None), Some(tap.clone()))
        .await
        .unwrap();
    c.notify(&link_down, 200, &vbs, false).await.unwrap();
    let ack = c
        .notify(&link_down, 300, &vbs, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ack.ty, PduType::Response);
    let mut c = Client::connect(mk(Version::V2c, "wrong", None), None)
        .await
        .unwrap();
    c.notify(&link_down, 1, &[], false).await.unwrap();
    // v3 traps and informs (authPriv), one with a wrong pass phrase.
    let shaaes = users()[2].clone();
    let mut c = Client::connect(mk(Version::V3, "", Some(shaaes.clone())), Some(tap.clone()))
        .await
        .unwrap();
    c.notify(&link_down, 400, &vbs, false).await.unwrap();
    let ack = c
        .notify(&link_down, 500, &vbs, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ack.varbinds.len(), 3);
    let mut bad = shaaes;
    bad.auth.as_mut().unwrap().1 = "not-the-pass".into();
    let mut c = Client::connect(mk(Version::V3, "", Some(bad)), None)
        .await
        .unwrap();
    c.notify(&link_down, 600, &vbs, false).await.unwrap();

    for _ in 0..100 {
        if got.lock().unwrap().len() >= 8 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    rx.stop().await;
    let got = got.lock().unwrap();
    assert_eq!(got.len(), 8, "{got:#?}");
    let v1 = &got[0];
    assert_eq!(v1.version, Version::V1);
    assert_eq!(v1.trap_oid, Some(oid("1.3.6.1.4.1.99999.0.5")));
    assert_eq!(v1.uptime, Some(77));
    assert_eq!(v1.v1.as_ref().unwrap().specific, 5);
    assert_eq!(
        got[1].trap_oid,
        Some(link_down.clone()),
        "converted v1 linkDown"
    );
    assert_eq!(got[2].pdu_type, Some(PduType::TrapV2));
    assert_eq!(got[2].uptime, Some(200));
    assert_eq!(got[2].varbinds, vbs);
    assert_eq!(got[3].pdu_type, Some(PduType::Inform));
    assert!(got[3].acknowledged);
    assert!(
        got[4].error.as_deref().unwrap().contains("wrong"),
        "{:?}",
        got[4]
    );
    assert_eq!(got[5].version, Version::V3);
    assert_eq!(got[5].user.as_deref(), Some("shaaes"));
    assert!(got[5].error.is_none(), "{:?}", got[5].error);
    assert_eq!(got[5].trap_oid, Some(link_down.clone()));
    assert_eq!(got[6].pdu_type, Some(PduType::Inform));
    assert!(got[6].acknowledged);
    assert!(got[7].error.is_some(), "wrong digest is reported");

    // The recording: every datagram (both directions, both sides), decodable
    // as Ethernet / IPv4 / UDP by wpg's own parser (lengths and checksums
    // consistent), carrying the SNMP message.
    let frames = rec.frames();
    assert!(frames.len() >= 12, "{}", frames.len());
    let catalog = wpg_protocols::catalog();
    for f in &frames {
        let p = wpg_core::parse::parse(&f.data, "ethernet2", catalog).unwrap();
        let protos: Vec<&str> = p
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(&protos[..3], ["ethernet2", "ipv4", "udp"], "{protos:?}");
        assert!(!p.has_errors(), "{:?}", p.diagnostics);
        let v = wpg_snmp::pdu::peek_version(&f.data[42..]).unwrap();
        assert!(matches!(v, 0 | 1 | 3), "{v}");
    }
    let pcap = rec.export(PcapFormat::Pcap);
    assert_eq!(
        pcap.len(),
        24 + frames.iter().map(|f| 16 + f.data.len()).sum::<usize>()
    );
}
