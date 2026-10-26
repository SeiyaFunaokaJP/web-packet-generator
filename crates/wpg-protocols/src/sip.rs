//! SIP (RFC 3261) messages (spec §2.1 P2, M7).
//!
//! SIP is text: one [`sip`] layer holds the start line (request line or status
//! line), the header lines and the message body. Lines are CRLF-terminated text
//! fields ([`wpg_core::Length::Until`]): the builder appends each CRLF and the
//! empty line that ends the headers, the parser splits at them. The body is the
//! rest of the layer (an SDP offer / answer, ...); its lines are not split.
//!
//! Reached through UDP and TCP 5060. `Content-Length` is an ordinary header line
//! (Manual): the engine has no rule that writes a decimal length into text, so
//! keep it in step with the body (the sample templates do).

use wpg_core::{protocol, FieldType, FieldValue, ItemDef, Length, ProtocolDef};

/// Id of the SIP message ([`sip`]).
pub const SIP: &str = "sip";

/// UDP / TCP port of SIP (RFC 3261 §19.1.2).
pub const SIP_PORT: u64 = 5060;

/// A CRLF-terminated text line (the value is the line without the CRLF).
#[must_use]
pub fn line() -> FieldType {
    FieldType::Bytes {
        len: Length::Until {
            terminator: b"\r\n".to_vec(),
        },
    }
}

/// Header lines ended by an empty line.
fn header_lines() -> FieldType {
    FieldType::List {
        item: ItemDef::Field {
            ty: Box::new(line()),
        },
        length: Length::Until {
            terminator: b"\r\n".to_vec(),
        },
    }
}

fn text(s: &str) -> FieldValue {
    FieldValue::Bytes(s.as_bytes().to_vec())
}

/// SIP message (RFC 3261 §7): the start line, the header lines (each `Name:
/// value`, the empty line after them is added automatically) and the body.
///
/// Default: an OPTIONS request from `alice@example.com` to `bob@example.com` over
/// UDP without a body.
#[must_use]
pub fn sip() -> ProtocolDef {
    let headers = [
        "Via: SIP/2.0/UDP 192.0.2.1:5060;branch=z9hG4bK776asdhds",
        "Max-Forwards: 70",
        "To: <sip:bob@example.com>",
        "From: <sip:alice@example.com>;tag=1928301774",
        "Call-ID: a84b4c76e66710@192.0.2.1",
        "CSeq: 63104 OPTIONS",
        "Contact: <sip:alice@192.0.2.1>",
        "Accept: application/sdp",
        "Content-Length: 0",
    ];
    protocol! {
        id: "sip",
        name: "SIP",
        layer: L7,
        fields: {
            start_line: ty(line()) = text("OPTIONS sip:bob@example.com SIP/2.0"), desc = "sip-start-line";
            headers: ty(header_lines()) = FieldValue::List(headers.iter().map(|h| text(h)).collect()), desc = "sip-headers";
            body: bytes(to_end) = [], desc = "sip-body";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sip_is_valid() {
        let d = sip();
        d.validate().unwrap();
        assert_eq!(d.id, SIP);
    }
}
