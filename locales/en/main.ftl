# English (en) — base locale. Every message here must also exist in locales/ja/main.ftl
# (enforced by the wpg-i18n unit tests).

app-name = Web Packet Generator
app-tagline = High-performance packet generator for network testing, driven from your browser.

## Server / API
health-ok = Service is healthy
server-listening = Listening on { $addr }
server-placeholder-ui = The web UI has not been built yet. Run `trunk build` in crates/wpg-web.

## Errors
error-not-implemented = `{ $command }` is not implemented yet.
error-not-found = Not found
error-internal = Internal server error

## UI
ui-language = Language
ui-protocols = Protocols

## Diagnostics (wpg-core `diag::keys`; argument names are part of the contract)
diag-field-out-of-range = Field `{ $field }` = { $value } is outside the range { $min }..{ $max }.
diag-field-not-equal = Field `{ $field }` = { $value } differs from the expected value { $expected }.
diag-field-forbidden-value = Field `{ $field }` has a discouraged value: { $value }.
diag-length-too-short = `{ $span }` is { $len } bytes long, at least { $min } required.
diag-length-too-long = `{ $span }` is { $len } bytes long, at most { $max } allowed.
diag-length-mismatch = Length field `{ $field }` = { $value } differs from the computed length { $expected }.
diag-count-mismatch = Count field `{ $field }` = { $value } differs from the actual number of elements { $expected }.
diag-fill-inexact = `{ $field }` cannot be reached exactly with filler items: { $len } octet(s) are missing.
diag-checksum-mismatch = Checksum field `{ $field }` = { $value } differs from the computed checksum { $expected }.
diag-next-protocol-mismatch = Field `{ $field }` = { $value } does not announce the next layer `{ $next }` (expected { $expected }).
diag-auto-overflow = Automatic value { $value } of `{ $field }` does not fit in { $bits } bits.
diag-length-not-aligned = Length { $len } of `{ $field }` is not a multiple of { $unit } bytes.
diag-no-pseudo-header = Checksum `{ $field }` needs a pseudo header but there is no IP layer below.
diag-not-implemented = `{ $field }`: { $what } is not implemented yet.
diag-truncated = The layer was truncated to { $len } bytes on purpose.
diag-short-header = Input ends inside the { $protocol } header at offset { $offset }.
diag-length-exceeds-data = Length field `{ $field }` = { $len } exceeds the { $available } bytes available.
diag-trailing-bytes = { $len } bytes remain after the end of the packet.
diag-unsupported-field = The parser cannot decode field `{ $field }` of { $protocol } yet.

## Protocol field descriptions (wpg-protocols `desc = "..."`)
arp-hlen = Hardware address length in bytes (6 for Ethernet).
arp-htype = Hardware type (1 = Ethernet).
arp-oper = Operation (1 = request, 2 = reply).
arp-plen = Protocol address length in bytes (4 for IPv4).
arp-ptype = Protocol type (EtherType, 0x0800 = IPv4).
arp-sha = Sender hardware address.
arp-spa = Sender protocol address.
arp-tha = Target hardware address.
arp-tpa = Target protocol address.
icmp-code = Message code, meaning depends on the type.
icmp-identifier = Identifier used to match echo requests and replies.
icmp-rest = Type-specific 4-byte "rest of header" field.
icmp-sequence = Sequence number used to match echo requests and replies.
icmp-type = Message type.
ipv6-fragment-offset = Offset of this fragment in 8-byte units.
ipv6-more-fragments = More-fragments flag (1 = more fragments follow).
ipv6-options = Raw TLV options, padded to a multiple of 8 bytes.
ipv6-routing-data = Type-specific routing data.
ipv6-routing-type = Routing header type (e.g. 2 = Mobile IPv6, 4 = SRH).
ipv6-segments-left = Number of route segments remaining.
ndp-destination = Destination address of a redirect.
ndp-managed = Managed address configuration flag (M).
ndp-options = Raw NDP options (TLV).
ndp-other = Other configuration flag (O).
ndp-override-flag = Override flag (O).
ndp-router-flag = Router flag (R).
ndp-solicited-flag = Solicited flag (S).
ndp-target = Target address.

## wpg ifaces
ifaces-col-name = NAME
ifaces-col-index = IDX
ifaces-col-mac = MAC
ifaces-col-ipv4 = IPV4
ifaces-col-ipv6 = IPV6
ifaces-col-mtu = MTU
ifaces-col-state = STATE
ifaces-col-speed = SPEED
ifaces-col-driver = DRIVER
ifaces-state-up = up
ifaces-state-down = down
ifaces-state-no-carrier = no carrier
ifaces-state-loopback = loopback
ifaces-none = No network interfaces found.
ifaces-error = Could not list network interfaces: { $error }

## wpg doctor
doctor-title = wpg doctor: environment check
doctor-summary = { $ok } ok, { $warn } warning(s), { $fail } failure(s)
doctor-skip = Not applicable on this operating system
doctor-caps-ok = Raw sockets are available
doctor-caps-missing = Missing privileges for raw packet transmission (CAP_NET_RAW / root / Administrator)
doctor-fix-caps = Grant the privilege: { $hint }
doctor-iface-ok = At least one usable network interface is up
doctor-iface-none = No non-loopback interface is up with carrier
doctor-iface-error = Could not enumerate network interfaces
doctor-fix-iface = Inspect the interfaces: { $hint }
doctor-af-xdp-ok = Kernel supports AF_XDP (5.10 or newer)
doctor-af-xdp-old = Kernel is too old for AF_XDP (5.10 or newer required); the afpacket backend still works
doctor-tpacket-v3-ok = Kernel supports TPACKET_V3
doctor-tpacket-v3-old = Kernel does not support TPACKET_V3; afpacket falls back to sendmmsg
doctor-hugepages-ok = Hugepages are configured
doctor-hugepages-none = No hugepages configured (only needed for the DPDK backend)
doctor-fix-hugepages = Reserve hugepages and bind the NIC to DPDK: { $hint }
doctor-npcap-ok = Npcap is installed (WinPcap-compatible mode)
doctor-npcap-missing = Npcap is not installed; it is required to transmit on Windows
doctor-npcap-not-compat = Npcap is installed but not in WinPcap API-compatible mode, so wpcap.dll may not be found
doctor-fix-npcap = Install Npcap: { $hint }
doctor-fix-npcap-compat = { $hint }
doctor-pcaplib-ok = libpcap is available
doctor-pcaplib-missing = libpcap was not found; the pcap backend cannot be used
doctor-fix-pcaplib = Install libpcap: { $hint }

## wpg template lint (wpg-template `LintIssue` keys)
lint-level-error = error
lint-level-warning = warning
lint-schema-version = Unsupported schema version { $value } (supported: { $expected }).
lint-missing-name = `meta.name` is empty.
lint-no-layers = The template has no layers.
lint-unknown-protocol = Unknown protocol `{ $protocol }`.
lint-unknown-field = Protocol `{ $protocol }` has no field `{ $field }`.
lint-invalid-value = Invalid value for `{ $field }`: { $value }
lint-invalid-generator = Invalid generator for `{ $field }`: { $value }
lint-unknown-variable = Unknown variable `{ $value }`.
lint-missing-variable = Variable `{ $value }` has no default and no value was given.
lint-unused-variable = Variable `{ $value }` is never used.
lint-duplicate-variable = Variable `{ $value }` is declared more than once.
lint-var-syntax = Unterminated variable placeholder in `{ $value }`.
lint-override-unknown = Unknown variable override `{ $value }`.
lint-field-and-generator = `{ $field }` has both a fixed value and a generator; the generator wins.
lint-next-protocol-manual = `{ $next }` is not in the binding table of `{ $protocol }`; `{ $field }` will be Manual.
lint-unknown-order = `{ $value }` in the stream order has no generator.
lint-invalid-stream = Invalid stream setting: { $value }
lint-build-error = Cannot build the packet: { $value }

## wpg template / wpg render
template-lint-ok = { $value }: no issues
template-lint-load-error = { $value }: { $error }
template-lint-summary = { $fail } error(s), { $warn } warning(s) in { $ok } file(s)
template-list-empty = No templates found in { $value }.
template-show-hex = Hex dump ({ $len } bytes)
template-show-layout = Layout
render-done = Wrote { $len } packets to { $value }.
render-unbounded = The stream is unbounded; pass --count N.
cli-bad-var = Invalid --var `{ $value }`; expected NAME=VALUE.

