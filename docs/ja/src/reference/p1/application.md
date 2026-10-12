# アプリケーションプロトコル (UDP)

Status: draft

DHCPv4、DHCPv6、DNS、NTP、SNMP v1 / v2c、Syslog、RADIUS、TFTP の補足です。定義は `crates/wpg-protocols/src/app.rs` にあります。

## 識別と積み方

すべて `udp` の上に積み、ポートで識別します。解析時は宛先ポート、未知なら送信元ポートで選ぶので、well-known ポートからエフェメラルポートへの応答 (DNS / SNMP / RADIUS の応答など) も解析できます。合成時は宛先ポートだけが Auto で決まります。

| プロトコル ID | UDP ポート | 備考 |
|---|---|---|
| `dhcpv4` | 67 (サーバ) / 68 (クライアント) | リレーは `hops` / `giaddr` / Relay Agent Information (オプション 82) |
| `dhcpv6` | 546 (クライアント) / 547 (サーバ・リレー) | type 12 / 13 は `dhcpv6_relay` として読み直す |
| `dhcpv6_relay` | 547 | Relay-Forward / Relay-Reply |
| `dns` | 53 | |
| `ntp` | 123 | |
| `snmp` → `snmp_pdu` / `snmp_trap` | 161 (エージェント) / 162 (トラップ受信) | |
| `syslog` | 514 | |
| `radius` | 1812 (認証) / 1813 (アカウンティング) | |
| `tftp` | 69 | DATA / ACK / ERROR / OACK は `tftp_data` / `tftp_ack` / `tftp_error` / `tftp_oack` として読み直す |

TFTP の転送はエフェメラルポート (TID) 同士で行われるため、解析では `udp` の後が `raw` になります。合成時は `tftp_data` などを直接積み、`udp` のポートを Manual で書きます。

## Auto フィールド

| プロトコル | Auto |
|---|---|
| `snmp` | `msg_length` |
| `snmp_pdu` / `snmp_trap` | `pdu_length` |
| `radius` | `length` |
| TLV 形式のオプション / 属性 (DHCPv4 / DHCPv6 オプション、RADIUS 属性、BER TLV) | 各要素の長さ |

## 制約と回避方法

- **DNS** は 12 オクテットのヘッダ (フラグワードはビットフィールドに分割) だけを型付きにし、質問 / 回答 / 権威 / 追加の各セクションは opaque な `sections` 1 つにまとめています。名前は長さプレフィックス付きのラベル列なので、`sections` には `07 "example" 03 "com" 00` のようにバイト列で書きます (`dns-query.wpg.yaml` 参照)。`qdcount` / `ancount` / `nscount` / `arcount` は Manual なので、セクションの内容と合わせてください。圧縮ポインタ (`c0 0c` など) もそのまま書けます。
- **SNMP** は BER の **短形式長** (内容が 128 オクテット未満) だけに対応します。長形式 (`0x81 nn`、`0x82 nn nn`) は表現できません。`community_length` は Manual なので、`community` のオクテット数を書いてください。構造型 (SEQUENCE) は「varbind リスト → varbind → 値」の固定の深さまでネストできます。PDU の種類は PDU の中でしか分からないため、`snmp` の次は既定で `snmp_pdu` になり、v1 Trap のタグ (`0xa4`) なら `snmp_trap` として読み直します。
- **DHCPv6** の Relay Message オプションは内側のメッセージを 1 段だけ型付きで表現します。それより深いリレーの入れ子は opaque です。
- **RADIUS** の Response Authenticator と Message-Authenticator は共有シークレットを使う MD5 / HMAC-MD5 で、鍵付きダイジェストの Auto はありません。`authenticator` は Manual です (Access-Request ではランダムな 16 オクテットでよい)。
- **NTP** は RFC 5905 の時刻パケットを表現します。制御 (mode 6) / プライベート (mode 7) メッセージや拡張フィールド / MAC は opaque な `extensions` です。タイムスタンプは 1900 年起点の 32.32 固定小数点です。
- **Syslog** は `<PRI>` を含むメッセージ全体を 1 つのテキスト `message` として扱います (PRI = facility × 8 + severity)。
- **TFTP** の要求 / エラーの文字列 (ファイル名、モード、オプション名と値、エラーメッセージ) は NUL 終端文字列として型付きです。

## サンプル

| テンプレート | 内容 |
|---|---|
| `templates/protocols/app/dhcpv4-discover.wpg.yaml` / `dhcpv4-relay.wpg.yaml` | ブロードキャストの DHCPDISCOVER / giaddr と Relay Agent Information 付きでリレーされた DHCPREQUEST |
| `templates/protocols/app/dhcpv6-solicit.wpg.yaml` / `dhcpv6-relay-forward.wpg.yaml` | Solicit / Interface-Id と Relay Message 付きの Relay-Forward |
| `templates/protocols/app/dns-query.wpg.yaml` / `dns-response.wpg.yaml` | example.com の A 問い合わせ / 圧縮名を使う権威応答 |
| `templates/protocols/app/ntp-client.wpg.yaml` | クライアントモード (mode 3) の要求 |
| `templates/protocols/app/snmp-get-request.wpg.yaml` / `snmp-trap-v1.wpg.yaml` | SNMPv2c GetRequest (sysDescr.0 / sysName.0) / SNMPv1 linkDown トラップ |
| `templates/protocols/app/syslog.wpg.yaml` | RFC 5424 形式のログ行 |
| `templates/protocols/app/radius-access-request.wpg.yaml` | User-Name / NAS-IP-Address / NAS-Port 付きの Access-Request |
| `templates/protocols/app/tftp-rrq.wpg.yaml` ほか `tftp-*.wpg.yaml` | RRQ (blksize オプション付き) / OACK / DATA / ACK / ERROR |
