//! Application protocols over UDP (TODO §2.1 P1): `DHCPv4` (incl. relay agent
//! information), `DHCPv6` (incl. Relay-Forward / Relay-Reply), DNS, NTP, SNMP v1 /
//! v2c (`GetRequest` family and Trap), Syslog, RADIUS and TFTP.
//!
//! All of them are reached through the UDP **destination** port binding in
//! [`crate::l4::udp`] (67/68, 546/547, 53, 123, 161/162, 514, 1812/1813, 69).
//! A [`wpg_core::Binding`] selects on one field, so replies sent *from* a
//! well-known port to an ephemeral one (DNS / SNMP / RADIUS responses, TFTP
//! transfers between TIDs) build fine but parse back as `raw` after UDP; a
//! "source or destination port" selector would close the gap.
//!
//! ## Engine limitations exercised here
//!
//! * **DNS** names are sequences of length-prefixed labels ended by a zero octet,
//!   and the question / resource-record sections are sized by the `qdcount` /
//!   `ancount` / ... header counters. The engine's [`wpg_core::FieldType::List`]
//!   has neither a stop-on-terminator nor a count-driven length, and a
//!   [`wpg_core::Length`] cannot be "to the end minus N octets", so a name cannot
//!   be followed by typed `qtype` / `qclass` fields. [`dns`] therefore types the
//!   12-octet header and keeps the sections as one opaque `sections` region;
//!   [`dns_name`] / [`dns_question`] / [`dns_rr`] encode the label form for
//!   defaults and tests. A `List` stop condition (`until = 0`) plus a count-driven
//!   `Length::FromCount` would allow a fully typed message.
//! * **SNMP** is BER-encoded ASN.1. The message is modelled as a fixed layout of
//!   pre-encoded tag / length / value triples ([`snmp`] envelope, [`snmp_pdu`] /
//!   [`snmp_trap`] PDU) whose bodies are a [`wpg_core::FieldType::Tlv`] with
//!   8-bit tag and 8-bit length ([`ber_tlvs`]). That only covers the BER
//!   **short-form** length (contents below 128 octets): long-form lengths
//!   (`0x81 nn`, `0x82 nn nn`) need a variable-width length in `TlvDef`. The
//!   community length is Manual (auto rules only measure whole headers /
//!   payloads), and constructed tags nest to a fixed depth (varbind list →
//!   varbind → value) because a `TlvDef` cannot refer to itself. The PDU type is
//!   only known inside the PDU, so [`snmp`] defaults to [`snmp_pdu`], which
//!   re-decodes itself as [`snmp_trap`] for the v1 Trap tag (`0xa4`).
//! * **`DHCPv6`** Relay-Forward / Relay-Reply have a different fixed layout from the
//!   client / server messages on the same port, so [`dhcpv6`] re-decodes itself as
//!   [`dhcpv6_relay`] for message types 12 / 13 (a re-parse binding entry). The
//!   Relay Message option types its inner message one level deep only.
//! * **TFTP** request / error strings are NUL-terminated; the engine has no
//!   C-string type, so the RRQ / WRQ / OACK body and the error message are opaque
//!   octets (write them as `"file.bin\0octet\0"`). DATA / ACK / ERROR are split
//!   into [`tftp_data`] / [`tftp_ack`] / [`tftp_error`] via re-parse entries on
//!   the opcode.
//! * **RADIUS** Response Authenticator and Message-Authenticator are MD5 /
//!   HMAC-MD5 over the packet plus the shared secret; the engine has no keyed
//!   digest auto rule, so `authenticator` is Manual.
//! * **NTP** control (mode 6) and private (mode 7) messages have their own
//!   layouts; [`ntp`] models the RFC 5905 time packet and keeps extension fields
//!   / MAC as opaque `extensions`. **Syslog** is free text, a single opaque
//!   `message`.

use wpg_core::{
    protocol, Binding, BindingEntry, FieldDef, FieldType, FieldValue, ItemDef, Length, LengthUnit,
    ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn ip4(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv4)
}

fn ip6(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv6)
}

fn rest(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

fn list_to_end(name: &str, item: FieldType) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::List {
            item: ItemDef::Field { ty: Box::new(item) },
            length: Length::ToEnd,
        },
    )
}

/// Adds re-parse entries (`value` → `protocol`, re-decoding the current layer) to
/// `d.next`, creating the binding on `field` when there is none.
fn reparse(d: &mut ProtocolDef, field: &str, entries: &[(u64, &str)]) {
    let b = d
        .next
        .get_or_insert_with(|| Binding::new(field, Vec::new()));
    for (v, p) in entries {
        b.table.push(BindingEntry::new(*v, *p).reparsing());
    }
}

// --------------------------------------------------------------------------
// DHCPv4 (RFC 2131 / 2132 / 3046), UDP 67 / 68
// --------------------------------------------------------------------------

