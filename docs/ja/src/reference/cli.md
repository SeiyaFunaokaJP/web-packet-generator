# CLI リファレンス

Status: draft

`wpg` バイナリは serve、send、render、import、template、ifaces、doctor、bench、capture、config の各サブコマンドを提供します。サーバとヘッドレス送出が同一バイナリで動くため、CI や ssh 先での実行に向いています。例えば `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` でテンプレートを送出します。以下のコードブロックは現行ビルドの `--help` 出力そのままです。ここに載せていないサブコマンド (`serve`、`import`) は未整備で、`wpg <command> --help` を参照してください。計画中のフラグ (`--workers`、`--ramp-up`、`--start-at`、テンプレートを使う bench) は、`--help` に現れた時点でここに載ります。

## コマンド

### `wpg`

```text
Web Packet Generator

Usage: wpg <COMMAND>

Commands:
  serve     Start the web UI / API server
  send      Send a template headlessly
  render    Render a template to a pcapng file without transmitting
  import    Convert a pcap/pcapng capture into a template
  template  Template tools (lint, list, export, import)
  ifaces    List network interfaces usable for transmission
  doctor    Diagnose privileges, drivers, hugepages and Npcap
  bench     Measure transmit performance
  capture   Receive: counters, the first frames, pcapng output, signature verification
  config    Configuration file tools (init, show, validate)
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `wpg send`

テンプレートを送出します。`--plan` は、全フレームを先に展開する (`prerender`) か、送出しながら組み立てる (`onthefly`) かを選びます。`--json` は最終レポートを標準出力に JSON で出します。`--signature` は各フレームに受信検証用のシグネチャを埋め込み、`wpg capture --verify` で照合できます ([受信と検証](../operations/rx-verify.md))。

```text
Send a template headlessly

Usage: wpg send [OPTIONS] <TEMPLATE>

Arguments:
  <TEMPLATE>  Template file (`.wpg.yaml`)

Options:
  -i, --iface <IFACE>        Interface to transmit on
      --backend <BACKEND>    Transmit backend (null, pcapfile, pcap, afpacket, afxdp, dpdk)
      --count <COUNT>        Number of packets
      --duration <DURATION>  Duration limit, e.g. `10s`, `2m`
      --rate <RATE>          Rate, e.g. `1Mpps`, `10Gbps`, `50%`, `ifg=10us`, `unlimited`
      --burst <BURST>        Frames sent back to back per rate-control grant
      --ramp-up <DURATION>   Ramp the rate up linearly from 0 over this time, e.g. `5s`
      --start-at <TIME>      Start at this time: RFC 3339 (`2026-09-29T12:00:00Z`) or a delay (`+10s`)
      --plan <PLAN>          Expansion strategy: auto, prerender or onthefly [default: auto]
      --json                 Print the final report as JSON on stdout
      --var <NAME=VALUE>     Template variable override (`name=value`), repeatable
      --signature            Stamp every frame with a receive-verification signature (magic, stream id, sequence number, TX time over the last 24 payload octets; see `wpg capture --verify`)
      --stream-id <ID>       Signature stream id (with `--signature`) [default: 0]
      --batch <N>            Frames per `send_batch` call
      --ring-depth <N>       TX ring depth in frames (`afpacket`)
      --pin-core <CORE>      Pin the TX thread to this CPU core (Linux; the first worker)
      --pin-cores <CORES>    Pin worker i to the i-th core of this list, e.g. `1,2` (Linux)
      --workers <N>          TX worker threads, each with its own backend instance (afpacket: socket and ring) and share of the stream and rate [default: 1]
      --qdisc-bypass         Bypass the qdisc layer (`afpacket`, `PACKET_QDISC_BYPASS`)
      --no-tx-ring           Use `sendmmsg` instead of the TPACKET_V3 TX ring (`afpacket`)
  -h, --help                 Print help
  -V, --version              Print version
```

### `wpg render`

送出せずに、テンプレートのフレームを pcapng ファイルへ書き出します。

```text
Render a template to a pcapng file without transmitting

Usage: wpg render [OPTIONS] --output <OUTPUT> <TEMPLATE>

Arguments:
  <TEMPLATE>  Template file

Options:
  -o, --output <OUTPUT>   Output pcapng file
      --count <COUNT>     Number of packets
      --var <NAME=VALUE>  Template variable override (`name=value`), repeatable
  -h, --help              Print help
  -V, --version           Print version
```

### `wpg bench`

合成フレーム (EtherType 0x88b5) をレート無制限で送出します。ベースラインは `docs/perf.md` を参照。`--json` の項目: backend、detail、iface、frame_size、batch_size、packets、dropped、pps、bps (L2)、l1_bps、elapsed。

```text
Measure transmit performance

