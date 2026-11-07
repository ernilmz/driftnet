param([bool]$UseRebuiltRouter = $false)
# Private, disposable-runner diagnostics. Never run on an end user's machine.
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true') { throw 'Disposable Windows CI only' }
$output = Join-Path $PSScriptRoot '../target/windows-diagnostics'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$install = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Drift'
if (Test-Path $install) { throw 'Refusing to touch a pre-existing installation directory' }
$packages = @(Get-ChildItem (Join-Path $PSScriptRoot '../target/diagnostic-input') -Recurse -Filter '*-setup.exe')
if ($packages.Count -ne 1) { throw 'Expected exactly one authenticated workflow installer artifact' }
if (Get-Service DriftNetwork -ErrorAction SilentlyContinue) { throw 'Refusing to touch a pre-existing service' }
$report = [ordered]@{ installerExit = $null; files = @(); installServiceExit = $null; error = $null }
function Snapshot-Service([string]$Name) {
    Get-CimInstance Win32_Service -Filter "Name='DriftNetwork'" |
        Select-Object Name,State,StartName,StartMode,PathName,ProcessId,ExitCode |
        ConvertTo-Json | Set-Content (Join-Path $output "$Name.json")
}
try {
    $p = Start-Process $packages[0].FullName -ArgumentList @('/S', "/D=$install") -PassThru
    if (-not $p.WaitForExit(180000)) { $p.Kill($true); throw 'Installer timed out' }
    $report.installerExit = $p.ExitCode
    if (Test-Path $install) { $report.files = @(Get-ChildItem $install -Recurse -File | ForEach-Object { $_.FullName.Substring($install.Length + 1) }) }
    Snapshot-Service 'service-after-installer'
    $router = Join-Path $install 'drift-router.exe'
    if ($UseRebuiltRouter) {
        $service = Get-Service DriftNetwork -ErrorAction SilentlyContinue
        if ($null -ne $service) {
            try { $service.Stop(); $service.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(25)) } finally { $service.Dispose() }
            & sc.exe delete DriftNetwork | Out-Null
            Start-Sleep -Seconds 1
        }
        Copy-Item (Join-Path $PSScriptRoot '../apps/desktop/binaries/drift-router-x86_64-pc-windows-msvc.exe') $router -Force
    }
    if (Test-Path $router) {
        $p = Start-Process $router -ArgumentList '--install-service' -PassThru `
            -RedirectStandardOutput (Join-Path $output 'router-install.stdout.txt') `
            -RedirectStandardError (Join-Path $output 'router-install.stderr.txt')
        if (-not $p.WaitForExit(45000)) { $p.Kill($true); throw 'Service registration timed out' }
        $report.installServiceExit = $p.ExitCode
        Snapshot-Service 'service-after-manual-registration'
        & sc.exe qc DriftNetwork | Set-Content (Join-Path $output 'sc-config.txt')
        & sc.exe sdshow DriftNetwork | Set-Content (Join-Path $output 'sc-security.txt')
    }
    Get-WinEvent -FilterHashtable @{LogName='System'; ProviderName='Service Control Manager'; StartTime=(Get-Date).AddMinutes(-10)} -ErrorAction SilentlyContinue |
        Where-Object Message -Match 'Drift' | Select-Object Id,LevelDisplayName,TimeCreated,Message |
        ConvertTo-Json -Depth 4 | Set-Content (Join-Path $output 'service-events.json')
} catch {
    $report.error = $_.Exception.Message
} finally {
    $router = Join-Path $install 'drift-router.exe'
    if (Test-Path $router) {
        $p = Start-Process $router -ArgumentList '--uninstall-service' -PassThru
        if (-not $p.WaitForExit(30000)) { $p.Kill($true) }
    }
    if (Test-Path $install) { Remove-Item $install -Recurse -Force -ErrorAction Continue }
    $report | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $output 'result.json')
    Write-Host ($report | ConvertTo-Json -Compress -Depth 5)
    Get-ChildItem $output -Filter '*.stderr.txt' | ForEach-Object { Write-Host $_.Name; Get-Content $_.FullName }
}
