# Tutorial: Bringing up an IS-IS adjacency with Hellos

Time: about 30 minutes. Privileges: none until Step 2; root from Step 3 (network namespace and afpacket).

## What you will build

An IS-IS Level 2 LAN Hello (IIH), sent to FRR's isisd, and the peer's view of the adjacency going from `Initializing` to `Up`. The template is the interop check `templates/interop/isis-l2-lan-hello-up.wpg.yaml`. It is an OSI stack without IP, with LLC (`0xfe`) and IS-IS directly on Ethernet, and it shows the PDU length, padding up to the MTU, and the two-way check through the IS Neighbors TLV.

wpg is stateless. Keep sending Hellos and the adjacency stays up, but wpg does not flood LSPs or act as the DIS.

Only use this against routers you own or are authorised to test. A fake adjacency in a production IS-IS domain can affect route computation.

## Before you start

Build wpg and run from the repository root ([Installing on Linux](../getting-started/install-linux.md)). From Step 3 you need Linux, iproute2, FRR (`sudo apt install frr`; the script starts the daemons itself, so the `frr` service can stay stopped) and root. The IS-IS PDUs and TLVs are listed in [IS-IS (P2)](../protocols/isis.md).

## Step 1. Read the template

```yaml
layers:
  - protocol: dot3
    fields:
      dst: "01:80:c2:00:00:15" # AllL2ISs
      src: ${src_mac}
      length: auto
  - protocol: llc
  - protocol: isis_l2_lan_hello
    fields:
      circuit_type: l2
      source_id: ${system_id}
      holding_time: 30
      priority: 0 # wpg does not flood LSPs, so let the peer be the DIS
      lan_id: "0000 0000 0001 01" # overwritten by the DIS's LAN ID once the peer elects itself
      # Manual PDU length: Padding TLVs fill the PDU up to it (MTU 1500 - LLC 3).
      pdu_length: 1497
      tlvs:
        - { protocols_supported: [ipv4] }
        - { area_addresses: [{ address: "${area}" }] }
        - { ip_interface_addresses: ["${ip}"] }
        - { is_neighbors: ["${peer_mac}"] }

tx:
  count: 40 # two minutes of Hellos
  rate: ifg=3s
```

Things worth knowing:

* **IS-IS does not use IP.** With `llc` on top of `dot3` (802.3 with a length field), DSAP / SSAP become `0xfe` (OSI) automatically, and the IS-IS NLPID `0x83` comes from the PDU layer. `01:80:c2:00:00:15` is the all-Level-2-ISs address (AllL2ISs).
* **A Manual `pdu_length` adds padding.** Left Auto, the PDU is as long as its TLVs. With a value, Padding TLVs (type 8) are added until the PDU is exactly that long. 1497 is the Ethernet MTU of 1500 minus the 3-octet LLC header. A Hello that arrives at that size tells the peer that the MTUs match.
* **`is_neighbors` is the two-way check.** A LAN adjacency only comes `Up` once a router sees its own MAC (SNPA) in the other side's Hello. Set `peer_mac` to the MAC of the peer's interface.
* **`priority: 0`** makes wpg lose the DIS (designated IS) election, because it does not do the DIS's work (CSNPs, pseudonode LSP).
* The System ID (`source_id`) is 6 octets; area addresses use NSAP notation (`49.0001`).

## Step 2. Look at the frame

```console
$ wpg render templates/interop/isis-l2-lan-hello-up.wpg.yaml -o /tmp/isis.pcapng --count 1
Wrote 1 packets to /tmp/isis.pcapng.
$ tshark -r /tmp/isis.pcapng
    1   0.000000 02:00:00:00:00:01 → ISIS-all-level-2-IS's ISIS HELLO 1514 L2 HELLO, System-ID: 0000.0000.0001
$ tshark -r /tmp/isis.pcapng -V
...
ISIS HELLO
    .... ..10 = Circuit type: Level 2 only (0x2)
    SystemID {Sender of PDU}: 0000.0000.0001
    Holding timer: 30
    PDU length: 1497
    .000 0000 = Priority: 0
    SystemID {Designated IS}: 0000.0000.0001.01
    Protocols Supported (t=129, l=1)
    Area address(es) (t=1, l=4)
        Area address (3): 49.0001
    IP Interface address(es) (t=132, l=4)
        IPv4 interface address: 10.99.0.1
    IS Neighbor(s) (t=6, l=6)
        IS Neighbor: 02:00:00:00:00:02 (02:00:00:00:00:02)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=255)
    Padding (t=8, l=160)
```

(The `-V` output is abridged.) The frame is 1514 octets (MTU 1500 plus the 14-octet Ethernet header). The 1497-octet PDU is the 27-octet header, 23 octets for the four TLVs above, and six Padding TLVs (five of 255 octets and one of 160, each plus 2 octets of type and length: 1447).

## Step 3. Set up the FRR lab

`scripts/interop/frr-netns.sh` starts FRR in a namespace `wpgfrr` connected to wpg by a veth pair (`wpg0` ⇔ `frr0`). FRR's IS-IS is `net 49.0001.0000.0000.0002.00`, `is-type level-2-only`. With `--keep` the lab stays up after the interop checks.

