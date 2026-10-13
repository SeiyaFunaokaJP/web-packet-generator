<#
.SYNOPSIS
  Build wpg: frontend (trunk, WASM) first, then the backend workspace.
.PARAMETER Release
  Optimised build.
.PARAMETER Features
  Extra cargo features for the workspace. "pcap" (the Npcap backend, the only way to send
  on Windows) is added automatically when the Npcap SDK is found; it needs the SDK at build
  time, and wpg.exe then delay-loads wpcap.dll at run time.
.PARAMETER Dpdk
  Shorthand for -Features dpdk.
.PARAMETER NoPcap
  Do not add the pcap feature automatically when the Npcap SDK is found.
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [string]$Features = "",
    [switch]$Dpdk,
    [switch]$NoPcap
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Test-Command([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

# trunk reads NO_COLOR as a boolean and rejects the usual NO_COLOR=1 (no-color.org).
if ($env:NO_COLOR) { $env:NO_COLOR = "true" }

function Invoke-Checked([scriptblock]$Block) {
    & $Block
    if ($LASTEXITCODE -ne 0) { throw "command failed with exit code $LASTEXITCODE" }
}

if ($Dpdk) {
    if ($Features) { $Features = "$Features,dpdk" } else { $Features = "dpdk" }
}

# ---- dependency check -------------------------------------------------------
if (-not (Test-Command rustup)) { throw "rustup not found. Install from https://rustup.rs" }
if (-not (Test-Command cargo)) { throw "cargo not found. Install from https://rustup.rs" }

$targets = & rustup target list --installed
if ($targets -notcontains "wasm32-unknown-unknown") {
    throw "wasm32-unknown-unknown target missing. Run: rustup target add wasm32-unknown-unknown"
}

$haveTrunk = Test-Command trunk
if (-not $haveTrunk) {
    Write-Warning "trunk not found; the frontend build will be skipped. Install: cargo install trunk --locked"
}

# Npcap runtime: needed to transmit (not to build). `wpg doctor` checks it in detail.
# ($env:SystemRoot is only set on Windows; pwsh on Linux / macOS skips the check.)
if ($env:SystemRoot) {
    $system32 = Join-Path $env:SystemRoot "System32"
    if (-not (Test-Path (Join-Path $system32 "Npcap\wpcap.dll")) -and -not (Test-Path (Join-Path $system32 "wpcap.dll"))) {
        Write-Warning "Npcap not found: building works, but sending will not. Install it from https://npcap.com/#download, then run: wpg doctor"
    }
}

# Npcap SDK: wpcap.lib is needed to link the pcap feature. scripts/release.sh downloads a
# pinned SDK by itself; here it is looked up in LIBPCAP_LIBDIR, C:\Npcap-SDK\Lib\x64 or the
# LIB directories.
# The pcap backend is the only way to send on Windows, so it is added whenever the SDK is
# found (unless -NoPcap); without the SDK the build still works but cannot send.
$wantPcap = $Features -match "(^|,)\s*pcap\s*(,|$)"
$sdkLib = $env:LIBPCAP_LIBDIR
if (-not $sdkLib) {
    $candidates = @("C:\Npcap-SDK\Lib\x64") + @(($env:LIB -split ";") | Where-Object { $_ })
    $sdkLib = $candidates | Where-Object { Test-Path (Join-Path $_ "wpcap.lib") } | Select-Object -First 1
}
if (-not $wantPcap -and -not $NoPcap -and $env:SystemRoot) {
    if ($sdkLib) {
        if ($Features) { $Features = "$Features,pcap" } else { $Features = "pcap" }
        $wantPcap = $true
    } else {
        Write-Warning "Npcap SDK not found: building without the pcap backend, so this wpg.exe cannot send. Unpack the SDK (https://npcap.com/#download) to C:\Npcap-SDK, or set LIBPCAP_LIBDIR to its Lib\x64 directory, and build again."
    }
}
if ($wantPcap) {
    if (-not $sdkLib) {
        throw "the pcap feature needs the Npcap SDK (https://npcap.com/#download): unpack it to C:\Npcap-SDK or set LIBPCAP_LIBDIR to its Lib\x64 directory"
    }
    $env:LIBPCAP_LIBDIR = $sdkLib
    # The pcap crate cannot read the libpcap version from the SDK; Npcap 1.x is libpcap 1.10.
    if (-not $env:LIBPCAP_VER) { $env:LIBPCAP_VER = "1.10.0" }
    Write-Host "==> Npcap SDK: LIBPCAP_LIBDIR=$env:LIBPCAP_LIBDIR LIBPCAP_VER=$env:LIBPCAP_VER"
}

# ---- frontend ---------------------------------------------------------------
if ($haveTrunk) {
    $trunkArgs = @("build")
    if ($Release) { $trunkArgs += "--release" }
    Write-Host "==> building frontend (trunk $($trunkArgs -join ' '))"
    Push-Location (Join-Path $Root "crates/wpg-web")
    try { Invoke-Checked { trunk @trunkArgs } } finally { Pop-Location }
} else {
    Write-Warning "skipping frontend build (crates/wpg-web/dist will be missing or stale)"
}

# ---- backend ----------------------------------------------------------------
$cargoArgs = @("build", "--workspace")
if ($Release) { $cargoArgs += "--release" }
if ($Features) { $cargoArgs += @("--features", $Features) }
Write-Host "==> building backend (cargo $($cargoArgs -join ' '))"
Invoke-Checked { cargo @cargoArgs }

$profileDir = if ($Release) { "release" } else { "debug" }
Write-Host "==> done: target\$profileDir\wpg.exe"
