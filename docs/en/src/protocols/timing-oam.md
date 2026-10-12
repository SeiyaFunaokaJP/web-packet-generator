# PTP / ESMC / CFM / R-APS / 802.3ah OAM (P2)

Status: draft

Notes on PTP (IEEE 1588-2008), SyncE ESMC (ITU-T G.8264), Ethernet CFM / OAM (IEEE 802.1Q clause 21 = 802.1ag, ITU-T Y.1731) with the ERPS R-APS messages (ITU-T G.8032) and EFM OAM (IEEE 802.3 clause 57 = 802.3ah) (M7). The definitions live in `crates/wpg-protocols/src/{ptp,esmc,cfm,efm_oam}.rs`, the samples in `templates/protocols/timing/` and `templates/protocols/oam/`.

## Stacks and samples

```text
ethernet2 / ptp_*                         # EtherType 0x88f7 (IEEE 1588 annex F)
ethernet2 / ipv4|ipv6 / udp / ptp_*       # UDP 319 (event messages), 320 (general messages)
ethernet2 / esmc                          # Slow Protocols 0x8809, subtype 0x0a
ethernet2 [/ dot1q] / cfm_*               # EtherType 0x8902, also behind 802.1Q / 802.1ad tags
ethernet2 / efm_oam_*                     # Slow Protocols 0x8809, subtype 0x03
```

The Slow Protocols EtherType `0x8809` is demultiplexed by its first octet, the subtype: 1 LACP, 2 Marker, 3 OAM, 0x0a ESMC (`l2control::slow_protocols_peek`; other subtypes decode as `raw`). Marker PDUs now decode as `marker` instead of `lacp`. UDP destination ports are not Auto: set 319 for Sync / Delay_Req / Pdelay_Req / Pdelay_Resp and 320 for the other messages.

| Sample | Contents |
|---|---|
| `ptp-sync-two-step-l2` | Two-step Sync over Ethernet, 8 per second, incrementing sequence ID |
| `ptp-follow-up-l2` | Follow_Up with the precise origin timestamp and a correction |
| `ptp-pdelay-req-l2` / `ptp-pdelay-resp-l2` / `ptp-pdelay-resp-follow-up-l2` | Peer delay exchange of IEEE 802.1AS (majorSdoId 1) to `01:80:c2:00:00:0e` |
| `ptp-delay-req-udp` / `ptp-delay-resp-udp` | Delay request / response over UDP/IPv4 (224.0.1.129) |
| `ptp-announce-udp` | Announce of a GPS-locked grandmaster (clock class 6) |
| `ptp-signaling-unicast` | Unicast negotiation: Request Unicast Transmission TLVs for Announce and Sync |
| `ptp-management-get` | Management GET of the default data set |
| `ptp-generic` | A Sync written in the generic layout |
| `esmc-ql-prc` / `esmc-event-dnu` | ESMC information PDU (QL-PRC, Extended QL TLV) / event PDU (QL-DNU) |
| `cfm-ccm` | CCM on VLAN 100 (MD level 5, MEP 10, ICC-based MEG ID, Port / Interface Status TLVs) |
| `cfm-lbm` / `cfm-lbr` | Loopback message / reply with a Data TLV |
| `cfm-ltm` / `cfm-ltr` | Linktrace message / reply with their egress identifier and Reply Ingress TLVs |
| `cfm-generic` | A G.8031 Linear APS (OpCode 39) in the generic layout |
| `y1731-ais` / `y1731-lck` / `y1731-csf` | AIS, LCK and CSF signals |
| `y1731-tst` | Test signal with a Test TLV |
| `y1731-lmm` / `y1731-lmr` / `y1731-slm` / `y1731-slr` / `y1731-1sl` | Loss measurement (frame counters, synthetic loss) |
| `y1731-dmm` / `y1731-dmr` / `y1731-1dm` | Delay measurement (IEEE 1588 timestamps) |
| `erps-raps-nr-rb` / `erps-raps-sf` | R-APS(NR, RB) of the RPL owner / R-APS(SF) after a failure |
| `efm-oam-information` / `efm-oam-generic` | Information OAMPDU (Local / Remote Information TLVs) |
| `efm-oam-event` | Event Notification (Errored Frame and Errored Symbol Period events) |
| `efm-oam-variable-request` / `efm-oam-variable-response` | Variable retrieval |
| `efm-oam-loopback` | Loopback Control (enable remote loopback) |
| `efm-oam-organization-specific` | Organization Specific OAMPDU |

