# DPDK / AF_XDP のセットアップ

Status: draft

AF_XDP には Linux カーネル 5.10 以降が必要で、NIC ドライバが対応していない場合は zero-copy から copy モードへ自動的にフォールバックします。DPDK は DPDK 24.11 LTS に対して `--features dpdk` を付けてビルドすることで有効になります (23.11 LTS も CI で確認)。hugepages の確保、dpdk-devbind による NIC のバインド、設定ファイルからの EAL 引数が必要です。CML2 では virtio-net PMD を使用します。
