# チュートリアル: pcap からテンプレートへ変換して再生

所要時間: 約 20 分。必要な権限: 最後のステップまで不要です。

## 作るもの

キャプチャ (ここでは再現しやすいよう wpg で作りますが、Ethernet の pcap/pcapng なら何でも構いません) を編集可能なテンプレートに変換し、元と同じバイト列に戻す往復です。途中で wpg は、パケット間で変化するフィールドを見つけてジェネレータにします。

`wpg import` は pcap (両バイトオーダー、マイクロ秒/ナノ秒) と pcapng を読みます。リンク層は Ethernet のみで、フレームに FCS は付いていない前提です。貼り付けた 16 進も読めます。

## 始める前に

wpg をビルドし、リポジトリのルートで実行します。`tshark` は確認にだけ使います。

## ステップ 1. キャプチャを作る

```console
$ wpg render templates/examples/arp-sweep.wpg.yaml -o /tmp/arp.pcapng
Wrote 254 packets to /tmp/arp.pcapng.
```

実際には `tcpdump -i eth0 -w capture.pcap` や Wireshark で取得したファイルを使います。

## ステップ 2. インポートする

```console
$ wpg import /tmp/arp.pcapng -o /tmp/arp.wpg.yaml
Imported 254 packet(s) into 1 template(s): /tmp/arp.wpg.yaml
```

結果は次のとおりです。

```yaml
schema: 1
meta:
  name: arp
  description: Imported from /tmp/arp.pcapng (254 packets)
  ...
  tags:
  - imported
layers:
- protocol: ethernet2
  fields:
    dst: ff:ff:ff:ff:ff:ff
    src: 02:00:00:00:00:01
    ethertype: auto
- protocol: arp
  fields:
    htype: ethernet
    ptype: ipv4
    hlen: 6
    plen: 4
    oper: request
    sha: 02:00:00:00:00:01
    spa: 192.168.1.10
    tha: 00:00:00:00:00:00
  generators:
    tpa:
      kind: range
      start: 192.168.1.1
      end: 192.168.1.254
      step: 1
- protocol: raw
  fields:
    data: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
stream:
  mode: zip
  count: 254
```

3 つのことが起きています。

1. **フィールドは人間向け表記** (`192.168.1.10`、`request`、`ethernet`) になり、wpg が再計算できるフィールドは、そうしても取り込んだバイト列が再現できる場合に限り `auto` に戻されます。チェックサムや長さが誤ったキャプチャでは、そのフィールドが手動値のまま残るので、わざと壊したトラフィックも失われません。
2. **ジェネレータ推定。** 254 フレームすべてが同じレイヤ構成で、`arp.tpa` が毎回 +1 なので、(厳密な) `range` ジェネレータになりました。周期的な繰り返しは `list`、少数の値の集合は近似の `list`、それ以外は観測した最小/最大の `random` になります (近似のときは警告が出ます)。`--no-detect-generators` で先頭パケットの値だけを使うこともできます。
3. **ARP の後ろの 0 が 18 オクテット** は、キャプチャに含まれていた Ethernet のパディングです。インポータはプロトコルに帰属できないバイトを `raw` レイヤに残し、フレームを正確に再現します。このレイヤは削除して構いません。`packet.pad_to_min` (既定) で wpg が自分でパディングします。

## ステップ 3. lint して再レンダリングする

```console
$ wpg template lint /tmp/arp.wpg.yaml
/tmp/arp.wpg.yaml: no issues
0 error(s), 0 warning(s) in 1 file(s)
$ wpg render /tmp/arp.wpg.yaml -o /tmp/arp2.pcapng
Wrote 254 packets to /tmp/arp2.pcapng.
$ diff <(tshark -r /tmp/arp.pcapng -x) <(tshark -r /tmp/arp2.pcapng -x) && echo identical
identical
```

フレームはバイト単位で一致します (タイムスタンプは wpg が独自に付けるので異なります)。

## ステップ 4. 編集して再利用する

テンプレートになったので、何でも変更できます。

* 定数を変数にする: `192.168.1.10` を `${src_ip}` に置き換え、`variables: [{ name: src_ip, default: 192.168.1.10 }]` を追加して、`wpg render … --var src_ip=10.9.8.7` を実行します。
* スイープを広げる: `range` を `192.168.0.1-192.168.7.254` に変更します。
* `oper: reply` にするとキャプチャを応答に変えられます。
* `packet: { fcs: bad }` を加えると、不正な FCS を機器がどう扱うか試せます。

## ステップ 5. パケットごとのテンプレート、貼り付けた 16 進

スイープではなく会話のキャプチャなら、パケットごとに 1 テンプレートを作らせます。

```console
$ wpg import /tmp/capture.pcap --per-packet /tmp/templates/
```

Wireshark の「Copy → as a Hex Stream」、`xxd`/`hexdump -C` の出力、空白区切りのバイト列はパイプで渡せます。

```console
$ echo "ff ff ff ff ff ff 02 00 00 00 00 01 08 06 00 01 08 00 06 04 00 01 02 00 00 00 00 01 c0 00 02 01 00 00 00 00 00 00 c0 00 02 02" \
    | wpg import --hex - -o -
```

Ethernet II 以外で始まるキャプチャには `--first-protocol` を使います。wpg が知らないプロトコルのフレームは、デコードできた最後のレイヤの後ろに `raw` バイト列として残るので、再生はできます。

## ステップ 6. 共有と再生

テンプレートと元のキャプチャを、同僚がインポートできるバンドルにまとめます。

```console
$ wpg template export /tmp/arp.wpg.yaml -o /tmp/arp.wpgbundle --attach /tmp/arp.pcapng
$ wpg template import /tmp/arp.wpgbundle -o ./my-templates
```

ほかのテンプレートと同様に、好きなレートで送信します。

```console
$ wpg send /tmp/arp.wpg.yaml -i eth1 --rate 500pps
```

オプションは [負荷とレート制御](rate-control.md) を参照してください。

## 次のステップ

* ファイルに書ける内容は [テンプレートスキーマ](../reference/template-schema.md)。
* [ARP スイープ](arp-sweep.md)、[VLAN タグ付き TCP SYN 大量送信](vlan-syn-flood.md)、[IPv6 と近隣探索 (NDP)](ipv6-ndp.md)。