/// Relay Agent Information sub-options (RFC 3046): 8-bit code / length.
fn dhcpv4_relay_suboptions() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(1, "circuit_id", Vec::new()),
            TlvVariant::new(2, "remote_id", Vec::new()),
            TlvVariant::new(5, "link_selection", vec![ip4("address")]),
            TlvVariant::new(11, "server_id_override", vec![ip4("address")]),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// `DHCPv4` options (RFC 2132): 8-bit code / length (excluding the header), with
/// the type-only Pad (0) and End (255). Typed: subnet mask, router, DNS servers,
/// host / domain name, requested address, lease time, message type, server id,
/// parameter request list, client id and relay agent information (82, with
/// sub-options).
#[must_use]
pub fn dhcpv4_options() -> FieldType {
    let ipv4_list = |n: &str| list_to_end(n, FieldType::Ipv4);
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::type_only(0, "pad"),
            TlvVariant::new(1, "subnet_mask", vec![ip4("mask")]),
            TlvVariant::new(3, "router", vec![ipv4_list("addresses")]),
            TlvVariant::new(6, "dns_servers", vec![ipv4_list("addresses")]),
            TlvVariant::new(12, "host_name", Vec::new()),
            TlvVariant::new(15, "domain_name", Vec::new()),
            TlvVariant::new(50, "requested_ip", vec![ip4("address")]),
            TlvVariant::new(51, "lease_time", vec![u("seconds", 32)]),
            TlvVariant::new(
                53,
                "message_type",
                vec![FieldDef::new(
                    "type",
                    FieldType::Enum {
                        bits: 8,
                        variants: [
                            (1, "discover"),
                            (2, "offer"),
                            (3, "request"),
                            (4, "decline"),
                            (5, "ack"),
                            (6, "nak"),
                            (7, "release"),
                            (8, "inform"),
                        ]
                        .into_iter()
                        .map(|(value, name)| wpg_core::EnumVariant {
                            value,
                            name: name.into(),
                            description: None,
                        })
                        .collect(),
                    },
                )],
            ),
            TlvVariant::new(54, "server_id", vec![ip4("address")]),
            TlvVariant::new(
                55,
                "parameter_request_list",
                vec![list_to_end("codes", FieldType::UInt { bits: 8 })],
            ),
            TlvVariant::new(58, "renewal_time", vec![u("seconds", 32)]),
            TlvVariant::new(59, "rebinding_time", vec![u("seconds", 32)]),
            TlvVariant::new(61, "client_id", Vec::new()),
            TlvVariant::new(
                82,
                "relay_agent_info",
                vec![FieldDef::new("suboptions", dhcpv4_relay_suboptions())],
            ),
            TlvVariant::type_only(255, "end"),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// `DHCPv4` (RFC 2131, UDP 67 server / 68 client): the fixed BOOTP header
/// (`chaddr` split into the 6-octet Ethernet address and 10 octets of padding),
/// the magic cookie and the options. Relaying is expressed with `hops`, `giaddr`
/// and the Relay Agent Information option (82). Default: a DHCPDISCOVER.
#[must_use]
pub fn dhcpv4() -> ProtocolDef {
    let mut d = protocol! {
        id: "dhcpv4",
        name: "DHCPv4",
        layer: L7,
        fields: {
            op: enum(8) { 1 => boot_request, 2 => boot_reply } = 1, desc = "dhcp-op";
            htype: uint(8) = 1, desc = "dhcp-htype";
            hlen: uint(8) = 6, desc = "dhcp-hlen";
            hops: uint(8) = 0, desc = "dhcp-hops";
            xid: uint(32) = 0x1234_5678, desc = "dhcp-xid";
            secs: uint(16) = 0, desc = "dhcp-secs";
            flags: flags(16) { 15 => broadcast } = 0, desc = "dhcp-flags";
            ciaddr: ipv4 = [0, 0, 0, 0], desc = "dhcp-ciaddr";
            yiaddr: ipv4 = [0, 0, 0, 0], desc = "dhcp-yiaddr";
            siaddr: ipv4 = [0, 0, 0, 0], desc = "dhcp-siaddr";
            giaddr: ipv4 = [0, 0, 0, 0], desc = "dhcp-giaddr";
            chaddr: mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01], desc = "dhcp-chaddr";
            chaddr_padding: bytes(10) = [0u8; 10];
            sname: bytes(64) = [0u8; 64], desc = "dhcp-sname";
            file: bytes(128) = [0u8; 128], desc = "dhcp-file";
            magic_cookie: uint(32) = 0x6382_5363, desc = "dhcp-magic-cookie";
            options: ty(dhcpv4_options()), desc = "dhcp-options";
        },
        constraints: {
            warning: equals(magic_cookie, 0x6382_5363);
        },
    };
    d.fields[16].default = Some(FieldValue::from(vec![
        TlvItem::named("message_type").with("type", 1u8),
        TlvItem::named("end"),
    ]));
    d
}

// --------------------------------------------------------------------------
// DHCPv6 (RFC 8415), UDP 546 / 547
// --------------------------------------------------------------------------

