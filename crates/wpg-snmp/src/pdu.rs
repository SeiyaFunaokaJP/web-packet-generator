//! PDUs (RFC 3416, RFC 1157 Trap) and community-based messages (v1 / v2c).

use std::fmt;

use crate::ber::{self, tag, BerError, Reader};
use crate::oid::{known, Oid};
use crate::value::{decode_varbinds, encode_varbinds, Value, VarBind};

/// PDU types (context-specific constructed tags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PduType {
    /// `GetRequest-PDU` (`0xa0`).
    Get,
    /// `GetNextRequest-PDU` (`0xa1`).
    GetNext,
    /// `Response-PDU` (`0xa2`).
    Response,
    /// `SetRequest-PDU` (`0xa3`).
    Set,
    /// SNMPv1 `Trap-PDU` (`0xa4`).
    TrapV1,
    /// `GetBulkRequest-PDU` (`0xa5`).
    GetBulk,
    /// `InformRequest-PDU` (`0xa6`).
    Inform,
    /// `SNMPv2-Trap-PDU` (`0xa7`).
    TrapV2,
    /// `Report-PDU` (`0xa8`).
    Report,
}

impl PduType {
    /// The tag octet.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            PduType::Get => 0xa0,
            PduType::GetNext => 0xa1,
            PduType::Response => 0xa2,
            PduType::Set => 0xa3,
            PduType::TrapV1 => 0xa4,
            PduType::GetBulk => 0xa5,
            PduType::Inform => 0xa6,
            PduType::TrapV2 => 0xa7,
            PduType::Report => 0xa8,
        }
    }

    /// From the tag octet.
    #[must_use]
    pub const fn from_tag(t: u8) -> Option<Self> {
        Some(match t {
            0xa0 => PduType::Get,
            0xa1 => PduType::GetNext,
            0xa2 => PduType::Response,
            0xa3 => PduType::Set,
            0xa4 => PduType::TrapV1,
            0xa5 => PduType::GetBulk,
            0xa6 => PduType::Inform,
            0xa7 => PduType::TrapV2,
            0xa8 => PduType::Report,
            _ => return None,
        })
    }

    /// Stable lower-case name (`get`, `trap_v2`, ...).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            PduType::Get => "get",
            PduType::GetNext => "getnext",
            PduType::Response => "response",
            PduType::Set => "set",
            PduType::TrapV1 => "trap_v1",
            PduType::GetBulk => "getbulk",
            PduType::Inform => "inform",
            PduType::TrapV2 => "trap_v2",
            PduType::Report => "report",
        }
    }

    /// Whether the PDU belongs to the confirmed class (expects a response and
    /// is sent with the v3 `reportable` flag).
    #[must_use]
    pub const fn is_confirmed(self) -> bool {
        matches!(
            self,
            PduType::Get | PduType::GetNext | PduType::GetBulk | PduType::Set | PduType::Inform
        )
    }
}

impl fmt::Display for PduType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `error-status` values (RFC 3416 §3).
pub const ERROR_STATUS_NAMES: [&str; 19] = [
    "noError",
    "tooBig",
    "noSuchName",
    "badValue",
    "readOnly",
    "genErr",
    "noAccess",
    "wrongType",
    "wrongLength",
    "wrongEncoding",
    "wrongValue",
    "noCreation",
    "inconsistentValue",
    "resourceUnavailable",
    "commitFailed",
    "undoFailed",
    "authorizationError",
    "notWritable",
    "inconsistentName",
];

/// Name of an `error-status` (`noSuchName(2)` style without the number).
#[must_use]
pub fn error_status_name(s: i32) -> String {
    usize::try_from(s)
        .ok()
        .and_then(|i| ERROR_STATUS_NAMES.get(i))
        .map_or_else(|| format!("error-status {s}"), |n| (*n).to_owned())
}

/// Every PDU but the v1 Trap. For GetBulk, `error_status` is `non-repeaters`
/// and `error_index` is `max-repetitions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pdu {
    /// Type.
    pub ty: PduType,
    /// `request-id`.
    pub request_id: i32,
    /// `error-status` (`non-repeaters` of GetBulk).
    pub error_status: i32,
    /// `error-index` (`max-repetitions` of GetBulk).
    pub error_index: i32,
    /// Bindings.
    pub varbinds: Vec<VarBind>,
}

