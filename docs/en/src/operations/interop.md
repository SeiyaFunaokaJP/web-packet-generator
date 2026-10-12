# Interop checks

Status: draft

The templates in `templates/interop/` check that a peer router accepts packets built by wpg. wpg is a stateless sender (no TCP sessions, no database synchronization), so each scenario checks one state change you can observe on the peer.

> **Verification status**: the seven scenarios below were checked against **FRR 8.4.4** with `scripts/interop/frr-netns.sh` (network namespace + veth, afpacket backend) on 2026-10-12, all PASS. The IOS-XR / Junos configuration examples and show commands come from general knowledge and have **not been checked on real devices or CML2 yet**. Please report differences in an issue.

## Scenarios

| Template | What wpg sends | Expected state on the peer |
|---|---|---|
| `ospfv2-hello-2way` | OSPFv2 Hellos listing the peer's router ID (priority 0), every 10 s | Neighbor `2-Way/DROther` (never Full) |
| `ripv2-response-route` | RIPv2 Responses with 198.51.100.0/24 (metric 1) and 203.0.113.0/25 (metric 5), every 30 s | R routes in the RIB (metric + 1) |
| `isis-l2-lan-hello-up` | L2 LAN IIHs listing the peer's MAC in the IS Neighbors TLV (priority 0, padded to the MTU), every 3 s | L2 adjacency `Up` |
| `vrrpv3-preempt` | VRRPv3 Advertisements with priority 200 (VRID 5), every 1 s | The peer is `Backup`; about 3.6 s after wpg stops it is `Master` again |
| `pim-hello-neighbor` | PIMv2 Hellos (105 s holdtime, DR priority 0), every 30 s | wpg in the PIM neighbor list |
| `ldp-link-hello` | LDP link Hellos (224.0.0.2, 15 s hold time), every 5 s | A `Link` entry in LDP discovery (no TCP session) |
| `bfd-session-up` | BFD control packets in state Down, then in state Up with the peer's discriminator, every 300 ms | BFD session `Up`; Down one detection time (about 0.9 s) after wpg stops |

