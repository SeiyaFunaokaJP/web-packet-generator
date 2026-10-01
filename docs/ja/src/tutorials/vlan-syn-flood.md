# チュートリアル: VLAN タグ付き TCP SYN 大量送信

所要時間: 約 20 分。必要な権限: ステップ 5 までは不要です。

## 作るもの

802.1Q VLAN 内で、送信元アドレス・送信元ポート・初期シーケンス番号をランダムにした TCP SYN を 10 万個送るストリームです。一般的な SYN オプション (MSS、SACK-permitted、ウィンドウスケール) を付けます。多数のクライアントからファイアウォールやロードバランサ、コネクション追跡テーブルに負荷をかけ、SYN の扱いを観察する用途です。wpg はステートレスで、ハンドシェイクを完了させず SYN-ACK にも応答しません。

自分が所有する、または試験を許可されたラボ機器に対してのみ使用してください。

## 始める前に

wpg をビルドし、リポジトリのルートで実行します ([Linux へのインストール](../getting-started/install-linux.md))。使うテンプレートは `templates/scenarios/vlan-syn-flood.wpg.yaml` です。より小さい例として `templates/examples/vlan-tcp-syn.wpg.yaml` もあります。

## ステップ 1. テンプレートを読む

```yaml
layers:
  - protocol: ethernet2
    fields: { dst: ${dst_mac}, src: ${src_mac} }

  - protocol: dot1q                 # TPID 0x8100 は ethernet2 に自動で入る
    fields:
      vid: ${vid}

  - protocol: ipv4
    fields:
      dst: ${dst_ip}
      ttl: 64
      flags: df
    generators:
      src: { kind: random, min: 10.0.0.1, max: 10.255.255.254 }

  - protocol: tcp
    fields:
      dst_port: ${dst_port}
      flags: syn
      window: 64240
      options:
        - mss: 1460
        - sack_permitted
        - ws: 7
    generators:
      src_port: { kind: random, min: 1024, max: 65535 }
      seq: { kind: random, min: 0, max: 4294967295 }

stream:
  seed: 7
  count: 100000

tx:
  rate: 50kpps
  burst: 32
```

知っておくと良い点です。

* **それ以外は何も書いていません。** IPv4 の `ihl`・`total_length`・`protocol`・`header_checksum`、TCP の `data_offset` と `checksum` (IPv4 疑似ヘッダ込み)、dot1q/Ethernet の EtherType はすべて Auto で、フレームごとに再計算されます。TCP オプション領域は 32 ビット境界まで自動でパディングされます。
* **オプションは型付きです。** `mss: 1460`、`sack_permitted`、`ws: 7`、`nop`、`timestamps: { tsval: 1000, tsecr: 0 }` が組み込み名で、それ以外は `{ type: 99, data: "01 02" }` と書けます。
* **`stream.seed` でランダムを再現可能にします。** 同じ seed なら、実行するたび、マシンが違っても同じ 10 万フレームになります。
* **`tx.rate` と `tx.burst`** は既定値で、`wpg send --rate` で上書きできます。

## ステップ 2. 最初のフレームを見る

```console
$ wpg render templates/scenarios/vlan-syn-flood.wpg.yaml -o /tmp/syn.pcapng --count 3
Wrote 3 packets to /tmp/syn.pcapng.
$ tshark -r /tmp/syn.pcapng -o tcp.check_checksum:TRUE
    1   0.000000 10.114.228.243 → 198.51.100.7 TCP 70 2311 → 443 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM WS=128
    2   0.000001 10.99.228.10 → 198.51.100.7 TCP 70 51321 → 443 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM WS=128
    3   0.000002 10.251.163.120 → 198.51.100.7 TCP 70 10483 → 443 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM WS=128
```

Wireshark はチェックサム不正を報告しません。(`Seq=0` は Wireshark が表示する相対シーケンス番号で、実際の値はランダムです。) `tshark -V -c 1` で `ID: 100` の 802.1Q ヘッダも確認できます。

## ステップ 3. ファイルを編集せずに宛先を変える

`${...}` はすべて変数で、既定値はファイル先頭の `variables:` にあります。コマンドラインで上書きします。

```console
$ wpg render templates/scenarios/vlan-syn-flood.wpg.yaml -o /tmp/syn.pcapng \
      --var vid=200 --var dst_ip=203.0.113.9 --var dst_port=8443 --count 1000
```

MAC アドレスは `--var src_mac=…` と `--var dst_mac=…` で設定します。宛先 MAC は、送信インタフェースから見た試験対象 (ネクストホップ) の MAC にします。

## ステップ 4. count・seed・事前レンダリング

ストリームの長さは固定 (`stream.count: 100000`) です。`wpg send` は小さなストリームをメモリに事前レンダリングし (既定のしきい値 256 MiB)、大きなものはその場で展開します。`--plan prerender` または `--plan onthefly` で強制できます。

## ステップ 5. 送信する

まず予行演習をしてから、ラボのインタフェースに送信します ([権限設定](../getting-started/permissions.md))。

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --rate 20kpps --count 5000
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml -i eth1 --rate 50kpps \
      --var vid=200 --var dst_ip=203.0.113.9 --var dst_mac=00:11:22:33:44:55
```

最初のバーストで落ちる機器には、緩やかに上げていくのが有効です。`--ramp-up 10s` はレートをゼロから直線的に上げます。詳しくは [負荷とレート制御](rate-control.md) を参照してください。

## ステップ 6. 結果を読む

* ファイアウォールやロードバランサの接続数・SYN カウンタ (`show conn count`、`conntrack -C`、SYN cookie カウンタ) を見ます。
* Linux の対象機器では `nstat -az TcpExtTCPReqQFullDoCookies` と `ss -s` で SYN キューの挙動が分かります。
* 送信側では、`wpg send` 終了時のサマリに packets・bytes・dropped と、目標に対する達成レートが出ます。

## 次のステップ

* `flags: syn` を `syn|ack`、`rst`、`fin|ack` に変えると、ステートフル機器が状態外のセグメントをどう扱うかを試せます。
* 宛先ポート 1-1024 を走査するには `templates/scenarios/tcp-syn-port-scan.wpg.yaml` を使います。
* 実際のキャプチャを編集できるテンプレートにするには [pcap からテンプレートへ変換して再生](pcap-replay.md) を参照してください。
