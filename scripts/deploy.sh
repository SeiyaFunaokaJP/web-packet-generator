#!/usr/bin/env bash
# Deploy a wpg build to a Linux host over SSH (spec §2.8; first target: Ubuntu on CML2).
#
# Usage: scripts/deploy.sh                      (no arguments: interactive wizard)
#        scripts/deploy.sh <user@host> [options]
#   --interactive           ask everything step by step (also the default without arguments)
#   --iface <name>          interface jobs may send on (repeatable; tx.allowed_ifaces)
#   --service               install / update the systemd unit and (re)start it
#   --bind <addr:port>      listen address of the service (default 0.0.0.0:8080)
#   --password-file <file>  local file with the admin password or its argon2 hash;
#                           installed as /etc/wpg/password (0640 root:wpg)
#   --allow-insecure        start without a password on a non-loopback address
#   --binary <file>         binary to deploy (default: target/<target>/release/wpg,
#                           built with scripts/build.sh --release when missing)
#   --target <triple>       cargo target of the binary (default: host target)
#   --build-on-host         build from source on the host instead (needs internet there;
#                           installs build-essential, libpcap-dev, git, rustup and trunk)
#   --source <local|git>    with --build-on-host: upload this checkout (default) or fetch
#                           --git-url at --git-ref (default: GitHub, main) on the host
#   --git-url <url>, --git-ref <ref>
#   --release-tag <tag>     deploy the Linux binary of a GitHub Release ("latest" for the newest)
#   --prefix <dir>          install directory on the host (default /usr/local)
#   --port <n>              SSH port
#   --identity <file>       SSH private key
#   --ask-password          ask for the SSH password once and install a deploy key
#                           (~/.ssh/wpg_deploy_ed25519) on the host; later runs use the key
#   --sudo-password         ask for the sudo password and pass it to sudo -S on stdin
#                           (default: sudo asks on the SSH terminal when it needs one)
#   --ssh-opt <opt>         extra ssh / scp -o option (repeatable)
#   --terminal              enable the web terminal ([terminal], only in a new wpg.toml)
#   --snmp-trap             enable the SNMP trap receiver on UDP 162 ([snmp] trap_listen)
#   --no-templates          do not copy the sample templates
#   --no-healthcheck        skip GET /api/health after starting the service
#   --rollback              restore the binary saved by the previous deploy and restart
#   --dpdk                  prepare the host for DPDK (hugepages, vfio-pci). NICs are
#                           not bound and the binary must be built with --features dpdk,
#                           see docs/*/src/operations/dpdk-afxdp.md
#   --hugepages <n>         number of 2 MiB hugepages for --dpdk (default 1024)
#   --profile <file>        wizard: answers (no secrets) to load and save
#                           (default ~/.config/wpg/deploy.conf)
#   --lang <ja|en>          language of the wizard (default: from LANG)
#   --log <file>            also write the output to this file (wizard: always, under
#                           ~/.local/state/wpg/)
#   --dry-run               print the commands instead of running them
#   -h, --help              this help
#
# The remote user needs sudo. Re-running the script upgrades in place: the previous
# binary is kept as <prefix>/bin/wpg.prev, /etc/wpg/wpg.toml is never overwritten.
# SC2029: remote commands are deliberately built on this side. SC2015: `a && b || c`
# is only used where b cannot fail.
# shellcheck disable=SC2029,SC2015
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() { sed -n '2,48p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    c_step=$'\033[1;36m' c_ok=$'\033[1;32m' c_warn=$'\033[1;33m' c_err=$'\033[1;31m' c_off=$'\033[0m'
else
    c_step="" c_ok="" c_warn="" c_err="" c_off=""
fi

host=""
ifaces=()
service=0
bind="0.0.0.0:8080"
password_file=""
allow_insecure=0
binary=""
target=""
prefix="/usr/local"
ssh_port=""
ssh_opts=()
templates=1
healthcheck=1
rollback=0
dpdk=0
hugepages=1024
dry_run=0
interactive=0
build_on_host=0
source_kind="local"
gh_repo="${WPG_GITHUB_REPO:-SeiyaFunaokaJP/web-packet-generator}"
git_url="https://github.com/$gh_repo.git"
git_ref="main"
release_tag=""
identity=""
ask_password=0
sudo_mode="tty"
ask_sudo=0
terminal=0
snmp_trap=0
profile="${XDG_CONFIG_HOME:-$HOME/.config}/wpg/deploy.conf"
lang=""
log_file=""
ssh_pw=""
sudo_pw=""
admin_pw=""
deploy_key="$HOME/.ssh/wpg_deploy_ed25519"
current_step=""
step_no=0
step_total=0
askpass_dir=""
tmp_dir=""
rollback_health=0

need_value() { [ "$2" -ge 2 ] || die "$1 needs a value"; }

[ "$#" -gt 0 ] || interactive=1
while [ "$#" -gt 0 ]; do
    case "$1" in
        --interactive) interactive=1 ;;
        --iface) need_value "$1" "$#"; ifaces+=("$2"); shift ;;
        --service) service=1 ;;
        --bind) need_value "$1" "$#"; bind="$2"; shift ;;
        --password-file) need_value "$1" "$#"; password_file="$2"; shift ;;
        --allow-insecure) allow_insecure=1 ;;
        --binary) need_value "$1" "$#"; binary="$2"; shift ;;
        --target) need_value "$1" "$#"; target="$2"; shift ;;
        --build-on-host) build_on_host=1 ;;
        --source) need_value "$1" "$#"; source_kind="$2"; shift ;;
        --git-url) need_value "$1" "$#"; git_url="$2"; shift ;;
        --git-ref) need_value "$1" "$#"; git_ref="$2"; shift ;;
        --release-tag) need_value "$1" "$#"; release_tag="$2"; shift ;;
        --prefix) need_value "$1" "$#"; prefix="$2"; shift ;;
        --port) need_value "$1" "$#"; ssh_port="$2"; shift ;;
        --identity) need_value "$1" "$#"; identity="$2"; shift ;;
        --ask-password) ask_password=1 ;;
        --sudo-password) ask_sudo=1 ;;
        --ssh-opt) need_value "$1" "$#"; ssh_opts+=(-o "$2"); shift ;;
        --terminal) terminal=1 ;;
        --snmp-trap) snmp_trap=1 ;;
        --no-templates) templates=0 ;;
        --no-healthcheck) healthcheck=0 ;;
        --rollback) rollback=1 ;;
        --dpdk) dpdk=1 ;;
        --hugepages) need_value "$1" "$#"; hugepages="$2"; shift ;;
        --profile) need_value "$1" "$#"; profile="$2"; shift ;;
        --lang) need_value "$1" "$#"; lang="$2"; shift ;;
        --log) need_value "$1" "$#"; log_file="$2"; shift ;;
        --dry-run) dry_run=1 ;;
        -h | --help) usage; exit 0 ;;
        -*) die "unknown option: $1 (see --help)" ;;
        *)
            [ -z "$host" ] || die "unexpected argument: $1"
            host="$1"
            ;;
    esac
    shift
