# Application protocols (UDP)

Status: draft

Notes on DHCPv4, DHCPv6, DNS, NTP, SNMP v1 / v2c, Syslog, RADIUS and TFTP. The definitions are in `crates/wpg-protocols/src/app.rs`.

## Identification and stacking

All of them go on `udp` and are identified by port. Parsing uses the destination port, else the source port, so replies from a well-known port to an ephemeral one (DNS / SNMP / RADIUS responses and so on) decode too. Building makes only the destination port Auto.

| Protocol ID | UDP port | Notes |
|---|---|---|
| `dhcpv4` | 67 (server) / 68 (client) | relaying via `hops` / `giaddr` / Relay Agent Information (option 82) |
| `dhcpv6` | 546 (client) / 547 (server, relay) | types 12 / 13 re-parse as `dhcpv6_relay` |
| `dhcpv6_relay` | 547 | Relay-Forward / Relay-Reply |
| `dns` | 53 | |
| `ntp` | 123 | |
| `snmp` → `snmp_pdu` / `snmp_trap` | 161 (agent) / 162 (trap receiver) | |
| `syslog` | 514 | |
| `radius` | 1812 (authentication) / 1813 (accounting) | |
| `tftp` | 69 | DATA / ACK / ERROR / OACK re-parse as `tftp_data` / `tftp_ack` / `tftp_error` / `tftp_oack` |

TFTP transfers run between ephemeral ports (TIDs), so they parse as `raw` after `udp`. When building, stack `tftp_data` and so on directly and set the `udp` ports manually.

## Auto fields

| Protocol | Auto |
|---|---|
| `snmp` | `msg_length` |
| `snmp_pdu` / `snmp_trap` | `pdu_length` |
| `radius` | `length` |
| TLV-style options / attributes (DHCPv4 / DHCPv6 options, RADIUS attributes, BER TLVs) | each element's length |

## Limitations and workarounds

- **DNS** types only the 12-octet header (the flag word split into bit fields); the question / answer / authority / additional sections are one opaque `sections` region. Names are length-prefixed labels, so write them as octets in `sections`, like `07 "example" 03 "com" 00` (see `dns-query.wpg.yaml`). `qdcount` / `ancount` / `nscount` / `arcount` are Manual: keep them in step with the sections. Compression pointers (`c0 0c` and so on) can be written as is.
- **SNMP** supports only the BER **short-form** length (contents below 128 octets); long-form lengths (`0x81 nn`, `0x82 nn nn`) cannot be expressed. `community_length` is Manual: set it to the octet count of `community`. Constructed types (SEQUENCE) nest to a fixed depth (varbind list → varbind → value). The PDU type is only known inside the PDU, so `snmp` is followed by `snmp_pdu` by default, which re-parses as `snmp_trap` for the v1 Trap tag (`0xa4`).
- **DHCPv6**'s Relay Message option types the inner message one level deep; deeper relay nesting is opaque.
- **RADIUS** Response Authenticator and Message-Authenticator are MD5 / HMAC-MD5 with the shared secret, and there is no keyed-digest Auto rule. `authenticator` is Manual (16 random octets are fine for an Access-Request).
- **NTP** models the RFC 5905 time packet. Control (mode 6) / private (mode 7) messages and extension fields / MAC are opaque `extensions`. Timestamps are 32.32 fixed point, seconds since 1900.
- **Syslog** is the whole message, including `<PRI>`, as one text `message` (PRI = facility × 8 + severity).
- **TFTP** request / error strings (file name, mode, option names and values, error message) are typed as NUL-terminated strings.

## Samples

| Template | Contents |
|---|---|
| `templates/protocols/app/dhcpv4-discover.wpg.yaml` / `dhcpv4-relay.wpg.yaml` | a broadcast DHCPDISCOVER / a DHCPREQUEST relayed with giaddr and Relay Agent Information |
| `templates/protocols/app/dhcpv6-solicit.wpg.yaml` / `dhcpv6-relay-forward.wpg.yaml` | Solicit / Relay-Forward with Interface-Id and Relay Message |
| `templates/protocols/app/dns-query.wpg.yaml` / `dns-response.wpg.yaml` | an A query for example.com / an authoritative response with a compressed name |
| `templates/protocols/app/ntp-client.wpg.yaml` | a client-mode (mode 3) request |
| `templates/protocols/app/snmp-get-request.wpg.yaml` / `snmp-trap-v1.wpg.yaml` | SNMPv2c GetRequest (sysDescr.0 / sysName.0) / SNMPv1 linkDown trap |
| `templates/protocols/app/syslog.wpg.yaml` | an RFC 5424 log line |
| `templates/protocols/app/radius-access-request.wpg.yaml` | an Access-Request with User-Name / NAS-IP-Address / NAS-Port |
| `templates/protocols/app/tftp-rrq.wpg.yaml` and the other `tftp-*.wpg.yaml` | RRQ (with the blksize option) / OACK / DATA / ACK / ERROR |
