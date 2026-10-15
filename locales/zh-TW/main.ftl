# 正體中文 (zh-TW) 語系。每個訊息都必須存在於所有語系檔中 (locales/en/main.ftl 等)
# (由 wpg-i18n 單元測試強制檢查)。

app-name = Web Packet Generator
app-tagline = 從瀏覽器操作的高效能網路測試封包產生器。

## 伺服器 / API
health-ok = 服務運作正常
server-listening = 正在監聽 { $addr }
server-placeholder-ui = 尚未建置 Web UI。請在 crates/wpg-web 中執行 `trunk build`。

## 錯誤
error-not-implemented = 尚未實作 `{ $command }`。
error-not-found = 找不到
error-internal = 內部伺服器錯誤

## UI
ui-language = 語言
ui-protocols = 協定

## 診斷 (wpg-core `diag::keys`；參數名稱屬於介面約定的一部分)
diag-field-out-of-range = 欄位 `{ $field }` = { $value } 超出範圍 { $min }..{ $max }。
diag-field-not-equal = 欄位 `{ $field }` = { $value } 與預期值 { $expected } 不同。
diag-field-forbidden-value = 欄位 `{ $field }` 的值不建議使用：{ $value }。
diag-length-too-short = `{ $span }` 長度為 { $len } 位元組，至少需要 { $min }。
diag-length-too-long = `{ $span }` 長度為 { $len } 位元組，最多允許 { $max }。
diag-length-mismatch = 長度欄位 `{ $field }` = { $value } 與計算出的長度 { $expected } 不同。
diag-count-mismatch = 計數欄位 `{ $field }` = { $value } 與實際元素數量 { $expected } 不同。
diag-fill-inexact = 無法以填充項目剛好達到 `{ $field }`：尚缺 { $len } 個八位元組。
diag-checksum-mismatch = 檢查碼欄位 `{ $field }` = { $value } 與計算出的檢查碼 { $expected } 不同。
diag-next-protocol-mismatch = 欄位 `{ $field }` = { $value } 未指出下一層 `{ $next }` (預期值為 { $expected })。
diag-auto-overflow = `{ $field }` 的自動值 { $value } 超出 { $bits } 位元可表示的範圍。
diag-length-not-aligned = `{ $field }` 的長度 { $len } 不是 { $unit } 位元組的倍數。
diag-no-pseudo-header = 檢查碼 `{ $field }` 需要虛擬標頭，但下層沒有 IP 層。
diag-not-implemented = `{ $field }`：尚未實作 { $what }。
diag-truncated = 此層已刻意截斷為 { $len } 位元組。
diag-short-header = 輸入在 { $protocol } 標頭內的偏移 { $offset } 處結束。
diag-length-exceeds-data = 長度欄位 `{ $field }` = { $len } 超過可用的 { $available } 位元組。
diag-trailing-bytes = 封包結尾之後還剩 { $len } 位元組。
diag-unsupported-field = 剖析器尚無法解碼 { $protocol } 的欄位 `{ $field }`。

## 協定欄位說明 (wpg-protocols `desc = "..."`)
arp-hlen = 硬體位址長度 (位元組，Ethernet 為 6)。
arp-htype = 硬體類型 (1 = Ethernet)。
arp-oper = 操作 (1 = 請求，2 = 回覆)。
arp-plen = 協定位址長度 (位元組，IPv4 為 4)。
arp-ptype = 協定類型 (EtherType，0x0800 = IPv4)。
arp-sha = 傳送端硬體位址。
arp-spa = 傳送端協定位址。
arp-tha = 目標硬體位址。
arp-tpa = 目標協定位址。
icmp-code = 訊息代碼，意義取決於類型。
icmp-identifier = 用於配對 Echo 請求與回覆的識別碼。
icmp-rest = 依類型而定的 4 位元組「標頭其餘部分」欄位。
icmp-sequence = 用於配對 Echo 請求與回覆的序號。
icmp-type = 訊息類型。
ipv6-fragment-offset = 此片段的偏移 (以 8 位元組為單位)。
ipv6-more-fragments = More Fragments 旗標 (1 = 後面還有片段)。
ipv6-options = 原始 TLV 選項，填補至 8 位元組的倍數。
ipv6-routing-data = 依類型而定的路由資料。
ipv6-routing-type = 路由標頭類型 (例如 2 = Mobile IPv6，4 = SRH)。
ipv6-segments-left = 剩餘的路由區段數。
srh-segments-left = Segments Left：下一個要造訪之區段 (在區段清單中) 的索引；最後一個區段時為 0。
srh-last-entry = Last Entry：區段清單最後一個元素的索引，即區段數 - 1 (Auto)。
srh-flags = 旗標 (RFC 8754 未定義任何旗標；0)。
srh-tag = Tag：標記封包屬於某個類別或群組 (0 = 未使用)。
srh-segment-list = 依路徑反向排列的區段清單 (SID)：元素 0 為最後一個區段，最後一個元素為第一個區段。當 Segments Left 不為零時，上層檢查碼以元素 0 作為目的位址。
srh-tlvs = 選用 TLV (8 位元類型、8 位元長度)：Pad1、PadN、HMAC (HMAC 值為 Manual)。之後標頭以 Pad1 八位元組填補至 8 的倍數 (Auto)。
ndp-destination = 重新導向的目的位址。
ndp-managed = Managed 位址設定旗標 (M)。
ndp-options = 原始 NDP 選項 (TLV)。
ndp-other = Other 設定旗標 (O)。
ndp-override-flag = Override 旗標 (O)。
ndp-router-flag = Router 旗標 (R)。
ndp-solicited-flag = Solicited 旗標 (S)。
ndp-target = 目標位址。

## wpg ifaces
ifaces-col-name = 名稱
ifaces-col-index = IDX
ifaces-col-mac = MAC
ifaces-col-ipv4 = IPV4
ifaces-col-ipv6 = IPV6
ifaces-col-mtu = MTU
ifaces-col-state = 狀態
ifaces-col-speed = 速度
ifaces-col-driver = 驅動程式
ifaces-state-up = up
ifaces-state-down = down
ifaces-state-no-carrier = 無載波
ifaces-state-loopback = 迴路
ifaces-none = 找不到網路介面。
ifaces-error = 無法列出網路介面：{ $error }

## wpg doctor
doctor-title = wpg doctor：環境檢查
doctor-summary = 正常 { $ok } 項，警告 { $warn } 項，失敗 { $fail } 項
doctor-skip = 不適用於此作業系統
doctor-caps-ok = 可使用原始通訊端 (raw socket)
doctor-caps-missing = 缺少傳送原始封包所需的權限 (CAP_NET_RAW / root / Administrator)
doctor-caps-npcap-user = 未以系統管理員身分執行，但 Npcap 允許非系統管理員擷取與傳送
doctor-caps-npcap-admin = Npcap 僅限系統管理員使用；若要傳送，請在以系統管理員身分開啟的終端機中執行 wpg
doctor-fix-caps = 授予權限：{ $hint }
doctor-iface-ok = 至少有一個可用的網路介面處於 up 狀態
doctor-iface-none = 沒有任何具載波且處於 up 狀態的非迴路介面
doctor-iface-error = 無法列舉網路介面
doctor-fix-iface = 檢查介面：{ $hint }
doctor-af-xdp-ok = 核心支援 AF_XDP (5.10 或更新版本)
doctor-af-xdp-old = 核心版本過舊，不支援 AF_XDP (需要 5.10 或更新版本)；afpacket 後端仍可使用
doctor-tpacket-v3-ok = 核心支援 TPACKET_V3
doctor-tpacket-v3-old = 核心不支援 TPACKET_V3；afpacket 將改用 sendmmsg
doctor-hugepages-ok = 已設定 hugepages
doctor-hugepages-none = 未設定 hugepages (僅 DPDK 後端需要)
doctor-fix-hugepages = 保留 hugepages 並將 NIC 繫結至 DPDK：{ $hint }
doctor-npcap-ok = 已安裝 Npcap
doctor-npcap-missing = 未安裝 Npcap；在 Windows 上傳送封包需要 Npcap
doctor-npcap-no-dll = Npcap 似乎已安裝，但在 System32\Npcap 與 System32 中都找不到 wpcap.dll
doctor-fix-npcap = 安裝 (或重新安裝) Npcap，然後再次執行 wpg doctor：{ $hint }
doctor-pcaplib-ok = 可使用 libpcap
doctor-pcaplib-missing = 找不到 libpcap；無法使用 pcap 後端
doctor-fix-pcaplib = 安裝 libpcap：{ $hint }

## wpg template lint (wpg-template `LintIssue` 鍵)
lint-level-error = 錯誤
lint-level-warning = 警告
lint-schema-version = 不支援的結構描述版本 { $value } (支援：{ $expected })。
lint-missing-name = `meta.name` 為空。
lint-no-layers = 範本中沒有任何層。
lint-unknown-protocol = 未知的協定 `{ $protocol }`。
lint-unknown-field = 協定 `{ $protocol }` 沒有欄位 `{ $field }`。
lint-invalid-value = `{ $field }` 的值無效：{ $value }
lint-invalid-generator = `{ $field }` 的產生器無效：{ $value }
lint-unknown-variable = 未知的變數 `{ $value }`。
lint-missing-variable = 變數 `{ $value }` 沒有預設值，也未指定值。
lint-unused-variable = 變數 `{ $value }` 從未被使用。
lint-duplicate-variable = 變數 `{ $value }` 被宣告了不只一次。
lint-var-syntax = `{ $value }` 中的變數預留位置未結束。
lint-override-unknown = 未知的變數覆寫 `{ $value }`。
lint-field-and-generator = `{ $field }` 同時有固定值與產生器；以產生器為準。
lint-next-protocol-manual = `{ $next }` 不在 `{ $protocol }` 的繫結表中；`{ $field }` 將為 Manual。
lint-unknown-order = 串流順序中的 `{ $value }` 沒有對應的產生器。
lint-invalid-stream = 無效的串流設定：{ $value }
lint-build-error = 無法建構封包：{ $value }

## wpg template / wpg render
template-lint-ok = { $value }：沒有問題
template-lint-load-error = { $value }：{ $error }
template-lint-summary = { $ok } 個檔案中有 { $fail } 個錯誤、{ $warn } 個警告
template-list-empty = 在 { $value } 中找不到範本。
template-show-hex = 十六進位傾印 ({ $len } 位元組)
template-show-layout = 版面配置
render-done = 已將 { $len } 個封包寫入 { $value }。
render-unbounded = 串流沒有上限；請指定 --count N。
cli-bad-var = 無效的 --var `{ $value }`；格式應為 NAME=VALUE。

## wpg send / wpg bench (進度、摘要、錯誤)
tx-progress = [{ $t } 秒] { $packets } 個封包  { $pps }pps  { $bps }bps  丟棄 { $dropped }
tx-progress-rate = 速率 { $ratio }%
tx-progress-paused = (已暫停)
tx-progress-scheduled = (已排程，{ $secs } 秒後開始)
tx-interrupted = 已中斷，正在停止...
tx-ctrlc-warning = 警告：無法安裝 Ctrl-C 處理常式：{ $error }
tx-open-failed = 無法在 `{ $iface }` 上開啟 { $backend }：{ $error }
tx-start-failed = 無法啟動工作：{ $error }
tx-available-backends = 可用的後端：{ $value }
tx-iface-required = { $backend } 後端需要 --iface。
tx-iface-required-pcapfile = pcapfile 後端需要 --iface <輸出路徑>。
tx-bad-workers = --workers 必須至少為 1。
tx-workers-unsupported = { $backend } 後端僅支援單一工作執行緒；請使用 --workers 1。
tx-pin-cores-short = 警告：--pin-cores 只列出 { $count } 個核心，但有 { $workers } 個工作執行緒；其餘不會綁定核心。
tx-json-error = 無法寫入 JSON：{ $error }
send-bad-rate = 無效的 --rate：{ $error }
send-bad-duration = 無效的 --duration：{ $error }
send-bad-ramp-up = 無效的 --ramp-up：{ $error }
send-bad-start-at = 無效的 --start-at：{ $error }
send-link-speed-unknown = 無法判斷 `{ $iface }` 的連結速度，因此無法換算 `{ $value }%`；請指定絕對速率，例如 `1Gbps`。
send-bad-plan = --plan 必須為 auto、prerender 或 onthefly。
send-stream-error = 串流錯誤：{ $error }
send-render-error = 算繪錯誤：{ $error }
send-starting = 透過 { $backend }{ $detail } 在 { $iface } 上傳送：速率 { $rate }，突發 { $burst }，{ $limit }，{ $workers } 個工作執行緒，計畫 { $strategy } ({ $reason })
send-limit-count = { $count } 個封包
send-limit-count-or-duration = { $count } 個封包或 { $duration }
send-limit-duration = 持續 { $duration }
send-limit-forever = 直到按下 Ctrl-C
send-ramp-up = 漸增：在 { $duration } 內從 0 增加到 { $rate }
send-scheduled = 排定於 { $time } 開始 ({ $secs } 秒後)；按 Ctrl-C 取消。
summary-state = 狀態       { $value }
summary-state-finished = 已完成 ({ $reason })
summary-state-failed = 失敗：{ $error }
summary-reason-count-reached = 已達指定數量
summary-reason-duration-elapsed = 已達指定時間
summary-reason-source-exhausted = 串流已結束
summary-reason-stopped = 已停止
summary-backend = 後端       { $backend }{ $detail }
summary-workers = 工作執行緒 { $workers }
summary-packets = 封包       { $value }
summary-bytes = 位元組     { $value }
summary-dropped = 丟棄       { $value }
summary-elapsed = 經過時間   { $secs } 秒
summary-average = 平均       { $pps }pps  { $bps }bps (L2)  { $l1 }bps (L1)
summary-accuracy = 精確度     目標 { $target }{ $unit }  達成率 { $ratio }%  最大抖動 { $max }  平均抖動 { $mean }
summary-stream = 串流 { $index }   { $packets } 個封包  { $bytes } 位元組
summary-worker = 工作執行緒 { $index }   { $packets } 個封包  { $bytes } 位元組{ $core }
summary-worker-core = ，核心 { $core }
summary-warning = 警告       { $error }
summary-unsigned = 未簽章     { $value } (沒有空間放入簽章)