By default wpg is `10.99.0.1` and the peer `10.99.0.2/24`; change them with `--var` (see each template's `variables`). Passing `--var src_mac=<MAC of the sending interface>` makes the peer's tables easier to read.

BFD takes two steps: send a few packets with `--var state=down` so the peer moves to `Init`, read the peer's local discriminator, then keep sending state Up with `--var peer_discriminator=<value>`.

## All checks against FRR on one Linux host

```sh
sudo apt install frr iproute2       # the frr service does not need to run
cargo build -p wpg-cli
sudo scripts/interop/frr-netns.sh   # runs every scenario and prints PASS / FAIL
```

The script creates the namespace `wpgfrr`, starts the FRR daemons (zebra, ospfd, ripd, isisd, vrrpd, pimd, ldpd, bfdd) directly behind a veth pair (`wpg0` ⇔ `frr0`), sends each template with `wpg send --backend afpacket` and checks the `vtysh` output. It cleans up on exit; with `--keep` the lab stays up and `sudo ip netns exec wpgfrr vtysh -N wpg` opens a shell in it (`--down` removes it).

Example output:

```text
PASS  OSPFv2 neighbor 2-Way
PASS  RIPv2 routes installed
PASS  IS-IS L2 adjacency Up
PASS  VRRPv3 peer is Backup
PASS  PIM neighbor
PASS  LDP link discovery
PASS  BFD session Up
```

The FRR configuration applied by the script is below. FRR's VRRP needs a macvlan `vrrp4-2-5` carrying the virtual MAC `00:00:5e:00:01:05`, which the script creates.

```text
router ospf
 ospf router-id 10.99.0.2
 network 10.99.0.0/24 area 0
router rip
 version 2
 network 10.99.0.0/24
router isis 1
 net 49.0001.0000.0000.0002.00
 is-type level-2-only
interface frr0
 ip router isis 1
 ip pim
 vrrp 5 version 3
 vrrp 5 priority 100
 vrrp 5 ip 10.99.0.254
mpls ldp
 router-id 10.99.0.2
 address-family ipv4
  discovery transport-address 10.99.0.2
  interface frr0
bfd
 peer 10.99.0.1 interface frr0
  receive-interval 300
  transmit-interval 300
```

## On CML2 or real devices

1. Connect the sending interface of the Ubuntu node running wpg and the router's interface to the same segment (prepare the node as in [CML2 notes](cml2.md) first).
2. Apply the configuration below to the router (adjust interface names).
3. For each scenario run `wpg send templates/interop/<name>.wpg.yaml -i <iface> --backend afpacket --var src_mac=<MAC> ...` and look at the state with the show commands. IS-IS needs `--var peer_mac=<router MAC>`; BFD needs the two steps above.

### Show commands

| Scenario | FRR | IOS-XR (unverified) | Junos (unverified) |
|---|---|---|---|
| OSPFv2 | `show ip ospf neighbor` | `show ospf neighbor` | `show ospf neighbor` |
| RIPv2 | `show ip route rip` | `show route rip` | `show route protocol rip` |
| IS-IS | `show isis neighbor` | `show isis adjacency` | `show isis adjacency` |
| VRRPv3 | `show vrrp 5` | `show vrrp` | `show vrrp` |
| PIM | `show ip pim neighbor` | `show pim neighbor` | `show pim neighbors` |
| LDP | `show mpls ldp discovery` | `show mpls ldp discovery` | `show ldp neighbor` |
| BFD | `show bfd peers brief` | `show bfd session` | `show bfd session` |

### IOS-XR configuration example (unverified)

```text
interface GigabitEthernet0/0/0/0
 ipv4 address 10.99.0.2 255.255.255.0
router ospf 1
 router-id 10.99.0.2
 area 0
  interface GigabitEthernet0/0/0/0
router rip
 interface GigabitEthernet0/0/0/0
router isis 1
 is-type level-2-only
 net 49.0001.0000.0000.0002.00
 address-family ipv4 unicast
 interface GigabitEthernet0/0/0/0
  address-family ipv4 unicast
router vrrp
 interface GigabitEthernet0/0/0/0
  address-family ipv4
   vrrp 5 version 3
    priority 100
    address 10.99.0.254
multicast-routing
 address-family ipv4
  interface GigabitEthernet0/0/0/0
   enable
router pim
 address-family ipv4
  interface GigabitEthernet0/0/0/0
mpls ldp
 router-id 10.99.0.2
 interface GigabitEthernet0/0/0/0
router static
 address-family ipv4 unicast
  192.0.2.0/24 10.99.0.1 bfd fast-detect minimum-interval 300 multiplier 3
```

### Junos configuration example (unverified)

```text
set interfaces ge-0/0/0 unit 0 family inet address 10.99.0.2/24 vrrp-group 5 virtual-address 10.99.0.254 priority 100
set interfaces ge-0/0/0 unit 0 family iso
set interfaces ge-0/0/0 unit 0 family mpls
set interfaces lo0 unit 0 family iso address 49.0001.0000.0000.0002.00
set protocols vrrp version-3
set protocols ospf area 0 interface ge-0/0/0.0
set protocols rip group wpg neighbor ge-0/0/0.0
set protocols isis interface ge-0/0/0.0 level 1 disable
set protocols pim interface ge-0/0/0.0
set protocols mpls interface ge-0/0/0.0
set protocols ldp interface ge-0/0/0.0
set routing-options static route 192.0.2.0/24 next-hop 10.99.0.1 bfd-liveness-detection minimum-interval 300 multiplier 3
```

## Limitations / out of scope

- **No stateful procedures** (a non-goal): OSPF never reaches Full (wpg stays a priority-0 DROther), there are no LDP / BGP TCP sessions, no IS-IS LSP flooding and no automatic BFD state machine. BGP needs TCP and is not covered.
- RIP routes time out 180 s after the last Response; keep sending to keep them.
- IPv6 scenarios (OSPFv3, RIPng, MLD, VRRPv3 for IPv6) are not included because IPv6 was disabled in the environment used for verification.
- The source MAC of the VRRP template is the virtual MAC `00:00:5e:00:01:<VRID>`; change `src` together with the VRID.
