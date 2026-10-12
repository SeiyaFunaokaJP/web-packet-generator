# FHRP (ファーストホップ冗長化)

Status: draft

VRRPv2 / v3、HSRPv1 / v2、GLBP (基本) の補足です。定義は `crates/wpg-protocols/src/fhrp.rs` にあります。

## 識別と積み方

| プロトコル ID | 下位レイヤ | 識別子 | 宛先 |
|---|---|---|---|
| `vrrpv2` | `ipv4` | プロトコル 112 (version 2) | 224.0.0.18、TTL 255 |
| `vrrpv3` | `ipv4` | プロトコル 112 (version 3) | 224.0.0.18、TTL 255 |
| `vrrpv3_ipv6` | `ipv6` | 次ヘッダ 112 | `ff02::12`、Hop Limit 255 |
| `hsrp` (v1) | `udp` | ポート 1985 | 224.0.0.2 |
| `hsrpv2` | `udp` | ポート 1985 (IPv4) / 2029 (IPv6) | 224.0.0.102 / `ff02::66` |
| `glbp` | `udp` | ポート 3222 | 224.0.0.102 |

同じ番号を共有するものは解析時に読み直します。

- IPv4 プロトコル 112 はまず `vrrpv3` として読み、`version` が 2 なら `vrrpv2` として読み直します。
- UDP 1985 はまず `hsrp` (v1) として読み、先頭オクテットが 1 (HSRPv2 の Group State TLV) なら `hsrpv2` として読み直します。

合成時はレイヤに書いたプロトコルがそのまま使われ、IP プロトコル番号 / UDP ポートは Auto で決まります。

## Auto フィールド

| プロトコル | Auto |
|---|---|
| `vrrpv2` | `count_ip_addrs` (アドレス数)、`checksum` (疑似ヘッダなし) |
| `vrrpv3` / `vrrpv3_ipv6` | `count_ipvx_addrs`、`checksum` (IPv4 / IPv6 疑似ヘッダ付き) |

## 制約と回避方法

- **VRRPv3** の仮想アドレスは、外側の IP のバージョンによって IPv4 か IPv6 になります。下位レイヤからフィールド幅を決められないため、IPv4 用の `vrrpv3` と IPv6 用の `vrrpv3_ipv6` の 2 つの定義に分けています。IPv6 では先頭のアドレスがリンクローカルの仮想アドレスです。
- タイマの単位に注意してください。VRRPv2 の広告間隔は秒、VRRPv3 の `max_adver_int` はセンチ秒、HSRPv1 は秒、HSRPv2 の Group State TLV と GLBP の Hello TLV はミリ秒です。
- **HSRPv1** の `auth_data` は 8 オクテットの平文パスワード (既定 `cisco`) です。
- **HSRPv2** の 16 オクテットの仮想アドレスは `ip_version` によって IPv4 (ゼロ埋め) か IPv6 になるため、opaque な 16 バイトです。MD5 認証 TLV (type 4) も opaque です。
- **GLBP** は公開仕様がないため、Hello TLV (IPv4 仮想アドレス) だけを型付きにしています。Request/Response TLV と認証 TLV は値のバイト列のままです。

## サンプル

| テンプレート | 内容 |
|---|---|
| `templates/protocols/fhrp/vrrpv2.wpg.yaml` | マスタからの VRRPv2 Advertisement |
| `templates/protocols/fhrp/vrrpv3.wpg.yaml` / `vrrpv3-ipv6.wpg.yaml` | VRRPv3 Advertisement (IPv4 / IPv6) |
| `templates/protocols/fhrp/hsrpv1.wpg.yaml` | Active ルータの HSRPv1 Hello |
| `templates/protocols/fhrp/hsrpv2.wpg.yaml` | Group State TLV + テキスト認証の HSRPv2 Hello |
| `templates/protocols/fhrp/glbp.wpg.yaml` | AVG の GLBP Hello |
