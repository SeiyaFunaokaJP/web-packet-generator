# Introduction

Status: draft

wpg (web-packet-generator) is a Rust packet generator for network verification, driven from a web GUI. It lets you assemble arbitrary packets, including deep OSI-family protocols such as IS-IS and CLNP, and transmit them at high speed. Transmission backends range from pcap/Npcap up to AF_PACKET, AF_XDP and DPDK. It ships as a single binary for Windows, macOS and Linux. Use is limited to lab and verification environments.

![Building Ethernet / IPv4 / UDP in the packet builder, with a source-address range](images/builder.gif)
