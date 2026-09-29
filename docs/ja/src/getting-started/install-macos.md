# macOS へのインストール

対応: macOS 12 以降。リリースバイナリはまだ公開していないため、ソースからビルドします。成果物は Web UI を組み込んだ単一の `wpg` 実行ファイルです。macOS は libpcap を標準で持つので、追加のライブラリは不要です。

## 1. 前提ソフトウェアを入れる

```sh
xcode-select --install                     # コンパイラとリンカ
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cargo install just --locked                # 任意のタスクランナー
brew install wireshark                     # 任意: tshark と ChmodBPF (下記)
```

## 2. ビルドする

```sh
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
just release                # または: ./scripts/build.sh --release
```

最後に `==> done: target/release/wpg` と表示されます。

## 3. 権限なしで試す

```sh
./target/release/wpg doctor
./target/release/wpg template lint templates/examples/*.wpg.yaml
./target/release/wpg render templates/examples/arp-sweep.wpg.yaml -o /tmp/arp.pcapng
```

## 4. 送信を許可する

送信は `/dev/bpf*` を通ります。`sudo` で wpg を実行するか、デバイスをアクセス可能にします。Wireshark を ChmodBPF ヘルパー付きでインストールすると、`access_bpf` グループのメンバーに対してそれが行われます。お使いのマシンに合った案内は `wpg doctor` が表示します。[権限設定](permissions.md) を参照してください。

## 5. サーバを起動する

```sh
./target/release/wpg serve
```

既定では `127.0.0.1:8080` で待ち受け、`./templates` のテンプレートを読み込みます。ブラウザで <http://127.0.0.1:8080> を開き、[初回ログイン](first-login.md) に進んでください。

## 最初のフレームを送る

```sh
./target/release/wpg ifaces
sudo ./target/release/wpg send templates/examples/arp-sweep.wpg.yaml -i en0 --rate 1000pps
```

送信してよいのは、自分が所有する、または試験を許可されたネットワークだけです。続けて [ARP スイープのチュートリアル](../tutorials/arp-sweep.md) に進んでください。
