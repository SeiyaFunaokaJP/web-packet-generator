# ADR-0010: SNMP マネージャ機能 (wpg-snmp)

> Add a real SNMP manager (Get / Walk / Set / traps, SNMPv3 USM, MIB parser) as a pure-Rust crate `wpg-snmp`, exposed under `/api/snmp` and a new SNMP screen.

- Status: Accepted
- Date: 2026-10-27
- Deciders: wpg maintainers

## Context

wpg は `wpg-protocols` の `snmp` / `snmp_pdu` / `snmp_trap` で SNMP パケットを「作る」ことはできるが、エージェントと会話する (応答を待つ、Walk する、v3 で認証・暗号化する、トラップを受ける) ことはできなかった。ネットワーク検証では「パケットを流しながら機器のカウンタを SNMP で読む」「トラップが飛ぶか確かめる」ことが頻繁にあり、別ツール (net-snmp、MIB ブラウザ) を併用していた。MIB の名前・列挙値・表示ヒントを使った読める表示と、やり取りを pcap に残す機能も求められている。

## Decision Drivers

- 既存のパケット生成エンジンとは別物 (状態を持つクライアント / サーバ) で、`wpg-protocols` の短形式 BER の制約に縛られないこと
- 純 Rust (C の net-snmp / libsmi に依存しない。Windows / macOS / Linux で同じビルド)
- SNMPv3 USM (MD5 / SHA-1 / SHA-2、DES / AES) を RFC のテストベクタで検証できること
- 実機ベンダ MIB の癖 (コメント、未知マクロ、IMPORTS 漏れ) で落ちないこと
- パスフレーズをサーバに保存しないこと

## Decision

- 新しい crate `crates/wpg-snmp` を追加する (`default-members` にも入れる)。内容は BER (長形式長), 全 PDU と SMI 値, v1/v2c メッセージ, SNMPv3 (RFC 3412) と USM (RFC 3414 / 3826 / 7860。AES-192/256 は draft-blumenthal-aes-usm-04 の鍵拡張), tokio の UDP クライアント (get / getnext / getbulk / walk / set / trap / inform、再送とリクエスト ID 照合、v3 エンジン探索と時刻同期), トラップ / インフォーム受信, SMIv1/SMIv2 MIB パーサとレジストリ, 値の整形, pcap / pcapng 記録, テスト用の小さなエージェント。
- 暗号はハッシュとブロック暗号のプリミティブだけ RustCrypto (`md-5`, `sha1`, `sha2`, `des`, `aes`) を使い、HMAC と CBC / CFB は自前で書く (数十行、RFC 2202 / 4231 / FIPS 197 / FIPS 81 の既知解でテスト)。
- MIB パーサは手書きの寛容な再帰下降。理解できない文は警告として記録し、次の代入から再開する。名前解決は自モジュール → IMPORTS (再輸出を含む) → ASN.1 ルート → 全ロード済みモジュール (警告付き) の順。不足モジュールは「missing imports」として名前で報告する。
- IETF の標準 MIB (SNMPv2-SMI/TC/CONF/MIB, RFC1213-MIB, IF-MIB, IP-MIB, HOST-RESOURCES-MIB, ENTITY-MIB, SNMP-FRAMEWORK-MIB など) を同梱する。RFC から抽出した MIB モジュールは再配布可能。ベンダ MIB は同梱せず、ユーザがアップロードして `<data_dir>/mibs` に保存する。
- サーバは `/api/snmp/...` を追加し、設定 `[snmp]` (`enabled`, `trap_listen`, `mib_dir`) を持つ。v3 のパスフレーズは各リクエスト (または動作中の受信器のメモリ) にのみ存在し、保存もログ出力もしない。Web UI のプロファイルもコミュニティとパスフレーズを含まない。
- pcap は UDP ソケットの上で送受信したデータグラムに Ethernet / IP / UDP ヘッダを合成して作る (MAC はプレースホルダ)。生パケットのキャプチャ権限は不要。

## Alternatives considered

- `wpg-protocols` の `snmp` 定義を拡張して送受信する: パケット定義エンジンは状態 (要求 ID、エンジン時刻、暗号) を持たず、長形式長も扱わないため不適。
- net-snmp / libsmi への FFI: C ツールチェーンと各 OS のライブラリが必要になり、配布方針 (単一バイナリ) に反する。
- 既存の Rust crate (`snmp`, `csnmp`, `snmp2` 等): v3 USM、MIB、トラップ受信、pcap 記録をまとめて満たすものがなく、保守状況もまちまち。プロトコル部分は大きくないため自前で持つ。
- MIB を事前コンパイル (smidump 等で JSON 化) して同梱: アップロードされたベンダ MIB をその場で読めないため不可。

## Consequences

- 標準 MIB とアップロード MIB の名前で Get / Walk / Set / トラップを扱える。表 (ifTable 等) は INDEX で行に組み立てて表示する。
- v3 は RFC 3414 の鍵ローカライズのテストベクタと、全セキュリティレベル × 全プロトコルの往復テスト、インプロセスのエージェントとの結合テストで検証する。実機 (Cisco / Juniper など) との相互接続確認はフォローアップ。
- 受信器は時刻窓 (notInTimeWindow) を強制しない (試験ツールとして遅れたメッセージも表示する)。noAuthNoPriv の v3 メッセージは任意のユーザ名で受け付ける。
- UDP 162 で待ち受けるには root か `CAP_NET_BIND_SERVICE` が必要。権限がない場合は明示的なエラーと 1162 などの代替を案内する。
- MIB パーサは厳密な SMI 検証器ではない (構文の誤りを正す目的ではなく、名前と型を引くため)。
