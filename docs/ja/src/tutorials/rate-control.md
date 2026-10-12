# チュートリアル: 負荷とレート制御

所要時間: 約 20 分。必要な権限: なし (全体を通して `null` と `pcapfile` バックエンドを使います)。

> ここで扱う技術 (大量生成、レート、時間、ランプアップ、ワーカー) はどのテンプレートでも使えます。実際のサーバに負荷をかけて応答を数える例は [DHCP サーバの負荷試験](dhcp-load.md) にあります。

## 作るもの

テンプレートを指定のレートと時間で送出し、返ってくるサマリを読み、ラボの制限内に収めるためのコマンド群です。題材は MAC フラッド (`templates/scenarios/mac-flood.wpg.yaml`、ランダムな送信元 MAC の 10 万フレーム) と SYN フラッド (`templates/scenarios/vlan-syn-flood.wpg.yaml`) です。

## 始める前に

wpg をビルドし、リポジトリのルートで実行します。実際の送信には権限とラボのインタフェースが必要ですが、このチュートリアルは `--backend null` (フレームを作って数えるだけで送らない) を使うので、どこでも実行できます。実際に送るときは `--backend null -i lo` を `-i eth1` などに置き換えます。

負荷をかけてよいのは、自分が所有する、または試験を許可された機器だけです。

## ステップ 1. 回数・時間・レート

停止条件と速度はテンプレートの `tx:` にあり、コマンドラインで上書きできます。

| オプション | 意味 | 例 |
|---|---|---|
| `--count N` | N フレームで停止 (N がストリームより長ければ先頭に戻る) | `--count 50000` |
| `--duration T` | T 秒/分で停止 | `--duration 10s`、`--duration 2m` |
| `--rate R` | 目標レート | `250kpps`、`1Mpps`、`1Gbps`、`50%`、`ifg=10us`、`unlimited` |
| `--burst B` | レート制御 1 回あたりに連続送信するフレーム数 | `--burst 32` |

回数と時間の両方を指定した場合は先に達した方で終了します。`bps` 指定はレイヤ 1 のバイト数 (プリアンブル、FCS、フレーム間ギャップ込み) で数えます。`%` はリンク速度が分かる実インタフェースが必要です (`lo` では失敗します)。

```console
$ wpg send templates/scenarios/udp-qos-matrix.wpg.yaml --backend null -i lo --rate 250kpps --count 50000
Sending on lo via null: rate 250000pps, burst 1, 50000 packets, 1 worker(s), plan PreRender (...)
state      finished (count reached)
backend    null
packets    50000
bytes      10100000
dropped    0
elapsed    0.200 s
average    250.00 kpps  404.00 Mbps (L2)  452.00 Mbps (L1)
accuracy   target 250.00 kpps  achieved 100.00%  max jitter 454.12µs  mean jitter 202ns
```

(数値は環境によって異なります。) まず最後の 2 行を読みます。`average` は実際に送れたレート、`accuracy` は目標にどれだけ近く送れたかです。`dropped` が大きいときは、バックエンドが追い付いていません。

## ステップ 2. ランプアップ

最初のバーストで落ちる機器は、レートを上げていくと特性を調べやすくなります。`--ramp-up` はレートをゼロから目標まで指定時間で直線的に上げます。

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --rate 20kpps --ramp-up 1s --duration 3s
[    3.0 s]        50023 pkts  16.67 kpps  9.34 Mbps  dropped 0  rate 100.0%
state      finished (duration elapsed)
packets    50023
elapsed    3.000 s
average    16.67 kpps  9.34 Mbps (L2)  12.54 Mbps (L1)
accuracy   target 20.00 kpps  achieved 100.04%  max jitter 3.196862ms  mean jitter 115.686µs
```

最初の 1 秒がランプなので、平均 (16.67 kpps) は目標 20 kpps を下回ります。

## ステップ 3. 開始時刻を指定する

`--start-at` は最初のフレームの前に待ちます。遅延 (`+10s`) か RFC 3339 の時刻 (`2026-09-29T12:00:00Z`) を指定します。複数の送信側を同時に始めたいときや、キャプチャを起動する時間が欲しいときに便利です。

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --start-at +2s --count 10
Scheduled start at +2s (in 2.0 s); Ctrl-C cancels.
```

## ステップ 4. 複数ワーカーを使う

ワーカー 1 つは CPU コア 1 つが上限です。`--workers N` は N 本の送信スレッドを起動し、それぞれが独立したバックエンドインスタンスと、ストリームおよびレートの分担を持ちます。`--pin-cores 2,3` でコアに固定できます (Linux)。

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --rate unlimited --count 100000 --workers 2
average    10.35 Mpps  5.79 Gbps (L2)  7.78 Gbps (L1)
worker 0     50000 pkts  3500000 bytes
worker 1     50000 pkts  3500000 bytes
```

`null` バックエンドでの `unlimited` は、フレームを作る速さの測定であり、ネットワークの数値ではありません。`afpacket` バックエンドでは `--batch`、`--ring-depth`、`--qdisc-bypass`、`--no-tx-ring` も使えます (`wpg send --help`)。詳しくは [性能チューニング](../operations/tuning.md) を参照してください。

## ステップ 5. 事前レンダリングかその場生成か

`--plan auto` (既定) は、ストリームが小さければ (しきい値 256 MiB) メモリに事前レンダリングして送信ループではバイトのコピーだけを行い、大きければその場で展開します。`--plan prerender` または `--plan onthefly` で強制できます。ランダムジェネレータの値は `stream.seed` から決まるので、再現性が必要なら seed を指定してください。

## ステップ 6. 機械可読な結果

`--json` は最終レポートを JSON で標準出力に出します (進捗とバナーは標準エラー)。CI やスクリプト向けです。

```console
$ wpg send templates/scenarios/mac-flood.wpg.yaml --backend null -i lo --count 100000 --json | jq '{state: .state.state, packets: .backend.packets, dropped: .backend.dropped}'
```

実行に失敗したとき (たとえばバックエンドが拒否された) は終了コードが 0 以外になるので、`wpg send` はパイプラインの 1 ステップにできます。

## ステップ 7. 制限内に収める

* `wpg.toml` の `[tx] max_rate` (例: `max_rate = "100kpps"`) で全員に対する上限を設定し、`allowed_ifaces` で使えるインタフェースを制限します。[設定リファレンス](../reference/config.md) を参照してください。
* `Ctrl-C` で実行をキャンセルできます。
* 新しいテンプレートは、毎回予行演習用のバックエンド (`null`、または `-i out.pcapng` を指定した `pcapfile`) で試してから送ります。

## 次のステップ

* 実ネットワークで記録したトラフィックで機器に負荷をかけるには [pcap からテンプレートへ変換して再生](pcap-replay.md)。
* バックエンドごとの調整は [性能チューニング](../operations/tuning.md)。
