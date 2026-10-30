# デプロイ (deploy.bat / deploy.ps1 / deploy.sh)

Status: draft

wpg を SSH で Linux ホスト (Ubuntu 20.04〜24.04、Debian。第一の対象は CML2 上の Ubuntu ノード) に入れるスクリプトです。Windows からは `scripts\deploy.bat` (中身は `scripts\deploy.ps1`)、Linux / macOS からは `scripts/deploy.sh` を使います。どちらもホスト側では同じスクリプト (`scripts/lib/*.sh`) を使うため、結果は同じです。

**引数を覚える必要はありません。** 引数なしで起動すると、ウィザードが必要なことを 1 つずつ日本語で聞きます (英語の環境では英語)。どの質問も Enter だけで `[ ]` 内の既定値を選べます。

**自動実行もできます。** ウィザードの最後に保存できる回答ファイルを使うと、同じデプロイを質問なしで実行できます (定期アップグレード、複数台、CI)。[自動実行 (無人実行)](#自動実行-無人実行) を参照してください。

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
DHCP サーバーが応答するインタフェース (空 / - = 画面で選択。対応する wpg のみ): ens4
ens4 の固定 IP (address/len をカンマ区切り。例 192.168.100.1/24,2001:db8:1::1/64。空 / - = 設定しない): 192.168.100.1/24
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
  DHCP サーバー : ens4 192.168.100.1/24
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

回答 (パスワード以外) は Windows では `%APPDATA%\wpg\deploy.conf`、Linux / macOS では `~/.config/wpg/deploy.conf` に保存され、次回は最初に「前回の設定を使いますか?」と聞きます。Enter で前回と同じ内容になり、聞かれるのはパスワードだけです (鍵でログインできるので SSH パスワードは不要です)。デプロイが成功すると最後に、同じ内容を質問なしで実行するためのファイルを保存するかを聞きます ([自動実行](#自動実行-無人実行))。

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
| `--no-service` | サービスを入れません (`--config` の `service=1` を打ち消します)。 |
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
| `--dhcp-iface <名前>` | DHCP サーバーが応答するインタフェースを固定します (`/etc/wpg/wpg.env` の `WPG_DHCP__IFACE`。`--service` と併用)。[DHCP サーバー](dhcp.md) を参照。 |
| `--dhcp-iface-addr <アドレス/長さ>` | `--dhcp-iface` の固定 IP (複数可、カンマ区切り可。例 `192.168.100.1/24,2001:db8:1::1/64`)。`/etc/netplan/90-wpg-<IF>.yaml` を書いて `netplan apply` します。デフォルトルートのある IF には設定しません。 |
| `--no-templates` | サンプルテンプレートを `/usr/local/share/wpg/templates` にコピーしません。 |
| `--no-healthcheck` | ヘルスチェックを省略します。 |
| `--rollback` | `/usr/local/bin/wpg` と `wpg.prev` を入れ替えてサービスを再起動します。2 回実行すると元に戻ります。 |
| `--dpdk`、`--hugepages <n>` | DPDK 向けにホストを準備します (後述)。 |
| `--profile <file>`、`--lang <ja\|en>`、`--log <file>` | ウィザードの回答ファイル、表示言語、ログファイル。 |
| `--dry-run` | コマンドを表示するだけで実行しません。 |
| `--config`、`--yes`、`--json`、`--skip-if-current`、`--check-config`、`--targets`、`--section`、`--parallel`、`--set` | 自動実行用 ([自動実行](#自動実行-無人実行))。 |

### deploy.ps1

ウィザードの質問はすべてパラメータでも渡せます。`-HostName` を付けるとウィザードは起動しません (足りないパスワードだけを聞きます。`-Yes` を付けると何も聞かず、足りなければすぐに失敗します)。

```powershell
# ホスト上でビルドしてサービスとして入れる (質問なし。sudo パスワードは環境変数 WPG_DEPLOY_SUDO_PASSWORD など)
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
| `-DhcpIface`、`-DhcpIfaceAddr` | DHCP サーバーのインタフェースと固定 IP (`--dhcp-iface` / `--dhcp-iface-addr` と同じ)。 |
| `-Rollback`、`-NoTemplates`、`-NoHealthcheck` | deploy.sh と同じ。 |
| `-ProfilePath`、`-Lang ja\|en`、`-LogFile` | 回答ファイル、表示言語、ログファイル。 |
| `-Interactive`、`-Yes`、`-DryRun` | パラメータを既定値にしてウィザードを起動 / 何も質問しない (確認もパスワードも) / 接続と確認だけ行い、ビルドとインストールはしない。 |
| `-Prefix`、`-AllowInsecure` | ホスト側のインストール先、パスワード無しでの起動 (deploy.sh と同じ)。 |
| `-ConfigFile`、`-Json`、`-SkipIfCurrent`、`-CheckConfig`、`-Targets`、`-Section`、`-Parallel`、`-Set` | 自動実行用 ([自動実行](#自動実行-無人実行))。 |

## 自動実行 (無人実行)

手で入れるときはウィザード、決まった構成を繰り返し入れるとき (定期アップグレード、複数台、CI) は **回答ファイル + `--yes`** で、質問なしに最後まで実行できます。

### いちばん簡単な方法: ウィザードで作る

ウィザードの最後 (デプロイが成功した後) に次のように聞かれます。

```text
この設定で次回から自動実行するファイルを保存しますか? (パスワードは保存しません) [Y/n]:
保存先 [C:\Users\you\AppData\Roaming\wpg\deploy-192.0.2.10.conf]:
sudo パスワードを暗号化して保存しますか? (Windows の DPAPI。この Windows ユーザーがこの PC でだけ復号できます) [Y/n]:
保存しました: C:\Users\you\AppData\Roaming\wpg\deploy-192.0.2.10.conf (パスワードは平文では含みません)
次回からは、質問なしで次の 1 行で実行できます:
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "C:\work\web-packet-generator\scripts\deploy.ps1" -ConfigFile "C:\Users\you\AppData\Roaming\wpg\deploy-192.0.2.10.conf" -Yes -SkipIfCurrent
定期実行の例 (タスク スケジューラ、毎週日曜 3:17。ログは %LOCALAPPDATA%\wpg\deploy-logs に残ります):
  schtasks /Create /TN "wpg deploy 192.0.2.10" /SC WEEKLY /D SUN /ST 03:17 /TR "..."
```

表示された 1 行をそのままコピーして使えます (deploy.sh では `scripts/deploy.sh --config ~/.config/wpg/deploy-192.0.2.10.conf --yes --skip-if-current` と cron の例が出ます)。SSH はウィザードが登録したデプロイ用の鍵で入るので、SSH パスワードは不要です。sudo にパスワードが要るホストでは、Windows なら上の質問で DPAPI で暗号化して保存でき、Linux / macOS では環境変数かパスワードファイル (後述) を使います。

### 回答ファイル

ウィザードが保存する `deploy.conf` と同じ形式です (`キー=値` の行、`#` で始まる行はコメント)。手で書いてもかまいません。どちらのスクリプトでも同じファイルを読めます。

```ini
# deploy.wpg.conf
host=192.0.2.10
user=cisco
binary_source=build
source=git
git_ref=main
bind=0.0.0.0:8080
ifaces=ens3,ens4
service=1
sudo_password_file=/home/you/.config/wpg/sudo.pw
```

```sh
scripts/deploy.sh --config deploy.wpg.conf --yes                       # Linux / macOS
scripts/deploy.sh --config deploy.wpg.conf --check-config              # 読み込んだ設定を表示するだけ (接続しない)
```

```powershell
.\scripts\deploy.ps1 -ConfigFile .\deploy.wpg.conf -Yes                 # Windows
```

ファイル内の相対パス (`binary_file`、`identity`、`*_password_file` など) は回答ファイルのあるフォルダから見たパスです。コマンドラインのオプションは回答ファイルより優先されます (`--set key=value` / `-Set "key=value"` で任意のキーを上書きできます)。知らないキーがあるとエラー (終了コード 1) になるので、書き間違いに気付けます。

| キー | 既定値 | 意味 |
|---|---|---|
| `host` | (必須) | 接続先。`user@host` の形も可。 |
| `user` | `ubuntu` | SSH ユーザ。 |
| `ssh_port` (`port`) | `22` | SSH ポート。 |
| `identity` | (なし) | 使う SSH 秘密鍵。省略時はデプロイ用の鍵 (`wpg_deploy_ed25519`)、ssh-agent / 既定の鍵の順に試します。 |
| `binary_source` | `build` | `build` (ホスト上でビルド)、`file` (`binary_file` をアップロード)、`release` (GitHub Release)。deploy.sh では `local` (このマシンでビルド) も可。 |
| `binary_file` | | `binary_source=file` のときの Linux 用バイナリ。 |
| `source` | `local` | `build` のときのソース: `local` (このチェックアウトをアップロード) か `git`。 |
| `git_url`、`git_ref` | GitHub、`main` | `source=git` のときのリポジトリとブランチ / タグ / コミット。 |
| `release_tag` | `latest` | `binary_source=release` のときのタグ。 |
| `bind` | `0.0.0.0:8080` | 待ち受けアドレス。 |
| `ifaces` | (すべて) | 送信を許可するインタフェース (カンマ区切り)。 |
| `service` | `1` | systemd サービスとして入れる。 |
| `terminal`、`snmp_trap` | `0` | Web ターミナル / SNMP トラップ受信。 |
| `dhcp_iface`、`dhcp_iface_addr` | 空 | DHCP サーバーのインタフェースと固定 IP (カンマ区切り)。 |
| `templates`、`healthcheck` | `1` | サンプルテンプレートのコピー / ヘルスチェック。 |
| `allow_insecure` | `0` | パスワード無しでループバック以外で待ち受ける。 |
| `prefix` | `/usr/local` | ホスト側のインストール先。 |
| `skip_if_current` | `0` | `--skip-if-current` と同じ。 |
| `dpdk`、`hugepages` | `0`、`1024` | DPDK の準備 (deploy.sh のみ)。 |
| `admin_password_file`、`ssh_password_file`、`sudo_password_file` | | パスワード (1 行目) を書いた、自分で作ったファイル。`admin_password_file` は argon2 ハッシュでも可。 |
| `admin_password_dpapi`、`ssh_password_dpapi`、`sudo_password_dpapi` | | DPAPI で暗号化したファイル (deploy.ps1、Windows のみ。ウィザードが作ります)。 |
| `name` | セクション名 / ホスト | 結果の表と JSON に出す名前。 |
| `parallel` | `1` | 複数台のとき、同時に入れる台数。 |
| `lang` | 環境から | `ja` / `en`。 |

真偽値は `1` / `0` (`yes` / `no`、`true` / `false` も可) です。

### パスワードの渡し方 (質問なしのとき)

| 方法 | SSH | sudo | 管理者 (Web UI) |
|---|---|---|---|
| SSH 鍵 (おすすめ) | デプロイ用の鍵 / `identity=` / ssh-agent | - | - |
| 環境変数 | `WPG_DEPLOY_SSH_PASSWORD` | `WPG_DEPLOY_SUDO_PASSWORD` | `WPG_DEPLOY_ADMIN_PASSWORD` |
| ファイル (自分で作る、`chmod 600`) | `ssh_password_file=` | `sudo_password_file=` | `admin_password_file=` |
| DPAPI ファイル (Windows) | `ssh_password_dpapi=` | `sudo_password_dpapi=` | `admin_password_dpapi=` |

- 上から順に探します。deploy.ps1 では `-SshPassword` などの `SecureString` パラメータが最優先です。
- SSH パスワードは鍵でログインできないときだけ使い、そのときデプロイ用の鍵を登録します (次回からは不要)。sudo パスワードが無いときは SSH パスワードを試します。`NOPASSWD` の sudo や root ログインなら不要です。
- 管理者パスワードは、初回のインストールでループバック以外 (`0.0.0.0:8080` など) に待ち受けるときだけ必須です。2 回目以降は省略すると変更しません (指定すると毎回書き換えます)。
- **スクリプトがパスワードを平文でファイルに書くことはありません。** DPAPI ファイルは `ConvertFrom-SecureString` (鍵なし) の出力で、作った Windows ユーザーがその PC でだけ復号できます。タスク スケジューラでは同じユーザーで実行してください。
- 必要なパスワードが見つからないときは、質問せずにすぐ失敗します (どの環境変数 / キーを設定すればよいかを表示し、終了コード 2 (SSH / sudo) または 1 (管理者パスワード))。`--yes` / `-Yes` が無くても、標準入力が端末でない (cron、パイプ、`-NonInteractive`) ときは質問できないので、`--yes` を付けるよう表示して終了コード 1 で終わります。

### 終了コード

| コード | 意味 |
|---|---|
| 0 | 成功。`--skip-if-current` で何もしなかったとき、`--dry-run`、`--check-config` も 0。 |
| 1 | 使い方の誤り、回答ファイルの誤り、必要な設定や管理者パスワードが無い、`--yes` 無しで質問できない。 |
| 2 | 接続 / SSH 認証 / sudo の失敗 (SSH / sudo のパスワードが無い場合を含む)。 |
| 3 | バイナリの用意 (ローカルのビルド、Release のダウンロード、ホスト上の依存パッケージ導入とビルド) の失敗。 |
| 4 | アップロードまたはインストールの失敗。 |
| 5 | ヘルスチェックの失敗 (インストールは済んでいます)。 |
| 6 | 複数台のうち 1 台以上が失敗 (各ホストのコードは表と JSON にあります)。 |
| 130 | Ctrl+C などで中断 (deploy.sh)。 |

### `--json` / `-Json`

標準出力に結果の JSON だけを出します (ログは標準エラーとログファイルへ)。1 台でも複数台でも同じ形です。

```json
{"tool":"wpg-deploy","format":1,"status":"ok","exit_code":0,"dry_run":false,"message":"",
 "hosts":[{"name":"lab1","host":"192.0.2.10","user":"cisco","port":22,"status":"ok","exit_code":0,
  "failed_step":"","message":"","version_before":"wpg 0.1.0","version_after":"wpg 0.1.0",
  "source_id":"git:24d946f6614eecbff02ca481df380aafeb63f368","url":"http://192.0.2.10:8080/",
  "dry_run":false,"log":"/home/you/.local/state/wpg/deploy-20261027-031700-192.0.2.10.log",
  "started_at":"2026-10-27T03:17:00Z","duration_s":41}]}
```

ホストの `status` は `ok`、`skipped` (最新だったので何もしなかった)、`dry-run`、`checked`、`aborted`、`failed` のいずれかです。失敗すると `failed_step` (`config`、`connect`、`sudo`、`binary`、`deps`、`build`、`upload`、`install`、`health`) と `message` が入ります。

### `--skip-if-current` / `-SkipIfCurrent`: 変わっていなければ何もしない

デプロイのたびに、ホストの `/usr/local/share/wpg/deploy.stamp` に「何をどの設定で入れたか」(ソースの識別子、設定、バイナリの SHA-256、日時。パスワードは含みません) を記録します。`--skip-if-current` を付けると、接続して次がすべて一致するときはビルドもインストールもせずに終了します (終了コード 0、`status` は `skipped`)。sudo パスワードも使いません。

- ソースの識別子が同じ: `source=git` ならブランチ / タグが指すコミット (`git ls-remote` で解決)、`source=local` ならこのチェックアウトのコミット (コミットしていない変更があればその差分のハッシュも)、`binary_source=file` ならファイルの SHA-256、`release` なら Release のファイルの URL。
- 設定 (`bind`、`ifaces`、`service`、`terminal`、`snmp_trap`、`dhcp_iface`、`dhcp_iface_addr`、`templates`、`allow_insecure`、`prefix`) が同じ。
- インストール済みのバイナリが記録と同じ (ロールバックや手動の置き換えがあれば再デプロイします)。
- サービスとして入れるときは `wpg.service` が動いていて、ヘルスチェックに応答する。

`wpg --version` はコミットを含まないため、この記録ファイルで判定します。判定できないとき (Git が無い、参照を解決できない) は警告を出して通常どおりデプロイします。なお、同じバイナリを入れ直したときは `wpg.prev` を上書きしません (1 つ前のバージョンが残ります)。

### `--dry-run` / `-DryRun` (質問なし)

`--yes` と組み合わせると、接続してホストの情報を読み、パスワードがそろっているか (sudo のパスワードは実際に確認) を調べ、実行するはずのコマンドを表示します。ホストは変更しません (鍵の登録もしません)。CI で設定を確かめるのに使えます。接続せずに回答ファイルだけを確かめるには `--check-config` / `-CheckConfig` を使います。

### 複数台に入れる

回答ファイルに `[host <名前>]` のセクションを書くと、セクションごとに 1 台ずつ入れます。セクションの前に書いたキーは全台の共通の値で、セクション内の値が優先します (`host=` を省くとセクション名がホスト名になります)。

```ini
# lab.conf
user=cisco
binary_source=build
source=git
git_ref=v0.2.0
sudo_password_file=sudo.pw

[host r1]
host=192.0.2.11
ifaces=ens3

[host r2]
host=192.0.2.12
ifaces=ens3,ens4
bind=127.0.0.1:8080
```

ホストの一覧だけを別のファイルにすることもできます (`--targets` / `-Targets`。1 行 1 台、`[user@]host[:port]` の後に `キー=値` を並べられます。相対パスはこのファイルから見たパス)。

```text
# hosts.txt
cisco@192.0.2.11 ifaces=ens3
cisco@192.0.2.12:2222 ifaces=ens3,ens4
```

```sh
scripts/deploy.sh --config lab.conf --yes --parallel 2
scripts/deploy.sh --config common.conf --targets hosts.txt --yes --json > result.json
```

```powershell
.\scripts\deploy.ps1 -ConfigFile .\lab.conf -Yes -Parallel 2
```

各ホストは別のプロセスで実行され (`--parallel` / `-Parallel` で同時に実行する台数を指定、既定 1)、ホストごとのログは `~/.local/state/wpg/deploy-<時刻>/` (Windows: `%LOCALAPPDATA%\wpg\deploy-logs\deploy-<時刻>\`) に残ります。最後に表を表示し、1 台でも失敗すれば終了コード 6 で終わります。

```text
結果
  NAME HOST                     STATUS    EXIT VERSION      DETAIL
  r1   cisco@192.0.2.11         ok        0    0.2.0        git:5d0c1e2...
  r2   cisco@192.0.2.12         skipped   0    0.2.0        git:5d0c1e2...
  r3   cisco@192.0.2.13         failed    2                 cannot connect to cisco@192.0.2.13 (port 22): ssh: connect to host ...
成功 2 台、失敗 1 台
```

### 定期実行

#### Windows (タスク スケジューラ)

ウィザードが表示する `schtasks` の行をコマンド プロンプトに貼り付けます。手で作るときは次のとおりです (ログオンしていないときも実行するには `/RU <ユーザー名> /RP` を加えます。DPAPI ファイルを使うときは、ファイルを作ったユーザーで実行してください)。

```bat
schtasks /Create /TN "wpg deploy lab" /SC WEEKLY /D SUN /ST 03:17 ^
  /TR "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\work\web-packet-generator\scripts\deploy.ps1 -ConfigFile C:\Users\you\AppData\Roaming\wpg\lab.conf -Yes -SkipIfCurrent"
```

結果はタスク スケジューラの「前回の実行結果」(終了コード) と `%LOCALAPPDATA%\wpg\deploy-logs\` のログで確認できます。

#### Linux / macOS (cron)

```sh
# crontab -e
17 3 * * 0 /home/you/web-packet-generator/scripts/deploy.sh --config /home/you/.config/wpg/lab.conf --yes --skip-if-current >/dev/null 2>&1
```

sudo パスワードが要るときは、`sudo_password_file=` (自分で作り `chmod 600`) を回答ファイルに書くか、ホスト側でデプロイ用ユーザに `NOPASSWD` を設定してください (cron の環境では環境変数を使いにくいため)。

#### Linux (systemd ユーザタイマー)

```ini
# ~/.config/systemd/user/wpg-deploy.service
[Unit]
Description=wpg unattended deploy

[Service]
Type=oneshot
ExecStart=%h/web-packet-generator/scripts/deploy.sh --config %h/.config/wpg/lab.conf --yes --skip-if-current
```

```ini
# ~/.config/systemd/user/wpg-deploy.timer
[Timer]
OnCalendar=Sun 03:17
Persistent=true

[Install]
WantedBy=timers.target
```

```sh
systemctl --user daemon-reload && systemctl --user enable --now wpg-deploy.timer
journalctl --user -u wpg-deploy      # 実行結果
```

### CI から (GitHub Actions の例)

SSH 鍵と sudo パスワードをリポジトリのシークレットに入れておきます。ホストの公開鍵を `known_hosts` に入れておくと、初回接続で鍵を自動で受け入れずに済みます。

```yaml
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: deploy to the lab
        env:
          WPG_DEPLOY_SUDO_PASSWORD: ${{ secrets.LAB_SUDO_PASSWORD }}
          WPG_DEPLOY_ADMIN_PASSWORD: ${{ secrets.LAB_ADMIN_PASSWORD }}
        run: |
          install -m 700 -d ~/.ssh
          printf '%s\n' "${{ secrets.LAB_SSH_KEY }}" > ~/.ssh/lab_ed25519 && chmod 600 ~/.ssh/lab_ed25519
          printf '%s\n' "${{ secrets.LAB_KNOWN_HOSTS }}" >> ~/.ssh/known_hosts
          scripts/deploy.sh --config deploy/lab.conf --yes --skip-if-current --json \
            --set identity=$HOME/.ssh/lab_ed25519 --set source=git --set git_ref=${{ github.sha }} > deploy-result.json
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: deploy-result
          path: deploy-result.json
```

### 自動実行のオプション一覧

| deploy.sh | deploy.ps1 | 意味 |
|---|---|---|
| `--config <file>` | `-ConfigFile <file>` | 回答ファイル。 |
| `--yes` | `-Yes` | 何も質問しない。 |
| `--json` | `-Json` | 標準出力に JSON の結果。 |
| `--skip-if-current` | `-SkipIfCurrent` | 最新なら何もしない。 |
| `--dry-run` | `-DryRun` | 変更せずに確認とコマンドの表示。 |
| `--check-config` | `-CheckConfig` | 回答ファイルを確かめて設定を表示 (接続しない)。 |
| `--targets <file>` | `-Targets <file>` | ホストの一覧ファイル。 |
| `--section <name>` | `-Section <name>` | 回答ファイルの 1 つのセクションだけ。 |
| `--parallel <n>` | `-Parallel <n>` | 同時に入れる台数。 |
| `--set <key=value>` | `-Set "key=value"` | 回答ファイルのキーを上書き (複数可)。 |
| `--log <file>` | `-LogFile <file>` | ログファイル (既定: `~/.local/state/wpg/`、`%LOCALAPPDATA%\wpg\deploy-logs\`)。 |

## セキュリティ

- **SSH パスワードは鍵の登録に 1 回だけ使います。** デプロイ用の鍵 (`~/.ssh/wpg_deploy_ed25519`、Windows では `%USERPROFILE%\.ssh\wpg_deploy_ed25519`、パスフレーズ無し) を作り、公開鍵をホストの `~/.ssh/authorized_keys` に追記します。パスワードは `sshpass` (deploy.sh で入っていれば)、または `SSH_ASKPASS` の一時ヘルパー (OpenSSH 8.4 以降。パスワードは環境変数で渡し、ファイルには書きません。ヘルパーはすぐ削除します) で渡し、どちらも使えなければ ssh 自身がもう一度パスワードを聞きます。鍵が不要になったら、ホストの `~/.ssh/authorized_keys` から `wpg-deploy@...` の行を削除してください。
- **sudo パスワードと管理者パスワードは ssh の標準入力で渡します** (`sudo -S`、`cat > /tmp/wpg-deploy.*/password`)。コマンドラインやプロセス一覧、手元のディスクには現れません。保存する回答ファイルにもパスワードは入りません。
- 初めて接続するホストの鍵は `StrictHostKeyChecking=accept-new` で自動的に受け入れます (変わった鍵は拒否します)。検証ネットワーク以外では、事前に `ssh` で一度接続して指紋を確認してください。
- 管理者パスワードはホストに `/etc/wpg/password` (`0640 root:wpg`) として置かれ、wpg が起動時にハッシュ化します。
- 自動実行のパスワードは、環境変数、自分で作るファイル、Windows では DPAPI で暗号化したファイルから読みます。スクリプトがパスワードを平文で保存することはありません ([自動実行](#自動実行-無人実行))。
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
| `/usr/local/share/wpg/deploy.stamp` | 最後にデプロイしたソースと設定の記録 (`--skip-if-current` 用。パスワードは含まない) | する |
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

`deploy.sh` は、Ubuntu 24.04 コンテナ (sshd、systemd の代わりに簡易な `systemctl`) に対して、ウィザード (パスワードでの鍵登録: `sshpass` と `SSH_ASKPASS` の両方、sudo パスワード、インタフェース選択、回答の保存と再利用、ロールバックのメニュー)、ホスト上でのビルド (このチェックアウトのアップロード、rustup と trunk の導入からヘルスチェックまで)、新しい `wpg.toml` への `[terminal]` / `[snmp]` の追記 (対応するバイナリを模したスタブ) と既存の `wpg.toml` を変更しないことを確認しました。従来の、systemd が動くコンテナでのインストール、パスワード無しでの拒否、`--service` とヘルスチェック、`--rollback`、`--dpdk` の確認結果はそのままです。自動実行は、同じ Ubuntu 24.04 コンテナ (2 つの sshd、それぞれ別のマウント名前空間で `/usr/local`、`/etc/wpg`、`/var/lib/wpg` を分離) に対して `deploy.sh` で確認しました: 回答ファイルと `--yes` での初回インストール (環境変数の SSH パスワードで鍵を登録、sudo と管理者パスワードは環境変数、sudo はパスワードファイルからも)、パスワードが足りないときに質問せずすぐ失敗すること (SSH 鍵もパスワードも無い: 2、sudo のパスワードが無い / 違う: 2、初回で管理者パスワードが無い: 1、端末でない標準入力で `--yes` 無し: 1)、`--skip-if-current` (変化なしで sudo パスワード無しでもスキップ、バイナリや設定の変更、ロールバックの後は再デプロイ)、`--dry-run`、`--check-config`、`--json` の出力 (`python3 -m json.tool` で検証)、複数台 (セクション 3 台のうち 1 台が接続できない: 表と JSON にホストごとの結果、終了コード 6。`--parallel 2`。`--targets`)、ウィザードの最後の自動実行ファイルの保存と、そのファイルでの再実行。**`deploy.ps1` / `deploy.bat` は Windows で実行していません** (CI でパースと PowerShell 5.1 互換の構文チェックのみ。自動実行の部分も Windows では未確認です)。CML2 のノードや実機でもまだ確認していません。
