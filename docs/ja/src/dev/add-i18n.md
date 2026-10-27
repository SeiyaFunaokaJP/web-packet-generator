# i18n 文字列の追加手順

ユーザーに見える文字列は、サーバ・CLI・Web UI (エラーメッセージを含む) のいずれでも直書きせず、Fluent のキーにします。文字列は `wpg-i18n` クレートが扱い、`locales/<lang>/main.ftl` の Fluent ファイルから `include_str!` で埋め込まれます (ブラウザでも動くようファイル I/O はありません)。対応言語は日本語 (`ja`)・英語 (`en`)・中国語簡体字 (`zh-CN`)・中国語繁体字 (`zh-TW`)・韓国語 (`ko`)・ドイツ語 (`de`)・フランス語 (`fr`)・スペイン語 (`es`) の 8 つです。

マニュアル (mdBook) と `wpg docs gen` が生成するリファレンス、同梱テンプレートの説明 (`meta.descriptions`) は日本語と英語だけです。テンプレートに表示言語の説明が無いときは英語の説明を表示します。

## キーを追加する

1. `locales/*/main.ftl` の **すべて** (8 言語) に、同じキーで 1 行ずつ足します。英語が基準かつフォールバックです。プレースホルダ (`{ $name }`) とバッククォートで囲んだコード・ID は翻訳せずにそのまま残します。

   ```ftl
   # locales/en/main.ftl
   send-done = Sent { $packets } packets in { $elapsed }

   # locales/ja/main.ftl
   send-done = { $packets } パケットを { $elapsed } で送信しました

   # locales/de/main.ftl (zh-CN / zh-TW / ko / fr / es も同様)
   send-done = { $packets } Pakete in { $elapsed } gesendet
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

すべての言語の `.ftl` は **同じキー集合** でなければなりません。`wpg-i18n` のユニットテストがこれを検査し、どれかの言語にキーが欠けていたり、英語に無いキーがあったりすると失敗します。

```sh
cargo test -p wpg-i18n
```

これが仕様 (`docs/spec/11-risks.md`) の「i18n 抜け漏れ 0 (lint)」にあたります。専用の `wpg` サブコマンドはありません。同じテストで、プロトコルカタログのフィールド説明キーがすべて存在すること、使われていないキーが無いこと (`no_unused_keys`: どのソースからも参照されないキーは削除します。実行時に接頭辞から組み立てる `docgen-*` と `rtcp-*` は対象外) も検査します。CI では `cargo test --workspace` の一部として走ります。GUI の E2E (`just e2e`、[e2e/README.md](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/e2e/README.md)) はブラウザ上での言語切替も確認します。

## プロトコル名・フィールド説明

プロトコル定義 (`wpg-protocols`) のフィールドに付ける `desc = "key"` は Fluent キーです。プロトコルを足したら、その `desc` キーをすべての言語の `.ftl` に追加します (CONTRIBUTING.ja.md の「プロトコルの追加」、[プロトコルの追加手順](add-protocol.md))。

```rust
// crates/wpg-protocols/src/app.rs
op: enum(8) { 1 => boot_request, 2 => boot_reply } = 1, desc = "dhcp-op";
```

```ftl
# locales/*/main.ftl
dhcp-op = ...
```

診断 (`diag-*`)・lint (`lint-*`) のメッセージも同じ仕組みです。引数名 (`{ $field }` など) は `wpg-core` の `diag::keys` との契約です。

## 言語を追加する

1. `locales/<tag>/main.ftl` を作り、全キーを翻訳します (`<tag>` は BCP 47 の言語タグ。例: `pt-BR`)。
2. `wpg-i18n` の `Locale` に変種を追加し、`Locale::ALL`・`as_str()`・`source()`・`from_langid` (`Accept-Language` や `LANG` からの判定) に足します (`crates/wpg-i18n/src/lib.rs` の doc コメント参照)。
3. 言語名のキー `ui-lang-<tag>` (その言語自身での名前。例: `ui-lang-pt-br = Português (Brasil)`) をすべての `.ftl` に足し、`crates/wpg-web/src/i18n.rs` の `locale_label_key` に対応を追加します。ヘッダと設定画面の言語メニューに自動で並びます。
4. `cargo test -p wpg-i18n -p wpg-web` でキーの抜けが無いことを確認します。

言語の判定は、Web UI では保存済みの設定、なければブラウザの `navigator.languages`、API では `Accept-Language`、CLI では `LC_ALL` / `LC_MESSAGES` / `LANG` の順です。中国語は文字体系と地域で振り分け、`zh-Hant`・`zh-TW`・`zh-HK`・`zh-MO` は繁体字、それ以外の `zh` は簡体字になります。対応していない言語は英語になります。
