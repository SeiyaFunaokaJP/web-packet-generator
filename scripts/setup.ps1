<#
.SYNOPSIS
  Prepare this Windows machine for building wpg from source.
.DESCRIPTION
  Needs rustup (https://rustup.rs). Installs the toolchain pinned in rust-toolchain.toml
  (with the wasm32 target), a prebuilt trunk and the Npcap SDK at the versions pinned in
  scripts\versions.env, then lists what to install yourself (Npcap, the C++ build tools).
  Linux / macOS: use scripts/setup.sh.
.PARAMETER Just
  Also install the just task runner (cargo install just --locked).
.PARAMETER NoTrunk
  Do not install trunk (backend-only builds, no web UI).
.PARAMETER NoNpcapSdk
  Do not download the Npcap SDK. Without it wpg.exe is built without the Npcap backend
  and cannot send.
.PARAMETER NpcapSdkDir
  Where to unpack the Npcap SDK (default C:\Npcap-SDK, where scripts\build.ps1 looks).
#>
[CmdletBinding()]
param(
    [switch]$Just,
    [switch]$NoTrunk,
    [switch]$NoNpcapSdk,
    [string]$NpcapSdkDir = "C:\Npcap-SDK"
)

$ErrorActionPreference = "Stop"
# Invoke-WebRequest is very slow with its progress bar in Windows PowerShell 5.1.
$ProgressPreference = "SilentlyContinue"
# Windows PowerShell 5.1 may default to TLS 1.0, which GitHub and npcap.com refuse.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Test-Command([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Invoke-Checked([scriptblock]$Block) {
    & $Block
    if ($LASTEXITCODE -ne 0) { throw "command failed with exit code $LASTEXITCODE" }
}

# KEY=value from scripts\versions.env.
function Get-Pinned([string]$Key) {
    $line = Get-Content (Join-Path $PSScriptRoot "versions.env") | Where-Object { $_ -match "^$Key=" } | Select-Object -First 1
    if (-not $line) { throw "$Key missing in scripts\versions.env" }
    return ($line -replace "^$Key=", "").Trim()
}

function Get-Download([string]$Url, [string]$OutFile) {
    Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing
}

$trunkVersion = Get-Pinned "TRUNK_VERSION"
$sdkVersion = Get-Pinned "NPCAP_SDK_VERSION"
$sdkSha256 = Get-Pinned "NPCAP_SDK_SHA256"

# ---- Rust toolchain ---------------------------------------------------------
if (-not (Test-Command rustup)) {
    throw "rustup not found. Install it from https://rustup.rs (default MSVC toolchain), open a new terminal, then run this script again."
}
Write-Host "==> Rust toolchain from rust-toolchain.toml"
# rustup 1.28+ installs the pinned toolchain with a bare `toolchain install`;
# older versions do it on `rustup show`.
& rustup toolchain install --no-self-update
if ($LASTEXITCODE -ne 0) { Invoke-Checked { rustup show active-toolchain } }
Invoke-Checked { rustup target add wasm32-unknown-unknown }
Invoke-Checked { cargo --version }

$cargoBin = if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME "bin" } else { Join-Path $HOME ".cargo\bin" }

# ---- trunk ------------------------------------------------------------------
function Install-Trunk {
    if ($env:PROCESSOR_ARCHITECTURE -eq "AMD64") {
        $asset = "trunk-x86_64-pc-windows-msvc.zip"
        $base = "https://github.com/trunk-rs/trunk/releases/download/v$trunkVersion"
        $tmp = Join-Path ([IO.Path]::GetTempPath()) ("wpg-trunk-" + [Guid]::NewGuid())
        New-Item -ItemType Directory -Path $tmp | Out-Null
        try {
            $zip = Join-Path $tmp $asset
            Get-Download "$base/$asset" $zip
            Get-Download "$base/$asset.sha256" "$zip.sha256"
            $expected = ((Get-Content "$zip.sha256" -Raw).Trim() -split '\s+')[0]
            $actual = (Get-FileHash $zip -Algorithm SHA256).Hash
            if ($expected -ne $actual) { throw "checksum mismatch for $asset" }
            Expand-Archive -Path $zip -DestinationPath $tmp -Force
            New-Item -ItemType Directory -Force -Path $cargoBin | Out-Null
            Copy-Item (Join-Path $tmp "trunk.exe") (Join-Path $cargoBin "trunk.exe") -Force
            return
        } catch {
            Write-Warning "prebuilt trunk failed ($($_.Exception.Message)); building it instead"
        } finally {
            Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        }
    }
    Write-Host "==> cargo install trunk --version $trunkVersion --locked (takes a few minutes)"
    Invoke-Checked { cargo install trunk --version $trunkVersion --locked }
}

