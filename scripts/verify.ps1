$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
. "$PSScriptRoot/toolchain.ps1"
$env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
New-Item -ItemType Directory -Force docs/logs | Out-Null
function Check($name, [scriptblock]$work) {
  $ErrorActionPreference = 'Continue'
  Write-Output "[$([DateTime]::UtcNow.ToString('o'))] $name"
  & $work 2>&1 | Tee-Object -FilePath "docs/logs/$name.txt"
  if ($LASTEXITCODE -ne 0) { throw "$name failed: $LASTEXITCODE" }
}
Check 'frontend-tests' { npm --prefix desktop test }
Check 'frontend-build' { npm --prefix desktop run build }
if (!(Test-Path desktop/src-tauri/binaries/codebeat-runner.exe)) {
  Check 'runner-debug' { cargo build --locked -p codebeat-runner }
  New-Item -ItemType Directory -Force desktop/src-tauri/binaries | Out-Null
  Copy-Item target/debug/codebeat-runner.exe desktop/src-tauri/binaries/codebeat-runner.exe -ErrorAction Stop
}
Check 'rust-format' { cargo fmt --all -- --check }
Check 'rust-tests' { cargo test --workspace --locked }
Check 'rust-clippy' { cargo clippy --workspace --all-targets --locked -- -D warnings }
exit 0
