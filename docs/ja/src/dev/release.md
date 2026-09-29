# リリース手順

Status: draft

リリースは cargo-dist を使い、GitHub Actions の matrix で Windows / macOS / Linux のバイナリとチェックサムを生成します。ローカルの入口は `scripts/release.sh` です。v1.0 リリースの前には、golden pcap と tshark の突合、`cargo audit`、`cargo deny`、i18n lint がすべて緑である必要があります。CHANGELOG と日英のドキュメントも完備してください。
