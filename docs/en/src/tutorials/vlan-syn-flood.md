# Tutorial: VLAN-tagged TCP SYN flood

Time: about 20 minutes. Privileges: none until Step 5.

## What you will build

A stream of 100,000 TCP SYN segments in an 802.1Q VLAN, each with a random source address, source port and initial sequence number, and the usual SYN options (MSS, SACK-permitted, window scale). It is the standard way to load a firewall, load balancer or connection-tracking table from many "clients" while you watch the device's SYN handling. wpg is stateless: it never completes a handshake and does not answer SYN-ACKs.

Use it only against equipment in a lab that you own or are authorised to test.

## Before you start

Build wpg and run from the repository root (see [Install on Linux](../getting-started/install-linux.md)). The template is `templates/scenarios/vlan-syn-flood.wpg.yaml`; a smaller relative is `templates/examples/vlan-tcp-syn.wpg.yaml`.

## Step 1. Read the template

```yaml
layers:
  - protocol: ethernet2
    fields: { dst: ${dst_mac}, src: ${src_mac} }

  - protocol: dot1q                 # TPID 0x8100 goes into ethernet2 automatically
    fields:
      vid: ${vid}

  - protocol: ipv4
    fields:
      dst: ${dst_ip}
      ttl: 64
      flags: df
    generators:
      src: { kind: random, min: 10.0.0.1, max: 10.255.255.254 }

  - protocol: tcp
    fields:
      dst_port: ${dst_port}
      flags: syn
      window: 64240
      options:
        - mss: 1460
        - sack_permitted
        - ws: 7
    generators:
      src_port: { kind: random, min: 1024, max: 65535 }
      seq: { kind: random, min: 0, max: 4294967295 }

stream:
  seed: 7
  count: 100000

tx:
  rate: 50kpps
  burst: 32
```

Points worth knowing:

* **Nothing else is written.** IPv4 `ihl`, `total_length`, `protocol` and `header_checksum`, TCP `data_offset` and `checksum` (with the IPv4 pseudo header), and the dot1q/Ethernet EtherTypes are all Auto and are recomputed for every frame. The TCP options region is padded to a 32-bit boundary automatically.
* **Options are typed.** `mss: 1460`, `sack_permitted`, `ws: 7`, `nop` and `timestamps: { tsval: 1000, tsecr: 0 }` are the built-in names; anything else can be written as `{ type: 99, data: "01 02" }`.
* **`stream.seed` makes random reproducible.** The same seed produces the same 100,000 frames on every run and on every machine.
* **`tx.rate` and `tx.burst`** are only defaults; `wpg send --rate` overrides them.

## Step 2. Look at the first frames

```console
$ wpg render templates/scenarios/vlan-syn-flood.wpg.yaml -o /tmp/syn.pcapng --count 3
Wrote 3 packets to /tmp/syn.pcapng.
$ tshark -r /tmp/syn.pcapng -o tcp.check_checksum:TRUE
    1   0.000000 10.114.228.243 → 198.51.100.7 TCP 70 2311 → 443 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM WS=128
    2   0.000001 10.99.228.10 → 198.51.100.7 TCP 70 51321 → 443 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM WS=128
    3   0.000002 10.251.163.120 → 198.51.100.7 TCP 70 10483 → 443 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM WS=128
```

Wireshark reports no bad checksum. (`Seq=0` is the relative sequence number Wireshark displays; the raw values are random.) `tshark -V -c 1` shows the 802.1Q header with `ID: 100`.

## Step 3. Change the target without editing the file

All the `${...}` placeholders are variables; their defaults are in the `variables:` list at the top of the file. Override any of them on the command line:

```console
$ wpg render templates/scenarios/vlan-syn-flood.wpg.yaml -o /tmp/syn.pcapng \
      --var vid=200 --var dst_ip=203.0.113.9 --var dst_port=8443 --count 1000
```

Use `--var src_mac=…` and `--var dst_mac=…` to set the MAC addresses; the next hop's MAC on the sending interface is what the device under test expects as the destination.

## Step 4. Understand `count`, `seed` and the pre-render plan

The stream has a fixed length (`stream.count: 100000`). `wpg send` pre-renders small streams into memory (limit 256 MiB by default) and expands large ones on the fly; force either with `--plan prerender` or `--plan onthefly`.

## Step 5. Send it

Rehearse first, then send on the lab interface (see [Permissions](../getting-started/permissions.md)):

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --rate 20kpps --count 5000
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml -i eth1 --rate 50kpps \
      --var vid=200 --var dst_ip=203.0.113.9 --var dst_mac=00:11:22:33:44:55
```

Start gently and climb: `--ramp-up 10s` increases the rate linearly from zero, which is kind to devices that fall over at the first burst. See [Load and rate control](rate-control.md).

## Step 6. Read the result

* Watch the connection or SYN counters on the firewall or load balancer (`show conn count`, `conntrack -C`, SYN-cookie counters).
* On a Linux target, `nstat -az TcpExtTCPReqQFullDoCookies` and `ss -s` show what the SYN queue did.
* On the sending side, the summary at the end of `wpg send` gives packets, bytes, dropped and the achieved rate compared to the target.

## Next steps

* Replace `flags: syn` with `syn|ack`, `rst`, or `fin|ack` to test how a stateful device treats out-of-state segments.
* Use `templates/scenarios/tcp-syn-port-scan.wpg.yaml` to scan destination ports 1-1024 instead.
* [pcap to template replay](pcap-replay.md) turns a real capture into a template you can edit.