fn dhcpv6_tlv(variants: Vec<TlvVariant>) -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// Options that appear inside `IA_NA` / `IA_TA` / `IA_PD`.
fn dhcpv6_ia_suboptions() -> FieldType {
    dhcpv6_tlv(vec![
        TlvVariant::new(
            5,
            "ia_addr",
            vec![
                ip6("address"),
                u("preferred_lifetime", 32),
                u("valid_lifetime", 32),
            ],
        ),
        TlvVariant::new(13, "status_code", vec![u("code", 16), rest("message")]),
        TlvVariant::new(
            26,
            "ia_prefix",
            vec![
                u("preferred_lifetime", 32),
                u("valid_lifetime", 32),
                u("prefix_length", 8),
                ip6("prefix"),
            ],
        ),
    ])
}

fn dhcpv6_msg_type() -> FieldType {
    FieldType::Enum {
        bits: 8,
        variants: [
            (1, "solicit"),
            (2, "advertise"),
            (3, "request"),
            (4, "confirm"),
            (5, "renew"),
            (6, "rebind"),
            (7, "reply"),
            (8, "release"),
            (9, "decline"),
            (10, "reconfigure"),
            (11, "information_request"),
            (12, "relay_forw"),
            (13, "relay_repl"),
        ]
        .into_iter()
        .map(|(value, name)| wpg_core::EnumVariant {
            value,
            name: name.into(),
            description: None,
        })
        .collect(),
    }
}

/// `DHCPv6` options (RFC 8415): 16-bit code / length (excluding the header). `IA_NA` /
/// `IA_TA` / `IA_PD` carry nested IA options (IA Address, IA Prefix, Status Code).
/// With `relay`, the Relay Message option (9) is typed as one inner client /
/// server message (`msg_type`, `transaction_id`, `options`); otherwise its value
/// is opaque.
#[must_use]
pub fn dhcpv6_options(relay: bool) -> FieldType {
    let ia = |code, name: &str, with_times: bool| {
        let mut f = vec![u("iaid", 32)];
        if with_times {
            f.push(u("t1", 32));
            f.push(u("t2", 32));
        }
        f.push(FieldDef::new("options", dhcpv6_ia_suboptions()));
        TlvVariant::new(code, name, f)
    };
    let relay_message = if relay {
        TlvVariant::new(
            9,
            "relay_message",
            vec![
                FieldDef::new("msg_type", dhcpv6_msg_type()),
                u("transaction_id", 24),
                FieldDef::new("options", dhcpv6_options(false)),
            ],
        )
    } else {
        TlvVariant::new(9, "relay_message", Vec::new())
    };
    dhcpv6_tlv(vec![
        TlvVariant::new(1, "client_id", Vec::new()),
        TlvVariant::new(2, "server_id", Vec::new()),
        ia(3, "ia_na", true),
        ia(4, "ia_ta", false),
        TlvVariant::new(
            6,
            "oro",
            vec![list_to_end("codes", FieldType::UInt { bits: 16 })],
        ),
        TlvVariant::new(7, "preference", vec![u("value", 8)]),
        TlvVariant::new(8, "elapsed_time", vec![u("time", 16)]),
        relay_message,
        TlvVariant::new(13, "status_code", vec![u("code", 16), rest("message")]),
        TlvVariant::new(14, "rapid_commit", Vec::new()),
        TlvVariant::new(18, "interface_id", Vec::new()),
        TlvVariant::new(
            23,
            "dns_servers",
            vec![list_to_end("addresses", FieldType::Ipv6)],
        ),
        TlvVariant::new(24, "domain_list", Vec::new()),
        ia(25, "ia_pd", true),
        TlvVariant::new(37, "remote_id", vec![u("enterprise", 32), rest("id")]),
    ])
}

/// DUID-LL (type 3, hardware type 1) for `mac`.
fn duid_ll(mac: [u8; 6]) -> Vec<u8> {
    let mut v = vec![0x00, 0x03, 0x00, 0x01];
    v.extend_from_slice(&mac);
    v
}

/// `DHCPv6` client / server message (RFC 8415, UDP 546 client / 547 server):
/// message type, 24-bit transaction id and options. Relay-Forward / Relay-Reply
/// (12 / 13) re-decode as [`dhcpv6_relay`]. Default: a Solicit with Client
/// Identifier (DUID-LL), Elapsed Time and `IA_NA`.
#[must_use]
pub fn dhcpv6() -> ProtocolDef {
    let mut d = protocol! {
        id: "dhcpv6",
        name: "DHCPv6",
        layer: L7,
        fields: {
            msg_type: ty(dhcpv6_msg_type()) = 1u8, desc = "dhcpv6-msg-type";
            transaction_id: uint(24) = 0x00_1234, desc = "dhcpv6-transaction-id";
            options: ty(dhcpv6_options(false)), desc = "dhcpv6-options";
        },
    };
    d.fields[2].default = Some(FieldValue::from(vec![
        TlvItem::with_type(1, duid_ll([0x02, 0, 0, 0, 0, 0x01])),
        TlvItem::named("elapsed_time").with("time", 0u16),
        TlvItem::named("ia_na")
            .with("iaid", 1u32)
            .with("t1", 0u32)
            .with("t2", 0u32)
            .with("options", FieldValue::Tlv(Vec::new())),
    ]));
    reparse(
        &mut d,
        "msg_type",
        &[(12, "dhcpv6_relay"), (13, "dhcpv6_relay")],
    );
    d
}

