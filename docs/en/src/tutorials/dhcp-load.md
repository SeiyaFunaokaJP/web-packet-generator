# Tutorial: Load-testing a DHCP server

Time: about 25 minutes. Privileges: none until Step 3; root from Step 4 (network namespace and afpacket).

## What you will build

A stream of DHCPDISCOVERs from many simulated clients, and a count of the DHCPOFFERs the server sends back, to find the rate the server can keep up with. Every frame changes the source MAC and `chaddr` (the client hardware address) together and uses a random transaction ID (`xid`). The server is dnsmasq running in a network namespace on the same Linux host.

wpg is stateless. It does not answer an OFFER with a DHCPREQUEST, so DORA (Discover / Offer / Request / Ack) never completes and no lease is created. What you measure is how fast the server handles DISCOVERs, not pool exhaustion (Step 6).

Only use this against DHCP servers you own or are authorised to test. A burst of DISCOVERs on a production segment can stop real clients from getting an address.

## Before you start

Build wpg and run from the repository root ([Installing on Linux](../getting-started/install-linux.md)). The template is `templates/scenarios/dhcp-discover-flood.wpg.yaml`; a single-frame example is `templates/protocols/app/dhcpv4-discover.wpg.yaml`. From Step 4 you need Linux, iproute2, dnsmasq (`sudo apt install dnsmasq-base`) and root.

## Step 1. Read the template

```yaml
variables:
  - name: oui
    default: "02:00:5e"

layers:
  - protocol: ethernet2
    fields:
      dst: "ff:ff:ff:ff:ff:ff"
    generators:
      src: { kind: range, start: "${oui}:00:00:01", end: "${oui}:ff:ff:ff" } # one MAC per client
  - protocol: ipv4
    fields:
      src: 0.0.0.0
      dst: 255.255.255.255
  - protocol: udp
    fields:
      src_port: 68
      dst_port: 67
  - protocol: dhcpv4
    fields:
      op: boot_request
      flags: broadcast
      options:
        - { message_type: discover }
        - { parameter_request_list: [1, 3, 6, 15, 51] } # mask, router, DNS, domain, lease
        - end
    generators:
      chaddr: { kind: expr, expr: "$0.src" } # the same MAC as the Ethernet source (layer 0)
      xid: { kind: expr, expr: "rand(0, 0xffffffff)" }

stream:
  seed: 1
  count: 10000

tx:
  rate: 200pps
```

Things worth knowing:

