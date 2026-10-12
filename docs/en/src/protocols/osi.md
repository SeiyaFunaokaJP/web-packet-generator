# OSI encapsulation (P2)

Status: draft

Notes on the OSI network-layer encapsulation shared by CLNP / ES-IS / IS-IS, and on the NSAP address type (M7). The definitions are in `crates/wpg-protocols/src/osi.rs`; the sample is `templates/protocols/l2/dot3-llc-osi.wpg.yaml`.

## How it is reached

| Layer below | Selector | Layer above |
|---|---|---|
| `llc` | DSAP / SSAP `0xfe` | chosen by the PDU's leading NLPID octet |
| `ppp` | protocol `0x0023` (RFC 1377) | same |

No "next protocol" number precedes an OSI PDU: its own first octet, the NLPID (ISO/IEC TR 9577), says what it is (`0x81` CLNP, `0x82` ES-IS, `0x83` IS-IS). `llc` and `ppp` therefore peek at the first 8 payload bits (`Binding::peek`) to pick the next layer. The dedicated definitions are registered in that peek table: CLNP (`0x81`) and ES-IS (`0x82`), see [CLNP / ES-IS](clnp.md), and IS-IS (`0x83`), see [IS-IS](isis.md). Other NLPIDs (for example `0x00`, the inactive subset of ISO 8473), and PDUs too short for their definition, decode as the generic `osi` PDU (the NLPID plus the rest as opaque octets).

When building, stack `osi` or a dedicated PDU (`clnp`, `esis_ish`, `isis_l1_lan_hello`, ...) on top of `llc`; DSAP / SSAP become `0xfe` automatically. The PPP `protocol` likewise becomes `0x0023` automatically.

## The NSAP type

- An `nsap` value is the address itself (1..=20 octets). On the wire it is preceded by a 1-octet length indicator, as is usual in OSI; the builder writes it and the parser reads it.
- Notation: hex such as `49.0001.1921.6800.1001.00` (`.` and spaces are ignored, a `0x` prefix is allowed).
- The GUI field editor splits the NSAP being typed into AFI / area / system ID (6 octets) / NSEL (addresses shorter than 8 octets are not split).
- A `range` generator steps the system ID (for example `49.0001.1921.6800.1001.00-49.0001.1921.6800.1004.00`).

## Limits

- The `osi` NLPID is not Auto.
- A PDU with NLPID `0x81` / `0x82` / `0x83` always decodes as CLNP / ES-IS / IS-IS (when long enough), so a generic `osi` layer carrying one of them reads back as the dedicated protocol.
