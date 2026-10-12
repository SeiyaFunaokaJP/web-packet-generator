# OSI カプセル化 (P2)

Status: draft

CLNP / ES-IS / IS-IS に共通する OSI ネットワーク層のカプセル化と、NSAP アドレス型の補足です (M7)。定義は `crates/wpg-protocols/src/osi.rs`、サンプルは `templates/protocols/l2/dot3-llc-osi.wpg.yaml` にあります。

## 到達経路

| 下位レイヤ | 選択子 | 上位レイヤ |
|---|---|---|
| `llc` | DSAP / SSAP `0xfe` | PDU 先頭の NLPID オクテットで判別 |
| `ppp` | protocol `0x0023` (RFC 1377) | 同上 |

OSI の PDU の前には「次は何か」を示す番号がなく、PDU 自身の先頭オクテット NLPID (ISO/IEC TR 9577) が種類を表します (`0x81` CLNP、`0x82` ES-IS、`0x83` IS-IS)。そのため `llc` と `ppp` はペイロードの先頭 8 ビットを覗いて (`Binding::peek`) 次のレイヤを決めます。専用の定義がまだない NLPID は汎用 PDU `osi` (NLPID + 残りの不透明なバイト列) としてデコードされます。CLNP / ES-IS / IS-IS の定義が追加されると、この peek の表に登録されます。

ビルド時は `llc` の上に `osi` (将来は `clnp` などの各 PDU) を積むだけで、DSAP / SSAP は Auto で `0xfe` になります。PPP の `protocol` も同様に Auto で `0x0023` になります。

## NSAP 型

- `nsap` 型の値はアドレス本体 (1..=20 オクテット) で、ワイヤ上では OSI の慣例どおり 1 オクテットの長さ指示子が前に付きます。長さ指示子は合成時に自動で書かれ、解析時に読まれます。
- 表記は `49.0001.1921.6800.1001.00` のような 16 進 (`.` と空白は無視、`0x` 接頭辞可)。
- GUI のフィールドエディタは入力中の NSAP を AFI / エリア / System ID (6 オクテット) / NSEL に分解して表示します (8 オクテット未満のアドレスは分解しません)。
- ジェネレータ `range` は System ID 部分を増分します (例 `49.0001.1921.6800.1001.00-49.0001.1921.6800.1004.00`)。

## 制約

- `osi` の NLPID は Auto ではありません。CLNP / ES-IS / IS-IS の各定義が追加されるまでは、本体を `data` に 16 進で与えてください。
