# PTP / ESMC / CFM / R-APS / 802.3ah OAM (P2)

Status: draft

PTP (IEEE 1588-2008)、SyncE ESMC (ITU-T G.8264)、Ethernet CFM / OAM (IEEE 802.1Q 21 章 = 802.1ag、ITU-T Y.1731) と ERPS の R-APS メッセージ (ITU-T G.8032)、EFM OAM (IEEE 802.3 57 章 = 802.3ah) の補足 (M7)。定義は `crates/wpg-protocols/src/{ptp,esmc,cfm,efm_oam}.rs`、サンプルは `templates/protocols/timing/` と `templates/protocols/oam/` にあります。

## スタックとサンプル

```text
ethernet2 / ptp_*                         # EtherType 0x88f7 (IEEE 1588 附属書 F)
ethernet2 / ipv4|ipv6 / udp / ptp_*       # UDP 319 (イベントメッセージ)、320 (一般メッセージ)
ethernet2 / esmc                          # Slow Protocols 0x8809、サブタイプ 0x0a
ethernet2 [/ dot1q] / cfm_*               # EtherType 0x8902。802.1Q / 802.1ad タグの内側も可
ethernet2 / efm_oam_*                     # Slow Protocols 0x8809、サブタイプ 0x03
```

Slow Protocols の EtherType `0x8809` は先頭オクテット (サブタイプ) で振り分けます: 1 LACP、2 Marker、3 OAM、0x0a ESMC (`l2control::slow_protocols_peek`。その他のサブタイプは `raw`)。Marker PDU は `lacp` ではなく `marker` としてデコードされるようになりました。UDP の宛先ポートは自動ではありません。Sync / Delay_Req / Pdelay_Req / Pdelay_Resp は 319、その他は 320 を指定します。

| サンプル | 内容 |
|---|---|
| `ptp-sync-two-step-l2` | Ethernet 上の 2 ステップ Sync (毎秒 8 回、シーケンス ID を加算) |
| `ptp-follow-up-l2` | 正確な送信時刻と補正値付きの Follow_Up |
| `ptp-pdelay-req-l2` / `ptp-pdelay-resp-l2` / `ptp-pdelay-resp-follow-up-l2` | `01:80:c2:00:00:0e` 宛ての IEEE 802.1AS (majorSdoId 1) ピア遅延測定 |
| `ptp-delay-req-udp` / `ptp-delay-resp-udp` | UDP/IPv4 (224.0.1.129) 上の遅延要求 / 応答 |
| `ptp-announce-udp` | GPS 同期のグランドマスタ (クロッククラス 6) の Announce |
| `ptp-signaling-unicast` | ユニキャストネゴシエーション: Announce と Sync の Request Unicast Transmission TLV |
| `ptp-management-get` | DEFAULT_DATA_SET の Management GET |
| `ptp-generic` | 汎用レイアウトで書いた Sync |
| `esmc-ql-prc` / `esmc-event-dnu` | ESMC 情報 PDU (QL-PRC、拡張 QL TLV) / イベント PDU (QL-DNU) |
| `cfm-ccm` | VLAN 100 上の CCM (MD レベル 5、MEP 10、ICC 形式 MEG ID、Port / Interface Status TLV) |
| `cfm-lbm` / `cfm-lbr` | Data TLV 付きのループバックメッセージ / 応答 |
| `cfm-ltm` / `cfm-ltr` | Egress Identifier / Reply Ingress TLV 付きのリンクトレースメッセージ / 応答 |
| `cfm-generic` | 汎用レイアウトの G.8031 Linear APS (OpCode 39) |
| `y1731-ais` / `y1731-lck` / `y1731-csf` | AIS、LCK、CSF 信号 |
| `y1731-tst` | Test TLV 付きのテスト信号 |
| `y1731-lmm` / `y1731-lmr` / `y1731-slm` / `y1731-slr` / `y1731-1sl` | ロス測定 (フレームカウンタ、合成ロス) |
| `y1731-dmm` / `y1731-dmr` / `y1731-1dm` | 遅延測定 (IEEE 1588 形式のタイムスタンプ) |
| `erps-raps-nr-rb` / `erps-raps-sf` | RPL オーナーの R-APS(NR, RB) / 障害時の R-APS(SF) |
| `efm-oam-information` / `efm-oam-generic` | Information OAMPDU (Local / Remote Information TLV) |
| `efm-oam-event` | Event Notification (Errored Frame と Errored Symbol Period イベント) |
| `efm-oam-variable-request` / `efm-oam-variable-response` | 変数の取得 |
| `efm-oam-loopback` | Loopback Control (リモートループバックの有効化) |
| `efm-oam-organization-specific` | Organization Specific OAMPDU |

