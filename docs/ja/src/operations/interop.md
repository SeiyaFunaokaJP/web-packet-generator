# 相互接続確認

Status: draft

`templates/interop/` のテンプレートで、wpg が組み立てたパケットを相手のルータが正しく受け取るかを確かめます。wpg はステートレスな送出器なので (TCP セッションやデータベース同期はしない)、各シナリオは「相手側で観測できる状態の変化」1 つを確かめるものです。

> **検証状況**: 下表の 7 シナリオは、**FRR 8.4.4** に対して `scripts/interop/frr-netns.sh` (ネットワーク名前空間 + veth、afpacket で送出) で確認済みです (2026-10-12、すべて PASS)。IOS-XR / Junos の設定例と確認コマンドは一般的な知識に基づくもので、**実機・CML2 での確認はまだしていません。** 結果が異なった場合は issue で知らせてください。

## シナリオ

| テンプレート | 送るもの | 相手側で期待する状態 |
|---|---|---|
| `ospfv2-hello-2way` | 相手のルータ ID を載せた OSPFv2 Hello (優先度 0)、10 秒間隔 | ネイバー `2-Way/DROther` (Full にはならない) |
| `ripv2-response-route` | 198.51.100.0/24 (メトリック 1) と 203.0.113.0/25 (メトリック 5) の RIPv2 Response、30 秒間隔 | RIB に R 経路 (メトリック +1) |
| `isis-l2-lan-hello-up` | 相手の MAC を IS Neighbors TLV に載せた L2 LAN IIH (優先度 0、MTU までパディング)、3 秒間隔 | L2 隣接 `Up` |
| `vrrpv3-preempt` | 優先度 200 の VRRPv3 広告 (VRID 5)、1 秒間隔 | 相手が `Backup`。止めると約 3.6 秒で `Master` に戻る |
| `pim-hello-neighbor` | PIMv2 Hello (ホールドタイム 105 秒、DR 優先度 0)、30 秒間隔 | PIM ネイバー一覧に wpg |
| `ldp-link-hello` | LDP リンク Hello (224.0.0.2、ホールドタイム 15 秒)、5 秒間隔 | LDP ディスカバリに `Link` (TCP セッションは張らない) |
| `bfd-session-up` | 状態 Down → (相手の Discriminator を入れて) 状態 Up の BFD 制御パケット、300 ms 間隔 | BFD セッション `Up`。止めると検出時間 (約 0.9 秒) 後に Down |

アドレスは既定で wpg が `10.99.0.1`、相手が `10.99.0.2/24` です。違う場合は `--var` で変えます (各テンプレートの `variables` 参照)。どのテンプレートも `--var src_mac=<送出インタフェースの MAC>` を渡すと分かりやすくなります。

BFD は 2 段階です。まず `--var state=down` で数パケット送ると相手が `Init` になるので、相手の Local Discriminator を確認して `--var peer_discriminator=<値>` で状態 Up を送り続けます。

## FRR で一括確認する (Linux 1 台)

```sh
sudo apt install frr iproute2       # frr サービスは起動しなくてよい
cargo build -p wpg-cli
sudo scripts/interop/frr-netns.sh   # 全シナリオを実行して PASS / FAIL を表示
```

スクリプトは名前空間 `wpgfrr` を作り、veth (`wpg0` ⇔ `frr0`) の向こうで FRR のデーモン (zebra、ospfd、ripd、isisd、vrrpd、pimd、ldpd、bfdd) を直接起動し、各テンプレートを `wpg send --backend afpacket` で送って `vtysh` の出力を確かめます。終了時に後片付けします。`--keep` で環境を残せば `sudo ip netns exec wpgfrr vtysh -N wpg` で中を見られます (`--down` で削除)。

出力例:

```text
PASS  OSPFv2 neighbor 2-Way
PASS  RIPv2 routes installed
PASS  IS-IS L2 adjacency Up
PASS  VRRPv3 peer is Backup
PASS  PIM neighbor
PASS  LDP link discovery
PASS  BFD session Up
```

FRR 側の設定は次のとおりです (スクリプトが投入)。VRRP は FRR の要件として、仮想 MAC `00:00:5e:00:01:05` を持つ macvlan `vrrp4-2-5` を作っています。

