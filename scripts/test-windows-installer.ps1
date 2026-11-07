# Native, elevated integration test for disposable Windows GitHub runners only.
# It changes routes temporarily, installs a service, and uninstalls the test app.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [string]$Report = "$PSScriptRoot/../target/windows-installer-test.json"
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true') {
    throw 'Run this destructive integration test only on an ephemeral Windows GitHub Actions runner.'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'The disposable runner must already be elevated; this test never prompts for credentials.'
}
$Installer = (Resolve-Path $Installer).Path
$installDir = Join-Path $env:RUNNER_TEMP ('DriftnetTest-' + [guid]::NewGuid().ToString('N'))
$configDir = Join-Path $env:APPDATA 'app.drift.desktop'
$rulesPath = Join-Path $configDir 'rules.json'
$savedRules = if (Test-Path $rulesPath) { [IO.File]::ReadAllBytes($rulesPath) } else { $null }
$results = [ordered]@{
    os = [Environment]::OSVersion.VersionString
    installer = [IO.Path]::GetFileName($Installer)
    install = $false; packagedEngine = $false; repairPreservesRules = $false
    selectedRoute = $false; unselectedRoute = $false; leaseCleanup = $false; crashLeaseCleanup = $false
    uninstall = $false; error = $null
}
$lease = $null
$engine = $null
$directCurl = Join-Path $env:RUNNER_TEMP ('driftnet-direct-' + [guid]::NewGuid().ToString('N') + '.exe')

function Wait-Until([scriptblock]$Check, [string]$Description, [int]$Seconds = 30) {
    $limit = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        if (& $Check) { return }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $limit)
    throw "Timed out: $Description"
}
function Run-Checked([string]$File, [string[]]$Arguments, [int]$Seconds = 120) {
    $process = Start-Process -FilePath $File -ArgumentList $Arguments -PassThru
    if (-not $process.WaitForExit($Seconds * 1000)) {
        $process.Kill($true)
        throw "Timed out running $File"
    }
    if ($process.ExitCode -ne 0) { throw "$File exited with $($process.ExitCode)" }
}
function Wait-ServiceReady {
    Wait-Until {
        $service = Get-Service -Name DriftNetwork -ErrorAction SilentlyContinue
        $null -ne $service -and $service.Status -eq 'Running'
    } 'DriftNetwork service running'
}
function Open-Control {
    # The service authenticates the caller SID by named-pipe impersonation.
    $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', 'Drift.Network',
        [IO.Pipes.PipeDirection]::InOut, [IO.Pipes.PipeOptions]::Asynchronous,
        [Security.Principal.TokenImpersonationLevel]::Impersonation)
    try {
        $pipe.Connect(10000)
        $writer = [IO.StreamWriter]::new($pipe, [Text.UTF8Encoding]::new($false), 1024, $true)
        $writer.AutoFlush = $true
        $reader = [IO.StreamReader]::new($pipe, [Text.UTF8Encoding]::new($false), $false, 1024, $true)
        return @{ Pipe = $pipe; Writer = $writer; Reader = $reader }
    } catch { $pipe.Dispose(); throw }
}
function Read-Control($Control) {
    $read = $Control.Reader.ReadLineAsync()
    if (-not $read.Wait(30000)) { throw 'Network control reply timed out' }
    if ($null -eq $read.Result) { throw 'Network control pipe closed before replying' }
    return ($read.Result | ConvertFrom-Json)
}
function Close-Control($Control) {
    if ($null -ne $Control) {
        $Control.Reader.Dispose(); $Control.Writer.Dispose(); $Control.Pipe.Dispose()
    }
}
function Get-RoutingStats {
    $control = Open-Control
    try {
        $control.Writer.WriteLine('{"action":"stats"}')
        return (Read-Control $control)
    } finally { Close-Control $control }
}
function Stop-Lease {
    if ($null -ne $script:lease) {
        try {
            $script:lease.Writer.WriteLine('stop')
            $reply = Read-Control $script:lease
            if (-not $reply.ok) { throw "Network teardown failed: $($reply.error)" }
        } finally {
            Close-Control $script:lease
            $script:lease = $null
        }
    }
}
function Assert-Curl([string]$Curl) {
    # Public HTTPS, no credentials, no browser/Discord dependency, no proxy env.
    $status = & $Curl --ipv4 --noproxy '*' --silent --show-error --fail --connect-timeout 15 --max-time 35 --output NUL --write-out '%{http_code}' https://example.com/
    if ($LASTEXITCODE -ne 0 -or $status -ne '200') { throw "HTTPS through $Curl failed: $status (exit $LASTEXITCODE)" }
}

