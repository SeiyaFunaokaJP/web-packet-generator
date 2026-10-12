//! OSI network-layer encapsulation (TODO §2.1 P2, M7): the common entry point of
//! CLNP / ES-IS / IS-IS.
//!
//! OSI network-layer PDUs travel over 802.3 + LLC with DSAP / SSAP `0xfe` (and
//! over PPP as protocol `0x0023`). There is no protocol selector in front of the
//! PDU: its own first octet, the network layer protocol identifier (NLPID, ISO/IEC
//! TR 9577), tells CLNP (`0x81`), ES-IS (`0x82`) and IS-IS (`0x83`) apart. The
//! layers below therefore demultiplex by peeking that octet ([`osi_peek`]).
//!
//! [`osi`] is the generic PDU decoded for NLPIDs without a dedicated definition:
//! the NLPID followed by the rest of the PDU as opaque octets. Dedicated PDU
//! definitions are added to the [`osi_peek`] table as they land (IS-IS:
//! [`crate::isis`]); their address fields use the `nsap` field type (length
//! indicator + NSAP octets).

use wpg_core::{protocol, BindingEntry, Peek, ProtocolDef};

/// Id of the generic OSI network-layer PDU ([`osi`]).
pub const OSI: &str = "osi";

/// Payload demux on the NLPID octet when the binding selector is `on` (LLC DSAP
/// `0xfe`, PPP protocol `0x0023`). PDUs without a dedicated definition decode as
/// [`osi`].
///
/// IS-IS (`0x83`) decodes as the generic [`crate::isis::isis`], which re-decodes
/// itself by PDU type; the dedicated IS-IS PDUs follow it in the table (the first
/// entry wins when parsing) so that the builder announces them too.
#[must_use]
pub fn osi_peek(on: u64) -> Peek {
    let isis = std::iter::once(crate::isis::ISIS).chain(crate::isis::PDUS.iter().map(|p| p.1));
    let table = isis.map(|p| BindingEntry::new(0x83, p)).collect();
    let mut p = Peek::new(on, 8, table);
    p.default = Some(OSI.into());
    p
}

/// Generic OSI network-layer PDU: NLPID (ISO/IEC TR 9577) and the rest of the PDU
/// as opaque octets.
#[must_use]
pub fn osi() -> ProtocolDef {
    protocol! {
        id: "osi",
        name: "OSI network PDU",
        layer: L3,
        fields: {
            nlpid: enum(8) {
                0x00 => null,
                0x81 => clnp,
                0x82 => esis,
                0x83 => isis,
                0x8e => ipv6,
                0xcc => ipv4,
            } = 0x83, desc = "osi-nlpid";
            data: bytes(to_end) = [], desc = "osi-data";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osi_is_valid() {
        osi().validate().unwrap();
        let p = osi_peek(0xfe);
        assert_eq!(p.on, 0xfe);
        assert_eq!(p.bits, 8);
        assert_eq!(p.default.as_deref(), Some(OSI));
    }
}
