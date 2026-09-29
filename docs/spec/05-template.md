# 05. テンプレート (wpg-template)

Status: draft / 元: TODO.md §2.4

## 概要

- 形式: YAML (人が読める、Git で diff できる)。JSON も同等に読み書き可。拡張子は `.wpg.yaml` ([ADR-0008](../adr/0008-naming.md))。
- スキーマバージョニング: `schema: 1`。後方互換のマイグレーションを提供する。
- 1 テンプレート = 1 ストリーム (展開結果 + 送出プロファイル)。

## トップレベル構造

| キー | 必須 | 内容 |
|---|---|---|
| `schema` | 必須 | スキーマバージョン (整数) |
| `meta` | 必須 | 名前 / 説明 / タグ / 作者 / 作成日 / 多言語説明 |
| `layers` | 必須 | レイヤ配列 (プロトコル + フィールド + ジェネレータ)。外側から内側の順 |
| `tx` | 任意 | 送出プロファイル (数量・レート・バースト等) |
| `variables` | 任意 | テンプレ引数化 (実行時オーバーライド可) |

## 例: Ethernet / IPv4 / UDP (src IP に range ジェネレータ)

これは設計検討用のドラフト例であり、キー名は M5 で確定する。

```yaml
schema: 1

meta:
  name: udp-src-sweep
  description: 送信元 IP を 192.168.1.1-192.168.5.254 で変化させる UDP パケット
  description_i18n:
    en: UDP packets whose source IP sweeps 192.168.1.1-192.168.5.254
  tags: [udp, sample, l3]
  author: example
  created: 2026-09-29

variables:
  dst_ip:
    type: ipv4
    default: 10.0.0.1
    description: 宛先 IP (実行時に --var dst_ip=... で上書き)
  dst_port:
    type: uint16
    default: 5001

layers:
  - protocol: ethernet
    fields:
      dst: "ff:ff:ff:ff:ff:ff"
      src: "00:11:22:33:44:55"
      ethertype: auto            # 下位レイヤ (IPv4) から自動決定

  - protocol: ipv4
    fields:
      src:
        generator:
          type: range
          start: 192.168.1.1
          end: 192.168.5.254
          step: 1
      dst: "${dst_ip}"
      ttl: 64
      dscp: 0
      total_length: auto
      checksum: auto

  - protocol: udp
    fields:
      src_port: 12345
      dst_port: "${dst_port}"
      length: auto
      checksum: auto

  - protocol: payload
    fields:
      data:
        pattern: "00 01 02 03"   # 繰り返しで 64 バイトまで埋める
        length: 64

tx:
  count: 1000000               # または duration: 10s / 省略で無限
  rate:
    pps: 100000                # bps / line_rate_percent も指定可 (排他)
  burst:
    size: 32
    interval: 1ms
  backend: auto                # afpacket / pcap / afxdp / dpdk / pcapfile
```

### 補足

- range は 192.168.1.1 から 192.168.5.254 まで 1 ずつ増加する。範囲の終端に達した後の挙動 (wrap / 停止) は `tx.count` と生成器の `wrap` 指定で決める (Proposal)。
- `auto` は Auto/Manual トグルの Auto に対応。値を書けば Manual として扱う。
- `"${...}"` は `variables` の参照。型は `variables.<name>.type` で検査する。

## 管理機能

フォルダ/タグ、検索、複製、バージョン履歴 (簡易)、お気に入り、最近使った。ファイル実体は YAML でディスクに置き、履歴等のメタは SQLite ([ADR-0003](../adr/0003-database.md))。

## 共有

- 単体エクスポート: `.wpg.yaml`
- 複数まとめ: `.wpgbundle` (zip: templates + README + 添付 pcap)
- URL 共有 (サーバ内リンク)
- Git 連携: テンプレートディレクトリを任意パスに置くだけで読み込む「ライブラリパス」を複数指定可能
- 同梱ライブラリ: プロトコル別サンプル、代表シナリオ (「VLAN 越し ARP スイープ」「IS-IS L2 IIH フラッド」「DHCP Discover 大量」)

## 検証

`wpg template lint <file>` でスキーマ・制約チェック。JSON Schema を公開する (M5)。

## 未決事項 (Open questions)

- `layers` 内の入れ子表現 (トンネルの内側パケット、TLV/list の記述形式)。
- 複数ジェネレータの合成モード (`zip`/`product`/`nested`) をテンプレートのどこに書くか。
- `variables` の型システム (プロトコルのフィールド型と共通化するか) と、実行時オーバーライドの文法。
- `.wpgbundle` 内のディレクトリ構造、マニフェストの有無、添付 pcap の参照方法。
- マイグレーション方式 (読み込み時に変換 / `wpg template migrate` コマンド)。
- 多言語説明 (`description_i18n`) のキー形式。
- Git ライブラリパスの競合 (同名テンプレート) の解決規則と、ホットリロードの検出方法。
