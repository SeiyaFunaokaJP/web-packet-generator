# 日本語 (ja)。locales/en/main.ftl と同じキー集合を保つこと
# (wpg-i18n のユニットテストで検査される)。

app-name = Web Packet Generator
app-tagline = ブラウザから操作できる、ネットワーク検証向け高性能パケットジェネレータ

## Server / API
health-ok = サービスは正常です
server-listening = { $addr } で待ち受け中
server-placeholder-ui = Web UI はまだビルドされていません。crates/wpg-web で `trunk build` を実行してください。

## Errors
error-not-implemented = `{ $command }` はまだ実装されていません。
error-not-found = 見つかりません
error-internal = サーバ内部エラー

## UI
ui-language = 言語
ui-protocols = プロトコル

## 診断 (wpg-core の `diag::keys`。引数名は契約の一部)
diag-field-out-of-range = フィールド `{ $field }` = { $value } は範囲 { $min }..{ $max } の外です。
diag-field-not-equal = フィールド `{ $field }` = { $value } は期待値 { $expected } と異なります。
diag-field-forbidden-value = フィールド `{ $field }` の値 { $value } は推奨されません。
diag-length-too-short = `{ $span }` は { $len } バイトで、最低 { $min } バイト必要です。
diag-length-too-long = `{ $span }` は { $len } バイトで、最大 { $max } バイトまでです。
diag-length-mismatch = 長さフィールド `{ $field }` = { $value } は計算値 { $expected } と異なります。
diag-checksum-mismatch = チェックサム `{ $field }` = { $value } は計算値 { $expected } と異なります。
diag-next-protocol-mismatch = フィールド `{ $field }` = { $value } は次の層 `{ $next }` を示していません (期待値 { $expected })。
diag-auto-overflow = `{ $field }` の自動値 { $value } は { $bits } ビットに収まりません。
diag-length-not-aligned = `{ $field }` の長さ { $len } は { $unit } バイトの倍数ではありません。
diag-no-pseudo-header = チェックサム `{ $field }` には疑似ヘッダが必要ですが、下位に IP 層がありません。
diag-not-implemented = `{ $field }`: { $what } はまだ実装されていません。
diag-truncated = この層は意図的に { $len } バイトへ切り詰められました。
diag-short-header = { $protocol } ヘッダの途中 (オフセット { $offset }) で入力が終わっています。
diag-length-exceeds-data = 長さフィールド `{ $field }` = { $len } が利用可能な { $available } バイトを超えています。
diag-trailing-bytes = パケットの終端の後に { $len } バイトが残っています。
diag-unsupported-field = { $protocol } のフィールド `{ $field }` はまだパーサで解読できません。

## プロトコルフィールドの説明 (wpg-protocols の `desc = "..."`)
arp-hlen = ハードウェアアドレス長 (バイト、Ethernet は 6)。
arp-htype = ハードウェア種別 (1 = Ethernet)。
arp-oper = オペレーション (1 = 要求、2 = 応答)。
arp-plen = プロトコルアドレス長 (バイト、IPv4 は 4)。
arp-ptype = プロトコル種別 (EtherType、0x0800 = IPv4)。
arp-sha = 送信元ハードウェアアドレス。
arp-spa = 送信元プロトコルアドレス。
arp-tha = 宛先ハードウェアアドレス。
arp-tpa = 宛先プロトコルアドレス。
icmp-code = メッセージコード (意味は種別による)。
icmp-identifier = エコー要求と応答を対応付ける識別子。
icmp-rest = 種別ごとに意味が異なる 4 バイトのヘッダ残り部分。
icmp-sequence = エコー要求と応答を対応付けるシーケンス番号。
icmp-type = メッセージ種別。
ipv6-fragment-offset = このフラグメントのオフセット (8 バイト単位)。
ipv6-more-fragments = MF フラグ (1 = 後続のフラグメントあり)。
ipv6-options = 生の TLV オプション (8 バイトの倍数にパディング)。
ipv6-routing-data = ルーティングタイプ固有のデータ。
ipv6-routing-type = ルーティングヘッダ種別 (例: 2 = Mobile IPv6、4 = SRH)。
ipv6-segments-left = 残りのルートセグメント数。
ndp-destination = リダイレクト先の宛先アドレス。
ndp-managed = 管理アドレス設定フラグ (M)。
ndp-options = 生の NDP オプション (TLV)。
ndp-other = その他設定フラグ (O)。
ndp-override-flag = 上書きフラグ (O)。
ndp-router-flag = ルータフラグ (R)。
ndp-solicited-flag = 要請応答フラグ (S)。
ndp-target = ターゲットアドレス。