## wpg capture (接收 / 驗證)
rx-progress = [{ $t } 秒] { $packets } 個封包  { $pps }pps  { $bps }bps  符合 { $matched } ({ $mpps }pps)  丟棄 { $dropped }
capture-starting = 透過 { $backend } 在 { $iface } 上擷取，篩選條件 { $filter } (按 Ctrl-C 停止)
capture-filter-all = (所有訊框)
capture-iface-required-pcapfile = pcapfile 後端需要 --iface <擷取檔案>。
capture-bad-filter = 無效的 --filter：{ $error }
capture-running = 執行中
capture-reason-end-of-input = 輸入結束
capture-packet = #{ $index }  { $time }  { $len } B  { $layers }{ $signature }
capture-packet-signature = { "  " }[wpg 串流 { $stream } 序號 { $seq }]
capture-summary-matched = 符合       { $packets } 個封包  { $bytes } 位元組
capture-summary-average = 平均       { $pps }pps  { $bps }bps  符合 { $mpps }pps  { $mbps }bps
capture-summary-saved = 已儲存     { $count } 個封包至 { $path }
capture-summary-signed = 簽章       有簽章 { $signed }，無簽章 { $unsigned }
capture-summary-stream = 串流 { $stream } 已接收 { $received }  序號 { $first }-{ $last }  遺失 { $lost } ({ $ratio }%)  順序錯亂 { $reordered }  重複 { $duplicates }
capture-summary-latency = { "             " }延遲 最小 { $min }  平均 { $avg }  最大 { $max }  抖動 { $jitter }

bench-iface = 介面       { $iface }
bench-worker = 工作執行緒 { $index }   { $detail }
bench-frame = 訊框       { $len } B (+4 FCS)  批次 { $batch }
bench-template = 範本       { $value } (即時產生，{ $mode })
bench-mode-patch = 就地修補
bench-mode-rebuild = 每個訊框完整重建
bench-rate = 速率       { $pps }pps
bench-bits = { $bps }bps (L2)  { $l1 }bps (L1)
bench-bad-duration = --duration 必須是非負的秒數。
bench-failed = 效能測試失敗：{ $error }
import-done = 已將 { $len } 個封包匯入至 { $count } 個範本：{ $value }
import-read-error = { $value }：{ $error }
import-write-error = { $value }：{ $error }
import-note = { $value }
import-note-packet = 封包 { $index }：{ $value }
import-packet-problems = 封包 { $index } 有 { $count } 個剖析問題
import-packet-skipped = 封包 { $index } 無法解碼，已略過
import-not-reproducible = 匯入的範本無法重建擷取到的八位元組；該訊框已以原始資料儲存。
import-mixed-stacks = 有 { $count } 個封包的層堆疊與第一個封包不同；未推斷任何產生器。
import-generator-approximate = { $value } 僅能近似擷取到的值。
import-generators-dropped = 推斷出的產生器產生了無效的串流，已捨棄。
template-export-none = 未指定要匯出的範本。
template-export-not-found = 找不到範本：{ $value }
template-export-done = 已將 { $count } 個範本匯出至 { $value }。
template-export-error = { $value }：{ $error }
template-import-done = 已從 { $value } 擷取 { $count } 個範本至 { $detail }。
template-import-exists = { $value }：已存在 (使用 --force 覆寫)
template-import-error = { $value }：{ $error }
template-migrate-outdated = { $value }：結構描述 { $found }，需要遷移至 { $expected }
template-migrate-done = { $value }：已從結構描述 { $found } 遷移至 { $expected }
template-migrate-error = { $value }：{ $error }
template-migrate-summary = 已遷移 { $count } 個，{ $ok } 個已是結構描述 { $expected }，{ $fail } 個錯誤
template-migrate-check-summary = 需遷移 { $count } 個，{ $ok } 個已是結構描述 { $expected }，{ $fail } 個錯誤

## API 錯誤 (wpg-server)
api-error-not-found = 找不到 { $what }。
api-error-invalid-body = 無效的請求本文：{ $error }
api-error-invalid-query = 無效的請求參數：{ $error }
api-error-invalid-template = 無效的範本：{ $error }
api-error-invalid-hex = 無效的十六進位輸入：{ $error }
api-error-parse-failed = 無法解碼位元組：{ $error }
api-error-render-failed = 算繪失敗：{ $error }
api-error-count-range = `count` 必須介於 1 到 { $max } 之間 (收到 { $error })。
api-error-read-only = 範本 { $id } 屬於唯讀範本庫；請先複製再編輯。
api-error-policy-iface = 伺服器政策不允許在 { $iface } 上傳送 (允許：{ $allowed })。
api-error-policy-rate = 速率 { $rate } 超過伺服器政策允許的上限 ({ $max })。
api-error-policy-unlimited = 伺服器政策不允許無限制速率 (上限 { $max })。
api-error-invalid-state = 工作 { $id } 目前為 { $state }；無法 { $action }。
api-error-job-spec = 無效的工作：{ $error }
api-error-backend = 傳送後端錯誤：{ $error }
api-error-capture-spec = 無效的擷取：{ $error }
api-error-capture-filter = 無效的擷取篩選條件：{ $error }
api-error-capture-backend = 接收後端錯誤：{ $error }
api-error-capture-state = 擷取 { $id } 目前為 { $state }；無法 { $action }。
api-error-internal = 內部伺服器錯誤：{ $error }
auth-error-unauthorized = 請先登入。
auth-error-bad-credentials = 使用者名稱或密碼錯誤。
auth-error-setup-required = 尚未設定管理員密碼：請先完成初始設定。
auth-error-already-configured = 已設定過管理員密碼。
auth-error-rate-limited = 登入失敗次數過多。請在 { $secs } 秒後再試。
auth-error-csrf = CSRF 權杖 (X-CSRF-Token 標頭) 遺失或無效。
auth-error-origin = 不允許來自其他來源的 WebSocket 連線。
auth-error-setup-remote = 管理員密碼只能從伺服器本機 (localhost) 設定。
auth-error-password-mismatch = 密碼不一致。
auth-error-password-short = 密碼長度至少需要 { $min } 個字元。
lint-parse-error = 無法剖析範本：{ $error }

## 設定 (wpg.toml)
config-issue-file-missing = 找不到檔案
config-issue-bad-url = 必須以 http:// 或 https:// 開頭
config-issue-not-argon2 = 看起來不是 argon2 雜湊 ($argon2id$...)
config-issue-password-both = 同時設定了 password_hash 與 password_file；以 password_hash 為準
config-issue-zero = 必須大於 0
config-issue-exposed-without-password = 在非迴路位址上監聽，但未設定密碼
config-issue-dir-missing = 找不到目錄
config-issue-protocols = 無法載入使用者自訂協定
protocols-ext-loaded = 已載入 { $count } 個使用者自訂協定
protocols-ext-error = 無法載入使用者自訂協定: { $file }: { $error }
config-issue-max-rate-percent = 連結百分比不能作為伺服器全域上限；請使用 pps 或 bps
config-issue-bad-rate = 無效的速率
config-issue-unknown-backend = 未知的後端
config-issue-backend-unavailable = 此建置版本 / 此作業系統不提供該後端
config-issue-bad-log-level = 無效的記錄篩選條件
config-level-error = 錯誤
config-level-warning = 警告
config-issue-line = { $level }：{ $key }：{ $message }{ $detail }
config-load-error = 設定錯誤：{ $error }
config-init-written = 已寫入 { $path }。請編輯後以 `wpg config validate` 檢查。
config-init-exists = { $path } 已存在 (使用 --force 覆寫)。
config-validate-ok = 設定正確 ({ $source })。
config-validate-summary = { $errors } 個錯誤，{ $warnings } 個警告 ({ $source })。
config-source-defaults = 內建預設值與環境變數
config-password-prompt = 新的管理員密碼：{ " " }
config-password-confirm = 再次輸入密碼：{ " " }
config-password-mismatch = 密碼不一致。
config-password-short = 密碼長度至少需要 { $min } 個字元。
config-password-written = 已將密碼雜湊儲存至 { $path } (auth.password_hash)。請重新啟動 `wpg serve` 以套用。

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = 監聽位址       { $url }/
serve-banner-docs = API 文件       { $url }/api/docs/
serve-banner-config = 設定           { $source }
serve-banner-data = 資料目錄       { $path }
serve-banner-library = 範本庫         { $path }{ $state }
serve-banner-user-dir = 使用者範本     { $path }
serve-banner-templates = 範本數         { $count }
serve-banner-backends = 後端           { $list }
serve-banner-missing = { " " }(不存在)
serve-banner-dev = 開發模式：允許來自 { $origins } 的 CORS
serve-banner-no-password = 未設定密碼，且伺服器正在 { $url } 上監聽 (--allow-insecure)：任何能連線到它的人都可以設定管理員密碼並傳送封包。請盡快設定密碼 (wpg config set-password)。
serve-failed = 伺服器錯誤：{ $error }
serve-log-error = 無法設定記錄功能：{ $error }
serve-banner-tls = TLS            { $path }
serve-banner-tls-self-signed = TLS            { $path } (自我簽署)
serve-banner-fingerprint = SHA-256        { $fingerprint }
serve-banner-auth = 登入           { $state }
serve-banner-auth-setup = 尚未設定密碼：請在初始設定畫面設定
serve-banner-auth-configured = 管理員密碼來自 { $source }
serve-banner-auth-database = 管理員密碼已在初始設定畫面設定
serve-banner-setup-local = 尚未設定管理員密碼。請在本機的瀏覽器中開啟 { $url } 進行設定。
serve-refuse-insecure = 拒絕在 { $addr } 上監聽：未設定管理員密碼。請使用迴路位址 (127.0.0.1)、設定密碼 (--password-file、WPG_PASSWORD、wpg config set-password)，或指定 --allow-insecure。
serve-password-arg-warning = 警告：--password 會在處理程序清單中被其他使用者看到；建議改用 --password-file、WPG_PASSWORD 或 `wpg config set-password`。

