# LDP / RSVP-TE / BFD (P2)

Status: draft

LDP (RFC 5036)、RSVP-TE (RFC 2205 / RFC 3209)、BFD (RFC 5880 / 5881 / 5883、Micro-BFD RFC 7130) の補足 (M7)。定義は `crates/wpg-protocols/src/{ldp,rsvp,bfd}.rs`、サンプルは `templates/protocols/mpls/` と `templates/protocols/oam/` にあります。

## 到達経路とサンプル

```text
ethernet2 / ipv4 / udp / ldp       # UDP 646: Hello (基本ディスカバリは 224.0.0.2、拡張はユニキャスト)
ethernet2 / ipv4 / tcp / ldp       # TCP 646: セッション (Initialization 以降)
ethernet2 / ipv4 / rsvp            # IPv4 protocol 46 (Auto)
ethernet2 / ipv6 / rsvp            # IPv6 next header 46 (Auto)
ethernet2 / ipv4 / udp / bfd       # UDP 3784 (シングルホップ)、4784 (マルチホップ)、6784 (Micro-BFD)
```

IPv6 の LDP (RFC 7552) も `ipv6 / udp|tcp / ldp` で組めます。UDP / TCP の宛先ポートは Auto ではありません。646 / 3784 などを指定してください (解析時は送信元ポートでも判別します)。BFD のエコーポート 3785 の中身は送信側にしか意味がないためデコードしません。

| サンプル | 内容 |
|---|---|
| `ldp-hello` | 224.0.0.2 へのリンク Hello (GTSM ビット、IPv4 トランスポートアドレス) |
| `ldp-targeted-hello` | 拡張ディスカバリの Targeted Hello (T / R ビット、構成シーケンス番号) |
| `ldp-initialization` | 1 PDU に Initialization (Common Session Parameters) と KeepAlive |
| `ldp-label-mapping` | Address と Label Mapping 2 つ (プレフィックス FEC、汎用ラベル。暗黙 NULL 3 を含む) |
| `ldp-label-withdraw` | ワイルドカード FEC の Label Withdraw と Notification (Shutdown) |
| `rsvp-te-path` | Router Alert 付き Path (厳密 / 緩い ERO、SESSION_ATTRIBUTE、SENDER_TSPEC、RRO) |
| `rsvp-te-resv` | SE スタイルの Resv (FLOWSPEC、FILTER_SPEC、LABEL、ラベル付き RRO) |
| `rsvp-path-err` | ERROR_SPEC (Routing Problem) 付き PathErr |
| `rsvp-path-tear` | PathTear |
| `rsvp-hello` | HELLO REQUEST 付き Hello |
| `bfd-single-hop` | 状態 Up、300 ms 間隔の制御パケット (10 個、4 pps) |
| `bfd-auth` | Meticulous Keyed SHA1 認証付きの Poll |
| `bfd-multihop-ipv6` | IPv6 上のマルチホップ (UDP 4784、状態 Init) |
| `micro-bfd` | LAG メンバーリンクの Micro-BFD (UDP 6784、宛先 MAC `01:00:5e:90:00:01`) |

## LDP: 1 レイヤ = 1 PDU

`ldp` レイヤは PDU ヘッダ (`version`、`pdu_length`、`lsr_id`、`label_space`) と PDU 内の全メッセージ (`messages`) です。メッセージは PDU 長で終わるので、同じ TCP セグメントの後続 PDU は次のレイヤ (`raw`) になります。

メッセージと TLV は同じ形の項目です: 16 ビット型 (U ビット、TLV では F ビットも型の上位ビットに含む)、値だけの 16 ビット長、値。どのメッセージも先頭が `message_id`、続いて `tlvs` です。

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

| 型 | メッセージ (`name`) | 型 | TLV (`name`) と値フィールド |
|---|---|---|---|
| 0x0001 | `notification` | 0x0100 | `fec` (`elements`: `{element_type (2), address_family, prefix_length, prefix}`)、`fec_wildcard` (要素型 1) |
| 0x0100 | `hello` | 0x0101 | `address_list` (IPv4) / `address_list_ipv6` (`address_family`、`addresses`) |
| 0x0200 | `initialization` | 0x0103 / 0x0104 | `hop_count`、`path_vector` (`lsr_ids`) |
| 0x0201 | `keepalive` | 0x0200 | `generic_label` (`label`: 20 ビット) |
| 0x0300 / 0x0301 | `address` / `address_withdraw` | 0x0300 | `status` (`e`、`f`、`status_code`、`message_id`、`message_type`) |
| 0x0400 | `label_mapping` | 0x0400 | `common_hello_parameters` (`hold_time`、`t`、`r`、`g` (GTSM、RFC 6720)) |
| 0x0401 | `label_request` | 0x0401 / 0x0403 | `ipv4_transport_address` / `ipv6_transport_address` |
| 0x0402 | `label_withdraw` | 0x0402 | `configuration_sequence_number` |
| 0x0403 | `label_release` | 0x0500 | `common_session_parameters` (`protocol_version`、`keepalive_time`、`a`、`d`、`pv_lim`、`max_pdu_length`、`receiver_lsr_id`、`receiver_label_space`) |
| 0x0404 | `label_abort_request` | 0x0600 | `label_request_message_id` |

FEC TLV は値の先頭オクテット (要素型) で `fec_wildcard` と `fec` を、Address List は先頭 2 オクテット (アドレスファミリ) で IPv4 / IPv6 を選び分けます。その他の型 (ATM / Frame Relay のラベルとセッションパラメータ、ベンダー TLV など) は `{ type: N, data: "..." }` として保持します。

## RSVP-TE: オブジェクトは「長さ → 型」

