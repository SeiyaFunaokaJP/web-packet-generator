# 简体中文 (zh-CN) 语言文件。此处的每条消息都必须存在于每个语言文件中
# (由 wpg-i18n 单元测试强制检查)。

app-name = Web Packet Generator
app-tagline = 面向网络测试的高性能数据包生成器，可直接在浏览器中操作。

## 服务器 / API
health-ok = 服务运行正常
server-listening = 正在监听 { $addr }
server-placeholder-ui = Web UI 尚未构建。请在 crates/wpg-web 中运行 `trunk build`。

## 错误
error-not-implemented = `{ $command }` 尚未实现。
error-not-found = 未找到
error-internal = 服务器内部错误

## UI
ui-language = 语言
ui-protocols = 协议

## 诊断 (wpg-core `diag::keys`；参数名属于接口约定的一部分)
diag-field-out-of-range = 字段 `{ $field }` = { $value } 超出范围 { $min }..{ $max }。
diag-field-not-equal = 字段 `{ $field }` = { $value } 与期望值 { $expected } 不同。
diag-field-forbidden-value = 字段 `{ $field }` 的值不推荐使用：{ $value }。
diag-length-too-short = `{ $span }` 长度为 { $len } 字节，至少需要 { $min } 字节。
diag-length-too-long = `{ $span }` 长度为 { $len } 字节，最多允许 { $max } 字节。
diag-length-mismatch = 长度字段 `{ $field }` = { $value } 与计算出的长度 { $expected } 不同。
diag-count-mismatch = 计数字段 `{ $field }` = { $value } 与实际元素个数 { $expected } 不同。
diag-fill-inexact = 无法用填充项精确达到 `{ $field }`：还缺少 { $len } 个八位组。
diag-checksum-mismatch = 校验和字段 `{ $field }` = { $value } 与计算出的校验和 { $expected } 不同。
diag-next-protocol-mismatch = 字段 `{ $field }` = { $value } 未指明下一层 `{ $next }` (期望值为 { $expected })。
diag-auto-overflow = `{ $field }` 的自动值 { $value } 超出 { $bits } 位所能表示的范围。
diag-length-not-aligned = `{ $field }` 的长度 { $len } 不是 { $unit } 字节的整数倍。
diag-no-pseudo-header = 校验和 `{ $field }` 需要伪首部，但其下方没有 IP 层。
diag-not-implemented = `{ $field }`：{ $what } 尚未实现。
diag-truncated = 该层被有意截断为 { $len } 字节。
diag-short-header = 输入在 { $protocol } 首部内的偏移 { $offset } 处结束。
diag-length-exceeds-data = 长度字段 `{ $field }` = { $len } 超过了可用的 { $available } 字节。
diag-trailing-bytes = 数据包结束后还剩余 { $len } 字节。
diag-unsupported-field = 解析器暂时无法解码 { $protocol } 的字段 `{ $field }`。

## 协议字段说明 (wpg-protocols `desc = "..."`)
arp-hlen = 硬件地址长度，单位为字节 (Ethernet 为 6)。
arp-htype = 硬件类型 (1 = Ethernet)。
arp-oper = 操作码 (1 = 请求，2 = 应答)。
arp-plen = 协议地址长度，单位为字节 (IPv4 为 4)。
arp-ptype = 协议类型 (EtherType，0x0800 = IPv4)。
arp-sha = 发送方硬件地址。
arp-spa = 发送方协议地址。
arp-tha = 目标硬件地址。
arp-tpa = 目标协议地址。
icmp-code = 消息代码，含义取决于类型。
icmp-identifier = 用于匹配回显请求与回显应答的标识符。
icmp-rest = 与类型相关的 4 字节 "首部其余部分" 字段。
icmp-sequence = 用于匹配回显请求与回显应答的序列号。
icmp-type = 消息类型。
ipv6-fragment-offset = 本分片的偏移量，以 8 字节为单位。
ipv6-more-fragments = 更多分片标志 (1 = 后面还有分片)。
ipv6-options = 原始 TLV 选项，填充至 8 字节的整数倍。
ipv6-routing-data = 与类型相关的路由数据。
ipv6-routing-type = 路由首部类型 (例如 2 = Mobile IPv6，4 = SRH)。
ipv6-segments-left = 剩余的路由段数。
srh-segments-left = Segments Left：下一个要访问的段在段列表中的索引；到达最后一个段时为 0。
srh-last-entry = Last Entry：段列表最后一个元素的索引，即段数 - 1 (Auto)。
srh-flags = 标志 (RFC 8754 未定义任何标志；0)。
srh-tag = Tag：将数据包标记为某个类别或组的成员 (0 = 未使用)。
srh-segment-list = 段列表 (SID)，按路径的逆序排列：元素 0 为最后一个段，最后一个元素为第一个段。当 Segments Left 不为零时，上层校验和以元素 0 作为目的地址。
srh-tlvs = 可选 TLV (8 位类型，8 位长度)：Pad1、PadN、HMAC (HMAC 值为 Manual)。随后用 Pad1 八位组将首部填充至 8 的整数倍 (Auto)。
ndp-destination = 重定向的目的地址。
ndp-managed = 管理地址配置标志 (M)。
ndp-options = 原始 NDP 选项 (TLV)。
ndp-other = 其他配置标志 (O)。
ndp-override-flag = 覆盖标志 (O)。
ndp-router-flag = 路由器标志 (R)。
ndp-solicited-flag = 请求标志 (S)。
ndp-target = 目标地址。

## wpg ifaces
ifaces-col-name = 名称
ifaces-col-index = 索引
ifaces-col-mac = MAC
ifaces-col-ipv4 = IPV4
ifaces-col-ipv6 = IPV6
ifaces-col-mtu = MTU
ifaces-col-state = 状态
ifaces-col-speed = 速率
ifaces-col-driver = 驱动
ifaces-state-up = up
ifaces-state-down = down
ifaces-state-no-carrier = 无载波
ifaces-state-loopback = 环回
ifaces-none = 未找到网络接口。
ifaces-error = 无法列出网络接口：{ $error }

## wpg doctor
doctor-title = wpg doctor：环境检查
doctor-summary = { $ok } 项正常，{ $warn } 项警告，{ $fail } 项失败
doctor-skip = 不适用于此操作系统
doctor-caps-ok = 可以使用原始套接字
doctor-caps-missing = 缺少发送原始数据包所需的权限 (CAP_NET_RAW / root / Administrator)
doctor-caps-npcap-user = 未以管理员身份运行，但 Npcap 允许非管理员捕获和发送
doctor-caps-npcap-admin = Npcap 仅限管理员使用；要发送，请在以管理员身份打开的终端中运行 wpg
doctor-fix-caps = 授予权限：{ $hint }
doctor-iface-ok = 至少有一个可用的网络接口处于 up 状态
doctor-iface-none = 没有处于 up 状态且有载波的非环回接口
doctor-iface-error = 无法枚举网络接口
doctor-fix-iface = 检查接口：{ $hint }
doctor-af-xdp-ok = 内核支持 AF_XDP (5.10 或更高版本)
doctor-af-xdp-old = 内核版本过旧，不支持 AF_XDP (需要 5.10 或更高版本)；afpacket 后端仍可使用
doctor-tpacket-v3-ok = 内核支持 TPACKET_V3
doctor-tpacket-v3-old = 内核不支持 TPACKET_V3；afpacket 将回退到 sendmmsg
doctor-hugepages-ok = 已配置大页内存 (hugepages)
doctor-hugepages-none = 未配置大页内存 (仅 DPDK 后端需要)
doctor-fix-hugepages = 预留大页内存并将网卡绑定到 DPDK：{ $hint }
doctor-npcap-ok = 已安装 Npcap
doctor-npcap-missing = 未安装 Npcap；在 Windows 上发送数据包需要 Npcap
doctor-npcap-no-dll = Npcap 似乎已安装，但在 System32\Npcap 和 System32 中均未找到 wpcap.dll
doctor-fix-npcap = 安装 (或重新安装) Npcap，然后再次运行 wpg doctor：{ $hint }
doctor-pcaplib-ok = 可以使用 libpcap
doctor-pcaplib-missing = 未找到 libpcap；无法使用 pcap 后端
doctor-fix-pcaplib = 安装 libpcap：{ $hint }

## wpg template lint (wpg-template `LintIssue` 键)
lint-level-error = 错误
lint-level-warning = 警告
lint-schema-version = 不支持的模式版本 { $value } (支持：{ $expected })。
lint-missing-name = `meta.name` 为空。
lint-no-layers = 模板中没有任何层。
lint-unknown-protocol = 未知协议 `{ $protocol }`。
lint-unknown-field = 协议 `{ $protocol }` 中没有字段 `{ $field }`。
lint-invalid-value = `{ $field }` 的值无效：{ $value }
lint-invalid-generator = `{ $field }` 的生成器无效：{ $value }
lint-unknown-variable = 未知变量 `{ $value }`。
lint-missing-variable = 变量 `{ $value }` 没有默认值，也未指定值。
lint-unused-variable = 变量 `{ $value }` 从未被使用。
lint-duplicate-variable = 变量 `{ $value }` 被声明了多次。
lint-var-syntax = `{ $value }` 中的变量占位符未闭合。
lint-override-unknown = 未知的变量覆盖 `{ $value }`。
lint-field-and-generator = `{ $field }` 同时设置了固定值和生成器；以生成器为准。
lint-next-protocol-manual = `{ $next }` 不在 `{ $protocol }` 的绑定表中；`{ $field }` 将为 Manual。
lint-unknown-order = 流顺序中的 `{ $value }` 没有对应的生成器。
lint-invalid-stream = 流设置无效：{ $value }
lint-build-error = 无法构建数据包：{ $value }

## wpg template / wpg render
template-lint-ok = { $value }：没有问题
template-lint-load-error = { $value }：{ $error }
template-lint-summary = { $ok } 个文件中有 { $fail } 个错误，{ $warn } 个警告
template-list-empty = 在 { $value } 中未找到模板。
template-show-hex = 十六进制转储 ({ $len } 字节)
template-show-layout = 布局
render-done = 已将 { $len } 个数据包写入 { $value }。
render-unbounded = 流没有上限；请指定 --count N。
cli-bad-var = 无效的 --var `{ $value }`；应为 NAME=VALUE。

## wpg send / wpg bench (进度、摘要、错误)
tx-progress = [{ $t } s] { $packets } 包  { $pps }pps  { $bps }bps  丢弃 { $dropped }
tx-progress-rate = 速率 { $ratio }%
tx-progress-paused = (已暂停)
tx-progress-scheduled = (已计划，{ $secs } s 后开始)
tx-interrupted = 已中断，正在停止...
tx-ctrlc-warning = 警告：无法安装 Ctrl-C 处理程序：{ $error }
tx-open-failed = 无法在 `{ $iface }` 上打开 { $backend }：{ $error }
tx-start-failed = 无法启动任务：{ $error }
tx-available-backends = 可用的后端：{ $value }
tx-iface-required = { $backend } 后端需要指定 --iface。
tx-iface-required-pcapfile = pcapfile 后端需要指定 --iface <输出路径>。
tx-bad-workers = --workers 必须至少为 1。
tx-workers-unsupported = { $backend } 后端只支持单个工作线程；请使用 --workers 1。
tx-pin-cores-short = 警告：--pin-cores 为 { $workers } 个工作线程只列出了 { $count } 个核心；其余线程不绑定核心。
tx-json-error = 无法写入 JSON：{ $error }
send-bad-rate = 无效的 --rate：{ $error }
send-bad-duration = 无效的 --duration：{ $error }
send-bad-ramp-up = 无效的 --ramp-up：{ $error }
send-bad-start-at = 无效的 --start-at：{ $error }
send-link-speed-unknown = 无法确定 `{ $iface }` 的链路速率，因此无法使用 `{ $value }%`；请指定绝对速率，例如 `1Gbps`。
send-bad-plan = --plan 必须为 auto、prerender 或 onthefly。
send-stream-error = 流错误：{ $error }
send-render-error = 渲染错误：{ $error }
send-starting = 通过 { $backend }{ $detail } 在 { $iface } 上发送：速率 { $rate }，突发 { $burst }，{ $limit }，{ $workers } 个工作线程，计划 { $strategy } ({ $reason })
send-limit-count = { $count } 个数据包
send-limit-count-or-duration = { $count } 个数据包或 { $duration }
send-limit-duration = 持续 { $duration }
send-limit-forever = 直到按下 Ctrl-C
send-ramp-up = 渐增：在 { $duration } 内从 0 增加到 { $rate }
send-scheduled = 计划于 { $time } 开始 ({ $secs } s 后)；按 Ctrl-C 取消。
summary-state = 状态       { $value }
summary-state-finished = 已完成 ({ $reason })
summary-state-failed = 失败：{ $error }
summary-reason-count-reached = 已达到数量
summary-reason-duration-elapsed = 已达到时长
summary-reason-source-exhausted = 流已耗尽
summary-reason-stopped = 已停止
summary-backend = 后端       { $backend }{ $detail }
summary-workers = 工作线程   { $workers }
summary-packets = 数据包     { $value }
summary-bytes = 字节       { $value }
summary-dropped = 丢弃       { $value }
summary-elapsed = 耗时       { $secs } s
summary-average = 平均       { $pps }pps  { $bps }bps (L2)  { $l1 }bps (L1)
summary-accuracy = 精度       目标 { $target }{ $unit }  达成 { $ratio }%  最大抖动 { $max }  平均抖动 { $mean }
summary-stream = 流 { $index }   { $packets } 包  { $bytes } 字节
summary-worker = 工作线程 { $index }   { $packets } 包  { $bytes } 字节{ $core }
summary-worker-core = ，核心 { $core }
summary-warning = 警告       { $error }
summary-unsigned = 无签名     { $value } (没有空间放置签名)