done

if [ -z "$lang" ]; then
    case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in ja*) lang=ja ;; *) lang=en ;; esac
fi
case "$lang" in ja | en) ;; *) die "--lang must be ja or en" ;; esac
case "$source_kind" in local | git) ;; *) die "--source must be local or git" ;; esac

# m <japanese> <english>: the message in the wizard language.
m() { if [ "$lang" = ja ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '%s\n' "$(m "$1" "$2")"; }
warn() { printf '%s%s%s\n' "$c_warn" "$(m "警告: $1" "warning: $2")" "$c_off" >&2; }

step() {
    step_no=$((step_no + 1))
    current_step="$1"
    if [ "$step_total" -gt 0 ]; then
        printf '\n%s[%d/%d] %s%s\n' "$c_step" "$step_no" "$step_total" "$(m "$2" "$3")" "$c_off"
    else
        printf '\n%s[%d] %s%s\n' "$c_step" "$step_no" "$(m "$2" "$3")" "$c_off"
    fi
}

run() {
    if [ "$dry_run" -eq 1 ]; then
        printf '+'
        printf ' %q' "$@"
        printf '\n'
    else
        "$@"
    fi
}

cleanup() {
    [ -z "$askpass_dir" ] || rm -rf "$askpass_dir"
    [ -z "$tmp_dir" ] || rm -rf "$tmp_dir"
}

# What to try when a step fails (shown with the last lines of the output).
hint() {
    case "$1" in
        connect) say "ホスト名 / IP、SSH ポート、ユーザ名を確認してください。手動で接続できるか試してください: ssh -p ${ssh_port:-22} $host" \
            "Check the host name / IP, the SSH port and the user name. Try connecting by hand: ssh -p ${ssh_port:-22} $host" ;;
        sudo) say "sudo パスワードが違うか、ユーザに sudo 権限がありません (sudo グループに入っているか確認してください)。" \
            "Wrong sudo password, or the user may not use sudo (is it in the sudo group?)." ;;
        binary) say "ローカルのビルドに失敗しました。ターゲット上でビルドする (--build-on-host) か、--binary で既存のバイナリを指定してください。" \
            "The local build failed. Build on the host instead (--build-on-host) or pass an existing binary with --binary." ;;
        upload) say "アップロードに失敗しました。ホストの /tmp の空き容量と SSH 接続を確認してください。" \
            "The upload failed. Check the free space in /tmp on the host and the SSH connection." ;;
        deps) say "パッケージの導入に失敗しました。ホストからインターネットに出られるか (プロキシ設定)、apt が他の処理 (unattended-upgrades) にロックされていないか確認し、数分後に再実行してください。" \
            "Installing packages failed. Check that the host can reach the internet (proxy?) and that apt is not locked by another process (unattended-upgrades); retry in a few minutes." ;;
        build) say "ビルドに失敗しました。メモリ不足 (killed) ならスワップを追加してください。ネットワークエラーならプロキシを確認してください。最初からやり直すにはホスト上で rm -rf ~/wpg-build を実行します。" \
            "The build failed. If rustc was killed, add swap. On network errors check the proxy. To start from scratch run rm -rf ~/wpg-build on the host." ;;
        install) say "インストールに失敗しました。上のエラーを確認してください (バイナリがホストで動かない場合は、ホスト上でビルドしてください)。" \
            "The installation failed; see the error above (if the binary does not run on the host, build it on the host)." ;;
        health) say "wpg が応答しません。ホストで sudo journalctl -u wpg -n 50 を確認してください。ポートの重複は sudo ss -ltnp で確認できます。" \
            "wpg does not answer. Check sudo journalctl -u wpg -n 50 on the host; sudo ss -ltnp shows a port already in use." ;;
    esac
}

on_exit() {
    local rc=$?
    cleanup
    if [ "$rc" -ne 0 ] && [ -n "$current_step" ]; then
        printf '\n%s%s%s\n' "$c_err" "$(m "失敗しました (ステップ $step_no: $current_step, 終了コード $rc)" "FAILED (step $step_no: $current_step, exit code $rc)")" "$c_off" >&2
        hint "$current_step" >&2
        if [ -n "$log_file" ] && [ -f "$log_file" ]; then
            say "ログ: $log_file" "log: $log_file" >&2
        fi
    fi
}
trap on_exit EXIT

# ---- prompts (wizard) -------------------------------------------------------
# All prompts read stdin and write to stderr, so answers can also be piped in.
prompt_line() { # <var> <label>
    local __ans
    printf '%s' "$2" >&2
    IFS= read -r __ans || die "$(m "入力が終わりました" "end of input")"
    printf -v "$1" '%s' "$__ans"
}

ask() { # <var> <ja> <en> <default>
    local __a
    if [ -n "$4" ]; then
        prompt_line __a "$(m "$2" "$3") [$4]: "
    else
        prompt_line __a "$(m "$2" "$3"): "
    fi
    [ -n "$__a" ] || __a="$4"
    printf -v "$1" '%s' "$__a"
}

ask_required() { # <var> <ja> <en> <default>
    local __r=""
    while [ -z "$__r" ]; do
        ask __r "$2" "$3" "$4"
        [ -n "$__r" ] || say "  入力してください。" "  Please enter a value."
    done
    printf -v "$1" '%s' "$__r"
}

ask_secret() { # <var> <ja> <en>   (input not echoed)
    local __s
    printf '%s: ' "$(m "$2" "$3")" >&2
    if [ -t 0 ]; then
        IFS= read -r -s __s || die "$(m "入力が終わりました" "end of input")"
        printf '\n' >&2
    else
        IFS= read -r __s || die "$(m "入力が終わりました" "end of input")"
    fi
    printf -v "$1" '%s' "$__s"
}

ask_yn() { # <var> <ja> <en> <default 1|0>
    local __y hint_yn
    if [ "$4" -eq 1 ]; then hint_yn="Y/n"; else hint_yn="y/N"; fi
    while :; do
        prompt_line __y "$(m "$2" "$3") [$hint_yn]: "
        case "$__y" in
            "") printf -v "$1" '%s' "$4"; return ;;
            y | Y | yes | YES | はい) printf -v "$1" 1; return ;;
            n | N | no | NO | いいえ) printf -v "$1" 0; return ;;
        esac
        say "  y か n で答えてください。" "  Please answer y or n."
    done
}

