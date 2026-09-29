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
