# DPDK / AF_XDP のセットアップ

Status: draft

> **`afxdp` バックエンドは実装済み (M8) ですが、任意機能です。** `--features afxdp` を付けてビルドしたときだけ有効になります (`cargo build --release -p wpg-cli --features afxdp`)。**copy モードは `lo` と `veth` で動作を確認済み、zero-copy は実機 (物理 NIC) では未確認です。** `dpdk` バックエンドはまだ実装されていません ("not implemented yet" で失敗します)。DPDK の節は事前準備用の想定手順で、wpg では検証していません。

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

`scripts/deploy.sh <host> --dpdk [--hugepages N]` が以下のホスト側の準備 (hugepages の確保と起動時の設定、`hugetlbfs`、`vfio-pci`、IOMMU の確認) を行います。NIC のバインドは行いません。[scripts/deploy.sh によるデプロイ](deploy.md) を参照してください。

要件 (ADR 0004 の決定に基づく):

- DPDK **24.11 LTS** (23.11 LTS も CI で確認)。ビルド時に DPDK の開発パッケージと libclang が必要です ([バインディングのビルド](#バインディングのビルド-dpdk-sys)を参照)。
- Hugepages (`wpg doctor` のチェック `hugepages`。未設定なら警告):

```sh
echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
grep Huge /proc/meminfo
```

- NIC を DPDK 対応ドライバにバインドします。NIC はカーネルから外れるため、管理用に別のインタフェースを残してください。

```sh
dpdk-devbind.py --status
sudo modprobe vfio-pci        # または uio_pci_generic
sudo dpdk-devbind.py --bind=vfio-pci 0000:00:04.0
```

- EAL 引数 (コア、メモリチャネル、allow-list) は設定ファイルから渡します。[設定リファレンス](../reference/config.md)を参照。
- **CML2**: Ubuntu ノードには virtio-net デバイスが見え、DPDK は virtio PMD で扱います。ゲストに IOMMU がない場合は `uio_pci_generic`、または vfio の no-IOMMU モード (`enable_unsafe_noiommu_mode`) を使います。性能は DPDK ではなくホスト側の vhost 経路で頭打ちになります。

期待性能 (TODO §2.2、未計測): 物理 NIC で AF_XDP は 1 コアあたり数 Mpps、DPDK は 1 コアあたり 10 Mpps 以上。AF_XDP の copy モードを `lo` / `veth` で測った値は `docs/perf.md` にあります。

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

DPDK 24.11 は、親ディレクトリが world-writable (`/tmp` など) な場所から PMD プラグインを読み込むことを拒否します。`/opt` や `/usr/local` にインストールしてください。スモークテストは EAL を `--no-huge --no-pci` で起動し、`net_ring` の仮想ポートを使うため、hugepages・NIC・root のいずれも不要です。CI (ジョブ `dpdk`) が両方の LTS で実行します。DPDK は共有ライブラリ版のみ対応です (PMD はプラグインとして読み込まれます)。`dpdk` 送出バックエンド自体はまだで、`wpg` には `dpdk` のビルド feature がありません。
