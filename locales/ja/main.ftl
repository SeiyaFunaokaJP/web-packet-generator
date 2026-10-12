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
ui-rx-placeholder = 受信カウンタと検証機能はマイルストーン M9 で提供します。

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
mpls-label = 20 ビット MPLS ラベル値。
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
ospf-ls-type = LSA タイプ (LSA 本体レイヤから自動)。
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
bgp-withdrawn-routes-length = 撤回経路の長さ (オクテット、手動)。
bgp-withdrawn-routes = 撤回経路 ((プレフィックス長, プレフィックス) のオクテット列)。
bgp-total-path-attribute-length = パス属性の長さ (オクテット、手動)。
bgp-path-attributes = パス属性 (フラグ + タイプコード、8 ビット長)。
bgp-nlri = NLRI ((プレフィックス長, プレフィックス) のオクテット列)。
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