ask_choice() { # <var> <default> <ja title> <en title> "<ja>|<en>"...
    local __var="$1" __def="$2" __title_ja="$3" __title_en="$4" __c __i
    shift 4
    say "$__title_ja" "$__title_en" >&2
    __i=1
    for __o in "$@"; do
        printf '  %d) %s\n' "$__i" "$(m "${__o%%|*}" "${__o#*|}")" >&2
        __i=$((__i + 1))
    done
    while :; do
        prompt_line __c "$(m "番号を選んでください" "Choose a number") [$__def]: "
        [ -n "$__c" ] || __c="$__def"
        case "$__c" in
            '' | *[!0-9]*) ;;
            *) if [ "$__c" -ge 1 ] && [ "$__c" -le "$#" ]; then printf -v "$__var" '%s' "$__c"; return; fi ;;
        esac
        say "  1 から $# の番号を入力してください。" "  Enter a number from 1 to $#."
    done
}

# ---- profile (wizard answers without secrets) -------------------------------
p_host="" p_user="" p_bin="" p_binary_file=""
load_profile() {
    local k v
    while IFS='=' read -r k v; do
        case "$k" in
            host) p_host="$v" ;;
            user) p_user="$v" ;;
            ssh_port) ssh_port="$v" ;;
            identity) identity="$v" ;;
            binary_source) p_bin="$v" ;;
            binary_file) p_binary_file="$v" ;;
            source) case "$v" in local | git) source_kind="$v" ;; esac ;;
            git_url) git_url="$v" ;;
            git_ref) git_ref="$v" ;;
            bind) bind="$v" ;;
            ifaces) IFS=',' read -r -a ifaces <<<"$v" ;;
            service) service="${v:-0}" ;;
            terminal) terminal="${v:-0}" ;;
            snmp_trap) snmp_trap="${v:-0}" ;;
        esac
    done <"$profile"
}

save_profile() {
    local dir
    dir="$(dirname "$profile")"
    mkdir -p "$dir"
    {
        echo "# wpg deploy wizard answers (scripts/deploy.sh / deploy.ps1). No passwords are stored."
        echo "host=${host#*@}"
        echo "user=${host%@*}"
        echo "ssh_port=${ssh_port:-22}"
        echo "identity=$identity"
        echo "binary_source=$bin_choice"
        echo "binary_file=$p_binary_file"
        echo "source=$source_kind"
        echo "git_url=$git_url"
        echo "git_ref=$git_ref"
        echo "bind=$bind"
        echo "ifaces=$(IFS=,; echo "${ifaces[*]}")"
        echo "service=$service"
        echo "terminal=$terminal"
        echo "snmp_trap=$snmp_trap"
    } >"$profile"
    say "設定を保存しました: $profile (パスワードは保存していません)" "Saved the answers to $profile (no passwords)."
}

# ---- SSH ----------------------------------------------------------------------
ssh_args=()
scp_args=()
setup_ssh_args() {
    ssh_args=(-o BatchMode=no "${ssh_opts[@]}")
    scp_args=("${ssh_opts[@]}")
    if [ -n "$ssh_port" ]; then
        ssh_args+=(-p "$ssh_port")
        scp_args+=(-P "$ssh_port")
    fi
}

use_identity() {
    ssh_opts+=(-o "IdentityFile=$1" -o IdentitiesOnly=yes)
    setup_ssh_args
}

ssh_batch_ok() { # [extra ssh args...]: can we log in without a password?
    # ssh keeps the first value of an option, so these go before ssh_args (BatchMode=no).
    ssh -o BatchMode=yes -o ConnectTimeout=10 "$@" "${ssh_args[@]}" "$host" true </dev/null >/dev/null 2>&1
}

openssh_at_least() { # <major> <minor>
    local v
    v="$(ssh -V 2>&1 | sed -n 's/^OpenSSH_\([0-9]*\)\.\([0-9]*\).*/\1 \2/p')"
    [ -n "$v" ] || return 1
    local maj min
    read -r maj min <<<"$v"
    set -- "$1" "$2" "$maj" "$min"
    [ "$3" -gt "$1" ] || { [ "$3" -eq "$1" ] && [ "$4" -ge "$2" ]; }
}

