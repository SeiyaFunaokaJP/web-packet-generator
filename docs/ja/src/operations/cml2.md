# CML2 での注意点

Status: draft

最初のデプロイ先は、virtio-net インタフェースを持つ Cisco CML2 上の Ubuntu ノードで、`afpacket` バックエンドが主経路です。このページでは準備と注意点をまとめます。

> **検証状況**: wpg 自身の挙動 (doctor のチェック、エラーメッセージ、フラグ) はコードとコンテナ上での `wpg` 実行に基づきます。**CML2 上での実測はまだ行っていません。** `docs/perf.md` の数値はすべて `lo` と `null` バックエンドで測ったもので、afpacket の上限の目安にすぎません。以下の CML2 固有の記述 (ブリッジの挙動、MAC フィルタ) は一般的な Linux/KVM の知識であり、お使いの CML2 バージョンでの確認が必要です。M2 の「CML2 上での実測」は未完了です。

## 前提条件 (Prerequisites)

- virtio-net インタフェースが接続された Ubuntu ノード (22.04 以降、カーネル 5.15 以降推奨)。`wpg doctor` は TPACKET_V3 に 3.2 以上、AF_XDP に 5.10 以上を要求します。
- `pcap` バックエンドも使う場合は `--features pcap` でビルドし、`libpcap0.8` を入れます。`afpacket` にライブラリは不要です。
- vCPU は 2 つ以上。送出は 1 本のビジーなスレッドなので、OS 用に 1 コア空けます。

## ラボの組み方

最小の構成は、wpg を動かす Ubuntu ノード 1 台と試験対象のノードです。受信と照合 ([受信と検証](rx-verify.md)) も行うなら、受信用の Ubuntu ノードをもう 1 台置きます。

```text
External Connector (NAT / ブリッジ) ── ens2 [Ubuntu: wpg] ens3 ── 試験対象 (IOSv、CSR1000v、XRv9k、FRR …) ── ens3 [Ubuntu: 受信側 (任意)]
```

1. **管理用のリンク**: Ubuntu ノードの最初のインタフェース (`ens2`) を External Connector につなぎます。バイナリのコピー (`scripts/deploy.sh`)、ssh、Web UI への接続にだけ使い、試験トラフィックは流しません。
2. **データ用のリンク**: 2 番目以降 (`ens3`、`ens4` …) を試験対象につなぎます。リンクの両端が起動していないとインタフェースは carrier なしのままです。
3. **ノードの大きさ**: vCPU 2 以上 (`--workers N` なら N + 1 以上)、メモリ 2 GiB 以上。DPDK を使うなら hugepages のぶん (既定の 1024 ページで 2 GiB) を足します。
4. **初期設定**: Ubuntu ノードの Config (cloud-init の user-data) で、ssh 公開鍵と、必要なら `ethtool`、`tcpdump`、`libpcap0.8` を入れておくと後が楽です。

   ```yaml
   #cloud-config
   ssh_authorized_keys:
     - ssh-ed25519 AAAA... you@workstation
   packages: [ethtool, tcpdump]
   ```

5. **デプロイ**: 手元のマシンから管理用アドレスに対して `scripts/deploy.sh` を実行します ([scripts/deploy.sh によるデプロイ](deploy.md))。送信を許すインタフェースを `--iface` で限定してください。

   ```sh
   scripts/deploy.sh ubuntu@192.0.2.10 --service --iface ens3 --password-file ./wpg-password
   ```

6. **Web UI**: パスワードを設定して `0.0.0.0:8080` で待ち受けるか、サービスを `--bind 127.0.0.1:8080` にして ssh のポート転送 (`ssh -L 8080:127.0.0.1:8080 ubuntu@192.0.2.10`) で開きます ([認証と TLS](security.md))。

## 権限: setcap か root

AF_PACKET ソケットには `CAP_NET_RAW` が必要です。root で実行するか、バイナリに一度だけ capability を付与します。

