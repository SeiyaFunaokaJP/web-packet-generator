# Adding i18n strings

User-facing text — in the server, the CLI and the web UI, error messages included — is never hard-coded; it is a Fluent key. Strings are handled by the `wpg-i18n` crate, which embeds `locales/ja/main.ftl` and `locales/en/main.ftl` with `include_str!` (no file I/O, so it builds for the browser).

## Add a key

1. Add one line with the **same key** to **both** `locales/en/main.ftl` and `locales/ja/main.ftl`. English is the base and the fallback.

   ```ftl
   # locales/en/main.ftl
   send-done = Sent { $packets } packets in { $elapsed }

   # locales/ja/main.ftl
   send-done = { $packets } パケットを { $elapsed } で送信しました
   ```

2. Reference it from code: `t` when there are no arguments, `t_args` when there are.

   ```rust
   use wpg_i18n::{t, t_args, FluentArgs, Locale};

   let s = t(locale, "error-not-found");

   let mut args = FluentArgs::new();
   args.set("packets", 1022);
   args.set("elapsed", "0.5s");
   let s = t_args(locale, "send-done", &args);
   ```

A missing key falls back to English, then to the key name itself. The `{ $name }` placeholder names are a contract with the caller — do not rename them in a translation.

## Keep the key sets equal

`locales/ja` and `locales/en` must hold the **same set of keys**. A `wpg-i18n` unit test checks this and fails if a key exists in only one locale.

```sh
cargo test -p wpg-i18n
```

This is the "i18n parity lint" of TODO.md §5.

## Protocol names and field descriptions

The `desc = "key"` attached to a field in a protocol definition (`wpg-protocols`) is a Fluent key. When you add a protocol, add its `desc` keys to both `.ftl` files (see "Adding a protocol" in CONTRIBUTING.md and [Adding a protocol](add-protocol.md)).

```rust
// crates/wpg-protocols/src/app.rs
op: enum(8) { 1 => boot_request, 2 => boot_reply } = 1, desc = "dhcp-op";
```

```ftl
# locales/{ja,en}/main.ftl
dhcp-op = ...
```

Diagnostic (`diag-*`) and lint (`lint-*`) messages work the same way. Their argument names (`{ $field }` and so on) are a contract with `wpg-core`'s `diag::keys`.

## Add a language

1. Create `locales/<lang>/main.ftl` and translate every key.
2. Add a variant to `wpg-i18n`'s `Locale` with its `source()` and language-tag mapping (see the doc comment in `crates/wpg-i18n/src/lib.rs`).
3. Run `cargo test -p wpg-i18n` to confirm no key is missing.
