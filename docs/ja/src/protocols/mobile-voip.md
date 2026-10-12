# MACsec / GTPv2-C / Diameter / SIP / RTP (P2)

Status: draft

MACsec (IEEE 802.1AE、構造のみ)、GTPv2-C (3GPP TS 29.274)、Diameter (RFC 6733、Credit-Control は RFC 4006)、SIP (RFC 3261)、RTP / RTCP (RFC 3550) (M7) のメモです。定義は `crates/wpg-protocols/src/{macsec,gtpv2,diameter,sip,rtp}.rs`、サンプルは `templates/protocols/l2/macsec.wpg.yaml`、`templates/protocols/mobile/`、`templates/protocols/voip/` にあります。

## スタックとサンプル

```text
ethernet2 / macsec                          # EtherType 0x88e5 (Auto)。dot1q の後にも置ける
ethernet2 / ipv4 / udp / gtpv2 [/ gtpv2]    # UDP 2123。ピギーバックされたメッセージは 2 つ目の gtpv2 レイヤ
ethernet2 / ipv4 / tcp / diameter           # TCP 3868
ethernet2 / ipv4 / udp|tcp / sip            # UDP / TCP 5060
ethernet2 / ipv4 / udp / rtp / payload      # 任意のポート (レイヤを明示)。UDP 5004 はデコードされる
ethernet2 / ipv4 / udp / rtcp_sr / rtcp_sdes ...  # RTCP 複合パケット。UDP 5005 はデコードされる
```

UDP / TCP の宛先ポートは Auto ではないので 2123、3868、5060 などを指定します (デコードでは送信元ポートも見ます)。

| サンプル | 内容 |
|---|---|
| `macsec` | 明示的な SCI 付きの暗号化フレーム (アソシエーション番号 1、48 オクテットのセキュアデータ、不透明な ICV) |
| `gtpv2-create-session-request` | S11 の Create Session Request: IMSI、MSISDN、Serving Network、RAT Type、F-TEID 2 つ、APN、Selection Mode、PDN Type、PAA、AMBR、Bearer Context |
| `gtpv2-create-session-response` | Create Session Response: Cause、F-TEID、PAA、APN Restriction、S1-U の F-TEID と Charging ID を含む Bearer Context |
| `gtpv2-echo-request` | TEID なしの Echo Request (Recovery IE) |
| `diameter-cer` | Vendor-Specific-Application-Id (S6a) 付きの Capabilities-Exchange-Request |
| `diameter-ccr-initial` | Subscription-Id 付きの初回 Credit-Control-Request |
| `sip-invite` | UDP での SDP オファー付き INVITE |
| `sip-register` | TCP での本文なしの REGISTER |
| `rtp-pcmu` | G.711 PCMU の 50 パケット (シーケンス番号とタイムスタンプのジェネレータ、160 オクテットのペイロード) |
| `rtcp-sr-sdes` | 受信レポート 1 件の送信者レポートと SDES の CNAME |
| `rtcp-rr-bye` | 受信者レポート、SDES の CNAME、理由付きの BYE |
| `rtcp-feedback-app` | 受信者レポート、PLI (汎用 `rtcp` レイヤ)、APP パケット |

## MACsec: 構造のみ

`macsec` レイヤは SecTAG とフレームの保護部分です。

| フィールド | 内容 |
|---|---|
| `v`、`es`、`sc`、`scb`、`e`、`c` | TCI のビット。`sc` は Auto: `sci` に要素があれば 1 |
| `an` | アソシエーション番号 (2 ビット) |
| `sl` | ショート長 (6 ビット): セキュアデータが 48 オクテット未満のときその長さ、それ以外は 0。手動 |
| `pn` | パケット番号 (32 ビット) |
| `sci` | `[]` または `[{ system_identifier, port_identifier }]` |
| `secure_data` | 不透明なオクテット列。書いた値がそのまま送られ、暗号化はしない |
| `icv` | 不透明な 16 オクテット。計算しない |

解析ではフレームの最後の 16 オクテットを ICV、SecTAG と ICV の間をセキュアデータとします。Ethernet の最小長より短いフレームは (実際の回線と同じく) ICV の後に詰め物が付き、解析し直すと詰め物はセキュアデータ / ICV に含まれます。完全性保護のみのフレーム (E = C = 0) もそれ以上はデコードせず、ユーザデータは `secure_data` に残ります。

## GTPv2-C: 1 レイヤ = 1 メッセージ

`gtpv2` レイヤはヘッダ (`version` 2、`p`、`t`、`mp`、`message_type`、`message_length`、`teid`、`sequence_number`、`message_priority`) と IE (`ies`) です。

