# チュートリアル: IPv6 と近隣探索 (NDP)

所要時間: 約 25 分。必要な権限: ステップ 6 までは不要です。

> このページは、予定していた「IS-IS IIH」チュートリアルの代わりです。IS-IS (と CLNP) は現在同梱している P0 プロトコルの範囲外で、ロードマップ (TODO.md の P2) にあります。当面は、型・長さ・チェックサムが互いに導出される複数レイヤと、ネットワーク層アドレスから計算するリンク層マルチキャストアドレスという、同じ考え方を IPv6 スタックで練習します。

## 作るもの

IPv6 のリンク上の動作を支える近隣探索メッセージと、近隣要請のスイープです。

| テンプレート | メッセージ |
|---|---|
| `templates/protocols/l3/ndp-rs.wpg.yaml` | `ff02::2` 宛てのルータ要請 |
| `templates/protocols/l3/ndp-ra.wpg.yaml` | SLLA・MTU・/64 プレフィックス付きのルータ通知 |
| `templates/protocols/l3/ndp-ns.wpg.yaml` | 被請求ノードグループ宛ての近隣要請 |
| `templates/protocols/l3/ndp-na.wpg.yaml` | ターゲットリンク層アドレス付き近隣通知 (S=1, O=1) |
| `templates/protocols/l3/ndp-redirect.wpg.yaml` | リダイレクト |
| `templates/scenarios/ndp-ns-sweep.wpg.yaml` | `2001:db8::1` から `::100` までの近隣要請 256 個 |
| `templates/scenarios/ra-flood.wpg.yaml` | ランダムなリンクローカル送信元からの RA 1 万個 (RA guard の試験用) |

## 始める前に

wpg をビルドし、リポジトリのルートで実行します。送信してよいのは自分が所有する、または試験を許可されたラボだけです。偽の RA はセグメント上の全ホストを混乱させる可能性があります。

## ステップ 1. 近隣要請 1 個のレイヤをたどる

```console
$ wpg template show templates/protocols/l3/ndp-ns.wpg.yaml
Hex dump (86 bytes)
0000  33 33 ff 00 00 02 02 00  00 00 00 01 86 dd 60 00  33............`.
0010  00 00 00 20 3a ff 20 01  0d b8 00 00 00 00 00 00  ... :. .........
...
Layout
[0] ethernet2  bytes 0..86
  dst: 33:33:ff:00:00:02
  src: 02:00:00:00:00:01
  ethertype: 0x86dd (ipv6)  (auto)
[1] ipv6  bytes 14..86
  payload_length: 32  (auto)
  next_header: 0x3a (icmpv6)  (auto)
  hop_limit: 255
  src: 2001:db8::1
  dst: ff02::1:ff00:2
[2] icmpv6  bytes 54..86
  type: 0x87 (neighbor_solicitation)  (auto)
  code: 0
  checksum: 7207  (auto)
[3] ndp_ns  bytes 58..86
  reserved: 0
  target: 2001:db8::2
  options: 01 01 02 00 00 00 00 01
```

`(auto)` の値はすべて、入力ではなく導出されたものです。

* `ethernet2.ethertype` = `0x86dd`: 次のレイヤが `ipv6` だからです。
* `ipv6.next_header` = 58: 次のレイヤが `icmpv6` だからです。`payload_length` は 40 オクテットのヘッダ以降すべてを数えます。
* `icmpv6.type` = 135: 次のレイヤが `ndp_ns` だからです。ICMPv6 メッセージは意図的に 2 つのレイヤに分かれています。`icmpv6` が `type`・`code`・`checksum` を持ち、本体レイヤ (`ndp_ns`、`ndp_na`、`ndp_rs`、`ndp_ra`、`ndp_redirect`、`icmpv6_echo`) が type を決めます。
* `icmpv6.checksum` は IPv6 疑似ヘッダ、ICMPv6 ヘッダ、本体全体にかかるため、下位のどのフィールドを変えても変わります。

元の YAML は短いものです。

```yaml
layers:
  - protocol: ethernet2
    fields: { dst: "33:33:ff:00:00:02", src: "02:00:00:00:00:01" }
  - protocol: ipv6
    fields:
      src: 2001:db8::1
      dst: ff02::1:ff00:2
      hop_limit: 255          # NDP は 255 が必須
  - protocol: icmpv6          # type: auto
  - protocol: ndp_ns
    fields:
      target: ${target}
      options: "01 01 02 00 00 00 00 01"   # 送信元リンク層アドレス
