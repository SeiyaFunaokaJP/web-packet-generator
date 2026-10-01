# Tutorial: pcap to template replay

Time: about 20 minutes. Privileges: none until the last step.

## What you will build

A round trip: take a capture (here one produced by wpg so the commands are reproducible, but any Ethernet pcap or pcapng works), convert it into an editable template, and render it back to the same bytes. Along the way wpg recognises which fields vary between packets and turns them into generators.

`wpg import` reads pcap (both byte orders, microsecond and nanosecond) and pcapng, Ethernet link type only, and assumes frames carry no FCS. It can also read pasted hex.

## Before you start

Build wpg and run from the repository root. `tshark` is only used for checking.

## Step 1. Make a capture

```console
$ wpg render templates/examples/arp-sweep.wpg.yaml -o /tmp/arp.pcapng
Wrote 254 packets to /tmp/arp.pcapng.
```

In real life this file would come from `tcpdump -i eth0 -w capture.pcap` or Wireshark.

## Step 2. Import it

```console
$ wpg import /tmp/arp.pcapng -o /tmp/arp.wpg.yaml
Imported 254 packet(s) into 1 template(s): /tmp/arp.wpg.yaml
```

The result:

```yaml
schema: 1
meta:
  name: arp
  description: Imported from /tmp/arp.pcapng (254 packets)
  ...
  tags:
  - imported
layers:
- protocol: ethernet2
  fields:
    dst: ff:ff:ff:ff:ff:ff
    src: 02:00:00:00:00:01
    ethertype: auto
- protocol: arp
  fields:
    htype: ethernet
    ptype: ipv4
    hlen: 6
    plen: 4
    oper: request
    sha: 02:00:00:00:00:01
    spa: 192.168.1.10
    tha: 00:00:00:00:00:00
  generators:
    tpa:
      kind: range
      start: 192.168.1.1
      end: 192.168.1.254
      step: 1
- protocol: raw
  fields:
    data: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
stream:
  mode: zip
  count: 254
```

Three things happened:

1. **Fields are in human notation** (`192.168.1.10`, `request`, `ethernet`), and fields that wpg can recompute were put back to `auto` only where doing so reproduces the captured bytes. A capture with a wrong checksum or length keeps that field as a manual value, so deliberately malformed traffic survives the trip.
2. **Generator inference.** All 254 frames have the same layer stack, and `arp.tpa` changes by +1 each time, so it became a `range` generator (an exact one). Other patterns are recognised too: a repeating cycle becomes `list`, a small set of values becomes an approximate `list`, and anything else becomes a `random` generator over the observed minimum and maximum (wpg warns when the result is approximate). Use `--no-detect-generators` to keep only the first packet's values.
3. **The 18 zero octets** after ARP are the Ethernet padding that was in the capture. The importer keeps bytes it cannot attribute to a protocol in a `raw` layer so the frame is reproduced exactly. You may delete that layer: with `packet.pad_to_min` (the default) wpg pads the frame itself.

## Step 3. Lint and render again

```console
$ wpg template lint /tmp/arp.wpg.yaml
/tmp/arp.wpg.yaml: no issues
0 error(s), 0 warning(s) in 1 file(s)
$ wpg render /tmp/arp.wpg.yaml -o /tmp/arp2.pcapng
Wrote 254 packets to /tmp/arp2.pcapng.
$ diff <(tshark -r /tmp/arp.pcapng -x) <(tshark -r /tmp/arp2.pcapng -x) && echo identical
identical
```

The frames are byte-for-byte the same (timestamps differ, since wpg writes its own).

## Step 4. Edit and reuse

Because it is now a template, you can change anything:

* Turn constants into variables: replace `192.168.1.10` by `${src_ip}` and add `variables: [{ name: src_ip, default: 192.168.1.10 }]`, then run `wpg render … --var src_ip=10.9.8.7`.
* Widen the sweep: change the `range` to `192.168.0.1-192.168.7.254`.
* Set `oper: reply` to turn the capture into replies.
* Add `packet: { fcs: bad }` to see how a device treats a bad FCS.

## Step 5. One template per packet, or from pasted hex

For a capture of a conversation rather than a sweep, ask for one template per packet:

```console
$ wpg import /tmp/capture.pcap --per-packet /tmp/templates/
```

A packet copied from Wireshark ("Copy → as a Hex Stream"), a `xxd`/`hexdump -C` listing or plain spaced bytes can be piped in:

```console
$ echo "ff ff ff ff ff ff 02 00 00 00 00 01 08 06 00 01 08 00 06 04 00 01 02 00 00 00 00 01 c0 00 02 01 00 00 00 00 00 00 c0 00 02 02" \
    | wpg import --hex - -o -
```

For captures that start with something other than Ethernet II, use `--first-protocol`. Frames that use a protocol wpg does not know are kept as `raw` bytes after the last layer it could decode, so they still replay.

## Step 6. Share and replay

Pack the template and the original capture into a bundle that a colleague can import:

```console
$ wpg template export /tmp/arp.wpg.yaml -o /tmp/arp.wpgbundle --attach /tmp/arp.pcapng
$ wpg template import /tmp/arp.wpgbundle -o ./my-templates
```

Send the template like any other, with a rate of your choice:

```console
$ wpg send /tmp/arp.wpg.yaml -i eth1 --rate 500pps
```

See [Load and rate control](rate-control.md) for the options.

## Next steps

* [Template schema](../reference/template-schema.md) for everything you can put in the file.
* [ARP sweep](arp-sweep.md), [VLAN-tagged TCP SYN flood](vlan-syn-flood.md) and [IPv6 and Neighbor Discovery](ipv6-ndp.md).
