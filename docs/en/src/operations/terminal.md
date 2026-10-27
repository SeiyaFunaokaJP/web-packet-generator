# Web terminal (SSH / Telnet)

The wpg UI can open SSH and Telnet sessions to the devices under test. While sending packets you can watch the counters and logs of the peer without a separate terminal program (Tera Term, PuTTY) or jump host. The wpg server opens the connections, so it also works in labs where your PC cannot reach the devices directly (CML2, EVE-NG, closed hardware labs). The design is recorded in [ADR-0011](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/adr/0011-web-terminal.md).

## Enabling it

The web terminal is disabled by default. Add a `[terminal]` section to `wpg.toml` and restart the server.

```toml
[terminal]
enabled = true                                 # Web terminal (SSH / Telnet)
allowed_targets = ["10.0.0.0/8", "*.lab.example", "console-srv:2001"]
record_sessions = true                         # keep session logs under <data_dir>/terminal-logs
idle_timeout_secs = 1800                       # close idle sessions after this many seconds (0 = never)
max_sessions = 8                               # simultaneous sessions, all users
```

| Key | Default | Meaning |
|---|---|---|
| `enabled` | `false` | Enable the web terminal |
| `allowed_targets` | `[]` | Targets sessions may connect to; empty = any target |
| `record_sessions` | `true` | Allow session logs (`false` disables server-side logs) |
| `idle_timeout_secs` | `1800` | Seconds without input or output before a session is closed; `0` = never |
| `max_sessions` | `8` | Simultaneous sessions |

Environment variables work as well (`WPG_TERMINAL__ENABLED=true`, `WPG_TERMINAL__ALLOWED_TARGETS='["10.0.0.0/8"]'`). Check the file with `wpg config validate`; it warns when the terminal is enabled with an empty `allowed_targets` on a non-loopback address.

### allowed_targets syntax

| Entry | Meaning |
|---|---|
| `r1`, `r1.lab.example` | That host name (case-insensitive), any port |
| `r1:22` | That host name, port 22 only |
| `*.lab.example` | Every subdomain of `lab.example` |
| `192.0.2.10`, `192.0.2.10:2001` | That IP address (and port) |
| `[2001:db8::1]:830` | IPv6 address and port |
| `10.0.0.0/8`, `2001:db8::/32` | An address block (CIDR) |

Host name entries match the name the user typed. Address and CIDR entries match the addresses the name resolves to, and the session only connects to the addresses that matched. The checked address is the one dialled, so changing DNS afterwards cannot bypass the list.

## Using it

Open **Terminal** in the sidebar (shortcut `g c`).

1. In the connection form choose the protocol (SSH / Telnet), host, port (default 22 / 23) and, for SSH, the user name.
2. Choose the SSH authentication:
   - **Password**: log in with the password. Leave it empty to get the server's prompts (keyboard-interactive, password) in a dialog. A single hidden keyboard-interactive prompt ("Password:", common on network devices) is answered with the password you entered.
   - **Private key**: paste or load an OpenSSH, PEM or PuTTY `.ppk` key; passphrases are supported.
3. **Connect** opens a new tab. Several tabs can be open at once and they stay connected while you use other pages.

**Save profile** stores the protocol, host, port, user name, authentication method and log options in the browser (localStorage). Passwords, keys and passphrases are never stored.

### First connection to an SSH host

For a host it has not seen before, wpg shows the host key algorithm and its SHA-256 fingerprint (`SHA256:...`). Compare it with the real value before you continue, for example:

- run `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` on the host itself;
- run `ssh-keyscan -p 22 host | ssh-keygen -lf -` from a machine with a trusted path to the host;
- on network devices, convert the public key the device shows (`show crypto key mypubkey rsa` on Cisco IOS, ...) to OpenSSH format and pass it to `ssh-keygen -lf`; some devices print MD5 or hexadecimal fingerprints, so compare like with like.

Then choose:

- **Trust and save**: store the key in the server's `<data_dir>/terminal/known_hosts` (OpenSSH known_hosts format) and connect. Later connections are silent.
- **Connect once**: connect without storing the key (useful for lab devices that are rebuilt often).
- **Cancel**: do not connect.

When a host presents a key that differs from the stored one, the connection is refused (possible man-in-the-middle) and both keys are shown. If the change is expected (device replaced, keys regenerated), remove the stored key under **Known hosts** at the bottom of the page and connect again to confirm the new key. You can also pre-seed `known_hosts` with lines from `ssh-keyscan`.

### Telnet

The Telnet client only answers the server's option requests (ECHO, SGA, NAWS window size, terminal type `XTERM-256COLOR`, BINARY); it never starts a negotiation, so raw TCP console ports of terminal servers (CML2, EVE-NG consoles) never see stray bytes. Enter is sent as CR NUL as the NVT rules require.

### Copy and paste

| Action | How |
|---|---|
| Copy | Select (copied automatically by default), `Ctrl+Shift+C`, `Ctrl+Insert` |
| Paste | Right / middle click, `Ctrl+Shift+V`, `Shift+Insert`, `Ctrl+V` |
| Find | The search box of the toolbar (Enter: next, Shift+Enter: previous) |

