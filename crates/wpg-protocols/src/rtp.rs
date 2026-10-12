//! RTP and RTCP (RFC 3550) (TODO §2.1 P2, M7).
//!
//! * [`rtp`]: the fixed header, the CSRC list (count Auto) and the optional header
//!   extension (X bit and length Auto). The media payload is the next layer
//!   (`payload` / `raw`); padding (P bit) is Manual.
//! * RTCP: one layer per RTCP packet, so a compound packet is a stack of RTCP
//!   layers. [`rtcp`] is the generic packet (common header and opaque body); it
//!   re-decodes itself as [`rtcp_sr`], [`rtcp_rr`], [`rtcp_sdes`], [`rtcp_bye`] or
//!   [`rtcp_app`] by its packet type. The report / source counts and the length
//!   (32-bit words minus one) are Auto; every packet ends at its length, and what
//!   follows parses as the next RTCP packet.
//!
//! RTP has no well-known port: the session ports are signalled (SDP), so UDP
//! demultiplexes RTP only on the IANA `rtp-avp` / `rtcp-avp` ports 5004 / 5005.
//! For any other port put the `rtp` / `rtcp` layer after `udp` explicitly (the
//! parser then leaves the payload as `raw`).

use wpg_core::{
    protocol, AutoCompute, Binding, BindingEntry, EnumVariant, FieldDef, FieldType, FieldValue,
    ItemDef, Length, ProtocolDef,
};

/// Id of the RTP header ([`rtp`]).
pub const RTP: &str = "rtp";

/// Id of the generic RTCP packet ([`rtcp`]).
pub const RTCP: &str = "rtcp";

/// IANA `rtp-avp` UDP port (RFC 3551 §8).
pub const RTP_PORT: u64 = 5004;

/// IANA `rtcp-avp` UDP port (RFC 3551 §8).
pub const RTCP_PORT: u64 = 5005;

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn u32_list(length: Length) -> FieldType {
    FieldType::List {
        item: ItemDef::Field {
            ty: Box::new(FieldType::UInt { bits: 32 }),
        },
        length,
    }
}

fn count(field: &str) -> Length {
    Length::Count {
        field: field.into(),
    }
}

/// The rest of an RTCP packet: up to `(length + 1) * 4` octets from its start.
fn to_packet_end() -> Length {
    Length::UntilOffset {
        field: "length".into(),
        unit: 4,
        adjust: 4,
    }
}

/// The RTP header extension (RFC 3550 §5.3.1): a 16-bit profile-defined value
/// (`0xbede` for RFC 8285 one-byte elements), the length in 32-bit words (Auto)
/// and the extension words. Present when the X bit is set (one element).
fn header_extension() -> FieldType {
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                uint("profile", 16, 0xbede),
                u("length", 16).with_auto(AutoCompute::Count {
                    field: "data".into(),
                }),
                FieldDef::new(
                    "data",
                    u32_list(Length::FromField {
                        field: "length".into(),
                        unit: 4,
                        adjust: 0,
                        div: 1,
                    }),
                ),
            ],
        },
        length: count("x"),
    }
}

/// RTP header (RFC 3550 §5.1): version 2, P, X (Auto), CSRC count (Auto), M,
/// payload type, sequence number, timestamp, SSRC, the CSRC list and the header
/// extension.
///
/// Default: payload type 0 (PCMU), sequence number 1, timestamp 0, SSRC
/// 0x12345678, no CSRC and no extension.
#[must_use]
pub fn rtp() -> ProtocolDef {
    protocol! {
        id: "rtp",
        name: "RTP",
        layer: L7,
        fields: {
            version: uint(2) = 2, desc = "rtp-version";
            padding: uint(1) = 0, desc = "rtp-padding";
            x: uint(1), auto = count(header_extension), desc = "rtp-x";
            cc: uint(4), auto = count(csrc), desc = "rtp-cc";
            marker: uint(1) = 0, desc = "rtp-marker";
            payload_type: enum(7) {
                0 => pcmu,
                3 => gsm,
                4 => g723,
                8 => pcma,
                9 => g722,
                13 => cn,
                18 => g729,
                96 => dynamic_96,
                101 => telephone_event,
            } = 0, desc = "rtp-payload-type";
            sequence_number: uint(16) = 1, desc = "rtp-sequence";
            timestamp: uint(32) = 0, desc = "rtp-timestamp";
            ssrc: uint(32) = 0x1234_5678, desc = "rtp-ssrc";
            csrc: ty(u32_list(count("cc"))) = FieldValue::List(Vec::new()), desc = "rtp-csrc";
            header_extension: ty(header_extension()) = FieldValue::List(Vec::new()), desc = "rtp-extension";
        },
        constraints: {
            warning: equals(version, 2);
        },
    }
}