impl Pdu {
    /// A PDU with zero error fields.
    #[must_use]
    pub fn new(ty: PduType, request_id: i32, varbinds: Vec<VarBind>) -> Self {
        Self {
            ty,
            request_id,
            error_status: 0,
            error_index: 0,
            varbinds,
        }
    }

    /// A GetBulk request.
    #[must_use]
    pub fn bulk(request_id: i32, non_repeaters: i32, max_repetitions: i32, oids: &[Oid]) -> Self {
        Self {
            ty: PduType::GetBulk,
            request_id,
            error_status: non_repeaters,
            error_index: max_repetitions,
            varbinds: oids.iter().cloned().map(VarBind::null).collect(),
        }
    }

    /// An SNMPv2-Trap / Inform with the mandatory `sysUpTime.0` and
    /// `snmpTrapOID.0` bindings in front of `varbinds`.
    #[must_use]
    pub fn notification(
        ty: PduType,
        request_id: i32,
        uptime: u32,
        trap_oid: &Oid,
        varbinds: &[VarBind],
    ) -> Self {
        let mut v = Vec::with_capacity(varbinds.len() + 2);
        v.push(VarBind::new(
            known::sys_uptime_0(),
            Value::TimeTicks(uptime),
        ));
        v.push(VarBind::new(
            known::snmp_trap_oid_0(),
            Value::Oid(trap_oid.clone()),
        ));
        v.extend_from_slice(varbinds);
        Self::new(ty, request_id, v)
    }

    /// BER encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        ber::seq(
            self.ty.tag(),
            &[
                &ber::integer(i64::from(self.request_id)),
                &ber::integer(i64::from(self.error_status)),
                &ber::integer(i64::from(self.error_index)),
                &encode_varbinds(&self.varbinds),
            ],
        )
    }
}

/// SNMPv1 `Trap-PDU` (RFC 1157).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapV1 {
    /// `enterprise` (`sysObjectID` of the agent).
    pub enterprise: Oid,
    /// `agent-addr`.
    pub agent_addr: [u8; 4],
    /// `generic-trap` (0 coldStart ... 6 enterpriseSpecific).
    pub generic: i32,
    /// `specific-trap`.
    pub specific: i32,
    /// `time-stamp` (TimeTicks).
    pub timestamp: u32,
    /// Bindings.
    pub varbinds: Vec<VarBind>,
}

impl TrapV1 {
    /// BER encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        ber::seq(
            PduType::TrapV1.tag(),
            &[
                &ber::oid(&self.enterprise),
                &ber::tlv(tag::IP_ADDRESS, &self.agent_addr),
                &ber::integer(i64::from(self.generic)),
                &ber::integer(i64::from(self.specific)),
                &ber::tlv(
                    tag::TIMETICKS,
                    &ber::uint_content(u64::from(self.timestamp)),
                ),
                &encode_varbinds(&self.varbinds),
            ],
        )
    }

    /// The equivalent SNMPv2 `snmpTrapOID` (RFC 3584 §3.1): `snmpTraps.(g+1)`
    /// for a generic trap, else `enterprise.0.specific`.
    #[must_use]
    pub fn trap_oid(&self) -> Oid {
        match u32::try_from(self.generic) {
            Ok(g @ 0..=5) => known::snmp_traps().child(&[g + 1]),
            _ => self
                .enterprise
                .child(&[0, u32::try_from(self.specific).unwrap_or(0)]),
        }
    }
}

/// Any PDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyPdu {
    /// Every PDU but the v1 Trap.
    Pdu(Pdu),
    /// SNMPv1 Trap.
    TrapV1(TrapV1),
}

impl AnyPdu {
    /// The type.
    #[must_use]
    pub fn ty(&self) -> PduType {
        match self {
            AnyPdu::Pdu(p) => p.ty,
            AnyPdu::TrapV1(_) => PduType::TrapV1,
        }
    }

