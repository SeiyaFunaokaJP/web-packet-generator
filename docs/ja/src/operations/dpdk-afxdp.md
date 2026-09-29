# DPDK / AF_XDP のセットアップ

Status: draft

> **`afxdp` と `dpdk` バックエンドはまだ実装されていません (マイルストーン M8)。** `--backend afxdp` / `--backend dpdk` は CLI のヘルプには載っていますが、現状は "not implemented yet (planned for M8)" で失敗します。このページは、事前に環境を用意できるよう、想定するセットアップ手順を記録したものです。それまでは `afpacket` バックエンドを使ってください。このページの内容は wpg では検証していません。

## AF_XDP

要件:

- Linux カーネル 5.10 以降 (`wpg doctor` のチェック `kernel.af_xdp`)。zero-copy にはそれに対応したドライバが必要で、非対応なら wpg は自動で copy モードにフォールバックする予定です。
- AF_XDP の zero-copy に対応するドライバの例: `i40e`、`ice`、`ixgbe`、`mlx5`、`igc`。`virtio_net` は XDP に対応しますが、AF_XDP は copy モードで動きます。お使いのカーネルのドキュメントで確認してください。
- root、または `CAP_NET_RAW`、`CAP_NET_ADMIN`、`CAP_BPF` (正確な組はバックエンド実装時に確定)。memlock の上限を上げます (`ulimit -l unlimited`、systemd なら `LimitMEMLOCK=infinity`)。
- AF_XDP ソケットは 1 つの RX/TX キューペアに結び付きます。使うキュー数を設定し、スレッドもそれに合わせて pin します。

```sh
ethtool -l ens3               # 現在値と最大チャネル数を表示
sudo ethtool -L ens3 combined 4
```

## DPDK

要件 (ADR 0004 の決定に基づく):

- DPDK **24.11 LTS** (23.11 LTS も CI で確認)。`--features dpdk` でビルドし、DPDK の開発パッケージが必要です。
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

期待性能 (TODO §2.2、未計測): 物理 NIC で AF_XDP は 1 コアあたり数 Mpps、DPDK は 1 コアあたり 10 Mpps 以上。
