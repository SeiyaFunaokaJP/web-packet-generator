#!/usr/bin/env bash
# Deploy a wpg build to a Linux host over SSH (spec §2.8; first target: Ubuntu on CML2).
#
# Usage: scripts/deploy.sh                      (no arguments: interactive wizard)
#        scripts/deploy.sh <user@host> [options]
#        scripts/deploy.sh --config <file> --yes [options]     (unattended)
#   --interactive           ask everything step by step (also the default without arguments)
#   --iface <name>          interface jobs may send on (repeatable; tx.allowed_ifaces)
#   --service               install / update the systemd unit and (re)start it
#   --no-service            do not (overrides service=1 of --config)
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
#   --lang <ja|en>          language of the messages (default: from LANG)
#   --log <file>            also write the output to this file (wizard and unattended
#                           runs: always, under ~/.local/state/wpg/)
#   --dry-run               print the commands instead of running them (unattended: still
#                           connects to read the host facts, changes nothing)
#   -h, --help              this help
#
# Unattended / automation (docs/*/src/operations/deploy.md, "Unattended deploy"):
#   --config <file>         answer file: key=value lines in the format of the wizard's
#                           deploy.conf; [host <name>] sections deploy to several hosts
#   --targets <file>        several hosts: one "[user@]host[:port] [key=value ...]" per line
#   --parallel <n>          several hosts: deploy to up to <n> hosts at a time (default 1)
#   --section <name>        only the [host <name>] section of --config
#   --set <key=value>       override one answer-file key (repeatable)
#   --yes                   never prompt. Secrets come from an SSH key, the environment
#                           (WPG_DEPLOY_SSH_PASSWORD, WPG_DEPLOY_SUDO_PASSWORD,
#                           WPG_DEPLOY_ADMIN_PASSWORD) or *_password_file keys; a missing
#                           one fails at once
#   --json                  JSON summary on stdout (all other output goes to stderr)
#   --skip-if-current       do nothing when the host already runs this source (git
#                           commit / binary checksum) with the same settings
#   --check-config          check the answer file / targets, print the settings, no SSH
#
# Exit codes: 0 done (also: skipped, dry run), 1 usage, answer file, missing setting or
# admin password, 2 connection / SSH / sudo (also a missing SSH or sudo password),
# 3 preparing or building the binary, 4 upload or installation, 5 health check,
# 6 several hosts and at least one failed, 130 interrupted.
#
# The remote user needs sudo. Re-running the script upgrades in place: the previous
# binary is kept as <prefix>/bin/wpg.prev, /etc/wpg/wpg.toml is never overwritten.
# ---
# SC2029: remote commands are deliberately built on this side. SC2015: `a && b || c`
# is only used where b cannot fail.
# shellcheck disable=SC2029,SC2015
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SELF="$ROOT/scripts/deploy.sh"
orig_pwd="$PWD"
cd "$ROOT"

