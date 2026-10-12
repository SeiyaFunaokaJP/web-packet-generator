# トンネル / カプセル化

Status: draft

VXLAN、GRE、NVGRE、MPLS、MPLS-in-UDP、L2TPv2 / v3、GTP-U、Geneve、IP-in-IP の補足です。定義は `crates/wpg-protocols/src/encap.rs` にあります。

トンネルは「外側ヘッダ → トンネルヘッダ → 内側パケット」の順にレイヤを並べるだけで表現できます。内側にも `ethernet2` / `ipv4` / `udp` などを普通に積めます。

## 識別と積み方

| プロトコル ID | 下位レイヤ | 識別子 | 内側 (次のレイヤ) |
|---|---|---|---|
| `vxlan` | `udp` | ポート 4789 | `ethernet2` |
| `gre` | `ipv4` | プロトコル 47 | `protocol_type` で `ipv4` / `ipv6` / `ethernet2` / `mpls` |
| `nvgre` | `ipv4` | プロトコル 47 (**手動指定**、後述) | `ethernet2` |
| `mpls` | `ethernet2` / `gre` / `udp` | EtherType `0x8847` / GRE `0x8847` / ポート 6635 | `mpls` (S=0)、IPv4 / IPv6 (S=1) |
| `l2tpv2` | `udp` | ポート 1701 | opaque な `body` |
| `l2tpv3` | `ipv4` | プロトコル 115 | opaque な `body` |
| `gtpu` | `udp` | ポート 2152 | G-PDU (type 255) なら `ipv4` / `ipv6` |
| `geneve` | `udp` | ポート 6081 | `protocol_type` で `ethernet2` / `ipv4` / `ipv6` |
| IP-in-IP | `ipv4` | プロトコル 4 (`ipv4`) / 41 (`ipv6`) | 専用のプロトコル定義はなく、`ipv4` の上に `ipv4` / `ipv6` を積む |

## Auto フィールド

| プロトコル | Auto |
|---|---|
| `gre` / `geneve` | `protocol_type` (次のレイヤから) |
| `mpls` | `s` (次のレイヤが `mpls` なら 0、それ以外は 1) |
| `gtpu` | `length` (ペイロード長) |
| 外側の `ipv4` / `udp` | `protocol` / 長さ / チェックサム。`udp.dst_port` もトンネルのポートになる |

## 制約と回避方法

- **GRE** は 4 オクテットの基本ヘッダ (フラグ + バージョン、`protocol_type`) だけを表現します。チェックサム / キー / シーケンス番号の拡張フィールドは表現しません。キー付きの代表例である NVGRE は専用の `nvgre` を使ってください。
- **NVGRE** は IPv4 プロトコル 47 の解析先が `gre` なので、合成時に Auto で 47 になりません。外側 `ipv4` に `protocol: 47` を明示します (`nvgre.wpg.yaml` 参照)。解析すると `gre` として読まれ、キーの 4 オクテットは内側フレームの先頭として解釈されます。
- **MPLS** はラベル 1 個が 1 レイヤです。ラベルスタックは `mpls` を必要な数だけ積みます。ボトムラベルの下は先頭ニブル (4 → IPv4、6 → IPv6) で判別します。疑似回線 (コントロールワードや Ethernet) には判別できる印がないため解析では `raw` になります。合成時は `ethernet2` などを明示的に積んでください。
- **MPLS-in-UDP** (RFC 7510) はポート 6635 の `udp` の上に `mpls` を積みます。送信元ポートはエントロピー用に変化させると実機に近くなります (ジェネレータで `src_port` を range にする)。
- **GTP-U** は必須の 8 オクテットヘッダだけを表現します。E / S / PN フラグで現れるシーケンス番号・N-PDU 番号・拡張ヘッダは表現しません。
- **Geneve** のオプションはサブオクテットのヘッダを持つため、`opt_len` (4 オクテット単位) で長さが決まる opaque な `options` です。`opt_len` は Manual なので、`options` を書いたら `opt_len` も合わせてください。
- **L2TPv2 / v3** はヘッダのフィールドがフラグや制御 / データの別で出現したりしなかったりするため、v2 はフラグ / バージョンの 16 ビット、v3 は Session ID の後ろを opaque な `body` にしています。長さ・トンネル ID・セッション ID・PPP フレームなどは `body` にバイト列で書きます。

## サンプル

| テンプレート | 内容 |
|---|---|
| `templates/protocols/encap/vxlan.wpg.yaml` | 内側 Ethernet / IPv4 / UDP を VXLAN で運ぶ (VNI は変数) |
| `templates/protocols/encap/gre-ipv4.wpg.yaml` | GRE で IPv4 / ICMP を運ぶ |
| `templates/protocols/encap/nvgre.wpg.yaml` | NVGRE (VSID は変数) |
| `templates/protocols/encap/mpls-ipv4.wpg.yaml` | 2 段ラベルの MPLS 上の IPv4 / UDP |
| `templates/protocols/encap/mpls-in-udp.wpg.yaml` | MPLS-in-UDP |
| `templates/protocols/encap/l2tpv2.wpg.yaml` / `l2tpv3.wpg.yaml` | L2TPv2 データ / L2TPv3 over IP |
| `templates/protocols/encap/gtpu-ipv4.wpg.yaml` | IPv4 / UDP を運ぶ G-PDU (TEID は変数) |
| `templates/protocols/encap/geneve.wpg.yaml` | 内側 Ethernet を Geneve で運ぶ |
| `templates/protocols/encap/ipip.wpg.yaml` / `6in4.wpg.yaml` | IP-in-IP (プロトコル 4) / 6in4 (プロトコル 41) |
