# API リファレンス

Status: draft

wpg-server は、テンプレート、プロトコルカタログ、プレビュー合成、ジョブ、インタフェース、統計、認証のための REST (JSON) API を提供します。リアルタイム統計・ログ・進捗は WebSocket で配信されます。OpenAPI スキーマは utoipa で自動生成され、Swagger UI が同梱されます。自動化向けの API トークンは P1 機能として計画されています。

## 受信 / 検証 (`/api/captures`)

受信は「キャプチャ」単位で扱います。詳しい使い方は [受信と検証](../operations/rx-verify.md) を参照してください。

| メソッド / パス | 内容 |
|---|---|
| `GET /api/captures` | キャプチャ一覧 (実行中と直近の終了分、新しい順) |
| `POST /api/captures` | キャプチャを開始 (`backend`、`iface`、`filter`、`count`、`duration`、`keep`、`save`、`verify`、`promisc`、`include_outgoing`) |
| `GET /api/captures/{id}` | 状態と最新の統計 (`stats`: packets / bytes / matched / pps / bps / dropped / saved / `verify`) |
| `POST /api/captures/{id}/stop` | 停止 |
| `DELETE /api/captures/{id}` | 一覧から削除 (保存した pcapng は残ります) |
| `GET /api/captures/{id}/packets` | 最初の N フレーム (`index`、`ts_ns`、`len`、`hex`、`signature`)。デコードは `POST /api/parse` |
| `GET /api/captures/{id}/pcapng` | `save` で書き出した pcapng のダウンロード (終了後) |
| `POST /api/captures/filter` | フィルタ式の検査 (`{ "ok", "normalized" }` または `{ "ok": false, "error", "pos" }`) |
| `GET /api/captures/backends` | 受信バックエンドの利用可否 |

WebSocket (`/api/ws`) には、実行中のキャプチャごとに 100 ms 間隔の `rx_stats` (`capture_id`、`snapshot`) と、開始・終了時の `capture_state` が流れます。

ジョブ (`POST /api/jobs`) に `"signature": true` を付けると、送出する全フレームにシグネチャ (magic + ストリーム ID + 連番 + 送出時刻) を埋め込みます。`"stream_id"` で先頭ストリームの ID を指定できます (既定 0、ストリーム `i` は `stream_id + i`)。
