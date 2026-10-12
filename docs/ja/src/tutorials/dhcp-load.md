# チュートリアル: DHCP サーバの負荷試験

所要時間: 約 25 分。必要な権限: ステップ 3 まで不要、ステップ 4 から root (名前空間と afpacket)。

## 作るもの

多数のクライアントを装って DHCPDISCOVER を送り、DHCP サーバが返す DHCPOFFER を数えて、サーバがどのレートまで追従できるかを調べます。1 フレームごとに送信元 MAC と `chaddr` (クライアントのハードウェアアドレス) を揃えて変え、トランザクション ID (`xid`) をランダムにします。サーバには、1 台の Linux 上の名前空間で動かす dnsmasq を使います。

wpg はステートレスです。OFFER を受け取っても DHCPREQUEST は返さないので、DORA (Discover / Offer / Request / Ack) は完了せず、リースも作られません。測れるのは DISCOVER の処理能力で、アドレスプールの枯渇試験ではありません (ステップ 6)。

自分が所有する、または試験を許可された DHCP サーバに対してのみ使用してください。本番セグメントに DISCOVER を大量に流すと、本物のクライアントがアドレスを取れなくなることがあります。

## 始める前に

wpg をビルドし、リポジトリのルートで実行します ([Linux へのインストール](../getting-started/install-linux.md))。使うテンプレートは `templates/scenarios/dhcp-discover-flood.wpg.yaml` です。1 フレームだけの例として `templates/protocols/app/dhcpv4-discover.wpg.yaml` もあります。ステップ 4 以降は Linux、iproute2、dnsmasq (`sudo apt install dnsmasq-base`) と root が必要です。

## ステップ 1. テンプレートを読む

```yaml
variables:
  - name: oui
    default: "02:00:5e"

layers:
  - protocol: ethernet2
    fields:
      dst: "ff:ff:ff:ff:ff:ff"
    generators:
      src: { kind: range, start: "${oui}:00:00:01", end: "${oui}:ff:ff:ff" } # one MAC per client
  - protocol: ipv4
    fields:
      src: 0.0.0.0
      dst: 255.255.255.255
  - protocol: udp
    fields:
      src_port: 68
      dst_port: 67
  - protocol: dhcpv4
    fields:
      op: boot_request
      flags: broadcast
      options:
        - { message_type: discover }
        - { parameter_request_list: [1, 3, 6, 15, 51] } # mask, router, DNS, domain, lease
        - end
    generators:
      chaddr: { kind: expr, expr: "$0.src" } # the same MAC as the Ethernet source (layer 0)
      xid: { kind: expr, expr: "rand(0, 0xffffffff)" }

stream:
  seed: 1
  count: 10000

tx:
  rate: 200pps
```

知っておくと良い点です。