## Generic layer and dedicated messages

PTP, CFM and 802.3ah OAM follow the IS-IS pattern: one protocol per message (PTP), OpCode (CFM) or code (OAM), plus a generic layer (`ptp`, `cfm`, `efm_oam`: the common header and the rest as `data` octets). The parser decodes the generic layer first and re-decodes it as the dedicated one from `message_type` / `opcode` / `code`; values without a dedicated definition (CFM Linear APS, MCC, EXM / VSM, ...) stay generic. When building, every dedicated protocol sets the EtherType / subtype by itself.

| Protocol | `message_type` | Body |
|---|---|---|
| `ptp_sync` / `ptp_delay_req` | 0 / 1 | `origin_timestamp_{seconds,nanoseconds}` |
| `ptp_pdelay_req` | 2 | origin timestamp, 10 reserved octets |
| `ptp_pdelay_resp` | 3 | `request_receipt_timestamp_*`, `requesting_port_{clock_identity,port_number}` |
| `ptp_follow_up` | 8 | `precise_origin_timestamp_*` |
| `ptp_delay_resp` | 9 | `receive_timestamp_*`, `requesting_port_*` |
| `ptp_pdelay_resp_follow_up` | 10 | `response_origin_timestamp_*`, `requesting_port_*` |
| `ptp_announce` | 11 | origin timestamp, `current_utc_offset`, `grandmaster_priority1/2`, `grandmaster_clock_{class,accuracy}`, `grandmaster_offset_scaled_log_variance`, `grandmaster_identity`, `steps_removed`, `time_source` |
| `ptp_signaling` | 12 | `target_port_*` |
| `ptp_management` | 13 | `target_port_*`, `starting_boundary_hops`, `boundary_hops`, `action` |

## PTP

The 34-octet common header has `major_sdo_id` (transportSpecific), `message_type`, `minor_version`, `version`, `message_length` (Auto), `domain_number`, `minor_sdo_id`, `flags` (16 bits: `two_step` is bit 9, `unicast` bit 10, `ptp_timescale` bit 3, ...), `correction_field` (signed, nanoseconds × 2^16), `message_type_specific`, the source port identity (`clock_identity`, `port_number`), `sequence_id`, `control_field` and `log_message_interval` (signed). The defaults of `control_field` and `log_message_interval` follow the message (0x7f where not applicable).

Every message ends with `tlvs` (16-bit type, 16-bit length of the value, Auto). They end at `message_length`, so the zero padding of a short Ethernet frame is not read as TLVs.

```yaml
- protocol: ptp_signaling
  fields:
    flags: 0x0400 # unicast
    tlvs:
      - request_unicast_transmission: { message_type: announce, log_inter_message_period: 1, duration_field: 300 }
      - grant_unicast_transmission: { message_type: sync, log_inter_message_period: -4, duration_field: 300, renewal_invited: 1 }
```

| Type | TLV (`name`) | Value fields |
|---|---|---|
| 0x0001 | `management` | `management_id`, `data_field` (octets) |
| 0x0002 | `management_error_status` | `management_error_id`, `management_id`, `reserved`, `display_data` |
| 0x0003 | `organization_extension` | `organization_id`, `organization_sub_type`, `data_field` |
| 0x0004 / 0x0005 | `request_unicast_transmission` / `grant_unicast_transmission` | `message_type`, `log_inter_message_period`, `duration_field` (grant: `renewal_invited`) |
| 0x0006 / 0x0007 | `cancel_unicast_transmission` / `acknowledge_cancel_unicast_transmission` | `message_type` |
| 0x0008 | `path_trace` | `path_sequence` (list of clock identities) |
| 0x0009 | `alternate_time_offset_indicator` | `key_field`, `current_offset`, `jump_seconds`, `time_of_next_jump`, `display_name` (length Auto) |

## ESMC

