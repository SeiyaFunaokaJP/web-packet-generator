# SCTP (P2)

Status: draft

Notes on SCTP (RFC 9260) (M7). The definition is in `crates/wpg-protocols/src/sctp.rs`, the samples in `templates/protocols/l4/sctp-*.wpg.yaml`.

## How it is reached, and samples

```text
ethernet2 / ipv4 / sctp            # IPv4 protocol 132 (Auto)
ethernet2 / ipv6 / sctp            # IPv6 next header 132 (Auto)
ethernet2 / ipv4 / udp / sctp      # SCTP over UDP, port 9899 (RFC 6951)
```

| Sample | Contents |
|---|---|
| `sctp-init` | INIT with IPv4 address, supported address types, ECN, FORWARD TSN supported and supported extensions parameters |
| `sctp-init-ack` | INIT ACK with a state cookie |
| `sctp-data` | DATA with 16 octets of user data |
| `sctp-sack` | SACK with two gap ack blocks and a duplicate TSN |
| `sctp-heartbeat` | HEARTBEAT with heartbeat info |
| `sctp-abort` | ABORT (T bit) with a user initiated abort cause |
| `sctp-shutdown` | SHUTDOWN over IPv6 |
| `sctp-over-udp` | COOKIE ECHO bundled with DATA over UDP 9899 |

The UDP destination port is not Auto: set it to 9899 for SCTP over UDP (the parser also accepts 9899 as the source port).

## One layer, chunks as items

The `sctp` layer is the common header (`src_port`, `dst_port`, `verification_tag`, `checksum`) and every chunk of the packet in `chunks`. Several chunks can be bundled; there is no layer after `sctp`. The user data of DATA / I-DATA is the `user_data` octets of the chunk; it is not demultiplexed on the payload protocol identifier.

Chunks are items of 8-bit type, 8-bit flags and 16-bit length; the length counts the 4-octet chunk header but not the zero padding to a multiple of 4 octets, which is always added. Every chunk has a `flags` field (written between the type and the length):

```yaml
- protocol: sctp
  fields:
    verification_tag: 0x3a5c7e91
    chunks:
      - sack:
          cumulative_tsn_ack: 100
          gap_ack_blocks: [{ start: 2, end: 3 }]
      - name: data # `data` is a reserved item key: use {name, fields}
        fields:
          flags: b|e
          tsn: 101
          user_data: "68 65 6c 6c 6f"
      - abort: { flags: t }
```

| Type | Name (`name`) | Value fields (after `flags`) |
|---|---|---|
| 0 | `data` | flags `i`/`u`/`b`/`e`, `tsn`, `stream_identifier`, `stream_sequence_number`, `payload_protocol_identifier`, `user_data` |
| 1 / 2 | `init` / `init_ack` | `initiate_tag`, `a_rwnd`, `outbound_streams`, `inbound_streams`, `initial_tsn`, `parameters` |
| 3 | `sack` | `cumulative_tsn_ack`, `a_rwnd`, `number_of_gap_ack_blocks` / `number_of_duplicate_tsns` (Auto), `gap_ack_blocks` (`{start, end}`), `duplicate_tsns` |
| 4 / 5 | `heartbeat` / `heartbeat_ack` | `parameters` (`heartbeat_info`) |
| 6 | `abort` | flag `t`, `causes` |
| 7 | `shutdown` | `cumulative_tsn_ack` |
| 8 / 11 | `shutdown_ack` / `cookie_ack` | (flags only) |
| 9 | `error` | `causes` |
| 10 | `cookie_echo` | `cookie` |
| 14 | `shutdown_complete` | flag `t` |
| 15 | `auth` (RFC 4895) | `shared_key_identifier`, `hmac_identifier`, `hmac` |
| 0x40 | `i_data` (RFC 8260) | flags `i`/`u`/`b`/`e`, `tsn`, `stream_identifier`, `message_identifier`, `ppid_or_fsn`, `user_data` |
| 0x80 / 0xc1 | `asconf_ack` / `asconf` (RFC 5061) | `serial_number`, `parameters` |
| 0x82 | `re_config` (RFC 6525) | `parameters` |
| 0x84 | `pad` (RFC 4820) | `padding` |
| 0xc0 | `forward_tsn` (RFC 3758) | `new_cumulative_tsn`, `streams` (`{stream, stream_sequence}`) |

Other chunk types are kept as `{ type: N, data: "..." }`; the first `data` octet is the flags.

## Parameters and error causes

Parameters (INIT, INIT ACK, HEARTBEAT, ASCONF, RE-CONFIG) and error causes (ABORT, ERROR) are items of 16-bit type and 16-bit length, padded to 4 octets. As RFC 9260 §3.2 requires, the padding of the **last** parameter / cause is not counted in the chunk length: the chunk padding covers it.

| Parameters | Error causes |
|---|---|
| `ipv4_address`, `ipv6_address`, `heartbeat_info`, `state_cookie`, `unrecognized_parameter`, `cookie_preservative`, `host_name_address`, `supported_address_types` (`[ipv4, ipv6]`), `ecn_capable`, `random`, `chunk_list`, `requested_hmac_algorithm`, `padding`, `supported_extensions` (chunk types), `forward_tsn_supported`, `add_ip_address` / `delete_ip_address` / `set_primary_address` (`correlation_id`, `address`: an address parameter), `error_cause_indication`, `success_indication`, `adaptation_layer_indication`, the RE-CONFIG requests / response | `invalid_stream_identifier`, `missing_mandatory_parameter` (count Auto), `stale_cookie`, `out_of_resource`, `unresolvable_address`, `unrecognized_chunk_type`, `invalid_mandatory_parameter`, `unrecognized_parameters`, `no_user_data`, `cookie_received_while_shutting_down`, `restart_with_new_addresses`, `user_initiated_abort`, `protocol_violation`, the ASCONF causes (`0x00a0`-`0x00a4`), `unsupported_hmac_identifier` |

## Auto fields

| Field | Rule |
|---|---|
| `checksum` | CRC32c of the whole SCTP packet with the field zeroed (RFC 9260 appendix A), sent least significant octet first |
| chunk / parameter / cause `length` | Size without the padding; the zero padding itself |
| `number_of_gap_ack_blocks`, `number_of_duplicate_tsns` (SACK) | Size of the lists |
| `number_of_missing_parameters` | Size of `missing_parameters` |

## Limits

- A received chunk whose length also counts the padding of its last parameter (some implementations do) decodes the same, with an empty item after that parameter that keeps the padding where it was.
- No association state: TSNs, stream sequence numbers and tags are values (use generators to vary them).
- Unknown chunk types decode with their flags as the first `data` octet; the "action" bits of the type are not interpreted.