`rsvp` レイヤは共通ヘッダ (`version`、`flags`、`msg_type`、`checksum`、`send_ttl`、`length`) とメッセージ内の全オブジェクト (`objects`) です。オブジェクトは 16 ビット長 (4 オクテットのヘッダを含む) の後に Class-Num と C-Type が続き、2 つを合わせた 16 ビット (`class_num << 8 | c_type`) を型として扱います (wpg-core の `TlvDef::len_first`)。項目名はクラスと C-Type から付けています。

| 型 | 名前 (`name`) | 型 | 名前 (`name`) |
|---|---|---|---|
| 0x0101 / 0x0102 | `session_ipv4` / `session_ipv6` | 0x0b01 | `sender_template_ipv4` |
| 0x0107 / 0x0108 | `session_lsp_tunnel_ipv4` / `_ipv6` | 0x0b07 / 0x0b08 | `sender_template_lsp_tunnel_ipv4` / `_ipv6` |
| 0x0301 / 0x0302 | `rsvp_hop_ipv4` / `rsvp_hop_ipv6` | 0x0c02 | `sender_tspec` (IntServ) |
| 0x0501 | `time_values` | 0x0f01 | `resv_confirm_ipv4` |
| 0x0601 / 0x0602 | `error_spec_ipv4` / `error_spec_ipv6` | 0x1001 | `label` |
| 0x0701 | `scope_ipv4` | 0x1301 | `label_request` (`l3pid`) |
| 0x0801 | `style` (`sharing`、`sender_selection`。既定 SE) | 0x1401 / 0x1501 | `explicit_route` / `record_route` (`subobjects`) |
| 0x0902 | `flowspec` (IntServ、制御負荷) | 0x1601 / 0x1602 | `hello_request` / `hello_ack` |
| 0x0a01 | `filter_spec_ipv4` | 0x1701 / 0x1801 / 0x1802 | `message_id` / `message_id_ack` / `message_id_nack` (RFC 2961) |
| 0x0a07 / 0x0a08 | `filter_spec_lsp_tunnel_ipv4` / `_ipv6` | 0xcf07 / 0xcf01 | `session_attribute` / `session_attribute_ra` (リソースアフィニティ付き) |

EXPLICIT_ROUTE のサブオブジェクトは `ipv4_prefix`、`ipv6_prefix`、`unnumbered_interface`、`as_number` で、`_loose` を付けた名前が L ビット付き (緩いホップ) です。RECORD_ROUTE のサブオブジェクトは `ipv4_address`、`ipv6_address`、`label`、`unnumbered_interface` (それぞれ `flags` 付き) です。ADSPEC・POLICY_DATA・INTEGRITY などその他のオブジェクトは値オクテットのまま保持します。

SENDER_TSPEC / FLOWSPEC のトークンバケット (`token_bucket_rate`、`token_bucket_size`、`peak_data_rate`) は IEEE 754 単精度浮動小数点のビットパターンを 32 ビット整数で指定します (例: 1,000,000 オクテット/秒 = `0x49742400`、無限大 = `0x7f800000`)。既定は 125,000 オクテット/秒 (1 Mbit/s)。

Path・PathTear・ResvConf は IP の Router Alert オプション付きで送るのが通常です (`ipv4` の `options: [router_alert: 0]`)。

## BFD

`bfd` レイヤは必須セクションと省略可能な認証セクション (`authentication`) です。フラグは `p`、`f`、`c`、`a`、`d`、`m` の 1 ビットフィールドです。認証セクションの型は簡易パスワード (`simple_password`: `key_id`、`password`)、`keyed_md5` / `meticulous_keyed_md5` (`key_id`、`sequence_number`、16 オクテットの `digest`)、`keyed_sha1` / `meticulous_keyed_sha1` (20 オクテットの `hash`) です。

```yaml
- protocol: bfd
  fields:
    state: up
    my_discriminator: 0x101
    your_discriminator: 0x201
    desired_min_tx_interval: 300000 # マイクロ秒
    required_min_rx_interval: 300000
    authentication:
      - simple_password: { key_id: 1, password: '"secret"' }
```

## Auto フィールド

| フィールド | 規則 |
|---|---|
| `ldp.pdu_length` | バージョンと PDU 長を除く PDU の長さ |
| LDP のメッセージ / TLV の長さ | 値の長さ (型と長さを除く) |
| `rsvp.checksum` | フィールドを 0 としたメッセージ全体のインターネットチェックサム |
| `rsvp.length`、オブジェクト / サブオブジェクトの長さ | ヘッダを含む長さ |
| SESSION_ATTRIBUTE の `name_length` と `padding` | 名前のオクテット数、4 オクテット境界までのゼロ詰め (長さに含む) |
| `bfd.length` | 認証セクションを含む制御パケットの長さ |
| `bfd.a` | 認証セクションがあれば 1 |

## 制約

- 状態を持ちません。LDP のセッション確立、RSVP のリフレッシュ、BFD の状態遷移とタイマーは行いません。値はそのまま送り、変化させるにはジェネレータを使います。
- BFD の MD5 ダイジェスト / SHA1 ハッシュ、RSVP の INTEGRITY は手動です (鍵による計算はしません)。
- LDP の 1 レイヤは 1 PDU です。同じセグメントに PDU を複数入れる場合、後続の PDU はデコード時に `raw` になります。
- RSVP は GMPLS (RFC 3473) のオブジェクト、ADSPEC、POLICY_DATA を型付けしません。RSVP over UDP (RFC 2205 付録 C) は対象外です。
- IntServ のパラメータは 1 つ目 (トークンバケット) だけを型付けし、以降 (保証サービスの Rate / Slack など) は `parameters` のオクテット列です。
