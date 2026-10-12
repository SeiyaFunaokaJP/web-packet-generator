# CLNP / ES-IS (P2)

Status: draft

CLNP (ISO 8473-1) と ES-IS (ISO 9542) の補足です (M7)。定義は `crates/wpg-protocols/src/clnp.rs`、サンプルは `templates/protocols/l3/clnp-*.wpg.yaml` と `templates/protocols/routing/esis-*.wpg.yaml` にあります。どちらも [OSI カプセル化](osi.md) の上に直接載ります。

## PDU と到達経路、サンプル

| id | PDU | サンプル |
|---|---|---|
| `clnp` | CLNP の全 PDU 種別: DT (`0x1c`)、MD (`0x1d`)、ER (`0x01`)、ERQ / ERP (`0x1e` / `0x1f`) | `clnp-dt` (セグメンテーション部)、`clnp-er` (破棄理由)、`clnp-erq` (優先度、ソースルーティング) |
| `esis_esh` | ES-IS エンドシステム Hello (2) | `esis-esh` |
| `esis_ish` | ES-IS 中間システム Hello (4) | `esis-ish` (ES 設定タイマ) |
| `esis_rd` | ES-IS リダイレクト (6) | `esis-rd` |
| `esis` | 任意の ES-IS PDU: 固定部 + 不透明なオクテット列 | `esis-generic` |

```text
dot3 / llc / clnp / payload        # トランスポート PDU を運ぶ DT
dot3 / llc / clnp / raw            # 破棄した PDU のヘッダを運ぶ ER
dot3 / llc / esis_esh              # 宛先 09:00:2b:00:00:05 (AllIntermediateSystems)
dot3 / llc / esis_ish              # 宛先 09:00:2b:00:00:04 (AllEndSystems)
ppp / clnp / payload               # PPP protocol 0x0023
```

LLC の DSAP / SSAP (`0xfe`) と PPP の protocol (`0x0023`) は Auto です。解析時は NLPID `0x81` で `clnp`、`0x82` で汎用 `esis` が選ばれ、`esis` は `pdu_type` に応じて専用 PDU として読み直されます (未知の種別は `esis` のまま)。固定部より短い PDU (CLNP 11 オクテット、ES-IS 9 オクテット未満) は汎用 `osi` としてデコードされます。

`clnp` レイヤはヘッダだけです。データ (DT: トランスポート PDU、ER: 破棄した PDU のヘッダ、ERQ / ERP: エコーする PDU) は次のレイヤ (通常 `payload` か `raw`) に置きます。

## Auto フィールド

| フィールド | 規則 |
|---|---|
| `length_indicator` | ヘッダ長 (オクテット)。CLNP はオプションの終わりまで、ES-IS は PDU 全体 |
| `segment_length` (CLNP) | PDU 全体 (ヘッダ + データ) の長さ |
| `checksum` | ヘッダ (CLNP) / PDU (ES-IS) の ISO 8473 Fletcher チェックサム。Manual の `0` は「チェックサム不使用」で、そのまま送出され Auto との不一致として報告されます (計算値が 0 になることはありません) |
| `sp` (CLNP) | `segmentation` に要素があれば 1、なければ 0 |
| `number_of_source_addresses` (ESH) | `source_addresses` の数 |
| `bsnpa_length` (RD) | `bsnpa` のサイズ |

CLNP のセグメンテーション部は `{data_unit_identifier, segment_offset, total_length}` を最大 1 個持つリストです。`total_length` (セグメント化前の PDU 長) は計算しません。

```yaml
- protocol: clnp
  fields:
    segmentation:
      - { data_unit_identifier: 1, segment_offset: 0, total_length: 69 }
```

## オプション

オプションはアドレス (とセグメンテーション部) の後ろに並ぶ `{コード, 長さ, 値}` で、`length_indicator` の位置で終わります (`Length::UntilOffset`)。アドレスが可変長でも、ヘッダより後ろは次のレイヤになります。

| コード | 名前 (`name`) | 値のフィールド | 対象 |
|---|---|---|---|
| `0xc5` | `security` | `value` (オクテット列) | CLNP、ES-IS |
| `0xc3` | `qos_maintenance` | `value` (オクテット列) | CLNP、ES-IS |
| `0xcd` | `priority` | `value` | CLNP、ES-IS |
| `0xc8` / `0xcb` | `source_routing` / `recording_of_route` | `route_type` (`partial` / `complete`)、`next_address`、`addresses`: `{address}` のリスト | CLNP |
| `0xcc` | `padding` | `data` | CLNP |
| `0xc1` | `reason_for_discard` | `reason` (`destination_unknown`、`lifetime_expired_in_transit` など)、`pointer` | CLNP (ER) |
| `0xe1` / `0xe2` | `address_mask` / `snpa_mask` | `mask` | ES-IS |
| `0xc6` | `es_configuration_timer` | `seconds` | ES-IS (ISH) |

その他のオプションは `{ type: N, data: "..." }` として保持されます。

## 制約

- セグメンテーション部の `total_length` は計算せず、大きな PDU を複数フレームに分割 / 再構成する機能もありません。
- セキュリティ / QoS 維持の値はオクテット列で、グローバル一意 / 送信元アドレス固有などの形式は分解しません。
- リダイレクトのサブネットワークアドレス (`bsnpa`) はオクテット列です (MAC アドレスは 6 オクテット: `"02 00 00 00 00 02"`)。`net` を空 (`""`) にするとエンドシステムへのリダイレクトになります。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`osi`](../reference/protocols/osi.md) · [`clnp`](../reference/protocols/clnp.md) · [`esis`](../reference/protocols/esis.md) · [`esis_esh`](../reference/protocols/esis_esh.md) · [`esis_ish`](../reference/protocols/esis_ish.md) · [`esis_rd`](../reference/protocols/esis_rd.md) · [`payload`](../reference/protocols/payload.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
