<#
.SYNOPSIS
  Deploy wpg from Windows to a Linux host (Ubuntu / Debian) over SSH: a wizard that asks
  everything, or parameters for unattended use.
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
    4. waits for GET /api/health and prints the URL.

  Needs the OpenSSH client of Windows 10 / 11 (ssh, scp, ssh-keygen) and tar.exe (both
  ship with Windows 10 1803 and later). Works with Windows PowerShell 5.1 and PowerShell 7.
  Passwords are never written to disk or put on a command line: the sudo password and the
  admin password go to the host through the standard input of ssh.
.PARAMETER HostName
  Target host (IP address or name). Without it the wizard starts.
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
  Start the wizard even when -HostName is given (its values become the defaults).
.PARAMETER Yes
  Do not ask for confirmation (parameter mode).
.PARAMETER DryRun
  Ask and connect, but only print what would change on the host.
.EXAMPLE
  .\scripts\deploy.ps1
  The wizard.
.EXAMPLE
  .\scripts\deploy.ps1 -HostName 192.0.2.10 -User ubuntu -Iface ens4,ens5 -Yes
  Build on the host from this checkout and install the service; passwords are asked.
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
    [string]$ReleaseTag = "latest",
    [string]$Bind = "",
    [string[]]$Iface = @(),
    [switch]$NoService,
    [switch]$Terminal,
    [switch]$SnmpTrap,
    [switch]$Rollback,
    [switch]$NoTemplates,
    [switch]$NoHealthcheck,
    [string]$ProfilePath = "",
    [ValidateSet("", "ja", "en")]
    [string]$Lang = "",
    [string]$LogFile = "",
    [switch]$Interactive,
    [switch]$Yes,
    [switch]$DryRun
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

function Add-Tail([string]$Line) {
    $script:Tail.Add($Line)
    if ($script:Tail.Count -gt 40) { $script:Tail.RemoveAt(0) }
}

function Say([string]$Ja, [string]$En, [string]$Color = "") {
    $msg = T $Ja $En
    if ($Color) { Write-Host $msg -ForegroundColor $Color } else { Write-Host $msg }
    Write-Log $msg
}

function Warn([string]$Ja, [string]$En) {
    Say ((T "警告: " "warning: ") + (T $Ja $En)) "" "Yellow"
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
    Write-Host ""
    $line = "[$($script:StepNo)$total] $($script:StepName)"
    Write-Host $line -ForegroundColor Cyan
    Write-Log ""
    Write-Log $line
}