## wpg capture (接收 / 验证)
rx-progress = [{ $t } s] { $packets } 包  { $pps }pps  { $bps }bps  匹配 { $matched } ({ $mpps }pps)  丢弃 { $dropped }
capture-starting = 通过 { $backend } 在 { $iface } 上捕获，过滤器 { $filter } (按 Ctrl-C 停止)
capture-filter-all = (所有帧)
capture-iface-required-pcapfile = pcapfile 后端需要指定 --iface <捕获文件>。
capture-bad-filter = 无效的 --filter：{ $error }
capture-running = 运行中
capture-reason-end-of-input = 输入结束
capture-packet = #{ $index }  { $time }  { $len } B  { $layers }{ $signature }
capture-packet-signature = { "  " }[wpg 流 { $stream } 序号 { $seq }]
capture-summary-matched = 匹配       { $packets } 包  { $bytes } 字节
capture-summary-average = 平均       { $pps }pps  { $bps }bps  匹配 { $mpps }pps  { $mbps }bps
capture-summary-saved = 已保存     { $count } 包，保存到 { $path }
capture-summary-signed = 签名       { $signed } 个带签名，{ $unsigned } 个无签名
capture-summary-stream = 流 { $stream } 接收 { $received }  序号 { $first }-{ $last }  丢失 { $lost } ({ $ratio }%)  乱序 { $reordered }  重复 { $duplicates }
capture-summary-latency = { "             " }时延 最小 { $min }  平均 { $avg }  最大 { $max }  抖动 { $jitter }

bench-iface = 接口       { $iface }
bench-worker = 工作线程 { $index }   { $detail }
bench-frame = 帧         { $len } B (+4 FCS)  批量 { $batch }
bench-template = 模板       { $value } (实时生成，{ $mode })
bench-mode-patch = 原地修补
bench-mode-rebuild = 每帧完全重建
bench-rate = 速率       { $pps }pps
bench-bits = { $bps }bps (L2)  { $l1 }bps (L1)
bench-bad-duration = --duration 必须为非负的秒数。
bench-failed = 基准测试失败：{ $error }
import-done = 已将 { $len } 个数据包导入到 { $count } 个模板：{ $value }
import-read-error = { $value }：{ $error }
import-write-error = { $value }：{ $error }
import-note = { $value }
import-note-packet = 数据包 { $index }：{ $value }
import-packet-problems = 数据包 { $index } 有 { $count } 个解析问题
import-packet-skipped = 数据包 { $index } 无法解码，已跳过
import-not-reproducible = 导入的模板无法重建所捕获的八位组；该帧已作为原始数据保存。
import-mixed-stacks = 有 { $count } 个数据包的层结构与第一个数据包不同；未推断生成器。
import-generator-approximate = { $value } 只能近似表示所捕获的值。
import-generators-dropped = 推断出的生成器产生了无效的流，已被舍弃。
template-export-none = 未指定要导出的模板。
template-export-not-found = 未找到模板：{ $value }
template-export-done = 已将 { $count } 个模板导出到 { $value }。
template-export-error = { $value }：{ $error }
template-import-done = 已从 { $value } 提取 { $count } 个模板到 { $detail }。
template-import-exists = { $value }：已存在 (使用 --force 覆盖)
template-import-error = { $value }：{ $error }
template-migrate-outdated = { $value }：模式 { $found }，需要迁移到 { $expected }
template-migrate-done = { $value }：已从模式 { $found } 迁移到 { $expected }
template-migrate-error = { $value }：{ $error }
template-migrate-summary = 已迁移 { $count } 个，{ $ok } 个已是模式 { $expected }，{ $fail } 个错误
template-migrate-check-summary = 待迁移 { $count } 个，{ $ok } 个已是模式 { $expected }，{ $fail } 个错误

## API 错误 (wpg-server)
api-error-not-found = 未找到 { $what }。
api-error-invalid-body = 请求正文无效：{ $error }
api-error-invalid-query = 请求参数无效：{ $error }
api-error-invalid-template = 模板无效：{ $error }
api-error-invalid-hex = 十六进制输入无效：{ $error }
api-error-parse-failed = 无法解码这些字节：{ $error }
api-error-render-failed = 渲染失败：{ $error }
api-error-count-range = `count` 必须介于 1 和 { $max } 之间 (实际为 { $error })。
api-error-read-only = 模板 { $id } 属于只读库；请复制后再编辑。
api-error-policy-iface = 服务器策略不允许在 { $iface } 上发送 (允许：{ $allowed })。
api-error-policy-rate = 速率 { $rate } 超过了服务器策略允许的最大值 ({ $max })。
api-error-policy-unlimited = 服务器策略不允许不限速率 (最大值 { $max })。
api-error-invalid-state = 任务 { $id } 处于 { $state } 状态；无法对其执行 { $action }。
api-error-job-spec = 任务无效：{ $error }
api-error-backend = 发送后端错误：{ $error }
api-error-capture-spec = 捕获无效：{ $error }
api-error-capture-filter = 捕获过滤器无效：{ $error }
api-error-capture-backend = 接收后端错误：{ $error }
api-error-capture-state = 捕获 { $id } 处于 { $state } 状态；无法对其执行 { $action }。
api-error-internal = 服务器内部错误：{ $error }
auth-error-unauthorized = 请登录。
auth-error-bad-credentials = 用户名或密码错误。
auth-error-setup-required = 尚未设置管理员密码：请先完成初始设置。
auth-error-already-configured = 管理员密码已设置。
auth-error-rate-limited = 登录失败次数过多。请在 { $secs } 秒后重试。
auth-error-csrf = CSRF 令牌缺失或无效 (X-CSRF-Token 首部)。
auth-error-origin = 不允许来自其他源的 WebSocket 连接。
auth-error-setup-remote = 只能在服务器本机 (localhost) 上设置管理员密码。
auth-error-password-mismatch = 两次输入的密码不一致。
auth-error-password-short = 密码长度至少为 { $min } 个字符。
lint-parse-error = 无法解析模板：{ $error }

## 配置 (wpg.toml)
config-issue-file-missing = 未找到文件
config-issue-bad-url = 必须以 http:// 或 https:// 开头
config-issue-not-argon2 = 看起来不是 argon2 哈希 ($argon2id$...)
config-issue-password-both = 同时设置了 password_hash 和 password_file；以 password_hash 为准
config-issue-zero = 必须大于 0
config-issue-exposed-without-password = 在非环回地址上监听但未设置密码
config-issue-dir-missing = 未找到目录
config-issue-protocols = 无法加载用户自定义协议
protocols-ext-loaded = 已加载 { $count } 个用户自定义协议
protocols-ext-error = 无法加载用户自定义协议: { $file }: { $error }
config-issue-max-rate-percent = 链路百分比不能作为服务器全局最大值；请使用 pps 或 bps
config-issue-bad-rate = 无效的速率
config-issue-unknown-backend = 未知后端
config-issue-backend-unavailable = 此构建 / 此操作系统中不可用该后端
config-issue-bad-log-level = 无效的日志过滤器
config-level-error = 错误
config-level-warning = 警告
config-issue-line = { $level }：{ $key }：{ $message }{ $detail }
config-load-error = 配置错误：{ $error }
config-init-written = 已写入 { $path }。请编辑该文件，然后用 `wpg config validate` 检查。
config-init-exists = { $path } 已存在 (使用 --force 覆盖)。
config-validate-ok = 配置正确 ({ $source })。
config-validate-summary = { $errors } 个错误，{ $warnings } 个警告 ({ $source })。
config-source-defaults = 内置默认值和环境变量
config-password-prompt = 新的管理员密码：{ " " }
config-password-confirm = 再次输入密码：{ " " }
config-password-mismatch = 两次输入的密码不一致。
config-password-short = 密码长度至少为 { $min } 个字符。
config-password-written = 已将密码哈希保存到 { $path } (auth.password_hash)。请重启 `wpg serve` 使其生效。

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = 监听地址       { $url }/
serve-banner-docs = API 文档       { $url }/api/docs/
serve-banner-config = 配置           { $source }
serve-banner-data = 数据目录       { $path }
serve-banner-library = 模板库         { $path }{ $state }
serve-banner-user-dir = 用户模板       { $path }
serve-banner-templates = 模板           { $count }
serve-banner-backends = 后端           { $list }
serve-banner-missing = { " " }(不存在)
serve-banner-dev = 开发模式：允许来自 { $origins } 的 CORS
serve-banner-no-password = 未配置密码，且服务器正在 { $url } 上监听 (--allow-insecure)：任何能访问它的人都可以设置管理员密码并发送数据包。请尽快设置密码 (wpg config set-password)。
serve-failed = 服务器错误：{ $error }
serve-log-error = 无法设置日志：{ $error }
serve-banner-tls = TLS            { $path }
serve-banner-tls-self-signed = TLS            { $path } (自签名)
serve-banner-fingerprint = SHA-256        { $fingerprint }
serve-banner-auth = 登录           { $state }
serve-banner-auth-setup = 尚未设置密码：请在初始设置页面中设置
serve-banner-auth-configured = 管理员密码来自 { $source }
serve-banner-auth-database = 管理员密码已在初始设置页面中设置
serve-banner-setup-local = 尚未设置管理员密码。请在本机的浏览器中打开 { $url } 进行设置。
serve-refuse-insecure = 拒绝在 { $addr } 上监听：未配置管理员密码。请使用环回地址 (127.0.0.1)、设置密码 (--password-file、WPG_PASSWORD、wpg config set-password)，或指定 --allow-insecure。
serve-password-arg-warning = 警告：--password 会在进程列表中被其他用户看到；建议使用 --password-file、WPG_PASSWORD 或 `wpg config set-password`。