`esmc` is the OSSP header (`subtype` 0x0a, `itu_oui` 00-19-a7, `itu_subtype` 1), `version` (1), `event_flag` and the TLVs (8-bit type, 16-bit length including the 3-octet header, Auto): `ql` (`ssm_code`: `prc`, `ssu_a`, `ssu_b`, `sec`, `dnu`, and the option 2 codes) and `extended_ql` (`enhanced_ssm_code`, `synce_clock_identity`, `flags` (mixed EEC / eEEC, partial chain), `cascaded_eeecs`, `cascaded_eecs`). The padding after the TLVs decodes as one raw item.

## CFM / Y.1731 / R-APS

The common header is `md_level`, `version`, `opcode`, the flags and `first_tlv_offset`; the flags are split per OpCode (CCM: `rdi`, `period`; LTM / LTR: `use_fdb_only`, `fwd_yes`, `terminal_mep`; AIS / LCK / CSF: `period`, CSF `type`; others: `flags`). `first_tlv_offset` is constant per OpCode (a warning when changed). The TLVs end with the End TLV (`end`, type 0); the Ethernet padding after it is padding, not more End TLVs (wpg-core `Length::UntilTlv`).

| OpCode | Protocol | Fields |
|---|---|---|
| 1 | `cfm_ccm` | `sequence_number`, `mep_id`, `maid` (48 octets), `tx_fcf`, `rx_fcb`, `tx_fcb` |
| 2 / 3 | `cfm_lbr` / `cfm_lbm` | `transaction_id` |
| 4 | `cfm_ltr` | `transaction_id`, `reply_ttl`, `relay_action` |
| 5 | `cfm_ltm` | `transaction_id`, `ttl`, `original_mac`, `target_mac` |
| 33 / 35 | `cfm_ais` / `cfm_lck` | (period only) |
| 37 | `cfm_tst` | `sequence_number` |
| 40 | `cfm_raps` | `request_state`, `sub_code`, `rb`, `dnf`, `bpr`, `node_id` |
| 42 / 43 | `cfm_lmr` / `cfm_lmm` | `tx_fcf`, `rx_fcf`, `tx_fcb` |
| 45 | `cfm_1dm` | `tx_timestamp_f_*`, `rx_timestamp_f_*` |
| 46 / 47 | `cfm_dmr` / `cfm_dmm` | `tx_timestamp_f_*`, `rx_timestamp_f_*`, `tx_timestamp_b_*`, `rx_timestamp_b_*` |
| 52 | `cfm_csf` | (type and period) |
| 53 | `cfm_1sl` | `source_mep_id`, `test_id`, `tx_fcf` |
| 54 / 55 | `cfm_slr` / `cfm_slm` | `source_mep_id`, `responder_mep_id`, `test_id`, `tx_fcf`, `tx_fcb` |

TLVs (8-bit type, 16-bit length of the value, Auto): `end` (0), `sender_id` (1: `chassis_id_length` Auto, `chassis_id_subtype`, `chassis_id`, `management_address` octets), `port_status` (2), `data_tlv` (3: `value`), `interface_status` (4), `reply_ingress` / `reply_egress` (5 / 6: action, MAC, port ID octets), `ltm_egress_identifier` (7), `ltr_egress_identifier` (8), `organization_specific` (31), `test` (32: `pattern_type`, `test_pattern`).

R-APS (`cfm_raps`, OpCode 40) defaults to version 1 (G.8032v2), `first_tlv_offset` 32 and R-APS(NR, RB). Send it to `01:19:a7:00:00:<ring ID>`, usually on the R-APS VLAN at MEL 7.

## 802.3ah OAM

Every OAMPDU starts with `subtype` (3), `flags` (link fault, dying gasp, critical event, local / remote evaluating and stable; default 0x0050) and `code`.

| Code | Protocol | Fields |
|---|---|---|
| 0x00 | `efm_oam_information` | `tlvs`: `local_information` / `remote_information` (`oam_version`, `revision`, `multiplexer_action`, `parser_action`, `oam_configuration`, `max_oampdu_size`, `oui`, `vendor_specific_information`), `organization_specific`, `end` |
| 0x01 | `efm_oam_event_notification` | `sequence_number`, `tlvs`: `errored_symbol_period`, `errored_frame`, `errored_frame_period`, `errored_frame_seconds_summary` (`timestamp`, `window`, `threshold`, `errors`, `error_running_total`, `event_running_total`), `organization_specific`, `end` |
| 0x02 | `efm_oam_variable_request` | `variable_descriptors` (octets: branch, leaf, ..., zero branch) |
| 0x03 | `efm_oam_variable_response` | `variable_containers` (octets: branch, leaf, width, value, ..., zero branch) |
| 0x04 | `efm_oam_loopback_control` | `remote_loopback_command` (1 enable, 2 disable) |
| 0xfe | `efm_oam_organization_specific` | `oui`, `data` |

