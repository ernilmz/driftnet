param(
    [Parameter(Mandatory = $true)][string]$InstallDirectory,
    [ValidateSet('Prepare', 'Remove')][string]$Mode = 'Prepare'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Called by the elevated installer, before copying or executing any installed
# helper. Program Files comes from the Windows known-folder API, not the caller's
# environment. No existing executable or registry command is run here.
$root = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
$directory = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
$allowed = @((Join-Path $root 'Driftnet'), (Join-Path $root 'Drift'))
if ($directory -notin $allowed) { throw 'Install Driftnet in its protected Program Files location.' }
$trusted = @('S-1-5-18', 'S-1-5-32-544', 'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
function Assert-ProtectedItem([IO.FileSystemInfo]$Item) {
    if (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'The installation contains a reparse point. Remove the old installation before retrying.' }
    $acl = Get-Acl -LiteralPath $Item.FullName
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trusted) { throw 'The installation has an unsafe owner. Remove the old installation before retrying.' }
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -ne 'Allow' -or ($rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) -ne 0) { continue }
        if (([int64]$rule.FileSystemRights -band 0x500d0156) -ne 0 -and $rule.IdentityReference.Value -notin $trusted) {
            throw 'The installation is writable by an unprivileged account. Remove the old installation before retrying.'
        }
    }
}
# Validate ancestors before traversing children. An unsafe ancestor is rejected
# before any descendant can be replaced, read, or executed with elevation.
Assert-ProtectedItem (Get-Item -LiteralPath $root -Force)
if (Test-Path -LiteralPath $directory) {
    $queue = [Collections.Generic.Queue[IO.FileSystemInfo]]::new()
    $queue.Enqueue((Get-Item -LiteralPath $directory -Force))
    while ($queue.Count -gt 0) {
        $item = $queue.Dequeue()
        Assert-ProtectedItem $item
        if ($item -is [IO.DirectoryInfo]) {
            foreach ($child in $item.EnumerateFileSystemInfos()) { $queue.Enqueue($child) }
        }
    }
}
$service = Get-Service -Name DriftNetwork -ErrorAction SilentlyContinue
if ($null -ne $service) {
    try {
        if ($service.Status -ne 'Stopped') {
            if ($service.Status -ne 'StopPending') { $service.Stop() }
            $service.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(25))
        }
    } finally { $service.Dispose() }
    & (Join-Path ([Environment]::SystemDirectory) 'sc.exe') delete DriftNetwork | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'The networking service could not be removed.' }
    Start-Sleep -Milliseconds 500
}
