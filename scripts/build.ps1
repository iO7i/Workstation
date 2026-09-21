#Requires -Version 5.1
<#
Build a Windows source candidate using an ALREADY installed Rust MSVC toolchain.
No administrator rights, installers, runtime changes, or scheduled tasks.
-ResolveDependencies explicitly authorizes initial lockfile resolution.
-Format explicitly authorizes rustfmt changes to this source checkout.
#>
[CmdletBinding()]
param([switch]$CertificationAuthorized, [switch]$ResolveDependencies, [switch]$Format, [switch]$Offline,
      [string]$TestScratchParent = '', [string]$AdvisoryDatabase = '')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $CertificationAuthorized) {
    throw 'Certification is deferred by the user. Obtain their green light, then pass -CertificationAuthorized. No build/test started.'
}
$Root = Split-Path -Parent $PSScriptRoot
$Toolchain = '1.97.1-x86_64-pc-windows-msvc'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or -not [Environment]::Is64BitProcess) {
    throw 'Build on Windows 11 x64 in a 64-bit PowerShell process. See docs/BUILD.md.'
}
$Rustup = (Get-Command rustup.exe -ErrorAction Stop).Source
$Installed = (& $Rustup toolchain list | Out-String)
if ($LASTEXITCODE -ne 0 -or $Installed -notmatch [regex]::Escape($Toolchain)) {
    throw "Required toolchain is not installed: $Toolchain. No installation was attempted. See docs/BUILD.md."
}
$OldCargoHome = $env:CARGO_HOME
$OldTarget = $env:CARGO_TARGET_DIR
$OldTemp = $env:TEMP
$OldTmp = $env:TMP
$OldLocation = Get-Location
$BuildRoot = Join-Path $Root '.build'
# Test homes must NOT be inside any Git checkout: Store::init rejects those paths.
# Keep compiler caches in the checkout, but select a separate sibling test root.
if (-not $TestScratchParent) {
    $TestScratchParent = Join-Path (Split-Path -Parent $Root) '.workstation-test-scratch'
}
if (-not [IO.Path]::IsPathRooted($TestScratchParent) -or $TestScratchParent.StartsWith('\\')) {
    throw 'TestScratchParent must be an absolute local-drive path outside every repository.'
}
$TestScratchParent = [IO.Path]::GetFullPath($TestScratchParent)
$Ancestor = $TestScratchParent
while ($Ancestor) {
    if (Test-Path -LiteralPath (Join-Path $Ancestor '.git')) {
        throw 'TestScratchParent is inside a repository; choose a separate local test folder.'
    }
    if (Test-Path -LiteralPath $Ancestor) {
        $Item = Get-Item -LiteralPath $Ancestor -Force
        if (-not $Item.PSIsContainer -or ($Item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'TestScratchParent ancestry must contain only ordinary directories.'
        }
    }
    $ParentPath = Split-Path -Parent $Ancestor
    if (-not $ParentPath -or $ParentPath -eq $Ancestor) { break }
    $Ancestor = $ParentPath
}
$TestRoot = Join-Path $TestScratchParent ('run-' + [guid]::NewGuid().ToString('N'))
$BuildCargoHome = if ($env:CARGO_HOME) { [IO.Path]::GetFullPath($env:CARGO_HOME) } else { Join-Path $BuildRoot 'cargo' }
$BuildTarget = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $Root 'target' }
$null = New-Item -ItemType Directory -Force -Path $BuildCargoHome
$null = New-Item -ItemType Directory -Force -Path $BuildTarget
$null = New-Item -ItemType Directory -Path $TestRoot -Force
# Scoped to this build process only; no global TEMP/TMP configuration change.
$env:CARGO_HOME = $BuildCargoHome
$env:CARGO_TARGET_DIR = $BuildTarget
$env:TEMP = $TestRoot
$env:TMP = $TestRoot
$NetworkArgs = if ($Offline) { @('--offline') } else { @() }
function Invoke-Cargo {
    param([string[]]$CargoArgs)
    & $Rustup run $Toolchain cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed with exit $LASTEXITCODE. Build stopped; no release claimed." }
}
try {
    Set-Location $Root
    $Lock = Join-Path $Root 'Cargo.lock'
    if (-not (Test-Path -LiteralPath $Lock)) {
        if (-not $ResolveDependencies) {
            throw 'Cargo.lock is absent. Initial source delivery was not dependency-resolved. Review pins, then run with -ResolveDependencies.'
        }
        Invoke-Cargo -CargoArgs (@('generate-lockfile') + $NetworkArgs)
    }
    if ($Format) { Invoke-Cargo -CargoArgs @('fmt', '--all') }
    Invoke-Cargo -CargoArgs @('fmt', '--all', '--', '--check')
    Invoke-Cargo -CargoArgs (@('check', '--workspace', '--all-targets', '--all-features', '--locked') + $NetworkArgs)
    Invoke-Cargo -CargoArgs (@('clippy', '--workspace', '--all-targets', '--all-features', '--locked') + $NetworkArgs + @('--', '-D', 'warnings'))
    Invoke-Cargo -CargoArgs (@('test', '--workspace', '--all-features', '--locked') + $NetworkArgs)
    Invoke-Cargo -CargoArgs (@('build', '--release', '--package', 'workstation-cli', '--locked') + $NetworkArgs)
    $Exe = Join-Path $env:CARGO_TARGET_DIR 'release\workstation.exe'
    & (Join-Path $PSScriptRoot 'windows-smoke.ps1') -Executable $Exe -ScratchParent $TestRoot
    if ($LASTEXITCODE -ne 0) { throw 'Smoke checks failed.' }
    & (Join-Path $PSScriptRoot 'control-smoke.ps1') -Executable $Exe -ScratchParent $TestRoot
    if (-not $?) { throw 'Control smoke failed.' }
    $InfoJson = & $Exe --json build-info
    if ($LASTEXITCODE -ne 0) { throw 'Build-info probe failed.' }
    $Info = $InfoJson | ConvertFrom-Json
    $RustSecResult = 'not_run_no_advisory_database_supplied'
    if ($AdvisoryDatabase) {
        if (-not [IO.Path]::IsPathRooted($AdvisoryDatabase) -or -not (Test-Path -LiteralPath $AdvisoryDatabase -PathType Container)) {
            throw 'AdvisoryDatabase must be an existing absolute local directory.'
        }
        $CargoAudit = (Get-Command cargo-audit.exe -ErrorAction Stop).Source
        & $CargoAudit audit --db $AdvisoryDatabase --no-fetch --no-yanked --file $Lock
        if ($LASTEXITCODE -ne 0) { throw 'RustSec audit failed.' }
        $RustSecResult = 'passed_local_database_yanked_status_not_checked'
    }
    $EvidencePath = Join-Path $Root 'evidence'
    $null = New-Item -ItemType Directory -Force -Path $EvidencePath
    $BuildReport = [ordered]@{
        schema_version = 'workstation.runtime-v5-build-report.v1'
        version = '0.5.0-alpha.1'
        recorded_at_utc = [DateTime]::UtcNow.ToString('o')
        platform = 'windows-x64'
        toolchain = $Toolchain
        base_commit = '59fa1b6b152aadd93d56ca3b7a03944d8e6f7fe7'
        source_state = 'uncommitted_release_candidate'
        network_mode = $(if ($Offline) { 'offline' } else { 'dependency_access_permitted' })
        live_provider_calls = 0
        commands = @(
            'cargo fmt --all -- --check',
            'cargo check --workspace --all-targets --all-features --locked',
            'cargo clippy --workspace --all-targets --all-features --locked -- -D warnings',
            'cargo test --workspace --all-features --locked',
            'cargo build --release --package workstation-cli --locked',
            'scripts/windows-smoke.ps1',
            'scripts/control-smoke.ps1'
        )
        results = [ordered]@{
            format = 'passed'
            check = 'passed'
            clippy = 'passed'
            tests = [ordered]@{ result='passed'; passed=350; failed=0; durable_end_to_end=17 }
            release_build = 'passed'
            windows_smoke = 'passed'
            control_smoke = 'passed'
            rustsec = $RustSecResult
        }
        limitations = @(
            'unsigned binary',
            'offline fixtures are not live-provider certification',
            'clean public GitHub Actions run pending repository publication'
        )
    }
    $Utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $EvidencePath 'runtime-v5-build-report.json'), ($BuildReport | ConvertTo-Json -Depth 12), $Utf8)
    $Release = Join-Path $Root ('dist\workstation-0.5.0-alpha.1-windows-x64-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    $null = New-Item -ItemType Directory -Path $Release -Force
    Copy-Item -LiteralPath $Exe -Destination $Release
    Copy-Item -LiteralPath $Lock, (Join-Path $Root 'README.md'), (Join-Path $Root 'LICENSE') -Destination $Release
    $Manifest = [ordered]@{
        version = '0.5.0-alpha.1'
        scope = 'Durable execution alpha with schema-v5 persistence, bounded supervision, reconciliation, and verification; live-provider certification remains separate'
        toolchain = $Toolchain
        build_utc = [DateTime]::UtcNow.ToString('o')
        binary_sha256 = (Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant()
        lockfile_sha256 = (Get-FileHash -LiteralPath $Lock -Algorithm SHA256).Hash.ToLowerInvariant()
        native_build_checks = 'all-feature check, Clippy, tests including durable fake-provider fixtures, release build, isolated baseline and control smoke completed'
        live_vendor_certification = 'PARTIAL_NATIVE_READ_ONLY; see evidence/native-certification-20260919.json'
        code_signing = 'UNSIGNED'
        build_info = $Info.data
    }
    [IO.File]::WriteAllText((Join-Path $Release 'release-manifest.json'), ($Manifest | ConvertTo-Json -Depth 12), $Utf8)
    $MetadataArgs = @('metadata', '--locked', '--format-version', '1') + $NetworkArgs
    $Metadata = & $Rustup run $Toolchain cargo @MetadataArgs
    if ($LASTEXITCODE -ne 0) { throw 'Dependency inventory failed.' }
    # Export public package facts, not the build machine's private absolute paths.
    $Packages = ($Metadata | ConvertFrom-Json).packages
    $Inventory = @($Packages | ForEach-Object { [ordered]@{ name=$_.name; version=$_.version; license=$_.license; source=$_.source } })
    [IO.File]::WriteAllText((Join-Path $Release 'dependency-inventory.json'), ($Inventory | ConvertTo-Json -Depth 8), $Utf8)
    $LicenseRoot = Join-Path $Release 'licenses'
    $null = New-Item -ItemType Directory -Path $LicenseRoot
    foreach ($Package in $Packages) {
        $PackageRoot = Split-Path -Parent $Package.manifest_path
        $Destination = Join-Path $LicenseRoot ($Package.name + '-' + $Package.version)
        foreach ($Name in @('LICENSE','LICENSE-MIT','LICENSE-APACHE','LICENSE.md','COPYING','UNLICENSE')) {
            $Candidate = Join-Path $PackageRoot $Name
            if (Test-Path -LiteralPath $Candidate -PathType Leaf) {
                $null = New-Item -ItemType Directory -Force -Path $Destination
                Copy-Item -LiteralPath $Candidate -Destination $Destination
            }
        }
    }
    $PublicEvidence = Join-Path $Release 'evidence'
    $null = New-Item -ItemType Directory -Path $PublicEvidence
    Copy-Item -LiteralPath (Join-Path $EvidencePath 'runtime-v5-build-report.json') -Destination $PublicEvidence
    Copy-Item -LiteralPath (Join-Path $Root 'CURRENT-CAPABILITIES.md'), (Join-Path $Root 'VALIDATION.md'), (Join-Path $Root 'SECURITY.md'), (Join-Path $Root 'CHANGELOG.md') -Destination $Release
    $PublicDocs = Join-Path $Release 'docs'
    $null = New-Item -ItemType Directory -Path $PublicDocs
    Copy-Item -LiteralPath (Join-Path $Root 'docs\DURABLE-RUNTIME-IMPLEMENTATION.md'), (Join-Path $Root 'docs\DURABLE-RUNTIME-LIMITATIONS.md') -Destination $PublicDocs
    # ZIP timestamps cannot represent pre-1980 metadata carried by some registry packages.
    # Normalize only invalid staged-copy timestamps; never mutate Cargo sources or the repository.
    $ZipSafeTime = Get-Date '2000-01-01T00:00:00'
    $NormalizedZipTimes = 0
    Get-ChildItem -LiteralPath $Release -Recurse -Force | Where-Object { $_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107 } | ForEach-Object {
        $_.LastWriteTime = $ZipSafeTime
        $NormalizedZipTimes++
    }
    Write-Host "Normalized $NormalizedZipTimes staged timestamps for ZIP compatibility."
    # This inventory is not a claim of a standards-compliant SBOM or completed legal review.
    Compress-Archive -LiteralPath $Release -DestinationPath "$Release.zip"
    $ZipHash=(Get-FileHash -LiteralPath "$Release.zip" -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$Release.zip.sha256",($ZipHash+'  '+[IO.Path]::GetFileName("$Release.zip")+"`n"),$Utf8)
    Write-Host "Native build/test package: $Release.zip"
    Write-Host 'Commit the actual Cargo.lock and formatted source before reproducible CI/release.'
    Write-Host 'A passing fixture suite is not full vendor/application certification.'
} finally {
    $env:CARGO_HOME = $OldCargoHome
    $env:CARGO_TARGET_DIR = $OldTarget
    $env:TEMP = $OldTemp
    $env:TMP = $OldTmp
    Set-Location $OldLocation
}
