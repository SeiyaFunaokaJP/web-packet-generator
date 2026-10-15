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
diag-count-mismatch = 件数フィールド `{ $field }` = { $value } は実際の要素数 { $expected } と異なります。
diag-fill-inexact = `{ $field }` の値まで埋め草アイテムで正確に埋められません（{ $len } オクテット不足）。
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
srh-segments-left = Segments Left: 次に向かうセグメントのセグメントリスト上の位置。最後のセグメントで 0。
srh-last-entry = Last Entry: セグメントリスト最後の要素の位置 (= セグメント数 - 1、Auto)。
srh-flags = フラグ (RFC 8754 では未定義。0)。
srh-tag = Tag: パケットをクラスやグループで分類するための値 (0 = 未使用)。
srh-segment-list = セグメントリスト (SID) を経路の逆順で: 要素 0 が最後のセグメント、最後の要素が最初のセグメント。Segments Left が 0 でない間は、上位層チェックサムの宛先に要素 0 を使用。
srh-tlvs = 省略可能な TLV (8 ビット型、8 ビット長): Pad1、PadN、HMAC (HMAC 値は手動)。ヘッダは Pad1 で 8 オクテットの倍数まで自動で詰める。
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
doctor-caps-npcap-user = 管理者として実行していませんが、Npcap は管理者以外にもキャプチャと送出を許可しています
doctor-caps-npcap-admin = Npcap は管理者のみに制限されています。送出するには管理者として開いたターミナルで wpg を実行してください
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
doctor-npcap-ok = Npcap がインストールされています
doctor-npcap-missing = Npcap がインストールされていません。Windows での送出に必要です
doctor-npcap-no-dll = Npcap はインストールされているようですが、wpcap.dll が System32\Npcap にも System32 にも見つかりません
doctor-fix-npcap = Npcap をインストール (または再インストール) して、wpg doctor を再実行してください: { $hint }
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
summary-unsigned = 未署名       { $value } (シグネチャを入れる余地なし)

## wpg capture (受信・検証)
rx-progress = [{ $t } 秒] { $packets } パケット  { $pps }pps  { $bps }bps  一致 { $matched } ({ $mpps }pps)  ドロップ { $dropped }
capture-starting = { $iface } で { $backend } により受信します。フィルタ { $filter } (Ctrl-C で停止)
capture-filter-all = (全フレーム)
capture-iface-required-pcapfile = pcapfile バックエンドには --iface <キャプチャファイル> が必要です。
capture-bad-filter = --filter が不正です: { $error }
capture-running = 受信中
capture-reason-end-of-input = 入力の終わり
capture-packet = #{ $index }  { $time }  { $len } B  { $layers }{ $signature }
capture-packet-signature = { "  " }[wpg ストリーム { $stream } 連番 { $seq }]
capture-summary-matched = 一致         { $packets } パケット  { $bytes } バイト
capture-summary-average = 平均         { $pps }pps  { $bps }bps  一致 { $mpps }pps  { $mbps }bps
capture-summary-saved = 保存         { $count } パケットを { $path } へ
capture-summary-signed = シグネチャ   あり { $signed }、なし { $unsigned }
capture-summary-stream = ストリーム { $stream } 受信 { $received }  連番 { $first }-{ $last }  ロス { $lost } ({ $ratio }%)  順序逆転 { $reordered }  重複 { $duplicates }
capture-summary-latency = { "             " }遅延 最小 { $min }  平均 { $avg }  最大 { $max }  ジッタ { $jitter }

bench-iface = インタフェース { $iface }
bench-worker = ワーカー { $index }  { $detail }
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
template-migrate-outdated = { $value }: schema { $found } のため { $expected } への移行が必要です
template-migrate-done = { $value }: schema { $found } から { $expected } へ移行しました
template-migrate-error = { $value }: { $error }
template-migrate-summary = 移行 { $count } 件、schema { $expected } のまま { $ok } 件、エラー { $fail } 件
template-migrate-check-summary = 移行が必要 { $count } 件、schema { $expected } のまま { $ok } 件、エラー { $fail } 件

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
api-error-capture-spec = 受信設定が不正です: { $error }
api-error-capture-filter = 受信フィルタが不正です: { $error }
api-error-capture-backend = 受信バックエンドのエラー: { $error }
api-error-capture-state = 受信 { $id } は { $state } のため { $action } できません。
api-error-internal = サーバ内部エラー: { $error }
auth-error-unauthorized = ログインしてください。
auth-error-bad-credentials = ユーザ名またはパスワードが違います。
auth-error-setup-required = 管理パスワードが未設定です。先に初期設定を完了してください。
auth-error-already-configured = 管理パスワードは既に設定されています。
auth-error-rate-limited = ログイン失敗が多すぎます。{ $secs } 秒後に再試行してください。
auth-error-csrf = CSRF トークン (X-CSRF-Token ヘッダ) がないか不正です。
auth-error-origin = 別オリジンからの WebSocket 接続は許可されていません。
auth-error-setup-remote = 管理パスワードはサーバ自身 (localhost) からのみ設定できます。
auth-error-password-mismatch = パスワードが一致しません。
auth-error-password-short = パスワードは { $min } 文字以上にしてください。
lint-parse-error = テンプレートを解析できません: { $error }

## Configuration (wpg.toml)
config-issue-file-missing = ファイルが見つかりません
config-issue-bad-url = http:// または https:// で始めてください
config-issue-not-argon2 = argon2 ハッシュ ($argon2id$...) ではないようです
config-issue-password-both = password_hash と password_file の両方が設定されています (password_hash を優先)
config-issue-zero = 0 より大きい値を指定してください
config-issue-exposed-without-password = パスワード未設定のままループバック以外のアドレスで待ち受けます
config-issue-dir-missing = ディレクトリが見つかりません
config-issue-protocols = ユーザ定義プロトコルを読み込めません
protocols-ext-loaded = ユーザ定義プロトコルを { $count } 件読み込みました
protocols-ext-error = ユーザ定義プロトコルを読み込めません: { $file }: { $error }
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
config-password-prompt = 新しい管理パスワード:{ " " }
config-password-confirm = もう一度入力:{ " " }
config-password-mismatch = パスワードが一致しません。
config-password-short = パスワードは { $min } 文字以上にしてください。
config-password-written = パスワードのハッシュを { $path } (auth.password_hash) に保存しました。反映するには `wpg serve` を再起動してください。

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = 待ち受け         { $url }/
serve-banner-docs = API ドキュメント  { $url }/api/docs/
serve-banner-config = 設定ファイル      { $source }
serve-banner-data = データ           { $path }
serve-banner-library = ライブラリ        { $path }{ $state }
serve-banner-user-dir = ユーザテンプレート { $path }
serve-banner-templates = テンプレート数    { $count }
serve-banner-backends = バックエンド      { $list }
serve-banner-missing = { " " }(存在しません)
serve-banner-dev = 開発モード: { $origins } からの CORS を許可
serve-banner-no-password = パスワード未設定のまま { $url } で待ち受けています (--allow-insecure): 到達できる誰もが管理パスワードを設定してパケットを送出できます。直ちにパスワードを設定してください (wpg config set-password)。
serve-failed = サーバエラー: { $error }
serve-log-error = ログを初期化できません: { $error }
serve-banner-tls = TLS              { $path }
serve-banner-tls-self-signed = TLS              { $path } (自己署名)
serve-banner-fingerprint = SHA-256          { $fingerprint }
serve-banner-auth = ログイン          { $state }
serve-banner-auth-setup = パスワード未設定: 初期設定画面で設定してください
serve-banner-auth-configured = 管理パスワード: { $source }
serve-banner-auth-database = 管理パスワード: 初期設定画面で設定済み
serve-banner-setup-local = 管理パスワードが未設定です。このマシンのブラウザで { $url } を開いて設定してください。
serve-refuse-insecure = { $addr } では待ち受けません: 管理パスワードが設定されていません。ループバックアドレス (127.0.0.1) を使うか、パスワードを設定する (--password-file、WPG_PASSWORD、wpg config set-password) か、--allow-insecure を指定してください。
serve-password-arg-warning = 警告: --password はプロセス一覧から他のユーザに見えます。--password-file、WPG_PASSWORD、`wpg config set-password` を推奨します。

