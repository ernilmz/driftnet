# Repackage only a fixed, previously network-tested 1.0.1 payload on disposable CI.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or $env:GITHUB_REPOSITORY -ne 'ernilmz/driftnet') {
    throw 'Run only in the trusted disposable Driftnet Windows workflow.'
}
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$baseline = 'f108eee5c06863b0944f4613d3cfd52e73c3eefe'
$runId = '42375361885'
$artifactId = '13575921225'
$artifactDigest = '5fd5f5fe82bb725132c6cf9341cc12858596b902ab06b74339485d734de34466'
$reportPath = Join-Path $root 'target/windows-repack.json'
$report = [ordered]@{ baselineCommit=$baseline; baselineRun=$runId; artifactId=$artifactId; artifactSHA256=$artifactDigest; version='1.0.1'; sourceGuard=$false; baselineNetworkEvidence=$false; exactPayload=$false; payload=@{}; installer=$null; error=$null }
New-Item -ItemType Directory -Path (Split-Path $reportPath) -Force | Out-Null
try {
    # Anything outside this installer/test/documentation allowlist requires the
    # ordinary full build. In particular no Rust/C/Go/UI/config/lock changes pass.
    $changed = @(& git diff --name-only "$baseline..HEAD")
    if ($LASTEXITCODE -ne 0) { throw 'Could not compare the pinned payload source commit.' }
    foreach ($path in $changed) {
        $allowed = $path -match '^(docs/|\.github/workflows/)' -or
            $path -in @('README.md','CHANGELOG.md','CONTRIBUTING.md','SECURITY.md',
                'native/windows/installer-hooks.nsh','native/windows/installer-template.nsi',
                'native/windows/validate-install.ps1','native/windows/validator-embedded.nsh',
                'native/windows/NSIS_UPSTREAM.json','native/windows/README.md',
                'scripts/repack-windows-installer.ps1','scripts/test-windows-shortcuts.ps1',
                'scripts/windows-shortcut-fixture.nsi','scripts/test-windows-installer.ps1')
        if (-not $allowed) { throw "Payload-related source changed ($path); use the full native build workflow." }
    }
    $config = Get-Content apps/desktop/tauri.conf.json -Raw | ConvertFrom-Json
    if ($config.version -ne '1.0.1') { throw 'The pinned payload is only valid for product version 1.0.1.' }
    $report.sourceGuard = $true
    $report.changedPaths = $changed
    $headers = @{ Authorization="Bearer $env:GH_TOKEN"; Accept='application/vnd.github+json'; 'X-GitHub-Api-Version'='2022-11-28' }
    $api = 'https://api.github.com/repos/ernilmz/driftnet'
    $run = Invoke-RestMethod "$api/actions/runs/$runId" -Headers $headers
    $artifact = Invoke-RestMethod "$api/actions/artifacts/$artifactId" -Headers $headers
    if ($run.head_sha -ne $baseline -or $run.status -ne 'completed' -or
        $artifact.name -ne 'Driftnet-Windows-native' -or $artifact.expired -or
        "$($artifact.workflow_run.id)" -ne $runId -or $artifact.workflow_run.head_sha -ne $baseline -or
        $artifact.digest -ne "sha256:$artifactDigest") { throw 'Pinned artifact/run provenance mismatch.' }
    $work = Join-Path $env:RUNNER_TEMP 'driftnet-windows-repack'
    if (Test-Path $work) { throw 'Refusing to reuse an existing repack workspace.' }
    New-Item -ItemType Directory $work | Out-Null
    $zip = Join-Path $work 'source.zip'
    Invoke-WebRequest "$api/actions/artifacts/$artifactId/zip" -Headers $headers -OutFile $zip
    if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne $artifactDigest) { throw 'Source artifact ZIP checksum mismatch.' }
    Expand-Archive $zip (Join-Path $work 'artifact')
    $oldReports = @(Get-ChildItem (Join-Path $work 'artifact') -Filter windows-installer-test.json -Recurse)
    if ($oldReports.Count -ne 1) { throw 'Expected the baseline native network evidence.' }
    $oldReport = Get-Content $oldReports[0].FullName -Raw | ConvertFrom-Json
    foreach ($field in @('install','runtimeImports','protectedService','packagedEngine','repairPreservesRules','selectedRoute','unselectedRoute','leaseCleanup','crashLeaseCleanup')) {
        if ($oldReport.$field -ne $true) { throw "Baseline payload lacks passing evidence: $field" }
    }
    $report.baselineNetworkEvidence = $true
    # Official 7-Zip 26.06 x64 release, SHA256 pinned from its release asset.
    $toolInstaller = Join-Path $work '7z2606-x64.exe'
    Invoke-WebRequest 'https://github.com/ip7z/7zip/releases/download/26.06/7z2606-x64.exe' -OutFile $toolInstaller
    if ((Get-FileHash $toolInstaller -Algorithm SHA256).Hash -ne '9B697F65ECC2D9B0152F5EE96D72DDE522A7827690349D9307935DACF40F42EC') { throw 'Official 7-Zip checksum mismatch.' }
    $tools = Join-Path $work '7zip'
    $p = Start-Process $toolInstaller -ArgumentList @('/S', "/D=$tools") -PassThru -Wait
    if ($p.ExitCode -ne 0) { throw 'Could not prepare the verified extractor.' }
    $seven = Join-Path $tools '7z.exe'
    if (-not (Test-Path $seven)) { throw 'Verified 7-Zip executable missing.' }
    $installers = @(Get-ChildItem (Join-Path $work 'artifact') -Filter '*-setup.exe' -Recurse)
    if ($installers.Count -ne 1) { throw 'Expected exactly one source NSIS installer.' }
    $before = Join-Path $work 'before'
    & $seven x -y "-o$before" $installers[0].FullName | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Source NSIS extraction failed.' }
    $names = @('drift-desktop.exe','drift-agent.exe','drift-engine.exe','drift-router.exe')
    foreach ($name in $names) {
        $source = Join-Path $before $name
        if (-not (Test-Path $source -PathType Leaf)) { throw "Missing immutable payload file $name" }
        $report.payload[$name] = @{ before=(Get-FileHash $source -Algorithm SHA256).Hash; after=$null }
    }
    # Explicit target output avoids depending on Cargo's implicit host path.
    foreach ($directory in @('target/release','target/x86_64-pc-windows-msvc/release','apps/desktop/binaries')) {
        New-Item -ItemType Directory $directory -Force | Out-Null
    }
    Copy-Item (Join-Path $before 'drift-desktop.exe') target/release/drift-desktop.exe
    Copy-Item (Join-Path $before 'drift-desktop.exe') target/x86_64-pc-windows-msvc/release/drift-desktop.exe
    foreach ($name in @('drift-agent','drift-engine','drift-router')) {
        Copy-Item (Join-Path $before "$name.exe") "apps/desktop/binaries/$name-x86_64-pc-windows-msvc.exe"
    }
    $env:CARGO_TARGET_DIR = Join-Path $root 'target'
    Push-Location apps/desktop
    try {
        & ../../ui/node_modules/.bin/tauri.cmd bundle --target x86_64-pc-windows-msvc --bundles nsis --no-binary-patching --no-sign --ci
        if ($LASTEXITCODE -ne 0) { throw 'Installer-only bundling failed; use the full build workflow.' }
    } finally { Pop-Location }
    $packages = @(Get-ChildItem target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe)
    if ($packages.Count -ne 1) { throw 'Expected exactly one repacked installer.' }
    $version = $packages[0].VersionInfo.ProductVersion
    if ($version -notmatch '^1\.0\.1(?:\.0)?$') { throw "Unexpected installer product version $version" }
    $after = Join-Path $work 'after'
    & $seven x -y "-o$after" $packages[0].FullName | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Repacked NSIS extraction failed.' }
    foreach ($name in $names) {
        $hash = (Get-FileHash (Join-Path $after $name) -Algorithm SHA256).Hash
        $report.payload[$name].after = $hash
        if ($hash -ne $report.payload[$name].before) { throw "Immutable payload changed: $name" }
    }
    $report.exactPayload = $true
    $report.installer = $packages[0].FullName
    $report.installerSHA256 = (Get-FileHash $packages[0].FullName -Algorithm SHA256).Hash
    $packages[0].FullName | Set-Content target/windows-repacked-path.txt
} catch { $report.error = $_.Exception.Message; throw }
finally {
    $report | ConvertTo-Json -Depth 8 | Set-Content $reportPath -Encoding utf8
    Write-Host ($report | ConvertTo-Json -Depth 8 -Compress)
}