## wpg send / wpg bench (progress, summary, errors)
tx-progress = [{ $t } s] { $packets } pkts  { $pps }pps  { $bps }bps  dropped { $dropped }
tx-progress-rate = rate { $ratio }%
tx-progress-paused = (paused)
tx-progress-scheduled = (scheduled, starts in { $secs } s)
tx-interrupted = Interrupted, stopping...
tx-ctrlc-warning = Warning: cannot install the Ctrl-C handler: { $error }
tx-open-failed = Cannot open { $backend } on `{ $iface }`: { $error }
tx-start-failed = Cannot start the job: { $error }
tx-available-backends = Available backends: { $value }
tx-iface-required = --iface is required for the { $backend } backend.
tx-iface-required-pcapfile = --iface <output path> is required for the pcapfile backend.
tx-bad-workers = --workers must be at least 1.
tx-workers-unsupported = The { $backend } backend supports a single worker; use --workers 1.
tx-pin-cores-short = Warning: --pin-cores lists { $count } core(s) for { $workers } workers; the rest are not pinned.
tx-json-error = Cannot write JSON: { $error }
send-bad-rate = Invalid --rate: { $error }
send-bad-duration = Invalid --duration: { $error }
send-bad-ramp-up = Invalid --ramp-up: { $error }
send-bad-start-at = Invalid --start-at: { $error }
send-link-speed-unknown = Cannot determine the link speed of `{ $iface }` for `{ $value }%`; give an absolute rate such as `1Gbps`.
send-bad-plan = --plan must be auto, prerender or onthefly.
send-stream-error = Stream error: { $error }
send-render-error = Render error: { $error }
send-starting = Sending on { $iface } via { $backend }{ $detail }: rate { $rate }, burst { $burst }, { $limit }, { $workers } worker(s), plan { $strategy } ({ $reason })
send-limit-count = { $count } packets
send-limit-count-or-duration = { $count } packets or { $duration }
send-limit-duration = for { $duration }
send-limit-forever = until Ctrl-C
send-ramp-up = Ramp-up: 0 to { $rate } over { $duration }
send-scheduled = Scheduled start at { $time } (in { $secs } s); Ctrl-C cancels.
summary-state = state      { $value }
summary-state-finished = finished ({ $reason })
summary-state-failed = failed: { $error }
summary-reason-count-reached = count reached
summary-reason-duration-elapsed = duration elapsed
summary-reason-source-exhausted = stream exhausted
summary-reason-stopped = stopped
summary-backend = backend    { $backend }{ $detail }
summary-workers = workers    { $workers }
summary-packets = packets    { $value }
summary-bytes = bytes      { $value }
summary-dropped = dropped    { $value }
summary-elapsed = elapsed    { $secs } s
summary-average = average    { $pps }pps  { $bps }bps (L2)  { $l1 }bps (L1)
summary-accuracy = accuracy   target { $target }{ $unit }  achieved { $ratio }%  max jitter { $max }  mean jitter { $mean }
summary-stream = stream { $index }   { $packets } pkts  { $bytes } bytes
summary-worker = worker { $index }   { $packets } pkts  { $bytes } bytes{ $core }
summary-worker-core = , core { $core }
summary-warning = warning    { $error }
bench-iface = iface      { $iface }
bench-frame = frame      { $len } B (+4 FCS)  batch { $batch }
bench-template = template   { $value } (on the fly, { $mode })
bench-mode-patch = patched in place
bench-mode-rebuild = full rebuild per frame
bench-rate = rate       { $pps }pps
bench-bits = { $bps }bps (L2)  { $l1 }bps (L1)
bench-bad-duration = --duration must be a non-negative number of seconds.
bench-failed = Benchmark failed: { $error }
import-done = Imported { $len } packet(s) into { $count } template(s): { $value }
import-read-error = { $value }: { $error }
import-write-error = { $value }: { $error }
import-note = { $value }
import-note-packet = packet { $index }: { $value }
import-packet-problems = packet { $index } has { $count } parse problem(s)
import-packet-skipped = packet { $index } could not be decoded and was skipped
import-not-reproducible = The imported template does not rebuild the captured octets; the frame was stored as raw data.
import-mixed-stacks = { $count } packet(s) have a different layer stack than the first one; no generators were inferred.
import-generator-approximate = { $value } only approximates the captured values.
import-generators-dropped = The inferred generators produced an invalid stream and were dropped.
template-export-none = No templates were given to export.
template-export-not-found = Template not found: { $value }
template-export-done = Exported { $count } template(s) to { $value }.
template-export-error = { $value }: { $error }
template-import-done = Extracted { $count } template(s) from { $value } to { $detail }.
template-import-exists = { $value }: already exists (use --force to overwrite)
template-import-error = { $value }: { $error }

## API errors (wpg-server)
api-error-not-found = { $what } was not found.
api-error-invalid-body = Invalid request body: { $error }
api-error-invalid-query = Invalid request parameter: { $error }
api-error-invalid-template = Invalid template: { $error }
api-error-invalid-hex = Invalid hex input: { $error }
api-error-parse-failed = The bytes could not be decoded: { $error }
api-error-render-failed = Rendering failed: { $error }
api-error-count-range = `count` must be between 1 and { $max } (got { $error }).
api-error-read-only = Template { $id } belongs to a read-only library; duplicate it to edit.
api-error-policy-iface = Sending on { $iface } is not allowed by the server policy (allowed: { $allowed }).
api-error-policy-rate = The rate { $rate } exceeds the maximum allowed by the server policy ({ $max }).
api-error-policy-unlimited = An unlimited rate is not allowed by the server policy (maximum { $max }).
api-error-invalid-state = Job { $id } is { $state }; cannot { $action } it.
api-error-job-spec = Invalid job: { $error }
api-error-backend = Transmit backend error: { $error }
api-error-internal = Internal server error: { $error }
auth-error-unauthorized = Please log in.
auth-error-bad-credentials = Wrong user name or password.
auth-error-setup-required = No admin password is set yet: complete the setup first.
auth-error-already-configured = The admin password has already been set.
auth-error-rate-limited = Too many failed logins. Try again in { $secs } seconds.
auth-error-csrf = Missing or invalid CSRF token (X-CSRF-Token header).
auth-error-origin = WebSocket connections from another origin are not allowed.
auth-error-setup-remote = The admin password can only be set from the server itself (localhost).
auth-error-password-mismatch = The passwords do not match.
auth-error-password-short = The password must be at least { $min } characters long.
lint-parse-error = The template cannot be parsed: { $error }

## Configuration (wpg.toml)
config-issue-file-missing = file not found
config-issue-bad-url = must start with http:// or https://
config-issue-not-argon2 = does not look like an argon2 hash ($argon2id$...)
config-issue-password-both = both password_hash and password_file are set; password_hash wins
config-issue-zero = must be greater than 0
config-issue-exposed-without-password = listening on a non-loopback address without a password
config-issue-dir-missing = directory not found
config-issue-max-rate-percent = a link percentage cannot be a server-wide maximum; use pps or bps
config-issue-bad-rate = invalid rate
config-issue-unknown-backend = unknown backend
config-issue-backend-unavailable = backend not available in this build / on this OS
config-issue-bad-log-level = invalid log filter
config-level-error = error
config-level-warning = warning
config-issue-line = { $level }: { $key }: { $message }{ $detail }
config-load-error = Configuration error: { $error }
config-init-written = Wrote { $path }. Edit it, then check it with `wpg config validate`.
config-init-exists = { $path } already exists (use --force to overwrite).
config-validate-ok = Configuration OK ({ $source }).
config-validate-summary = { $errors } error(s), { $warnings } warning(s) ({ $source }).
config-source-defaults = built-in defaults and environment
config-password-prompt = New admin password:{ " " }
config-password-confirm = Repeat the password:{ " " }
config-password-mismatch = The passwords do not match.
config-password-short = The password must be at least { $min } characters long.
config-password-written = Stored the password hash in { $path } (auth.password_hash). Restart `wpg serve` to apply it.

## wpg serve
serve-banner-title = { $app } { $version }
serve-banner-listen = Listening on   { $url }/
serve-banner-docs = API docs       { $url }/api/docs/
serve-banner-config = Config         { $source }
serve-banner-data = Data dir       { $path }
serve-banner-library = Library        { $path }{ $state }
serve-banner-user-dir = User templates { $path }
serve-banner-templates = Templates      { $count }
serve-banner-backends = Backends       { $list }
serve-banner-missing = { " " }(missing)
serve-banner-dev = Development mode: CORS allowed for { $origins }
serve-banner-no-password = No password is configured and the server listens on { $url } (--allow-insecure): anyone who can reach it can set the admin password and send packets. Set a password (wpg config set-password) as soon as possible.
serve-failed = Server error: { $error }
serve-log-error = Cannot set up logging: { $error }
serve-banner-tls = TLS            { $path }
serve-banner-tls-self-signed = TLS            { $path } (self-signed)
serve-banner-fingerprint = SHA-256        { $fingerprint }
serve-banner-auth = Login          { $state }
serve-banner-auth-setup = no password yet: set it on the setup screen
serve-banner-auth-configured = admin password from { $source }
serve-banner-auth-database = admin password set on the setup screen
serve-banner-setup-local = No admin password is set yet. Open { $url } in a browser on this machine to set it.
serve-refuse-insecure = Refusing to listen on { $addr }: no admin password is configured. Use a loopback address (127.0.0.1), set a password (--password-file, WPG_PASSWORD, wpg config set-password) or pass --allow-insecure.
serve-password-arg-warning = Warning: --password is visible to other users in the process list; prefer --password-file, WPG_PASSWORD or `wpg config set-password`.

