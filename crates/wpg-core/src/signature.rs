//! Transmit signatures for receive-side verification (TODO §2.3, M9).
//!
//! A signature is a fixed 24-octet trailer written over the **last** octets of a
//! frame just before it is sent:
//!
//! | offset | size | field                                            |
//! |-------:|-----:|--------------------------------------------------|
//! | 0      | 4    | magic `57 50 47 01` (`"WPG"` + version 1)        |
//! | 4      | 4    | stream id (big endian)                           |
//! | 8      | 8    | sequence number within the stream (big endian)   |
//! | 16     | 8    | TX time, nanoseconds since the Unix epoch (BE)   |
//!
//! [`stamp`] only overwrites payload: it walks Ethernet / 802.1Q / 802.1ad / IPv4 /
//! IPv6 (extension headers) / UDP / TCP / ICMP / ICMPv6 and refuses frames whose
//! payload (or Ethernet padding) is shorter than [`LEN`]. When the trailer lies
//! inside a UDP / TCP / ICMP / ICMPv6 payload the transport checksum is updated
//! incrementally (RFC 1624), so stamped frames stay valid; an IPv4 UDP checksum of
//! zero (disabled) is left alone. Other checksummed payloads (e.g. OSPF, GRE with a
//! checksum, non-first IP fragments) are not fixed up.
//!
//! [`find`] looks for the magic in the last [`SEARCH_WINDOW`] octets of a received
//! frame, so Ethernet padding or a trailing FCS added on the way do not hide it.

/// Magic number at the start of a signature (`"WPG"` + format version 1).
pub const MAGIC: [u8; 4] = [0x57, 0x50, 0x47, 0x01];

/// Length of a signature in octets.
pub const LEN: usize = 24;

/// How far from the end of a received frame [`find`] looks for the magic
/// (the signature plus room for Ethernet padding and an FCS).
pub const SEARCH_WINDOW: usize = LEN + 64 + 4;

/// One decoded signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature {
    /// Stream id (per job stream).
    pub stream_id: u32,
    /// Sequence number within the stream, starting at 0.
    pub seq: u64,
    /// TX time in nanoseconds since the Unix epoch.
    pub ts_ns: u64,
}

impl Signature {
    /// Encodes the signature (24 octets, see the module docs).
    #[must_use]
    pub fn encode(&self) -> [u8; LEN] {
        let mut b = [0u8; LEN];
        b[..4].copy_from_slice(&MAGIC);
        b[4..8].copy_from_slice(&self.stream_id.to_be_bytes());
        b[8..16].copy_from_slice(&self.seq.to_be_bytes());
        b[16..24].copy_from_slice(&self.ts_ns.to_be_bytes());
        b
    }

    /// Decodes a signature from the first [`LEN`] octets of `b` (`None` if the
    /// magic does not match or `b` is too short).
    #[must_use]
    pub fn decode(b: &[u8]) -> Option<Self> {
        let b = b.get(..LEN)?;
        if b[..4] != MAGIC {
            return None;
        }
        let u32_at = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let u64_at = |o: usize| (u64::from(u32_at(o)) << 32) | u64::from(u32_at(o + 4));
        Some(Self {
            stream_id: u32_at(4),
            seq: u64_at(8),
            ts_ns: u64_at(16),
        })
    }
}

/// Why a frame cannot carry a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampError {
    /// The payload after the last recognised header is shorter than [`LEN`]
    /// (`room` octets available).
    NoRoom {
        /// Octets available at the end of the frame.
        room: usize,
    },
}

impl core::fmt::Display for StampError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StampError::NoRoom { room } => write!(
                f,
                "the frame has {room} octets of payload, a signature needs {LEN}"
            ),
        }
    }
}

impl std::error::Error for StampError {}

/// Where a frame's payload starts and which checksum (if any) covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layout {
    /// First octet that may be overwritten.
    payload: usize,
    /// Transport checksum: (start of the checksummed transport segment, offset of
    /// the checksum field, end of the segment).
    checksum: Option<(usize, usize, usize)>,
    /// The transport is UDP.
    udp: bool,
}

fn be16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}