try {
    if (Get-Service -Name DriftNetwork -ErrorAction SilentlyContinue) {
        throw 'Runner already has DriftNetwork installed; refusing to modify a pre-existing service.'
    }
    Write-Host 'Installing the optimized NSIS package without launching its GUI'
    # NSIS /D must be last and must not be quoted (RUNNER_TEMP has no spaces).
    if ($installDir.Contains(' ')) { throw 'Test installation path must have no spaces for NSIS /D.' }
    Run-Checked $Installer @('/S', "/D=$installDir") 300
    Wait-ServiceReady
    $desktop = Join-Path $installDir 'drift-desktop.exe'
    $router = Join-Path $installDir 'drift-router.exe'
    $enginePath = Join-Path $installDir 'drift-engine.exe'
    foreach ($path in @($desktop, $router, $enginePath, (Join-Path $installDir 'uninstall.exe'))) {
        if (-not (Test-Path $path -PathType Leaf)) { throw "Missing installed resource: $path" }
    }
    $results.install = $true
    Run-Checked $desktop @('--check-engine')
    $results.packagedEngine = $true

    New-Item -ItemType Directory -Force -Path $configDir | Out-Null
    $fixture = '{"schema_version":1,"rules":[{"id":"c4ec487a-0111-4eec-9222-0123456789ab","enabled":true,"target":{"kind":"site","domain":"example.com","subdomains":true}}]}'
    [IO.File]::WriteAllText($rulesPath, $fixture, [Text.UTF8Encoding]::new($false))
    Write-Host 'Re-running setup and checking user rules survive'
    Run-Checked $Installer @('/S', "/D=$installDir") 300
    Wait-ServiceReady
    if ([IO.File]::ReadAllText($rulesPath) -cne $fixture) { throw 'Setup changed existing per-user rules' }
    Run-Checked $desktop @('--check-engine')
    $results.repairPreservesRules = $true

    # Read the original physical NIC before creating TUN, then bind the engine's
    # outbound sockets to it so fragments cannot be reassembled by the tunnel.
    $interfaceFile = Join-Path $env:RUNNER_TEMP 'driftnet-uplink.txt'
    $p = Start-Process -FilePath $router -ArgumentList '--interface-index' -PassThru -RedirectStandardOutput $interfaceFile
    if (-not $p.WaitForExit(15000) -or $p.ExitCode -ne 0) { throw 'Physical interface lookup failed' }
    $interface = [uint32]([IO.File]::ReadAllText($interfaceFile).Trim())
    if ($interface -eq 0) { throw 'Invalid physical interface index' }
    Remove-Item $interfaceFile -Force

    $portReservation = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $portReservation.Start()
    $port = $portReservation.LocalEndpoint.Port
    $portReservation.Stop()
    $engineStart = [Diagnostics.ProcessStartInfo]::new($enginePath)
    $engineStart.UseShellExecute = $false
    $engineStart.RedirectStandardInput = $true
    $engineStart.CreateNoWindow = $true
    foreach ($arg in @('--ip', '127.0.0.1', '--port', "$port", '--max-conn', '256', '--split', '2', '--tlsrec', '1+s')) {
        $engineStart.ArgumentList.Add($arg)
    }
    $engineStart.Environment['DRIFT_OUTBOUND_IFINDEX'] = "$interface"
    $engineStart.Environment.Remove('SS_LOCAL_PORT') | Out-Null
    $engineStart.Environment.Remove('SS_PLUGIN_OPTIONS') | Out-Null
    $engine = [Diagnostics.Process]::Start($engineStart)
    Wait-Until {
        $tcp = [Net.Sockets.TcpClient]::new()
        try {
            $tcp.Connect('127.0.0.1', $port)
            $stream = $tcp.GetStream()
            $stream.ReadTimeout = 1500
            $stream.Write([byte[]]@(5, 1, 0))
            return ($stream.ReadByte() -eq 5 -and $stream.ReadByte() -eq 0)
        } catch { return $false } finally { $tcp.Dispose() }
    } 'packaged SOCKS engine readiness' 15

    $curl = (Get-Command curl.exe -CommandType Application | Select-Object -First 1).Source
    Copy-Item $curl $directCurl
    $lease = Open-Control
    $request = @{ url = ''; engine_port = $port; interface_index = $interface; apps = @(@{path = $curl; bundle = $false}) }
    $lease.Writer.WriteLine(($request | ConvertTo-Json -Compress -Depth 5))
    $reply = Read-Control $lease
    if (-not $reply.ok -or -not $reply.router -or $reply.protocol -ne 2) { throw "TUN did not start: $($reply | ConvertTo-Json -Compress)" }
    $tun = @(Get-NetAdapter -IncludeHidden | Where-Object Name -eq 'Drift')
    if ($tun.Count -ne 1) { throw 'The native Wintun adapter was not created' }
    $tunIndex = $tun[0].ifIndex
    if (@(Get-NetRoute -InterfaceIndex $tunIndex -ErrorAction SilentlyContinue).Count -eq 0) { throw 'TUN has no routes' }
    $before = Get-RoutingStats
    Assert-Curl $curl
    $afterSelected = Get-RoutingStats
    if ($afterSelected.selected -le $before.selected) { throw 'Selected app traffic was not attributed to the selected route' }
    $results.selectedRoute = $true
    Assert-Curl $directCurl
    $afterDirect = Get-RoutingStats
    if ($afterDirect.direct -le $afterSelected.direct) { throw 'Unselected app traffic was not attributed to the direct route' }
    if ($afterDirect.selected -ne $afterSelected.selected) { throw 'Unselected app unexpectedly incremented the selected route counter' }
    $results.unselectedRoute = $true
    $results.stats = $afterDirect
    Stop-Lease
    Wait-Until {
        @(Get-NetRoute -InterfaceIndex $tunIndex -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/1', '128.0.0.0/1', '0.0.0.0/0', '::/0', '::/1', '8000::/1') }).Count -eq 0
    } 'TUN default routes removed after access stops'
    Assert-Curl $curl
    $results.leaseCleanup = $true
    # A desktop crash/EOF must also release routes, and a fresh lease must work
    # without reinstalling the service or flipping the global network settings.
    $lease = Open-Control
    $lease.Writer.WriteLine(($request | ConvertTo-Json -Compress -Depth 5))
    $reply = Read-Control $lease
    if (-not $reply.ok -or -not $reply.router) { throw 'A second access session could not start after stopping the first' }
    $crashTun = @(Get-NetAdapter -IncludeHidden | Where-Object Name -eq 'Drift')
    if ($crashTun.Count -ne 1) { throw 'Restarted session has no Wintun adapter' }
    $crashTunIndex = $crashTun[0].ifIndex
    Close-Control $lease
    $lease = $null
    Wait-Until {
        @(Get-NetRoute -InterfaceIndex $crashTunIndex -ErrorAction SilentlyContinue | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/1', '128.0.0.0/1', '0.0.0.0/0', '::/0', '::/1', '8000::/1') }).Count -eq 0
    } 'TUN default routes removed after desktop lifetime pipe closes'
    Assert-Curl $curl
    $results.crashLeaseCleanup = $true
    $engine.StandardInput.Close()
    if (-not $engine.WaitForExit(10000)) { throw 'Engine survived closing its lifetime pipe' }
    $engine.Dispose(); $engine = $null

    Write-Host 'Uninstalling and checking service, program files, and rules'
    Run-Checked (Join-Path $installDir 'uninstall.exe') @('/S') 120
    Wait-Until { $null -eq (Get-Service -Name DriftNetwork -ErrorAction SilentlyContinue) } 'service removal'
    Wait-Until { -not (Test-Path $desktop) -and -not (Test-Path $router) -and -not (Test-Path $enginePath) -and -not (Test-Path (Join-Path $installDir 'uninstall.exe')) } 'installed executables removal'
    if ([IO.File]::ReadAllText($rulesPath) -cne $fixture) { throw 'Uninstall unexpectedly removed user rules' }
    $results.uninstall = $true
} catch {
    $results.error = $_.Exception.Message
    throw
} finally {
    try { Stop-Lease } catch { Write-Warning "Lease cleanup: $_" }
    if ($null -ne $engine) {
        $engine.StandardInput.Close()
        if (-not $engine.WaitForExit(10000)) { $engine.Kill($true) }
        $engine.Dispose()
    }
    # Always attempt teardown after a failed assertion; this is a throwaway host.
    $cleanupRouter = Join-Path $installDir 'drift-router.exe'
    if ((Get-Service -Name DriftNetwork -ErrorAction SilentlyContinue) -and (Test-Path $cleanupRouter)) {
        try { Run-Checked $cleanupRouter @('--uninstall-service') 45 } catch { Write-Warning "Service cleanup: $_" }
    }
    if ($null -ne $savedRules) { [IO.File]::WriteAllBytes($rulesPath, $savedRules) }
    elseif (Test-Path $rulesPath) { Remove-Item $rulesPath -Force }
    if (Test-Path $directCurl) { Remove-Item $directCurl -Force }
    if (Test-Path $installDir) { Remove-Item $installDir -Recurse -Force -ErrorAction Continue }
    New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Report))) | Out-Null
    $results | ConvertTo-Json -Depth 6 | Set-Content -Path $Report -Encoding utf8
    Write-Host ($results | ConvertTo-Json -Compress -Depth 6)
}
