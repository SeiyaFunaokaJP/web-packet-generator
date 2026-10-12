# 冗長化プロトコル (P1)

Status: draft

定義: `crates/wpg-protocols/src/fhrp.rs` / テスト: `crates/wpg-protocols/tests/p1_fhrp_mcast.rs` / サンプル: `templates/protocols/fhrp/`

## 対象と到達方法

| id | プロトコル | 下位からの到達 | 宛先 | サンプル |
|---|---|---|---|---|
| `vrrpv2` | VRRPv2 (RFC 3768) | IPv4 プロトコル 112 (`version` 2) | `224.0.0.18` | `vrrpv2` |
| `vrrpv3` | VRRPv3 for IPv4 (RFC 5798) | IPv4 プロトコル 112 | `224.0.0.18` | `vrrpv3` |
| `vrrpv3_ipv6` | VRRPv3 for IPv6 (RFC 5798) | IPv6 次ヘッダ 112 | `ff02::12` | `vrrpv3-ipv6` |
| `hsrp` | HSRPv1 (RFC 2281) | UDP 1985 | `224.0.0.2` | `hsrpv1` |
| `hsrpv2` | HSRPv2 | UDP 1985 (先頭オクテット 1) / UDP 2029 | `224.0.0.102` / `ff02::66` | `hsrpv2` |
| `glbp` | GLBP (基本) | UDP 3222 | `224.0.0.102` | `glbp` |

## 解析時の判別

- **VRRPv2 / v3**: どちらも IP プロトコル 112 で、バージョンのニブルだけが違います。IPv4 112 はまず `vrrpv3` として解析し、`version` が 2 なら `vrrpv2` として読み直します。
- **HSRPv1 / v2**: どちらも UDP 1985 です。先頭オクテットは v1 ではバージョン (0)、v2 では最初の TLV の type (1、Group State) なので、UDP 1985 はまず `hsrp` として解析し、先頭が 1 なら `hsrpv2` として読み直します。

## Auto フィールド

| フィールド | 内容 |
|---|---|
| `vrrpv2.count_ip_addrs` / `vrrpv3*.count_ipvx_addrs` | `ip_addresses` の要素数 |
| `vrrpv2.checksum` | インターネットチェックサム (疑似ヘッダなし) |
| `vrrpv3.checksum` / `vrrpv3_ipv6.checksum` | 疑似ヘッダ (IPv4 / IPv6) 込みのインターネットチェックサム |

## 制約と近似

- **VRRPv3 の 2 定義**: VRRPv3 のアドレスは外側の IP のバージョンに応じて IPv4 / IPv6 になりますが、下位レイヤでフィールドの大きさを決められないため定義を分けています。IPv4 上では `vrrpv3`、IPv6 上では `vrrpv3_ipv6` を使います。IPv6 版の最初のアドレスはリンクローカルの仮想アドレスです。
- **時間の単位**: VRRPv2 の広告間隔は秒、VRRPv3 の `max_adver_int` (12 ビット) はセンチ秒、HSRP のタイマは秒です。
- **HSRPv1 の認証**: `auth_data` は 8 オクテットの平文パスワードです (既定 `cisco`)。
- **HSRPv2 の仮想アドレス**: 16 オクテットの仮想アドレスは `ip_version` に応じて IPv4 (ゼロ埋め) または IPv6 になるため、opaque な `bytes(16)` です。
- **GLBP**: 仕様が公開されていないため、型付けしているのは Hello TLV (IPv4 仮想アドレス) だけです。Request/Response TLV と Authentication TLV は値をオクテット列のまま保持します。