    /// BER encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            AnyPdu::Pdu(p) => p.encode(),
            AnyPdu::TrapV1(t) => t.encode(),
        }
    }

    /// The bindings.
    #[must_use]
    pub fn varbinds(&self) -> &[VarBind] {
        match self {
            AnyPdu::Pdu(p) => &p.varbinds,
            AnyPdu::TrapV1(t) => &t.varbinds,
        }
    }

    /// Decodes a PDU TLV.
    ///
    /// # Errors
    /// Unknown PDU tag or malformed content.
    pub fn decode(r: &mut Reader<'_>) -> Result<AnyPdu, BerError> {
        let t = r.read()?;
        let ty = PduType::from_tag(t.tag).ok_or_else(|| {
            BerError::Invalid(format!(
                "unknown PDU tag 0x{:02x} at offset {}",
                t.tag, t.offset
            ))
        })?;
        let mut pr = Reader::inner(&t);
        if ty == PduType::TrapV1 {
            let enterprise = pr.oid()?;
            let a = pr.expect(tag::IP_ADDRESS)?;
            let agent_addr: [u8; 4] = a
                .content
                .try_into()
                .map_err(|_| BerError::Invalid("agent-addr is not 4 octets".into()))?;
            let generic = pr.int32()?;
            let specific = pr.int32()?;
            let ts = pr.expect(tag::TIMETICKS)?;
            #[allow(clippy::cast_possible_truncation)] // low 32 bits
            let timestamp = ber::decode_uint(ts.content, ts.offset)? as u32;
            let varbinds = decode_varbinds(&mut pr)?;
            return Ok(AnyPdu::TrapV1(TrapV1 {
                enterprise,
                agent_addr,
                generic,
                specific,
                timestamp,
                varbinds,
            }));
        }
        let request_id = pr.int32()?;
        let error_status = pr.int32()?;
        let error_index = pr.int32()?;
        let varbinds = decode_varbinds(&mut pr)?;
        Ok(AnyPdu::Pdu(Pdu {
            ty,
            request_id,
            error_status,
            error_index,
            varbinds,
        }))
    }
}

/// Message versions (`msgVersion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    /// SNMPv1 (0).
    V1,
    /// SNMPv2c (1).
    V2c,
    /// SNMPv3 (3).
    V3,
}

impl Version {
    /// The `msgVersion` number.
    #[must_use]
    pub const fn number(self) -> i64 {
        match self {
            Version::V1 => 0,
            Version::V2c => 1,
            Version::V3 => 3,
        }
    }

    /// From the `msgVersion` number.
    #[must_use]
    pub const fn from_number(n: i64) -> Option<Self> {
        match n {
            0 => Some(Version::V1),
            1 => Some(Version::V2c),
            3 => Some(Version::V3),
            _ => None,
        }
    }

    /// `v1`, `v2c`, `v3`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
            Version::V2c => "v2c",
            Version::V3 => "v3",
        }
    }
}

/// A community-based message (SNMPv1 / SNMPv2c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityMessage {
    /// `V1` or `V2c`.
    pub version: Version,
    /// Community string.
    pub community: Vec<u8>,
    /// The PDU.
    pub pdu: AnyPdu,
}

impl CommunityMessage {
    /// BER encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        ber::seq(
            tag::SEQUENCE,
            &[
                &ber::integer(self.version.number()),
                &ber::octets(&self.community),
                &self.pdu.encode(),
            ],
        )
    }
}

/// Reads the `msgVersion` of a message without decoding the rest.
///
/// # Errors
/// Not a SEQUENCE starting with an INTEGER.
pub fn peek_version(data: &[u8]) -> Result<i64, BerError> {
    let mut r = Reader::new(data);
    let outer = r.expect(tag::SEQUENCE)?;
    Reader::inner(&outer).integer()
}

