param([string]$Executable = 'target/debug/codebeat.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
# Build: npm --prefix desktop run build; cargo build -p codebeat --features tauri/custom-protocol
& node scripts/test-notification-helper.mjs $Executable
if ($LASTEXITCODE -ne 0) { throw 'Rust notification helper verification failed' }
