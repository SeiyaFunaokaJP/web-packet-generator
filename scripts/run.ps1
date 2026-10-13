<#
.SYNOPSIS
  Run wpg.
.PARAMETER Dev
  Hot reload: `trunk serve` (frontend) + `cargo watch` (backend).
  Without it, runs the release binary `wpg serve` (built first if missing).
.PARAMETER ServeArgs
  Extra arguments passed to `wpg serve`.
#>
[CmdletBinding()]
param(
    [switch]$Dev,
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
    if (-not (Test-Path $bin)) {
        Write-Host "==> release binary missing, building"
        & (Join-Path $PSScriptRoot "build.ps1") -Release
    }
    & $bin serve @ServeArgs
    exit $LASTEXITCODE
}
