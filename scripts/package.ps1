$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
. "$PSScriptRoot/toolchain.ps1"
$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
New-Item -ItemType Directory -Force docs/logs,desktop/src-tauri/binaries | Out-Null
$ErrorActionPreference = 'Continue'
& cargo build --locked --release -p codebeat-runner 2>&1 | Tee-Object docs/logs/runner-release.txt
if ($LASTEXITCODE -ne 0) { throw 'Runner build failed' }
$ErrorActionPreference = 'Stop'
Copy-Item -LiteralPath target/release/codebeat-runner.exe -Destination desktop/src-tauri/binaries/codebeat-runner.exe -Force
$ErrorActionPreference = 'Continue'
$tauriArguments = @('--prefix','desktop','run','tauri','--','build','--bundles','nsis')
& npm @tauriArguments 2>&1 | Tee-Object docs/logs/desktop-release.txt
if ($LASTEXITCODE -ne 0) { throw 'Tauri bundle failed' }
$ErrorActionPreference = 'Stop'
$installer = Get-ChildItem target/release/bundle/nsis/CodeBeat_*-setup.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (!$installer) { throw 'NSIS output missing' }
Copy-Item -LiteralPath $installer.FullName -Destination target/release/bundle/nsis/codebeat-Setup-x64.exe -Force
$hash = (Get-FileHash target/release/bundle/nsis/codebeat-Setup-x64.exe -Algorithm SHA256).Hash.ToLower()
"$hash  codebeat-Setup-x64.exe" | Set-Content -Encoding ascii target/release/bundle/nsis/SHA256SUMS
Write-Output "Installer: target/release/bundle/nsis/codebeat-Setup-x64.exe"

@{notification_demo=$false;sha256=$hash;built_at_utc=(Get-Date).ToUniversalTime().ToString('o');runner_sha256=(Get-FileHash target/release/codebeat-runner.exe -Algorithm SHA256).Hash.ToLower();app_sha256=(Get-FileHash target/release/codebeat.exe -Algorithm SHA256).Hash.ToLower()} | ConvertTo-Json | Set-Content -Encoding UTF8 target/release/bundle/nsis/build-info.json
