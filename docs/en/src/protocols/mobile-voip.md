# MACsec / GTPv2-C / Diameter / SIP / RTP (P2)

Status: draft

Notes on MACsec (IEEE 802.1AE, structure only), GTPv2-C (3GPP TS 29.274), Diameter (RFC 6733, Credit-Control RFC 4006), SIP (RFC 3261) and RTP / RTCP (RFC 3550) (M7). The definitions live in `crates/wpg-protocols/src/{macsec,gtpv2,diameter,sip,rtp}.rs`, the samples in `templates/protocols/l2/macsec.wpg.yaml`, `templates/protocols/mobile/` and `templates/protocols/voip/`.

## Stacks and samples

```text
ethernet2 / macsec                          # EtherType 0x88e5 (Auto); also after dot1q
ethernet2 / ipv4 / udp / gtpv2 [/ gtpv2]    # UDP 2123; a piggybacked message is a second gtpv2 layer
ethernet2 / ipv4 / tcp / diameter           # TCP 3868
ethernet2 / ipv4 / udp|tcp / sip            # UDP / TCP 5060
ethernet2 / ipv4 / udp / rtp / payload      # any port (explicit layer); UDP 5004 is decoded
ethernet2 / ipv4 / udp / rtcp_sr / rtcp_sdes ...  # compound RTCP; UDP 5005 is decoded
```

UDP / TCP destination ports are not Auto: set 2123, 3868, 5060, ... (decoding also recognises the source port).

| Sample | Contents |
|---|---|
| `macsec` | Encrypted frame with an explicit SCI (association number 1, 48 octets of secure data, opaque ICV) |
| `gtpv2-create-session-request` | S11 Create Session Request: IMSI, MSISDN, Serving Network, RAT type, two F-TEIDs, APN, Selection Mode, PDN type, PAA, AMBR, Bearer Context |
| `gtpv2-create-session-response` | Create Session Response: Cause, F-TEIDs, PAA, APN Restriction, Bearer Context with an S1-U F-TEID and a Charging ID |
| `gtpv2-echo-request` | Echo Request without TEID, Recovery IE |
| `diameter-cer` | Capabilities-Exchange-Request with a Vendor-Specific-Application-Id (S6a) |
| `diameter-ccr-initial` | Initial Credit-Control-Request with a Subscription-Id |
| `sip-invite` | INVITE over UDP with an SDP offer |
| `sip-register` | REGISTER over TCP without a body |
| `rtp-pcmu` | 50 packets of G.711 PCMU (sequence number and timestamp generators, 160-octet payload) |
| `rtcp-sr-sdes` | Sender Report with one report block and an SDES CNAME |
| `rtcp-rr-bye` | Receiver Report, SDES CNAME and BYE with a reason |
| `rtcp-feedback-app` | Receiver Report, a Picture Loss Indication (generic `rtcp` layer) and an APP packet |

## MACsec: structure only

The `macsec` layer is the SecTAG and the protected part of the frame:

| Field | Contents |
|---|---|
| `v`, `es`, `sc`, `scb`, `e`, `c` | TCI bits. `sc` is Auto: set when `sci` holds an element |
| `an` | Association number (2 bits) |
| `sl` | Short length (6 bits): the secure data length when it is below 48 octets, otherwise 0. Manual |
| `pn` | Packet number (32 bits) |
| `sci` | `[]` or `[{ system_identifier, port_identifier }]` |
| `secure_data` | Opaque octets: what you write is sent, nothing is encrypted |
| `icv` | 16 opaque octets: not computed |

When parsing, the last 16 octets of the frame are the ICV and everything between the SecTAG and the ICV is the secure data. A frame shorter than the Ethernet minimum is padded after the ICV (as on the wire); parsing it back keeps the padding inside the secure data / ICV. Integrity-only frames (E = C = 0) are not decoded further: the user data stays in `secure_data`.

## GTPv2-C: one layer is one message

The `gtpv2` layer is the header (`version` 2, `p`, `t`, `mp`, `message_type`, `message_length`, `teid`, `sequence_number`, `message_priority`) and the IEs (`ies`).

