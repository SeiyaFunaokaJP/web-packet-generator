<#
.SYNOPSIS
  Run the wpg server in the background on Windows (start at boot, restart on failure).
.DESCRIPTION
  wpg.exe is a console program; it does not implement the Service Control Manager
  protocol, so a plain `sc.exe create` service fails to start (error 1053). This script
  registers a Scheduled Task that runs `wpg.exe serve` as SYSTEM at boot instead. To run
  wpg as a real Windows service, wrap it with WinSW or NSSM (see
  docs/en/src/operations/services.md).

  Run from an elevated PowerShell.
.PARAMETER Action
  Install, Uninstall, Start, Stop or Status.
.PARAMETER Exe
  Path of wpg.exe (default: C:\Program Files\wpg\wpg.exe).
.PARAMETER ConfigFile
  wpg.toml used by the task (default: C:\ProgramData\wpg\wpg.toml; created with
  `wpg config init` when missing).
.PARAMETER DataDir
  Data directory (default: C:\ProgramData\wpg\data).
.EXAMPLE
  .\wpg-service.ps1 -Action Install -Exe C:\Tools\wpg\wpg.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Install", "Uninstall", "Start", "Stop", "Status")]
    [string]$Action,
    [string]$Exe = (Join-Path $env:ProgramFiles "wpg\wpg.exe"),
    [string]$ConfigFile = (Join-Path $env:ProgramData "wpg\wpg.toml"),
    [string]$DataDir = (Join-Path $env:ProgramData "wpg\data")
)

$ErrorActionPreference = "Stop"
$TaskName = "wpg"

function Assert-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($id)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw "Run this script from an elevated PowerShell (Run as administrator)."
    }
}

function Test-Npcap {
    $system32 = Join-Path $env:SystemRoot "System32"
    if (-not (Test-Path (Join-Path $system32 "Npcap\wpcap.dll")) -and -not (Test-Path (Join-Path $system32 "wpcap.dll"))) {
        Write-Warning "Npcap not found; transmitting will fail. Install it from https://npcap.com/#download, then run: wpg doctor"
    }
}

# Creates a directory that only SYSTEM (the task) and Administrators can open. Every user
# may read new folders under C:\ProgramData, but wpg.toml holds the password hash and the
# data directory the database and the TLS key. Existing directories are left alone.
function New-PrivateDirectory([string]$Path) {
    if (Test-Path $Path) { return }
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
    & icacls.exe $Path /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "icacls failed on $Path ($LASTEXITCODE)" }
}

switch ($Action) {
    "Install" {
        Assert-Admin
        if (-not (Test-Path $Exe)) { throw "wpg.exe not found: $Exe (use -Exe)" }
        Test-Npcap
        New-PrivateDirectory (Split-Path -Parent $ConfigFile)
        New-PrivateDirectory $DataDir
        if (-not (Test-Path $ConfigFile)) {
            & $Exe config init --path $ConfigFile
            if ($LASTEXITCODE -ne 0) { throw "wpg config init failed ($LASTEXITCODE)" }
            Write-Host "==> wrote $ConfigFile; set a password with: & '$Exe' config set-password --config '$ConfigFile'"
        }
        # The task runs through cmd.exe so that the environment variables apply to wpg only.
        $cmdArgs = "/c set `"WPG_CONFIG=$ConfigFile`" && set `"WPG_DATA_DIR=$DataDir`" && `"$Exe`" serve"
        $taskAction = New-ScheduledTaskAction -Execute "cmd.exe" -Argument $cmdArgs -WorkingDirectory $DataDir
        $trigger = New-ScheduledTaskTrigger -AtStartup
        $principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
        $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
            -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1)
        Register-ScheduledTask -TaskName $TaskName -Action $taskAction -Trigger $trigger `
            -Principal $principal -Settings $settings -Force | Out-Null
        Start-ScheduledTask -TaskName $TaskName
        Write-Host "==> scheduled task '$TaskName' installed and started"
    }
    "Uninstall" {
        Assert-Admin
        if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
            Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
            Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
            Write-Host "==> scheduled task '$TaskName' removed ($ConfigFile and $DataDir are kept)"
        } else {
            Write-Host "scheduled task '$TaskName' is not installed"
        }
    }
    "Start" {
        Assert-Admin
        Start-ScheduledTask -TaskName $TaskName
    }
    "Stop" {
        Assert-Admin
        Stop-ScheduledTask -TaskName $TaskName
        # Stopping the task ends cmd.exe; make sure wpg.exe goes with it.
        Get-Process -Name wpg -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq (Resolve-Path $Exe).Path } |
            Stop-Process -Force
    }
    "Status" {
        $task = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
        if (-not $task) { Write-Host "not installed"; exit 1 }
        $info = Get-ScheduledTaskInfo -TaskName $TaskName
        Write-Host "state=$($task.State) last_run=$($info.LastRunTime) last_result=$($info.LastTaskResult)"
    }
}