```console
$ cargo build --release -p wpg-cli
$ sudo scripts/interop/frr-netns.sh --keep target/release/wpg
...
PASS  IS-IS L2 adjacency Up
...
lab kept: ip netns exec wpgfrr vtysh -N wpg; tear down with scripts/interop/frr-netns.sh --down
```

The script's own check has already brought the adjacency up once. Wait until it expires, one holding time (30 s) after the last Hello, before you go on:

```console
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor"
Area 1:
  System Id           Interface   L  State        Holdtime SNPA
```

Note both MACs (veth MACs change every time the pair is created):

```sh
SRC=$(cat /sys/class/net/wpg0/address)
PEER=$(sudo ip netns exec wpgfrr cat /sys/class/net/frr0/address)
```

## Step 4. Receive the peer's Hello

FRR sends its own Hello every 3 seconds. Receive one with `wpg capture` to check that wpg's parser reads it:

```console
$ sudo wpg capture -i wpg0 -f "ether dst 01:80:c2:00:00:15" --count 1 --show 1
Capturing on wpg0 via afpacket, filter ether dst 01:80:c2:00:00:15 (Ctrl-C stops)
#   0  16:10:42.763122508   1514 B  dot3 / llc / isis_l2_lan_hello
state      finished (count reached)
...
```

FRR's Hello is also padded to 1514 octets and parses as `dot3 / llc / isis_l2_lan_hello`. Save it with `-w /tmp/frr.pcapng` to turn it into a template ([pcap to template replay](pcap-replay.md)).

## Step 5. Bring the adjacency up

First send one Hello without the peer's MAC. `peer_mac` keeps its default `02:00:00:00:00:02`, so to FRR this is a neighbor that does not know it yet:

```console
$ sudo wpg send templates/interop/isis-l2-lan-hello-up.wpg.yaml -i wpg0 --backend afpacket \
      --var src_mac=$SRC --count 1
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor"
Area 1:
  System Id           Interface   L  State        Holdtime SNPA
 0000.0000.0001      frr0        2  Initializing  29       a63f.1a8d.7239
```

FRR has found wpg, but without the two-way check the adjacency stays `Initializing`. Now send with FRR's MAC in `peer_mac`:

```console
$ sudo wpg send templates/interop/isis-l2-lan-hello-up.wpg.yaml -i wpg0 --backend afpacket \
      --var src_mac=$SRC --var peer_mac=$PEER --count 2 --rate 1pps
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor"
Area 1:
  System Id           Interface   L  State        Holdtime SNPA
 0000.0000.0001      frr0        2  Up            30       a63f.1a8d.7239
$ sudo ip netns exec wpgfrr vtysh -N wpg -c "show isis neighbor detail"
Area 1:
 0000.0000.0001
    Interface: frr0, Level: 2, State: Up, Expires in 30s
    Adjacency flaps: 1, Last: 1s ago
    Circuit type: L2, Speaks: IPv4
    SNPA: a63f.1a8d.7239, LAN id: 0000.0000.0001.01
    LAN Priority: 0, unknown DIS state, DIS flaps: 0, Last: 2s ago
    Area Address(es):
      49.0001
    IPv4 Address(es):
      10.99.0.1
```

FRR sees the template's TLVs as written: the area, the IPv4 address, priority 0 and the 30-second holding time. To keep the adjacency, send Hellos more often than the holding time. The template's default (without `--count`) is 40 Hellos 3 seconds apart, two minutes in all. Stop sending and the adjacency is gone 30 seconds later.

## Step 6. Variations

* **A different area**: sending with `--var area=49.0002 --var "system_id=0000 0000 0003"` still brings the adjacency `Up`. Only Level 1 adjacencies need a matching area address; Level 2 adjacencies cross areas (ISO 10589). To try Level 1, change `isis_l2_lan_hello` to `isis_l1_lan_hello` and the destination to `01:80:c2:00:00:14` (AllL1ISs) in the template, and set FRR to `is-type level-1`.
* **MTU mismatch**: put `pdu_length` back to Auto (delete the line) and the Hello is short and unpadded. FRR does not require padded Hellos from its neighbors by default, so the adjacency still comes up, but implementations that insist on padding will not form it. Use this to compare devices.
* **Authentication**: `templates/protocols/routing/isis-l2-lan-hello.wpg.yaml` carries clear-text authentication (TLV 10). wpg does not compute HMAC-MD5 values; enter them by hand.
* **Other PDUs**: samples of LSPs, CSNPs, PSNPs and point-to-point Hellos are in `templates/protocols/routing/isis-*.wpg.yaml`. The LSP checksum (Fletcher) is Auto.

Clean up:

```sh
sudo scripts/interop/frr-netns.sh --down
```

## Next steps

* The procedure and configuration examples for real routers (IOS-XR, Junos) and routers on CML2 are in [Interop checks](../operations/interop.md).
* The IPv6 counterpart of the same ideas (several layers whose type, length and checksum fields are derived from each other) is [IPv6 and Neighbor Discovery](ipv6-ndp.md).