- `t` は `teid` から Auto: `teid: [0x1001]` で TEID を送り、`teid: []` (Echo) で省きます。
- `message_length` は Auto (先頭 4 オクテットより後のオクテット数)。IE はこの長さで終わるので、ピギーバックされたメッセージ (`p: 1`) は次の `gtpv2` レイヤになります。
- IE は 8 ビット型、IE データの 16 ビット長、長さに数えないスペア / インスタンスの 1 オクテット (wpg-core `TlvDef::uncounted_octets`) です。型付きの IE はどれも先頭に `spare` と `instance` フィールドを持ちます。

```yaml
- protocol: gtpv2
  fields:
    message_type: create_session_request
    teid: [0]
    ies:
      - imsi: { digits: "00 01 01 21 43 65 87 f9" }   # TBCD
      - fteid: { interface_type: s11_mme_gtp_c, teid: 0x1001, ipv4: 192.0.2.1 }
      - apn: { apn: "08 69 6e 74 65 72 6e 65 74" }    # DNS ラベル
      - bearer_context:
          ies:
            - ebi: { ebi: 5 }
            - bearer_qos: { pl: 9, qci: 9 }
```

| 型 | IE (`name`) とデータのフィールド |
|---|---|
| 1 / 75 / 76 | `imsi` / `mei` / `msisdn` (`digits`、TBCD のオクテット列) |
| 2 | `cause` (`cause`、`pce`、`bce`、`cs`、`offending_ie`) |
| 3 | `recovery` (`restart_counter`) |
| 71 | `apn` (`apn`、DNS ラベルのオクテット列) |
| 72 | `ambr` (`uplink`、`downlink`、kbps) |
| 73 | `ebi` (`ebi`) |
| 74 | `ip_address` / `ip_address_ipv6` (`address`)。IE 長で判別 |
| 77 / 78 / 84 / 86 | `indication` (`flags`)、`pco`、`bearer_tft`、`uli` (`data` / `tft` のオクテット列) |
| 79 | `paa` / `paa_ipv6` / `paa_ipv4v6` (`pdn_type`、`ipv6_prefix_length`、`ipv6`、`ipv4`)。IE 長で判別 |
| 80 | `bearer_qos` (`pci`、`pl`、`pvi`、`qci`、`mbr_uplink`、`mbr_downlink`、`gbr_uplink`、`gbr_downlink`) |
| 82 | `rat_type` |
| 83 | `serving_network` (MCC / MNC の各桁) |
| 87 | `fteid` / `fteid_ipv6` / `fteid_ipv4v6` (`v4`、`v6`、`interface_type`、`teid`、`ipv4`、`ipv6`)。IE 長で判別 |
| 93 | `bearer_context` (`ies`: 上の IE。1 段のみ) |
| 94 / 95 | `charging_id`、`charging_characteristics` |
| 99 | `pdn_type` |
| 114 / 127 / 128 | `ue_time_zone`、`apn_restriction`、`selection_mode` |
| 255 | `private_extension` (`enterprise_id`、`value`) |

その他の IE は `{ type: N, data: "..." }` (データはインスタンスのオクテットから始まる) で保持します。GTPv1-C (バージョン 1、同じ UDP 2123) はモデル化していません。そのメッセージはバージョンの警告付きで `gtpv2` としてデコードされます。

## Diameter: ヘッダと AVP

`diameter` レイヤはヘッダ (`version`、`message_length` (Auto)、`r` / `p` / `e` / `t` フラグ、`command_code`、`application_id`、`hop_by_hop`、`end_to_end`) と AVP (`avps`) です。AVP はメッセージ長で終わるので、同じ TCP セグメント内の 2 つ目のメッセージは次のレイヤ (`raw`) になります。

AVP は 32 ビットのコード、フラグのオクテット (コードと長さの間: wpg-core `TlvDef::head_octets`)、ヘッダとデータを数え詰め物を数えない 24 ビット長 (Auto)、V フラグが 1 のときの Vendor-ID、4 オクテット境界まで詰めたデータ (Auto) です。型付きの AVP はどれも `v` (`vendor_id` から Auto)、`m`、`p`、`reserved`、`vendor_id` (`[]` または `[10415]`) の後にデータのフィールドを持ちます。

