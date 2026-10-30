<#
.SYNOPSIS
  Deploy wpg from Windows to a Linux host (Ubuntu / Debian) over SSH: a wizard that asks
  everything, or parameters / an answer file for unattended use.
.DESCRIPTION
  Run without parameters (or double-click scripts\deploy.bat) to start the wizard. It
  asks for the host, the SSH user and password, the sudo password, how to get the binary,
  the listen address, the admin password of the web UI and the interfaces jobs may send
  on, shows a summary and then:

    1. logs in with the SSH password once and installs a dedicated key
       (%USERPROFILE%\.ssh\wpg_deploy_ed25519) on the host; later runs use the key,
    2. builds wpg on the host from source (default; installs build-essential, libpcap-dev,
       git, rustup and trunk there), or uploads a Linux binary, or a GitHub Release,
    3. installs it with the same host-side script as scripts/deploy.sh
       (scripts/lib/deploy-remote.sh): /usr/local/bin/wpg (the previous one is kept as
       wpg.prev), setcap, the systemd unit, /etc/wpg (wpg.toml is never overwritten),
    4. waits for GET /api/health and prints the URL,
    5. offers to save an answer file for unattended runs (-ConfigFile ... -Yes).

  Unattended: -ConfigFile <answer file> -Yes never asks anything. Secrets come from an SSH
  key, the environment (WPG_DEPLOY_SSH_PASSWORD, WPG_DEPLOY_SUDO_PASSWORD,
  WPG_DEPLOY_ADMIN_PASSWORD), *_password_file keys or DPAPI files (*_password_dpapi,
  written by the wizard); a missing one fails at once. Exit codes: 0 done (also skipped
  or dry run), 1 usage / answer file / missing setting or admin password, 2 connection,
  SSH or sudo (also a missing SSH or sudo password), 3 preparing or building the binary,
  4 upload or installation, 5 health check, 6 several hosts and at least one failed.

  Needs the OpenSSH client of Windows 10 / 11 (ssh, scp, ssh-keygen) and tar.exe (both
  ship with Windows 10 1803 and later). Works with Windows PowerShell 5.1 and PowerShell 7.
  Passwords are never written to disk in plain text or put on a command line: the sudo
  password and the admin password go to the host through the standard input of ssh.
.PARAMETER HostName
  Target host (IP address or name). Without it (and without -ConfigFile) the wizard starts.
.PARAMETER User
  SSH user (default: ubuntu).
.PARAMETER Port
  SSH port (default: 22).
.PARAMETER Identity
  Existing SSH private key to use instead of the deploy key.
.PARAMETER SshPassword
  SSH password as a SecureString (default: asked when a key login is not possible).
.PARAMETER SudoPassword
  sudo password as a SecureString (default: asked when sudo needs one).
.PARAMETER AdminPassword
  Admin password of the web UI as a SecureString (default: asked on a first install).
.PARAMETER BinarySource
  build (default: build on the host from source), file (upload -Binary) or release
  (download a GitHub Release).
.PARAMETER Binary
  Linux binary to upload with -BinarySource file.
.PARAMETER Source
  With -BinarySource build: local (default: upload this checkout) or git.
.PARAMETER GitUrl
  Repository for -Source git (default: the GitHub repository).
.PARAMETER GitRef
  Branch, tag or commit for -Source git (default: main).
.PARAMETER ReleaseTag
  Release for -BinarySource release (default: latest).
.PARAMETER Bind
  Listen address of the service (default: 0.0.0.0:8080).
.PARAMETER Iface
  Interfaces jobs may send on (tx.allowed_ifaces); empty = all.
.PARAMETER NoService
  Do not install the systemd service (only the binary and setcap).
.PARAMETER Terminal
  Enable the web terminal ([terminal], only written into a new /etc/wpg/wpg.toml and only
  when the installed wpg supports it).
.PARAMETER SnmpTrap
  Enable the SNMP trap receiver on UDP 162 ([snmp] trap_listen, same rules as -Terminal).
.PARAMETER DhcpIface
  Interface the DHCP server answers on ([dhcp] iface in /etc/wpg/wpg.env; with the service).
.PARAMETER DhcpIfaceAddr
  Static addresses for -DhcpIface (e.g. 192.168.100.1/24, 2001:db8:1::1/64), written to
  /etc/netplan/90-wpg-<iface>.yaml on the host.
.PARAMETER Rollback
  Swap /usr/local/bin/wpg with wpg.prev and restart the service.
.PARAMETER NoTemplates
  Do not copy the sample templates.
.PARAMETER NoHealthcheck
  Skip the health check.
.PARAMETER ProfilePath
  Wizard answers (without passwords) to load and save
  (default: %APPDATA%\wpg\deploy.conf).
.PARAMETER Lang
  Language of the messages: ja or en (default: from the Windows display language).
.PARAMETER LogFile
  Log file (default: %LOCALAPPDATA%\wpg\deploy-logs\deploy-<time>.log).
.PARAMETER Interactive
  Start the wizard even when -HostName / -ConfigFile is given (its values become the
  defaults).
.PARAMETER Yes
  Never ask anything (no confirmation, no password prompt): for scheduled tasks and CI.
.PARAMETER DryRun
  Connect and check, but only print what would change on the host.
