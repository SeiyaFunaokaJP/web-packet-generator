//! Encapsulation / tunnelling protocols (TODO §2.1 P1): VXLAN, GRE, NVGRE, MPLS,
//! MPLS-over-UDP, `L2TPv2` / v3, GTP-U and Geneve. (IP-in-IP needs no new protocol:
//! IPv4 protocol 4 → [`crate::l3::ipv4`] and 41 → [`crate::ipv6::ipv6`] are already
//! bound.)
//!
//! ## Engine limitations exercised here
//!
//! * **GRE** checksum / key / sequence fields are present only when the matching
//!   flag bit is set. The engine has no presence-conditional fields, so [`gre`]
//!   models just the 4-octet base (flags + protocol type); the common keyed case
//!   (NVGRE) has its own [`nvgre`] definition. A `present_if` predicate on
//!   [`wpg_core::FieldDef`] driven by a flag bit would remove the need.
//! * **MPLS** repeats a 4-octet label entry until the S (bottom-of-stack) bit is
//!   set. [`mpls`] is a single label entry chained through its `s` binding
//!   (`s = 0` → another MPLS label); below the bottom label the payload is
//!   demultiplexed by peeking its first nibble ([`wpg_core::Peek`]: 4 → IPv4,
//!   6 → IPv6). Pseudowire payloads (control word / Ethernet) have no reliable
//!   marker and parse as `raw`; stack `ethernet2` explicitly when building. `s` is
//!   Auto from the next layer.
//! * **GTP-U** optional sequence / N-PDU / next-extension fields (present per the
//!   E/S/PN flags) are not modelled: [`gtpu`] is the mandatory 8-octet header.
//!   A G-PDU's payload is demultiplexed by its first nibble like MPLS.
//! * **Geneve** options have a 3-bit-flags + 5-bit-length sub-octet header, so they
//!   are modelled as an opaque `options` region sized by `opt_len` rather than a
//!   typed TLV.
//! * **`L2TPv2` / v3** header fields are heavily presence-conditional (per the flag
//!   bits and control/data type); the bodies are modelled as opaque octets.

use wpg_core::{protocol, BindingEntry, Peek, ProtocolDef};

// --------------------------------------------------------------------------
// VXLAN (RFC 7348), UDP 4789
// --------------------------------------------------------------------------

/// VXLAN (RFC 7348, UDP 4789): flags (I bit = VNI valid), 24-bit VNI and reserved
/// fields; the payload is an inner Ethernet frame.
#[must_use]
pub fn vxlan() -> ProtocolDef {
    let mut d = protocol! {
        id: "vxlan",
        name: "VXLAN",
        layer: L2,
        fields: {
            flags: flags(8) { 3 => vni_present } = 0x08, desc = "vxlan-flags";
            reserved1: uint(24) = 0;
            vni: uint(24) = 0, desc = "vxlan-vni";
            reserved2: uint(8) = 0;
        },
        next: flags {},
    };
    if let Some(b) = d.next.as_mut() {
        b.default = Some("ethernet2".into());
    }
    d
}

// --------------------------------------------------------------------------
// GRE (RFC 2784), IP protocol 47
// --------------------------------------------------------------------------

/// GRE (RFC 2784, IP protocol 47): the 4-octet base header (flags + version and a
/// `protocol_type` EtherType). Checksum / key / sequence extensions are not
/// modelled (see the module docs); use [`nvgre`] for the keyed case.
#[must_use]
pub fn gre() -> ProtocolDef {
    protocol! {
        id: "gre",
        name: "GRE",
        layer: L3,
        fields: {
            flags: flags(16) {
                15 => checksum_present,
                13 => key_present,
                12 => sequence_present,
            } = 0, desc = "gre-flags";
            protocol_type: enum(16) {
                0x0800 => ipv4,
                0x86dd => ipv6,
                0x6558 => transparent_ethernet,
                0x8847 => mpls,
            } = 0x0800, auto = next_protocol, desc = "gre-protocol-type";
        },
        next: protocol_type {
            0x0800 => "ipv4",
            0x86dd => "ipv6",
            0x6558 => "ethernet2",
            0x8847 => "mpls",
        },
    }
}

