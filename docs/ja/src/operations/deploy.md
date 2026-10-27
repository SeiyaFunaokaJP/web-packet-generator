# デプロイ (deploy.bat / deploy.ps1 / deploy.sh)

Status: draft

wpg を SSH で Linux ホスト (Ubuntu 20.04〜24.04、Debian。第一の対象は CML2 上の Ubuntu ノード) に入れるスクリプトです。Windows からは `scripts\deploy.bat` (中身は `scripts\deploy.ps1`)、Linux / macOS からは `scripts/deploy.sh` を使います。どちらもホスト側では同じスクリプト (`scripts/lib/*.sh`) を使うため、結果は同じです。

**引数を覚える必要はありません。** 引数なしで起動すると、ウィザードが必要なことを 1 つずつ日本語で聞きます (英語の環境では英語)。どの質問も Enter だけで `[ ]` 内の既定値を選べます。

## ウィザードで入れる (おすすめ)

### Windows

エクスプローラでリポジトリの `scripts` フォルダを開き、**`deploy.bat` をダブルクリック**します。PowerShell から実行してもかまいません。

```powershell
.\scripts\deploy.bat           # または: powershell -ExecutionPolicy Bypass -File .\scripts\deploy.ps1
```

必要なもの: Windows 10 1809 以降 / Windows 11 に標準で入っている OpenSSH クライアント (`ssh`、`scp`、`ssh-keygen`) と `tar.exe`。OpenSSH クライアントが無い場合は「設定 > アプリ > オプション機能」で「OpenSSH クライアント」を追加します。Windows 上に Rust や Git は不要です (Git があれば、ソースのアップロードに `git archive` を使います)。

### Linux / macOS

```sh
scripts/deploy.sh              # 引数なし = ウィザード (just deploy-wizard でも同じ)
```

`sshpass` が入っていれば SSH パスワードの入力に使います (無くても動きます)。

### 画面の例

初回 (Windows の PowerShell 5.1)。`****` の部分は入力が表示されません。

```text
wpg デプロイウィザード (Enter で [ ] 内の既定値を使います。Ctrl+C で中止)
接続先ホスト (IP アドレスまたはホスト名): 192.0.2.10
SSH ポート [22]:
SSH ユーザ名 [ubuntu]: cisco
SSH の認証方法:
  1) パスワード (初回だけ使い、デプロイ用の鍵を登録します)
  2) 既存の秘密鍵ファイル
番号を選んでください [1]:

[1] cisco@192.0.2.10 に接続
SSH パスワード (cisco@192.0.2.10): ********
  デプロイ用の鍵を作りました: C:\Users\you\.ssh\wpg_deploy_ed25519
  公開鍵をホストの ~/.ssh/authorized_keys に登録します (パスワードを使うのはこの 1 回だけです)。
  鍵でログインできるようになりました。次回からパスワードは不要です。
  OS: Ubuntu 24.04.1 LTS (x86_64)   sudo: password   wpg: -
wpg のバイナリの用意:
  1) ホスト上でソースからビルド (推奨。ホストにインターネット接続が必要。初回 10〜30 分)
  2) 手元の Linux 用バイナリファイルをアップロード
  3) GitHub Release からダウンロード (まだリリースがありません)
番号を選んでください [1]:
ソースコード:
  1) このパソコンのソース (C:\work\web-packet-generator) をアップロード
  2) GitHub から git clone
番号を選んでください [1]:
systemd サービスとしてインストールし、起動時に自動で開始しますか? [Y/n]:
Web UI の待ち受けアドレス:ポート (0.0.0.0 = すべて) [0.0.0.0:8080]:
送信を許可するインタフェース (tx.allowed_ifaces):
  1) ens2         UP       192.0.2.10/24  <- 管理用 (SSH 接続中)
  2) ens3         UP
  3) ens4         UP
番号をカンマ区切りで (例 2,3。all = すべて許可) [2,3]:
Web ターミナルを有効にしますか? (対応する wpg のみ) [y/N]:
SNMP トラップ受信 (UDP 162) を有効にしますか? (対応する wpg のみ) [y/N]: y
sudo パスワード (Enter = SSH パスワードと同じ):
wpg の管理者パスワード (Web UI のログイン用): ********
もう一度入力してください: ********

確認
  接続先 : cisco@192.0.2.10 (port 22)
  バイナリ : ホスト上でビルド (このパソコンのソース: C:\work\web-packet-generator)
  サービス : systemd (wpg.service)
  待ち受け : 0.0.0.0:8080
  送信インタフェース : ens3,ens4
  管理者パスワード : 設定する
  Web ターミナル : 無効
  SNMP トラップ受信 : 有効 (UDP 162)
この内容で実行しますか? [Y/n]:
この設定を次回のために保存しますか? (パスワードは保存しません) [Y/n]:
設定を保存しました: C:\Users\you\AppData\Roaming\wpg\deploy.conf (パスワードは保存していません)
ログ: C:\Users\you\AppData\Local\wpg\deploy-logs\deploy-20261027-101500.log

[2/6] アップロード (cisco@192.0.2.10:/tmp/wpg-deploy.Xa81Qe)
[3/6] ビルドに必要なパッケージの導入 (apt)
  -> apt-get install build-essential pkg-config libpcap-dev curl git ca-certificates tar
[4/6] ホスト上でビルド (初回は 10〜30 分かかります)
  -> installing rustup (https://rustup.rs) into ~/.cargo
  -> installed trunk 0.22.1 (prebuilt)
  ...
  -> built wpg 0.1.0
[5/6] インストール (sudo)
  -> installed wpg 0.1.0 to /usr/local/bin/wpg
  -> setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip /usr/local/bin/wpg
  -> systemd unit wpg.service (re)started
[6/6] ヘルスチェック (http://127.0.0.1:8080/api/health)
  {"status":"ok","version":"0.1.0",...}

完了しました。ブラウザで開いてください:
  http://192.0.2.10:8080/
```

