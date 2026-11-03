<#
.SYNOPSIS
  Run wpg.
.PARAMETER Dev
  Hot reload: `trunk serve` (frontend) + `cargo watch` (backend).
  Without it, brings the release build up to date (scripts\build.ps1 -Release), then
  runs `wpg serve`.
.PARAMETER NoBuild
  Run target\release\wpg.exe as it is, without rebuilding it first.
.PARAMETER ServeArgs
  Extra arguments passed to `wpg serve`.
#>
[CmdletBinding()]
param(
    [switch]$Dev,
    [switch]$NoBuild,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ServeArgs = @()
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($Dev) {
    if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
        throw "trunk not found. Install: cargo install trunk --locked"
    }
    & cargo watch --version *> $null
    if ($LASTEXITCODE -ne 0) { throw "cargo-watch not found. Install: cargo install cargo-watch --locked" }

    # trunk reads NO_COLOR as a boolean and rejects the usual NO_COLOR=1 (no-color.org).
    if ($env:NO_COLOR) { $env:NO_COLOR = "true" }
    $trunk = Start-Process -FilePath "trunk" -ArgumentList "serve" `
        -WorkingDirectory (Join-Path $Root "crates/wpg-web") -PassThru -NoNewWindow
    try {
        cargo watch -x "run -p wpg-cli -- serve $($ServeArgs -join ' ')"
    } finally {
        if (-not $trunk.HasExited) { Stop-Process -Id $trunk.Id -Force }
    }
} else {
    $bin = Join-Path $Root "target\release\wpg.exe"
    if (-not $NoBuild) {
        & (Join-Path $PSScriptRoot "build.ps1") -Release
    } elseif (-not (Test-Path $bin)) {
        throw "$bin missing; run without -NoBuild"
    }
    & $bin serve @ServeArgs
    exit $LASTEXITCODE
}
