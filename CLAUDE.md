# CLAUDE.md

Claude Code がこのリポジトリで作業するときの指示です。開発の進め方の詳細は [CONTRIBUTING.ja.md](CONTRIBUTING.ja.md)、ロードマップと仕様は [TODO.md](TODO.md) にあります。

## 作業の完了条件: 毎回 PR まで作成する

作業 (TODO の項目、バグ修正、ドキュメント更新など) が終わったら、確認を求めずに次の手順まで行ってください。PR の作成は事前に許可済みです。

1. 下の「プッシュ前のチェック」をすべて通す。
2. Conventional Commits 形式でコミットする (`feat(core): ...`、`fix(protocols): ...`、`docs: ...` など)。
3. 作業ブランチにプッシュする。
4. `main` 向けの PR を作成する。すでにそのブランチの PR が開いていれば、新しく作らずに既存の PR を更新する。
   - タイトル: コミットと同じ Conventional Commits 形式。
   - 本文: 変更内容の要約、対応する TODO.md のマイルストーン / 項目、テスト結果 (実行したコマンドと結果、tshark での突合の有無)、対象外にした事項。
5. 最終報告に PR の URL を書く。

チェックが通らない場合は、PR を作らずに失敗内容を報告してください。

## プッシュ前のチェック

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown
```

プロトコルを変更したときは、tshark が使えれば次も実行してください。

```sh
WPG_TSHARK=1 cargo test -p wpg-protocols
WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored
```

## 守ること

- ユーザーに見える文字列は直書きせず、Fluent キーを `locales/ja/main.ftl` と `locales/en/main.ftl` の両方に追加する。
- プロトコルを追加・変更したら、サンプルテンプレート、golden テスト、ドキュメント (ja / en) も更新する (CONTRIBUTING.ja.md の「プロトコルの追加」)。
- TODO.md のチェックボックスは、項目が完全に終わったときだけ更新する。
- プロトコル、CLI オプション、API、`wpg.toml` のキー、テンプレート形式、同梱テンプレートを変えたら `just docs-gen`（`cargo run -p wpg-cli -- docs gen`）でリファレンス (`docs/*/src/reference/` など) を再生成してコミットする。生成物は手で編集しない。
- golden の `.hex` / `.pcap` は手で編集しない。`just golden-update` で再生成し、差分を確認する。
