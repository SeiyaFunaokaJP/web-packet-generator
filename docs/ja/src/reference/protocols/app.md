# アプリケーションプロトコル (P1)

Status: draft

定義: `crates/wpg-protocols/src/app.rs` / テスト: `crates/wpg-protocols/tests/p1_app.rs` / サンプル: `templates/protocols/app/`

## 対象と到達方法

いずれも UDP のポート番号で到達します。解析では宛先ポート、該当しなければ送信元ポートで判別するため、既知ポートからエフェメラルポートへの応答 (DNS / SNMP / RADIUS の応答など) も解析できます。

| id | プロトコル | UDP ポート | サンプル |
|---|---|---|---|
| `dhcpv4` | DHCPv4 (RFC 2131) | 67 / 68 | `dhcpv4-discover` / `dhcpv4-relay` |
| `dhcpv6` / `dhcpv6_relay` | DHCPv6 (RFC 8415) | 546 / 547 | `dhcpv6-solicit` / `dhcpv6-relay-forward` |
| `dns` | DNS (RFC 1035) | 53 | `dns-query` / `dns-response` |
| `ntp` | NTPv4 (RFC 5905) | 123 | `ntp-client` |
| `snmp` + `snmp_pdu` / `snmp_trap` | SNMP v1 / v2c | 161 / 162 | `snmp-get-request` / `snmp-trap-v1` |
| `syslog` | Syslog (RFC 5426) | 514 | `syslog` |
| `radius` | RADIUS (RFC 2865) | 1812 / 1813 | `radius-access-request` |
| `tftp` + `tftp_data` / `tftp_ack` / `tftp_error` / `tftp_oack` | TFTP (RFC 1350 / 2347) | 69 | `tftp-rrq` / `tftp-data` / `tftp-ack` / `tftp-error` / `tftp-oack` |

## 解析時の読み直し

同じポートで構造の違うメッセージは、先頭のフィールドを見て別の定義で読み直します。

- **DHCPv6**: メッセージタイプ 12 / 13 (Relay-Forward / Relay-Reply) は `dhcpv6_relay` として読み直します。Relay Message オプション内のメッセージは 1 段だけ型付けされます。
- **SNMP**: `snmp` の次は `snmp_pdu` で、v1 Trap のタグ (`0xa4`) なら `snmp_trap` として読み直します。
- **TFTP**: opcode で DATA / ACK / ERROR / OACK をそれぞれの定義で読み直します。ただし実際の転送は 2 つのエフェメラルポート (TID) 間で行われるため、ポート 69 を含まないパケットは UDP の後が `raw` になります。

## Auto フィールド

| フィールド | 内容 |
|---|---|
| `snmp.msg_length` | 外側 SEQUENCE の内容の長さ |
| `snmp_pdu.pdu_length` / `snmp_trap.pdu_length` | PDU の内容の長さ |
| `radius.length` | RADIUS パケット全体の長さ |

## 制約と近似

- **DNS のセクションは opaque**: 名前は長さ付きラベルの並びをゼロオクテットで終える形式で、各セクションはヘッダのカウンタで大きさが決まります。この形を型付けできないため、12 オクテットのヘッダ (フラグは各ビットに分割) だけを型付けし、質問 / 回答 / 権威 / 追加セクションは 1 つの opaque な `sections` 領域です。`qdcount` などのカウンタは Manual です。
- **SNMP は BER の短形式のみ**: メッセージは事前にエンコードされたタグ / 長さ / 値の並びで、長さは 8 ビット固定です。内容が 128 オクテット未満の短形式だけを扱い、長形式 (`0x81 nn`、`0x82 nn nn`) は扱えません。`community_length` は Manual で、構造型の入れ子は固定の深さ (varbind リスト → varbind → 値) までです。
- **RADIUS の Authenticator は Manual**: Response Authenticator と Message-Authenticator は共有シークレットを使う MD5 / HMAC-MD5 で、鍵付きダイジェストの自動計算がないため `authenticator` は手動で設定します。
- **NTP**: RFC 5905 の時刻パケットを型付けしています。拡張フィールドと MAC は opaque な `extensions` です。制御メッセージ (mode 6) とプライベートメッセージ (mode 7) は扱いません。タイムスタンプは 1900 年起点の 32.32 固定小数点です。
- **Syslog**: メッセージ全体が 1 つのテキスト (`<PRI>` から始まる) です。PRI は facility * 8 + severity です。
- **DHCPv4**: `chaddr` は 6 オクテットの Ethernet アドレスと 10 オクテットのパディングに分かれています。リレーは `hops`、`giaddr` と Relay Agent Information オプション (82) で表現します。