## 汎用レイヤと専用メッセージ

PTP、CFM、802.3ah OAM は IS-IS と同じ方式です: メッセージタイプ (PTP)、OpCode (CFM)、コード (OAM) ごとに 1 プロトコルと、汎用レイヤ (`ptp`、`cfm`、`efm_oam`: 共通ヘッダと残りを `data` オクテット列で) があります。パーサーはまず汎用レイヤとしてデコードし、`message_type` / `opcode` / `code` から専用レイヤに読み直します。専用定義のない値 (CFM の Linear APS、MCC、EXM / VSM など) は汎用のままです。合成時は専用プロトコルを置くだけで EtherType / サブタイプが決まります。

| プロトコル | `message_type` | 本体 |
|---|---|---|
| `ptp_sync` / `ptp_delay_req` | 0 / 1 | `origin_timestamp_{seconds,nanoseconds}` |
| `ptp_pdelay_req` | 2 | 送信時刻、予約 10 オクテット |
| `ptp_pdelay_resp` | 3 | `request_receipt_timestamp_*`、`requesting_port_{clock_identity,port_number}` |
| `ptp_follow_up` | 8 | `precise_origin_timestamp_*` |
| `ptp_delay_resp` | 9 | `receive_timestamp_*`、`requesting_port_*` |
| `ptp_pdelay_resp_follow_up` | 10 | `response_origin_timestamp_*`、`requesting_port_*` |
| `ptp_announce` | 11 | 送信時刻、`current_utc_offset`、`grandmaster_priority1/2`、`grandmaster_clock_{class,accuracy}`、`grandmaster_offset_scaled_log_variance`、`grandmaster_identity`、`steps_removed`、`time_source` |
| `ptp_signaling` | 12 | `target_port_*` |
| `ptp_management` | 13 | `target_port_*`、`starting_boundary_hops`、`boundary_hops`、`action` |

## PTP

34 オクテットの共通ヘッダは `major_sdo_id` (transportSpecific)、`message_type`、`minor_version`、`version`、`message_length` (自動)、`domain_number`、`minor_sdo_id`、`flags` (16 ビット: `two_step` はビット 9、`unicast` はビット 10、`ptp_timescale` はビット 3 など)、`correction_field` (符号付き、ナノ秒 × 2^16)、`message_type_specific`、送信元ポート ID (`clock_identity`、`port_number`)、`sequence_id`、`control_field`、`log_message_interval` (符号付き) です。`control_field` と `log_message_interval` の既定値はメッセージに合わせてあります (該当しない場合は 0x7f)。

各メッセージの最後は `tlvs` (16 ビット型、値の 16 ビット長、自動) です。TLV は `message_length` で終わるため、短い Ethernet フレームのゼロ詰めを TLV と誤読しません。

```yaml
- protocol: ptp_signaling
  fields:
    flags: 0x0400 # unicast
    tlvs:
      - request_unicast_transmission: { message_type: announce, log_inter_message_period: 1, duration_field: 300 }
      - grant_unicast_transmission: { message_type: sync, log_inter_message_period: -4, duration_field: 300, renewal_invited: 1 }
```

| タイプ | TLV (`name`) | 値のフィールド |
|---|---|---|
| 0x0001 | `management` | `management_id`、`data_field` (オクテット列) |
| 0x0002 | `management_error_status` | `management_error_id`、`management_id`、`reserved`、`display_data` |
| 0x0003 | `organization_extension` | `organization_id`、`organization_sub_type`、`data_field` |
| 0x0004 / 0x0005 | `request_unicast_transmission` / `grant_unicast_transmission` | `message_type`、`log_inter_message_period`、`duration_field` (grant は `renewal_invited` も) |
| 0x0006 / 0x0007 | `cancel_unicast_transmission` / `acknowledge_cancel_unicast_transmission` | `message_type` |
| 0x0008 | `path_trace` | `path_sequence` (クロック ID のリスト) |
| 0x0009 | `alternate_time_offset_indicator` | `key_field`、`current_offset`、`jump_seconds`、`time_of_next_jump`、`display_name` (長さは自動) |

## ESMC

