$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
. "$PSScriptRoot/toolchain.ps1"
# 使用隔离任务名和不可执行的测试文件，不发送模型请求。
& cargo test -p codebeat-lib --lib platform::scheduler::tests::scheduler_preserves_all_trigger_and_execution_policies -- --ignored --nocapture
if ($LASTEXITCODE -ne 0) { throw 'Native scheduler verification failed' }
# 通过两小时后的临时任务验证公共注册及诊断接口，验证后删除，不实际执行。
& cargo test -p codebeat-lib --test platform_scheduler -- --ignored --nocapture
if ($LASTEXITCODE -ne 0) { throw 'Public scheduler diagnostics verification failed' }
