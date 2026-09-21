#Requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Executable,
      [Parameter(Mandatory=$true)][string]$ScratchParent)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Exe = (Resolve-Path -LiteralPath $Executable).Path
$Parent = (Resolve-Path -LiteralPath $ScratchParent).Path
$Sandbox = Join-Path $Parent ('workstation-smoke-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $Sandbox
$WsHome = Join-Path $Sandbox 'home'
$Data = Join-Path $Sandbox 'data'
$null = New-Item -ItemType Directory -Path $Data
$Utf8 = New-Object System.Text.UTF8Encoding($false)
$Canary = 'WORKSTATION_TEST_SECRET_CANARY_DO_NOT_EXPOSE_8194'
[IO.File]::WriteAllText((Join-Path $Data '.env'), $Canary, $Utf8)
$Before = (Get-FileHash -LiteralPath (Join-Path $Data '.env') -Algorithm SHA256).Hash
$Checks = New-Object System.Collections.Generic.List[string]
function Invoke-Json {
    param([string[]]$Arguments, [int[]]$AllowedExitCodes = @(0))
    $Text = & $Exe --home $WsHome --json @Arguments
    $Code = $LASTEXITCODE
    if ($AllowedExitCodes -notcontains $Code) { throw "Unexpected exit $Code from $($Arguments -join ' '). $Text" }
    return ($Text | ConvertFrom-Json)
}
$null = Invoke-Json -Arguments @('init')
$Checks.Add('new-home-initialized')
$null = Invoke-Json -Arguments @('root','add','--id','fixture','--path',$Data,'--kind','other')
$Observed = Invoke-Json -Arguments @('doctor') -AllowedExitCodes @(0,3)
if ($Observed.data.storage[0].logical_entry_bytes -ne [Text.Encoding]::UTF8.GetByteCount($Canary)) { throw 'Byte observation mismatch.' }
if (($Observed | ConvertTo-Json -Depth 30) -match $Canary) { throw 'Secret canary disclosed.' }
$Checks.Add('metadata-observation-without-content-disclosure')
if ((Get-FileHash -LiteralPath (Join-Path $Data '.env') -Algorithm SHA256).Hash -ne $Before) { throw 'Inspected data changed.' }
$Checks.Add('inspected-file-unchanged')
$Shared = Invoke-Json -Arguments @('report','--share') -AllowedExitCodes @(0)
$SharedText = $Shared | ConvertTo-Json -Depth 30
if ($SharedText.Contains($Canary) -or $SharedText.Contains($Data) -or $SharedText.Contains('installation_id')) { throw 'Share allowlist failure.' }
$Checks.Add('share-report-private-identifiers-omitted')
$Backup = Invoke-Json -Arguments @('backup')
if (-not (Test-Path -LiteralPath $Backup.data.backup)) { throw 'Backup missing.' }
$Checks.Add('backup-created-and-checked')
$Missing = Join-Path $Sandbox 'must-stay-missing'
$Text = & $Exe --home $Missing --json doctor
if ($LASTEXITCODE -ne 2 -or (Test-Path -LiteralPath $Missing)) { throw 'Unavailable-home behavior failed.' }
$Checks.Add('missing-home-does-not-create-fallback')
$null = Invoke-Json -Arguments @('init') -AllowedExitCodes @(2)
$Checks.Add('existing-home-not-overwritten')
$Result = [ordered]@{ status='passed'; checks=$Checks; sandbox=$Sandbox; observed_utc=[DateTime]::UtcNow.ToString('o');
    limitations=@('No live Codex/Cursor/Claude repair or session-ownership testing.', 'No drive-disconnect/Defender/kernel-failure certification.') }
[IO.File]::WriteAllText((Join-Path $Sandbox 'smoke-result.json'), ($Result | ConvertTo-Json -Depth 10), $Utf8)
# Keep only this small isolated sandbox as evidence; no recursive delete is performed.
Write-Host "Windows smoke checks passed. Evidence: $Sandbox"
$global:LASTEXITCODE = 0