- `t` is Auto from `teid`: `teid: [0x1001]` sends a TEID, `teid: []` (Echo) leaves it out.
- `message_length` is Auto (the octets after the first four). The IEs end at that length, so a piggybacked message (`p: 1`) is the next `gtpv2` layer.
- An IE is an 8-bit type, a 16-bit length of the IE data, and a spare / instance octet that the length does not count (wpg-core `TlvDef::uncounted_octets`). Every typed IE has the `spare` and `instance` fields first.

```yaml
- protocol: gtpv2
  fields:
    message_type: create_session_request
    teid: [0]
    ies:
      - imsi: { digits: "00 01 01 21 43 65 87 f9" }   # TBCD
      - fteid: { interface_type: s11_mme_gtp_c, teid: 0x1001, ipv4: 192.0.2.1 }
      - apn: { apn: "08 69 6e 74 65 72 6e 65 74" }    # DNS labels
      - bearer_context:
          ies:
            - ebi: { ebi: 5 }
            - bearer_qos: { pl: 9, qci: 9 }
```

| Type | IE (`name`) and data fields |
|---|---|
| 1 / 75 / 76 | `imsi` / `mei` / `msisdn` (`digits`, TBCD octets) |
| 2 | `cause` (`cause`, `pce`, `bce`, `cs`, `offending_ie`) |
| 3 | `recovery` (`restart_counter`) |
| 71 | `apn` (`apn`, DNS label octets) |
| 72 | `ambr` (`uplink`, `downlink` in kbps) |
| 73 | `ebi` (`ebi`) |
| 74 | `ip_address` / `ip_address_ipv6` (`address`), by the IE length |
| 77 / 78 / 84 / 86 | `indication` (`flags`), `pco`, `bearer_tft`, `uli` (`data` / `tft` octets) |
| 79 | `paa` / `paa_ipv6` / `paa_ipv4v6` (`pdn_type`, `ipv6_prefix_length`, `ipv6`, `ipv4`), by the IE length |
| 80 | `bearer_qos` (`pci`, `pl`, `pvi`, `qci`, `mbr_uplink`, `mbr_downlink`, `gbr_uplink`, `gbr_downlink`) |
| 82 | `rat_type` |
| 83 | `serving_network` (MCC / MNC digits) |
| 87 | `fteid` / `fteid_ipv6` / `fteid_ipv4v6` (`v4`, `v6`, `interface_type`, `teid`, `ipv4`, `ipv6`), by the IE length |
| 93 | `bearer_context` (`ies`: the IEs above, one level deep) |
| 94 / 95 | `charging_id`, `charging_characteristics` |
| 99 | `pdn_type` |
| 114 / 127 / 128 | `ue_time_zone`, `apn_restriction`, `selection_mode` |
| 255 | `private_extension` (`enterprise_id`, `value`) |

Other IEs are kept as `{ type: N, data: "..." }` (the data starts with the instance octet). GTPv1-C (version 1, also on UDP 2123) is not modelled: such a message decodes as `gtpv2` with a version warning.

## Diameter: header and AVPs

The `diameter` layer is the header (`version`, `message_length` (Auto), the `r` / `p` / `e` / `t` flags, `command_code`, `application_id`, `hop_by_hop`, `end_to_end`) and the AVPs (`avps`). The AVPs end at the message length, so a second message in the same TCP segment is the next layer (`raw`).

An AVP is a 32-bit code, the flags octet (between the code and the length: wpg-core `TlvDef::head_octets`), a 24-bit length counting the header and data but not the padding (Auto), the Vendor-ID when the V flag is set, and the data padded to 4 octets (Auto). Every typed AVP has the fields `v` (Auto from `vendor_id`), `m`, `p`, `reserved`, `vendor_id` (`[]` or `[10415]`), then its data:

