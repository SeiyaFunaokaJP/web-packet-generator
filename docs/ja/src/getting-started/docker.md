# Docker で動かす

イメージはコンテナの中で wpg と Web UI をビルドするので、ホストに必要なのは Docker だけです。Rust ツールチェーンも trunk も要りません。**Linux ホスト** を使ってください。macOS / Windows の Docker Desktop はコンテナを VM の中で動かすため、そこから送ったフレームは物理 NIC に届きません。

このページは最短の手順です。ビルドオプション、プロキシ、全設定、制限は [Docker](../operations/docker.md) にあります。

## 1. リポジトリを取得して起動する

```sh
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
WPG_PASSWORD='change me' WPG_ALLOWED_IFACES='["eth1"]' docker compose up -d --build
```

- `WPG_PASSWORD` は必須です。サーバはホストの全アドレスで待ち受けるため、パスワードがないと起動しません。自分で決めた値にしてください。Web UI の管理者パスワードになります。
- `WPG_ALLOWED_IFACES` はジョブが送信してよいインタフェースの一覧です。検証用インタフェースの名前を指定します (`ip -br link` で一覧できます)。省略するとすべて許可します。
- 初回のビルドはすべてをコンパイルするので時間がかかります。2 回目以降はイメージを再利用します。

コンテナはホストのネットワークを使い、`CAP_NET_RAW` と `CAP_NET_ADMIN` だけを持ちます。ホスト上で wpg を root で動かさずに、ホストのインタフェースから送信できます。

## 2. 確認する

```sh
docker compose ps                         # サーバが応答すると STATUS が "healthy" になる
docker compose exec wpg wpg doctor        # コンテナから見た raw ソケットとインタフェース
```

ホストで <http://127.0.0.1:8080> (別のマシンからは `http://<ホスト>:8080`) を開き、パスワードでログインします。画面の説明は [初回ログイン](first-login.md) にあります。

## 3. テンプレートを試す

同梱テンプレートはコンテナ内の `/usr/share/wpg/templates` にあり、**テンプレート** 画面に表示されます。コマンドラインでは、まずファイルに書き出し (何も送信しません)、それから送信します。

```sh
docker compose exec wpg wpg render /usr/share/wpg/templates/examples/arp-sweep.wpg.yaml -o /var/lib/wpg/arp.pcapng
docker compose exec wpg wpg send /usr/share/wpg/templates/examples/arp-sweep.wpg.yaml -i eth1 --rate 1000pps
```

`/var/lib/wpg` は `wpg-data` ボリュームです。データベース、自分のテンプレート、上の pcapng のようなファイルは、再起動やイメージの更新後も残ります。送信は、自分が管理しているか試験を許可されたネットワークでだけ行ってください。続きは [ARP スイープのチュートリアル](../tutorials/arp-sweep.md) です。チュートリアルの `wpg ...` は `docker compose exec wpg wpg ...` に読み替えてください。

## 停止・更新・削除

```sh
docker compose logs -f wpg                # ログを追う
docker compose down                       # 停止 (wpg-data ボリュームは残る)
git pull && WPG_PASSWORD='change me' docker compose up -d --build   # 更新
docker compose down -v                    # 停止して wpg-data ボリュームも削除
```

設定をシェルの履歴に残したくない場合は、`docker-compose.yml` と同じ場所の `.env` ファイルに書きます (`WPG_PASSWORD=...`、`WPG_ALLOWED_IFACES=...`、`WPG_BIND=...`)。リポジトリは `.env` を git と Docker のビルドコンテキストの両方で無視するので、パスワードがコミットされたりイメージに入ったりすることはありません。
