@echo off
rem Double-click to deploy wpg to a Linux host over SSH (wizard: scripts\deploy.ps1).
rem Parameters are passed on, e.g.:  deploy.bat -HostName 192.0.2.10 -User ubuntu
rem Only ASCII in this file: cmd.exe reads it in the console code page.
setlocal
set "PSEXE=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if not exist "%PSEXE%" set "PSEXE=powershell.exe"
"%PSEXE%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0deploy.ps1" %*
set "RC=%ERRORLEVEL%"
rem Keep the window open when started by double-click (no parameters).
if "%~1"=="" pause
exit /b %RC%
