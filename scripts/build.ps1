<#
.SYNOPSIS
  Build wpg: frontend (trunk, WASM) first, then the backend workspace.
.PARAMETER Release
  Optimised build.
.PARAMETER Features
  Extra cargo features for the workspace (e.g. "dpdk").
.PARAMETER Dpdk
  Shorthand for -Features dpdk.
#>
[CmdletBinding()]
param(
    [switch]$Release,
    [string]$Features = "",
    [switch]$Dpdk
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Test-Command([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

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
    if (-not (Test-Path (Join-Path $system32 "Npcap\wpcap.dll"))) {
        Write-Warning "Npcap not found: building works, but sending will not. Install it from https://npcap.com/#download and tick 'Install Npcap in WinPcap API-compatible Mode', then run: wpg doctor"
    } elseif (-not (Test-Path (Join-Path $system32 "wpcap.dll"))) {
        Write-Warning "Npcap is installed without 'WinPcap API-compatible Mode'; reinstall it with that option if wpg cannot load wpcap.dll (see: wpg doctor)"
    }
}

# Npcap SDK (only needed once the pcap feature is used).
if (-not ($env:LIB -and (Test-Path (Join-Path $env:LIB "wpcap.lib"))) -and -not (Test-Path "C:\Npcap-SDK")) {
    Write-Warning "Npcap SDK not found (https://npcap.com/#download). Not required yet: the pcap feature is unused. Set LIB to the SDK Lib\x64 directory when it is."
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