## Web UI (wpg-web)。键由 crates/wpg-web 中的 `ui_keys_exist` 测试检查。
ui-lang-ja = 日本語
ui-lang-en = English
ui-lang-zh-cn = 简体中文
ui-lang-zh-tw = 繁體中文
ui-lang-ko = 한국어
ui-lang-de = Deutsch
ui-lang-fr = Français
ui-lang-es = Español
ui-loading = 正在加载…
ui-close = 关闭
ui-cancel = 取消
ui-delete = 删除
ui-save = 保存
ui-refresh = 刷新
ui-retry = 重试
ui-yes = 是
ui-no = 否
ui-unlimited = 不限
ui-help = 键盘快捷键
ui-help-title = 键盘快捷键
ui-skip-to-content = 跳到主要内容
ui-not-found = 此页面不存在。
ui-back-to-dashboard = 返回仪表板
ui-view-only = 屏幕较窄：仅可查看。请在更宽的屏幕上进行编辑。
ui-server-unreachable = 无法连接到服务器。`wpg serve` 是否正在运行？
ui-error-http = 服务器返回了 HTTP { $id }。
ui-error-network = 网络错误：{ $error }
ui-error-decode = 服务器返回了意外的响应：{ $error }
ui-nav-label = 主导航
ui-nav-dashboard = 仪表板
ui-nav-builder = 构建器
ui-nav-jobs = 任务
ui-nav-templates = 模板
ui-nav-rx = 接收
ui-nav-settings = 设置
ui-shortcut-hint = 显示键盘快捷键
ui-shortcut-dashboard = 转到仪表板
ui-shortcut-builder = 转到构建器
ui-shortcut-jobs = 转到任务
ui-shortcut-templates = 转到模板
ui-shortcut-rx = 转到接收
ui-shortcut-settings = 转到设置
ui-shortcut-new-job = 新建任务
ui-shortcut-logs = 显示 / 隐藏服务器日志
ui-shortcut-help = 显示此帮助
ui-shortcut-escape = 关闭对话框
ui-ws-connecting = 正在连接…
ui-ws-open = 实时
ui-ws-closed = 已断开 (正在重试)
ui-theme = 主题
ui-theme-light = 浅色
ui-theme-dark = 深色
ui-theme-system = 跟随系统
ui-theme-to-dark = 切换到深色主题
ui-theme-to-light = 切换到浅色主题
ui-logout = 退出登录
ui-logs-toggle = 服务器日志
ui-logs-title = 服务器日志
ui-logs-level = 级别
ui-logs-follow = 自动滚动
ui-logs-clear = 清空日志
ui-logs-count = { $count } 行
ui-logs-empty = 暂无日志。
ui-login-title = 登录
ui-login-submit = 登录
ui-login-busy = 正在登录…
ui-login-wrong = 密码错误。
ui-login-rate-limited = 尝试次数过多。请在 { $secs } s 后重试。
ui-login-wait = 请等待 { $secs } s
ui-password = 密码
ui-password-confirm = 确认密码
ui-setup-title = 设置管理员密码
ui-setup-intro = 此服务器尚未设置密码。请设置一个密码来保护 Web UI 和 API。
ui-setup-hint = 至少 { $min } 个字符。
ui-setup-too-short = 密码至少需要 { $min } 个字符。
ui-setup-mismatch = 两次输入的密码不一致。
ui-setup-submit = 设置密码
ui-dash-totals = 总计
ui-dash-active-jobs = 活动任务
ui-dash-running-jobs = 发送中
ui-dash-total-pps = 总 pps
ui-dash-total-bps = 总比特率
ui-dash-templates = 模板
ui-dash-uptime = 服务器运行时间
ui-dash-running = 运行中的任务
ui-dash-no-running = 没有正在运行的任务。
ui-dash-doctor = 自检
ui-dash-ifaces = 接口
ui-doctor-ok = 全部检查通过
ui-doctor-warn = 有警告
ui-doctor-fail = 发现问题
ui-doctor-counts = { $ok } 项正常 · { $warn } 项警告 · { $fail } 项失败
ui-doctor-details = 完整报告
ui-doctor-status = 状态
ui-doctor-check = 检查项
ui-doctor-detail = 详情 / 解决方法
ui-doctor-status-ok = 正常
ui-doctor-status-warn = 警告
ui-doctor-status-fail = 失败
ui-doctor-status-skip = 已跳过
ui-iface-name = 名称
ui-iface-state = 状态
ui-iface-mac = MAC
ui-iface-addrs = 地址
ui-iface-mtu = MTU
ui-iface-speed = 速率
ui-iface-allowed = 允许发送
ui-iface-up = Up
ui-iface-down = Down
ui-iface-no-carrier = 无载波
ui-stat-pps = 包/秒
ui-stat-bps = 比特率 (L2)
ui-stat-l1-bps = 比特率 (L1)
ui-stat-packets = 数据包
ui-stat-bytes = 字节
ui-stat-dropped = 丢弃
ui-stat-ratio = 速率达成率
ui-stat-elapsed = 已用时间
ui-spark-pps = 每秒数据包数 (最近 30 秒)
ui-state-created = 已创建
ui-state-scheduled = 已计划
ui-state-running = 运行中
ui-state-paused = 已暂停
ui-state-finished = 已完成
ui-state-failed = 失败
ui-state-unknown = 未知
ui-reason-count-reached = 已达到数据包数量
ui-reason-duration-elapsed = 已达到时长
ui-reason-source-exhausted = 已发送全部数据包
ui-reason-stopped = 已停止
ui-jobs-new = 新建任务
ui-jobs-tab-live = 当前
ui-jobs-tab-history = 历史
ui-jobs-empty = 没有任务。
ui-jobs-streams = 流
ui-jobs-template = 模板
ui-jobs-choose-template = 选择模板…
ui-jobs-weight = 权重
ui-jobs-add-stream = 添加流
ui-jobs-remove-stream = 删除流
ui-jobs-mix = 混合方式
ui-mix-weighted = 加权
ui-mix-sequential = 顺序
ui-mix-round-robin = 轮询
ui-jobs-target = 发送位置
ui-jobs-backend = 后端
ui-jobs-iface = 接口
ui-jobs-choose-iface = 选择接口…
ui-jobs-iface-not-needed = 此后端不需要
ui-jobs-iface-denied = 不允许
ui-jobs-dry-run = 试运行：pcapng 文件名
ui-jobs-dry-run-hint = 设置后不会实际发送：数据包将写入服务器捕获目录下的此文件。
ui-jobs-how = 发送方式
ui-jobs-rate = 速率
ui-jobs-count = 数据包数量
ui-jobs-duration = 时长
ui-jobs-burst = 突发
ui-jobs-workers = 工作线程
ui-jobs-workers-disabled = 多个工作线程需要支持多线程的后端，且只能有一个流。
ui-jobs-from-template = 使用模板设置
ui-jobs-name = 名称
ui-jobs-name-placeholder = 默认为模板名称
ui-jobs-start-now = 立即开始
ui-jobs-create = 创建任务
ui-jobs-create-start = 创建并开始
ui-jobs-created = 已创建任务 “{ $id }”。
ui-jobs-need-iface = 请选择接口。
ui-jobs-need-template = 请为每个流选择模板。
ui-jobs-bad-weight = 权重必须为正整数。
ui-rate-default = 留空：使用模板的速率 (或服务器的上限)。
ui-rate-is-pps = → { $rate }
ui-rate-is-bps = → 线路上的 { $rate } (第 1 层)
ui-rate-is-percent = → 链路速率的 { $rate } %
ui-rate-is-ifg = 帧间固定间隔：{ $rate }
ui-rate-is-unlimited = 尽可能快
ui-valid-empty = 必填。
ui-valid-number = 不是数字。
ui-valid-integer = 请输入整数。
ui-valid-range = 必须大于零。
ui-valid-percent = 百分比不能超过 100。
ui-valid-rate-unit = 未知单位：请使用 pps、kpps、Mpps、bps、kbps、Mbps、Gbps、%、ifg=<时间> 或 unlimited。
ui-valid-duration-unit = 未知单位：请使用 ns、us、ms、s、m 或 h。
ui-valid-capture-name = 请使用不含 “..” 的相对文件名。
ui-confirm-send-title = 在真实接口上发送？
ui-confirm-send-body = 此任务会向网络发送数据包。请确认接口和速率适合该网络。
ui-confirm-send = 发送
ui-job-detail = 任务
ui-job-state = 状态
ui-job-start = 开始
ui-job-pause = 暂停
ui-job-resume = 继续
ui-job-stop = 停止
ui-job-download-report = 报告 (JSON)
ui-job-live = 实时统计
ui-job-streams = 按流
ui-job-inline-template = (内联模板)
ui-job-workers = 按工作线程
ui-job-workers-pending = 各工作线程的计数包含在最终报告中，任务结束后即可查看。
ui-job-active = 活动时间
ui-job-core = 核心
ui-job-accuracy = 速率控制精度
ui-acc-target = 目标
ui-acc-achieved = 实际
ui-acc-jitter = 抖动 (平均 / 最大)
ui-job-spec = 设置
ui-job-created = 创建时间
ui-job-started = 开始时间
ui-job-finished = 结束时间
ui-job-deleted = 已删除任务。
ui-job-delete-title = 删除此任务？
ui-job-delete-body = 正在运行的任务会先被停止；其报告仍保留在历史记录中。
ui-tpl-import = 导入
ui-tpl-new = 新建模板
ui-tpl-search = 搜索名称、描述、标签、协议…
ui-tpl-source = 来源
ui-tpl-source-all = 全部
ui-tpl-source-library = 模板库
ui-tpl-source-user = 我的
ui-tpl-favorites-only = 仅收藏
ui-tpl-tags = 标签
ui-tpl-recent = 最近使用
ui-tpl-empty = 没有匹配的模板。
ui-tpl-favorite = 收藏
ui-tpl-view = 查看 / 编辑 YAML
ui-tpl-open-builder = 在构建器中打开
ui-tpl-new-job = 使用此模板新建任务
ui-tpl-export = 下载 YAML
ui-tpl-duplicate = 复制
ui-tpl-imported = 已导入模板 “{ $id }”。
ui-tpl-duplicated = 已创建副本 “{ $id }”。
ui-tpl-deleted = 已删除模板 “{ $id }”。
ui-tpl-delete-title = 删除此模板？
ui-tpl-delete-body = 将从你的模板目录中删除 “{ $id }”。
ui-tpl-saved = 模板已保存。
ui-tpl-yaml = 模板 YAML
ui-tpl-read-only = 模板库中的模板为只读：请复制后编辑副本。
ui-lint-run = 检查 (lint)
ui-lint-clean = 未发现问题。
ui-lint-summary = { $errors } 个错误，{ $warnings } 个警告
ui-set-appearance = 外观
ui-set-account = 账户
ui-set-logged-in = 已以 { $id } 身份登录。
ui-set-auth-disabled = 此服务器未启用身份验证。
ui-set-client = 此浏览器
ui-set-client-note = 仅保存在此浏览器中。服务器配置 (wpg.toml) 不会通过 API 公开。
ui-set-api = API
ui-set-ws = 事件
ui-set-ui-version = UI 版本
ui-set-api-docs = API 文档 (Swagger UI)
ui-set-about = 关于
ui-set-server-version = 服务器版本
ui-set-api-version = API 版本
ui-set-platform = 平台
ui-set-features = 可选功能
ui-set-locales = 语言
ui-set-license = wpg 采用 MIT 或 Apache-2.0 双重许可。
ui-builder-placeholder = 数据包构建器 (协议面板、层堆栈、字段编辑器、十六进制 / 树视图) 将在下一个里程碑步骤中提供。
ui-builder-placeholder-template = 数据包构建器将在下一个里程碑步骤中提供。模板 “{ $id }” 将在此处打开。
ui-reason-end-of-input = 已到达捕获文件末尾
ui-valid-stream-id = 请输入 0 到 4294967295 之间的整数。
ui-jobs-verification = 接收验证
ui-jobs-signature = 添加验证签名
ui-jobs-signature-hint = 在每个帧载荷的最后 24 个八位组中写入流 ID、序列号和发送时间，从而在启用 “验证签名” 的捕获中测量丢包、顺序和时延。
ui-jobs-stream-id = 签名流 ID (第一个流)
ui-job-signature = 签名
ui-job-signature-on = 开启 (流 ID 从 { $id } 开始)
ui-rx-new = 新建捕获
ui-rx-file = 捕获文件
ui-rx-file-hint = 服务器捕获目录中的 pcap / pcapng 文件名，例如任务试运行的输出。
ui-rx-iface-any = any (所有接口)
ui-rx-filter = 过滤器
ui-rx-filter-empty = 留空：匹配所有帧。
ui-rx-filter-checking = 正在检查过滤器…
ui-rx-filter-ok = 有效：{ $filter }
ui-rx-filter-error = { $error } (位于第 { $offset } 个字符)
ui-rx-count-hint = 匹配到此数量的帧后停止。
ui-rx-keep = 保留的帧数
ui-rx-keep-hint = 保留最先匹配到的 N 个帧用于显示 (最多 10000)。
ui-rx-keep-max = 最多 10000。
ui-rx-save = 另存为 pcapng (文件名)
ui-rx-save-hint = 可选：匹配的帧还会写入服务器捕获目录中的此文件。
ui-rx-save-same = 输出文件必须与输入文件不同。
ui-rx-name-placeholder = 默认为 后端:接口
ui-rx-options = 选项
ui-rx-verify = 验证签名 (丢包、顺序、时延)
ui-rx-promisc = 混杂模式
ui-rx-outgoing = 包含本机发送的帧
ui-rx-start = 开始捕获
ui-rx-started = 已开始捕获 “{ $id }”。
ui-rx-list = 捕获
ui-rx-empty = 暂无捕获。
ui-rx-source = 接口 / 文件
ui-rx-actions = 操作
ui-rx-stop = 停止
ui-rx-stop-named = 停止 “{ $id }”
ui-rx-delete-named = 删除 “{ $id }”
ui-rx-deleted = 已删除捕获。
ui-rx-delete-title = 删除此捕获？
ui-rx-delete-body = 正在运行的捕获会先被停止。已保存的 pcapng 文件仍保留在服务器上。
ui-rx-download = 下载 pcapng
ui-rx-counters = 计数器
ui-rx-matched = 匹配
ui-rx-matched-pps = 匹配包/秒
ui-rx-matched-bps = 匹配比特率
ui-rx-matched-bytes = 匹配字节数
ui-rx-saved = 已保存到文件
ui-rx-kept = 保留用于显示
ui-rx-verify-title = 验证
ui-rx-verify-off = 此捕获未启用签名验证。
ui-rx-verify-none = 尚未收到带签名的帧。
ui-rx-signed = 带签名的帧
ui-rx-unsigned = 无签名的帧
ui-rx-lost = 丢失
ui-rx-stream = 流
ui-rx-received = 已接收
ui-rx-seq-range = 序列号
ui-rx-out-of-order = 乱序
ui-rx-duplicates = 重复
ui-rx-latency = 时延 (最小 / 平均 / 最大)
ui-rx-jitter = 抖动
ui-rx-latency-note = 丢失的帧根据序列号的间隙估算。只有当发送方与接收方使用同一时钟 (同一主机，或已通过 PTP / NTP 同步) 时，时延才有意义。
ui-rx-frames = 已捕获的帧
ui-rx-frames-empty = 暂无匹配的帧。
ui-rx-time = 时间 (s)
ui-rx-length = 长度
ui-rx-signature = 签名
ui-rx-sig-value = 流 { $stream } · 序号 { $seq }
ui-rx-show-more = 显示更多 ({ $count } / { $max })
ui-rx-frame-title = 帧 { $index }
ui-rx-select-frame = 选择一个帧以查看其八位组和解码后的各层。
ui-rx-decode-failed = 无法解码该帧：{ $error }
ui-rx-truncated = 仅捕获了 { $len } 个八位组中的 { $bytes } 个。

