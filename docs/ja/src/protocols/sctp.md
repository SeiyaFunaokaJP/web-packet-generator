# SCTP (P2)

Status: draft

SCTP (RFC 9260) の補足 (M7)。定義は `crates/wpg-protocols/src/sctp.rs`、サンプルは `templates/protocols/l4/sctp-*.wpg.yaml` にあります。

## 到達経路とサンプル

```text
ethernet2 / ipv4 / sctp            # IPv4 protocol 132 (Auto)
ethernet2 / ipv6 / sctp            # IPv6 next header 132 (Auto)
ethernet2 / ipv4 / udp / sctp      # SCTP over UDP、ポート 9899 (RFC 6951)
```

| サンプル | 内容 |
|---|---|
| `sctp-init` | IPv4 アドレス・対応アドレス種別・ECN・FORWARD TSN 対応・対応拡張のパラメータ付き INIT |
| `sctp-init-ack` | State Cookie 付き INIT ACK |
| `sctp-data` | 16 オクテットのユーザデータを持つ DATA |
| `sctp-sack` | Gap Ack ブロック 2 つと重複 TSN 付き SACK |
| `sctp-heartbeat` | Heartbeat Info 付き HEARTBEAT |
| `sctp-abort` | ユーザ起因 ABORT 原因付き ABORT (T ビット) |
| `sctp-shutdown` | IPv6 上の SHUTDOWN |
| `sctp-over-udp` | UDP 9899 上で COOKIE ECHO と DATA をバンドル |

UDP の宛先ポートは Auto ではありません。SCTP over UDP では 9899 を指定してください (解析時は送信元ポート 9899 でも判別します)。

## 1 レイヤ、チャンクは項目

`sctp` レイヤは共通ヘッダ (`src_port`、`dst_port`、`verification_tag`、`checksum`) とパケット内の全チャンク (`chunks`) です。複数チャンクをバンドルでき、`sctp` の後ろにレイヤは続きません。DATA / I-DATA のユーザデータはチャンクの `user_data` (オクテット列) で、ペイロードプロトコル識別子による demux はしません。

チャンクは 8 ビット型・8 ビットフラグ・16 ビット長の項目です。長さは 4 オクテットのチャンクヘッダを含み、4 オクテット境界までのゼロ詰め (常に付加) は含みません。どのチャンクにも `flags` フィールドがあります (型と長さの間に書かれます)。

```yaml
- protocol: sctp
  fields:
    verification_tag: 0x3a5c7e91
    chunks:
      - sack:
          cumulative_tsn_ack: 100
          gap_ack_blocks: [{ start: 2, end: 3 }]
      - name: data # `data` は項目の予約キーなので {name, fields} で書く
        fields:
          flags: b|e
          tsn: 101
          user_data: "68 65 6c 6c 6f"
      - abort: { flags: t }
```

| 型 | 名前 (`name`) | 値フィールド (`flags` の後) |
|---|---|---|
| 0 | `data` | フラグ `i`/`u`/`b`/`e`、`tsn`、`stream_identifier`、`stream_sequence_number`、`payload_protocol_identifier`、`user_data` |
| 1 / 2 | `init` / `init_ack` | `initiate_tag`、`a_rwnd`、`outbound_streams`、`inbound_streams`、`initial_tsn`、`parameters` |
| 3 | `sack` | `cumulative_tsn_ack`、`a_rwnd`、`number_of_gap_ack_blocks` / `number_of_duplicate_tsns` (Auto)、`gap_ack_blocks` (`{start, end}`)、`duplicate_tsns` |
| 4 / 5 | `heartbeat` / `heartbeat_ack` | `parameters` (`heartbeat_info`) |
| 6 | `abort` | フラグ `t`、`causes` |
| 7 | `shutdown` | `cumulative_tsn_ack` |
| 8 / 11 | `shutdown_ack` / `cookie_ack` | (フラグのみ) |
| 9 | `error` | `causes` |
| 10 | `cookie_echo` | `cookie` |
| 14 | `shutdown_complete` | フラグ `t` |
| 15 | `auth` (RFC 4895) | `shared_key_identifier`、`hmac_identifier`、`hmac` |
| 0x40 | `i_data` (RFC 8260) | フラグ `i`/`u`/`b`/`e`、`tsn`、`stream_identifier`、`message_identifier`、`ppid_or_fsn`、`user_data` |
| 0x80 / 0xc1 | `asconf_ack` / `asconf` (RFC 5061) | `serial_number`、`parameters` |
| 0x82 | `re_config` (RFC 6525) | `parameters` |
| 0x84 | `pad` (RFC 4820) | `padding` |
| 0xc0 | `forward_tsn` (RFC 3758) | `new_cumulative_tsn`、`streams` (`{stream, stream_sequence}`) |

その他のチャンク型は `{ type: N, data: "..." }` として保持します (`data` の先頭オクテットがフラグ)。

## パラメータとエラー原因

パラメータ (INIT、INIT ACK、HEARTBEAT、ASCONF、RE-CONFIG) とエラー原因 (ABORT、ERROR) は 16 ビット型・16 ビット長の項目で、4 オクテット境界まで詰めます。RFC 9260 §3.2 のとおり、**最後の**パラメータ / 原因の詰め物はチャンク長に含めず、チャンクの詰め物で兼ねます。

| パラメータ | エラー原因 |
|---|---|
| `ipv4_address`、`ipv6_address`、`heartbeat_info`、`state_cookie`、`unrecognized_parameter`、`cookie_preservative`、`host_name_address`、`supported_address_types` (`[ipv4, ipv6]`)、`ecn_capable`、`random`、`chunk_list`、`requested_hmac_algorithm`、`padding`、`supported_extensions` (チャンク型)、`forward_tsn_supported`、`add_ip_address` / `delete_ip_address` / `set_primary_address` (`correlation_id`、`address`: アドレスパラメータ)、`error_cause_indication`、`success_indication`、`adaptation_layer_indication`、RE-CONFIG の要求 / 応答 | `invalid_stream_identifier`、`missing_mandatory_parameter` (件数は Auto)、`stale_cookie`、`out_of_resource`、`unresolvable_address`、`unrecognized_chunk_type`、`invalid_mandatory_parameter`、`unrecognized_parameters`、`no_user_data`、`cookie_received_while_shutting_down`、`restart_with_new_addresses`、`user_initiated_abort`、`protocol_violation`、ASCONF の原因 (`0x00a0`〜`0x00a4`)、`unsupported_hmac_identifier` |

## Auto フィールド

| フィールド | 規則 |
|---|---|
| `checksum` | フィールドを 0 とした SCTP パケット全体の CRC32c (RFC 9260 付録 A)。下位オクテットから送出 |
| チャンク / パラメータ / 原因の `length` | 詰め物を除いた大きさ。ゼロ詰めそのものも自動 |
| `number_of_gap_ack_blocks`、`number_of_duplicate_tsns` (SACK) | リストの要素数 |
| `number_of_missing_parameters` | `missing_parameters` の要素数 |

## 制約

- 最後のパラメータの詰め物までチャンク長に数える実装のチャンクも同じようにデコードされ、そのパラメータの後に詰め物の位置を保つ空の項目が入ります。
- アソシエーションの状態は持ちません。TSN・ストリームシーケンス番号・タグは値です (変化させるにはジェネレータを使います)。
- 未知のチャンク型はフラグを `data` の先頭オクテットとしてデコードし、型の「動作」ビットは解釈しません。
