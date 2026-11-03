# 認証と TLS

## 管理パスワード

管理アカウントは `admin` の 1 つです。パスワードは次のうち最初に設定されているものを使います (優先度の高い順)。

1. `wpg serve --password <PASSWORD>`。プロセス一覧で他のユーザから見えるため、試用時のみにしてください。
2. `wpg serve --password-file <FILE>`。ファイルには平文パスワードか argon2 ハッシュを書きます。
3. 環境変数 `WPG_PASSWORD` (平文か argon2 ハッシュ)
4. `wpg.toml` の `auth.password_hash`。`wpg config set-password` がパスワードを尋ねてこのキーに書き込みます。スクリプトでは `--stdin` を使います。
5. `wpg.toml` の `auth.password_file` (平文か argon2 ハッシュ)
6. 上のいずれも無い場合は、初回アクセス時のセットアップ画面

平文のパスワードは起動時に argon2id (19 MiB、2 パス。OWASP 推奨値) でハッシュ化し、`<data_dir>/wpg.sqlite` に保存します。設定したパスワードを変えると、次回の起動時に既存のセッションはすべて無効になります。

### パスワードが無い間

パスワードが設定されるまでは次のように動作します。

- `/api/health`、`/api/version`、`/api/auth/*`、`/api/docs` 以外の API はすべて `401 setup_required` を返します。
- `wpg serve` はループバック以外のアドレスで待ち受けず、終了コード 2 で終了します。`--allow-insecure` を付けると待ち受けますが、大きな警告をログに出し、サーバに到達できる誰もがパスワードを設定できる状態になります。
- `--allow-insecure` を付けない限り、セットアップ画面はサーバ自身 (ループバック) からのリクエストしか受け付けません。

```sh
wpg serve                                  # 127.0.0.1:8080 を開いてパスワードを設定
wpg config set-password                    # または先に wpg.toml へハッシュを保存
wpg serve --bind 0.0.0.0:8080 --tls        # 以後は任意のアドレスで待ち受け可能
```

## セッションと CSRF

