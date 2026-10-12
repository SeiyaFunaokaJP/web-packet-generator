# API reference

Status: draft

wpg-server exposes a REST (JSON) API for templates, the protocol catalog, preview rendering, jobs, interfaces, stats and auth. Live statistics, logs and progress are pushed over WebSocket. The OpenAPI schema is generated with utoipa and Swagger UI is bundled. API tokens for automation are planned as a P1 feature.

## Receive / verification (`/api/captures`)

Receiving is organised in "captures". See [Receive and verification](../operations/rx-verify.md) for how to use them.

| Method / path | Purpose |
|---|---|
| `GET /api/captures` | Captures (running and the most recent finished ones), newest first |
| `POST /api/captures` | Start a capture (`backend`, `iface`, `filter`, `count`, `duration`, `keep`, `save`, `verify`, `promisc`, `include_outgoing`) |
| `GET /api/captures/{id}` | State and latest statistics (`stats`: packets / bytes / matched / pps / bps / dropped / saved / `verify`) |
| `POST /api/captures/{id}/stop` | Stop |
| `DELETE /api/captures/{id}` | Remove from the list (a saved pcapng file is kept) |
| `GET /api/captures/{id}/packets` | The first N frames (`index`, `ts_ns`, `len`, `hex`, `signature`); decode with `POST /api/parse` |
| `GET /api/captures/{id}/pcapng` | Download the pcapng written with `save` (after the capture ended) |
| `POST /api/captures/filter` | Check a filter expression (`{ "ok", "normalized" }` or `{ "ok": false, "error", "pos" }`) |
| `GET /api/captures/backends` | Availability of the receive backends |

The WebSocket (`/api/ws`) carries an `rx_stats` event (`capture_id`, `snapshot`) every 100 ms per running capture and a `capture_state` event when one starts or ends.

A job (`POST /api/jobs`) with `"signature": true` stamps every frame it sends with a signature (magic + stream id + sequence number + TX time). `"stream_id"` sets the id of the first stream (default 0; stream `i` gets `stream_id + i`).
