# Application protocols

Status: draft

Notes on DHCPv4 / DHCPv6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP. Definitions are in `crates/wpg-protocols/src/app.rs`, samples in `templates/protocols/app/`.

## How they are reached, and samples

All of them are demultiplexed on the UDP port: the destination port, else the source port, so replies from a well-known port to an ephemeral one (DNS / SNMP / RADIUS responses, ...) decode too.

| id | Protocol | UDP port | Samples |
|---|---|---|---|
| `dhcpv4` | DHCPv4 (incl. relay agent information) | 67 / 68 | `dhcpv4-discover`, `dhcpv4-relay` |
| `dhcpv6` / `dhcpv6_relay` | DHCPv6 / Relay-Forward and Relay-Reply | 546 / 547 | `dhcpv6-solicit`, `dhcpv6-relay-forward` |
| `dns` | DNS | 53 | `dns-query`, `dns-response` |
| `ntp` | NTP (RFC 5905 time packet) | 123 | `ntp-client` |
| `snmp` + `snmp_pdu` / `snmp_trap` | SNMP v1 / v2c | 161 / 162 | `snmp-get-request`, `snmp-trap-v1` |
| `syslog` | Syslog | 514 | `syslog` |
| `radius` | RADIUS | 1812 / 1813 | `radius-access-request` |
| `tftp` + `tftp_data` / `tftp_ack` / `tftp_error` / `tftp_oack` | TFTP | 69 | `tftp-rrq` / `-data` / `-ack` / `-error` / `-oack` |

## Auto fields

- `radius.length`.
- SNMP `msg_length` / `pdu_length` (BER short form).
- The length octets of DHCPv4 / DHCPv6 options and RADIUS attributes.

## Fields you set by hand

- **DNS** `qdcount` / `ancount` / `nscount` / `arcount`. The sections are opaque, so set the counts to match their contents.
- **SNMP** `community_length` (update it when you change the community string).
- **RADIUS** `authenticator` (Response Authenticator) and the Message-Authenticator attribute. Both are MD5 / HMAC-MD5 with the shared secret, and there is no keyed-digest auto rule. An Access-Request's Request Authenticator is random, so any 16 octets will do.

## Limits and approximations

- **DNS**: names are length-prefixed labels ended by a zero octet, and each section is sized by a header count. The engine's list type has neither a stop-on-terminator nor a count-driven length, so only the 12-octet header is typed and the sections (question, answer, authority, additional) are one opaque `sections` region. The default is an `example.com` A/IN question. Write compression pointers as octets too.
- **SNMP** is BER-encoded ASN.1, modelled as a fixed layout of pre-encoded tag / length / value triples. Only the BER **short-form length (contents below 128 octets)** is supported; long-form lengths (`0x81 nn`, `0x82 nn nn`) are not. Constructed tags nest to a fixed depth (varbind list → varbind → value). The PDU type is only known inside the PDU, so `snmp` is followed by `snmp_pdu` by default, which re-decodes as `snmp_trap` for the v1 Trap tag (`0xa4`).
- **DHCPv6** Relay-Forward / Relay-Reply (msg-type 12 / 13) have a different fixed layout from client / server messages, so decoding starts as `dhcpv6` and re-decodes as `dhcpv6_relay`. The message inside the Relay Message option is typed one level deep only.
- **TFTP**: the RRQ / WRQ file name, mode and options and the error message are typed as NUL-terminated strings. DATA / ACK / ERROR / OACK split into `tftp_data` and so on via re-parse on the opcode. The transfer itself runs between two ephemeral ports (TIDs), so packets not on port 69 decode as `raw` after UDP.
- **NTP** control (mode 6) and private (mode 7) messages have their own layouts and are out of scope. Extension fields and the MAC are an opaque `extensions`.
- **Syslog** is free text, a single opaque `message`. Give the string as is, in RFC 5424 or RFC 3164 format.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`dhcpv4`](../reference/protocols/dhcpv4.md) · [`dhcpv6`](../reference/protocols/dhcpv6.md) · [`dhcpv6_relay`](../reference/protocols/dhcpv6_relay.md) · [`dns`](../reference/protocols/dns.md) · [`ntp`](../reference/protocols/ntp.md) · [`snmp`](../reference/protocols/snmp.md) · [`snmp_pdu`](../reference/protocols/snmp_pdu.md) · [`snmp_trap`](../reference/protocols/snmp_trap.md) · [`syslog`](../reference/protocols/syslog.md) · [`radius`](../reference/protocols/radius.md) · [`tftp`](../reference/protocols/tftp.md) · [`tftp_data`](../reference/protocols/tftp_data.md) · [`tftp_ack`](../reference/protocols/tftp_ack.md) · [`tftp_error`](../reference/protocols/tftp_error.md) · [`tftp_oack`](../reference/protocols/tftp_oack.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
