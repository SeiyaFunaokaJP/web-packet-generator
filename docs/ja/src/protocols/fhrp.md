# 冗長化プロトコル (FHRP)

Status: draft

VRRPv2・v3 / HSRPv1・v2 / GLBP の補足です。定義は `crates/wpg-protocols/src/fhrp.rs`、サンプルは `templates/protocols/fhrp/` にあります。

## 到達経路とサンプル

| id | プロトコル | 下位レイヤ | サンプル |
|---|---|---|---|
| `vrrpv3` | VRRPv3 (IPv4 アドレス) | IPv4 protocol 112 | `vrrpv3.wpg.yaml` |
| `vrrpv2` | VRRPv2 | IPv4 protocol 112 (`version` = 2 で読み直し) | `vrrpv2.wpg.yaml` |
| `vrrpv3_ipv6` | VRRPv3 (IPv6 アドレス) | IPv6 next header 112 | `vrrpv3-ipv6.wpg.yaml` |
| `hsrp` | HSRPv1 | UDP 1985 | `hsrpv1.wpg.yaml` |
| `hsrpv2` | HSRPv2 | UDP 1985 (先頭オクテット 1 で読み直し)、UDP 2029 (IPv6) | `hsrpv2.wpg.yaml` |
| `glbp` | GLBP (基本) | UDP 3222 | `glbp.wpg.yaml` |

## Auto になるフィールド

- VRRP の `checksum` (v2 はインターネットチェックサム、v3 は IP 疑似ヘッダ込み) とアドレス数 (`count_ip_addrs` / `count_ipvx_addrs`)。

## 制約と近似

- **VRRPv2 と v3** は同じ IP protocol 112 で、版数ニブルでしか区別できません。デコードはまず `vrrpv3` で行い、`version` が 2 なら `vrrpv2` として読み直します。ビルド時は目的の定義を選んでください。
- **VRRPv3 のアドレス種別**は外側の IP の版で決まりますが、下位レイヤからフィールドの大きさを決める仕組みがないため、`vrrpv3` (IPv4) と `vrrpv3_ipv6` (IPv6) の 2 定義に分かれています。
- **HSRPv1 と v2** は同じ UDP 1985 を使います。先頭オクテットが v1 では版数 (0)、v2 では最初の TLV の type (1 = Group State) なので、`hsrp` でデコードして 1 なら `hsrpv2` として読み直します。HSRPv2 の 16 オクテットの仮想アドレスは `ip_version` によって IPv4 (ゼロ詰め) か IPv6 になるため、不透明な `bytes(16)` です。
- **GLBP** は非公開仕様のため、型付けされているのは Hello TLV (IPv4 仮想アドレス) のみです。Request/Response と Authentication TLV は値のオクテットをそのまま保持します。
- 認証は平文のみ型付けされています: VRRPv2 の `auth_data` (8 オクテット)、HSRPv1 の `auth_data` と HSRPv2 の Text Authentication TLV (どちらも既定 `cisco`)。HSRPv2 の MD5 認証 TLV は値のオクテットのままで、ダイジェストは自動計算されません。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`vrrpv2`](../reference/protocols/vrrpv2.md) · [`vrrpv3`](../reference/protocols/vrrpv3.md) · [`vrrpv3_ipv6`](../reference/protocols/vrrpv3_ipv6.md) · [`hsrp`](../reference/protocols/hsrp.md) · [`hsrpv2`](../reference/protocols/hsrpv2.md) · [`glbp`](../reference/protocols/glbp.md)
<!-- wpg-docs-gen:end reference-links -->