`deploy.sh` のウィザードも同じ質問をします (バイナリの用意に「このマシンでビルドしてアップロード」が加わります。sudo パスワードは接続の直後に聞きます)。

### 聞かれる項目

| 項目 | 既定値 | 補足 |
|---|---|---|
| 接続先ホスト | 前回の値 | IP アドレスまたはホスト名。 |
| SSH ポート | 22 | |
| SSH ユーザ名 | ubuntu | `sudo` を使えるユーザ。 |
| 認証方法 | パスワード | パスワードは 1 回だけ使い、デプロイ用の鍵を登録します (後述)。既存の鍵ファイルも選べます。鍵で入れるときは聞きません。 |
| sudo パスワード | SSH パスワードと同じ | `NOPASSWD` の sudo や root ログインなら聞きません。間違えると 3 回まで聞き直します。 |
| バイナリの用意 | ホスト上でビルド | ホストがインターネットに出られないときは既定が変わります。下の「バイナリの用意」を参照。 |
| サービスとして入れる | はい | systemd の `wpg.service`。 |
| 待ち受けアドレス | `0.0.0.0:8080` | `127.0.0.1:8080` にすると SSH トンネル経由でだけ使えます。 |
| 管理者パスワード | (必須) | Web UI のログイン (ユーザ `admin`) 用。2 回入力します。インストール済みなら Enter で変更しません。 |
| 送信インタフェース | 管理用以外のすべて | ホストの `ip -br addr` を番号付きで表示します。SSH に使っているインタフェースには「管理用」と出ます。`all` で制限なし。 |
| Web ターミナル / SNMP トラップ受信 | いいえ | 対応するバージョンの wpg のときだけ有効になります (後述)。 |

### 2 回目以降

回答 (パスワード以外) は Windows では `%APPDATA%\wpg\deploy.conf`、Linux / macOS では `~/.config/wpg/deploy.conf` に保存され、次回は最初に「前回の設定を使いますか?」と聞きます。Enter で前回と同じ内容になり、聞かれるのはパスワードだけです (鍵でログインできるので SSH パスワードは不要です)。

wpg がすでに入っていて `wpg.prev` があると、最初にメニューが出ます。

