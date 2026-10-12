# scripts/deploy.sh によるデプロイ

Status: draft

`scripts/deploy.sh` は、wpg のビルドを SSH で Linux ホストにコピーしてインストールします。第一の対象は CML2 上の Ubuntu ノードですが、systemd を使うディストリビューションならどれでも動きます。1 回の実行で次を行います。

1. `target/release/wpg` (Web UI 込み) が無ければビルドします。`--binary` で既存のファイルも指定できます。
2. バイナリ、サンプルテンプレート、systemd unit を `scp` でアップロードします。
3. バイナリを `/usr/local/bin/wpg` にインストールし (直前のものは `wpg.prev` として残します)、`setcap cap_net_raw,cap_net_admin+eip` を付与します (`setcap` が無く `apt-get` がある場合は先に `libcap2-bin` を入れます)。
4. `--service` のときは、システムユーザ `wpg`、`/etc/wpg/wpg.toml` (無い場合のみ)、`/etc/wpg/wpg.env` を作り、`wpg.service` をインストールして (再) 起動します。
5. ホスト上の `wpg doctor` の結果を表示し、`--service` のときは `GET /api/health` が応答するまで (最大 30 秒) 待ちます。応答しなければ journal の末尾を表示します。

リモートのアカウントには `sudo` 権限が必要です。必要なときはパスワードを聞かれます。

## 例

```sh
# ビルド、アップロード、setcap のみ (ホスト上で `wpg send` / `wpg serve` を自分で実行する)
scripts/deploy.sh ubuntu@192.0.2.10

# 全アドレスで待ち受け、ens3 でだけ送信を許可するサービスとしてインストール
scripts/deploy.sh ubuntu@192.0.2.10 --service --iface ens3 --password-file ./admin-password

# タスクランナー経由でも同じ
just deploy ubuntu@192.0.2.10

# 何も変更せず、実行されるコマンドだけを表示
scripts/deploy.sh ubuntu@192.0.2.10 --service --dry-run
```

`--password-file` には、1 行目に管理者パスワード (平文) または argon2id ハッシュを書いたローカルファイルを指定します。ホストには `/etc/wpg/password` (モード `0640`、グループ `wpg`) として置かれます。指定しない場合、ホストにパスワードが既に無ければ (`/etc/wpg/password`、`/etc/wpg/wpg.toml` の `password_hash` / `password_file`、既存のユーザデータベースのいずれも無ければ)、ループバック以外のアドレスでサーバを起動することを拒否します。代わりに `--bind 127.0.0.1:8080` と SSH トンネル (`ssh -L 8080:127.0.0.1:8080 ubuntu@192.0.2.10`) を使うか、隔離された検証ネットワークに限って `--allow-insecure` を使ってください。

## オプション

| オプション | 意味 |
|---|---|
| `--iface <name>` | ジョブが送信してよいインタフェース (複数指定可)。`WPG_TX__ALLOWED_IFACES` に書かれます。管理用インタフェースは含めないでください。 |
| `--service` | `wpg.service` をインストール / 更新して再起動します。 |
| `--bind <addr:port>` | サービスの待ち受けアドレス (既定 `0.0.0.0:8080`)。 |
| `--password-file <file>` | サービスの管理者パスワード (または argon2 ハッシュ)。 |
| `--allow-insecure` | パスワード無しでループバック以外のアドレスで起動します (`wpg serve --allow-insecure` を実行する drop-in を追加)。 |
| `--binary <file>` | `target/release/wpg` の代わりにデプロイするバイナリ。 |
| `--target <triple>` | 別ターゲット向けにビルドします。例: `aarch64-unknown-linux-gnu` (クロスリンカが必要)。 |
| `--prefix <dir>` | ホスト側のインストール先 (既定 `/usr/local`)。 |
| `--port <n>`、`--ssh-opt <opt>` | SSH のポートと追加の `-o` オプション (例: `--ssh-opt IdentityFile=~/.ssh/lab`)。 |
| `--no-templates` | サンプルテンプレートを `/usr/local/share/wpg/templates` にコピーしません。 |
| `--no-healthcheck` | ヘルスチェックを省略します。 |
| `--rollback` | `/usr/local/bin/wpg` と `wpg.prev` を入れ替えてサービスを再起動します。2 回実行すると元に戻ります。 |
| `--dpdk`、`--hugepages <n>` | DPDK 向けにホストを準備します (後述)。 |
| `--dry-run` | コマンドを表示するだけで実行しません。 |