| Data type | AVPs (`name`) | Data field |
|---|---|---|
| OctetString / UTF8String / DiameterIdentity / DiameterURI | `session_id`, `origin_host`, `origin_realm`, `destination_host`, `destination_realm`, `product_name`, `user_name`, `error_message`, `route_record`, `proxy_host`, `proxy_state`, `class`, `redirect_host`, `service_context_id`, `subscription_id_data`, ... | `value` (octets; write text as `'"..."'`) |
| Unsigned32 / Time | `vendor_id`, `supported_vendor_id`, `auth_application_id`, `acct_application_id`, `origin_state_id`, `firmware_revision`, `session_timeout`, `experimental_result_code`, `cc_request_number`, `event_timestamp`, ... | `value` |
| Unsigned64 | `accounting_sub_session_id` | `value` |
| Enumerated | `result_code`, `disconnect_cause`, `auth_session_state`, `auth_request_type`, `re_auth_request_type`, `termination_cause`, `accounting_record_type`, `cc_request_type`, `subscription_id_type`, `multiple_services_indicator` | `value` (names) |
| Address | `host_ip_address` / `host_ip_address_ipv6`, by the AVP length | `address_family`, `address` |
| Grouped | `vendor_specific_application_id`, `failed_avp`, `proxy_info`, `experimental_result`, `subscription_id` | `avps` (the AVPs above, one level deep) |

Vendor-specific AVPs (3GPP and others) whose code is not listed are kept as `{ type: N, data: "..." }` with the flags octet, the Vendor-ID and the value in `data`. Diameter over SCTP is built by putting the message octets into the user data of an SCTP DATA chunk; SCTP does not decode its user data (see [SCTP](sctp.md)).

## SIP: text lines

The `sip` layer is text. `start_line` and every element of `headers` are CRLF-terminated lines (wpg-core `Length::Until`): write them without the CRLF, the builder adds it, and the empty line after the headers. `body` is the rest of the message (an SDP offer / answer, ...).

```yaml
- protocol: sip
  fields:
    start_line: '"INVITE sip:bob@example.com SIP/2.0"'
    headers:
      - '"Via: SIP/2.0/UDP 192.0.2.1:5060;branch=z9hG4bK74bf9"'
      - '"Content-Type: application/sdp"'
      - '"Content-Length: 132"'
    body: '"v=0\r\no=alice ...\r\n"'
```

`Content-Length` is an ordinary header line and stays Manual: keep it in step with the body (the samples do). A message without the empty line does not decode as `sip`.

## RTP and RTCP

There is no well-known RTP port: the session ports come from SDP. Put the `rtp` / `rtcp_*` layer after `udp` explicitly; when parsing, only the IANA `rtp-avp` / `rtcp-avp` ports 5004 / 5005 select RTP / RTCP (tshark needs `-d udp.port==N,rtp` as well).

`rtp` is the fixed header, `csrc` (count `cc` Auto) and `header_extension` (`[]` or `[{ profile, data }]`: X bit and the length in words Auto, `data` is a list of 32-bit words; `profile` 0xBEDE is RFC 8285 one-byte elements). The media payload is the next layer (`payload` / `raw`). The padding bit and the padding octets are Manual.

RTCP is one layer per packet, so a compound packet is a stack of RTCP layers. The count (`report_count` / `source_count`) and `length` (32-bit words minus one) are Auto; every packet ends at its length.

| Layer | Packet type | Body fields |
|---|---|---|
| `rtcp_sr` | 200 | `ssrc`, `ntp_timestamp_msw` / `lsw`, `rtp_timestamp`, `packet_count`, `octet_count`, `report_blocks`, `extension` |
| `rtcp_rr` | 201 | `ssrc`, `report_blocks`, `extension` |
| `rtcp_sdes` | 202 | `chunks`: `[{ ssrc, items: [{ type, text }] }]` (item length Auto, null octet and padding Auto) |
| `rtcp_bye` | 203 | `ssrcs`, `reason` (`[]` or `[{ text }]`) |
| `rtcp_app` | 204 | `subtype` (the count field), `ssrc`, `name` (4 octets), `data` |
| `rtcp` | any | `count`, `body` (octets); parsing re-decodes it as the typed packet for 200-204 |

## Not covered

- MACsec encryption / ICV computation, XPN (64-bit packet numbers), decoding of integrity-only user data
- GTPv1-C, GTPv2-C IEs not listed above as typed (ULI, Indication flags, PCO contents are octets), nested Bearer Contexts
- Diameter vendor-specific AVP dictionaries (3GPP S6a / Gx / Gy ...), grouped AVPs deeper than one level, demultiplexing Diameter from SCTP DATA chunks
- SIP `Content-Length` Auto, header parsing into name / value, SIP over TLS (5061), multipart bodies
- RTP payload formats, SRTP, RTCP feedback (RTPFB / PSFB) and XR bodies (kept as octets in `rtcp`)