```sh
sudo setcap cap_net_raw,cap_net_admin+eip /usr/local/bin/wpg
```

これは `wpg doctor` が (実行中のバイナリの実パスで) 表示するヒントです。バックエンド自体が拒否された場合は次のメッセージが出ます: ``AF_PACKET needs CAP_NET_RAW: run as root or grant it with `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)` ``。ファイルを置き換えると capability は消えるため、コピーのたびに `setcap` をやり直します (`scripts/deploy.sh` が行います)。systemd では file capability の代わりに `AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN` を使います。`nosuid` でマウントされたファイルシステム上では capability は無効です。

## インタフェースの選び方

```console
$ wpg ifaces
NAME  IDX  MAC                IPV4       IPV6  MTU    STATE         SPEED  DRIVER
lo    1    -                  127.0.0.1  -     65536  up, loopback  -      -
ifb0  2    66:00:97:e0:50:14  -          -     1500   down          -      -
ifb1  3    62:7f:e1:7b:e9:80  -          -     1500   down          -      -
eth0  4    02:fc:00:00:00:01  192.0.2.2  -     1400   up            -      virtio_net
```

(上の一覧は開発用コンテナのもので、CML2 のものではありません。) 状態が `up` で、ドライバが `virtio_net` で、試験したいリンクにつながっているインタフェースを選びます。ssh セッションが通っている管理用インタフェースでは送出しないでください。CML2 上の Ubuntu では名前は通常 `ens2`、`ens3` などで、CML2 トポロジ上のインタフェース順に並びます。CML2 の UI に表示される MAC と照合して確認します。MTU 列は最大フレームサイズを決めます (後述)。インタフェースは carrier 付きで `up` である必要があり、CML2 で未接続のインタフェースは carrier なしのままです。

## `wpg doctor` による確認手順

```console
$ wpg doctor
wpg doctor: environment check

✓ Raw sockets are available  [caps.raw_socket]
    euid/elevated=true CAP_NET_RAW=yes CAP_NET_ADMIN=yes bpf_writable=n/a
✓ At least one usable network interface is up  [iface.any_up]
    eth0
✓ Kernel supports AF_XDP (5.10 or newer)  [kernel.af_xdp]
    kernel 6.18.44-fc-v49 (need >= 5.10.0)
✓ Kernel supports TPACKET_V3  [kernel.tpacket_v3]
    kernel 6.18.44-fc-v49 (need >= 3.2.0)
! No hugepages configured (only needed for the DPDK backend)  [hugepages]
    0 x 2048 kB (0 free), 0 x 1048576 kB (0 free)
    → Reserve hugepages and bind the NIC to DPDK: echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages ; sudo dpdk-devbind.py --status (then: --bind=vfio-pci <PCI address>)
- Not applicable on this operating system  [npcap]
✓ libpcap is available  [pcap_lib]
    /usr/lib/x86_64-linux-gnu/libpcap.so

5 ok, 1 warning(s), 0 failure(s)
```

(要約行は Fluent メッセージで多言語化されています。ここでは英語の実出力を載せています。) afpacket で重要なのは `caps.raw_socket` (なければ fail)、`iface.any_up`、`kernel.tpacket_v3` です。`hugepages` の警告は DPDK を使わない限り想定内です。`wpg doctor --json` でスクリプト向けに同じ一覧を得られます。続いて、ホストに影響しない順で動作確認します。

```sh
wpg send tpl.wpg.yaml --backend pcapfile -i /tmp/out.pcapng   # ドライラン: pcapng に書くだけで送出しない
wpg bench --backend afpacket -i eth0 --duration 5             # 64 バイトを無制限で送出します
wpg send tpl.wpg.yaml -i eth0 --backend afpacket --rate 10kpps --count 1000
```

最初は低い `--rate` から始めてください。`wpg bench` は CML2 のリンクへ全速力で送ります。

## プロミスキャスモードと MAC 制限