バイナリはホスト上で実行できる必要があります。ホストより新しいディストリビューションでビルドしたバイナリは glibc のバージョンエラーで動かないことがあります。スクリプトはインストール前にホスト上で `wpg --version` を実行して確認し、動かなければ中止します。デプロイ先で最も古いディストリビューションでビルドするか、リリースアーカイブ (Ubuntu 22.04 でビルド) を使ってください。

## ホストに置かれるもの

| パス | 内容 | deploy.sh が書き換えるか |
|---|---|---|
| `/usr/local/bin/wpg`、`wpg.prev` | 現在と 1 つ前のバイナリ | する |
| `/usr/local/share/wpg/templates/` | サンプルテンプレート (テンプレートライブラリ) | する |
| `/etc/systemd/system/wpg.service` | `packaging/systemd/wpg.service` の unit | する |
| `/etc/wpg/wpg.env` | `WPG_SERVER__BIND`、`WPG_TX__ALLOWED_IFACES`、ライブラリのパス、パスワードファイル | する |
| `/etc/wpg/wpg.toml` | 設定ファイル全体 (`wpg config init`) | **しない** (初回のみ作成) |
| `/etc/wpg/password` | 管理者パスワード | `--password-file` 指定時のみ |
| `/var/lib/wpg/` | データディレクトリ: データベース、ユーザテンプレート、TLS 証明書 | しない |

環境変数は `wpg.toml` より優先されます。`wpg.env` のキーは deploy.sh のオプションで変更し、それ以外は `wpg.toml` で設定してください。unit 自体については [サービスとして動かす](services.md) を参照してください。

## `--dpdk`: ホストの準備のみ

> **DPDK バックエンドはまだ実装されていません (マイルストーン M8)。** `--dpdk` はバックエンドの実装に備えてホストを準備するだけです。wpg の送信は引き続き `afpacket` で行います。

`--dpdk` を付けると、スクリプトは次を行います。

- 2 MiB の hugepage を `--hugepages` 枚 (既定 1024 = 2 GiB) すぐに確保し、起動時にも確保されるようにします (`/etc/sysctl.d/80-wpg-hugepages.conf`)。何もマウントされていなければ `/dev/hugepages` に `hugetlbfs` をマウントします。
- `vfio-pci` をすぐに読み込み、起動時にも読み込まれるようにします (`/etc/modules-load.d/wpg-dpdk.conf`)。
- IOMMU グループが無いときに警告します (仮想 IOMMU の無い CML2 の VM で典型的です)。

NIC を `vfio-pci` にバインドすることはしません。インタフェースがカーネルから外れ、SSH セッションが切れることがあるためです。`dpdk-devbind.py` で手動で行ってください。[DPDK / AF_XDP のセットアップ](dpdk-afxdp.md) を参照してください。

## アップグレードとロールバック

同じコマンドを再実行するとアップグレードになります。新しいバイナリが古いものを置き換え、サービスが再起動し、設定とデータは残ります。[アップグレード](upgrade.md) を参照してください。新しいバージョンに問題があれば次を実行します。

```sh
scripts/deploy.sh ubuntu@192.0.2.10 --rollback
```

## 確認状況

systemd と sshd が動く Ubuntu 24.04 コンテナに対して、インストール、パスワード無しでの拒否、`--service` とヘルスチェック、`--rollback`、`--dpdk` (hugepages。コンテナでは `vfio-pci` は使えません) を確認しました。**CML2 のノードや実機ではまだ確認していません。**
