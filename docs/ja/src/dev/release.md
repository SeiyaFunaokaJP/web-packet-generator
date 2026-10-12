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

TODO.md §4 の v1.0 リリース判定 (M12) では、これに加えて次を満たします。

- P0 + P1 プロトコルの golden pcap と tshark 突合がすべて緑。
- 3 OS でビルド・起動・pcap バックエンド送出が確認できている。
- GUI の E2E (`just e2e`) が緑、i18n の ja / en 抜け漏れが 0 (`cargo test -p wpg-i18n`)。
- UI を変えたときは `docs/{ja,en}/src/images/` のスクリーンショットとビルダーの GIF を `just screenshots` で再生成する (コミット前に目で確認)。
- ドキュメントが ja / en そろっており、`docs/build.sh` が通る。
- `CHANGELOG` が更新されている。

## 手順

1. ゲートをすべて通す (上記)。
2. `CHANGELOG` に、前回のタグ以降の変更を Conventional Commits から整理してまとめる。アップグレード時の注意 (テンプレートスキーマ、データベース、設定の変更。[アップグレード](../operations/upgrade.md) を参照) も書く。
3. `Cargo.toml` のワークスペースバージョン (と `[workspace.dependencies]` の内部 `wpg-*` の `version`) を上げ、`Cargo.lock` を更新する (`cargo check`)。
4. `release: vX.Y.Z` でコミットし、注釈付きタグ `vX.Y.Z` を打つ (`git tag -a vX.Y.Z -m "wpg X.Y.Z"`)。
5. タグをプッシュする (`git push origin vX.Y.Z`)。`.github/workflows/release.yml` がアーカイブをビルドし、`SHA256SUMS` を書き、自動生成のノート付きで GitHub Release を作成するので、その後ノートを編集する。ハイフンを含むタグ (`v1.0.0-rc.1`) はプレリリースになる。タグがワークスペースのバージョンと一致しないと `scripts/release.sh` が失敗する。

ワークフローを手動実行 (`workflow_dispatch`) すると、何も公開せずにアーカイブをビルドしてワークフローの artifact として残します。パイプラインの確認に使ってください。

## バイナリ配布

リリースは `.github/workflows/release.yml` がビルドします。各ターゲットをそれぞれのランナーで `scripts/release.sh` がビルドし、最後のジョブがすべてのアーカイブと `SHA256SUMS` ファイルを付けた GitHub Release を公開します。

### ターゲット

| アーカイブ | ランナー | 補足 |
|---|---|---|
| `wpg-<ver>-x86_64-unknown-linux-gnu.tar.gz` | `ubuntu-22.04` | glibc 2.35 でリンクするので、Ubuntu 22.04 / Debian 12 以降で動きます |
| `wpg-<ver>-aarch64-apple-darwin.tar.gz` | `macos-14` | Apple シリコン |
| `wpg-<ver>-x86_64-apple-darwin.tar.gz` | `macos-14` (クロス) | Intel Mac |
| `wpg-<ver>-x86_64-pc-windows-msvc.zip` | `windows-latest` | 実行時に Npcap が必要 |

各アーカイブには、`wpg` / `wpg.exe` (Web UI を組み込み済み)、`README.md`、`README.ja.md`、2 つのライセンス、`templates/`、`packaging/` (systemd、launchd、Windows サービスのサンプル) が入ります。バイナリは任意の feature 無し (`pcap` 無し) でビルドするので、起動に追加のライブラリは不要です。**署名と公証はしていません**。利用者が行う手順は macOS と Windows のインストールページを参照してください。

### ローカルでアーカイブを作る

```sh
scripts/release.sh                                    # ホストのターゲット、dist/ に出力
scripts/release.sh --target x86_64-apple-darwin       # クロスターゲット (そのリンカ / SDK が必要)
scripts/release.sh --skip-frontend --features pcap    # crates/wpg-web/dist を再利用
scripts/release.sh --checksums dist                   # dist/ のすべての SHA256SUMS
```

`--skip-frontend` を付けない限り `trunk` (`cargo install trunk --locked`) が必要です。`crates/wpg-web/dist/index.html` が無いとパッケージ化を拒否するので、Web UI の無いリリースが出ることはありません。Windows では Git Bash から実行します (`7z` か `zip` を使います)。

### cargo-dist を使わない理由

ロードマップでは cargo-dist を予定していました。小さなスクリプトにしたのは、wpg では `cargo build` の前に Trunk でフロントエンドをビルドする必要があり (UI は `rust-embed` で組み込まれる)、アーカイブにはバイナリの横にテンプレートとサービスファイルを入れるためです。cargo-dist でも独自のビルド前ステップと `include` で両方できますが、生成されたワークフローは cargo-dist を更新するたびに再生成が必要です。`release.sh` はローカルでも CI でも同じスクリプトです。インストーラ (shell / PowerShell / Homebrew / MSI) は作りません。必要になったら cargo-dist を再検討してください。

### 署名

まだ行っていません。macOS の利用者は quarantine 属性を外す必要があります ([macOS へのインストール](../getting-started/install-macos.md))。Windows では SmartScreen が不明な発行元として警告することがあります ([Windows へのインストール](../getting-started/install-windows.md))。署名と公証には Apple Developer ID とコード署名証明書が必要ですが、プロジェクトはまだ持っていません。
