# リリース手順

Status: draft

リリースは `.github/workflows/release.yml` がビルドします。各ターゲットをそれぞれのランナーで `scripts/release.sh` がビルドし、最後のジョブがすべてのアーカイブと `SHA256SUMS` ファイルを付けた GitHub Release を公開します。

## ターゲット

| アーカイブ | ランナー | 補足 |
|---|---|---|
| `wpg-<ver>-x86_64-unknown-linux-gnu.tar.gz` | `ubuntu-22.04` | glibc 2.35 でリンクするので、Ubuntu 22.04 / Debian 12 以降で動きます |
| `wpg-<ver>-aarch64-apple-darwin.tar.gz` | `macos-14` | Apple シリコン |
| `wpg-<ver>-x86_64-apple-darwin.tar.gz` | `macos-14` (クロス) | Intel Mac |
| `wpg-<ver>-x86_64-pc-windows-msvc.zip` | `windows-latest` | 実行時に Npcap が必要 |

各アーカイブには、`wpg` / `wpg.exe` (Web UI を組み込み済み)、`README.md`、`README.ja.md`、2 つのライセンス、`templates/`、`packaging/` (systemd、launchd、Windows サービスのサンプル) が入ります。バイナリは任意の feature 無し (`pcap` 無し) でビルドするので、起動に追加のライブラリは不要です。**署名と公証はしていません**。利用者が行う手順は macOS と Windows のインストールページを参照してください。

## リリースの作り方

1. `main` が緑であることを確認します。`cargo audit`、`cargo deny`、golden / tshark の確認、i18n lint も含みます。v1.0 の前には CHANGELOG と日英のドキュメントも完備している必要があります。
2. ルートの `Cargo.toml` の `[workspace.package]` の `version` (と `[workspace.dependencies]` の内部 `wpg-*` の version) を上げ、`cargo check` で `Cargo.lock` を更新して、その変更をマージします。
3. マージコミットにタグを付けてプッシュします。

   ```sh
   git tag -a v0.2.0 -m "wpg 0.2.0"
   git push origin v0.2.0
   ```

4. ワークフローが 4 つのアーカイブをビルドし、`SHA256SUMS` を書き、自動生成のノート付きでリリースを作成します。ハイフンを含むタグ (`v1.0.0-rc.1`) はプレリリースになります。タグがワークスペースのバージョンと一致しないと `release.sh` が失敗します。
5. リリースノートを編集し、アップグレード時の注意 (テンプレートスキーマ、データベース、設定の変更。[アップグレード](../operations/upgrade.md) を参照) を書きます。

ワークフローを手動実行 (`workflow_dispatch`) すると、何も公開せずにアーカイブをビルドしてワークフローの artifact として残します。パイプラインの確認に使ってください。

## ローカルでアーカイブを作る

```sh
scripts/release.sh                                    # ホストのターゲット、dist/ に出力
scripts/release.sh --target x86_64-apple-darwin       # クロスターゲット (そのリンカ / SDK が必要)
scripts/release.sh --skip-frontend --features pcap    # crates/wpg-web/dist を再利用
scripts/release.sh --checksums dist                   # dist/ のすべての SHA256SUMS
```

`--skip-frontend` を付けない限り `trunk` (`cargo install trunk --locked`) が必要です。`crates/wpg-web/dist/index.html` が無いとパッケージ化を拒否するので、Web UI の無いリリースが出ることはありません。Windows では Git Bash から実行します (`7z` か `zip` を使います)。

## cargo-dist を使わない理由

ロードマップでは cargo-dist を予定していました。小さなスクリプトにしたのは、wpg では `cargo build` の前に Trunk でフロントエンドをビルドする必要があり (UI は `rust-embed` で組み込まれる)、アーカイブにはバイナリの横にテンプレートとサービスファイルを入れるためです。cargo-dist でも独自のビルド前ステップと `include` で両方できますが、生成されたワークフローは cargo-dist を更新するたびに再生成が必要です。`release.sh` はローカルでも CI でも同じスクリプトです。インストーラ (shell / PowerShell / Homebrew / MSI) は作りません。必要になったら cargo-dist を再検討してください。

## 署名

まだ行っていません。macOS の利用者は quarantine 属性を外す必要があります ([macOS へのインストール](../getting-started/install-macos.md))。Windows では SmartScreen が不明な発行元として警告することがあります ([Windows へのインストール](../getting-started/install-windows.md))。署名と公証には Apple Developer ID とコード署名証明書が必要ですが、プロジェクトはまだ持っていません。
