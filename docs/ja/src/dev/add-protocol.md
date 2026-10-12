# プロトコルの追加手順

Status: draft

コアのプロトコルは Rust マクロで、ユーザ拡張は `protocols.d/` の YAML で定義し、どちらも同一の IR にコンパイルされます。IR から合成、解析、GUI エディタ、リファレンスドキュメントが生成されます。新しいプロトコルには、定義、サンプルテンプレート、golden pcap、可能であれば tshark 突合テストが必要です。これにより、プロトコル追加のコストを一定に保ちます。

## 定義の部品

近似 (opaque なバイト列や Manual 固定) に逃げる前に、次の部品で表現できないか確認してください。

| 目的 | 部品 | 例 |
|---|---|---|
| 要素数フィールド | `auto = count(list_field)` (リスト要素・TLV 値の中でも可) | IGMPv3 `num_group_records`、VRRP `count_ip_addrs` |
| 一部を除いたチェックサム | `auto = checksum(kind, coverage, exclude = [field])` | OSPFv2 `authentication`、LSA `ls_age` |
| 後続の可変長フィールドのオクテット数 | `auto = size(field)` (リスト要素・TLV 値の中でも可) | IS-IS のサブ TLV 長 |
| `ceil(ビット数 / 8)` オクテットのプレフィックス | `bytes(from = len, adjust = 7, div = 8)` (`Length::FromField { div }`) | IS-IS 拡張 IP / IPv6 到達性 |
| 可変長要素 N 個のリスト | `Length::Count { field }` | IS-IS の省略可能なサブ TLV ブロック (S ビット) |
| 可変長フィールドの後、ヘッダ長フィールドの位置で終わる領域 | `Length::UntilOffset { field, unit, adjust }` | 長さ指示子までの CLNP / ES-IS オプション、GTPv2-C の IE (長さは 5 オクテット目から: `adjust = 4`)、RTCP (ワード数 − 1: `unit = 4, adjust = 4`) |
| 末尾の N オクテットを残す領域 | `Length::ToEndExcept { octets }` | ICV の前までの MACsec セキュアデータ |
| 終端オクテット列で終わるテキスト / リスト | `Length::Until { terminator }` (`bytes`: 終端まで、値は終端を含まない。`list`: 要素の先頭が終端になるまで) | SIP の CRLF で終わる行と空行、null オクテットで終わる RTCP SDES 項目 |
| TLV の型と長さの間にある値オクテット | `TlvDef::head_octets` | SCTP チャンクのフラグ |
| N オクテット境界まで詰める TLV 項目 (詰め物は長さに含めない) | `TlvDef::align` (`unpadded_last`: 最後の項目は外側の項目が詰める) | SCTP のチャンク・パラメータ・エラー原因 |
| 同じタイプで値によってレイアウトが変わる TLV 項目 | `TlvVariant::when_value_starts` / `when_value_len` (タイプを共有するバリアント。判別条件のないものが既定) | AFI / SAFI ごとの BGP `MP_REACH_NLRI`、3 / 4 オクテットの OSPF SR の SID、EVPN の IPv4 / IPv6 IP Prefix ルート |
| 長さが型より前にある TLV 項目 | `TlvDef::len_first` (`head_octets` / `long_length` とは併用不可) | RSVP オブジェクト (長さ、Class-Num、C-Type) |
| 長さの直後にあり長さに数えない TLV の値オクテット | `TlvDef::uncounted_octets` (ヘッダを含まない長さのときのみ) | GTPv2-C IE のスペア / インスタンスのオクテット |
| TLV の長さを広げるタイプのフラグ | `TlvDef::long_length` (`TlvLongLength { flag, len_bits }`。値が長すぎると自動で付く) | BGP の Extended Length パス属性 |
| 指定 PDU 長まで埋め草 TLV で埋める | `TlvDef::fill` (`TlvFill { variant, until }`) | IS-IS Hello の Padding TLV |
| NUL 終端文字列 | `cstring` 型 | TFTP `filename` / `mode` |
| 長さ指示子付き OSI アドレス | `nsap` 型 (長さオクテットは自動) | CLNP / ES-IS / IS-IS の NSAP・NET |
| 送信元ポートでも demux | `Binding::alt_field` | UDP / TCP の `src_port` |
| ペイロード先頭ビットで demux | `Binding::peek` (`Peek { on, bits, table }`) | MPLS、GTP-U、OSI (NLPID) |
| 長さだけが違うメッセージ | `BindingEntry::with_min_len` | IGMPv3 / MLDv2 クエリ |
| 同じ番号でレイアウト違い | `BindingEntry::reparsing` | VRRPv2/v3、HSRPv1/v2、TFTP |

評価順などの詳細は仕様 `docs/spec/02-packet-builder.md` の「次プロトコルの選択」を参照してください。
