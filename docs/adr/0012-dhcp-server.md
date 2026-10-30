# ADR-0012: DHCP サーバー機能 (wpg-dhcp)

> Add a DHCPv4 / DHCPv6 (IA_NA, IA_PD) server with optional router advertisements as a pure-Rust crate `wpg-dhcp`, answering on one interface fixed at deploy time (or chosen on the new DHCP screen), with static interface addresses set by netlink or by a netplan file written at deploy time.

- Status: Accepted
- Date: 2026-10-29
- Deciders: wpg maintainers

## Context

検証環境では、wpg のホストを試験対象 (ルーター、CPE、端末) と直結し、相手に DHCP でアドレスやプレフィックスを配りたいことが多い。これまでは dnsmasq / Kea / isc-dhcp-server を別に入れて設定していたが、オフラインの検証ホストではパッケージの追加自体が手間で、設定も wpg の画面と別になる。また DHCP サーバーには固定アドレスが必要なので、試験用インタフェースに IP を振る作業 (netplan の編集) もセットで発生していた。

`wpg-protocols` の `dhcpv4` / `dhcpv6` はパケットを「作る」だけで、要求に応答してリースを管理することはできない。

## Decision Drivers

- 外部のデーモンやパッケージに依存しない (単一バイナリ、オフラインホストでそのまま使える)
- デプロイ時に指定したインタフェースからだけ応答する (管理用インタフェースに DHCP を出さない)
- DHCPv6 は IA_NA に加えてプレフィックス委任 (IA_PD) を扱う。DHCPv6 を使わせるための RA (M / O フラグ) も出せる
- 試験ツールとして、やり取りとリースを画面で確認・削除できる
- 固定 IP は「その場で付ける」(画面) と「永続化する」(デプロイ) の両方

## Decision

- 新しい crate `crates/wpg-dhcp` を追加する (`default-members` にも入れる)。内容は DHCPv4 (RFC 2131 / 2132) と DHCPv6 (RFC 8415、DNS は RFC 3646) のコーデック、設定と検証、リース表、プロトコル処理 (要求 + リース表 + 時刻 → 応答の純粋関数。ソケットなしで単体テストする)、RA の組み立て、ソケットと実行部、rtnetlink によるアドレス付与。
- ソケットは Linux の UDP ソケットを `SO_BINDTODEVICE` でインタフェースに縛る (v4 は 0.0.0.0:67 + `SO_BROADCAST`、v6 は [::]:547 で `ff02::1:2` に参加)。生パケット (AF_PACKET) は使わない。v4 の応答は `giaddr` があればリレーへ、`ciaddr` があればユニキャスト、それ以外はブロードキャストで返す (未設定クライアントへのユニキャストには ARP 表の操作が要るため、RFC 2131 が認めるブロードキャストにする)。RA は生 ICMPv6 ソケット (チェックサムはカーネル計算) で、定期送信と RS への応答を行う。Linux 以外では起動時に「Linux のみ」のエラーを返す。
- 対応する DHCPv4 メッセージは DISCOVER / REQUEST / DECLINE / RELEASE / INFORM。固定割り当て (MAC → IP)、クライアント識別子 (option 61) の優先、option 82 のエコーを行う。DHCPv6 は SOLICIT / REQUEST / CONFIRM / RENEW / REBIND / RELEASE / DECLINE / INFORMATION-REQUEST と Rapid Commit、IA_NA と IA_PD (プールを委任長で分割)。リレーメッセージ (RELAY-FORW) は扱わない。
- サーバーは `[dhcp]` (`enabled`, `iface`) を持つ。`iface` はデプロイ時に固定する値 (`scripts/deploy.sh --dhcp-iface` が `/etc/wpg/wpg.env` に `WPG_DHCP__IFACE` を書く)。空なら画面で `tx.allowed_ifaces` の範囲から選ぶ。プールやオプションは画面で編集し `<data_dir>/dhcp.json` に、リースは `<data_dir>/dhcp-leases.json` に保存する。停止時に動いていたサーバーは次の起動時に再開する。
- 固定 IP は 2 通り。画面の「インタフェースのアドレス」はサーバーの起動時に rtnetlink (`RTM_NEWADDR`、`CAP_NET_ADMIN`) で付与し、停止時に wpg が付けた分だけ削除する (永続化しない)。デプロイ時の `--dhcp-iface-addr` は `/etc/netplan/90-wpg-<IF>.yaml` を書いて `netplan apply` する (永続)。デフォルトルートを持つインタフェース (管理用) には適用しない。
- API は `/api/dhcp` (状態 / 設定 / 起動 / 停止 / リース / アクティビティ)、WebSocket に `dhcp_activity` と `dhcp_state` を追加し、Web UI に DHCP 画面を追加する。

## Alternatives considered

- dnsmasq / Kea を同梱して設定ファイルを生成する: 外部バイナリの配布とバージョン差の吸収が必要で、単一バイナリ方針に反する。リースの表示もファイル解析になる。
- AF_PACKET で生フレームを送受信する: 未設定クライアントへのユニキャストは書けるが、IP / UDP の組み立てとフィルタを自前で持つことになり、ブロードキャスト応答で十分な試験用途には過剰。
- `ip addr` コマンドを呼んで固定 IP を付ける: systemd サービスのアンビエント権限は子プロセスの `ip` に引き継がれない (NoNewPrivileges)。netlink を直接話す。

## Consequences

- 試験対象に DHCPv4 / DHCPv6 (アドレスとプレフィックス委任) と DNS を配れる。やり取りは画面のアクティビティに、リースは表に出る。
- 権限: ポート 67 / 547 に `CAP_NET_BIND_SERVICE`、RA に `CAP_NET_RAW`、アドレス付与に `CAP_NET_ADMIN`。deploy.sh のサービスは `--dhcp-iface` 指定時に `CAP_NET_BIND_SERVICE` のドロップインを追加する (NET_RAW / NET_ADMIN は元から付与)。
- 検証は単体テスト (DORA、更新、NAK、固定割り当て、リレー、DHCPv6 の各メッセージと PD 分割、RA、netlink メッセージ) と、応答を `wpg-protocols` の `dhcpv4` / `dhcpv6` 定義で復号する相互確認。実機クライアントとの接続確認はフォローアップ。
- 同じリンクに別の DHCP サーバーがあると競合する。試験用インタフェースでのみ使う前提で、画面とドキュメントで案内する。