`esmc` は OSSP ヘッダ (`subtype` 0x0a、`itu_oui` 00-19-a7、`itu_subtype` 1)、`version` (1)、`event_flag` と TLV (8 ビット型、3 オクテットのヘッダを含む 16 ビット長、自動) です: `ql` (`ssm_code`: `prc`、`ssu_a`、`ssu_b`、`sec`、`dnu` とオプション 2 のコード)、`extended_ql` (`enhanced_ssm_code`、`synce_clock_identity`、`flags` (EEC / eEEC 混在、部分チェーン)、`cascaded_eeecs`、`cascaded_eecs`)。TLV の後の詰め物は 1 つの raw 項目としてデコードされます。

## CFM / Y.1731 / R-APS

共通ヘッダは `md_level`、`version`、`opcode`、フラグ、`first_tlv_offset` です。フラグは OpCode ごとに分けてあります (CCM: `rdi`、`period`、LTM / LTR: `use_fdb_only`、`fwd_yes`、`terminal_mep`、AIS / LCK / CSF: `period`、CSF の `type`、その他: `flags`)。`first_tlv_offset` は OpCode ごとに一定です (変えると警告)。TLV は End TLV (`end`、タイプ 0) で終わり、その後の Ethernet の詰め物は End TLV の繰り返しではなく詰め物として扱います (wpg-core の `Length::UntilTlv`)。

| OpCode | プロトコル | フィールド |
|---|---|---|
| 1 | `cfm_ccm` | `sequence_number`、`mep_id`、`maid` (48 オクテット)、`tx_fcf`、`rx_fcb`、`tx_fcb` |
| 2 / 3 | `cfm_lbr` / `cfm_lbm` | `transaction_id` |
| 4 | `cfm_ltr` | `transaction_id`、`reply_ttl`、`relay_action` |
| 5 | `cfm_ltm` | `transaction_id`、`ttl`、`original_mac`、`target_mac` |
| 33 / 35 | `cfm_ais` / `cfm_lck` | (周期のみ) |
| 37 | `cfm_tst` | `sequence_number` |
| 40 | `cfm_raps` | `request_state`、`sub_code`、`rb`、`dnf`、`bpr`、`node_id` |
| 42 / 43 | `cfm_lmr` / `cfm_lmm` | `tx_fcf`、`rx_fcf`、`tx_fcb` |
| 45 | `cfm_1dm` | `tx_timestamp_f_*`、`rx_timestamp_f_*` |
| 46 / 47 | `cfm_dmr` / `cfm_dmm` | `tx_timestamp_f_*`、`rx_timestamp_f_*`、`tx_timestamp_b_*`、`rx_timestamp_b_*` |
| 52 | `cfm_csf` | (種類と周期) |
| 53 | `cfm_1sl` | `source_mep_id`、`test_id`、`tx_fcf` |
| 54 / 55 | `cfm_slr` / `cfm_slm` | `source_mep_id`、`responder_mep_id`、`test_id`、`tx_fcf`、`tx_fcb` |

TLV (8 ビット型、値の 16 ビット長、自動): `end` (0)、`sender_id` (1: `chassis_id_length` は自動、`chassis_id_subtype`、`chassis_id`、`management_address` オクテット列)、`port_status` (2)、`data_tlv` (3: `value`)、`interface_status` (4)、`reply_ingress` / `reply_egress` (5 / 6: 動作、MAC、ポート ID のオクテット列)、`ltm_egress_identifier` (7)、`ltr_egress_identifier` (8)、`organization_specific` (31)、`test` (32: `pattern_type`、`test_pattern`)。

R-APS (`cfm_raps`、OpCode 40) の既定はバージョン 1 (G.8032v2)、`first_tlv_offset` 32、R-APS(NR, RB) です。宛先は `01:19:a7:00:00:<リング ID>`、通常は R-APS 用 VLAN 上の MEL 7 で送ります。

## 802.3ah OAM

OAMPDU は `subtype` (3)、`flags` (リンク障害、Dying Gasp、重大イベント、ローカル / リモートの評価中・安定。既定 0x0050)、`code` で始まります。

