# DHCP サーバー

Status: draft

wpg には試験用の DHCP サーバーが入っています (Web UI の「DHCP」画面、API は [`/api/dhcp`](../reference/api.md))。wpg のホストを試験対象 (ルーター、CPE、端末) と直結し、相手にアドレスやプレフィックスを配れます。設計は [ADR-0012](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/adr/0012-dhcp-server.md) を参照してください。

- DHCPv4: アドレスプール、サブネットマスク・ルーター・DNS・ドメイン名・NTP、固定割り当て (MAC → IP)、リレー (giaddr / option 82)
- DHCPv6: アドレス (IA_NA)、プレフィックス委任 (IA_PD)、DNS サーバー / ドメイン検索リスト、Rapid Commit
- ルーター広告 (RA): ホストに DHCPv6 を使わせる M / O フラグ、任意のオンリンクプレフィックス
- インタフェースの固定 IP (画面からは一時的に、デプロイ時は netplan で永続的に)
- リース表とやり取りのログ

Linux でのみ動作します。

## インタフェースを決める

DHCP は**指定した 1 つのインタフェースからだけ**応答します (ソケットを `SO_BINDTODEVICE` でそのインタフェースに縛ります)。管理用のインタフェースには出しません。

デプロイ時に固定するのが基本です。

```sh
scripts/deploy.sh user@host --service --iface ens3 \
    --dhcp-iface ens3 --dhcp-iface-addr 192.168.100.1/24,2001:db8:1::1/64
```

- `--dhcp-iface ens3`: `/etc/wpg/wpg.env` に `WPG_DHCP__IFACE=ens3` を書きます。画面ではインタフェースを変更できなくなります。サービスには `CAP_NET_BIND_SERVICE` (ポート 67 / 547) のドロップインが付きます。
- `--dhcp-iface-addr`: そのインタフェースの固定 IP です。`/etc/netplan/90-wpg-ens3.yaml` を書いて `netplan apply` します (DHCP クライアントと RA の受け入れは無効になります)。デフォルトルートのあるインタフェースには書き込みません。元に戻すにはこのファイルを消して `sudo netplan apply` してください。

Windows からは `deploy.ps1 -DhcpIface ens3 -DhcpIfaceAddr 192.168.100.1/24` です。回答ファイルでは `dhcp_iface=` / `dhcp_iface_addr=` です。

`wpg.toml` で直接指定することもできます。

```toml
[dhcp]
enabled = true
iface = "ens3"
```

`iface` が空のときは、画面で `tx.allowed_ifaces` に含まれるインタフェースから選びます。

## 画面

### 状態

インタフェース、DHCPv4 のサーバー識別子、DHCPv6 のサーバー DUID、wpg が付与したアドレス、受信 / 送信の数を表示します。ヘッダの「保存して起動」で設定を保存してサーバーを起動 (動作中なら再起動) し、「停止」で止めます。動作中に wpg を再起動すると、サーバーも自動で再開します。

### 設定

**インタフェースのアドレス (固定 IP)**: `192.168.100.1/24, 2001:db8:1::1/64` のように書くと、サーバーの起動時にインタフェースに付与し (リンクも up にします)、停止時に wpg が付けた分だけ削除します。再起動 (OS) をまたいで残したい場合はデプロイ時の `--dhcp-iface-addr` を使ってください。最初の IPv4 アドレスが DHCPv4 のサーバー識別子になります (「サーバー識別子」欄で上書きできます)。

**DHCPv4**: 範囲の先頭 / 末尾、サブネットマスク、リース時間 (T1 / T2 はその 1/2 と 7/8)、ルーター、DNS サーバー、ドメイン名、NTP サーバー。リストはカンマかスペースで区切ります。固定割り当ては 1 行に `MAC IP` (範囲外のアドレスも可) です。

**DHCPv6**: 「アドレス (IA_NA)」は範囲から 1 つずつ、「プレフィックス委任 (IA_PD)」はプールのプレフィックス (例 `2001:db8:100::` / 40) を委任長 (例 56) で区切って順に配ります。推奨有効期間と有効期間 (T1 / T2 は推奨有効期間の 1/2 と 4/5)、DNS サーバー、ドメイン検索リスト、ADVERTISE の優先度、Rapid Commit を設定します。

**ルーター広告**: ホストは RA の M フラグ (アドレス) か O フラグ (その他の設定) を見て初めて DHCPv6 を使います。リンク上に RA を出すルーターがなければ「ルーター広告を送る」を有効にしてください。M フラグは IA_NA が有効なとき、O フラグは常に立てます。オンリンクプレフィックスを書くと Prefix Information を付け、「SLAAC」で A フラグも立てます。ルーター寿命を 0 にすると、デフォルトルーターにはなりません。

### リース

割り当て中・提示中・辞退 (DECLINE、使用中と報告されたアドレス) のリースを表示します。削除すると wpg 側の記録が消えます (クライアントは次の更新まで使い続けます)。期限切れのリースは自動で消えます。

### アクティビティ

要求と応答 (`DISCOVER → OFFER`、`SOLICIT → ADVERTISE`、RA の送信など)、応答しなかった理由 (プール枯渇、別サーバーを選択、未知のサーバー識別子) を新しい順に表示します。

## 権限

| 処理 | 必要な権限 |
|---|---|
| ポート 67 / 547 で待ち受け | `CAP_NET_BIND_SERVICE` |
| RA の送信 (生 ICMPv6) | `CAP_NET_RAW` |
| インタフェースへのアドレス付与 | `CAP_NET_ADMIN` |

deploy.sh が入れる systemd サービスはすべて持っています (`--dhcp-iface` を指定したとき、または SNMP トラップの設定があるとき `bind-service.conf` ドロップインが付きます)。手動で動かす場合は `sudo setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip /usr/local/bin/wpg` を実行してください。

## 制限と注意

- 同じリンクに別の DHCP サーバーがあると競合します。試験用インタフェースでのみ使ってください。
- DHCPv4 の応答は、リレー経由ならリレーへ、更新中のクライアントにはユニキャスト、それ以外はブロードキャストで送ります。
- DHCPv6 のリレーメッセージ (RELAY-FORW) には応答しません。
- 静的な DHCPv6 割り当て (DUID → アドレス) はありません。
