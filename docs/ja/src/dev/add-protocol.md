# プロトコルの追加手順

コアのプロトコルは Rust マクロ (`crates/wpg-protocols/src/`) で定義し、同一の IR にコンパイルされます。IR から合成、解析、GUI エディタ、リファレンスドキュメントが生成されます。新しいプロトコルには、定義、Fluent 文字列、サンプルテンプレート、golden pcap、可能であれば tshark 突合テストが必要です。これにより、プロトコル追加のコストを一定に保ちます。

## 手順

1. **定義する。** `crates/wpg-protocols/src/` の該当モジュール (例: アプリ層なら `app.rs`) にマクロでプロトコルを足し、カタログに登録します。フィールドには `desc = "key"` で Fluent キーを付けます。近似に逃げる前に、下の「定義の部品」で表現できないか確認してください。
2. **Fluent 文字列を足す。** プロトコル名と各フィールドの `desc` キーを `locales/ja/main.ftl` と `locales/en/main.ftl` の両方に追加します ([i18n 文字列の追加手順](add-i18n.md))。
3. **サンプルテンプレートを置く。** `templates/protocols/<group>/` に `.wpg.yaml` を 1 つ以上追加します。`descriptions` の ja / en と `protocol:` / `layer:` タグを付けます。`samples.rs` のテストが、全プロトコルにサンプルがあること、整形式サンプルが同じスタックに読み戻せることを検査します。
4. **golden を再生成する。** `just golden-update` で `tests/golden/*.hex` / `*.pcap` を作り直し、差分を確認します。`.hex` / `.pcap` は手で編集しないでください。
5. **突合する。** tshark があれば `WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored` で Wireshark と突き合わせます。
6. **補足ドキュメントを更新する。** 近似が残る場合やカプセル化が特殊な場合は `docs/{ja,en}/src/protocols/` に ja / en そろえて追記します。
7. **リファレンスを再生成する。** `just docs-gen` で[プロトコルリファレンス](../reference/protocols.md)などを作り直してコミットします (下の「リファレンスドキュメント」)。
8. **プッシュ前のチェック** (CLAUDE.md) をすべて通します。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown
```

## 定義の部品

近似 (opaque なバイト列や Manual 固定) に逃げる前に、次の部品で表現できないか確認してください。

| 目的 | 部品 | 例 |
|---|---|---|
| 要素数フィールド | `auto = count(list_field)` (リスト要素・TLV 値の中でも可。`count(list_field, adjust = -1)` で「最後の要素の位置」) | IGMPv3 `num_group_records`、VRRP `count_ip_addrs`、SRH `last_entry` |
| 一部を除いたチェックサム | `auto = checksum(kind, coverage, exclude = [field])` | OSPFv2 `authentication`、LSA `ls_age` |
| 後続の可変長フィールドのオクテット数 | `auto = size(field)` (リスト要素・TLV 値の中でも可) | IS-IS のサブ TLV 長 |
| `ceil(ビット数 / 8)` オクテットのプレフィックス | `bytes(from = len, adjust = 7, div = 8)` (`Length::FromField { div }`) | IS-IS 拡張 IP / IPv6 到達性 |
| 可変長要素 N 個のリスト | `Length::Count { field }` | IS-IS の省略可能なサブ TLV ブロック (S ビット) |
| 可変長フィールドの後、ヘッダ長フィールドの位置で終わる領域 | `Length::UntilOffset { field, unit, adjust }` (終わりは `field * unit + adjust`) | 長さ指示子までの CLNP / ES-IS オプション、SRH の TLV (`hdr_ext_len * 8 + 8`)、GTPv2-C の IE (長さは 5 オクテット目から: `adjust = 4`)、RTCP (ワード数 − 1: `unit = 4, adjust = 4`) |
| 末尾の N オクテットを残す領域 | `Length::ToEndExcept { octets }` | ICV の前までの MACsec セキュアデータ |
| 終端オクテット列で終わるテキスト / リスト | `Length::Until { terminator }` (`bytes`: 終端まで、値は終端を含まない。`list`: 要素の先頭が終端になるまで) | SIP の CRLF で終わる行と空行、null オクテットで終わる RTCP SDES 項目 |
| TLV の型と長さの間にある値オクテット | `TlvDef::head_octets` | SCTP チャンクのフラグ |
| N オクテット境界まで詰める TLV 項目 (詰め物は長さに含めない) | `TlvDef::align` (`unpadded_last`: 最後の項目は外側の項目が詰める) | SCTP のチャンク・パラメータ・エラー原因 |
| 同じタイプで値によってレイアウトが変わる TLV 項目 | `TlvVariant::when_value_starts` / `when_value_len` (タイプを共有するバリアント。判別条件のないものが既定) | AFI / SAFI ごとの BGP `MP_REACH_NLRI`、3 / 4 オクテットの OSPF SR の SID、EVPN の IPv4 / IPv6 IP Prefix ルート |
| 長さが型より前にある TLV 項目 | `TlvDef::len_first` (`head_octets` / `long_length` とは併用不可) | RSVP オブジェクト (長さ、Class-Num、C-Type) |
| 長さの直後にあり長さに数えない TLV の値オクテット | `TlvDef::uncounted_octets` (ヘッダを含まない長さのときのみ) | GTPv2-C IE のスペア / インスタンスのオクテット |
| End 項目で終わり、その後に詰め物が続く TLV 列 | `Length::UntilTlv { type_value }` (`tlv` フィールドのみ) | CFM、802.3ah OAM の TLV (End TLV 0) |
| TLV の長さを広げるタイプのフラグ | `TlvDef::long_length` (`TlvLongLength { flag, len_bits }`。値が長すぎると自動で付く) | BGP の Extended Length パス属性 |
| 指定 PDU 長まで埋め草 TLV で埋める | `TlvDef::fill` (`TlvFill { variant, until }`) | IS-IS Hello の Padding TLV |
| NUL 終端文字列 | `cstring` 型 | TFTP `filename` / `mode` |
| 長さ指示子付き OSI アドレス | `nsap` 型 (長さオクテットは自動) | CLNP / ES-IS / IS-IS の NSAP・NET |
| 送信元ポートでも demux | `Binding::alt_field` | UDP / TCP の `src_port` |
| ペイロード先頭ビットで demux | `Binding::peek` (`Peek { on, bits, table }`。`skipping(n)` で先頭 `n` ビットを飛ばしてから読む) | MPLS、GTP-U、OSI (NLPID)、IPv6 ルーティングタイプ (`skip = 16`) |
| 上位層チェックサムの宛先を置き換えるルーティングヘッダ | `ProtocolDef::final_destination` (`FinalDestination { list, when }`: `when` が 0 以外のとき `list` の要素 0 を疑似ヘッダの宛先に) | SRv6 SRH |
| 長さだけが違うメッセージ | `BindingEntry::with_min_len` | IGMPv3 / MLDv2 クエリ |
| 同じ番号でレイアウト違い | `BindingEntry::reparsing` | VRRPv2/v3、HSRPv1/v2、TFTP |

評価順などの詳細は仕様 `docs/spec/02-packet-builder.md` の「次プロトコルの選択」を参照してください。

## リファレンスドキュメント

[プロトコルリファレンス](../reference/protocols.md) は定義から自動生成します。`just docs-gen` (`cargo run -p wpg-cli -- docs gen`) を実行して結果をコミットしてください。新しいプロトコル、フィールド、バインディング、サンプルテンプレートは自動で反映され、フィールド説明は `desc = "..."` の Fluent キー (ja / en) から取ります。手書きの補足は `docs/{ja,en}/src/protocols/` に書き、プロトコル ID をバッククォートで書けば (`` `isis_l1_lsp` ``)、生成ツールが双方向のリンクを張ります。コミット済みのページが古いと、テスト `generated_docs_are_up_to_date` (wpg-cli) が失敗します。