/// Walks the headers of `frame`.
fn layout(frame: &[u8]) -> Layout {
    let mut off = 12;
    let Some(mut ethertype) = be16(frame, off) else {
        return Layout {
            payload: frame.len().max(14),
            checksum: None,
            udp: false,
        };
    };
    off += 2;
    // 802.1Q / 802.1ad / old QinQ tags.
    while matches!(ethertype, 0x8100 | 0x88a8 | 0x9100) {
        match be16(frame, off + 2) {
            Some(t) => {
                ethertype = t;
                off += 4;
            }
            None => {
                return Layout {
                    payload: off + 4,
                    checksum: None,
                    udp: false,
                }
            }
        }
    }
    let l2 = Layout {
        payload: off,
        checksum: None,
        udp: false,
    };
    let (proto, l4, end) = match ethertype {
        0x0800 => {
            let Some(&vihl) = frame.get(off) else {
                return l2;
            };
            let ihl = usize::from(vihl & 0x0f) * 4;
            let (Some(total), Some(frag)) = (be16(frame, off + 2), be16(frame, off + 6)) else {
                return l2;
            };
            let Some(&proto) = frame.get(off + 9) else {
                return l2;
            };
            let end = (off + usize::from(total)).min(frame.len());
            let l4 = off + ihl;
            if ihl < 20 || l4 > end {
                return l2;
            }
            if frag & 0x1fff != 0 {
                // Non-first fragment: no transport header here.
                return Layout {
                    payload: l4,
                    checksum: None,
                    udp: false,
                };
            }
            (proto, l4, end)
        }
        0x86dd => {
            let (Some(plen), Some(&nh)) = (be16(frame, off + 4), frame.get(off + 6)) else {
                return l2;
            };
            let end = (off + 40 + usize::from(plen)).min(frame.len());
            let mut nh = nh;
            let mut l4 = off + 40;
            // Hop-by-hop, routing, destination options; stop at a fragment header.
            while matches!(nh, 0 | 43 | 60) {
                let (Some(&next), Some(&len)) = (frame.get(l4), frame.get(l4 + 1)) else {
                    return Layout {
                        payload: l4,
                        checksum: None,
                        udp: false,
                    };
                };
                nh = next;
                l4 += (usize::from(len) + 1) * 8;
            }
            if l4 > end {
                return l2;
            }
            (nh, l4, end)
        }
        _ => return l2,
    };
    let (hdr, ck) = match proto {
        // ICMP / ICMPv6: the checksum covers the whole message.
        1 | 58 => (8, Some(2)),
        6 => {
            let doff = frame.get(l4 + 12).map_or(20, |b| usize::from(b >> 4) * 4);
            (doff.max(20), Some(16))
        }
        17 => {
            let ck = be16(frame, l4 + 6);
            // IPv4 UDP checksum 0 = not computed.
            let disabled = ethertype == 0x0800 && ck == Some(0);
            (8, (!disabled).then_some(6))
        }
        _ => (0, None),
    };
    let payload = l4 + hdr;
    Layout {
        payload,
        checksum: ck
            .filter(|c| l4 + c + 2 <= end && payload <= end)
            .map(|c| (l4, l4 + c, end)),
        udp: proto == 17,
    }
}

/// One's complement sum of `data` treated as starting at an even (`odd == false`)
/// or odd offset of the checksummed segment.
fn ones_sum(data: &[u8], odd: bool) -> u32 {
    let mut sum = 0u32;
    for (i, &b) in data.iter().enumerate() {
        let high = (i % 2 == 0) != odd;
        sum += if high {
            u32::from(b) << 8
        } else {
            u32::from(b)
        };
    }
    sum
}

fn fold(mut s: u32) -> u16 {
    while s > 0xffff {
        s = (s & 0xffff) + (s >> 16);
    }
    #[allow(clippy::cast_possible_truncation)] // folded to 16 bits above
    let f = s as u16;
    f
}

/// Octets available for a signature at the end of `frame` (payload plus Ethernet
/// padding after the last recognised header).
#[must_use]
pub fn room(frame: &[u8]) -> usize {
    frame.len().saturating_sub(layout(frame).payload)
}

