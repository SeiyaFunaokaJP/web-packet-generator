# Bundled MIB modules

These modules are compiled into wpg (`wpg_snmp::mib::BUNDLED`) so that the
SNMP tool shows names, enumerations and display hints for the standard
objects without any upload.

| Module | Source |
|---|---|
| RFC1155-SMI, RFC-1212, RFC-1215, RFC1213-MIB | RFC 1155, 1212, 1215, 1213 (SMIv1) |
| SNMPv2-SMI, SNMPv2-TC, SNMPv2-CONF | RFC 2578, 2579, 2580 |
| SNMPv2-MIB | RFC 3418 |
| SNMP-FRAMEWORK-MIB | RFC 3411 |
| SNMP-USER-BASED-SM-MIB | RFC 3414 |
| IF-MIB | RFC 2863 |
| IANAifType-MIB | IANA (<https://www.iana.org/assignments/ianaiftype-mib>) |
| INET-ADDRESS-MIB | RFC 4001 |
| IP-MIB | RFC 4293 |
| HOST-RESOURCES-MIB | RFC 2790 |
| ENTITY-MIB, UUID-TC-MIB | RFC 6933 |
| IANA-ENTITY-MIB | IANA (<https://www.iana.org/assignments/ianaentity-mib>) |

The files are the modules as extracted from the RFCs (the copies distributed
with net-snmp and LibreNMS were used, with line endings normalised to LF).
MIB modules published in RFCs are code components that the IETF Trust's
Legal Provisions (and, for the older RFCs, the RFC copyright notice) allow to
be copied and redistributed; the IANA-maintained modules are published for
the same use. Each file keeps its original copyright notice.

Vendor MIBs (Cisco, Juniper, ...) are not bundled: upload them on the SNMP
screen (they are stored under `<data_dir>/mibs`).
