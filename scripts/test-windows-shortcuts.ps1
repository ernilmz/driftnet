$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true') { throw 'Disposable Windows CI only' }
$install = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Drift'
$programs = [Environment]::GetFolderPath('CommonPrograms')
$desktop = [Environment]::GetFolderPath('CommonDesktopDirectory')
$oldMenu = Join-Path $programs 'Drift'
$newMenu = Join-Path $programs 'Driftnet'
$oldDesktop = Join-Path $desktop 'Drift.lnk'
$newDesktop = Join-Path $desktop 'Driftnet.lnk'
foreach ($path in @($install, $oldMenu, $newMenu, $oldDesktop, $newDesktop)) {
    if (Test-Path $path) { throw "Refusing to modify pre-existing fixture path $path" }
}
$reportPath = Join-Path $PSScriptRoot '../target/shortcut-fixture.json'
$report = [ordered]@{ phase = 'prepare'; legacyRemoved = $false; currentRemoved = $false; unrelatedPreserved = $false; error = $null }
try {
    foreach ($path in @($install, $oldMenu, $newMenu)) { New-Item -ItemType Directory $path | Out-Null }
    $target = Join-Path $install 'drift-desktop.exe'
    Copy-Item (Join-Path ([Environment]::SystemDirectory) 'whoami.exe') $target
    $shell = [Activator]::CreateInstance([type]::GetTypeFromProgID('WScript.Shell'))
    try {
        foreach ($spec in @(
            @{ path=(Join-Path $oldMenu 'Drift.lnk'); target=$target },
            @{ path=$oldDesktop; target=$target },
            @{ path=(Join-Path $newMenu 'Driftnet.lnk'); target=$target },
            @{ path=$newDesktop; target=$target },
            @{ path=(Join-Path $newMenu 'Unrelated.lnk'); target=(Join-Path ([Environment]::SystemDirectory) 'notepad.exe') }
        )) {
            $link = $shell.CreateShortcut($spec.path)
            try { $link.TargetPath = $spec.target; $link.Save() } finally { $null = [Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
        }
    } finally { $null = [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    $report.phase = 'native-nsis-powershell5'
    $p = Start-Process (Join-Path $PSScriptRoot '../target/Driftnet-Shortcut-Fixture.exe') -ArgumentList '/S' -PassThru
    if (-not $p.WaitForExit(45000)) { $p.Kill($true); throw 'Shortcut fixture timed out' }
    if ($p.ExitCode -ne 0) {
        $detail = if (Test-Path (Join-Path $install 'fixture-error.txt')) { Get-Content (Join-Path $install 'fixture-error.txt') -Raw } else { '' }
        throw "Fixture exit $($p.ExitCode): $detail"
    }
    $report.legacyRemoved = -not (Test-Path (Join-Path $oldMenu 'Drift.lnk')) -and -not (Test-Path $oldDesktop)
    $report.currentRemoved = -not (Test-Path (Join-Path $newMenu 'Driftnet.lnk')) -and -not (Test-Path $newDesktop)
    $report.unrelatedPreserved = Test-Path (Join-Path $newMenu 'Unrelated.lnk')
    if (-not $report.legacyRemoved -or -not $report.currentRemoved -or -not $report.unrelatedPreserved) { throw 'Shortcut cleanup assertions failed' }
    $report.phase = 'passed'
} catch { $report.error = $_.Exception.Message; throw }
finally {
    foreach ($path in @($oldDesktop, $newDesktop, $oldMenu, $newMenu, $install)) {
        if (Test-Path $path) { Remove-Item $path -Recurse -Force }
    }
    $report | ConvertTo-Json | Set-Content $reportPath
    Write-Host ($report | ConvertTo-Json -Compress)
}
