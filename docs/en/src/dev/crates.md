# Crate layout

Status: draft

The repository is a Cargo workspace under `crates/`. wpg-core holds the protocol IR, builder, parser and generators, and must also build for wasm32. Other crates are wpg-protocols, wpg-tx, wpg-rx, wpg-template, wpg-server, wpg-cli and wpg-web. Locales, scripts, docs, templates and tests sit beside `crates/`.
