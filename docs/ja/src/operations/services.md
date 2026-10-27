# サービスとして動かす

Status: draft

サンプルはリポジトリと各リリースアーカイブの `packaging/` にあります。どれも、決まった場所の設定ファイル (`WPG_CONFIG`) と決まったデータディレクトリ (`WPG_DATA_DIR`) を使って `wpg serve` を実行します。先に `wpg config init --path <file>` で設定ファイルを作り、`wpg config set-password --config <file>` でパスワードを設定してください。ループバック以外のアドレスで待ち受けるサーバは、パスワードが無いと起動しません。

## Linux: systemd

`packaging/systemd/wpg.service` は [`scripts/deploy.sh --service`](deploy.md) がインストールするものです。手動で入れる場合:

```sh
sudo useradd --system --home-dir /var/lib/wpg --shell /usr/sbin/nologin wpg
sudo install -m 0755 wpg /usr/local/bin/wpg
sudo install -d -m 0750 -o root -g wpg /etc/wpg
sudo wpg config init --path /etc/wpg/wpg.toml          # その後 bind や allowed_ifaces などを編集
sudo wpg config set-password --config /etc/wpg/wpg.toml
sudo chgrp wpg /etc/wpg/wpg.toml && sudo chmod 0640 /etc/wpg/wpg.toml
sudo install -m 0644 packaging/systemd/wpg.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now wpg
journalctl -u wpg -f
```

unit のポイント:

- 特権の無いユーザ `wpg` で動きます。raw ソケットは `AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN` で得るので、サービス用にはバイナリへの `setcap` は不要です (`NoNewPrivileges=yes` の下ではファイルケーパビリティはどのみち無視されます)。
- `StateDirectory=wpg` が `wpg` 所有の `/var/lib/wpg` を作ります。`ProtectSystem=strict` により、それ以外のファイルシステムはサーバから読み取り専用になります。
- `EnvironmentFile=-/etc/wpg/wpg.env` でホストごとの `WPG_*` を上書きできます (deploy.sh が書きます)。
- `LimitMEMLOCK=infinity` は AF_XDP バックエンド用です (5.11 より前のカーネルでは UMEM が `RLIMIT_MEMLOCK` に計上されます)。
- `RestrictAddressFamilies` は IP と Unix ソケットに加えて `AF_PACKET` と `AF_XDP` を許可します。

待ち受けアドレスは `wpg.toml` の `bind` か `wpg.env` の `WPG_SERVER__BIND=` で変更し、`sudo systemctl restart wpg` を実行します。

## macOS: launchd

`packaging/launchd/io.github.seiyafunaokajp.wpg.plist` はシステムデーモンです (root で動きます。ChmodBPF を設定していなければ `/dev/bpf*` に必要です。[権限設定](../getting-started/permissions.md) を参照)。

```sh
sudo install -m 0755 wpg /usr/local/bin/wpg
sudo mkdir -p /usr/local/etc/wpg /usr/local/var/wpg /usr/local/var/log
sudo /usr/local/bin/wpg config init --path /usr/local/etc/wpg/wpg.toml
sudo /usr/local/bin/wpg config set-password --config /usr/local/etc/wpg/wpg.toml
sudo install -m 0644 packaging/launchd/io.github.seiyafunaokajp.wpg.plist /Library/LaunchDaemons/
sudo launchctl bootstrap system /Library/LaunchDaemons/io.github.seiyafunaokajp.wpg.plist
tail -f /usr/local/var/log/wpg.log
```

停止と削除は `sudo launchctl bootout system/io.github.seiyafunaokajp.wpg` です。`KeepAlive` により、サーバがエラーで終了すると再起動されます。ダウンロードしたバイナリは先に quarantine 属性を外してください。外さないと launchd が起動できません ([macOS へのインストール](../getting-started/install-macos.md) の「リリースバイナリ: Gatekeeper と quarantine」を参照)。

## Windows

`wpg.exe` はコンソールプログラムで、サービスコントロールマネージャ (SCM) のプロトコルに対応していません。そのため、単純な `sc.exe create wpg binPath= "C:\...\wpg.exe serve"` で登録したサービスはエラー 1053 で起動に失敗します。代わりに次のどちらかを使ってください。

### タスクスケジューラ (標準機能)

`packaging/windows/wpg-service.ps1` は、起動時に `SYSTEM` として `wpg.exe serve` を実行し、停止したら 1 分ごとに再起動するタスクを登録します。管理者として開いた PowerShell で実行します。

```powershell
.\packaging\windows\wpg-service.ps1 -Action Install -Exe "C:\Program Files\wpg\wpg.exe"
.\packaging\windows\wpg-service.ps1 -Action Status
.\packaging\windows\wpg-service.ps1 -Action Stop
.\packaging\windows\wpg-service.ps1 -Action Uninstall
```

`C:\ProgramData\wpg\wpg.toml` が無ければ (`wpg config init` で) 作り、データは `C:\ProgramData\wpg\data` に置きます。スクリプトが作るディレクトリは `SYSTEM` と Administrators だけが開けます。`C:\ProgramData` の下に新しく作ったフォルダはすべてのユーザが読めますが、ここにはパスワードのハッシュ、データベース、TLS 鍵が入るためです (既存のディレクトリの権限は変えません)。ループバック以外のアドレスで待ち受ける前に、管理者として開いた PowerShell で `& "C:\Program Files\wpg\wpg.exe" config set-password --config C:\ProgramData\wpg\wpg.toml` でパスワードを設定してください。Npcap が無いときはスクリプトが警告します。

### ラッパーで本物のサービスにする

`sc.exe` / `services.msc` で管理したい場合は、[WinSW](https://github.com/winsw/winsw) か [NSSM](https://nssm.cc/) でラップします。NSSM の例:

```powershell
nssm install wpg "C:\Program Files\wpg\wpg.exe" serve
nssm set wpg AppEnvironmentExtra WPG_CONFIG=C:\ProgramData\wpg\wpg.toml WPG_DATA_DIR=C:\ProgramData\wpg\data
nssm set wpg AppDirectory C:\ProgramData\wpg\data
sc.exe start wpg
sc.exe query wpg
```

どちらの場合も Npcap が必要です (`wpg doctor` が確認します)。Windows Defender ファイアウォールで `wpg.exe` を許可するのは、ループバック以外で待ち受ける場合に限り、プライベートネットワークだけにしてください。

## 確認状況

systemd unit は、systemd が動く Ubuntu 24.04 コンテナで `scripts/deploy.sh` から起動しました (サーバは `CAP_NET_RAW` と `CAP_NET_ADMIN` だけを持つユーザ `wpg` で動き、`/api/health` に応答しました)。`systemd-analyze verify` も通ります。リリースのワークフローは、macOS 14 ランナーで Apple silicon 向けアーカイブから launchd デーモンを、Windows Server ランナーで Windows 向けアーカイブからタスクスケジューラのタスクを上記の手順どおりに登録し、`/api/health` を確認してから停止・削除します。Windows では設定ディレクトリを `BUILTIN\Users` が読めないことも確認します ([リリース手順](../dev/release.md) の「CI で確認していること」)。どちらのジョブも通っています。再起動 (起動時トリガーと `RunAtLoad` そのもの)、NSSM / WinSW によるラップ、Windows での Npcap 経由の送信は確認していません。
