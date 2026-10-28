# Windows へのインストール

対応: Windows 10 21H2 以降。リリースバイナリはまだ公開していないため、ソースからビルドします。送信は Npcap のみを使い、GUI での低〜中レート利用を想定しています。高いレートが必要なときは Linux を使ってください。以下のコマンドは PowerShell 用です。

> **Windows から Linux の検証用ホストに wpg を入れたいだけなら**、Windows 上でビルドする必要はありません。リポジトリを手元に用意し (`git clone`、または GitHub の「Download ZIP」を展開)、**`scripts\deploy.bat` をダブルクリック**します。ウィザードが接続先、SSH のユーザとパスワード、sudo パスワード、Web UI の管理者パスワード、送信に使うインタフェースを日本語で聞き、ホスト上で wpg をビルドしてサービスとして入れます。必要なのは Windows 10 / 11 標準の OpenSSH クライアントだけです。[デプロイ](../operations/deploy.md) を参照してください。

## 1. 前提ソフトウェアを入れる

1. [rustup](https://rustup.rs) をインストールします (既定の MSVC ツールチェーンを選びます。Visual Studio Build Tools の「C++ によるデスクトップ開発」が無い場合は案内されます)。
2. WASM ターゲットとフロントエンドのバンドラを追加します。

   ```powershell
   rustup target add wasm32-unknown-unknown
   cargo install trunk --locked
   cargo install just --locked      # 任意
   ```

3. [Npcap](https://npcap.com/#download) をインストールします (既定のオプションのままでかまいません。WinPcap API 互換モードは不要です)。送信に必要です。
4. [Npcap SDK](https://npcap.com/#download) をダウンロードして `C:\Npcap-SDK` に展開します。`scripts\build.ps1` は `C:\npcap-sdk-<バージョン>` (zip の名前のまま。複数あれば新しいもの)、環境変数 `NPCAP_SDK` で指定したディレクトリ、SDK の `Lib\x64` を指す `LIBPCAP_LIBDIR` も探します。見つかると `scripts\build.ps1` が Npcap バックエンド (`pcap` feature) を組み込みます。Windows で送信できるのはこのバックエンドだけです。SDK が無くてもビルドは成功しますが、できた `wpg.exe` は送信できず、`build.ps1` が警告します。
5. 任意: Wireshark (`tshark` 用)。

## 2. ビルドする

```powershell
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
.\scripts\build.ps1 -Release       # または: just release
```

SDK が見つかると `==> Npcap SDK: LIBPCAP_LIBDIR=...` と表示し、最後に `==> done: target\release\wpg.exe` と表示されます。主な使い方は次のとおりです。

| コマンド | 結果 |
|---|---|
| `.\scripts\build.ps1` | デバッグビルド (`target\debug\wpg.exe`)。SDK が見つかれば Npcap バックエンドを組み込む |
| `.\scripts\build.ps1 -Release` | 最適化ビルド (`target\release\wpg.exe`)。SDK の検出は同じ |
| `.\scripts\build.ps1 -NoPcap` | SDK があっても Npcap バックエンドなしでビルドする (送信不可) |
| `.\scripts\build.ps1 -Features pcap` | Npcap バックエンドを必須にする。SDK が見つからなければエラーで止まる |
| `Get-Help .\scripts\build.ps1 -Detailed` | すべてのパラメータ |

trunk が無いとフロントエンドのビルドは警告付きで省略されます (Web UI は古い、または存在しない `crates\wpg-web\dist` を配信します)。

## 3. 権限なしで試す

```powershell
.\target\release\wpg.exe doctor
.\target\release\wpg.exe render templates\examples\arp-sweep.wpg.yaml -o $env:TEMP\arp.pcapng
```

## 4. 送信を許可する

`wpg doctor` が Npcap をインストール済み (`npcap`)、Npcap バックエンドを組み込み済み (`backend.pcap`) と報告することを確認します。`backend.pcap` が fail なら `build.ps1` が Npcap SDK を見つけられていないので、手順 1 のとおり SDK を入れてビルドし直してください。Npcap を既定のオプションで入れていれば、通常の PowerShell から送信できます (`caps.raw_socket` は ok)。「Restrict Npcap driver's access to Administrators only」を選んで入れた場合は `caps.raw_socket` が fail になるので、送信するときは **管理者として PowerShell を開いて** ください。[権限設定](permissions.md) を参照してください。

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

インタフェース名は `wpg ifaces` の表示 (`{GUID}`) を使います (`tshark -D` が表示する `\Device\NPF_{GUID}` でもかまいません)。Npcap のループバックアダプタ (`\Device\NPF_Loopback`) は Ethernet フレームを受け付けません (`PacketSendPacket failed ... (50)`)。実ネットワークに出さずに送信を試すときは、Hyper-V の `vEthernet (Default Switch)` や VMware のホストオンリーアダプタなどの仮想アダプタを使い、`wpg capture -i <インタフェース> --backend pcap --include-outgoing` か `tshark` で確認してください。送信してよいのは、自分が所有する、または試験を許可されたネットワークだけです。続けて [ARP スイープのチュートリアル](../tutorials/arp-sweep.md) に進んでください。

## リリースアーカイブと Npcap の確認

リリースアーカイブ (`wpg-<version>-x86_64-pc-windows-msvc.zip`) には、Web UI と Npcap バックエンドを組み込んだ `wpg.exe`、サンプルテンプレート、`packaging\windows\wpg-service.ps1` が入っています。コード署名はしていません。

```powershell
# 確認してから、展開前に「インターネットからダウンロードした」印を外す
(Get-FileHash .\wpg-0.2.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash   # SHA256SUMS と比べる
Unblock-File .\wpg-0.2.0-x86_64-pc-windows-msvc.zip
Expand-Archive .\wpg-0.2.0-x86_64-pc-windows-msvc.zip -DestinationPath "C:\Program Files\wpg"
```

それでも SmartScreen が「Windows によって PC が保護されました」と表示した場合は、一度だけ **詳細情報 → 実行** を選びます。

Npcap は同梱していません (無償版のライセンスでは再配布できません)。`wpg.exe` は `wpcap.dll` を `/DELAYLOAD` でリンクしており、`pcap` バックエンドを初めて使うときに読み込みます。その際 `System32\Npcap` を自分で DLL 検索パスに加えるので、Npcap が無くても起動でき、Npcap のどちらのインストールモードでも動きます。`wpg doctor` と `scripts\build.ps1` の両方が確認します。

- `wpg doctor` は、Npcap が無いと `npcap` のチェックを失敗にし、Npcap がインストールされているように見えるのに `wpcap.dll` が `System32\Npcap` にも `System32` にも無いときは警告します。対処の行に <https://npcap.com/#download> へのリンクが出ます。
- Npcap 無しで送信すると、クラッシュせずに `Npcap is not installed; install it from https://npcap.com/#download and run wpg doctor` で止まります。
- `wpg doctor` は、`wpg.exe` が Npcap バックエンドなしでビルドされていると `backend.pcap` を失敗にし、SDK を展開して `scripts\build.ps1` を再実行するよう案内します。
- Npcap を管理者専用モードで入れた場合、昇格していないターミナルでは `caps.raw_socket` が fail になります。管理者ターミナルで実行するか、そのオプションを外して Npcap を入れ直してください。
- `scripts\build.ps1` もビルドの前に警告を出します。Npcap が無くてもビルドは成功しますが送信はできないため、また Npcap SDK なしでビルドするときも警告します。

起動時に wpg をバックグラウンドで動かす方法は [サービスとして動かす](../operations/services.md) を参照してください。