/// Writes `sig` over the last [`LEN`] octets of `frame`, keeping the UDP / TCP /
/// ICMP checksum valid.
///
/// # Errors
/// [`StampError::NoRoom`] when the frame has fewer than [`LEN`] octets of payload;
/// the frame is left unchanged.
pub fn stamp(frame: &mut [u8], sig: &Signature) -> Result<(), StampError> {
    let lay = layout(frame);
    let room = frame.len().saturating_sub(lay.payload);
    if room < LEN {
        return Err(StampError::NoRoom { room });
    }
    let at = frame.len() - LEN;
    let new = sig.encode();
    if let Some((seg, ck_at, end)) = lay.checksum {
        if at < end {
            let covered = (end - at).min(LEN);
            let odd = (at - seg) % 2 == 1;
            let old_sum = ones_sum(&frame[at..at + covered], odd);
            let new_sum = ones_sum(&new[..covered], odd);
            let old_ck = u16::from_be_bytes([frame[ck_at], frame[ck_at + 1]]);
            // RFC 1624 eqn. 3: HC' = ~(~HC + ~m + m').
            let s = u32::from(!old_ck) + u32::from(!fold(old_sum)) + new_sum;
            let mut ck = !fold(s);
            if ck == 0 && lay.udp {
                // RFC 768: a computed UDP checksum of 0 is sent as all ones.
                ck = 0xffff;
            }
            frame[ck_at..ck_at + 2].copy_from_slice(&ck.to_be_bytes());
        }
    }
    frame[at..].copy_from_slice(&new);
    Ok(())
}