| データ型 | AVP (`name`) | データのフィールド |
|---|---|---|
| OctetString / UTF8String / DiameterIdentity / DiameterURI | `session_id`、`origin_host`、`origin_realm`、`destination_host`、`destination_realm`、`product_name`、`user_name`、`error_message`、`route_record`、`proxy_host`、`proxy_state`、`class`、`redirect_host`、`service_context_id`、`subscription_id_data` など | `value` (オクテット列。テキストは `'"..."'`) |
| Unsigned32 / Time | `vendor_id`、`supported_vendor_id`、`auth_application_id`、`acct_application_id`、`origin_state_id`、`firmware_revision`、`session_timeout`、`experimental_result_code`、`cc_request_number`、`event_timestamp` など | `value` |
| Unsigned64 | `accounting_sub_session_id` | `value` |
| Enumerated | `result_code`、`disconnect_cause`、`auth_session_state`、`auth_request_type`、`re_auth_request_type`、`termination_cause`、`accounting_record_type`、`cc_request_type`、`subscription_id_type`、`multiple_services_indicator` | `value` (名前で指定可) |
| Address | `host_ip_address` / `host_ip_address_ipv6`。AVP 長で判別 | `address_family`、`address` |
| Grouped | `vendor_specific_application_id`、`failed_avp`、`proxy_info`、`experimental_result`、`subscription_id` | `avps` (上の AVP。1 段のみ) |

一覧にないコードのベンダ固有 AVP (3GPP など) は、フラグのオクテット・Vendor-ID・値を `data` に含めた `{ type: N, data: "..." }` で保持します。SCTP 上の Diameter はメッセージのオクテット列を SCTP DATA チャンクのユーザデータに入れて作ります。SCTP はユーザデータをデコードしません ([SCTP](sctp.md) を参照)。

## SIP: テキストの行

`sip` レイヤはテキストです。`start_line` と `headers` の各要素は CRLF で終わる行 (wpg-core `Length::Until`) で、CRLF なしで書くと合成時に CRLF とヘッダ後の空行が付きます。`body` はメッセージの残り (SDP のオファー / アンサーなど) です。

```yaml
- protocol: sip
  fields:
    start_line: '"INVITE sip:bob@example.com SIP/2.0"'
    headers:
      - '"Via: SIP/2.0/UDP 192.0.2.1:5060;branch=z9hG4bK74bf9"'
      - '"Content-Type: application/sdp"'
      - '"Content-Length: 132"'
    body: '"v=0\r\no=alice ...\r\n"'
```

`Content-Length` は通常のヘッダ行で手動のままです。本文に合わせてください (サンプルは合わせてあります)。空行のないメッセージは `sip` としてデコードされません。

## RTP と RTCP

RTP には決まったポートがなく、セッションのポートは SDP で決まります。`udp` の後に `rtp` / `rtcp_*` レイヤを明示的に置いてください。解析では IANA の `rtp-avp` / `rtcp-avp` ポート 5004 / 5005 だけを RTP / RTCP として選びます (tshark でも `-d udp.port==N,rtp` の指定が必要です)。

`rtp` は固定ヘッダ、`csrc` (数 `cc` は Auto)、`header_extension` (`[]` または `[{ profile, data }]`。X ビットとワード単位の長さは Auto、`data` は 32 ビットワードのリスト、`profile` 0xBEDE は RFC 8285 の 1 バイト要素) です。メディアのペイロードは次のレイヤ (`payload` / `raw`) です。パディングビットと詰め物は手動です。

RTCP は 1 パケット 1 レイヤなので、複合パケットは RTCP レイヤを重ねて作ります。件数 (`report_count` / `source_count`) と `length` (32 ビットワード数 − 1) は Auto で、各パケットはその長さで終わります。

| レイヤ | パケットタイプ | 本体のフィールド |
|---|---|---|
| `rtcp_sr` | 200 | `ssrc`、`ntp_timestamp_msw` / `lsw`、`rtp_timestamp`、`packet_count`、`octet_count`、`report_blocks`、`extension` |
| `rtcp_rr` | 201 | `ssrc`、`report_blocks`、`extension` |
| `rtcp_sdes` | 202 | `chunks`: `[{ ssrc, items: [{ type, text }] }]` (項目の長さ、null オクテット、詰め物は Auto) |
| `rtcp_bye` | 203 | `ssrcs`、`reason` (`[]` または `[{ text }]`) |
| `rtcp_app` | 204 | `subtype` (件数フィールド)、`ssrc`、`name` (4 オクテット)、`data` |
| `rtcp` | 任意 | `count`、`body` (オクテット列)。解析時は 200〜204 なら型付きのパケットとして読み直す |

## 対象外

- MACsec の暗号化 / ICV 計算、XPN (64 ビットのパケット番号)、完全性保護のみのユーザデータのデコード
- GTPv1-C、上の表で型付けしていない GTPv2-C の IE (ULI、Indication のフラグ、PCO の中身はオクテット列)、入れ子の Bearer Context
- Diameter のベンダ固有 AVP 辞書 (3GPP の S6a / Gx / Gy など)、2 段以上のグループ化 AVP、SCTP DATA チャンクからの Diameter の demux
- SIP の `Content-Length` の Auto、ヘッダの名前 / 値への分解、SIP over TLS (5061)、マルチパート本文
- RTP のペイロード形式、SRTP、RTCP のフィードバック (RTPFB / PSFB) と XR の本体 (`rtcp` でオクテット列として保持)
