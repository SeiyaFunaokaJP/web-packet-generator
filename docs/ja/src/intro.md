# はじめに

Status: draft

wpg (web-packet-generator) は、Web GUI で動作するネットワーク検証向けの Rust 製パケットジェネレータです。IS-IS や CLNP など OSI 系の深いプロトコルまで含め、任意のパケットを組み立てて高速に送出できます。送出バックエンドは pcap/Npcap から AF_PACKET、AF_XDP、DPDK まで選択可能です。Windows / macOS / Linux 向けの単一バイナリとして配布されます。利用は検証環境に限定してください。