usage() { sed -n '2,/^# ---$/p' "$SELF" | sed '$d' | sed 's/^# \{0,1\}//'; }
fail_msg=""
die() { fail_msg="$*"; echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    c_step=$'\033[1;36m' c_ok=$'\033[1;32m' c_warn=$'\033[1;33m' c_err=$'\033[1;31m' c_off=$'\033[0m'
else
    c_step="" c_ok="" c_warn="" c_err="" c_off=""
fi

host=""
ifaces=()
service=""
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
lang_cli=0
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
# unattended mode
config_file=""
section=""
targets_file=""
parallel=1
sets=()
yes=0
json=0
skip_if_current=0
check_config=0
result_file=""
managed=0
batch_mode=no
label=""
ssh_password_file=""
sudo_password_file=""
dpapi_keys=()
cli_bin=""
cli_iface=0
result_status="ok"
version_before=""
version_after=""
source_id=""
deployed_url=""
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SECONDS=0

need_value() { [ "$2" -ge 2 ] || die "$1 needs a value"; }

# abs_path <path> <base dir>: ~ and relative paths resolved (empty stays empty).
abs_path() {
    # shellcheck disable=SC2088 # a literal ~/ prefix
    case "$1" in
        '') ;;
        '~') printf '%s' "$HOME" ;;
        '~/'*) printf '%s/%s' "$HOME" "${1#'~/'}" ;;
        /*) printf '%s' "$1" ;;
        *) printf '%s/%s' "$2" "$1" ;;
    esac
}

trim() { # <var>: strips leading and trailing white space in place
    local __t="${!1}"
    __t="${__t#"${__t%%[![:space:]]*}"}"
    __t="${__t%"${__t##*[![:space:]]}"}"
    printf -v "$1" '%s' "$__t"
}

# ---- options: pre-scan (answer file, language), then the answer file, then the rest ----
orig_args=("$@")
[ "$#" -gt 0 ] || interactive=1
prev=""
for a in "$@"; do
    case "$prev" in
        --config) config_file="$(abs_path "$a" "$orig_pwd")" ;;
        --section) section="$a" ;;
        --set) sets+=("$a") ;;
        --lang) lang="$a"; lang_cli=1 ;;
        --result-file) result_file="$a" ;;
    esac
    case "$a" in --interactive) interactive=1 ;; --json) json=1 ;; esac
    prev="$a"
done
if [ -z "$lang" ]; then
    case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in ja*) lang=ja ;; *) lang=en ;; esac
fi

# m <japanese> <english>: the message in the wizard language.
m() { if [ "$lang" = ja ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '%s\n' "$(m "$1" "$2")"; }
warn() { printf '%s%s%s\n' "$c_warn" "$(m "警告: $1" "warning: $2")" "$c_off" >&2; }
# An error in the answer file / settings (exit code 1).
cfg_err() { current_step=config; die "$*"; }

# Can a question be asked? Never with --yes; outside the wizard only on a terminal.
can_prompt() { [ "$yes" -eq 0 ] && { [ "$interactive" -eq 1 ] || [ -t 0 ]; }; }

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

# Exit code of a failed step (documented in the help and in deploy.md).
step_exit_code() {
    case "$1" in
        connect | sudo) echo 2 ;;
        binary | deps | build) echo 3 ;;
        upload | install) echo 4 ;;
        health) echo 5 ;;
        *) echo 1 ;;
    esac
}

# ---- machine-readable result ----------------------------------------------------------
json_str() {
    local s="$1"
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    s="${s//$'\n'/\\n}"
    s="${s//$'\r'/}"
    s="${s//$'\t'/ }"
    s="$(printf '%s' "$s" | tr -d '\000-\010\013\014\016-\037')"
    printf '"%s"' "$s"
}

host_json() { # <exit code>
    local u="" h="$host" dr=false
    case "$host" in *@*) u="${host%@*}"; h="${host#*@}" ;; esac
    [ "$dry_run" -eq 0 ] || dr=true
    printf '{"name":%s,"host":%s,"user":%s,"port":%s,"status":%s,"exit_code":%d,"failed_step":%s,"message":%s,"version_before":%s,"version_after":%s,"source_id":%s,"url":%s,"dry_run":%s,"log":%s,"started_at":%s,"duration_s":%d}' \
        "$(json_str "${label:-$h}")" "$(json_str "$h")" "$(json_str "$u")" "${ssh_port:-22}" "$(json_str "$result_status")" "$1" \
        "$(json_str "$failed_step")" "$(json_str "$fail_msg")" "$(json_str "$version_before")" "$(json_str "$version_after")" \
        "$(json_str "$source_id")" "$(json_str "$deployed_url")" "$dr" "$(json_str "$log_file")" "$(json_str "$started_at")" "$SECONDS"
}

# One row per host for the summary table, fields separated by US (0x1f): unlike a tab
# it is not IFS white space, so `read` keeps empty fields.
US=$'\037'
tsv() { local t="${1//$US/ }"; t="${t//$'\r'/}"; printf '%s' "${t//$'\n'/ }"; }

failed_step=""
on_exit() {
    local rc=$? dr=false objs
    cleanup
    if [ "$rc" -ne 0 ]; then
        result_status=failed
        failed_step="$current_step"
        case "$rc" in 129 | 130 | 131 | 143) ;; *) rc="$(step_exit_code "$current_step")" ;; esac
        [ -n "$fail_msg" ] || fail_msg="$(m "コマンドが失敗しました" "a command failed")"
        if [ -n "$current_step" ] && [ "$current_step" != config ]; then
            printf '\n%s%s%s\n' "$c_err" "$(m "失敗しました (ステップ $step_no: $current_step, 終了コード $rc)" "FAILED (step $step_no: $current_step, exit code $rc)")" "$c_off" >&2
            hint "$current_step" >&2
            if [ -n "$log_file" ] && [ -f "$log_file" ]; then
                say "ログ: $log_file" "log: $log_file" >&2
            fi
        fi
    fi
    if [ -n "$result_file" ]; then
        host_json "$rc" >"$result_file" 2>/dev/null || true
        # A plain table row for the summary of a multi-host run.
        printf '%s\n' "$(tsv "${label:-${host#*@}}")$US$(tsv "$host")$US$result_status$US$rc$US$(tsv "${version_after:-$version_before}")$US$(tsv "${fail_msg:-$source_id}")" \
            >"$result_file.row" 2>/dev/null || true
    fi
    if [ "$json" -eq 1 ] && [ -z "$result_file" ]; then
        [ "$dry_run" -eq 0 ] || dr=true
        objs=""
        [ "$multi" -eq 1 ] || objs="$(host_json "$rc")"
        printf '{"tool":"wpg-deploy","format":1,"status":%s,"exit_code":%d,"dry_run":%s,"message":%s,"hosts":[%s]}\n' \
            "$(json_str "$( [ "$rc" -eq 0 ] && echo ok || echo failed)")" "$rc" "$dr" "$(json_str "$fail_msg")" "$objs" >&3 || true
    fi
    exit "$rc"
}
multi=0
trap on_exit EXIT
# --json: the summary goes to the original stdout (fd 3), everything else to stderr.
if [ "$json" -eq 1 ]; then
    exec 3>&1 1>&2
fi

truthy() { # prints 1 or 0, fails on anything else
    case "$1" in
        1 | y | Y | yes | Yes | YES | true | True | TRUE | on | ON) echo 1 ;;
        0 | n | N | no | No | NO | false | False | FALSE | off | OFF | '') echo 0 ;;
        *) return 1 ;;
    esac
}

# ---- answer file ------------------------------------------------------------------
# The wizard's deploy.conf and the --config file share one format: key=value lines,
# '#' comments, and optional [host <name>] sections with per-host values.
p_host="" p_user="" p_bin="" p_binary_file="" p_release_tag=""
apply_kv() { # <key> <value> <base dir> <where>
    local k="$1" v="$2" base="$3" where="$4" b
    case "$k" in
        host)
            case "$v" in
                *@*) p_user="${v%@*}"; p_host="${v#*@}" ;;
                *) p_host="$v" ;;
            esac
            ;;
        user) p_user="$v" ;;
        ssh_port | port)
            case "$v" in '') ;; *[!0-9]*) cfg_err "$where: $k must be a number: $v" ;; esac
            ssh_port="$v"
            ;;
        identity) identity="$(abs_path "$v" "$base")" ;;
        binary_source)
            case "$v" in build | local | file | release | '') p_bin="$v" ;; *) cfg_err "$where: binary_source must be build, local, file or release: $v" ;; esac
            ;;
        binary_file) p_binary_file="$(abs_path "$v" "$base")" ;;
        source)
            case "$v" in local | git) source_kind="$v" ;; '') ;; *) cfg_err "$where: source must be local or git: $v" ;; esac
            ;;
        git_url) [ -z "$v" ] || git_url="$v" ;;
        git_ref) [ -z "$v" ] || git_ref="$v" ;;
        release_tag) p_release_tag="$v" ;;
        prefix) [ -z "$v" ] || prefix="$v" ;;
        bind) [ -z "$v" ] || bind="$v" ;;
        ifaces) ifaces=(); IFS=', ' read -r -a ifaces <<<"$v" ;;
        service | terminal | snmp_trap | templates | healthcheck | allow_insecure | skip_if_current | dpdk)
            b="$(truthy "$v")" || cfg_err "$where: $k must be 1 or 0: $v"
            printf -v "$k" '%s' "$b"
            ;;
        hugepages) hugepages="$v" ;;
        admin_password_file) password_file="$(abs_path "$v" "$base")" ;;
        ssh_password_file) ssh_password_file="$(abs_path "$v" "$base")" ;;
        sudo_password_file) sudo_password_file="$(abs_path "$v" "$base")" ;;
        ssh_password_dpapi | sudo_password_dpapi | admin_password_dpapi) [ -z "$v" ] || dpapi_keys+=("$k") ;;
        name) label="$v" ;;
        parallel) parallel="$v" ;;
        lang) [ "$lang_cli" -eq 1 ] || [ -z "$v" ] || lang="$v" ;;
        *) cfg_err "$where: unknown key '$k'" ;;
    esac
}

# load_answers <file> [section]: the global keys, then those of [host <section>].
load_answers() {
    local file="$1" want="${2:-}" base line k v cur="" n=0 found=0 sec_host=0
    [ -f "$file" ] && [ -r "$file" ] || cfg_err "$(m "設定ファイルを読めません" "cannot read the answer file"): $file"
    base="$(cd "$(dirname "$file")" && pwd)"
    while IFS= read -r line || [ -n "$line" ]; do
        n=$((n + 1))
        line="${line%$'\r'}"
        trim line
        case "$line" in '' | '#'* | ';'*) continue ;; esac
        if [[ "$line" == \[*\] ]]; then
            cur="${line#[}"
            cur="${cur%]}"
            trim cur
            case "$cur" in host[[:space:]]*) cur="${cur#host}"; trim cur ;; esac
            [ -n "$cur" ] || cfg_err "$file:$n: empty section name"
            [ "$cur" != "$want" ] || found=1
            continue
        fi
        case "$line" in *=*) ;; *) cfg_err "$file:$n: expected key=value: $line" ;; esac
        k="${line%%=*}"
        v="${line#*=}"
        trim k
        trim v
        case "$v" in \"*\") v="${v#\"}"; v="${v%\"}" ;; esac
        if [ -z "$cur" ] || { [ -n "$want" ] && [ "$cur" = "$want" ]; }; then
            apply_kv "$k" "$v" "$base" "$file:$n"
            [ -z "$cur" ] || [ "$k" != host ] || sec_host=1
        elif [ -n "$cur" ]; then
            # Other sections: only check the key names.
            case "$k" in host | user | ssh_port | port | identity | binary_source | binary_file | source | git_url | git_ref | release_tag | prefix | bind | ifaces | service | terminal | snmp_trap | templates | healthcheck | allow_insecure | skip_if_current | dpdk | hugepages | admin_password_file | ssh_password_file | sudo_password_file | ssh_password_dpapi | sudo_password_dpapi | admin_password_dpapi | name | parallel | lang) ;;
                *) cfg_err "$file:$n: unknown key '$k'" ;;
            esac
        fi
    done <"$file"
    if [ -n "$want" ]; then
        [ "$found" -eq 1 ] || cfg_err "$(m "セクション [host $want] がありません" "no section [host $want] in") $file"
        [ "$sec_host" -eq 1 ] || p_host="$want"
        [ -n "$label" ] || label="$want"
    fi
}

answer_sections() { # <file>: the section names, one per line
    local line cur
    while IFS= read -r line || [ -n "$line" ]; do
        line="${line%$'\r'}"
        trim line
        if [[ "$line" == \[*\] ]]; then
            cur="${line#[}"
            cur="${cur%]}"
            trim cur
            case "$cur" in host[[:space:]]*) cur="${cur#host}"; trim cur ;; esac
            printf '%s\n' "$cur"
        fi
    done <"$1"
}

if [ "$interactive" -eq 0 ] && [ -n "$config_file" ]; then
    load_answers "$config_file" "$section"
fi
for s in "${sets[@]}"; do
    case "$s" in *=*) ;; *) cfg_err "--set needs key=value: $s" ;; esac
    k="${s%%=*}"
    trim k
    apply_kv "$k" "${s#*=}" "$orig_pwd" "--set"
done

while [ "$#" -gt 0 ]; do
    case "$1" in
        --interactive) interactive=1 ;;
        --iface)
            need_value "$1" "$#"
            if [ "$cli_iface" -eq 0 ]; then ifaces=(); cli_iface=1; fi
            ifaces+=("$2"); shift
            ;;
        --service) service=1 ;;
        --no-service) service=0 ;;
        --bind) need_value "$1" "$#"; bind="$2"; shift ;;
        --password-file) need_value "$1" "$#"; password_file="$(abs_path "$2" "$orig_pwd")"; shift ;;
        --allow-insecure) allow_insecure=1 ;;
        --binary) need_value "$1" "$#"; binary="$(abs_path "$2" "$orig_pwd")"; cli_bin="file"; shift ;;
        --target) need_value "$1" "$#"; target="$2"; shift ;;
        --build-on-host) build_on_host=1; cli_bin=build ;;
        --source) need_value "$1" "$#"; source_kind="$2"; shift ;;
        --git-url) need_value "$1" "$#"; git_url="$2"; shift ;;
        --git-ref) need_value "$1" "$#"; git_ref="$2"; shift ;;
        --release-tag) need_value "$1" "$#"; release_tag="$2"; cli_bin=release; shift ;;
        --prefix) need_value "$1" "$#"; prefix="$2"; shift ;;
        --port) need_value "$1" "$#"; ssh_port="$2"; shift ;;
        --identity) need_value "$1" "$#"; identity="$(abs_path "$2" "$orig_pwd")"; shift ;;
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
        --profile) need_value "$1" "$#"; profile="$(abs_path "$2" "$orig_pwd")"; shift ;;
        --lang) need_value "$1" "$#"; shift ;;
        --log) need_value "$1" "$#"; log_file="$(abs_path "$2" "$orig_pwd")"; shift ;;
        --dry-run) dry_run=1 ;;
        --config | --section | --set) need_value "$1" "$#"; shift ;;
        --targets) need_value "$1" "$#"; targets_file="$(abs_path "$2" "$orig_pwd")"; shift ;;
        --parallel) need_value "$1" "$#"; parallel="$2"; shift ;;
        --yes) yes=1 ;;
        --json) json=1 ;;
        --skip-if-current) skip_if_current=1 ;;
        --check-config) check_config=1 ;;
        --result-file) need_value "$1" "$#"; result_file="$2"; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) cfg_err "unknown option: $1 (see --help)" ;;
        *)
            [ -z "$host" ] || cfg_err "unexpected argument: $1"
            host="$1"
            ;;
    esac
    shift
done

case "$lang" in ja | en) ;; *) cfg_err "--lang must be ja or en" ;; esac
case "$source_kind" in local | git) ;; *) cfg_err "--source must be local or git" ;; esac
case "$parallel" in '' | *[!0-9]* | 0) cfg_err "--parallel must be a positive number" ;; esac
# An answer file means the wizard's defaults (a service, built on the host).
if [ -z "$service" ]; then
    if [ -n "$config_file" ]; then service=1; else service=0; fi
fi
if [ "$interactive" -eq 0 ]; then
    if [ -n "$config_file" ] || [ "$yes" -eq 1 ] || [ "$json" -eq 1 ] || [ "$skip_if_current" -eq 1 ] \
        || [ "$check_config" -eq 1 ] || [ -n "$result_file" ] || [ -n "$targets_file" ]; then
        managed=1
    fi
elif [ -n "$config_file" ]; then
    profile="$config_file"
fi


# ---- prompts (wizard) -------------------------------------------------------
# All prompts read stdin and write to stderr, so answers can also be piped in.
prompt_line() { # <var> <label>
    local __ans
    can_prompt || die "$(m "質問できません (--yes または端末ではない標準入力): $2" "cannot ask (--yes, or stdin is not a terminal): $2")"
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
    can_prompt || die "$(m "パスワードを聞けません (--yes または端末ではない標準入力): $2" "cannot ask for a password (--yes, or stdin is not a terminal): $3")"
    printf '%s: ' "$(m "$2" "$3")" >&2
    if [ -t 0 ]; then
        IFS= read -r -s __s || die "$(m "入力が終わりました" "end of input")"
        printf '\n' >&2
    else
        IFS= read -r __s || die "$(m "入力が終わりました" "end of input")"
    fi
    printf -v "$1" '%s' "$__s"
}

# obtain_secret <var> <ENV_NAME> <file> <ja prompt> <en prompt>: the environment
# variable, else the first line of <file>, else a prompt when one is possible.
# Returns 1 (and leaves <var> alone) when none of them is available.
obtain_secret() {
    local __v=""
    if [ -n "${!2:-}" ]; then
        __v="${!2}"
    elif [ -n "$3" ]; then
        [ -r "$3" ] || cfg_err "$(m "パスワードファイルを読めません" "cannot read the password file"): $3"
        IFS= read -r __v <"$3" || true
        __v="${__v%$'\r'}"
        [ -n "$__v" ] || cfg_err "$(m "パスワードファイルが空です" "the password file is empty"): $3"
    elif can_prompt; then
        ask_secret __v "$4" "$5"
    else
        return 1
    fi
    printf -v "$1" '%s' "$__v"
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

# ---- profile / answer file (no secrets) ---------------------------------------
write_answers() { # <file> [automation]
    local dir
    dir="$(dirname "$1")"
    mkdir -p "$dir"
    (
        umask 077
        {
            echo "# wpg deploy answers (scripts/deploy.sh / deploy.ps1). No passwords are stored."
            if [ "${2:-}" = automation ]; then
                echo "# Unattended: scripts/deploy.sh --config $1 --yes [--skip-if-current] [--json]"
                echo "# Secrets: an SSH key (the wizard installed ~/.ssh/wpg_deploy_ed25519), or the"
                echo "# environment variables WPG_DEPLOY_SSH_PASSWORD / WPG_DEPLOY_SUDO_PASSWORD /"
                echo "# WPG_DEPLOY_ADMIN_PASSWORD, or files you create (mode 0600) named here:"
                echo "#   sudo_password_file=/path/to/file    admin_password_file=/path/to/file"
                echo "# Several hosts: add [host <name>] sections with host=, user=, ifaces=... below."
                echo "# Reference: docs/en/src/operations/deploy.md, \"Unattended deploy\"."
            fi
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
        } >"$1"
    )
}

save_profile() {
    write_answers "$profile"
    say "設定を保存しました: $profile (パスワードは保存していません)" "Saved the answers to $profile (no passwords)."
}

# ---- SSH ----------------------------------------------------------------------
ssh_args=()
scp_args=()
setup_ssh_args() {
    ssh_args=(-o "BatchMode=$batch_mode" "${ssh_opts[@]}")
    scp_args=(-o "BatchMode=$batch_mode" "${ssh_opts[@]}")
    if [ -n "$ssh_port" ]; then
        ssh_args+=(-p "$ssh_port")
        scp_args+=(-P "$ssh_port")
    fi
}

use_identity() {
    ssh_opts+=(-o "IdentityFile=$1" -o IdentitiesOnly=yes)
    setup_ssh_args
}

ssh_last_err=""
ssh_batch_ok() { # [extra ssh args...]: can we log in without a password?
    # ssh keeps the first value of an option, so these go before ssh_args (BatchMode=no).
    ssh_last_err="$(ssh -o BatchMode=yes -o ConnectTimeout=10 "$@" "${ssh_args[@]}" "$host" true </dev/null 2>&1 >/dev/null)"
}

# Fails at once when the host cannot be reached at all (rather than trying passwords).
check_reachable() {
    case "$ssh_last_err" in
        *"Connection refused"* | *"timed out"* | *"Could not resolve"* | *"No route to host"* | *"Network is unreachable"* \
            | *"Host key verification failed"* | *"REMOTE HOST IDENTIFICATION HAS CHANGED"* | *"Connection closed"* | *"Connection reset"*)
            die "$(m "$host (ポート ${ssh_port:-22}) に接続できません" "cannot connect to $host (port ${ssh_port:-22})"): $(printf '%s\n' "$ssh_last_err" | grep -v '^Warning' | tail -n 1)"
            ;;
    esac
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
        [ -f "$identity" ] || die "$(m "鍵ファイルがありません: $identity" "no such key file: $identity")"
        use_identity "$identity"
        ssh_batch_ok || { check_reachable; die "$(m "鍵 $identity でログインできません" "cannot log in with the key $identity")"; }
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
    check_reachable
    [ -n "$ssh_pw" ] || obtain_secret ssh_pw WPG_DEPLOY_SSH_PASSWORD "$ssh_password_file" "SSH パスワード ($host)" "SSH password ($host)" \
        || die "$(m "$host に鍵でログインできず、SSH パスワードもありません。環境変数 WPG_DEPLOY_SSH_PASSWORD か ssh_password_file= を設定するか、identity= で鍵を指定するか、一度ウィザードを実行してデプロイ用の鍵を登録してください" "cannot log in to $host with a key and no SSH password is given: set WPG_DEPLOY_SSH_PASSWORD or ssh_password_file=, name a key with identity=, or run the wizard once to install the deploy key")"
    if [ ! -f "$deploy_key" ]; then
        run mkdir -p "$(dirname "$deploy_key")"
        run ssh-keygen -q -t ed25519 -N "" -C "wpg-deploy@$(hostname 2>/dev/null || echo local)" -f "$deploy_key" </dev/null
        say "  デプロイ用の鍵を作りました: $deploy_key" "  Created the deploy key $deploy_key"
    fi
    say "  公開鍵をホストの ~/.ssh/authorized_keys に登録します (パスワードを使うのはこの 1 回だけです)。" \
        "  Installing the public key into ~/.ssh/authorized_keys on the host (the password is used this once)."
    # shellcheck disable=SC2016 # expanded on the host
    local install_cmd='umask 077; mkdir -p ~/.ssh && k=$(cat) && touch ~/.ssh/authorized_keys && { grep -qxF "$k" ~/.ssh/authorized_keys || printf "%s\n" "$k" >>~/.ssh/authorized_keys; }'
    # BatchMode=no first: ssh keeps the first value, and ssh_args may say BatchMode=yes.
    local pw_opts=(-o BatchMode=no -o "PreferredAuthentications=password,keyboard-interactive" -o PubkeyAuthentication=no -o NumberOfPasswordPrompts=1)
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
        can_prompt || die "$(m "公開鍵を登録できませんでした (SSH パスワードが違うか、sshpass も OpenSSH 8.4 以降もありません)" "could not install the public key (wrong SSH password, or neither sshpass nor OpenSSH 8.4+ is available)")"
        say "  自動入力できませんでした。ssh がパスワードを聞いたら、もう一度入力してください。" \
            "  Could not pass the password automatically; enter it again when ssh asks."
        ssh -o BatchMode=no "${ssh_args[@]}" -o PubkeyAuthentication=no "$host" "$install_cmd" <"$deploy_key.pub" \
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

check_sudo() { # [force]: verifies the sudo password (stdin mode; a dry run only with force)
    [ "$sudo_mode" = stdin ] || return 0
    [ "$dry_run" -eq 0 ] || [ "${1:-}" = force ] || return 0
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
    probe_out="$(ssh "${ssh_args[@]}" "$host" "bash -s -- $(printf '%q' "$prefix")" <"$ROOT/scripts/lib/remote-probe.sh" 2>/dev/null)" || probe_out=""
}

# ---- what is deployed (--skip-if-current) -----------------------------------------
sha256_stdin() { { sha256sum 2>/dev/null || shasum -a 256; } | awk '{print $1}'; }

# The git commit of this checkout ("+dirty.<hash>" with uncommitted changes of tracked
# files, which the source upload includes); nothing outside a git checkout.
local_tree_id() {
    local head d
    command -v git >/dev/null 2>&1 || return 0
    head="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null)" || return 0
    if [ -n "$(git -C "$ROOT" status --porcelain --untracked-files=no 2>/dev/null)" ]; then
        d="$(git -C "$ROOT" diff HEAD 2>/dev/null | sha256_stdin | cut -c1-12)"
        printf 'git:%s+dirty.%s' "$head" "$d"
    else
        printf 'git:%s' "$head"
    fi
}

# The commit <ref> of <url> points to (asked here, else on the host); nothing if unknown.
remote_commit() { # <url> <ref>
    local out="" sha
    if [ "${#2}" -eq 40 ] && [ -z "$(printf '%s' "$2" | tr -d '0-9a-f')" ]; then
        printf 'git:%s' "$2"
        return 0
    fi
    if command -v git >/dev/null 2>&1; then
        out="$(GIT_TERMINAL_PROMPT=0 git -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=20 ls-remote "$1" "$2" </dev/null 2>/dev/null)" || out=""
    fi
    if [ -z "$out" ] && [ -n "$probe_out" ]; then
        out="$(ssh "${ssh_args[@]}" "$host" "GIT_TERMINAL_PROMPT=0 git ls-remote $(printf '%q' "$1") $(printf '%q' "$2")" </dev/null 2>/dev/null)" || out=""
    fi
    sha="$(printf '%s\n' "$out" | awk -v r="$2" '
        $2 == "refs/tags/" r "^{}" { p = $1 }
        $2 == "refs/heads/" r { h = $1 }
        $2 == "refs/tags/" r { t = $1 }
        END { if (p) print p; else if (h) print h; else if (t) print t }')"
    [ -z "$sha" ] || printf 'git:%s' "$sha"
}

compute_source_id() {
    case "$bin_choice" in
        build)
            if [ "$source_kind" = git ]; then remote_commit "$git_url" "$git_ref"; else local_tree_id; fi
            ;;
        local) local_tree_id ;;
        file) [ ! -f "$binary" ] || printf 'sha256:%s' "$(sha256_stdin <"$binary")" ;;
        release)
            local u
            u="$(release_asset_url "${release_tag:-latest}" "$(probe ARCH | sed 's/^$/x86_64/')")"
            [ -z "$u" ] || printf 'release:%s' "$u"
            ;;
    esac
}

# The deploy settings recorded next to the binary; a change means "not current".
settings_id() {
    printf 'bind=%s;ifaces=%s;service=%s;terminal=%s;snmp_trap=%s;templates=%s;allow_insecure=%s;prefix=%s;dpdk=%s' \
        "$bind" "$(IFS=,; echo "${ifaces[*]}")" "$service" "$terminal" "$snmp_trap" "$templates" "$allow_insecure" "$prefix" "$dpdk"
}

# Is the host already running <source_id> with these settings (and healthy)?
host_is_current() {
    local st cur
    if [ -z "$source_id" ]; then
        warn "どのバージョンをデプロイするか判定できません (git が無い / 参照を解決できない)。スキップせずにデプロイします。" \
            "cannot tell which version this deploy would install (no git, or the ref cannot be resolved); deploying."
        return 1
    fi
    st="$(probe STAMP_SOURCE)"
    cur="$(probe WPG_SHA256)"
    if [ -z "$st" ] || [ -z "$cur" ]; then
        say "  ホストにデプロイ記録がありません。" "  No deploy record on the host."
        return 1
    fi
    if [ "$st" != "$source_id" ]; then
        say "  ホストは $st です (今回: $source_id)。" "  The host runs $st (this deploy: $source_id)."
        return 1
    fi
    if [ "$(probe STAMP_SETTINGS)" != "$(settings_id)" ]; then
        say "  設定が変わっています。" "  The settings changed."
        return 1
    fi
    if [ "$(probe STAMP_SHA256)" != "$cur" ]; then
        say "  インストール済みのバイナリが記録と違います (ロールバック / 手動の変更)。" "  The installed binary differs from the record (rollback / changed by hand)."
        return 1
    fi
    if [ "$service" -eq 1 ]; then
        if [ "$(probe SERVICE)" != active ]; then
            say "  wpg.service が動いていません。" "  wpg.service is not running."
            return 1
        fi
        if [ "$healthcheck" -eq 1 ] && ! ssh "${ssh_args[@]}" "$host" "bash -s -- $(printf '%q' "$hc_addr") $port 5" \
            <"$ROOT/scripts/lib/remote-health.sh" >/dev/null 2>&1; then
            say "  wpg が応答しません。" "  wpg does not answer."
            return 1
        fi
    fi
    return 0
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
        release) echo "GitHub Release (${release_tag:-latest})" ;;
    esac
}

show_summary() {
    printf '\n%s%s%s\n' "$c_step" "$(m "確認" "Summary")" "$c_off"
    printf '  %-22s %s\n' "$(m "接続先" "Host")" "$host (port ${ssh_port:-22})"
    printf '  %-22s %s\n' "$(m "バイナリ" "Binary")" "$(summary_binary)"
    printf '  %-22s %s\n' "$(m "サービス" "Service")" "$( [ "$service" -eq 1 ] && m "systemd (wpg.service)" "systemd (wpg.service)" || m "なし" "no")"
    printf '  %-22s %s\n' "$(m "待ち受け" "Listen")" "$bind"
    printf '  %-22s %s\n' "$(m "送信インタフェース" "TX interfaces")" "$( [ "${#ifaces[@]}" -gt 0 ] && (IFS=,; echo "${ifaces[*]}") || m "すべて (制限なし)" "all (no restriction)")"
    printf '  %-22s %s\n' "$(m "管理者パスワード" "Admin password")" "$( [ -n "$admin_pw$password_file" ] && m "設定する" "set" || m "変更しない" "unchanged")"
    printf '  %-22s %s\n' "$(m "Web ターミナル" "Web terminal")" "$( [ "$terminal" -eq 1 ] && m "有効" "on" || m "無効" "off")"
    printf '  %-22s %s\n' "$(m "SNMP トラップ受信" "SNMP traps")" "$( [ "$snmp_trap" -eq 1 ] && m "有効 (UDP 162)" "on (UDP 162)" || m "無効" "off")"
}

wizard() {
    local use_prev=0 h u v n i list name state addrs mgmt def_list answer
    printf '%s%s%s\n' "$c_step" "$(m "wpg デプロイウィザード (Enter で [ ] 内の既定値を使います。Ctrl+C で中止)" "wpg deploy wizard (Enter accepts the default in [ ]; Ctrl+C aborts)")" "$c_off"
    if [ -n "$host" ]; then
        p_host="${host#*@}"
        case "$host" in *@*) p_user="${host%@*}" ;; esac
    fi
    if [ -f "$profile" ]; then
        load_answers "$profile"
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
            identity="$(abs_path "$identity" "$orig_pwd")"
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
    version_before="$(probe WPG_VERSION)"

    # 2. upgrade or rollback
    if [ "$(probe WPG_PREV)" = 1 ]; then
        ask_choice v 1 "wpg はインストール済みです ($(probe WPG_VERSION))。何をしますか?" "wpg is installed ($(probe WPG_VERSION)). What do you want to do?" \
            "インストール / アップグレード|Install / upgrade" \
            "ロールバック (1 つ前のバイナリ wpg.prev に戻す)|Roll back (restore the previous binary wpg.prev)" \
            "中止|Quit"
        case "$v" in 2) rollback=1 ;; 3) result_status=aborted; exit 0 ;; esac
    fi

    # 3. sudo
    case "$(probe SUDO)" in
        root) sudo_mode=root ;;
        nopasswd) sudo_mode=nopasswd ;;
        none) die "$(m "ホストに sudo がありません。root でログインするか sudo を入れてください。" "sudo is missing on the host; log in as root or install sudo.")" ;;
        *)
            sudo_mode=stdin
            # A password from the environment / a file is tried first, without asking.
            if [ -n "${WPG_DEPLOY_SUDO_PASSWORD:-}$sudo_password_file" ]; then
                obtain_secret sudo_pw WPG_DEPLOY_SUDO_PASSWORD "$sudo_password_file" "" "" || true
                if check_sudo; then
                    say "  sudo パスワード: 環境変数 / ファイルの値を使います。" "  sudo password: using the environment / file value."
                else
                    sudo_pw=""
                    warn "環境変数 / ファイルの sudo パスワードが違います" "the sudo password from the environment / file is wrong"
                fi
            fi
            if [ -z "$sudo_pw" ]; then
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
            fi
            ;;
    esac

    if [ "$rollback" -eq 1 ]; then
        ask_yn answer "/usr/local/bin/wpg と wpg.prev を入れ替えてサービスを再起動します。よろしいですか?" \
            "Swap /usr/local/bin/wpg with wpg.prev and restart the service?" 1
        [ "$answer" -eq 1 ] || { say "中止しました。" "Aborted."; result_status=aborted; exit 0; }
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
                v="$(abs_path "$v" "$orig_pwd")"
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
        if [ -n "${WPG_DEPLOY_ADMIN_PASSWORD:-}" ]; then
            admin_pw="$WPG_DEPLOY_ADMIN_PASSWORD"
            say "  管理者パスワード: 環境変数 WPG_DEPLOY_ADMIN_PASSWORD の値を使います。" "  Admin password: using WPG_DEPLOY_ADMIN_PASSWORD."
        elif [ -n "$password_file" ]; then
            :
        else
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
    fi

    # 9. summary
    show_summary
    ask_yn answer "この内容で実行しますか?" "Go ahead?" 1
    [ "$answer" -eq 1 ] || { say "中止しました。" "Aborted."; result_status=aborted; exit 0; }
    ask_yn answer "この設定を次回のために保存しますか? (パスワードは保存しません)" "Save these answers for next time? (no passwords)" 1
    [ "$answer" -eq 0 ] || save_profile
}

# After a successful wizard run: save an answer file for unattended runs and show how.
offer_automation() {
    local answer file cmd hostpart
    printf '\n'
    ask_yn answer "この設定で次回から自動実行するファイルを保存しますか? (パスワードは保存しません)" \
        "Save a file to run this deploy unattended (automatically) from now on? (no passwords)" 1
    [ "$answer" -eq 1 ] || return 0
    hostpart="$(printf '%s' "${host#*@}" | tr -c 'A-Za-z0-9._-' '_')"
    ask file "保存先" "File" "${XDG_CONFIG_HOME:-$HOME/.config}/wpg/deploy-$hostpart.conf"
    file="$(abs_path "$file" "$orig_pwd")"
    write_answers "$file" automation
    cmd="$(printf '%q' "$SELF") --config $(printf '%q' "$file") --yes --skip-if-current"
    printf '%s%s%s\n' "$c_ok" "$(m "保存しました: $file (パスワードは含みません)" "Saved $file (no passwords).")" "$c_off"
    say "次回からは、質問なしで次の 1 行で実行できます:" "From now on this one line runs the same deploy without questions:"
    printf '  %s\n' "$cmd"
    if [ "$sudo_mode" = stdin ]; then
        say "  * sudo にパスワードが必要なホストです。実行前に環境変数 WPG_DEPLOY_SUDO_PASSWORD を設定するか、自分で作ったファイル (chmod 600) を sudo_password_file= に書いてください。" \
            "  * sudo needs a password on this host: set WPG_DEPLOY_SUDO_PASSWORD before running, or name a file you create yourself (chmod 600) with sudo_password_file=."
    fi
    if [ "$bin_choice" = build ] && [ "$source_kind" = local ]; then
        say "  * source=local は実行時点のこのチェックアウトをアップロードします。定期アップグレードには source=git がおすすめです。" \
            "  * source=local uploads this checkout as it is at that time; for scheduled upgrades source=git is the better choice."
    fi
    say "定期実行の例 (cron、毎週日曜 3:17。ログは ~/.local/state/wpg/ に残ります):" "Scheduled run, e.g. cron every Sunday at 3:17 (logs stay in ~/.local/state/wpg/):"
    printf '  17 3 * * 0 %s >/dev/null 2>&1\n' "$cmd"
    say "  (crontab -e で追加します。systemd タイマーの例は docs/ja/src/operations/deploy.md の「自動実行」にあります)" \
        "  (add it with crontab -e; a systemd timer example is in docs/en/src/operations/deploy.md, \"Unattended deploy\")"
}

# ---- several hosts --------------------------------------------------------------------
run_multi() {
    local entries=() line e i name th f kv work logdir pass_args=() skip=0 a n_ok=0 n_fail=0 rc
    multi=1
    if [ -n "$targets_file" ]; then
        [ -r "$targets_file" ] || cfg_err "$(m "ターゲットファイルを読めません" "cannot read the targets file"): $targets_file"
        while IFS= read -r line || [ -n "$line" ]; do
            line="${line%$'\r'}"
            trim line
            case "$line" in '' | '#'*) continue ;; esac
            entries+=("t:$line")
        done <"$targets_file"
    else
        while IFS= read -r line; do entries+=("s:$line"); done < <(answer_sections "$config_file")
    fi
    [ "${#entries[@]}" -gt 0 ] || cfg_err "$(m "デプロイ先のホストがありません" "no hosts to deploy to")"
    # Options passed on to every host run (the host-specific ones are added below).
    for a in "${orig_args[@]}"; do
        if [ "$skip" -eq 1 ]; then skip=0; continue; fi
        case "$a" in
            --targets | --parallel | --log | --result-file | --section | --config | --lang) skip=1 ;;
            --json | --yes | --interactive) ;;
            *) pass_args+=("$a") ;;
        esac
    done
    if [ "$yes" -eq 0 ] && [ "$check_config" -eq 0 ]; then
        can_prompt || cfg_err "$(m "標準入力が端末ではありません。質問なしで実行するには --yes を付けてください" "stdin is not a terminal: add --yes to run without questions")"
        say "デプロイ先 (${#entries[@]} 台):" "Hosts (${#entries[@]}):" >&2
        for e in "${entries[@]}"; do printf '  %s\n' "${e#?:}" >&2; done
        ask_yn a "実行しますか?" "Go ahead?" 1
        [ "$a" -eq 1 ] || { say "中止しました。" "Aborted."; exit 0; }
    fi
    # Parallel runs that need the deploy key must not each create it.
    if [ "$parallel" -gt 1 ] && [ ! -f "$deploy_key" ] && [ "$dry_run" -eq 0 ] && [ "$check_config" -eq 0 ]; then
        mkdir -p "$(dirname "$deploy_key")"
        ssh-keygen -q -t ed25519 -N "" -C "wpg-deploy@$(hostname 2>/dev/null || echo local)" -f "$deploy_key" </dev/null
    fi
    work="$(mktemp -d)"
    # The host runs resolve relative paths of the options against the caller's directory.
    cd "$orig_pwd"
    tmp_dir="$work"
    logdir="${XDG_STATE_HOME:-$HOME/.local/state}/wpg/deploy-$(date +%Y%m%d-%H%M%S)"
    [ "$check_config" -eq 1 ] || mkdir -p "$logdir"
    local pids=() names=() logs=() running=0 total="${#entries[@]}"
    # shellcheck disable=SC2317 # called from the INT / TERM trap
    kill_children() {
        local c
        for c in "${pids[@]}"; do [ -z "$c" ] || kill "$c" 2>/dev/null || true; done
    }
    trap 'kill_children; exit 130' INT TERM
    reap() { # waits for finished background runs
        local j r
        for j in "${!pids[@]}"; do
            [ -n "${pids[$j]}" ] || continue
            if ! kill -0 "${pids[$j]}" 2>/dev/null; then
                r=0
                wait "${pids[$j]}" || r=$?
                printf '%s\n' "$r" >"$work/$j.rc"
                say "[$((j + 1))/$total] ${names[$j]}: 終了 (終了コード $r, ログ ${logs[$j]})" "[$((j + 1))/$total] ${names[$j]}: finished (exit code $r, log ${logs[$j]})"
                pids[j]=""
                running=$((running - 1))
            fi
        done
    }
    for i in "${!entries[@]}"; do
        e="${entries[$i]}"
        local cmd=("$SELF" "${pass_args[@]}" --yes --lang "$lang" --result-file "$work/$i.json")
        [ -z "$config_file" ] || cmd+=(--config "$config_file")
        case "$e" in
            s:*)
                name="${e#s:}"
                cmd+=(--section "$name")
                ;;
            t:*)
                read -r -a f <<<"${e#t:}"
                name="${f[0]}"
                th="${f[0]}"
                # host:port (one colon; IPv6 addresses need port=)
                case "$th" in *:*:*) ;; *:*) cmd+=(--port "${th##*:}"); th="${th%:*}" ;; esac
                for kv in "${f[@]:1}"; do
                    case "$kv" in
                        identity=* | binary_file=* | *_password_file=*)
                            # Relative paths: relative to the targets file.
                            cmd+=(--set "${kv%%=*}=$(abs_path "${kv#*=}" "$(dirname "$targets_file")")")
                            ;;
                        *=*) cmd+=(--set "$kv") ;;
                        *) cfg_err "$targets_file: expected key=value after the host: $kv" ;;
                    esac
                done
                cmd+=(--set "name=$name" "$th")
                ;;
        esac
        names[i]="$name"
        logs[i]="$logdir/$(printf '%s' "$name" | tr -c 'A-Za-z0-9._@-' '_').log"
        if [ "$check_config" -eq 1 ]; then
            printf '\n%s== %s ==%s\n' "$c_step" "$name" "$c_off"
            rc=0
            "${cmd[@]}" </dev/null || rc=$?
            printf '%s\n' "$rc" >"$work/$i.rc"
        elif [ "$parallel" -le 1 ]; then
            printf '\n%s=== [%d/%d] %s ===%s\n' "$c_step" "$((i + 1))" "$total" "$name" "$c_off"
            rc=0
            "${cmd[@]}" --log "${logs[$i]}" </dev/null || rc=$?
            printf '%s\n' "$rc" >"$work/$i.rc"
        else
            while [ "$running" -ge "$parallel" ]; do
                reap
                [ "$running" -lt "$parallel" ] || sleep 1
            done
            "${cmd[@]}" </dev/null >"${logs[$i]}" 2>&1 &
            pids[i]=$!
            running=$((running + 1))
            say "[$((i + 1))/$total] $name: 開始 (ログ ${logs[$i]})" "[$((i + 1))/$total] $name: started (log ${logs[$i]})"
        fi
    done
    while [ "$running" -gt 0 ]; do
        reap
        [ "$running" -eq 0 ] || sleep 1
    done
    trap - INT TERM

    # Summary table and JSON.
    local objs="" row status code ver detail hostcol
    printf '\n%s%s%s\n' "$c_step" "$(m "結果" "Result")" "$c_off"
    local wn=4 wh=4
    for i in "${!entries[@]}"; do
        [ "${#names[$i]}" -le "$wn" ] || wn="${#names[$i]}"
    done
    wh=24
    [ "$wn" -le "$wh" ] || wh="$wn"
    printf '  %-*s %-*s %-9s %-4s %-12s %s\n' "$wn" NAME "$wh" HOST STATUS EXIT VERSION DETAIL
    for i in "${!entries[@]}"; do
        rc="$(cat "$work/$i.rc" 2>/dev/null || echo 1)"
        if [ -s "$work/$i.json.row" ]; then
            IFS="$US" read -r name hostcol status code ver detail <"$work/$i.json.row" || true
        else
            name="${names[$i]}" hostcol="${names[$i]}" status=failed code="$rc" ver="" detail="$(m "結果がありません (ログを確認)" "no result (see the log)")"
            [ "$check_config" -eq 0 ] || status=$([ "$rc" -eq 0 ] && echo ok || echo failed)
        fi
        if [ -s "$work/$i.json" ]; then
            row="$(cat "$work/$i.json")"
        else
            row="{\"name\":$(json_str "${names[$i]}"),\"status\":$(json_str "$status"),\"exit_code\":$rc,\"message\":$(json_str "$detail"),\"log\":$(json_str "${logs[$i]}")}"
        fi
        objs="${objs:+$objs,}$row"
        case "$status" in ok | skipped | dry-run | checked) n_ok=$((n_ok + 1)) ;; *) n_fail=$((n_fail + 1)) ;; esac
        [ "${#detail}" -le 60 ] || detail="${detail:0:57}..."
        printf '  %-*s %-*s %-9s %-4s %-12s %s\n' "$wn" "$name" "$wh" "$hostcol" "$status" "$code" "${ver#wpg }" "$detail"
    done
    [ "$check_config" -eq 1 ] || say "ログ: $logdir" "logs: $logdir"
    rc=0
    [ "$n_fail" -eq 0 ] || rc=6
    say "成功 $n_ok 台、失敗 $n_fail 台" "$n_ok succeeded, $n_fail failed"
    if [ "$json" -eq 1 ]; then
        local dr=false
        [ "$dry_run" -eq 0 ] || dr=true
        printf '{"tool":"wpg-deploy","format":1,"status":%s,"exit_code":%d,"dry_run":%s,"message":"","hosts":[%s]}\n' \
            "$(json_str "$( [ "$rc" -eq 0 ] && echo ok || echo failed)")" "$rc" "$dr" "$objs" >&3
    fi
    trap - EXIT
    cleanup
    exit "$rc"
}

# ---- unattended: connect, read the host facts, sudo -------------------------------
managed_connect() {
    batch_mode=yes
    ssh_opts+=(-o StrictHostKeyChecking=accept-new -o ConnectTimeout=15 -o ServerAliveInterval=30)
    setup_ssh_args
    step connect "$host に接続" "Connecting to $host"
    setup_key
    run_probe
    if [ -z "$probe_out" ]; then
        [ "$dry_run" -eq 1 ] || die "$(m "ホストの情報を取得できません ($host)" "cannot read the host facts of $host")"
        warn "ホストの情報を取得できません (dry run なので続けます)" "cannot read the host facts (dry run: continuing)"
        sudo_mode=nopasswd
        return 0
    fi
    version_before="$(probe WPG_VERSION)"
    say "  OS: $(probe OS_NAME) ($(probe ARCH))   sudo: $(probe SUDO)   wpg: ${version_before:--}" \
        "  OS: $(probe OS_NAME) ($(probe ARCH))   sudo: $(probe SUDO)   wpg: ${version_before:--}"
    [ "$(probe KERNEL)" = Linux ] || die "$(m "Linux ホストにだけデプロイできます" "only Linux hosts are supported")"
}

# After the --skip-if-current check, so that a run with nothing to do needs no sudo.
managed_sudo() {
    [ -n "$probe_out" ] || return 0
    case "$(probe SUDO)" in
        root) sudo_mode=root ;;
        nopasswd) sudo_mode=nopasswd ;;
        none) current_step=sudo; die "$(m "ホストに sudo がありません。root でログインするか sudo を入れてください。" "sudo is missing on the host; log in as root or install sudo.")" ;;
        *)
            sudo_mode=stdin
            current_step=sudo
            if ! obtain_secret sudo_pw WPG_DEPLOY_SUDO_PASSWORD "$sudo_password_file" "sudo パスワード ($host)" "sudo password ($host)"; then
                [ -n "$ssh_pw" ] || die "$(m "$host の sudo にはパスワードが必要です。環境変数 WPG_DEPLOY_SUDO_PASSWORD か sudo_password_file= を設定するか、ホストで NOPASSWD を設定してください" "sudo on $host needs a password: set WPG_DEPLOY_SUDO_PASSWORD or sudo_password_file=, or allow NOPASSWD for this user on the host")"
                sudo_pw="$ssh_pw"
            fi
            check_sudo force || die "$(m "sudo に失敗しました (パスワードが違うか、sudo の権限がありません)" "sudo failed (wrong password, or the user may not use sudo)")"
            current_step=connect
            ;;
    esac
}

# ---- main -------------------------------------------------------------------------
case "$bind" in *:*) ;; *) cfg_err "--bind must be <addr>:<port>" ;; esac
port="${bind##*:}"
# Address the health check connects to (on the host itself).
hc_addr="${bind%:*}"
case "$hc_addr" in 0.0.0.0 | "" | "[::]") hc_addr=127.0.0.1 ;; esac

if [ "$interactive" -eq 0 ]; then
    if [ -n "$targets_file" ]; then
        [ -z "$host" ] || cfg_err "$(m "--targets とホストの指定は同時に使えません" "--targets cannot be combined with a host argument")"
        run_multi
    elif [ -n "$config_file" ] && [ -z "$section" ] && [ -z "$host" ] && [ -n "$(answer_sections "$config_file")" ]; then
        run_multi
    fi
fi

# Which binary (the answer file's binary_source unless an option chose one).
if [ -n "$cli_bin" ]; then
    bin_choice="$cli_bin"
elif [ -n "$config_file" ] && [ "$interactive" -eq 0 ]; then
    bin_choice="${p_bin:-build}"
    case "$bin_choice" in
        build) build_on_host=1 ;;
        file)
            binary="$p_binary_file"
            [ -n "$binary" ] || cfg_err "$(m "binary_source=file には binary_file= が必要です" "binary_source=file needs binary_file=")"
            ;;
        release) release_tag="${p_release_tag:-latest}" ;;
    esac
else
    bin_choice=local
fi

if [ "$managed" -eq 1 ]; then
    if [ -n "$host" ]; then
        case "$host" in *@*) ;; *) [ -z "$p_user" ] || host="$p_user@$host" ;; esac
    elif [ -n "$p_host" ]; then
        host="${p_user:-ubuntu}@$p_host"
    fi
    [ -n "$host" ] || cfg_err "$(m "接続先がありません (user@host、設定ファイルの host=、または --targets)" "no host: give user@host, host= in the answer file, or --targets")"
    [ "${ssh_port:-22}" != 22 ] || ssh_port=""
fi

if [ "$interactive" -eq 1 ]; then
    [ -z "$log_file" ] && log_file="${XDG_STATE_HOME:-$HOME/.local/state}/wpg/deploy-$(date +%Y%m%d-%H%M%S).log"
    wizard
else
    [ -n "$host" ] || { usage >&2; current_step=config; fail_msg="no host"; exit 1; }
    setup_ssh_args
fi

# Settings that every mode must get right.
case "$hugepages" in '' | *[!0-9]*) cfg_err "--hugepages must be a number" ;; esac
case "$bind" in *:*) ;; *) cfg_err "--bind must be <addr>:<port>" ;; esac
port="${bind##*:}"
hc_addr="${bind%:*}"
case "$hc_addr" in 0.0.0.0 | "" | "[::]") hc_addr=127.0.0.1 ;; esac
case "$port" in '' | *[!0-9]*) cfg_err "--bind must end in a port number" ;; esac
for i in "${ifaces[@]}"; do
    case "$i" in '' | *[!A-Za-z0-9_.:@-]*) cfg_err "invalid interface name: $i" ;; esac
done
[ -z "$password_file" ] || [ -r "$password_file" ] || cfg_err "cannot read $password_file"
[ "$build_on_host" -eq 0 ] || [ -z "$binary$release_tag" ] || cfg_err "--build-on-host cannot be combined with --binary / --release-tag"
if [ "$bin_choice" = file ] && [ "$managed" -eq 1 ]; then
    [ -f "$binary" ] || cfg_err "$(m "バイナリがありません" "binary not found"): $binary"
fi

if [ "$check_config" -eq 1 ]; then
    secret_src() { # <ENV> <file var value> <dpapi key>
        if [ -n "${!1:-}" ]; then echo "\$$1"
        elif [ -n "$2" ]; then echo "$2"
        else echo "-"; fi
    }
    printf '  %-16s %s\n' name "${label:-${host#*@}}" host "$host" ssh_port "${ssh_port:-22}" identity "${identity:--}" \
        binary "$(summary_binary)" bind "$bind" ifaces "$( [ "${#ifaces[@]}" -gt 0 ] && (IFS=,; echo "${ifaces[*]}") || echo all)" \
        service "$service" terminal "$terminal" snmp_trap "$snmp_trap" templates "$templates" healthcheck "$healthcheck" \
        prefix "$prefix" ssh_password "$(secret_src WPG_DEPLOY_SSH_PASSWORD "$ssh_password_file")" \
        sudo_password "$(secret_src WPG_DEPLOY_SUDO_PASSWORD "$sudo_password_file")" \
        admin_password "$(secret_src WPG_DEPLOY_ADMIN_PASSWORD "$password_file")"
    [ "${#dpapi_keys[@]}" -eq 0 ] || warn "${dpapi_keys[*]} は deploy.ps1 (Windows) だけが読みます" "${dpapi_keys[*]} are only read by deploy.ps1 on Windows"
    result_status=checked
    exit 0
fi

if [ "$managed" -eq 1 ] && [ -z "$log_file" ]; then
    log_file="${XDG_STATE_HOME:-$HOME/.local/state}/wpg/deploy-$(date +%Y%m%d-%H%M%S)-$(printf '%s' "${host#*@}" | tr -c 'A-Za-z0-9._-' '_').log"
fi
if [ "$managed" -eq 1 ] && [ "$yes" -eq 0 ] && ! can_prompt; then
    cfg_err "$(m "標準入力が端末ではありません。質問なしで実行するには --yes を付けてください" "stdin is not a terminal: add --yes to run without questions")"
fi

if [ -n "$log_file" ]; then
    mkdir -p "$(dirname "$log_file")"
    exec > >(tee -a "$log_file") 2>&1
    say "ログ: $log_file" "log: $log_file"
fi
[ "${#dpapi_keys[@]}" -eq 0 ] || warn "${dpapi_keys[*]} は deploy.ps1 (Windows) だけが読みます。無視します。" "${dpapi_keys[*]} are only read by deploy.ps1 on Windows; ignored."

if [ "$managed" -eq 1 ]; then
    managed_connect
    if [ "$rollback" -eq 0 ]; then
        source_id="$(compute_source_id)"
        if [ "$skip_if_current" -eq 1 ] && [ -n "$probe_out" ]; then
            if host_is_current; then
                current_step=""
                result_status=skipped
                version_after="$version_before"
                printf '%s%s%s\n' "$c_ok" "$(m "$host はすでに最新です ($source_id, ${version_before:-?})。何もしません。" "$host is already current ($source_id, ${version_before:-?}); nothing to do.")" "$c_off"
                exit 0
            fi
        fi
    fi
    managed_sudo
    if [ "$rollback" -eq 0 ]; then
        # Admin password: needed for a first install that listens on a non-loopback address.
        if [ "$service" -eq 1 ] && [ -z "$password_file" ]; then
            if [ -n "${WPG_DEPLOY_ADMIN_PASSWORD:-}" ]; then
                admin_pw="$WPG_DEPLOY_ADMIN_PASSWORD"
            else
                loop=0
                case "${bind%:*}" in 127.* | localhost | "[::1]") loop=1 ;; esac
                if [ "$loop" -eq 0 ] && [ "$allow_insecure" -eq 0 ] && { [ -z "$version_before" ] || [ "$(probe SERVICE)" = absent ]; }; then
                    if can_prompt; then
                        while :; do
                            ask_secret admin_pw "wpg の管理者パスワード (Web UI のログイン用)" "wpg admin password (Web UI login)"
                            [ -n "$admin_pw" ] || continue
                            ask_secret v "もう一度入力してください" "Repeat it"
                            [ "$v" = "$admin_pw" ] && break
                            say "  一致しません。" "  The passwords do not match."
                        done
                    elif [ "$dry_run" -eq 0 ]; then
                        cfg_err "$(m "$host は初回のインストールなので管理者パスワードが必要です。環境変数 WPG_DEPLOY_ADMIN_PASSWORD か admin_password_file= を設定するか、bind=127.0.0.1:$port にしてください" "$host is a first install and needs an admin password: set WPG_DEPLOY_ADMIN_PASSWORD or admin_password_file=, or use bind=127.0.0.1:$port")"
                    fi
                fi
            fi
        fi
    elif [ "$(probe SERVICE)" != absent ] && [ -n "$probe_out" ]; then
        rollback_health=1
        service=0 terminal=0 snmp_trap=0
    fi
    if [ "$yes" -eq 0 ]; then
        show_summary
        ask_yn answer "この内容で実行しますか?" "Go ahead?" 1
        [ "$answer" -eq 1 ] || { say "中止しました。" "Aborted."; result_status=aborted; exit 0; }
    fi
elif [ "$interactive" -eq 0 ]; then
    if [ "$ask_password" -eq 1 ] || [ -n "$identity" ]; then
        ssh_opts+=(-o StrictHostKeyChecking=accept-new)
        setup_ssh_args
        step connect "$host に接続" "Connecting to $host"
        setup_key
    fi
    if [ "$ask_sudo" -eq 1 ]; then
        sudo_mode=stdin
        current_step=sudo
        obtain_secret sudo_pw WPG_DEPLOY_SUDO_PASSWORD "$sudo_password_file" "sudo パスワード" "sudo password" \
            || die "$(m "sudo パスワードがありません (WPG_DEPLOY_SUDO_PASSWORD)" "no sudo password (WPG_DEPLOY_SUDO_PASSWORD)")"
        check_sudo || die "$(m "sudo に失敗しました" "sudo failed")"
        current_step=""
    fi
fi
# What is deployed, recorded on the host for --skip-if-current (unattended runs computed
# it above). Without a connection (plain options) a git ref is not resolved.
if [ "$managed" -eq 0 ] && [ "$rollback" -eq 0 ]; then
    if [ "$interactive" -eq 1 ] || [ "$bin_choice" != build ] || [ "$source_kind" = local ]; then
        source_id="$(compute_source_id)"
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
        # The wizard and unattended runs always rebuild (cargo is incremental), so the
        # binary matches the checkout recorded for --skip-if-current.
        if [ ! -x "$binary" ] || [ "$interactive" -eq 1 ] || [ "$managed" -eq 1 ]; then
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
if [ "$build_on_host" -eq 1 ] && [ "$source_kind" = local ] && [ "$rollback" -eq 0 ]; then
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
    # Recorded on the host for --skip-if-current.
    remote_args+=(--stamp-source "$source_id" --stamp-settings "$(settings_id)" --stamp-tool deploy.sh)
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
if [ "$dry_run" -eq 0 ]; then
    version_after="$(ssh "${ssh_args[@]}" "$host" "$(printf '%q' "$prefix/bin/wpg") --version" </dev/null 2>/dev/null | head -n 1)" || version_after=""
fi

# ---- healthcheck ------------------------------------------------------------
h="${host#*@}"
case "${bind%:*}" in
    127.* | localhost | "[::1]") deployed_url="http://127.0.0.1:$port/" ;;
    *) deployed_url="http://$h:$port/" ;;
esac
if [ "$do_health" -eq 1 ]; then
    step health "ヘルスチェック (http://$hc_addr:$port/api/health)" "Health check (http://$hc_addr:$port/api/health)"
    run ssh "${ssh_args[@]}" "$host" "bash '$stage/remote-health.sh' '$hc_addr' '$port' 30; rc=\$?; rm -rf '$stage'; exit \$rc" </dev/null
    current_step=""
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
    deployed_url=""
    info "done (no service started; run 'wpg serve' on $host or pass --service)"
fi
[ "$dry_run" -eq 0 ] || result_status=dry-run

if [ "$interactive" -eq 1 ] && [ "$dry_run" -eq 0 ] && [ "$rollback" -eq 0 ]; then
    offer_automation || true
fi
