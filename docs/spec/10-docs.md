# 10. ドキュメント方針

Status: draft / 元: TODO.md §2.9

mdBook を使い、日本語と英語を別ブックで提供する (`docs/ja/`, `docs/en/`。ビルドは `bash docs/build.sh`)。

## 構成

| 章 | 内容 |
|---|---|
| Getting Started | OS 別インストール、権限設定、初回ログイン |
| チュートリアル (5 本程度) | ARP スイープ、VLAN タグ付き TCP SYN 大量、IS-IS IIH 生成、DHCP 負荷、pcap → テンプレ再生 |
| リファレンス | プロトコル一覧 (定義 IR から自動生成)、テンプレスキーマ、API (OpenAPI)、CLI、設定 |
| 運用 | DPDK / AF_XDP セットアップ、CML2 での注意点 (virtio, promiscuous, MAC 制限)、性能チューニング |
| 開発者ガイド | crate 構成、プロトコル追加手順、i18n 追加手順、リリース手順 |
| FAQ / トラブルシュート | `wpg doctor` 出力との対応表 |
| 法的注意 | 検証環境以外での使用禁止の明記、ライセンス (MIT / Apache-2.0 デュアル) |

## 運用ルール (Proposal)

- ja / en のツリー構造を一致させる。各ページ冒頭に `Status:` 行を置く。
- 自動生成部分と手書き補足を分け、生成物は手で編集しない。
- 仕様の詳細は `docs/spec/`、決定の経緯は `docs/adr/` に置き、ブックからリンクする。

## 未決事項 (Open questions)

- 3 つ目以降の言語追加時に mdBook を言語ごとに増やす運用の負担 (翻訳管理ツールの要否)。
- プロトコルリファレンスの自動生成の出力形式 (Markdown 生成を `build.rs` か xtask のどちらで行うか)。
- OpenAPI リファレンスをブックに埋め込む方法 (静的 HTML 化 or Swagger UI へのリンク)。
- スクリーンショット・GIF の更新方法と保管場所。
- ドキュメントの公開先 (GitHub Pages 等)。
