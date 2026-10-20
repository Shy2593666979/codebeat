param([string]$Executable = 'target/debug/codebeat.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
# Build: npm --prefix desktop run build; cargo build -p codebeat --features tauri/custom-protocol,notification-test
$testIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$testPrincipal = [Security.Principal.WindowsPrincipal]::new($testIdentity)
Write-Output "Native notification environment: elevated=$($testPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)); session=$([Diagnostics.Process]::GetCurrentProcess().SessionId)"
& node scripts/test-notification-helper.mjs $Executable
if ($LASTEXITCODE -ne 0) { throw 'Rust notification helper verification failed' }