* **Source MACs are sequential.** `range` works on MAC addresses too and counts up from `02:00:5e:00:00:01`. The `02` first octet marks a locally administered address that cannot clash with real hardware. `--var oui=…` changes the upper three octets.
* **`chaddr` copies the source MAC through an expression.** In an `expr` generator, `$0.src` means "this frame's value of the `src` generator in layer 0 (ethernet2)". Servers usually identify a client by `chaddr`, so the two must not drift apart. The expression syntax is in [Template schema](../reference/template-schema.md#expressions-expr).
* **`xid` is random per frame.** `rand(min, max)` is derived from the frame index, so the same `stream.seed` gives the same values on every run.
* **Fields you do not write are Auto or defaults.** IPv4 length and checksum and UDP length and checksum (with the pseudo-header) are Auto; DHCP `htype` / `hlen`, the zero-filled `sname` and `file`, and the magic cookie come from defaults.

## Step 2. Look at the first frames

```console
$ wpg render templates/scenarios/dhcp-discover-flood.wpg.yaml -o /tmp/dhcp.pcapng --count 3
Wrote 3 packets to /tmp/dhcp.pcapng.
$ tshark -r /tmp/dhcp.pcapng
    1   0.000000      0.0.0.0 → 255.255.255.255 DHCP 293 DHCP Discover - Transaction ID 0xc4c98940
    2   0.000001      0.0.0.0 → 255.255.255.255 DHCP 293 DHCP Discover - Transaction ID 0xef7b0650
    3   0.000002      0.0.0.0 → 255.255.255.255 DHCP 293 DHCP Discover - Transaction ID 0x330355d7
$ tshark -r /tmp/dhcp.pcapng -o udp.check_checksum:TRUE -T fields -e eth.src -e dhcp.hw.mac_addr -e udp.checksum.status
02:00:5e:00:00:01	02:00:5e:00:00:01	1
02:00:5e:00:00:02	02:00:5e:00:00:02	1
02:00:5e:00:00:03	02:00:5e:00:00:03	1
```

The source MAC matches `Client MAC address`, and the UDP checksum is reported good (`1`). The same holds for all 10,000 frames:

```console
$ wpg render templates/scenarios/dhcp-discover-flood.wpg.yaml -o /tmp/dhcp-all.pcapng
Wrote 10000 packets to /tmp/dhcp-all.pcapng.
$ tshark -r /tmp/dhcp-all.pcapng -Y 'eth.src != dhcp.hw.mac_addr' | wc -l
0
```

## Step 3. Change the scale

`--count` sets the number of clients. Past the 10,000 of `stream.count` the MAC sequence starts over, so for more clients add a second stream with a different `--var oui=…`:

```console
$ wpg render templates/scenarios/dhcp-discover-flood.wpg.yaml -o /tmp/dhcp-b.pcapng --var oui=02:00:5f
Wrote 10000 packets to /tmp/dhcp-b.pcapng.
```

The source MACs in this file run from `02:00:5f:00:00:01` to `02:00:5f:00:27:10` (10000 = 0x2710).

## Step 4. Set up a DHCP server to test

Start dnsmasq in a namespace `wpgdhcp`, connected to wpg by a veth pair (`dh0` ⇔ `dhs0`), with a 50-address pool:

```sh
sudo ip netns add wpgdhcp
sudo ip link add dh0 type veth peer name dhs0
sudo ip link set dhs0 netns wpgdhcp
sudo ip link set dh0 up
sudo ip netns exec wpgdhcp ip link set lo up
sudo ip netns exec wpgdhcp ip link set dhs0 up
sudo ip netns exec wpgdhcp ip addr add 10.98.0.1/24 dev dhs0
sudo ip netns exec wpgdhcp dnsmasq --no-daemon --port=0 --interface=dhs0 --bind-interfaces \
    --dhcp-range=10.98.0.10,10.98.0.59,10m --dhcp-authoritative --dhcp-leasefile=/tmp/wpg-leases \
    --log-dhcp --log-facility=/tmp/wpg-dnsmasq.log &
```

`--port=0` turns DNS off. To test a real DHCP server or router, use the lab interface that reaches it instead of `dh0`.

## Step 5. Send and count the OFFERs

Count OFFERs (source port 67) with `wpg capture` while another terminal sends 100 DISCOVERs:

```console
$ sudo wpg capture -i dh0 -f "udp src port 67" --duration 3s --show 2 -w /tmp/offers.pcapng
$ sudo wpg send templates/scenarios/dhcp-discover-flood.wpg.yaml -i dh0 --backend afpacket --count 100
```

```console
Capturing on dh0 via afpacket, filter udp src port 67 (Ctrl-C stops)
#   0  16:14:33.696984775    342 B  ethernet2 / ipv4 / udp / dhcpv4
#   1  16:14:33.703225873    342 B  ethernet2 / ipv4 / udp / dhcpv4
state      finished (duration elapsed)
...
matched    100 pkts  34200 bytes
saved      100 pkts to /tmp/offers.pcapng
$ tshark -r /tmp/offers.pcapng -c 3
    1   0.000000    10.98.0.1 → 255.255.255.255 DHCP 342 DHCP Offer    - Transaction ID 0xc4c98940
    2   0.006241    10.98.0.1 → 255.255.255.255 DHCP 342 DHCP Offer    - Transaction ID 0xef7b0650
    3   0.009784    10.98.0.1 → 255.255.255.255 DHCP 342 DHCP Offer    - Transaction ID 0x330355d7
```

Each OFFER carries the `xid` of its DISCOVER (Step 2). The `broadcast` flag is set, so the OFFERs go to `255.255.255.255`.

**Your first run will probably look very slow.** By default dnsmasq sends an ICMP Echo to an address before offering it, to make sure it is unused. That costs about 3 seconds per DISCOVER, so a 3-second capture counts a single OFFER (the log `/tmp/wpg-dnsmasq.log` shows one `DHCPOFFER` every 3 seconds). The result above is with dnsmasq restarted with `--no-ping`. Finding limits like this one, set by how the server is built, is one of the points of this kind of test.

Now raise the rate to find where the OFFERs stop keeping up:

```sh
for r in 1kpps 5kpps 20kpps; do
  sudo wpg capture -i dh0 -f "udp src port 67" --duration 4s --show 0 > /tmp/cap-$r.txt &
  sudo wpg send templates/scenarios/dhcp-discover-flood.wpg.yaml -i dh0 --backend afpacket --count 2000 --rate $r
  wait; grep '^matched' /tmp/cap-$r.txt
done
```

Results in the development container (`--no-ping`); yours depend on the host and the server:

| Send rate | DISCOVERs | OFFERs | Answered |
|---|---|---|---|
| 1 kpps | 2000 | 2000 | 100% |
| 5 kpps | 2000 | 2000 | 100% |
| 20 kpps | 2000 | 1175 | 59% |

The rate at which the answered share starts to fall is a rough measure of the server's DISCOVER capacity. The unanswered DISCOVERs were dropped in the server's socket receive buffer. On real equipment, DHCP snooping or CoPP (control-plane policing) may drop them first, so check the switch or router counters too.

## Step 6. Read the results

* **An OFFER does not reserve the address.** The pool has 50 addresses, yet all 100 DISCOVERs were answered, with 50 distinct addresses offered (`tshark -r /tmp/offers.pcapng -T fields -e dhcp.ip.your | sort -u | wc -l`). dnsmasq, like most servers, creates the lease on REQUEST / ACK, so DISCOVERs alone do not exhaust a pool. The lease file (`/tmp/wpg-leases`) stays empty.
* **Tests that create leases are out of scope for wpg.** Answering an OFFER with a REQUEST is stateful, which is a non-goal. For pool exhaustion or lease-holding tests use a DHCP client emulator such as perfdhcp.
* **Through a relay**: start from `templates/protocols/app/dhcpv4-relay.wpg.yaml` (unicast, with `giaddr` and option 82). Set the destination to the server's address to test a server behind a router.
* For DHCPv6 there are `templates/protocols/app/dhcpv6-solicit.wpg.yaml` and `dhcpv6-relay-forward.wpg.yaml`.

Clean up:

```sh
sudo pkill -f 'dnsmasq --no-daemon --port=0 --interface=dhs0'
sudo ip netns del wpgdhcp      # removes dh0 as well
```

## Next steps

* Use `--ramp-up 30s` with the target `--rate` to raise the rate linearly and find where the answered share starts to fall. See [Load and rate control](rate-control.md).
* In the GUI, open this scenario from the Templates page, send it from a job, and count the OFFERs on the Receive page (`/rx`) with the filter `udp src port 67` ([Receive and verification](../operations/rx-verify.md)).