## Web UI (wpg-web). Keys are checked by the `ui_keys_exist` test in crates/wpg-web.
ui-lang-ja = 日本語
ui-lang-en = English
ui-loading = Loading…
ui-close = Close
ui-cancel = Cancel
ui-delete = Delete
ui-save = Save
ui-refresh = Refresh
ui-retry = Retry
ui-yes = Yes
ui-no = No
ui-unlimited = Unlimited
ui-help = Keyboard shortcuts
ui-help-title = Keyboard shortcuts
ui-skip-to-content = Skip to content
ui-not-found = This page does not exist.
ui-back-to-dashboard = Back to the dashboard
ui-view-only = Narrow screen: view only. Editing is available on wider screens.
ui-server-unreachable = The server cannot be reached. Is `wpg serve` running?
ui-error-http = The server answered with HTTP { $id }.
ui-error-network = Network error: { $error }
ui-error-decode = Unexpected response from the server: { $error }
ui-nav-label = Main navigation
ui-nav-dashboard = Dashboard
ui-nav-builder = Builder
ui-nav-jobs = Jobs
ui-nav-templates = Templates
ui-nav-rx = Receive
ui-nav-settings = Settings
ui-shortcut-hint = shows the keyboard shortcuts
ui-shortcut-dashboard = Go to the dashboard
ui-shortcut-builder = Go to the builder
ui-shortcut-jobs = Go to jobs
ui-shortcut-templates = Go to templates
ui-shortcut-rx = Go to receive
ui-shortcut-settings = Go to settings
ui-shortcut-new-job = New job
ui-shortcut-logs = Show / hide the server log
ui-shortcut-help = Show this help
ui-shortcut-escape = Close the dialog
ui-ws-connecting = Connecting…
ui-ws-open = Live
ui-ws-closed = Disconnected (retrying)
ui-theme = Theme
ui-theme-light = Light
ui-theme-dark = Dark
ui-theme-system = System
ui-theme-to-dark = Switch to the dark theme
ui-theme-to-light = Switch to the light theme
ui-logout = Log out
ui-logs-toggle = Server log
ui-logs-title = Server log
ui-logs-level = Level
ui-logs-follow = Follow
ui-logs-clear = Clear the log
ui-logs-count = { $count } lines
ui-logs-empty = No log lines yet.
ui-login-title = Log in
ui-login-submit = Log in
ui-login-busy = Logging in…
ui-login-wrong = Wrong password.
ui-login-rate-limited = Too many attempts. Try again in { $secs } s.
ui-login-wait = Wait { $secs } s
ui-password = Password
ui-password-confirm = Confirm password
ui-setup-title = Set the administrator password
ui-setup-intro = This server has no password yet. Choose one to protect the web UI and the API.
ui-setup-hint = At least { $min } characters.
ui-setup-too-short = The password must have at least { $min } characters.
ui-setup-mismatch = The passwords do not match.
ui-setup-submit = Set password
ui-dash-totals = Totals
ui-dash-active-jobs = Active jobs
ui-dash-running-jobs = Sending
ui-dash-total-pps = Total pps
ui-dash-total-bps = Total bit rate
ui-dash-templates = Templates
ui-dash-uptime = Server uptime
ui-dash-running = Running jobs
ui-dash-no-running = No job is running.
ui-dash-doctor = Self-diagnosis
ui-dash-ifaces = Interfaces
ui-doctor-ok = All checks passed
ui-doctor-warn = Warnings
ui-doctor-fail = Problems found
ui-doctor-counts = { $ok } OK · { $warn } warnings · { $fail } failures
ui-doctor-details = Full report
ui-doctor-status = Status
ui-doctor-check = Check
ui-doctor-detail = Detail / fix
ui-doctor-status-ok = OK
ui-doctor-status-warn = Warning
ui-doctor-status-fail = Failure
ui-doctor-status-skip = Skipped
ui-iface-name = Name
ui-iface-state = State
ui-iface-mac = MAC
ui-iface-addrs = Addresses
ui-iface-mtu = MTU
ui-iface-speed = Speed
ui-iface-allowed = Allowed for TX
ui-iface-up = Up
ui-iface-down = Down
ui-iface-no-carrier = No carrier
ui-stat-pps = Packets/s
ui-stat-bps = Bit rate (L2)
ui-stat-l1-bps = Bit rate (L1)
ui-stat-packets = Packets
ui-stat-bytes = Bytes
ui-stat-dropped = Dropped
ui-stat-ratio = Rate achieved
ui-stat-elapsed = Elapsed
ui-spark-pps = Packets per second, last 30 seconds
ui-state-created = Created
ui-state-scheduled = Scheduled
ui-state-running = Running
ui-state-paused = Paused
ui-state-finished = Finished
ui-state-failed = Failed
ui-state-unknown = Unknown
ui-reason-count-reached = Packet count reached
ui-reason-duration-elapsed = Duration elapsed
ui-reason-source-exhausted = All packets sent
ui-reason-stopped = Stopped
ui-jobs-new = New job
ui-jobs-tab-live = Current
ui-jobs-tab-history = History
ui-jobs-empty = No jobs.
ui-jobs-streams = Streams
ui-jobs-template = Template
ui-jobs-choose-template = Choose a template…
ui-jobs-weight = Weight
ui-jobs-add-stream = Add stream
ui-jobs-remove-stream = Remove stream
ui-jobs-mix = Mix
ui-mix-weighted = Weighted
ui-mix-sequential = Sequential
ui-mix-round-robin = Round robin
ui-jobs-target = Where
ui-jobs-backend = Backend
ui-jobs-iface = Interface
ui-jobs-choose-iface = Choose an interface…
ui-jobs-iface-not-needed = Not needed for this backend
ui-jobs-iface-denied = not allowed
ui-jobs-dry-run = Dry run: pcapng file name
ui-jobs-dry-run-hint = When set, nothing is sent: the packets are written to this file under the server's capture directory.
ui-jobs-how = How
ui-jobs-rate = Rate
ui-jobs-count = Packet count
ui-jobs-duration = Duration
ui-jobs-burst = Burst
ui-jobs-workers = Workers
ui-jobs-workers-disabled = Several workers need a multi-worker backend and a single stream.
ui-jobs-from-template = From the template
ui-jobs-name = Name
ui-jobs-name-placeholder = Defaults to the template name
ui-jobs-start-now = Start immediately
ui-jobs-create = Create job
ui-jobs-create-start = Create and start
ui-jobs-created = Job “{ $id }” created.
ui-jobs-need-iface = Choose an interface.
ui-jobs-need-template = Choose a template for every stream.
ui-jobs-bad-weight = Weights must be positive integers.
ui-rate-default = Empty: the template's rate (or the server's limit).
ui-rate-is-pps = → { $rate }
ui-rate-is-bps = → { $rate } on the wire (layer 1)
ui-rate-is-percent = → { $rate } % of the link speed
ui-rate-is-ifg = Fixed gap between frames: { $rate }
ui-rate-is-unlimited = As fast as possible
ui-valid-empty = Required.
ui-valid-number = Not a number.
ui-valid-integer = Enter a whole number.
ui-valid-range = Must be greater than zero.
ui-valid-percent = A percentage must be at most 100.
ui-valid-rate-unit = Unknown unit: use pps, kpps, Mpps, bps, kbps, Mbps, Gbps, %, ifg=<time> or unlimited.
ui-valid-duration-unit = Unknown unit: use ns, us, ms, s, m or h.
ui-valid-capture-name = Use a relative file name without “..”.
ui-confirm-send-title = Send on a real interface?
ui-confirm-send-body = This job transmits packets on the network. Make sure the interface and the rate are right for this network.
ui-confirm-send = Send
ui-job-detail = Job
ui-job-state = State
ui-job-start = Start
ui-job-pause = Pause
ui-job-resume = Resume
ui-job-stop = Stop
ui-job-download-report = Report (JSON)
ui-job-live = Live statistics
ui-job-streams = Per stream
ui-job-inline-template = (inline template)
ui-job-workers = Per worker
ui-job-workers-pending = Per-worker counters are part of the final report, available when the job has ended.
ui-job-active = Active time
ui-job-core = Core
ui-job-accuracy = Pacing accuracy
ui-acc-target = Target
ui-acc-achieved = Achieved
ui-acc-jitter = Jitter (mean / max)
ui-job-spec = Settings
ui-job-created = Created
ui-job-started = Started
ui-job-finished = Ended
ui-job-deleted = Job deleted.
ui-job-delete-title = Delete this job?
ui-job-delete-body = A running job is stopped first; its report stays in the history.
ui-tpl-import = Import
ui-tpl-new = New template
ui-tpl-search = Search name, description, tag, protocol…
ui-tpl-source = Source
ui-tpl-source-all = All
ui-tpl-source-library = Library
ui-tpl-source-user = Mine
ui-tpl-favorites-only = Favourites only
ui-tpl-tags = Tags
ui-tpl-recent = Recently used
ui-tpl-empty = No template matches.
ui-tpl-favorite = Favourite
ui-tpl-view = View / edit YAML
ui-tpl-open-builder = Open in the builder
ui-tpl-new-job = New job with this template
ui-tpl-export = Download YAML
ui-tpl-duplicate = Duplicate
ui-tpl-imported = Template “{ $id }” imported.
ui-tpl-duplicated = Copy “{ $id }” created.
ui-tpl-deleted = Template “{ $id }” deleted.
ui-tpl-delete-title = Delete this template?
ui-tpl-delete-body = “{ $id }” will be removed from your template directory.
ui-tpl-saved = Template saved.
ui-tpl-yaml = Template YAML
ui-tpl-read-only = Library templates are read-only: duplicate it to edit a copy.
ui-lint-run = Check (lint)
ui-lint-clean = No problems found.
ui-lint-summary = { $errors } errors, { $warnings } warnings
ui-set-appearance = Appearance
ui-set-account = Account
ui-set-logged-in = Logged in as { $id }.
ui-set-auth-disabled = Authentication is not enabled on this server.
ui-set-client = This browser
ui-set-client-note = Stored in this browser only. The server configuration (wpg.toml) is not exposed through the API.
ui-set-api = API
ui-set-ws = Events
ui-set-ui-version = UI version
ui-set-api-docs = API documentation (Swagger UI)
ui-set-about = About
ui-set-server-version = Server version
ui-set-api-version = API version
ui-set-platform = Platform
ui-set-features = Optional features
ui-set-locales = Languages
ui-set-license = wpg is dual-licensed under MIT or Apache-2.0.
ui-builder-placeholder = The packet builder (protocol palette, layer stack, field editor, hex / tree view) arrives in the next milestone step.
ui-builder-placeholder-template = The packet builder arrives in the next milestone step. Template “{ $id }” will open here.
ui-rx-placeholder = Receive counters and verification arrive with milestone M9.