Pasting several lines shows the line count and a preview first (can be switched off). With a **pause per line** each line is followed by that many milliseconds, for consoles with small input buffers when pasting configurations. **Send file** sends a text file after the same confirmation.

Browsers only allow the clipboard API on HTTPS or localhost. Over plain HTTP from another host, right-click paste cannot read the clipboard: use `Ctrl+V` or `Shift+Insert` (copying still works).

Font size, scrollback lines, copy on select, right-click paste, paste confirmation and the pause per line are in **Terminal settings** and stored in the browser.

### Session logs

With `record_sessions = true`, "Record a session log on the server" (on by default) writes what the remote side sends to `<data_dir>/terminal-logs/`, named like `20261027-101500_ssh_r1.lab_22_ab12cd34.log` (UTC time, protocol, host, port, session id).

- Only the remote side's output is recorded, never your keystrokes (input that is not echoed, such as passwords, does not end up in the log).
- By default escape sequences are removed (Tera Term's "plain text"); without stripping, `cat` replays the log with colours.
- **Start log / Stop log** in the toolbar control recording during a session; restarting creates a new file.
- A file stops growing at 256 MiB (the session continues).
- The **Session logs** list downloads and deletes logs; a log that is being written cannot be deleted.
- Logs are never deleted automatically; manage retention yourself. Files are created with mode 0600.

Without server-side logs, **Save scrollback** saves the terminal's scrollback from the browser as a text file.

## Security

With the web terminal enabled the wpg server becomes a jump host into the test network:

- **Restrict the targets**: list only the lab ranges in `allowed_targets`. Left empty, everyone who can log in to wpg can reach any host the server can reach, management networks and internal servers included.
- **Use HTTPS**: passwords and private keys travel from the browser to the wpg server. Away from localhost use `wpg serve --tls` or a certificate ([Authentication and TLS](security.md)).
- **No stored secrets**: wpg never writes passwords, keys or passphrases to disk; they stay in memory while the connection is being set up. The browser tab keeps what you typed in memory for reconnecting until the tab is closed.
- **Audit trail**: session start, end and refusals are logged with target `wpg::audit` (wpg user, client address, target, login name, end reason, bytes in / out, log file). `RUST_LOG=wpg::audit=info` prints only audit events. Storing / removing host keys and deleting logs are logged as well.
- **Access**: only logged-in users; the WebSocket only accepts same-origin connections (including a reverse proxy configured in `server.base_url`). Users with the viewer role (P1) cannot open sessions.
- **Idle timeout and session limit**: abandoned sessions are closed after `idle_timeout_secs`; `max_sessions` caps simultaneous sessions.
- **Legacy algorithms**: to reach old network equipment, SHA-1 key exchange and CBC ciphers are offered with the lowest priority; they are only used when the device offers nothing better.

## Reverse proxy

Behind a reverse proxy, forward the WebSocket upgrade of `/api/terminal/ws` like that of `/api/ws`. nginx:

```nginx
location /api/terminal/ws {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_read_timeout 3600s;
}
```

Keep `proxy_read_timeout` above `idle_timeout_secs`.

## API

| Method | Path | Purpose |
|---|---|---|
| GET | `/api/terminal/status` | Configuration (enabled, allowlist) and running sessions |
| GET | `/api/terminal/ws` | Session WebSocket (`403 terminal_disabled` when disabled) |
| GET | `/api/terminal/logs` | Session logs |
| GET / DELETE | `/api/terminal/logs/{name}` | Download / delete a log |
| GET | `/api/terminal/known-hosts` | Stored host keys |
| DELETE | `/api/terminal/known-hosts/{host}/{port}` | Remove a host key |

On the WebSocket, send `{"type":"connect","protocol":"ssh","host":"r1","username":"admin","password":"...","cols":120,"rows":40}` first. The server answers with JSON `state`, `host_key`, `auth_prompt`, `connected`, `record`, `error` and `closed` messages and sends the terminal output as binary frames; keystrokes go back as binary frames (or `{"type":"input","data":"..."}`). See `TerminalClientMessage` / `TerminalServerMessage` in the [API reference](../reference/api.md).

## Troubleshooting

| Message | Cause and fix |
|---|---|
| "This host is not in the server's allowed targets" | Add it to `allowed_targets` and restart the server |
| "the host key has changed" | The device's key changed. If that is legitimate, remove the old key under **Known hosts** |
| "SSH error: No common ... algorithm" | The device shares no algorithm with wpg; enable newer ones on the device (aes-ctr, ecdh, ed25519, ...) |
| "Login failed" | Check user name, password or key; some devices disable public key authentication |
| Right-click paste does nothing | You use plain HTTP from another host; use `Ctrl+V` / `Shift+Insert` or HTTPS |
| Garbled layout | Resizing the window notifies the device; for Telnet devices without NAWS set the size on the device (`terminal length 0`, ...) |