/// Decodes a v1 / v2c message.
///
/// # Errors
/// Malformed BER or another version.
pub fn decode_community(data: &[u8]) -> Result<CommunityMessage, BerError> {
    let mut r = Reader::new(data);
    let outer = r.expect(tag::SEQUENCE)?;
    let mut mr = Reader::inner(&outer);
    let v = mr.integer()?;
    let Some(version @ (Version::V1 | Version::V2c)) = Version::from_number(v) else {
        return Err(BerError::Invalid(format!(
            "not a v1/v2c message (version {v})"
        )));
    };
    let community = mr.octets()?.to_vec();
    let pdu = AnyPdu::decode(&mut mr)?;
    Ok(CommunityMessage {
        version,
        community,
        pdu,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(s: &str) -> Oid {
        s.parse().unwrap()
    }

    #[test]
    fn get_request_bytes() {
        // snmpget -v2c -c public host sysDescr.0 with request-id 1, as seen on
        // the wire (and as the wpg `snmp` template builds it).
        let m = CommunityMessage {
            version: Version::V2c,
            community: b"public".to_vec(),
            pdu: AnyPdu::Pdu(Pdu::new(
                PduType::Get,
                1,
                vec![VarBind::null(oid("1.3.6.1.2.1.1.1.0"))],
            )),
        };
        let enc = m.encode();
        let expected: Vec<u8> = vec![
            0x30, 0x26, 0x02, 0x01, 0x01, 0x04, 0x06, b'p', b'u', b'b', b'l', b'i', b'c', 0xa0,
            0x19, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00, 0x02, 0x01, 0x00, 0x30, 0x0e, 0x30, 0x0c,
            0x06, 0x08, 0x2b, 0x06, 0x01, 0x02, 0x01, 0x01, 0x01, 0x00, 0x05, 0x00,
        ];
        assert_eq!(enc, expected);
        assert_eq!(decode_community(&enc).unwrap(), m);
        assert_eq!(peek_version(&enc).unwrap(), 1);
    }

    #[test]
    fn all_pdu_types_round_trip() {
        for ty in [
            PduType::Get,
            PduType::GetNext,
            PduType::Response,
            PduType::Set,
            PduType::GetBulk,
            PduType::Inform,
            PduType::TrapV2,
            PduType::Report,
        ] {
            let p = Pdu {
                ty,
                request_id: -12345,
                error_status: 2,
                error_index: 1,
                varbinds: vec![VarBind::new(
                    oid("1.3.6.1.2.1.2.2.1.10.1"),
                    Value::Counter32(99),
                )],
            };
            let m = CommunityMessage {
                version: Version::V2c,
                community: b"c".to_vec(),
                pdu: AnyPdu::Pdu(p),
            };
            assert_eq!(decode_community(&m.encode()).unwrap(), m);
        }
        let t = TrapV1 {
            enterprise: oid("1.3.6.1.4.1.8072.3.2.10"),
            agent_addr: [192, 0, 2, 7],
            generic: 6,
            specific: 17,
            timestamp: 4242,
            varbinds: vec![VarBind::new(
                oid("1.3.6.1.2.1.1.5.0"),
                Value::OctetString(b"rtr".to_vec()),
            )],
        };
        assert_eq!(t.trap_oid(), oid("1.3.6.1.4.1.8072.3.2.10.0.17"));
        let m = CommunityMessage {
            version: Version::V1,
            community: b"public".to_vec(),
            pdu: AnyPdu::TrapV1(t),
        };
        assert_eq!(decode_community(&m.encode()).unwrap(), m);
        let cold = TrapV1 {
            generic: 0,
            ..match m.pdu {
                AnyPdu::TrapV1(t) => t,
                AnyPdu::Pdu(_) => unreachable!(),
            }
        };
        assert_eq!(cold.trap_oid(), oid("1.3.6.1.6.3.1.1.5.1"));
    }

    #[test]
    fn notification_varbinds() {
        let p = Pdu::notification(PduType::TrapV2, 7, 100, &oid("1.3.6.1.6.3.1.1.5.3"), &[]);
        assert_eq!(p.varbinds[0].oid, known::sys_uptime_0());
        assert_eq!(p.varbinds[1].value, Value::Oid(oid("1.3.6.1.6.3.1.1.5.3")));
        assert_eq!(error_status_name(2), "noSuchName");
        assert_eq!(error_status_name(99), "error-status 99");
    }
}
