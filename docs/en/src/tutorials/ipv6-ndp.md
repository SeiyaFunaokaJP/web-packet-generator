# Tutorial: IPv6 and Neighbor Discovery

Time: about 25 minutes. Privileges: none until Step 6.

> This page uses the IPv6 stack to practise two ideas: several layers whose type, length and checksum fields are derived from each other, and a link-layer multicast address computed from a network-layer one. The OSI stack without IP (IS-IS over LLC) is covered in [Bringing up an IS-IS adjacency with Hellos](isis-adjacency.md).

## What you will build

The four kinds of Neighbor Discovery messages that make IPv6 work on a link, then a sweep of Neighbor Solicitations:

| Template | Message |
|---|---|
| `templates/protocols/l3/ndp-rs.wpg.yaml` | Router Solicitation to `ff02::2` |
| `templates/protocols/l3/ndp-ra.wpg.yaml` | Router Advertisement with SLLA, MTU and a /64 prefix |
| `templates/protocols/l3/ndp-ns.wpg.yaml` | Neighbor Solicitation to a solicited-node group |
| `templates/protocols/l3/ndp-na.wpg.yaml` | Neighbor Advertisement (S=1, O=1) with a target link-layer address |
| `templates/protocols/l3/ndp-redirect.wpg.yaml` | Redirect |
| `templates/scenarios/ndp-ns-sweep.wpg.yaml` | 256 Neighbor Solicitations, `2001:db8::1` to `::100` |
| `templates/scenarios/ra-flood.wpg.yaml` | 10,000 RAs from random link-local sources (for RA-guard tests) |

## Before you start

Build wpg and run from the repository root. Only send on lab networks you own or are authorised to test: forged RAs can disrupt every host on a segment.

## Step 1. Follow the layers of one Neighbor Solicitation

```console
$ wpg template show templates/protocols/l3/ndp-ns.wpg.yaml
Hex dump (86 bytes)
0000  33 33 ff 00 00 02 02 00  00 00 00 01 86 dd 60 00  33............`.
0010  00 00 00 20 3a ff 20 01  0d b8 00 00 00 00 00 00  ... :. .........
...
Layout
[0] ethernet2  bytes 0..86
  dst: 33:33:ff:00:00:02
  src: 02:00:00:00:00:01
  ethertype: 0x86dd (ipv6)  (auto)
[1] ipv6  bytes 14..86
  payload_length: 32  (auto)
  next_header: 0x3a (icmpv6)  (auto)
  hop_limit: 255
  src: 2001:db8::1
  dst: ff02::1:ff00:2
[2] icmpv6  bytes 54..86
  type: 0x87 (neighbor_solicitation)  (auto)
  code: 0
  checksum: 7207  (auto)
[3] ndp_ns  bytes 58..86
  reserved: 0
  target: 2001:db8::2
  options: 01 01 02 00 00 00 00 01
```

Every `(auto)` value was derived, not typed:

* `ethernet2.ethertype` = `0x86dd` because the next layer is `ipv6`.
* `ipv6.next_header` = 58 because the next layer is `icmpv6`, and `payload_length` counts everything after the 40-octet header.
* `icmpv6.type` = 135 because the next layer is `ndp_ns`. The ICMPv6 message is split in two layers on purpose: `icmpv6` holds `type`, `code` and `checksum`, and the body layer (`ndp_ns`, `ndp_na`, `ndp_rs`, `ndp_ra`, `ndp_redirect`, `icmpv6_echo`) selects the type.
* `icmpv6.checksum` covers the IPv6 pseudo header, the ICMPv6 header and the whole body, so it changes whenever any field below it changes.

The YAML behind it is short:

```yaml
layers:
  - protocol: ethernet2
    fields: { dst: "33:33:ff:00:00:02", src: "02:00:00:00:00:01" }
  - protocol: ipv6
    fields:
      src: 2001:db8::1
      dst: ff02::1:ff00:2
      hop_limit: 255          # NDP requires 255
  - protocol: icmpv6          # type: auto
  - protocol: ndp_ns
    fields:
      target: ${target}
      options: "01 01 02 00 00 00 00 01"   # source link-layer address
