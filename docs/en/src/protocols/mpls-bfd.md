# LDP / RSVP-TE / BFD (P2)

Status: draft

Notes on LDP (RFC 5036), RSVP-TE (RFC 2205 / RFC 3209) and BFD (RFC 5880 / 5881 / 5883, Micro-BFD RFC 7130) (M7). The definitions live in `crates/wpg-protocols/src/{ldp,rsvp,bfd}.rs`, the samples in `templates/protocols/mpls/` and `templates/protocols/oam/`.

## Stacks and samples

```text
ethernet2 / ipv4 / udp / ldp       # UDP 646: Hello (basic discovery to 224.0.0.2, extended to a unicast address)
ethernet2 / ipv4 / tcp / ldp       # TCP 646: the session (Initialization onwards)
ethernet2 / ipv4 / rsvp            # IPv4 protocol 46 (Auto)
ethernet2 / ipv6 / rsvp            # IPv6 next header 46 (Auto)
ethernet2 / ipv4 / udp / bfd       # UDP 3784 (single hop), 4784 (multihop), 6784 (Micro-BFD)
```

LDP over IPv6 (RFC 7552) is `ipv6 / udp|tcp / ldp`. UDP / TCP destination ports are not Auto: set 646, 3784, ... (decoding also recognises the source port). The BFD echo port 3785 is not decoded: its contents only mean something to the sender.

| Sample | Contents |
|---|---|
| `ldp-hello` | Link Hello to 224.0.0.2 (GTSM bit, IPv4 transport address) |
| `ldp-targeted-hello` | Targeted Hello for extended discovery (T / R bits, configuration sequence number) |
| `ldp-initialization` | Initialization (Common Session Parameters) and KeepAlive in one PDU |
| `ldp-label-mapping` | Address and two Label Mappings (prefix FECs, generic labels including implicit null 3) |
| `ldp-label-withdraw` | Label Withdraw with the wildcard FEC and a Notification (Shutdown) |
| `rsvp-te-path` | Path with Router Alert (strict / loose ERO, SESSION_ATTRIBUTE, SENDER_TSPEC, RRO) |
| `rsvp-te-resv` | Resv with the SE style (FLOWSPEC, FILTER_SPEC, LABEL, RRO with a label) |
| `rsvp-path-err` | PathErr with an ERROR_SPEC (Routing Problem) |
| `rsvp-path-tear` | PathTear |
| `rsvp-hello` | Hello with a HELLO REQUEST |
| `bfd-single-hop` | Control packets in state Up with 300 ms intervals (10 packets, 4 pps) |
| `bfd-auth` | Poll with meticulous keyed SHA1 authentication |
| `bfd-multihop-ipv6` | Multihop over IPv6 (UDP 4784, state Init) |
| `micro-bfd` | Micro-BFD on a LAG member link (UDP 6784, destination MAC `01:00:5e:90:00:01`) |

## LDP: one layer is one PDU

The `ldp` layer is the PDU header (`version`, `pdu_length`, `lsr_id`, `label_space`) and every message of the PDU (`messages`). Messages end at the PDU length, so a further PDU in the same TCP segment becomes the next layer (`raw`).

Messages and TLVs are items of the same shape: a 16-bit type (the U bit, and for TLVs the F bit, are its top bits), a 16-bit length of the value only, and the value. Every message starts with `message_id`, followed by `tlvs`.

```yaml
- protocol: ldp
  fields:
    lsr_id: 192.0.2.1
    messages:
      - label_mapping:
          message_id: 11
          tlvs:
            - fec:
                elements: [{ prefix_length: 24, prefix: "c6 33 64" }]
            - generic_label: { label: 299776 }
      - label_withdraw:
          message_id: 12
          tlvs:
            - fec_wildcard
```

| Type | Message (`name`) | Type | TLV (`name`) and value fields |
|---|---|---|---|
| 0x0001 | `notification` | 0x0100 | `fec` (`elements`: `{element_type (2), address_family, prefix_length, prefix}`), `fec_wildcard` (element type 1) |
| 0x0100 | `hello` | 0x0101 | `address_list` (IPv4) / `address_list_ipv6` (`address_family`, `addresses`) |
| 0x0200 | `initialization` | 0x0103 / 0x0104 | `hop_count`, `path_vector` (`lsr_ids`) |
| 0x0201 | `keepalive` | 0x0200 | `generic_label` (`label`: 20 bits) |
| 0x0300 / 0x0301 | `address` / `address_withdraw` | 0x0300 | `status` (`e`, `f`, `status_code`, `message_id`, `message_type`) |
| 0x0400 | `label_mapping` | 0x0400 | `common_hello_parameters` (`hold_time`, `t`, `r`, `g` (GTSM, RFC 6720)) |
| 0x0401 | `label_request` | 0x0401 / 0x0403 | `ipv4_transport_address` / `ipv6_transport_address` |
| 0x0402 | `label_withdraw` | 0x0402 | `configuration_sequence_number` |
| 0x0403 | `label_release` | 0x0500 | `common_session_parameters` (`protocol_version`, `keepalive_time`, `a`, `d`, `pv_lim`, `max_pdu_length`, `receiver_lsr_id`, `receiver_label_space`) |
| 0x0404 | `label_abort_request` | 0x0600 | `label_request_message_id` |