Information / Event TLV lengths include the 2-octet header (Auto) and the list ends with `end`.

## Not covered

- PTP: the authentication / security TLVs (IEEE 1588-2019 annex P), the data fields of the Management TLVs per management ID, PTP over other transports (annexes C, E-G except IPv6 / Ethernet), state machines and timestamping on transmit.
- CFM: the MAID / MEG ID is the 48 octets as sent (not split into name formats), the CRC-32 of the Test TLV is not computed; Linear APS, MCC, EXM / EXR and VSM / VSR have no dedicated definition (generic `cfm`).
- 802.3ah OAM: variable descriptors / containers are octets; the OAM discovery state machine.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`lacp`](../reference/protocols/lacp.md) · [`marker`](../reference/protocols/marker.md) · [`ptp`](../reference/protocols/ptp.md) · [`ptp_sync`](../reference/protocols/ptp_sync.md) · [`ptp_delay_req`](../reference/protocols/ptp_delay_req.md) · [`ptp_pdelay_req`](../reference/protocols/ptp_pdelay_req.md) · [`ptp_pdelay_resp`](../reference/protocols/ptp_pdelay_resp.md) · [`ptp_follow_up`](../reference/protocols/ptp_follow_up.md) · [`ptp_delay_resp`](../reference/protocols/ptp_delay_resp.md) · [`ptp_pdelay_resp_follow_up`](../reference/protocols/ptp_pdelay_resp_follow_up.md) · [`ptp_announce`](../reference/protocols/ptp_announce.md) · [`ptp_signaling`](../reference/protocols/ptp_signaling.md) · [`ptp_management`](../reference/protocols/ptp_management.md) · [`esmc`](../reference/protocols/esmc.md) · [`cfm`](../reference/protocols/cfm.md) · [`cfm_ccm`](../reference/protocols/cfm_ccm.md) · [`cfm_lbm`](../reference/protocols/cfm_lbm.md) · [`cfm_lbr`](../reference/protocols/cfm_lbr.md) · [`cfm_ltm`](../reference/protocols/cfm_ltm.md) · [`cfm_ltr`](../reference/protocols/cfm_ltr.md) · [`cfm_ais`](../reference/protocols/cfm_ais.md) · [`cfm_lck`](../reference/protocols/cfm_lck.md) · [`cfm_tst`](../reference/protocols/cfm_tst.md) · [`cfm_raps`](../reference/protocols/cfm_raps.md) · [`cfm_lmm`](../reference/protocols/cfm_lmm.md) · [`cfm_lmr`](../reference/protocols/cfm_lmr.md) · [`cfm_1dm`](../reference/protocols/cfm_1dm.md) · [`cfm_dmm`](../reference/protocols/cfm_dmm.md) · [`cfm_dmr`](../reference/protocols/cfm_dmr.md) · [`cfm_csf`](../reference/protocols/cfm_csf.md) · [`cfm_slm`](../reference/protocols/cfm_slm.md) · [`cfm_slr`](../reference/protocols/cfm_slr.md) · [`cfm_1sl`](../reference/protocols/cfm_1sl.md) · [`efm_oam`](../reference/protocols/efm_oam.md) · [`efm_oam_information`](../reference/protocols/efm_oam_information.md) · [`efm_oam_event_notification`](../reference/protocols/efm_oam_event_notification.md) · [`efm_oam_variable_request`](../reference/protocols/efm_oam_variable_request.md) · [`efm_oam_variable_response`](../reference/protocols/efm_oam_variable_response.md) · [`efm_oam_loopback_control`](../reference/protocols/efm_oam_loopback_control.md) · [`efm_oam_organization_specific`](../reference/protocols/efm_oam_organization_specific.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