/// Looks for a signature near the end of a received frame (see [`SEARCH_WINDOW`]).
#[must_use]
pub fn find(frame: &[u8]) -> Option<Signature> {
    if frame.len() < LEN {
        return None;
    }
    let last = frame.len() - LEN;
    let first = frame.len().saturating_sub(SEARCH_WINDOW);
    (first..=last)
        .rev()
        .find(|&i| frame[i..i + 4] == MAGIC)
        .and_then(|i| Signature::decode(&frame[i..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::InternetSum;

    const SIG: Signature = Signature {
        stream_id: 7,
        seq: 0x0102_0304_0506_0708,
        ts_ns: 1_700_000_000_123_456_789,
    };

    fn eth(ethertype: u16) -> Vec<u8> {
        let mut f = vec![0xff; 6];
        f.extend_from_slice(&[0x02, 0, 0, 0, 0, 1]);
        f.extend_from_slice(&ethertype.to_be_bytes());
        f
    }

    /// IPv4 + UDP with a correct checksum; `payload` octets of 0xab.
    fn udp4(payload: usize, pad_to: usize) -> Vec<u8> {
        let mut f = eth(0x0800);
        let total = 20 + 8 + payload;
        let mut ip = vec![
            0x45, 0, 0, 0, 0, 1, 0, 0, 64, 17, 0, 0, 10, 0, 0, 1, 10, 0, 0, 2,
        ];
        ip[2..4].copy_from_slice(&u16::try_from(total).unwrap().to_be_bytes());
        f.extend_from_slice(&ip);
        let ulen = u16::try_from(8 + payload).unwrap();
        f.extend_from_slice(&[0x30, 0x39, 0x00, 0x09]);
        f.extend_from_slice(&ulen.to_be_bytes());
        f.extend_from_slice(&[0, 0]);
        f.extend(std::iter::repeat_n(0xab, payload));
        let ck = udp4_checksum(&f);
        f[14 + 20 + 6..14 + 20 + 8].copy_from_slice(&ck.to_be_bytes());
        f.resize(f.len().max(pad_to), 0);
        f
    }

    fn udp4_checksum(f: &[u8]) -> u16 {
        let total = usize::from(u16::from_be_bytes([f[16], f[17]]));
        let seg = &f[34..14 + total];
        let mut s = InternetSum::new();
        s.add(&f[26..34]).add(&[0, 17]);
        s.add(&u16::try_from(seg.len()).unwrap().to_be_bytes());
        let mut copy = seg.to_vec();
        copy[6] = 0;
        copy[7] = 0;
        s.add(&copy);
        s.finish()
    }

    #[test]
    fn encode_decode_find() {
        let b = SIG.encode();
        assert_eq!(&b[..4], b"WPG\x01");
        assert_eq!(Signature::decode(&b), Some(SIG));
        assert_eq!(Signature::decode(&b[..23]), None);
        let mut f = vec![0u8; 100];
        f[60..84].copy_from_slice(&b);
        assert_eq!(find(&f), Some(SIG));
        assert_eq!(find(&[0u8; 100]), None);
        assert_eq!(find(&b), Some(SIG));
    }

    #[test]
    fn stamps_udp_and_keeps_checksum() {
        for payload in [24, 25, 30, 31, 100, 101] {
            let mut f = udp4(payload, 0);
            stamp(&mut f, &SIG).unwrap();
            assert_eq!(find(&f), Some(SIG), "payload {payload}");
            let stored = u16::from_be_bytes([f[40], f[41]]);
            assert_eq!(stored, udp4_checksum(&f), "payload {payload}");
        }
    }

    #[test]
    fn padding_and_room() {
        // 4 octets of UDP payload, padded to 60: signature straddles payload and padding.
        let mut f = udp4(4, 60);
        assert_eq!(room(&f), 60 - 42);
        assert_eq!(stamp(&mut f, &SIG), Err(StampError::NoRoom { room: 18 }));
        let mut f = udp4(10, 80);
        stamp(&mut f, &SIG).unwrap();
        assert_eq!(
            u16::from_be_bytes([f[40], f[41]]),
            udp4_checksum(&f),
            "covered part fixed up"
        );
        assert_eq!(find(&f), Some(SIG));
        // Disabled IPv4 UDP checksum stays 0.
        let mut f = udp4(40, 0);
        f[40] = 0;
        f[41] = 0;
        stamp(&mut f, &SIG).unwrap();
        assert_eq!(&f[40..42], &[0, 0]);
    }

    #[test]
    fn vlan_and_non_ip() {
        let mut f = eth(0x8100);
        f.extend_from_slice(&[0, 10, 0x88, 0xb5]);
        f.resize(64, 0x11);
        assert_eq!(room(&f), 64 - 18);
        stamp(&mut f, &SIG).unwrap();
        assert_eq!(find(&f), Some(SIG));
        let mut short = eth(0x88b5);
        short.resize(30, 0);
        assert!(stamp(&mut short, &SIG).is_err());
        assert_eq!(short[14..], [0u8; 16]);
    }

    #[test]
    fn tcp_and_ipv6() {
        // IPv6 / UDP: checksum mandatory; verify by recomputation.
        let mut f = eth(0x86dd);
        let payload = 33usize;
        let plen = u16::try_from(8 + payload).unwrap();
        let mut ip = vec![0x60, 0, 0, 0];
        ip.extend_from_slice(&plen.to_be_bytes());
        ip.extend_from_slice(&[17, 64]);
        ip.extend_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        ip.extend_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        f.extend_from_slice(&ip);
        f.extend_from_slice(&[0x30, 0x39, 0x00, 0x09]);
        f.extend_from_slice(&plen.to_be_bytes());
        f.extend_from_slice(&[0, 0]);
        f.extend(std::iter::repeat_n(0x5a, payload));
        let ck6 = |f: &[u8]| {
            let mut s = InternetSum::new();
            s.add(&f[22..54]).add(&[0, 0]).add(&plen.to_be_bytes());
            s.add(&[0, 0, 0, 17]);
            let mut seg = f[54..].to_vec();
            seg[6] = 0;
            seg[7] = 0;
            s.add(&seg);
            match s.finish() {
                0 => 0xffff,
                c => c,
            }
        };
        let c = ck6(&f);
        f[60..62].copy_from_slice(&c.to_be_bytes());
        stamp(&mut f, &SIG).unwrap();
        assert_eq!(u16::from_be_bytes([f[60], f[61]]), ck6(&f));
        assert_eq!(find(&f), Some(SIG));

        // TCP header with options (data offset 8 = 32 octets): only payload is used.
        let mut t = eth(0x0800);
        let total = 20 + 32 + 10;
        let mut ip = vec![
            0x45, 0, 0, 0, 0, 1, 0, 0, 64, 6, 0, 0, 10, 0, 0, 1, 10, 0, 0, 2,
        ];
        ip[2..4].copy_from_slice(&u16::try_from(total).unwrap().to_be_bytes());
        t.extend_from_slice(&ip);
        let mut tcp = vec![0u8; 32];
        tcp[12] = 8 << 4;
        t.extend_from_slice(&tcp);
        t.extend_from_slice(&[0; 10]);
        assert_eq!(room(&t), 10);
        assert!(stamp(&mut t, &SIG).is_err());
    }
}