wpg は送出専用で、afpacket ソケットはプロトコル 0 で作られるため何も受信しません。したがって **送信ノードでプロミスキャスモードは不要** です。制約は経路側にあります。

- **CML2 ノード間の内部リンク** は CML2 ホストが実装する仮想リンクです。ノードのインタフェースに割り当てられた MAC 以外を送信元にしたフレームは、フィルタされたり、学習が期待どおりに行われなかったりする可能性があります。送信元 MAC を偽装するフレーム (ARP スイープ、多数のクライアント MAC を使う DHCP 負荷) が相手に届かない場合は、まずインタフェース本来の MAC を送信元にして試し、それでも届くなら CML2 リンクが残りを落としている可能性を疑います。これは CML2 のバージョンごとに確認が必要です。
- **External connector** (CML2 ホストのネットワークや物理 NIC へのブリッジ): ホストのブリッジや上流スイッチが port security や MAC 学習数制限を掛けている場合があります。多数 MAC の高レート送出は物理スイッチの CAM テーブルを溢れさせるため、隔離したセグメントでのみ使ってください。
- 受信側 (CML2 のルータや `tcpdump` を動かす別の Ubuntu ノード) は、宛先 MAC が自分でない場合にだけプロミスキャスが必要です: `sudo ip link set ens3 promisc on`。
- wpg は、テスト権限のあるラボ環境でのみ使用してください。[法的注意](../legal.md)を参照。

## MTU とジャンボフレーム

afpacket のリングはスロットの大きさをインタフェースの MTU (上限 9216) から決め、スロットより大きいフレームは別の通常ソケットから送出されます。送るフレーム長は FCS を除く Ethernet ヘッダ + ペイロードなので、1514 バイトのフレームには MTU 1500 が必要です。上の例では virtio インタフェースの MTU が 1400 で、1514 バイトのフレームはカーネルに拒否され (`EMSGSIZE`)、`dropped` に数えられます。`wpg ifaces` で MTU を確認し、テンプレートのフレームを MTU + 14 以下にするか、両端の MTU を上げます (`sudo ip link set ens3 mtu 9000`)。ジャンボフレームは経路上のすべての CML2 リンクと相手ノードが対応している必要があり、CML2 のバージョンやノード種別に依存します。

## virtio 上で期待される afpacket 性能

CML2 上では未計測です。手元にあるのは `docs/perf.md` の `lo` のベースラインで、64 バイトフレーム・1 スレッドで約 1.4–1.9 Mpps、律速は loopback の受信経路です。virtio-net ではゲストからホストへの経路 (virtio TX キュー、CML2 ホストの vhost スレッド、仮想リンク) が律速になるため、`lo` の値を下回り、ホストに強く依存すると見込まれます。試す価値のある調整は順に `--qdisc-bypass`、`--batch` を大きくする、`--pin-core`、`--no-tx-ring` との比較です。結果は `docs/perf.md` のチェックリストに沿って記録してください。

## `--qdisc-bypass`

`--qdisc-bypass` は `PACKET_QDISC_BYPASS` を設定し、qdisc 層を経由しません。`lo` では 64 バイトのスループットが約 30% 向上しました。代償として、そのインタフェースの tc シェーピング、優先制御、`netem` がこのトラフィックに効かなくなり、qdisc の統計 (`tc -s qdisc`) に載らず、デバイスキューが満杯のときは qdisc で待たずにカーネルが `ENOBUFS` を返して破棄します。wpg は短時間リトライしたうえで `dropped` に計上します。`netem` で障害を模擬する場合は使わないでください。

## 受信側の GRO / TSO / チェックサムオフロード