# Logs in with a key when possible; otherwise installs ~/.ssh/wpg_deploy_ed25519 on the
# host using the SSH password once (sshpass, SSH_ASKPASS, or ssh's own prompt).
setup_key() {
    if [ -n "$identity" ]; then
        use_identity "$identity"
        ssh_batch_ok || die "$(m "鍵 $identity でログインできません" "cannot log in with the key $identity")"
        return 0
    fi
    if [ -f "$deploy_key" ] && ssh_batch_ok -o "IdentityFile=$deploy_key" -o IdentitiesOnly=yes; then
        use_identity "$deploy_key"
        say "  鍵 $deploy_key でログインできました。" "  Logged in with the key $deploy_key."
        return 0
    fi
    if ssh_batch_ok; then
        say "  既存の SSH 鍵 (ssh-agent / ~/.ssh) でログインできました。" "  Logged in with your existing SSH key (ssh-agent / ~/.ssh)."
        return 0
    fi
    [ -n "$ssh_pw" ] || ask_secret ssh_pw "SSH パスワード ($host)" "SSH password ($host)"
    if [ ! -f "$deploy_key" ]; then
        run mkdir -p "$(dirname "$deploy_key")"
        run ssh-keygen -q -t ed25519 -N "" -C "wpg-deploy@$(hostname 2>/dev/null || echo local)" -f "$deploy_key"
        say "  デプロイ用の鍵を作りました: $deploy_key" "  Created the deploy key $deploy_key"
    fi
    say "  公開鍵をホストの ~/.ssh/authorized_keys に登録します (パスワードを使うのはこの 1 回だけです)。" \
        "  Installing the public key into ~/.ssh/authorized_keys on the host (the password is used this once)."
    # shellcheck disable=SC2016 # expanded on the host
    local install_cmd='umask 077; mkdir -p ~/.ssh && k=$(cat) && touch ~/.ssh/authorized_keys && { grep -qxF "$k" ~/.ssh/authorized_keys || printf "%s\n" "$k" >>~/.ssh/authorized_keys; }'
    local pw_opts=(-o "PreferredAuthentications=password,keyboard-interactive" -o PubkeyAuthentication=no -o NumberOfPasswordPrompts=1)
    if [ "$dry_run" -eq 1 ]; then
        echo "+ ssh ${ssh_args[*]} ${pw_opts[*]} $host '<install $deploy_key.pub>' (password via sshpass / SSH_ASKPASS)"
        use_identity "$deploy_key"
        return 0
    fi
    local ok=0
    if command -v sshpass >/dev/null 2>&1; then
        SSHPASS="$ssh_pw" sshpass -e ssh "${pw_opts[@]}" "${ssh_args[@]}" "$host" "$install_cmd" <"$deploy_key.pub" && ok=1
    elif openssh_at_least 8 4; then
        # The helper prints the password from its environment; nothing is written to disk.
        askpass_dir="$(mktemp -d)"
        # shellcheck disable=SC2016 # expanded by the helper
        printf '#!/bin/sh\nprintf "%%s\\n" "$WPG_DEPLOY_ASKPASS"\n' >"$askpass_dir/askpass"
        chmod 0700 "$askpass_dir/askpass"
        WPG_DEPLOY_ASKPASS="$ssh_pw" SSH_ASKPASS="$askpass_dir/askpass" SSH_ASKPASS_REQUIRE=force DISPLAY="${DISPLAY:-:0}" \
            ssh "${pw_opts[@]}" "${ssh_args[@]}" "$host" "$install_cmd" <"$deploy_key.pub" && ok=1
        rm -rf "$askpass_dir"
        askpass_dir=""
    fi
    if [ "$ok" -eq 0 ]; then
        say "  自動入力できませんでした。ssh がパスワードを聞いたら、もう一度入力してください。" \
            "  Could not pass the password automatically; enter it again when ssh asks."
        ssh "${ssh_args[@]}" -o PubkeyAuthentication=no "$host" "$install_cmd" <"$deploy_key.pub" \
            || die "$(m "公開鍵を登録できませんでした (パスワードが違う?)" "could not install the public key (wrong password?)")"
    fi
    ssh_batch_ok -o "IdentityFile=$deploy_key" -o IdentitiesOnly=yes \
        || die "$(m "鍵を登録しましたが、鍵でログインできません (サーバ側で公開鍵認証が無効?)" "the key was installed but key login fails (public key authentication disabled on the server?)")"
    use_identity "$deploy_key"
    printf '%s  %s%s\n' "$c_ok" "$(m "鍵でログインできるようになりました。次回からパスワードは不要です。" "Key login works; no password needed next time.")" "$c_off"
}

# Runs <root command> on the host as root (sudo as needed), then <user suffix> as the user.
ssh_root() { # <root command> [user suffix]
    local cmd="$1" suffix="${2:-}"
    case "$sudo_mode" in
        root) run ssh "${ssh_args[@]}" "$host" "$cmd$suffix" </dev/null ;;
        nopasswd) run ssh "${ssh_args[@]}" "$host" "sudo -n $cmd$suffix" </dev/null ;;
        stdin)
            if [ "$dry_run" -eq 1 ]; then
                printf '+ ssh %s %s %q   # sudo password on stdin\n' "${ssh_args[*]}" "$host" "sudo -S -p '' $cmd$suffix"
            else
                printf '%s\n' "$sudo_pw" | ssh "${ssh_args[@]}" "$host" "sudo -S -p '' $cmd$suffix"
            fi
            ;;
        # -t: sudo may need to ask for a password.
        *) run ssh -t "${ssh_args[@]}" "$host" "sudo $cmd$suffix" ;;
    esac
}

check_sudo() { # verifies the sudo password (stdin mode)
    [ "$sudo_mode" = stdin ] || return 0
    [ "$dry_run" -eq 0 ] || return 0
    printf '%s\n' "$sudo_pw" | ssh "${ssh_args[@]}" "$host" "sudo -S -p '' true" >/dev/null 2>&1
}

# ---- GitHub Release -----------------------------------------------------------
# release_asset_url <tag|latest> <arch>: prints the archive URL, nothing when there is none.
release_asset_url() {
    local api json
    if [ "$1" = latest ]; then
        api="https://api.github.com/repos/$gh_repo/releases/latest"
    else
        api="https://api.github.com/repos/$gh_repo/releases/tags/$1"
    fi
    json="$(curl -fsSL --max-time 20 "$api" 2>/dev/null)" || return 0
    printf '%s\n' "$json" | grep -o '"browser_download_url": *"[^"]*-'"$2"'-unknown-linux-gnu\.tar\.gz"' \
        | head -n 1 | sed 's/.*"\(https[^"]*\)"$/\1/'
}

# ---- probe ----------------------------------------------------------------------
probe_out=""
probe() { # <key>: a value from remote-probe.sh output
    printf '%s\n' "$probe_out" | sed -n "s/^WPG_PROBE_$1=//p" | head -n 1
}

run_probe() {
    probe_out="$(ssh "${ssh_args[@]}" "$host" 'bash -s' <"$ROOT/scripts/lib/remote-probe.sh" 2>/dev/null)" || probe_out=""
}

# ---- wizard -------------------------------------------------------------------
bin_choice=""
summary_binary() {
    case "$bin_choice" in
        build)
            if [ "$source_kind" = git ]; then
                echo "$(m "ホスト上でビルド" "build on the host") (git: $git_url $git_ref)"
            else
                echo "$(m "ホスト上でビルド" "build on the host") ($(m "このマシンのソース" "this checkout"): $ROOT)"
            fi
            ;;
        local) m "このマシンでビルドしてアップロード" "build here and upload" ;;
        file) echo "$binary" ;;
        release) echo "GitHub Release (latest)" ;;
    esac
}