The FEC TLV is decoded as `fec_wildcard` or `fec` from the first value octet (the element type), the Address List as IPv4 or IPv6 from the first two (the address family). Other types (ATM / Frame Relay labels and session parameters, vendor TLVs, ...) are kept as `{ type: N, data: "..." }`.

## RSVP-TE: objects are "length, then type"

The `rsvp` layer is the common header (`version`, `flags`, `msg_type`, `checksum`, `send_ttl`, `length`) and every object of the message (`objects`). An object is a 16-bit length (counting the 4-octet header) followed by the Class-Num and C-Type, read together as one 16-bit type (`class_num << 8 | c_type`; wpg-core `TlvDef::len_first`). Item names combine the class and the C-Type.

| Type | Name (`name`) | Type | Name (`name`) |
|---|---|---|---|
| 0x0101 / 0x0102 | `session_ipv4` / `session_ipv6` | 0x0b01 | `sender_template_ipv4` |
| 0x0107 / 0x0108 | `session_lsp_tunnel_ipv4` / `_ipv6` | 0x0b07 / 0x0b08 | `sender_template_lsp_tunnel_ipv4` / `_ipv6` |
| 0x0301 / 0x0302 | `rsvp_hop_ipv4` / `rsvp_hop_ipv6` | 0x0c02 | `sender_tspec` (IntServ) |
| 0x0501 | `time_values` | 0x0f01 | `resv_confirm_ipv4` |
| 0x0601 / 0x0602 | `error_spec_ipv4` / `error_spec_ipv6` | 0x1001 | `label` |
| 0x0701 | `scope_ipv4` | 0x1301 | `label_request` (`l3pid`) |
| 0x0801 | `style` (`sharing`, `sender_selection`; default SE) | 0x1401 / 0x1501 | `explicit_route` / `record_route` (`subobjects`) |
| 0x0902 | `flowspec` (IntServ, controlled load) | 0x1601 / 0x1602 | `hello_request` / `hello_ack` |
| 0x0a01 | `filter_spec_ipv4` | 0x1701 / 0x1801 / 0x1802 | `message_id` / `message_id_ack` / `message_id_nack` (RFC 2961) |
| 0x0a07 / 0x0a08 | `filter_spec_lsp_tunnel_ipv4` / `_ipv6` | 0xcf07 / 0xcf01 | `session_attribute` / `session_attribute_ra` (with resource affinities) |

EXPLICIT_ROUTE subobjects are `ipv4_prefix`, `ipv6_prefix`, `unnumbered_interface` and `as_number`; the names ending in `_loose` have the L bit (loose hop). RECORD_ROUTE subobjects are `ipv4_address`, `ipv6_address`, `label` and `unnumbered_interface` (each with `flags`). ADSPEC, POLICY_DATA, INTEGRITY and other objects keep their value octets.

The token bucket of SENDER_TSPEC / FLOWSPEC (`token_bucket_rate`, `token_bucket_size`, `peak_data_rate`) is given as IEEE 754 single-precision bit patterns in 32-bit integers (1,000,000 octets/s = `0x49742400`, infinity = `0x7f800000`). The default is 125,000 octets/s (1 Mbit/s).

Path, PathTear and ResvConf are normally sent with the IP Router Alert option (`ipv4` `options: [router_alert: 0]`).

## BFD

The `bfd` layer is the mandatory section and the optional authentication section (`authentication`). The flags are the 1-bit fields `p`, `f`, `c`, `a`, `d` and `m`. Authentication types are simple password (`simple_password`: `key_id`, `password`), `keyed_md5` / `meticulous_keyed_md5` (`key_id`, `sequence_number`, a 16-octet `digest`) and `keyed_sha1` / `meticulous_keyed_sha1` (a 20-octet `hash`).

```yaml
- protocol: bfd
  fields:
    state: up
    my_discriminator: 0x101
    your_discriminator: 0x201
    desired_min_tx_interval: 300000 # microseconds
    required_min_rx_interval: 300000
    authentication:
      - simple_password: { key_id: 1, password: '"secret"' }
```

## Auto fields

| Field | Rule |
|---|---|
| `ldp.pdu_length` | PDU length after the version and PDU length fields |
| LDP message / TLV lengths | Value length (without the type and length) |
| `rsvp.checksum` | Internet checksum of the whole message with the field zeroed |
| `rsvp.length`, object / subobject lengths | Length including the header |
| SESSION_ATTRIBUTE `name_length` and `padding` | Octets of the name, zero padding to 4 octets (counted in the length) |
| `bfd.length` | Control packet length including the authentication section |
| `bfd.a` | 1 when there is an authentication section |

## Limitations

- No state: no LDP session setup, RSVP refresh, or BFD state machine and timers. Values are sent as given; use generators to vary them.
- BFD MD5 digests / SHA1 hashes and the RSVP INTEGRITY object are Manual (nothing is computed from a key).
- One LDP layer is one PDU; further PDUs in the same segment decode as `raw`.
- RSVP does not type the GMPLS objects (RFC 3473), ADSPEC or POLICY_DATA; RSVP over UDP (RFC 2205 appendix C) is out of scope.
- Only the first IntServ parameter (the token bucket) is typed; later ones (the guaranteed service rate / slack term, ...) are `parameters` octets.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`ipv4`](../reference/protocols/ipv4.md) · [`ldp`](../reference/protocols/ldp.md) · [`rsvp`](../reference/protocols/rsvp.md) · [`bfd`](../reference/protocols/bfd.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
