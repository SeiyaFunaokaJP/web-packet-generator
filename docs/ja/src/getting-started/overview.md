# インストール方法を選ぶ

wpg は Web UI を組み込んだ単一の `wpg` 実行ファイルです。リリースバイナリはまだ公開していないため、下のどの方法でもソースからビルドします。違いは、どこでビルドし、どう動かすかです。

## 必要なもの

- wpg を動かすマシン: Linux (Ubuntu 20.04 以降と同等のディストリビューション)、macOS 12 以降、Windows 10 21H2 以降。
- 試験したい検証ネットワークにつながったネットワークインタフェース。自分が管理していない、または試験を許可されていないネットワークへの送信はできません。
- ブラウザ。同じマシン上のもの、またはパスワードを設定してローカル以外のアドレスで待ち受ければ別のマシンのもの。

## 方法を選ぶ

| やりたいこと | 方法 | 必要なもの |
|---|---|---|
| Linux の検証ホスト (CML2、EVE-NG、VM) で、高レートで、サービスとして動かす | [scripts/deploy.sh によるデプロイ](../operations/deploy.md): SSH でホスト上に wpg をビルドし、systemd ユニットを入れるウィザード。Windows (`scripts\deploy.bat`)、Linux、macOS のどれからでも始められます。 | ホストへの SSH と sudo |
| Rust ツールチェーンを入れずに Linux ホストで動かす | [Docker で動かす](docker.md) | Linux ホストの Docker |
| 自分の PC で使う、または開発する | ソースからビルド: [Linux](install-linux.md)、[macOS](install-macos.md)、[Windows](install-windows.md) | rustup、trunk。Windows では Npcap とその SDK |

選ぶときの注意:

- **レート。** Windows は Npcap だけで送信し、GUI での低〜中レートの利用を想定しています。macOS は BPF で送信します。高レート (AF_PACKET、AF_XDP、DPDK) には Linux を使ってください。
- **Docker Desktop** (macOS / Windows) はコンテナを VM の中で動かすので、フレームは物理 NIC に届きません。Docker は Linux ホストでだけ使ってください。
- **まだ権限がない場合。** どの方法でも、テンプレートの pcapng への書き出しと Web UI は raw ソケットの権限なしで使えます。権限が要るのは送信だけです ([権限設定](permissions.md))。

## インストールの後

どの方法でも、次の手順は同じです。

1. `wpg doctor` を実行し、送信について報告された問題を直します ([権限設定](permissions.md))。
2. `wpg serve` でサーバを起動し (デプロイスクリプトと Docker では起動済みです)、<http://127.0.0.1:8080> を開きます。
3. パスワードを設定して画面を見て回ります: [初回ログイン](first-login.md)。
4. まずドライラン (`pcapfile` バックエンドは送信せずに pcapng ファイルへ書き出します) を試し、[ARP スイープのチュートリアル](../tutorials/arp-sweep.md) で実際に送信します。

うまくいかないときは [FAQ / トラブルシュート](../faq/troubleshooting.md) を参照してください。
