# Linux へのインストール

対応: Ubuntu 20.04 以降 (glibc 2.31) と同等のディストリビューション。リリースバイナリはまだ公開していないため、ソースからビルドします。成果物は Web UI を組み込んだ単一の `wpg` 実行ファイルです。

## 1. 前提ソフトウェアを入れる

```sh
# Rust ツールチェーン (rustup.rs) と Web UI 用の WASM ターゲット
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# フロントエンドのバンドラと、任意でタスクランナー
cargo install trunk --locked
cargo install just --locked

# 任意の libpcap バックエンド (--features pcap) を使う場合のみ
sudo apt install build-essential pkg-config libpcap-dev
```

`tshark` (パッケージ名 `tshark`) は任意です。チュートリアルで生成したキャプチャの確認に使います。

## 2. ビルドする

```sh
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
just release                # または: ./scripts/build.sh --release
```

スクリプトは前提条件を確認し、`trunk` でフロントエンドを、続いて `cargo` でワークスペースをビルドして、`==> done: target/release/wpg` と表示します。オプションのバックエンドには `scripts/build.sh` に `--features pcap` (または `--dpdk`) を付けます。`trunk` が無い場合、バックエンドはビルドされますが Web UI はスキップされます。

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
