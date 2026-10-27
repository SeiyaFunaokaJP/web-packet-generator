# wpg docs

Documentation is split into three parts.

| Path | Contents |
|---|---|
| `docs/en/` | mdBook (English user/developer docs) |
| `docs/ja/` | mdBook (Japanese user/developer docs) |
| `docs/spec/` | Detailed specification (Japanese primary; milestones in `ROADMAP.md`) |
| `docs/adr/` | Architecture Decision Records (MADR style, Japanese) |

## Building the books

Two independent mdBook projects are used (no plugin needed). The books have the same page tree, and `docs/lang-switch.js` (loaded by both through `additional-js`) adds an English / 日本語 switcher to the menu bar that opens the same page in the other language.

```sh
mdbook build docs/en      # -> docs/book/en
mdbook build docs/ja      # -> docs/book/ja
bash docs/build.sh        # builds both and writes docs/book/index.html
```

`docs/book/index.html` redirects to the language last picked with the switcher, else by `navigator.language` (`ja*` goes to `ja/`, anything else to `en/`), and shows visible links to both. Keep the two `src/` trees file-for-file identical so the switcher always finds the page. Install mdBook with `cargo install mdbook --locked`. The `Docs` workflow (`.github/workflows/pages.yml`) runs the same script on every push to `main` that touches `docs/` and deploys `docs/book/` to GitHub Pages (the Pages source must be "GitHub Actions").

Each page carries a `Status: draft` line until reviewed. Keep the `en` and `ja` trees structurally identical.
