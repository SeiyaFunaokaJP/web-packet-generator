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
