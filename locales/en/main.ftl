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