if (-not $NoTrunk) {
    $current = ""
    if (Test-Command trunk) { $current = ((& trunk --version) -split '\s+')[1] }
    if ($current -eq $trunkVersion) {
        Write-Host "==> trunk $trunkVersion already installed"
    } else {
        $found = if ($current) { " (found $current)" } else { "" }
        Write-Host "==> installing trunk $trunkVersion$found"
        Install-Trunk
        if (-not (($env:PATH -split ';') -contains $cargoBin)) {
            Write-Warning "$cargoBin is not on PATH; open a new terminal before building"
        }
    }
}

# ---- just -------------------------------------------------------------------
if ($Just) {
    if (Test-Command just) {
        Write-Host "==> $(& just --version) already installed"
    } else {
        Write-Host "==> cargo install just --locked"
        Invoke-Checked { cargo install just --locked }
    }
}

# ---- Npcap SDK --------------------------------------------------------------
# wpcap.lib for linking the Npcap backend, the only way wpg sends on Windows.
$sdkArch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "ARM64" } else { "x64" }
$sdkRoots = @($NpcapSdkDir)
if ($env:NPCAP_SDK) { $sdkRoots += $env:NPCAP_SDK }
$sdkRoots += @(Get-ChildItem -Path "C:\" -Directory -Filter "npcap-sdk-*" -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
$sdkFound = $sdkRoots | Where-Object { Test-Path (Join-Path $_ "Lib\$sdkArch\wpcap.lib") } | Select-Object -First 1
if ($env:LIBPCAP_LIBDIR -and (Test-Path (Join-Path $env:LIBPCAP_LIBDIR "wpcap.lib"))) { $sdkFound = $env:LIBPCAP_LIBDIR }

if ($sdkFound) {
    Write-Host "==> Npcap SDK found: $sdkFound"
} elseif ($NoNpcapSdk) {
    Write-Warning "Npcap SDK not installed (-NoNpcapSdk): wpg.exe will be built without the Npcap backend and cannot send."
} else {
    Write-Host "==> downloading the Npcap SDK $sdkVersion to $NpcapSdkDir"
    $zip = Join-Path ([IO.Path]::GetTempPath()) "npcap-sdk-$sdkVersion.zip"
    try {
        Get-Download "https://npcap.com/dist/npcap-sdk-$sdkVersion.zip" $zip
        if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne $sdkSha256) {
            throw "checksum mismatch for npcap-sdk-$sdkVersion.zip"
        }
        Expand-Archive -Path $zip -DestinationPath $NpcapSdkDir -Force
    } finally {
        Remove-Item -Force $zip -ErrorAction SilentlyContinue
    }
}

# ---- what to install yourself -----------------------------------------------
Write-Host ""
$todo = @()
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vc = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vc) { $todo += "Visual Studio Build Tools with the 'Desktop development with C++' workload: https://visualstudio.microsoft.com/visual-cpp-build-tools/" }
} else {
    $todo += "Visual Studio Build Tools with the 'Desktop development with C++' workload: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
}
$system32 = Join-Path $env:SystemRoot "System32"
if (-not (Test-Path (Join-Path $system32 "Npcap\wpcap.dll")) -and -not (Test-Path (Join-Path $system32 "wpcap.dll"))) {
    $todo += "Npcap (needed to send; default options are fine): https://npcap.com/#download"
}
if ($todo.Count -gt 0) {
    Write-Host "Install these yourself:"
    foreach ($t in $todo) { Write-Host "    - $t" }
    Write-Host ""
}

Write-Host "Setup done. Next:"
if (Test-Command just) {
    Write-Host "    just release; .\target\release\wpg.exe serve"
} else {
    Write-Host "    .\scripts\build.ps1 -Release; .\target\release\wpg.exe serve"
}