Usage: wpg bench [OPTIONS]

Options:
      --backend <BACKEND>        Backend to benchmark
  -i, --iface <IFACE>            Interface (for real backends)
      --duration <DURATION>      Duration in seconds [default: 5]
      --frame-size <FRAME_SIZE>  Frame size in bytes (without FCS) [default: 64]
      --json                     Output JSON
      --batch <N>                Frames per `send_batch` call
      --ring-depth <N>           TX ring depth in frames (`afpacket`)
      --pin-core <CORE>          Pin the TX thread to this CPU core (Linux)
      --qdisc-bypass             Bypass the qdisc layer (`afpacket`, `PACKET_QDISC_BYPASS`)
      --no-tx-ring               Use `sendmmsg` instead of the TPACKET_V3 TX ring (`afpacket`)
  -h, --help                     Print help
  -V, --version                  Print version
```

### `wpg capture`

受信カウンタ (pps / bps、フィルタ一致数) を取り、最初の N フレームの表示、pcapng 保存、`wpg send --signature` で埋め込んだシグネチャの照合 (ロス / 順序逆転 / 重複 / 遅延) を行います。フィルタの書式と結果の読み方は [受信と検証](../operations/rx-verify.md) を参照してください。`--json` は `{ "report": …, "packets": [...] }` を標準出力に出します。

```text
Receive: counters, the first frames, pcapng output, signature verification

Usage: wpg capture [OPTIONS]

Options:
  -i, --iface <IFACE>        Interface to receive on (`any` = every interface with `afpacket`); for the `pcapfile` backend the capture file to replay
      --backend <BACKEND>    Receive backend: afpacket (Linux), pcap (feature `pcap`), pcapfile, null
  -f, --filter <EXPR>        tcpdump-style filter, e.g. `udp port 5001 and wpg` (default: every frame)
  -c, --count <COUNT>        Stop after this many matching frames
      --duration <DURATION>  Stop after this long, e.g. `10s`
      --show <N>             Print the first N matching frames [default: 5]
      --hex                  Also print a hex dump of the shown frames
  -w, --write <FILE>         Write the matching frames to this pcapng file
      --verify               Check the signatures stamped by `wpg send --signature` (loss / order / latency)
      --promisc              Promiscuous mode
      --include-outgoing     Also count frames this host transmits
      --json                 Machine-readable output
  -h, --help                 Print help
  -V, --version              Print version
```

### `wpg ifaces`

MAC、アドレス、MTU、リンク状態、ドライバ付きでインタフェースを一覧します。

```text
List network interfaces usable for transmission

Usage: wpg ifaces [OPTIONS]

Options:
      --json     Output JSON
  -h, --help     Print help
  -V, --version  Print version
```

### `wpg doctor`

[権限設定](../getting-started/permissions.md)にある環境チェックを実行します。チェックが fail のとき終了ステータスは 0 以外です。

```text
Diagnose privileges, drivers, hugepages and Npcap

Usage: wpg doctor [OPTIONS]

Options:
      --json     Output JSON
  -h, --help     Print help
  -V, --version  Print version
```

### `wpg template`

テンプレートのツール。`lint` はエラー時に終了ステータス 1 を返します。`export` と `import` は未実装です。

```text
Template tools (lint, list, export, import)

Usage: wpg template <COMMAND>

Commands:
  lint    Check templates for errors and warnings (exit status 1 on errors)
  list    List the templates of a directory (default: `templates/`)
  show    Print the built packet (hex dump and layout tree) of a template
  schema  Print the JSON Schema of the template format
  export  Export templates (not implemented yet)
  import  Import templates (not implemented yet)
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `wpg template lint`

```text
Check templates for errors and warnings (exit status 1 on errors)

Usage: wpg template lint <FILES>...

Arguments:
  <FILES>...  Template files

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `wpg template list`

```text
List the templates of a directory (default: `templates/`)

Usage: wpg template list [DIR]

Arguments:
  [DIR]  Directory to scan recursively

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `wpg template show`

```text
Print the built packet (hex dump and layout tree) of a template

Usage: wpg template show [OPTIONS] <FILE>

Arguments:
  <FILE>  Template file

Options:
      --var <NAME=VALUE>  Template variable override (`name=value`), repeatable
  -h, --help              Print help
  -V, --version           Print version
```

### `wpg template schema`

```text
Print the JSON Schema of the template format

Usage: wpg template schema

Options:
  -h, --help     Print help
  -V, --version  Print version
```
