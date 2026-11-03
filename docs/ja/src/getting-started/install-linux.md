# Linux へのインストール

対応: Ubuntu 20.04 以降 (glibc 2.31) と同等のディストリビューション。リリースバイナリはまだ公開していないため、ソースからビルドします。成果物は Web UI を組み込んだ単一の `wpg` 実行ファイルです。

## 1. 前提ソフトウェアを入れる

```sh
# Rust ツールチェーンのインストーラ (rustup.rs) と、リンク用の C ツールチェーン
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install build-essential pkg-config git

git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
./scripts/setup.sh          # --just を付けるとタスクランナー just も入れる
```

`scripts/setup.sh` は `rust-toolchain.toml` で固定した Rust ツールチェーンと Web UI 用の WASM ターゲット、`scripts/versions.env` で固定したバージョンのビルド済み `trunk` (Web UI のバンドラ) を入れ、最後に次に実行するコマンドを表示します。

Linux は AF_PACKET で送信するので、追加のライブラリは不要です。任意の libpcap バックエンド (`--features pcap`) を使う場合だけ `sudo apt install libpcap-dev` が必要です。`tshark` (パッケージ名 `tshark`) は任意です。チュートリアルで生成したキャプチャの確認に使います。

## 2. ビルドする

```sh
./scripts/build.sh --release        # または: just release
```

スクリプトは `trunk` で Web UI を、続いて `cargo` で `wpg` バイナリをビルドして、`==> done: target/release/wpg` と表示します。オプションのバックエンドには `--features pcap`、`--features afxdp`、`--dpdk` を付けます。`trunk` が無い場合、バイナリはビルドされますが Web UI はスキップされます。

## 3. 権限なしで試す

ファイルへの書き出しに特別な権限は不要なので、まず動作を確認できます。

```sh
./target/release/wpg doctor
./target/release/wpg template lint templates/examples/*.wpg.yaml
./target/release/wpg render templates/examples/arp-sweep.wpg.yaml -o /tmp/arp.pcapng
```

`wpg doctor` は送信に不足しているものを報告します ([権限設定](permissions.md))。次の手順を行うまでは `caps.raw_socket` が失敗して構いません。

## 4. 生ソケット送信を許可する

root で実行するか、バイナリに一度だけ capability を付与します (ファイルを置き換えると失われます)。

```sh
sudo setcap cap_net_raw,cap_net_admin+ep ./target/release/wpg
./target/release/wpg doctor
```

## 5. サーバを起動する

```sh
./target/release/wpg serve            # 開発用のホットリロードは: just run
```

既定では `127.0.0.1:8080` で待ち受け、`./templates` のテンプレートを読み込みます。ブラウザで <http://127.0.0.1:8080> を開き、パスワードと最初の画面は次ページの [初回ログイン](first-login.md) を参照してください。別のマシンからアクセスするには、パスワードを設定したうえで `--bind 0.0.0.0:8080` (または `wpg config init` で作る `wpg.toml` の `bind`) を使います。[設定リファレンス](../reference/config.md) を参照してください。ラボのホストに常設するときは `scripts/deploy.sh` がバイナリのコピー、`setcap`、systemd ユニットの投入を行います。[CML2 での注意点](../operations/cml2.md) も参照してください。

## 最初のフレームを送る

```sh
./target/release/wpg ifaces
./target/release/wpg send templates/examples/arp-sweep.wpg.yaml -i eth1 --rate 1000pps
```

送信してよいのは、自分が所有する、または試験を許可されたネットワークだけです。続けて [ARP スイープのチュートリアル](../tutorials/arp-sweep.md) に進んでください。
