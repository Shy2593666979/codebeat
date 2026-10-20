# Load an optional workspace-local MSVC environment. Standard VS installations need no override.
$portableRoot = $env:PORTABLE_MSVC_ROOT
if (-not $portableRoot) {
    $portableRoot = [Environment]::GetEnvironmentVariable('PORTABLE_MSVC_ROOT', 'User')
}
if (-not $portableRoot) {
    $portableRoot = Join-Path (Split-Path -Parent $PSScriptRoot) '.build-tools/msvc'
}
if (Test-Path -LiteralPath "$portableRoot/VC/Tools/MSVC") {
    $compilerVersion = (Get-ChildItem -LiteralPath "$portableRoot/VC/Tools/MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1).Name
    $sdkVersion = (Get-ChildItem -LiteralPath "$portableRoot/Windows Kits/10/Lib" -Directory | Sort-Object Name -Descending | Select-Object -First 1).Name
    $compilerDir = "$portableRoot/VC/Tools/MSVC/$compilerVersion"
    $sdkDir = "$portableRoot/Windows Kits/10"
    $env:PATH = "$compilerDir/bin/Hostx64/x64;$sdkDir/bin/$sdkVersion/x64;" + $env:PATH
    $env:INCLUDE = "$compilerDir/include;$sdkDir/Include/$sdkVersion/ucrt;$sdkDir/Include/$sdkVersion/shared;$sdkDir/Include/$sdkVersion/um;$sdkDir/Include/$sdkVersion/winrt"
    $env:LIB = "$compilerDir/lib/x64;$sdkDir/Lib/$sdkVersion/ucrt/x64;$sdkDir/Lib/$sdkVersion/um/x64"
    $env:VCToolsInstallDir = "$compilerDir/"
    $env:WindowsSdkDir = "$sdkDir/"
    $env:WindowsSDKVersion = "$sdkVersion/"
}
$env:PATH = "$env:USERPROFILE/.cargo/bin;" + $env:PATH