## Web UI (wpg-web). Keys are checked by the `ui_keys_exist` test in crates/wpg-web.
ui-lang-ja = 日本語
ui-lang-en = English
ui-lang-zh-cn = 简体中文
ui-lang-zh-tw = 繁體中文
ui-lang-ko = 한국어
ui-lang-de = Deutsch
ui-lang-fr = Français
ui-lang-es = Español
ui-loading = 読み込み中…
ui-close = 閉じる
ui-cancel = キャンセル
ui-delete = 削除
ui-save = 保存
ui-refresh = 更新
ui-retry = 再試行
ui-yes = はい
ui-no = いいえ
ui-unlimited = 無制限
ui-help = キーボードショートカット
ui-help-title = キーボードショートカット
ui-skip-to-content = 本文へスキップ
ui-not-found = このページは存在しません。
ui-back-to-dashboard = ダッシュボードへ戻る
ui-view-only = 狭い画面では閲覧のみです。編集は広い画面で行ってください。
ui-server-unreachable = サーバに接続できません。`wpg serve` は起動していますか？
ui-error-http = サーバが HTTP { $id } を返しました。
ui-error-network = 通信エラー: { $error }
ui-error-decode = サーバの応答を解釈できません: { $error }
ui-nav-label = メインナビゲーション
ui-nav-dashboard = ダッシュボード
ui-nav-builder = ビルダー
ui-nav-jobs = ジョブ
ui-nav-templates = テンプレート
ui-nav-rx = 受信
ui-nav-settings = 設定
ui-shortcut-hint = でショートカット一覧
ui-shortcut-dashboard = ダッシュボードへ移動
ui-shortcut-builder = ビルダーへ移動
ui-shortcut-jobs = ジョブへ移動
ui-shortcut-templates = テンプレートへ移動
ui-shortcut-rx = 受信へ移動
ui-shortcut-settings = 設定へ移動
ui-shortcut-new-job = 新しいジョブ
ui-shortcut-logs = サーバログの表示 / 非表示
ui-shortcut-help = このヘルプを表示
ui-shortcut-escape = ダイアログを閉じる
ui-ws-connecting = 接続中…
ui-ws-open = ライブ
ui-ws-closed = 切断 (再接続中)
ui-theme = テーマ
ui-theme-light = ライト
ui-theme-dark = ダーク
ui-theme-system = システム設定に従う
ui-theme-to-dark = ダークテーマに切り替え
ui-theme-to-light = ライトテーマに切り替え
ui-logout = ログアウト
ui-logs-toggle = サーバログ
ui-logs-title = サーバログ
ui-logs-level = レベル
ui-logs-follow = 自動スクロール
ui-logs-clear = ログを消去
ui-logs-count = { $count } 行
ui-logs-empty = ログはまだありません。
ui-login-title = ログイン
ui-login-submit = ログイン
ui-login-busy = ログイン中…
ui-login-wrong = パスワードが違います。
ui-login-rate-limited = 試行回数が多すぎます。{ $secs } 秒後に再試行してください。
ui-login-wait = { $secs } 秒待機
ui-password = パスワード
ui-password-confirm = パスワード (確認)
ui-setup-title = 管理パスワードの設定
ui-setup-intro = このサーバにはまだパスワードがありません。Web UI と API を保護するパスワードを設定してください。
ui-setup-hint = { $min } 文字以上。
ui-setup-too-short = パスワードは { $min } 文字以上にしてください。
ui-setup-mismatch = パスワードが一致しません。
ui-setup-submit = パスワードを設定
ui-dash-totals = 合計
ui-dash-active-jobs = アクティブなジョブ
ui-dash-running-jobs = 送出中
ui-dash-total-pps = 合計 pps
ui-dash-total-bps = 合計ビットレート
ui-dash-templates = テンプレート
ui-dash-uptime = サーバ稼働時間
ui-dash-running = 実行中のジョブ
ui-dash-no-running = 実行中のジョブはありません。
ui-dash-doctor = 自己診断
ui-dash-ifaces = インタフェース
ui-doctor-ok = すべて正常
ui-doctor-warn = 警告あり
ui-doctor-fail = 問題あり
ui-doctor-counts = 正常 { $ok } · 警告 { $warn } · 失敗 { $fail }
ui-doctor-details = 詳細レポート
ui-doctor-status = 状態
ui-doctor-check = 項目
ui-doctor-detail = 詳細 / 対処
ui-doctor-status-ok = 正常
ui-doctor-status-warn = 警告
ui-doctor-status-fail = 失敗
ui-doctor-status-skip = スキップ
ui-iface-name = 名前
ui-iface-state = 状態
ui-iface-mac = MAC
ui-iface-addrs = アドレス
ui-iface-mtu = MTU
ui-iface-speed = 速度
ui-iface-allowed = 送出許可
ui-iface-up = Up
ui-iface-down = Down
ui-iface-no-carrier = リンクなし
ui-stat-pps = パケット/秒
ui-stat-bps = ビットレート (L2)
ui-stat-l1-bps = ビットレート (L1)
ui-stat-packets = パケット数
ui-stat-bytes = バイト数
ui-stat-dropped = ドロップ
ui-stat-ratio = レート達成率
ui-stat-elapsed = 経過時間
ui-spark-pps = 直近 30 秒のパケット/秒
ui-state-created = 作成済み
ui-state-scheduled = 開始待ち
ui-state-running = 実行中
ui-state-paused = 一時停止
ui-state-finished = 終了
ui-state-failed = 失敗
ui-state-unknown = 不明
ui-reason-count-reached = 指定パケット数に到達
ui-reason-duration-elapsed = 指定時間が経過
ui-reason-source-exhausted = 全パケット送出済み
ui-reason-stopped = 停止
ui-jobs-new = 新しいジョブ
ui-jobs-tab-live = 現在
ui-jobs-tab-history = 履歴
ui-jobs-empty = ジョブはありません。
ui-jobs-streams = ストリーム
ui-jobs-template = テンプレート
ui-jobs-choose-template = テンプレートを選択…
ui-jobs-weight = 重み
ui-jobs-add-stream = ストリームを追加
ui-jobs-remove-stream = ストリームを削除
ui-jobs-mix = 混合方式
ui-mix-weighted = 重み付き
ui-mix-sequential = 順番
ui-mix-round-robin = ラウンドロビン
ui-jobs-target = 送出先
ui-jobs-backend = バックエンド
ui-jobs-iface = インタフェース
ui-jobs-choose-iface = インタフェースを選択…
ui-jobs-iface-not-needed = このバックエンドでは不要
ui-jobs-iface-denied = 許可されていません
ui-jobs-dry-run = ドライラン: pcapng ファイル名
ui-jobs-dry-run-hint = 指定すると送出せず、サーバのキャプチャディレクトリ内のこのファイルにパケットを書き出します。
ui-jobs-how = 送出方法
ui-jobs-rate = レート
ui-jobs-count = パケット数
ui-jobs-duration = 送出時間
ui-jobs-burst = バースト
ui-jobs-workers = ワーカ数
ui-jobs-workers-disabled = 複数ワーカにはマルチワーカ対応のバックエンドと単一ストリームが必要です。
ui-jobs-from-template = テンプレートの設定
ui-jobs-name = 名前
ui-jobs-name-placeholder = 省略時はテンプレート名
ui-jobs-start-now = すぐに開始する
ui-jobs-create = ジョブを作成
ui-jobs-create-start = 作成して開始
ui-jobs-created = ジョブ「{ $id }」を作成しました。
ui-jobs-need-iface = インタフェースを選択してください。
ui-jobs-need-template = すべてのストリームでテンプレートを選択してください。
ui-jobs-bad-weight = 重みは正の整数で指定してください。
ui-rate-default = 空欄: テンプレートのレート (またはサーバの上限)
ui-rate-is-pps = → { $rate }
ui-rate-is-bps = → 回線上 { $rate } (レイヤ 1)
ui-rate-is-percent = → リンク速度の { $rate } %
ui-rate-is-ifg = フレーム間隔を固定: { $rate }
ui-rate-is-unlimited = 可能な限り高速に送出
ui-valid-empty = 必須項目です。
ui-valid-number = 数値ではありません。
ui-valid-integer = 整数を入力してください。
ui-valid-range = 0 より大きい値にしてください。
ui-valid-percent = パーセントは 100 以下にしてください。
ui-valid-rate-unit = 不明な単位です: pps, kpps, Mpps, bps, kbps, Mbps, Gbps, %, ifg=<時間>, unlimited のいずれかを使ってください。
ui-valid-duration-unit = 不明な単位です: ns, us, ms, s, m, h のいずれかを使ってください。
ui-valid-capture-name = 「..」を含まない相対ファイル名にしてください。
ui-confirm-send-title = 実インタフェースへ送出しますか？
ui-confirm-send-body = このジョブはネットワークへパケットを送出します。インタフェースとレートがこのネットワークに適切か確認してください。
ui-confirm-send = 送出する
ui-job-detail = ジョブ
ui-job-state = 状態
ui-job-start = 開始
ui-job-pause = 一時停止
ui-job-resume = 再開
ui-job-stop = 停止
ui-job-download-report = レポート (JSON)
ui-job-live = ライブ統計
ui-job-streams = ストリーム別
ui-job-inline-template = (インラインテンプレート)
ui-job-workers = ワーカ別
ui-job-workers-pending = ワーカ別カウンタは最終レポートに含まれ、ジョブ終了後に表示されます。
ui-job-active = 稼働時間
ui-job-core = コア
ui-job-accuracy = レート制御精度
ui-acc-target = 目標
ui-acc-achieved = 実績
ui-acc-jitter = ジッタ (平均 / 最大)
ui-job-spec = 設定
ui-job-created = 作成
ui-job-started = 開始
ui-job-finished = 終了
ui-job-deleted = ジョブを削除しました。
ui-job-delete-title = このジョブを削除しますか？
ui-job-delete-body = 実行中の場合は先に停止します。レポートは履歴に残ります。
ui-tpl-import = インポート
ui-tpl-new = 新しいテンプレート
ui-tpl-search = 名前・説明・タグ・プロトコルで検索…
ui-tpl-source = 種別
ui-tpl-source-all = すべて
ui-tpl-source-library = ライブラリ
ui-tpl-source-user = マイテンプレート
ui-tpl-favorites-only = お気に入りのみ
ui-tpl-tags = タグ
ui-tpl-recent = 最近使ったもの
ui-tpl-empty = 該当するテンプレートはありません。
ui-tpl-favorite = お気に入り
ui-tpl-view = YAML を表示 / 編集
ui-tpl-open-builder = ビルダーで開く
ui-tpl-new-job = このテンプレートでジョブを作成
ui-tpl-export = YAML をダウンロード
ui-tpl-duplicate = 複製
ui-tpl-imported = テンプレート「{ $id }」をインポートしました。
ui-tpl-duplicated = 複製「{ $id }」を作成しました。
ui-tpl-deleted = テンプレート「{ $id }」を削除しました。
ui-tpl-delete-title = このテンプレートを削除しますか？
ui-tpl-delete-body = 「{ $id }」をテンプレートディレクトリから削除します。
ui-tpl-saved = テンプレートを保存しました。
ui-tpl-yaml = テンプレート YAML
ui-tpl-read-only = ライブラリのテンプレートは読み取り専用です。編集するには複製してください。
ui-lint-run = 検査 (lint)
ui-lint-clean = 問題は見つかりませんでした。
ui-lint-summary = エラー { $errors } 件、警告 { $warnings } 件
ui-set-appearance = 表示
ui-set-account = アカウント
ui-set-logged-in = { $id } としてログイン中です。
ui-set-auth-disabled = このサーバでは認証が有効になっていません。
ui-set-client = このブラウザ
ui-set-client-note = このブラウザにのみ保存されます。サーバ設定 (wpg.toml) は API では公開されていません。
ui-set-api = API
ui-set-ws = イベント
ui-set-ui-version = UI バージョン
ui-set-api-docs = API ドキュメント (Swagger UI)
ui-set-about = このソフトウェアについて
ui-set-server-version = サーババージョン
ui-set-api-version = API バージョン
ui-set-platform = プラットフォーム
ui-set-features = オプション機能
ui-set-locales = 言語
ui-set-license = wpg は MIT または Apache-2.0 のデュアルライセンスです。
ui-builder-placeholder = パケットビルダー (プロトコルパレット、レイヤスタック、フィールドエディタ、Hex / ツリー表示) は次のステップで提供します。
ui-builder-placeholder-template = パケットビルダーは次のステップで提供します。テンプレート「{ $id }」はここで開けるようになります。
ui-reason-end-of-input = キャプチャファイルの終端
ui-valid-stream-id = 0〜4294967295 の整数を入力してください。
ui-jobs-verification = 受信検証
ui-jobs-signature = 検証用シグネチャを付与
ui-jobs-signature-hint = 各フレームのペイロード末尾 24 オクテットにストリーム ID・シーケンス番号・送信時刻を書き込みます。「シグネチャを検証」を有効にしたキャプチャで損失・順序・遅延を測定できます。
ui-jobs-stream-id = シグネチャのストリーム ID（先頭ストリーム）
ui-job-signature = シグネチャ
ui-job-signature-on = あり（ストリーム ID { $id } から）
ui-rx-new = 新しいキャプチャ
ui-rx-file = キャプチャファイル
ui-rx-file-hint = サーバのキャプチャディレクトリ内の pcap / pcapng ファイル名（ジョブのドライラン出力など）。
ui-rx-iface-any = any（すべてのインターフェース）
ui-rx-filter = フィルタ
ui-rx-filter-empty = 空: すべてのフレームが一致します。
ui-rx-filter-checking = フィルタを確認しています…
ui-rx-filter-ok = 有効: { $filter }
ui-rx-filter-error = { $error }（{ $offset } 文字目）
ui-rx-count-hint = 一致したフレームがこの数に達したら停止します。
ui-rx-keep = 保持するフレーム数
ui-rx-keep-hint = 一致したフレームのうち先頭 N 個を表示用に保持します（最大 10000）。
ui-rx-keep-max = 10000 以下にしてください。
ui-rx-save = pcapng に保存（ファイル名）
ui-rx-save-hint = 任意: 一致したフレームをサーバのキャプチャディレクトリ内のこのファイルにも書き込みます。
ui-rx-save-same = 出力ファイルは入力ファイルと別の名前にしてください。
ui-rx-name-placeholder = 省略時は バックエンド:インターフェース
ui-rx-options = オプション
ui-rx-verify = シグネチャを検証（損失・順序・遅延）
ui-rx-promisc = プロミスキャスモード
ui-rx-outgoing = このホストが送信したフレームも含める
ui-rx-start = キャプチャ開始
ui-rx-started = キャプチャ「{ $id }」を開始しました。
ui-rx-list = キャプチャ
ui-rx-empty = キャプチャはまだありません。
ui-rx-source = インターフェース / ファイル
ui-rx-actions = 操作
ui-rx-stop = 停止
ui-rx-stop-named = 「{ $id }」を停止
ui-rx-delete-named = 「{ $id }」を削除
ui-rx-deleted = キャプチャを削除しました。
ui-rx-delete-title = このキャプチャを削除しますか？
ui-rx-delete-body = 実行中のキャプチャは先に停止します。保存済みの pcapng ファイルはサーバに残ります。
ui-rx-download = pcapng をダウンロード
ui-rx-counters = カウンタ
ui-rx-matched = 一致
ui-rx-matched-pps = 一致 パケット/秒
ui-rx-matched-bps = 一致 ビットレート
ui-rx-matched-bytes = 一致 バイト数
ui-rx-saved = ファイルに保存
ui-rx-kept = 表示用に保持
ui-rx-verify-title = 検証
ui-rx-verify-off = このキャプチャではシグネチャ検証が無効です。
ui-rx-verify-none = シグネチャ付きのフレームはまだ受信していません。
ui-rx-signed = シグネチャあり
ui-rx-unsigned = シグネチャなし
ui-rx-lost = 損失
ui-rx-stream = ストリーム
ui-rx-received = 受信
ui-rx-seq-range = シーケンス番号
ui-rx-out-of-order = 順序逆転
ui-rx-duplicates = 重複
ui-rx-latency = 遅延（最小 / 平均 / 最大）
ui-rx-jitter = ジッタ
ui-rx-latency-note = 損失はシーケンス番号の欠番から推定した値です。遅延は送信側と受信側が同じ時計を共有している場合（同一ホスト、または PTP / NTP で同期）にのみ意味があります。
ui-rx-frames = 受信フレーム
ui-rx-frames-empty = 一致したフレームはまだありません。
ui-rx-time = 時刻（秒）
ui-rx-length = 長さ
ui-rx-signature = シグネチャ
ui-rx-sig-value = ストリーム { $stream } · seq { $seq }
ui-rx-show-more = さらに表示（{ $max } 件中 { $count } 件）
ui-rx-frame-title = フレーム { $index }
ui-rx-select-frame = フレームを選択すると、オクテットとデコードしたレイヤを表示します。
ui-rx-decode-failed = フレームをデコードできません: { $error }
ui-rx-truncated = { $len } オクテット中 { $bytes } オクテットのみキャプチャしました。

