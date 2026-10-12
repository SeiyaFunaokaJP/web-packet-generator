# DPDK / AF_XDP のセットアップ

Status: draft

> **`afxdp` バックエンドは実装済み (M8) ですが、任意機能です。** `--features afxdp` を付けてビルドしたときだけ有効になります (`cargo build --release -p wpg-cli --features afxdp`)。**copy モードは `lo` と `veth` で動作を確認済み、zero-copy は実機 (物理 NIC) では未確認です。** `dpdk` バックエンド (M8) も任意機能 (`--features dpdk`) で、確認済みなのは DPDK の仮想デバイス (`net_null`、`net_ring`、`net_pcap`) のみです。物理 NIC と CML2 の virtio PMD は未確認です。

## AF_XDP

### 仕組み

`afxdp` バックエンドは libbpf / libxdp を使わず、カーネルの ABI (`<linux/if_xdp.h>`、`bpf(2)`) を直接呼ぶ最小実装です。追加のビルド依存はありません。ワーカー (`--workers`) ごとに次を持ちます。

- AF_XDP ソケット 1 つ。NIC のキュー 1 つに bind します。ワーカー `i` はキュー `--queue` + `i` を使います (既定は 0 から)。
- UMEM (送出用のフレーム領域、最大 16 MiB)。NIC の NUMA ノードにページを割り当てます (`mbind`)。
- TX リングと completion リング。フレームを UMEM のチャンクにコピーし、ディスクリプタを TX リングに積み、カーネルが求めたとき (`XDP_USE_NEED_WAKEUP`) だけ `sendto` で起こします。

### copy / zero-copy の自動選択 (`--xdp-mode`)

| モード | 動作 |
|---|---|
| `auto` (既定) | まず zero-copy を試し、だめなら copy モードに切り替えます |
| `copy` | 常に copy モード (`XDP_COPY`)。`lo`、`veth` を含むどのインタフェースでも動きます |
| `zerocopy` | zero-copy (`XDP_ZEROCOPY`) のみ。使えなければエラー |

多くのドライバ (i40e、ice、ixgbe、igc など) は、XDP プログラムが付いている間しか zero-copy 送出用の XDP リングを作りません。そこで `auto` / `zerocopy` は、何もしない 2 命令の `XDP_PASS` プログラムをドライバモードで付けてから zero-copy で bind します (`bpf_link` なので、バックエンドを閉じると外れます。すでに別の XDP プログラムが付いていればそのまま使います)。プログラムを読み込めない、ドライバが zero-copy の bind を拒否する、最初の wake-up が失敗する、のいずれかなら copy モードに切り替えます。どちらになったかは `wpg bench` / `wpg send` の backend 行に表示されます (`AF_XDP zero-copy` / `AF_XDP copy mode`、切り替えた場合は理由も)。

> **注意**: ドライバによっては XDP プログラムの付け外しで NIC のキューを作り直し、リンクが一瞬落ちます。運用中のインタフェースでは `--xdp-mode copy` を使うか、事前に影響を確認してください。

### 要件

- Linux カーネル 5.10 以降 (`wpg doctor` のチェック `kernel.af_xdp`)。`--busy-poll` は 5.11 以降。
- 権限: copy モードは `CAP_NET_RAW`。zero-copy は加えて `CAP_NET_ADMIN` と `CAP_BPF` (5.8 より前は `CAP_SYS_ADMIN`)。

```sh
sudo setcap cap_net_raw,cap_net_admin,cap_bpf,cap_ipc_lock+ep "$(command -v wpg)"
```

- 5.11 より前のカーネルでは UMEM が `RLIMIT_MEMLOCK` に計上されます。wpg はソフト上限をハード上限まで上げますが、足りなければ `ulimit -l unlimited` (systemd なら `LimitMEMLOCK=infinity`) を設定してください。
- 1 つのキューに bind できる AF_XDP ソケットは 1 つです。使うキュー数ぶんのチャネルを用意します。キューが足りなければ wpg は `ethtool -L` の案内付きで失敗します。

```sh
ethtool -l ens3               # 現在値と最大チャネル数
sudo ethtool -L ens3 combined 4
wpg bench --backend afxdp -i ens3 --workers 4 --auto-pin
```

- フレーム長の上限は UMEM のチャンク長です (MTU が 2030 以下なら 2048 B、それより大きければ 4096 B)。超えるフレームは `dropped` に数えます (ジャンボフレームに必要なマルチバッファ AF_XDP は使っていません)。

### 主なオプション

