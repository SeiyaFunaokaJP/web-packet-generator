# §2.10 SNMP マネージャ (wpg-snmp)

Status: draft。決定事項は [ADR-0010](../adr/0010-snmp-manager.md)。利用者向けの説明は `docs/{ja,en}/src/operations/snmp.md`。

## 確定した仕様

- crate `wpg-snmp` (純 Rust): BER (長形式長)、SNMPv1 / v2c / v3 メッセージ、全 PDU (Get, GetNext, GetBulk, Set, Response, v1 Trap, SNMPv2-Trap, Inform, Report)、全 SMI 値 (INTEGER, OCTET STRING, NULL, OID, IpAddress, Counter32, Gauge32, TimeTicks, Opaque, Counter64, noSuchObject / noSuchInstance / endOfMibView)。
- USM: noAuthNoPriv / authNoPriv / authPriv。認証 HMAC-MD5-96, HMAC-SHA-96, HMAC-SHA-2 (RFC 7860: 224/256/384/512)。暗号 CBC-DES (RFC 3414), CFB128-AES-128 (RFC 3826), AES-192/256 (draft-blumenthal-aes-usm-04 の鍵拡張)。パスフレーズは 8 文字以上。
- クライアント: タイムアウトと再送 (同じ request-id)、request-id / msgID 照合、v3 のエンジン探索 (空 Get → Report)、notInTimeWindow / unknownEngineID の Report で 1 回だけ再同期して再送。Walk は v2c/v3 で GetBulk、v1 で GetNext。サブツリー外・endOfMibView・非増加 OID・上限で停止し、空ならルートを Get する。
- 通知: v1 Trap-PDU、SNMPv2-Trap、Inform (Response を待つ)。v1 宛ての v2 形式通知は RFC 3584 §3.2 で変換。v3 トラップの権威エンジンは送信側 (wpg)、v3 インフォームの権威エンジンは受信側。
- 受信: v1/v2c はコミュニティ許可リスト (不一致もエラー付きで記録)、v3 は設定ユーザー。インフォームには Response を返す。v3 インフォームの探索には usmStatsUnknownEngineIDs の Report で自エンジン ID を返す。時刻窓は検査しない。
- MIB: SMIv1/SMIv2 の寛容なパーサ。OID 代入、MODULE-IDENTITY, OBJECT-IDENTITY, OBJECT-TYPE (SYNTAX、列挙、BITS、サイズ制約、UNITS、MAX-ACCESS、STATUS、DESCRIPTION、INDEX / IMPLIED、AUGMENTS、DEFVAL)、NOTIFICATION-TYPE、TRAP-TYPE (`enterprise.0.n`)、TEXTUAL-CONVENTION (DISPLAY-HINT)、IMPORTS。MACRO 定義と未知の句・マクロは読み飛ばす。名前解決: 自モジュール → IMPORTS (再輸出可) → ASN.1 ルート → 全モジュール (警告)。
- 表示: 最長一致による `MODULE::name.instance`、列挙 `label(n)`、RFC 2579 DISPLAY-HINT (整数 `d-N` `x` `o` `b`、オクテット列の繰り返し指示子・区切り・終端)、TimeTicks `Nd hh:mm:ss.cc`、BITS 名、UNITS、OID 値の名前、INDEX の復号 (整数、IpAddress、固定長 / 長さ前置 / IMPLIED のオクテット列と OID)、Walk 結果の概念表の組み立て。
- pcap: 送受信した UDP データグラムに Ethernet II / IPv4 (または IPv6) / UDP を合成 (チェックサム付き、MAC はプレースホルダ)。pcap (μs) と pcapng (ns)。上限 10 万フレーム、超過分は古い順に破棄。
- サーバ: `/api/snmp/*` (status, get, getnext, getbulk, walk, set, trap, inform, mibs, mibs/{module}, mibs/tree, mibs/node, mibs/search, receiver, receiver/start, receiver/stop, traps, capture, capture/download)。エラーコード `snmp_error` (`details.kind` = `timeout` 504 / `report` 502 / `error` 502)。WebSocket イベント `snmp_trap`。設定 `[snmp] enabled / trap_listen / mib_dir`。アップロード MIB は `<data_dir>/mibs` に保存 (拡張子 .mib .txt .my .smi .asn .asn1、1 ファイル 4 MiB まで)。
- 秘密情報: コミュニティとパスフレーズはリクエスト本体 (と動作中の受信器のメモリ) にのみ存在し、保存・ログ出力しない。

## 未決事項 (Open questions)

- CLI (`wpg snmp get|walk|set|trap`) の要否と形。
- 実機 (Cisco / Juniper / Arista) との v3 相互接続確認の手順を [相互接続確認](../ja/src/operations/interop.md) に加えるか。
- 受信器の v3 時刻窓検査を設定で有効にできるようにするか。
