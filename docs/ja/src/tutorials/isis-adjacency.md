# チュートリアル: IS-IS Hello で隣接を Up にする

所要時間: 約 30 分。必要な権限: ステップ 2 まで不要、ステップ 3 から root (名前空間と afpacket)。

## 作るもの

IS-IS のレベル 2 LAN Hello (IIH) を組み立てて FRR の isisd に送り、相手側で隣接が `Initializing` から `Up` になるまでを追います。題材は相互接続確認用のテンプレート `templates/interop/isis-l2-lan-hello-up.wpg.yaml` です。Ethernet の上に LLC (`0xfe`) と IS-IS が直接載る、IP を使わない OSI 系のスタックで、PDU 長、MTU までのパディング、IS Neighbors TLV による双方向確認を順に見ます。

wpg はステートレスです。Hello を送り続ければ隣接は維持されますが、LSP のフラッディングや DIS としての振る舞いはしません。

自分が所有する、または試験を許可されたルータに対してのみ使用してください。運用中の IS-IS ドメインに偽の隣接を作ると、経路計算に影響することがあります。

## 始める前に

wpg をビルドし、リポジトリのルートで実行します ([Linux へのインストール](../getting-started/install-linux.md))。ステップ 3 以降は Linux、iproute2、FRR (`sudo apt install frr`。デーモンはスクリプトが直接起動するので、`frr` サービスは止めたままで構いません) と root が必要です。IS-IS の PDU と TLV の一覧は [IS-IS (P2)](../protocols/isis.md) にあります。

## ステップ 1. テンプレートを読む

```yaml
layers:
  - protocol: dot3
    fields:
      dst: "01:80:c2:00:00:15" # AllL2ISs
      src: ${src_mac}
      length: auto
  - protocol: llc
  - protocol: isis_l2_lan_hello
    fields:
      circuit_type: l2
      source_id: ${system_id}
      holding_time: 30
      priority: 0 # wpg does not flood LSPs, so let the peer be the DIS
      lan_id: "0000 0000 0001 01" # overwritten by the DIS's LAN ID once the peer elects itself
      # Manual PDU length: Padding TLVs fill the PDU up to it (MTU 1500 - LLC 3).
      pdu_length: 1497
      tlvs:
        - { protocols_supported: [ipv4] }
        - { area_addresses: [{ address: "${area}" }] }
        - { ip_interface_addresses: ["${ip}"] }
        - { is_neighbors: ["${peer_mac}"] }

tx:
  count: 40 # two minutes of Hellos
  rate: ifg=3s
```

知っておくと良い点です。

* **IS-IS は IP を使いません。** `dot3` (長さフィールド付きの 802.3) の上に `llc` を置くと、DSAP / SSAP は `0xfe` (OSI) に自動で決まります。IS-IS の NLPID `0x83` も PDU レイヤから自動で入ります。宛先 `01:80:c2:00:00:15` はレベル 2 の全 IS 宛て (AllL2ISs) です。
* **`pdu_length` を Manual にするとパディングが付きます。** Auto のままなら PDU は TLV の分だけの長さです。値を書くと、PDU がちょうどその長さになるまで Padding TLV (種別 8) が自動で足されます。1497 は Ethernet の MTU 1500 から LLC ヘッダ 3 オクテットを引いた値です。相手は、Hello がこの大きさで届くことで MTU が一致していると判断します。
* **`is_neighbors` が双方向確認の鍵です。** LAN の隣接は、相手の Hello に自分の MAC (SNPA) が載っているのを見て初めて `Up` になります。`peer_mac` には相手インタフェースの MAC を入れます。
* **`priority: 0`** で DIS (代表 IS) の選出に負けるようにしています。wpg は DIS の仕事 (CSNP の送出や疑似ノード LSP) をしないためです。
* System ID (`source_id`) は 6 オクテット、エリアアドレスは NSAP の表記 (`49.0001`) で書きます。

## ステップ 2. フレームを見る

```console
$ wpg render templates/interop/isis-l2-lan-hello-up.wpg.yaml -o /tmp/isis.pcapng --count 1
Wrote 1 packets to /tmp/isis.pcapng.
$ tshark -r /tmp/isis.pcapng
    1   0.000000 02:00:00:00:00:01 → ISIS-all-level-2-IS's ISIS HELLO 1514 L2 HELLO, System-ID: 0000.0000.0001
$ tshark -r /tmp/isis.pcapng -V
...
ISIS HELLO
    .... ..10 = Circuit type: Level 2 only (0x2)
    SystemID {Sender of PDU}: 0000.0000.0001
    Holding timer: 30
    PDU length: 1497
    .000 0000 = Priority: 0
    SystemID {Designated IS}: 0000.0000.0001.01
    Protocols Supported (t=129, l=1)
    Area address(es) (t=1, l=4)
        Area address (3): 49.0001
    IP Interface address(es) (t=132, l=4)
        IPv4 interface address: 10.99.0.1
    IS Neighbor(s) (t=6, l=6)
        IS Neighbor: 02:00:00:00:00:02 (02:00:00:00:00:02)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=160)
```

(`-V` の出力は抜粋です。) フレームは 1514 オクテット (MTU 1500 + Ethernet ヘッダ 14) です。PDU 1497 オクテットの内訳は、ヘッダ 27、上の 4 つの TLV 23、Padding TLV 6 個 (255 オクテットが 5 個と 160 オクテットが 1 個、それぞれ種別と長さの 2 オクテットを加えて 1447) です。

## ステップ 3. FRR のラボを用意する

