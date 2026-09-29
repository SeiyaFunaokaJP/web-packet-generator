# ADR-0002: フロントエンド: Leptos (CSR) + Tailwind CSS

> Frontend: Leptos (CSR) + Tailwind CSS, compiled to WASM.

- Status: Accepted
- Date: 2026-09-29

## Context

要件は「Rust 一択」(バックエンドからフロントエンド WASM まで) である。ビルダー GUI はブラウザ内で `wpg-core` を使いサーバ往復なしでプレビュー合成を行い、サーバ側結果と一致させる必要がある。Hex ダンプの同期ハイライトなど描画負荷の高い UI も含む。

## Decision

**Leptos (CSR) + Tailwind CSS** を採用し、WASM としてビルドする。`wpg-core` を同一コードで WASM ビルドして共有する。将来 SSR/ハイドレーションへ移行可能な構成 (状態管理・ルーティング) を維持する。D&D は HTML5 DnD API の薄い自作ラッパーとし、Hex 表示は仮想スクロールの自作コンポーネントとする。

## Alternatives considered

- Yew: 歴史が長く実績があるが、fine-grained reactivity を持たず、大量要素の更新が重くなりやすい。
- Dioxus: クロスプラットフォーム志向で魅力的だが、Web 向けのエコシステムと SSR の成熟度で Leptos を優先した。
- TypeScript + React (or Svelte): エコシステムは最大だが Rust 一貫の要件に反し、`wpg-core` の共有にも WASM 境界が必要になる。

## Consequences

- Rust の型と `wpg-core` をフロントと共有でき、プレビューとサーバの一致を保証しやすい。
- Hex 9000B の描画性能リスクがあり、仮想スクロールと canvas フォールバックで対処する (リスク表参照)。
- フロントのビルドに `trunk` と wasm ターゲットが必要になり、ビルドスクリプトが複雑化する。
- UI コンポーネントは自作が多くなる。
