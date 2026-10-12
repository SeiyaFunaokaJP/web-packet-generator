# カプセル化プロトコル (P1)

Status: draft

定義: `crates/wpg-protocols/src/encap.rs` / テスト: `crates/wpg-protocols/tests/p1_encap.rs` / サンプル: `templates/protocols/encap/`

## 対象と到達方法

| id | プロトコル | 下位からの到達 | 上位 (ペイロード) | サンプル |
|---|---|---|---|---|
| `vxlan` | VXLAN (RFC 7348) | UDP 4789 | 内側 Ethernet | `vxlan` |
| `gre` | GRE (RFC 2784) | IP プロトコル 47 | `protocol_type` で IPv4 / IPv6 / Ethernet (`0x6558`) / MPLS | `gre-ipv4` |
| `nvgre` | NVGRE (RFC 7637) | 構築のみ (IP `protocol: 47` を手動指定) | 内側 Ethernet | `nvgre` |
| `mpls` | MPLS ラベル 1 段 (RFC 3032) | EtherType `0x8847` / `0x8848`、GRE、UDP 6635 (MPLS-in-UDP) | 次の `mpls`、または先頭ニブルで IPv4 / IPv6 | `mpls-ipv4` / `mpls-in-udp` |
| `gtpu` | GTP-U (3GPP TS 29.281) | UDP 2152 | G-PDU (type 255) は先頭ニブルで IPv4 / IPv6 | `gtpu-ipv4` |
| `geneve` | Geneve (RFC 8926) | UDP 6081 | `protocol_type` で Ethernet / IPv4 / IPv6 | `geneve` |
| `l2tpv2` | L2TPv2 (RFC 2661) | UDP 1701 | opaque な `body` | `l2tpv2` |
| `l2tpv3` | L2TPv3 (RFC 3931) | IP プロトコル 115 | opaque な `body` | `l2tpv3` |

**IP-in-IP** は新しいプロトコルを持ちません。IPv4 の上に `ipv4` (プロトコル 4) や `ipv6` (プロトコル 41、6in4) をそのまま積みます (サンプル `ipip` / `6in4`)。

## 積み方

- **MPLS ラベルスタック**: `mpls` を必要な段数だけ積みます。各段の `s` (bottom-of-stack) は Auto で、下に `mpls` が続けば 0、それ以外は 1 になります。最下段のラベルの次は IPv4 / IPv6 を積みます。
- **NVGRE**: IPv4 の対応表ではプロトコル 47 が `gre` に割り当てられているため、`ipv4` に `protocol: 47` を手動で指定します。解析すると `gre` として読まれ、Key 以降は正しく解釈されません。
- **MPLS-in-UDP**: `udp` (`dst_port: 6635`) → `mpls` → `ipv4` …。
- **L2TPv3 over UDP**: UDP 1701 は `l2tpv2` に割り当てられているため、UDP 上の L2TPv3 は解析すると `l2tpv2` として読まれます。

## Auto フィールド

| フィールド | 内容 |
|---|---|
| `gre.protocol_type` / `geneve.protocol_type` | 次のレイヤ (EtherType) |
| `mpls.s` | 下に `mpls` が続くかどうか |
| `gtpu.length` | ペイロードの長さ |

## 制約と近似

- **GRE**: 4 オクテットの基本ヘッダ (フラグ + バージョン、`protocol_type`) だけを型付けしています。フラグに応じて現れるチェックサム / Key / シーケンス番号は表現できません。Key 付きの代表例である NVGRE は専用の `nvgre` を使ってください。
- **MPLS**: 最下段の下のペイロードは先頭ニブル (4 → IPv4、6 → IPv6) で判別します。疑似回線 (コントロールワード / Ethernet) には確実な目印がないため、解析では `raw` になります。構築時は `ethernet2` を明示的に積んでください。
- **GTP-U**: 必須の 8 オクテットヘッダだけを型付けしています。E / S / PN フラグに応じて続くシーケンス番号 / N-PDU 番号 / 拡張ヘッダは扱いません。フラグは 0 のままにしてください。
- **Geneve**: オプションのヘッダは 3 ビットのフラグ + 5 ビットの長さでオクテット境界に揃わないため、`opt_len` (4 オクテット単位、Manual) で長さが決まる opaque な `options` 領域です。`options` を変えたら `opt_len` も合わせてください。
- **L2TPv2 / v3**: ヘッダのフィールドはフラグや制御 / データ種別に応じて現れるため、`flags` (v2) / `session_id` (v3) 以降は opaque な `body` です。