/// NVGRE (RFC 7637, IP protocol 47): GRE with the Key bit set, carrying a 24-bit
/// Virtual Subnet ID and an 8-bit `FlowID`; the payload is an inner Ethernet frame
/// (protocol type `0x6558`, Transparent Ethernet Bridging).
#[must_use]
pub fn nvgre() -> ProtocolDef {
    protocol! {
        id: "nvgre",
        name: "NVGRE",
        layer: L3,
        fields: {
            flags: flags(16) {
                15 => checksum_present,
                13 => key_present,
                12 => sequence_present,
            } = 0x2000, desc = "gre-flags";
            protocol_type: enum(16) { 0x6558 => transparent_ethernet } = 0x6558, desc = "gre-protocol-type";
            vsid: uint(24) = 0, desc = "nvgre-vsid";
            flow_id: uint(8) = 0, desc = "nvgre-flowid";
        },
        next: protocol_type { 0x6558 => "ethernet2" },
    }
}

// --------------------------------------------------------------------------
// MPLS (RFC 3032), EtherType 0x8847 / 0x8848
// --------------------------------------------------------------------------

/// One MPLS label entry (RFC 3032): 20-bit label, 3-bit traffic class, 1-bit
/// bottom-of-stack `s` (Auto: 0 when another label follows) and 8-bit TTL. The
/// reserved labels 0-15 (RFC 3032 / 6790 / 5586 / 3429 / 7274) have names, e.g.
/// `entropy_label_indicator` (7) before an entropy label; SR-MPLS SIDs are plain
/// labels (SRGB base + SID index, see the SR-MPLS guide). Below
/// the bottom label the payload's first nibble selects IPv4 (4) or IPv6 (6);
/// other payloads (pseudowires) parse as `raw`.
#[must_use]
pub fn mpls() -> ProtocolDef {
    let mut d = protocol! {
        id: "mpls",
        name: "MPLS",
        layer: L2,
        fields: {
            label: enum(20) {
                0 => ipv4_explicit_null,
                1 => router_alert,
                2 => ipv6_explicit_null,
                3 => implicit_null,
                7 => entropy_label_indicator,
                13 => gal,
                14 => oam_alert,
                15 => extension,
            } = 16, desc = "mpls-label";
            tc: uint(3) = 0, desc = "mpls-tc";
            s: uint(1) = 1, auto = next_protocol, desc = "mpls-s";
            ttl: uint(8) = 64, desc = "mpls-ttl";
        },
        next: s { 0 => "mpls" },
    };
    if let Some(b) = d.next.as_mut() {
        b.peek = Some(ip_version_peek(1));
    }
    d
}

/// Payload demux on the IP version nibble (4 → IPv4, 6 → IPv6) when the binding
/// selector is `on`.
#[must_use]
pub fn ip_version_peek(on: u64) -> Peek {
    Peek::new(
        on,
        4,
        vec![BindingEntry::new(4, "ipv4"), BindingEntry::new(6, "ipv6")],
    )
}

// --------------------------------------------------------------------------
// GTP-U (3GPP TS 29.281), UDP 2152
// --------------------------------------------------------------------------

/// GTP-U (3GPP TS 29.281, UDP 2152): the mandatory 8-octet header (version, flags,
/// message type, `length` (Auto over the payload) and TEID). Optional sequence /
/// N-PDU / next-extension fields are not modelled (see the module docs). A G-PDU
/// (type 255) carries IPv4 or IPv6, told apart by the first payload nibble.
#[must_use]
pub fn gtpu() -> ProtocolDef {
    let mut d = protocol! {
        id: "gtpu",
        name: "GTP-U",
        layer: L4,
        fields: {
            version: uint(3) = 1, desc = "gtpu-version";
            protocol_type: uint(1) = 1, desc = "gtpu-pt";
            spare: uint(1) = 0;
            e_flag: uint(1) = 0, desc = "gtpu-e";
            s_flag: uint(1) = 0, desc = "gtpu-s";
            pn_flag: uint(1) = 0, desc = "gtpu-pn";
            message_type: enum(8) {
                1 => echo_request,
                2 => echo_response,
                26 => error_indication,
                31 => supported_extension_headers,
                254 => end_marker,
                255 => g_pdu,
            } = 255, desc = "gtpu-message-type";
            length: uint(16), auto = length(payload), desc = "gtpu-length";
            teid: uint(32) = 0, desc = "gtpu-teid";
        },
        next: message_type {},
    };
    if let Some(b) = d.next.as_mut() {
        b.peek = Some(ip_version_peek(255));
    }
    d
}

