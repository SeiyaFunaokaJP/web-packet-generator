# リリース手順

## バージョン

ワークスペースは単一バージョンです (`Cargo.toml` の `[workspace.package] version`。各クレートは `version.workspace = true`)。[セマンティックバージョニング](https://semver.org/lang/ja/) に従い、v1.0 までは破壊的変更があり得ます。

## リリース判定 (ゲート)

タグを打つ前に、次がすべて緑であることを確認します。普段の「プッシュ前のチェック」(CLAUDE.md) に加えて、プロトコルとセキュリティ、ドキュメントのゲートがあります。

```sh
# プッシュ前のチェック
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown

# プロトコル (golden pcap と tshark 突合)
cargo test -p wpg-golden-tests
WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored

# セキュリティ (just audit / just deny)
cargo audit
cargo deny check
```

[ROADMAP.ja.md](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/ROADMAP.ja.md) の v1.0 リリース判定 (M12) では、これに加えて次を満たします。

- P0 + P1 プロトコルの golden pcap と tshark 突合がすべて緑。
- 3 OS でビルド・起動・pcap バックエンド送出が確認できている。
- GUI の E2E (`just e2e`) が緑、i18n の全言語の抜け漏れが 0 (`cargo test -p wpg-i18n`)。
- UI を変えたときは `docs/{ja,en}/src/images/` のスクリーンショットとビルダーの GIF を `just screenshots` で再生成する (コミット前に目で確認)。
- ドキュメントが ja / en そろっており、`docs/build.sh` が通る。
- `CHANGELOG.md` が更新されている。

## 手順

1. ゲートをすべて通す (上記)。
2. `CHANGELOG.md` の **Unreleased** の項目を新しいバージョンの節 (`## [X.Y.Z] - YYYY-MM-DD`) に移し、前回のタグ以降の変更を Conventional Commits から補う。アップグレード時の注意 (テンプレートスキーマ、データベース、設定の変更。[アップグレード](../operations/upgrade.md) を参照) も書く。
3. `Cargo.toml` のワークスペースバージョン (と `[workspace.dependencies]` の内部 `wpg-*` の `version`) を上げ、`Cargo.lock` を更新する (`cargo check`)。
4. `release: vX.Y.Z` でコミットし、注釈付きタグ `vX.Y.Z` を打つ (`git tag -a vX.Y.Z -m "wpg X.Y.Z"`)。
5. タグをプッシュする (`git push origin vX.Y.Z`)。`.github/workflows/release.yml` がアーカイブをビルドし、`SHA256SUMS` を書き、自動生成のノート付きで GitHub Release を作成するので、その後ノートを編集する。ハイフンを含むタグ (`v1.0.0-rc.1`) はプレリリースになる。タグがワークスペースのバージョンと一致しないと `scripts/release.sh` が失敗する。

ワークフローを手動実行 (`workflow_dispatch`) したとき、またはパッケージングに関わるファイル (`scripts/release.sh`、`scripts/smoke-archive.sh`、`scripts/build.ps1`、`packaging/`、Docker のファイル、ワークフロー自身、Npcap の読み込みコード) を変える PR では、何も公開せずにアーカイブをビルド・テストしてワークフローの artifact として残します。パイプラインの確認に使ってください。

## バイナリ配布

リリースは `.github/workflows/release.yml` がビルドします。各ターゲットをそれぞれのランナーで `scripts/release.sh` がビルドして `scripts/smoke-archive.sh` で確認し、さらに各 OS でアーカイブを試すジョブ (下記) が走ります。ほかのジョブがすべて通ると、最後のジョブがすべてのアーカイブと `SHA256SUMS` ファイルを付けた GitHub Release を公開します。リリースビルドの trunk はワークフローでバージョンを固定しているので、同じタグを再実行しても同じ Web UI がビルドされます。

### ターゲット

| アーカイブ | ランナー | feature | 補足 |
|---|---|---|---|
| `wpg-<ver>-x86_64-unknown-linux-gnu.tar.gz` | `ubuntu-22.04` | (なし) | glibc 2.35 でリンクするので、Ubuntu 22.04 / Debian 12 以降で動きます。`afpacket` で送信するので libpcap は不要です |
| `wpg-<ver>-aarch64-apple-darwin.tar.gz` | `macos-14` | `pcap` | Apple シリコン。libpcap は macOS に標準で入っています |
| `wpg-<ver>-x86_64-apple-darwin.tar.gz` | `macos-14` (クロス) | `pcap` | Intel Mac |
| `wpg-<ver>-x86_64-pc-windows-msvc.zip` | `windows-latest` | `pcap` | Npcap が無くても起動します。送信には Npcap が必要です |

各アーカイブには、`wpg` / `wpg.exe` (Web UI を組み込み済み)、`README.md`、`README.ja.md`、`CHANGELOG.md`、2 つのライセンス、`templates/`、`packaging/` (systemd、launchd、Windows サービスのサンプル) が入ります。macOS と Windows は libpcap / Npcap でしか送信できないため、この 2 つは `pcap` feature 付きでビルドします。Windows では `release.sh` が `wpcap.lib` をリンクするために Npcap SDK (バージョンと SHA-256 はスクリプトに固定、`target/` にキャッシュ) をダウンロードします。SDK は同梱しません。`wpg.exe` は `wpcap.dll` を遅延ロードする (`crates/wpg-cli/build.rs`) ので、Npcap の無いマシンでも起動し、`wpg doctor` を実行できます。**署名と公証はしていません**。利用者が行う手順は macOS と Windows のインストールページを参照してください。

### CI で確認していること

| ジョブ | 確認内容 |
|---|---|
| `build (<target>)` | `scripts/smoke-archive.sh`: `.sha256` ファイル、アーカイブの中身、`wpg --version`、`wpg doctor --json`、`wpg render`、`wpg serve` が `/api/health` に応答し組み込みの UI を返すこと (クロスビルドの Intel macOS 版は中身のみ) |
| `macOS (launchd, pcap)` | Apple シリコン版のアーカイブと launchd デーモンを [サービスとして動かす](../operations/services.md) のとおりに入れて `/api/health` を確認し、削除する。続けて `feth` ペアの片側で `--backend pcap` により 16 フレーム送信し、もう片側で `wpg capture --backend pcap` により受信する |
| `Windows (scheduled task, no Npcap)` | `wpg doctor` が `npcap` チェックを失敗にすること、`wpg send --backend pcap` がクラッシュせずに「Npcap is not installed」で止まること、`packaging\windows\wpg-service.ps1` でタスクを登録・起動 (`/api/health` で確認)・停止・削除できること、`C:\ProgramData\wpg` と `wpg.toml` を `BUILTIN\Users` が読めないこと |
| `Windows (build.ps1)` | Npcap SDK を `C:\Npcap-SDK` に置いて (`pcap` が自動で付く) `scripts\build.ps1` を実行し、できたバイナリを動かす |
| `Docker (compose)` | `docker compose up --build --wait`、コンテナの `/api/health` と `wpg doctor` |

ランナーには Npcap を無人インストールできない (サイレントインストールは OEM 版のみ) ため、Npcap 経由の送信は CI では確認できません。リリース前に Windows 実機で手動確認してください。

### ローカルでアーカイブを作る

```sh
scripts/release.sh                                    # ホストのターゲット、dist/ に出力
scripts/release.sh --target x86_64-apple-darwin       # クロスターゲット (そのリンカ / SDK が必要)
scripts/release.sh --skip-frontend --features pcap    # crates/wpg-web/dist を再利用
scripts/smoke-archive.sh dist/wpg-*.tar.gz            # または: just smoke-archive <archive>
scripts/release.sh --checksums dist                   # dist/ のすべての SHA256SUMS
```

`--skip-frontend` を付けない限り `trunk` (`cargo install trunk --locked`) が必要です。`crates/wpg-web/dist/index.html` が無いとパッケージ化を拒否するので、Web UI の無いリリースが出ることはありません。Windows では Git Bash から実行します (`7z` か `zip` を使います)。

### cargo-dist を使わない理由

ロードマップでは cargo-dist を予定していました。小さなスクリプトにしたのは、wpg では `cargo build` の前に Trunk でフロントエンドをビルドする必要があり (UI は `rust-embed` で組み込まれる)、アーカイブにはバイナリの横にテンプレートとサービスファイルを入れるためです。cargo-dist でも独自のビルド前ステップと `include` で両方できますが、生成されたワークフローは cargo-dist を更新するたびに再生成が必要です。`release.sh` はローカルでも CI でも同じスクリプトです。インストーラ (shell / PowerShell / Homebrew / MSI) は作りません。必要になったら cargo-dist を再検討してください。

### 署名

まだ行っていません。macOS の利用者は quarantine 属性を外す必要があります ([macOS へのインストール](../getting-started/install-macos.md))。Windows では SmartScreen が不明な発行元として警告することがあります ([Windows へのインストール](../getting-started/install-windows.md))。署名と公証には Apple Developer ID とコード署名証明書が必要ですが、プロジェクトはまだ持っていません。
