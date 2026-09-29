# ADR-0007: プロトコル定義方式: Rust マクロ + YAML (同一 IR)

> Protocol definitions: hybrid of Rust macros (core) and YAML (user extensions), both lowering to one IR.

- Status: Accepted
- Date: 2026-09-29

## Context

プロトコル数が多く (P0〜P3)、1 プロトコル追加のコストを一定に保つ必要がある。定義からは合成、解析、GUI、ドキュメントを生成したい。一方、ユーザーはコード変更なしに独自プロトコルを追加したい。

## Decision

**ハイブリッド方式**とする。コアプロトコルは **Rust マクロ**で定義し、ユーザー拡張は `protocols.d/` の **YAML** で定義する。どちらも**同一の IR** にコンパイルし、IR から合成 / 解析 / GUI / ドキュメントを生成する。

## Alternatives considered

- Rust マクロのみ: 型安全で高速だが、ユーザーが再コンパイルなしに拡張できない。
- YAML のみ: 拡張しやすいが、コア部の型検査とコンパイル時最適化が弱く、複雑なプロトコル (チェックサム、TLV) の表現が難しい。
- Kaitai Struct / P4 等の既存記述言語: 解析用途には強いが、合成 (自動計算、malformed) と GUI 生成の要件に合わない。

## Consequences

- コアの型安全性とユーザー拡張性を両立できる。
- IR の設計が全体の要となり、M1 で最優先に固める必要がある。
- マクロ側と YAML 側で記述力に差が出ないよう、テストで同値性を確認する必要がある。
- IR が唯一の情報源となり、ドキュメント自動生成が可能になる。
