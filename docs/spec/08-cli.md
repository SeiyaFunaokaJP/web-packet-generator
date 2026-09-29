# 08. CLI (wpg-cli)

Status: draft / 元: TODO.md §2.7

サーバと同一バイナリ `wpg` のサブコマンドとして提供する。CI や ssh 先でのヘッドレス実行に利用する。

## コマンド一覧

```
wpg serve   [--bind 0.0.0.0:8080] [--password-file] [--tls] [--config wpg.toml]
wpg send    <template.yaml> -i eth0 [--backend afpacket] [--count N | --duration 10s] [--rate 1Mpps] [--var src_ip=...]
wpg render  <template.yaml> -o out.pcapng [--count N]      # 送出なし
wpg import  <in.pcap> -o template.yaml
wpg template lint|list|export|import
wpg ifaces  [--json]
wpg doctor                                                 # 権限・ドライバ・hugepages・Npcap 診断
wpg bench   [--backend ...]                                # 送出性能計測
```

M3 で `wpg config init` (設定ファイル雛形出力) を追加する。

## コマンド別概要

| コマンド | 内容 | マイルストーン |
|---|---|---|
| `serve` | HTTP/WS サーバ起動 | M3 |
| `send` | ヘッドレス送出。`--var` でテンプレ変数を上書き | M3 (バックエンドは M2/M8) |
| `render` | 送出せず pcapng に書き出す (`pcapfile` バックエンド) | M3 |
| `import` | pcap/pcapng からテンプレート生成 | M5 |
| `template` | lint / list / export / import | M3, M5 |
| `ifaces` | インタフェース列挙 (`--json`) | M2 |
| `doctor` | 診断。出力とトラブルシュートの対応表を docs に置く | M2 |
| `bench` | 性能計測 (CI nightly で使用) | M2, M8 |

## 終了コード (Proposal)

0 成功 / 1 一般エラー / 2 引数エラー / 3 テンプレ検証エラー / 4 権限・環境エラー (`doctor` 系)。`--json` 出力時は機械可読。

## 未決事項 (Open questions)

- `--rate` の単位表記 (`1Mpps`, `10Gbps`, `50%`) の文法。
- `--var` を複数回指定する場合と、変数ファイル読み込みの要否。
- 終了コードの確定と、`doctor` の JSON スキーマ。
- 進捗表示 (TTY 時のプログレス、非 TTY 時のログ) の仕様。
- CLI のシェル補完・man ページ生成の要否。
- `wpg send` がサーバ経由のジョブとして実行できるか (リモート API 呼び出しモード)。