| コード | プロトコル | フィールド |
|---|---|---|
| 0x00 | `efm_oam_information` | `tlvs`: `local_information` / `remote_information` (`oam_version`、`revision`、`multiplexer_action`、`parser_action`、`oam_configuration`、`max_oampdu_size`、`oui`、`vendor_specific_information`)、`organization_specific`、`end` |
| 0x01 | `efm_oam_event_notification` | `sequence_number`、`tlvs`: `errored_symbol_period`、`errored_frame`、`errored_frame_period`、`errored_frame_seconds_summary` (`timestamp`、`window`、`threshold`、`errors`、`error_running_total`、`event_running_total`)、`organization_specific`、`end` |
| 0x02 | `efm_oam_variable_request` | `variable_descriptors` (オクテット列: ブランチ、リーフ、…、0 のブランチ) |
| 0x03 | `efm_oam_variable_response` | `variable_containers` (オクテット列: ブランチ、リーフ、幅、値、…、0 のブランチ) |
| 0x04 | `efm_oam_loopback_control` | `remote_loopback_command` (1 有効化、2 無効化) |
| 0xfe | `efm_oam_organization_specific` | `oui`、`data` |

Information / Event TLV の長さは 2 オクテットのヘッダを含み (自動)、リストは `end` で終わります。

## 対象外

- PTP: 認証 / セキュリティ TLV (IEEE 1588-2019 附属書 P)、管理 ID ごとの Management TLV データの型付け、その他のトランスポート (附属書 C、E〜G のうち IPv6 / Ethernet 以外)、状態機械と送信時のタイムスタンプ付与。
- CFM: MAID / MEG ID は送信される 48 オクテットのまま (名前形式ごとに分解しない)、Test TLV の CRC-32 は計算しない。Linear APS、MCC、EXM / EXR、VSM / VSR は専用定義なし (汎用 `cfm`)。
- 802.3ah OAM: 変数記述子 / コンテナはオクテット列。OAM ディスカバリの状態機械。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`lacp`](../reference/protocols/lacp.md) · [`marker`](../reference/protocols/marker.md) · [`ptp`](../reference/protocols/ptp.md) · [`ptp_sync`](../reference/protocols/ptp_sync.md) · [`ptp_delay_req`](../reference/protocols/ptp_delay_req.md) · [`ptp_pdelay_req`](../reference/protocols/ptp_pdelay_req.md) · [`ptp_pdelay_resp`](../reference/protocols/ptp_pdelay_resp.md) · [`ptp_follow_up`](../reference/protocols/ptp_follow_up.md) · [`ptp_delay_resp`](../reference/protocols/ptp_delay_resp.md) · [`ptp_pdelay_resp_follow_up`](../reference/protocols/ptp_pdelay_resp_follow_up.md) · [`ptp_announce`](../reference/protocols/ptp_announce.md) · [`ptp_signaling`](../reference/protocols/ptp_signaling.md) · [`ptp_management`](../reference/protocols/ptp_management.md) · [`esmc`](../reference/protocols/esmc.md) · [`cfm`](../reference/protocols/cfm.md) · [`cfm_ccm`](../reference/protocols/cfm_ccm.md) · [`cfm_lbm`](../reference/protocols/cfm_lbm.md) · [`cfm_lbr`](../reference/protocols/cfm_lbr.md) · [`cfm_ltm`](../reference/protocols/cfm_ltm.md) · [`cfm_ltr`](../reference/protocols/cfm_ltr.md) · [`cfm_ais`](../reference/protocols/cfm_ais.md) · [`cfm_lck`](../reference/protocols/cfm_lck.md) · [`cfm_tst`](../reference/protocols/cfm_tst.md) · [`cfm_raps`](../reference/protocols/cfm_raps.md) · [`cfm_lmm`](../reference/protocols/cfm_lmm.md) · [`cfm_lmr`](../reference/protocols/cfm_lmr.md) · [`cfm_1dm`](../reference/protocols/cfm_1dm.md) · [`cfm_dmm`](../reference/protocols/cfm_dmm.md) · [`cfm_dmr`](../reference/protocols/cfm_dmr.md) · [`cfm_csf`](../reference/protocols/cfm_csf.md) · [`cfm_slm`](../reference/protocols/cfm_slm.md) · [`cfm_slr`](../reference/protocols/cfm_slr.md) · [`cfm_1sl`](../reference/protocols/cfm_1sl.md) · [`efm_oam`](../reference/protocols/efm_oam.md) · [`efm_oam_information`](../reference/protocols/efm_oam_information.md) · [`efm_oam_event_notification`](../reference/protocols/efm_oam_event_notification.md) · [`efm_oam_variable_request`](../reference/protocols/efm_oam_variable_request.md) · [`efm_oam_variable_response`](../reference/protocols/efm_oam_variable_response.md) · [`efm_oam_loopback_control`](../reference/protocols/efm_oam_loopback_control.md) · [`efm_oam_organization_specific`](../reference/protocols/efm_oam_organization_specific.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