.PARAMETER ConfigFile
  Answer file: key=value lines (the format of the wizard's deploy.conf). [host <name>]
  sections deploy to several hosts.
.PARAMETER Section
  Only the [host <name>] section of -ConfigFile.
.PARAMETER Targets
  Several hosts: a file with one "[user@]host[:port] [key=value ...]" per line.
.PARAMETER Parallel
  Several hosts: deploy to up to this many hosts at a time (default 1).
.PARAMETER Set
  Answer-file keys to override, as "key=value" strings.
.PARAMETER Prefix
  Install directory on the host (default /usr/local).
.PARAMETER AllowInsecure
  Start the service without a password on a non-loopback address.
.PARAMETER SkipIfCurrent
  Do nothing when the host already runs this source (git commit / binary checksum) with
  the same settings.
.PARAMETER Json
  Print a JSON summary on the standard output; all other output goes to the standard error.
.PARAMETER CheckConfig
  Check the answer file / targets and print the settings, without connecting.
.PARAMETER ResultFile
  Internal (several hosts): where a host run writes its JSON result.
.EXAMPLE
  .\scripts\deploy.ps1
  The wizard.
.EXAMPLE
  .\scripts\deploy.ps1 -ConfigFile $env:APPDATA\wpg\deploy-192.0.2.10.conf -Yes -SkipIfCurrent
  Unattended, with the answer file the wizard saved.
.EXAMPLE
  .\scripts\deploy.ps1 -HostName 192.0.2.10 -User ubuntu -Iface ens4,ens5 -Yes
  Build on the host from this checkout and install the service, without questions.
.EXAMPLE
  .\scripts\deploy.ps1 -HostName 192.0.2.10 -User ubuntu -Rollback
#>
[CmdletBinding()]
param(
    [string]$HostName = "",
    [string]$User = "",
    [int]$Port = 0,
    [string]$Identity = "",
    [System.Security.SecureString]$SshPassword = $null,
    [System.Security.SecureString]$SudoPassword = $null,
    [System.Security.SecureString]$AdminPassword = $null,
    [ValidateSet("", "build", "file", "release")]
    [string]$BinarySource = "",
    [string]$Binary = "",
    [ValidateSet("", "local", "git")]
    [string]$Source = "",
    [string]$GitUrl = "",
    [string]$GitRef = "",
    [string]$ReleaseTag = "",
    [string]$Bind = "",
    [string[]]$Iface = @(),
    [switch]$NoService,
    [switch]$Terminal,
    [switch]$SnmpTrap,
    [string]$DhcpIface = "",
    [string[]]$DhcpIfaceAddr = @(),
    [switch]$Rollback,
    [switch]$NoTemplates,
    [switch]$NoHealthcheck,
    [string]$ProfilePath = "",
    [ValidateSet("", "ja", "en")]
    [string]$Lang = "",
    [string]$LogFile = "",
    [switch]$Interactive,
    [switch]$Yes,
    [switch]$DryRun,
    [string]$ConfigFile = "",
    [string]$Section = "",
    [string]$Targets = "",
    [ValidateRange(1, 64)]
    [int]$Parallel = 1,
    [string[]]$Set = @(),
    [string]$Prefix = "",
    [switch]$AllowInsecure,
    [switch]$SkipIfCurrent,
    [switch]$Json,
    [switch]$CheckConfig,
    [string]$ResultFile = ""
)

# Windows PowerShell 5.1 compatible: no ?? / ?: / && / ||, no ::new on generics.
Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"
# The progress bar of Invoke-WebRequest makes downloads very slow in Windows PowerShell.
$ProgressPreference = "SilentlyContinue"

$Root = Split-Path -Parent $PSScriptRoot
$GitHubRepo = "SeiyaFunaokaJP/web-packet-generator"
$DefaultGitUrl = "https://github.com/$GitHubRepo.git"
$OnWindows = ($env:OS -eq "Windows_NT")
$HomeDir = $env:USERPROFILE
if (-not $HomeDir) { $HomeDir = $env:HOME }
$StartedAt = [DateTime]::UtcNow

# A full path, relative to the current PowerShell location (.NET methods would resolve a
# relative path against the process directory instead).
function Get-FullPath([string]$Path) {
    return $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
}

# ---- language / output ------------------------------------------------------
if (-not $Lang) {
    $Lang = "en"
    $ui = "$PSUICulture"
    if (-not $ui) { $ui = "$env:LANG" }
    if ($ui -like "ja*") { $Lang = "ja" }
}

function T([string]$Ja, [string]$En) {
    if ($script:Lang -eq "ja") { return $Ja }
    return $En
}

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$script:LogPath = ""
$script:Tail = New-Object System.Collections.Generic.List[string]

function Write-Log([string]$Line) {
    if ($script:LogPath) {
        try { [System.IO.File]::AppendAllText($script:LogPath, $Line + "`n", $script:Utf8NoBom) } catch { }
    }
}

# Console output. With -Json everything goes to the standard error, so that the standard
# output only carries the JSON summary.
function Write-Ui([string]$Text = "", [string]$Color = "") {
    if ($script:Json) {
        [Console]::Error.WriteLine($Text)
        return
    }
    if ($Color) { Write-Host $Text -ForegroundColor $Color } else { Write-Host $Text }
}

function Add-Tail([string]$Line) {
    $script:Tail.Add($Line)
    if ($script:Tail.Count -gt 40) { $script:Tail.RemoveAt(0) }
}

function Say([string]$Ja, [string]$En, [string]$Color = "") {
    $msg = T $Ja $En
    Write-Ui $msg $Color
    Write-Log $msg
}

function Warn([string]$Ja, [string]$En) {
    $msg = (T "警告: " "warning: ") + (T $Ja $En)
    Say $msg $msg "Yellow"
}

$script:StepNo = 0
$script:StepTotal = 0
$script:StepId = ""
$script:StepName = ""
function Step([string]$Id, [string]$Ja, [string]$En) {
    $script:StepNo++
    $script:StepId = $Id
    $script:StepName = T $Ja $En
    $total = ""
    if ($script:StepTotal -gt 0) { $total = "/$($script:StepTotal)" }
    Write-Ui ""
    $line = "[$($script:StepNo)$total] $($script:StepName)"
    Write-Ui $line "Cyan"
    Write-Log ""
    Write-Log $line
}

function Fail([string]$Ja, [string]$En) {
    throw (T $Ja $En)
}

# A problem with the parameters or the answer file (exit code 1).
function Fail-Config([string]$Ja, [string]$En) {
    $script:StepId = "config"
    throw (T $Ja $En)
}

# Ends the run early without an error (aborted by the user, nothing to do, several hosts).
$StopMarker = "__wpg_deploy_stop__"
$script:ResultStatus = "ok"
function Stop-Run([string]$Status) {
    $script:ResultStatus = $Status
    throw $script:StopMarker
}

# Exit code of a failed step (documented in the help and in deploy.md).
function Get-StepExitCode([string]$Id) {
    switch ($Id) {
        "connect" { return 2 }
        "sudo" { return 2 }
        "binary" { return 3 }
        "deps" { return 3 }
        "build" { return 3 }
        "upload" { return 4 }
        "install" { return 4 }
        "health" { return 5 }
    }
    return 1
}

# ---- JSON (ASCII only, so that the console code page does not matter) -------------
function ConvertTo-JsonText([object]$Value) {
    if ($null -eq $Value) { return "null" }
    if ($Value -is [bool]) { if ($Value) { return "true" } else { return "false" } }
    if ($Value -is [int] -or $Value -is [long]) { return [string]$Value }
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append('"')
    foreach ($ch in ([string]$Value).ToCharArray()) {
        $code = [int]$ch
        if ($ch -eq [char]'"') { [void]$sb.Append('\"') }
        elseif ($ch -eq [char]'\') { [void]$sb.Append('\\') }
        elseif ($code -eq 10) { [void]$sb.Append('\n') }
        elseif ($code -lt 32) { }
        elseif ($code -gt 126) { [void]$sb.Append(('\u{0:x4}' -f $code)) }
        else { [void]$sb.Append($ch) }
    }
    [void]$sb.Append('"')
    return $sb.ToString()
}

# pairs: "key", value, "key", value, ...
function ConvertTo-JsonObject([object[]]$Pairs) {
    $parts = @()
    for ($i = 0; $i -lt $Pairs.Count; $i += 2) {
        $parts += ((ConvertTo-JsonText ([string]$Pairs[$i])) + ":" + (ConvertTo-JsonText $Pairs[$i + 1]))
    }
    return "{" + ($parts -join ",") + "}"
}

# ---- interaction ----------------------------------------------------------------------
# No questions with -Yes. Outside the wizard also none when nobody can answer (input
# redirected, -NonInteractive, a scheduled task without a desktop).
$script:NoConsoleInput = $false
try { if ([Console]::IsInputRedirected) { $script:NoConsoleInput = $true } } catch { }
if (-not [Environment]::UserInteractive) { $script:NoConsoleInput = $true }
foreach ($a in [Environment]::GetCommandLineArgs()) { if ($a -match '^-noni') { $script:NoConsoleInput = $true } }
$script:wizard = $false

function Test-CanPrompt {
    if ($script:Yes) { return $false }
    if ($script:wizard) { return $true }
    return (-not $script:NoConsoleInput)
}

function Assert-CanPrompt([string]$What) {
    if (-not (Test-CanPrompt)) {
        Fail "質問できません (-Yes が指定されているか、対話できない実行です): $What" "cannot ask (-Yes, or not an interactive session): $What"
    }
}

# ---- native processes -------------------------------------------------------
# Quotes one argument for a Windows command line (CommandLineToArgvW rules).
function ConvertTo-WinArg([string]$Arg) {
    if ($Arg.Length -gt 0 -and $Arg -notmatch '[\s"]') { return $Arg }
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append('"')
    $bs = 0
    foreach ($ch in $Arg.ToCharArray()) {
        if ($ch -eq [char]'\') { $bs++; continue }
        if ($ch -eq [char]'"') {
            [void]$sb.Append([string]'\' * ($bs * 2 + 1))
            [void]$sb.Append('"')
            $bs = 0
            continue
        }
        if ($bs -gt 0) { [void]$sb.Append([string]'\' * $bs); $bs = 0 }
        [void]$sb.Append($ch)
    }
    [void]$sb.Append([string]'\' * ($bs * 2))
    [void]$sb.Append('"')
    return $sb.ToString()
}

# Quotes a string for the remote POSIX shell.
function ConvertTo-ShArg([string]$Arg) {
    return "'" + $Arg.Replace("'", "'\''") + "'"
}

# Runs a program, streams its standard output (unless -Quiet), feeds -InputText (plus a
# LF) to its standard input. Returns @{ ExitCode; Output; Error }.
function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$ArgumentList = @(),
        [AllowNull()][object]$InputText = $null,
        [switch]$Quiet
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = (($ArgumentList | ForEach-Object { ConvertTo-WinArg $_ }) -join " ")
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = $script:Utf8NoBom
    $psi.StandardErrorEncoding = $script:Utf8NoBom
    # Share the console: an SSH_ASKPASS helper or an unexpected prompt can still use it.
    $psi.CreateNoWindow = $false
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    [void]$p.Start()
    $errTask = $p.StandardError.ReadToEndAsync()
    if ($null -ne $InputText) {
        $bytes = $script:Utf8NoBom.GetBytes($InputText + "`n")
        try {
            $p.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
            $p.StandardInput.BaseStream.Flush()
        } catch { }
        [Array]::Clear($bytes, 0, $bytes.Length)
    }
    try { $p.StandardInput.Close() } catch { }
    $out = New-Object System.Collections.Generic.List[string]
    while ($true) {
        $line = $p.StandardOutput.ReadLine()
        if ($null -eq $line) { break }
        $out.Add($line)
        if (-not $Quiet) {
            Write-Ui "  $line"
            Write-Log "  $line"
            Add-Tail $line
        }
    }
    $p.WaitForExit()
    $err = $errTask.Result
    if ($err -and -not $Quiet) {
        foreach ($l in ($err -split "`r?`n")) {
            if ($l) { Write-Ui "  $l" "DarkYellow"; Write-Log "  $l"; Add-Tail $l }
        }
    }
    $rc = $p.ExitCode
    $p.Dispose()
    return @{ ExitCode = $rc; Output = $out.ToArray(); Error = $err }
}

function Find-Tool([string]$Name) {
    $c = Get-Command "$Name.exe" -ErrorAction SilentlyContinue
    if (-not $c) { $c = Get-Command $Name -ErrorAction SilentlyContinue }
    if ($c) { return $c.Path }
    if ($OnWindows) {
        # 32-bit PowerShell on 64-bit Windows does not see System32\OpenSSH.
        foreach ($d in @("$env:SystemRoot\Sysnative\OpenSSH", "$env:SystemRoot\System32\OpenSSH", "$env:SystemRoot\System32")) {
            $f = Join-Path $d "$Name.exe"
            if (Test-Path $f) { return $f }
        }
    }
    return $null
}

# ---- prompts ----------------------------------------------------------------
function Read-Text([string]$Ja, [string]$En, [string]$Default = "") {
    Assert-CanPrompt (T $Ja $En)
    $p = T $Ja $En
    if ($Default) { $p = "$p [$Default]" }
    $a = Read-Host $p
    if ($null -eq $a -or $a.Trim() -eq "") { return $Default }
    return $a.Trim()
}

function Read-Required([string]$Ja, [string]$En, [string]$Default = "") {
    while ($true) {
        $a = Read-Text $Ja $En $Default
        if ($a) { return $a }
        Say "  入力してください。" "  Please enter a value."
    }
}

function ConvertFrom-Secure([System.Security.SecureString]$Secure) {
    if ($null -eq $Secure) { return "" }
    $bstr = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($Secure)
    try { return [System.Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) }
    finally { [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
}

function Read-Secret([string]$Ja, [string]$En) {
    Assert-CanPrompt (T $Ja $En)
    $s = Read-Host (T $Ja $En) -AsSecureString
    return (ConvertFrom-Secure $s)
}

function Read-YesNo([string]$Ja, [string]$En, [bool]$Default) {
    Assert-CanPrompt (T $Ja $En)
    $hint = "y/N"
    if ($Default) { $hint = "Y/n" }
    while ($true) {
        $a = Read-Host ((T $Ja $En) + " [$hint]")
        if ($null -eq $a -or $a.Trim() -eq "") { return $Default }
        switch -regex ($a.Trim()) {
            '^(y|yes|はい)$' { return $true }
            '^(n|no|いいえ)$' { return $false }
        }
        Say "  y か n で答えてください。" "  Please answer y or n."
    }
}

# Options are "<ja>|<en>" strings; returns the chosen number (1-based).
function Read-Choice([string]$TitleJa, [string]$TitleEn, [string[]]$Options, [int]$Default = 1) {
    Assert-CanPrompt (T $TitleJa $TitleEn)
    Say $TitleJa $TitleEn
    for ($i = 0; $i -lt $Options.Count; $i++) {
        $parts = $Options[$i].Split("|")
        Write-Ui ("  {0}) {1}" -f ($i + 1), (T $parts[0] $parts[1]))
    }
    while ($true) {
        $a = Read-Host ((T "番号を選んでください" "Choose a number") + " [$Default]")
        if ($null -eq $a -or $a.Trim() -eq "") { return $Default }
        $n = 0
        if ([int]::TryParse($a.Trim(), [ref]$n) -and $n -ge 1 -and $n -le $Options.Count) { return $n }
        Say "  1 から $($Options.Count) の番号を入力してください。" "  Enter a number from 1 to $($Options.Count)."
    }
}

# ---- settings -----------------------------------------------------------------
# $cfg: the wizard answers (saved in the profile); $opt: the other answer-file keys.
$cfg = [ordered]@{
    host          = $HostName
    user          = $User
    ssh_port      = ""
    identity      = $Identity
    binary_source = $BinarySource
    binary_file   = $Binary
    source        = $Source
    git_url       = $GitUrl
    git_ref       = $GitRef
    bind          = $Bind
    ifaces        = ($Iface -join ",")
    service       = ""
    terminal      = ""
    snmp_trap     = ""
}
if ($Port -gt 0) { $cfg.ssh_port = "$Port" }
if ($NoService) { $cfg.service = "0" }
if ($Terminal) { $cfg.terminal = "1" }
if ($SnmpTrap) { $cfg.snmp_trap = "1" }
if ($Identity) { $cfg.identity = Get-FullPath $Identity }
if ($Binary) { $cfg.binary_file = Get-FullPath $Binary }

$opt = @{}
if ($ReleaseTag) { $opt["release_tag"] = $ReleaseTag }
if ($Prefix) { $opt["prefix"] = $Prefix }
if ($NoTemplates) { $opt["templates"] = "0" }
if ($NoHealthcheck) { $opt["healthcheck"] = "0" }
if ($AllowInsecure) { $opt["allow_insecure"] = "1" }
if ($SkipIfCurrent) { $opt["skip_if_current"] = "1" }
if ($DhcpIface) { $opt["dhcp_iface"] = $DhcpIface }
if ($DhcpIfaceAddr.Count -gt 0) { $opt["dhcp_iface_addr"] = (($DhcpIfaceAddr | ForEach-Object { $_ -split "," }) -join ",") }

$AnswerKeys = @("host", "user", "ssh_port", "port", "identity", "binary_source", "binary_file", "source", "git_url",
    "git_ref", "release_tag", "prefix", "bind", "ifaces", "dhcp_iface", "dhcp_iface_addr", "service", "terminal", "snmp_trap", "templates",
    "healthcheck", "allow_insecure", "skip_if_current", "dpdk", "hugepages", "admin_password_file",
    "ssh_password_file", "sudo_password_file", "admin_password_dpapi", "ssh_password_dpapi",
    "sudo_password_dpapi", "name", "parallel", "lang")
$PathKeys = @("identity", "binary_file", "admin_password_file", "ssh_password_file", "sudo_password_file",
    "admin_password_dpapi", "ssh_password_dpapi", "sudo_password_dpapi")
$BoolKeys = @("service", "terminal", "snmp_trap", "templates", "healthcheck", "allow_insecure", "skip_if_current", "dpdk")

if (-not $ProfilePath) {
    $appData = $env:APPDATA
    if (-not $appData) { $appData = Join-Path $HomeDir ".config" }
    $ProfilePath = Join-Path (Join-Path $appData "wpg") "deploy.conf"
}
$ProfilePath = Get-FullPath $ProfilePath
if ($ConfigFile) { $ConfigFile = Get-FullPath $ConfigFile }
if ($Targets) { $Targets = Get-FullPath $Targets }
if ($ResultFile) { $ResultFile = Get-FullPath $ResultFile }

function Resolve-AnswerPath([string]$Value, [string]$Base) {
    if (-not $Value) { return "" }
    if ($Value -eq "~") { return $HomeDir }
    if ($Value.StartsWith("~/") -or $Value.StartsWith("~\")) { return (Join-Path $HomeDir $Value.Substring(2)) }
    if ([System.IO.Path]::IsPathRooted($Value)) { return $Value }
    return [System.IO.Path]::GetFullPath((Join-Path $Base $Value))
}

# Checks and normalizes one answer-file value.
function ConvertTo-AnswerValue([string]$Key, [string]$Value, [string]$Base, [string]$Where) {
    if ($BoolKeys -contains $Key) {
        if ($Value -match '^(1|y|yes|true|on)$') { return "1" }
        if ($Value -match '^(0|n|no|false|off|)$') { return "0" }
        Fail-Config "${Where}: $Key は 1 か 0 で指定してください: $Value" "${Where}: $Key must be 1 or 0: $Value"
    }
    if ($PathKeys -contains $Key) { return (Resolve-AnswerPath $Value $Base) }
    if ($Key -eq "ifaces") { return ((($Value -split '[,\s]+') | Where-Object { $_ }) -join ",") }
    switch ($Key) {
        "binary_source" {
            if ($Value -eq "local") { Fail-Config "${Where}: binary_source=local は deploy.sh だけで使えます" "${Where}: binary_source=local is only supported by deploy.sh" }
            if ($Value -notmatch '^(build|file|release|)$') { Fail-Config "${Where}: binary_source は build / file / release です: $Value" "${Where}: binary_source must be build, file or release: $Value" }
        }
        "source" { if ($Value -notmatch '^(local|git|)$') { Fail-Config "${Where}: source は local か git です: $Value" "${Where}: source must be local or git: $Value" } }
        "ssh_port" { if ($Value -notmatch '^\d*$') { Fail-Config "${Where}: ssh_port は数字です: $Value" "${Where}: ssh_port must be a number: $Value" } }
        "port" { if ($Value -notmatch '^\d*$') { Fail-Config "${Where}: port は数字です: $Value" "${Where}: port must be a number: $Value" } }
        "lang" { if ($Value -notmatch '^(ja|en|)$') { Fail-Config "${Where}: lang は ja か en です" "${Where}: lang must be ja or en" } }
    }
    return $Value
}

function Get-AnswerSections([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { Fail-Config "設定ファイルを読めません: $Path" "cannot read the answer file: $Path" }
    $names = @()
    foreach ($raw in [System.IO.File]::ReadAllLines($Path)) {
        $line = $raw.Trim()
        if ($line -match '^\[(.*)\]$') {
            $n = $Matches[1].Trim()
            if ($n -match '^host\s+(.+)$') { $n = $Matches[1].Trim() }
            $names += $n
        }
    }
    return , $names
}

# The global keys, then those of [host <Want>]. Returns an ordered hashtable.
function Read-AnswerFile([string]$Path, [string]$Want = "") {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { Fail-Config "設定ファイルを読めません: $Path" "cannot read the answer file: $Path" }
    $full = (Resolve-Path -LiteralPath $Path).ProviderPath
    $base = Split-Path -Parent $full
    $result = [ordered]@{}
    $cur = ""
    $found = $false
    $secHost = $false
    $n = 0
    foreach ($raw in [System.IO.File]::ReadAllLines($full)) {
        $n++
        $line = $raw.Trim()
        if (-not $line -or $line.StartsWith("#") -or $line.StartsWith(";")) { continue }
        if ($line -match '^\[(.*)\]$') {
            $cur = $Matches[1].Trim()
            if ($cur -match '^host\s+(.+)$') { $cur = $Matches[1].Trim() }
            if (-not $cur) { Fail-Config "${full}:${n}: セクション名がありません" "${full}:${n}: empty section name" }
            if ($cur -eq $Want) { $found = $true }
            continue
        }
        $i = $line.IndexOf("=")
        if ($i -lt 1) { Fail-Config "${full}:${n}: key=value の形ではありません: $line" "${full}:${n}: expected key=value: $line" }
        $k = $line.Substring(0, $i).Trim()
        $v = $line.Substring($i + 1).Trim()
        if ($v.Length -ge 2 -and $v.StartsWith('"') -and $v.EndsWith('"')) { $v = $v.Substring(1, $v.Length - 2) }
        if ($AnswerKeys -notcontains $k) { Fail-Config "${full}:${n}: 不明なキー '$k'" "${full}:${n}: unknown key '$k'" }
        if ($cur -and $cur -ne $Want) { continue }
        $v = ConvertTo-AnswerValue $k $v $base "${full}:${n}"
        if ($k -eq "port") { $k = "ssh_port" }
        if ($cur -and $k -eq "host") { $secHost = $true }
        $result[$k] = $v
    }
    if ($Want) {
        if (-not $found) { Fail-Config "セクション [host $Want] がありません: $full" "no section [host $Want] in $full" }
        if (-not $secHost) { $result["host"] = $Want }
        if (-not $result.Contains("name")) { $result["name"] = $Want }
    }
    return $result
}

# Answers into $cfg / $opt: only into empty values (parameters win) unless -Force.
function Merge-Answers([object]$Answers, [switch]$Force) {
    foreach ($k in @($Answers.Keys)) {
        $v = [string]$Answers[$k]
        if ($k -eq "host" -and $v -match '^(.+)@(.+)$') {
            if ($Force -or -not $cfg.user) { $cfg.user = $Matches[1] }
            $v = $Matches[2]
        }
        if ($cfg.Contains($k)) {
            if ($Force -or -not $cfg[$k]) { $cfg[$k] = $v }
        } elseif ($Force -or -not $opt.ContainsKey($k) -or -not $opt[$k]) {
            $opt[$k] = $v
        }
    }
}

function Get-Opt([string]$Key, [string]$Fallback) {
    if ($opt.ContainsKey($Key) -and $opt[$Key]) { return [string]$opt[$Key] }
    return $Fallback
}

function Save-Profile([string]$Path, [switch]$Automation, [string[]]$Extra = @()) {
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    $lines = @("# wpg deploy answers (scripts/deploy.sh / deploy.ps1). No passwords are stored.")
    if ($Automation) {
        $lines += "# Unattended: scripts\deploy.ps1 -ConfigFile `"$Path`" -Yes [-SkipIfCurrent] [-Json]"
        $lines += "# Secrets: the SSH key the wizard installed (%USERPROFILE%\.ssh\wpg_deploy_ed25519), the"
        $lines += "# environment variables WPG_DEPLOY_SSH_PASSWORD / WPG_DEPLOY_SUDO_PASSWORD /"
        $lines += "# WPG_DEPLOY_ADMIN_PASSWORD, files you create (sudo_password_file=, admin_password_file=)"
        $lines += "# or DPAPI files written by the wizard (sudo_password_dpapi=; this Windows user only)."
        $lines += "# Several hosts: add [host <name>] sections with host=, user=, ifaces=... below."
        $lines += "# Reference: docs/en/src/operations/deploy.md, `"Unattended deploy`"."
    }
    foreach ($k in $cfg.Keys) { $lines += "$k=$($cfg[$k])" }
    $lines += $Extra
    [System.IO.File]::WriteAllText($Path, (($lines -join "`n") + "`n"), $script:Utf8NoBom)
    if (-not $Automation) { Say "設定を保存しました: $Path (パスワードは保存していません)" "Saved the answers to $Path (no passwords)." }
}

function Get-Default([string]$Key, [string]$Fallback) {
    if ($cfg[$Key]) { return [string]$cfg[$Key] }
    return $Fallback
}

# ---- secrets ----------------------------------------------------------------------
# The environment variable, else the first line of <name>_file, else the DPAPI file
# <name>_dpapi (Windows, this user only). "" when none is set.
function Get-SecretValue([string]$EnvName, [string]$Name) {
    $v = [Environment]::GetEnvironmentVariable($EnvName, "Process")
    if ($v) { return $v }
    $file = Get-Opt "${Name}_file" ""
    if ($file) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { Fail-Config "パスワードファイルを読めません: $file" "cannot read the password file: $file" }
        $lines = [System.IO.File]::ReadAllLines($file)
        if ($lines.Count -lt 1 -or -not $lines[0]) { Fail-Config "パスワードファイルが空です: $file" "the password file is empty: $file" }
        return $lines[0]
    }
    $dp = Get-Opt "${Name}_dpapi" ""
    if ($dp) {
        if (-not $script:OnWindows) { Fail-Config "${Name}_dpapi は Windows でだけ使えます" "${Name}_dpapi only works on Windows" }
        if (-not (Test-Path -LiteralPath $dp -PathType Leaf)) { Fail-Config "DPAPI ファイルがありません: $dp" "no such DPAPI file: $dp" }
        try {
            $ss = ConvertTo-SecureString ([System.IO.File]::ReadAllText($dp).Trim())
            return (ConvertFrom-Secure $ss)
        } catch {
            Fail-Config "$dp を復号できません (別の Windows ユーザー / PC で作ったファイル?)" "cannot decrypt $dp (made by another Windows user or on another PC?)"
        }
    }
    return ""
}

# A user-bound DPAPI file (ConvertFrom-SecureString without a key). Never plain text.
function Save-DpapiSecret([string]$Path, [string]$Plain) {
    $ss = New-Object System.Security.SecureString
    foreach ($ch in $Plain.ToCharArray()) { $ss.AppendChar($ch) }
    $enc = ConvertFrom-SecureString $ss
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    [System.IO.File]::WriteAllText($Path, $enc + "`n", $script:Utf8NoBom)
}

# ---- state --------------------------------------------------------------------
$ssh = $null
$scp = $null
$sshBase = @()
$scpBase = @()
$target = ""
$sshPw = ""
$sudoPw = ""
$adminPw = ""
$sudoMode = ""
$stage = ""
$probe = @{}
$probeIfaces = @()
$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("wpg-deploy-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
$deployKey = Join-Path (Join-Path $HomeDir ".ssh") "wpg_deploy_ed25519"
$doRollback = [bool]$Rollback
$rollbackHealth = $false
$script:LastSshError = ""
$script:SourceId = ""
$script:VersionBefore = ""
$script:VersionAfter = ""
$script:DeployedUrl = ""
$script:FailMessage = ""
$script:FailedStep = ""
$script:MultiDone = $false

function Set-SshBase([string]$KeyFile) {
    $common = @("-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=15", "-o", "ServerAliveInterval=30")
    if ($KeyFile) { $common += @("-o", "IdentityFile=$KeyFile", "-o", "IdentitiesOnly=yes") }
    $port = Get-Default "ssh_port" "22"
    $script:sshBase = @("-o", "BatchMode=yes") + $common + @("-p", $port)
    $script:scpBase = @("-q", "-o", "BatchMode=yes") + $common + @("-P", $port)
}

function Invoke-Ssh([string]$Command, [object]$InputText = $null, [switch]$Quiet) {
    $args2 = $script:sshBase + @($script:target, "{ $Command ; } 2>&1")
    return (Invoke-Native -FilePath $script:ssh -ArgumentList $args2 -InputText $InputText -Quiet:$Quiet)
}

function Invoke-SshChecked([string]$Command, [object]$InputText = $null) {
    $r = Invoke-Ssh $Command $InputText
    if ($r.ExitCode -ne 0) { Fail "リモートのコマンドが失敗しました (終了コード $($r.ExitCode))" "the remote command failed (exit code $($r.ExitCode))" }
    return $r
}

# Runs <Command> as root on the host (sudo as needed).
function Invoke-Root([string]$Command) {
    switch ($script:sudoMode) {
        "root" { return (Invoke-SshChecked $Command) }
        "nopasswd" { return (Invoke-SshChecked "sudo -n $Command") }
        default { return (Invoke-SshChecked "sudo -S -p '' $Command" $script:sudoPw) }
    }
}

function Invoke-Upload([string[]]$Paths, [string]$Dest, [switch]$Recurse) {
    $a = $script:scpBase
    if ($Recurse) { $a = $a + @("-r") }
    $a = $a + $Paths + @("$($script:target):$Dest")
    $r = Invoke-Native -FilePath $script:scp -ArgumentList $a
    if ($r.ExitCode -ne 0) { Fail "アップロードに失敗しました: $($Paths -join ', ')" "upload failed: $($Paths -join ', ')" }
}

function Test-KeyLogin([string]$KeyFile) {
    $a = @("-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "StrictHostKeyChecking=accept-new")
    if ($KeyFile) { $a += @("-o", "IdentityFile=$KeyFile", "-o", "IdentitiesOnly=yes") }
    $a += @("-p", (Get-Default "ssh_port" "22"), $script:target, "true")
    $r = Invoke-Native -FilePath $script:ssh -ArgumentList $a -Quiet
    $script:LastSshError = [string]$r.Error
    return ($r.ExitCode -eq 0)
}

# Fails at once when the host cannot be reached at all (rather than trying passwords).
function Assert-Reachable {
    $e = $script:LastSshError
    if ($e -match 'Connection refused|timed out|Could not resolve|No route to host|Network is unreachable|Host key verification failed|REMOTE HOST IDENTIFICATION HAS CHANGED|Connection closed|Connection reset') {
        $last = (($e -split "`r?`n") | Where-Object { $_ -and $_ -notmatch '^Warning' } | Select-Object -Last 1)
        Fail "$($script:target) (ポート $(Get-Default 'ssh_port' '22')) に接続できません: $last" "cannot connect to $($script:target) (port $(Get-Default 'ssh_port' '22')): $last"
    }
}

function Get-OpenSshVersion {
    $r = Invoke-Native -FilePath $script:ssh -ArgumentList @("-V") -Quiet
    $text = "$($r.Error) $($r.Output -join ' ')"
    if ($text -match 'OpenSSH[_A-Za-z]*_(\d+)\.(\d+)') { return [version]("{0}.{1}" -f $Matches[1], $Matches[2]) }
    return [version]"0.0"
}

# Key login, or install the deploy key with the SSH password once.
function Initialize-SshKey {
    if ($cfg.identity) {
        if (-not (Test-Path $cfg.identity)) { Fail "鍵ファイルがありません: $($cfg.identity)" "no such key file: $($cfg.identity)" }
        if (-not (Test-KeyLogin $cfg.identity)) {
            Assert-Reachable
            Fail "鍵 $($cfg.identity) でログインできません" "cannot log in with the key $($cfg.identity)"
        }
        Set-SshBase $cfg.identity
        Say "  鍵 $($cfg.identity) でログインしました。" "  Logged in with the key $($cfg.identity)."
        return
    }
    if ((Test-Path $script:deployKey) -and (Test-KeyLogin $script:deployKey)) {
        Set-SshBase $script:deployKey
        Say "  鍵 $($script:deployKey) でログインしました (パスワードは不要です)。" "  Logged in with the key $($script:deployKey) (no password needed)."
        return
    }
    if (Test-KeyLogin "") {
        Set-SshBase ""
        Say "  既存の SSH 鍵でログインしました。" "  Logged in with your existing SSH key."
        return
    }
    Assert-Reachable
    if (-not $script:sshPw) { $script:sshPw = Get-SecretValue "WPG_DEPLOY_SSH_PASSWORD" "ssh_password" }
    if (-not $script:sshPw) {
        if (-not (Test-CanPrompt)) {
            Fail "$($script:target) に鍵でログインできず、SSH パスワードもありません。環境変数 WPG_DEPLOY_SSH_PASSWORD か ssh_password_file= / ssh_password_dpapi= を設定するか、identity= で鍵を指定するか、一度ウィザードを実行してデプロイ用の鍵を登録してください" "cannot log in to $($script:target) with a key and no SSH password is given: set WPG_DEPLOY_SSH_PASSWORD or ssh_password_file= / ssh_password_dpapi=, name a key with identity=, or run the wizard once to install the deploy key"
        }
        $script:sshPw = Read-Secret "SSH パスワード ($($script:target))" "SSH password ($($script:target))"
    }
    $sshDir = Split-Path -Parent $script:deployKey
    if (-not (Test-Path $sshDir)) { New-Item -ItemType Directory -Force -Path $sshDir | Out-Null }
    if (-not (Test-Path $script:deployKey)) {
        $keygen = Find-Tool "ssh-keygen"
        if (-not $keygen) { Fail "ssh-keygen が見つかりません" "ssh-keygen not found" }
        $comment = "wpg-deploy@" + ("$env:COMPUTERNAME" -replace '[^A-Za-z0-9._-]', '')
        $r = Invoke-Native -FilePath $keygen -ArgumentList @("-q", "-t", "ed25519", "-N", "", "-C", $comment, "-f", $script:deployKey) -Quiet
        if ($r.ExitCode -ne 0) { Fail "鍵を作れませんでした: $($r.Error)" "could not create the key: $($r.Error)" }
        Say "  デプロイ用の鍵を作りました: $($script:deployKey)" "  Created the deploy key $($script:deployKey)"
    }
    $pub = ([System.IO.File]::ReadAllText("$($script:deployKey).pub")).Trim()
    if ($pub -notmatch '^ssh-ed25519 [A-Za-z0-9+/=]+ [A-Za-z0-9@._-]*$') { Fail "公開鍵の形式が想定外です: $($script:deployKey).pub" "unexpected public key format: $($script:deployKey).pub" }
    # Only single quotes: the command also goes through PowerShell's own argument passing.
    $install = "umask 077; mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && { grep -qxF '$pub' ~/.ssh/authorized_keys || echo '$pub' >>~/.ssh/authorized_keys; }"
    Say "  公開鍵をホストの ~/.ssh/authorized_keys に登録します (パスワードを使うのはこの 1 回だけです)。" "  Installing the public key into ~/.ssh/authorized_keys on the host (the password is used this once)."
    $port = Get-Default "ssh_port" "22"
    $pwOpts = @("-o", "PreferredAuthentications=password,keyboard-interactive", "-o", "PubkeyAuthentication=no",
        "-o", "NumberOfPasswordPrompts=1", "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=15", "-p", $port)
    $ok = $false
    if ($script:DryRun) {
        Say "  (dry run) ssh $($script:target) <install $($script:deployKey).pub>" "  (dry run) ssh $($script:target) <install $($script:deployKey).pub>"
        Set-SshBase $script:deployKey
        return
    }
    if ((Get-OpenSshVersion) -ge [version]"8.4") {
        # SSH_ASKPASS: a helper prints the password from this process's environment.
        $askDir = Join-Path $script:tmpDir "askpass"
        New-Item -ItemType Directory -Force -Path $askDir | Out-Null
        $saved = @{}
        foreach ($n in @("SSH_ASKPASS", "SSH_ASKPASS_REQUIRE", "DISPLAY", "WPG_DEPLOY_ASKPASS")) {
            $saved[$n] = [Environment]::GetEnvironmentVariable($n, "Process")
        }
        try {
            if ($OnWindows) {
                $helper = Join-Path $askDir "askpass.cmd"
                $body = "@echo off`r`npowershell.exe -NoProfile -NonInteractive -Command `"[Console]::Out.Write(`$env:WPG_DEPLOY_ASKPASS)`"`r`n"
                [System.IO.File]::WriteAllText($helper, $body, (New-Object System.Text.ASCIIEncoding))
            } else {
                $helper = Join-Path $askDir "askpass"
                [System.IO.File]::WriteAllText($helper, "#!/bin/sh`nprintf '%s\n' `"`$WPG_DEPLOY_ASKPASS`"`n", $script:Utf8NoBom)
                & chmod 0700 $helper
            }
            $env:SSH_ASKPASS = $helper
            $env:SSH_ASKPASS_REQUIRE = "force"
            if (-not $env:DISPLAY) { $env:DISPLAY = "localhost:0" }
            $env:WPG_DEPLOY_ASKPASS = $script:sshPw
            $r = Invoke-Native -FilePath $script:ssh -ArgumentList ($pwOpts + @($script:target, $install)) -Quiet
            $ok = ($r.ExitCode -eq 0)
        } finally {
            foreach ($n in $saved.Keys) { [Environment]::SetEnvironmentVariable($n, $saved[$n], "Process") }
            Remove-Item -Recurse -Force $askDir -ErrorAction SilentlyContinue
        }
    }
    if (-not $ok) {
        if (-not (Test-CanPrompt)) { Fail "公開鍵を登録できませんでした (SSH パスワードが違うか、OpenSSH が 8.4 より古い)" "could not install the public key (wrong SSH password, or OpenSSH older than 8.4)" }
        Say "  パスワードを自動入力できませんでした。ssh が聞いてきたら、もう一度パスワードを入力してください。" "  Could not pass the password automatically; enter it again when ssh asks."
        & $script:ssh -o PubkeyAuthentication=no -o StrictHostKeyChecking=accept-new -p $port $script:target $install
        if ($LASTEXITCODE -ne 0) { Fail "公開鍵を登録できませんでした (パスワードが違う?)" "could not install the public key (wrong password?)" }
    }
    if (-not (Test-KeyLogin $script:deployKey)) { Fail "鍵を登録しましたが、鍵でログインできません (サーバ側で公開鍵認証が無効?)" "the key was installed but key login fails (public key authentication disabled on the server?)" }
    Set-SshBase $script:deployKey
    Say "  鍵でログインできるようになりました。次回からパスワードは不要です。" "  Key login works; no password needed next time." "Green"
}

# Uploads the host-side scripts into a fresh staging directory and runs remote-probe.sh.
function Initialize-Stage {
    $r = Invoke-Ssh "mktemp -d /tmp/wpg-deploy.XXXXXX" -Quiet
    if ($r.ExitCode -ne 0 -or $r.Output.Count -lt 1) { Fail "ホストに一時ディレクトリを作れません: $($r.Output -join ' ')" "cannot create a staging directory on the host: $($r.Output -join ' ')" }
    $script:stage = $r.Output[$r.Output.Count - 1].Trim()
    $files = @(
        (Join-Path $Root "scripts\lib\deploy-remote.sh"),
        (Join-Path $Root "scripts\lib\remote-build.sh"),
        (Join-Path $Root "scripts\lib\remote-health.sh"),
        (Join-Path $Root "scripts\lib\remote-probe.sh"),
        (Join-Path $Root "packaging\systemd\wpg.service")
    )
    Invoke-Upload $files "$($script:stage)/"
    # A Windows checkout may have CRLF line endings; bash needs LF.
    $null = Invoke-SshChecked "sed -i 's/\r`$//' $($script:stage)/*.sh $($script:stage)/wpg.service"
    $r = Invoke-Ssh ("bash $($script:stage)/remote-probe.sh " + (ConvertTo-ShArg $script:InstallPrefix)) -Quiet
    $script:probe = @{}
    $list = @()
    foreach ($line in $r.Output) {
        if ($line -match '^WPG_PROBE_([A-Z0-9_]+)=(.*)$') {
            if ($Matches[1] -eq "IFACE") { $list += $Matches[2] } else { $script:probe[$Matches[1]] = $Matches[2] }
        }
    }
    $script:probeIfaces = $list
}

function Get-Probe([string]$Key) {
    if ($script:probe.ContainsKey($Key)) { return [string]$script:probe[$Key] }
    return ""
}

function Remove-Stage {
    if ($script:stage -and $script:ssh -and $script:target) {
        $null = Invoke-Ssh "rm -rf $($script:stage)" -Quiet
        $script:stage = ""
    }
}

# ---- GitHub Release -----------------------------------------------------------
function Get-ReleaseAsset([string]$Tag, [string]$Arch) {
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    } catch { }
    $api = "https://api.github.com/repos/$GitHubRepo/releases/latest"
    if ($Tag -and $Tag -ne "latest") { $api = "https://api.github.com/repos/$GitHubRepo/releases/tags/$Tag" }
    try {
        $rel = Invoke-RestMethod -Uri $api -UseBasicParsing -TimeoutSec 20 -Headers @{ "User-Agent" = "wpg-deploy" }
    } catch {
        return $null
    }
    foreach ($a in $rel.assets) {
        if ($a.name -like "wpg-*-$Arch-unknown-linux-gnu.tar.gz") { return $a.browser_download_url }
    }
    return $null
}

# ---- what is deployed (-SkipIfCurrent) ------------------------------------------------
function Get-Sha256Hex([byte[]]$Bytes) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $h = $sha.ComputeHash($Bytes) } finally { $sha.Dispose() }
    return (($h | ForEach-Object { $_.ToString("x2") }) -join "")
}

# The git commit of this checkout ("+dirty.<hash>" with uncommitted changes of tracked
# files, which the source upload includes); "" outside a git checkout.
function Get-LocalTreeId {
    $git = Find-Tool "git"
    if (-not $git -or -not (Test-Path (Join-Path $Root ".git"))) { return "" }
    $r = Invoke-Native -FilePath $git -ArgumentList @("-C", $Root, "rev-parse", "HEAD") -Quiet
    if ($r.ExitCode -ne 0 -or $r.Output.Count -lt 1) { return "" }
    $head = $r.Output[0].Trim()
    $st = Invoke-Native -FilePath $git -ArgumentList @("-C", $Root, "status", "--porcelain", "--untracked-files=no") -Quiet
    if ($st.Output.Count -gt 0) {
        $d = Invoke-Native -FilePath $git -ArgumentList @("-C", $Root, "diff", "HEAD") -Quiet
        $text = ($d.Output -join "`n") + "`n"
        return ("git:{0}+dirty.{1}" -f $head, (Get-Sha256Hex ($script:Utf8NoBom.GetBytes($text))).Substring(0, 12))
    }
    return "git:$head"
}

# The commit <Ref> of <Url> points to (asked here, else on the host); "" if unknown.
function Get-RemoteCommit([string]$Url, [string]$Ref) {
    if ($Ref -match '^[0-9a-f]{40}$') { return "git:$Ref" }
    $lines = @()
    $git = Find-Tool "git"
    if ($git) {
        # Never a credential prompt (terminal or Git Credential Manager window).
        $saved = @($env:GIT_TERMINAL_PROMPT, $env:GCM_INTERACTIVE)
        $env:GIT_TERMINAL_PROMPT = "0"
        $env:GCM_INTERACTIVE = "never"
        try { $lines = (Invoke-Native -FilePath $git -ArgumentList @("ls-remote", $Url, $Ref) -Quiet).Output }
        finally { $env:GIT_TERMINAL_PROMPT = $saved[0]; $env:GCM_INTERACTIVE = $saved[1] }
    }
    if ($lines.Count -eq 0 -and $script:target) {
        $lines = (Invoke-Ssh ("GIT_TERMINAL_PROMPT=0 git ls-remote " + (ConvertTo-ShArg $Url) + " " + (ConvertTo-ShArg $Ref)) -Quiet).Output
    }
    $peeled = ""; $head = ""; $tag = ""
    foreach ($l in $lines) {
        $f = $l -split '\s+'
        if ($f.Count -lt 2) { continue }
        if ($f[1] -eq "refs/tags/$Ref^{}") { $peeled = $f[0] }
        elseif ($f[1] -eq "refs/heads/$Ref") { $head = $f[0] }
        elseif ($f[1] -eq "refs/tags/$Ref") { $tag = $f[0] }
    }
    foreach ($s in @($peeled, $head, $tag)) { if ($s -match '^[0-9a-f]{40}$') { return "git:$s" } }
    return ""
}

function Get-SourceId {
    switch ($cfg.binary_source) {
        "build" {
            if ($cfg.source -eq "git") { return (Get-RemoteCommit $cfg.git_url $cfg.git_ref) }
            return (Get-LocalTreeId)
        }
        "file" {
            if (Test-Path -LiteralPath $cfg.binary_file -PathType Leaf) { return ("sha256:" + (Get-FileHash -Algorithm SHA256 -LiteralPath $cfg.binary_file).Hash.ToLower()) }
        }
        "release" {
            $arch = Get-Probe "ARCH"
            if (-not $arch) { $arch = "x86_64" }
            if (-not $script:releaseUrl) { $script:releaseUrl = Get-ReleaseAsset $script:ReleaseTagValue $arch }
            if ($script:releaseUrl) { return "release:$($script:releaseUrl)" }
        }
    }
    return ""
}

# The deploy settings recorded next to the binary (same format as deploy.sh).
function Get-SettingsId {
    $id = ("bind={0};ifaces={1};service={2};terminal={3};snmp_trap={4};templates={5};allow_insecure={6};prefix={7};dpdk=0" -f `
            $cfg.bind, $cfg.ifaces, $cfg.service, $cfg.terminal, $cfg.snmp_trap, (Get-Opt "templates" "1"), (Get-Opt "allow_insecure" "0"), $script:InstallPrefix)
    # Only when used, so that hosts deployed before these options stay "current".
    $dhcp = Get-Opt "dhcp_iface" ""
    if ($dhcp) { $id += (";dhcp_iface={0};dhcp_iface_addr={1}" -f $dhcp, (Get-Opt "dhcp_iface_addr" "")) }
    return $id
}

# Is the host already running this source with these settings (and healthy)?
function Test-HostCurrent([string]$HcAddr, [string]$HcPort) {
    if (-not $script:SourceId) {
        Warn "どのバージョンをデプロイするか判定できません (git が無い / 参照を解決できない)。スキップせずにデプロイします。" "cannot tell which version this deploy would install (no git, or the ref cannot be resolved); deploying."
        return $false
    }
    $stamp = Get-Probe "STAMP_SOURCE"
    $cur = Get-Probe "WPG_SHA256"
    if (-not $stamp -or -not $cur) { Say "  ホストにデプロイ記録がありません。" "  No deploy record on the host."; return $false }
    if ($stamp -ne $script:SourceId) { Say "  ホストは $stamp です (今回: $($script:SourceId))。" "  The host runs $stamp (this deploy: $($script:SourceId))."; return $false }
    if ((Get-Probe "STAMP_SETTINGS") -ne (Get-SettingsId)) { Say "  設定が変わっています。" "  The settings changed."; return $false }
    if ((Get-Probe "STAMP_SHA256") -ne $cur) { Say "  インストール済みのバイナリが記録と違います (ロールバック / 手動の変更)。" "  The installed binary differs from the record (rollback / changed by hand)."; return $false }
    if ($cfg.service -eq "1") {
        if ((Get-Probe "SERVICE") -ne "active") { Say "  wpg.service が動いていません。" "  wpg.service is not running."; return $false }
        if ((Get-Opt "healthcheck" "1") -eq "1") {
            $r = Invoke-Ssh ("bash $($script:stage)/remote-health.sh " + (ConvertTo-ShArg $HcAddr) + " $HcPort 5") -Quiet
            if ($r.ExitCode -ne 0) { Say "  wpg が応答しません。" "  wpg does not answer."; return $false }
        }
    }
    return $true
}

# ---- hints on failure -----------------------------------------------------------
function Show-Hint([string]$Id) {
    switch ($Id) {
        "tools" { Say "Windows の「設定 > アプリ > オプション機能」で「OpenSSH クライアント」を追加してください (管理者 PowerShell なら: Add-WindowsCapability -Online -Name OpenSSH.Client~~~~0.0.1.0)。" "Add the 'OpenSSH Client' optional feature (Settings > Apps > Optional features; elevated PowerShell: Add-WindowsCapability -Online -Name OpenSSH.Client~~~~0.0.1.0)." }
        "connect" { Say "ホスト名 / IP、SSH ポート、ユーザ名、パスワードを確認してください。手動で接続できるか試してください: ssh -p $(Get-Default 'ssh_port' '22') $($script:target)" "Check the host / IP, SSH port, user and password. Try connecting by hand: ssh -p $(Get-Default 'ssh_port' '22') $($script:target)" }
        "sudo" { Say "sudo パスワードが違うか、ユーザに sudo 権限がありません (sudo グループに入っているか確認してください)。" "Wrong sudo password, or the user may not use sudo (is it in the sudo group?)." }
        "binary" { Say "バイナリの用意に失敗しました。ホスト上でのビルドを選ぶか、Linux 用 (ELF) のバイナリを指定してください。" "Preparing the binary failed. Choose building on the host, or give a Linux (ELF) binary." }
        "upload" { Say "アップロードに失敗しました。ホストの /tmp の空き容量と SSH 接続を確認してください。" "The upload failed. Check the free space in /tmp on the host and the SSH connection." }
        "deps" { Say "パッケージの導入に失敗しました。ホストからインターネットに出られるか (プロキシ設定)、apt が他の処理 (unattended-upgrades) にロックされていないか確認し、数分後に再実行してください。" "Installing packages failed. Check that the host reaches the internet (proxy?) and that apt is not locked by another process (unattended-upgrades); retry in a few minutes." }
        "build" { Say "ビルドに失敗しました。メモリ不足 (killed) ならスワップを追加してください。ネットワークエラーならプロキシを確認してください。最初からやり直すにはホスト上で rm -rf ~/wpg-build を実行します。" "The build failed. If rustc was killed, add swap. On network errors check the proxy. To start from scratch run rm -rf ~/wpg-build on the host." }
        "install" { Say "インストールに失敗しました。上のエラーを確認してください (バイナリがホストで動かない場合は、ホスト上でビルドしてください)。" "The installation failed; see the error above (if the binary does not run on the host, build it on the host)." }
        "health" { Say "wpg が応答しません。ホストで sudo journalctl -u wpg -n 50 を確認してください。ポートの重複は sudo ss -ltnp で確認できます。" "wpg does not answer. Check sudo journalctl -u wpg -n 50 on the host; sudo ss -ltnp shows a port already in use." }
    }
}

# ---- wizard ---------------------------------------------------------------------
function Start-Wizard {
    Say "wpg デプロイウィザード (Enter で [ ] 内の既定値を使います。Ctrl+C で中止)" "wpg deploy wizard (Enter accepts the default in [ ]; Ctrl+C aborts)" "Cyan"
    $usePrev = $false
    if (Test-Path $ProfilePath) {
        Merge-Answers (Read-AnswerFile $ProfilePath)
        $p = Get-Default "ssh_port" "22"
        Say "前回の設定: $($cfg.user)@$($cfg.host) (ポート $p, $(Get-Default 'bind' '0.0.0.0:8080'))" "Previous settings: $($cfg.user)@$($cfg.host) (port $p, $(Get-Default 'bind' '0.0.0.0:8080'))"
        $usePrev = Read-YesNo "前回の設定を使いますか?" "Use the previous settings?" $true
    }
    $script:usePrev = $usePrev

    # 1. connection
    if (-not ($usePrev -and $cfg.host)) {
        $cfg.host = Read-Required "接続先ホスト (IP アドレスまたはホスト名)" "Target host (IP address or name)" $cfg.host
        while ($true) {
            $v = Read-Text "SSH ポート" "SSH port" (Get-Default "ssh_port" "22")
            $n = 0
            if ([int]::TryParse($v, [ref]$n) -and $n -ge 1 -and $n -le 65535) { $cfg.ssh_port = "$n"; break }
            Say "  1〜65535 の数字を入力してください。" "  Enter a number from 1 to 65535."
        }
        $cfg.user = Read-Required "SSH ユーザ名" "SSH user" (Get-Default "user" "ubuntu")
        $c = Read-Choice "SSH の認証方法:" "SSH authentication:" @(
            "パスワード (初回だけ使い、デプロイ用の鍵を登録します)|Password (used once to install a deploy key)",
            "既存の秘密鍵ファイル|An existing private key file") 1
        if ($c -eq 2) {
            $cfg.identity = Read-Required "秘密鍵ファイルのパス" "Private key file" (Get-Default "identity" (Join-Path (Join-Path $HomeDir ".ssh") "id_ed25519"))
        } else {
            $cfg.identity = ""
        }
    }
}

function Start-WizardHost {
    $usePrev = $script:usePrev
    $ver = Get-Probe "WPG_VERSION"
    # 2. upgrade or rollback
    if ((Get-Probe "WPG_PREV") -eq "1") {
        $c = Read-Choice "wpg はインストール済みです ($ver)。何をしますか?" "wpg is installed ($ver). What do you want to do?" @(
            "インストール / アップグレード|Install / upgrade",
            "ロールバック (1 つ前のバイナリ wpg.prev に戻す)|Roll back (restore the previous binary wpg.prev)",
            "中止|Quit") 1
        if ($c -eq 3) { return $false }
        if ($c -eq 2) { $script:doRollback = $true }
    }
    if ($script:doRollback) { return $true }

    # 3. binary
    $internet = Get-Probe "INTERNET"
    $arch = Get-Probe "ARCH"
    if (-not $arch) { $arch = "x86_64" }
    if (-not ($usePrev -and $cfg.binary_source)) {
        $script:releaseUrl = Get-ReleaseAsset "latest" $arch
        $relNote = T "(まだリリースがありません)" "(no release yet)"
        if ($script:releaseUrl) { $relNote = "" }
        $def = 1
        if ($internet -eq "0") { $def = 2 }
        $c = Read-Choice "wpg のバイナリの用意:" "How to get the wpg binary:" @(
            "ホスト上でソースからビルド (推奨。ホストにインターネット接続が必要。初回 10〜30 分)|Build from source on the host (recommended; needs internet there; 10-30 min the first time)",
            "手元の Linux 用バイナリファイルをアップロード|Upload a Linux binary file you already have",
            "GitHub Release からダウンロード $relNote|Download from a GitHub Release $relNote") $def
        $cfg.binary_source = @("build", "file", "release")[$c - 1]
        if ($cfg.binary_source -eq "build") {
            if ($internet -eq "0") { Warn "ホストからインターネットに接続できないようです。ビルドは失敗する可能性があります。" "the host does not seem to reach the internet; the build will probably fail." }
            $def = 1
            if ($cfg.source -eq "git") { $def = 2 }
            $c = Read-Choice "ソースコード:" "Source code:" @(
                "このパソコンのソース ($Root) をアップロード|Upload this checkout ($Root)",
                "GitHub から git clone|git clone from GitHub") $def
            if ($c -eq 2) {
                $cfg.source = "git"
                $cfg.git_url = Read-Text "リポジトリ URL" "Repository URL" (Get-Default "git_url" $DefaultGitUrl)
                $cfg.git_ref = Read-Text "ブランチ / タグ" "Branch / tag" (Get-Default "git_ref" "main")
            } else {
                $cfg.source = "local"
            }
        }
    }
    if ($cfg.binary_source -eq "file") {
        $v = $cfg.binary_file
        while ($true) {
            $v = Read-Required "バイナリファイルのパス" "Path of the binary" $v
            $v = $v.Trim('"')
            if (Test-Path -PathType Leaf $v) { break }
            Say "  ファイルがありません: $v" "  No such file: $v"
            $v = ""
        }
        $cfg.binary_file = Get-FullPath $v
    }

    # 4. server settings
    if (-not $usePrev) {
        $svc = Read-YesNo "systemd サービスとしてインストールし、起動時に自動で開始しますか?" "Install as a systemd service (starts at boot)?" ((Get-Default "service" "1") -eq "1")
        if ($svc) { $cfg.service = "1" } else { $cfg.service = "0" }
        while ($true) {
            $v = Read-Text "Web UI の待ち受けアドレス:ポート (0.0.0.0 = すべて)" "Web UI listen address:port (0.0.0.0 = all)" (Get-Default "bind" "0.0.0.0:8080")
            if ($v -match '^.+:\d{1,5}$') { $cfg.bind = $v; break }
            Say "  <アドレス>:<ポート> の形で入力してください (例 0.0.0.0:8080)" "  Use <address>:<port> (e.g. 0.0.0.0:8080)"
        }
    }

    # 5. allowed interfaces
    if (-not $usePrev -or -not $cfg.ifaces) {
        $mgmt = Get-Probe "MGMT_IFACE"
        if ($script:probeIfaces.Count -gt 0) {
            Say "送信を許可するインタフェース (tx.allowed_ifaces):" "Interfaces jobs may send on (tx.allowed_ifaces):"
            $defs = @()
            $names = @()
            for ($i = 0; $i -lt $script:probeIfaces.Count; $i++) {
                $f = $script:probeIfaces[$i].Split("|")
                $name = $f[0]
                $state = ""
                $addrs = ""
                if ($f.Count -gt 1) { $state = $f[1] }
                if ($f.Count -gt 2) { $addrs = $f[2] }
                $names += $name
                $mark = ""
                if ($name -eq $mgmt) { $mark = T "  <- 管理用 (SSH 接続中)" "  <- management (this SSH session)" } else { $defs += ($i + 1) }
                Write-Ui ("  {0}) {1,-12} {2,-8} {3}{4}" -f ($i + 1), $name, $state, $addrs, $mark)
            }
            $defStr = ($defs -join ",")
            if (-not $defStr) { $defStr = "all" }
            while ($true) {
                $a = Read-Text "番号をカンマ区切りで (例 2,3。all = すべて許可)" "Numbers, comma separated (e.g. 2,3; all = no restriction)" $defStr
                if ($a -eq "all") { $cfg.ifaces = ""; break }
                $sel = @()
                $bad = $false
                foreach ($t in ($a -split '[,\s]+')) {
                    if (-not $t) { continue }
                    $n = 0
                    if ([int]::TryParse($t, [ref]$n) -and $n -ge 1 -and $n -le $names.Count) { $sel += $names[$n - 1] } else { $bad = $true }
                }
                if (-not $bad) { $cfg.ifaces = ($sel -join ","); break }
                Say "  1 から $($names.Count) の番号を入力してください。" "  Enter numbers from 1 to $($names.Count)."
            }
            if ($mgmt -and (",$($cfg.ifaces)," -like "*,$mgmt,*")) {
                Warn "管理用インタフェース $mgmt で送信を許可しました。SSH が切れる可能性があります。" "you allowed sending on the management interface $mgmt; this may cut your SSH session."
            }
        } else {
            $cfg.ifaces = (Read-Text "送信を許可するインタフェース名 (カンマ区切り。空 = すべて)" "Interfaces jobs may send on (comma separated; empty = all)" $cfg.ifaces) -replace '\s', ''
        }
    }

    # 6. optional features
    if (-not $usePrev -and $cfg.service -eq "1") {
        $t = Read-YesNo "Web ターミナルを有効にしますか? (対応する wpg のみ)" "Enable the web terminal? (newer wpg only)" ($cfg.terminal -eq "1")
        if ($t) { $cfg.terminal = "1" } else { $cfg.terminal = "0" }
        $t = Read-YesNo "SNMP トラップ受信 (UDP 162) を有効にしますか? (対応する wpg のみ)" "Enable the SNMP trap receiver (UDP 162)? (newer wpg only)" ($cfg.snmp_trap -eq "1")
        if ($t) { $cfg.snmp_trap = "1" } else { $cfg.snmp_trap = "0" }
    }
    return $true
}

function Read-AdminPassword([bool]$Optional) {
    while ($true) {
        if ($Optional) {
            $pw = Read-Secret "wpg の管理者パスワード (Enter = 変更しない / 設定しない)" "wpg admin password (Enter = keep / none)"
            if (-not $pw) { return "" }
        } else {
            $pw = Read-Secret "wpg の管理者パスワード (Web UI のログイン用)" "wpg admin password (Web UI login)"
            if (-not $pw) { Say "  入力してください。" "  Please enter a password."; continue }
        }
        $again = Read-Secret "もう一度入力してください" "Repeat it"
        if ($again -eq $pw) { return $pw }
        Say "  一致しません。" "  The passwords do not match."
    }
}

function Get-BinaryText {
    switch ($cfg.binary_source) {
        "build" {
            if ($cfg.source -eq "git") { return ((T "ホスト上でビルド" "build on the host") + " (git: $(Get-Default 'git_url' $DefaultGitUrl) $(Get-Default 'git_ref' 'main'))") }
            return ((T "ホスト上でビルド" "build on the host") + " (" + (T "このパソコンのソース" "this checkout") + ": $Root)")
        }
        "file" { return $cfg.binary_file }
        "release" { return "GitHub Release ($($script:ReleaseTagValue))" }
    }
    return ""
}

function Show-Summary {
    Write-Ui ""
    Say "確認" "Summary" "Cyan"
    $ifs = $cfg.ifaces
    if (-not $ifs) { $ifs = T "すべて (制限なし)" "all (no restriction)" }
    $rows = @(
        @((T "接続先" "Host"), "$($script:target) (port $(Get-Default 'ssh_port' '22'))"),
        @((T "バイナリ" "Binary"), (Get-BinaryText)),
        @((T "サービス" "Service"), $(if ($cfg.service -eq "1") { "systemd (wpg.service)" } else { T "なし" "no" })),
        @((T "待ち受け" "Listen"), (Get-Default "bind" "0.0.0.0:8080")),
        @((T "送信インタフェース" "TX interfaces"), $ifs),
        @((T "管理者パスワード" "Admin password"), $(if ($script:adminPw) { T "設定する" "set" } else { T "変更しない" "unchanged" })),
        @((T "Web ターミナル" "Web terminal"), $(if ($cfg.terminal -eq "1") { T "有効" "on" } else { T "無効" "off" })),
        @((T "SNMP トラップ受信" "SNMP traps"), $(if ($cfg.snmp_trap -eq "1") { T "有効 (UDP 162)" "on (UDP 162)" } else { T "無効" "off" })),
        @((T "DHCP サーバー" "DHCP server"), $(if (Get-Opt "dhcp_iface" "") { "{0} {1}" -f (Get-Opt "dhcp_iface" ""), (Get-Opt "dhcp_iface_addr" "") } else { T "画面で選択" "chosen on the screen" }))
    )
    foreach ($r in $rows) {
        $line = "  {0} : {1}" -f $r[0], $r[1]
        Write-Ui $line
        Write-Log $line
    }
}

# After a successful wizard run: an answer file for unattended runs, and how to use it.
function Save-Automation {
    Write-Ui ""
    if (-not (Read-YesNo "この設定で次回から自動実行するファイルを保存しますか? (パスワードは保存しません)" "Save a file to run this deploy unattended (automatically) from now on? (no passwords)" $true)) { return }
    $safe = $cfg.host -replace '[^A-Za-z0-9._-]', '_'
    $file = Read-Text "保存先" "File" (Join-Path (Split-Path -Parent $ProfilePath) "deploy-$safe.conf")
    $file = Get-FullPath $file.Trim('"')
    $extra = @()
    $sudoSaved = $false
    if ($script:sudoMode -eq "password" -and $script:sudoPw -and $script:OnWindows) {
        if (Read-YesNo "sudo パスワードを暗号化して保存しますか? (Windows の DPAPI。この Windows ユーザーがこの PC でだけ復号できます)" "Save the sudo password encrypted? (Windows DPAPI: only this Windows user on this PC can decrypt it)" $true) {
            $dp = Join-Path (Join-Path (Split-Path -Parent $file) "secrets") "$safe.sudo.dpapi"
            Save-DpapiSecret $dp $script:sudoPw
            $extra += "sudo_password_dpapi=$dp"
            $sudoSaved = $true
            Say "  暗号化して保存しました: $dp" "  Saved encrypted: $dp"
        }
    }
    Save-Profile $file -Automation -Extra $extra
    Say "保存しました: $file (パスワードは平文では含みません)" "Saved $file (no plain-text passwords)." "Green"
    $ps1 = Join-Path $Root "scripts\deploy.ps1"
    $cmd = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$ps1`" -ConfigFile `"$file`" -Yes -SkipIfCurrent"
    Say "次回からは、質問なしで次の 1 行で実行できます:" "From now on this one line runs the same deploy without questions:"
    Write-Ui "  $cmd"
    Write-Log "  $cmd"
    if ($script:sudoMode -eq "password" -and -not $sudoSaved) {
        Say "  * sudo にパスワードが必要なホストです。実行前に環境変数 WPG_DEPLOY_SUDO_PASSWORD を設定するか、sudo_password_file= / sudo_password_dpapi= を設定ファイルに書いてください。" "  * sudo needs a password on this host: set WPG_DEPLOY_SUDO_PASSWORD before running, or add sudo_password_file= / sudo_password_dpapi= to the file."
    }
    if ($cfg.binary_source -eq "build" -and $cfg.source -eq "local") {
        Say "  * source=local は実行時点のこのパソコンのソースをアップロードします。定期アップグレードには source=git がおすすめです。" "  * source=local uploads this checkout as it is at that time; for scheduled upgrades source=git is the better choice."
    }
    $tr = $cmd.Replace('"', '\"')
    Say "定期実行の例 (タスク スケジューラ、毎週日曜 3:17。ログは %LOCALAPPDATA%\wpg\deploy-logs に残ります):" "Scheduled run, e.g. Task Scheduler every Sunday at 3:17 (logs stay in %LOCALAPPDATA%\wpg\deploy-logs):"
    Write-Ui "  schtasks /Create /TN `"wpg deploy $safe`" /SC WEEKLY /D SUN /ST 03:17 /TR `"$tr`""
    Say "  (このユーザーで実行されます。DPAPI のファイルはこのユーザーでしか復号できません。詳しくは docs\ja\src\operations\deploy.md の「自動実行」)" "  (it runs as this user; DPAPI files only decrypt for this user. Details: docs\en\src\operations\deploy.md, `"Unattended deploy`")"
}

# ---- several hosts --------------------------------------------------------------------
# Every host runs in its own PowerShell process (deploy.ps1 -Yes -ResultFile ...), up to
# -Parallel at a time; then a table and, with -Json, the JSON summary.
function Invoke-Multi {
    $entries = @()
    if ($Targets) {
        if (-not (Test-Path -LiteralPath $Targets -PathType Leaf)) { Fail-Config "ターゲットファイルを読めません: $Targets" "cannot read the targets file: $Targets" }
        foreach ($raw in [System.IO.File]::ReadAllLines($Targets)) {
            $line = $raw.Trim()
            if (-not $line -or $line.StartsWith("#")) { continue }
            $f = $line -split '\s+'
            if ($f[0] -notmatch '^(?:([^@]+)@)?([^:@]+)(?::(\d+))?$') { Fail-Config "ターゲットの形式が違います: $($f[0])" "bad target: $($f[0])" }
            $hostArgs = @("-HostName", $Matches[2])
            if ($Matches[1]) { $hostArgs += @("-User", $Matches[1]) }
            if ($Matches[3]) { $hostArgs += @("-Port", $Matches[3]) }
            $sets = @("name=$($f[0])")
            for ($i = 1; $i -lt $f.Count; $i++) {
                $j = $f[$i].IndexOf("=")
                if ($j -lt 1) { Fail-Config "ターゲットのホストの後は key=value です: $($f[$i])" "expected key=value after the host: $($f[$i])" }
                $k = $f[$i].Substring(0, $j)
                $v = $f[$i].Substring($j + 1)
                # Relative paths: relative to the targets file.
                if ($PathKeys -contains $k) { $v = Resolve-AnswerPath $v (Split-Path -Parent $Targets) }
                $sets += "$k=$v"
            }
            $entries += , @{ Name = $f[0]; Args = $hostArgs; Sets = $sets }
        }
    } else {
        foreach ($s in (Get-AnswerSections $ConfigFile)) { $entries += , @{ Name = $s; Args = @("-Section", $s); Sets = @() } }
    }
    if ($entries.Count -eq 0) { Fail-Config "デプロイ先のホストがありません" "no hosts to deploy to" }
    if (-not $Yes -and -not $CheckConfig) {
        if (-not (Test-CanPrompt)) { Fail-Config "対話できない実行です。質問なしで実行するには -Yes を付けてください" "not an interactive session: add -Yes to run without questions" }
        Say "デプロイ先 ($($entries.Count) 台):" "Hosts ($($entries.Count)):"
        foreach ($e in $entries) { Write-Ui "  $($e.Name)" }
        if (-not (Read-YesNo "実行しますか?" "Go ahead?" $true)) { Stop-Run "aborted" }
    }
    # Options passed on to every host run.
    $common = @("-Yes", "-Lang", $script:Lang)
    if ($ConfigFile) { $common += @("-ConfigFile", $ConfigFile) }
    $skip = @("Targets", "Parallel", "Json", "ResultFile", "Section", "ConfigFile", "LogFile", "Yes", "Interactive", "Lang",
        "SshPassword", "SudoPassword", "AdminPassword", "Set", "HostName", "User", "Port")
    foreach ($kv in $script:BoundParams.GetEnumerator()) {
        if ($skip -contains $kv.Key) { continue }
        $v = $kv.Value
        if ($v -is [System.Management.Automation.SwitchParameter]) {
            if ($v.IsPresent) { $common += "-$($kv.Key)" }
        } elseif ($v -is [array]) {
            $common += @("-$($kv.Key)", ($v -join ","))
        } else {
            $common += @("-$($kv.Key)", [string]$v)
        }
    }
    # Secrets given as parameters reach the host runs through their environment.
    $savedEnv = @{}
    foreach ($pair in @(@("SshPassword", "WPG_DEPLOY_SSH_PASSWORD"), @("SudoPassword", "WPG_DEPLOY_SUDO_PASSWORD"), @("AdminPassword", "WPG_DEPLOY_ADMIN_PASSWORD"), @("", "WPG_DEPLOY_CHILD_SET"))) {
        $savedEnv[$pair[1]] = [Environment]::GetEnvironmentVariable($pair[1], "Process")
        if ($pair[0] -and $script:BoundParams.ContainsKey($pair[0])) {
            [Environment]::SetEnvironmentVariable($pair[1], (ConvertFrom-Secure $script:BoundParams[$pair[0]]), "Process")
        }
    }
    $work = Join-Path ([System.IO.Path]::GetTempPath()) ("wpg-multi-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    $logDir = Join-Path (Split-Path -Parent $script:LogPath) ("deploy-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    $psExe = (Get-Process -Id $PID).Path
    $total = $entries.Count
    $running = @()
    $procs = @{}
    try {
        for ($i = 0; $i -lt $total; $i++) {
            $e = $entries[$i]
            while ($running.Count -ge $Parallel) {
                Start-Sleep -Milliseconds 500
                $still = @()
                foreach ($j in $running) {
                    if ($procs[$j].HasExited) { Say "[$($j + 1)/$total] $($entries[$j].Name): 終了" "[$($j + 1)/$total] $($entries[$j].Name): finished" }
                    else { $still += $j }
                }
                $running = $still
            }
            $log = Join-Path $logDir (($e.Name -replace '[^A-Za-z0-9._@-]', '_') + ".log")
            $e["Log"] = $log
            $a = @("-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $PSCommandPath) + $common + $e.Args +
                @("-ResultFile", (Join-Path $work "$i.json"), "-LogFile", $log)
            # -Set values (the parent's and this target's) through the environment: an
            # array parameter cannot be passed to -File.
            [Environment]::SetEnvironmentVariable("WPG_DEPLOY_CHILD_SET", ((@($Set) + $e.Sets) -join "`n"), "Process")
            $argLine = (($a | ForEach-Object { ConvertTo-WinArg ([string]$_) }) -join " ")
            $p = Start-Process -FilePath $psExe -ArgumentList $argLine -NoNewWindow -PassThru `
                -RedirectStandardOutput (Join-Path $work "$i.out") -RedirectStandardError (Join-Path $work "$i.err")
            $null = $p.Handle
            $procs[$i] = $p
            $running += $i
            Say "[$($i + 1)/$total] $($e.Name): 開始 (ログ $log)" "[$($i + 1)/$total] $($e.Name): started (log $log)"
        }
        foreach ($j in $running) {
            $procs[$j].WaitForExit()
            Say "[$($j + 1)/$total] $($entries[$j].Name): 終了" "[$($j + 1)/$total] $($entries[$j].Name): finished"
        }
    } finally {
        foreach ($k in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($k, $savedEnv[$k], "Process") }
    }

    if ($CheckConfig) {
        for ($i = 0; $i -lt $total; $i++) {
            Write-Ui ""
            Write-Ui "== $($entries[$i].Name) ==" "Cyan"
            $o = Join-Path $work "$i.out"
            if (Test-Path -LiteralPath $o) { foreach ($l in [System.IO.File]::ReadAllLines($o)) { Write-Ui $l } }
        }
    }

    # Summary.
    Write-Ui ""
    Say "結果" "Result" "Cyan"
    $w = 4
    foreach ($e in $entries) { if ($e.Name.Length -gt $w) { $w = $e.Name.Length } }
    $fmt = "  {0,-$w} {1,-9} {2,-4} {3,-12} {4}"
    Write-Ui ($fmt -f "NAME", "STATUS", "EXIT", "VERSION", "DETAIL")
    $objs = @()
    $nOk = 0
    $nFail = 0
    for ($i = 0; $i -lt $total; $i++) {
        $e = $entries[$i]
        $rf = Join-Path $work "$i.json"
        $status = "failed"
        $code = 1
        $ver = ""
        $detail = T "結果がありません (ログを確認)" "no result (see the log)"
        $raw = ""
        if (Test-Path -LiteralPath $rf) { $raw = ([System.IO.File]::ReadAllText($rf)).Trim() }
        if ($raw) {
            $o = $raw | ConvertFrom-Json
            $status = [string]$o.status
            $code = [int]$o.exit_code
            $ver = [string]$o.version_after
            if (-not $ver) { $ver = [string]$o.version_before }
            $detail = [string]$o.message
            if (-not $detail) { $detail = [string]$o.source_id }
            $objs += $raw
        } else {
            try { if ($procs[$i].ExitCode -ne 0) { $code = $procs[$i].ExitCode } } catch { }
            $objs += (ConvertTo-JsonObject @("name", $e.Name, "status", "failed", "exit_code", $code, "message", $detail, "log", [string]$e["Log"]))
        }
        if ($status -in @("ok", "skipped", "dry-run", "checked")) { $nOk++ } else { $nFail++ }
        if ($detail.Length -gt 60) { $detail = $detail.Substring(0, 57) + "..." }
        $line = $fmt -f $e.Name, $status, $code, ($ver -replace '^wpg ', ''), $detail
        Write-Ui $line
        Write-Log $line
    }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    if (-not $CheckConfig) { Say "ログ: $logDir" "logs: $logDir" }
    Say "成功 $nOk 台、失敗 $nFail 台" "$nOk succeeded, $nFail failed"
    $rc = 0
    if ($nFail -gt 0) { $rc = 6 }
    if ($Json) {
        $st = "ok"
        if ($rc -ne 0) { $st = "failed" }
        [Console]::Out.WriteLine('{"tool":"wpg-deploy","format":1,"status":' + (ConvertTo-JsonText $st) + ',"exit_code":' + $rc +
            ',"dry_run":' + (ConvertTo-JsonText ([bool]$DryRun)) + ',"message":"","hosts":[' + ($objs -join ",") + ']}')
    }
    $script:MultiDone = $true
    return $rc
}

function Set-Defaults {
    if (-not $cfg.user) { $cfg.user = "ubuntu" }
    if (-not $cfg.binary_source) { $cfg.binary_source = "build" }
    if (-not $cfg.source) { $cfg.source = "local" }
    if (-not $cfg.git_url) { $cfg.git_url = $DefaultGitUrl }
    if (-not $cfg.git_ref) { $cfg.git_ref = "main" }
    if (-not $cfg.bind) { $cfg.bind = "0.0.0.0:8080" }
    if (-not $cfg.service) { $cfg.service = "1" }
    if (-not $cfg.terminal) { $cfg.terminal = "0" }
    if (-not $cfg.snmp_trap) { $cfg.snmp_trap = "0" }
    if ($cfg.host -match '^(.+)@(.+)$') { $cfg.user = $Matches[1]; $cfg.host = $Matches[2] }
}

# ---- main -------------------------------------------------------------------------
$script:BoundParams = $PSBoundParameters
$script:usePrev = $false
$script:releaseUrl = $null
$script:InstallPrefix = "/usr/local"
$script:ReleaseTagValue = "latest"
$exitCode = 0

if (-not $LogFile) {
    $base = $env:LOCALAPPDATA
    if (-not $base) { $base = Join-Path $HomeDir ".local/state" }
    $LogFile = Join-Path (Join-Path (Join-Path $base "wpg") "deploy-logs") ("deploy-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".log")
}
$LogFile = Get-FullPath $LogFile

try {
    $logDir = Split-Path -Parent $LogFile
    if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Force -Path $logDir | Out-Null }
    $script:LogPath = $LogFile
    Write-Log "# wpg deploy $(Get-Date -Format s) (PowerShell $($PSVersionTable.PSVersion))"

    # ---- answer file ----
    $script:StepId = "config"
    if ($ConfigFile -and $Interactive) {
        $ProfilePath = $ConfigFile
    } elseif ($ConfigFile -and -not $Targets -and -not $Section -and -not $HostName -and (Get-AnswerSections $ConfigFile).Count -gt 0) {
        $exitCode = Invoke-Multi
        Stop-Run "multi"
    } elseif ($ConfigFile) {
        Merge-Answers (Read-AnswerFile $ConfigFile $Section)
    }
    if ($Targets) {
        if ($HostName) { Fail-Config "-Targets と -HostName は同時に使えません" "-Targets cannot be combined with -HostName" }
        $exitCode = Invoke-Multi
        Stop-Run "multi"
    }
    # -Set, and the per-host values of a multi-host run (WPG_DEPLOY_CHILD_SET, internal).
    $sets = @($Set)
    if ($ResultFile -and $env:WPG_DEPLOY_CHILD_SET) { $sets += ($env:WPG_DEPLOY_CHILD_SET -split "`n") }
    $setAnswers = [ordered]@{}
    foreach ($s in $sets) {
        if (-not $s) { continue }
        $i = $s.IndexOf("=")
        if ($i -lt 1) { Fail-Config "-Set は key=value です: $s" "-Set needs key=value: $s" }
        $k = $s.Substring(0, $i).Trim()
        if ($AnswerKeys -notcontains $k) { Fail-Config "不明なキー '$k' (-Set)" "unknown key '$k' (-Set)" }
        $v = ConvertTo-AnswerValue $k ($s.Substring($i + 1).Trim()) ((Get-Location).ProviderPath) "-Set"
        if ($k -eq "port") { $k = "ssh_port" }
        $setAnswers[$k] = $v
    }
    if ($setAnswers.Count -gt 0) { Merge-Answers $setAnswers -Force }
    if (-not $BoundParams.ContainsKey("Lang") -and (Get-Opt "lang" "")) { $script:Lang = Get-Opt "lang" "" }
    $script:InstallPrefix = Get-Opt "prefix" "/usr/local"
    $script:ReleaseTagValue = Get-Opt "release_tag" "latest"
    if ((Get-Opt "dpdk" "0") -eq "1") { Warn "dpdk は deploy.sh だけで使えます。無視します。" "dpdk is only supported by deploy.sh; ignored." }

    $script:wizard = ($Interactive -or (-not $cfg.host -and -not $ConfigFile))
    if (-not $script:wizard -and -not $Yes -and -not $CheckConfig -and -not (Test-CanPrompt)) {
        Fail-Config "対話できない実行です。質問なしで実行するには -Yes を付けてください" "not an interactive session: add -Yes to run without questions"
    }

    if (-not $script:wizard) {
        if (-not $cfg.host) { Fail-Config "接続先がありません (-HostName、設定ファイルの host=、または -Targets)" "no host: give -HostName, host= in the answer file, or -Targets" }
        Set-Defaults
    }
    if (-not $script:wizard -and $cfg.binary_source -eq "file") {
        if (-not $cfg.binary_file) { Fail-Config "binary_source=file には binary_file= (-Binary) が必要です" "binary_source=file needs binary_file= (-Binary)" }
        if (-not (Test-Path -LiteralPath $cfg.binary_file -PathType Leaf)) { Fail-Config "バイナリがありません: $($cfg.binary_file)" "binary not found: $($cfg.binary_file)" }
    }

    if ($CheckConfig) {
        $name = Get-Opt "name" $cfg.host
        $src = @{}
        foreach ($pair in @(@("ssh_password", "WPG_DEPLOY_SSH_PASSWORD"), @("sudo_password", "WPG_DEPLOY_SUDO_PASSWORD"), @("admin_password", "WPG_DEPLOY_ADMIN_PASSWORD"))) {
            $s = "-"
            if ([Environment]::GetEnvironmentVariable($pair[1], "Process")) { $s = "`$env:$($pair[1])" }
            elseif (Get-Opt "$($pair[0])_file" "") { $s = Get-Opt "$($pair[0])_file" "" }
            elseif (Get-Opt "$($pair[0])_dpapi" "") { $s = "DPAPI " + (Get-Opt "$($pair[0])_dpapi" "") }
            $src[$pair[0]] = $s
        }
        $ifs = $cfg.ifaces
        if (-not $ifs) { $ifs = "all" }
        $idv = $cfg.identity
        if (-not $idv) { $idv = "-" }
        $rows = @(@("name", $name), @("host", "$($cfg.user)@$($cfg.host)"), @("ssh_port", (Get-Default "ssh_port" "22")),
            @("identity", $idv), @("binary", (Get-BinaryText)), @("bind", $cfg.bind), @("ifaces", $ifs),
            @("service", $cfg.service), @("terminal", $cfg.terminal), @("snmp_trap", $cfg.snmp_trap),
            @("templates", (Get-Opt "templates" "1")), @("healthcheck", (Get-Opt "healthcheck" "1")), @("prefix", $script:InstallPrefix),
            @("ssh_password", $src["ssh_password"]), @("sudo_password", $src["sudo_password"]), @("admin_password", $src["admin_password"]))
        foreach ($r in $rows) { Write-Ui ("  {0,-16} {1}" -f $r[0], $r[1]) }
        Stop-Run "checked"
    }

    $script:StepId = "tools"
    $ssh = Find-Tool "ssh"
    $scp = Find-Tool "scp"
    if (-not $ssh -or -not $scp) { Fail "ssh / scp が見つかりません (OpenSSH クライアントが必要です)" "ssh / scp not found (the OpenSSH client is required)" }
    New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

    if ($script:wizard) {
        # The saved answers are loaded first, then the defaults fill what is still empty.
        Start-Wizard
        Set-Defaults
    }
    $target = "$($cfg.user)@$($cfg.host)"
    Set-SshBase ""
    if ($cfg.bind -notmatch '^(.+):(\d{1,5})$') { Fail-Config "待ち受けアドレスは <アドレス>:<ポート> で指定してください: $($cfg.bind)" "the listen address must be <address>:<port>: $($cfg.bind)" }
    $bindAddr = $Matches[1]
    $bindPort = $Matches[2]
    foreach ($i in ($cfg.ifaces -split ",")) {
        if ($i -and $i -notmatch '^[A-Za-z0-9_.:@-]+$') { Fail-Config "インタフェース名が不正です: $i" "invalid interface name: $i" }
    }
    $hc = $bindAddr
    if ($hc -in @("0.0.0.0", "[::]", "::", "")) { $hc = "127.0.0.1" }

    # ---- connect ----
    Step "connect" "$target に接続" "Connecting to $target"
    if ($null -ne $SshPassword) { $sshPw = ConvertFrom-Secure $SshPassword }
    Initialize-SshKey
    Initialize-Stage
    $osName = Get-Probe "OS_NAME"
    $script:VersionBefore = Get-Probe "WPG_VERSION"
    $verText = $script:VersionBefore
    if (-not $verText) { $verText = "-" }
    Say "  OS: $osName ($(Get-Probe 'ARCH'))   sudo: $(Get-Probe 'SUDO')   wpg: $verText" "  OS: $osName ($(Get-Probe 'ARCH'))   sudo: $(Get-Probe 'SUDO')   wpg: $verText"
    if ((Get-Probe "KERNEL") -and (Get-Probe "KERNEL") -ne "Linux") { Fail "Linux ホストにだけデプロイできます" "only Linux hosts are supported" }
    if ((Get-Probe "OS_ID") -notin @("ubuntu", "debian", "")) {
        Warn "$osName は Ubuntu / Debian 以外です。できる範囲で進めます。" "$osName is not Ubuntu / Debian; continuing on a best-effort basis."
    }

    if ($script:wizard) {
        if (-not (Start-WizardHost)) { Say "中止しました。" "Aborted."; Stop-Run "aborted" }
    }

    # ---- nothing to do? (before sudo, so a scheduled run with nothing to do needs no password) ----
    if (-not $doRollback) {
        $script:SourceId = Get-SourceId
        if (-not $script:wizard -and (Get-Opt "skip_if_current" "0") -eq "1") {
            if (Test-HostCurrent $hc $bindPort) {
                $script:VersionAfter = $script:VersionBefore
                $script:StepId = ""
                Say "$target はすでに最新です ($($script:SourceId), $verText)。何もしません。" "$target is already current ($($script:SourceId), $verText); nothing to do." "Green"
                Stop-Run "skipped"
            }
        }
    }

    # ---- sudo ----
    switch (Get-Probe "SUDO") {
        "root" { $sudoMode = "root" }
        "nopasswd" { $sudoMode = "nopasswd" }
        "none" { $script:StepId = "sudo"; Fail "ホストに sudo がありません。root でログインするか sudo を入れてください。" "sudo is missing on the host; log in as root or install sudo." }
        default {
            $sudoMode = "password"
            $script:StepId = "sudo"
            if ($null -ne $SudoPassword) { $sudoPw = ConvertFrom-Secure $SudoPassword }
            if (-not $sudoPw) { $sudoPw = Get-SecretValue "WPG_DEPLOY_SUDO_PASSWORD" "sudo_password" }
            $script:StepId = "sudo"
            for ($try = 1; $try -le 3; $try++) {
                if (-not $sudoPw) {
                    if (Test-CanPrompt) {
                        if ($sshPw) {
                            $sudoPw = Read-Secret "sudo パスワード (Enter = SSH パスワードと同じ)" "sudo password (Enter = same as the SSH password)"
                            if (-not $sudoPw) { $sudoPw = $sshPw }
                        } else {
                            $sudoPw = Read-Secret "sudo パスワード" "sudo password"
                        }
                    } elseif ($sshPw) {
                        $sudoPw = $sshPw
                    } else {
                        Fail "$target の sudo にはパスワードが必要です。環境変数 WPG_DEPLOY_SUDO_PASSWORD か sudo_password_file= / sudo_password_dpapi= を設定するか、ホストで NOPASSWD を設定してください" "sudo on $target needs a password: set WPG_DEPLOY_SUDO_PASSWORD or sudo_password_file= / sudo_password_dpapi=, or allow NOPASSWD for this user on the host"
                    }
                }
                $r = Invoke-Ssh "sudo -S -p '' true" $sudoPw -Quiet
                if ($r.ExitCode -eq 0) { break }
                $sudoPw = ""
                if ($try -eq 3 -or -not (Test-CanPrompt)) { Fail "sudo に失敗しました (パスワードが違うか、sudo の権限がありません)" "sudo failed (wrong password, or the user may not use sudo)" }
                Say "  sudo パスワードが違います。もう一度入力してください。" "  Wrong sudo password; try again."
            }
            $script:StepId = "connect"
        }
    }

    # ---- admin password, summary ----
    if (-not $doRollback -and $cfg.service -eq "1") {
        if ($null -ne $AdminPassword) {
            $adminPw = ConvertFrom-Secure $AdminPassword
        } else {
            $adminPw = Get-SecretValue "WPG_DEPLOY_ADMIN_PASSWORD" "admin_password"
            if (-not $adminPw) {
                $loop = ($bindAddr -match '^(127\.|localhost$|\[::1\]$)')
                $installed = ((Get-Probe "WPG_VERSION") -and (Get-Probe "SERVICE") -ne "absent")
                $optional = ($installed -or $loop -or (Get-Opt "allow_insecure" "0") -eq "1")
                if (Test-CanPrompt) {
                    if ($script:wizard -or -not $installed) { $adminPw = Read-AdminPassword $optional }
                } elseif (-not $optional -and -not $DryRun) {
                    Fail-Config "$target は初回のインストールなので管理者パスワードが必要です。環境変数 WPG_DEPLOY_ADMIN_PASSWORD か admin_password_file= / admin_password_dpapi= を設定するか、bind=127.0.0.1:$bindPort にしてください" "$target is a first install and needs an admin password: set WPG_DEPLOY_ADMIN_PASSWORD or admin_password_file= / admin_password_dpapi=, or use bind=127.0.0.1:$bindPort"
                }
            }
        }
    }
    if ($doRollback) {
        if (Test-CanPrompt) {
            if (-not (Read-YesNo "/usr/local/bin/wpg と wpg.prev を入れ替えてサービスを再起動します。よろしいですか?" "Swap /usr/local/bin/wpg with wpg.prev and restart the service?" $true)) { Say "中止しました。" "Aborted."; Stop-Run "aborted" }
        }
        # The listen address is only known from the saved answers, -Bind or the answer file.
        $rollbackHealth = ((Get-Probe "SERVICE") -ne "absent" -and ($script:usePrev -or $Bind -or $ConfigFile))
    } else {
        if (Test-CanPrompt) {
            Show-Summary
            if (-not (Read-YesNo "この内容で実行しますか?" "Go ahead?" $true)) { Say "中止しました。" "Aborted."; Stop-Run "aborted" }
        }
        if ($script:wizard) {
            if (Read-YesNo "この設定を次回のために保存しますか? (パスワードは保存しません)" "Save these answers for next time? (no passwords)" $true) { Save-Profile $ProfilePath }
        }
    }
    Say "ログ: $LogFile" "log: $LogFile"

    $build = (-not $doRollback -and $cfg.binary_source -eq "build")
    $templatesOn = ((Get-Opt "templates" "1") -eq "1")
    $health = ((Get-Opt "healthcheck" "1") -eq "1" -and (($cfg.service -eq "1" -and -not $doRollback) -or $rollbackHealth))
    $script:StepTotal = $script:StepNo + 2
    if ($build) { $script:StepTotal += 2 }
    if ($health) { $script:StepTotal += 1 }

    # ---- upload ----
    Step "upload" "アップロード ($($target):$stage)" "Uploading to $($target):$stage"
    $remoteArgs = @("--stage", $stage, "--bind", $cfg.bind, "--prefix", $script:InstallPrefix)
    if (-not $doRollback) {
        if ($templatesOn) {
            Invoke-Upload @((Join-Path $Root "templates")) "$stage/templates" -Recurse
            $remoteArgs += @("--templates", "$stage/templates")
        }
        if ($adminPw) {
            if ($DryRun) { Say "  (dry run) 管理者パスワードを送信" "  (dry run) send the admin password" }
            else { $null = Invoke-SshChecked "umask 077; cat >$stage/password" $adminPw }
            $remoteArgs += @("--password-file", "$stage/password")
        }
        if ($build -and $cfg.source -eq "local") {
            $tarball = Join-Path $tmpDir "src.tar.gz"
            $git = Find-Tool "git"
            $packed = $false
            if ($git -and (Test-Path (Join-Path $Root ".git"))) {
                Say "  ソースを固めています (git archive)" "  Packing the source (git archive)"
                $rev = (Invoke-Native -FilePath $git -ArgumentList @("-C", $Root, "stash", "create") -Quiet).Output
                $ref = "HEAD"
                if ($rev -and $rev.Count -gt 0 -and $rev[0]) { $ref = $rev[0].Trim() }
                $r = Invoke-Native -FilePath $git -ArgumentList @("-C", $Root, "-c", "core.autocrlf=false", "archive", "--format=tar.gz", "-o", $tarball, $ref)
                $packed = ($r.ExitCode -eq 0)
            }
            if (-not $packed) {
                $tar = Find-Tool "tar"
                if (-not $tar) { $script:StepId = "binary"; Fail "git も tar.exe もありません。GitHub から git clone を選んでください。" "neither git nor tar.exe found; choose git clone from GitHub." }
                Say "  ソースを固めています (tar)" "  Packing the source (tar)"
                $r = Invoke-Native -FilePath $tar -ArgumentList @("-czf", $tarball, "--exclude=target", "--exclude=.git", "--exclude=node_modules", "--exclude=dist", "-C", $Root, ".")
                if ($r.ExitCode -ne 0) { $script:StepId = "binary"; Fail "ソースを固められませんでした" "could not pack the source" }
            }
            Invoke-Upload @($tarball) "$stage/src.tar.gz"
        }
        if ($cfg.binary_source -eq "file") {
            $bytes = New-Object byte[] 4
            $fs = [System.IO.File]::OpenRead($cfg.binary_file)
            try { [void]$fs.Read($bytes, 0, 4) } finally { $fs.Close() }
            if (-not ($bytes[0] -eq 0x7F -and $bytes[1] -eq 0x45 -and $bytes[2] -eq 0x4C -and $bytes[3] -eq 0x46)) {
                $script:StepId = "binary"
                Fail "$($cfg.binary_file) は Linux 用 (ELF) の実行ファイルではありません (wpg.exe は使えません)" "$($cfg.binary_file) is not a Linux (ELF) executable (wpg.exe does not work)"
            }
            Invoke-Upload @($cfg.binary_file) "$stage/wpg"
        }
        if ($cfg.binary_source -eq "release") {
            $arch = Get-Probe "ARCH"
            if (-not $arch) { $arch = "x86_64" }
            $url = $script:releaseUrl
            if (-not $url) { $url = Get-ReleaseAsset $script:ReleaseTagValue $arch }
            if (-not $url) { $script:StepId = "binary"; Fail "GitHub Release ($($script:ReleaseTagValue)) に Linux ($arch) 用のアーカイブがありません (リリースはまだありません)。ホスト上でのビルドを選んでください。" "no Linux ($arch) archive in the GitHub Release ($($script:ReleaseTagValue)) (there are no releases yet); build on the host instead." }
            $archive = Join-Path $tmpDir "wpg.tar.gz"
            Say "  ダウンロード: $url" "  downloading $url"
            Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing
            $haveSum = $false
            try {
                Invoke-WebRequest -Uri "$url.sha256" -OutFile "$archive.sha256" -UseBasicParsing
                $haveSum = $true
            } catch {
                Warn "チェックサムファイルがありません" "no checksum file"
            }
            if ($haveSum) {
                $want = (([System.IO.File]::ReadAllText("$archive.sha256")).Trim() -split '\s+')[0].ToLower()
                $got = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLower()
                if ($want -ne $got) { $script:StepId = "binary"; Fail "チェックサムが一致しません: $url" "checksum mismatch: $url" }
                Say "  sha256 ok" "  sha256 ok"
            }
            Invoke-Upload @($archive) "$stage/release.tar.gz"
            $null = Invoke-SshChecked "mkdir -p $stage/release && tar -xzf $stage/release.tar.gz -C $stage/release && cp $stage/release/*/wpg $stage/wpg"
        }
    }

    # ---- build on the host ----
    if ($build) {
        Step "deps" "ビルドに必要なパッケージの導入 (apt)" "Installing the build dependencies (apt)"
        if ($DryRun) { Say "  (dry run) sudo bash remote-build.sh --deps" "  (dry run) sudo bash remote-build.sh --deps" }
        else { $null = Invoke-Root "bash $stage/remote-build.sh --deps" }
        Step "build" "ホスト上でビルド (初回は 10〜30 分かかります)" "Building on the host (10-30 minutes the first time)"
        $b = "bash $stage/remote-build.sh --build --out $stage/wpg"
        if ($cfg.source -eq "local") { $b += " --tarball $stage/src.tar.gz" }
        else { $b += " --git-url " + (ConvertTo-ShArg $cfg.git_url) + " --git-ref " + (ConvertTo-ShArg $cfg.git_ref) }
        if ($DryRun) { Say "  (dry run) $b" "  (dry run) $b" }
        else { $null = Invoke-SshChecked $b }
    }

    # ---- install ----
    Step "install" "インストール (sudo)" "Installing (sudo)"
    if ($doRollback) {
        $remoteArgs += "--rollback"
    } else {
        $remoteArgs += @("--binary", "$stage/wpg")
        # Recorded on the host for -SkipIfCurrent.
        $remoteArgs += @("--stamp-source", $script:SourceId, "--stamp-settings", (Get-SettingsId), "--stamp-tool", "deploy.ps1")
    }
    if ($cfg.service -eq "1" -and -not $doRollback) { $remoteArgs += "--service" }
    if ($cfg.terminal -eq "1" -and -not $doRollback) { $remoteArgs += "--terminal" }
    if ($cfg.snmp_trap -eq "1" -and -not $doRollback) { $remoteArgs += "--snmp-trap" }
    $dhcpIf = Get-Opt "dhcp_iface" ""
    if ($dhcpIf -and -not $doRollback) {
        if ($dhcpIf -notmatch '^[A-Za-z0-9_.:@-]+$') { throw "invalid dhcp_iface: $dhcpIf" }
        $remoteArgs += @("--dhcp-iface", $dhcpIf)
        foreach ($a in ((Get-Opt "dhcp_iface_addr" "") -split "[, ]+")) {
            if (-not $a) { continue }
            if ($a -notmatch '^[0-9A-Fa-f.:]+/[0-9]+$') { throw "invalid dhcp_iface_addr: $a" }
            $remoteArgs += @("--dhcp-iface-addr", $a)
        }
    }
    if ((Get-Opt "allow_insecure" "0") -eq "1") { $remoteArgs += "--allow-insecure" }
    if (-not $doRollback) {
        foreach ($i in ($cfg.ifaces -split ",")) { if ($i) { $remoteArgs += @("--iface", $i) } }
    }
    $cmd = "bash $stage/deploy-remote.sh " + (($remoteArgs | ForEach-Object { ConvertTo-ShArg $_ }) -join " ")
    if ($DryRun) { Say "  (dry run) sudo $cmd" "  (dry run) sudo $cmd" }
    else {
        $null = Invoke-Root $cmd
        $vr = Invoke-Ssh ((ConvertTo-ShArg "$($script:InstallPrefix)/bin/wpg") + " --version") -Quiet
        if ($vr.ExitCode -eq 0 -and $vr.Output.Count -gt 0) { $script:VersionAfter = $vr.Output[0].Trim() }
    }

    # ---- health ----
    if ($health) {
        Step "health" "ヘルスチェック (http://$($hc):$bindPort/api/health)" "Health check (http://$($hc):$bindPort/api/health)"
        if (-not $DryRun) { $null = Invoke-SshChecked ("bash $stage/remote-health.sh " + (ConvertTo-ShArg $hc) + " $bindPort 30") }
    }
    $script:StepId = ""
    Write-Ui ""
    if ($doRollback) {
        Say "ロールバックしました。" "Rolled back." "Green"
    } elseif ($cfg.service -ne "1") {
        Say "完了しました (サービスは入れていません。ホストで wpg serve を実行してください)。" "Done (no service installed; run wpg serve on the host)." "Green"
    } elseif ($bindAddr -match '^(127\.|localhost$|\[::1\]$)') {
        $script:DeployedUrl = "http://127.0.0.1:$bindPort/"
        Say "完了しました。wpg はホストのループバックで待ち受けています。SSH トンネル経由で開いてください:" "Done. wpg listens on the host's loopback; open it through an SSH tunnel:" "Green"
        Say "  ssh -L $($bindPort):127.0.0.1:$bindPort -p $(Get-Default 'ssh_port' '22') $target   ->  http://127.0.0.1:$bindPort/" "  ssh -L $($bindPort):127.0.0.1:$bindPort -p $(Get-Default 'ssh_port' '22') $target   ->  http://127.0.0.1:$bindPort/"
    } else {
        $script:DeployedUrl = "http://$($cfg.host):$bindPort/"
        Say "完了しました。ブラウザで開いてください:" "Done. Open in a browser:" "Green"
        Say "  http://$($cfg.host):$bindPort/" "  http://$($cfg.host):$bindPort/" "Green"
        if ($adminPw -or -not (Get-Probe "WPG_VERSION")) { Say "  (ユーザ名 admin と、設定した管理者パスワードでログインします)" "  (log in as admin with the admin password you set)" }
    }
    if ($DryRun) { $script:ResultStatus = "dry-run" }
    if ($script:wizard -and -not $DryRun -and -not $doRollback) {
        try { Save-Automation } catch { Warn "自動実行用のファイルを保存できませんでした: $($_.Exception.Message)" "could not save the file for unattended runs: $($_.Exception.Message)" }
    }
} catch {
    if ($_.Exception.Message -ne $StopMarker) {
        $script:ResultStatus = "failed"
        $script:FailedStep = $script:StepId
        $script:FailMessage = $_.Exception.Message
        $exitCode = Get-StepExitCode $script:StepId
        Write-Ui ""
        $where = ""
        if ($script:StepNo -gt 0) { $where = T " (ステップ $($script:StepNo): $($script:StepName))" " (step $($script:StepNo): $($script:StepName))" }
        $msg = (T "失敗しました" "FAILED") + $where + ": " + $_.Exception.Message
        Write-Ui $msg "Red"
        Write-Log $msg
        if ($script:Tail.Count -gt 0) {
            Say "--- 直前の出力 (最後の 20 行) ---" "--- last 20 lines of output ---" "DarkGray"
            $start = [Math]::Max(0, $script:Tail.Count - 20)
            for ($i = $start; $i -lt $script:Tail.Count; $i++) { Write-Ui "  $($script:Tail[$i])" "DarkGray" }
        }
        Show-Hint $script:StepId
        if ($script:LogPath) { Say "ログ: $($script:LogPath)" "log: $($script:LogPath)" }
    }
} finally {
    try { Remove-Stage } catch { }
    if (Test-Path $tmpDir) { Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue }
    $sshPw = ""
    $sudoPw = ""
    $adminPw = ""
}

# ---- machine-readable result ----------------------------------------------------------
if (-not $script:MultiDone -and ($Json -or $ResultFile)) {
    $hostJson = ConvertTo-JsonObject @(
        "name", (Get-Opt "name" ([string]$cfg.host)),
        "host", [string]$cfg.host,
        "user", [string]$cfg.user,
        "port", [int](Get-Default "ssh_port" "22"),
        "status", $script:ResultStatus,
        "exit_code", [int]$exitCode,
        "failed_step", $script:FailedStep,
        "message", $script:FailMessage,
        "version_before", $script:VersionBefore,
        "version_after", $script:VersionAfter,
        "source_id", $script:SourceId,
        "url", $script:DeployedUrl,
        "dry_run", [bool]$DryRun,
        "log", $script:LogPath,
        "started_at", $StartedAt.ToString("yyyy-MM-ddTHH:mm:ssZ"),
        "duration_s", [int]([DateTime]::UtcNow - $StartedAt).TotalSeconds)
    if ($ResultFile) {
        try { [System.IO.File]::WriteAllText($ResultFile, $hostJson + "`n", (New-Object System.Text.ASCIIEncoding)) } catch { }
    } else {
        $st = "ok"
        if ($exitCode -ne 0) { $st = "failed" }
        [Console]::Out.WriteLine('{"tool":"wpg-deploy","format":1,"status":' + (ConvertTo-JsonText $st) + ',"exit_code":' + $exitCode +
            ',"dry_run":' + (ConvertTo-JsonText ([bool]$DryRun)) + ',"message":' + (ConvertTo-JsonText $script:FailMessage) + ',"hosts":[' + $hostJson + ']}')
    }
}
exit $exitCode