```text
router ospf
 ospf router-id 10.99.0.2
 network 10.99.0.0/24 area 0
router rip
 version 2
 network 10.99.0.0/24
router isis 1
 net 49.0001.0000.0000.0002.00
 is-type level-2-only
interface frr0
 ip router isis 1
 ip pim
 vrrp 5 version 3
 vrrp 5 priority 100
 vrrp 5 ip 10.99.0.254
mpls ldp
 router-id 10.99.0.2
 address-family ipv4
  discovery transport-address 10.99.0.2
  interface frr0
bfd
 peer 10.99.0.1 interface frr0
  receive-interval 300
  transmit-interval 300
```

## CML2 / 実機で確認する

1. wpg を動かす Ubuntu ノードの送出インタフェースと、相手ルータのインタフェースを同じセグメントにつなぎます ([CML2 での注意点](cml2.md) の準備を先に)。
2. 相手に下の設定を入れます (インタフェース名は環境に合わせる)。
3. シナリオごとに `wpg send templates/interop/<名前>.wpg.yaml -i <インタフェース> --backend afpacket --var src_mac=<MAC> ...` を実行し、確認コマンドで状態を見ます。IS-IS は `--var peer_mac=<相手の MAC>`、BFD は上記の 2 段階が必要です。

### 確認コマンド

| シナリオ | FRR | IOS-XR (未確認) | Junos (未確認) |
|---|---|---|---|
| OSPFv2 | `show ip ospf neighbor` | `show ospf neighbor` | `show ospf neighbor` |
| RIPv2 | `show ip route rip` | `show route rip` | `show route protocol rip` |
| IS-IS | `show isis neighbor` | `show isis adjacency` | `show isis adjacency` |
| VRRPv3 | `show vrrp 5` | `show vrrp` | `show vrrp` |
| PIM | `show ip pim neighbor` | `show pim neighbor` | `show pim neighbors` |
| LDP | `show mpls ldp discovery` | `show mpls ldp discovery` | `show ldp neighbor` |
| BFD | `show bfd peers brief` | `show bfd session` | `show bfd session` |

### IOS-XR の設定例 (未確認)

```text
interface GigabitEthernet0/0/0/0
 ipv4 address 10.99.0.2 255.255.255.0
router ospf 1
 router-id 10.99.0.2
 area 0
  interface GigabitEthernet0/0/0/0
router rip
 interface GigabitEthernet0/0/0/0
router isis 1
 is-type level-2-only
 net 49.0001.0000.0000.0002.00
 address-family ipv4 unicast
 interface GigabitEthernet0/0/0/0
  address-family ipv4 unicast
router vrrp
 interface GigabitEthernet0/0/0/0
  address-family ipv4
   vrrp 5 version 3
    priority 100
    address 10.99.0.254
multicast-routing
 address-family ipv4
  interface GigabitEthernet0/0/0/0
   enable
router pim
 address-family ipv4
  interface GigabitEthernet0/0/0/0
mpls ldp
 router-id 10.99.0.2
 interface GigabitEthernet0/0/0/0
router static
 address-family ipv4 unicast
  192.0.2.0/24 10.99.0.1 bfd fast-detect minimum-interval 300 multiplier 3
```

### Junos の設定例 (未確認)

```text
set interfaces ge-0/0/0 unit 0 family inet address 10.99.0.2/24 vrrp-group 5 virtual-address 10.99.0.254 priority 100
set interfaces ge-0/0/0 unit 0 family iso
set interfaces ge-0/0/0 unit 0 family mpls
set interfaces lo0 unit 0 family iso address 49.0001.0000.0000.0002.00
set protocols vrrp version-3
set protocols ospf area 0 interface ge-0/0/0.0
set protocols rip group wpg neighbor ge-0/0/0.0
set protocols isis interface ge-0/0/0.0 level 1 disable
set protocols pim interface ge-0/0/0.0
set protocols mpls interface ge-0/0/0.0
set protocols ldp interface ge-0/0/0.0
set routing-options static route 192.0.2.0/24 next-hop 10.99.0.1 bfd-liveness-detection minimum-interval 300 multiplier 3
```

## 制約 / 対象外

- **ステートフルな手順は扱いません** (非ゴール): OSPF は Full にならず (優先度 0 の DROther で止める)、LDP / BGP の TCP セッション、IS-IS の LSP フラッディング、BFD 状態機械の自動追従はしません。BGP は TCP が必要なため対象外です。
- RIP の経路は最後の Response から 180 秒でタイムアウトします。継続するには送出を続けます。
- IPv6 のシナリオ (OSPFv3、RIPng、MLD、VRRPv3 for IPv6) は、確認に使った環境で IPv6 が無効だったため含めていません。
- VRRP の送信元 MAC は仮想 MAC `00:00:5e:00:01:<VRID>` です。VRID を変えるときは `src` も合わせて変えてください。