## Web UI (wpg-web)。鍵由 crates/wpg-web 中的 `ui_keys_exist` 測試檢查。
ui-lang-ja = 日本語
ui-lang-en = English
ui-lang-zh-cn = 简体中文
ui-lang-zh-tw = 繁體中文
ui-lang-ko = 한국어
ui-lang-de = Deutsch
ui-lang-fr = Français
ui-lang-es = Español
ui-loading = 載入中…
ui-close = 關閉
ui-cancel = 取消
ui-delete = 刪除
ui-save = 儲存
ui-refresh = 重新整理
ui-retry = 重試
ui-yes = 是
ui-no = 否
ui-unlimited = 無限制
ui-help = 鍵盤快速鍵
ui-help-title = 鍵盤快速鍵
ui-skip-to-content = 跳至主要內容
ui-not-found = 此頁面不存在。
ui-back-to-dashboard = 返回儀表板
ui-view-only = 螢幕較窄：僅供檢視。請在較寬的螢幕上編輯。
ui-server-unreachable = 無法連線到伺服器。`wpg serve` 是否正在執行？
ui-error-http = 伺服器回應 HTTP { $id }。
ui-error-network = 網路錯誤：{ $error }
ui-error-decode = 伺服器傳回非預期的回應：{ $error }
ui-nav-label = 主要導覽
ui-nav-dashboard = 儀表板
ui-nav-builder = 建構器
ui-nav-jobs = 工作
ui-nav-templates = 範本
ui-nav-rx = 接收
ui-nav-settings = 設定
ui-shortcut-hint = 顯示鍵盤快速鍵
ui-shortcut-dashboard = 前往儀表板
ui-shortcut-builder = 前往建構器
ui-shortcut-jobs = 前往工作
ui-shortcut-templates = 前往範本
ui-shortcut-rx = 前往接收
ui-shortcut-settings = 前往設定
ui-shortcut-new-job = 新增工作
ui-shortcut-logs = 顯示 / 隱藏伺服器記錄
ui-shortcut-help = 顯示此說明
ui-shortcut-escape = 關閉對話方塊
ui-ws-connecting = 連線中…
ui-ws-open = 即時
ui-ws-closed = 已中斷連線 (重試中)
ui-theme = 佈景主題
ui-theme-light = 淺色
ui-theme-dark = 深色
ui-theme-system = 跟隨系統
ui-theme-to-dark = 切換為深色主題
ui-theme-to-light = 切換為淺色主題
ui-logout = 登出
ui-logs-toggle = 伺服器記錄
ui-logs-title = 伺服器記錄
ui-logs-level = 層級
ui-logs-follow = 自動捲動
ui-logs-clear = 清除記錄
ui-logs-count = { $count } 行
ui-logs-empty = 尚無記錄。
ui-login-title = 登入
ui-login-submit = 登入
ui-login-busy = 登入中…
ui-login-wrong = 密碼錯誤。
ui-login-rate-limited = 嘗試次數過多。請在 { $secs } 秒後再試。
ui-login-wait = 請等待 { $secs } 秒
ui-password = 密碼
ui-password-confirm = 確認密碼
ui-setup-title = 設定管理員密碼
ui-setup-intro = 此伺服器尚未設定密碼。請設定密碼以保護 Web UI 與 API。
ui-setup-hint = 至少 { $min } 個字元。
ui-setup-too-short = 密碼至少需要 { $min } 個字元。
ui-setup-mismatch = 密碼不一致。
ui-setup-submit = 設定密碼
ui-dash-totals = 總計
ui-dash-active-jobs = 進行中的工作
ui-dash-running-jobs = 傳送中
ui-dash-total-pps = 總 pps
ui-dash-total-bps = 總位元速率
ui-dash-templates = 範本
ui-dash-uptime = 伺服器運作時間
ui-dash-running = 執行中的工作
ui-dash-no-running = 沒有執行中的工作。
ui-dash-doctor = 自我診斷
ui-dash-ifaces = 介面
ui-doctor-ok = 所有檢查皆通過
ui-doctor-warn = 有警告
ui-doctor-fail = 發現問題
ui-doctor-counts = 正常 { $ok } · 警告 { $warn } · 失敗 { $fail }
ui-doctor-details = 完整報告
ui-doctor-status = 狀態
ui-doctor-check = 檢查項目
ui-doctor-detail = 詳細資訊 / 修正方式
ui-doctor-status-ok = 正常
ui-doctor-status-warn = 警告
ui-doctor-status-fail = 失敗
ui-doctor-status-skip = 已略過
ui-iface-name = 名稱
ui-iface-state = 狀態
ui-iface-mac = MAC
ui-iface-addrs = 位址
ui-iface-mtu = MTU
ui-iface-speed = 速度
ui-iface-allowed = 允許傳送
ui-iface-up = Up
ui-iface-down = Down
ui-iface-no-carrier = 無載波
ui-stat-pps = 封包/秒
ui-stat-bps = 位元速率 (L2)
ui-stat-l1-bps = 位元速率 (L1)
ui-stat-packets = 封包
ui-stat-bytes = 位元組
ui-stat-dropped = 丟棄
ui-stat-ratio = 速率達成率
ui-stat-elapsed = 經過時間
ui-spark-pps = 每秒封包數，最近 30 秒
ui-state-created = 已建立
ui-state-scheduled = 已排程
ui-state-running = 執行中
ui-state-paused = 已暫停
ui-state-finished = 已完成
ui-state-failed = 失敗
ui-state-unknown = 未知
ui-reason-count-reached = 已達封包數
ui-reason-duration-elapsed = 已達持續時間
ui-reason-source-exhausted = 所有封包已傳送
ui-reason-stopped = 已停止
ui-jobs-new = 新增工作
ui-jobs-tab-live = 目前
ui-jobs-tab-history = 歷程記錄
ui-jobs-empty = 沒有工作。
ui-jobs-streams = 串流
ui-jobs-template = 範本
ui-jobs-choose-template = 選擇範本…
ui-jobs-weight = 權重
ui-jobs-add-stream = 新增串流
ui-jobs-remove-stream = 移除串流
ui-jobs-mix = 混合方式
ui-mix-weighted = 加權
ui-mix-sequential = 依序
ui-mix-round-robin = 輪流 (Round robin)
ui-jobs-target = 傳送目標
ui-jobs-backend = 後端
ui-jobs-iface = 介面
ui-jobs-choose-iface = 選擇介面…
ui-jobs-iface-not-needed = 此後端不需要
ui-jobs-iface-denied = 不允許
ui-jobs-dry-run = 試執行：pcapng 檔名
ui-jobs-dry-run-hint = 設定後不會實際傳送：封包會寫入伺服器擷取目錄下的此檔案。
ui-jobs-how = 傳送方式
ui-jobs-rate = 速率
ui-jobs-count = 封包數
ui-jobs-duration = 持續時間
ui-jobs-burst = 突發
ui-jobs-workers = 工作執行緒
ui-jobs-workers-disabled = 多個工作執行緒需要支援多工作執行緒的後端，且只能有單一串流。
ui-jobs-from-template = 使用範本設定
ui-jobs-name = 名稱
ui-jobs-name-placeholder = 預設為範本名稱
ui-jobs-start-now = 立即開始
ui-jobs-create = 建立工作
ui-jobs-create-start = 建立並開始
ui-jobs-created = 已建立工作「{ $id }」。
ui-jobs-need-iface = 請選擇介面。
ui-jobs-need-template = 請為每個串流選擇範本。
ui-jobs-bad-weight = 權重必須是正整數。
ui-rate-default = 留空：使用範本的速率 (或伺服器的上限)。
ui-rate-is-pps = → { $rate }
ui-rate-is-bps = → 線路上 { $rate } (第 1 層)
ui-rate-is-percent = → 連結速度的 { $rate } %
ui-rate-is-ifg = 訊框間固定間隔：{ $rate }
ui-rate-is-unlimited = 盡可能快速
ui-valid-empty = 必填。
ui-valid-number = 不是數字。
ui-valid-integer = 請輸入整數。
ui-valid-range = 必須大於零。
ui-valid-percent = 百分比不得超過 100。
ui-valid-rate-unit = 未知的單位：請使用 pps、kpps、Mpps、bps、kbps、Mbps、Gbps、%、ifg=<時間> 或 unlimited。
ui-valid-duration-unit = 未知的單位：請使用 ns、us、ms、s、m 或 h。
ui-valid-capture-name = 請使用不含「..」的相對檔名。
ui-confirm-send-title = 要在實體介面上傳送嗎？
ui-confirm-send-body = 此工作會在網路上傳送封包。請確認介面與速率適合此網路。
ui-confirm-send = 傳送
ui-job-detail = 工作
ui-job-state = 狀態
ui-job-start = 開始
ui-job-pause = 暫停
ui-job-resume = 繼續
ui-job-stop = 停止
ui-job-download-report = 報告 (JSON)
ui-job-live = 即時統計
ui-job-streams = 各串流
ui-job-inline-template = (內嵌範本)
ui-job-workers = 各工作執行緒
ui-job-workers-pending = 各工作執行緒的計數器包含在最終報告中，工作結束後即可查看。
ui-job-active = 作用時間
ui-job-core = 核心
ui-job-accuracy = 速率控制精確度
ui-acc-target = 目標
ui-acc-achieved = 實際達成
ui-acc-jitter = 抖動 (平均 / 最大)
ui-job-spec = 設定
ui-job-created = 建立時間
ui-job-started = 開始時間
ui-job-finished = 結束時間
ui-job-deleted = 已刪除工作。
ui-job-delete-title = 要刪除此工作嗎？
ui-job-delete-body = 執行中的工作會先停止；其報告會保留在歷程記錄中。
ui-tpl-import = 匯入
ui-tpl-new = 新增範本
ui-tpl-search = 搜尋名稱、說明、標籤、協定…
ui-tpl-source = 來源
ui-tpl-source-all = 全部
ui-tpl-source-library = 範本庫
ui-tpl-source-user = 我的
ui-tpl-favorites-only = 僅顯示我的最愛
ui-tpl-tags = 標籤
ui-tpl-recent = 最近使用
ui-tpl-empty = 沒有符合的範本。
ui-tpl-favorite = 我的最愛
ui-tpl-view = 檢視 / 編輯 YAML
ui-tpl-open-builder = 在建構器中開啟
ui-tpl-new-job = 使用此範本新增工作
ui-tpl-export = 下載 YAML
ui-tpl-duplicate = 複製
ui-tpl-imported = 已匯入範本「{ $id }」。
ui-tpl-duplicated = 已建立複本「{ $id }」。
ui-tpl-deleted = 已刪除範本「{ $id }」。
ui-tpl-delete-title = 要刪除此範本嗎？
ui-tpl-delete-body = 「{ $id }」將從您的範本目錄中移除。
ui-tpl-saved = 已儲存範本。
ui-tpl-yaml = 範本 YAML
ui-tpl-read-only = 範本庫中的範本為唯讀：請複製後編輯複本。
ui-lint-run = 檢查 (lint)
ui-lint-clean = 未發現問題。
ui-lint-summary = { $errors } 個錯誤，{ $warnings } 個警告
ui-set-appearance = 外觀
ui-set-account = 帳戶
ui-set-logged-in = 已以 { $id } 身分登入。
ui-set-auth-disabled = 此伺服器未啟用驗證。
ui-set-client = 此瀏覽器
ui-set-client-note = 僅儲存在此瀏覽器中。伺服器設定 (wpg.toml) 不會透過 API 公開。
ui-set-api = API
ui-set-ws = 事件
ui-set-ui-version = UI 版本
ui-set-api-docs = API 文件 (Swagger UI)
ui-set-about = 關於
ui-set-server-version = 伺服器版本
ui-set-api-version = API 版本
ui-set-platform = 平台
ui-set-features = 選用功能
ui-set-locales = 語言
ui-set-license = wpg 採用 MIT 或 Apache-2.0 雙重授權。
ui-builder-placeholder = 封包建構器 (協定面板、層堆疊、欄位編輯器、十六進位 / 樹狀檢視) 將在下一個里程碑步驟中提供。
ui-builder-placeholder-template = 封包建構器將在下一個里程碑步驟中提供。範本「{ $id }」將在此開啟。
ui-reason-end-of-input = 擷取檔案結束
ui-valid-stream-id = 請輸入 0 到 4294967295 之間的整數。
ui-jobs-verification = 接收驗證
ui-jobs-signature = 加上驗證簽章
ui-jobs-signature-hint = 在每個訊框酬載的最後 24 個八位元組寫入串流 ID、序號與傳送時間，讓啟用「驗證簽章」的擷取可以量測遺失、順序與延遲。
ui-jobs-stream-id = 簽章串流 ID (第一個串流)
ui-job-signature = 簽章
ui-job-signature-on = 開啟 (串流 ID { $id } 起)
ui-rx-new = 新增擷取
ui-rx-file = 擷取檔案
ui-rx-file-hint = 伺服器擷取目錄中的 pcap / pcapng 檔名，例如工作試執行的輸出。
ui-rx-iface-any = any (所有介面)
ui-rx-filter = 篩選條件
ui-rx-filter-empty = 留空：所有訊框都符合。
ui-rx-filter-checking = 正在檢查篩選條件…
ui-rx-filter-ok = 有效：{ $filter }
ui-rx-filter-error = { $error } (位於第 { $offset } 個字元)
ui-rx-count-hint = 符合的訊框達到此數量後停止。
ui-rx-keep = 保留的訊框數
ui-rx-keep-hint = 保留前 N 個符合的訊框以供顯示 (最多 10000)。
ui-rx-keep-max = 最多 10000。
ui-rx-save = 另存為 pcapng (檔名)
ui-rx-save-hint = 選用：符合的訊框也會寫入伺服器擷取目錄中的此檔案。
ui-rx-save-same = 輸出檔案必須與輸入檔案不同。
ui-rx-name-placeholder = 預設為 後端:介面
ui-rx-options = 選項
ui-rx-verify = 驗證簽章 (遺失、順序、延遲)
ui-rx-promisc = 混雜模式
ui-rx-outgoing = 包含本機傳送的訊框
ui-rx-start = 開始擷取
ui-rx-started = 已開始擷取「{ $id }」。
ui-rx-list = 擷取
ui-rx-empty = 尚無擷取。
ui-rx-source = 介面 / 檔案
ui-rx-actions = 動作
ui-rx-stop = 停止
ui-rx-stop-named = 停止「{ $id }」
ui-rx-delete-named = 刪除「{ $id }」
ui-rx-deleted = 已刪除擷取。
ui-rx-delete-title = 要刪除此擷取嗎？
ui-rx-delete-body = 執行中的擷取會先停止。已儲存的 pcapng 檔案會保留在伺服器上。
ui-rx-download = 下載 pcapng
ui-rx-counters = 計數器
ui-rx-matched = 符合
ui-rx-matched-pps = 符合的封包/秒
ui-rx-matched-bps = 符合的位元速率
ui-rx-matched-bytes = 符合的位元組
ui-rx-saved = 已存入檔案
ui-rx-kept = 已保留供顯示
ui-rx-verify-title = 驗證
ui-rx-verify-off = 此擷取未啟用簽章驗證。
ui-rx-verify-none = 尚未收到帶簽章的訊框。
ui-rx-signed = 帶簽章的訊框
ui-rx-unsigned = 無簽章的訊框
ui-rx-lost = 遺失
ui-rx-stream = 串流
ui-rx-received = 已接收
ui-rx-seq-range = 序號
ui-rx-out-of-order = 順序錯亂
ui-rx-duplicates = 重複
ui-rx-latency = 延遲 (最小 / 平均 / 最大)
ui-rx-jitter = 抖動
ui-rx-latency-note = 遺失的訊框是根據序號的缺口推估的。只有在傳送端與接收端共用時脈 (同一主機，或經 PTP / NTP 同步) 時，延遲才有意義。
ui-rx-frames = 已擷取的訊框
ui-rx-frames-empty = 尚無符合的訊框。
ui-rx-time = 時間 (秒)
ui-rx-length = 長度
ui-rx-signature = 簽章
ui-rx-sig-value = 串流 { $stream } · 序號 { $seq }
ui-rx-show-more = 顯示更多 ({ $max } 中的 { $count })
ui-rx-frame-title = 訊框 { $index }
ui-rx-select-frame = 選擇訊框以檢視其八位元組與解碼後的各層。
ui-rx-decode-failed = 無法解碼此訊框：{ $error }
ui-rx-truncated = 只擷取了 { $len } 個八位元組中的 { $bytes } 個。

