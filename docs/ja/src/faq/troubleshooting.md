# FAQ / トラブルシュート

困ったときはまず `wpg doctor` を実行してください。権限・インタフェース・カーネル・hugepages・Npcap / libpcap を 1 行ずつ診断し、OS 別の対処コマンドを表示します。`wpg doctor --json` はスクリプト向けに同じ内容を返し、チェックが fail のとき終了ステータスは 0 以外です。

## `wpg doctor` のチェック

| チェック id | 未達のときの状態 | 意味と対処 |
|---|---|---|
| `caps.raw_socket` | fail (Windows は Npcap の管理者専用モードしだい) | raw ソケットの権限。Linux は `CAP_NET_RAW` / root、macOS は BPF アクセス、Windows は Npcap が管理者専用モードのときだけ管理者昇格。[権限設定](../getting-started/permissions.md) |
| `iface.any_up` | warn | loopback 以外のインタフェースが carrier 付きで up。ケーブル / リンクを確認 |
| `kernel.af_xdp` | warn | AF_XDP バックエンド用にカーネル 5.10 以降 |
| `kernel.tpacket_v3` | warn | カーネル 3.2 以降。なければ afpacket は `sendmmsg` にフォールバック |
| `hugepages` | warn | Hugepages の設定。DPDK バックエンドでのみ必要 |
| `npcap` | fail / warn (Windows のみ) | Npcap が導入され、`wpcap.dll` が `System32\Npcap` か `System32` にあるか。他 OS では skip |
| `pcap_lib` | warn | libpcap が見つかること (`pcap` バックエンドで必要)。Windows では skip |

各結果は `ok` / `warn` / `fail` / `skip` (この OS では対象外) のいずれかです。

## よくある質問

### 送信が「permission denied」で拒否されます

生ソケットの送信には権限が必要です。`wpg doctor` を実行し、`caps.raw_socket` の行に表示されるコマンド (Linux なら `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)` など) を実行してください。file capability はバイナリを置き換えると失われます。[権限設定](../getting-started/permissions.md) を参照してください。

### 目標レートに届きません

- `average` が目標を下回り `dropped` が多いときは、バックエンドが追い付いていません。`--burst` を上げる、`afpacket` で `--qdisc-bypass` を使う、`--workers` を増やす (1 ワーカー = 1 コア) を試してください。[性能チューニング](../operations/tuning.md)。
- `--rate 50%` は、リンク速度の分かる実インタフェースが必要です。`lo` では `Cannot determine the link speed` になります。絶対値 (`1Gbps` など) を指定してください。
- `null` バックエンドの `unlimited` は、フレームを作る速さの測定であり、ネットワークの数値ではありません。

### 送信せずに確認したい

`--backend null` (作って数えるだけ)、または `--backend pcapfile -i out.pcapng` (ファイルに書き出し) で予行演習できます。`wpg render FILE -o out.pcapng` も送信せず pcapng を書き出します。

### テンプレートのエラーを送信前に見つけたい

`wpg template lint FILE` が、未知のフィールド・範囲外の値・ジェネレータの書式誤り・長さ / チェックサムの不一致を報告します。わざと壊したテンプレート (`templates/malformed/`) は警告 (`diag-*`) 止まりで、送信はできます。

### インポートしたテンプレートは元のキャプチャと同じですか

`wpg import` → `wpg render` の往復はバイト単位で一致します (タイムスタンプは wpg が独自に付けるため異なります)。`diff <(tshark -r a.pcapng -x) <(tshark -r b.pcapng -x)` で確認できます。

### Wireshark で確認したい

golden pcap の tshark 突合は `WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored` です。自分のキャプチャは `tshark -r out.pcapng -V` で、チェックサムが `[correct]` になることなどを確認できます。

### ユーザー向けの文字列が英語のままです

言語は、GUI は設定画面、CLI は `LANG` / `LC_MESSAGES` / `LC_ALL` 環境変数 (POSIX の優先順)、API は `Accept-Language` ヘッダで切り替わります。文字列が未翻訳なら英語にフォールバックします。キーを足す手順は [i18n 文字列の追加手順](../dev/add-i18n.md) を参照してください。

ここに無い問題は、`wpg doctor --json` の出力を添えて issue で報告してください。項目は報告に応じて追加します。
