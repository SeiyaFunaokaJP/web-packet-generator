//! The server's replies decode cleanly with wpg's own `dhcpv4` / `dhcpv6`
//! protocol definitions (field layout, option lengths).

use wpg_dhcp::config::{V4Config, V6Config};
use wpg_dhcp::v4::{opt, Message, MessageType, FLAG_BROADCAST};
use wpg_dhcp::v6::{self, duid_ll, Ia, IaKind};
use wpg_dhcp::{server4, server6, Leases};

fn clean(bytes: &[u8], proto: &str) -> Vec<String> {
    let p = wpg_core::parse::parse(bytes, proto, wpg_protocols::catalog()).unwrap();
    assert!(!p.has_errors(), "{proto}: {:?}", p.diagnostics);
    p.packet.layers.iter().map(|l| l.protocol.clone()).collect()
}

#[test]
fn dhcpv4_offer_decodes() {
    let cfg = V4Config {
        routers: vec!["192.168.100.1".parse().unwrap()],
        dns: vec!["192.168.100.1".parse().unwrap()],
        domain: "lab.example".into(),
        ntp: vec!["192.168.100.2".parse().unwrap()],
        ..V4Config::default()
    };
    let mut d = Message {
        xid: 7,
        flags: FLAG_BROADCAST,
        ..Message::default()
    };
    d.chaddr[..6].copy_from_slice(&[2, 0, 0, 0, 0, 1]);
    d.set_option(opt::MESSAGE_TYPE, vec![MessageType::Discover.as_u8()]);
    d.set_option(opt::HOSTNAME, b"host".to_vec());
    let out = server4::handle(
        &cfg,
        "192.168.100.1".parse().unwrap(),
        &mut Leases::default(),
        0,
        &d,
    );
    let offer = out.reply.unwrap().msg.encode();
    assert_eq!(clean(&offer, "dhcpv4")[0], "dhcpv4");
    assert_eq!(clean(&d.encode(), "dhcpv4")[0], "dhcpv4");
}

#[test]
fn dhcpv6_advertise_decodes() {
    let cfg = V6Config {
        enabled: true,
        pd_enabled: true,
        dns: vec!["2001:db8::53".parse().unwrap()],
        domains: vec!["lab.example".into()],
        rapid_commit: false,
        preference: 10,
        ..V6Config::default()
    };
    let mut sol = v6::Message {
        msg_type: v6::msg::SOLICIT,
        xid: 1,
        options: vec![(v6::opt::CLIENTID, duid_ll([2, 0, 0, 0, 0, 1]))],
    };
    for kind in [IaKind::Na, IaKind::Pd] {
        let (c, d) = Ia {
            kind,
            iaid: 1,
            t1: 0,
            t2: 0,
            addrs: Vec::new(),
            status: None,
        }
        .encode();
        sol.push(c, d);
    }
    let server = duid_ll([2, 0, 0, 0, 0, 0xff]);
    let out = server6::handle(&cfg, &server, &mut Leases::default(), 0, &sol);
    let adv = out.reply.unwrap().encode();
    assert_eq!(clean(&adv, "dhcpv6")[0], "dhcpv6");
    assert_eq!(clean(&sol.encode(), "dhcpv6")[0], "dhcpv6");
}