wizard() {
    local use_prev=0 h u v n i list name state addrs mgmt def_list answer
    printf '%s%s%s\n' "$c_step" "$(m "wpg デプロイウィザード (Enter で [ ] 内の既定値を使います。Ctrl+C で中止)" "wpg deploy wizard (Enter accepts the default in [ ]; Ctrl+C aborts)")" "$c_off"
    if [ -n "$host" ]; then
        p_host="${host#*@}"
        case "$host" in *@*) p_user="${host%@*}" ;; esac
    fi
    if [ -f "$profile" ]; then
        load_profile
        say "前回の設定: $p_user@$p_host (ポート ${ssh_port:-22}, $bind)" "Previous settings: $p_user@$p_host (port ${ssh_port:-22}, $bind)"
        ask_yn use_prev "前回の設定を使いますか?" "Use the previous settings?" 1
    fi

    # 1. connection
    if [ "$use_prev" -eq 1 ] && [ -n "$p_host" ]; then
        h="$p_host"; u="$p_user"
    else
        ask_required h "接続先ホスト (IP アドレスまたはホスト名)" "Target host (IP address or name)" "$p_host"
        ask v "SSH ポート" "SSH port" "${ssh_port:-22}"
        ssh_port="$v"
        ask_required u "SSH ユーザ名" "SSH user" "${p_user:-ubuntu}"
        ask_choice v 1 "SSH の認証方法:" "SSH authentication:" \
            "パスワード (初回だけ使い、デプロイ用の鍵を登録します)|Password (used once to install a deploy key)" \
            "既存の秘密鍵ファイル|An existing private key file"
        if [ "$v" -eq 2 ]; then
            ask_required identity "秘密鍵ファイルのパス" "Private key file" "${identity:-$HOME/.ssh/id_ed25519}"
        else
            identity=""
        fi
    fi
    host="$u@$h"
    [ "${ssh_port:-22}" != 22 ] || ssh_port=""
    ssh_opts+=(-o StrictHostKeyChecking=accept-new -o ConnectTimeout=15)
    setup_ssh_args

    step connect "$host に接続" "Connecting to $host"
    setup_key
    run_probe
    if [ -z "$probe_out" ]; then
        warn "ホストの情報を取得できませんでした" "could not read the host facts"
    else
        say "  OS: $(probe OS_NAME) ($(probe ARCH))   sudo: $(probe SUDO)   wpg: $(probe WPG_VERSION | sed 's/^$/-/')" \
            "  OS: $(probe OS_NAME) ($(probe ARCH))   sudo: $(probe SUDO)   wpg: $(probe WPG_VERSION | sed 's/^$/-/')"
        [ "$(probe KERNEL)" = Linux ] || die "$(m "Linux ホストにだけデプロイできます" "only Linux hosts are supported")"
        case "$(probe OS_ID)" in
            ubuntu | debian) ;;
            *) warn "$(probe OS_NAME) は Ubuntu / Debian 以外です。できる範囲で進めます。" "$(probe OS_NAME) is not Ubuntu / Debian; continuing on a best-effort basis." ;;
        esac
    fi

    # 2. upgrade or rollback
    if [ "$(probe WPG_PREV)" = 1 ]; then
        ask_choice v 1 "wpg はインストール済みです ($(probe WPG_VERSION))。何をしますか?" "wpg is installed ($(probe WPG_VERSION)). What do you want to do?" \
            "インストール / アップグレード|Install / upgrade" \
            "ロールバック (1 つ前のバイナリ wpg.prev に戻す)|Roll back (restore the previous binary wpg.prev)" \
            "中止|Quit"
        case "$v" in 2) rollback=1 ;; 3) exit 0 ;; esac
    fi

    # 3. sudo
    case "$(probe SUDO)" in
        root) sudo_mode=root ;;
        nopasswd) sudo_mode=nopasswd ;;
        none) die "$(m "ホストに sudo がありません。root でログインするか sudo を入れてください。" "sudo is missing on the host; log in as root or install sudo.")" ;;
        *)
            sudo_mode=stdin
            for i in 1 2 3; do
                if [ -n "$ssh_pw" ]; then
                    ask_secret sudo_pw "sudo パスワード (Enter = SSH パスワードと同じ)" "sudo password (Enter = same as the SSH password)"
                    [ -n "$sudo_pw" ] || sudo_pw="$ssh_pw"
                else
                    ask_secret sudo_pw "sudo パスワード" "sudo password"
                fi
                if check_sudo; then break; fi
                [ "$i" -lt 3 ] || { current_step=sudo; die "$(m "sudo に失敗しました" "sudo failed")"; }
                say "  sudo パスワードが違います。もう一度入力してください。" "  Wrong sudo password; try again."
            done
            ;;
    esac

    if [ "$rollback" -eq 1 ]; then
        ask_yn answer "/usr/local/bin/wpg と wpg.prev を入れ替えてサービスを再起動します。よろしいですか?" \
            "Swap /usr/local/bin/wpg with wpg.prev and restart the service?" 1
        [ "$answer" -eq 1 ] || { say "中止しました。" "Aborted."; exit 0; }
        # deploy-remote.sh restarts an enabled service itself; --service would also
        # rewrite wpg.env. Check the health only when the listen address is known.
        [ "$(probe SERVICE)" != absent ] && [ "$use_prev" -eq 1 ] && rollback_health=1
        service=0 terminal=0 snmp_trap=0 admin_pw=""
        return 0
    fi

    # 4. binary
    local internet arch
    internet="$(probe INTERNET)"
    arch="$(probe ARCH)"
    if [ "$use_prev" -eq 1 ] && [ -n "$p_bin" ]; then
        bin_choice="$p_bin"
    else
        local rel_note
        rel_note="$(m "(まだリリースがありません)" "(no release yet)")"
        if [ -n "$(release_asset_url latest "${arch:-x86_64}")" ]; then rel_note=""; fi
        local def=1
        [ "$internet" != 0 ] || def=2
        ask_choice v "$def" "wpg のバイナリの用意:" "How to get the wpg binary:" \
            "ホスト上でソースからビルド (推奨。ホストにインターネット接続が必要。初回 10〜30 分)|Build from source on the host (recommended; needs internet there; 10-30 min the first time)" \
            "このマシンでビルドしてアップロード|Build on this machine and upload" \
            "手元の Linux 用バイナリファイルをアップロード|Upload a Linux binary file you already have" \
            "GitHub Release からダウンロード $rel_note|Download from a GitHub Release $rel_note"
        case "$v" in 1) bin_choice="build" ;; 2) bin_choice="local" ;; 3) bin_choice="file" ;; 4) bin_choice="release" ;; esac
    fi
    case "$bin_choice" in
        build)
            build_on_host=1
            [ "$internet" != 0 ] || warn "ホストからインターネットに接続できないようです。ビルドは失敗する可能性があります。" "the host does not seem to reach the internet; the build will probably fail."
            if [ "$use_prev" -eq 0 ]; then
                if [ "$source_kind" = git ]; then def=2; else def=1; fi
                ask_choice v "$def" "ソースコード:" "Source code:" \
                    "このマシンのソース ($ROOT) をアップロード|Upload this checkout ($ROOT)" \
                    "GitHub から git clone|git clone from GitHub"
                if [ "$v" -eq 2 ]; then
                    source_kind=git
                    ask git_url "リポジトリ URL" "Repository URL" "$git_url"
                    ask git_ref "ブランチ / タグ" "Branch / tag" "$git_ref"
                else
                    source_kind=local
                fi
            fi
            ;;
        local) ;;
        file)
            v="$p_binary_file"
            while :; do
                ask_required v "バイナリファイルのパス" "Path of the binary" "$v"
                [ -f "$v" ] && break
                say "  ファイルがありません: $v" "  No such file: $v"
                v=""
            done
            binary="$v"
            p_binary_file="$v"
            ;;
        release)
            release_tag=latest
            [ -n "$(release_asset_url latest "${arch:-x86_64}")" ] \
                || die "$(m "GitHub Release はまだありません。ホスト上でビルドを選んでください。" "There is no GitHub Release yet; choose building on the host.")"
            ;;
    esac

    # 5. server settings
    if [ "$use_prev" -eq 0 ]; then
        ask_yn service "systemd サービスとしてインストールし、起動時に自動で開始しますか?" "Install as a systemd service (starts at boot)?" 1
        while :; do
            ask v "Web UI の待ち受けアドレス:ポート (0.0.0.0 = すべて)" "Web UI listen address:port (0.0.0.0 = all)" "$bind"
            case "$v" in *:*[0-9]) bind="$v"; break ;; esac
            say "  <アドレス>:<ポート> の形で入力してください (例 0.0.0.0:8080)" "  Use <address>:<port> (e.g. 0.0.0.0:8080)"
        done
    fi

    # 6. allowed interfaces
    if [ "$use_prev" -eq 0 ] || [ "${#ifaces[@]}" -eq 0 ]; then
        mgmt="$(probe MGMT_IFACE)"
        list=()
        while IFS= read -r v; do
            [ -n "$v" ] && list+=("$v")
        done < <(printf '%s\n' "$probe_out" | sed -n 's/^WPG_PROBE_IFACE=//p')
        if [ "${#list[@]}" -gt 0 ]; then
            say "送信を許可するインタフェース (tx.allowed_ifaces):" "Interfaces jobs may send on (tx.allowed_ifaces):" >&2
            def_list=()
            n=1
            for v in "${list[@]}"; do
                IFS='|' read -r name state addrs <<<"$v"
                if [ "$name" = "$mgmt" ]; then
                    printf '  %d) %-12s %-8s %s  %s\n' "$n" "$name" "$state" "$addrs" "$(m "<- 管理用 (SSH 接続中)" "<- management (this SSH session)")" >&2
                else
                    printf '  %d) %-12s %-8s %s\n' "$n" "$name" "$state" "$addrs" >&2
                    def_list+=("$n")
                fi
                n=$((n + 1))
            done
            local def_str
            def_str="$(IFS=,; echo "${def_list[*]}")"
            while :; do
                ask answer "番号をカンマ区切りで (例 2,3。all = すべて許可)" "Numbers, comma separated (e.g. 2,3; all = no restriction)" "${def_str:-all}"
                ifaces=()
                [ "$answer" = all ] && break
                local bad=0
                IFS=', ' read -r -a sel <<<"$answer"
                for i in "${sel[@]}"; do
                    case "$i" in '' ) continue ;; *[!0-9]*) bad=1; break ;; esac
                    if [ "$i" -lt 1 ] || [ "$i" -gt "${#list[@]}" ]; then bad=1; break; fi
                    ifaces+=("$(printf '%s' "${list[$((i - 1))]}" | cut -d'|' -f1)")
                done
                [ "$bad" -eq 0 ] && break
                say "  1 から ${#list[@]} の番号を入力してください。" "  Enter numbers from 1 to ${#list[@]}."
            done
            for i in "${ifaces[@]}"; do
                [ "$i" != "$mgmt" ] || warn "管理用インタフェース $i で送信を許可しました。SSH が切れる可能性があります。" "you allowed sending on the management interface $i; this may cut your SSH session."
            done
        else
            ask answer "送信を許可するインタフェース名 (スペース区切り。空 = すべて)" "Interfaces jobs may send on (space separated; empty = all)" "$(IFS=' '; echo "${ifaces[*]}")"
            read -r -a ifaces <<<"$answer"
        fi
    fi

    # 7. optional features
    if [ "$use_prev" -eq 0 ] && [ "$service" -eq 1 ]; then
        ask_yn terminal "Web ターミナルを有効にしますか? (対応する wpg のみ)" "Enable the web terminal? (newer wpg only)" "$terminal"
        ask_yn snmp_trap "SNMP トラップ受信 (UDP 162) を有効にしますか? (対応する wpg のみ)" "Enable the SNMP trap receiver (UDP 162)? (newer wpg only)" "$snmp_trap"
    fi

    # 8. admin password
    if [ "$service" -eq 1 ]; then
        local loop=0 have_pw=0
        case "${bind%:*}" in 127.* | localhost | "[::1]") loop=1 ;; esac
        [ -z "$(probe WPG_VERSION)" ] || [ "$(probe SERVICE)" = absent ] || have_pw=1
        while :; do
            if [ "$have_pw" -eq 1 ] || [ "$loop" -eq 1 ]; then
                ask_secret admin_pw "wpg の管理者パスワード (Enter = 変更しない / 設定しない)" "wpg admin password (Enter = keep / none)"
                [ -n "$admin_pw" ] || break
            else
                ask_secret admin_pw "wpg の管理者パスワード (Web UI のログイン用)" "wpg admin password (Web UI login)"
                if [ -z "$admin_pw" ]; then say "  入力してください。" "  Please enter a password."; continue; fi
            fi
            ask_secret v "もう一度入力してください" "Repeat it"
            [ "$v" = "$admin_pw" ] && break
            say "  一致しません。" "  The passwords do not match."
        done
    fi

    # 9. summary
    printf '\n%s%s%s\n' "$c_step" "$(m "確認" "Summary")" "$c_off"
    printf '  %-22s %s\n' "$(m "接続先" "Host")" "$host (port ${ssh_port:-22})"
    printf '  %-22s %s\n' "$(m "バイナリ" "Binary")" "$(summary_binary)"
    printf '  %-22s %s\n' "$(m "サービス" "Service")" "$( [ "$service" -eq 1 ] && m "systemd (wpg.service)" "systemd (wpg.service)" || m "なし" "no")"
    printf '  %-22s %s\n' "$(m "待ち受け" "Listen")" "$bind"
    printf '  %-22s %s\n' "$(m "送信インタフェース" "TX interfaces")" "$( [ "${#ifaces[@]}" -gt 0 ] && (IFS=,; echo "${ifaces[*]}") || m "すべて (制限なし)" "all (no restriction)")"
    printf '  %-22s %s\n' "$(m "管理者パスワード" "Admin password")" "$( [ -n "$admin_pw" ] && m "設定する" "set" || m "変更しない" "unchanged")"
    printf '  %-22s %s\n' "$(m "Web ターミナル" "Web terminal")" "$( [ "$terminal" -eq 1 ] && m "有効" "on" || m "無効" "off")"
    printf '  %-22s %s\n' "$(m "SNMP トラップ受信" "SNMP traps")" "$( [ "$snmp_trap" -eq 1 ] && m "有効 (UDP 162)" "on (UDP 162)" || m "無効" "off")"
    ask_yn answer "この内容で実行しますか?" "Go ahead?" 1
    [ "$answer" -eq 1 ] || { say "中止しました。" "Aborted."; exit 0; }
    ask_yn answer "この設定を次回のために保存しますか? (パスワードは保存しません)" "Save these answers for next time? (no passwords)" 1
    [ "$answer" -eq 0 ] || save_profile
}