オフロードが変えるのは、受信側に **見える** ものであって、wpg が送ったものではありません。GRO / LRO が有効だと、Linux の受信側の `tcpdump` は TCP ストリームで MTU を超える結合フレームを表示することがあります。正確に観測するには `sudo ethtool -K ens3 gro off lro off` で無効にします。wpg はチェックサム込みでフレームを自分で組み立てるので、送信ノードのチェックサムオフロードは影響しません (意図的に壊したチェックサムはそのまま送られます)。virtio ゲストの送信側キャプチャで "incorrect checksum" と出るのはカーネル自身のスタックが作ったパケットだけで、wpg のフレームではありません。現在の設定は `ethtool -k` で確認します。

## CML2 でのバックエンドの選び方

| バックエンド | CML2 (virtio-net) での使いどころ | 準備 |
|---|---|---|
| `afpacket` | **既定。まずこれを使います。** 追加の準備が要らず、管理用インタフェースと共存できます | `CAP_NET_RAW` (上記) |
| `afxdp` | `afpacket` で頭打ちのとき比べてみる価値があります。virtio-net の zero-copy は新しいカーネルが必要で、多くの場合 copy モードになり、`afpacket` との差は小さくなります | `--features afxdp` でビルド、カーネル 5.10 以上、キュー数 (`ethtool -L`) ([AF_XDP](dpdk-afxdp.md#af_xdp)) |
| `dpdk` | virtio PMD でインタフェースをカーネルから外して使います。律速はホストの vhost 経路なので、効果は環境次第です | `--features dpdk` でビルド、hugepages、vfio の no-IOMMU モードか `uio_pci_generic` ([CML2 と virtio](dpdk-afxdp.md#cml2-と-virtio)) |
| `pcap` | 他の OS と同じ手順で試したいときだけ | `--features pcap` でビルド、`libpcap0.8` |

どのバックエンドでも、送る前に `--backend pcapfile` でフレームを確かめ、低い `--rate` から始めます。性能を比べるときは同じリンクで `wpg bench` を順に実行し ([性能チューニング](tuning.md))、結果を `docs/perf.md` のチェックリストに沿って記録してください。

## トラブルシューティング

| 症状 | 想定される原因 | 対処 |
|---|---|---|
| `AF_PACKET needs CAP_NET_RAW ...` / EPERM / `caps.raw_socket` が fail | 権限がない | root で実行するか `sudo setcap cap_net_raw,cap_net_admin+eip $(command -v wpg)`。バイナリ入れ替え後は再実行。`nosuid` マウント上では無効 |
| ENOBUFS、または `dropped` が 0 でない | TX キューが満杯: 仮想リンクか vhost が送出レートより遅い。約 100 ms リトライして破棄 | `--rate` を下げる、`--batch` を小さくする、`--qdisc-bypass` を外す、相手とリンク速度を確認、`--no-tx-ring` と比較 |
| 特定サイズのフレームだけ `dropped` になる | MTU より大きいフレーム (`EMSGSIZE`) | `wpg ifaces` の MTU を確認し、フレームを小さくするか両端の MTU を上げる |
| "interface not found" / `ENODEV` | 名前の誤り、または名前が変わった | `wpg ifaces`。`eth0` 形式と `ens3` 形式の違いに注意 |
| `iface.any_up` が "none up with carrier" と警告 | インタフェースが down、または CML2 のリンクが未接続/未起動 | `sudo ip link set ens3 up`。ノードを起動し、CML2 トポロジでリンクを確認 |
| 送信側は packets を数えるが相手に届かない | リンクの取り違え、CML2 リンクの MAC フィルタ、VLAN 不一致、受信側が promisc でない | まず送信ノードのインタフェースで、次に相手でキャプチャ。本来の送信元 MAC で試す。`ip link set ... promisc on` |
| `lo` のベースラインよりはるかに遅い | virtio/vhost の限界、vCPU が他の処理と共有 | ノードに 2 vCPU 以上、`--pin-core`、[性能チューニング](tuning.md)参照 |
| 指定したレートに届かない | 経路の持続可能な上限を超えている。`wpg send` の出力の精度も参照 | レートを下げる。[性能チューニング](tuning.md)参照 |