/// RTCP packet types (RFC 3550 §12.1, RFC 4585).
const PACKET_TYPES: &[(u64, &str)] = &[
    (200, "sr"),
    (201, "rr"),
    (202, "sdes"),
    (203, "bye"),
    (204, "app"),
    (205, "rtpfb"),
    (206, "psfb"),
    (207, "xr"),
];

fn packet_type(default: u64) -> FieldDef {
    FieldDef::new(
        "packet_type",
        FieldType::Enum {
            bits: 8,
            variants: PACKET_TYPES
                .iter()
                .map(|(v, n)| EnumVariant {
                    value: *v,
                    name: (*n).into(),
                    description: None,
                })
                .collect(),
        },
    )
    .with_default(FieldValue::UInt(default))
    .with_description("rtcp-packet-type")
}

/// The common header (RFC 3550 §6.4.1): version, padding, the 5-bit count
/// (`count_name`, Auto over `counted` when given), packet type and length in
/// 32-bit words minus one (Auto).
fn common_header(count_name: &str, counted: Option<&str>, pt: u64) -> Vec<FieldDef> {
    let mut count =
        u(count_name, 5).with_description(format!("rtcp-{}", count_name.replace('_', "-")));
    count = match counted {
        Some(f) => count.with_auto(AutoCompute::Count { field: f.into() }),
        None => count.with_default(FieldValue::UInt(0)),
    };
    vec![
        uint("version", 2, 2).with_description("rtcp-version"),
        uint("padding", 1, 0).with_description("rtcp-padding"),
        count,
        packet_type(pt),
        u("length", 16)
            .with_auto(AutoCompute::Length {
                of: wpg_core::LengthOf::Header,
                unit: wpg_core::LengthUnit::Words32,
                adjust: -1,
            })
            .with_description("rtcp-length"),
    ]
}

/// The typed RTCP packets and their packet types.
const TYPED: [(u64, &str); 5] = [
    (200, "rtcp_sr"),
    (201, "rtcp_rr"),
    (202, "rtcp_sdes"),
    (203, "rtcp_bye"),
    (204, "rtcp_app"),
];

/// The next packet of a compound RTCP packet: another RTCP layer when at least a
/// common header follows. The generic [`rtcp`] decodes it (and re-decodes itself
/// as the typed packet); the typed ones are listed for building.
fn compound(d: &mut ProtocolDef) {
    let next = |v: u64, max: Option<u64>| {
        std::iter::once(RTCP)
            .chain(TYPED.iter().map(|(_, p)| *p))
            .map(move |p| {
                let e = match max {
                    Some(hi) => BindingEntry::range(v, hi, p),
                    None => BindingEntry::new(v, p),
                };
                e.with_min_len(4)
            })
    };
    let b = if d.id == RTCP {
        // The generic packet: known types re-decode it, others are followed by
        // the next packet.
        let mut b = Binding::new("packet_type", Vec::new());
        for (v, p) in TYPED {
            b.table.push(BindingEntry::new(v, p).reparsing());
        }
        b.table.extend(next(0, Some(255)));
        b
    } else {
        Binding::new("version", next(2, None).collect())
    };
    d.next = Some(b);
}

fn def(id: &str, name: &str, fields: Vec<FieldDef>) -> ProtocolDef {
    let mut d = ProtocolDef::new(id, name, wpg_core::ProtocolLayer::L7);
    d.fields = fields;
    d.constraints = vec![wpg_core::Constraint::new(
        wpg_core::DiagLevel::Warning,
        wpg_core::ConstraintRule::FieldEquals {
            field: "version".into(),
            value: 2,
        },
    )];
    compound(&mut d);
    d
}