// --------------------------------------------------------------------------
// Geneve (RFC 8926), UDP 6081
// --------------------------------------------------------------------------

/// Geneve (RFC 8926, UDP 6081): version, `opt_len` (option region length in 4-octet
/// words, Manual), O/C flags, `protocol_type` (EtherType of the payload), 24-bit
/// VNI and the opaque `options` region. The payload is demultiplexed by
/// `protocol_type` (Ethernet / IPv4 / IPv6).
#[must_use]
pub fn geneve() -> ProtocolDef {
    protocol! {
        id: "geneve",
        name: "Geneve",
        layer: L2,
        fields: {
            version: uint(2) = 0, desc = "geneve-version";
            opt_len: uint(6) = 0, desc = "geneve-opt-len";
            o_flag: uint(1) = 0, desc = "geneve-o";
            c_flag: uint(1) = 0, desc = "geneve-c";
            reserved_flags: uint(6) = 0;
            protocol_type: enum(16) {
                0x6558 => transparent_ethernet,
                0x0800 => ipv4,
                0x86dd => ipv6,
            } = 0x6558, auto = next_protocol, desc = "geneve-protocol-type";
            vni: uint(24) = 0, desc = "geneve-vni";
            reserved: uint(8) = 0;
            options: bytes(from = opt_len, unit = 4) = [], desc = "geneve-options";
        },
        next: protocol_type {
            0x6558 => "ethernet2",
            0x0800 => "ipv4",
            0x86dd => "ipv6",
        },
    }
}

// --------------------------------------------------------------------------
// L2TP (RFC 2661 / RFC 3931)
// --------------------------------------------------------------------------

/// `L2TPv2` (RFC 2661, UDP 1701): the 16-bit flags/version word and an opaque `body`.
/// The presence-conditional length / tunnel-id / session-id / Ns / Nr / offset
/// fields are not modelled (see the module docs).
#[must_use]
pub fn l2tpv2() -> ProtocolDef {
    protocol! {
        id: "l2tpv2",
        name: "L2TPv2",
        layer: L4,
        fields: {
            flags: flags(16) {
                15 => type_control,
                14 => length_present,
                9 => sequence_present,
                7 => offset_present,
                3 => priority,
            } = 0x0002, desc = "l2tp-flags";
            body: bytes(to_end) = [], desc = "l2tp-body";
        },
    }
}

/// `L2TPv3` (RFC 3931, IP protocol 115 or UDP 1701): the 32-bit Session ID and an
/// opaque `body` (optional cookie + L2-specific sublayer + payload).
#[must_use]
pub fn l2tpv3() -> ProtocolDef {
    protocol! {
        id: "l2tpv3",
        name: "L2TPv3",
        layer: L4,
        fields: {
            session_id: uint(32) = 0, desc = "l2tpv3-session-id";
            body: bytes(to_end) = [], desc = "l2tp-body";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defs_are_valid() {
        for d in [
            vxlan(),
            gre(),
            nvgre(),
            mpls(),
            gtpu(),
            geneve(),
            l2tpv2(),
            l2tpv3(),
        ] {
            d.validate()
                .unwrap_or_else(|e| panic!("{} invalid: {e}", d.id));
        }
        assert_eq!(vxlan().fixed_header_bits(), Some(64));
        assert_eq!(gre().fixed_header_bits(), Some(32));
        assert_eq!(nvgre().fixed_header_bits(), Some(64));
        assert_eq!(mpls().fixed_header_bits(), Some(32));
        // GTP-U mandatory header is 8 octets.
        let gtpu_bits: u32 = gtpu().fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
        assert_eq!(gtpu_bits, 64);
    }
}