/// `DHCPv6` Relay-Forward / Relay-Reply (RFC 8415 §9, UDP 547): message type, hop
/// count, link / peer addresses and options (typically Interface-Id and the
/// Relay Message carrying the client message).
#[must_use]
pub fn dhcpv6_relay() -> ProtocolDef {
    let mut d = protocol! {
        id: "dhcpv6_relay",
        name: "DHCPv6 Relay",
        layer: L7,
        fields: {
            msg_type: enum(8) { 12 => relay_forw, 13 => relay_repl } = 12, desc = "dhcpv6-msg-type";
            hop_count: uint(8) = 0, desc = "dhcpv6-hop-count";
            link_address: ipv6 = [0u16; 8], desc = "dhcpv6-link-address";
            peer_address: ipv6 = [0xfe80u16, 0, 0, 0, 0, 0, 0, 1], desc = "dhcpv6-peer-address";
            options: ty(dhcpv6_options(true)), desc = "dhcpv6-options";
        },
        constraints: {
            warning: in_range(msg_type, 12, 13);
        },
    };
    d.fields[4].default = Some(FieldValue::from(vec![TlvItem::named("relay_message")
        .with("msg_type", 1u8)
        .with("transaction_id", 0x00_1234u32)
        .with(
            "options",
            FieldValue::from(vec![TlvItem::with_type(
                1,
                duid_ll([0x02, 0, 0, 0, 0, 0x01]),
            )]),
        )]));
    d
}

// --------------------------------------------------------------------------
// DNS (RFC 1035), UDP 53
// --------------------------------------------------------------------------

