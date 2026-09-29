# チュートリアル: ARP スイープ

所要時間: 約 15 分。必要な権限: ステップ 5 までは不要です (それまではドライランです)。

## 作るもの

/22 の全ホストアドレスに対する ARP 要求 (「10.0.0.x は誰?」) を順に送るストリームと、同じスイープを複数の VLAN の中で繰り返すストリームです。ARP キャッシュを埋める、スイッチのブロードキャスト転送を確認する、といった用途に使えます。

同梱の 3 つのテンプレートを使います。

| テンプレート | フレーム数 | 内容 |
|---|---|---|
| `templates/examples/arp-sweep.wpg.yaml` | 254 | 最小のスイープ (range ジェネレータ 1 つ) |
| `templates/scenarios/arp-sweep-slash22.wpg.yaml` | 1022 | CIDR ブロックをジェネレータにする |
| `templates/scenarios/vlan-arp-sweep.wpg.yaml` | 1016 | 2 つのジェネレータ (VLAN x アドレス) を `nested` で組み合わせる |

## 始める前に

wpg を一度ビルドし ([Linux](../getting-started/install-linux.md)、[macOS](../getting-started/install-macos.md)、[Windows](../getting-started/install-windows.md))、リポジトリのルートでコマンドを実行します。例ではバイナリを `wpg` と書いています。PATH に無い場合は `./target/release/wpg` に読み替えてください。`tshark` は必須ではありませんが確認に役立ちます。

送信してよいのは、自分が所有する、または試験を許可されたネットワークだけです ([法的注意](../legal.md))。

## ステップ 1. 最小のスイープを読む

```yaml
layers:
  - protocol: ethernet2
    fields:
      dst: ff:ff:ff:ff:ff:ff
      src: ${src_mac}

  - protocol: arp
    fields:
      oper: request
      sha: ${src_mac}
      spa: ${src_ip}
      tha: 00:00:00:00:00:00
    generators:
      tpa: ${first}-${last}   # range ジェネレータの短縮表記
```

`generators.tpa` がストリーム全体で ARP のターゲット IP を変化させます。`${...}` はテンプレート変数で、実行時に `--var 名前=値` で上書きできます。Ethernet の `ethertype` は書いていません。Auto なので、次のレイヤが ARP であることから `0x0806` になります。

最初のフレームを表示します。

```console
$ wpg template show templates/scenarios/vlan-arp-sweep.wpg.yaml
Hex dump (60 bytes)
0000  ff ff ff ff ff ff 02 00  00 00 00 01 81 00 00 64  ...............d
0010  08 06 00 01 08 00 06 04  00 01 02 00 00 00 00 01  ................
0020  0a 00 00 fa 00 00 00 00  00 00 0a 00 00 01 00 00  ................
0030  00 00 00 00 00 00 00 00  00 00 00 00              ................
...
```

末尾はゼロパディングです。ARP 要求は 42 オクテットですが、Ethernet のフレームは 60 オクテット必要です (FCS は NIC が付けます)。

## ステップ 2. /22 全体をスイープする

`arp-sweep-slash22.wpg.yaml` は、変数に入れた CIDR ブロックをジェネレータにします。

```yaml
variables:
  - name: net
    default: 10.0.0.0/22
layers:
  - protocol: arp
    generators:
      tpa: ${net}
```

CIDR ジェネレータはホストアドレスだけを返すので、/22 では 1022 フレーム (`10.0.0.1` から `10.0.3.254`) になります。ネットワークには触れず pcapng に書き出します。

```console
$ wpg render templates/scenarios/arp-sweep-slash22.wpg.yaml -o /tmp/sweep.pcapng
Wrote 1022 packets to /tmp/sweep.pcapng.
```

ファイルを編集せず別のサブネットにすることもできます。

```console
$ wpg render templates/scenarios/arp-sweep-slash22.wpg.yaml -o /tmp/sweep.pcapng --var net=10.1.0.0/22
```

## ステップ 3. 複数の VLAN で繰り返す

`vlan-arp-sweep.wpg.yaml` は 802.1Q タグを挿入し、ジェネレータをもう 1 つ追加します。

```yaml
  - protocol: dot1q
    generators:
      vid: ${vids}          # 既定 100-103

  - protocol: arp
    generators:
      tpa: ${first}-${last} # 既定 10.0.0.1-10.0.0.254

stream:
  mode: nested
  order: [dot1q.vid, arp.tpa] # 遅い方から
```

ジェネレータが 2 つあるときは組み合わせ方を指定します。`zip` は同時に進め、`product` は直積、`nested` は順序を明示した直積です。ここでは VLAN が遅いループなので、VLAN 100 で 254 アドレス、次に VLAN 101、と続き、4 x 254 = 1016 フレームになります。リストで 2 つの VLAN に絞ることもできます。

```console
$ wpg render templates/scenarios/vlan-arp-sweep.wpg.yaml -o /tmp/vlan-sweep.pcapng --var vids=100,200
Wrote 508 packets to /tmp/vlan-sweep.pcapng.
```

## ステップ 4. フレームを確認する

```console
$ tshark -r /tmp/vlan-sweep.pcapng -T fields -e vlan.id -e arp.dst.proto_ipv4 | sed -n '1p;254p;255p;$p'
100	10.0.0.1
100	10.0.0.254
200	10.0.0.1
200	10.0.0.254
```

テンプレートを編集したら `wpg template lint` を実行してください。未知のフィールド、範囲外の値、ジェネレータの書式誤りを送信前に検出できます。

## ステップ 5. 送信する

まずファイル出力 (`pcapfile`。`-i` が出力パスになります) または破棄 (`null`) のバックエンドで予行演習します。

```console
$ wpg send templates/scenarios/arp-sweep-slash22.wpg.yaml --backend null -i lo --rate 100kpps
Sending on lo via null: rate 100000pps, burst 1, 1022 packets, 1 worker(s), plan PreRender (...)
state      finished (stream exhausted)
packets    1022
bytes      61320
```

次にラボのインタフェースへ実際に送信します。生ソケットの送信には権限が必要です ([権限設定](../getting-started/permissions.md))。`wpg ifaces` でインタフェースを確認し、バックエンドが拒否されたら `wpg doctor` を実行します。

```console
$ wpg ifaces
$ wpg send templates/scenarios/vlan-arp-sweep.wpg.yaml -i eth1 --rate 2000pps
```

ストリームを 1 回送信して (1016 フレームを 2000 fps で約 0.5 秒) サマリを表示します。`--count 10000` でスイープを繰り返し、`--duration 30s` で時間指定もできます。

## ステップ 6. 結果を読む

* 試験対象の ARP/ネイバーテーブル (`ip neigh`、`show ip arp`) とスイッチの MAC テーブルを確認します (送信元 MAC `02:00:00:00:00:01` が使った VLAN に現れるはずです)。
* 同じセグメントの Linux ホストで `tcpdump -e -n -i eth1 arp` を実行すると、VLAN タグ付きの要求が見えます。
* スイープの送信元アドレスは架空の `10.0.0.250` です。応答を観測したいときは `--var` で `src_mac` と `src_ip` を変更します。

## 次のステップ

* ランダムな送信元や TCP オプションは [VLAN タグ付き TCP SYN 大量送信](vlan-syn-flood.md)。
* `--rate` や `--duration`、複数ワーカーは [負荷とレート制御](rate-control.md)。
* ジェネレータの種類は [テンプレートスキーマ](../reference/template-schema.md)。