/// Profile-specific extension octets after the reports (to the packet length).
fn extension() -> FieldDef {
    FieldDef::new(
        "extension",
        FieldType::Bytes {
            len: to_packet_end(),
        },
    )
    .with_default(FieldValue::Bytes(Vec::new()))
    .with_description("rtcp-extension")
}

/// Reception report blocks (RFC 3550 §6.4.1), `report_count` of them.
fn report_blocks() -> FieldDef {
    FieldDef::new(
        "report_blocks",
        FieldType::List {
            item: ItemDef::Struct {
                fields: vec![
                    uint("ssrc", 32, 0x8765_4321),
                    uint("fraction_lost", 8, 0),
                    uint("cumulative_lost", 24, 0),
                    uint("highest_sequence", 32, 0),
                    uint("jitter", 32, 0),
                    uint("lsr", 32, 0),
                    uint("dlsr", 32, 0),
                ],
            },
            length: count("report_count"),
        },
    )
    .with_default(FieldValue::List(Vec::new()))
    .with_description("rtcp-report-blocks")
}

fn ssrc() -> FieldDef {
    uint("ssrc", 32, 0x1234_5678).with_description("rtcp-ssrc")
}

/// Generic RTCP packet: the common header and the body as octets. Re-decodes
/// itself as the typed packet for SR / RR / SDES / BYE / APP.
#[must_use]
pub fn rtcp() -> ProtocolDef {
    let mut fields = common_header("count", None, 205);
    fields.push(
        FieldDef::new(
            "body",
            FieldType::Bytes {
                len: to_packet_end(),
            },
        )
        .with_default(FieldValue::Bytes(vec![0x12, 0x34, 0x56, 0x78, 0, 0, 0, 0]))
        .with_description("rtcp-body"),
    );
    def(RTCP, "RTCP", fields)
}

/// RTCP Sender Report (RFC 3550 §6.4.1): sender SSRC, NTP and RTP timestamps,
/// packet / octet counts, report blocks and the profile-specific extension.
#[must_use]
pub fn rtcp_sr() -> ProtocolDef {
    let mut fields = common_header("report_count", Some("report_blocks"), 200);
    fields.extend([
        ssrc(),
        uint("ntp_timestamp_msw", 32, 0xeb00_0000).with_description("rtcp-ntp-timestamp"),
        uint("ntp_timestamp_lsw", 32, 0),
        uint("rtp_timestamp", 32, 0).with_description("rtcp-rtp-timestamp"),
        uint("packet_count", 32, 0).with_description("rtcp-packet-count"),
        uint("octet_count", 32, 0).with_description("rtcp-octet-count"),
        report_blocks(),
        extension(),
    ]);
    def("rtcp_sr", "RTCP SR", fields)
}

/// RTCP Receiver Report (RFC 3550 §6.4.2): reporter SSRC, report blocks and the
/// profile-specific extension.
#[must_use]
pub fn rtcp_rr() -> ProtocolDef {
    let mut fields = common_header("report_count", Some("report_blocks"), 201);
    fields.extend([ssrc(), report_blocks(), extension()]);
    def("rtcp_rr", "RTCP RR", fields)
}

/// SDES items (RFC 3550 §6.5): 8-bit type, 8-bit length (Auto), text; the list
/// ends with a null octet.
fn sdes_items() -> FieldType {
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                FieldDef::new(
                    "type",
                    FieldType::Enum {
                        bits: 8,
                        variants: [
                            (1, "cname"),
                            (2, "name"),
                            (3, "email"),
                            (4, "phone"),
                            (5, "loc"),
                            (6, "tool"),
                            (7, "note"),
                            (8, "priv"),
                        ]
                        .iter()
                        .map(|(v, n)| EnumVariant {
                            value: *v,
                            name: (*n).into(),
                            description: None,
                        })
                        .collect(),
                    },
                )
                .with_default(FieldValue::UInt(1)),
                u("length", 8).with_auto(AutoCompute::Size {
                    field: "text".into(),
                }),
                FieldDef::new(
                    "text",
                    FieldType::Bytes {
                        len: Length::FromField {
                            field: "length".into(),
                            unit: 1,
                            adjust: 0,
                            div: 1,
                        },
                    },
                ),
            ],
        },
        length: Length::Until {
            terminator: vec![0],
        },
    }
}

