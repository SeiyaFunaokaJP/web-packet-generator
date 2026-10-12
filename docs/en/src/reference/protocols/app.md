# Application protocols (P1)

Status: draft

Definitions: `crates/wpg-protocols/src/app.rs` / tests: `crates/wpg-protocols/tests/p1_app.rs` / samples: `templates/protocols/app/`

## Protocols and how they are reached

All of them are reached through UDP ports. Parsing uses the destination port, else the source port, so replies from a well-known port to an ephemeral one (DNS / SNMP / RADIUS responses and so on) decode too.

| id | Protocol | UDP port | Sample |
|---|---|---|---|
| `dhcpv4` | DHCPv4 (RFC 2131) | 67 / 68 | `dhcpv4-discover` / `dhcpv4-relay` |
| `dhcpv6` / `dhcpv6_relay` | DHCPv6 (RFC 8415) | 546 / 547 | `dhcpv6-solicit` / `dhcpv6-relay-forward` |
| `dns` | DNS (RFC 1035) | 53 | `dns-query` / `dns-response` |
| `ntp` | NTPv4 (RFC 5905) | 123 | `ntp-client` |
| `snmp` + `snmp_pdu` / `snmp_trap` | SNMP v1 / v2c | 161 / 162 | `snmp-get-request` / `snmp-trap-v1` |
| `syslog` | Syslog (RFC 5426) | 514 | `syslog` |
| `radius` | RADIUS (RFC 2865) | 1812 / 1813 | `radius-access-request` |
| `tftp` + `tftp_data` / `tftp_ack` / `tftp_error` / `tftp_oack` | TFTP (RFC 1350 / 2347) | 69 | `tftp-rrq` / `tftp-data` / `tftp-ack` / `tftp-error` / `tftp-oack` |

## Re-decoding while parsing

Messages with a different layout on the same port are re-decoded with another definition based on their leading field.

- **DHCPv6**: message types 12 / 13 (Relay-Forward / Relay-Reply) re-decode as `dhcpv6_relay`. The message inside the Relay Message option is typed one level deep only.
- **SNMP**: `snmp` is followed by `snmp_pdu`, which re-decodes as `snmp_trap` for the v1 Trap tag (`0xa4`).
- **TFTP**: the opcode re-decodes DATA / ACK / ERROR / OACK with their own definitions. Actual transfers run between two ephemeral ports (TIDs), though, so a packet without port 69 parses as `raw` after UDP.

## Auto fields

| Field | Value |
|---|---|
| `snmp.msg_length` | length of the outer SEQUENCE contents |
| `snmp_pdu.pdu_length` / `snmp_trap.pdu_length` | length of the PDU contents |
| `radius.length` | length of the whole RADIUS packet |

## Constraints and approximations

- **DNS sections are opaque**: names are length-prefixed labels ended by a zero octet and each section is sized by a header counter. That shape cannot be typed, so only the 12-octet header (with the flag word split into bits) is typed and the question / answer / authority / additional sections are one opaque `sections` region. The counters such as `qdcount` are Manual.
- **SNMP uses BER short form only**: the message is a sequence of pre-encoded tag / length / value triples with a fixed 8-bit length. Only the short form (contents below 128 octets) is supported; long-form lengths (`0x81 nn`, `0x82 nn nn`) are not. `community_length` is Manual and constructed types nest to a fixed depth (varbind list → varbind → value).
- **RADIUS Authenticator is Manual**: the Response Authenticator and Message-Authenticator are MD5 / HMAC-MD5 with the shared secret. There is no keyed-digest auto rule, so set `authenticator` manually.
- **NTP**: the RFC 5905 time packet is typed. Extension fields and the MAC are opaque `extensions`. Control (mode 6) and private (mode 7) messages are not modelled. Timestamps are 32.32 fixed point, seconds since 1900.
- **Syslog**: the whole message is one text (starting with `<PRI>`). PRI is facility * 8 + severity.
- **DHCPv4**: `chaddr` is split into the 6-octet Ethernet address and 10 octets of padding. Relaying is expressed with `hops`, `giaddr` and the Relay Agent Information option (82).
