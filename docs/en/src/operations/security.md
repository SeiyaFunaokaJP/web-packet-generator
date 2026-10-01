# Authentication and TLS

## Admin password

wpg has one admin account (`admin`). Its password comes from the first of these sources that is set:

1. `wpg serve --password <PASSWORD>`. Other users can see it in the process list, so use it only for quick tests.
2. `wpg serve --password-file <FILE>`. The file holds the plain-text password or an argon2 hash.
3. The environment variable `WPG_PASSWORD`, holding the plain-text password or an argon2 hash.
4. `auth.password_hash` in `wpg.toml`. `wpg config set-password` prompts for the password and writes this key. Use `--stdin` in scripts.
5. `auth.password_file` in `wpg.toml`, holding the plain-text password or an argon2 hash.
6. The first-run setup screen, when none of the above is set.

wpg hashes plain-text passwords with argon2id at startup (19 MiB, 2 passes; the OWASP recommendation) and stores the hash in `<data_dir>/wpg.sqlite`. If a configured password changes, the next start ends all existing sessions.

### Without a password

Until a password exists:

- every API endpoint except `/api/health`, `/api/version`, `/api/auth/*` and `/api/docs` answers `401 setup_required`;
- `wpg serve` refuses to listen on a non-loopback address and exits with status 2. Pass `--allow-insecure` to override this. wpg then logs a loud warning, and anyone who can reach the server can set the password;
- the setup screen accepts requests only from the machine itself (loopback), unless the server runs with `--allow-insecure`.

```sh
wpg serve                                  # 127.0.0.1:8080: open it and set the password
wpg config set-password                    # or: store a hash in wpg.toml first
wpg serve --bind 0.0.0.0:8080 --tls        # now any address is fine
```

## Sessions and CSRF

- After a login, the `wpg_session` cookie identifies the session. It holds a random 256-bit token and is set with `HttpOnly; SameSite=Strict`, plus `Secure` over HTTPS. The database stores only the token's SHA-256.
- Sessions last `auth.session_ttl_secs` (default 24 h). Every request extends the session.
- POST, PUT and DELETE requests must send the session's CSRF token in `X-CSRF-Token`. The server returns the token in `GET /api/auth/status` and in the readable `wpg_csrf` cookie. Login and setup take no CSRF token, but they accept only `Content-Type: application/json`.
- WebSocket connections (`/api/ws`) need the session cookie. The server rejects them when their `Origin` names another host.

API: `GET /api/auth/status`, `POST /api/auth/setup {password, confirm}`, `POST /api/auth/login {password, username?}`, `POST /api/auth/logout`. See the [API reference](../reference/api.md).

## Login rate limit

wpg allows 5 failed logins per minute per client address. After that, the client is locked out for 1 minute. The lock-out doubles each time it happens again, up to 15 minutes. While a client is locked out, logins answer `429 rate_limited` with `details.retry_after_secs` and a `Retry-After` header.

wpg records every attempt in the `login_attempts` table and keeps it for 30 days. Behind a reverse proxy, all clients share the proxy's address.

## Audit log

Logins, failed logins, logouts, the setup and every job action (create, start, pause, resume, stop, delete) are logged with the `wpg::audit` target, the user and the client address. To log only these events, set `RUST_LOG=wpg::audit=info`.

## TLS

- `wpg serve --tls`, or `server.self_signed_tls = true`, generates a self-signed ECDSA certificate into `<data_dir>/tls/` and reuses it on later starts. wpg renews it about a month before it expires; certificates are valid for 800 days. It covers `localhost`, `127.0.0.1`, `::1`, the bind address and the host name. wpg prints the certificate's SHA-256 fingerprint at startup, so you can compare it with the one your browser shows.
- To use your own certificate, set `[server.tls] cert = "...", key = "..."` (PEM files) in `wpg.toml`. This takes precedence over the self-signed certificate.
- The server does not answer plain HTTP on a TLS port and does not redirect to HTTPS.

Behind a reverse proxy (nginx, Caddy), keep wpg on `127.0.0.1` and let the proxy handle TLS. Forward the WebSocket upgrade for `/api/ws`.

## Security headers

Every response carries a Content Security Policy (`default-src 'self'`; scripts only from the server plus `'wasm-unsafe-eval'` and the hashes of the inline bootstrap script; inline styles; `connect-src 'self' ws: wss:`; `frame-ancestors 'none'`), plus `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer` and a restrictive `Permissions-Policy`. The Swagger UI under `/api/docs` gets a relaxed policy that allows its inline scripts.
