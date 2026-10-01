# Windows へのインストール

対応: Windows 10 21H2 以降。リリースバイナリはまだ公開していないため、ソースからビルドします。送信は Npcap のみを使い、GUI での低〜中レート利用を想定しています。高いレートが必要なときは Linux を使ってください。以下のコマンドは PowerShell 用です。

## 1. 前提ソフトウェアを入れる

1. [rustup](https://rustup.rs) をインストールします (既定の MSVC ツールチェーンを選びます。Visual Studio Build Tools の「C++ によるデスクトップ開発」が無い場合は案内されます)。
2. WASM ターゲットとフロントエンドのバンドラを追加します。

   ```powershell
   rustup target add wasm32-unknown-unknown
   cargo install trunk --locked
   cargo install just --locked      # 任意
   ```

3. [Npcap](https://npcap.com/#download) をインストールし、**Install Npcap in WinPcap API-compatible Mode** にチェックを入れます。送信に必要です。
4. 任意の libpcap バックエンド (`--features pcap`) をビルドする場合のみ、Npcap SDK を入れ、その `Lib\x64` を環境変数 `LIB` に加えます。
5. 任意: Wireshark (`tshark` 用)。

## 2. ビルドする

```powershell
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
.\scripts\build.ps1 -Release       # または: just release
```

最後に `==> done: target\release\wpg.exe` と表示されます。

## 3. 権限なしで試す

```powershell
.\target\release\wpg.exe doctor
.\target\release\wpg.exe render templates\examples\arp-sweep.wpg.yaml -o $env:TEMP\arp.pcapng
```

## 4. 送信を許可する

送信するときは **管理者として PowerShell を開き**、`wpg doctor` が Npcap を WinPcap 互換モードのインストール済みと報告することを確認します。[権限設定](permissions.md) を参照してください。

## 5. サーバを起動する

```powershell
.\scripts\run.ps1                  # または: .\target\release\wpg.exe serve
```

既定では `127.0.0.1:8080` で待ち受け、`.\templates` のテンプレートを読み込みます。ブラウザで <http://127.0.0.1:8080> を開き、[初回ログイン](first-login.md) に進んでください。Windows ファイアウォールが `wpg.exe` の許可を求めたときは、ローカル以外のアドレスに bind する場合に限り、プライベートネットワークだけを許可してください。

## 最初のフレームを送る

```powershell
.\target\release\wpg.exe ifaces
.\target\release\wpg.exe send templates\examples\arp-sweep.wpg.yaml -i <インタフェース> --rate 1000pps
```

インタフェース名は `wpg ifaces` の表示を使います。送信してよいのは、自分が所有する、または試験を許可されたネットワークだけです。続けて [ARP スイープのチュートリアル](../tutorials/arp-sweep.md) に進んでください。