# ---- main -------------------------------------------------------------------------
if [ "$interactive" -eq 1 ]; then
    [ -z "$log_file" ] && log_file="${XDG_STATE_HOME:-$HOME/.local/state}/wpg/deploy-$(date +%Y%m%d-%H%M%S).log"
    wizard
else
    [ -n "$host" ] || { usage >&2; exit 2; }
    setup_ssh_args
fi

if [ -n "$log_file" ]; then
    mkdir -p "$(dirname "$log_file")"
    exec > >(tee -a "$log_file") 2>&1
    say "ログ: $log_file" "log: $log_file"
fi

case "$hugepages" in '' | *[!0-9]*) die "--hugepages must be a number" ;; esac
case "$bind" in *:*) ;; *) die "--bind must be <addr>:<port>" ;; esac
port="${bind##*:}"
# Address the health check connects to (on the host itself).
hc_addr="${bind%:*}"
case "$hc_addr" in 0.0.0.0 | "" | "[::]") hc_addr=127.0.0.1 ;; esac
case "$port" in '' | *[!0-9]*) die "--bind must end in a port number" ;; esac
for i in "${ifaces[@]}"; do
    case "$i" in '' | *[!A-Za-z0-9_.:@-]*) die "invalid interface name: $i" ;; esac
done
[ -z "$password_file" ] || [ -r "$password_file" ] || die "cannot read $password_file"
[ "$build_on_host" -eq 0 ] || [ -z "$binary$release_tag" ] || die "--build-on-host cannot be combined with --binary / --release-tag"

