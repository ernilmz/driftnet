[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$InstallDirectory,
    [ValidateSet('Validate', 'Prepare', 'Remove', 'Shortcut', 'CleanupLegacy', 'CleanupAll')][string]$Mode = 'Prepare',
    [string]$ShortcutPath = '',
    [string]$ShortcutTarget = '',
    [string]$StartMenuFolder = 'Driftnet'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules')
$PSModuleAutoLoadingPreference = 'ModuleQualified'
# Called by the elevated installer, before copying or executing any installed
# helper. Program Files comes from the Windows known-folder API, not the caller's
# environment. No existing executable or registry command is run here.
$root = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
$directory = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
$allowed = @([IO.Path]::Combine($root, 'Driftnet'), [IO.Path]::Combine($root, 'Drift'))
if ($directory -notin $allowed) { throw 'Install Driftnet in its protected Program Files location.' }
$trusted = @('S-1-5-18', 'S-1-5-32-544', 'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
$shortcutTrusted = @($trusted)
$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $principal = [Security.Principal.WindowsPrincipal]::new($currentIdentity)
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        $shortcutTrusted += $currentIdentity.User.Value
    }
} finally { $currentIdentity.Dispose() }
function Assert-ProtectedItem([IO.FileSystemInfo]$Item) {
    if (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'The installation contains a reparse point. Remove the old installation before retrying.' }
    $acl = Microsoft.PowerShell.Security\Get-Acl -LiteralPath $Item.FullName
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trusted) { throw 'The installation has an unsafe owner. Remove the old installation before retrying.' }
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -ne 'Allow' -or ($rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) -ne 0) { continue }
        if (([int64]$rule.FileSystemRights -band 0x500d0156) -ne 0 -and $rule.IdentityReference.Value -notin $trusted) {
            throw 'The installation is writable by an unprivileged account. Remove the old installation before retrying.'
        }
    }
}
function Assert-ShortcutDirectory([IO.DirectoryInfo]$Item) {
    # Common Start Menu/Desktop directories may permit NEW entries. Creation
    # alone cannot replace an existing protected link. Keep all mutation,
    # deletion, ownership, ACL, and reparse-point checks for these ancestors.
    if (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Shortcut directory is a reparse point.' }
    $acl = Microsoft.PowerShell.Security\Get-Acl -LiteralPath $Item.FullName
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $shortcutTrusted) { throw 'Shortcut directory has an unsafe owner.' }
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -ne 'Allow' -or ($rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) -ne 0) { continue }
        if (([int64]$rule.FileSystemRights -band 0x500d0150) -ne 0 -and $rule.IdentityReference.Value -notin $shortcutTrusted) {
            Microsoft.PowerShell.Utility\Write-Verbose "Rejected shortcut directory ACL: $($Item.FullName) / $($rule.IdentityReference.Value) / $($rule.FileSystemRights)"
            throw 'Shortcut directory allows existing entries to be changed or deleted.'
        }
    }
}
function Test-ShortcutTarget([string]$Path, [string]$Target) {
    $shell = $null
    $link = $null
    try {
        $full = [IO.Path]::GetFullPath($Path)
        $expected = [IO.Path]::GetFullPath($Target)
        if (-not $expected.StartsWith($directory + '\', [StringComparison]::OrdinalIgnoreCase) -or -not [IO.File]::Exists($full)) {
            Microsoft.PowerShell.Utility\Write-Verbose "Shortcut missing or target outside installation: $full / $expected / $directory"
            return $false
        }
        $commonRoots = @([Environment]::GetFolderPath('CommonPrograms'), [Environment]::GetFolderPath('CommonDesktopDirectory'))
        $common = $null
        foreach ($candidate in $commonRoots) {
            if ($full.StartsWith($candidate + '\', [StringComparison]::OrdinalIgnoreCase)) { $common = $candidate; break }
        }
        if ($null -eq $common -or [IO.Path]::GetExtension($full) -ine '.lnk') {
            Microsoft.PowerShell.Utility\Write-Verbose "Shortcut outside common folders: $full"
            return $false
        }
        # A shortcut is user-editable metadata, never executable service code.
        # Do not require its content ACL to match Program Files. Reject leaf
        # reparse points and retain the protected-ancestor checks below.
        $info = [IO.FileInfo]::new($full)
        if (($info.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $false }
        if ($VerbosePreference -eq 'Continue') {
            $metadataAcl = Microsoft.PowerShell.Security\Get-Acl -LiteralPath $full
            foreach ($rule in $metadataAcl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
                Microsoft.PowerShell.Utility\Write-Verbose "Shortcut metadata ACL: $full / $($rule.IdentityReference.Value) / $($rule.FileSystemRights)"
            }
        }
        $parent = [IO.DirectoryInfo]::new([IO.Path]::GetDirectoryName($full))
        while ($null -ne $parent) {
            Assert-ShortcutDirectory $parent
            if ([string]::Equals($parent.FullName, $common, [StringComparison]::OrdinalIgnoreCase)) { break }
            $parent = $parent.Parent
        }
        # Read metadata only. Never Save, Resolve, launch, or execute the link.
        $shell = [Activator]::CreateInstance([type]::GetTypeFromProgID('WScript.Shell'))
        $link = $shell.CreateShortcut($full)
        if ([string]::IsNullOrWhiteSpace($link.TargetPath)) { return $false }
        $actual = [IO.Path]::GetFullPath([Environment]::ExpandEnvironmentVariables($link.TargetPath))
        $matches = [string]::Equals($actual, $expected, [StringComparison]::OrdinalIgnoreCase)
        Microsoft.PowerShell.Utility\Write-Verbose "Shortcut target match=${matches}: $actual / $expected"
        return $matches
    } catch {
        Microsoft.PowerShell.Utility\Write-Verbose "Shortcut metadata rejected: $($_.Exception.Message)"
        return $false
    }
    finally {
        if ($null -ne $link) { $null = [Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
        if ($null -ne $shell) { $null = [Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
    }
}
function Remove-ProductShortcuts([bool]$All) {
    $programs = [Environment]::GetFolderPath('CommonPrograms')
    $desktop = [Environment]::GetFolderPath('CommonDesktopDirectory')
    $paths = @([IO.Path]::Combine($programs, 'Drift', 'Drift.lnk'), [IO.Path]::Combine($desktop, 'Drift.lnk'))
    if ($All) {
        $paths += @([IO.Path]::Combine($programs, 'Driftnet', 'Driftnet.lnk'), [IO.Path]::Combine($programs, 'Driftnet.lnk'), [IO.Path]::Combine($desktop, 'Driftnet.lnk'))
        if (-not [string]::IsNullOrWhiteSpace($StartMenuFolder)) { $paths += [IO.Path]::Combine($programs, $StartMenuFolder, 'Driftnet.lnk') }
    }
    foreach ($path in $paths) {
        if (Test-ShortcutTarget $path ([IO.Path]::Combine($directory, 'drift-desktop.exe'))) {
            [IO.File]::Delete($path)
            $parent = [IO.Path]::GetDirectoryName($path)
            if (-not [string]::Equals($parent, $programs, [StringComparison]::OrdinalIgnoreCase) -and -not [string]::Equals($parent, $desktop, [StringComparison]::OrdinalIgnoreCase)) {
                try { [IO.Directory]::Delete($parent, $false) } catch [IO.IOException] { }
            }
        }
    }
}
# Validate ancestors before traversing children. An unsafe ancestor is rejected
# before any descendant can be replaced, read, or executed with elevation.
Assert-ProtectedItem (Microsoft.PowerShell.Management\Get-Item -LiteralPath $root -Force)
if ([IO.Directory]::Exists($directory)) {
    $queue = [Collections.Generic.Queue[IO.FileSystemInfo]]::new()
    $queue.Enqueue((Microsoft.PowerShell.Management\Get-Item -LiteralPath $directory -Force))
    while ($queue.Count -gt 0) {
        $item = $queue.Dequeue()
        Assert-ProtectedItem $item
        if ($item -is [IO.DirectoryInfo]) {
            foreach ($child in $item.EnumerateFileSystemInfos()) { $queue.Enqueue($child) }
        }
    }
}
if ($Mode -eq 'Validate') { exit 0 }
if ($Mode -eq 'Shortcut') {
    if (Test-ShortcutTarget $ShortcutPath $ShortcutTarget) { exit 0 }
    exit 10
}
if ($Mode -eq 'CleanupLegacy' -or $Mode -eq 'CleanupAll') {
    Remove-ProductShortcuts ($Mode -eq 'CleanupAll')
    exit 0
}
$service = Microsoft.PowerShell.Management\Get-Service -Name DriftNetwork -ErrorAction SilentlyContinue
if ($null -ne $service) {
    try {
        if ($service.Status -ne 'Stopped') {
            if ($service.Status -ne 'StopPending') { $service.Stop() }
            $service.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(25))
        }
    } finally { $service.Dispose() }
    $null = & ([IO.Path]::Combine([Environment]::SystemDirectory, 'sc.exe')) delete DriftNetwork
    if ($LASTEXITCODE -ne 0) { throw 'The networking service could not be removed.' }
    Microsoft.PowerShell.Utility\Start-Sleep -Milliseconds 500
}