- ログイン後は `wpg_session` Cookie でセッションを識別します。値はランダムな 256 ビットのトークンで、`HttpOnly; SameSite=Strict` (HTTPS では `Secure` も) を付けて発行します。データベースにはトークンの SHA-256 だけを保存します。
- セッションの有効期間は `auth.session_ttl_secs` (既定 24 時間) で、リクエストのたびに延長されます。
- POST、PUT、DELETE のリクエストは、セッションの CSRF トークンを `X-CSRF-Token` ヘッダで送る必要があります。トークンは `GET /api/auth/status` と、スクリプトから読める `wpg_csrf` Cookie で受け取れます。ログインとセットアップには CSRF トークンは不要ですが、`Content-Type: application/json` のリクエストしか受け付けません。
- WebSocket (`/api/ws`、Web ターミナルの `/api/terminal/ws`) の接続にもセッション Cookie が必要です。`Origin` が別ホストを指す接続は拒否します。Web ターミナルの安全策は [Web ターミナル](terminal.md#セキュリティ) を参照してください。

API: `GET /api/auth/status`、`POST /api/auth/setup {password, confirm}`、`POST /api/auth/login {password, username?}`、`POST /api/auth/logout`。詳しくは [API リファレンス](../reference/api.md) を参照してください。

## API トークン

スクリプト、CI ジョブ、[`wpg remote`](../reference/cli.md#wpg-remote) は、セッションの代わりに API トークンで認証します。

```sh
# サーバ上で実行 (稼働中でも可)。標準出力にはトークンだけを出力
wpg config token create ci --expires-days 90
# または Web UI の「設定」→「API トークン」

export WPG_SERVER=https://wpg.example.net:8080 WPG_TOKEN=wpg_...
wpg remote jobs start examples-arp-sweep --backend null --count 100 --wait
curl -H "Authorization: Bearer $WPG_TOKEN" "$WPG_SERVER/api/jobs"
```

- トークンは `wpg_` に 16 進 64 桁 (ランダムな 256 ビット) を続けたものです。表示するのは作成時の 1 回だけです。データベースには SHA-256 と、見分けるための先頭 12 文字 (`wpg_` と 16 進 8 桁) だけを保存します。トークンは十分に長いランダム値なので高速なハッシュで足ります。argon2 にすると API リクエストのたびに約 19 MiB と数十ミリ秒かかります。
- `Authorization: Bearer <トークン>` を付けたリクエストは、トークンを作成したユーザとして、そのユーザの役割で動作します。細かいスコープはありません。CSRF トークンは不要で、Cookie も発行しません。
- `Authorization` ヘッダがあるリクエストでは、セッション Cookie を見ません。誤った、期限切れの、または失効したトークンには `401 unauthorized` を返します。失敗はクライアントのアドレスごとの [ログインのレート制限](#ログインのレート制限) に数えます。
- 有効期限は 1〜3650 日、または無期限です。パスワードを変えてもトークンは有効なままです。失効 (Web UI、`wpg config token revoke <id>`、`DELETE /api/tokens/{id}`) はトークンを削除し、次のリクエストから拒否します。
- トークンではトークンを管理できません。Bearer のリクエストには `/api/tokens` が `403 forbidden` を返します。作成と失効はセッション (Web UI) か、サーバ上の `wpg config token` で行います。admin はすべてのユーザのトークンを、その他のユーザは自分のトークンだけを表示・失効できます。
- トークンの作成、失効、拒否は監査ログに記録します。最終使用日時は最大 1 分に 1 回記録します。

API: `GET /api/tokens`、`POST /api/tokens {name, expires_in_days?}` (`{token, secret}` を返す)、`DELETE /api/tokens/{id}`。

`wpg remote` はサーバ証明書を検証します。自己署名証明書のサーバには `--insecure` を付けてください。

## ログインのレート制限

ログインの失敗はクライアントのアドレスごとに 1 分あたり 5 回まで許可します。それを超えるとそのクライアントは 1 分間ロックアウトされます。ロックアウトが繰り返されるたびに時間は 2 倍になり、最大 15 分です。ロックアウト中のログインには `429 rate_limited` を返し、`details.retry_after_secs` と `Retry-After` ヘッダで待ち時間を示します。

すべてのログイン試行を `login_attempts` テーブルに記録し、30 日間保持します。リバースプロキシの後ろでは、全クライアントがプロキシのアドレスを共有します。

## 監査ログ

ログイン、ログイン失敗、ログアウト、セットアップ、API トークンの作成・失効・拒否、ジョブ操作 (作成、開始、一時停止、再開、停止、削除) は、ユーザ名とクライアントのアドレスを付けて `wpg::audit` ターゲットでログに記録します。これらのイベントだけを出力するには `RUST_LOG=wpg::audit=info` を指定します。

## TLS

- `wpg serve --tls` (または `server.self_signed_tls = true`) を指定すると、自己署名の ECDSA 証明書を `<data_dir>/tls/` に生成し、次回以降の起動でも再利用します。証明書の有効期間は 800 日で、期限の約 1 か月前に作り直します。対象は `localhost`、`127.0.0.1`、`::1`、bind アドレス、ホスト名です。起動時に SHA-256 フィンガープリントを表示するので、ブラウザに表示される値と照合してください。
- 自前の証明書を使うには `wpg.toml` に `[server.tls] cert = "...", key = "..."` (PEM ファイル) を指定します。自己署名証明書より優先されます。
- TLS のポートでは平文の HTTP に応答せず、HTTPS へのリダイレクトもしません。

リバースプロキシ (nginx、Caddy) を使う場合は、wpg を `127.0.0.1` で待ち受けさせ、TLS はプロキシで終端します。`/api/ws` の WebSocket アップグレードも転送してください。

## セキュリティヘッダ

すべての応答に Content Security Policy を付けます (`default-src 'self'`。スクリプトはサーバからのものと `'wasm-unsafe-eval'`、起動用インラインスクリプトのハッシュのみ許可。インラインスタイル許可。`connect-src 'self' ws: wss:`。`frame-ancestors 'none'`)。あわせて `X-Content-Type-Options: nosniff`、`X-Frame-Options: DENY`、`Referrer-Policy: no-referrer`、制限的な `Permissions-Policy` も付けます。`/api/docs` の Swagger UI には、インラインスクリプトを許可する緩いポリシーを適用します。
