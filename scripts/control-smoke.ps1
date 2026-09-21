#Requires -Version 5.1
<# Isolated native Slice 2 gate. No provider calls, credentials, agent launch or user repositories. #>
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Executable,[Parameter(Mandatory=$true)][string]$ScratchParent)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$Exe=[IO.Path]::GetFullPath($Executable)
$Root=Split-Path -Parent $PSScriptRoot
$Scratch=Join-Path $ScratchParent ('control-smoke-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path $Scratch
$WsHome=Join-Path $Scratch 'home'
function Invoke-Workstation([string[]]$Arguments) {
 $raw=& $Exe --home $WsHome --json @Arguments
 if ($LASTEXITCODE -ne 0) { throw 'Native control smoke failed. No completed release should be packaged.' }
 return (($raw | Out-String) | ConvertFrom-Json)
}
$null=Invoke-Workstation -Arguments @('init')
$upgrade=Invoke-Workstation -Arguments @('upgrade')
if ($upgrade.data.to -ne 5) { throw 'Expected explicit v1 -> v2 -> v3 -> v4 -> v5 upgrade.' }
$second=Invoke-Workstation -Arguments @('upgrade')
if ($second.data.changed -ne $false) { throw 'Repeated upgrade was not idempotent.' }
$model=Invoke-Workstation -Arguments @('model-frontier','--input',(Join-Path $Root 'fixtures\control\models.json'))
$fit=Invoke-Workstation -Arguments @('plan-fit','--input',(Join-Path $Root 'fixtures\control\plan-cycles.json'))
$diagnostic=Invoke-Workstation -Arguments @('diagnose-evidence','--input',(Join-Path $Root 'fixtures\control\diagnostic-evidence.json'),'--at','100')
if ($diagnostic.data.live_host_collected -ne $false -or $diagnostic.data.repair_available -ne $false) { throw 'Imported symptoms gained live/repair authority.' }
$null=Invoke-Workstation -Arguments @('backup')
$receipt=[ordered]@{status='native-isolated-control-smoke-completed';schema=5;provider_calls=0;live_agents_exercised=0;real_handoff='NOT_TESTED';secret_providers='NOT_TESTED';timestamp_utc=[DateTime]::UtcNow.ToString('o')}
$Utf8=New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $Scratch 'control-smoke-result.json'),($receipt|ConvertTo-Json),$Utf8)
Write-Host "Native control smoke evidence preserved at $Scratch"