| オプション | 意味 |
|---|---|
| `--backend afxdp` | AF_XDP で送出 |
| `--xdp-mode auto\|copy\|zerocopy` | bind モード (上記) |
| `--queue N` | 先頭のキュー番号 (ワーカー i はキュー N+i) |
| `--workers N` | ワーカー数 = 使うキュー数 |
| `--auto-pin` / `--pin-cores 2,3` | ワーカーのコア固定 ([性能チューニング](tuning.md)) |
| `--ring-depth N` | TX リングのスロット数 (2 のべき乗に丸め、上限は UMEM 16 MiB から決まる) |
| `--busy-poll` | `SO_PREFER_BUSY_POLL` / `SO_BUSY_POLL` を設定し、送出スレッド自身がドライバの NAPI を回す (5.11+)。`wpg send` では待ち方も `busy-poll` になる |

### 対応 NIC (zero-copy)

AF_XDP の copy モードは、どのネットワークデバイスでも動きます。zero-copy はドライバの対応が必要です。以下はメインラインの Linux で AF_XDP zero-copy に対応している主なドライバで、カーネル版は対応が入ったおおよその版です。**wpg ではどれも実機で確認していません**。ディストリビューションのカーネルではバックポートの有無で変わるため、必ず `wpg bench` の表示で zero-copy になったかを確認してください。

| ドライバ | NIC | zero-copy 対応 (目安) | wpg での確認 |
|---|---|---|---|
| `i40e` | Intel X710 / XL710 / XXV710 | 4.20 | 未確認 |
| `ixgbe` | Intel 82599 / X520 / X540 / X550 | 4.20 | 未確認 |
| `mlx5_core` | NVIDIA (Mellanox) ConnectX-4 Lx 以降 | 5.3 | 未確認 |
| `ice` | Intel E810 | 5.5 | 未確認 |
| `stmmac` | Synopsys DWMAC (組み込み SoC) | 5.13 | 未確認 |
| `igc` | Intel I225 / I226 | 5.14 | 未確認 |
| `virtio_net` | KVM / CML2 の virtio-net | 新しいカーネル (6.x) で対応。それ以前は copy モード | 未確認 |
| `veth` | コンテナ / netns | なし (native XDP は可、送出は copy モード) | **copy モードで確認済み** (4 キュー、2 ワーカー) |
| `lo` | ループバック | なし (copy モード) | **copy モードで確認済み** |

確認済みの範囲 (2026-10、Linux 6.18 のコンテナ、root): `lo` と `veth` での copy モード送出と受信側での到着、`auto` での zero-copy からの切り替え (veth には `XDP_PASS` プログラムがドライバモードで付き、zero-copy の bind が `EOPNOTSUPP` で拒否されて copy モードになる)、veth の 2 キューに 2 ワーカー。単体テストは `cargo test -p wpg-tx --features afxdp` (veth は `WPG_AFXDP_VETH=vxa,vxb` を付けたときだけ)。性能は `docs/perf.md` を参照。

## DPDK

