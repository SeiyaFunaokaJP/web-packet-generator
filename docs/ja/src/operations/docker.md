# Docker

Status: draft

リポジトリには multi-stage の `Dockerfile` と `docker-compose.yml` があります。ビルドステージ (`rust:1-bookworm`) は Trunk をダウンロードし、Web UI とリリースバイナリをビルドします。実行用イメージ (`debian:bookworm-slim`) に入るのは `wpg`、`curl` (ヘルスチェック用)、`/usr/share/wpg/templates` のサンプルテンプレートだけです。

## ビルド

```sh
docker build -t wpg .
docker build -t wpg --build-arg FEATURES=pcap .     # libpcap バックエンド付き
```

ビルドにはインターネット接続が必要です (crates.io、GitHub 上の Trunk のリリース、Trunk がダウンロードする Tailwind CSS と wasm-bindgen)。TLS を中継するプロキシの内側では、プロキシの CA 証明書 (PEM) をビルドシークレットで、プロキシのアドレスを標準のビルド引数で渡します。証明書を信頼するのはビルドステージだけで、実行用イメージには入りません。

```sh
docker build -t wpg --secret id=ca_certs,src=/path/to/proxy-ca.pem \
  --build-arg HTTPS_PROXY=http://proxy.example:3128 .
```

Docker Hub の匿名 pull のレート制限にかかる場合は、`--build-arg RUST_IMAGE=mirror.gcr.io/library/rust:1-bookworm --build-arg RUNTIME_IMAGE=mirror.gcr.io/library/debian:bookworm-slim` のように別のレジストリのベースイメージを使えます。

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
- コンテナ内の `wpg doctor` はコンテナから見た状態 (ケーパビリティ、インタフェース) を報告します。イメージは `dpdk` feature なしでビルドします (両方のステージに DPDK のライブラリが必要になるため)。DPDK 用の hugepages と `vfio-pci` は、いずれにしてもホスト側で準備します。
- `--network host` を使わない場合 (既定のブリッジネットワークと `-p 8080:8080`)、GUI は使えますが、ジョブはコンテナ自身の `eth0` でしか送信できません。

## 確認状況

2026 年 10 月に次を確認しました (Docker 29.8、Linux 6.18 のホスト、追加 feature なしのイメージ)。

- `docker build` が最後まで完了 (Trunk による Web UI と `cargo build --release --locked`)。実行用イメージはディスク上で約 170 MB (圧縮時 44 MB)。
- `docker run --network host --cap-drop ALL --cap-add NET_RAW --cap-add NET_ADMIN -e WPG_PASSWORD=...` でヘルスチェックが `healthy` になり、`/api/health` と Web UI が応答。コンテナ内の `wpg doctor` は `CAP_NET_RAW` / `CAP_NET_ADMIN` 付きで raw ソケットが使えると報告。
- コンテナ内で `afpacket` バックエンドの `wpg send` により `lo` へ 100 フレームを送出し、同じコンテナの `wpg capture` で受信。
- `--build-arg FEATURES=pcap` のイメージには `libpcap0.8` が入り、`wpg doctor` が検出し、`wpg send --backend pcap` で `lo` へ 50 フレームを送出。
- `docker compose up -d --no-build` でサービスが `healthy` になり、`WPG_PASSWORD` でのログインが成功 (204)、誤ったパスワードは拒否 (401)。`WPG_PASSWORD` が無いと compose ファイルのメッセージで起動を拒否。

開発用サンドボックスのプロキシは HTTPS しか通さないため、`apt-get` の取得先だけを使い捨ての Dockerfile のコピーで `https://deb.debian.org` に変えました。それ以外はリポジトリの `Dockerfile` をそのまま使っています。Docker Desktop (macOS / Windows) は確認していません。