`scripts/interop/frr-netns.sh` が、名前空間 `wpgfrr` に FRR を起動し、veth (`wpg0` ⇔ `frr0`) で wpg とつなぎます。FRR 側の IS-IS は `net 49.0001.0000.0000.0002.00`、`is-type level-2-only` です。`--keep` を付けると、相互接続確認のあともラボが残ります。

```console
$ cargo build --release -p wpg-cli
$ sudo scripts/interop/frr-netns.sh --keep target/release/wpg
...
PASS  IS-IS L2 adjacency Up
...
lab kept: ip netns exec wpgfrr vtysh -N wpg; tear down with scripts/interop/frr-netns.sh --down
```

スクリプト自身の確認で隣接が一度 `Up` になっています。最後の Hello から保持時間 (30 秒) が過ぎて消えるまで待ってから先に進みます。

```console
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor"
Area 1:
  System Id           Interface   L  State        Holdtime SNPA
```

両端の MAC を控えておきます (veth の MAC は作るたびに変わります)。

```sh
SRC=$(cat /sys/class/net/wpg0/address)
PEER=$(sudo ip netns exec wpgfrr cat /sys/class/net/frr0/address)
```

## ステップ 4. 相手の Hello を受信する

FRR は 3 秒ごとに自分の Hello を送っています。`wpg capture` で 1 つ受け取って、wpg のパーサで読めることを確かめます。

```console
$ sudo wpg capture -i wpg0 -f "ether dst 01:80:c2:00:00:15" --count 1 --show 1
Capturing on wpg0 via afpacket, filter ether dst 01:80:c2:00:00:15 (Ctrl-C stops)
#   0  16:10:42.763122508   1514 B  dot3 / llc / isis_l2_lan_hello
state      finished (count reached)
...
```

FRR の Hello も MTU までパディングされた 1514 オクテットで、`dot3 / llc / isis_l2_lan_hello` と解析されます。`-w /tmp/frr.pcapng` で保存して、テンプレートに取り込むこともできます ([pcap からテンプレートへ変換して再生](pcap-replay.md))。

## ステップ 5. 隣接を Up にする

まず、相手の MAC を載せずに Hello を 1 つ送ります。`peer_mac` は既定値の `02:00:00:00:00:02` のままなので、FRR から見ると「自分を知らない相手」です。

```console
$ sudo wpg send templates/interop/isis-l2-lan-hello-up.wpg.yaml -i wpg0 --backend afpacket \
      --var src_mac=$SRC --count 1
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor"
Area 1:
  System Id           Interface   L  State        Holdtime SNPA
 0000.0000.0001      frr0        2  Initializing  29       a63f.1a8d.7239
```

FRR は wpg を見つけましたが、双方向の確認が取れていないので `Initializing` です。次に `peer_mac` に FRR の MAC を入れて送ります。

```console
$ sudo wpg send templates/interop/isis-l2-lan-hello-up.wpg.yaml -i wpg0 --backend afpacket \
      --var src_mac=$SRC --var peer_mac=$PEER --count 2 --rate 1pps
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor"
Area 1:
  System Id           Interface   L  State        Holdtime SNPA
 0000.0000.0001      frr0        2  Up            30       a63f.1a8d.7239
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor detail"
Area 1:
 0000.0000.0001
    Interface: frr0, Level: 2, State: Up, Expires in 30s
    Adjacency flaps: 1, Last: 1s ago
    Circuit type: L2, Speaks: IPv4
    SNPA: a63f.1a8d.7239, LAN id: 0000.0000.0001.01
    LAN Priority: 0, unknown DIS state, DIS flaps: 0, Last: 2s ago
    Area Address(es):
      49.0001
    IPv4 Address(es):
      10.99.0.1
```

テンプレートの TLV (エリア、IPv4 アドレス、優先度 0、保持時間 30 秒) がそのまま FRR に見えています。隣接を保つには、保持時間より短い間隔で Hello を送り続けます。テンプレートの既定 (`--count` なし) は 3 秒間隔で 40 個、2 分間です。止めると 30 秒で隣接が消えます。

## ステップ 6. 変えてみる

* **エリアを変える**: `--var area=49.0002 --var "system_id=0000 0000 0003"` で送っても、隣接は `Up` になります。エリアアドレスの一致が必要なのはレベル 1 の隣接だけで、レベル 2 はエリアをまたいで隣接するためです (ISO 10589)。レベル 1 を試すには、テンプレートの `isis_l2_lan_hello` を `isis_l1_lan_hello` に、宛先を `01:80:c2:00:00:14` (AllL1ISs) に変え、FRR を `is-type level-1` にします。
* **MTU の不一致**: `pdu_length` を Auto に戻す (行を消す) と、パディングのない短い Hello になります。FRR は既定で Hello のパディングを要求しないため隣接は張れますが、パディングを必須にしている実装では張れません。実機での挙動の違いを確かめるのに使えます。
* **認証**: `templates/protocols/routing/isis-l2-lan-hello.wpg.yaml` は平文認証 (TLV 10) 付きの例です。HMAC-MD5 の値は wpg が計算しないので、手で入れる必要があります。
* **他の PDU**: LSP・CSNP・PSNP・ポイントツーポイント Hello のサンプルが `templates/protocols/routing/isis-*.wpg.yaml` にあります。LSP のチェックサム (Fletcher) は Auto です。

後片付け:

```sh
sudo scripts/interop/frr-netns.sh --down
```

## 次のステップ

* 実機 (IOS-XR、Junos) や CML2 上のルータで同じ確認をする手順と設定例は [相互接続確認](../operations/interop.md) にあります。
* 同じ考え方の IPv6 版 (型・長さ・チェックサムが互いに導出される複数レイヤ) は [IPv6 と近隣探索 (NDP)](ipv6-ndp.md) で扱います。
