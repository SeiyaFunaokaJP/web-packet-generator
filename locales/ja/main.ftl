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

## wpg send / wpg bench (進捗・サマリ・エラー)
tx-progress = [{ $t } 秒] { $packets } パケット  { $pps }pps  { $bps }bps  ドロップ { $dropped }
tx-progress-rate = レート達成率 { $ratio }%
tx-progress-paused = (一時停止中)
tx-progress-scheduled = (予約済み、開始まで { $secs } 秒)
tx-interrupted = 中断を受け付けました。停止しています...
tx-ctrlc-warning = 警告: Ctrl-C ハンドラを設定できません: { $error }
tx-open-failed = { $backend } で `{ $iface }` を開けません: { $error }
tx-start-failed = ジョブを開始できません: { $error }
tx-available-backends = 利用可能なバックエンド: { $value }
tx-iface-required = { $backend } バックエンドには --iface が必要です。
tx-iface-required-pcapfile = pcapfile バックエンドには --iface <出力パス> が必要です。
tx-bad-workers = --workers は 1 以上を指定してください。
tx-workers-unsupported = { $backend } バックエンドはワーカー 1 つのみ対応です。--workers 1 を指定してください。
tx-pin-cores-short = 警告: --pin-cores の指定は { $count } コアですがワーカーは { $workers } 個です。残りのワーカーは固定されません。
tx-json-error = JSON を出力できません: { $error }
send-bad-rate = --rate が不正です: { $error }
send-bad-duration = --duration が不正です: { $error }
send-bad-ramp-up = --ramp-up が不正です: { $error }
send-bad-start-at = --start-at が不正です: { $error }
send-link-speed-unknown = `{ $iface }` のリンク速度が不明なため `{ $value }%` を解決できません。`1Gbps` のような絶対レートを指定してください。
send-bad-plan = --plan には auto、prerender、onthefly のいずれかを指定してください。
send-stream-error = ストリームのエラー: { $error }
send-render-error = 生成エラー: { $error }
send-starting = { $iface } に { $backend }{ $detail } で送出します: レート { $rate }、バースト { $burst }、{ $limit }、ワーカー { $workers }、展開方式 { $strategy } ({ $reason })
send-limit-count = { $count } パケット
send-limit-count-or-duration = { $count } パケットまたは { $duration }
send-limit-duration = { $duration } 間
send-limit-forever = Ctrl-C まで
send-ramp-up = ランプアップ: { $duration } かけて 0 から { $rate } へ
send-scheduled = { $time } に開始を予約しました (あと { $secs } 秒)。Ctrl-C で取り消せます。
summary-state = 状態         { $value }
summary-state-finished = 完了 ({ $reason })
summary-state-failed = 失敗: { $error }
summary-reason-count-reached = 指定数に到達
summary-reason-duration-elapsed = 指定時間が経過
summary-reason-source-exhausted = ストリーム終了
summary-reason-stopped = 停止
summary-backend = バックエンド { $backend }{ $detail }
summary-workers = ワーカー     { $workers }
summary-packets = パケット     { $value }
summary-bytes = バイト       { $value }
summary-dropped = ドロップ     { $value }
summary-elapsed = 経過時間     { $secs } 秒
summary-average = 平均         { $pps }pps  { $bps }bps (L2)  { $l1 }bps (L1)
summary-accuracy = 精度         目標 { $target }{ $unit }  達成率 { $ratio }%  最大ジッタ { $max }  平均ジッタ { $mean }
summary-stream = ストリーム { $index }  { $packets } パケット  { $bytes } バイト
summary-worker = ワーカー { $index }  { $packets } パケット  { $bytes } バイト{ $core }
summary-worker-core = 、コア { $core }
summary-warning = 警告         { $error }
bench-iface = インタフェース { $iface }
bench-frame = フレーム     { $len } B (+4 FCS)  バッチ { $batch }
bench-template = テンプレート { $value } (オンザフライ、{ $mode })
bench-mode-patch = インプレース書き換え
bench-mode-rebuild = フレーム毎に全再構築
bench-rate = レート       { $pps }pps
bench-bits = { $bps }bps (L2)  { $l1 }bps (L1)
bench-bad-duration = --duration には 0 以上の秒数を指定してください。
bench-failed = ベンチマークに失敗しました: { $error }
import-done = { $len } パケットを { $count } 個のテンプレートにインポートしました: { $value }
import-read-error = { $value }: { $error }
import-write-error = { $value }: { $error }
import-note = { $value }
import-note-packet = パケット { $index }: { $value }
import-packet-problems = パケット { $index } に { $count } 件の解析上の問題があります
import-packet-skipped = パケット { $index } は解析できなかったためスキップしました
import-not-reproducible = インポートしたテンプレートはキャプチャしたオクテットを再現できないため、フレームを raw データとして保存しました。
import-mixed-stacks = { $count } 個のパケットが先頭パケットと異なるレイヤ構成のため、ジェネレータは推定しませんでした。
import-generator-approximate = { $value } はキャプチャ値の近似です。
import-generators-dropped = 推定したジェネレータが不正なストリームになったため破棄しました。
template-export-none = エクスポートするテンプレートが指定されていません。
template-export-not-found = テンプレートが見つかりません: { $value }
template-export-done = { $count } 個のテンプレートを { $value } にエクスポートしました。
template-export-error = { $value }: { $error }
template-import-done = { $value } から { $count } 個のテンプレートを { $detail } に展開しました。
template-import-exists = { $value }: 既に存在します (上書きするには --force)
template-import-error = { $value }: { $error }

