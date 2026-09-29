# wpg docs

Documentation is split into three parts.

| Path | Contents |
|---|---|
| `docs/en/` | mdBook (English user/developer docs) |
| `docs/ja/` | mdBook (Japanese user/developer docs) |
| `docs/spec/` | Detailed specification split out of `TODO.md` (Japanese primary) |
| `docs/adr/` | Architecture Decision Records (MADR style, Japanese) |

## Building the books

Two independent mdBook projects are used (no plugin needed); language switching is done by a landing page.

```sh
mdbook build docs/en      # -> docs/book/en
mdbook build docs/ja      # -> docs/book/ja
bash docs/build.sh        # builds both and writes docs/book/index.html
```

`docs/book/index.html` redirects by `navigator.language` (`ja*` goes to `ja/`, anything else to `en/`) and shows visible links to both. Install mdBook with `cargo install mdbook --locked`.

Each page carries a `Status: draft` line until reviewed. Keep the `en` and `ja` trees structurally identical.