```

## Step 2. Two addresses that must agree

A Neighbor Solicitation for `2001:db8::2` goes to the solicited-node multicast group `ff02::1:ff00:2` (`ff02::1:ff` plus the low 24 bits of the target) and, on Ethernet, to the multicast MAC `33:33:` plus the low 32 bits of the IPv6 destination: `33:33:ff:00:00:02`. wpg does not derive one address from the other, so if you change `target` you must change `ipv6.dst` and `ethernet2.dst` too (or send to the all-nodes group `ff02::1` / `33:33:00:00:00:01` as the sweep does).

Also note `hop_limit: 255`. Receivers drop NDP messages that arrive with any other value, so leaving the default of 64 produces packets that look right in Wireshark and do nothing.

## Step 3. Options are raw bytes

NDP options (`options`) are written as hex. `01 01 02 00 00 00 00 01` is type 1 (source link-layer address), length 1 (in 8-octet units), then the six-octet MAC. The Router Advertisement template chains three options:

```yaml
options: >-
  01 01 02 00 00 00 00 01
  05 01 00 00 00 00 05 dc
  03 04 40 c0 00 27 8d 00 00 09 3a 80 00 00 00 00
  20 01 0d b8 00 01 00 00 00 00 00 00 00 00 00 00
```

| Octets | Option |
|---|---|
| `01 01 02 00 00 00 00 01` | source link-layer address `02:00:00:00:00:01` |
| `05 01 00 00 00 00 05 dc` | MTU 1500 |
| `03 04 40 c0 00 27 8d 00 00 09 3a 80 00 00 00 00 20 01 0d b8 00 01 00 00 00 00 00 00 00 00 00 00` | prefix information: /64 (`40`), on-link + autonomous (`c0`), valid lifetime 2592000 s, preferred 604800 s, prefix `2001:db8:1::` |

Because the options are raw octets you can also build deliberately odd ones: a zero-length option, an option that runs past the end of the packet, or an unknown type.

## Step 4. Verify with Wireshark

```console
$ wpg render templates/protocols/l3/ndp-ns.wpg.yaml -o /tmp/ns.pcapng
$ tshark -r /tmp/ns.pcapng -V | sed -n '/Internet Control/,$p'
Internet Control Message Protocol v6
    Type: Neighbor Solicitation (135)
    Code: 0
    Checksum: 0x1c27 [correct]
    [Checksum Status: Good]
    Reserved: 00000000
    Target Address: 2001:db8::2
    ICMPv6 Option (Source link-layer address : 02:00:00:00:00:01)
```

`[correct]` is the important part: wpg computed the pseudo-header checksum itself.

## Step 5. Sweep it

`ndp-ns-sweep.wpg.yaml` puts an IPv6 range in the `target` generator:

```yaml
  - protocol: ndp_ns
    generators:
      target: 2001:db8::1-2001:db8::100
```

```console
$ wpg render templates/scenarios/ndp-ns-sweep.wpg.yaml -o /tmp/ns-sweep.pcapng
Wrote 256 packets to /tmp/ns-sweep.pcapng.
```

Since the destination is the all-nodes group, every host on the segment sees all 256 solicitations and answers only for the addresses it owns. A real host would send each one to that target's own solicited-node group; the sweep trades that for a single, simple template.

`ra-flood.wpg.yaml` shows random generators for the source MAC (`random_mac` with a fixed OUI) and the source link-local address, both reproducible via `stream.seed`.

## Step 6. Send it

```console
$ wpg send templates/scenarios/ndp-ns-sweep.wpg.yaml --backend null -i lo --rate 1000pps
$ wpg send templates/scenarios/ndp-ns-sweep.wpg.yaml -i eth1 --rate 1000pps
```

(Privileges: see [Permissions](../getting-started/permissions.md).) On the target, `ip -6 neigh`, `ndisc6` output or `show ipv6 neighbors` show what was learned, and `tcpdump -n -i eth1 icmp6` on another host shows the solicitations and any advertisements.

## Where other ICMPv6 types go

For a type that has no body protocol yet (for example MLD), set `icmpv6.type` explicitly and follow it with a `raw` layer that contains the four octets after the checksum and then the message. The same trick lets you send IPv6 extension-header chains, which have their own samples: `templates/protocols/l3/ipv6-hopopts`, `ipv6-dstopts`, `ipv6-routing`, `ipv6-frag` and `ipv6-ext-chain`.

## Next steps

* [Load and rate control](rate-control.md) for sending the RA flood at a chosen rate.
* [Protocol reference](../reference/protocols.md) for every IPv6 and ICMPv6 field.