```text
wpg はインストール済みです (wpg 0.1.0)。何をしますか?
  1) インストール / アップグレード
  2) ロールバック (1 つ前のバイナリ wpg.prev に戻す)
  3) 中止
```

## バイナリの用意

| 方法 | 向いている場合 | 必要なもの |
|---|---|---|
| ホスト上でソースからビルド (既定) | ほとんどの場合。ホストの glibc に合ったバイナリができます。 | ホストからインターネット (apt、rustup、crates.io、GitHub) に出られること。メモリ 2 GiB 以上 (足りなければスワップ)、空きディスク 5 GiB 程度。 |
| 手元の Linux 用バイナリをアップロード | 別の Linux マシンでビルド済みのとき。 | `wpg` (ELF) ファイル。Windows の `wpg.exe` は使えません。ホストより新しい glibc でビルドしたものは動きません。 |
| GitHub Release からダウンロード | リリースが公開された後。 | 手元のマシンからインターネット。**現時点ではリリースはまだありません** (ウィザードに「まだリリースがありません」と出ます)。 |
| このマシンでビルド (deploy.sh のみ) | 手元が同じ CPU アーキテクチャの Linux のとき。 | Rust と trunk (従来の `deploy.sh` の動作)。 |

ホスト上でビルドするときは次を行います。

1. root で build-essential、pkg-config、libpcap-dev、curl、git、ca-certificates を `apt-get` で入れます (dnf のディストリビューションでは相当するパッケージをできる範囲で入れます)。
2. SSH ユーザで rustup (stable、minimal プロファイル、`~/.cargo`)、`wasm32-unknown-unknown` ターゲット、trunk (ビルド済みのものを取得。取れなければ `cargo install trunk`) を入れます。
3. ソースを `~/wpg-build/src` に用意します。既定はこのパソコンのソースのアップロードです (Git があれば `git archive`。コミットしていない変更も含みますが、Git 管理外のファイルは含みません。Git が無ければ `tar.exe` で `target` などを除いて固めます)。GitHub からの `git clone` (ブランチ / タグ指定) も選べます。
4. `scripts/build.sh --release` でビルドします。`~/wpg-build/target` は次回も使うため、2 回目以降のビルドは速くなります。メモリが少ないホストでは並列数を自動で減らします。

## 1 回の実行で行うこと

1. SSH 接続 (必要なら鍵の登録) と、ホストの情報の取得 (OS、CPU、sudo、インタフェース、インストール済みの wpg、インターネット接続)。
2. ホストの一時ディレクトリ (`/tmp/wpg-deploy.*`、終了時に削除) へのアップロード。
3. (ホスト上でビルドするとき) 依存パッケージの導入とビルド。
4. インストール (`scripts/lib/deploy-remote.sh` を root で実行): バイナリを `/usr/local/bin/wpg` に置き (直前のものは `wpg.prev`)、`setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip` を付けます (`setcap` が無く `apt-get` があれば `libcap2-bin` を先に入れます)。サービスとして入れるときは、システムユーザ `wpg`、`/etc/wpg/wpg.toml` (無い場合のみ)、`/etc/wpg/wpg.env`、`/etc/wpg/password` を作り、`wpg.service` を (再) 起動します。ufw / firewalld が有効でポートが開いていなければ、開け方を表示します (ファイアウォールは変更しません)。
5. ホスト上の `wpg doctor` の表示と、`GET /api/health` が応答するまでの待機 (最大 30 秒。応答しなければ journal の末尾を表示)。
6. 開く URL の表示。