# --- P1 L2/encap protocol field descriptions ---
lldp-tlvs = LLDP TLVs (7-bit type + 9-bit length header).
cdp-version = CDP protocol version (2).
cdp-ttl = Time this advertisement stays valid, in seconds.
cdp-checksum = Internet checksum over the CDP header and TLVs (Auto).
cdp-tlvs = CDP TLVs (16-bit type, 16-bit length including the header).
stp-protocol-id = Spanning Tree protocol identifier (always 0).
stp-version = Protocol version (0 = STP, 2 = RSTP, 3 = MSTP).
stp-type = BPDU type (0 = configuration, 2 = RST/MST, 0x80 = TCN).
stp-flags = BPDU flags (topology change, proposal, role, learning, forwarding, agreement, TCA).
stp-root-priority = Root bridge priority and system-id extension.
stp-root-mac = Root bridge MAC address.
stp-root-path-cost = Cost of the path to the root bridge.
stp-bridge-priority = Sending bridge priority and system-id extension.
stp-bridge-mac = Sending bridge MAC address.
stp-port-id = Sending port identifier (priority + number).
stp-message-age = Age of the BPDU, in 1/256 seconds.
stp-max-age = Maximum BPDU age before timeout, in 1/256 seconds.
stp-hello-time = Interval between configuration BPDUs, in 1/256 seconds.
stp-forward-delay = Forwarding delay timer, in 1/256 seconds.
stp-trailer = RSTP / MSTP extension (version-1 length, v3 length, MSTI records); opaque octets.
lacp-subtype = Slow Protocols subtype (1 = LACP).
lacp-actor-system = Actor system MAC address.
lacp-actor-state = Actor state flags (activity, timeout, aggregation, sync, collecting, distributing, defaulted, expired).
lacp-partner-system = Partner system MAC address.
lacp-partner-state = Partner state flags.
marker-subtype = Slow Protocols subtype (2 = Marker).
eapol-version = EAPOL protocol version.
eapol-type = EAPOL packet type (0 = EAP, 1 = Start, 2 = Logoff, 3 = Key).
eapol-length = Length of the EAPOL body, in octets (Auto).
eapol-body = EAPOL body (the EAP packet for type 0); opaque octets.
pppoe-version = PPPoE protocol version (1).
pppoe-type = PPPoE type (1).
pppoe-code = PPPoE code (PADI / PADO / PADR / PADS / PADT, 0 = session data).
pppoe-session-id = PPPoE session identifier.
pppoe-length = Length of the PPPoE payload, in octets (Auto).
pppoe-tags = PPPoE discovery TAGs (16-bit type, 16-bit length).
ppp-protocol = PPP protocol identifier of the payload (LCP / IPCP / IP / IPv6 / ...).
ppp-cp-code = PPP control-protocol code (Configure-Request / Ack / Nak / ...).
ppp-cp-identifier = PPP control-protocol request identifier.
ppp-cp-length = Length of the PPP control-protocol packet, in octets (Auto).
ppp-cp-options = PPP control-protocol options (8-bit type / length TLVs).
vxlan-flags = VXLAN flags (I bit = VNI is valid).
vxlan-vni = 24-bit VXLAN Network Identifier.
gre-flags = GRE flags and version (checksum / key / sequence present bits).
gre-protocol-type = EtherType of the encapsulated payload.
nvgre-vsid = 24-bit NVGRE Virtual Subnet ID.
nvgre-flowid = NVGRE FlowID (entropy for load balancing).
mpls-label = 20-bit MPLS label value.
mpls-tc = 3-bit traffic class (QoS / ECN).
mpls-s = Bottom-of-stack bit (1 = last label; Auto from the next layer).
mpls-ttl = MPLS time to live.
gtpu-version = GTP version (1).
gtpu-pt = Protocol type (1 = GTP, 0 = GTP').
gtpu-e = Extension-header-present flag.
gtpu-s = Sequence-number-present flag.
gtpu-pn = N-PDU-number-present flag.
gtpu-message-type = GTP-U message type (255 = G-PDU).
gtpu-length = Length of the payload after the mandatory header, in octets (Auto).
gtpu-teid = Tunnel Endpoint Identifier.
geneve-version = Geneve version (0).
geneve-opt-len = Length of the options region, in 4-octet words.
geneve-o = OAM packet flag.
geneve-c = Critical options present flag.
geneve-protocol-type = EtherType of the encapsulated payload.
geneve-vni = 24-bit Geneve Virtual Network Identifier.
geneve-options = Geneve options (opaque; sized by opt_len).
l2tp-flags = L2TP flags and version (type, length, sequence, offset, priority bits).
l2tp-body = L2TP body (presence-conditional fields + payload); opaque octets.
l2tpv3-session-id = L2TPv3 Session ID.

# --- packet builder (M4 phase 2) ---
ui-bld-palette = Protocols
ui-bld-search = Search protocols
ui-bld-favorite = Favourite
ui-bld-favorites = Favourites
ui-bld-layer-l2 = Data link (L2)
ui-bld-layer-l3 = Network (L3)
ui-bld-layer-l4 = Transport (L4)
ui-bld-layer-l7 = Application (L7)
ui-bld-layer-payload = Payload
ui-bld-stack = Layers
ui-bld-empty = Drag a protocol here, or add one from the palette.
ui-bld-unknown = unknown
ui-bld-dup = Duplicate layer
ui-bld-suggest = Add next layer
ui-bld-binding-warn = Not announced by the layer below
ui-bld-fix = Fix
ui-bld-added = Added { $id }.
ui-bld-moved = Moved to position { $index }.
ui-bld-removed = Layer removed.
ui-bld-fields = Field editor
ui-bld-select-layer = Select a layer to edit its fields.
ui-bld-auto = Auto
ui-bld-manual = Manual
ui-bld-gen = Gen
ui-bld-computed = computed on build
ui-bld-nsap-parts = AFI { $afi } · area { $area } · system ID { $sys } · NSEL { $nsel }
ui-bld-estimate = Packets in the stream and their total size
ui-bld-expands = expands to { $count } (~{ $bytes })
ui-bld-rate = Rate
ui-bld-rate-hint = Transmit rate saved with the template (1000pps, 1.5Mpps, 10Gbps, ifg=10us)
ui-bld-duration = ~{ $time } to send
ui-bld-duration-unknown = send time: set a rate
ui-bld-duration-hint = Packets to send (tx.count, else the stream length) at this rate, capped by tx.duration_secs
ui-bld-gen-kind = Kind
ui-bld-gen-count = count:
ui-bld-gen-shorthand = Range / list / CIDR
ui-bld-gen-random = Random
ui-bld-gen-random-mac = Random MAC
ui-bld-gen-counter = Counter
ui-bld-gen-file = File
ui-bld-gen-expr = Expression
ui-bld-hex = Hex
ui-bld-tree = Tree
ui-bld-no-bytes = No packet yet.
ui-bld-name = Template name
ui-bld-need-name = Give the template a name first.
ui-bld-unsaved = Unsaved changes
ui-bld-save-as = Save as…
ui-bld-save-first = Save the template before creating a job.
ui-bld-recompute = Recompute
ui-bld-auto-recompute = Recompute the preview live on every edit
ui-bld-index = Stream index
ui-bld-index-prev = Previous packet
ui-bld-index-next = Next packet
ui-bld-undo = Undo
ui-bld-redo = Redo
ui-bld-parity-ok = The in-browser preview matches the server render.
ui-bld-parity-ok-short = matches
ui-bld-parity-bad = The in-browser preview does not match the server render.
ui-bld-parity-bad-short = mismatch
ui-bld-parity-pending = Checking the server render…
ui-bld-help-title = Builder shortcuts
ui-bld-help-add = Add the focused protocol
ui-bld-help-move = Move the selected layer
ui-bld-help-del = Remove the selected layer
ui-bld-help-undo = Undo
ui-bld-help-redo = Redo
ui-bld-help-hex = Type hex over the caret byte
ui-bld-help-caret = Move the hex caret
ui-bld-suggest-prev = Add layer above

# --- P1 redundancy / multicast protocol field descriptions ---
vrrp-version = VRRP version (2 = RFC 3768, 3 = RFC 5798).
vrrp-type = VRRP packet type (1 = Advertisement).
vrrp-vrid = Virtual Router Identifier.
vrrp-priority = Sending router priority (255 = address owner, 0 = stop participating).
vrrp-count = Number of virtual addresses in the advertisement (Auto).
vrrp-auth-type = VRRPv2 authentication type (0 = none, 1 = simple text, 2 = IP AH).
vrrp-adver-int = VRRPv2 advertisement interval, in seconds.
vrrp-max-adver-int = VRRPv3 maximum advertisement interval, in centiseconds.
vrrp-checksum = Internet checksum over the VRRP message (VRRPv3 includes the IP pseudo header) (Auto).
vrrp-addresses = Virtual router addresses.
vrrp-auth-data = VRRPv2 authentication data (8 octets).
hsrp-version = HSRP version (0 for HSRPv1).
hsrp-op-code = HSRP operation (0 = Hello, 1 = Coup, 2 = Resign, 3 = Advertise).
hsrp-state = Sender state (1 = Learn, 2 = Listen, 4 = Speak, 8 = Standby, 16 = Active).
hsrp-hello-time = Interval between Hello messages, in seconds.
hsrp-hold-time = Time a Hello stays valid, in seconds.
hsrp-priority = Router priority within the group.
hsrp-group = Standby group number.
hsrp-auth-data = Clear-text authentication password (8 octets, default "cisco").
hsrp-virtual-ip = Virtual IPv4 address of the group.
hsrpv2-tlvs = HSRPv2 TLVs (Group State, Interface State, Text / MD5 Authentication; 8-bit type and length).
glbp-version = GLBP version (1).
glbp-group = GLBP group number.
glbp-owner-id = MAC address of the sending router.
glbp-tlvs = GLBP TLVs (Hello, Request/Response, Authentication; length includes the header).
igmp-type = IGMP message type (Auto from the body: 0x11 query, 0x12 / 0x16 report, 0x17 leave, 0x22 v3 report).
igmp-max-resp-code = Maximum response time / code in 1/10 s (queries; zero in reports).
igmp-checksum = Internet checksum over the whole IGMP message (Auto).
igmp-group = Multicast group address (0.0.0.0 in a general query).
igmp-s-flag = Suppress router-side processing flag.
igmp-qrv = Querier's Robustness Variable.
igmp-qqic = Querier's Query Interval Code.
igmp-num-sources = Number of source addresses.
igmp-sources = Source addresses.
igmp-num-records = Number of group records.
igmp-records = Group records (record type, aux data length, sources, multicast address).
mld-max-resp = Maximum response delay / code, in milliseconds.
mld-address = IPv6 multicast address (:: in a general query).
pim-version = PIM version (2).
pim-type = PIM message type (Auto from the body: 0 Hello, 1 Register, 3 Join/Prune).
pim-checksum = Internet checksum (whole message; Register: first 8 octets only) (Auto).
pim-hello-options = PIM Hello options (Holdtime, LAN Prune Delay, DR Priority, Generation ID, Address List).
pim-addr-family = Address family of the encoded address (1 = IPv4).
pim-upstream-neighbor = Upstream neighbor address the Join/Prune is addressed to.
pim-num-groups = Number of group sets.
pim-holdtime = Time the Join/Prune state is kept, in seconds.
pim-groups = Group sets (encoded group, joined and pruned encoded sources).
pim-register-border = Border bit (sender is a PIM Multicast Border Router).
pim-register-null = Null-Register bit (no encapsulated data packet).

# --- P1 application protocol field descriptions ---
dhcp-op = BOOTP message op code (1 = BOOTREQUEST, 2 = BOOTREPLY).
dhcp-htype = Hardware address type (1 = Ethernet).
dhcp-hlen = Hardware address length, in octets (6 for Ethernet).
dhcp-hops = Relay hop count, incremented by each relay agent.
dhcp-xid = Transaction ID chosen by the client.
dhcp-secs = Seconds elapsed since the client began acquisition.
dhcp-flags = BOOTP flags (bit 15 = broadcast reply requested).
dhcp-ciaddr = Client IP address (when already bound).
dhcp-yiaddr = "Your" (client) IP address assigned by the server.
dhcp-siaddr = Next server IP address (boot server).
dhcp-giaddr = Relay agent (gateway) IP address; non-zero when relayed.
dhcp-chaddr = Client hardware (MAC) address; followed by 10 octets of padding.
dhcp-sname = Optional server host name (64 octets, NUL padded).
dhcp-file = Boot file name (128 octets, NUL padded).
dhcp-magic-cookie = DHCP magic cookie (0x63825363).
dhcp-options = DHCP options (8-bit code / length; Pad, End, Message Type, Relay Agent Information, ...).
dhcpv6-msg-type = DHCPv6 message type (Solicit, Advertise, Request, Reply, Relay-Forward, ...).
dhcpv6-transaction-id = 24-bit transaction ID.
dhcpv6-options = DHCPv6 options (16-bit code / length; Client ID, IA_NA, Relay Message, ...).
dhcpv6-hop-count = Number of relay agents that have relayed this message.
dhcpv6-link-address = Address identifying the client's link (relay).
dhcpv6-peer-address = Address of the client or relay the message came from.
dns-id = Query identifier, copied into the response.
dns-qr = Query (0) or response (1).
dns-opcode = Kind of query (0 = standard query, 4 = NOTIFY, 5 = UPDATE).
dns-aa = Authoritative answer flag.
dns-tc = Truncation flag.
dns-rd = Recursion desired flag.
dns-ra = Recursion available flag.
dns-z = Reserved bit (must be 0).
dns-ad = Authentic data flag (DNSSEC).
dns-cd = Checking disabled flag (DNSSEC).
dns-rcode = Response code (0 = NOERROR, 3 = NXDOMAIN, ...).
dns-qdcount = Number of entries in the question section.
dns-ancount = Number of resource records in the answer section.
dns-nscount = Number of resource records in the authority section.
dns-arcount = Number of resource records in the additional section.
dns-sections = Question / answer / authority / additional sections (length-prefixed label names); opaque octets.
ntp-li = Leap indicator (3 = clock unsynchronized).
ntp-version = NTP version number (4).
ntp-mode = Association mode (3 = client, 4 = server, 5 = broadcast).
ntp-stratum = Stratum (0 = unspecified, 1 = primary reference, 2-15 = secondary).
ntp-poll = Maximum poll interval, log2 seconds (signed).
ntp-precision = Clock precision, log2 seconds (signed).
ntp-root-delay = Round-trip delay to the reference clock (16.16 fixed point seconds).
ntp-root-dispersion = Dispersion to the reference clock (16.16 fixed point seconds).
ntp-reference-id = Reference ID (ASCII code for stratum 1, else an IPv4 address or hash).
ntp-reference-ts = Time the system clock was last set (64-bit NTP timestamp).
ntp-origin-ts = Client transmit time echoed by the server (64-bit NTP timestamp).
ntp-receive-ts = Time the request arrived at the server (64-bit NTP timestamp).
ntp-transmit-ts = Time the packet left the sender (64-bit NTP timestamp).
ntp-extensions = Extension fields and MAC; opaque octets.
snmp-tag = BER tag octet (0x30 SEQUENCE, 0x02 INTEGER, 0x04 OCTET STRING).
snmp-msg-length = BER length of the SNMP message SEQUENCE (Auto; short form, below 128).
snmp-version-length = BER length of the version INTEGER (1).
snmp-version = SNMP version (0 = v1, 1 = v2c).
snmp-community-length = BER length of the community string, in octets.
snmp-community = Community string (e.g. "public").
snmp-pdu-type = PDU tag (0xa0 GetRequest, 0xa2 Response, 0xa4 v1 Trap, 0xa7 SNMPv2-Trap, ...).
snmp-pdu-length = BER length of the PDU contents (Auto; short form, below 128).
snmp-pdu-body = PDU contents as BER TLVs: request-id, error-status, error-index and the variable-bindings SEQUENCE.
snmp-trap-body = Trap contents as BER TLVs: enterprise, agent-addr, generic-trap, specific-trap, time-stamp and variable bindings.
syslog-message = Syslog message text, starting with <PRI> (facility * 8 + severity).
radius-code = RADIUS packet type (Access-Request, Access-Accept, Accounting-Request, ...).
radius-identifier = Identifier matching requests and replies.
radius-length = Length of the whole RADIUS packet, in octets (Auto).
radius-authenticator = 16-octet Request / Response Authenticator (Manual).
radius-attributes = RADIUS attributes (8-bit type, 8-bit length including the header).
tftp-opcode = TFTP opcode (1 = RRQ, 2 = WRQ, 3 = DATA, 4 = ACK, 5 = ERROR, 6 = OACK).
tftp-filename = File name (NUL-terminated).
tftp-mode = Transfer mode: netascii, octet or mail (NUL-terminated).
tftp-options = Option name / value string pairs (RFC 2347): blksize, timeout, tsize, windowsize, ...
tftp-block = Block number (starting at 1).
tftp-data = Data octets (512 per block; a shorter block ends the transfer).
tftp-error-code = TFTP error code (1 = file not found, 2 = access violation, ...).
tftp-error-message = Error message (NUL-terminated).

# --- P1 routing protocol field descriptions ---
ospf-version = OSPF version (2 = OSPFv2, 3 = OSPFv3).
ospf-type = OSPF packet type (Hello / DB Description / LS Request / LS Update / LS Ack; Auto from the next layer).
ospf-packet-length = Length of the OSPF packet including the header, in octets (Auto).
ospf-router-id = Router ID of the sending router.
ospf-area-id = Area the packet belongs to (0.0.0.0 = backbone).
ospf-checksum = Internet checksum over the OSPF packet without the authentication field (Auto).
ospf-au-type = Authentication type (0 = null, 1 = simple password, 2 = cryptographic).
ospf-authentication = 64-bit authentication field (password, or key id / sequence for cryptographic auth).
ospf-network-mask = Network mask of the interface or advertised network.
ospf-hello-interval = Interval between Hello packets, in seconds.
ospf-options = Optional capabilities (E, MC, N/P, L, DC, O, DN bits).
ospf-router-priority = Router priority for the DR election (0 = never DR).
ospf-router-dead-interval = Seconds without a Hello before the neighbor is declared down.
ospf-designated-router = Designated Router (0.0.0.0 = none).
ospf-backup-designated-router = Backup Designated Router (0.0.0.0 = none).
ospf-neighbors = Router IDs of the neighbors seen on the link.
ospf-interface-mtu = Largest IP datagram the interface can send without fragmentation.
ospf-dbd-flags = Database Description flags (I = init, M = more, MS = master).
ospf-dd-sequence = Database Description sequence number.
ospf-lsa-headers = LSA headers (20 octets each; checksums and lengths are Manual).
ospf-lsr-requests = Requested LSAs (LS type, Link State ID, advertising router).
ospf-num-lsas = Number of LSAs in the update.
ospf-ls-age = Time since the LSA was originated, in seconds (keep 0 for an exact Auto checksum).
ospf-ls-type = LSA type (Auto from the LSA body layer; 9 / 10 / 11 = opaque LSA).
ospf-link-state-id = Link State ID identifying the described part of the network.
ospf-advertising-router = Router ID of the router that originated the LSA.
ospf-ls-sequence = LSA sequence number (0x80000001 = initial).
ospf-ls-checksum = Fletcher checksum of the LSA without LS age (Auto).
ospf-lsa-length = Length of the LSA including its header, in octets (Auto).
ospf-router-lsa-flags = Router-LSA flags (V = virtual link endpoint, E = ASBR, B = ABR).
ospf-num-links = Number of router links described.
ospf-router-links = Router links (link ID, link data, type, TOS count, metric).
ospf-attached-routers = Router IDs of the routers attached to the network.
ospf-metric = Cost of the route.
ospf-external-e = External metric type (1 = type 2, 0 = type 1).
ospf-forwarding-address = Address traffic for the destination is forwarded to (0.0.0.0 = the ASBR).
ospf-external-route-tag = 32-bit tag carried with the external route.
ospf-opaque-ls-type = Opaque LSA flooding scope (9 = link-local, 10 = area, 11 = AS).
ospf-opaque-type = Opaque type (1 = TE, 4 = Router Information, 7 = Extended Prefix, 8 = Extended Link; Auto from the body layer).
ospf-opaque-id = Opaque ID distinguishing LSAs of the same opaque type (24 bits).
ospf-opaque-tlvs = TLVs of the opaque LSA (16-bit type and length, values padded to 4 octets; lengths Auto).
ospfv3-checksum = Internet checksum over the OSPFv3 packet with the IPv6 pseudo header (Auto).
ospfv3-instance-id = Instance ID (separates OSPFv3 instances on one link).
ospfv3-interface-id = Interface ID of the sending interface.
ospfv3-options = OSPFv3 options (V6, E, N, R, DC, AF bits).
ospfv3-ls-type = LSA function code with U / S scope bits (Auto from the LSA body layer).
ospfv3-router-links = Router interfaces (type, metric, interface IDs, neighbor router ID).
ospfv3-prefix-length = Prefix length in bits.
ospfv3-prefix-options = Prefix options (NU, LA, P, DN bits).
ospfv3-address-prefix = Address prefix, padded to a multiple of 4 octets.
ospfv3-external-flags = AS-External-LSA flags (E = type 2 metric, F = forwarding address present, T = route tag present).
ospfv3-referenced-ls-type = LS type of the referenced LSA.
ospfv3-link-local-address = Link-local address of the interface.
ospfv3-num-prefixes = Number of prefixes that follow.
ospfv3-prefixes = Prefix entries (length, options, metric / reserved, address prefix); opaque octets.
ospfv3-referenced-link-state-id = Link State ID of the referenced LSA.
ospfv3-referenced-advertising-router = Advertising router of the referenced LSA.
bgp-marker = 16-octet marker (all ones).
bgp-length = Length of the BGP message including the header, in octets (Auto).
bgp-type = BGP message type (OPEN / UPDATE / NOTIFICATION / KEEPALIVE / ROUTE-REFRESH; Auto from the next layer).
bgp-version = BGP version (4).
bgp-my-as = Autonomous system number of the sender (23456 = AS_TRANS for 4-octet AS).
bgp-hold-time = Proposed hold time, in seconds (0 or at least 3).
bgp-identifier = BGP identifier (router ID) of the sender.
bgp-opt-params-length = Length of the optional parameters, in octets (Auto).
bgp-opt-params = Optional parameters (type 2 = capabilities).
bgp-withdrawn-routes-length = Length of the withdrawn routes, in octets (Auto).
bgp-withdrawn-routes = Withdrawn IPv4 routes (prefix length in bits, prefix).
bgp-total-path-attribute-length = Length of the path attributes, in octets (Auto).
bgp-path-attributes = Path attributes (flags + type code; the length is 16 bits with the Extended Length flag, set automatically above 255 octets).
bgp-nlri = Announced IPv4 routes (prefix length in bits, prefix); other families go in MP_REACH_NLRI.
bgp-error-code = NOTIFICATION error code.
bgp-error-subcode = NOTIFICATION error subcode.
bgp-error-data = Diagnostic data of the NOTIFICATION.
bgp-afi = Address Family Identifier (1 = IPv4, 2 = IPv6).
bgp-rr-subtype = ROUTE-REFRESH subtype (0 = normal, 1 = BoRR, 2 = EoRR).
bgp-safi = Subsequent Address Family Identifier (1 = unicast, 2 = multicast).
rip-command = RIP command (1 = request, 2 = response).
rip-version = RIP version (1 or 2; RIPng uses 1).
rip-entries = Route entries (AFI, route tag, address, mask, next hop, metric).
ripng-entries = Route table entries (prefix, route tag, prefix length, metric; 0xff = next hop).
eigrp-version = EIGRP header version (2).
eigrp-opcode = EIGRP opcode (Update / Query / Reply / Hello / SIA-Query / SIA-Reply).
eigrp-checksum = Internet checksum over the EIGRP packet (Auto).
eigrp-flags = EIGRP flags (Init, Conditional Receive, Restart, End of Table).
eigrp-sequence = Sequence number for reliable delivery.
eigrp-acknowledge = Acknowledged sequence number (0 = none).
eigrp-virtual-router-id = Virtual router ID (0 = unicast address family).
eigrp-autonomous-system = EIGRP autonomous system number.
eigrp-tlvs = EIGRP TLVs (16-bit type, 16-bit length including the header).

# --- P2 OSI encapsulation field descriptions ---
osi-nlpid = Network layer protocol identifier (ISO/IEC TR 9577): 0x81 CLNP, 0x82 ES-IS, 0x83 IS-IS.
osi-data = Rest of the OSI PDU after the NLPID (opaque octets).
clnp-nlpid = Network layer protocol identifier (0x81 = CLNP).
clnp-length-indicator = Header length in octets, up to the end of the options (Auto).
clnp-version = Version / protocol ID extension (1).
clnp-lifetime = Remaining lifetime in units of 500 ms.
clnp-sp = SP: segmentation permitted; the segmentation part is present (Auto from the segmentation part).
clnp-ms = MS: more segments follow.
clnp-er = E/R: send an error report if the PDU is discarded.
clnp-pdu-type = PDU type: 0x1c DT (data), 0x1d MD (multicast data), 0x01 ER (error report), 0x1e ERQ / 0x1f ERP (echo request / reply).
clnp-segment-length = Length of the whole PDU, header and data (Auto).
clnp-checksum = ISO 8473 Fletcher checksum of the header (Auto). 0 = checksum not used.
clnp-destination = Destination NSAP address.
clnp-source = Source NSAP address.
clnp-segmentation = Segmentation part (data unit identifier, segment offset, total length), present when SP is set. The total length is not computed.
clnp-options = Options (8-bit code / 8-bit length) up to the length indicator: security, QoS maintenance, priority, source routing, recording of route, padding, reason for discard.
esis-nlpid = Network layer protocol identifier (0x82 = ES-IS).
esis-length-indicator = Length of the PDU in octets (Auto).
esis-version = Version / protocol ID extension (1).
esis-pdu-type = PDU type: 2 ESH (end system hello), 4 ISH (intermediate system hello), 6 RD (redirect).
esis-holding-time = Holding time in seconds.
esis-checksum = ISO 8473 Fletcher checksum of the PDU (Auto). 0 = checksum not used.
esis-number-of-source-addresses = Number of source addresses (Auto).
esis-source-addresses = NSAP addresses served by the end system.
esis-net = Network entity title of the intermediate system (redirect: empty when the next hop is an end system).
esis-destination = Destination NSAP address being redirected.
esis-bsnpa-length = Length of the subnetwork address (Auto).
esis-bsnpa = Subnetwork address (e.g. MAC address) of the better next hop.
esis-options = Options (8-bit code / 8-bit length) up to the length indicator: security, QoS maintenance, priority, address mask, SNPA mask, suggested ES configuration timer.
esis-data = Rest of the PDU after the fixed part (opaque octets).
isis-irpd = Intradomain routing protocol discriminator (0x83 = IS-IS).
isis-header-length = Length of the fixed header in octets (constant for each PDU type: LAN Hello / LSP 27, P2P Hello 20, CSNP 33, PSNP 17).
isis-version-ext = Version / protocol ID extension (1).
isis-id-length = System ID length (0 = 6 octets).
isis-pdu-type = PDU type (15/16 LAN Hello L1/L2, 17 P2P Hello, 18/20 LSP L1/L2, 24/25 CSNP, 26/27 PSNP).
isis-version = Version (1).
isis-max-area-addresses = Maximum number of area addresses (0 = 3).
isis-pdu-length = Length of the whole PDU (Auto). On a Hello, a Manual value pads the PDU to that length with Padding TLVs (e.g. 1497 for an MTU of 1500).
isis-tlvs = TLVs (8-bit code / 8-bit length) and their sub-TLVs.
isis-source-id = System ID of the sender.
isis-holding-time = Holding time in seconds.
isis-priority = Priority for designated IS election (0–127).
isis-lan-id = LAN ID: system ID of the designated IS and pseudonode ID.
isis-circuit-type = Circuit type: 1 = L1, 2 = L2, 3 = L1/L2.
isis-local-circuit-id = Local circuit ID.
isis-remaining-lifetime = Remaining lifetime of the LSP in seconds (not covered by the checksum).
isis-lsp-id = LSP ID: system ID, pseudonode ID and fragment number.
isis-sequence-number = LSP sequence number.
isis-lsp-checksum = Fletcher checksum from the LSP ID to the end of the PDU (Auto).
isis-partition-repair = P bit: partition repair supported.
isis-attached = ATT bits: attached to another area (default / delay / expense / error metric).
isis-overload = OL bit: LSP database overload.
isis-is-type = IS type: 1 = L1, 3 = L1/L2.
isis-snp-source-id = System ID of the sender and circuit ID (0).
isis-start-lsp-id = First LSP ID covered by the CSNP.
isis-end-lsp-id = Last LSP ID covered by the CSNP.
isis-data = Rest of the PDU after the common header (opaque octets).

# --- P2 SCTP field descriptions ---
sctp-src-port = Source port.
sctp-dst-port = Destination port.
sctp-verification-tag = Verification tag: the peer's initiate tag (0 in a packet carrying INIT).
sctp-checksum = CRC32c of the whole SCTP packet with this field zeroed (Auto), sent least significant octet first.
sctp-chunks = Chunks (8-bit type, 8-bit flags, 16-bit length including the 4-octet header, zero padding to 4 octets): DATA, INIT, INIT ACK, SACK, HEARTBEAT, ABORT, SHUTDOWN, ERROR, COOKIE ECHO / ACK, AUTH, I-DATA, ASCONF, RE-CONFIG, FORWARD TSN, PAD. Lengths, padding and SACK counts are Auto.

# --- P2 LDP / RSVP-TE / BFD field descriptions ---
ldp-version = LDP version (1).
ldp-pdu-length = Length of the PDU after the version and PDU length fields (Auto).
ldp-lsr-id = LSR ID: the first four octets of the LDP identifier.
ldp-label-space = Label space ID: the last two octets of the LDP identifier (0 = platform-wide).
ldp-messages = Messages (16-bit type with the U bit, 16-bit length of the value, message ID, TLVs): Notification, Hello, Initialization, KeepAlive, Address / Address Withdraw, Label Mapping / Request / Withdraw / Release / Abort Request. TLVs: FEC, Address List, Hop Count, Path Vector, Generic Label, Status, Common Hello / Session Parameters, transport addresses, Configuration Sequence Number. Lengths are Auto.
rsvp-version = RSVP version (1).
rsvp-flags = Flags (bit 0: refresh reduction capable, RFC 2961).
rsvp-msg-type = Message type (1 Path, 2 Resv, 3 PathErr, 4 ResvErr, 5 PathTear, 6 ResvTear, 7 ResvConf, 12 Bundle, 13 Ack, 15 Srefresh, 20 Hello).
rsvp-checksum = Internet checksum of the whole message with this field zeroed (Auto).
rsvp-send-ttl = IP TTL the message was sent with.
rsvp-reserved = Reserved (0).
rsvp-length = Length of the whole message in octets, including the common header (Auto).
rsvp-objects = Objects (16-bit length including the 4-octet header, Class-Num, C-Type): SESSION, RSVP_HOP, TIME_VALUES, ERROR_SPEC, SCOPE, STYLE, FLOWSPEC, FILTER_SPEC, SENDER_TEMPLATE, SENDER_TSPEC, RESV_CONFIRM, LABEL, LABEL_REQUEST, EXPLICIT_ROUTE, RECORD_ROUTE, HELLO, MESSAGE_ID, SESSION_ATTRIBUTE. Lengths, the session name length and padding are Auto.
bfd-version = BFD version (1).
bfd-diag = Diagnostic code: the reason of the last state change of the local session.
bfd-state = Session state: 0 AdminDown, 1 Down, 2 Init, 3 Up.
bfd-p = P bit: Poll (a parameter change, the peer answers with F).
bfd-f = F bit: Final (answer to a Poll).
bfd-c = C bit: Control Plane Independent.
bfd-a = A bit: authentication present (Auto: set when there is an authentication section).
bfd-d = D bit: Demand mode.
bfd-m = M bit: Multipoint (0).
bfd-detect-mult = Detect time multiplier.
bfd-length = Length of the control packet in octets, including the authentication section (Auto).
bfd-my-discriminator = My discriminator: a non-zero value unique to the session on the sender.
bfd-your-discriminator = Your discriminator: the peer's discriminator (0 while unknown).
bfd-desired-min-tx = Desired minimum TX interval in microseconds.
bfd-required-min-rx = Required minimum RX interval in microseconds.
bfd-required-min-echo-rx = Required minimum echo RX interval in microseconds (0 = no echo).
bfd-authentication = Authentication section (8-bit type, 8-bit length including the header): simple password, keyed MD5 / SHA1 and their meticulous variants. The digest / hash is Manual.

# --- Generated documentation (`wpg docs gen`, crates/wpg-cli/src/cmd/docs) ---
docs-gen-no-root = { $path } is not the repository root (no docs/en/src/SUMMARY.md); pass --root.
docs-gen-error = Generating the docs failed: { $error }
docs-gen-stale = stale: { $path }
docs-gen-extra = no longer generated: { $path }
docs-gen-up-to-date = The generated docs are up to date.
docs-gen-run-hint = Run `just docs-gen` (or `wpg docs gen`) and commit the result.
docs-gen-done = Docs generated: { $written } files written, { $removed } removed.
docgen-yes = yes
docgen-octets = { $octets } octets
docgen-items = { $count } items
docgen-variant = Variant { $n }
docgen-col-id = ID
docgen-col-name = Name
docgen-col-fields = Fields
docgen-col-next-by = Next layer by
docgen-col-notes = Notes
docgen-col-lower = Lower layer
docgen-col-selector = Selector
docgen-col-value = Value
docgen-col-protocol = Protocol
docgen-col-note = Note
docgen-col-level = Level
docgen-col-rule = Rule
docgen-col-field = Field
docgen-col-type = Type
docgen-col-default = Default
docgen-col-auto = Auto
docgen-col-description = Description
docgen-col-bit = Bit
docgen-col-mask = Mask
docgen-col-type-value = Type
docgen-col-value-fields = Value fields
docgen-col-key = Key
docgen-col-required = Required
docgen-col-in = In
docgen-col-status = Status
docgen-col-body = Body
docgen-col-tag = Tag
docgen-col-endpoint = Endpoint
docgen-col-summary = Summary
docgen-col-env = Environment variable
docgen-layer-l2 = L2 (data link)
docgen-layer-l3 = L3 (network)
docgen-layer-l4 = L4 (transport)
docgen-layer-l7 = Application and control
docgen-layer-payload = Payload
docgen-proto-title = Protocol reference
docgen-proto-intro = This chapter is generated from the protocol definition IR of `wpg-protocols` (the catalog served at `GET /api/protocols`), so it always matches the code: { $count } protocols. Each page lists the fields in wire order with their type, default, Auto rule and description, the lower layers that lead to the protocol, the next-layer table, the constraints checked after building and the bundled sample templates. The hand-written [protocol notes](../protocols/README.md) explain how the protocols are reached, which fields you set by hand and the approximations that remain. After changing a definition, regenerate with `just docs-gen`.
docgen-proto-layer = Layer
docgen-proto-header = Header
docgen-proto-header-fixed = { $octets } octets (fixed)
docgen-proto-header-variable = variable length
docgen-proto-min-frame = Minimum frame
docgen-proto-synth = Generator
docgen-proto-synth-payload = built-in payload generator (the fields below are parameters, not wire fields)
docgen-proto-pseudo = Pseudo header
docgen-proto-pseudo-fields = supplies { $src } / { $dst } to upper-layer checksums
docgen-proto-notes = Notes
docgen-proto-inbound = Reached from
docgen-proto-inbound-none = No protocol of the catalog leads here: use it as the first layer or stack it explicitly.
docgen-proto-fields = Fields
docgen-proto-params = Parameters
docgen-proto-no-fields = No fields.
docgen-proto-next = Next layer
docgen-proto-next-intro = The next layer is selected by { $field }; when the field is Auto the builder writes the value of the protocol stacked above.
docgen-proto-next-alt = When decoding, { $field } is looked up too if the first field matches no entry.
docgen-proto-constraints = Constraints
docgen-proto-samples = Sample templates
docgen-proto-samples-none = No bundled template uses this protocol yet.
docgen-proto-samples-more = and { $count } more
docgen-bind-default = (any other value)
docgen-bind-min-len = at least { $octets } octets remaining
docgen-bind-reparse = re-decodes this layer
docgen-bind-peek = First { $bits } bits of the payload when { $field } = { $value }
docgen-level-error = error
docgen-level-warning = warning
docgen-level-info = info
docgen-c-range = { $field } within { $min }..{ $max }
docgen-c-equals = { $field } = { $value }
docgen-c-not-equals = { $field } ≠ { $value }
docgen-c-min-length = { $span } at least { $octets } octets
docgen-c-max-length = { $span } at most { $octets } octets
docgen-span-header = header
docgen-span-payload = payload
docgen-span-total = header and payload
docgen-unit-bytes = octets
docgen-unit-words16 = 16-bit words
docgen-unit-words32 = 32-bit words
docgen-unit-words64 = 64-bit words
docgen-auto-length = Length of the { $span } in { $unit }
docgen-auto-checksum = { $algorithm } over { $covers }
docgen-auto-nonzero = a computed 0 is sent as all ones
docgen-auto-exclude = { $fields } counted as zero
docgen-auto-next = Protocol number of the next layer (see Next layer)
docgen-auto-count = Number of items of { $field }
docgen-auto-size = Size of { $field } in octets
docgen-auto-counter = Per-stream counter (the default value is used for now)
docgen-cksum-internet = Internet checksum
docgen-cksum-internet-pseudo = Internet checksum with pseudo header
docgen-cksum-fletcher = Fletcher checksum
docgen-cksum-crc32c = CRC-32C
docgen-cksum-crc32 = CRC-32
docgen-cover-header = the header
docgen-cover-all = the header and everything after it
docgen-len-from-field = { $expr } octets
docgen-len-count = { $field } items
docgen-len-until = up to offset { $expr } of the group
docgen-len-to-end = rest of the PDU
docgen-list-item = Each item is a group of the fields below.
docgen-tlv-format = Type { $type } bits, length { $len } bits.
docgen-tlv-len-first = The length comes before the type.
docgen-tlv-len-incl = The length counts the type and length fields too.
docgen-tlv-len-excl = The length counts the value only.
docgen-tlv-unit = Length unit: { $unit }.
docgen-tlv-head = The first { $octets } value octets sit between the type and the length.
docgen-tlv-align = Each item is zero-padded to a multiple of { $align } octets (not counted in the length)
docgen-tlv-unpadded-last = the last item is not padded
docgen-tlv-long = Type bit { $flag } selects a { $len }-bit length.
docgen-tlv-fill = A Manual value of { $field } pads the region with { $variant } items up to that size.
docgen-tlv-unknown-raw = Unknown types are kept as raw value octets.
docgen-tlv-unknown-stop = An unknown type ends the typed decoding (the rest becomes one raw item).
docgen-tlv-type-only = (type only, no length or value)
docgen-tlv-when-prefix = when the value starts with { $octets }
docgen-tlv-when-len = when the value is { $octets } octets
docgen-notes-heading = Generated reference
docgen-notes-intro = Field reference of the protocols on this page (generated from the definitions):
docgen-cli-title = CLI reference
docgen-cli-intro = The `wpg` binary runs both the server (`wpg serve`) and the headless tools, so it suits CI and ssh sessions too. For example, `wpg send tpl.wpg.yaml -i eth0 --rate 1Mpps` transmits a template. This page is generated from the command definitions: each block is the verbatim `--help` output of the current build. Run-time messages follow `LANG` / `LC_ALL`; the `--help` text itself is English.
docgen-cli-commands = Commands
docgen-cli-note-send = Sends a template. `--plan` chooses between expanding all frames up front (`prerender`) and building them while sending (`onthefly`). `--json` prints the final report on stdout.
docgen-cli-note-render = Writes the frames of a template to a pcapng file without transmitting.
docgen-cli-note-import = Turns a capture (or pasted hex) into a template; see the [pcap replay tutorial](../tutorials/pcap-replay.md).
docgen-cli-note-bench = Unlimited-rate transmission of a synthetic frame (EtherType 0x88b5), or of a template with `--template`. See `docs/perf.md` for baselines. `--json` output fields: backend, detail, iface, frame_size, batch_size, packets, dropped, pps, bps (L2), l1_bps, elapsed.
docgen-cli-note-ifaces = Lists interfaces with MAC, addresses, MTU, link state and driver.
docgen-cli-note-doctor = Runs the environment checks described in [Permissions](../getting-started/permissions.md); the exit status is non-zero when a check fails.
docgen-cli-note-template = Template tools. `lint` exits with status 1 on errors; `export` and `import` pack templates into, and unpack them from, `.wpgbundle` archives.
docgen-cli-note-config = `wpg.toml` tools; see the [configuration reference](config.md).
docgen-api-title = API reference
docgen-api-intro = wpg-server serves a REST (JSON) API under `/api` and pushes live statistics, job state changes and logs over a WebSocket (`GET /api/ws`). This page is generated from the OpenAPI document of the server (utoipa). A running server serves the same document at `/api/openapi.json` with Swagger UI at `/api/docs/`, and a copy is kept in [`docs/schema/openapi.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/openapi.json). Log in with `POST /api/auth/login` (or `POST /api/auth/setup` on first run); the `wpg_session` cookie then authenticates every request, and POST / PUT / DELETE must also send the CSRF token of `GET /api/auth/status` in the `X-CSRF-Token` header. Errors are a JSON object `error` with `code`, `message` (in the language of `Accept-Language`) and `details`. The endpoint and schema descriptions below come from the code.
docgen-api-endpoints = Endpoints
docgen-api-params = Parameters
docgen-api-body = Request body
docgen-api-responses = Responses
docgen-api-schemas = Schemas
docgen-api-schemas-intro = Request and response bodies referenced above.
docgen-api-tag-auth = Login, logout and first-run setup.
docgen-api-tag-system = Health, version, interfaces, self-diagnosis and transmit backends.
docgen-api-tag-protocols = The protocol catalog (see the [protocol reference](protocols.md)).
docgen-api-tag-packets = Rendering, decoding and linting packets.
docgen-api-tag-templates = The template library.
docgen-api-tag-jobs = Transmit jobs and statistics.
docgen-api-tag-events = The WebSocket event stream.
docgen-config-title = Configuration
docgen-config-intro = The server reads `wpg.toml` from the working directory, or the file given with `--config` / `WPG_CONFIG`. Precedence, lowest first: built-in defaults, the file, environment variables `WPG_<SECTION>__<KEY>` (`__` separates nesting levels, for example `WPG_SERVER__BIND=0.0.0.0:8080` or `WPG_TX__ALLOWED_IFACES='["eth1"]'`), then command line options such as `wpg serve --bind`. Unknown keys are errors. `wpg config init` writes the commented example below, `wpg config show` prints the effective configuration, `wpg config validate` checks it and `wpg config set-password` stores the admin password hash. The admin password itself is not a key: `wpg serve --password`, `--password-file` and `WPG_PASSWORD` take precedence over `auth.password_hash` and `auth.password_file`. This page is generated from the configuration types.
docgen-config-keys = Keys
docgen-config-top-level = Top level
docgen-config-default-machine = OS data directory (Linux: `~/.local/share/wpg`)
docgen-config-example = Example (`wpg config init`)
docgen-config-example-intro = Every active line equals the built-in default, so the file changes nothing until edited; the commented lines show the optional keys.
docgen-config-data-dir = Directory for server state: user templates, favourites, the SQLite database (users, sessions, job history), dry-run captures and the self-signed TLS certificate.
docgen-config-server = HTTP listener.
docgen-config-server-bind = Listen address. Without a password (and no user created on the setup screen) only loopback addresses are accepted (`--allow-insecure` overrides).
docgen-config-server-tls = TLS certificate chain and private key (PEM) to serve HTTPS with; takes precedence over `self_signed_tls`.
docgen-config-server-tls-cert = PEM certificate chain.
docgen-config-server-tls-key = PEM private key.
docgen-config-server-self-signed-tls = Serve HTTPS with a self-signed certificate generated into `<data_dir>/tls/` (reused on later starts); `wpg serve --tls` sets it.
docgen-config-server-base-url = Public URL of the server behind a reverse proxy (`https://wpg.example.net`), used for absolute links.
docgen-config-auth = Authentication.
docgen-config-auth-password-hash = argon2id PHC string of the admin password; `wpg config set-password` writes it.
docgen-config-auth-password-file = File holding the admin password: an argon2 PHC string, or plain text that is hashed at startup.
docgen-config-auth-session-ttl-secs = Session lifetime in seconds (sliding: every request extends it).
docgen-config-templates = Template library.
docgen-config-templates-library-paths = Read-only template libraries (Git checkouts, for example), scanned recursively for `*.wpg.yaml` / `*.wpg.json` and reloaded when files change.
docgen-config-templates-user-dir = Writable directory for templates created or imported in the GUI; default `<data_dir>/templates`.
docgen-config-tx = Transmit policy and defaults.
docgen-config-tx-allowed-ifaces = Interfaces jobs may send on; empty = all. The `null` backend and dry runs are not restricted.
docgen-config-tx-max-rate = Highest rate a job may request (`100kpps`, `1Gbps`); jobs without a rate are sent at this rate.
docgen-config-tx-default-backend = Backend used when a job names none (`null`, `pcapfile`, `afpacket`, `pcap`, `afxdp`, `dpdk`); unset = `afpacket` on Linux, else `pcap`, else `null`.
docgen-config-log = Logging.
docgen-config-log-level = Filter directives (`RUST_LOG` overrides them), for example `info` or `wpg_tx=debug,info`.
docgen-config-log-format = Line format: `pretty` (human readable) or `json` (one object per line).
docgen-config-log-file = Also write to this file (its directory is created).
docgen-config-log-rotation = Rotation of the file: `never`, `hourly` or `daily`.
docgen-config-log-max-files = Keep at most this many rotated files (the oldest are deleted).
docgen-config-log-ansi = Colour the console output when it is a terminal.
docgen-schema-heading = Key reference (generated)
docgen-schema-intro = Generated from the JSON Schema of the template format (`wpg template schema`); the descriptions are the doc comments of the code (English). `Template` is the document root.
