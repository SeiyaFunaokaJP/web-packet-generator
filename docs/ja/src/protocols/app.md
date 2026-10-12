# アプリケーションプロトコル

Status: draft

DHCPv4 / DHCPv6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP の補足です。定義は `crates/wpg-protocols/src/app.rs`、サンプルは `templates/protocols/app/` にあります。

## 到達経路とサンプル

すべて UDP のポートで判別します。宛先ポートが未知なら送信元ポートで判別するので、既知ポートから一時ポートへの応答 (DNS / SNMP / RADIUS の応答など) もデコードできます。

| id | プロトコル | UDP ポート | サンプル |
|---|---|---|---|
| `dhcpv4` | DHCPv4 (リレーエージェント情報を含む) | 67 / 68 | `dhcpv4-discover`、`dhcpv4-relay` |
| `dhcpv6` / `dhcpv6_relay` | DHCPv6 / Relay-Forward・Relay-Reply | 546 / 547 | `dhcpv6-solicit`、`dhcpv6-relay-forward` |
| `dns` | DNS | 53 | `dns-query`、`dns-response` |
| `ntp` | NTP (RFC 5905 の時刻パケット) | 123 | `ntp-client` |
| `snmp` + `snmp_pdu` / `snmp_trap` | SNMP v1 / v2c | 161 / 162 | `snmp-get-request`、`snmp-trap-v1` |
| `syslog` | Syslog | 514 | `syslog` |
| `radius` | RADIUS | 1812 / 1813 | `radius-access-request` |
| `tftp` + `tftp_data` / `tftp_ack` / `tftp_error` / `tftp_oack` | TFTP | 69 | `tftp-rrq` / `-data` / `-ack` / `-error` / `-oack` |

## Auto になるフィールド

- `radius.length`。
- SNMP の `msg_length` / `pdu_length` (BER 短形式)。
- DHCPv4 / DHCPv6 / RADIUS のオプション・属性の長さ欄。

## 手動で合わせるフィールド

- **DNS** の `qdcount` / `ancount` / `nscount` / `arcount`。セクション部分は不透明なので、件数はセクションの内容に合わせて設定します。
- **SNMP** の `community_length` (コミュニティ文字列を変えたら合わせます)。
- **RADIUS** の `authenticator` (Response Authenticator) と Message-Authenticator 属性。どちらも共有シークレットを使う MD5 / HMAC-MD5 で、鍵付きダイジェストの自動計算はありません。Access-Request の Request Authenticator は乱数なので任意の 16 オクテットで構いません。

## 制約と近似

- **DNS**: 名前は長さ付きラベルの列 + 終端の 0 オクテットで、各セクションはヘッダの件数で大きさが決まります。エンジンのリスト型には終端停止も件数駆動の長さもないため、12 オクテットのヘッダだけを型付けし、セクション (質問・回答・権威・追加) は 1 つの不透明な `sections` 領域です。既定値は `example.com` A/IN の質問です。名前圧縮ポインタもそのままバイト列として書きます。
- **SNMP** は BER エンコードの ASN.1 で、事前にエンコードされた tag / length / value の固定レイアウトとして表現しています。BER の **短形式長 (内容が 128 オクテット未満)** のみ対応し、長形式 (`0x81 nn`、`0x82 nn nn`) は扱えません。構造化タグの入れ子は固定の深さ (varbind リスト → varbind → 値) です。PDU 種別は PDU の中でしか分からないため、`snmp` は既定で `snmp_pdu` に続き、v1 Trap タグ (`0xa4`) なら `snmp_trap` として読み直します。
- **DHCPv6** の Relay-Forward / Relay-Reply (msg-type 12 / 13) はクライアント・サーバのメッセージと固定部の形が違うため、`dhcpv6` でデコードしてから `dhcpv6_relay` に読み直します。Relay Message オプション内のメッセージは 1 段だけ型付けされます。
- **TFTP**: RRQ / WRQ のファイル名・モード・オプションと、エラーメッセージは NUL 終端文字列として型付けされています。DATA / ACK / ERROR / OACK は opcode による読み直しで `tftp_data` などに分かれます。転送本体は両端とも一時ポート (TID) 同士で行われるため、ポート 69 以外のパケットは UDP の後ろが `raw` としてデコードされます。
- **NTP** の制御メッセージ (mode 6) とプライベートメッセージ (mode 7) は別レイアウトのため対象外です。拡張フィールドと MAC は不透明な `extensions` です。
- **Syslog** は自由テキストで、1 つの不透明な `message` です。RFC 5424 / RFC 3164 いずれの形式でも、文字列をそのまま与えてください。
