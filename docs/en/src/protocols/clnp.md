# CLNP / ES-IS (P2)

Status: draft

Notes on CLNP (ISO 8473-1) and ES-IS (ISO 9542) (M7). The definitions are in `crates/wpg-protocols/src/clnp.rs`, the samples in `templates/protocols/l3/clnp-*.wpg.yaml` and `templates/protocols/routing/esis-*.wpg.yaml`. Both run directly over the [OSI encapsulation](osi.md).

## PDUs, how they are reached, and samples

| id | PDU | Samples |
|---|---|---|
| `clnp` | CLNP, every PDU type: DT (`0x1c`), MD (`0x1d`), ER (`0x01`), ERQ / ERP (`0x1e` / `0x1f`) | `clnp-dt` (segmentation part), `clnp-er` (reason for discard), `clnp-erq` (priority, source routing) |
| `esis_esh` | ES-IS End System Hello (2) | `esis-esh` |
| `esis_ish` | ES-IS Intermediate System Hello (4) | `esis-ish` (ES configuration timer) |
| `esis_rd` | ES-IS Redirect (6) | `esis-rd` |
| `esis` | Any ES-IS PDU: fixed part + opaque octets | `esis-generic` |

```text
dot3 / llc / clnp / payload        # DT carrying a transport PDU
dot3 / llc / clnp / raw            # ER carrying the discarded header
dot3 / llc / esis_esh              # to 09:00:2b:00:00:05 (AllIntermediateSystems)
dot3 / llc / esis_ish              # to 09:00:2b:00:00:04 (AllEndSystems)
ppp / clnp / payload               # PPP protocol 0x0023
```

The LLC DSAP / SSAP (`0xfe`) and the PPP protocol (`0x0023`) are Auto. When parsing, the NLPID `0x81` selects `clnp` and `0x82` the generic `esis`, whose `pdu_type` then re-decodes the layer as the dedicated PDU; unknown ES-IS types stay `esis`. PDUs shorter than the fixed part (CLNP 11 octets, ES-IS 9) decode as the generic `osi`.

The `clnp` layer is the header only. Its data (DT: the transport PDU; ER: the header of the discarded PDU; ERQ / ERP: the echoed PDU) is the next layer, typically `payload` or `raw`.

## Auto fields

| Field | Rule |
|---|---|
| `length_indicator` | Header length in octets (CLNP: up to the end of the options; ES-IS: the whole PDU) |
| `segment_length` (CLNP) | Length of the whole PDU (header + data) |
| `checksum` | ISO 8473 Fletcher checksum of the header (CLNP) / of the PDU (ES-IS). A Manual `0` means "checksum not used"; it is sent as is and reported as differing from Auto (a computed checksum is never 0) |
| `sp` (CLNP) | 1 when the `segmentation` part has an element, else 0 |
| `number_of_source_addresses` (ESH) | Number of `source_addresses` |
| `bsnpa_length` (RD) | Size of `bsnpa` |

The CLNP segmentation part is a list of at most one `{data_unit_identifier, segment_offset, total_length}`; `total_length` (the length of the unsegmented PDU) is not computed:

```yaml
- protocol: clnp
  fields:
    segmentation:
      - { data_unit_identifier: 1, segment_offset: 0, total_length: 69 }
```

## Options

Options are `{code, length, value}` items after the addresses (and the segmentation part); they end at the `length_indicator` (`Length::UntilOffset`), so whatever follows the header is the next layer even though the addresses have variable size.

| Code | Name (`name`) | Value fields | In |
|---|---|---|---|
| `0xc5` | `security` | `value` (octets) | CLNP, ES-IS |
| `0xc3` | `qos_maintenance` | `value` (octets) | CLNP, ES-IS |
| `0xcd` | `priority` | `value` | CLNP, ES-IS |
| `0xc8` / `0xcb` | `source_routing` / `recording_of_route` | `route_type` (`partial` / `complete`), `next_address`, `addresses`: list of `{address}` | CLNP |
| `0xcc` | `padding` | `data` | CLNP |
| `0xc1` | `reason_for_discard` | `reason` (`destination_unknown`, `lifetime_expired_in_transit`, ...), `pointer` | CLNP (ER) |
| `0xe1` / `0xe2` | `address_mask` / `snpa_mask` | `mask` | ES-IS |
| `0xc6` | `es_configuration_timer` | `seconds` | ES-IS (ISH) |

Other options are kept as `{ type: N, data: "..." }`.

## Limits

- The segmentation `total_length` is not computed, and no segmentation / reassembly of a large PDU into several frames is done.
- Security and quality of service maintenance values are octets; the globally unique / source address specific formats are not split.
- A Redirect's subnetwork address (`bsnpa`) is octets (a MAC address is 6 octets: `"02 00 00 00 00 02"`). An empty `net` (`""`) redirects to an end system.