* **送信元 MAC は連番です。** `range` は MAC アドレスにも使え、`02:00:5e:00:00:01` から 1 ずつ増えます。先頭オクテットの `02` はローカル管理アドレスで、実在の機器と重なりません。`--var oui=…` で上位 3 オクテットを変えられます。
* **`chaddr` は式で送信元 MAC を写します。** `expr` ジェネレータの `$0.src` は「レイヤ 0 (ethernet2) の `src` ジェネレータの、このフレームでの値」です。サーバはふつう `chaddr` でクライアントを識別するので、2 つがずれると試験になりません。式の書き方は [テンプレートスキーマ](../reference/template-schema.md#式-expr) を参照してください。
* **`xid` はフレームごとのランダム値です。** `rand(min, max)` はフレーム番号から決まる乱数で、`stream.seed` が同じなら何度実行しても同じ値になります。
* **書いていないフィールドは Auto か既定値です。** IPv4 の長さとチェックサム、UDP の長さとチェックサム (疑似ヘッダ込み) は Auto で、DHCP の `htype` / `hlen`、`sname` と `file` のゼロ埋め、マジッククッキーは既定値で入ります。

## ステップ 2. 最初のフレームを見る

```console
$ wpg render templates/scenarios/dhcp-discover-flood.wpg.yaml -o /tmp/dhcp.pcapng --count 3
Wrote 3 packets to /tmp/dhcp.pcapng.
$ tshark -r /tmp/dhcp.pcapng
    1   0.000000      0.0.0.0 → 255.255.255.255 DHCP 293 DHCP Discover - Transaction ID 0xc4c98940
    2   0.000001      0.0.0.0 → 255.255.255.255 DHCP 293 DHCP Discover - Transaction ID 0xef7b0650
    3   0.000002      0.0.0.0 → 255.255.255.255 DHCP 293 DHCP Discover - Transaction ID 0x330355d7
$ tshark -r /tmp/dhcp.pcapng -o udp.check_checksum:TRUE -T fields -e eth.src -e dhcp.hw.mac_addr -e udp.checksum.status
02:00:5e:00:00:01	02:00:5e:00:00:01	1
02:00:5e:00:00:02	02:00:5e:00:00:02	1
02:00:5e:00:00:03	02:00:5e:00:00:03	1
```

送信元 MAC と `Client MAC address` が一致し、UDP チェックサムは正しい (`1` = Good) と判定されます。1 万フレーム全体でも確かめられます。

```console
$ wpg render templates/scenarios/dhcp-discover-flood.wpg.yaml -o /tmp/dhcp-all.pcapng
Wrote 10000 packets to /tmp/dhcp-all.pcapng.
$ tshark -r /tmp/dhcp-all.pcapng -Y 'eth.src != dhcp.hw.mac_addr' | wc -l
0
```

## ステップ 3. 規模を変える

クライアント数は `--count` で決まります (`stream.count` の 1 万を超えると MAC の連番が先頭に戻るので、それ以上は `--var oui=…` を変えた 2 本目のストリームにします)。

```console
$ wpg render templates/scenarios/dhcp-discover-flood.wpg.yaml -o /tmp/dhcp-b.pcapng --var oui=02:00:5f
Wrote 10000 packets to /tmp/dhcp-b.pcapng.
```

このファイルの送信元 MAC は `02:00:5f:00:00:01` から `02:00:5f:00:27:10` (10000 = 0x2710) です。

## ステップ 4. 試験用の DHCP サーバを用意する

名前空間 `wpgdhcp` に dnsmasq を起動し、veth (`dh0` ⇔ `dhs0`) で wpg とつなぎます。プールは 50 アドレスです。

```sh
sudo ip netns add wpgdhcp
sudo ip link add dh0 type veth peer name dhs0
sudo ip link set dhs0 netns wpgdhcp
sudo ip link set dh0 up
sudo ip netns exec wpgdhcp ip link set lo up
sudo ip netns exec wpgdhcp ip link set dhs0 up
sudo ip netns exec wpgdhcp ip addr add 10.98.0.1/24 dev dhs0
sudo ip netns exec wpgdhcp dnsmasq --no-daemon --port=0 --interface=dhs0 --bind-interfaces \
    --dhcp-range=10.98.0.10,10.98.0.59,10m --dhcp-authoritative --dhcp-leasefile=/tmp/wpg-leases \
    --log-dhcp --log-facility=/tmp/wpg-dnsmasq.log &
```

`--port=0` は DNS を無効にします。実機の DHCP サーバやルータを試験する場合は、`dh0` の代わりにそのサーバにつながるラボのインタフェースを使います。

## ステップ 5. 送って OFFER を数える

OFFER (送信元ポート 67) を `wpg capture` で数えながら、別の端末から DISCOVER を 100 個送ります。

```console
$ sudo wpg capture -i dh0 -f "udp src port 67" --duration 3s --show 2 -w /tmp/offers.pcapng
$ sudo wpg send templates/scenarios/dhcp-discover-flood.wpg.yaml -i dh0 --backend afpacket --count 100
```

```console
Capturing on dh0 via afpacket, filter udp src port 67 (Ctrl-C stops)
#   0  16:14:33.696984775    342 B  ethernet2 / ipv4 / udp / dhcpv4
#   1  16:14:33.703225873    342 B  ethernet2 / ipv4 / udp / dhcpv4
state      finished (duration elapsed)
...
matched    100 pkts  34200 bytes
saved      100 pkts to /tmp/offers.pcapng
$ tshark -r /tmp/offers.pcapng -c 3
    1   0.000000    10.98.0.1 → 255.255.255.255 DHCP 342 DHCP Offer    - Transaction ID 0xc4c98940
    2   0.006241    10.98.0.1 → 255.255.255.255 DHCP 342 DHCP Offer    - Transaction ID 0xef7b0650
    3   0.009784    10.98.0.1 → 255.255.255.255 DHCP 342 DHCP Offer    - Transaction ID 0x330355d7
```

OFFER の `xid` は DISCOVER のもの (ステップ 2) と一致します。`broadcast` フラグを立てているので、OFFER は `255.255.255.255` 宛てに返ります。

**最初はとても遅く見えるはずです。** dnsmasq の既定の設定では、OFFER の前にそのアドレスへ ICMP Echo を送って使われていないことを確かめるため、DISCOVER 1 件に約 3 秒かかり、3 秒の捕捉では 1 件しか数えられません (ログ `/tmp/wpg-dnsmasq.log` に 3 秒おきの `DHCPOFFER` が並びます)。上の結果は dnsmasq に `--no-ping` を付けて起動し直したものです。このような「サーバ側の作りで決まる上限」を見つけることが、この種の試験の目的の 1 つです。

次にレートを上げて、OFFER が DISCOVER に追い付かなくなる点を探します。

```sh
for r in 1kpps 5kpps 20kpps; do
  sudo wpg capture -i dh0 -f "udp src port 67" --duration 4s --show 0 > /tmp/cap-$r.txt &
  sudo wpg send templates/scenarios/dhcp-discover-flood.wpg.yaml -i dh0 --backend afpacket --count 2000 --rate $r
  wait; grep '^matched' /tmp/cap-$r.txt
done
```

開発用コンテナ (`--no-ping`) での結果です。値はホストとサーバ次第です。

| 送出レート | DISCOVER | OFFER | 応答率 |
|---|---|---|---|
| 1 kpps | 2000 | 2000 | 100% |
| 5 kpps | 2000 | 2000 | 100% |
| 20 kpps | 2000 | 1175 | 59% |

応答率が下がり始めるレートが、このサーバの DISCOVER 処理能力の目安です。応答しなかった DISCOVER は、サーバのソケットの受信バッファで捨てられています。実機では、DHCP スヌーピングや CoPP (制御プレーンのポリシング) が先に落とすこともあります。その場合はスイッチやルータのカウンタも確認してください。

## ステップ 6. 結果を読む

* **OFFER はアドレスを予約しません。** プールは 50 アドレスですが、100 個の DISCOVER すべてに OFFER が返り、提示されたアドレスは 50 種類でした (`tshark -r /tmp/offers.pcapng -T fields -e dhcp.ip.your | sort -u | wc -l`)。dnsmasq をはじめ多くのサーバは REQUEST / ACK でリースを作るので、DISCOVER だけではプールは枯渇しません。リース ファイル (`/tmp/wpg-leases`) も空のままです。
* **リースまで作る試験は wpg の対象外です。** OFFER を受けて REQUEST を返すのはステートフルな処理で、wpg の非ゴールです。プールの枯渇やリース保持の試験には、DHCP クライアントのエミュレータ (perfdhcp など) を使ってください。
* **リレー経由の試験**には `templates/protocols/app/dhcpv4-relay.wpg.yaml` (`giaddr` とオプション 82 付き、ユニキャスト) を元にします。宛先をサーバのアドレスにすれば、ルーティングされた先のサーバを試験できます。
* DHCPv6 には `templates/protocols/app/dhcpv6-solicit.wpg.yaml` と `dhcpv6-relay-forward.wpg.yaml` があります。

後片付け:

```sh
sudo pkill -f 'dnsmasq --no-daemon --port=0 --interface=dhs0'
sudo ip netns del wpgdhcp      # dh0 も一緒に消えます
```

## 次のステップ

* `--rate` の代わりに `--ramp-up 30s` を付けると、レートを直線的に上げながら応答率の落ち始めを探せます。詳しくは [負荷とレート制御](rate-control.md)。
* GUI では、テンプレート画面でこのシナリオを開いてジョブから送出し、受信画面 (`/rx`) でフィルタ `udp src port 67` を指定して OFFER を数えられます ([受信と検証](../operations/rx-verify.md))。