function Fail([string]$Ja, [string]$En) {
    throw (T $Ja $En)
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
            Write-Host "  $line"
            Write-Log "  $line"
            Add-Tail $line
        }
    }
    $p.WaitForExit()
    $err = $errTask.Result
    if ($err -and -not $Quiet) {
        foreach ($l in ($err -split "`r?`n")) {
            if ($l) { Write-Host "  $l" -ForegroundColor DarkYellow; Write-Log "  $l"; Add-Tail $l }
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
    $s = Read-Host (T $Ja $En) -AsSecureString
    return (ConvertFrom-Secure $s)
}

function Read-YesNo([string]$Ja, [string]$En, [bool]$Default) {
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
    Say $TitleJa $TitleEn
    for ($i = 0; $i -lt $Options.Count; $i++) {
        $parts = $Options[$i].Split("|")
        Write-Host ("  {0}) {1}" -f ($i + 1), (T $parts[0] $parts[1]))
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

if (-not $ProfilePath) {
    $appData = $env:APPDATA
    if (-not $appData) { $appData = Join-Path $HomeDir ".config" }
    $ProfilePath = Join-Path (Join-Path $appData "wpg") "deploy.conf"
}

function Read-Profile([string]$Path) {
    $p = @{}
    foreach ($line in [System.IO.File]::ReadAllLines($Path)) {
        if ($line -match '^\s*#' -or $line -notmatch '=') { continue }
        $i = $line.IndexOf("=")
        $p[$line.Substring(0, $i).Trim()] = $line.Substring($i + 1).Trim()
    }
    return $p
}

function Save-Profile([string]$Path) {
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    $lines = @("# wpg deploy wizard answers (scripts/deploy.sh / deploy.ps1). No passwords are stored.")
    foreach ($k in $cfg.Keys) { $lines += "$k=$($cfg[$k])" }
    [System.IO.File]::WriteAllText($Path, (($lines -join "`n") + "`n"), $script:Utf8NoBom)
    Say "設定を保存しました: $Path (パスワードは保存していません)" "Saved the answers to $Path (no passwords)."
}

function Get-Default([string]$Key, [string]$Fallback) {
    if ($cfg[$Key]) { return [string]$cfg[$Key] }
    return $Fallback
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
    return ($r.ExitCode -eq 0)
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
        if (-not (Test-KeyLogin $cfg.identity)) { Fail "鍵 $($cfg.identity) でログインできません" "cannot log in with the key $($cfg.identity)" }
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
    if (-not $script:sshPw) { $script:sshPw = Read-Secret "SSH パスワード ($($script:target))" "SSH password ($($script:target))" }
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
    $r = Invoke-Ssh "bash $($script:stage)/remote-probe.sh" -Quiet
    $script:probe = @{}
    $list = @()
    foreach ($line in $r.Output) {
        if ($line -match '^WPG_PROBE_([A-Z_]+)=(.*)$') {
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
        $prev = Read-Profile $ProfilePath
        foreach ($k in $prev.Keys) {
            if ($cfg.Contains($k) -and -not $cfg[$k]) { $cfg[$k] = $prev[$k] }
        }
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
        $cfg.binary_file = $v
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
                Write-Host ("  {0}) {1,-12} {2,-8} {3}{4}" -f ($i + 1), $name, $state, $addrs, $mark)
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

function Show-Summary {
    Write-Host ""
    Say "確認" "Summary" "Cyan"
    $bin = ""
    switch ($cfg.binary_source) {
        "build" {
            if ($cfg.source -eq "git") { $bin = (T "ホスト上でビルド" "build on the host") + " (git: $(Get-Default 'git_url' $DefaultGitUrl) $(Get-Default 'git_ref' 'main'))" }
            else { $bin = (T "ホスト上でビルド" "build on the host") + " (" + (T "このパソコンのソース" "this checkout") + ": $Root)" }
        }
        "file" { $bin = $cfg.binary_file }
        "release" { $bin = "GitHub Release ($ReleaseTag)" }
    }
    $ifs = $cfg.ifaces
    if (-not $ifs) { $ifs = T "すべて (制限なし)" "all (no restriction)" }
    $rows = @(
        @((T "接続先" "Host"), "$($script:target) (port $(Get-Default 'ssh_port' '22'))"),
        @((T "バイナリ" "Binary"), $bin),
        @((T "サービス" "Service"), $(if ($cfg.service -eq "1") { "systemd (wpg.service)" } else { T "なし" "no" })),
        @((T "待ち受け" "Listen"), (Get-Default "bind" "0.0.0.0:8080")),
        @((T "送信インタフェース" "TX interfaces"), $ifs),
        @((T "管理者パスワード" "Admin password"), $(if ($script:adminPw) { T "設定する" "set" } else { T "変更しない" "unchanged" })),
        @((T "Web ターミナル" "Web terminal"), $(if ($cfg.terminal -eq "1") { T "有効" "on" } else { T "無効" "off" })),
        @((T "SNMP トラップ受信" "SNMP traps"), $(if ($cfg.snmp_trap -eq "1") { T "有効 (UDP 162)" "on (UDP 162)" } else { T "無効" "off" }))
    )
    foreach ($r in $rows) {
        $line = "  {0} : {1}" -f $r[0], $r[1]
        Write-Host $line
        Write-Log $line
    }
}

# ---- main -------------------------------------------------------------------------
$wizard = ($Interactive -or -not $HostName)
$script:usePrev = $false
$script:releaseUrl = $null
$exitCode = 0

if (-not $LogFile) {
    $base = $env:LOCALAPPDATA
    if (-not $base) { $base = Join-Path $HomeDir ".local/state" }
    $LogFile = Join-Path (Join-Path (Join-Path $base "wpg") "deploy-logs") ("deploy-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".log")
}

try {
    $logDir = Split-Path -Parent $LogFile
    if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Force -Path $logDir | Out-Null }
    $script:LogPath = $LogFile
    Write-Log "# wpg deploy $(Get-Date -Format s) (PowerShell $($PSVersionTable.PSVersion))"

    $script:StepId = "tools"
    $ssh = Find-Tool "ssh"
    $scp = Find-Tool "scp"
    if (-not $ssh -or -not $scp) { Fail "ssh / scp が見つかりません (OpenSSH クライアントが必要です)" "ssh / scp not found (the OpenSSH client is required)" }
    New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null

    if ($wizard) {
        Start-Wizard
    } else {
        if (-not $cfg.user) { $cfg.user = "ubuntu" }
    }
    if (-not $cfg.binary_source) { $cfg.binary_source = "build" }
    if (-not $cfg.source) { $cfg.source = "local" }
    if (-not $cfg.git_url) { $cfg.git_url = $DefaultGitUrl }
    if (-not $cfg.git_ref) { $cfg.git_ref = "main" }
    if (-not $cfg.bind) { $cfg.bind = "0.0.0.0:8080" }
    if (-not $cfg.service) { $cfg.service = "1" }
    if (-not $cfg.terminal) { $cfg.terminal = "0" }
    if (-not $cfg.snmp_trap) { $cfg.snmp_trap = "0" }
    if ($cfg.host -match '^(.+)@(.+)$') { $cfg.user = $Matches[1]; $cfg.host = $Matches[2] }
    $target = "$($cfg.user)@$($cfg.host)"
    Set-SshBase ""
    if ($cfg.bind -notmatch '^(.+):(\d{1,5})$') { Fail "待ち受けアドレスは <アドレス>:<ポート> で指定してください: $($cfg.bind)" "the listen address must be <address>:<port>: $($cfg.bind)" }
    $bindAddr = $Matches[1]
    $bindPort = $Matches[2]
    foreach ($i in ($cfg.ifaces -split ",")) {
        if ($i -and $i -notmatch '^[A-Za-z0-9_.:@-]+$') { Fail "インタフェース名が不正です: $i" "invalid interface name: $i" }
    }

    # ---- connect ----
    Step "connect" "$target に接続" "Connecting to $target"
    if ($null -ne $SshPassword) { $sshPw = ConvertFrom-Secure $SshPassword }
    Initialize-SshKey
    Initialize-Stage
    $osName = Get-Probe "OS_NAME"
    $verText = Get-Probe "WPG_VERSION"
    if (-not $verText) { $verText = "-" }
    Say "  OS: $osName ($(Get-Probe 'ARCH'))   sudo: $(Get-Probe 'SUDO')   wpg: $verText" "  OS: $osName ($(Get-Probe 'ARCH'))   sudo: $(Get-Probe 'SUDO')   wpg: $verText"
    if ((Get-Probe "KERNEL") -and (Get-Probe "KERNEL") -ne "Linux") { Fail "Linux ホストにだけデプロイできます" "only Linux hosts are supported" }
    if ((Get-Probe "OS_ID") -notin @("ubuntu", "debian", "")) {
        Warn "$osName は Ubuntu / Debian 以外です。できる範囲で進めます。" "$osName is not Ubuntu / Debian; continuing on a best-effort basis."
    }

    if ($wizard) {
        if (-not (Start-WizardHost)) { Say "中止しました。" "Aborted."; return }
    }

    # ---- sudo ----
    switch (Get-Probe "SUDO") {
        "root" { $sudoMode = "root" }
        "nopasswd" { $sudoMode = "nopasswd" }
        "none" { $script:StepId = "sudo"; Fail "ホストに sudo がありません。root でログインするか sudo を入れてください。" "sudo is missing on the host; log in as root or install sudo." }
        default {
            $sudoMode = "password"
            if ($null -ne $SudoPassword) { $sudoPw = ConvertFrom-Secure $SudoPassword }
            for ($try = 1; $try -le 3; $try++) {
                if (-not $sudoPw) {
                    if ($sshPw) {
                        $sudoPw = Read-Secret "sudo パスワード (Enter = SSH パスワードと同じ)" "sudo password (Enter = same as the SSH password)"
                        if (-not $sudoPw) { $sudoPw = $sshPw }
                    } else {
                        $sudoPw = Read-Secret "sudo パスワード" "sudo password"
                    }
                }
                $r = Invoke-Ssh "sudo -S -p '' true" $sudoPw -Quiet
                if ($r.ExitCode -eq 0) { break }
                $sudoPw = ""
                if ($try -eq 3) { $script:StepId = "sudo"; Fail "sudo に失敗しました" "sudo failed" }
                Say "  sudo パスワードが違います。もう一度入力してください。" "  Wrong sudo password; try again."
            }
        }
    }

    # ---- admin password, summary ----
    if (-not $doRollback -and $cfg.service -eq "1") {
        if ($null -ne $AdminPassword) {
            $adminPw = ConvertFrom-Secure $AdminPassword
        } else {
            $loop = ($bindAddr -match '^(127\.|localhost$|\[::1\]$)')
            $installed = ((Get-Probe "WPG_VERSION") -and (Get-Probe "SERVICE") -ne "absent")
            if ($wizard -or -not $installed) { $adminPw = Read-AdminPassword ($installed -or $loop) }
        }
    }
    if ($doRollback) {
        if (-not $Yes) {
            if (-not (Read-YesNo "/usr/local/bin/wpg と wpg.prev を入れ替えてサービスを再起動します。よろしいですか?" "Swap /usr/local/bin/wpg with wpg.prev and restart the service?" $true)) { Say "中止しました。" "Aborted."; return }
        }
        # The listen address is only known from the saved answers or -Bind.
        $rollbackHealth = ((Get-Probe "SERVICE") -ne "absent" -and ($script:usePrev -or $Bind))
    } else {
        if ($wizard -or -not $Yes) {
            Show-Summary
            if (-not (Read-YesNo "この内容で実行しますか?" "Go ahead?" $true)) { Say "中止しました。" "Aborted."; return }
        }
        if ($wizard) {
            if (Read-YesNo "この設定を次回のために保存しますか? (パスワードは保存しません)" "Save these answers for next time? (no passwords)" $true) { Save-Profile $ProfilePath }
        }
    }
    Say "ログ: $LogFile" "log: $LogFile"

    $build = (-not $doRollback -and $cfg.binary_source -eq "build")
    $health = (-not $NoHealthcheck -and (($cfg.service -eq "1" -and -not $doRollback) -or $rollbackHealth))
    $script:StepTotal = $script:StepNo + 2
    if ($build) { $script:StepTotal += 2 }
    if ($health) { $script:StepTotal += 1 }

    # ---- upload ----
    Step "upload" "アップロード ($($target):$stage)" "Uploading to $($target):$stage"
    $remoteArgs = @("--stage", $stage, "--bind", $cfg.bind)
    if (-not $doRollback) {
        if (-not $NoTemplates) {
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
            if (-not $url) { $url = Get-ReleaseAsset $ReleaseTag $arch }
            if (-not $url) { $script:StepId = "binary"; Fail "GitHub Release ($ReleaseTag) に Linux ($arch) 用のアーカイブがありません (リリースはまだありません)。ホスト上でのビルドを選んでください。" "no Linux ($arch) archive in the GitHub Release ($ReleaseTag) (there are no releases yet); build on the host instead." }
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
    if ($doRollback) { $remoteArgs += "--rollback" } else { $remoteArgs += @("--binary", "$stage/wpg") }
    if ($cfg.service -eq "1" -and -not $doRollback) { $remoteArgs += "--service" }
    if ($cfg.terminal -eq "1" -and -not $doRollback) { $remoteArgs += "--terminal" }
    if ($cfg.snmp_trap -eq "1" -and -not $doRollback) { $remoteArgs += "--snmp-trap" }
    if (-not $doRollback) {
        foreach ($i in ($cfg.ifaces -split ",")) { if ($i) { $remoteArgs += @("--iface", $i) } }
    }
    $cmd = "bash $stage/deploy-remote.sh " + (($remoteArgs | ForEach-Object { ConvertTo-ShArg $_ }) -join " ")
    if ($DryRun) { Say "  (dry run) sudo $cmd" "  (dry run) sudo $cmd" }
    else { $null = Invoke-Root $cmd }

    # ---- health ----
    if ($health) {
        $hc = $bindAddr
        if ($hc -in @("0.0.0.0", "[::]", "::", "")) { $hc = "127.0.0.1" }
        Step "health" "ヘルスチェック (http://$($hc):$bindPort/api/health)" "Health check (http://$($hc):$bindPort/api/health)"
        if (-not $DryRun) { $null = Invoke-SshChecked ("bash $stage/remote-health.sh " + (ConvertTo-ShArg $hc) + " $bindPort 30") }
    }
    $script:StepId = ""
    Write-Host ""
    if ($doRollback) {
        Say "ロールバックしました。" "Rolled back." "Green"
    } elseif ($cfg.service -ne "1") {
        Say "完了しました (サービスは入れていません。ホストで wpg serve を実行してください)。" "Done (no service installed; run wpg serve on the host)." "Green"
    } elseif ($bindAddr -match '^(127\.|localhost$|\[::1\]$)') {
        Say "完了しました。wpg はホストのループバックで待ち受けています。SSH トンネル経由で開いてください:" "Done. wpg listens on the host's loopback; open it through an SSH tunnel:" "Green"
        Say "  ssh -L $($bindPort):127.0.0.1:$bindPort -p $(Get-Default 'ssh_port' '22') $target   ->  http://127.0.0.1:$bindPort/" "  ssh -L $($bindPort):127.0.0.1:$bindPort -p $(Get-Default 'ssh_port' '22') $target   ->  http://127.0.0.1:$bindPort/"
    } else {
        Say "完了しました。ブラウザで開いてください:" "Done. Open in a browser:" "Green"
        Say "  http://$($cfg.host):$bindPort/" "  http://$($cfg.host):$bindPort/" "Green"
        if ($adminPw -or -not (Get-Probe "WPG_VERSION")) { Say "  (ユーザ名 admin と、設定した管理者パスワードでログインします)" "  (log in as admin with the admin password you set)" }
    }
} catch {
    $exitCode = 1
    Write-Host ""
    $where = ""
    if ($script:StepNo -gt 0) { $where = T " (ステップ $($script:StepNo): $($script:StepName))" " (step $($script:StepNo): $($script:StepName))" }
    $msg = (T "失敗しました" "FAILED") + $where + ": " + $_.Exception.Message
    Write-Host $msg -ForegroundColor Red
    Write-Log $msg
    if ($script:Tail.Count -gt 0) {
        Say "--- 直前の出力 (最後の 20 行) ---" "--- last 20 lines of output ---" "DarkGray"
        $start = [Math]::Max(0, $script:Tail.Count - 20)
        for ($i = $start; $i -lt $script:Tail.Count; $i++) { Write-Host "  $($script:Tail[$i])" -ForegroundColor DarkGray }
    }
    Show-Hint $script:StepId
    if ($script:LogPath) { Say "ログ: $($script:LogPath)" "log: $($script:LogPath)" }
} finally {
    try { Remove-Stage } catch { }
    if (Test-Path $tmpDir) { Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue }
    $sshPw = ""
    $sudoPw = ""
    $adminPw = ""
}
exit $exitCode