```

## ステップ 2. 一致させるべき 2 つのアドレス

`2001:db8::2` の近隣要請は、被請求ノードマルチキャストグループ `ff02::1:ff00:2` (`ff02::1:ff` にターゲットの下位 24 ビット) 宛てに、Ethernet ではマルチキャスト MAC `33:33:` に IPv6 宛先の下位 32 ビットを付けた `33:33:ff:00:00:02` 宛てに送ります。wpg は一方から他方を導出しないので、`target` を変えたら `ipv6.dst` と `ethernet2.dst` も変える必要があります (スイープのように全ノード宛て `ff02::1` / `33:33:00:00:00:01` に送る方法もあります)。

`hop_limit: 255` にも注意してください。受信側は他の値の NDP メッセージを破棄するため、既定の 64 のままだと Wireshark では正しく見えるのに何も起きないパケットになります。

## ステップ 3. オプションは生のバイト列

NDP オプション (`options`) は 16 進で書きます。`01 01 02 00 00 00 00 01` は、タイプ 1 (送信元リンク層アドレス)、長さ 1 (8 オクテット単位)、続いて 6 オクテットの MAC です。ルータ通知のテンプレートは 3 つのオプションを連結しています。

```yaml
options: >-
  01 01 02 00 00 00 00 01
  05 01 00 00 00 00 05 dc
  03 04 40 c0 00 27 8d 00 00 09 3a 80 00 00 00 00
  20 01 0d b8 00 01 00 00 00 00 00 00 00 00 00 00
```

| オクテット | オプション |
|---|---|
| `01 01 02 00 00 00 00 01` | 送信元リンク層アドレス `02:00:00:00:00:01` |
| `05 01 00 00 00 00 05 dc` | MTU 1500 |
| `03 04 40 c0 00 27 8d 00 00 09 3a 80 00 00 00 00 20 01 0d b8 00 01 00 00 00 00 00 00 00 00 00 00` | プレフィックス情報: /64 (`40`)、on-link + autonomous (`c0`)、有効期間 2592000 秒、優先期間 604800 秒、プレフィックス `2001:db8:1::` |

オプションは生のオクテットなので、長さ 0 のオプションやパケット末尾を越えるオプション、未知のタイプなど、わざと変なものも作れます。

## ステップ 4. Wireshark で確認する

```console
$ wpg render templates/protocols/l3/ndp-ns.wpg.yaml -o /tmp/ns.pcapng
$ tshark -r /tmp/ns.pcapng -V | sed -n '/Internet Control/,$p'
Internet Control Message Protocol v6
    Type: Neighbor Solicitation (135)
    Code: 0
    Checksum: 0x1c27 [correct]
    [Checksum Status: Good]
    Reserved: 00000000
    Target Address: 2001:db8::2
    ICMPv6 Option (Source link-layer address : 02:00:00:00:00:01)
```

重要なのは `[correct]` です。疑似ヘッダ込みのチェックサムを wpg が自分で計算しています。

## ステップ 5. スイープする

`ndp-ns-sweep.wpg.yaml` は `target` ジェネレータに IPv6 の範囲を指定します。

```yaml
  - protocol: ndp_ns
    generators:
      target: 2001:db8::1-2001:db8::100
```

```console
$ wpg render templates/scenarios/ndp-ns-sweep.wpg.yaml -o /tmp/ns-sweep.pcapng
Wrote 256 packets to /tmp/ns-sweep.pcapng.
```

宛先は全ノードグループなので、セグメント上の全ホストが 256 個すべてを受け取り、自分のアドレス分にだけ応答します。実際のホストはターゲットごとの被請求ノードグループに送りますが、スイープでは単純な 1 テンプレートにするためそれを省いています。

`ra-flood.wpg.yaml` は、送信元 MAC (OUI 固定の `random_mac`) と送信元リンクローカルアドレスにランダムジェネレータを使う例で、どちらも `stream.seed` で再現できます。

## ステップ 6. 送信する

```console
$ wpg send templates/scenarios/ndp-ns-sweep.wpg.yaml --backend null -i lo --rate 1000pps
$ wpg send templates/scenarios/ndp-ns-sweep.wpg.yaml -i eth1 --rate 1000pps
```

(権限は [権限設定](../getting-started/permissions.md) を参照。) 対象機器では `ip -6 neigh` や `show ipv6 neighbors` で学習結果を、別ホストの `tcpdump -n -i eth1 icmp6` で要請と通知を確認できます。

## その他の ICMPv6 タイプ

本体プロトコルがまだ無いタイプ (たとえば MLD) は、`icmpv6.type` を明示的に設定し、チェックサム以降の 4 オクテットとメッセージを含む `raw` レイヤを続けます。IPv6 拡張ヘッダの連鎖は専用のサンプルがあります: `templates/protocols/l3/` の `ipv6-hopopts`、`ipv6-dstopts`、`ipv6-routing`、`ipv6-frag`、`ipv6-ext-chain` です。

## 次のステップ

* RA フラッドを指定レートで送るには [負荷とレート制御](rate-control.md)。
* IPv6/ICMPv6 の全フィールドは [プロトコルリファレンス](../reference/protocols.md)。