## wpg ifaces
ifaces-col-name = 名前
ifaces-col-index = IDX
ifaces-col-mac = MAC
ifaces-col-ipv4 = IPV4
ifaces-col-ipv6 = IPV6
ifaces-col-mtu = MTU
ifaces-col-state = 状態
ifaces-col-speed = 速度
ifaces-col-driver = ドライバ
ifaces-state-up = up
ifaces-state-down = down
ifaces-state-no-carrier = キャリアなし
ifaces-state-loopback = ループバック
ifaces-none = ネットワークインタフェースが見つかりません。
ifaces-error = ネットワークインタフェースを列挙できませんでした: { $error }

## wpg doctor
doctor-title = wpg doctor: 環境チェック
doctor-summary = OK { $ok } 件、警告 { $warn } 件、失敗 { $fail } 件
doctor-skip = このOSでは対象外です
doctor-caps-ok = raw ソケットを利用できます
doctor-caps-missing = パケット送出に必要な権限がありません (CAP_NET_RAW / root / 管理者)
doctor-fix-caps = 権限を付与してください: { $hint }
doctor-iface-ok = 利用可能なネットワークインタフェースが1つ以上あります
doctor-iface-none = キャリアありで up のループバック以外のインタフェースがありません
doctor-iface-error = ネットワークインタフェースを列挙できませんでした
doctor-fix-iface = インタフェースを確認してください: { $hint }
doctor-af-xdp-ok = カーネルは AF_XDP に対応しています (5.10 以降)
doctor-af-xdp-old = カーネルが AF_XDP には古すぎます (5.10 以降が必要)。afpacket バックエンドは利用できます
doctor-tpacket-v3-ok = カーネルは TPACKET_V3 に対応しています
doctor-tpacket-v3-old = カーネルが TPACKET_V3 に対応していないため、afpacket は sendmmsg にフォールバックします
doctor-hugepages-ok = ヒュージページが設定されています
doctor-hugepages-none = ヒュージページが未設定です (DPDK バックエンドでのみ必要)
doctor-fix-hugepages = ヒュージページを確保し NIC を DPDK にバインドしてください: { $hint }
doctor-npcap-ok = Npcap がインストールされています (WinPcap 互換モード)
doctor-npcap-missing = Npcap がインストールされていません。Windows での送出に必要です
doctor-npcap-not-compat = Npcap は WinPcap API 互換モードではないため、wpcap.dll が見つからない場合があります
doctor-fix-npcap = Npcap をインストールしてください: { $hint }
doctor-fix-npcap-compat = { $hint }
doctor-pcaplib-ok = libpcap を利用できます
doctor-pcaplib-missing = libpcap が見つかりません。pcap バックエンドは利用できません
doctor-fix-pcaplib = libpcap をインストールしてください: { $hint }

## wpg template lint (wpg-template `LintIssue` keys)
lint-level-error = エラー
lint-level-warning = 警告
lint-schema-version = 未対応のスキーマバージョン { $value } です (対応: { $expected })。
lint-missing-name = `meta.name` が空です。
lint-no-layers = テンプレートにレイヤがありません。
lint-unknown-protocol = 不明なプロトコル `{ $protocol }` です。
lint-unknown-field = プロトコル `{ $protocol }` にフィールド `{ $field }` はありません。
lint-invalid-value = `{ $field }` の値が不正です: { $value }
lint-invalid-generator = `{ $field }` のジェネレータが不正です: { $value }
lint-unknown-variable = 不明な変数 `{ $value }` です。
lint-missing-variable = 変数 `{ $value }` にはデフォルト値がなく、値も指定されていません。
lint-unused-variable = 変数 `{ $value }` は使われていません。
lint-duplicate-variable = 変数 `{ $value }` が複数回宣言されています。
lint-var-syntax = `{ $value }` 内の変数プレースホルダが閉じられていません。
lint-override-unknown = 不明な変数 `{ $value }` を上書きしようとしています。
lint-field-and-generator = `{ $field }` に固定値とジェネレータの両方があります。ジェネレータが優先されます。
lint-next-protocol-manual = `{ $next }` は `{ $protocol }` のバインディング表にないため、`{ $field }` は Manual になります。
lint-unknown-order = ストリームの順序に指定された `{ $value }` にはジェネレータがありません。
lint-invalid-stream = ストリーム設定が不正です: { $value }
lint-build-error = パケットを構築できません: { $value }

## wpg template / wpg render
template-lint-ok = { $value }: 問題なし
template-lint-load-error = { $value }: { $error }
template-lint-summary = { $ok } ファイル中 エラー { $fail } 件、警告 { $warn } 件
template-list-empty = { $value } にテンプレートが見つかりません。
template-show-hex = Hex ダンプ ({ $len } バイト)
template-show-layout = レイアウト
render-done = { $len } パケットを { $value } に書き出しました。
render-unbounded = ストリームが無限です。--count N を指定してください。
cli-bad-var = --var `{ $value }` が不正です。NAME=VALUE 形式で指定してください。
