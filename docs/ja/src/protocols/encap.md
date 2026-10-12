# カプセル化・トンネル

Status: draft

VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TPv2・v3 / GTP-U / Geneve / IP-in-IP の補足です。定義は `crates/wpg-protocols/src/encap.rs`、サンプルは `templates/protocols/encap/` にあります。

## 到達経路とサンプル

| id | プロトコル | 下位レイヤ | 上位レイヤ | サンプル |
|---|---|---|---|---|
| `vxlan` | VXLAN | UDP 4789 | `ethernet2` | `vxlan.wpg.yaml` |
| `geneve` | Geneve | UDP 6081 | `protocol_type` で選択 | `geneve.wpg.yaml` |
| `gre` | GRE (基本 4 オクテット) | IPv4 protocol / IPv6 next header 47 | `protocol_type` (IPv4 / IPv6 / `0x6558` Ethernet / MPLS) | `gre-ipv4.wpg.yaml` |
| `nvgre` | NVGRE | ビルド専用 (IP protocol 47 のデコードは `gre`) | `ethernet2` | `nvgre.wpg.yaml` |
| `mpls` | MPLS ラベル 1 段 | EtherType `0x8847` / `0x8848`、GRE、UDP 6635 (MPLS-in-UDP) | `s = 0` なら次の `mpls`、`s = 1` なら先頭ニブルで IPv4 / IPv6 | `mpls-ipv4.wpg.yaml`、`mpls-in-udp.wpg.yaml` |
| `l2tpv2` | L2TPv2 | UDP 1701 | 本体は不透明 | `l2tpv2.wpg.yaml` |
| `l2tpv3` | L2TPv3 over IP | IPv4 protocol 115 | 本体は不透明 | `l2tpv3.wpg.yaml` |
| `gtpu` | GTP-U (必須 8 オクテット) | UDP 2152 | 先頭ニブルで IPv4 / IPv6 | `gtpu-ipv4.wpg.yaml` |
| — | IP-in-IP / 6in4 | IPv4 protocol 4 → `ipv4`、41 → `ipv6` | — | `ipip.wpg.yaml`、`6in4.wpg.yaml` |

IP-in-IP 用の専用プロトコルはありません。`ipv4` の上に `ipv4` (または `ipv6`) を積むだけで、外側の `protocol` は Auto で 4 (41) になります。

## Auto になるフィールド

- `gre.protocol_type`、`geneve.protocol_type`: 次のレイヤから決まります。
- `gtpu.length`: GTP-U ヘッダ以降の長さ。
- `mpls.s` (bottom-of-stack): 次のレイヤが `mpls` なら 0、それ以外なら 1。ラベルを重ねるには `mpls` レイヤを必要な段数だけ積みます。
- 外側の UDP 宛先ポート・EtherType・IP protocol は、上に積んだレイヤから Auto で決まります。

## 制約と近似

- **GRE** のチェックサム / キー / シーケンス番号は対応するフラグが立ったときだけ存在しますが、エンジンには「存在条件付きフィールド」がありません。`gre` は基本の 4 オクテット (flags + protocol type) のみで、フラグを立てても追加フィールドは生成されません。キー付き GRE の代表例である NVGRE には専用の `nvgre` (Key ビット既定オン、`vsid` + `flow_id`) を使ってください。`nvgre` はビルド専用で、受信したパケットを解析すると `gre` としてデコードされ、キーの 4 オクテットは内側フレームの先頭として誤って読まれます。それ以外のオプションが必要な場合は、`gre` の直後に `raw` レイヤで値を置き、フラグを手動で立てます。
- **MPLS 疑似ワイヤ**: 最下段ラベルの下はペイロードの先頭ニブルで判別します (4 → IPv4、6 → IPv6)。コントロールワードや Ethernet 疑似ワイヤには判別できる印がないため、デコードでは `raw` になります。ビルド時は `ethernet2` を明示的に積んでください。
- **GTP-U** の任意フィールド (シーケンス番号、N-PDU 番号、拡張ヘッダ。E/S/PN フラグで存在) は表現されません。
- **Geneve** のオプションはヘッダが 3 ビットのフラグ + 5 ビット長と 1 オクテット未満の幅を含むため、`opt_len` で大きさが決まる不透明な `options` 領域です。
- **L2TPv2 / v3** のヘッダは種別 (制御 / データ) とフラグで存在するフィールドが大きく変わるため、本体は不透明なバイト列です。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`ethernet2`](../reference/protocols/ethernet2.md) · [`ipv4`](../reference/protocols/ipv4.md) · [`ipv6`](../reference/protocols/ipv6.md) · [`vxlan`](../reference/protocols/vxlan.md) · [`gre`](../reference/protocols/gre.md) · [`nvgre`](../reference/protocols/nvgre.md) · [`mpls`](../reference/protocols/mpls.md) · [`gtpu`](../reference/protocols/gtpu.md) · [`geneve`](../reference/protocols/geneve.md) · [`l2tpv2`](../reference/protocols/l2tpv2.md) · [`l2tpv3`](../reference/protocols/l2tpv3.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