## API errors (wpg-server)
api-error-not-found = { $what } が見つかりません。
api-error-invalid-body = リクエスト本文が不正です: { $error }
api-error-invalid-query = リクエストパラメータが不正です: { $error }
api-error-invalid-template = テンプレートが不正です: { $error }
api-error-invalid-hex = 16 進入力が不正です: { $error }
api-error-parse-failed = バイト列を解析できません: { $error }
api-error-render-failed = パケット生成に失敗しました: { $error }
api-error-count-range = `count` は 1 以上 { $max } 以下で指定してください (指定値 { $error })。
api-error-read-only = テンプレート { $id } は読み取り専用ライブラリにあります。編集するには複製してください。
api-error-policy-iface = サーバのポリシーにより { $iface } からの送出は許可されていません (許可: { $allowed })。
api-error-policy-rate = レート { $rate } はサーバのポリシーの上限 ({ $max }) を超えています。
api-error-policy-unlimited = サーバのポリシーにより無制限レートは許可されていません (上限 { $max })。
api-error-invalid-state = ジョブ { $id } は { $state } 状態のため { $action } できません。
api-error-job-spec = ジョブ指定が不正です: { $error }
api-error-backend = 送出バックエンドのエラー: { $error }
api-error-internal = サーバ内部エラー: { $error }
lint-parse-error = テンプレートを解析できません: { $error }

## Configuration (wpg.toml)
config-issue-file-missing = ファイルが見つかりません
config-issue-tls-not-implemented = TLS はまだ提供されていません。HTTPS にはリバースプロキシを前段に置いてください
config-issue-bad-url = http:// または https:// で始めてください
config-issue-not-argon2 = argon2 ハッシュ ($argon2id$...) ではないようです
config-issue-password-both = password_hash と password_file の両方が設定されています (password_hash を優先)
config-issue-zero = 0 より大きい値を指定してください
config-issue-exposed-without-password = パスワード未設定のままループバック以外のアドレスで待ち受けます
config-issue-dir-missing = ディレクトリが見つかりません
config-issue-max-rate-percent = 回線率 (%) はサーバ全体の上限に使えません。pps か bps で指定してください
config-issue-bad-rate = レートが不正です
config-issue-unknown-backend = 不明なバックエンドです
config-issue-backend-unavailable = このビルド / OS では使えないバックエンドです
config-issue-bad-log-level = ログフィルタが不正です
config-level-error = エラー
config-level-warning = 警告
config-issue-line = { $level }: { $key }: { $message }{ $detail }
config-load-error = 設定エラー: { $error }
config-init-written = { $path } を作成しました。編集後 `wpg config validate` で確認してください。
config-init-exists = { $path } は既に存在します (上書きするには --force)。
config-validate-ok = 設定に問題はありません ({ $source })。
config-validate-summary = エラー { $errors } 件、警告 { $warnings } 件 ({ $source })。
config-source-defaults = 組み込みの既定値と環境変数

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = 待ち受け         http://{ $addr }/
serve-banner-docs = API ドキュメント  http://{ $addr }/api/docs/
serve-banner-config = 設定ファイル      { $source }
serve-banner-data = データ           { $path }
serve-banner-library = ライブラリ        { $path }{ $state }
serve-banner-user-dir = ユーザテンプレート { $path }
serve-banner-templates = テンプレート数    { $count }
serve-banner-backends = バックエンド      { $list }
serve-banner-missing = { " " }(存在しません)
serve-banner-dev = 開発モード: { $origins } からの CORS を許可
serve-banner-no-password = パスワードが設定されていません: { $addr } に到達できる誰もがパケットを送出できます。サーバは localhost で運用してください (認証は今後のリリースで追加されます)。
serve-failed = サーバエラー: { $error }
serve-log-error = ログを初期化できません: { $error }