# --- P1 L2/encap protocol field descriptions ---
lldp-tlvs = LLDP TLV 群 (7 ビット型 + 9 ビット長ヘッダ)。
cdp-version = CDP プロトコルバージョン (2)。
cdp-ttl = このアドバタイズの有効時間 (秒)。
cdp-checksum = CDP ヘッダと TLV に対するインターネットチェックサム (自動)。
cdp-tlvs = CDP TLV 群 (16 ビット型、ヘッダを含む 16 ビット長)。
stp-protocol-id = スパニングツリープロトコル識別子 (常に 0)。
stp-version = プロトコルバージョン (0 = STP、2 = RSTP、3 = MSTP)。
stp-type = BPDU 種別 (0 = Configuration、2 = RST/MST、0x80 = TCN)。
stp-flags = BPDU フラグ (トポロジ変更、Proposal、ロール、Learning、Forwarding、Agreement、TCA)。
stp-root-priority = ルートブリッジの優先度とシステム ID 拡張。
stp-root-mac = ルートブリッジの MAC アドレス。
stp-root-path-cost = ルートブリッジまでの経路コスト。
stp-bridge-priority = 送信ブリッジの優先度とシステム ID 拡張。
stp-bridge-mac = 送信ブリッジの MAC アドレス。
stp-port-id = 送信ポート識別子 (優先度 + 番号)。
stp-message-age = BPDU の経過時間 (1/256 秒)。
stp-max-age = タイムアウトまでの最大 BPDU 経過時間 (1/256 秒)。
stp-hello-time = Configuration BPDU の送信間隔 (1/256 秒)。
stp-forward-delay = 転送遅延タイマ (1/256 秒)。
stp-trailer = RSTP / MSTP 拡張 (version1 長、v3 長、MSTI レコード)。不透明オクテット。
lacp-subtype = Slow Protocols サブタイプ (1 = LACP)。
lacp-actor-system = Actor システムの MAC アドレス。
lacp-actor-state = Actor 状態フラグ (Activity、Timeout、Aggregation、Sync、Collecting、Distributing、Defaulted、Expired)。
lacp-partner-system = Partner システムの MAC アドレス。
lacp-partner-state = Partner 状態フラグ。
marker-subtype = Slow Protocols サブタイプ (2 = Marker)。
eapol-version = EAPOL プロトコルバージョン。
eapol-type = EAPOL パケット種別 (0 = EAP、1 = Start、2 = Logoff、3 = Key)。
eapol-length = EAPOL ボディの長さ (オクテット、自動)。
eapol-body = EAPOL ボディ (種別 0 では EAP パケット)。不透明オクテット。
pppoe-version = PPPoE プロトコルバージョン (1)。
pppoe-type = PPPoE タイプ (1)。
pppoe-code = PPPoE コード (PADI / PADO / PADR / PADS / PADT、0 = セッションデータ)。
pppoe-session-id = PPPoE セッション識別子。
pppoe-length = PPPoE ペイロードの長さ (オクテット、自動)。
pppoe-tags = PPPoE ディスカバリ TAG (16 ビット型、16 ビット長)。
ppp-protocol = ペイロードの PPP プロトコル識別子 (LCP / IPCP / IP / IPv6 など)。
ppp-cp-code = PPP コントロールプロトコルのコード (Configure-Request / Ack / Nak など)。
ppp-cp-identifier = PPP コントロールプロトコルの要求識別子。
ppp-cp-length = PPP コントロールプロトコルパケットの長さ (オクテット、自動)。
ppp-cp-options = PPP コントロールプロトコルのオプション (8 ビット型 / 長の TLV)。
vxlan-flags = VXLAN フラグ (I ビット = VNI 有効)。
vxlan-vni = 24 ビット VXLAN ネットワーク識別子。
gre-flags = GRE フラグとバージョン (checksum / key / sequence の存在ビット)。
gre-protocol-type = カプセル化ペイロードの EtherType。
nvgre-vsid = 24 ビット NVGRE 仮想サブネット ID。
nvgre-flowid = NVGRE FlowID (負荷分散用エントロピー)。
mpls-label = 20 ビット MPLS ラベル値。0〜15 は予約 (0 / 2 Explicit Null、1 Router Alert、3 Implicit Null、7 エントロピーラベル指示子、13 GAL、14 OAM Alert、15 拡張)。SR-MPLS の SID は SRGB ベース + SID インデックス。
mpls-tc = 3 ビットトラフィッククラス (QoS / ECN)。
mpls-s = スタック最下位ビット (1 = 最後のラベル。次レイヤから自動)。
mpls-ttl = MPLS TTL。
gtpu-version = GTP バージョン (1)。
gtpu-pt = プロトコルタイプ (1 = GTP、0 = GTP')。
gtpu-e = 拡張ヘッダ存在フラグ。
gtpu-s = シーケンス番号存在フラグ。
gtpu-pn = N-PDU 番号存在フラグ。
gtpu-message-type = GTP-U メッセージ種別 (255 = G-PDU)。
gtpu-length = 必須ヘッダ以降のペイロード長 (オクテット、自動)。
gtpu-teid = トンネルエンドポイント識別子。
geneve-version = Geneve バージョン (0)。
geneve-opt-len = オプション領域の長さ (4 オクテット単位)。
geneve-o = OAM パケットフラグ。
geneve-c = クリティカルオプション存在フラグ。
geneve-protocol-type = カプセル化ペイロードの EtherType。
geneve-vni = 24 ビット Geneve 仮想ネットワーク識別子。
geneve-options = Geneve オプション (不透明、opt_len で長さ決定)。
l2tp-flags = L2TP フラグとバージョン (type、length、sequence、offset、priority ビット)。
l2tp-body = L2TP ボディ (存在条件付きフィールド + ペイロード)。不透明オクテット。
l2tpv3-session-id = L2TPv3 セッション ID。

# --- パケットビルダー (M4 phase 2) ---
ui-bld-palette = プロトコル
ui-bld-search = プロトコルを検索
ui-bld-favorite = お気に入り
ui-bld-favorites = お気に入り
ui-bld-layer-l2 = データリンク (L2)
ui-bld-layer-l3 = ネットワーク (L3)
ui-bld-layer-l4 = トランスポート (L4)
ui-bld-layer-l7 = アプリケーション (L7)
ui-bld-layer-payload = ペイロード
ui-bld-stack = レイヤ
ui-bld-empty = ここにプロトコルをドラッグするか、パレットから追加してください。
ui-bld-unknown = 不明
ui-bld-dup = レイヤを複製
ui-bld-suggest = 次のレイヤを追加
ui-bld-binding-warn = 下位レイヤから示されていません
ui-bld-fix = 修正
ui-bld-added = { $id } を追加しました。
ui-bld-moved = { $index } 番目に移動しました。
ui-bld-removed = レイヤを削除しました。
ui-bld-fields = フィールド編集
ui-bld-select-layer = 編集するレイヤを選択してください。
ui-bld-auto = 自動
ui-bld-manual = 手動
ui-bld-gen = 生成
ui-bld-computed = ビルド時に計算
ui-bld-nsap-parts = AFI { $afi } · エリア { $area } · システム ID { $sys } · NSEL { $nsel }
ui-bld-estimate = ストリームのパケット数と合計サイズ
ui-bld-expands = { $count } 個（約 { $bytes }）
ui-bld-rate = レート
ui-bld-rate-hint = テンプレートに保存する送出レート (1000pps, 1.5Mpps, 10Gbps, ifg=10us)
ui-bld-duration = 送出 約 { $time }
ui-bld-duration-unknown = 所要時間: レートを指定してください
ui-bld-duration-hint = 送出パケット数 (tx.count、なければストリーム長) をこのレートで送る時間。tx.duration_secs で上限
ui-bld-gen-kind = 種類
ui-bld-gen-count = 個数:
ui-bld-gen-shorthand = 範囲 / リスト / CIDR
ui-bld-gen-random = ランダム
ui-bld-gen-random-mac = ランダム MAC
ui-bld-gen-counter = カウンタ
ui-bld-gen-file = ファイル
ui-bld-gen-expr = 式
ui-bld-hex = Hex
ui-bld-tree = ツリー
ui-bld-no-bytes = まだパケットがありません。
ui-bld-name = テンプレート名
ui-bld-need-name = 先にテンプレート名を入力してください。
ui-bld-unsaved = 未保存の変更
ui-bld-save-as = 名前を付けて保存…
ui-bld-save-first = ジョブを作成する前にテンプレートを保存してください。
ui-bld-recompute = 再計算
ui-bld-auto-recompute = 編集のたびにプレビューを自動再計算
ui-bld-index = ストリーム位置
ui-bld-index-prev = 前のパケット
ui-bld-index-next = 次のパケット
ui-bld-undo = 元に戻す
ui-bld-redo = やり直し
ui-bld-parity-ok = ブラウザ内プレビューはサーバのレンダリングと一致します。
ui-bld-parity-ok-short = 一致
ui-bld-parity-bad = ブラウザ内プレビューがサーバのレンダリングと一致しません。
ui-bld-parity-bad-short = 不一致
ui-bld-parity-pending = サーバのレンダリングを確認中…
ui-bld-help-title = ビルダーのショートカット
ui-bld-help-add = フォーカス中のプロトコルを追加
ui-bld-help-move = 選択中のレイヤを移動
ui-bld-help-del = 選択中のレイヤを削除
ui-bld-help-undo = 元に戻す
ui-bld-help-redo = やり直し
ui-bld-help-hex = キャレット位置のバイトを Hex 入力
ui-bld-help-caret = Hex キャレットを移動
ui-bld-suggest-prev = 上位レイヤを追加

# --- P1 冗長 / マルチキャストプロトコルのフィールド説明 ---
vrrp-version = VRRP バージョン (2 = RFC 3768、3 = RFC 5798)。
vrrp-type = VRRP パケットタイプ (1 = Advertisement)。
vrrp-vrid = 仮想ルータ識別子 (VRID)。
vrrp-priority = 送信ルータの優先度 (255 = アドレス所有者、0 = 離脱)。
vrrp-count = 広告に含まれる仮想アドレス数 (Auto)。
vrrp-auth-type = VRRPv2 認証タイプ (0 = なし、1 = 平文、2 = IP AH)。
vrrp-adver-int = VRRPv2 広告間隔 (秒)。
vrrp-max-adver-int = VRRPv3 最大広告間隔 (センチ秒)。
vrrp-checksum = VRRP メッセージのインターネットチェックサム (VRRPv3 は IP 疑似ヘッダを含む) (Auto)。
vrrp-addresses = 仮想ルータアドレス。
vrrp-auth-data = VRRPv2 認証データ (8 オクテット)。
hsrp-version = HSRP バージョン (HSRPv1 は 0)。
hsrp-op-code = HSRP オペレーション (0 = Hello、1 = Coup、2 = Resign、3 = Advertise)。
hsrp-state = 送信者の状態 (1 = Learn、2 = Listen、4 = Speak、8 = Standby、16 = Active)。
hsrp-hello-time = Hello 送信間隔 (秒)。
hsrp-hold-time = Hello の有効時間 (秒)。
hsrp-priority = グループ内のルータ優先度。
hsrp-group = スタンバイグループ番号。
hsrp-auth-data = 平文認証パスワード (8 オクテット、既定値 "cisco")。
hsrp-virtual-ip = グループの仮想 IPv4 アドレス。
hsrpv2-tlvs = HSRPv2 TLV (Group State、Interface State、テキスト / MD5 認証。8 ビットのタイプと長さ)。
glbp-version = GLBP バージョン (1)。
glbp-group = GLBP グループ番号。
glbp-owner-id = 送信ルータの MAC アドレス。
glbp-tlvs = GLBP TLV (Hello、Request/Response、認証。長さはヘッダを含む)。
igmp-type = IGMP メッセージタイプ (ボディから Auto: 0x11 クエリ、0x12 / 0x16 レポート、0x17 離脱、0x22 v3 レポート)。
igmp-max-resp-code = 最大応答時間 / コード (1/10 秒単位、クエリ用。レポートでは 0)。
igmp-checksum = IGMP メッセージ全体のインターネットチェックサム (Auto)。
igmp-group = マルチキャストグループアドレス (一般クエリでは 0.0.0.0)。
igmp-s-flag = ルータ側処理抑止フラグ。
igmp-qrv = クエリアのロバストネス変数 (QRV)。
igmp-qqic = クエリアのクエリ間隔コード (QQIC)。
igmp-num-sources = 送信元アドレス数。
igmp-sources = 送信元アドレス。
igmp-num-records = グループレコード数。
igmp-records = グループレコード (レコードタイプ、補助データ長、送信元、マルチキャストアドレス)。
mld-max-resp = 最大応答遅延 / コード (ミリ秒)。
mld-address = IPv6 マルチキャストアドレス (一般クエリでは ::)。
pim-version = PIM バージョン (2)。
pim-type = PIM メッセージタイプ (ボディから Auto: 0 Hello、1 Register、3 Join/Prune)。
pim-checksum = インターネットチェックサム (メッセージ全体。Register は先頭 8 オクテットのみ) (Auto)。
pim-hello-options = PIM Hello オプション (Holdtime、LAN Prune Delay、DR Priority、Generation ID、Address List)。
pim-addr-family = エンコードアドレスのアドレスファミリ (1 = IPv4)。
pim-upstream-neighbor = Join/Prune の宛先となる上流ネイバーアドレス。
pim-num-groups = グループセット数。
pim-holdtime = Join/Prune 状態の保持時間 (秒)。
pim-groups = グループセット (エンコードグループ、Join / Prune するエンコード送信元)。
pim-register-border = Border ビット (送信者が PIM マルチキャスト境界ルータ)。
pim-register-null = Null-Register ビット (カプセル化データパケットなし)。

# --- P1 application protocol field descriptions ---
dhcp-op = BOOTP メッセージ種別 (1 = BOOTREQUEST、2 = BOOTREPLY)。
dhcp-htype = ハードウェアアドレス種別 (1 = Ethernet)。
dhcp-hlen = ハードウェアアドレス長 (オクテット、Ethernet は 6)。
dhcp-hops = リレーホップ数 (リレーエージェントごとに加算)。
dhcp-xid = クライアントが選ぶトランザクション ID。
dhcp-secs = クライアントが取得を開始してからの経過秒数。
dhcp-flags = BOOTP フラグ (ビット 15 = ブロードキャスト応答を要求)。
dhcp-ciaddr = クライアント IP アドレス (割り当て済みの場合)。
dhcp-yiaddr = サーバが割り当てるクライアント ("your") IP アドレス。
dhcp-siaddr = 次に使うサーバ (ブートサーバ) の IP アドレス。
dhcp-giaddr = リレーエージェント (ゲートウェイ) の IP アドレス。リレー時は非 0。
dhcp-chaddr = クライアントのハードウェア (MAC) アドレス。後ろに 10 オクテットのパディング。
dhcp-sname = サーバホスト名 (任意、64 オクテット、NUL 埋め)。
dhcp-file = ブートファイル名 (128 オクテット、NUL 埋め)。
dhcp-magic-cookie = DHCP マジッククッキー (0x63825363)。
dhcp-options = DHCP オプション (8 ビットコード / 長さ。Pad、End、Message Type、Relay Agent Information など)。
dhcpv6-msg-type = DHCPv6 メッセージ種別 (Solicit、Advertise、Request、Reply、Relay-Forward など)。
dhcpv6-transaction-id = 24 ビットのトランザクション ID。
dhcpv6-options = DHCPv6 オプション (16 ビットコード / 長さ。Client ID、IA_NA、Relay Message など)。
dhcpv6-hop-count = このメッセージを中継したリレーエージェントの数。
dhcpv6-link-address = クライアントのリンクを識別するアドレス (リレー)。
dhcpv6-peer-address = メッセージの送信元クライアントまたはリレーのアドレス。
dns-id = クエリ識別子 (応答にコピーされる)。
dns-qr = クエリ (0) または応答 (1)。
dns-opcode = クエリ種別 (0 = 標準クエリ、4 = NOTIFY、5 = UPDATE)。
dns-aa = 権威応答フラグ。
dns-tc = 切り詰め (トランケーション) フラグ。
dns-rd = 再帰要求フラグ。
dns-ra = 再帰可能フラグ。
dns-z = 予約ビット (0 でなければならない)。
dns-ad = 認証済みデータフラグ (DNSSEC)。
dns-cd = 検証無効フラグ (DNSSEC)。
dns-rcode = 応答コード (0 = NOERROR、3 = NXDOMAIN など)。
dns-qdcount = 質問セクションのエントリ数。
dns-ancount = 回答セクションのリソースレコード数。
dns-nscount = 権威セクションのリソースレコード数。
dns-arcount = 追加セクションのリソースレコード数。
dns-sections = 質問 / 回答 / 権威 / 追加セクション (長さ付きラベル形式の名前)。不透明なオクテット列。
ntp-li = うるう秒指示子 (3 = 時刻未同期)。
ntp-version = NTP バージョン番号 (4)。
ntp-mode = アソシエーションモード (3 = クライアント、4 = サーバ、5 = ブロードキャスト)。
ntp-stratum = 階層 (0 = 未指定、1 = 一次参照、2-15 = 二次)。
ntp-poll = 最大ポーリング間隔 (log2 秒、符号付き)。
ntp-precision = クロック精度 (log2 秒、符号付き)。
ntp-root-delay = 参照クロックまでの往復遅延 (16.16 固定小数点秒)。
ntp-root-dispersion = 参照クロックまでの分散 (16.16 固定小数点秒)。
ntp-reference-id = 参照 ID (階層 1 は ASCII コード、それ以外は IPv4 アドレスまたはハッシュ)。
ntp-reference-ts = システムクロックを最後に設定した時刻 (64 ビット NTP タイムスタンプ)。
ntp-origin-ts = サーバが返すクライアント送信時刻 (64 ビット NTP タイムスタンプ)。
ntp-receive-ts = 要求がサーバに到着した時刻 (64 ビット NTP タイムスタンプ)。
ntp-transmit-ts = パケットが送信元を出た時刻 (64 ビット NTP タイムスタンプ)。
ntp-extensions = 拡張フィールドと MAC。不透明なオクテット列。
snmp-tag = BER タグオクテット (0x30 SEQUENCE、0x02 INTEGER、0x04 OCTET STRING)。
snmp-msg-length = SNMP メッセージ SEQUENCE の BER 長 (自動。短形式、128 未満)。
snmp-version-length = バージョン INTEGER の BER 長 (1)。
snmp-version = SNMP バージョン (0 = v1、1 = v2c)。
snmp-community-length = コミュニティ文字列の BER 長 (オクテット)。
snmp-community = コミュニティ文字列 (例: "public")。
snmp-pdu-type = PDU タグ (0xa0 GetRequest、0xa2 Response、0xa4 v1 Trap、0xa7 SNMPv2-Trap など)。
snmp-pdu-length = PDU 内容の BER 長 (自動。短形式、128 未満)。
snmp-pdu-body = BER TLV で表した PDU 内容: request-id、error-status、error-index と variable-bindings SEQUENCE。
snmp-trap-body = BER TLV で表した Trap 内容: enterprise、agent-addr、generic-trap、specific-trap、time-stamp と variable-bindings。
syslog-message = <PRI> (ファシリティ * 8 + 重大度) で始まる Syslog メッセージ本文。
radius-code = RADIUS パケット種別 (Access-Request、Access-Accept、Accounting-Request など)。
radius-identifier = 要求と応答を対応付ける識別子。
radius-length = RADIUS パケット全体の長さ (オクテット、自動)。
radius-authenticator = 16 オクテットの Request / Response Authenticator (手動)。
radius-attributes = RADIUS 属性 (8 ビット型、ヘッダを含む 8 ビット長)。
tftp-opcode = TFTP オペコード (1 = RRQ、2 = WRQ、3 = DATA、4 = ACK、5 = ERROR、6 = OACK)。
tftp-filename = ファイル名 (NUL 終端)。
tftp-mode = 転送モード: netascii、octet、mail (NUL 終端)。
tftp-options = オプション名 / 値の文字列ペア (RFC 2347): blksize、timeout、tsize、windowsize など。
tftp-block = ブロック番号 (1 から開始)。
tftp-data = データオクテット (ブロックあたり 512。短いブロックで転送終了)。
tftp-error-code = TFTP エラーコード (1 = ファイルなし、2 = アクセス違反 など)。
tftp-error-message = エラーメッセージ (NUL 終端)。

# --- P1 ルーティングプロトコルのフィールド説明 ---
ospf-version = OSPF バージョン (2 = OSPFv2、3 = OSPFv3)。
ospf-type = OSPF パケットタイプ (Hello / DB Description / LS Request / LS Update / LS Ack。次レイヤから自動)。
ospf-packet-length = ヘッダを含む OSPF パケット長 (オクテット、自動)。
ospf-router-id = 送信ルータのルータ ID。
ospf-area-id = パケットが属するエリア (0.0.0.0 = バックボーン)。
ospf-checksum = 認証フィールドを除いた OSPF パケットのインターネットチェックサム (自動)。
ospf-au-type = 認証タイプ (0 = なし、1 = 平文パスワード、2 = 暗号認証)。
ospf-authentication = 64 ビットの認証フィールド (パスワード、または暗号認証のキー ID / シーケンス)。
ospf-network-mask = インタフェースまたは広告ネットワークのネットマスク。
ospf-hello-interval = Hello パケットの送信間隔 (秒)。
ospf-options = オプション機能 (E、MC、N/P、L、DC、O、DN ビット)。
ospf-router-priority = DR 選出のルータ優先度 (0 = DR にならない)。
ospf-router-dead-interval = Hello が途絶えてから隣接ルータをダウンと判断するまでの秒数。
ospf-designated-router = 代表ルータ DR (0.0.0.0 = なし)。
ospf-backup-designated-router = バックアップ代表ルータ BDR (0.0.0.0 = なし)。
ospf-neighbors = リンク上で認識している隣接ルータのルータ ID。
ospf-interface-mtu = フラグメントなしで送信できる最大 IP データグラム長。
ospf-dbd-flags = Database Description フラグ (I = 初期、M = 続きあり、MS = マスタ)。
ospf-dd-sequence = Database Description シーケンス番号。
ospf-lsa-headers = LSA ヘッダ (各 20 オクテット。チェックサムと長さは手動)。
ospf-lsr-requests = 要求する LSA (LS タイプ、リンクステート ID、広告ルータ)。
ospf-num-lsas = アップデートに含まれる LSA の数。
ospf-ls-age = LSA 生成からの経過秒数 (自動チェックサムを正確にするには 0 のまま)。
ospf-ls-type = LSA タイプ (LSA 本体レイヤから自動。9 / 10 / 11 = Opaque LSA)。
ospf-link-state-id = 記述対象を識別するリンクステート ID。
ospf-advertising-router = LSA を生成したルータのルータ ID。
ospf-ls-sequence = LSA シーケンス番号 (0x80000001 = 初期値)。
ospf-ls-checksum = LS age を除いた LSA の Fletcher チェックサム (自動)。
ospf-lsa-length = ヘッダを含む LSA 長 (オクテット、自動)。
ospf-router-lsa-flags = Router-LSA フラグ (V = 仮想リンク端点、E = ASBR、B = ABR)。
ospf-num-links = 記述するルータリンクの数。
ospf-router-links = ルータリンク (リンク ID、リンクデータ、タイプ、TOS 数、メトリック)。
ospf-attached-routers = ネットワークに接続しているルータのルータ ID。
ospf-metric = 経路のコスト。
ospf-external-e = 外部メトリックタイプ (1 = タイプ 2、0 = タイプ 1)。
ospf-forwarding-address = 宛先へのトラフィックの転送先アドレス (0.0.0.0 = ASBR 自身)。
ospf-external-route-tag = 外部経路に付く 32 ビットのタグ。
ospf-opaque-ls-type = Opaque LSA のフラッディング範囲 (9 = リンクローカル、10 = エリア、11 = AS)。
ospf-opaque-type = Opaque タイプ (1 = TE、4 = Router Information、7 = Extended Prefix、8 = Extended Link。本体レイヤから自動)。
ospf-opaque-id = 同じ Opaque タイプの LSA を区別する Opaque ID (24 ビット)。
ospf-opaque-tlvs = Opaque LSA の TLV (16 ビットのタイプと長さ、値は 4 オクテット境界まで詰め物。長さは自動)。
ospfv3-checksum = IPv6 疑似ヘッダを含む OSPFv3 パケットのインターネットチェックサム (自動)。
ospfv3-instance-id = インスタンス ID (同一リンク上の OSPFv3 インスタンスを区別)。
ospfv3-interface-id = 送信インタフェースのインタフェース ID。
ospfv3-options = OSPFv3 オプション (V6、E、N、R、DC、AF ビット)。
ospfv3-ls-type = U / S スコープビット付きの LSA 機能コード (LSA 本体レイヤから自動)。
ospfv3-router-links = ルータインタフェース (タイプ、メトリック、インタフェース ID、隣接ルータ ID)。
ospfv3-prefix-length = プレフィックス長 (ビット)。
ospfv3-prefix-options = プレフィックスオプション (NU、LA、P、DN ビット)。
ospfv3-address-prefix = 4 オクテット境界にパディングしたアドレスプレフィックス。
ospfv3-external-flags = AS-External-LSA フラグ (E = タイプ 2 メトリック、F = 転送アドレスあり、T = ルートタグあり)。
ospfv3-referenced-ls-type = 参照する LSA の LS タイプ。
ospfv3-link-local-address = インタフェースのリンクローカルアドレス。
ospfv3-num-prefixes = 後続のプレフィックス数。
ospfv3-prefixes = プレフィックスエントリ (長さ、オプション、メトリック / 予約、アドレスプレフィックス)。不透明なオクテット列。
ospfv3-referenced-link-state-id = 参照する LSA のリンクステート ID。
ospfv3-referenced-advertising-router = 参照する LSA の広告ルータ。
bgp-marker = 16 オクテットのマーカ (すべて 1)。
bgp-length = ヘッダを含む BGP メッセージ長 (オクテット、自動)。
bgp-type = BGP メッセージタイプ (OPEN / UPDATE / NOTIFICATION / KEEPALIVE / ROUTE-REFRESH。次レイヤから自動)。
bgp-version = BGP バージョン (4)。
bgp-my-as = 送信側の AS 番号 (4 オクテット AS では 23456 = AS_TRANS)。
bgp-hold-time = 提案するホールドタイム (秒。0 または 3 以上)。
bgp-identifier = 送信側の BGP 識別子 (ルータ ID)。
bgp-opt-params-length = オプションパラメータ長 (オクテット、自動)。
bgp-opt-params = オプションパラメータ (タイプ 2 = Capabilities)。
bgp-withdrawn-routes-length = 撤回経路の長さ (オクテット、自動)。
bgp-withdrawn-routes = 撤回する IPv4 経路 (ビット単位のプレフィックス長, プレフィックス)。
bgp-total-path-attribute-length = パス属性の長さ (オクテット、自動)。
bgp-path-attributes = パス属性 (フラグ + タイプコード。Extended Length フラグ付きなら長さは 16 ビットで、255 オクテットを超えると自動で付く)。
bgp-nlri = 広告する IPv4 経路 (ビット単位のプレフィックス長, プレフィックス)。他のファミリは MP_REACH_NLRI に入れる。
bgp-error-code = NOTIFICATION のエラーコード。
bgp-error-subcode = NOTIFICATION のエラーサブコード。
bgp-error-data = NOTIFICATION の診断データ。
bgp-afi = アドレスファミリ識別子 (1 = IPv4、2 = IPv6)。
bgp-rr-subtype = ROUTE-REFRESH サブタイプ (0 = 通常、1 = BoRR、2 = EoRR)。
bgp-safi = サブアドレスファミリ識別子 (1 = ユニキャスト、2 = マルチキャスト)。
rip-command = RIP コマンド (1 = 要求、2 = 応答)。
rip-version = RIP バージョン (1 または 2。RIPng は 1)。
rip-entries = 経路エントリ (AFI、ルートタグ、アドレス、マスク、ネクストホップ、メトリック)。
ripng-entries = 経路テーブルエントリ (プレフィックス、ルートタグ、プレフィックス長、メトリック。0xff = ネクストホップ)。
eigrp-version = EIGRP ヘッダのバージョン (2)。
eigrp-opcode = EIGRP オペコード (Update / Query / Reply / Hello / SIA-Query / SIA-Reply)。
eigrp-checksum = EIGRP パケットのインターネットチェックサム (自動)。
eigrp-flags = EIGRP フラグ (Init、Conditional Receive、Restart、End of Table)。
eigrp-sequence = 信頼性のある配送のためのシーケンス番号。
eigrp-acknowledge = 確認応答するシーケンス番号 (0 = なし)。
eigrp-virtual-router-id = 仮想ルータ ID (0 = ユニキャストアドレスファミリ)。
eigrp-autonomous-system = EIGRP の AS 番号。
eigrp-tlvs = EIGRP TLV (16 ビットタイプ、ヘッダを含む 16 ビット長)。

# --- P2 OSI カプセル化 フィールド説明 ---
osi-nlpid = ネットワーク層プロトコル識別子 (ISO/IEC TR 9577): 0x81 CLNP、0x82 ES-IS、0x83 IS-IS。
osi-data = NLPID 以降の OSI PDU の残り (不透明なオクテット列)。
clnp-nlpid = ネットワーク層プロトコル識別子（0x81 = CLNP）。
clnp-length-indicator = オプションの終わりまでのヘッダ長（オクテット、Auto）。
clnp-version = バージョン / プロトコル ID 拡張（1）。
clnp-lifetime = 残り寿命（500 ms 単位）。
clnp-sp = SP: セグメント化許可。セグメンテーション部が存在します（セグメンテーション部から Auto）。
clnp-ms = MS: 後続セグメントあり。
clnp-er = E/R: PDU が破棄されたらエラーレポートを送る。
clnp-pdu-type = PDU 種別: 0x1c DT（データ）、0x1d MD（マルチキャストデータ）、0x01 ER（エラーレポート）、0x1e ERQ / 0x1f ERP（エコー要求 / 応答）。
clnp-segment-length = ヘッダとデータを含む PDU 全体の長さ（Auto）。
clnp-checksum = ヘッダの ISO 8473 Fletcher チェックサム（Auto）。0 = チェックサム不使用。
clnp-destination = 宛先 NSAP アドレス。
clnp-source = 送信元 NSAP アドレス。
clnp-segmentation = セグメンテーション部（データユニット識別子、セグメントオフセット、全長）。SP が立っているときに存在します。全長は計算しません。
clnp-options = 長さ指示子までのオプション（8 ビットコード / 8 ビット長）: セキュリティ、QoS 維持、優先度、ソースルーティング、経路記録、パディング、破棄理由。
esis-nlpid = ネットワーク層プロトコル識別子（0x82 = ES-IS）。
esis-length-indicator = PDU の長さ（オクテット、Auto）。
esis-version = バージョン / プロトコル ID 拡張（1）。
esis-pdu-type = PDU 種別: 2 ESH（エンドシステム Hello）、4 ISH（中間システム Hello）、6 RD（リダイレクト）。
esis-holding-time = 保持時間（秒）。
esis-checksum = PDU の ISO 8473 Fletcher チェックサム（Auto）。0 = チェックサム不使用。
esis-number-of-source-addresses = 送信元アドレスの数（Auto）。
esis-source-addresses = エンドシステムが受け持つ NSAP アドレス。
esis-net = 中間システムのネットワークエンティティタイトル（リダイレクト: 次ホップがエンドシステムなら空）。
esis-destination = リダイレクト対象の宛先 NSAP アドレス。
esis-bsnpa-length = サブネットワークアドレスの長さ（Auto）。
esis-bsnpa = より良い次ホップのサブネットワークアドレス（MAC アドレスなど）。
esis-options = 長さ指示子までのオプション（8 ビットコード / 8 ビット長）: セキュリティ、QoS 維持、優先度、アドレスマスク、SNPA マスク、推奨 ES 設定タイマ。
esis-data = 固定部以降の PDU（不透明なオクテット列）。
isis-irpd = ドメイン内ルーティングプロトコル識別子（0x83 = IS-IS）。
isis-header-length = 固定ヘッダの長さ（オクテット）。PDU 種別ごとに一定（LAN Hello / LSP 27、P2P Hello 20、CSNP 33、PSNP 17）。
isis-version-ext = バージョン / プロトコル ID 拡張（1）。
isis-id-length = System ID の長さ（0 = 6 オクテット）。
isis-pdu-type = PDU 種別（15/16 LAN Hello L1/L2、17 P2P Hello、18/20 LSP L1/L2、24/25 CSNP、26/27 PSNP）。
isis-version = バージョン（1）。
isis-max-area-addresses = エリアアドレスの最大数（0 = 3）。
isis-pdu-length = PDU 全体の長さ（Auto）。Hello で Manual 値を指定すると Padding TLV でその長さまで埋めます（例: MTU 1500 なら 1497）。
isis-tlvs = TLV（8 ビットコード / 8 ビット長）とそのサブ TLV。
isis-source-id = 送信元の System ID。
isis-holding-time = 保持時間（秒）。
isis-priority = 代表 IS 選出の優先度（0〜127）。
isis-lan-id = LAN ID: 代表 IS の System ID と擬似ノード ID。
isis-circuit-type = 回線種別: 1 = L1、2 = L2、3 = L1/L2。
isis-local-circuit-id = ローカル回線 ID。
isis-remaining-lifetime = LSP の残り寿命（秒、チェックサム対象外）。
isis-lsp-id = LSP ID: System ID、擬似ノード ID、フラグメント番号。
isis-sequence-number = LSP シーケンス番号。
isis-lsp-checksum = LSP ID から PDU 末尾までの Fletcher チェックサム（Auto）。
isis-partition-repair = P ビット: パーティション修復対応。
isis-attached = ATT ビット: 他エリアへの接続（default / delay / expense / error メトリック）。
isis-overload = OL ビット: LSP データベース過負荷。
isis-is-type = IS 種別: 1 = L1、3 = L1/L2。
isis-snp-source-id = 送信元の System ID と回線 ID（0）。
isis-start-lsp-id = CSNP が対象とする最初の LSP ID。
isis-end-lsp-id = CSNP が対象とする最後の LSP ID。
isis-data = 共通ヘッダ以降の PDU（不透明なオクテット列）。

# --- P2 SCTP field descriptions ---
sctp-src-port = 送信元ポート。
sctp-dst-port = 宛先ポート。
sctp-verification-tag = 検証タグ: 相手の Initiate Tag（INIT を含むパケットでは 0）。
sctp-checksum = このフィールドを 0 とした SCTP パケット全体の CRC32c（Auto）。下位オクテットから送出。
sctp-chunks = チャンク（8 ビット型、8 ビットフラグ、4 オクテットのヘッダを含む 16 ビット長、4 オクテット境界までゼロ詰め）: DATA、INIT、INIT ACK、SACK、HEARTBEAT、ABORT、SHUTDOWN、ERROR、COOKIE ECHO / ACK、AUTH、I-DATA、ASCONF、RE-CONFIG、FORWARD TSN、PAD。長さ・詰め物・SACK の件数は Auto。

# --- P2 LDP / RSVP-TE / BFD field descriptions ---
ldp-version = LDP バージョン（1）。
ldp-pdu-length = バージョンと PDU 長を除く PDU の長さ（Auto）。
ldp-lsr-id = LSR ID: LDP 識別子の先頭 4 オクテット。
ldp-label-space = ラベル空間 ID: LDP 識別子の末尾 2 オクテット（0 = プラットフォーム共通）。
ldp-messages = メッセージ（U ビットを含む 16 ビット型、値の 16 ビット長、メッセージ ID、TLV）: Notification、Hello、Initialization、KeepAlive、Address / Address Withdraw、Label Mapping / Request / Withdraw / Release / Abort Request。TLV: FEC、Address List、Hop Count、Path Vector、Generic Label、Status、Common Hello / Session Parameters、トランスポートアドレス、Configuration Sequence Number。長さは Auto。
rsvp-version = RSVP バージョン（1）。
rsvp-flags = フラグ（ビット 0: リフレッシュ削減対応、RFC 2961）。
rsvp-msg-type = メッセージ種別（1 Path、2 Resv、3 PathErr、4 ResvErr、5 PathTear、6 ResvTear、7 ResvConf、12 Bundle、13 Ack、15 Srefresh、20 Hello）。
rsvp-checksum = このフィールドを 0 としたメッセージ全体のインターネットチェックサム（Auto）。
rsvp-send-ttl = メッセージ送出時の IP TTL。
rsvp-reserved = 予約（0）。
rsvp-length = 共通ヘッダを含むメッセージ全体のオクテット長（Auto）。
rsvp-objects = オブジェクト（4 オクテットのヘッダを含む 16 ビット長、Class-Num、C-Type）: SESSION、RSVP_HOP、TIME_VALUES、ERROR_SPEC、SCOPE、STYLE、FLOWSPEC、FILTER_SPEC、SENDER_TEMPLATE、SENDER_TSPEC、RESV_CONFIRM、LABEL、LABEL_REQUEST、EXPLICIT_ROUTE、RECORD_ROUTE、HELLO、MESSAGE_ID、SESSION_ATTRIBUTE。長さ・セッション名の長さと詰め物は Auto。
bfd-version = BFD バージョン（1）。
bfd-diag = 診断コード: ローカルセッションが最後に状態を変えた理由。
bfd-state = セッション状態: 0 AdminDown、1 Down、2 Init、3 Up。
bfd-p = P ビット: Poll（パラメータ変更。相手は F で応答）。
bfd-f = F ビット: Final（Poll への応答）。
bfd-c = C ビット: コントロールプレーン非依存。
bfd-a = A ビット: 認証あり（Auto: 認証セクションがあれば 1）。
bfd-d = D ビット: デマンドモード。
bfd-m = M ビット: マルチポイント（0）。
bfd-detect-mult = 検出時間の乗数。
bfd-length = 認証セクションを含む制御パケットのオクテット長（Auto）。
bfd-my-discriminator = My Discriminator: 送信側でセッションを一意に表す 0 以外の値。
bfd-your-discriminator = Your Discriminator: 相手の Discriminator（不明な間は 0）。
bfd-desired-min-tx = 希望する最小送信間隔（マイクロ秒）。
bfd-required-min-rx = 要求する最小受信間隔（マイクロ秒）。
bfd-required-min-echo-rx = 要求する最小エコー受信間隔（マイクロ秒、0 = エコーなし）。
bfd-authentication = 認証セクション（8 ビット型、ヘッダを含む 8 ビット長）: 簡易パスワード、Keyed MD5 / SHA1 とその Meticulous 版。ダイジェスト / ハッシュは手動。

# --- 生成ドキュメント (`wpg docs gen`、crates/wpg-cli/src/cmd/docs) ---
docs-gen-no-root = { $path } はリポジトリのルートではありません (docs/en/src/SUMMARY.md がありません)。--root で指定してください。
docs-gen-error = ドキュメントの生成に失敗しました: { $error }
docs-gen-stale = 古い: { $path }
docs-gen-extra = 生成対象外: { $path }
docs-gen-up-to-date = 生成ドキュメントは最新です。
docs-gen-run-hint = `just docs-gen` (または `wpg docs gen`) を実行し、結果をコミットしてください。
docs-gen-done = ドキュメントを生成しました: 書き込み { $written } ファイル、削除 { $removed } ファイル。
docgen-yes = はい
docgen-octets = { $octets } オクテット
docgen-items = { $count } 要素
docgen-variant = バリアント { $n }
docgen-col-id = ID
docgen-col-name = 名前
docgen-col-fields = フィールド数
docgen-col-next-by = 次レイヤの判別
docgen-col-notes = 補足
docgen-col-lower = 下位レイヤ
docgen-col-selector = 判別フィールド
docgen-col-value = 値
docgen-col-protocol = プロトコル
docgen-col-note = 備考
docgen-col-level = レベル
docgen-col-rule = 規則
docgen-col-field = フィールド
docgen-col-type = 型
docgen-col-default = 既定値
docgen-col-auto = Auto
docgen-col-description = 説明
docgen-col-bit = ビット
docgen-col-mask = マスク
docgen-col-type-value = タイプ値
docgen-col-value-fields = 値のフィールド
docgen-col-key = キー
docgen-col-required = 必須
docgen-col-in = 位置
docgen-col-status = ステータス
docgen-col-body = 本文
docgen-col-tag = タグ
docgen-col-endpoint = エンドポイント
docgen-col-summary = 概要
docgen-col-env = 環境変数
docgen-layer-l2 = L2 (データリンク)
docgen-layer-l3 = L3 (ネットワーク)
docgen-layer-l4 = L4 (トランスポート)
docgen-layer-l7 = アプリケーション・制御
docgen-layer-payload = ペイロード
docgen-proto-title = プロトコルリファレンス
docgen-proto-intro = 本章は `wpg-protocols` のプロトコル定義 IR (`GET /api/protocols` で返すカタログ) から自動生成しているため、常にコードと一致します。対象は { $count } プロトコルです。各ページには、ワイヤ順のフィールド (型、既定値、Auto の規則、説明)、そのプロトコルに至る下位レイヤ、次レイヤの対応表、組み立て後に検査する制約、同梱のサンプルテンプレートを載せています。到達経路、手で設定するフィールド、残っている近似は手書きの[プロトコル補足](../protocols/overview.md)を参照してください。定義を変更したら `just docs-gen` で再生成します。
docgen-proto-layer = レイヤ
docgen-proto-header = ヘッダ
docgen-proto-header-fixed = { $octets } オクテット (固定長)
docgen-proto-header-variable = 可変長
docgen-proto-min-frame = 最小フレーム長
docgen-proto-synth = 生成方式
docgen-proto-synth-payload = 組み込みのペイロード生成 (以下のフィールドはワイヤ上のフィールドではなくパラメータ)
docgen-proto-pseudo = 疑似ヘッダ
docgen-proto-pseudo-fields = 上位レイヤのチェックサムに { $src } / { $dst } を提供
docgen-proto-notes = 補足
docgen-proto-final-dst = 最終宛先
docgen-proto-final-dst-text = { $when } が 0 でない間は、{ $list } の要素 0 が下位レイヤの疑似ヘッダの宛先アドレスを置き換えます (RFC 8200 §8.1)
docgen-proto-inbound = 下位レイヤからの到達
docgen-proto-inbound-none = カタログ内にこのプロトコルへ至る下位レイヤはありません。先頭レイヤとして使うか、明示的に積んでください。
docgen-proto-fields = フィールド
docgen-proto-params = パラメータ
docgen-proto-no-fields = フィールドはありません。
docgen-proto-next = 次レイヤ
docgen-proto-next-intro = 次レイヤは { $field } で判別します。このフィールドが Auto のときは、上に積んだプロトコルの値を書き込みます。
docgen-proto-next-alt = デコード時は、最初のフィールドがどの項目にも一致しなければ { $field } も参照します。
docgen-proto-constraints = 制約
docgen-proto-samples = サンプルテンプレート
docgen-proto-samples-none = このプロトコルを使う同梱テンプレートはまだありません。
docgen-proto-samples-more = ほか { $count } 件
docgen-bind-default = (その他の値)
docgen-bind-min-len = 残り { $octets } オクテット以上
docgen-bind-reparse = このレイヤを読み直す
docgen-bind-peek = { $field } = { $value } のときのペイロード先頭 { $bits } ビット
docgen-bind-peek-skip = { $field } = { $value } のときのペイロードの { $skip } ビット目から { $bits } ビット
docgen-level-error = エラー
docgen-level-warning = 警告
docgen-level-info = 情報
docgen-c-range = { $field } が { $min }..{ $max } の範囲
docgen-c-equals = { $field } が { $value }
docgen-c-not-equals = { $field } が { $value } 以外
docgen-c-min-length = { $span }が { $octets } オクテット以上
docgen-c-max-length = { $span }が { $octets } オクテット以下
docgen-span-header = ヘッダ
docgen-span-payload = ペイロード
docgen-span-total = ヘッダとペイロード
docgen-unit-bytes = オクテット
docgen-unit-words16 = 16 ビットワード
docgen-unit-words32 = 32 ビットワード
docgen-unit-words64 = 64 ビットワード
docgen-auto-length = { $span }の長さ ({ $unit }単位)
docgen-auto-checksum = { $covers }に対する{ $algorithm }
docgen-auto-nonzero = 計算結果の 0 は全ビット 1 で送出
docgen-auto-exclude = { $fields } は 0 として計算
docgen-auto-next = 次レイヤのプロトコル番号 (「次レイヤ」参照)
docgen-auto-count = { $field } の要素数
docgen-auto-size = { $field } のオクテット長
docgen-auto-counter = ストリームごとのカウンタ (現状は既定値を使用)
docgen-cksum-internet = インターネットチェックサム
docgen-cksum-internet-pseudo = 疑似ヘッダ付きインターネットチェックサム
docgen-cksum-fletcher = Fletcher チェックサム
docgen-cksum-crc32c = CRC-32C
docgen-cksum-crc32 = CRC-32
docgen-cover-header = ヘッダ
docgen-cover-all = ヘッダとそれ以降すべて
docgen-len-from-field = { $expr } オクテット
docgen-len-count = { $field } 個の要素
docgen-len-until = グループ先頭から { $expr } オクテットまで
docgen-len-to-end = PDU の残りすべて
docgen-len-to-end-except = PDU の残りのうち末尾 { $octets } オクテットを除く部分
docgen-len-until-terminator = 終端 { $octets } まで (組み立て時に付加)
docgen-len-until-tlv = タイプ { $type } の最初の項目まで (なければ PDU の終わりまで)
docgen-tlv-uncounted = 長さの直後にある値の先頭 { $octets } オクテットは長さに数えません。
docgen-list-item = 各要素は以下のフィールドの組です。
docgen-tlv-format = タイプ { $type } ビット、長さ { $len } ビット。
docgen-tlv-len-first = 長さがタイプより前にあります。
docgen-tlv-len-incl = 長さはタイプと長さのフィールドも含みます。
docgen-tlv-len-excl = 長さは値の部分だけを数えます。
docgen-tlv-unit = 長さの単位: { $unit }。
docgen-tlv-head = 値の先頭 { $octets } オクテットはタイプと長さの間にあります。
docgen-tlv-align = 各項目は { $align } オクテットの倍数までゼロで詰めます (長さには含めない)
docgen-tlv-unpadded-last = 最後の項目は詰めない
docgen-tlv-long = タイプのビット { $flag } が立つと長さは { $len } ビットになります。
docgen-tlv-fill = { $field } を Manual にすると、その長さまで { $variant } の項目で詰めます。
docgen-tlv-unknown-raw = 未知のタイプは値をオクテット列のまま保持します。
docgen-tlv-unknown-stop = 未知のタイプで型付きのデコードを終え、残りを 1 つの生の項目にします。
docgen-tlv-type-only = (タイプのみ。長さと値なし)
docgen-tlv-when-prefix = 値が { $octets } で始まるとき
docgen-tlv-when-len = 値が { $octets } オクテットのとき
docgen-notes-heading = 自動生成リファレンス
docgen-notes-intro = このページのプロトコルのフィールドリファレンス (定義から自動生成):
docgen-cli-title = CLI リファレンス
docgen-cli-intro = `wpg` バイナリはサーバ (`wpg serve`) とヘッドレスのツールを兼ねるため、CI や ssh 先での実行にも向いています。例えば `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` でテンプレートを送出します。このページはコマンド定義から自動生成しており、各コードブロックは現行ビルドの `--help` 出力そのものです。実行時のメッセージは `LANG` / `LC_ALL` に従いますが、`--help` の文面は英語です。
docgen-cli-commands = コマンド
docgen-cli-note-send = テンプレートを送出します。`--plan` は、全フレームを先に展開する (`prerender`) か、送出しながら組み立てる (`onthefly`) かを選びます。`--json` は最終レポートを標準出力に JSON で出します。
docgen-cli-note-render = 送出せずに、テンプレートのフレームを pcapng ファイルへ書き出します。
docgen-cli-note-import = キャプチャ (または貼り付けた 16 進) をテンプレートに変換します。[pcap からテンプレートへのチュートリアル](../tutorials/pcap-replay.md)も参照してください。
docgen-cli-note-bench = 合成フレーム (EtherType 0x88b5)、または `--template` で指定したテンプレートをレート無制限で送出します。ベースラインは `docs/perf.md` を参照。`--json` の項目: backend、detail、iface、frame_size、batch_size、packets、dropped、pps、bps (L2)、l1_bps、elapsed。
docgen-cli-note-ifaces = MAC、アドレス、MTU、リンク状態、ドライバ付きでインタフェースを一覧します。
docgen-cli-note-doctor = [権限設定](../getting-started/permissions.md)にある環境チェックを実行します。チェックが fail のとき終了ステータスは 0 以外です。
docgen-cli-note-template = テンプレートのツール。`lint` はエラー時に終了ステータス 1 を返します。`export` と `import` はテンプレートを `.wpgbundle` アーカイブにまとめ、展開します。
docgen-cli-note-config = `wpg.toml` のツール。[設定リファレンス](config.md)を参照してください。
docgen-api-title = API リファレンス
docgen-api-intro = wpg-server は `/api` 以下で REST (JSON) API を提供し、リアルタイム統計・ジョブの状態変化・ログを WebSocket (`GET /api/ws`) で配信します。このページはサーバの OpenAPI ドキュメント (utoipa) から自動生成しています。起動中のサーバは同じドキュメントを `/api/openapi.json` で、Swagger UI を `/api/docs/` で提供し、写しを [`docs/schema/openapi.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/openapi.json) に置いています。`POST /api/auth/login` (初回は `POST /api/auth/setup`) でログインすると、以降は `wpg_session` クッキーで認証されます。POST / PUT / DELETE では `GET /api/auth/status` の CSRF トークンを `X-CSRF-Token` ヘッダでも送ってください。エラーは JSON オブジェクト `error` で、`code`、`message` (`Accept-Language` の言語)、`details` を持ちます。以下のエンドポイントとスキーマの説明はコード由来 (英語) です。
docgen-api-endpoints = エンドポイント
docgen-api-params = パラメータ
docgen-api-body = リクエスト本文
docgen-api-responses = レスポンス
docgen-api-schemas = スキーマ
docgen-api-schemas-intro = 上記で参照しているリクエスト・レスポンス本文の型です。
docgen-api-tag-auth = ログイン、ログアウト、初回セットアップ。
docgen-api-tag-system = ヘルスチェック、バージョン、インタフェース、自己診断、送出バックエンド。
docgen-api-tag-protocols = プロトコルカタログ ([プロトコルリファレンス](protocols.md)参照)。
docgen-api-tag-packets = パケットの合成、デコード、lint。
docgen-api-tag-templates = テンプレートライブラリ。
docgen-api-tag-jobs = 送出ジョブと統計。
docgen-api-tag-events = WebSocket のイベント配信。
docgen-config-title = 設定リファレンス
docgen-config-intro = サーバは作業ディレクトリの `wpg.toml`、または `--config` / `WPG_CONFIG` で指定したファイルを読みます。優先順位は低い順に、組み込みの既定値、ファイル、環境変数 `WPG_<SECTION>__<KEY>` (`__` で階層を区切る。例: `WPG_SERVER__BIND=0.0.0.0:8080`、`WPG_TX__ALLOWED_IFACES='["eth1"]'`)、`wpg serve --bind` などのコマンドラインオプションです。未知のキーはエラーです。`wpg config init` で下記のコメント付きの例を書き出し、`wpg config show` で実効設定を表示し、`wpg config validate` で検査し、`wpg config set-password` で管理者パスワードのハッシュを保存します。管理者パスワードそのものはキーではなく、`wpg serve --password`、`--password-file`、`WPG_PASSWORD` が `auth.password_hash`、`auth.password_file` より優先されます。このページは設定の型から自動生成しています。
docgen-config-keys = キー一覧
docgen-config-top-level = トップレベル
docgen-config-default-machine = OS のデータディレクトリ (Linux: `~/.local/share/wpg`)
docgen-config-example = 設定例 (`wpg config init`)
docgen-config-example-intro = 有効な行はすべて組み込みの既定値と同じなので、編集するまで動作は変わりません。コメントアウトした行は省略可能なキーの例です。
docgen-config-data-dir = サーバの状態を置くディレクトリ: ユーザテンプレート、お気に入り、SQLite データベース (ユーザ、セッション、ジョブ履歴)、ドライランのキャプチャ、自己署名 TLS 証明書。
docgen-config-server = HTTP リスナ。
docgen-config-server-bind = 待ち受けアドレス。パスワードがなく、セットアップ画面でユーザも作っていない間は、ループバックアドレスだけを受け付けます (`--allow-insecure` で解除)。
docgen-config-server-tls = HTTPS で使う証明書チェーンと秘密鍵 (PEM)。`self_signed_tls` より優先します。
docgen-config-server-tls-cert = PEM 形式の証明書チェーン。
docgen-config-server-tls-key = PEM 形式の秘密鍵。
docgen-config-server-self-signed-tls = `<data_dir>/tls/` に生成する自己署名証明書で HTTPS を提供します (次回以降も再利用)。`wpg serve --tls` と同じです。
docgen-config-server-base-url = リバースプロキシ配下で公開する URL (`https://wpg.example.net`)。絶対リンクに使います。
docgen-config-auth = 認証。
docgen-config-auth-password-hash = 管理者パスワードの argon2id PHC 文字列。`wpg config set-password` が書き込みます。
docgen-config-auth-password-file = 管理者パスワードを書いたファイル: argon2 PHC 文字列、または起動時にハッシュする平文。
docgen-config-auth-session-ttl-secs = セッションの有効期間 (秒)。リクエストのたびに延長されます。
docgen-config-templates = テンプレートライブラリ。
docgen-config-templates-library-paths = 読み取り専用のテンプレートライブラリ (Git のチェックアウトなど)。`*.wpg.yaml` / `*.wpg.json` を再帰的に読み込み、ファイルが変わると再読み込みします。
docgen-config-templates-user-dir = GUI で作成・インポートしたテンプレートを書き込むディレクトリ。既定は `<data_dir>/templates`。
docgen-config-protocols = ユーザ定義プロトコル。
docgen-config-protocols-dirs = ユーザ定義プロトコルのファイル (`*.yaml`: 新しいプロトコル定義と既存プロトコルの下への結び付け) を置くディレクトリ。起動時にファイル名順で読み込みます。`wpg serve --protocols-dir` で追加できます。
docgen-config-tx = 送出のポリシーと既定値。
docgen-config-tx-allowed-ifaces = ジョブが送出してよいインタフェース。空ならすべて。`null` バックエンドとドライランは制限しません。
docgen-config-tx-max-rate = ジョブが指定できる最大レート (`100kpps`、`1Gbps`)。レート指定のないジョブはこのレートで送出します。
docgen-config-tx-default-backend = ジョブがバックエンドを指定しないときのバックエンド (`null`、`pcapfile`、`afpacket`、`pcap`、`afxdp`、`dpdk`)。未設定なら Linux は `afpacket`、それ以外は `pcap`、使えなければ `null`。
docgen-config-tx-dpdk-eal-args = `dpdk` バックエンドの DPDK EAL 引数 (例: `["-a", "0000:00:04.0"]`)。指定がなければ wpg が `-l <先頭の CPU>` と `--in-memory` を加えます。EAL はプロセスごとに最初の `dpdk` ジョブで 1 度だけ起動するため、変更には再起動が必要です。
docgen-config-log = ログ。
docgen-config-log-level = フィルタ指定 (`RUST_LOG` が優先)。例: `info`、`wpg_tx=debug,info`。
docgen-config-log-format = 行の形式: `pretty` (人が読む形式) または `json` (1 行に 1 オブジェクト)。
docgen-config-log-file = このファイルにも書き出します (ディレクトリは作成されます)。
docgen-config-log-rotation = ファイルのローテーション: `never`、`hourly`、`daily`。
docgen-config-log-max-files = 残すローテーション済みファイルの最大数 (古いものから削除)。
docgen-config-log-ansi = 端末への出力に色を付けます。
docgen-schema-heading = キーリファレンス (自動生成)
docgen-schema-intro = テンプレート形式の JSON Schema (`wpg template schema`) から自動生成しています。説明文はコードの doc コメント (英語) です。`Template` が文書のルートです。
# --- P2 PTP / ESMC / CFM / R-APS / 802.3ah OAM field descriptions ---
ptp-major-sdo-id = majorSdoId（IEEE 1588-2008 の transportSpecific）。既定プロファイルは 0、IEEE 802.1AS（gPTP）は 1。
ptp-message-type = メッセージタイプ: 0 Sync、1 Delay_Req、2 Pdelay_Req、3 Pdelay_Resp、8 Follow_Up、9 Delay_Resp、10 Pdelay_Resp_Follow_Up、11 Announce、12 Signaling、13 Management。
ptp-minor-version = minorVersionPTP（IEEE 1588-2008 では 0、IEEE 1588-2019 では 1）。
ptp-version = versionPTP（2）。
ptp-message-length = TLV を含むメッセージ全体のオクテット長（自動）。
ptp-domain-number = ドメイン番号。
ptp-minor-sdo-id = minorSdoId（IEEE 1588-2008 では予約、0）。
ptp-flags = フラグ: 1 オクテット目に alternateMaster、twoStep、unicast、プロファイル固有ビット、セキュリティビット、2 オクテット目に leap61、leap59、currentUtcOffsetValid、ptpTimescale、timeTraceable、frequencyTraceable、synchronizationUncertain。
ptp-correction-field = 補正値（ナノ秒 × 2^16、符号付き）。
ptp-message-type-specific = メッセージタイプ固有フィールド（IEEE 1588-2008 では予約、0）。
ptp-clock-identity = 送信元ポートのクロック ID（EUI-64。MAC アドレスの中央に ff:fe を挟んだ値など）。
ptp-port-number = 送信元ポートのポート番号。
ptp-sequence-id = シーケンス ID。Follow_Up / Delay_Resp / Pdelay_Resp は応答先メッセージの値をコピーする。
ptp-control-field = 制御フィールド（IEEE 1588 バージョン 1 互換）: 0 Sync、1 Delay_Req、2 Follow_Up、3 Delay_Resp、4 Management、5 その他。
ptp-log-message-interval = メッセージ間隔（秒）の log2（符号付き。該当しない場合は 0x7f）。
ptp-timestamp-seconds = タイムスタンプ: 秒（48 ビット）。
ptp-timestamp-nanoseconds = タイムスタンプ: ナノ秒（32 ビット）。
ptp-port-identity-clock = ポート ID: クロック ID（全ビット 1 = すべてのクロック）。
ptp-port-identity-port = ポート ID: ポート番号（0xffff = すべてのポート）。
ptp-current-utc-offset = 現在の UTC オフセット（秒、TAI - UTC）。
ptp-grandmaster-priority = グランドマスタの優先度（BMCA、小さいほど優先）。
ptp-grandmaster-clock-class = グランドマスタのクロッククラス（6 = 一次基準に同期、248 = 既定、255 = スレーブ専用）。
ptp-grandmaster-clock-accuracy = グランドマスタのクロック精度（0x20 = 25 ns 以内 … 0xfe = 不明）。
ptp-grandmaster-variance = グランドマスタの offsetScaledLogVariance。
ptp-grandmaster-identity = グランドマスタのクロック ID。
ptp-steps-removed = グランドマスタと送信元の間のバウンダリクロック数。
ptp-time-source = グランドマスタの時刻源: 原子時計、GPS、地上波、PTP、NTP、手動設定、その他、内部発振器。
ptp-starting-boundary-hops = Management メッセージの開始時バウンダリホップ数。
ptp-boundary-hops = 残りのバウンダリホップ数。
ptp-action = Management の動作: GET、SET、RESPONSE、COMMAND、ACKNOWLEDGE。
ptp-tlvs = TLV（16 ビット型、値の 16 ビット長、自動）: Management、Management Error Status、Organization Extension、ユニキャストネゴシエーション（Request / Grant / Cancel / Acknowledge Cancel）、Path Trace、Alternate Time Offset Indicator。メッセージ長で終わる。
ptp-data = 共通ヘッダ以降のメッセージ（メッセージ長まで）をオクテット列で。
esmc-subtype = Slow Protocols サブタイプ（0x0a = 組織固有）。
esmc-itu-oui = ITU-T の OUI（00-19-a7）。
esmc-itu-subtype = ITU-T サブタイプ（1 = ESMC）。
esmc-version = ESMC バージョン（1）。
esmc-event-flag = イベントフラグ: QL 変化時に即座に送るイベント PDU は 1、毎秒の情報 PDU は 0。
esmc-tlvs = TLV（8 ビット型、3 オクテットのヘッダを含む 16 ビット長、自動）: SSM コードの QL TLV、拡張 SSM コード・SyncE クロック ID・カスケードクロック数の拡張 QL TLV。後続のオクテット（詰め物）は 1 つの raw 項目として保持。
cfm-md-level = メンテナンスドメイン / MEG レベル（0〜7）。
cfm-version = CFM バージョン（0。G.8032v2 の R-APS は 1）。
cfm-opcode = OpCode: 1 CCM、2 LBR、3 LBM、4 LTR、5 LTM、33 AIS、35 LCK、37 TST、39 Linear APS、40 R-APS、41 MCC、42 LMR、43 LMM、45 1DM、46 DMR、47 DMM、52 CSF、53 1SL、54 SLR、55 SLM など。
cfm-flags = フラグ（OpCode ごとに異なる）。
cfm-first-tlv-offset = このフィールドの直後から最初の TLV までのオフセット（OpCode ごとに一定）。
cfm-rdi = RDI: リモート障害表示。
cfm-period = 送信周期: 1 = 3.33 ms、2 = 10 ms、3 = 100 ms、4 = 1 秒、5 = 10 秒、6 = 1 分、7 = 10 分。
cfm-sequence-number = シーケンス番号。
cfm-mep-id = MEP ID（1〜8191）。
cfm-maid = MAID / MEG ID（48 オクテット）: MD 名の形式と名前、短縮 MA 名の形式・長さ・名前、ゼロ詰め（Y.1731: MD 名なし、形式 32 ICC ベース、長さ 13）。
cfm-frame-counter = フレームカウンタ（ロス測定）。
cfm-transaction-id = ループバック / リンクトレースのトランザクション ID。
cfm-use-fdb-only = UseFDBonly: 転送データベースだけで出力ポートを決める。
cfm-fwd-yes = FwdYes: LTM を中継した。
cfm-terminal-mep = TerminalMEP: 応答は宛先 MEP から。
cfm-ttl = リンクトレースの TTL。
cfm-relay-action = 中継動作: 1 RlyHit、2 RlyFDB、3 RlyMPDB。
cfm-original-mac = リンクトレースを開始した MEP の MAC アドレス。
cfm-target-mac = リンクトレースの宛先 MAC アドレス。
cfm-csf-type = クライアント信号障害の種類: 0 LOS、1 FDI / AIS、2 RDI、3 DCI。
cfm-timestamp = IEEE 1588 形式のタイムスタンプ（32 ビットの秒とナノ秒）。
cfm-source-mep-id = 送信元 MEP ID。
cfm-responder-mep-id = 応答側 MEP ID（SLM では 0）。
cfm-test-id = 合成ロス測定のテスト ID。
cfm-tlvs = TLV（8 ビット型、値の 16 ビット長、自動）: Sender ID、Port Status、Data、Interface Status、Reply Ingress / Egress、LTM / LTR Egress Identifier、Organization-Specific、Test。End TLV（タイプ 0）で終わる。
cfm-data = First TLV Offset 以降の PDU（OpCode 固有フィールドと TLV）をオクテット列で。
raps-request-state = 要求 / 状態: 0000 NR、0111 MS、1011 SF、1101 FS、1110 Event。
raps-sub-code = サブコード（Event 要求では 0000 = フラッシュ要求）。
raps-rb = RB: RPL がブロック中。
raps-dnf = DNF: 転送データベースをフラッシュしない。
raps-bpr = BPR: ブロック中ポートの参照（0 = リングリンク 0、1 = リングリンク 1）。
raps-node-id = ノード ID: 送信したリングノードの MAC アドレス。
efm-oam-subtype = Slow Protocols サブタイプ（3 = OAM）。
efm-oam-flags = フラグ: リンク障害、Dying Gasp、重大イベント、ローカル / リモートの評価中・安定。
efm-oam-code = OAMPDU コード: 0x00 Information、0x01 Event Notification、0x02 Variable Request、0x03 Variable Response、0x04 Loopback Control、0xfe Organization Specific。
efm-oam-information-tlvs = Information TLV（8 ビット型、ヘッダを含む 8 ビット長、自動）: Local / Remote Information、Organization Specific。End TLV（タイプ 0）で終わる。
efm-oam-sequence-number = イベント通知のシーケンス番号。
efm-oam-event-tlvs = リンクイベント TLV（8 ビット型、ヘッダを含む 8 ビット長、自動）: Errored Symbol Period、Errored Frame、Errored Frame Period、Errored Frame Seconds Summary、Organization Specific。End TLV（タイプ 0）で終わる。
efm-oam-variable-descriptors = 変数記述子のオクテット列: それぞれブランチ（1 オクテット）とリーフ（2 オクテット）、0 のブランチで終わる。
efm-oam-variable-containers = 変数コンテナのオクテット列: それぞれブランチ・リーフ・幅・値、0 のブランチで終わる。
efm-oam-loopback-command = リモートループバックコマンド: 1 有効化、2 無効化。
efm-oam-oui = 組織固有識別子（OUI）。
efm-oam-data = OAMPDU の残りをオクテット列で。

# --- P2 MACsec / GTPv2-C / Diameter / SIP / RTP field descriptions ---
macsec-v = TCI の V ビット: バージョン（0）。
macsec-es = TCI の ES ビット: エンドステーション（SCI は送信元 MAC アドレスとポート 1）。
macsec-sc = TCI の SC ビット: 明示的な SCI あり（Auto: SCI を指定すると 1）。
macsec-scb = TCI の SCB ビット: EPON のシングルコピーブロードキャスト。
macsec-e = TCI の E ビット: セキュアデータは暗号化されている。
macsec-c = TCI の C ビット: ユーザデータが変更された（既定の暗号スイートでは E と同時に 1）。
macsec-an = アソシエーション番号（0〜3）: チャネル内のセキュアアソシエーション。
macsec-sl = ショート長: セキュアデータが 48 オクテット未満のときその長さ、それ以外は 0（手動）。
macsec-pn = パケット番号（32 ビット、XPN では下位半分）。
macsec-sci = Secure Channel Identifier: システム識別子（MAC アドレス）とポート識別子。SC が 1 のときに存在。
macsec-secure-data = セキュアデータ: 暗号化（または完全性保護のみ）されたユーザデータを不透明なオクテット列として指定。暗号化は行わない。
macsec-icv = 完全性検査値（16 オクテット）。不透明な値で、計算はしない。
gtpv2-version = GTP バージョン（2）。
gtpv2-p = P フラグ: ピギーバックされたメッセージが続く（次の GTPv2-C レイヤ）。
gtpv2-t = T フラグ: TEID フィールドあり（Auto: TEID を指定すると 1）。
gtpv2-mp = MP フラグ: メッセージ優先度あり。
gtpv2-message-type = メッセージ種別（Echo、Create / Modify / Delete Session、Create / Update / Delete Bearer など）。
gtpv2-length = 先頭 4 オクテットを除くメッセージ長（Auto）。
gtpv2-teid = 受信側の Tunnel Endpoint Identifier（Echo メッセージにはない。最初の要求では 0）。
gtpv2-sequence = 応答と要求を対応付けるシーケンス番号（24 ビット）。
gtpv2-message-priority = メッセージ優先度（0 が最高）。MP が 1 のときに使用。
gtpv2-ies = 情報要素（8 ビット型、データの 16 ビット長、スペア / インスタンスの 1 オクテット）: IMSI、Cause、Recovery、APN、AMBR、EBI、IP Address、MEI、MSISDN、PAA、Bearer QoS、RAT Type、Serving Network、F-TEID、Bearer Context（グループ化）、PDN Type など。長さは Auto。
diameter-version = Diameter バージョン（1）。
diameter-length = ヘッダと AVP の詰め物を含むメッセージ長（Auto）。
diameter-r = R フラグ: 要求（0 = 応答）。
diameter-p = P フラグ: プロキシ可能。
diameter-e = E フラグ: 応答がプロトコルエラーを含む。
diameter-t = T フラグ: 再送の可能性があるメッセージ。
diameter-command-code = コマンドコード（Capabilities-Exchange、Device-Watchdog、Credit-Control など）。
diameter-application-id = Application-ID（0 = Diameter 共通メッセージ）。
diameter-hop-by-hop = 1 つのコネクション上で応答と要求を対応付ける Hop-by-Hop 識別子。
diameter-end-to-end = 重複した要求を検出する End-to-End 識別子。
diameter-avps = AVP（32 ビットコード、V / M / P フラグ、詰め物を含まない 24 ビット長、省略可能な Vendor-ID、4 オクテット境界まで詰めたデータ）: 基本プロトコルと Credit-Control の AVP、1 段のグループ化 AVP。長さ・詰め物・V フラグは Auto。
sip-start-line = 開始行: リクエスト行（メソッド、Request-URI、SIP/2.0）またはステータス行（SIP/2.0、コード、理由）。CRLF は自動で付く。
sip-headers = ヘッダ行（名前: 値）を 1 要素 1 行で指定。各行の CRLF と直後の空行は自動で付く。Content-Length は手動。
sip-body = メッセージ本文（SDP のオファー / アンサーなど）。
rtp-version = RTP バージョン（2）。
rtp-padding = P ビット: ペイロード末尾の詰め物（最後のオクテットがその数、手動）。
rtp-x = X ビット: CSRC リストの後に拡張ヘッダが続く（Auto）。
rtp-cc = CSRC の数（Auto）。
rtp-marker = マーカービット（プロファイル定義。例: 通話区間の先頭）。
rtp-payload-type = ペイロードタイプ（0 PCMU、8 PCMA、9 G.722、18 G.729、96〜127 動的）。
rtp-sequence = シーケンス番号。RTP パケットごとに 1 増える。
rtp-timestamp = タイムスタンプ: ペイロード先頭オクテットのサンプリング時刻。
rtp-ssrc = 同期送信元識別子。
rtp-csrc = 寄与送信元識別子（最大 15 個）。
rtp-extension = 拡張ヘッダ: プロファイル定義の 16 ビット（0xBEDE: RFC 8285 の 1 バイト要素）、32 ビットワード単位の長さ（Auto）、拡張ワード。
rtcp-version = RTCP バージョン（2）。
rtcp-padding = P ビット: パケット末尾の詰め物（手動）。
rtcp-count = 型付けしていないパケットタイプのカウント / フォーマット（5 ビット）。
rtcp-report-count = 受信レポートの数（Auto）。
rtcp-source-count = ソースの数: SDES のチャンクまたは BYE の識別子（Auto）。
rtcp-subtype = アプリケーション定義のサブタイプ。
rtcp-packet-type = パケットタイプ（200 SR、201 RR、202 SDES、203 BYE、204 APP、205 RTPFB、206 PSFB、207 XR）。
rtcp-length = 32 ビットワード単位のパケット長から 1 を引いた値（Auto）。
rtcp-body = 共通ヘッダ以降のパケット本体（オクテット列）。
rtcp-ssrc = このパケットの送信者の SSRC。
rtcp-ntp-timestamp = NTP タイムスタンプ（上位ワード。次のフィールドが小数部）。
rtcp-rtp-timestamp = NTP タイムスタンプに対応する RTP タイムスタンプ。
rtcp-packet-count = 送信者のパケット数。
rtcp-octet-count = 送信者のペイロードオクテット数。
rtcp-report-blocks = 受信レポートブロック: SSRC、損失率、累積損失数、拡張最大シーケンス番号、ジッタ、LSR、DLSR。
rtcp-extension = パケット長までのプロファイル固有の拡張オクテット。
rtcp-chunks = SDES チャンク: SSRC / CSRC とその項目（型、長さ（Auto）、テキスト）。null オクテットで終わり、32 ビット境界まで詰める。
rtcp-ssrcs = セッションを離れる SSRC / CSRC 識別子。
rtcp-reason = 省略可能な離脱理由: 長さ（Auto）、テキスト、32 ビット境界までの詰め物。
rtcp-app-name = 名前（ASCII 4 文字）。
rtcp-app-data = アプリケーション依存のデータ（4 オクテットの倍数）。