# --- P1 L2 / 封裝協定欄位說明 ---
lldp-tlvs = LLDP TLV (7 位元類型 + 9 位元長度標頭)。
cdp-version = CDP 協定版本 (2)。
cdp-ttl = 此通告的有效時間 (秒)。
cdp-checksum = 涵蓋 CDP 標頭與 TLV 的網際網路檢查碼 (Auto)。
cdp-tlvs = CDP TLV (16 位元類型、包含標頭的 16 位元長度)。
stp-protocol-id = Spanning Tree 協定識別碼 (固定為 0)。
stp-version = 協定版本 (0 = STP，2 = RSTP，3 = MSTP)。
stp-type = BPDU 類型 (0 = 設定，2 = RST/MST，0x80 = TCN)。
stp-flags = BPDU 旗標 (拓撲變更、提議、角色、學習、轉送、協議、TCA)。
stp-root-priority = 根橋接器優先順序與 system-id 延伸。
stp-root-mac = 根橋接器 MAC 位址。
stp-root-path-cost = 到根橋接器的路徑成本。
stp-bridge-priority = 傳送端橋接器優先順序與 system-id 延伸。
stp-bridge-mac = 傳送端橋接器 MAC 位址。
stp-port-id = 傳送端連接埠識別碼 (優先順序 + 編號)。
stp-message-age = BPDU 存在時間，以 1/256 秒為單位。
stp-max-age = BPDU 逾時前的最大存在時間，以 1/256 秒為單位。
stp-hello-time = 設定 BPDU 的傳送間隔，以 1/256 秒為單位。
stp-forward-delay = 轉送延遲計時器，以 1/256 秒為單位。
stp-trailer = RSTP / MSTP 延伸 (version-1 長度、v3 長度、MSTI 記錄)；不透明的八位元組。
lacp-subtype = Slow Protocols 子類型 (1 = LACP)。
lacp-actor-system = Actor 系統 MAC 位址。
lacp-actor-state = Actor 狀態旗標 (activity、timeout、aggregation、sync、collecting、distributing、defaulted、expired)。
lacp-partner-system = Partner 系統 MAC 位址。
lacp-partner-state = Partner 狀態旗標。
marker-subtype = Slow Protocols 子類型 (2 = Marker)。
eapol-version = EAPOL 協定版本。
eapol-type = EAPOL 封包類型 (0 = EAP，1 = Start，2 = Logoff，3 = Key)。
eapol-length = EAPOL 本文長度，以八位元組為單位 (Auto)。
eapol-body = EAPOL 本文 (類型 0 時為 EAP 封包)；不透明的八位元組。
pppoe-version = PPPoE 協定版本 (1)。
pppoe-type = PPPoE 類型 (1)。
pppoe-code = PPPoE 代碼 (PADI / PADO / PADR / PADS / PADT，0 = 工作階段資料)。
pppoe-session-id = PPPoE 工作階段識別碼。
pppoe-length = PPPoE 酬載長度，以八位元組為單位 (Auto)。
pppoe-tags = PPPoE 探索 TAG (16 位元類型、16 位元長度)。
ppp-protocol = 酬載的 PPP 協定識別碼 (LCP / IPCP / IP / IPv6 / ...)。
ppp-cp-code = PPP 控制協定代碼 (Configure-Request / Ack / Nak / ...)。
ppp-cp-identifier = PPP 控制協定請求識別碼。
ppp-cp-length = PPP 控制協定封包長度，以八位元組為單位 (Auto)。
ppp-cp-options = PPP 控制協定選項 (8 位元類型 / 長度 TLV)。
vxlan-flags = VXLAN 旗標 (I 位元 = VNI 有效)。
vxlan-vni = 24 位元 VXLAN 網路識別碼 (VNI)。
gre-flags = GRE 旗標與版本 (檢查碼 / 金鑰 / 序號存在位元)。
gre-protocol-type = 封裝酬載的 EtherType。
nvgre-vsid = 24 位元 NVGRE 虛擬子網路 ID (VSID)。
nvgre-flowid = NVGRE FlowID (負載平衡用的熵值)。
mpls-label = 20 位元 MPLS 標籤值。0-15 為保留值 (0 / 2 explicit null，1 router alert，3 implicit null，7 entropy label indicator，13 GAL，14 OAM alert，15 extension)；SR-MPLS SID 為 SRGB 基底 + SID 索引。
mpls-tc = 3 位元流量類別 (QoS / ECN)。
mpls-s = 堆疊底部位元 (1 = 最後一個標籤；依下一層 Auto)。
mpls-ttl = MPLS 存活時間 (TTL)。
gtpu-version = GTP 版本 (1)。
gtpu-pt = 協定類型 (1 = GTP，0 = GTP')。
gtpu-e = 延伸標頭存在旗標。
gtpu-s = 序號存在旗標。
gtpu-pn = N-PDU 編號存在旗標。
gtpu-message-type = GTP-U 訊息類型 (255 = G-PDU)。
gtpu-length = 必要標頭之後的酬載長度，以八位元組為單位 (Auto)。
gtpu-teid = 通道端點識別碼 (TEID)。
geneve-version = Geneve 版本 (0)。
geneve-opt-len = 選項區域長度，以 4 個八位元組 (word) 為單位。
geneve-o = OAM 封包旗標。
geneve-c = 存在關鍵選項旗標。
geneve-protocol-type = 封裝酬載的 EtherType。
geneve-vni = 24 位元 Geneve 虛擬網路識別碼 (VNI)。
geneve-options = Geneve 選項 (不透明；大小由 opt_len 決定)。
l2tp-flags = L2TP 旗標與版本 (type、length、sequence、offset、priority 位元)。
l2tp-body = L2TP 本文 (依存在條件而定的欄位 + 酬載)；不透明的八位元組。
l2tpv3-session-id = L2TPv3 工作階段 ID。

# --- 封包建構器 (M4 第 2 階段) ---
ui-bld-palette = 協定
ui-bld-search = 搜尋協定
ui-bld-favorite = 我的最愛
ui-bld-favorites = 我的最愛
ui-bld-layer-l2 = 資料連結層 (L2)
ui-bld-layer-l3 = 網路層 (L3)
ui-bld-layer-l4 = 傳輸層 (L4)
ui-bld-layer-l7 = 應用層 (L7)
ui-bld-layer-payload = 酬載
ui-bld-stack = 層
ui-bld-empty = 將協定拖曳到這裡，或從協定面板新增。
ui-bld-unknown = 未知
ui-bld-dup = 複製層
ui-bld-suggest = 新增下一層
ui-bld-binding-warn = 下層未指出此協定
ui-bld-fix = 修正
ui-bld-added = 已新增 { $id }。
ui-bld-moved = 已移至位置 { $index }。
ui-bld-removed = 已移除層。
ui-bld-fields = 欄位編輯器
ui-bld-select-layer = 選擇一個層以編輯其欄位。
ui-bld-auto = 自動
ui-bld-manual = 手動
ui-bld-gen = 產生器
ui-bld-computed = 建構時計算
ui-bld-nsap-parts = AFI { $afi } · 區域 { $area } · 系統 ID { $sys } · NSEL { $nsel }
ui-bld-estimate = 串流中的封包數與總大小
ui-bld-expands = 展開為 { $count } 個 (~{ $bytes })
ui-bld-rate = 速率
ui-bld-rate-hint = 隨範本儲存的傳送速率 (1000pps、1.5Mpps、10Gbps、ifg=10us)
ui-bld-duration = 傳送約需 { $time }
ui-bld-duration-unknown = 傳送時間：請設定速率
ui-bld-duration-hint = 以此速率傳送的封包數 (tx.count，否則為串流長度)，上限為 tx.duration_secs
ui-bld-gen-kind = 種類
ui-bld-gen-count = 數量：
ui-bld-gen-shorthand = 範圍 / 清單 / CIDR
ui-bld-gen-random = 隨機
ui-bld-gen-random-mac = 隨機 MAC
ui-bld-gen-counter = 計數器
ui-bld-gen-file = 檔案
ui-bld-gen-expr = 運算式
ui-bld-hex = 十六進位
ui-bld-tree = 樹狀
ui-bld-no-bytes = 尚無封包。
ui-bld-name = 範本名稱
ui-bld-need-name = 請先為範本命名。
ui-bld-unsaved = 未儲存的變更
ui-bld-save-as = 另存新檔…
ui-bld-save-first = 建立工作前請先儲存範本。
ui-bld-recompute = 重新計算
ui-bld-auto-recompute = 每次編輯時即時重新計算預覽
ui-bld-index = 串流索引
ui-bld-index-prev = 上一個封包
ui-bld-index-next = 下一個封包
ui-bld-undo = 復原
ui-bld-redo = 重做
ui-bld-parity-ok = 瀏覽器中的預覽與伺服器算繪結果一致。
ui-bld-parity-ok-short = 一致
ui-bld-parity-bad = 瀏覽器中的預覽與伺服器算繪結果不一致。
ui-bld-parity-bad-short = 不一致
ui-bld-parity-pending = 正在檢查伺服器算繪結果…
ui-bld-help-title = 建構器快速鍵
ui-bld-help-add = 新增目前焦點所在的協定
ui-bld-help-move = 移動選取的層
ui-bld-help-del = 移除選取的層
ui-bld-help-undo = 復原
ui-bld-help-redo = 重做
ui-bld-help-hex = 在游標所在位元組輸入十六進位值
ui-bld-help-caret = 移動十六進位游標
ui-bld-suggest-prev = 在上方新增層

# --- P1 備援 / 多點傳播協定欄位說明 ---
vrrp-version = VRRP 版本 (2 = RFC 3768，3 = RFC 5798)。
vrrp-type = VRRP 封包類型 (1 = Advertisement)。
vrrp-vrid = 虛擬路由器識別碼。
vrrp-priority = 傳送端路由器優先順序 (255 = 位址擁有者，0 = 停止參與)。
vrrp-count = 通告中的虛擬位址數量 (Auto)。
vrrp-auth-type = VRRPv2 驗證類型 (0 = 無，1 = 簡單文字，2 = IP AH)。
vrrp-adver-int = VRRPv2 通告間隔 (秒)。
vrrp-max-adver-int = VRRPv3 最大通告間隔，以百分之一秒為單位。
vrrp-checksum = 涵蓋 VRRP 訊息的網際網路檢查碼 (VRRPv3 包含 IP 虛擬標頭) (Auto)。
vrrp-addresses = 虛擬路由器位址。
vrrp-auth-data = VRRPv2 驗證資料 (8 個八位元組)。
hsrp-version = HSRP 版本 (HSRPv1 為 0)。
hsrp-op-code = HSRP 操作 (0 = Hello，1 = Coup，2 = Resign，3 = Advertise)。
hsrp-state = 傳送端狀態 (1 = Learn，2 = Listen，4 = Speak，8 = Standby，16 = Active)。
hsrp-hello-time = Hello 訊息的傳送間隔 (秒)。
hsrp-hold-time = Hello 的有效時間 (秒)。
hsrp-priority = 群組內的路由器優先順序。
hsrp-group = Standby 群組編號。
hsrp-auth-data = 明文驗證密碼 (8 個八位元組，預設為 "cisco")。
hsrp-virtual-ip = 群組的虛擬 IPv4 位址。
hsrpv2-tlvs = HSRPv2 TLV (Group State、Interface State、Text / MD5 Authentication；8 位元類型與長度)。
glbp-version = GLBP 版本 (1)。
glbp-group = GLBP 群組編號。
glbp-owner-id = 傳送端路由器的 MAC 位址。
glbp-tlvs = GLBP TLV (Hello、Request/Response、Authentication；長度包含標頭)。
igmp-type = IGMP 訊息類型 (依本文 Auto：0x11 query，0x12 / 0x16 report，0x17 leave，0x22 v3 report)。
igmp-max-resp-code = 最大回應時間 / 代碼，以 1/10 秒為單位 (query 使用；report 中為零)。
igmp-checksum = 涵蓋整個 IGMP 訊息的網際網路檢查碼 (Auto)。
igmp-group = 多點傳播群組位址 (一般查詢中為 0.0.0.0)。
igmp-s-flag = 抑制路由器端處理旗標。
igmp-qrv = 查詢者的強健性變數 (QRV)。
igmp-qqic = 查詢者的查詢間隔代碼 (QQIC)。
igmp-num-sources = 來源位址數量。
igmp-sources = 來源位址。
igmp-num-records = 群組記錄數量。
igmp-records = 群組記錄 (記錄類型、輔助資料長度、來源、多點傳播位址)。
mld-max-resp = 最大回應延遲 / 代碼 (毫秒)。
mld-address = IPv6 多點傳播位址 (一般查詢中為 ::)。
pim-version = PIM 版本 (2)。
pim-type = PIM 訊息類型 (依本文 Auto：0 Hello，1 Register，3 Join/Prune)。
pim-checksum = 網際網路檢查碼 (整個訊息；Register 僅涵蓋前 8 個八位元組) (Auto)。
pim-hello-options = PIM Hello 選項 (Holdtime、LAN Prune Delay、DR Priority、Generation ID、Address List)。
pim-addr-family = 編碼位址的位址家族 (1 = IPv4)。
pim-upstream-neighbor = Join/Prune 所指向的上游鄰居位址。
pim-num-groups = 群組集數量。
pim-holdtime = Join/Prune 狀態的保留時間 (秒)。
pim-groups = 群組集 (編碼群組、已加入與已修剪的編碼來源)。
pim-register-border = Border 位元 (傳送端為 PIM 多點傳播邊界路由器)。
pim-register-null = Null-Register 位元 (未封裝資料封包)。

# --- P1 應用層協定欄位說明 ---
dhcp-op = BOOTP 訊息操作碼 (1 = BOOTREQUEST，2 = BOOTREPLY)。
dhcp-htype = 硬體位址類型 (1 = Ethernet)。
dhcp-hlen = 硬體位址長度，以八位元組為單位 (Ethernet 為 6)。
dhcp-hops = 中繼躍點數，每經過一個中繼代理程式加一。
dhcp-xid = 由用戶端選擇的交易 ID。
dhcp-secs = 自用戶端開始取得位址以來經過的秒數。
dhcp-flags = BOOTP 旗標 (位元 15 = 要求以廣播回覆)。
dhcp-ciaddr = 用戶端 IP 位址 (已繫結時)。
dhcp-yiaddr = 伺服器指派的「Your」(用戶端) IP 位址。
dhcp-siaddr = 下一個伺服器的 IP 位址 (開機伺服器)。
dhcp-giaddr = 中繼代理程式 (閘道) IP 位址；經中繼時不為零。
dhcp-chaddr = 用戶端硬體 (MAC) 位址；後接 10 個八位元組的填補。
dhcp-sname = 選用的伺服器主機名稱 (64 個八位元組，以 NUL 填補)。
dhcp-file = 開機檔案名稱 (128 個八位元組，以 NUL 填補)。
dhcp-magic-cookie = DHCP magic cookie (0x63825363)。
dhcp-options = DHCP 選項 (8 位元代碼 / 長度；Pad、End、Message Type、Relay Agent Information、...)。
dhcpv6-msg-type = DHCPv6 訊息類型 (Solicit、Advertise、Request、Reply、Relay-Forward、...)。
dhcpv6-transaction-id = 24 位元交易 ID。
dhcpv6-options = DHCPv6 選項 (16 位元代碼 / 長度；Client ID、IA_NA、Relay Message、...)。
dhcpv6-hop-count = 已轉送此訊息的中繼代理程式數量。
dhcpv6-link-address = 識別用戶端所在連結的位址 (中繼)。
dhcpv6-peer-address = 訊息來源用戶端或中繼的位址。
dns-id = 查詢識別碼，會複製到回應中。
dns-qr = 查詢 (0) 或回應 (1)。
dns-opcode = 查詢種類 (0 = 標準查詢，4 = NOTIFY，5 = UPDATE)。
dns-aa = 授權回答旗標。
dns-tc = 截斷旗標。
dns-rd = 要求遞迴旗標。
dns-ra = 可遞迴旗標。
dns-z = 保留位元 (必須為 0)。
dns-ad = 已驗證資料旗標 (DNSSEC)。
dns-cd = 停用檢查旗標 (DNSSEC)。
dns-rcode = 回應代碼 (0 = NOERROR，3 = NXDOMAIN，...)。
dns-qdcount = 問題區段中的項目數。
dns-ancount = 答案區段中的資源記錄數。
dns-nscount = 授權區段中的資源記錄數。
dns-arcount = 附加區段中的資源記錄數。
dns-sections = 問題 / 答案 / 授權 / 附加區段 (以長度為前綴的標籤名稱)；不透明的八位元組。
ntp-li = 閏秒指示 (3 = 時脈未同步)。
ntp-version = NTP 版本號碼 (4)。
ntp-mode = 關聯模式 (3 = 用戶端，4 = 伺服器，5 = 廣播)。
ntp-stratum = 層級 (0 = 未指定，1 = 主要參考，2-15 = 次要)。
ntp-poll = 最大輪詢間隔，以 log2 秒表示 (有號數)。
ntp-precision = 時脈精確度，以 log2 秒表示 (有號數)。
ntp-root-delay = 到參考時脈的往返延遲 (16.16 定點數，秒)。
ntp-root-dispersion = 到參考時脈的離散度 (16.16 定點數，秒)。
ntp-reference-id = 參考 ID (stratum 1 時為 ASCII 代碼，否則為 IPv4 位址或雜湊)。
ntp-reference-ts = 系統時脈最後一次設定的時間 (64 位元 NTP 時間戳記)。
ntp-origin-ts = 由伺服器回傳的用戶端傳送時間 (64 位元 NTP 時間戳記)。
ntp-receive-ts = 請求抵達伺服器的時間 (64 位元 NTP 時間戳記)。
ntp-transmit-ts = 封包離開傳送端的時間 (64 位元 NTP 時間戳記)。
ntp-extensions = 延伸欄位與 MAC；不透明的八位元組。
snmp-tag = BER 標籤八位元組 (0x30 SEQUENCE，0x02 INTEGER，0x04 OCTET STRING)。
snmp-msg-length = SNMP 訊息 SEQUENCE 的 BER 長度 (Auto；短格式，小於 128)。
snmp-version-length = 版本 INTEGER 的 BER 長度 (1)。
snmp-version = SNMP 版本 (0 = v1，1 = v2c)。
snmp-community-length = Community 字串的 BER 長度，以八位元組為單位。
snmp-community = Community 字串 (例如 "public")。
snmp-pdu-type = PDU 標籤 (0xa0 GetRequest，0xa2 Response，0xa4 v1 Trap，0xa7 SNMPv2-Trap，...)。
snmp-pdu-length = PDU 內容的 BER 長度 (Auto；短格式，小於 128)。
snmp-pdu-body = 以 BER TLV 表示的 PDU 內容：request-id、error-status、error-index 與 variable-bindings SEQUENCE。
snmp-trap-body = 以 BER TLV 表示的 Trap 內容：enterprise、agent-addr、generic-trap、specific-trap、time-stamp 與 variable bindings。
syslog-message = Syslog 訊息文字，以 <PRI> (facility * 8 + severity) 開頭。
radius-code = RADIUS 封包類型 (Access-Request、Access-Accept、Accounting-Request、...)。
radius-identifier = 用於配對請求與回覆的識別碼。
radius-length = 整個 RADIUS 封包的長度，以八位元組為單位 (Auto)。
radius-authenticator = 16 個八位元組的 Request / Response Authenticator (Manual)。
radius-attributes = RADIUS 屬性 (8 位元類型、包含標頭的 8 位元長度)。
tftp-opcode = TFTP 操作碼 (1 = RRQ，2 = WRQ，3 = DATA，4 = ACK，5 = ERROR，6 = OACK)。
tftp-filename = 檔案名稱 (以 NUL 結尾)。
tftp-mode = 傳輸模式：netascii、octet 或 mail (以 NUL 結尾)。
tftp-options = 選項名稱 / 值字串配對 (RFC 2347)：blksize、timeout、tsize、windowsize、...
tftp-block = 區塊編號 (從 1 開始)。
tftp-data = 資料八位元組 (每個區塊 512 個；較短的區塊表示傳輸結束)。
tftp-error-code = TFTP 錯誤代碼 (1 = 找不到檔案，2 = 存取違規，...)。
tftp-error-message = 錯誤訊息 (以 NUL 結尾)。

# --- P1 路由協定欄位說明 ---
ospf-version = OSPF 版本 (2 = OSPFv2，3 = OSPFv3)。
ospf-type = OSPF 封包類型 (Hello / DB Description / LS Request / LS Update / LS Ack；依下一層 Auto)。
ospf-packet-length = 包含標頭的 OSPF 封包長度，以八位元組為單位 (Auto)。
ospf-router-id = 傳送端路由器的 Router ID。
ospf-area-id = 封包所屬的區域 (0.0.0.0 = 骨幹)。
ospf-checksum = 涵蓋 OSPF 封包 (不含驗證欄位) 的網際網路檢查碼 (Auto)。
ospf-au-type = 驗證類型 (0 = null，1 = 簡單密碼，2 = 加密)。
ospf-authentication = 64 位元驗證欄位 (密碼，或加密驗證時的金鑰 ID / 序號)。
ospf-network-mask = 介面或通告網路的網路遮罩。
ospf-hello-interval = Hello 封包的傳送間隔 (秒)。
ospf-options = 選用功能 (E、MC、N/P、L、DC、O、DN 位元)。
ospf-router-priority = DR 選舉用的路由器優先順序 (0 = 永不成為 DR)。
ospf-router-dead-interval = 未收到 Hello 經過多少秒後宣告鄰居失效。
ospf-designated-router = 指定路由器 (DR) (0.0.0.0 = 無)。
ospf-backup-designated-router = 備援指定路由器 (BDR) (0.0.0.0 = 無)。
ospf-neighbors = 在連結上看到的鄰居 Router ID。
ospf-interface-mtu = 介面在不分段的情況下可傳送的最大 IP 資料包。
ospf-dbd-flags = Database Description 旗標 (I = init，M = more，MS = master)。
ospf-dd-sequence = Database Description 序號。
ospf-lsa-headers = LSA 標頭 (每個 20 個八位元組；檢查碼與長度為 Manual)。
ospf-lsr-requests = 請求的 LSA (LS type、Link State ID、advertising router)。
ospf-num-lsas = 更新中的 LSA 數量。
ospf-ls-age = LSA 產生後經過的時間 (秒) (若要 Auto 檢查碼完全正確請保持為 0)。
ospf-ls-type = LSA 類型 (依 LSA 本文層 Auto；9 / 10 / 11 = opaque LSA)。
ospf-link-state-id = 識別所描述網路部分的 Link State ID。
ospf-advertising-router = 產生此 LSA 的路由器之 Router ID。
ospf-ls-sequence = LSA 序號 (0x80000001 = 初始值)。
ospf-ls-checksum = 不含 LS age 的 LSA Fletcher 檢查碼 (Auto)。
ospf-lsa-length = 包含標頭的 LSA 長度，以八位元組為單位 (Auto)。
ospf-router-lsa-flags = Router-LSA 旗標 (V = 虛擬連結端點，E = ASBR，B = ABR)。
ospf-num-links = 描述的路由器連結數量。
ospf-router-links = 路由器連結 (link ID、link data、類型、TOS 數量、度量)。
ospf-attached-routers = 連接到此網路的路由器 Router ID。
ospf-metric = 路由成本。
ospf-external-e = 外部度量類型 (1 = type 2，0 = type 1)。
ospf-forwarding-address = 目的地流量要轉送到的位址 (0.0.0.0 = ASBR)。
ospf-external-route-tag = 隨外部路由攜帶的 32 位元標記。
ospf-opaque-ls-type = Opaque LSA 泛洪範圍 (9 = link-local，10 = area，11 = AS)。
ospf-opaque-type = Opaque 類型 (1 = TE，4 = Router Information，7 = Extended Prefix，8 = Extended Link；依本文層 Auto)。
ospf-opaque-id = 用於區分同一 opaque 類型 LSA 的 Opaque ID (24 位元)。
ospf-opaque-tlvs = Opaque LSA 的 TLV (16 位元類型與長度，值填補至 4 個八位元組；長度為 Auto)。
ospfv3-checksum = 涵蓋 OSPFv3 封包與 IPv6 虛擬標頭的網際網路檢查碼 (Auto)。
ospfv3-instance-id = Instance ID (區分同一連結上的 OSPFv3 執行個體)。
ospfv3-interface-id = 傳送端介面的 Interface ID。
ospfv3-options = OSPFv3 選項 (V6、E、N、R、DC、AF 位元)。
ospfv3-ls-type = 含 U / S 範圍位元的 LSA 功能代碼 (依 LSA 本文層 Auto)。
ospfv3-router-links = 路由器介面 (類型、度量、interface ID、鄰居 Router ID)。
ospfv3-prefix-length = 前綴長度 (位元)。
ospfv3-prefix-options = 前綴選項 (NU、LA、P、DN 位元)。
ospfv3-address-prefix = 位址前綴，填補至 4 個八位元組的倍數。
ospfv3-external-flags = AS-External-LSA 旗標 (E = type 2 度量，F = 存在轉送位址，T = 存在路由標記)。
ospfv3-referenced-ls-type = 所參照 LSA 的 LS 類型。
ospfv3-link-local-address = 介面的鏈路本地位址。
ospfv3-num-prefixes = 後續的前綴數量。
ospfv3-prefixes = 前綴項目 (長度、選項、度量 / 保留、位址前綴)；不透明的八位元組。
ospfv3-referenced-link-state-id = 所參照 LSA 的 Link State ID。
ospfv3-referenced-advertising-router = 所參照 LSA 的 advertising router。
bgp-marker = 16 個八位元組的標記 (全為 1)。
bgp-length = 包含標頭的 BGP 訊息長度，以八位元組為單位 (Auto)。
bgp-type = BGP 訊息類型 (OPEN / UPDATE / NOTIFICATION / KEEPALIVE / ROUTE-REFRESH；依下一層 Auto)。
bgp-version = BGP 版本 (4)。
bgp-my-as = 傳送端的自治系統編號 (4 個八位元組 AS 時為 23456 = AS_TRANS)。
bgp-hold-time = 提議的保持時間 (秒) (0 或至少 3)。
bgp-identifier = 傳送端的 BGP 識別碼 (Router ID)。
bgp-opt-params-length = 選用參數長度，以八位元組為單位 (Auto)。
bgp-opt-params = 選用參數 (類型 2 = capabilities)。
bgp-withdrawn-routes-length = 撤回路由的長度，以八位元組為單位 (Auto)。
bgp-withdrawn-routes = 撤回的 IPv4 路由 (前綴長度 (位元)、前綴)。
bgp-total-path-attribute-length = 路徑屬性長度，以八位元組為單位 (Auto)。
bgp-path-attributes = 路徑屬性 (旗標 + 類型代碼；設定 Extended Length 旗標時長度為 16 位元，超過 255 個八位元組時自動設定)。
bgp-nlri = 通告的 IPv4 路由 (前綴長度 (位元)、前綴)；其他位址家族放在 MP_REACH_NLRI 中。
bgp-error-code = NOTIFICATION 錯誤代碼。
bgp-error-subcode = NOTIFICATION 錯誤子代碼。
bgp-error-data = NOTIFICATION 的診斷資料。
bgp-afi = 位址家族識別碼 (1 = IPv4，2 = IPv6)。
bgp-rr-subtype = ROUTE-REFRESH 子類型 (0 = normal，1 = BoRR，2 = EoRR)。
bgp-safi = 後續位址家族識別碼 (1 = 單點傳播，2 = 多點傳播)。
rip-command = RIP 命令 (1 = 請求，2 = 回應)。
rip-version = RIP 版本 (1 或 2；RIPng 使用 1)。
rip-entries = 路由項目 (AFI、路由標記、位址、遮罩、下一躍點、度量)。
ripng-entries = 路由表項目 (前綴、路由標記、前綴長度、度量；0xff = 下一躍點)。
eigrp-version = EIGRP 標頭版本 (2)。
eigrp-opcode = EIGRP 操作碼 (Update / Query / Reply / Hello / SIA-Query / SIA-Reply)。
eigrp-checksum = 涵蓋 EIGRP 封包的網際網路檢查碼 (Auto)。
eigrp-flags = EIGRP 旗標 (Init、Conditional Receive、Restart、End of Table)。
eigrp-sequence = 可靠傳遞用的序號。
eigrp-acknowledge = 確認的序號 (0 = 無)。
eigrp-virtual-router-id = 虛擬路由器 ID (0 = 單點傳播位址家族)。
eigrp-autonomous-system = EIGRP 自治系統編號。
eigrp-tlvs = EIGRP TLV (16 位元類型、包含標頭的 16 位元長度)。

# --- P2 OSI 封裝欄位說明 ---
osi-nlpid = 網路層協定識別碼 (ISO/IEC TR 9577)：0x81 CLNP，0x82 ES-IS，0x83 IS-IS。
osi-data = NLPID 之後的其餘 OSI PDU (不透明的八位元組)。
clnp-nlpid = 網路層協定識別碼 (0x81 = CLNP)。
clnp-length-indicator = 到選項結尾為止的標頭長度，以八位元組為單位 (Auto)。
clnp-version = 版本 / 協定 ID 延伸 (1)。
clnp-lifetime = 剩餘存活時間，以 500 ms 為單位。
clnp-sp = SP：允許分段；存在分段部分 (依分段部分 Auto)。
clnp-ms = MS：後面還有分段。
clnp-er = E/R：若 PDU 被丟棄則傳送錯誤報告。
clnp-pdu-type = PDU 類型：0x1c DT (資料)，0x1d MD (多點傳播資料)，0x01 ER (錯誤報告)，0x1e ERQ / 0x1f ERP (echo 請求 / 回覆)。
clnp-segment-length = 整個 PDU (標頭與資料) 的長度 (Auto)。
clnp-checksum = 標頭的 ISO 8473 Fletcher 檢查碼 (Auto)。0 = 不使用檢查碼。
clnp-destination = 目的 NSAP 位址。
clnp-source = 來源 NSAP 位址。
clnp-segmentation = 分段部分 (資料單元識別碼、分段偏移、總長度)，設定 SP 時存在。不會計算總長度。
clnp-options = 到長度指示為止的選項 (8 位元代碼 / 8 位元長度)：安全性、QoS 維護、優先順序、來源路由、路由記錄、填補、丟棄原因。
esis-nlpid = 網路層協定識別碼 (0x82 = ES-IS)。
esis-length-indicator = PDU 長度，以八位元組為單位 (Auto)。
esis-version = 版本 / 協定 ID 延伸 (1)。
esis-pdu-type = PDU 類型：2 ESH (end system hello)，4 ISH (intermediate system hello)，6 RD (redirect)。
esis-holding-time = 保持時間 (秒)。
esis-checksum = PDU 的 ISO 8473 Fletcher 檢查碼 (Auto)。0 = 不使用檢查碼。
esis-number-of-source-addresses = 來源位址數量 (Auto)。
esis-source-addresses = 終端系統所服務的 NSAP 位址。
esis-net = 中介系統的網路實體名稱 (NET) (redirect：下一躍點為終端系統時為空)。
esis-destination = 被重新導向的目的 NSAP 位址。
esis-bsnpa-length = 子網路位址長度 (Auto)。
esis-bsnpa = 較佳下一躍點的子網路位址 (例如 MAC 位址)。
esis-options = 到長度指示為止的選項 (8 位元代碼 / 8 位元長度)：安全性、QoS 維護、優先順序、位址遮罩、SNPA 遮罩、建議的 ES 設定計時器。
esis-data = 固定部分之後的其餘 PDU (不透明的八位元組)。
isis-irpd = 網域內路由協定鑑別碼 (0x83 = IS-IS)。
isis-header-length = 固定標頭長度，以八位元組為單位 (每種 PDU 類型固定：LAN Hello / LSP 27，P2P Hello 20，CSNP 33，PSNP 17)。
isis-version-ext = 版本 / 協定 ID 延伸 (1)。
isis-id-length = 系統 ID 長度 (0 = 6 個八位元組)。
isis-pdu-type = PDU 類型 (15/16 LAN Hello L1/L2，17 P2P Hello，18/20 LSP L1/L2，24/25 CSNP，26/27 PSNP)。
isis-version = 版本 (1)。
isis-max-area-addresses = 區域位址的最大數量 (0 = 3)。
isis-pdu-length = 整個 PDU 的長度 (Auto)。在 Hello 中，Manual 值會以 Padding TLV 將 PDU 填補到該長度 (例如 MTU 為 1500 時設為 1497)。
isis-tlvs = TLV (8 位元代碼 / 8 位元長度) 及其子 TLV。
isis-source-id = 傳送端的系統 ID。
isis-holding-time = 保持時間 (秒)。
isis-priority = 指定 IS 選舉的優先順序 (0–127)。
isis-lan-id = LAN ID：指定 IS 的系統 ID 與虛擬節點 ID。
isis-circuit-type = 電路類型：1 = L1，2 = L2，3 = L1/L2。
isis-local-circuit-id = 本地電路 ID。
isis-remaining-lifetime = LSP 的剩餘存活時間 (秒) (不在檢查碼涵蓋範圍內)。
isis-lsp-id = LSP ID：系統 ID、虛擬節點 ID 與分段編號。
isis-sequence-number = LSP 序號。
isis-lsp-checksum = 從 LSP ID 到 PDU 結尾的 Fletcher 檢查碼 (Auto)。
isis-partition-repair = P 位元：支援分割修復。
isis-attached = ATT 位元：連接到其他區域 (default / delay / expense / error 度量)。
isis-overload = OL 位元：LSP 資料庫超載。
isis-is-type = IS 類型：1 = L1，3 = L1/L2。
isis-snp-source-id = 傳送端的系統 ID 與電路 ID (0)。
isis-start-lsp-id = CSNP 涵蓋的第一個 LSP ID。
isis-end-lsp-id = CSNP 涵蓋的最後一個 LSP ID。
isis-data = 共通標頭之後的其餘 PDU (不透明的八位元組)。

# --- P2 SCTP 欄位說明 ---
sctp-src-port = 來源連接埠。
sctp-dst-port = 目的連接埠。
sctp-verification-tag = 驗證標籤：對端的 initiate tag (攜帶 INIT 的封包中為 0)。
sctp-checksum = 將此欄位設為零後整個 SCTP 封包的 CRC32c (Auto)，最低有效八位元組先傳送。
sctp-chunks = 區塊 (8 位元類型、8 位元旗標、包含 4 個八位元組標頭的 16 位元長度、以零填補至 4 個八位元組)：DATA、INIT、INIT ACK、SACK、HEARTBEAT、ABORT、SHUTDOWN、ERROR、COOKIE ECHO / ACK、AUTH、I-DATA、ASCONF、RE-CONFIG、FORWARD TSN、PAD。長度、填補與 SACK 計數為 Auto。

# --- P2 LDP / RSVP-TE / BFD 欄位說明 ---
ldp-version = LDP 版本 (1)。
ldp-pdu-length = 版本與 PDU 長度欄位之後的 PDU 長度 (Auto)。
ldp-lsr-id = LSR ID：LDP 識別碼的前四個八位元組。
ldp-label-space = 標籤空間 ID：LDP 識別碼的最後兩個八位元組 (0 = 全平台)。
ldp-messages = 訊息 (含 U 位元的 16 位元類型、值的 16 位元長度、訊息 ID、TLV)：Notification、Hello、Initialization、KeepAlive、Address / Address Withdraw、Label Mapping / Request / Withdraw / Release / Abort Request。TLV：FEC、Address List、Hop Count、Path Vector、Generic Label、Status、Common Hello / Session Parameters、傳輸位址、Configuration Sequence Number。長度為 Auto。
rsvp-version = RSVP 版本 (1)。
rsvp-flags = 旗標 (位元 0：支援 refresh reduction，RFC 2961)。
rsvp-msg-type = 訊息類型 (1 Path，2 Resv，3 PathErr，4 ResvErr，5 PathTear，6 ResvTear，7 ResvConf，12 Bundle，13 Ack，15 Srefresh，20 Hello)。
rsvp-checksum = 將此欄位設為零後整個訊息的網際網路檢查碼 (Auto)。
rsvp-send-ttl = 傳送訊息時使用的 IP TTL。
rsvp-reserved = 保留 (0)。
rsvp-length = 包含共通標頭的整個訊息長度，以八位元組為單位 (Auto)。
rsvp-objects = 物件 (包含 4 個八位元組標頭的 16 位元長度、Class-Num、C-Type)：SESSION、RSVP_HOP、TIME_VALUES、ERROR_SPEC、SCOPE、STYLE、FLOWSPEC、FILTER_SPEC、SENDER_TEMPLATE、SENDER_TSPEC、RESV_CONFIRM、LABEL、LABEL_REQUEST、EXPLICIT_ROUTE、RECORD_ROUTE、HELLO、MESSAGE_ID、SESSION_ATTRIBUTE。長度、工作階段名稱長度與填補為 Auto。
bfd-version = BFD 版本 (1)。
bfd-diag = 診斷代碼：本地工作階段最後一次狀態變更的原因。
bfd-state = 工作階段狀態：0 AdminDown，1 Down，2 Init，3 Up。
bfd-p = P 位元：Poll (參數變更，對端以 F 回應)。
bfd-f = F 位元：Final (對 Poll 的回應)。
bfd-c = C 位元：Control Plane Independent。
bfd-a = A 位元：存在驗證 (Auto：有驗證區段時設定)。
bfd-d = D 位元：Demand 模式。
bfd-m = M 位元：Multipoint (0)。
bfd-detect-mult = 偵測時間倍數。
bfd-length = 包含驗證區段的控制封包長度，以八位元組為單位 (Auto)。
bfd-my-discriminator = My discriminator：傳送端上此工作階段唯一的非零值。
bfd-your-discriminator = Your discriminator：對端的 discriminator (未知時為 0)。
bfd-desired-min-tx = 期望的最小傳送間隔 (微秒)。
bfd-required-min-rx = 要求的最小接收間隔 (微秒)。
bfd-required-min-echo-rx = 要求的最小 echo 接收間隔 (微秒) (0 = 不使用 echo)。
bfd-authentication = 驗證區段 (8 位元類型、包含標頭的 8 位元長度)：簡單密碼、keyed MD5 / SHA1 及其 meticulous 變體。摘要 / 雜湊為 Manual。

# --- 產生的文件 (`wpg docs gen`，crates/wpg-cli/src/cmd/docs) ---
docs-gen-no-root = { $path } 不是儲存庫根目錄 (沒有 docs/en/src/SUMMARY.md)；請指定 --root。
docs-gen-error = 產生文件失敗：{ $error }
docs-gen-stale = 已過時：{ $path }
docs-gen-extra = 已不再產生：{ $path }
docs-gen-up-to-date = 產生的文件已是最新狀態。
docs-gen-run-hint = 請執行 `just docs-gen` (或 `wpg docs gen`) 並提交結果。
docs-gen-done = 已產生文件：寫入 { $written } 個檔案，移除 { $removed } 個。
docgen-yes = 是
docgen-octets = { $octets } 個八位元組
docgen-items = { $count } 個項目
docgen-variant = 變體 { $n }
docgen-col-id = ID
docgen-col-name = 名稱
docgen-col-fields = 欄位
docgen-col-next-by = 下一層判定依據
docgen-col-notes = 備註
docgen-col-lower = 下層
docgen-col-selector = 選擇器
docgen-col-value = 值
docgen-col-protocol = 協定
docgen-col-note = 備註
docgen-col-level = 層級
docgen-col-rule = 規則
docgen-col-field = 欄位
docgen-col-type = 型別
docgen-col-default = 預設值
docgen-col-auto = Auto
docgen-col-description = 說明
docgen-col-bit = 位元
docgen-col-mask = 遮罩
docgen-col-type-value = 類型
docgen-col-value-fields = 值欄位
docgen-col-key = 鍵
docgen-col-required = 必要
docgen-col-in = 位置
docgen-col-status = 狀態
docgen-col-body = 本文
docgen-col-tag = 標籤
docgen-col-endpoint = 端點
docgen-col-summary = 摘要
docgen-col-env = 環境變數
docgen-layer-l2 = L2 (資料連結層)
docgen-layer-l3 = L3 (網路層)
docgen-layer-l4 = L4 (傳輸層)
docgen-layer-l7 = 應用與控制
docgen-layer-payload = 酬載
docgen-proto-title = 協定參考
docgen-proto-intro = 本章由 `wpg-protocols` 的協定定義 IR (`GET /api/protocols` 提供的目錄) 產生，因此永遠與程式碼一致：共 { $count } 個協定。每一頁依線路順序列出各欄位及其型別、預設值、Auto 規則與說明，以及通往此協定的下層、下一層對應表、建構後檢查的限制條件與內建的範例範本。手動撰寫的[協定說明](../protocols/overview.md)解釋了如何到達各協定、哪些欄位需要手動設定，以及仍存在的近似處理。變更定義後，請以 `just docs-gen` 重新產生。
docgen-proto-layer = 層
docgen-proto-header = 標頭
docgen-proto-header-fixed = { $octets } 個八位元組 (固定)
docgen-proto-header-variable = 可變長度
docgen-proto-min-frame = 最小訊框
docgen-proto-synth = 產生器
docgen-proto-synth-payload = 內建酬載產生器 (以下欄位為參數，而非線路上的欄位)
docgen-proto-pseudo = 虛擬標頭
docgen-proto-pseudo-fields = 提供 { $src } / { $dst } 給上層檢查碼
docgen-proto-notes = 備註
docgen-proto-final-dst = 最終目的地
docgen-proto-final-dst-text = 當 { $when } 不為 0 時，{ $list } 的元素 0 會取代下層虛擬標頭的目的位址 (RFC 8200 §8.1)
docgen-proto-inbound = 來源協定
docgen-proto-inbound-none = 目錄中沒有任何協定會通往此處：請將其作為第一層使用，或明確地堆疊。
docgen-proto-fields = 欄位
docgen-proto-params = 參數
docgen-proto-no-fields = 沒有欄位。
docgen-proto-next = 下一層
docgen-proto-next-intro = 下一層由 { $field } 選擇；當該欄位為 Auto 時，建構器會寫入堆疊在上方之協定的值。
docgen-proto-next-alt = 解碼時，若第一個欄位沒有符合的項目，也會查詢 { $field }。
docgen-proto-constraints = 限制條件
docgen-proto-samples = 範例範本
docgen-proto-samples-none = 目前尚無內建範本使用此協定。
docgen-proto-samples-more = 及其他 { $count } 個
docgen-bind-default = (其他任何值)
docgen-bind-min-len = 至少剩餘 { $octets } 個八位元組
docgen-bind-reparse = 重新解碼此層
docgen-bind-peek = 當 { $field } = { $value } 時，酬載的前 { $bits } 位元
docgen-bind-peek-skip = 當 { $field } = { $value } 時，酬載前 { $skip } 位元之後的 { $bits } 位元
docgen-level-error = 錯誤
docgen-level-warning = 警告
docgen-level-info = 資訊
docgen-c-range = { $field } 介於 { $min }..{ $max }
docgen-c-equals = { $field } = { $value }
docgen-c-not-equals = { $field } ≠ { $value }
docgen-c-min-length = { $span } 至少 { $octets } 個八位元組
docgen-c-max-length = { $span } 最多 { $octets } 個八位元組
docgen-span-header = 標頭
docgen-span-payload = 酬載
docgen-span-total = 標頭與酬載
docgen-unit-bytes = 八位元組
docgen-unit-words16 = 16 位元字組
docgen-unit-words32 = 32 位元字組
docgen-unit-words64 = 64 位元字組
docgen-auto-length = { $span } 的長度，以{ $unit }為單位
docgen-auto-checksum = 涵蓋{ $covers }的{ $algorithm }
docgen-auto-nonzero = 計算結果為 0 時以全 1 傳送
docgen-auto-exclude = { $fields } 以零計算
docgen-auto-next = 下一層的協定號碼 (請參閱「下一層」)
docgen-auto-count = { $field } 的項目數
docgen-auto-size = { $field } 的大小，以八位元組為單位
docgen-auto-counter = 每個串流的計數器 (目前使用預設值)
docgen-cksum-internet = 網際網路檢查碼
docgen-cksum-internet-pseudo = 含虛擬標頭的網際網路檢查碼
docgen-cksum-fletcher = Fletcher 檢查碼
docgen-cksum-crc32c = CRC-32C
docgen-cksum-crc32 = CRC-32
docgen-cover-header = 標頭
docgen-cover-all = 標頭及其後的所有內容
docgen-len-from-field = { $expr } 個八位元組
docgen-len-count = { $field } 個項目
docgen-len-until = 到群組的偏移 { $expr } 為止
docgen-len-to-end = PDU 的其餘部分
docgen-len-to-end-except = PDU 的其餘部分，但不含最後 { $octets } 個八位元組
docgen-len-until-terminator = 到結束符號 { $octets } 為止 (建構時附加)
docgen-len-until-tlv = 到第一個類型為 { $type } 的項目 (含) 為止，或到 PDU 結尾
docgen-tlv-uncounted = 長度之後的前 { $octets } 個值八位元組不計入長度。
docgen-list-item = 每個項目都是由下列欄位組成的群組。
docgen-tlv-format = 類型 { $type } 位元，長度 { $len } 位元。
docgen-tlv-len-first = 長度位於類型之前。
docgen-tlv-len-incl = 長度也包含類型與長度欄位。
docgen-tlv-len-excl = 長度只計算值。
docgen-tlv-unit = 長度單位：{ $unit }。
docgen-tlv-head = 前 { $octets } 個值八位元組位於類型與長度之間。
docgen-tlv-align = 每個項目以零填補至 { $align } 個八位元組的倍數 (不計入長度)
docgen-tlv-unpadded-last = 最後一個項目不填補
docgen-tlv-long = 類型位元 { $flag } 會選用 { $len } 位元的長度。
docgen-tlv-fill = { $field } 的 Manual 值會以 { $variant } 項目將區域填補到該大小。
docgen-tlv-unknown-raw = 未知的類型會保留為原始值八位元組。
docgen-tlv-unknown-stop = 遇到未知的類型時停止依類型解碼 (其餘部分成為一個原始項目)。
docgen-tlv-type-only = (僅類型，沒有長度或值)
docgen-tlv-when-prefix = 當值以 { $octets } 開頭時
docgen-tlv-when-len = 當值為 { $octets } 個八位元組時
docgen-notes-heading = 產生的參考資料
docgen-notes-intro = 本頁各協定的欄位參考 (由定義產生)：
docgen-cli-title = CLI 參考
docgen-cli-intro = `wpg` 執行檔同時執行伺服器 (`wpg serve`) 與無介面工具，因此也適用於 CI 與 ssh 工作階段。例如，`wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` 會傳送一個範本。本頁由命令定義產生：每個區塊都是目前建置版本 `--help` 輸出的原文。執行時的訊息依循 `LANG` / `LC_ALL`；`--help` 文字本身為英文。
docgen-cli-commands = 命令
docgen-cli-note-send = 傳送範本。`--plan` 用來選擇預先展開所有訊框 (`prerender`) 或在傳送時建構 (`onthefly`)。`--json` 會將最終報告輸出到 stdout。
docgen-cli-note-render = 將範本的訊框寫入 pcapng 檔案，不實際傳送。
docgen-cli-note-import = 將擷取檔 (或貼上的十六進位資料) 轉換為範本；請參閱 [pcap 重播教學](../tutorials/pcap-replay.md)。
docgen-cli-note-bench = 以無限制速率傳送合成訊框 (EtherType 0x88b5)，或以 `--template` 指定的範本。基準數值請參閱 `docs/perf.md`。`--json` 輸出欄位：backend、detail、iface、frame_size、batch_size、packets、dropped、pps、bps (L2)、l1_bps、elapsed。
docgen-cli-note-ifaces = 列出介面及其 MAC、位址、MTU、連結狀態與驅動程式。
docgen-cli-note-doctor = 執行[權限](../getting-started/permissions.md)中所述的環境檢查；任一檢查失敗時結束狀態不為零。
docgen-cli-note-template = 範本工具。`lint` 遇到錯誤時以狀態 1 結束；`export` 與 `import` 將範本打包至 `.wpgbundle` 封存檔或從中解開。
docgen-cli-note-config = `wpg.toml` 工具；請參閱[設定參考](config.md)。
docgen-api-title = API 參考
docgen-api-intro = wpg-server 在 `/api` 下提供 REST (JSON) API，並透過 WebSocket (`GET /api/ws`) 推送即時統計、工作狀態變更與記錄。本頁由伺服器的 OpenAPI 文件 (utoipa) 產生。執行中的伺服器會在 `/api/openapi.json` 提供相同的文件，並在 `/api/docs/` 提供 Swagger UI，副本則保存在 [`docs/schema/openapi.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/openapi.json)。請以 `POST /api/auth/login` 登入 (首次執行時使用 `POST /api/auth/setup`)；之後 `wpg_session` cookie 會驗證每個請求，而 POST / PUT / DELETE 還必須在 `X-CSRF-Token` 標頭中傳送 `GET /api/auth/status` 取得的 CSRF 權杖。錯誤會以 JSON 物件 `error` 傳回，包含 `code`、`message` (使用 `Accept-Language` 指定的語言) 與 `details`。以下的端點與結構描述說明皆來自程式碼。
docgen-api-endpoints = 端點
docgen-api-params = 參數
docgen-api-body = 請求本文
docgen-api-responses = 回應
docgen-api-schemas = 結構描述
docgen-api-schemas-intro = 上述參照的請求與回應本文。
docgen-api-tag-auth = 登入、登出與首次執行設定。
docgen-api-tag-system = 健康狀態、版本、介面、自我診斷與傳送後端。
docgen-api-tag-protocols = 協定目錄 (請參閱[協定參考](protocols.md))。
docgen-api-tag-packets = 封包的算繪、解碼與 lint 檢查。
docgen-api-tag-templates = 範本庫。
docgen-api-tag-jobs = 傳送工作與統計。
docgen-api-tag-events = WebSocket 事件串流。
docgen-config-title = 設定
docgen-config-intro = 伺服器會從工作目錄讀取 `wpg.toml`，或讀取以 `--config` / `WPG_CONFIG` 指定的檔案。優先順序由低到高為：內建預設值、設定檔、環境變數 `WPG_<SECTION>__<KEY>` (`__` 分隔巢狀層級，例如 `WPG_SERVER__BIND=0.0.0.0:8080` 或 `WPG_TX__ALLOWED_IFACES='["eth1"]'`)，最後是 `wpg serve --bind` 等命令列選項。未知的鍵視為錯誤。`wpg config init` 會寫入下方附註解的範例，`wpg config show` 會輸出實際生效的設定，`wpg config validate` 會檢查設定，`wpg config set-password` 則會儲存管理員密碼雜湊。管理員密碼本身不是設定鍵：`wpg serve --password`、`--password-file` 與 `WPG_PASSWORD` 優先於 `auth.password_hash` 與 `auth.password_file`。本頁由設定型別產生。
docgen-config-keys = 鍵
docgen-config-top-level = 最上層
docgen-config-default-machine = 作業系統的資料目錄 (Linux：`~/.local/share/wpg`)
docgen-config-example = 範例 (`wpg config init`)
docgen-config-example-intro = 每一行有效設定都等於內建預設值，因此在編輯前此檔案不會改變任何行為；註解行則列出選用的鍵。
docgen-config-data-dir = 伺服器狀態的目錄：使用者範本、我的最愛、SQLite 資料庫 (使用者、工作階段、工作歷程記錄)、試執行的擷取檔與自我簽署的 TLS 憑證。
docgen-config-server = HTTP 監聽器。
docgen-config-server-bind = 監聽位址。未設定密碼 (且未在初始設定畫面建立使用者) 時，只接受迴路位址 (`--allow-insecure` 可覆寫)。
docgen-config-server-tls = 用於提供 HTTPS 的 TLS 憑證鏈與私密金鑰 (PEM)；優先於 `self_signed_tls`。
docgen-config-server-tls-cert = PEM 憑證鏈。
docgen-config-server-tls-key = PEM 私密金鑰。
docgen-config-server-self-signed-tls = 以產生至 `<data_dir>/tls/` 的自我簽署憑證提供 HTTPS (之後啟動時重複使用)；`wpg serve --tls` 會設定此項。
docgen-config-server-base-url = 位於反向代理伺服器後方時伺服器的公開 URL (`https://wpg.example.net`)，用於絕對連結。
docgen-config-auth = 驗證。
docgen-config-auth-password-hash = 管理員密碼的 argon2id PHC 字串；由 `wpg config set-password` 寫入。
docgen-config-auth-password-file = 存放管理員密碼的檔案：argon2 PHC 字串，或在啟動時進行雜湊的純文字。
docgen-config-auth-session-ttl-secs = 工作階段的有效時間 (秒) (滑動式：每個請求都會延長)。
docgen-config-templates = 範本庫。
docgen-config-templates-library-paths = 唯讀的範本庫 (例如 Git 簽出目錄)，會遞迴掃描 `*.wpg.yaml` / `*.wpg.json`，並在檔案變更時重新載入。
docgen-config-templates-user-dir = 存放在 GUI 中建立或匯入之範本的可寫入目錄；預設為 `<data_dir>/templates`。
docgen-config-protocols = 使用者自訂協定。
docgen-config-protocols-dirs = 存放使用者自訂協定檔案 (`*.yaml`: 新的協定定義以及掛在既有協定下的繫結) 的目錄，啟動時依檔名順序載入。可用 `wpg serve --protocols-dir` 追加。
docgen-config-tx = 傳送政策與預設值。
docgen-config-tx-allowed-ifaces = 工作可用來傳送的介面；空白 = 全部。`null` 後端與試執行不受限制。
docgen-config-tx-max-rate = 工作可要求的最高速率 (`100kpps`、`1Gbps`)；未指定速率的工作會以此速率傳送。
docgen-config-tx-default-backend = 工作未指定後端時使用的後端 (`null`、`pcapfile`、`afpacket`、`pcap`、`afxdp`、`dpdk`)；未設定時 Linux 上為 `afpacket`，否則為 `pcap`，再否則為 `null`。
docgen-config-tx-dpdk-eal-args = `dpdk` 後端的 DPDK EAL 參數，例如 `["-a", "0000:00:04.0"]`。若未指定，wpg 會加上 `-l <first CPU>` 與 `--in-memory`。EAL 在每個處理程序中於第一個 `dpdk` 工作時啟動一次；變更參數需要重新啟動。
docgen-config-log = 記錄。
docgen-config-log-level = 篩選指示詞 (`RUST_LOG` 會覆寫)，例如 `info` 或 `wpg_tx=debug,info`。
docgen-config-log-format = 每行格式：`pretty` (人類可讀) 或 `json` (每行一個物件)。
docgen-config-log-file = 同時寫入此檔案 (會建立其目錄)。
docgen-config-log-rotation = 檔案輪替方式：`never`、`hourly` 或 `daily`。
docgen-config-log-max-files = 最多保留此數量的輪替檔案 (刪除最舊的檔案)。
docgen-config-log-ansi = 主控台輸出為終端機時以色彩顯示。
docgen-schema-heading = 鍵參考 (自動產生)
docgen-schema-intro = 由範本格式的 JSON Schema (`wpg template schema`) 產生；說明文字為程式碼的文件註解 (英文)。`Template` 為文件根節點。
# --- P2 PTP / ESMC / CFM / R-APS / 802.3ah OAM 欄位說明 ---
ptp-major-sdo-id = majorSdoId (IEEE 1588-2008 中為 transportSpecific)：預設設定檔為 0，IEEE 802.1AS (gPTP) 為 1。
ptp-message-type = 訊息類型：0 Sync，1 Delay_Req，2 Pdelay_Req，3 Pdelay_Resp，8 Follow_Up，9 Delay_Resp，10 Pdelay_Resp_Follow_Up，11 Announce，12 Signaling，13 Management。
ptp-minor-version = minorVersionPTP (IEEE 1588-2008 中為 0，IEEE 1588-2019 中為 1)。
ptp-version = versionPTP (2)。
ptp-message-length = 包含 TLV 的整個訊息長度，以八位元組為單位 (Auto)。
ptp-domain-number = 網域編號。
ptp-minor-sdo-id = minorSdoId (IEEE 1588-2008 中為保留值 0)。
ptp-flags = 旗標欄位：第一個八位元組為 alternateMaster、twoStep、unicast、設定檔專用位元與安全性位元；第二個八位元組為 leap61、leap59、currentUtcOffsetValid、ptpTimescale、timeTraceable、frequencyTraceable 與 synchronizationUncertain。
ptp-correction-field = 修正值，以奈秒乘以 2^16 表示 (有號數)。
ptp-message-type-specific = 訊息類型專用欄位 (IEEE 1588-2008 中為保留值 0)。
ptp-clock-identity = 來源連接埠的時脈識別 (EUI-64，例如中間插入 ff:fe 的 MAC 位址)。
ptp-port-number = 來源連接埠的連接埠編號。
ptp-sequence-id = 序號 ID；Follow_Up / Delay_Resp / Pdelay_Resp 會複製其所回應訊息的序號 ID。
ptp-control-field = 控制欄位 (與 IEEE 1588 第 1 版相容)：0 Sync，1 Delay_Req，2 Follow_Up，3 Delay_Resp，4 Management，5 其他訊息。
ptp-log-message-interval = 訊息間隔 (秒) 的 Log2 值 (有號數；不適用時為 0x7f)。
ptp-timestamp-seconds = 時間戳記：秒 (48 位元)。
ptp-timestamp-nanoseconds = 時間戳記：奈秒 (32 位元)。
ptp-port-identity-clock = 連接埠識別：時脈識別 (全為 1 = 所有時脈)。
ptp-port-identity-port = 連接埠識別：連接埠編號 (0xffff = 所有連接埠)。
ptp-current-utc-offset = 目前的 UTC 偏移 (秒) (TAI - UTC)。
ptp-grandmaster-priority = Grandmaster 優先順序 (最佳主時脈演算法，值較低者勝出)。
ptp-grandmaster-clock-class = Grandmaster 時脈等級 (6 = 鎖定至主要參考，248 = 預設，255 = 僅限 slave)。
ptp-grandmaster-clock-accuracy = Grandmaster 時脈精確度 (0x20 = 25 ns 以內 ... 0xfe = 未知)。
ptp-grandmaster-variance = Grandmaster offsetScaledLogVariance。
ptp-grandmaster-identity = Grandmaster 時脈識別。
ptp-steps-removed = Grandmaster 與傳送端之間的邊界時脈數量。
ptp-time-source = Grandmaster 的時間來源：原子鐘、GPS、地面無線電、PTP、NTP、手動設定、其他、內部振盪器。
ptp-starting-boundary-hops = Management 訊息的起始邊界躍點數。
ptp-boundary-hops = 剩餘的邊界躍點數。
ptp-action = Management 動作：GET、SET、RESPONSE、COMMAND、ACKNOWLEDGE。
ptp-tlvs = TLV (16 位元類型、值的 16 位元長度，Auto)：Management、Management Error Status、Organization Extension、單點傳播協商 (Request / Grant / Cancel / Acknowledge Cancel)、Path Trace、Alternate Time Offset Indicator。在訊息長度處結束。
ptp-data = 共通標頭之後的其餘訊息，以八位元組表示 (到訊息長度為止)。
esmc-subtype = Slow Protocols 子類型 (0x0a = 組織專用)。
esmc-itu-oui = ITU-T OUI (00-19-a7)。
esmc-itu-subtype = ITU-T 子類型 (1 = ESMC)。
esmc-version = ESMC 版本 (1)。
esmc-event-flag = 事件旗標：QL 變更後立即傳送的事件 PDU 為 1，每秒一次的資訊 PDU 為 0。
esmc-tlvs = TLV (8 位元類型、包含 3 個八位元組標頭的 16 位元長度，Auto)：含 SSM 代碼的 QL TLV、含增強型 SSM 代碼的 Extended QL TLV、SyncE 時脈識別與串接時脈數量。其後的八位元組 (填補) 保留為一個原始項目。
cfm-md-level = 維護網域 / MEG 層級 (0-7)。
cfm-version = CFM 版本 (0；G.8032v2 的 R-APS 為 1)。
cfm-opcode = OpCode：1 CCM，2 LBR，3 LBM，4 LTR，5 LTM，33 AIS，35 LCK，37 TST，39 Linear APS，40 R-APS，41 MCC，42 LMR，43 LMM，45 1DM，46 DMR，47 DMM，52 CSF，53 1SL，54 SLR，55 SLM ...
cfm-flags = 旗標 (依 OpCode 而定)。
cfm-first-tlv-offset = 第一個 TLV 從此欄位結尾起算的偏移 (每個 OpCode 固定)。
cfm-rdi = RDI：遠端缺陷指示。
cfm-period = 傳送週期：1 = 3.33 ms，2 = 10 ms，3 = 100 ms，4 = 1 s，5 = 10 s，6 = 1 min，7 = 10 min。
cfm-sequence-number = 序號。
cfm-mep-id = MEP ID (1-8191)。
cfm-maid = MAID / MEG ID，48 個八位元組：MD 名稱格式與名稱、短 MA 名稱格式、長度與名稱，以零填補 (Y.1731：無 MD 名稱，格式 32 ICC-based，長度 13)。
cfm-frame-counter = 訊框計數器 (遺失量測)。
cfm-transaction-id = Loopback / linktrace 交易識別碼。
cfm-use-fdb-only = UseFDBonly：僅由轉送資料庫決定輸出連接埠。
cfm-fwd-yes = FwdYes：LTM 已被轉送。
cfm-terminal-mep = TerminalMEP：回覆來自目標 MEP。
cfm-ttl = Linktrace TTL。
cfm-relay-action = 轉送動作：1 RlyHit，2 RlyFDB，3 RlyMPDB。
cfm-original-mac = 發起 linktrace 之 MEP 的 MAC 位址。
cfm-target-mac = Linktrace 的目標 MAC 位址。
cfm-csf-type = 用戶端訊號失效類型：0 LOS，1 FDI / AIS，2 RDI，3 DCI。
cfm-timestamp = IEEE 1588 格式的時間戳記 (32 位元秒與奈秒)。
cfm-source-mep-id = 來源 MEP ID。
cfm-responder-mep-id = 回應端 MEP ID (SLM 中為 0)。
cfm-test-id = 合成遺失量測的測試 ID。
cfm-tlvs = TLV (8 位元類型、值的 16 位元長度，Auto)：Sender ID、Port Status、Data、Interface Status、Reply Ingress / Egress、LTM / LTR Egress Identifier、Organization-Specific、Test，以 End TLV (類型 0) 結束。
cfm-data = 第一個 TLV 偏移之後的其餘 PDU，以八位元組表示 (依 OpCode 而定的欄位與 TLV)。
raps-request-state = 請求 / 狀態：0000 NR，0111 MS，1011 SF，1101 FS，1110 Event。
raps-sub-code = 子代碼 (0000 = 搭配 Event 請求的清除要求)。
raps-rb = RB：RPL 已封鎖。
raps-dnf = DNF：不要清除轉送資料庫。
raps-bpr = BPR：被封鎖連接埠參考 (0 = 環狀連結 0，1 = 環狀連結 1)。
raps-node-id = 節點 ID：傳送端環狀節點的 MAC 位址。
efm-oam-subtype = Slow Protocols 子類型 (3 = OAM)。
efm-oam-flags = 旗標：link fault、dying gasp、critical event、本地 / 遠端 evaluating 與 stable。
efm-oam-code = OAMPDU 代碼：0x00 Information，0x01 Event Notification，0x02 Variable Request，0x03 Variable Response，0x04 Loopback Control，0xfe Organization Specific。
efm-oam-information-tlvs = Information TLV (8 位元類型、包含標頭的 8 位元長度，Auto)：Local / Remote Information、Organization Specific，以 End TLV (類型 0) 結束。
efm-oam-sequence-number = 事件通知序號。
efm-oam-event-tlvs = 連結事件 TLV (8 位元類型、包含標頭的 8 位元長度，Auto)：errored symbol period、errored frame、errored frame period、errored frame seconds summary、Organization Specific，以 End TLV (類型 0) 結束。
efm-oam-variable-descriptors = 以八位元組表示的變數描述元：各為 branch (1 個八位元組) 與 leaf (2 個八位元組)，以零值 branch 結束。
efm-oam-variable-containers = 以八位元組表示的變數容器：各為 branch、leaf、width 與 value，以零值 branch 結束。
efm-oam-loopback-command = 遠端迴路命令：1 啟用，2 停用。
efm-oam-oui = 組織唯一識別碼 (OUI)。
efm-oam-data = OAMPDU 的其餘部分，以八位元組表示。

# --- P2 MACsec / GTPv2-C / Diameter / SIP / RTP 欄位說明 ---
macsec-v = TCI V 位元：版本 (0)。
macsec-es = TCI ES 位元：End Station (SCI 為來源 MAC 位址加上連接埠 1)。
macsec-sc = TCI SC 位元：存在明確的 SCI (Auto：指定 SCI 時設定)。
macsec-scb = TCI SCB 位元：EPON 單一複本廣播。
macsec-e = TCI E 位元：安全資料已加密。
macsec-c = TCI C 位元：使用者資料已變更 (預設加密套件中總是與 E 一起設定)。
macsec-an = 關聯編號 (0-3)：通道內的安全關聯。
macsec-sl = Short length：安全資料長度小於 48 個八位元組時為該長度，否則為 0 (Manual)。
macsec-pn = 封包編號 (32 位元，使用 XPN 時為低半部)。
macsec-sci = 安全通道識別碼 (SCI)：系統識別碼 (MAC 位址) 與連接埠識別碼；設定 SC 時存在。
macsec-secure-data = 安全資料：已加密 (或僅完整性保護) 的使用者資料，以不透明的八位元組表示；不會進行任何加密。
macsec-icv = 完整性檢查值 (ICV，16 個八位元組)，不透明：不會計算。
gtpv2-version = GTP 版本 (2)。
gtpv2-p = P 旗標：後接附帶的訊息 (下一個 GTPv2-C 層)。
gtpv2-t = T 旗標：存在 TEID 欄位 (Auto：指定 TEID 時設定)。
gtpv2-mp = MP 旗標：存在訊息優先順序。
gtpv2-message-type = 訊息類型 (Echo、Create / Modify / Delete Session、Create / Update / Delete Bearer、...)。
gtpv2-length = 不含前 4 個八位元組的訊息長度，以八位元組為單位 (Auto)。
gtpv2-teid = 接收端的通道端點識別碼 (Echo 訊息中不存在；初始請求中為 0)。
gtpv2-sequence = 用於配對回應與請求的序號 (24 位元)。
gtpv2-message-priority = 訊息優先順序 (0 最高)，設定 MP 時使用。
gtpv2-ies = 資訊元素 (8 位元類型、資料的 16 位元長度、spare / instance 八位元組)：IMSI、Cause、Recovery、APN、AMBR、EBI、IP Address、MEI、MSISDN、PAA、Bearer QoS、RAT Type、Serving Network、F-TEID、Bearer Context (分組)、PDN Type、... 長度為 Auto。
diameter-version = Diameter 版本 (1)。
diameter-length = 包含標頭與 AVP 填補的訊息長度，以八位元組為單位 (Auto)。
diameter-r = R 旗標：請求 (0 = 應答)。
diameter-p = P 旗標：可代理。
diameter-e = E 旗標：應答中帶有協定錯誤。
diameter-t = T 旗標：可能為重新傳送的訊息。
diameter-command-code = 命令代碼 (Capabilities-Exchange、Device-Watchdog、Credit-Control、...)。
diameter-application-id = Application-ID (0 = Diameter 共通訊息)。
diameter-hop-by-hop = 在單一連線上配對應答與請求的 Hop-by-Hop 識別碼。
diameter-end-to-end = 用於偵測重複請求的 End-to-End 識別碼。
diameter-avps = AVP (32 位元代碼、V / M / P 旗標、不含填補的 24 位元長度、選用的 Vendor-ID、填補至 4 個八位元組的資料)：基本協定與 Credit-Control AVP，分組 AVP 支援一層。長度、填補與 V 旗標為 Auto。
sip-start-line = 起始行：請求行 (方法、Request-URI、SIP/2.0) 或狀態行 (SIP/2.0、代碼、原因)。CRLF 會自動加上。
sip-headers = 標頭行 (Name: value)，每個元素一行；CRLF 與其後的空白行會自動加上。Content-Length 為 Manual。
sip-body = 訊息本文 (SDP offer / answer、...)。
rtp-version = RTP 版本 (2)。
rtp-padding = P 位元：酬載結尾有填補八位元組 (最後一個為填補數量，Manual)。
rtp-x = X 位元：CSRC 清單之後接著標頭延伸 (Auto)。
rtp-cc = CSRC 數量 (Auto)。
rtp-marker = Marker 位元 (由設定檔定義，例如 talkspurt 的開始)。
rtp-payload-type = 酬載類型 (0 PCMU，8 PCMA，9 G.722，18 G.729，96-127 動態)。
rtp-sequence = 序號，每個 RTP 封包加一。
rtp-timestamp = 時間戳記：第一個酬載八位元組的取樣時刻。
rtp-ssrc = 同步來源識別碼。
rtp-csrc = 貢獻來源識別碼 (最多 15 個)。
rtp-extension = 標頭延伸：由設定檔定義的 16 位元 (0xBEDE：RFC 8285 單位元組元素)、以 32 位元字組為單位的長度 (Auto) 與延伸字組。
rtcp-version = RTCP 版本 (2)。
rtcp-padding = P 位元：封包結尾有填補八位元組 (Manual)。
rtcp-count = 未定義型別之封包類型的計數 / 格式欄位 (5 位元)。
rtcp-report-count = 接收報告數量 (Auto)。
rtcp-source-count = 來源數量：SDES 區塊或 BYE 識別碼 (Auto)。
rtcp-subtype = 應用程式定義的子類型。
rtcp-packet-type = 封包類型 (200 SR，201 RR，202 SDES，203 BYE，204 APP，205 RTPFB，206 PSFB，207 XR)。
rtcp-length = 以 32 位元字組為單位的封包長度減一 (Auto)。
rtcp-body = 共通標頭之後的封包本文，以八位元組表示。
rtcp-ssrc = 此封包傳送端的 SSRC。
rtcp-ntp-timestamp = NTP 時間戳記 (最高有效字組；下一個欄位為小數部分)。
rtcp-rtp-timestamp = 對應 NTP 時間戳記的 RTP 時間戳記。
rtcp-packet-count = 傳送端的封包數。
rtcp-octet-count = 傳送端的酬載八位元組數。
rtcp-report-blocks = 接收報告區塊：SSRC、遺失比例、累計遺失數、延伸最高序號、抖動、LSR、DLSR。
rtcp-extension = 到封包長度為止的設定檔專用延伸八位元組。
rtcp-chunks = SDES 區塊：SSRC / CSRC 及其項目 (類型、長度 (Auto)、文字)，以 null 八位元組結束並填補至 32 位元。
rtcp-ssrcs = 離開工作階段的 SSRC / CSRC 識別碼。
rtcp-reason = 選用的離開原因：長度 (Auto)、文字、填補至 32 位元。
rtcp-app-name = 名稱 (4 個 ASCII 字元)。
rtcp-app-data = 依應用程式而定的資料 (4 個八位元組的倍數)。