# --- P1 L2/封装协议字段说明 ---
lldp-tlvs = LLDP TLV (7 位类型 + 9 位长度首部)。
cdp-version = CDP 协议版本 (2)。
cdp-ttl = 此通告保持有效的时间，单位为秒。
cdp-checksum = 覆盖 CDP 首部和 TLV 的 Internet 校验和 (Auto)。
cdp-tlvs = CDP TLV (16 位类型，16 位长度，包含首部)。
stp-protocol-id = 生成树协议标识符 (始终为 0)。
stp-version = 协议版本 (0 = STP，2 = RSTP，3 = MSTP)。
stp-type = BPDU 类型 (0 = 配置，2 = RST/MST，0x80 = TCN)。
stp-flags = BPDU 标志 (拓扑变更、提议、角色、学习、转发、同意、TCA)。
stp-root-priority = 根桥优先级和系统 ID 扩展。
stp-root-mac = 根桥 MAC 地址。
stp-root-path-cost = 到根桥的路径开销。
stp-bridge-priority = 发送桥的优先级和系统 ID 扩展。
stp-bridge-mac = 发送桥的 MAC 地址。
stp-port-id = 发送端口标识符 (优先级 + 编号)。
stp-message-age = BPDU 的消息老化时间，单位为 1/256 秒。
stp-max-age = BPDU 超时前的最大老化时间，单位为 1/256 秒。
stp-hello-time = 配置 BPDU 的发送间隔，单位为 1/256 秒。
stp-forward-delay = 转发延迟定时器，单位为 1/256 秒。
stp-trailer = RSTP / MSTP 扩展 (version-1 长度、v3 长度、MSTI 记录)；不透明八位组。
lacp-subtype = 慢协议子类型 (1 = LACP)。
lacp-actor-system = 本端 (Actor) 系统 MAC 地址。
lacp-actor-state = 本端 (Actor) 状态标志 (活动、超时、聚合、同步、收集、分发、默认、过期)。
lacp-partner-system = 对端 (Partner) 系统 MAC 地址。
lacp-partner-state = 对端 (Partner) 状态标志。
marker-subtype = 慢协议子类型 (2 = Marker)。
eapol-version = EAPOL 协议版本。
eapol-type = EAPOL 报文类型 (0 = EAP，1 = Start，2 = Logoff，3 = Key)。
eapol-length = EAPOL 主体的长度，单位为八位组 (Auto)。
eapol-body = EAPOL 主体 (类型 0 时为 EAP 报文)；不透明八位组。
pppoe-version = PPPoE 协议版本 (1)。
pppoe-type = PPPoE 类型 (1)。
pppoe-code = PPPoE 代码 (PADI / PADO / PADR / PADS / PADT，0 = 会话数据)。
pppoe-session-id = PPPoE 会话标识符。
pppoe-length = PPPoE 载荷的长度，单位为八位组 (Auto)。
pppoe-tags = PPPoE 发现阶段 TAG (16 位类型，16 位长度)。
ppp-protocol = 载荷的 PPP 协议标识符 (LCP / IPCP / IP / IPv6 / ...)。
ppp-cp-code = PPP 控制协议代码 (Configure-Request / Ack / Nak / ...)。
ppp-cp-identifier = PPP 控制协议请求标识符。
ppp-cp-length = PPP 控制协议报文的长度，单位为八位组 (Auto)。
ppp-cp-options = PPP 控制协议选项 (8 位类型 / 长度 TLV)。
vxlan-flags = VXLAN 标志 (I 位 = VNI 有效)。
vxlan-vni = 24 位 VXLAN 网络标识符 (VNI)。
gre-flags = GRE 标志和版本 (校验和 / 密钥 / 序列号存在位)。
gre-protocol-type = 封装载荷的 EtherType。
nvgre-vsid = 24 位 NVGRE 虚拟子网 ID (VSID)。
nvgre-flowid = NVGRE FlowID (用于负载均衡的熵)。
mpls-label = 20 位 MPLS 标签值。0-15 为保留值 (0 / 2 显式空，1 路由器告警，3 隐式空，7 熵标签指示符，13 GAL，14 OAM 告警，15 扩展)；SR-MPLS SID 为 SRGB 基值 + SID 索引。
mpls-tc = 3 位流量类别 (QoS / ECN)。
mpls-s = 栈底位 (1 = 最后一个标签；根据下一层自动设置)。
mpls-ttl = MPLS 生存时间。
gtpu-version = GTP 版本 (1)。
gtpu-pt = 协议类型 (1 = GTP，0 = GTP')。
gtpu-e = 扩展首部存在标志。
gtpu-s = 序列号存在标志。
gtpu-pn = N-PDU 编号存在标志。
gtpu-message-type = GTP-U 消息类型 (255 = G-PDU)。
gtpu-length = 必选首部之后的载荷长度，单位为八位组 (Auto)。
gtpu-teid = 隧道端点标识符 (TEID)。
geneve-version = Geneve 版本 (0)。
geneve-opt-len = 选项区域的长度，单位为 4 个八位组的字。
geneve-o = OAM 报文标志。
geneve-c = 存在关键选项标志。
geneve-protocol-type = 封装载荷的 EtherType。
geneve-vni = 24 位 Geneve 虚拟网络标识符 (VNI)。
geneve-options = Geneve 选项 (不透明；大小由 opt_len 决定)。
l2tp-flags = L2TP 标志和版本 (类型、长度、序列、偏移、优先级位)。
l2tp-body = L2TP 主体 (按条件出现的字段 + 载荷)；不透明八位组。
l2tpv3-session-id = L2TPv3 会话 ID。

# --- 数据包构建器 (M4 第 2 阶段) ---
ui-bld-palette = 协议
ui-bld-search = 搜索协议
ui-bld-favorite = 收藏
ui-bld-favorites = 收藏夹
ui-bld-layer-l2 = 数据链路层 (L2)
ui-bld-layer-l3 = 网络层 (L3)
ui-bld-layer-l4 = 传输层 (L4)
ui-bld-layer-l7 = 应用层 (L7)
ui-bld-layer-payload = 载荷
ui-bld-stack = 层
ui-bld-empty = 将协议拖到此处，或从协议面板中添加。
ui-bld-unknown = 未知
ui-bld-dup = 复制层
ui-bld-suggest = 添加下一层
ui-bld-binding-warn = 下层未指明此协议
ui-bld-fix = 修复
ui-bld-added = 已添加 { $id }。
ui-bld-moved = 已移动到位置 { $index }。
ui-bld-removed = 已删除层。
ui-bld-fields = 字段编辑器
ui-bld-select-layer = 选择一个层以编辑其字段。
ui-bld-auto = Auto
ui-bld-manual = Manual
ui-bld-gen = Gen
ui-bld-computed = 构建时计算
ui-bld-nsap-parts = AFI { $afi } · 区域 { $area } · 系统 ID { $sys } · NSEL { $nsel }
ui-bld-estimate = 流中的数据包数及总大小
ui-bld-expands = 展开为 { $count } (约 { $bytes })
ui-bld-rate = 速率
ui-bld-rate-hint = 随模板保存的发送速率 (1000pps、1.5Mpps、10Gbps、ifg=10us)
ui-bld-duration = 发送约需 { $time }
ui-bld-duration-unknown = 发送时间：请设置速率
ui-bld-duration-hint = 按此速率发送的数据包数 (tx.count，未设置时为流长度)，上限为 tx.duration_secs
ui-bld-gen-kind = 类型
ui-bld-gen-count = 数量：
ui-bld-gen-shorthand = 范围 / 列表 / CIDR
ui-bld-gen-random = 随机
ui-bld-gen-random-mac = 随机 MAC
ui-bld-gen-counter = 计数器
ui-bld-gen-file = 文件
ui-bld-gen-expr = 表达式
ui-bld-hex = 十六进制
ui-bld-tree = 树
ui-bld-no-bytes = 尚无数据包。
ui-bld-name = 模板名称
ui-bld-need-name = 请先为模板命名。
ui-bld-unsaved = 有未保存的更改
ui-bld-save-as = 另存为…
ui-bld-save-first = 创建任务前请先保存模板。
ui-bld-recompute = 重新计算
ui-bld-auto-recompute = 每次编辑时实时重新计算预览
ui-bld-index = 流索引
ui-bld-index-prev = 上一个数据包
ui-bld-index-next = 下一个数据包
ui-bld-undo = 撤销
ui-bld-redo = 重做
ui-bld-parity-ok = 浏览器内预览与服务器渲染结果一致。
ui-bld-parity-ok-short = 一致
ui-bld-parity-bad = 浏览器内预览与服务器渲染结果不一致。
ui-bld-parity-bad-short = 不一致
ui-bld-parity-pending = 正在检查服务器渲染结果…
ui-bld-help-title = 构建器快捷键
ui-bld-help-add = 添加获得焦点的协议
ui-bld-help-move = 移动所选的层
ui-bld-help-del = 删除所选的层
ui-bld-help-undo = 撤销
ui-bld-help-redo = 重做
ui-bld-help-hex = 在光标所在字节处输入十六进制
ui-bld-help-caret = 移动十六进制光标
ui-bld-suggest-prev = 在上方添加层

# --- P1 冗余 / 组播协议字段说明 ---
vrrp-version = VRRP 版本 (2 = RFC 3768，3 = RFC 5798)。
vrrp-type = VRRP 报文类型 (1 = Advertisement)。
vrrp-vrid = 虚拟路由器标识符 (VRID)。
vrrp-priority = 发送路由器的优先级 (255 = 地址所有者，0 = 停止参与)。
vrrp-count = 通告中的虚拟地址数量 (Auto)。
vrrp-auth-type = VRRPv2 认证类型 (0 = 无，1 = 简单文本，2 = IP AH)。
vrrp-adver-int = VRRPv2 通告间隔，单位为秒。
vrrp-max-adver-int = VRRPv3 最大通告间隔，单位为厘秒。
vrrp-checksum = 覆盖 VRRP 消息的 Internet 校验和 (VRRPv3 包含 IP 伪首部) (Auto)。
vrrp-addresses = 虚拟路由器地址。
vrrp-auth-data = VRRPv2 认证数据 (8 个八位组)。
hsrp-version = HSRP 版本 (HSRPv1 为 0)。
hsrp-op-code = HSRP 操作码 (0 = Hello，1 = Coup，2 = Resign，3 = Advertise)。
hsrp-state = 发送方状态 (1 = Learn，2 = Listen，4 = Speak，8 = Standby，16 = Active)。
hsrp-hello-time = Hello 消息的发送间隔，单位为秒。
hsrp-hold-time = Hello 保持有效的时间，单位为秒。
hsrp-priority = 组内的路由器优先级。
hsrp-group = 备份组编号。
hsrp-auth-data = 明文认证密码 (8 个八位组，默认为 "cisco")。
hsrp-virtual-ip = 组的虚拟 IPv4 地址。
hsrpv2-tlvs = HSRPv2 TLV (Group State、Interface State、Text / MD5 Authentication；8 位类型和长度)。
glbp-version = GLBP 版本 (1)。
glbp-group = GLBP 组编号。
glbp-owner-id = 发送路由器的 MAC 地址。
glbp-tlvs = GLBP TLV (Hello、Request/Response、Authentication；长度包含首部)。
igmp-type = IGMP 消息类型 (根据主体自动设置：0x11 查询，0x12 / 0x16 报告，0x17 离开，0x22 v3 报告)。
igmp-max-resp-code = 最大响应时间 / 代码，单位为 1/10 s (查询报文；报告中为零)。
igmp-checksum = 覆盖整个 IGMP 消息的 Internet 校验和 (Auto)。
igmp-group = 组播组地址 (通用查询中为 0.0.0.0)。
igmp-s-flag = 抑制路由器侧处理标志。
igmp-qrv = 查询器健壮性变量 (QRV)。
igmp-qqic = 查询器查询间隔代码 (QQIC)。
igmp-num-sources = 源地址数量。
igmp-sources = 源地址。
igmp-num-records = 组记录数量。
igmp-records = 组记录 (记录类型、辅助数据长度、源、组播地址)。
mld-max-resp = 最大响应延迟 / 代码，单位为毫秒。
mld-address = IPv6 组播地址 (通用查询中为 ::)。
pim-version = PIM 版本 (2)。
pim-type = PIM 消息类型 (根据主体自动设置：0 Hello，1 Register，3 Join/Prune)。
pim-checksum = Internet 校验和 (整个消息；Register 仅覆盖前 8 个八位组) (Auto)。
pim-hello-options = PIM Hello 选项 (Holdtime、LAN Prune Delay、DR Priority、Generation ID、Address List)。
pim-addr-family = 编码地址的地址族 (1 = IPv4)。
pim-upstream-neighbor = Join/Prune 所发往的上游邻居地址。
pim-num-groups = 组集合数量。
pim-holdtime = Join/Prune 状态的保持时间，单位为秒。
pim-groups = 组集合 (编码组地址、加入和剪除的编码源地址)。
pim-register-border = Border 位 (发送方为 PIM 组播边界路由器)。
pim-register-null = Null-Register 位 (不含封装的数据报文)。

# --- P1 应用层协议字段说明 ---
dhcp-op = BOOTP 消息操作码 (1 = BOOTREQUEST，2 = BOOTREPLY)。
dhcp-htype = 硬件地址类型 (1 = Ethernet)。
dhcp-hlen = 硬件地址长度，单位为八位组 (Ethernet 为 6)。
dhcp-hops = 中继跳数，每经过一个中继代理加一。
dhcp-xid = 由客户端选择的事务 ID。
dhcp-secs = 客户端开始获取地址以来经过的秒数。
dhcp-flags = BOOTP 标志 (第 15 位 = 请求广播应答)。
dhcp-ciaddr = 客户端 IP 地址 (已绑定时)。
dhcp-yiaddr = 服务器分配的 "你的" (客户端) IP 地址。
dhcp-siaddr = 下一个服务器的 IP 地址 (引导服务器)。
dhcp-giaddr = 中继代理 (网关) IP 地址；经中继时不为零。
dhcp-chaddr = 客户端硬件 (MAC) 地址；后跟 10 个八位组的填充。
dhcp-sname = 可选的服务器主机名 (64 个八位组，以 NUL 填充)。
dhcp-file = 引导文件名 (128 个八位组，以 NUL 填充)。
dhcp-magic-cookie = DHCP magic cookie (0x63825363)。
dhcp-options = DHCP 选项 (8 位代码 / 长度；Pad、End、Message Type、Relay Agent Information、...)。
dhcpv6-msg-type = DHCPv6 消息类型 (Solicit、Advertise、Request、Reply、Relay-Forward、...)。
dhcpv6-transaction-id = 24 位事务 ID。
dhcpv6-options = DHCPv6 选项 (16 位代码 / 长度；Client ID、IA_NA、Relay Message、...)。
dhcpv6-hop-count = 转发过此消息的中继代理数量。
dhcpv6-link-address = 标识客户端所在链路的地址 (中继)。
dhcpv6-peer-address = 消息来源的客户端或中继的地址。
dns-id = 查询标识符，会被复制到响应中。
dns-qr = 查询 (0) 或响应 (1)。
dns-opcode = 查询类型 (0 = 标准查询，4 = NOTIFY，5 = UPDATE)。
dns-aa = 权威应答标志。
dns-tc = 截断标志。
dns-rd = 期望递归标志。
dns-ra = 可用递归标志。
dns-z = 保留位 (必须为 0)。
dns-ad = 真实数据标志 (DNSSEC)。
dns-cd = 禁用检查标志 (DNSSEC)。
dns-rcode = 响应代码 (0 = NOERROR，3 = NXDOMAIN，...)。
dns-qdcount = 问题部分的条目数。
dns-ancount = 回答部分的资源记录数。
dns-nscount = 授权部分的资源记录数。
dns-arcount = 附加部分的资源记录数。
dns-sections = 问题 / 回答 / 授权 / 附加部分 (带长度前缀的标签名)；不透明八位组。
ntp-li = 闰秒指示符 (3 = 时钟未同步)。
ntp-version = NTP 版本号 (4)。
ntp-mode = 关联模式 (3 = 客户端，4 = 服务器，5 = 广播)。
ntp-stratum = 层级 (0 = 未指定，1 = 主参考源，2-15 = 次级)。
ntp-poll = 最大轮询间隔，以 2 为底的秒数对数 (有符号)。
ntp-precision = 时钟精度，以 2 为底的秒数对数 (有符号)。
ntp-root-delay = 到参考时钟的往返延迟 (16.16 定点秒数)。
ntp-root-dispersion = 相对参考时钟的离散度 (16.16 定点秒数)。
ntp-reference-id = 参考 ID (层级 1 时为 ASCII 代码，否则为 IPv4 地址或哈希)。
ntp-reference-ts = 系统时钟最后一次被设置的时间 (64 位 NTP 时间戳)。
ntp-origin-ts = 由服务器回送的客户端发送时间 (64 位 NTP 时间戳)。
ntp-receive-ts = 请求到达服务器的时间 (64 位 NTP 时间戳)。
ntp-transmit-ts = 数据包离开发送方的时间 (64 位 NTP 时间戳)。
ntp-extensions = 扩展字段和 MAC；不透明八位组。
snmp-tag = BER 标签八位组 (0x30 SEQUENCE，0x02 INTEGER，0x04 OCTET STRING)。
snmp-msg-length = SNMP 消息 SEQUENCE 的 BER 长度 (Auto；短格式，小于 128)。
snmp-version-length = 版本 INTEGER 的 BER 长度 (1)。
snmp-version = SNMP 版本 (0 = v1，1 = v2c)。
snmp-community-length = 团体字符串的 BER 长度，单位为八位组。
snmp-community = 团体字符串 (例如 "public")。
snmp-pdu-type = PDU 标签 (0xa0 GetRequest，0xa2 Response，0xa4 v1 Trap，0xa7 SNMPv2-Trap，...)。
snmp-pdu-length = PDU 内容的 BER 长度 (Auto；短格式，小于 128)。
snmp-pdu-body = 以 BER TLV 表示的 PDU 内容：request-id、error-status、error-index 以及 variable-bindings SEQUENCE。
snmp-trap-body = 以 BER TLV 表示的 Trap 内容：enterprise、agent-addr、generic-trap、specific-trap、time-stamp 以及变量绑定。
syslog-message = Syslog 消息文本，以 <PRI> 开头 (facility * 8 + severity)。
radius-code = RADIUS 报文类型 (Access-Request、Access-Accept、Accounting-Request、...)。
radius-identifier = 用于匹配请求与应答的标识符。
radius-length = 整个 RADIUS 报文的长度，单位为八位组 (Auto)。
radius-authenticator = 16 个八位组的 Request / Response Authenticator (Manual)。
radius-attributes = RADIUS 属性 (8 位类型，8 位长度，包含首部)。
tftp-opcode = TFTP 操作码 (1 = RRQ，2 = WRQ，3 = DATA，4 = ACK，5 = ERROR，6 = OACK)。
tftp-filename = 文件名 (以 NUL 结尾)。
tftp-mode = 传输模式：netascii、octet 或 mail (以 NUL 结尾)。
tftp-options = 选项名 / 值字符串对 (RFC 2347)：blksize、timeout、tsize、windowsize、...
tftp-block = 块编号 (从 1 开始)。
tftp-data = 数据八位组 (每块 512 个；较短的块表示传输结束)。
tftp-error-code = TFTP 错误代码 (1 = 文件未找到，2 = 访问违规，...)。
tftp-error-message = 错误消息 (以 NUL 结尾)。

# --- P1 路由协议字段说明 ---
ospf-version = OSPF 版本 (2 = OSPFv2，3 = OSPFv3)。
ospf-type = OSPF 报文类型 (Hello / DB Description / LS Request / LS Update / LS Ack；根据下一层自动设置)。
ospf-packet-length = 包含首部在内的 OSPF 报文长度，单位为八位组 (Auto)。
ospf-router-id = 发送路由器的 Router ID。
ospf-area-id = 报文所属的区域 (0.0.0.0 = 骨干区域)。
ospf-checksum = 覆盖 OSPF 报文 (不含认证字段) 的 Internet 校验和 (Auto)。
ospf-au-type = 认证类型 (0 = 无，1 = 简单密码，2 = 加密)。
ospf-authentication = 64 位认证字段 (密码，或加密认证时的密钥 ID / 序列号)。
ospf-network-mask = 接口或所通告网络的网络掩码。
ospf-hello-interval = Hello 报文的发送间隔，单位为秒。
ospf-options = 可选能力 (E、MC、N/P、L、DC、O、DN 位)。
ospf-router-priority = 用于 DR 选举的路由器优先级 (0 = 永不成为 DR)。
ospf-router-dead-interval = 未收到 Hello 多少秒后宣告邻居失效。
ospf-designated-router = 指定路由器 (DR) (0.0.0.0 = 无)。
ospf-backup-designated-router = 备份指定路由器 (BDR) (0.0.0.0 = 无)。
ospf-neighbors = 在链路上发现的邻居的 Router ID。
ospf-interface-mtu = 接口无需分片即可发送的最大 IP 数据报。
ospf-dbd-flags = 数据库描述标志 (I = 初始，M = 更多，MS = 主)。
ospf-dd-sequence = 数据库描述序列号。
ospf-lsa-headers = LSA 首部 (每个 20 个八位组；校验和与长度为 Manual)。
ospf-lsr-requests = 请求的 LSA (LS 类型、Link State ID、通告路由器)。
ospf-num-lsas = 更新中的 LSA 数量。
ospf-ls-age = LSA 生成以来经过的时间，单位为秒 (保持为 0 可使 Auto 校验和精确)。
ospf-ls-type = LSA 类型 (根据 LSA 主体层自动设置；9 / 10 / 11 = 不透明 LSA)。
ospf-link-state-id = 标识所描述网络部分的 Link State ID。
ospf-advertising-router = 生成该 LSA 的路由器的 Router ID。
ospf-ls-sequence = LSA 序列号 (0x80000001 = 初始值)。
ospf-ls-checksum = 不含 LS age 的 LSA 的 Fletcher 校验和 (Auto)。
ospf-lsa-length = 包含首部在内的 LSA 长度，单位为八位组 (Auto)。
ospf-router-lsa-flags = Router-LSA 标志 (V = 虚链路端点，E = ASBR，B = ABR)。
ospf-num-links = 所描述的路由器链路数量。
ospf-router-links = 路由器链路 (链路 ID、链路数据、类型、TOS 数量、度量值)。
ospf-attached-routers = 连接到该网络的路由器的 Router ID。
ospf-metric = 路由的开销。
ospf-external-e = 外部度量类型 (1 = 类型 2，0 = 类型 1)。
ospf-forwarding-address = 发往目的地的流量所转发到的地址 (0.0.0.0 = ASBR)。
ospf-external-route-tag = 随外部路由携带的 32 位标记。
ospf-opaque-ls-type = 不透明 LSA 的泛洪范围 (9 = 本地链路，10 = 区域，11 = AS)。
ospf-opaque-type = 不透明类型 (1 = TE，4 = Router Information，7 = Extended Prefix，8 = Extended Link；根据主体层自动设置)。
ospf-opaque-id = 区分同一不透明类型的各个 LSA 的 Opaque ID (24 位)。
ospf-opaque-tlvs = 不透明 LSA 的 TLV (16 位类型和长度，值填充至 4 个八位组；长度为 Auto)。
ospfv3-checksum = 覆盖 OSPFv3 报文及 IPv6 伪首部的 Internet 校验和 (Auto)。
ospfv3-instance-id = 实例 ID (用于区分同一链路上的 OSPFv3 实例)。
ospfv3-interface-id = 发送接口的接口 ID。
ospfv3-options = OSPFv3 选项 (V6、E、N、R、DC、AF 位)。
ospfv3-ls-type = 带 U / S 范围位的 LSA 功能代码 (根据 LSA 主体层自动设置)。
ospfv3-router-links = 路由器接口 (类型、度量值、接口 ID、邻居 Router ID)。
ospfv3-prefix-length = 前缀长度，单位为位。
ospfv3-prefix-options = 前缀选项 (NU、LA、P、DN 位)。
ospfv3-address-prefix = 地址前缀，填充至 4 个八位组的整数倍。
ospfv3-external-flags = AS-External-LSA 标志 (E = 类型 2 度量，F = 存在转发地址，T = 存在路由标记)。
ospfv3-referenced-ls-type = 所引用 LSA 的 LS 类型。
ospfv3-link-local-address = 接口的链路本地地址。
ospfv3-num-prefixes = 后续前缀的数量。
ospfv3-prefixes = 前缀条目 (长度、选项、度量值 / 保留、地址前缀)；不透明八位组。
ospfv3-referenced-link-state-id = 所引用 LSA 的 Link State ID。
ospfv3-referenced-advertising-router = 所引用 LSA 的通告路由器。
bgp-marker = 16 个八位组的标记 (全为 1)。
bgp-length = 包含首部在内的 BGP 消息长度，单位为八位组 (Auto)。
bgp-type = BGP 消息类型 (OPEN / UPDATE / NOTIFICATION / KEEPALIVE / ROUTE-REFRESH；根据下一层自动设置)。
bgp-version = BGP 版本 (4)。
bgp-my-as = 发送方的自治系统号 (4 字节 AS 时为 23456 = AS_TRANS)。
bgp-hold-time = 建议的保持时间，单位为秒 (0 或至少 3)。
bgp-identifier = 发送方的 BGP 标识符 (Router ID)。
bgp-opt-params-length = 可选参数的长度，单位为八位组 (Auto)。
bgp-opt-params = 可选参数 (类型 2 = 能力)。
bgp-withdrawn-routes-length = 撤销路由的长度，单位为八位组 (Auto)。
bgp-withdrawn-routes = 撤销的 IPv4 路由 (前缀长度 (位)、前缀)。
bgp-total-path-attribute-length = 路径属性的长度，单位为八位组 (Auto)。
bgp-path-attributes = 路径属性 (标志 + 类型代码；设置 Extended Length 标志时长度为 16 位，超过 255 个八位组时自动设置)。
bgp-nlri = 通告的 IPv4 路由 (前缀长度 (位)、前缀)；其他地址族放在 MP_REACH_NLRI 中。
bgp-error-code = NOTIFICATION 错误代码。
bgp-error-subcode = NOTIFICATION 错误子代码。
bgp-error-data = NOTIFICATION 的诊断数据。
bgp-afi = 地址族标识符 (1 = IPv4，2 = IPv6)。
bgp-rr-subtype = ROUTE-REFRESH 子类型 (0 = 普通，1 = BoRR，2 = EoRR)。
bgp-safi = 子地址族标识符 (1 = 单播，2 = 组播)。
rip-command = RIP 命令 (1 = 请求，2 = 响应)。
rip-version = RIP 版本 (1 或 2；RIPng 使用 1)。
rip-entries = 路由条目 (AFI、路由标记、地址、掩码、下一跳、度量值)。
ripng-entries = 路由表条目 (前缀、路由标记、前缀长度、度量值；0xff = 下一跳)。
eigrp-version = EIGRP 首部版本 (2)。
eigrp-opcode = EIGRP 操作码 (Update / Query / Reply / Hello / SIA-Query / SIA-Reply)。
eigrp-checksum = 覆盖 EIGRP 报文的 Internet 校验和 (Auto)。
eigrp-flags = EIGRP 标志 (Init、Conditional Receive、Restart、End of Table)。
eigrp-sequence = 用于可靠传输的序列号。
eigrp-acknowledge = 确认的序列号 (0 = 无)。
eigrp-virtual-router-id = 虚拟路由器 ID (0 = 单播地址族)。
eigrp-autonomous-system = EIGRP 自治系统号。
eigrp-tlvs = EIGRP TLV (16 位类型，16 位长度，包含首部)。

# --- P2 OSI 封装字段说明 ---
osi-nlpid = 网络层协议标识符 (ISO/IEC TR 9577)：0x81 CLNP，0x82 ES-IS，0x83 IS-IS。
osi-data = NLPID 之后 OSI PDU 的其余部分 (不透明八位组)。
clnp-nlpid = 网络层协议标识符 (0x81 = CLNP)。
clnp-length-indicator = 首部长度，单位为八位组，计算到选项末尾为止 (Auto)。
clnp-version = 版本 / 协议 ID 扩展 (1)。
clnp-lifetime = 剩余生存时间，单位为 500 ms。
clnp-sp = SP：允许分段；存在分段部分 (根据分段部分自动设置)。
clnp-ms = MS：后面还有分段。
clnp-er = E/R：PDU 被丢弃时发送错误报告。
clnp-pdu-type = PDU 类型：0x1c DT (数据)，0x1d MD (组播数据)，0x01 ER (错误报告)，0x1e ERQ / 0x1f ERP (回显请求 / 应答)。
clnp-segment-length = 整个 PDU (首部和数据) 的长度 (Auto)。
clnp-checksum = 首部的 ISO 8473 Fletcher 校验和 (Auto)。0 = 不使用校验和。
clnp-destination = 目的 NSAP 地址。
clnp-source = 源 NSAP 地址。
clnp-segmentation = 分段部分 (数据单元标识符、分段偏移、总长度)，在设置 SP 时存在。总长度不会被计算。
clnp-options = 选项 (8 位代码 / 8 位长度)，直到长度指示符所指位置：安全、QoS 维护、优先级、源路由、路由记录、填充、丢弃原因。
esis-nlpid = 网络层协议标识符 (0x82 = ES-IS)。
esis-length-indicator = PDU 的长度，单位为八位组 (Auto)。
esis-version = 版本 / 协议 ID 扩展 (1)。
esis-pdu-type = PDU 类型：2 ESH (终端系统 Hello)，4 ISH (中间系统 Hello)，6 RD (重定向)。
esis-holding-time = 保持时间，单位为秒。
esis-checksum = PDU 的 ISO 8473 Fletcher 校验和 (Auto)。0 = 不使用校验和。
esis-number-of-source-addresses = 源地址数量 (Auto)。
esis-source-addresses = 终端系统所服务的 NSAP 地址。
esis-net = 中间系统的网络实体名称 (NET) (重定向：下一跳为终端系统时为空)。
esis-destination = 被重定向的目的 NSAP 地址。
esis-bsnpa-length = 子网地址的长度 (Auto)。
esis-bsnpa = 更优下一跳的子网地址 (例如 MAC 地址)。
esis-options = 选项 (8 位代码 / 8 位长度)，直到长度指示符所指位置：安全、QoS 维护、优先级、地址掩码、SNPA 掩码、建议的 ES 配置定时器。
esis-data = 固定部分之后 PDU 的其余部分 (不透明八位组)。
isis-irpd = 域内路由协议鉴别符 (0x83 = IS-IS)。
isis-header-length = 固定首部的长度，单位为八位组 (每种 PDU 类型为固定值：LAN Hello / LSP 27，P2P Hello 20，CSNP 33，PSNP 17)。
isis-version-ext = 版本 / 协议 ID 扩展 (1)。
isis-id-length = 系统 ID 长度 (0 = 6 个八位组)。
isis-pdu-type = PDU 类型 (15/16 LAN Hello L1/L2，17 P2P Hello，18/20 LSP L1/L2，24/25 CSNP，26/27 PSNP)。
isis-version = 版本 (1)。
isis-max-area-addresses = 最大区域地址数 (0 = 3)。
isis-pdu-length = 整个 PDU 的长度 (Auto)。在 Hello 中，Manual 值会用 Padding TLV 将 PDU 填充到该长度 (例如 MTU 为 1500 时为 1497)。
isis-tlvs = TLV (8 位代码 / 8 位长度) 及其子 TLV。
isis-source-id = 发送方的系统 ID。
isis-holding-time = 保持时间，单位为秒。
isis-priority = 用于 DIS 选举的优先级 (0–127)。
isis-lan-id = LAN ID：DIS 的系统 ID 和伪节点 ID。
isis-circuit-type = 电路类型：1 = L1，2 = L2，3 = L1/L2。
isis-local-circuit-id = 本地电路 ID。
isis-remaining-lifetime = LSP 的剩余生存时间，单位为秒 (不在校验和覆盖范围内)。
isis-lsp-id = LSP ID：系统 ID、伪节点 ID 和分片编号。
isis-sequence-number = LSP 序列号。
isis-lsp-checksum = 从 LSP ID 到 PDU 末尾的 Fletcher 校验和 (Auto)。
isis-partition-repair = P 位：支持分区修复。
isis-attached = ATT 位：连接到其他区域 (default / delay / expense / error 度量)。
isis-overload = OL 位：LSP 数据库过载。
isis-is-type = IS 类型：1 = L1，3 = L1/L2。
isis-snp-source-id = 发送方的系统 ID 和电路 ID (0)。
isis-start-lsp-id = CSNP 覆盖的第一个 LSP ID。
isis-end-lsp-id = CSNP 覆盖的最后一个 LSP ID。
isis-data = 公共首部之后 PDU 的其余部分 (不透明八位组)。

# --- P2 SCTP 字段说明 ---
sctp-src-port = 源端口。
sctp-dst-port = 目的端口。
sctp-verification-tag = 验证标签：对端的初始标签 (携带 INIT 的报文中为 0)。
sctp-checksum = 将本字段置零后对整个 SCTP 报文计算的 CRC32c (Auto)，按最低有效八位组在前的顺序发送。
sctp-chunks = 数据块 (8 位类型，8 位标志，16 位长度 (包含 4 个八位组的首部)，以零填充至 4 个八位组)：DATA、INIT、INIT ACK、SACK、HEARTBEAT、ABORT、SHUTDOWN、ERROR、COOKIE ECHO / ACK、AUTH、I-DATA、ASCONF、RE-CONFIG、FORWARD TSN、PAD。长度、填充和 SACK 计数为 Auto。

# --- P2 LDP / RSVP-TE / BFD 字段说明 ---
ldp-version = LDP 版本 (1)。
ldp-pdu-length = 版本和 PDU 长度字段之后 PDU 的长度 (Auto)。
ldp-lsr-id = LSR ID：LDP 标识符的前四个八位组。
ldp-label-space = 标签空间 ID：LDP 标识符的最后两个八位组 (0 = 平台级)。
ldp-messages = 消息 (带 U 位的 16 位类型、16 位值长度、消息 ID、TLV)：Notification、Hello、Initialization、KeepAlive、Address / Address Withdraw、Label Mapping / Request / Withdraw / Release / Abort Request。TLV：FEC、Address List、Hop Count、Path Vector、Generic Label、Status、Common Hello / Session Parameters、传输地址、Configuration Sequence Number。长度为 Auto。
rsvp-version = RSVP 版本 (1)。
rsvp-flags = 标志 (第 0 位：支持刷新缩减，RFC 2961)。
rsvp-msg-type = 消息类型 (1 Path，2 Resv，3 PathErr，4 ResvErr，5 PathTear，6 ResvTear，7 ResvConf，12 Bundle，13 Ack，15 Srefresh，20 Hello)。
rsvp-checksum = 将本字段置零后对整个消息计算的 Internet 校验和 (Auto)。
rsvp-send-ttl = 发送消息时使用的 IP TTL。
rsvp-reserved = 保留 (0)。
rsvp-length = 包含公共首部在内的整个消息长度，单位为八位组 (Auto)。
rsvp-objects = 对象 (16 位长度 (包含 4 个八位组的首部)、Class-Num、C-Type)：SESSION、RSVP_HOP、TIME_VALUES、ERROR_SPEC、SCOPE、STYLE、FLOWSPEC、FILTER_SPEC、SENDER_TEMPLATE、SENDER_TSPEC、RESV_CONFIRM、LABEL、LABEL_REQUEST、EXPLICIT_ROUTE、RECORD_ROUTE、HELLO、MESSAGE_ID、SESSION_ATTRIBUTE。长度、会话名长度和填充为 Auto。
bfd-version = BFD 版本 (1)。
bfd-diag = 诊断代码：本地会话最近一次状态变化的原因。
bfd-state = 会话状态：0 AdminDown，1 Down，2 Init，3 Up。
bfd-p = P 位：Poll (参数变更，对端以 F 应答)。
bfd-f = F 位：Final (对 Poll 的应答)。
bfd-c = C 位：控制平面独立。
bfd-a = A 位：存在认证 (Auto：存在认证部分时置位)。
bfd-d = D 位：Demand 模式。
bfd-m = M 位：多点 (0)。
bfd-detect-mult = 检测时间倍数。
bfd-length = 控制报文的长度，单位为八位组，包含认证部分 (Auto)。
bfd-my-discriminator = 本端标识符：发送方上该会话唯一的非零值。
bfd-your-discriminator = 对端标识符：对端的标识符 (未知时为 0)。
bfd-desired-min-tx = 期望的最小发送间隔，单位为微秒。
bfd-required-min-rx = 要求的最小接收间隔，单位为微秒。
bfd-required-min-echo-rx = 要求的最小回声接收间隔，单位为微秒 (0 = 不使用回声)。
bfd-authentication = 认证部分 (8 位类型，8 位长度，包含首部)：简单密码、带密钥的 MD5 / SHA1 及其 meticulous 变体。摘要 / 哈希为 Manual。

# --- 生成的文档 (`wpg docs gen`，crates/wpg-cli/src/cmd/docs) ---
docs-gen-no-root = { $path } 不是仓库根目录 (没有 docs/en/src/SUMMARY.md)；请指定 --root。
docs-gen-error = 生成文档失败：{ $error }
docs-gen-stale = 已过期：{ $path }
docs-gen-extra = 不再生成：{ $path }
docs-gen-up-to-date = 生成的文档是最新的。
docs-gen-run-hint = 请运行 `just docs-gen` (或 `wpg docs gen`) 并提交结果。
docs-gen-done = 文档已生成：写入 { $written } 个文件，删除 { $removed } 个。
docgen-yes = 是
docgen-octets = { $octets } 个八位组
docgen-items = { $count } 项
docgen-variant = 变体 { $n }
docgen-col-id = ID
docgen-col-name = 名称
docgen-col-fields = 字段
docgen-col-next-by = 下一层判定依据
docgen-col-notes = 备注
docgen-col-lower = 下层
docgen-col-selector = 选择字段
docgen-col-value = 值
docgen-col-protocol = 协议
docgen-col-note = 备注
docgen-col-level = 级别
docgen-col-rule = 规则
docgen-col-field = 字段
docgen-col-type = 类型
docgen-col-default = 默认值
docgen-col-auto = Auto
docgen-col-description = 说明
docgen-col-bit = 位
docgen-col-mask = 掩码
docgen-col-type-value = 类型
docgen-col-value-fields = 值字段
docgen-col-key = 键
docgen-col-required = 必需
docgen-col-in = 位置
docgen-col-status = 状态
docgen-col-body = 正文
docgen-col-tag = 标签
docgen-col-endpoint = 端点
docgen-col-summary = 摘要
docgen-col-env = 环境变量
docgen-layer-l2 = L2 (数据链路层)
docgen-layer-l3 = L3 (网络层)
docgen-layer-l4 = L4 (传输层)
docgen-layer-l7 = 应用和控制
docgen-layer-payload = 载荷
docgen-proto-title = 协议参考
docgen-proto-intro = 本章根据 `wpg-protocols` 的协议定义 IR (即 `GET /api/protocols` 提供的目录) 生成，因此始终与代码保持一致：共 { $count } 个协议。每个页面按线路顺序列出各字段及其类型、默认值、Auto 规则和说明，以及通向该协议的下层、下一层表、构建后检查的约束和随附的示例模板。手写的 [协议说明](../protocols/overview.md) 介绍了如何到达各协议、哪些字段需要手动设置以及仍存在的近似处理。修改定义后，请用 `just docs-gen` 重新生成。
docgen-proto-layer = 层
docgen-proto-header = 首部
docgen-proto-header-fixed = { $octets } 个八位组 (固定)
docgen-proto-header-variable = 可变长度
docgen-proto-min-frame = 最小帧
docgen-proto-synth = 生成器
docgen-proto-synth-payload = 内置载荷生成器 (下面的字段是参数，而不是线路上的字段)
docgen-proto-pseudo = 伪首部
docgen-proto-pseudo-fields = 向上层校验和提供 { $src } / { $dst }
docgen-proto-notes = 备注
docgen-proto-final-dst = 最终目的地址
docgen-proto-final-dst-text = 当 { $when } 不为 0 时，{ $list } 的元素 0 将替换下层伪首部中的目的地址 (RFC 8200 §8.1)
docgen-proto-inbound = 可从以下协议到达
docgen-proto-inbound-none = 目录中没有协议通向此处：请将其用作第一层或显式堆叠。
docgen-proto-fields = 字段
docgen-proto-params = 参数
docgen-proto-no-fields = 没有字段。
docgen-proto-next = 下一层
docgen-proto-next-intro = 下一层由 { $field } 选择；该字段为 Auto 时，构建器会写入堆叠在其上方的协议所对应的值。
docgen-proto-next-alt = 解码时，如果第一个字段没有匹配的条目，还会查找 { $field }。
docgen-proto-constraints = 约束
docgen-proto-samples = 示例模板
docgen-proto-samples-none = 目前还没有随附的模板使用此协议。
docgen-proto-samples-more = 及其他 { $count } 个
docgen-bind-default = (其他任意值)
docgen-bind-min-len = 至少剩余 { $octets } 个八位组
docgen-bind-reparse = 重新解码此层
docgen-bind-peek = 当 { $field } = { $value } 时，载荷的前 { $bits } 位
docgen-bind-peek-skip = 当 { $field } = { $value } 时，载荷跳过前 { $skip } 位之后的 { $bits } 位
docgen-level-error = 错误
docgen-level-warning = 警告
docgen-level-info = 信息
docgen-c-range = { $field } 在 { $min }..{ $max } 范围内
docgen-c-equals = { $field } = { $value }
docgen-c-not-equals = { $field } ≠ { $value }
docgen-c-min-length = { $span } 至少 { $octets } 个八位组
docgen-c-max-length = { $span } 最多 { $octets } 个八位组
docgen-span-header = 首部
docgen-span-payload = 载荷
docgen-span-total = 首部和载荷
docgen-unit-bytes = 八位组
docgen-unit-words16 = 16 位字
docgen-unit-words32 = 32 位字
docgen-unit-words64 = 64 位字
docgen-auto-length = { $span } 的长度，单位为 { $unit }
docgen-auto-checksum = 覆盖 { $covers } 的 { $algorithm }
docgen-auto-nonzero = 计算结果为 0 时以全 1 发送
docgen-auto-exclude = { $fields } 按零计算
docgen-auto-next = 下一层的协议号 (参见下一层)
docgen-auto-count = { $field } 的项数
docgen-auto-size = { $field } 的大小，单位为八位组
docgen-auto-counter = 每个流的计数器 (目前使用默认值)
docgen-cksum-internet = Internet 校验和
docgen-cksum-internet-pseudo = 含伪首部的 Internet 校验和
docgen-cksum-fletcher = Fletcher 校验和
docgen-cksum-crc32c = CRC-32C
docgen-cksum-crc32 = CRC-32
docgen-cover-header = 首部
docgen-cover-all = 首部及其后的全部内容
docgen-len-from-field = { $expr } 个八位组
docgen-len-count = { $field } 项
docgen-len-until = 直到组内偏移 { $expr } 处
docgen-len-to-end = PDU 的其余部分
docgen-len-to-end-except = PDU 的其余部分，最后 { $octets } 个八位组除外
docgen-len-until-terminator = 直到终止符 { $octets } (构建时追加)
docgen-len-until-tlv = 直到第一个类型为 { $type } 的项 (含该项)，或 PDU 末尾
docgen-tlv-uncounted = 长度之后的前 { $octets } 个值八位组不计入长度。
docgen-list-item = 每一项都是由下列字段组成的组。
docgen-tlv-format = 类型 { $type } 位，长度 { $len } 位。
docgen-tlv-len-first = 长度位于类型之前。
docgen-tlv-len-incl = 长度也计入类型和长度字段。
docgen-tlv-len-excl = 长度仅计入值。
docgen-tlv-unit = 长度单位：{ $unit }。
docgen-tlv-head = 前 { $octets } 个值八位组位于类型和长度之间。
docgen-tlv-align = 每一项都以零填充至 { $align } 个八位组的整数倍 (填充不计入长度)
docgen-tlv-unpadded-last = 最后一项不填充
docgen-tlv-long = 类型位 { $flag } 选择 { $len } 位长度。
docgen-tlv-fill = { $field } 的 Manual 值会用 { $variant } 项将该区域填充到相应大小。
docgen-tlv-unknown-raw = 未知类型保留为原始值八位组。
docgen-tlv-unknown-stop = 遇到未知类型时结束按类型解码 (其余部分成为一个原始项)。
docgen-tlv-type-only = (仅类型，没有长度和值)
docgen-tlv-when-prefix = 当值以 { $octets } 开头时
docgen-tlv-when-len = 当值为 { $octets } 个八位组时
docgen-notes-heading = 生成的参考
docgen-notes-intro = 本页各协议的字段参考 (根据定义生成)：
docgen-cli-title = CLI 参考
docgen-cli-intro = `wpg` 二进制文件既运行服务器 (`wpg serve`)，也提供无界面工具，因此同样适用于 CI 和 ssh 会话。例如，`wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` 会发送一个模板。本页根据命令定义生成：每个代码块都是当前构建的 `--help` 原样输出。运行时消息遵循 `LANG` / `LC_ALL`；`--help` 文本本身为英文。
docgen-cli-commands = 命令
docgen-cli-note-send = 发送模板。`--plan` 用于选择预先展开所有帧 (`prerender`) 还是边发送边构建 (`onthefly`)。`--json` 会在 stdout 上输出最终报告。
docgen-cli-note-render = 将模板的帧写入 pcapng 文件，而不实际发送。
docgen-cli-note-import = 将捕获 (或粘贴的十六进制) 转换为模板；参见 [pcap 重放教程](../tutorials/pcap-replay.md)。
docgen-cli-note-bench = 以不限速率发送合成帧 (EtherType 0x88b5)，或使用 `--template` 发送模板。基准数据参见 `docs/perf.md`。`--json` 输出字段：backend、detail、iface、frame_size、batch_size、packets、dropped、pps、bps (L2)、l1_bps、elapsed。
docgen-cli-note-ifaces = 列出接口及其 MAC、地址、MTU、链路状态和驱动。
docgen-cli-note-doctor = 运行 [权限](../getting-started/permissions.md) 中所述的环境检查；有检查失败时退出状态为非零。
docgen-cli-note-template = 模板工具。`lint` 在出错时以状态 1 退出；`export` 和 `import` 用于将模板打包为 `.wpgbundle` 归档或从中解包。
docgen-cli-note-config = `wpg.toml` 工具；参见 [配置参考](config.md)。
docgen-api-title = API 参考
docgen-api-intro = wpg-server 在 `/api` 下提供 REST (JSON) API，并通过 WebSocket (`GET /api/ws`) 推送实时统计、任务状态变化和日志。本页根据服务器的 OpenAPI 文档 (utoipa) 生成。运行中的服务器会在 `/api/openapi.json` 提供同一文档，并在 `/api/docs/` 提供 Swagger UI；副本保存在 [`docs/schema/openapi.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/openapi.json) 中。使用 `POST /api/auth/login` 登录 (首次运行时使用 `POST /api/auth/setup`)；之后 `wpg_session` cookie 会对每个请求进行认证，POST / PUT / DELETE 还必须在 `X-CSRF-Token` 首部中发送 `GET /api/auth/status` 返回的 CSRF 令牌。错误以 JSON 对象 `error` 返回，包含 `code`、`message` (使用 `Accept-Language` 指定的语言) 和 `details`。下面的端点和模式说明来自代码。
docgen-api-endpoints = 端点
docgen-api-params = 参数
docgen-api-body = 请求正文
docgen-api-responses = 响应
docgen-api-schemas = 模式
docgen-api-schemas-intro = 上文引用的请求和响应正文。
docgen-api-tag-auth = 登录、退出登录和首次运行设置。
docgen-api-tag-system = 健康状态、版本、接口、自检和发送后端。
docgen-api-tag-protocols = 协议目录 (参见 [协议参考](protocols.md))。
docgen-api-tag-packets = 数据包的渲染、解码和检查。
docgen-api-tag-templates = 模板库。
docgen-api-tag-jobs = 发送任务和统计。
docgen-api-tag-events = WebSocket 事件流。
docgen-config-title = 配置
docgen-config-intro = 服务器从工作目录读取 `wpg.toml`，或读取通过 `--config` / `WPG_CONFIG` 指定的文件。优先级从低到高依次为：内置默认值、配置文件、环境变量 `WPG_<SECTION>__<KEY>` (`__` 分隔嵌套层级，例如 `WPG_SERVER__BIND=0.0.0.0:8080` 或 `WPG_TX__ALLOWED_IFACES='["eth1"]'`)，以及 `wpg serve --bind` 等命令行选项。未知的键会被视为错误。`wpg config init` 会写入下面带注释的示例，`wpg config show` 输出生效的配置，`wpg config validate` 进行检查，`wpg config set-password` 保存管理员密码哈希。管理员密码本身不是配置键：`wpg serve --password`、`--password-file` 和 `WPG_PASSWORD` 优先于 `auth.password_hash` 和 `auth.password_file`。本页根据配置类型生成。
docgen-config-keys = 键
docgen-config-top-level = 顶层
docgen-config-default-machine = 操作系统的数据目录 (Linux：`~/.local/share/wpg`)
docgen-config-example = 示例 (`wpg config init`)
docgen-config-example-intro = 每个生效的行都等于内置默认值，因此在编辑之前该文件不会改变任何设置；带注释的行展示了可选的键。
docgen-config-data-dir = 服务器状态目录：用户模板、收藏、SQLite 数据库 (用户、会话、任务历史)、试运行捕获以及自签名 TLS 证书。
docgen-config-server = HTTP 监听器。
docgen-config-server-bind = 监听地址。未设置密码 (且未在初始设置页面创建用户) 时，只接受环回地址 (`--allow-insecure` 可覆盖此限制)。
docgen-config-server-tls = 用于提供 HTTPS 的 TLS 证书链和私钥 (PEM)；优先于 `self_signed_tls`。
docgen-config-server-tls-cert = PEM 证书链。
docgen-config-server-tls-key = PEM 私钥。
docgen-config-server-self-signed-tls = 使用生成到 `<data_dir>/tls/` 的自签名证书提供 HTTPS (后续启动时复用)；`wpg serve --tls` 会设置此项。
docgen-config-server-base-url = 位于反向代理之后的服务器的公开 URL (`https://wpg.example.net`)，用于生成绝对链接。
docgen-config-auth = 身份验证。
docgen-config-auth-password-hash = 管理员密码的 argon2id PHC 字符串；由 `wpg config set-password` 写入。
docgen-config-auth-password-file = 保存管理员密码的文件：argon2 PHC 字符串，或在启动时进行哈希的明文。
docgen-config-auth-session-ttl-secs = 会话有效期，单位为秒 (滑动过期：每次请求都会延长)。
docgen-config-templates = 模板库。
docgen-config-templates-library-paths = 只读模板库 (例如 Git 检出目录)，会递归扫描 `*.wpg.yaml` / `*.wpg.json`，并在文件变化时重新加载。
docgen-config-templates-user-dir = 用于保存在 GUI 中创建或导入的模板的可写目录；默认为 `<data_dir>/templates`。
docgen-config-protocols = 用户自定义协议。
docgen-config-protocols-dirs = 存放用户自定义协议文件 (`*.yaml`: 新的协议定义以及挂在现有协议下的绑定) 的目录，启动时按文件名顺序加载。可用 `wpg serve --protocols-dir` 追加。
docgen-config-tx = 发送策略和默认值。
docgen-config-tx-allowed-ifaces = 任务可以用于发送的接口；为空 = 全部。`null` 后端和试运行不受限制。
docgen-config-tx-max-rate = 任务可请求的最高速率 (`100kpps`、`1Gbps`)；未指定速率的任务按此速率发送。
docgen-config-tx-default-backend = 任务未指定后端时使用的后端 (`null`、`pcapfile`、`afpacket`、`pcap`、`afxdp`、`dpdk`)；未设置时在 Linux 上为 `afpacket`，否则为 `pcap`，再否则为 `null`。
docgen-config-tx-dpdk-eal-args = `dpdk` 后端的 DPDK EAL 参数，例如 `["-a", "0000:00:04.0"]`。若未指定，wpg 会添加 `-l <第一个 CPU>` 和 `--in-memory`。EAL 在每个进程中随第一个 `dpdk` 任务启动一次；修改这些参数需要重启。
docgen-config-log = 日志。
docgen-config-log-level = 过滤指令 (`RUST_LOG` 会覆盖它们)，例如 `info` 或 `wpg_tx=debug,info`。
docgen-config-log-format = 行格式：`pretty` (人类可读) 或 `json` (每行一个对象)。
docgen-config-log-file = 同时写入此文件 (会创建其所在目录)。
docgen-config-log-rotation = 文件轮转方式：`never`、`hourly` 或 `daily`。
docgen-config-log-max-files = 最多保留的轮转文件数 (删除最旧的文件)。
docgen-config-log-ansi = 控制台输出为终端时使用彩色显示。
docgen-schema-heading = 键参考 (自动生成)
docgen-schema-intro = 根据模板格式的 JSON Schema (`wpg template schema`) 生成；说明文字来自代码的文档注释 (英文)。`Template` 为文档根。
# --- P2 PTP / ESMC / CFM / R-APS / 802.3ah OAM 字段说明 ---
ptp-major-sdo-id = majorSdoId (IEEE 1588-2008 中为 transportSpecific)：默认配置文件为 0，IEEE 802.1AS (gPTP) 为 1。
ptp-message-type = 消息类型：0 Sync，1 Delay_Req，2 Pdelay_Req，3 Pdelay_Resp，8 Follow_Up，9 Delay_Resp，10 Pdelay_Resp_Follow_Up，11 Announce，12 Signaling，13 Management。
ptp-minor-version = minorVersionPTP (IEEE 1588-2008 中为 0，IEEE 1588-2019 中为 1)。
ptp-version = versionPTP (2)。
ptp-message-length = 包含 TLV 在内的整个消息长度，单位为八位组 (Auto)。
ptp-domain-number = 域编号。
ptp-minor-sdo-id = minorSdoId (IEEE 1588-2008 中为保留，0)。
ptp-flags = 标志字段：第一个八位组为 alternateMaster、twoStep、unicast、配置文件专用位和安全位；第二个八位组为 leap61、leap59、currentUtcOffsetValid、ptpTimescale、timeTraceable、frequencyTraceable 和 synchronizationUncertain。
ptp-correction-field = 修正值，以纳秒乘以 2^16 表示 (有符号)。
ptp-message-type-specific = 消息类型专用字段 (IEEE 1588-2008 中为保留，0)。
ptp-clock-identity = 源端口的时钟标识 (EUI-64，例如在中间插入 ff:fe 的 MAC 地址)。
ptp-port-number = 源端口的端口号。
ptp-sequence-id = 序列 ID；Follow_Up / Delay_Resp / Pdelay_Resp 会复制其所应答消息的序列 ID。
ptp-control-field = 控制字段 (与 IEEE 1588 版本 1 兼容)：0 Sync，1 Delay_Req，2 Follow_Up，3 Delay_Resp，4 Management，5 其他消息。
ptp-log-message-interval = 消息间隔 (秒) 的以 2 为底的对数 (有符号；不适用时为 0x7f)。
ptp-timestamp-seconds = 时间戳：秒 (48 位)。
ptp-timestamp-nanoseconds = 时间戳：纳秒 (32 位)。
ptp-port-identity-clock = 端口标识：时钟标识 (全 1 = 所有时钟)。
ptp-port-identity-port = 端口标识：端口号 (0xffff = 所有端口)。
ptp-current-utc-offset = 当前 UTC 偏移，单位为秒 (TAI - UTC)。
ptp-grandmaster-priority = 主时钟 (Grandmaster) 优先级 (最佳主时钟算法，值越小越优先)。
ptp-grandmaster-clock-class = 主时钟等级 (6 = 锁定到主参考源，248 = 默认，255 = 仅从时钟)。
ptp-grandmaster-clock-accuracy = 主时钟精度 (0x20 = 25 ns 以内 ... 0xfe = 未知)。
ptp-grandmaster-variance = 主时钟的 offsetScaledLogVariance。
ptp-grandmaster-identity = 主时钟的时钟标识。
ptp-steps-removed = 主时钟与发送方之间的边界时钟数量。
ptp-time-source = 主时钟的时间源：原子钟、GPS、地面无线电、PTP、NTP、手动设置、其他、内部振荡器。
ptp-starting-boundary-hops = Management 消息的起始边界跳数。
ptp-boundary-hops = 剩余的边界跳数。
ptp-action = Management 动作：GET、SET、RESPONSE、COMMAND、ACKNOWLEDGE。
ptp-tlvs = TLV (16 位类型，16 位值长度，Auto)：Management、Management Error Status、Organization Extension、单播协商 (Request / Grant / Cancel / Acknowledge Cancel)、Path Trace、Alternate Time Offset Indicator。到消息长度处结束。
ptp-data = 公共首部之后消息的其余部分，以八位组表示 (直到消息长度)。
esmc-subtype = 慢协议子类型 (0x0a = 组织专用)。
esmc-itu-oui = ITU-T OUI (00-19-a7)。
esmc-itu-subtype = ITU-T 子类型 (1 = ESMC)。
esmc-version = ESMC 版本 (1)。
esmc-event-flag = 事件标志：QL 变化后立即发送的事件 PDU 为 1，每秒一次的信息 PDU 为 0。
esmc-tlvs = TLV (8 位类型，16 位长度 (包含 3 个八位组的首部)，Auto)：带 SSM 代码的 QL TLV、带增强 SSM 代码的 Extended QL TLV、SyncE 时钟标识和级联时钟计数。其后的八位组 (填充) 保留为一个原始项。
cfm-md-level = 维护域 / MEG 级别 (0-7)。
cfm-version = CFM 版本 (0；G.8032v2 的 R-APS 为 1)。
cfm-opcode = OpCode：1 CCM，2 LBR，3 LBM，4 LTR，5 LTM，33 AIS，35 LCK，37 TST，39 Linear APS，40 R-APS，41 MCC，42 LMR，43 LMM，45 1DM，46 DMR，47 DMM，52 CSF，53 1SL，54 SLR，55 SLM ...
cfm-flags = 标志 (取决于 OpCode)。
cfm-first-tlv-offset = 第一个 TLV 相对于本字段末尾的偏移 (每个 OpCode 为固定值)。
cfm-rdi = RDI：远端缺陷指示。
cfm-period = 发送周期：1 = 3.33 ms，2 = 10 ms，3 = 100 ms，4 = 1 s，5 = 10 s，6 = 1 min，7 = 10 min。
cfm-sequence-number = 序列号。
cfm-mep-id = MEP ID (1-8191)。
cfm-maid = MAID / MEG ID，48 个八位组：MD 名称格式和名称、短 MA 名称格式、长度和名称，以零填充 (Y.1731：无 MD 名称，格式 32 基于 ICC，长度 13)。
cfm-frame-counter = 帧计数器 (丢包测量)。
cfm-transaction-id = 环回 / 链路跟踪事务标识符。
cfm-use-fdb-only = UseFDBonly：仅由转发数据库决定出端口。
cfm-fwd-yes = FwdYes：LTM 已被中继。
cfm-terminal-mep = TerminalMEP：应答来自目标 MEP。
cfm-ttl = 链路跟踪 TTL。
cfm-relay-action = 中继动作：1 RlyHit，2 RlyFDB，3 RlyMPDB。
cfm-original-mac = 发起链路跟踪的 MEP 的 MAC 地址。
cfm-target-mac = 链路跟踪的目标 MAC 地址。
cfm-csf-type = 客户端信号失效类型：0 LOS，1 FDI / AIS，2 RDI，3 DCI。
cfm-timestamp = IEEE 1588 格式的时间戳 (32 位秒和纳秒)。
cfm-source-mep-id = 源 MEP ID。
cfm-responder-mep-id = 响应方 MEP ID (SLM 中为 0)。
cfm-test-id = 合成丢包测量的测试 ID。
cfm-tlvs = TLV (8 位类型，16 位值长度，Auto)：Sender ID、Port Status、Data、Interface Status、Reply Ingress / Egress、LTM / LTR Egress Identifier、Organization-Specific、Test，以 End TLV (类型 0) 结束。
cfm-data = 第一个 TLV 偏移之后 PDU 的其余部分，以八位组表示 (OpCode 专用字段和 TLV)。
raps-request-state = 请求 / 状态：0000 NR，0111 MS，1011 SF，1101 FS，1110 Event。
raps-sub-code = 子代码 (0000 = 与 Event 请求一起使用的刷新请求)。
raps-rb = RB：RPL 已阻塞。
raps-dnf = DNF：不刷新转发数据库。
raps-bpr = BPR：阻塞端口引用 (0 = 环链路 0，1 = 环链路 1)。
raps-node-id = 节点 ID：发送方环节点的 MAC 地址。
efm-oam-subtype = 慢协议子类型 (3 = OAM)。
efm-oam-flags = 标志：链路故障、Dying Gasp、严重事件、本端 / 远端评估中及稳定。
efm-oam-code = OAMPDU 代码：0x00 Information，0x01 Event Notification，0x02 Variable Request，0x03 Variable Response，0x04 Loopback Control，0xfe Organization Specific。
efm-oam-information-tlvs = 信息 TLV (8 位类型，8 位长度 (包含首部)，Auto)：Local / Remote Information、Organization Specific，以 End TLV (类型 0) 结束。
efm-oam-sequence-number = 事件通知序列号。
efm-oam-event-tlvs = 链路事件 TLV (8 位类型，8 位长度 (包含首部)，Auto)：错误符号周期、错误帧、错误帧周期、错误帧秒摘要、Organization Specific，以 End TLV (类型 0) 结束。
efm-oam-variable-descriptors = 以八位组表示的变量描述符：每项为 branch (1 个八位组) 和 leaf (2 个八位组)，以零 branch 结束。
efm-oam-variable-containers = 以八位组表示的变量容器：每项为 branch、leaf、宽度和值，以零 branch 结束。
efm-oam-loopback-command = 远端环回命令：1 启用，2 禁用。
efm-oam-oui = 组织唯一标识符 (OUI)。
efm-oam-data = OAMPDU 的其余部分，以八位组表示。

# --- P2 MACsec / GTPv2-C / Diameter / SIP / RTP 字段说明 ---
macsec-v = TCI V 位：版本 (0)。
macsec-es = TCI ES 位：终端站 (SCI 为源 MAC 地址加端口 1)。
macsec-sc = TCI SC 位：存在显式 SCI (Auto：指定 SCI 时置位)。
macsec-scb = TCI SCB 位：EPON 单副本广播。
macsec-e = TCI E 位：安全数据已加密。
macsec-c = TCI C 位：用户数据已被更改 (对于默认密码套件始终与 E 一起设置)。
macsec-an = 关联编号 (0-3)：通道内的安全关联。
macsec-sl = 短长度：安全数据长度小于 48 个八位组时为该长度，否则为 0 (Manual)。
macsec-pn = 报文编号 (32 位；使用 XPN 时为低半部分)。
macsec-sci = 安全通道标识符：系统标识符 (MAC 地址) 和端口标识符；设置 SC 时存在。
macsec-secure-data = 安全数据：以不透明八位组表示的加密 (或仅完整性保护) 用户数据；不会进行任何加密。
macsec-icv = 完整性校验值 (16 个八位组)，不透明：不会被计算。
gtpv2-version = GTP 版本 (2)。
gtpv2-p = P 标志：后面跟有捎带消息 (下一个 GTPv2-C 层)。
gtpv2-t = T 标志：存在 TEID 字段 (Auto：指定 TEID 时置位)。
gtpv2-mp = MP 标志：存在消息优先级。
gtpv2-message-type = 消息类型 (Echo、Create / Modify / Delete Session、Create / Update / Delete Bearer、...)。
gtpv2-length = 消息长度，单位为八位组，不含前 4 个八位组 (Auto)。
gtpv2-teid = 接收方的隧道端点标识符 (Echo 消息中不存在；初始请求中为 0)。
gtpv2-sequence = 用于匹配响应与请求的序列号 (24 位)。
gtpv2-message-priority = 消息优先级 (0 最高)，在设置 MP 时使用。
gtpv2-ies = 信息元素 (8 位类型，16 位数据长度，备用 / 实例八位组)：IMSI、Cause、Recovery、APN、AMBR、EBI、IP Address、MEI、MSISDN、PAA、Bearer QoS、RAT Type、Serving Network、F-TEID、Bearer Context (分组)、PDN Type、... 长度为 Auto。
diameter-version = Diameter 版本 (1)。
diameter-length = 包含首部和 AVP 填充在内的消息长度，单位为八位组 (Auto)。
diameter-r = R 标志：请求 (0 = 应答)。
diameter-p = P 标志：可代理。
diameter-e = E 标志：应答中携带协议错误。
diameter-t = T 标志：可能是重传的消息。
diameter-command-code = 命令代码 (Capabilities-Exchange、Device-Watchdog、Credit-Control、...)。
diameter-application-id = Application-ID (0 = Diameter 公共消息)。
diameter-hop-by-hop = 在一条连接上匹配应答与请求的逐跳标识符。
diameter-end-to-end = 用于检测重复请求的端到端标识符。
diameter-avps = AVP (32 位代码，V / M / P 标志，不含填充的 24 位长度，可选的 Vendor-ID，数据填充至 4 个八位组)：基础协议和 Credit-Control 的 AVP，分组 AVP 支持一层嵌套。长度、填充和 V 标志为 Auto。
sip-start-line = 起始行：请求行 (方法、Request-URI、SIP/2.0) 或状态行 (SIP/2.0、代码、原因短语)。CRLF 会自动添加。
sip-headers = 首部行 (Name: value)，每个元素一行；CRLF 及其后的空行会自动添加。Content-Length 为 Manual。
sip-body = 消息正文 (SDP offer / answer、...)。
rtp-version = RTP 版本 (2)。
rtp-padding = P 位：载荷末尾有填充八位组 (最后一个八位组为填充数量，Manual)。
rtp-x = X 位：CSRC 列表之后跟有首部扩展 (Auto)。
rtp-cc = CSRC 计数 (Auto)。
rtp-marker = 标记位 (由配置文件定义，例如话音突发的开始)。
rtp-payload-type = 载荷类型 (0 PCMU，8 PCMA，9 G.722，18 G.729，96-127 动态)。
rtp-sequence = 序列号，每个 RTP 报文加一。
rtp-timestamp = 时间戳：第一个载荷八位组的采样时刻。
rtp-ssrc = 同步源标识符。
rtp-csrc = 贡献源标识符 (最多 15 个)。
rtp-extension = 首部扩展：由配置文件定义的 16 位 (0xBEDE：RFC 8285 单字节元素)、以 32 位字为单位的长度 (Auto) 以及扩展字。
rtcp-version = RTCP 版本 (2)。
rtcp-padding = P 位：报文末尾有填充八位组 (Manual)。
rtcp-count = 未按类型解析的报文类型的计数 / 格式字段 (5 位)。
rtcp-report-count = 接收报告计数 (Auto)。
rtcp-source-count = 源计数：SDES 块或 BYE 标识符 (Auto)。
rtcp-subtype = 由应用定义的子类型。
rtcp-packet-type = 报文类型 (200 SR，201 RR，202 SDES，203 BYE，204 APP，205 RTPFB，206 PSFB，207 XR)。
rtcp-length = 以 32 位字为单位的报文长度减一 (Auto)。
rtcp-body = 公共首部之后的报文主体，以八位组表示。
rtcp-ssrc = 此报文发送方的 SSRC。
rtcp-ntp-timestamp = NTP 时间戳 (高位字；下一个字段为小数部分)。
rtcp-rtp-timestamp = 与 NTP 时间戳对应的 RTP 时间戳。
rtcp-packet-count = 发送方的报文计数。
rtcp-octet-count = 发送方的载荷八位组计数。
rtcp-report-blocks = 接收报告块：SSRC、丢包率、累计丢包数、扩展的最高序列号、抖动、LSR、DLSR。
rtcp-extension = 直到报文长度为止的配置文件专用扩展八位组。
rtcp-chunks = SDES 块：SSRC / CSRC 及其条目 (类型、长度 (Auto)、文本)，以空八位组结束并填充至 32 位。
rtcp-ssrcs = 离开会话的 SSRC / CSRC 标识符。
rtcp-reason = 可选的离开原因：长度 (Auto)、文本、填充至 32 位。
rtcp-app-name = 名称 (4 个 ASCII 字符)。
rtcp-app-data = 与应用相关的数据 (4 个八位组的整数倍)。
