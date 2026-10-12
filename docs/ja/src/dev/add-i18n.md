# i18n 文字列の追加手順

ユーザーに見える文字列は、サーバ・CLI・Web UI (エラーメッセージを含む) のいずれでも直書きせず、Fluent のキーにします。文字列は `wpg-i18n` クレートが扱い、`locales/ja/main.ftl` と `locales/en/main.ftl` の 2 つの Fluent ファイルから `include_str!` で埋め込まれます (ブラウザでも動くようファイル I/O はありません)。

## キーを追加する

1. `locales/en/main.ftl` と `locales/ja/main.ftl` の **両方** に、同じキーで 1 行ずつ足します。英語が基準かつフォールバックです。

   ```ftl
   # locales/en/main.ftl
   send-done = Sent { $packets } packets in { $elapsed }

   # locales/ja/main.ftl
   send-done = { $packets } パケットを { $elapsed } で送信しました
   ```

2. コードから参照します。引数が無ければ `t`、あれば `t_args` を使います。

   ```rust
   use wpg_i18n::{t, t_args, FluentArgs, Locale};

   let s = t(locale, "error-not-found");

   let mut args = FluentArgs::new();
   args.set("packets", 1022);
   args.set("elapsed", "0.5s");
   let s = t_args(locale, "send-done", &args);
   ```

未定義のキーは、英語へ、最後はキー名そのものへフォールバックします。`{ $name }` のプレースホルダ名は呼び出し側との契約なので、翻訳で変えないでください。

## キーの集合を揃える

`locales/ja` と `locales/en` は **同じキー集合** でなければなりません。`wpg-i18n` のユニットテストがこれを検査し、片方にしか無いキーがあると失敗します。

```sh
cargo test -p wpg-i18n
```

これが TODO.md §5 の「i18n 抜け漏れ 0 (lint)」にあたります。

## プロトコル名・フィールド説明

プロトコル定義 (`wpg-protocols`) のフィールドに付ける `desc = "key"` は Fluent キーです。プロトコルを足したら、その `desc` キーを両方の `.ftl` に追加します (CONTRIBUTING.ja.md の「プロトコルの追加」、[プロトコルの追加手順](add-protocol.md))。

```rust
// crates/wpg-protocols/src/app.rs
op: enum(8) { 1 => boot_request, 2 => boot_reply } = 1, desc = "dhcp-op";
```

```ftl
# locales/{ja,en}/main.ftl
dhcp-op = ...
```

診断 (`diag-*`)・lint (`lint-*`) のメッセージも同じ仕組みです。引数名 (`{ $field }` など) は `wpg-core` の `diag::keys` との契約です。

## 言語を追加する

1. `locales/<lang>/main.ftl` を作り、全キーを翻訳します。
2. `wpg-i18n` の `Locale` に変種を追加し、`source()` と言語タグの対応を足します (`crates/wpg-i18n/src/lib.rs` の doc コメント参照)。
3. `cargo test -p wpg-i18n` でキーの抜けが無いことを確認します。