/// Encodes a domain name as length-prefixed labels ending with the root label
/// (`"example.com"` → `07 example 03 com 00`). Empty labels are skipped, so `"."`
/// and `""` give the root name alone.
///
/// # Panics
/// If a label is longer than 63 octets.
#[must_use]
pub fn dns_name(name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(name.len() + 2);
    for label in name.split('.').filter(|l| !l.is_empty()) {
        let n = u8::try_from(label.len())
            .ok()
            .filter(|n| *n <= 63)
            .expect("DNS label longer than 63 octets");
        out.push(n);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    out
}

/// One question entry: name, `qtype` and `qclass`.
#[must_use]
pub fn dns_question(name: &str, qtype: u16, qclass: u16) -> Vec<u8> {
    let mut v = dns_name(name);
    v.extend_from_slice(&qtype.to_be_bytes());
    v.extend_from_slice(&qclass.to_be_bytes());
    v
}

/// One resource record: owner name, type, class, TTL, RDLENGTH and RDATA.
///
/// # Panics
/// If `rdata` is longer than 65535 octets.
#[must_use]
pub fn dns_rr(name: &str, rtype: u16, class: u16, ttl: u32, rdata: &[u8]) -> Vec<u8> {
    let mut v = dns_question(name, rtype, class);
    v.extend_from_slice(&ttl.to_be_bytes());
    let len = u16::try_from(rdata.len()).expect("RDATA longer than 65535 octets");
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(rdata);
    v
}

/// DNS message (RFC 1035 §4.1, UDP 53): the 12-octet header with the flag word
/// split into its bit fields, and the question / answer / authority / additional
/// sections as one opaque `sections` region (see the module docs). Default: a
/// recursive query for `example.com` A IN.
#[must_use]
pub fn dns() -> ProtocolDef {
    protocol! {
        id: "dns",
        name: "DNS",
        layer: L7,
        fields: {
            id: uint(16) = 0x1234, desc = "dns-id";
            qr: enum(1) { 0 => query, 1 => response } = 0, desc = "dns-qr";
            opcode: enum(4) {
                0 => query,
                1 => iquery,
                2 => status,
                4 => notify,
                5 => update,
            } = 0, desc = "dns-opcode";
            aa: uint(1) = 0, desc = "dns-aa";
            tc: uint(1) = 0, desc = "dns-tc";
            rd: uint(1) = 1, desc = "dns-rd";
            ra: uint(1) = 0, desc = "dns-ra";
            z: uint(1) = 0, desc = "dns-z";
            ad: uint(1) = 0, desc = "dns-ad";
            cd: uint(1) = 0, desc = "dns-cd";
            rcode: enum(4) {
                0 => no_error,
                1 => format_error,
                2 => server_failure,
                3 => nx_domain,
                4 => not_implemented,
                5 => refused,
            } = 0, desc = "dns-rcode";
            qdcount: uint(16) = 1, desc = "dns-qdcount";
            ancount: uint(16) = 0, desc = "dns-ancount";
            nscount: uint(16) = 0, desc = "dns-nscount";
            arcount: uint(16) = 0, desc = "dns-arcount";
            sections: bytes(to_end) = dns_question("example.com", 1, 1), desc = "dns-sections";
        },
        constraints: {
            warning: equals(z, 0);
            warning: max_length(total, 512);
        },
    }
}

// --------------------------------------------------------------------------
// NTP (RFC 5905), UDP 123
// --------------------------------------------------------------------------

/// `NTPv4` packet (RFC 5905, UDP 123): leap indicator, version, mode, stratum,
/// poll / precision (signed log2 seconds), root delay / dispersion (16.16 fixed
/// point), reference id and the four 64-bit timestamps (32.32 fixed point,
/// seconds since 1900), plus opaque extension fields / MAC. Default: a client
/// request (mode 3).
#[must_use]
pub fn ntp() -> ProtocolDef {
    protocol! {
        id: "ntp",
        name: "NTP",
        layer: L7,
        fields: {
            li: enum(2) {
                0 => no_warning,
                1 => last_minute_61,
                2 => last_minute_59,
                3 => unsynchronized,
            } = 0, desc = "ntp-li";
            version: uint(3) = 4, desc = "ntp-version";
            mode: enum(3) {
                0 => reserved,
                1 => symmetric_active,
                2 => symmetric_passive,
                3 => client,
                4 => server,
                5 => broadcast,
                6 => control,
                7 => private,
            } = 3, desc = "ntp-mode";
            stratum: uint(8) = 0, desc = "ntp-stratum";
            poll: int(8) = 6, desc = "ntp-poll";
            precision: int(8) = -20, desc = "ntp-precision";
            root_delay: uint(32) = 0, desc = "ntp-root-delay";
            root_dispersion: uint(32) = 0, desc = "ntp-root-dispersion";
            reference_id: bytes(4) = [0u8; 4], desc = "ntp-reference-id";
            reference_ts: uint(64) = 0, desc = "ntp-reference-ts";
            origin_ts: uint(64) = 0, desc = "ntp-origin-ts";
            receive_ts: uint(64) = 0, desc = "ntp-receive-ts";
            transmit_ts: uint(64) = 0, desc = "ntp-transmit-ts";
            extensions: bytes(to_end) = [], desc = "ntp-extensions";
        },
        constraints: {
            warning: in_range(version, 1, 4);
        },
    }
}

// --------------------------------------------------------------------------
// SNMP v1 / v2c (RFC 1157 / RFC 3416), UDP 161 / 162
// --------------------------------------------------------------------------

/// BER encoding of a non-negative INTEGER value (minimal two's complement).
#[must_use]
pub fn ber_uint(v: u64) -> Vec<u8> {
    let b = v.to_be_bytes();
    let first = b.iter().position(|x| *x != 0).unwrap_or(7);
    let mut out = b[first..].to_vec();
    if out[0] & 0x80 != 0 {
        out.insert(0, 0);
    }
    out
}

/// BER encoding of an OBJECT IDENTIFIER's contents (`[1, 3, 6, 1, ...]`).
///
/// # Panics
/// If fewer than two arcs are given.
#[must_use]
pub fn ber_oid(arcs: &[u64]) -> Vec<u8> {
    assert!(arcs.len() >= 2, "an OID has at least two arcs");
    let mut out = Vec::new();
    let mut push = |mut v: u64| {
        let mut tmp = vec![u8::try_from(v & 0x7f).unwrap_or(0)];
        v >>= 7;
        while v > 0 {
            tmp.push(u8::try_from(v & 0x7f).unwrap_or(0) | 0x80);
            v >>= 7;
        }
        tmp.reverse();
        out.extend(tmp);
    };
    push(arcs[0] * 40 + arcs[1]);
    for a in &arcs[2..] {
        push(*a);
    }
    out
}

/// BER TLVs with an 8-bit tag and an 8-bit (short-form) length. Universal
/// INTEGER / OCTET STRING / NULL / OBJECT IDENTIFIER, the SNMP application types
/// (`IpAddress`, Counter32, Gauge32, `TimeTicks`, Opaque, Counter64) and the v2
/// exception values; SEQUENCE (`0x30`) is constructed and nests `depth` levels
/// (its `items` are again BER TLVs). See the module docs for the short-form limit.
#[must_use]
pub fn ber_tlvs(depth: u8) -> FieldType {
    let mut variants = vec![
        TlvVariant::new(0x02, "integer", Vec::new()),
        TlvVariant::new(0x04, "octet_string", Vec::new()),
        TlvVariant::new(0x05, "null", Vec::new()),
        TlvVariant::new(0x06, "object_identifier", Vec::new()),
        TlvVariant::new(0x40, "ip_address", vec![ip4("address")]),
        TlvVariant::new(0x41, "counter32", Vec::new()),
        TlvVariant::new(0x42, "gauge32", Vec::new()),
        TlvVariant::new(0x43, "timeticks", Vec::new()),
        TlvVariant::new(0x44, "opaque", Vec::new()),
        TlvVariant::new(0x46, "counter64", Vec::new()),
        TlvVariant::new(0x80, "no_such_object", Vec::new()),
        TlvVariant::new(0x81, "no_such_instance", Vec::new()),
        TlvVariant::new(0x82, "end_of_mib_view", Vec::new()),
    ];
    if depth > 0 {
        variants.push(TlvVariant::new(
            0x30,
            "sequence",
            vec![FieldDef::new("items", ber_tlvs(depth - 1))],
        ));
    }
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// A SEQUENCE item holding `items`.
fn ber_seq(items: Vec<TlvItem>) -> TlvItem {
    TlvItem::named("sequence").with("items", FieldValue::from(items))
}

/// A `VarBind` `{ name, NULL }`.
fn ber_varbind_null(oid: &[u64]) -> TlvItem {
    ber_seq(vec![
        TlvItem::with_type(0x06, ber_oid(oid)),
        TlvItem::named("null"),
    ])
}

/// SNMP message envelope (RFC 1157 / 3416, UDP 161 agent / 162 trap receiver):
/// the outer SEQUENCE header (`msg_length` Auto), `version` INTEGER (0 = v1,
/// 1 = v2c) and `community` OCTET STRING (`community_length` Manual), as
/// pre-encoded BER triples. The PDU follows as the next layer ([`snmp_pdu`],
/// or [`snmp_trap`] for a v1 Trap).
#[must_use]
pub fn snmp() -> ProtocolDef {
    let mut d = protocol! {
        id: "snmp",
        name: "SNMP",
        layer: L7,
        fields: {
            msg_tag: uint(8) = 0x30, desc = "snmp-tag";
            msg_length: uint(8), auto = length(total, adjust = -2), desc = "snmp-msg-length";
            version_tag: uint(8) = 0x02, desc = "snmp-tag";
            version_length: uint(8) = 1, desc = "snmp-version-length";
            version: enum(8) { 0 => v1, 1 => v2c } = 1, desc = "snmp-version";
            community_tag: uint(8) = 0x04, desc = "snmp-tag";
            community_length: uint(8) = 6, desc = "snmp-community-length";
            community: bytes(from = community_length) = *b"public", desc = "snmp-community";
        },
        next: version {},
        constraints: {
            warning: equals(msg_tag, 0x30);
            warning: max_length(total, 129);
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.default = Some("snmp_pdu".into());
    }
    d
}

/// SNMP PDU (RFC 3416 / RFC 1157): `pdu_type` tag (`GetRequest` / `GetNext` /
/// Response / Set / `GetBulk` / Inform / SNMPv2-Trap / Report), `pdu_length` (Auto)
/// and the PDU contents as BER TLVs: request-id, error-status, error-index (or
/// non-repeaters / max-repetitions for `GetBulk`) and the variable-bindings
/// SEQUENCE. The v1 Trap tag (`0xa4`) re-decodes as [`snmp_trap`]. Default: a
/// `GetRequest` for `sysDescr.0`.
#[must_use]
pub fn snmp_pdu() -> ProtocolDef {
    let mut d = protocol! {
        id: "snmp_pdu",
        name: "SNMP PDU",
        layer: L7,
        fields: {
            pdu_type: enum(8) {
                0xa0 => get_request,
                0xa1 => get_next_request,
                0xa2 => response,
                0xa3 => set_request,
                0xa5 => get_bulk_request,
                0xa6 => inform_request,
                0xa7 => snmpv2_trap,
                0xa8 => report,
            } = 0xa0, desc = "snmp-pdu-type";
            pdu_length: uint(8), auto = length(total, adjust = -2), desc = "snmp-pdu-length";
            body: ty(ber_tlvs(2)), desc = "snmp-pdu-body";
        },
        constraints: {
            warning: max_length(total, 129);
        },
    };
    d.fields[2].default = Some(FieldValue::from(vec![
        TlvItem::with_type(0x02, ber_uint(1)), // request-id
        TlvItem::with_type(0x02, ber_uint(0)), // error-status
        TlvItem::with_type(0x02, ber_uint(0)), // error-index
        ber_seq(vec![ber_varbind_null(&[1, 3, 6, 1, 2, 1, 1, 1, 0])]),
    ]));
    reparse(&mut d, "pdu_type", &[(0xa4, "snmp_trap")]);
    d
}

/// `SNMPv1` Trap-PDU (RFC 1157 §4.1.6, tag `0xa4`): `pdu_length` (Auto) and the
/// BER contents: enterprise OID, agent-addr (`IpAddress`), generic-trap,
/// specific-trap, time-stamp (`TimeTicks`) and the variable-bindings SEQUENCE.
/// Default: a coldStart trap from `192.0.2.1`.
#[must_use]
pub fn snmp_trap() -> ProtocolDef {
    let mut d = protocol! {
        id: "snmp_trap",
        name: "SNMPv1 Trap",
        layer: L7,
        fields: {
            pdu_type: enum(8) { 0xa4 => trap } = 0xa4, desc = "snmp-pdu-type";
            pdu_length: uint(8), auto = length(total, adjust = -2), desc = "snmp-pdu-length";
            body: ty(ber_tlvs(2)), desc = "snmp-trap-body";
        },
        constraints: {
            warning: equals(pdu_type, 0xa4);
            warning: max_length(total, 129);
        },
    };
    d.fields[2].default = Some(FieldValue::from(vec![
        TlvItem::with_type(0x06, ber_oid(&[1, 3, 6, 1, 4, 1, 8072])), // enterprise
        TlvItem::named("ip_address").with("address", std::net::Ipv4Addr::new(192, 0, 2, 1)),
        TlvItem::with_type(0x02, ber_uint(0)), // generic-trap: coldStart
        TlvItem::with_type(0x02, ber_uint(0)), // specific-trap
        TlvItem::with_type(0x43, ber_uint(0)), // time-stamp
        ber_seq(Vec::new()),
    ]));
    d
}

// --------------------------------------------------------------------------
// Syslog (RFC 5424 / RFC 3164), UDP 514
// --------------------------------------------------------------------------

/// Syslog over UDP (RFC 5426, port 514): the whole message as text octets,
/// starting with the `<PRI>` part (facility * 8 + severity). Default: an RFC 5424
/// message (facility 4, severity 2).
#[must_use]
pub fn syslog() -> ProtocolDef {
    protocol! {
        id: "syslog",
        name: "Syslog",
        layer: L7,
        fields: {
            message: bytes(to_end) = *b"<34>1 2026-01-01T00:00:00Z host app - - - hello", desc = "syslog-message";
        },
    }
}

// --------------------------------------------------------------------------
// RADIUS (RFC 2865 / 2866), UDP 1812 / 1813
// --------------------------------------------------------------------------

/// RADIUS attributes (RFC 2865 / 2866): 8-bit type, 8-bit length including the
/// header. Address / integer attributes are typed; string / text attributes keep
/// their value octets.
#[must_use]
pub fn radius_attributes() -> FieldType {
    let int = |t, n: &str| TlvVariant::new(t, n, vec![u("value", 32)]);
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(1, "user_name", Vec::new()),
            TlvVariant::new(2, "user_password", Vec::new()),
            TlvVariant::new(3, "chap_password", Vec::new()),
            TlvVariant::new(4, "nas_ip_address", vec![ip4("address")]),
            int(5, "nas_port"),
            int(6, "service_type"),
            int(7, "framed_protocol"),
            TlvVariant::new(8, "framed_ip_address", vec![ip4("address")]),
            TlvVariant::new(18, "reply_message", Vec::new()),
            TlvVariant::new(24, "state", Vec::new()),
            TlvVariant::new(25, "class", Vec::new()),
            TlvVariant::new(
                26,
                "vendor_specific",
                vec![u("vendor_id", 32), rest("value")],
            ),
            int(27, "session_timeout"),
            TlvVariant::new(30, "called_station_id", Vec::new()),
            TlvVariant::new(31, "calling_station_id", Vec::new()),
            TlvVariant::new(32, "nas_identifier", Vec::new()),
            int(40, "acct_status_type"),
            TlvVariant::new(44, "acct_session_id", Vec::new()),
            int(61, "nas_port_type"),
            TlvVariant::new(79, "eap_message", Vec::new()),
            TlvVariant::new(80, "message_authenticator", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "length".into(),
            unit: 1,
            adjust: -20,
        },
    })
}

/// RADIUS (RFC 2865, UDP 1812 authentication / 1813 accounting): code,
/// identifier, `length` (Auto), the 16-octet authenticator (Manual, see the
/// module docs) and the attributes. Default: an Access-Request with User-Name
/// and NAS-IP-Address.
#[must_use]
pub fn radius() -> ProtocolDef {
    let mut d = protocol! {
        id: "radius",
        name: "RADIUS",
        layer: L7,
        fields: {
            code: enum(8) {
                1 => access_request,
                2 => access_accept,
                3 => access_reject,
                4 => accounting_request,
                5 => accounting_response,
                11 => access_challenge,
                12 => status_server,
                40 => disconnect_request,
                41 => disconnect_ack,
                42 => disconnect_nak,
                43 => coa_request,
                44 => coa_ack,
                45 => coa_nak,
            } = 1, desc = "radius-code";
            identifier: uint(8) = 0, desc = "radius-identifier";
            length: uint(16), auto = length(total), desc = "radius-length";
            authenticator: bytes(16) = [0u8; 16], desc = "radius-authenticator";
            attributes: ty(radius_attributes()), desc = "radius-attributes";
        },
        constraints: {
            warning: in_range(length, 20, 4096);
        },
    };
    d.fields[4].default = Some(FieldValue::from(vec![
        TlvItem::with_type(1, b"user".to_vec()),
        TlvItem::named("nas_ip_address").with("address", std::net::Ipv4Addr::new(192, 0, 2, 1)),
    ]));
    d
}

// --------------------------------------------------------------------------
// TFTP (RFC 1350 / RFC 2347), UDP 69
// --------------------------------------------------------------------------

fn tftp_opcode() -> FieldType {
    FieldType::Enum {
        bits: 16,
        variants: [
            (1, "rrq"),
            (2, "wrq"),
            (3, "data"),
            (4, "ack"),
            (5, "error"),
            (6, "oack"),
        ]
        .into_iter()
        .map(|(value, name)| wpg_core::EnumVariant {
            value,
            name: name.into(),
            description: None,
        })
        .collect(),
    }
}

/// TFTP request (RFC 1350 / 2347, UDP 69): `opcode` (RRQ / WRQ / OACK) and the
/// NUL-terminated filename, mode and option strings as opaque `body` octets (see
/// the module docs). DATA / ACK / ERROR re-decode as [`tftp_data`] /
/// [`tftp_ack`] / [`tftp_error`]. Default: an octet-mode read request.
#[must_use]
pub fn tftp() -> ProtocolDef {
    let mut d = protocol! {
        id: "tftp",
        name: "TFTP",
        layer: L7,
        fields: {
            opcode: ty(tftp_opcode()) = 1u16, desc = "tftp-opcode";
            body: bytes(to_end) = *b"boot.bin\0octet\0", desc = "tftp-body";
        },
    };
    reparse(
        &mut d,
        "opcode",
        &[(3, "tftp_data"), (4, "tftp_ack"), (5, "tftp_error")],
    );
    d
}

/// TFTP DATA (opcode 3): 16-bit block number and up to 512 data octets.
#[must_use]
pub fn tftp_data() -> ProtocolDef {
    protocol! {
        id: "tftp_data",
        name: "TFTP DATA",
        layer: L7,
        fields: {
            opcode: ty(tftp_opcode()) = 3u16, desc = "tftp-opcode";
            block: uint(16) = 1, desc = "tftp-block";
            data: bytes(to_end) = [], desc = "tftp-data";
        },
        constraints: {
            warning: equals(opcode, 3);
        },
    }
}

/// TFTP ACK (opcode 4): the acknowledged block number.
#[must_use]
pub fn tftp_ack() -> ProtocolDef {
    protocol! {
        id: "tftp_ack",
        name: "TFTP ACK",
        layer: L7,
        fields: {
            opcode: ty(tftp_opcode()) = 4u16, desc = "tftp-opcode";
            block: uint(16) = 1, desc = "tftp-block";
        },
        constraints: {
            warning: equals(opcode, 4);
        },
    }
}

/// TFTP ERROR (opcode 5): error code and the NUL-terminated message (opaque).
#[must_use]
pub fn tftp_error() -> ProtocolDef {
    protocol! {
        id: "tftp_error",
        name: "TFTP ERROR",
        layer: L7,
        fields: {
            opcode: ty(tftp_opcode()) = 5u16, desc = "tftp-opcode";
            error_code: enum(16) {
                0 => not_defined,
                1 => file_not_found,
                2 => access_violation,
                3 => disk_full,
                4 => illegal_operation,
                5 => unknown_tid,
                6 => file_exists,
                7 => no_such_user,
                8 => option_negotiation,
            } = 1, desc = "tftp-error-code";
            message: bytes(to_end) = *b"File not found\0", desc = "tftp-error-message";
        },
        constraints: {
            warning: equals(opcode, 5);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defs_are_valid() {
        for d in [
            dhcpv4(),
            dhcpv6(),
            dhcpv6_relay(),
            dns(),
            ntp(),
            snmp(),
            snmp_pdu(),
            snmp_trap(),
            syslog(),
            radius(),
            tftp(),
            tftp_data(),
            tftp_ack(),
            tftp_error(),
        ] {
            d.validate()
                .unwrap_or_else(|e| panic!("{} invalid: {e}", d.id));
        }
        // BOOTP fixed part (236 octets) + magic cookie.
        let dhcp_bits: u32 = dhcpv4()
            .fields
            .iter()
            .filter_map(|f| f.ty.fixed_bits())
            .sum();
        assert_eq!(dhcp_bits, 240 * 8);
        let ntp_bits: u32 = ntp().fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
        assert_eq!(ntp_bits, 48 * 8);
        let dns_bits: u32 = dns().fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
        assert_eq!(dns_bits, 12 * 8);
        assert_eq!(tftp_ack().fixed_header_bits(), Some(32));
    }

    #[test]
    fn encoders() {
        assert_eq!(dns_name("example.com"), b"\x07example\x03com\x00");
        assert_eq!(dns_name("."), [0]);
        assert_eq!(ber_uint(0), [0]);
        assert_eq!(ber_uint(127), [0x7f]);
        assert_eq!(ber_uint(128), [0x00, 0x80]);
        assert_eq!(ber_uint(0x1234), [0x12, 0x34]);
        assert_eq!(
            ber_oid(&[1, 3, 6, 1, 2, 1, 1, 1, 0]),
            [0x2b, 6, 1, 2, 1, 1, 1, 0]
        );
        assert_eq!(
            ber_oid(&[1, 3, 6, 1, 4, 1, 8072]),
            [0x2b, 6, 1, 4, 1, 0xbf, 0x08]
        );
    }
}