if [ "$interactive" -eq 0 ]; then
    if [ "$ask_password" -eq 1 ] || [ -n "$identity" ]; then
        ssh_opts+=(-o StrictHostKeyChecking=accept-new)
        setup_ssh_args
        step connect "$host に接続" "Connecting to $host"
        setup_key
    fi
    if [ "$ask_sudo" -eq 1 ]; then
        sudo_mode=stdin
        ask_secret sudo_pw "sudo パスワード" "sudo password"
        current_step=sudo
        check_sudo || die "$(m "sudo に失敗しました" "sudo failed")"
        current_step=""
    fi
fi

step_total=$((step_no + 4 + build_on_host))
[ "$rollback" -eq 0 ] || step_total=$((step_no + 3))
if [ "$healthcheck" -eq 0 ] || { [ "$service" -eq 0 ] && [ "$rollback_health" -eq 0 ]; }; then step_total=$((step_total - 1)); fi

# ---- binary -----------------------------------------------------------------
if [ "$rollback" -eq 0 ] && [ "$build_on_host" -eq 0 ]; then
    step binary "バイナリの用意" "Preparing the binary"
    if [ -n "$release_tag" ]; then
        arch="$(probe ARCH)"
        url="$(release_asset_url "$release_tag" "${arch:-x86_64}")"
        [ -n "$url" ] || die "$(m "GitHub Release $release_tag に Linux ($arch) 用のアーカイブがありません (リリースはまだありません。--build-on-host を使ってください)" "no Linux ($arch) archive in the GitHub Release $release_tag (there are no releases yet: use --build-on-host)")"
        tmp_dir="$(mktemp -d)"
        info "downloading $url"
        run curl -fSL --progress-bar -o "$tmp_dir/wpg.tar.gz" "$url"
        if [ "$dry_run" -eq 0 ]; then
            if curl -fsSL -o "$tmp_dir/wpg.tar.gz.sha256" "$url.sha256" 2>/dev/null; then
                want="$(awk '{print $1}' "$tmp_dir/wpg.tar.gz.sha256")"
                got="$( (sha256sum "$tmp_dir/wpg.tar.gz" 2>/dev/null || shasum -a 256 "$tmp_dir/wpg.tar.gz") | awk '{print $1}')"
                [ "$want" = "$got" ] || die "checksum mismatch for $url"
                info "sha256 ok"
            fi
            tar -xzf "$tmp_dir/wpg.tar.gz" -C "$tmp_dir"
            binary="$(find "$tmp_dir" -mindepth 2 -maxdepth 2 -type f -name wpg | head -n 1)"
        else
            binary="$tmp_dir/wpg-<version>/wpg"
        fi
    fi
    if [ -z "$binary" ]; then
        if [ -n "$target" ]; then
            binary="target/$target/release/wpg"
        else
            binary="target/release/wpg"
        fi
        if [ ! -x "$binary" ] || [ "$interactive" -eq 1 ]; then
            info "building $binary"
            if [ -n "$target" ]; then
                # The front-end is embedded at build time, so build it first.
                run "$ROOT/scripts/build.sh" --release
                run cargo build --release --locked -p wpg-cli --target "$target"
            else
                run "$ROOT/scripts/build.sh" --release
            fi
        fi
    fi
    if [ "$dry_run" -eq 0 ]; then
        [ -f "$binary" ] || die "binary not found: $binary"
        if command -v file >/dev/null 2>&1 && ! file -b "$binary" | grep -q '^ELF'; then
            die "$binary is not a Linux (ELF) executable; build for the host's target with --target"
        fi
    fi
    info "binary: $binary"
fi

