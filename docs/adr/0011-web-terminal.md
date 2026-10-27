# ADR-0011: Web ターミナル (ブラウザからの SSH / Telnet)

> Web terminal: the server bridges xterm.js in the browser to SSH (russh, ring backend) and Telnet over `/api/terminal/ws`; off by default, allowlisted targets, trust-on-first-use host keys, no stored passwords.

- Status: Accepted
- Date: 2026-10-27
- Deciders: wpg maintainers

## Context

wpg の利用者 (ネットワーク技術者) は、パケットを送りながら対向機器のカウンタやログを Tera Term などの端末で確認する。検証ラボ (CML2、EVE-NG、閉域の実機ラボ) では、wpg サーバだけが試験ネットワークに届き、作業者の PC からは機器に直接 SSH / Telnet できないことが多い。ブラウザから wpg 経由で機器のコンソールを開ければ、踏み台サーバや別の端末ソフトが不要になる。

一方で、サーバが利用者に代わって任意のホストへ接続する機能は、到達範囲の拡大 (SSRF 的な悪用) と認証情報の取り扱いという新しいリスクを持ち込む。wpg は単一バイナリ配布で、ラボはオフラインのことが多い ([ADR-0001](0001-license.md)、[09-distribution](../spec/09-distribution.md))。

## Decision Drivers

- 単一バイナリのまま動き、オフラインでも使えること (CDN を使わない)
- C ツールチェーンを増やさないこと (rustls と同じく ring を使う)
- ネットワーク機器との相互接続性 (古い IOS の鍵交換、Telnet コンソール、keyboard-interactive)
- 既定で安全であること (明示的に有効化するまで何もしない、接続先を絞れる、監査できる)
- Tera Term に近い操作感 (選択でコピー、右クリック貼り付け、ログ、複数行貼り付けの確認)

## Decision

- **構成**: ブラウザの xterm.js とサーバを WebSocket (`/api/terminal/ws`) でつなぎ、サーバが SSH / Telnet の TCP 接続を張る。制御は JSON テキストフレーム、端末の入出力はバイナリフレームで送る。既存のセッション Cookie、同一オリジン検査、viewer ロールの拒否を適用する。
- **SSH**: 純 Rust の `russh` を ring バックエンド (`default-features = false, features = ["ring", "rsa"]`) で使う。認証は none → 公開鍵 → パスワード → keyboard-interactive → パスワード入力の順で試す。PTY は `xterm-256color` でリサイズに追従する。鍵交換・暗号・MAC は安全な既定を優先し、その後ろに SHA-1 / CBC などの旧方式を最下位で加える (古い機器との接続のため。相手がそれしか持たない場合だけ選ばれる)。
- **Telnet**: 自前の受動的なオプション交渉 (ECHO、SGA、NAWS、TTYPE、BINARY)。wpg からは交渉を始めないので、生 TCP のコンソールポートにも IAC が混ざらない。
- **ホスト鍵**: Trust on first use。未知の鍵は SHA-256 フィンガープリントを利用者に示して確認を求め、「保存」すると `<data_dir>/terminal/known_hosts` (OpenSSH 形式) に記録する。保存済みと異なる鍵は拒否し、管理者が API / UI から古い鍵を消すまで接続できない。
- **安全策**: `[terminal] enabled = false` が既定。`allowed_targets` (ホスト名 / host:port / CIDR) で接続先を制限し、名前解決した結果のうち許可されたアドレスにだけ接続する (検査したアドレスに接続するので DNS の再解決で迂回できない)。同時セッション数の上限、無操作タイムアウト、`wpg::audit` への開始・終了・拒否の記録 (利用者、接続先、バイト数、理由) を設ける。パスワード・秘密鍵・パスフレーズはサーバに保存せず、接続処理が終わるまでメモリ上にだけ置く。
- **踏み台 (ProxyJump)**: 接続要求に SSH の踏み台を最大 4 段まで指定できる。サーバは 1 段目にだけ TCP で接続し、各段にログインしたうえで `direct-tcpip` チャネルを開いて次の段 (最後は宛先) に進む (OpenSSH の `-J` と同じ)。各段で TOFU のホスト鍵確認と認証を行い、失敗時は段 (`info.hop`) を示す。許可リストは全段に適用し、踏み台の奥は踏み台側で名前解決されるため名前か IP リテラルで照合する。宛先は SSH でも Telnet でもよい。
- **セッションログ**: Tera Term のログ相当として、接続先からの出力だけを `<data_dir>/terminal-logs/` に記録する (キー入力は記録しない)。既定でエスケープシーケンスを除去し、REST で一覧・ダウンロード・削除できる。`record_sessions = false` で無効にできる。
- **xterm.js の同梱**: xterm.js 6 と fit / web-links / search アドオン (MIT) を `crates/wpg-web/vendor/xterm/` に置き、Trunk の `copy-dir` で `/xterm/` に配置して wpg バイナリに埋め込む。ターミナル画面を開いたときに初めて読み込む。更新は `scripts/vendor-xterm.sh` で行う。

## Alternatives considered

- **ブラウザから直接接続 (WebSocket 対応の SSH ゲートウェイなど)**: 機器側に WebSocket プロキシが必要で、ラボの機器はそれを持たない。却下。
- **`ssh2` / libssh2 (C)**: 実績はあるが、OpenSSL か C ツールチェーンへの依存が増え、`deny.toml` の方針 (rustls / ring) と合わない。却下。
- **`russh` の aws-lc-rs バックエンド (既定)**: CMake / NASM などのビルド要件が増え、3 OS の静的ビルドが重くなる。ring で必要な方式は揃うため不採用。
- **xterm.js を CDN から読み込む**: オフラインのラボで使えず、CSP の `script-src 'self'` も緩める必要がある。却下。
- **自前の端末エミュレータ (Rust / WASM)**: VT100/xterm 互換の実装と保守のコストが大きい。xterm.js は事実上の標準で、IME や選択、検索も揃う。却下。
- **ホスト鍵を SQLite に保存**: 一覧・削除には十分だが、OpenSSH 形式のファイルなら `ssh-keyscan` での事前登録や既存の `known_hosts` の流用ができる。ファイルを採用した。

## Consequences

- wpg サーバは踏み台になる。到達範囲は `allowed_targets` とネットワーク構成で管理者が制御する必要がある (運用ガイドに記載)。
- 依存が増える (`russh` とその暗号 crate 群。`russh` 0.65 の MSRV は Rust 1.89)。`cargo deny` / `cargo audit` で監視する。
- 同梱した xterm.js の更新は手動 (`scripts/vendor-xterm.sh`)。Dependabot の対象外なので、リリース時に確認する。
- セッションログには、接続先が表示した機密情報 (設定の表示結果など) が含まれ得る。ファイルは 0600 で作成するが、保持期間の管理は運用者に委ねる。
- 役割ごとの権限 (P1) が入ったら、viewer 以外にも「ターミナル利用可否」を細かく分けることを検討する。