/// RTCP Source Description (RFC 3550 §6.5): `source_count` chunks, each an SSRC /
/// CSRC and its items, ended by a null octet and padded to 32 bits.
///
/// Default: one chunk for SSRC 0x12345678 with CNAME `user@192.0.2.1`.
#[must_use]
pub fn rtcp_sdes() -> ProtocolDef {
    let mut fields = common_header("source_count", Some("chunks"), 202);
    let chunk = |items: Vec<FieldValue>| {
        FieldValue::Struct(
            [("items".to_owned(), FieldValue::List(items))]
                .into_iter()
                .collect(),
        )
    };
    let cname = FieldValue::Struct(
        [(
            "text".to_owned(),
            FieldValue::Bytes(b"user@192.0.2.1".to_vec()),
        )]
        .into_iter()
        .collect(),
    );
    fields.push(
        FieldDef::new(
            "chunks",
            FieldType::List {
                item: ItemDef::Struct {
                    fields: vec![
                        uint("ssrc", 32, 0x1234_5678),
                        FieldDef::new("items", sdes_items()),
                        FieldDef::new("padding", FieldType::Padding { align: 4 }),
                    ],
                },
                length: count("source_count"),
            },
        )
        .with_default(FieldValue::List(vec![chunk(vec![cname])]))
        .with_description("rtcp-chunks"),
    );
    def("rtcp_sdes", "RTCP SDES", fields)
}

/// RTCP Goodbye (RFC 3550 §6.6): `source_count` SSRC / CSRC identifiers and the
/// optional reason (8-bit length (Auto), text, padding to 32 bits).
#[must_use]
pub fn rtcp_bye() -> ProtocolDef {
    let mut fields = common_header("source_count", Some("ssrcs"), 203);
    fields.extend([
        FieldDef::new("ssrcs", u32_list(count("source_count")))
            .with_default(FieldValue::List(vec![FieldValue::UInt(0x1234_5678)]))
            .with_description("rtcp-ssrcs"),
        FieldDef::new(
            "reason",
            FieldType::List {
                item: ItemDef::Struct {
                    fields: vec![
                        u("length", 8).with_auto(AutoCompute::Size {
                            field: "text".into(),
                        }),
                        FieldDef::new(
                            "text",
                            FieldType::Bytes {
                                len: Length::FromField {
                                    field: "length".into(),
                                    unit: 1,
                                    adjust: 0,
                                    div: 1,
                                },
                            },
                        ),
                        FieldDef::new("padding", FieldType::Padding { align: 4 }),
                    ],
                },
                length: to_packet_end(),
            },
        )
        .with_default(FieldValue::List(Vec::new()))
        .with_description("rtcp-reason"),
    ]);
    def("rtcp_bye", "RTCP BYE", fields)
}

/// RTCP Application-Defined packet (RFC 3550 §6.7): subtype, SSRC, a 4-character
/// name and the application data.
#[must_use]
pub fn rtcp_app() -> ProtocolDef {
    let mut fields = common_header("subtype", None, 204);
    fields.extend([
        ssrc(),
        FieldDef::new(
            "name",
            FieldType::Bytes {
                len: Length::Fixed { octets: 4 },
            },
        )
        .with_default(FieldValue::Bytes(b"WPG0".to_vec()))
        .with_description("rtcp-app-name"),
        FieldDef::new(
            "data",
            FieldType::Bytes {
                len: to_packet_end(),
            },
        )
        .with_default(FieldValue::Bytes(Vec::new()))
        .with_description("rtcp-app-data"),
    ]);
    def("rtcp_app", "RTCP APP", fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtp_and_rtcp_are_valid() {
        for d in [
            rtp(),
            rtcp(),
            rtcp_sr(),
            rtcp_rr(),
            rtcp_sdes(),
            rtcp_bye(),
            rtcp_app(),
        ] {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
    }
}
