param(
  [Parameter(Mandatory=$true)][string]$PreviousRoot,
  [Parameter(Mandatory=$true)][string]$CurrentRoot
)
$ErrorActionPreference='Stop'
if (![IO.Path]::IsPathRooted($PreviousRoot) -or ![IO.Path]::IsPathRooted($CurrentRoot)) {throw 'Absolute workspace roots required'}
$oldRoot=[IO.Path]::GetFullPath($PreviousRoot).TrimEnd('\')
$newRoot=(Resolve-Path -LiteralPath $CurrentRoot).Path.TrimEnd('\')
if ($oldRoot -eq $newRoot) {throw 'Distinct workspace roots required'}
$oldRunner=Join-Path $oldRoot 'target\release\codebeat-runner.exe'
$newRunner=Join-Path $newRoot 'target\release\codebeat-runner.exe'
$newApp=Join-Path $newRoot 'target\release\codebeat.exe'
if (!(Test-Path -LiteralPath $newRunner -PathType Leaf) -or !(Test-Path -LiteralPath $newApp -PathType Leaf)) {throw 'Build release binaries in the current workspace before moving registrations'}
$backup=Join-Path $env:TEMP ('codebeat-workspace-backup-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $backup | Out-Null
$service=New-Object -ComObject Schedule.Service
$service.Connect()
$folder=$service.GetFolder('\')
$records=@()
foreach ($task in @($folder.GetTasks(0))) {
  if (!$task.Name.StartsWith('codebeat-')) {continue}
  $suffix=$task.Name.Substring('codebeat-'.Length)
  $taskGuid=[guid]::Empty
  if ($suffix -notin @('maintenance','pretrigger') -and ![guid]::TryParse($suffix,[ref]$taskGuid)) {continue}
  $definition=$task.Definition
  if ($definition.Actions.Count -ne 1 -or $definition.Actions.Item(1).Type -ne 0 -or $definition.Principal.LogonType -ne 3) {continue}
  $action=$definition.Actions.Item(1)
  if (![string]::Equals([IO.Path]::GetFullPath($action.Path),$oldRunner,[StringComparison]::OrdinalIgnoreCase)) {continue}
  $expected=if ($suffix -in @('maintenance','pretrigger')) {$suffix} else {'run --task-id '+$suffix}
  if ($action.Arguments -ne $expected) {throw 'Unexpected arguments in an old workspace task; registration left untouched'}
  $before=$definition.XmlText
  $before | Set-Content -LiteralPath (Join-Path $backup ($task.Name+'.xml')) -Encoding UTF8
  $enabled=$task.Enabled
  $triggers=([xml]$before).DocumentElement.SelectSingleNode('*[local-name()="Triggers"]').OuterXml
  $settings=([xml]$before).DocumentElement.SelectSingleNode('*[local-name()="Settings"]').OuterXml
  $action.Path=$newRunner
  $action.WorkingDirectory=Split-Path -Parent $newRunner
  try {
    $null=$folder.RegisterTaskDefinition($task.Name,$definition,38,$definition.Principal.UserId,$null,3,$null)
    $updated=$folder.GetTask($task.Name)
    $xml=[xml]$updated.Definition.XmlText
    if ($updated.Enabled -ne $enabled -or $xml.DocumentElement.SelectSingleNode('*[local-name()="Triggers"]').OuterXml -ne $triggers -or $xml.DocumentElement.SelectSingleNode('*[local-name()="Settings"]').OuterXml -ne $settings) {throw 'Task timing or settings changed unexpectedly'}
    if ($updated.Definition.Actions.Item(1).Path -ne $newRunner -or $updated.Definition.Actions.Item(1).Arguments -ne $expected) {throw 'Task action verification failed'}
  } catch {
    $null=$folder.RegisterTask($task.Name,$before,6,$definition.Principal.UserId,$null,3,$null)
    throw
  }
  $records += [pscustomobject]@{name=$task.Name;previous=$oldRunner;current=$newRunner;triggers_and_settings_preserved=$true}
}
$runKey='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$existing=(Get-ItemProperty -LiteralPath $runKey -Name 'codebeat' -ErrorAction SilentlyContinue).'codebeat'
$autostartUpdated=$false
$expectedRun=@(('"'+(Join-Path $oldRoot 'target\debug\codebeat.exe')+'" --background'),('"'+(Join-Path $oldRoot 'target\release\codebeat.exe')+'" --background'))
if ($existing -and $expectedRun -contains $existing) {
  $existing | Set-Content -LiteralPath (Join-Path $backup 'autostart-before.txt') -Encoding UTF8
  Set-ItemProperty -LiteralPath $runKey -Name 'codebeat' -Value ('"'+$newApp+'" --background')
  $autostartUpdated=$true
}
$result=[pscustomobject]@{tasks_updated=$records.Count;tasks=$records;autostart_updated=$autostartUpdated;data_directory_unchanged=$true;backup_directory=$backup}
$result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $newRoot 'docs\logs\codebeat-registration-migration.json') -Encoding UTF8
$result | ConvertTo-Json -Depth 5