各ステップは `[3/6]` のように番号付きで色分けして表示され、すべてログファイルにも残ります (Windows: `%LOCALAPPDATA%\wpg\deploy-logs\`、deploy.sh のウィザード: `~/.local/state/wpg/`)。失敗すると、失敗したステップ、直前の出力、対処のヒントを表示します。

## コマンドラインで使う

### deploy.sh

従来のオプションはそのまま使えます。

```sh
# ビルド、アップロード、setcap のみ (ホスト上で `wpg send` / `wpg serve` を自分で実行する)
scripts/deploy.sh ubuntu@192.0.2.10

# 全アドレスで待ち受け、ens3 でだけ送信を許可するサービスとしてインストール
scripts/deploy.sh ubuntu@192.0.2.10 --service --iface ens3 --password-file ./admin-password

# ホスト上でソースからビルドしてインストール (パスワードは 1 回だけ聞かれ、鍵が登録される)
scripts/deploy.sh ubuntu@192.0.2.10 --ask-password --sudo-password --build-on-host --service --iface ens3

# タスクランナー経由でも同じ
just deploy ubuntu@192.0.2.10

# 何も変更せず、実行されるコマンドだけを表示
scripts/deploy.sh ubuntu@192.0.2.10 --service --dry-run
```

`--password-file` には、1 行目に管理者パスワード (平文) または argon2id ハッシュを書いたローカルファイルを指定します。ホストには `/etc/wpg/password` (モード `0640`、グループ `wpg`) として置かれます。指定しない場合、ホストにパスワードが既に無ければ (`/etc/wpg/password`、`/etc/wpg/wpg.toml` の `password_hash` / `password_file`、既存のユーザデータベースのいずれも無ければ)、ループバック以外のアドレスでサーバを起動することを拒否します。代わりに `--bind 127.0.0.1:8080` と SSH トンネル (`ssh -L 8080:127.0.0.1:8080 ubuntu@192.0.2.10`) を使うか、隔離された検証ネットワークに限って `--allow-insecure` を使ってください。

| オプション | 意味 |
|---|---|
| (引数なし)、`--interactive` | ウィザード。 |
| `--iface <name>` | ジョブが送信してよいインタフェース (複数指定可)。`WPG_TX__ALLOWED_IFACES` に書かれます。管理用インタフェースは含めないでください。 |
| `--service` | `wpg.service` をインストール / 更新して再起動します。 |
| `--bind <addr:port>` | サービスの待ち受けアドレス (既定 `0.0.0.0:8080`)。 |
| `--password-file <file>` | サービスの管理者パスワード (または argon2 ハッシュ)。 |
| `--allow-insecure` | パスワード無しでループバック以外のアドレスで起動します (`wpg serve --allow-insecure` を実行する drop-in を追加)。 |
| `--binary <file>` | `target/release/wpg` の代わりにデプロイするバイナリ。 |
| `--target <triple>` | 別ターゲット向けにビルドします。例: `aarch64-unknown-linux-gnu` (クロスリンカが必要)。 |
| `--build-on-host` | ホスト上でソースからビルドします。`--source local` (既定。このチェックアウトをアップロード) か `--source git` (`--git-url`、`--git-ref`。既定は GitHub の `main`)。 |
| `--release-tag <tag>` | GitHub Release (`latest` で最新) の Linux 用バイナリを使います。 |
| `--prefix <dir>` | ホスト側のインストール先 (既定 `/usr/local`)。 |
| `--port <n>`、`--ssh-opt <opt>` | SSH のポートと追加の `-o` オプション (例: `--ssh-opt IdentityFile=~/.ssh/lab`)。 |
| `--identity <file>` | 使う SSH 秘密鍵。 |
| `--ask-password` | SSH パスワードを 1 回聞き、デプロイ用の鍵 `~/.ssh/wpg_deploy_ed25519` をホストに登録します。 |
| `--sudo-password` | sudo パスワードを先に聞き、`sudo -S` に標準入力で渡します (既定では sudo が SSH の端末で聞きます)。 |
| `--terminal`、`--snmp-trap` | Web ターミナル / SNMP トラップ受信を有効にします (後述)。 |
| `--no-templates` | サンプルテンプレートを `/usr/local/share/wpg/templates` にコピーしません。 |
| `--no-healthcheck` | ヘルスチェックを省略します。 |
| `--rollback` | `/usr/local/bin/wpg` と `wpg.prev` を入れ替えてサービスを再起動します。2 回実行すると元に戻ります。 |
| `--dpdk`、`--hugepages <n>` | DPDK 向けにホストを準備します (後述)。 |
| `--profile <file>`、`--lang <ja\|en>`、`--log <file>` | ウィザードの回答ファイル、表示言語、ログファイル。 |
| `--dry-run` | コマンドを表示するだけで実行しません。 |

### deploy.ps1

ウィザードの質問はすべてパラメータでも渡せます。`-HostName` を付けるとウィザードは起動しません (足りないパスワードだけを聞きます)。

```powershell
# ホスト上でビルドしてサービスとして入れる (確認なし)
.\scripts\deploy.ps1 -HostName 192.0.2.10 -User cisco -Iface ens3,ens4 -Yes

# 手元の Linux 用バイナリを使う
.\scripts\deploy.ps1 -HostName 192.0.2.10 -User cisco -BinarySource file -Binary .\wpg

# GitHub の v0.2.0 タグからビルド
.\scripts\deploy.ps1 -HostName 192.0.2.10 -Source git -GitRef v0.2.0

# ロールバック
.\scripts\deploy.ps1 -HostName 192.0.2.10 -User cisco -Rollback
```

| パラメータ | 意味 |
|---|---|
| `-HostName`、`-User`、`-Port`、`-Identity` | 接続先、SSH ユーザ (既定 `ubuntu`)、ポート、既存の秘密鍵。 |
| `-SshPassword`、`-SudoPassword`、`-AdminPassword` | `SecureString` (`(Read-Host -AsSecureString)` など)。省略すると必要なときに聞きます。 |
| `-BinarySource build\|file\|release` | ホスト上でビルド (既定)、`-Binary` のファイル、GitHub Release (`-ReleaseTag`)。 |
| `-Source local\|git`、`-GitUrl`、`-GitRef` | ビルドするソース。 |
| `-Bind`、`-Iface` | 待ち受けアドレス、送信を許可するインタフェース。 |
| `-NoService` | サービスを入れません (既定は入れます)。 |
| `-Terminal`、`-SnmpTrap` | Web ターミナル / SNMP トラップ受信。 |
| `-Rollback`、`-NoTemplates`、`-NoHealthcheck` | deploy.sh と同じ。 |
| `-ProfilePath`、`-Lang ja\|en`、`-LogFile` | 回答ファイル、表示言語、ログファイル。 |
| `-Interactive`、`-Yes`、`-DryRun` | パラメータを既定値にしてウィザードを起動 / 確認を省略 / 接続と確認だけ行い、ビルドとインストールはしない。 |

## セキュリティ

- **SSH パスワードは鍵の登録に 1 回だけ使います。** デプロイ用の鍵 (`~/.ssh/wpg_deploy_ed25519`、Windows では `%USERPROFILE%\.ssh\wpg_deploy_ed25519`、パスフレーズ無し) を作り、公開鍵をホストの `~/.ssh/authorized_keys` に追記します。パスワードは `sshpass` (deploy.sh で入っていれば)、または `SSH_ASKPASS` の一時ヘルパー (OpenSSH 8.4 以降。パスワードは環境変数で渡し、ファイルには書きません。ヘルパーはすぐ削除します) で渡し、どちらも使えなければ ssh 自身がもう一度パスワードを聞きます。鍵が不要になったら、ホストの `~/.ssh/authorized_keys` から `wpg-deploy@...` の行を削除してください。
- **sudo パスワードと管理者パスワードは ssh の標準入力で渡します** (`sudo -S`、`cat > /tmp/wpg-deploy.*/password`)。コマンドラインやプロセス一覧、手元のディスクには現れません。保存する回答ファイルにもパスワードは入りません。
- 初めて接続するホストの鍵は `StrictHostKeyChecking=accept-new` で自動的に受け入れます (変わった鍵は拒否します)。検証ネットワーク以外では、事前に `ssh` で一度接続して指紋を確認してください。
- 管理者パスワードはホストに `/etc/wpg/password` (`0640 root:wpg`) として置かれ、wpg が起動時にハッシュ化します。
- 送信を許可するインタフェースから管理用インタフェースを外してください (ウィザードの既定はそうなっています)。試験トラフィックで SSH が切れることがあります。

## Web ターミナルと SNMP トラップ受信

ウィザードで有効にする (`--terminal` / `--snmp-trap`、`-Terminal` / `-SnmpTrap`) と、`/etc/wpg/wpg.toml` に次を書きます。

```toml
[terminal]
enabled = true
allowed_targets = []

[snmp]
trap_listen = "0.0.0.0:162"
```

- 書くのは **このデプロイで新しく作った `wpg.toml` だけ** です。既存の `wpg.toml` は変更せず、追記する内容と `sudo systemctl restart wpg` を表示します。
- 古い wpg は知らないキーがあると起動しないため、入れたバイナリの `wpg config validate` がそのキーを受け付けるときだけ書きます。受け付けなければ警告を出して飛ばします。
- `trap_listen` があると、UDP 162 (特権ポート) で待ち受けられるように `wpg.service` に drop-in (`/etc/systemd/system/wpg.service.d/bind-service.conf`、`CAP_NET_BIND_SERVICE`) を追加します。ufw が有効なら `sudo ufw allow 162/udp` を案内します。

## ホストに置かれるもの

| パス | 内容 | デプロイで書き換えるか |
|---|---|---|
| `/usr/local/bin/wpg`、`wpg.prev` | 現在と 1 つ前のバイナリ | する |
| `/usr/local/share/wpg/templates/` | サンプルテンプレート (テンプレートライブラリ) | する |
| `/etc/systemd/system/wpg.service` | `packaging/systemd/wpg.service` の unit | する |
| `/etc/systemd/system/wpg.service.d/` | `allow-insecure.conf` (`--allow-insecure`)、`bind-service.conf` (SNMP トラップ) | する |
| `/etc/wpg/wpg.env` | `WPG_SERVER__BIND`、`WPG_TX__ALLOWED_IFACES`、ライブラリのパス、パスワードファイル | する |
| `/etc/wpg/wpg.toml` | 設定ファイル全体 (`wpg config init`) | **しない** (初回のみ作成) |
| `/etc/wpg/password` | 管理者パスワード | 指定したときのみ |
| `/var/lib/wpg/` | データディレクトリ: データベース、ユーザテンプレート、TLS 証明書 | しない |
| `~/wpg-build/`、`~/.cargo/`、`~/.rustup/` (SSH ユーザ) | ホスト上でビルドしたときのソース、ビルドキャッシュ、Rust ツールチェーン | ビルド時のみ |

環境変数は `wpg.toml` より優先されます。`wpg.env` のキーはデプロイのオプションで変更し、それ以外は `wpg.toml` で設定してください。unit 自体については [サービスとして動かす](services.md) を参照してください。

## `--dpdk`: ホストの準備のみ

> `--dpdk` はホストを準備するだけです。`dpdk` バックエンドを使うには、`--features dpdk` 付きでビルドしたバイナリ (`scripts/build.sh --dpdk`。`--build-on-host` と組み合わせるとホスト上で `--features dpdk` でビルドします)、`vfio-pci` にバインドした NIC、`tx.dpdk_eal_args` の EAL 引数が必要です。[DPDK / AF_XDP のセットアップ](dpdk-afxdp.md#dpdk) を参照してください。

`--dpdk` を付けると、スクリプトは次を行います。

- 2 MiB の hugepage を `--hugepages` 枚 (既定 1024 = 2 GiB) すぐに確保し、起動時にも確保されるようにします (`/etc/sysctl.d/80-wpg-hugepages.conf`)。何もマウントされていなければ `/dev/hugepages` に `hugetlbfs` をマウントします。
- `vfio-pci` をすぐに読み込み、起動時にも読み込まれるようにします (`/etc/modules-load.d/wpg-dpdk.conf`)。
- IOMMU グループが無いときに警告します (仮想 IOMMU の無い CML2 の VM で典型的です)。

NIC を `vfio-pci` にバインドすることはしません。インタフェースがカーネルから外れ、SSH セッションが切れることがあるためです。`dpdk-devbind.py` で手動で行ってください。[DPDK / AF_XDP のセットアップ](dpdk-afxdp.md) を参照してください。

## アップグレードとロールバック

同じ操作 (ウィザードなら「前回の設定を使いますか?」に Enter) を再実行するとアップグレードになります。新しいバイナリが古いものを置き換え、サービスが再起動し、設定とデータは残ります。[アップグレード](upgrade.md) を参照してください。新しいバージョンに問題があれば、ウィザードのメニューで「ロールバック」を選ぶか、次を実行します。

```sh
scripts/deploy.sh ubuntu@192.0.2.10 --rollback
```

## トラブルシューティング

| 症状 | 対処 |
|---|---|
| `ssh / scp が見つかりません` (Windows) | 「設定 > アプリ > オプション機能」で「OpenSSH クライアント」を追加します (管理者 PowerShell: `Add-WindowsCapability -Online -Name OpenSSH.Client~~~~0.0.1.0`)。 |
| 接続できない / タイムアウト | アドレス、ポート、ユーザ名を確認し、手動で `ssh -p 22 user@host` を試します。CML2 では管理用リンク (External Connector) が必要です。 |
| `公開鍵を登録できませんでした` | パスワードの誤り、またはサーバでパスワード認証が無効 (`PasswordAuthentication no`)。既存の鍵ファイルを選ぶか、ホスト側で鍵を登録してください。 |
| `鍵でログインできません` | サーバで公開鍵認証が無効、または `~/.ssh` の権限が緩すぎます (`chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys`)。 |
| `sudo に失敗しました` | sudo パスワードの誤り、またはユーザが sudo グループに入っていません (`sudo usermod -aG sudo <user>`)。 |
| ステップ「パッケージの導入」で失敗 | ホストからインターネットに出られるか (プロキシなら `/etc/apt/apt.conf.d/` と `~/.bashrc` に設定)、apt が自動更新でロックされていないかを確認し、数分後に再実行します。 |
| ステップ「ビルド」で `killed` / `signal: 9` | メモリ不足です。スワップを追加して (`sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile`) 再実行します。 |
| ビルドが毎回失敗する | ホスト上で `rm -rf ~/wpg-build` を実行してから再実行します。 |
| `the uploaded binary does not run on this host` | 手元のバイナリがホストの glibc / CPU に合いません。ホスト上でのビルドを選びます。 |
| ヘルスチェックが失敗 | ホストで `sudo journalctl -u wpg -n 50`。ポートの重複は `sudo ss -ltnp`。管理者パスワードが無いとループバック以外では起動しません。 |
| ブラウザから開けない | ファイアウォール (`sudo ufw allow 8080/tcp`)、CML2 の外部接続、待ち受けアドレス (`127.0.0.1` だと SSH トンネルが必要) を確認します。 |
| `does not support [terminal]` / `[snmp]` | 入れた wpg がその機能より古いバージョンです。新しいソースでデプロイし直してください。 |
| 日本語が文字化けする (Windows) | `deploy.ps1` を編集して保存し直した場合は、UTF-8 (BOM 付き) で保存してください。 |

## 確認状況

`deploy.sh` は、Ubuntu 24.04 コンテナ (sshd、systemd の代わりに簡易な `systemctl`) に対して、ウィザード (パスワードでの鍵登録: `sshpass` と `SSH_ASKPASS` の両方、sudo パスワード、インタフェース選択、回答の保存と再利用、ロールバックのメニュー)、ホスト上でのビルド (このチェックアウトのアップロード、rustup と trunk の導入からヘルスチェックまで)、新しい `wpg.toml` への `[terminal]` / `[snmp]` の追記 (対応するバイナリを模したスタブ) と既存の `wpg.toml` を変更しないことを確認しました。従来の、systemd が動くコンテナでのインストール、パスワード無しでの拒否、`--service` とヘルスチェック、`--rollback`、`--dpdk` の確認結果はそのままです。**`deploy.ps1` / `deploy.bat` は Windows で実行していません** (CI でパースと PowerShell 5.1 互換の構文チェックのみ)。CML2 のノードや実機でもまだ確認していません。
