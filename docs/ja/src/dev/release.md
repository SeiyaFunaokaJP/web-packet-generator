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
- ドキュメントが ja / en そろっており、`docs/build.sh` が通る。
- `CHANGELOG` が更新されている。

## 手順

1. ゲートをすべて通す (上記)。
2. `CHANGELOG` に、前回のタグ以降の変更を Conventional Commits から整理してまとめる。
3. `Cargo.toml` のワークスペースバージョンを上げ、`Cargo.lock` を更新する。
4. `release: vX.Y.Z` でコミットし、注釈付きタグ `vX.Y.Z` を打つ。
5. タグをプッシュする。

## バイナリ配布 (予定)

v1.0 では GitHub Actions の matrix で Windows / macOS / Linux のバイナリとチェックサムを自動生成する予定です (cargo-dist 想定、TODO.md M10 / M12)。それまでは [Getting Started](../getting-started/install-linux.md) の手順でソースからビルドします。
