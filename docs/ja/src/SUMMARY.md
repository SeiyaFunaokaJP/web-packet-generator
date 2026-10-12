# wpg ドキュメント

- [はじめに](intro.md)

# はじめに (Getting Started)

- [Linux へのインストール](getting-started/install-linux.md)
- [macOS へのインストール](getting-started/install-macos.md)
- [Windows へのインストール](getting-started/install-windows.md)
- [権限設定](getting-started/permissions.md)
- [初回ログイン](getting-started/first-login.md)

# チュートリアル

- [チュートリアル: ARP スイープ](tutorials/arp-sweep.md)
- [チュートリアル: VLAN タグ付き TCP SYN 大量送信](tutorials/vlan-syn-flood.md)
- [チュートリアル: IPv6 と近隣探索 (NDP)](tutorials/ipv6-ndp.md)
- [チュートリアル: 負荷とレート制御](tutorials/rate-control.md)
- [チュートリアル: pcap からテンプレートへ変換して再生](tutorials/pcap-replay.md)

# リファレンス

- [プロトコルリファレンス](reference/protocols.md)
- [P1 プロトコル補足](protocols/README.md)
  - [L2 制御プロトコル](protocols/l2-control.md)
  - [カプセル化・トンネル](protocols/encap.md)
  - [ルーティングプロトコル](protocols/routing.md)
  - [冗長化 (FHRP)](protocols/fhrp.md)
  - [マルチキャスト制御](protocols/multicast.md)
  - [アプリケーションプロトコル](protocols/app.md)
  - [OSI カプセル化 (P2)](protocols/osi.md)
  - [CLNP / ES-IS (P2)](protocols/clnp.md)
  - [IS-IS (P2)](protocols/isis.md)
  - [SCTP (P2)](protocols/sctp.md)
  - [OSPF Opaque / SR、BGP MP / EVPN (P2)](protocols/routing-ext.md)
  - [LDP / RSVP-TE / BFD (P2)](protocols/mpls-bfd.md)
  - [SRv6 SRH / SR-MPLS (P2)](protocols/srv6.md)
  - [MACsec / GTPv2-C / Diameter / SIP / RTP (P2)](protocols/mobile-voip.md)
- [テンプレートスキーマ](reference/template-schema.md)
- [API リファレンス](reference/api.md)
- [CLI リファレンス](reference/cli.md)
- [設定リファレンス](reference/config.md)

# 運用

- [DPDK / AF_XDP のセットアップ](operations/dpdk-afxdp.md)
- [CML2 での注意点](operations/cml2.md)
- [相互接続確認](operations/interop.md)
- [性能チューニング](operations/tuning.md)
- [認証と TLS](operations/security.md)

# 開発者ガイド

- [crate 構成](dev/crates.md)
- [プロトコルの追加手順](dev/add-protocol.md)
- [i18n 文字列の追加手順](dev/add-i18n.md)
- [リリース手順](dev/release.md)

# FAQ / トラブルシュート

- [FAQ / トラブルシュート](faq/troubleshooting.md)

---

- [法的注意](legal.md)
