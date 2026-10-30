# DHCP server

Status: draft

wpg includes a DHCP server for test networks (the "DHCP" screen of the web UI; API [`/api/dhcp`](../reference/api.md)). Connect the wpg host directly to the device under test (router, CPE, host) and hand it addresses and prefixes. The design is in [ADR-0012](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/adr/0012-dhcp-server.md) (Japanese).

- DHCPv4: address pool, subnet mask, routers, DNS, domain name, NTP, reservations (MAC → IP), relays (giaddr / option 82)
- DHCPv6: addresses (IA_NA), prefix delegation (IA_PD), DNS servers / domain search list, Rapid Commit
- Router advertisements (RA): the M / O flags that make hosts use DHCPv6, an optional on-link prefix
- Static interface addresses (temporary from the screen, persistent with netplan at deploy time)
- The lease table and an activity log

Linux only.

## Choosing the interface

The server answers on **one interface only** (its sockets are bound to it with `SO_BINDTODEVICE`); the management interface never sees DHCP.

Normally the interface is fixed at deploy time:

```sh
scripts/deploy.sh user@host --service --iface ens3 \
    --dhcp-iface ens3 --dhcp-iface-addr 192.168.100.1/24,2001:db8:1::1/64
```

- `--dhcp-iface ens3` writes `WPG_DHCP__IFACE=ens3` into `/etc/wpg/wpg.env`; the screen can no longer change the interface. The service gets a `CAP_NET_BIND_SERVICE` drop-in (ports 67 / 547).
- `--dhcp-iface-addr` gives that interface static addresses: it writes `/etc/netplan/90-wpg-ens3.yaml` and runs `netplan apply` (DHCP client and RA acceptance are turned off on it). An interface carrying the default route is never touched. To undo, delete the file and run `sudo netplan apply`.

From Windows: `deploy.ps1 -DhcpIface ens3 -DhcpIfaceAddr 192.168.100.1/24`. In answer files: `dhcp_iface=` / `dhcp_iface_addr=`.

It can also be set in `wpg.toml`:

```toml
[dhcp]
enabled = true
iface = "ens3"
```

With an empty `iface`, the screen offers the interfaces in `tx.allowed_ifaces`.

## The screen

### Status

The interface, the DHCPv4 server identifier, the DHCPv6 server DUID, the addresses wpg added, and packet counters. "Save and start" in the header saves the settings and starts the server (restarts it when running); "Stop" stops it. A server running when wpg stops starts again with wpg.

### Settings

**Interface addresses (static IP)**: addresses such as `192.168.100.1/24, 2001:db8:1::1/64` are added to the interface when the server starts (the link is brought up too) and the ones wpg added are removed when it stops. Use `--dhcp-iface-addr` at deploy time for addresses that survive a reboot. The first IPv4 address becomes the DHCPv4 server identifier (the "server identifier" field overrides it).

**DHCPv4**: range start / end, subnet mask, lease time (T1 / T2 are 1/2 and 7/8 of it), routers, DNS servers, domain name, NTP servers. Lists are separated by commas or spaces. Reservations are one `MAC IP` per line (the address may be outside the range).

**DHCPv6**: "Addresses (IA_NA)" hands out one address of the range per IA; "Prefix delegation (IA_PD)" cuts the pool prefix (e.g. `2001:db8:100::` / 40) into prefixes of the delegated length (e.g. 56). Also the preferred and valid lifetimes (T1 / T2 are 1/2 and 4/5 of the preferred lifetime), DNS servers, domain search list, the ADVERTISE preference and Rapid Commit.

**Router advertisements**: hosts only use DHCPv6 when an RA sets the M flag (addresses) or the O flag (other configuration). Turn on "Send router advertisements" when no router on the link sends them. M is set when IA_NA is on, O always. An on-link prefix adds a Prefix Information option, with the A flag when "SLAAC" is checked. A router lifetime of 0 does not make wpg a default router.

### Leases

Bound, offered and declined (DECLINE: reported in use) leases. Deleting one only removes wpg's record (the client keeps the address until it renews). Expired leases disappear on their own.

### Activity

Requests and replies (`DISCOVER → OFFER`, `SOLICIT → ADVERTISE`, RAs sent, ...) and why nothing was sent (pool exhausted, another server selected, unknown server identifier), newest first.

## Permissions

| Action | Needs |
|---|---|
| Listening on ports 67 / 547 | `CAP_NET_BIND_SERVICE` |
| Sending RAs (raw ICMPv6) | `CAP_NET_RAW` |
| Adding interface addresses | `CAP_NET_ADMIN` |

The systemd service installed by deploy.sh has all of them (the `bind-service.conf` drop-in is added with `--dhcp-iface` or when an SNMP trap listener is configured). When running wpg by hand: `sudo setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip /usr/local/bin/wpg`.

## Limitations

- Another DHCP server on the same link conflicts with wpg. Use it on test interfaces only.
- DHCPv4 replies go to the relay when relayed, by unicast to renewing clients, and by broadcast otherwise.
- DHCPv6 relay messages (RELAY-FORW) are not answered.
- There are no static DHCPv6 assignments (DUID → address).
