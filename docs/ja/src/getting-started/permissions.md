# 権限設定

raw フレームの送出は特権操作であり、必要な権限は OS ごとに異なります。wpg は起動時と `wpg doctor` の実行時に自己診断を行い、OS 別の対処方法を表示します。管理者設定で、許可インタフェースや最大レートを制限することもできます。

## Linux

`afpacket` バックエンドには `CAP_NET_RAW` が必要です (`CAP_NET_ADMIN` も併せて付与します)。root で実行するか、バイナリに capability を付与します。

```sh
sudo setcap cap_net_raw,cap_net_admin+eip /path/to/wpg
```

これは `wpg doctor` が、実行中のバイナリの実パスで表示するヒントです。バックエンドが EPERM/EACCES で拒否された場合、エラーは次のように案内します。

```text
AF_PACKET needs CAP_NET_RAW: run as root or grant it with `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)`
```

file capability はバイナリを置き換えると失われ、`nosuid` マウント上では無視されます。systemd では `AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN` を使います。[CML2 での注意点](../operations/cml2.md)も参照してください。

## macOS

`/dev/bpf*` への読み書きアクセスが必要です。`sudo` で実行するか、デバイスをアクセス可能にします (例: Wireshark の ChmodBPF)。`wpg doctor` のヒントは `sudo /path/to/wpg …   (or make /dev/bpf* accessible, e.g. via Wireshark's ChmodBPF)` です。

## Windows

管理者ターミナルと、WinPcap API 互換モードで入れた Npcap が必要です。`wpg doctor` のヒントは `Run from an elevated (Administrator) terminal` で、Npcap がない場合は `https://npcap.com/#download` を案内します。Npcap を管理者専用モードで入れていなければ、管理者権限のないプロセスでも Npcap を使えるため、権限チェックはここでは警告止まりです。

## `wpg doctor` による確認

`wpg doctor` は 1 チェック 1 行で、角括弧内に安定した id を表示します。`wpg doctor --json` はスクリプト向けに同じ内容を返します。チェックが fail のとき終了ステータスは 0 以外です。

| チェック id | 未達のときの状態 | 意味 |
|---|---|---|
| `caps.raw_socket` | fail (Windows は warn) | raw ソケットの権限: CAP_NET_RAW / root / BPF アクセス / 昇格 |
| `iface.any_up` | warn | loopback 以外のインタフェースが carrier 付きで up |
| `kernel.af_xdp` | warn | AF_XDP バックエンド用にカーネル 5.10 以降 |
| `kernel.tpacket_v3` | warn | カーネル 3.2 以降。なければ afpacket は `sendmmsg` にフォールバック |
| `hugepages` | warn | Hugepages の設定。DPDK バックエンドでのみ必要 |
| `npcap` | fail / warn (Windows のみ) | Npcap が導入され WinPcap 互換モードであること。他の OS では skip |
| `pcap_lib` | warn | libpcap が見つかること (`pcap` バックエンドで必要)。Windows では skip |

各チェックの結果は `ok`、`warn`、`fail`、`skip` (この OS では対象外) のいずれかです。