`dpdk` バックエンドは DPDK のポーリングモードドライバ (PMD) で送出します。NIC はカーネルから外れ、wpg 自身のスレッドがバッチごとのシステムコールなしで直接扱います。任意機能なので `--features dpdk` を付けてビルドします (DPDK の開発パッケージと libclang が必要。[ビルド](#バインディングのビルド-dpdk-sys)を参照)。

```sh
cargo build --release -p wpg-cli --features dpdk
```

**確認済みの範囲** (2026-10、Ubuntu 24.04 + DPDK 23.11.4、hugepages と NIC のないコンテナ): `--no-huge` での EAL 起動、仮想デバイス `net_null` (`wpg bench`、2 ワーカー)・`net_ring`・`net_pcap`。テンプレートを `net_pcap` 経由で `wpg send` すると、`pcapfile` バックエンドと同じフレームが書き出されました。単体テストは CI で DPDK 24.11 と 23.11 に対して実行します。**物理 NIC (`vfio-pci`)、hugepages、CML2 上の virtio PMD は未確認です。**

### 仕組み

- **EAL** (DPDK の環境層) は wpg のプロセスごとに 1 度だけ、最初の `dpdk` ジョブで起動します。引数は指定した EAL 引数に、上書きできる既定値 2 つを足したものです: `-l <wpg が使える先頭の CPU>` (メイン lcore のみ。送出ワーカーは wpg 自身のスレッドで、非 EAL lcore として DPDK に登録します) と `--in-memory` (`/var/run/dpdk` にファイルを作らない。`--no-huge` を指定したときは代わりに `--no-shconf`)。EAL はプロセス内で再起動できません。失敗したときや引数を変えたいときは `wpg serve` を再起動してください (`wpg send` / `wpg bench` は毎回新しいプロセスです)。
- **インタフェースには DPDK のポートを指定します**: ポート番号 (`0`)、デバイス名 (バインドした NIC の PCI アドレス `0000:00:04.0`)、または仮想デバイス。仮想デバイスは devargs 付きで書けて、初回に追加されます: `net_tap0,iface=dtap0` (カーネルの TAP インタフェース。試験に便利)、`net_pcap0,tx_pcap=/tmp/out.pcap` (pcap ファイルに書き出す)、`net_null0` (捨てる。ベンチマーク用)。見つからない名前は、EAL が見つけたポートの一覧付きでエラーになります。
- **ワーカー** (`--workers N`) は 1 つのポートを共有します。ポートを `--queue` + N 本の TX キューで設定し、ワーカー `i` は TX キュー `--queue` + `i` で送ります。1 つのポートを同時に使えるジョブは 1 つです。ジョブが終わるとポートを停止します。
- **バッファ**: ポートごとに NIC の NUMA ソケット上の mbuf プールを 1 つ作り、大きさは `--ring-depth` (キューあたりの TX ディスクリプタ数。ドライバの上下限に丸めます) と `--batch` から決めます。フレームは mbuf にコピーします (データ領域 2048 オクテット。それより長いフレームは `dropped`。ジャンボフレームは非対応)。TX リングが一杯なら最大 10 ms 再試行し、残りは `dropped` に数えます。ドライバが対応していれば `MBUF_FAST_FREE` を有効にします。
- `wpg send` / `wpg bench` の `backend` 行に、DPDK のバージョン、ポート、ドライバ、キュー、ディスクリプタ数、プール、NUMA ソケット、リンク状態、EAL 引数の全体を表示します。

### ホストの準備

`scripts/deploy.sh <host> --dpdk [--hugepages N]` が最初の 2 手順 (hugepages の確保と起動時の設定、`hugetlbfs`、`vfio-pci`、IOMMU の確認) を行います。NIC のバインドは行いません。[scripts/deploy.sh によるデプロイ](deploy.md) を参照してください。

1. **Hugepages** (`wpg doctor` のチェック `hugepages`。未設定なら警告)。4 ワーカー・既定の `--ring-depth 4096` でポート 1 つのプールは約 70 MiB、これにドライバの確保分が加わります。

   ```sh
   echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
   grep Huge /proc/meminfo
   ```

2. **IOMMU と vfio**: 物理マシンでは IOMMU を有効にし (カーネル引数 `intel_iommu=on iommu=pt` または `amd_iommu=on`)、`sudo modprobe vfio-pci` します。
3. **NIC のバインド**。インタフェースはカーネルから (そして `wpg ifaces` から) 消えるので、ssh セッションが通っているインタフェースは絶対にバインドしないでください。

   ```sh
   dpdk-devbind.py --status                      # PCI アドレスと現在のドライバ
   sudo ip link set ens4 down
   sudo dpdk-devbind.py --bind=vfio-pci 0000:00:04.0
   # カーネルに戻す: sudo dpdk-devbind.py --bind=virtio-pci 0000:00:04.0 (または ixgbe、i40e など)
   ```

   NVIDIA / Mellanox ConnectX (`mlx5`) は例外で、カーネルのドライバのまま `rdma-core` 経由で DPDK から使います。バインドし直さないでください。
4. **権限**: root で実行します。一般ユーザーでの vfio には `/dev/vfio/*` の権限、`CAP_IPC_LOCK` / 大きな `RLIMIT_MEMLOCK`、hugepage のマウントへのアクセスが必要で、ここでは扱いません。

### EAL 引数

`wpg send` / `wpg bench` では `--eal-args` (1 つの文字列を空白で分割)、`wpg serve` では `wpg.toml` の `tx.dpdk_eal_args` ([設定リファレンス](../reference/config.md)) または `WPG_TX__DPDK_EAL_ARGS` で渡します。よく使うもの:

| 引数 | 意味 |
|---|---|
| `-a 0000:00:04.0` | この PCI デバイスだけを probe する (複数指定可)。指定しなければ DPDK ドライバにバインドされた NIC をすべて使う |
| `-l 2` | メイン lcore (既定は wpg が使える先頭の CPU)。送出ワーカーはこれまでどおり `--pin-cores` / `--auto-pin` で固定 |
| `--socket-mem 512` / `-m 512` | 最初に確保する hugepage メモリ (MiB) |
| `--vdev net_tap0,iface=dtap0` | 起動時に仮想デバイスを作る (インタフェースに名前を書いても可。上記) |
| `--file-prefix wpg` | `--in-memory` の代わりに共有のランタイムファイルを使う (`dpdk-proc-info` やテレメトリソケット用) |
| `--log-level lib.eal:debug` | 失敗したときに EAL のログを増やす |
| `--no-huge -m 256` | hugepages なしで動かす (仮想デバイスのみ。物理 NIC には hugepages が必要) |

```sh
sudo wpg bench --backend dpdk -i 0000:00:04.0 --eal-args "-a 0000:00:04.0" --workers 2 --auto-pin
sudo wpg send t.wpg.yaml --backend dpdk -i 0000:00:04.0 --eal-args "-a 0000:00:04.0" --rate 1Mpps

# NIC なし: tcpdump で見られる TAP インタフェース (root が必要)
sudo wpg send t.wpg.yaml --backend dpdk -i net_tap0,iface=dtap0 --count 10 &
sudo tcpdump -ni dtap0
```

```toml
# wpg.toml
[tx]
default_backend = "dpdk"
allowed_ifaces = ["0000:00:04.0"]
dpdk_eal_args = ["-a", "0000:00:04.0", "-l", "1"]
```

### CML2 と virtio

CML2 の Ubuntu ノードのデータ用インタフェースは virtio-net デバイスで、DPDK は virtio PMD (`net_virtio`、標準の DPDK パッケージに含まれる) で扱います。

- ゲストには通常 IOMMU がないため、`vfio-pci` はバインドを拒否します。vfio の no-IOMMU モード (バインド前に `echo 1 | sudo tee /sys/module/vfio/parameters/enable_unsafe_noiommu_mode`。DMA が隔離されないため "unsafe" ですが、検証用 VM なら許容範囲) か `uio_pci_generic` を使います。
- 管理用インタフェース (通常 `ens2`) はカーネルに残し、データ用インタフェースだけをバインドします。
- virtio PMD の TX キュー数は、ふつうノードの vCPU 数 / virtio のキューペア数までです。それを超える `--workers` はキュー数を示すエラーになります。
- 性能は DPDK ではなく、ホストの vhost 経路と仮想リンクで頭打ちになります。DPDK に頼る前に同じリンクで `afpacket` と比べてください。CML2 では差が小さいことがよくあります。
- 送出に NIC の MAC アドレスは関係ありません (送信元 MAC はテンプレートのもの)。送出だけならプロミスキャスモードも不要です。

### 制約

- 送出のみ (`wpg capture` の DPDK 受信はなし)。
- フレームは 2048 オクテットまで。マルチセグメント mbuf、TX のチェックサム / セグメンテーションオフロードは使いません (wpg が完成したフレームを作るため)。
- 1 つのポートを同時に使えるジョブは 1 つ。EAL 引数はプロセスの間固定です。
- DPDK は共有ライブラリ版のみ (PMD はプラグインとして読み込まれます)。

期待性能 (TODO §2.2、実機では未計測): 物理 NIC で AF_XDP は 1 コアあたり数 Mpps、DPDK は 1 コアあたり 10 Mpps 以上。計測したら `docs/perf.md` のチェックリストで記録してください。

### バインディングのビルド (`dpdk-sys`)

バインディングは `dpdk-sys` crate にあります (ADR-0004)。`link` feature (`wpg-tx` の `dpdk` feature が有効にします) を付けると、ビルドスクリプトが pkg-config で DPDK (`libdpdk`、23.11 以降。24.11 / 23.11 以外は警告付きでビルド) を探し、bindgen でバインディング (EAL、ethdev、mempool / mbuf、lcore、`RTE_*` 定数) を生成します。ヘッダ内で `static inline` になっている高速パスは、DPDK 自身の cflags でコンパイルした小さな C の shim で包みます (`wpg_rte_eth_tx_burst`、`wpg_rte_pktmbuf_alloc`、`wpg_rte_errno` など)。feature を付けなければ crate は空なので、通常のビルドに DPDK は要りません。

```sh
# Debian 13 (DPDK 24.11) / Ubuntu 24.04 (DPDK 23.11)
sudo apt install dpdk-dev libclang-dev pkg-config
cargo test -p dpdk-sys --features link     # バインディング + net_ring ポートでの EAL スモークテスト
cargo build -p wpg-tx --features dpdk      # バインディングをリンクした wpg-tx
```

ソースからビルドした DPDK (meson / ninja) を使う場合は、pkg-config と共有ライブラリの場所を指定します。

```sh
export PKG_CONFIG_PATH=/opt/dpdk-24.11/lib/x86_64-linux-gnu/pkgconfig
export LD_LIBRARY_PATH=/opt/dpdk-24.11/lib/x86_64-linux-gnu
```

DPDK 24.11 は、親ディレクトリが world-writable (`/tmp` など) な場所から PMD プラグインを読み込むことを拒否します。`/opt` や `/usr/local` にインストールしてください。スモークテストは EAL を `--no-huge --no-pci` で起動し、`net_ring` の仮想ポートを使うため、hugepages・NIC・root のいずれも不要です。CI (ジョブ `dpdk`) が両方の LTS で実行します。DPDK は共有ライブラリ版のみ対応です (PMD はプラグインとして読み込まれます)。バックエンド自体のテスト (`cargo test -p wpg-tx --features dpdk --lib dpdk`) も `net_ring` / `net_pcap` / `net_null` を使うため、hugepages・NIC・root なしで動きます。
