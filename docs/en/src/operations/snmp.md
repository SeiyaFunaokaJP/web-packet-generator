# SNMP tool

Status: draft

Besides generating packets, wpg contains an SNMP manager (the "SNMP" screen of the web UI; API [`/api/snmp`](../reference/api.md)). Reading device counters while you load a link, or checking that a trap is sent, no longer needs a second tool. See [ADR-0010](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/adr/0010-snmp-manager.md) for the design.

- Get / GetNext / GetBulk / Walk / Set (SNMPv1 / v2c / v3)
- SNMPv3 USM: noAuthNoPriv / authNoPriv / authPriv, authentication MD5 / SHA-1 / SHA-224 / SHA-256 / SHA-384 / SHA-512, privacy DES / AES-128 / AES-192 / AES-256; the agent's engine ID is discovered automatically
- sending and receiving traps / informs
- MIB upload, and readable values using names, enumerations, display hints and units
- pcap / pcapng recording of the exchanged SNMP datagrams

## The screen

The screen has three tabs; the "Record pcap" switch in the header applies to all of them.

### Operations

1. Fill in the target (host, port, version; the community for v1 / v2c; user, security level, protocols and pass phrases for v3). "Save profile" keeps a target in the browser. **Communities and pass phrases are never stored, neither in profiles nor on the server.**
2. Type object names or OIDs; names from the loaded MIBs are completed (`ifDescr.1`, `IF-MIB::ifTable`, `sysUpTime.0`, `1.3.6.1.2.1.1`). Separate several with spaces. "Pick from the MIB tree" opens the tree (whole, or one module) with check boxes that add OIDs to the field and remove them; scalars get `.0`.
3. Press Get / GetNext / GetBulk / Walk. Walk uses GetBulk on v2c / v3 and GetNext on v1 and stops at the end of the subtree, at `endOfMibView` or at an OID that does not increase.

Results are shown as name / type / value, formatted with the MIBs:

| Raw | Shown |
|---|---|
| `ifOperStatus` 1 | `up(1)` |
| MacAddress (`1x:`) | `00:1b:21:aa:bb:cc` |
| DateAndTime | `2026-10-27,13:5:9.4,+9:0` |
| TimeTicks 12345678 | `1d 10:17:36.78 (12345678)` |
| `hrMemorySize` 8048 | `8048 KBytes` (UNITS) |
| OID value | `IF-MIB::ifIndex` |

"Raw values" shows the values without MIB knowledge. When a walk contains a conceptual table (ifTable, ...) it is also shown as rows × columns keyed by its INDEX ("Table view").

"Auto refresh" repeats the last read operation at the chosen interval. Counter32 / Counter64 values get a per-second rate from the previous poll and a sparkline (Counter32 wraps are handled).

Set takes rows of OID / type / value. Type `auto` takes the type from the MIB and accepts enumeration labels such as `down`; for objects no MIB knows, choose a type (net-snmp's `i` `u` `c` `C` `t` `a` `o` `s` `x` `d` `n`). Agent errors (`notWritable`, `wrongType`, ...) are shown with the result.

### Traps

- **Receive**: choose the listen address (default `0.0.0.0:162`), the accepted communities (empty = any) and v3 users, then "Start". Notifications appear at once (WebSocket `snmp_trap` events) with the trap OID and bindings named by the MIBs. Informs are acknowledged automatically. Messages with a wrong community or failed authentication are listed too, with the error.
- **Send**: with the target of the Operations tab, send a trap OID (`linkDown`, ...) and bindings (one per line: "object value" or "object type value"). On v1 the notification is converted to a Trap-PDU (RFC 3584). "Inform" waits for the acknowledgement. For v3 traps wpg is the authoritative engine.

### MIBs

Drop MIB files (`.mib` `.txt` `.my`) on the upload area or use "Choose files". They are stored under `<data_dir>/mibs` (setting `snmp.mib_dir`) and loaded at once. The module list shows the load state:

- **loaded**: everything resolved
- **loaded with problems**: some definitions do not resolve or an imported module is missing; the missing modules are named, upload them too (vendor MIBs usually need common modules such as `CISCO-SMI`)
- **not usable**: nothing resolved

The quirks of vendor MIBs (comment banners, unknown macros, missing imports, broken definitions) become warnings; the rest stays usable. Uploaded modules can be deleted (with every module of the same file). Bundled modules cannot be deleted, but uploading a newer version with the same name replaces them.

The MIB tree shows descriptions, syntax, access, enumerations, INDEX and more. "Module to show" narrows it to one uploaded (or bundled) MIB module: only the objects that module defines, starting at its top nodes instead of 1.3.6.1. The magnifier button in the module list does the same. "Use in a request" copies the name to the Operations tab (scalars with `.0`); the Get (scalars only) / GetNext / Walk buttons switch to the Operations tab and run the request at once.

Bundled MIBs: SNMPv2-SMI / TC / CONF / MIB, RFC1155-SMI, RFC-1212, RFC-1215, RFC1213-MIB, IANAifType-MIB, IF-MIB, INET-ADDRESS-MIB, IP-MIB, HOST-RESOURCES-MIB, SNMP-FRAMEWORK-MIB, SNMP-USER-BASED-SM-MIB, UUID-TC-MIB, IANA-ENTITY-MIB, ENTITY-MIB (IETF MIB modules extracted from the RFCs).

## pcap recording

With "Record pcap" on, every SNMP datagram wpg sends or receives (requests, responses, traps, informs) is recorded; download it as `.pcap` or `.pcapng` and open it in Wireshark. The datagrams travel over ordinary UDP sockets, so the Ethernet / IP / UDP headers are synthesised with the real addresses and ports and placeholder MACs (`02:00:00:00:00:01` for wpg, `02:00:00:00:00:02` for the peer). At most 100,000 frames are kept in memory.

## Configuration

```toml
[snmp]
enabled = true        # SNMP tool screen and API
trap_listen = ""      # e.g. "0.0.0.0:162"; empty = trap receiver not started at boot (can be started from the UI)
mib_dir = ""          # default <data_dir>/mibs
```

With `trap_listen` set, the receiver starts at boot (accepting any community and no v3 users; restart it from the screen to change that).

## Permissions for UDP 162

Ports below 1024 need root or `CAP_NET_BIND_SERVICE`:

```sh
sudo setcap cap_net_bind_service=+ep "$(command -v wpg)"
```

Otherwise listen on e.g. `0.0.0.0:1162` and point the devices' trap destination port there. A start without the permission fails with an explanatory error.

## Limits

- The receiver does not enforce the SNMPv3 time window (notInTimeWindow) so that late messages are still shown; noAuthNoPriv v3 messages are accepted from any user name.
- v3 pass phrases need at least 8 characters (RFC 3414's recommendation, enforced by most devices).
- The MIB parser looks up names and types; it is not a strict SMI validator.
