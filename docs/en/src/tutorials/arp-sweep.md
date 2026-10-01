# Tutorial: ARP sweep

Time: about 15 minutes. Privileges: none until Step 5 (everything before it is a dry run).

## What you will build

A stream of ARP requests ("who has 10.0.0.x?") that walks every host address of a /22, and then the same sweep repeated inside several VLANs. ARP sweeps are the classic way to populate an ARP cache, to check that a switch floods broadcasts correctly, and to exercise a router's ARP glean rate limiting.

You will use three bundled templates:

| Template | Frames | What it shows |
|---|---|---|
| `templates/examples/arp-sweep.wpg.yaml` | 254 | the smallest sweep: one range generator |
| `templates/scenarios/arp-sweep-slash22.wpg.yaml` | 1022 | a CIDR block as the generator |
| `templates/scenarios/vlan-arp-sweep.wpg.yaml` | 1016 | two generators (VLAN x address) in `nested` mode |

## Before you start

Build wpg once (see [Install on Linux](../getting-started/install-linux.md), [macOS](../getting-started/install-macos.md) or [Windows](../getting-started/install-windows.md)) and run the commands from the repository root. The examples call the binary `wpg`; use `./target/release/wpg` (or `target\release\wpg.exe`) if it is not on your `PATH`. `tshark` is optional but makes the checks below more convincing.

Only send on networks you own or are authorised to test; see the [legal notice](../legal.md).

## Step 1. Read the smallest sweep

```yaml
layers:
  - protocol: ethernet2
    fields:
      dst: ff:ff:ff:ff:ff:ff
      src: ${src_mac}

  - protocol: arp
    fields:
      oper: request
      sha: ${src_mac}
      spa: ${src_ip}
      tha: 00:00:00:00:00:00
    generators:
      tpa: ${first}-${last}   # shorthand for a range generator
```

Two things do the work. `generators.tpa` varies the ARP target protocol address across the stream, and `${...}` placeholders are template variables (declared in the `variables:` list with defaults) that you can override at run time with `--var name=value`. `ethertype` in the Ethernet layer is not written: it is Auto and becomes `0x0806` because the next layer is ARP.

Print the first frame:

```console
$ wpg template show templates/scenarios/vlan-arp-sweep.wpg.yaml
Hex dump (60 bytes)
0000  ff ff ff ff ff ff 02 00  00 00 00 01 81 00 00 64  ...............d
0010  08 06 00 01 08 00 06 04  00 01 02 00 00 00 00 01  ................
0020  0a 00 00 fa 00 00 00 00  00 00 0a 00 00 01 00 00  ................
0030  00 00 00 00 00 00 00 00  00 00 00 00              ................
...
```

The last 14 octets are zero padding: an ARP request is 42 octets long and Ethernet needs 60 (the FCS is added by the NIC).

## Step 2. Sweep a whole /22

`arp-sweep-slash22.wpg.yaml` swaps the range for a CIDR block held in a variable:

```yaml
variables:
  - name: net
    default: 10.0.0.0/22
layers:
  - protocol: arp
    generators:
      tpa: ${net}
```

A CIDR generator yields the host addresses only, so a /22 gives 1022 frames (`10.0.0.1` to `10.0.3.254`). Render it to a pcapng file without touching the network:

```console
$ wpg render templates/scenarios/arp-sweep-slash22.wpg.yaml -o /tmp/sweep.pcapng
Wrote 1022 packets to /tmp/sweep.pcapng.
```

Point the same template at another subnet without editing it:

```console
$ wpg render templates/scenarios/arp-sweep-slash22.wpg.yaml -o /tmp/sweep.pcapng --var net=10.1.0.0/22
```

## Step 3. Repeat the sweep in several VLANs

`vlan-arp-sweep.wpg.yaml` inserts an 802.1Q tag and adds a second generator:

```yaml
  - protocol: dot1q
    generators:
      vid: ${vids}          # default 100-103

  - protocol: arp
    generators:
      tpa: ${first}-${last} # default 10.0.0.1-10.0.0.254

stream:
  mode: nested
  order: [dot1q.vid, arp.tpa] # slowest first
```

With two generators you have to say how they combine. `zip` advances both in lockstep, `product` is the Cartesian product, and `nested` is a product with an explicit order: here the VLAN is the slow loop, so all 254 addresses are asked in VLAN 100, then in VLAN 101, and so on: 4 x 254 = 1016 frames. Restrict it to two VLANs with a list:

```console
$ wpg render templates/scenarios/vlan-arp-sweep.wpg.yaml -o /tmp/vlan-sweep.pcapng --var vids=100,200
Wrote 508 packets to /tmp/vlan-sweep.pcapng.
```

## Step 4. Check the frames

```console
$ tshark -r /tmp/vlan-sweep.pcapng -T fields -e vlan.id -e arp.dst.proto_ipv4 | sed -n '1p;254p;255p;$p'
100	10.0.0.1
100	10.0.0.254
200	10.0.0.1
200	10.0.0.254
```

Also run `wpg template lint` on any template you edit: it catches unknown fields, out-of-range values and bad generator syntax before you send.

## Step 5. Send it

First rehearse with a backend that writes a file (for `pcapfile` the `-i` argument is the output path) or discards the frames (`null`):

```console
$ wpg send templates/scenarios/arp-sweep-slash22.wpg.yaml --backend null -i lo --rate 100kpps
Sending on lo via null: rate 100000pps, burst 1, 1022 packets, 1 worker(s), plan PreRender (...)
state      finished (stream exhausted)
packets    1022
bytes      61320
```

Then send for real on your lab interface. Raw sending needs privileges (see [Permissions](../getting-started/permissions.md)); run `wpg ifaces` to pick the interface and `wpg doctor` if the backend is refused:

```console
$ wpg ifaces
$ wpg send templates/scenarios/vlan-arp-sweep.wpg.yaml -i eth1 --rate 2000pps
```

The command runs the stream once (1016 frames at 2000 frames per second is about half a second) and prints a summary. Add `--count 10000` to loop the sweep, or `--duration 30s` to run for a fixed time.

## Step 6. Read the result

* On the device under test, look at the ARP or neighbour table (`ip neigh`, `show ip arp`) and at the MAC table of the switch (the sender MAC `02:00:00:00:00:01` should appear on the port and in the VLANs you used).
* On a Linux host in the same segment, `tcpdump -e -n -i eth1 arp` shows the requests with their VLAN tags.
* The sweep uses a made-up sender address (`10.0.0.250`). Change `src_mac` and `src_ip` with `--var` so that replies come back to an address you can observe.

## Next steps

* [VLAN-tagged TCP SYN flood](vlan-syn-flood.md) for random source addresses and TCP options.
* [Load and rate control](rate-control.md) for `--rate`, `--duration` and multi-worker sending.
* [Template schema](../reference/template-schema.md) for every generator kind.