# ---- upload -----------------------------------------------------------------
stage="/tmp/wpg-deploy.$$"
step upload "アップロード ($host:$stage)" "Uploading to $host:$stage"
run ssh "${ssh_args[@]}" "$host" "mkdir -p '$stage' && chmod 0700 '$stage'" </dev/null
upload=("$ROOT/scripts/lib/deploy-remote.sh" "$ROOT/scripts/lib/remote-health.sh" "$ROOT/packaging/systemd/wpg.service")
[ "$rollback" -eq 1 ] || [ "$build_on_host" -eq 1 ] || upload+=("$binary")
[ "$build_on_host" -eq 0 ] || upload+=("$ROOT/scripts/lib/remote-build.sh")
run scp -q "${scp_args[@]}" "${upload[@]}" "$host:$stage/"
[ -z "$password_file" ] || run scp -q "${scp_args[@]}" "$password_file" "$host:$stage/password"
if [ -n "$admin_pw" ]; then
    # Through stdin: the password never touches the local disk or a command line.
    if [ "$dry_run" -eq 1 ]; then
        echo "+ ssh ${ssh_args[*]} $host 'umask 077; cat >$stage/password'   # admin password on stdin"
    else
        printf '%s\n' "$admin_pw" | ssh "${ssh_args[@]}" "$host" "umask 077; cat >'$stage/password'"
    fi
    password_file="(wizard)"
fi
if [ "$templates" -eq 1 ] && [ "$rollback" -eq 0 ]; then
    run scp -q "${scp_args[@]}" -r "$ROOT/templates" "$host:$stage/templates"
fi
if [ "$build_on_host" -eq 1 ] && [ "$source_kind" = local ]; then
    tmp_dir="${tmp_dir:-$(mktemp -d)}"
    info "packing the source of $ROOT"
    if git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1; then
        # Includes uncommitted changes of tracked files (git stash create), not untracked files.
        rev="$(git -C "$ROOT" stash create 2>/dev/null || true)"
        run git -C "$ROOT" -c core.autocrlf=false archive --format=tar.gz -o "$tmp_dir/src.tar.gz" "${rev:-HEAD}"
    else
        run tar -C "$ROOT" --exclude=./target --exclude=./.git --exclude='./crates/wpg-web/dist' --exclude='*/node_modules' -czf "$tmp_dir/src.tar.gz" .
    fi
    run scp -q "${scp_args[@]}" "$tmp_dir/src.tar.gz" "$host:$stage/src.tar.gz"
fi

# ---- build on the host --------------------------------------------------------
if [ "$build_on_host" -eq 1 ] && [ "$rollback" -eq 0 ]; then
    step deps "ビルドに必要なパッケージの導入 (apt)" "Installing the build dependencies (apt)"
    ssh_root "bash '$stage/remote-build.sh' --deps"
    step build "ホスト上でビルド (初回は 10〜30 分かかります)" "Building on the host (10-30 minutes the first time)"
    build_args=(--build --out "$stage/wpg")
    if [ "$source_kind" = local ]; then
        build_args+=(--tarball "$stage/src.tar.gz")
    else
        build_args+=(--git-url "$git_url" --git-ref "$git_ref")
    fi
    [ "$dpdk" -eq 0 ] || build_args+=(--features dpdk)
    quoted=""
    for a in "${build_args[@]}"; do quoted+=" $(printf '%q' "$a")"; done
    run ssh "${ssh_args[@]}" "$host" "bash '$stage/remote-build.sh'$quoted" </dev/null
    binary="$stage/wpg"
fi

# ---- install ----------------------------------------------------------------
remote_args=(--stage "$stage" --prefix "$prefix" --bind "$bind" --hugepages "$hugepages")
if [ "$rollback" -eq 0 ]; then
    if [ "$build_on_host" -eq 1 ]; then
        remote_args+=(--binary "$stage/wpg")
    else
        remote_args+=(--binary "$stage/$(basename "$binary")")
    fi
fi
[ "$service" -eq 1 ] && remote_args+=(--service)
[ "$rollback" -eq 1 ] && remote_args+=(--rollback)
[ "$dpdk" -eq 1 ] && remote_args+=(--dpdk)
[ "$allow_insecure" -eq 1 ] && remote_args+=(--allow-insecure)
[ "$terminal" -eq 1 ] && remote_args+=(--terminal)
[ "$snmp_trap" -eq 1 ] && remote_args+=(--snmp-trap)
[ "$templates" -eq 1 ] && [ "$rollback" -eq 0 ] && remote_args+=(--templates "$stage/templates")
[ -z "$password_file" ] || remote_args+=(--password-file "$stage/password")
for i in "${ifaces[@]}"; do remote_args+=(--iface "$i"); done

quoted=""
for a in "${remote_args[@]}"; do quoted+=" $(printf '%q' "$a")"; done
step install "インストール (sudo)" "Installing (sudo)"
do_health=0
if [ "$healthcheck" -eq 1 ] && { [ "$service" -eq 1 ] || [ "$rollback_health" -eq 1 ]; }; then do_health=1; fi
if [ "$do_health" -eq 1 ]; then
    # The staging directory is removed after the health check.
    ssh_root "bash '$stage/deploy-remote.sh'$quoted" "; rc=\$?; [ \$rc -eq 0 ] || rm -rf '$stage'; exit \$rc"
else
    ssh_root "bash '$stage/deploy-remote.sh'$quoted" "; rc=\$?; rm -rf '$stage'; exit \$rc"
fi

# ---- healthcheck ------------------------------------------------------------
if [ "$do_health" -eq 1 ]; then
    step health "ヘルスチェック (http://$hc_addr:$port/api/health)" "Health check (http://$hc_addr:$port/api/health)"
    run ssh "${ssh_args[@]}" "$host" "bash '$stage/remote-health.sh' '$hc_addr' '$port' 30; rc=\$?; rm -rf '$stage'; exit \$rc" </dev/null
    current_step=""
    h="${host#*@}"
    case "${bind%:*}" in
        127.* | localhost | "[::1]")
            printf '\n%s%s%s\n' "$c_ok" "$(m "完了しました。wpg はホストのループバックで待ち受けています。SSH トンネル経由で開いてください:" "Done. wpg listens on the host's loopback; open it through an SSH tunnel:")" "$c_off"
            echo "  ssh -L $port:127.0.0.1:$port ${ssh_port:+-p $ssh_port }$host   ->  http://127.0.0.1:$port/"
            ;;
        *)
            printf '\n%s%s%s\n' "$c_ok" "$(m "完了しました。ブラウザで開いてください:" "Done. Open in a browser:")" "$c_off"
            echo "  http://$h:$port/"
            ;;
    esac
    info "deployed: http://$h:$port/"
elif [ "$rollback" -eq 1 ]; then
    current_step=""
    info "rolled back (wpg.service was restarted if it is enabled)"
else
    current_step=""
    info "done (no service started; run 'wpg serve' on $host or pass --service)"
fi
