# Linux へのインストール

Status: draft

最低対応環境は Ubuntu 20.04 以降 (glibc 2.31) です。リリースバイナリを配置し、root なしで動かすために `setcap cap_net_raw,cap_net_admin+ep` で権限を付与します。libpcap の開発パッケージはソースからビルドする場合のみ必要です。導入後は `wpg doctor` で権限を確認してください。
