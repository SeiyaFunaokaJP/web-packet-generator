# Docker

Status: draft

リポジトリには multi-stage の `Dockerfile` と `docker-compose.yml` があります。ビルドステージ (`rust:1-bookworm`) は Trunk をダウンロードし、Web UI とリリースバイナリをビルドします。実行用イメージ (`debian:bookworm-slim`) に入るのは `wpg`、`curl` (ヘルスチェック用)、`/usr/share/wpg/templates` のサンプルテンプレートだけです。

## ビルド

```sh
docker build -t wpg .
docker build -t wpg --build-arg FEATURES=pcap .     # libpcap バックエンド付き
```

ビルドにはインターネット接続が必要です (crates.io、GitHub 上の Trunk のリリース、Trunk がダウンロードする Tailwind CSS と wasm-bindgen)。TLS を中継するプロキシの内側では、先にプロキシの CA 証明書をビルドステージに追加してください。

## 実行

ホストのインタフェースで送信するには、ホストのネットワーク名前空間と 2 つのケーパビリティが必要です。それ以外はすべて外します。

```sh
docker run -d --name wpg --network host \
  --cap-drop ALL --cap-add NET_RAW --cap-add NET_ADMIN \
  -e WPG_PASSWORD='change me' \
  -e WPG_TX__ALLOWED_IFACES='["eth1"]' \
  -v wpg-data:/var/lib/wpg \
  wpg
```

Compose の場合 (`WPG_PASSWORD`、`WPG_BIND`、`WPG_ALLOWED_IFACES`、`WPG_FEATURES` を環境変数か `.env` ファイルから読みます):

```sh
WPG_PASSWORD='change me' WPG_ALLOWED_IFACES='["eth1"]' docker compose up -d --build
docker compose ps          # /api/health が応答すると "healthy" になる
docker compose logs -f wpg
```

| 設定 | イメージの既定値 | 補足 |
|---|---|---|
| `WPG_SERVER__BIND` | `0.0.0.0:8080` | `--network host` ではホストのポートです。ローカルに限るなら `127.0.0.1:8080`。 |
| `WPG_PASSWORD` | なし | ループバック以外で待ち受けるときは必須で、無いとサーバは起動しません。平文または argon2id ハッシュ。Docker secrets では `WPG_AUTH__PASSWORD_FILE=/run/secrets/...` が使えます。 |
| `WPG_DATA_DIR` | `/var/lib/wpg` (ボリューム) | データベース、ユーザテンプレート、TLS 証明書。 |
| `WPG_TEMPLATES__LIBRARY_PATHS` | `["/usr/share/wpg/templates"]` | 独自のライブラリをマウントしてパスを追加します。 |
| `WPG_TX__ALLOWED_IFACES` | すべて | ジョブが送信してよいインタフェース。 |

その他の設定は `WPG_<SECTION>__<KEY>` 変数で指定するか、`wpg.toml` をマウントして `WPG_CONFIG` を設定します。[設定リファレンス](../reference/config.md) を参照してください。

プロセスはコンテナ内では root で動きますが、持つのは `CAP_NET_RAW` と `CAP_NET_ADMIN` だけです。root 以外のユーザで動かすにはバイナリにファイルケーパビリティが必要で、コンテナのケーパビリティにその一部が無いと実行に失敗するため、イメージでは単純な構成にしています。

## 制限

- Linux ホストを使ってください。macOS / Windows の Docker Desktop はコンテナを VM の中で動かすので、`--network host` は VM のネットワークになり、フレームは物理 NIC に届きません。
- コンテナ内の `wpg doctor` はコンテナから見た状態 (ケーパビリティ、インタフェース) を報告します。将来の DPDK バックエンド用の hugepages と `vfio-pci` はホスト側で準備する必要があります。
- `--network host` を使わない場合 (既定のブリッジネットワークと `-p 8080:8080`)、GUI は使えますが、ジョブはコンテナ自身の `eth0` でしか送信できません。

## 確認状況

compose ファイルは `docker compose config` で確認しました。開発用サンドボックスでは、TLS を中継するプロキシをビルドコンテナが信頼しないため (Trunk のダウンロードが失敗する)、`docker build` を最後まで完了できませんでした。代わりにビルドのコマンドはホスト上で確認し (`scripts/release.sh` は同じ `trunk build --release` と `cargo build --release --locked -p wpg-cli` を実行します)、実行ステージはローカルでビルドしたバイナリで確認しました。**使う前に通常の環境で `docker build` を実行してください。**
