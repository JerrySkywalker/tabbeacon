[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Status', 'Acquire', 'Settle', 'ReclaimOrphan', 'RecoverPrepared')]
    [string]$Operation,

    [Parameter(Mandatory = $true)]
    [string]$LeasePath,

    [string]$Goal,
    [string]$Phase,
    [string]$Repository,
    [string]$SourceHead,
    [string]$Worktree,
    [string]$Branch,
    [string]$LeaseRegistryRoot,
    [string]$ExpectedLeaseSha256,
    [string]$ExpectedSchema,
    [string]$ExpectedGoal,
    [string]$ExpectedPhase,
    [string]$ExpectedRepository,
    [string]$ExpectedSourceHead,
    [string]$ExpectedWorktree,
    [string]$ExpectedBranch,
    [string]$ArchiveRoot,
    [string]$ArchivePath,
    [string]$ReceiptPath,
    [string]$ActiveWriterProofPath,
    [string]$ExpectedActiveWriterProofSha256,
    [ValidateSet('Settle', 'ReclaimOrphan')]
    [string]$PreparedOperation,
    [string]$FinalPhase,
    [string]$Disposition = 'SETTLED',
    [switch]$ExpectedHolderless
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$LeaseSchema = 'tabbeacon-writer-lease.v1'
$CanonicalLeaseRegistryRoot = 'V:\build\tabbeacon'

if ($env:OS -ne 'Windows_NT') {
    throw 'WRITER_LEASE_WINDOWS_ONLY'
}

if ($null -eq ('TabBeacon.WriterLease.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace TabBeacon.WriterLease {
    public static class Native {
        private const uint GENERIC_READ = 0x80000000;
        private const uint DELETE = 0x00010000;
        private const uint FILE_ADD_FILE = 0x00000002;
        private const uint FILE_SHARE_READ = 0x00000001;
        private const uint FILE_SHARE_WRITE = 0x00000002;
        private const uint FILE_SHARE_DELETE = 0x00000004;
        private const uint OPEN_EXISTING = 3;
        private const uint FILE_ATTRIBUTE_NORMAL = 0x00000080;
        private const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
        private const uint FILE_FLAG_OPEN_REPARSE_POINT = 0x00200000;
        private const uint FILE_ATTRIBUTE_REPARSE_POINT = 0x00000400;
        private const int FileRenameInformation = 10;

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct FILE_RENAME_INFO {
            [MarshalAs(UnmanagedType.Bool)] public bool ReplaceIfExists;
            public IntPtr RootDirectory;
            public uint FileNameLength;
            public char FileName;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct BY_HANDLE_FILE_INFORMATION {
            public uint FileAttributes;
            public System.Runtime.InteropServices.ComTypes.FILETIME CreationTime;
            public System.Runtime.InteropServices.ComTypes.FILETIME LastAccessTime;
            public System.Runtime.InteropServices.ComTypes.FILETIME LastWriteTime;
            public uint VolumeSerialNumber;
            public uint FileSizeHigh;
            public uint FileSizeLow;
            public uint NumberOfLinks;
            public uint FileIndexHigh;
            public uint FileIndexLow;
        }

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFile(
            string fileName,
            uint desiredAccess,
            uint shareMode,
            IntPtr securityAttributes,
            uint creationDisposition,
            uint flagsAndAttributes,
            IntPtr templateFile);

        [StructLayout(LayoutKind.Sequential)]
        private struct IO_STATUS_BLOCK {
            public IntPtr Status;
            public IntPtr Information;
        }

        [DllImport("ntdll.dll")]
        private static extern int NtSetInformationFile(
            SafeFileHandle file,
            out IO_STATUS_BLOCK ioStatusBlock,
            IntPtr fileInformation,
            uint length,
            int fileInformationClass);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetFileInformationByHandle(
            SafeFileHandle file,
            out BY_HANDLE_FILE_INFORMATION fileInformation);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern uint GetFinalPathNameByHandle(
            SafeFileHandle file,
            System.Text.StringBuilder path,
            uint pathLength,
            uint flags);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool ReadFile(
            SafeFileHandle file,
            byte[] buffer,
            uint numberOfBytesToRead,
            out uint numberOfBytesRead,
            IntPtr overlapped);

        private static BY_HANDLE_FILE_INFORMATION GetInformation(SafeFileHandle handle) {
            BY_HANDLE_FILE_INFORMATION information;
            if (!GetFileInformationByHandle(handle, out information)) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "GetFileInformationByHandle failed");
            }
            return information;
        }

        private static void EnsureNotReparse(SafeFileHandle handle) {
            if ((GetInformation(handle).FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0) {
                throw new System.IO.IOException("WRITER_LEASE_UNSAFE_REPARSE_HANDLE");
            }
        }

        public static SafeFileHandle OpenForExactMove(string path) {
            SafeFileHandle handle = CreateFile(
                path,
                GENERIC_READ | DELETE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                IntPtr.Zero,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                IntPtr.Zero);
            if (handle.IsInvalid) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateFile exact-move open failed");
            }
            EnsureNotReparse(handle);
            return handle;
        }

        public static SafeFileHandle OpenSafeDirectory(string path) {
            SafeFileHandle handle = CreateFile(
                path,
                GENERIC_READ | FILE_ADD_FILE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                IntPtr.Zero,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                IntPtr.Zero);
            if (handle.IsInvalid) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateFile archive-directory open failed");
            }
            EnsureNotReparse(handle);
            return handle;
        }

        public static string GetFinalPath(SafeFileHandle handle) {
            var path = new System.Text.StringBuilder(32768);
            uint result = GetFinalPathNameByHandle(handle, path, (uint)path.Capacity, 0);
            if (result == 0 || result >= path.Capacity) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "GetFinalPathNameByHandle failed");
            }
            return path.ToString();
        }

        public static string GetFileIdentity(SafeFileHandle handle) {
            BY_HANDLE_FILE_INFORMATION information = GetInformation(handle);
            return information.VolumeSerialNumber.ToString("x8") + ":" + information.FileIndexHigh.ToString("x8") + information.FileIndexLow.ToString("x8");
        }

        public static byte[] ReadExactBytes(SafeFileHandle handle) {
            BY_HANDLE_FILE_INFORMATION information = GetInformation(handle);
            ulong length = ((ulong)information.FileSizeHigh << 32) | information.FileSizeLow;
            if (length > 1048576) {
                throw new System.IO.IOException("WRITER_LEASE_SOURCE_TOO_LARGE");
            }
            byte[] bytes = new byte[(int)length];
            int offset = 0;
            while (offset < bytes.Length) {
                uint read;
                if (!ReadFile(handle, bytes, checked((uint)(bytes.Length - offset)), out read, IntPtr.Zero)) {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "ReadFile exact-source read failed");
                }
                if (read == 0) {
                    throw new System.IO.IOException("WRITER_LEASE_SOURCE_SHORT_READ");
                }
                offset += checked((int)read);
            }
            return bytes;
        }

        public static void MoveExactHandle(SafeFileHandle handle, SafeFileHandle rootDirectory, string destinationName) {
            byte[] destination = System.Text.Encoding.Unicode.GetBytes(destinationName);
            int nameOffset = Marshal.OffsetOf(typeof(FILE_RENAME_INFO), "FileName").ToInt32();
            int size = checked(nameOffset + destination.Length);
            IntPtr buffer = Marshal.AllocHGlobal(size);
            try {
                FILE_RENAME_INFO rename = new FILE_RENAME_INFO {
                    ReplaceIfExists = false,
                    RootDirectory = rootDirectory.DangerousGetHandle(),
                    FileNameLength = checked((uint)destination.Length),
                    FileName = '\0'
                };
                Marshal.StructureToPtr(rename, buffer, false);
                Marshal.Copy(destination, 0, IntPtr.Add(buffer, nameOffset), destination.Length);
                IO_STATUS_BLOCK status;
                int result = NtSetInformationFile(handle, out status, buffer, checked((uint)size), FileRenameInformation);
                if (result != 0) {
                    throw new System.IO.IOException("WRITER_LEASE_RENAME_NATIVE_FAILURE=NTSTATUS_0x" + result.ToString("x8"));
                }
            } finally {
                Marshal.FreeHGlobal(buffer);
            }
        }
    }
}
'@ -ErrorAction Stop
}

function Assert-RequiredString {
    param([string]$Name, [string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "WRITER_LEASE_REQUIRED_PARAMETER=$Name"
    }
}

function Get-FullPath {
    param([string]$Path)

    Assert-RequiredString -Name 'path' -Value $Path
    return [IO.Path]::GetFullPath($Path)
}

function Get-CanonicalLeaseRegistryRoot {
    param([string]$RepositoryId)

    if ($RepositoryId -ne 'JerrySkywalker/tabbeacon') {
        throw 'WRITER_LEASE_CANONICAL_REGISTRY_UNKNOWN_REPOSITORY'
    }
    Assert-SafeExistingDirectory -Path $CanonicalLeaseRegistryRoot
    return Get-FullPath -Path $CanonicalLeaseRegistryRoot
}

function Assert-CanonicalLeasePath {
    param([string]$Path, [string]$RegistryRoot)

    $fullPath = Get-FullPath -Path $Path
    $parent = Split-Path -Parent $fullPath
    $taskRoot = Split-Path -Parent $parent
    if ((Split-Path -Leaf $fullPath) -ne 'writer-lease.json') {
        throw 'WRITER_LEASE_NONCANONICAL_FILENAME'
    }
    if (-not [string]::Equals($taskRoot, $RegistryRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WRITER_LEASE_NONCANONICAL_TASK_ROOT'
    }
    return $fullPath
}

function Assert-SafeExistingDirectory {
    param([string]$Path)

    $current = Get-FullPath -Path $Path
    while ($true) {
        $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
        if (-not $item.PSIsContainer) {
            throw "WRITER_LEASE_UNSAFE_NOT_DIRECTORY=$current"
        }
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "WRITER_LEASE_UNSAFE_REPARSE_DIRECTORY=$current"
        }

        $parent = [IO.Directory]::GetParent($current)
        if ($null -eq $parent -or $parent.FullName -eq $current) {
            break
        }
        $current = $parent.FullName
    }
}

function Assert-SafeExistingFile {
    param([string]$Path)

    $fullPath = Get-FullPath -Path $Path
    $item = Get-Item -LiteralPath $fullPath -Force -ErrorAction Stop
    if ($item.PSIsContainer) {
        throw "WRITER_LEASE_UNSAFE_NOT_FILE=$fullPath"
    }
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "WRITER_LEASE_UNSAFE_REPARSE_FILE=$fullPath"
    }
    Assert-SafeExistingDirectory -Path (Split-Path -Parent $fullPath)
    return $fullPath
}

function Assert-PathInsideRoot {
    param([string]$Root, [string]$Child, [string]$Name)

    $rootFull = (Get-FullPath -Path $Root).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $childFull = Get-FullPath -Path $Child
    $prefix = $rootFull + [IO.Path]::DirectorySeparatorChar
    if (-not $childFull.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "WRITER_LEASE_UNSAFE_${Name}_OUTSIDE_ARCHIVE_ROOT=$childFull"
    }
    return $childFull
}

function Assert-SameVolume {
    param([string]$First, [string]$Second)

    $firstRoot = [IO.Path]::GetPathRoot((Get-FullPath -Path $First))
    $secondRoot = [IO.Path]::GetPathRoot((Get-FullPath -Path $Second))
    if (-not [string]::Equals($firstRoot, $secondRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WRITER_LEASE_ARCHIVE_MUST_SHARE_SOURCE_VOLUME'
    }
}

function Get-Sha256Hex {
    param([byte[]]$Bytes)

    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha256.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha256.Dispose()
    }
}

function Get-LeaseSnapshot {
    param([string]$Path)

    $safePath = Assert-SafeExistingFile -Path $Path
    $sourceDescriptor = Get-ExactFileDescriptor -Path $safePath
    $bytes = [IO.File]::ReadAllBytes($safePath)
    $text = [Text.UTF8Encoding]::new($false).GetString($bytes)
    try {
        $lease = $text | ConvertFrom-Json -ErrorAction Stop
    } catch {
        throw "WRITER_LEASE_INVALID_JSON=$safePath"
    }

    return [pscustomobject]@{
        Path = $safePath
        Bytes = $bytes
        Sha256 = Get-Sha256Hex -Bytes $bytes
        Lease = $lease
        SourceDescriptor = $sourceDescriptor
    }
}

function Get-LeaseProperty {
    param($Lease, [string]$Name)

    $property = $Lease.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Test-HolderlessLease {
    param($Lease)

    $holder = Get-LeaseProperty -Lease $Lease -Name 'holder'
    return ($null -eq $holder) -or [string]::IsNullOrWhiteSpace([string]$holder)
}

function Assert-LeaseIdentity {
    param(
        $Snapshot,
        [string]$Sha256,
        [string]$Schema,
        [string]$GoalId,
        [string]$State,
        [string]$RepositoryId,
        [string]$StartRemoteMain,
        [string]$WorktreePath,
        [string]$BranchName
    )

    foreach ($required in @(
        @{ Name = 'ExpectedLeaseSha256'; Value = $Sha256 },
        @{ Name = 'ExpectedSchema'; Value = $Schema },
        @{ Name = 'ExpectedGoal'; Value = $GoalId },
        @{ Name = 'ExpectedPhase'; Value = $State },
        @{ Name = 'ExpectedRepository'; Value = $RepositoryId },
        @{ Name = 'ExpectedSourceHead'; Value = $StartRemoteMain },
        @{ Name = 'ExpectedWorktree'; Value = $WorktreePath },
        @{ Name = 'ExpectedBranch'; Value = $BranchName }
    )) {
        Assert-RequiredString -Name $required.Name -Value $required.Value
    }

    if ($Schema -ne $LeaseSchema) {
        throw 'WRITER_LEASE_UNSUPPORTED_SCHEMA'
    }

    if (-not [string]::Equals($Snapshot.Sha256, $Sha256.ToLowerInvariant(), [StringComparison]::Ordinal)) {
        throw 'WRITER_LEASE_EXPECTED_DIGEST_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'schema') -ne $LeaseSchema) {
        throw 'WRITER_LEASE_EXPECTED_SCHEMA_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'goal_id') -ne $GoalId) {
        throw 'WRITER_LEASE_EXPECTED_GOAL_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'state') -ne $State) {
        throw 'WRITER_LEASE_EXPECTED_PHASE_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'repository') -ne $RepositoryId) {
        throw 'WRITER_LEASE_EXPECTED_REPOSITORY_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'start_remote_main') -ne $StartRemoteMain) {
        throw 'WRITER_LEASE_EXPECTED_SOURCE_HEAD_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'worktree') -ne $WorktreePath) {
        throw 'WRITER_LEASE_EXPECTED_WORKTREE_MISMATCH'
    }
    if ((Get-LeaseProperty -Lease $Snapshot.Lease -Name 'branch') -ne $BranchName) {
        throw 'WRITER_LEASE_EXPECTED_BRANCH_MISMATCH'
    }
}

function Enter-LeaseMutex {
    param([string]$Path)

    $nameSeed = [Text.Encoding]::UTF8.GetBytes((Get-FullPath -Path $Path).ToLowerInvariant())
    $name = 'Global\TabBeaconWriterLease-' + (Get-Sha256Hex -Bytes $nameSeed).Substring(0, 24)
    $mutex = [Threading.Mutex]::new($false, $name)
    try {
        if (-not $mutex.WaitOne(0)) {
            $mutex.Dispose()
            throw 'WRITER_LEASE_OPERATION_BUSY'
        }
    } catch [Threading.AbandonedMutexException] {
        # The OS has released an interrupted caller's mutex; immutable lease checks still apply.
    }
    return $mutex
}

function Enter-WriterScopeMutexes {
    param([string]$RepositoryId, [string]$WorktreePath, [string]$BranchName)

    $scopeSeeds = @(
        ('worktree|' + $RepositoryId.ToLowerInvariant() + '|' + (Get-FullPath -Path $WorktreePath).ToLowerInvariant()),
        ('branch|' + $RepositoryId.ToLowerInvariant() + '|' + $BranchName.ToLowerInvariant())
    ) | Sort-Object -Unique
    $mutexes = [Collections.Generic.List[Threading.Mutex]]::new()
    try {
        foreach ($seed in $scopeSeeds) {
            $nameBytes = [Text.Encoding]::UTF8.GetBytes($seed)
            $name = 'Global\TabBeaconWriterScope-' + (Get-Sha256Hex -Bytes $nameBytes).Substring(0, 24)
            $mutex = [Threading.Mutex]::new($false, $name)
            try {
                if (-not $mutex.WaitOne(0)) {
                    throw 'WRITER_LEASE_OPERATION_BUSY'
                }
            } catch [Threading.AbandonedMutexException] {
                # Ownership transfers after an interrupted caller; all identity checks still run.
            }
            $mutexes.Add($mutex)
        }
        return $mutexes.ToArray()
    } catch {
        foreach ($mutex in $mutexes) {
            $mutex.ReleaseMutex()
            $mutex.Dispose()
        }
        throw
    }
}

function Exit-WriterScopeMutexes {
    param([Threading.Mutex[]]$Mutexes)

    for ($index = $Mutexes.Count - 1; $index -ge 0; $index--) {
        $mutex = $Mutexes[$index]
        $mutex.ReleaseMutex()
        $mutex.Dispose()
    }
}

function Assert-NoActiveScopeConflict {
    param(
        [string]$RegistryRoot,
        [string]$LeaseFilePath,
        [string]$RepositoryId,
        [string]$WorktreePath,
        [string]$BranchName
    )

    Assert-SafeExistingDirectory -Path $RegistryRoot
    $candidatePath = Get-FullPath -Path $LeaseFilePath
    $candidateWorktree = Get-FullPath -Path $WorktreePath
    $files = foreach ($taskRoot in Get-ChildItem -LiteralPath $RegistryRoot -Directory -Force -ErrorAction Stop) {
        if (($taskRoot.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "WRITER_LEASE_REGISTRY_ENTRY_UNSAFE_OR_INVALID=$($taskRoot.FullName)"
        }
        $candidate = Join-Path $taskRoot.FullName 'writer-lease.json'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            Get-Item -LiteralPath $candidate -Force -ErrorAction Stop
        }
    }
    foreach ($file in $files) {
        $existingPath = Get-FullPath -Path $file.FullName
        if ([string]::Equals($existingPath, $candidatePath, [StringComparison]::OrdinalIgnoreCase)) {
            continue
        }
        try {
            $snapshot = Get-LeaseSnapshot -Path $existingPath
        } catch {
            throw "WRITER_LEASE_REGISTRY_ENTRY_UNSAFE_OR_INVALID=$existingPath"
        }
        if ((Get-LeaseProperty -Lease $snapshot.Lease -Name 'schema') -ne $LeaseSchema) {
            $legacyRepository = [string](Get-LeaseProperty -Lease $snapshot.Lease -Name 'repository')
            $legacyWorktree = [string](Get-LeaseProperty -Lease $snapshot.Lease -Name 'worktree')
            $legacyBranch = [string](Get-LeaseProperty -Lease $snapshot.Lease -Name 'branch')
            if ($legacyRepository -eq $RepositoryId -and (($legacyWorktree -and [string]::Equals((Get-FullPath -Path $legacyWorktree), $candidateWorktree, [StringComparison]::OrdinalIgnoreCase)) -or $legacyBranch -eq $BranchName)) {
                throw "WRITER_LEASE_REGISTRY_ENTRY_UNSUPPORTED_SCHEMA_CONFLICT=$existingPath"
            }
            continue
        }
        $state = [string](Get-LeaseProperty -Lease $snapshot.Lease -Name 'state')
        if ($state -notlike 'ACTIVE*') {
            continue
        }
        if ((Get-LeaseProperty -Lease $snapshot.Lease -Name 'repository') -ne $RepositoryId) {
            continue
        }
        $existingWorktree = Get-FullPath -Path ([string](Get-LeaseProperty -Lease $snapshot.Lease -Name 'worktree'))
        $existingBranch = [string](Get-LeaseProperty -Lease $snapshot.Lease -Name 'branch')
        if ([string]::Equals($existingWorktree, $candidateWorktree, [StringComparison]::OrdinalIgnoreCase) -or $existingBranch -eq $BranchName) {
            throw "WRITER_LEASE_ACQUIRE_BLOCKED_SCOPE_CONFLICT=$existingPath"
        }
    }
}

function Write-NewUtf8File {
    param([string]$Path, [string]$Content)

    $encoding = [Text.UTF8Encoding]::new($false)
    $bytes = $encoding.GetBytes($Content)
    $stream = $null
    try {
        $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    } finally {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
    }
}

function Write-ExistingUtf8File {
    param([string]$Path, [string]$Content)

    $parent = Split-Path -Parent $Path
    $leaf = Split-Path -Leaf $Path
    $stagingPath = Join-Path $parent ('.' + $leaf + '.' + [Guid]::NewGuid().ToString('N') + '.staging')
    $backupPath = Join-Path $parent ('.' + $leaf + '.' + [Guid]::NewGuid().ToString('N') + '.backup')
    try {
        Write-NewUtf8File -Path $stagingPath -Content $Content
        [IO.File]::Replace($stagingPath, $Path, $backupPath, $true)
    } finally {
        if (Test-Path -LiteralPath $stagingPath) {
            Remove-Item -LiteralPath $stagingPath -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path -LiteralPath $backupPath) {
            Remove-Item -LiteralPath $backupPath -Force -ErrorAction SilentlyContinue
        }
    }
}

function Publish-NewLeaseAtomically {
    param([string]$Path, [string]$Content)

    $parent = Split-Path -Parent $Path
    $leaf = Split-Path -Leaf $Path
    $stagingPath = Join-Path $parent ('.' + $leaf + '.' + [Guid]::NewGuid().ToString('N') + '.staging')
    try {
        Write-NewUtf8File -Path $stagingPath -Content $Content
        [IO.File]::Move($stagingPath, $Path)
    } finally {
        if (Test-Path -LiteralPath $stagingPath) {
            Remove-Item -LiteralPath $stagingPath -Force -ErrorAction SilentlyContinue
        }
    }
}

function Get-ExactFileDescriptor {
    param([string]$Path)

    $handle = $null
    try {
        $handle = [TabBeacon.WriterLease.Native]::OpenForExactMove($Path)
        return [pscustomobject]@{
            FinalPath = [TabBeacon.WriterLease.Native]::GetFinalPath($handle)
            Identity = [TabBeacon.WriterLease.Native]::GetFileIdentity($handle)
        }
    } finally {
        if ($null -ne $handle) {
            $handle.Dispose()
        }
    }
}

function Get-ExactDirectoryDescriptor {
    param([string]$Path)

    $handle = $null
    try {
        $handle = [TabBeacon.WriterLease.Native]::OpenSafeDirectory($Path)
        return [pscustomobject]@{
            FinalPath = [TabBeacon.WriterLease.Native]::GetFinalPath($handle)
            Identity = [TabBeacon.WriterLease.Native]::GetFileIdentity($handle)
        }
    } finally {
        if ($null -ne $handle) {
            $handle.Dispose()
        }
    }
}

function Move-ExactLeaseByHandle {
    param(
        $Snapshot,
        [string]$DestinationPath,
        $ExpectedSource,
        $ExpectedDestinationDirectory
    )

    $handle = $null
    $destinationDirectoryHandle = $null
    try {
        $handle = [TabBeacon.WriterLease.Native]::OpenForExactMove($Snapshot.Path)
        $destinationDirectoryHandle = [TabBeacon.WriterLease.Native]::OpenSafeDirectory((Split-Path -Parent $DestinationPath))
        if (-not [string]::Equals([TabBeacon.WriterLease.Native]::GetFinalPath($handle), $ExpectedSource.FinalPath, [StringComparison]::OrdinalIgnoreCase) -or [TabBeacon.WriterLease.Native]::GetFileIdentity($handle) -ne $ExpectedSource.Identity) {
            throw 'WRITER_LEASE_CONCURRENT_SOURCE_IDENTITY_DRIFT_BLOCKED'
        }
        if (-not [string]::Equals([TabBeacon.WriterLease.Native]::GetFinalPath($destinationDirectoryHandle), $ExpectedDestinationDirectory.FinalPath, [StringComparison]::OrdinalIgnoreCase) -or [TabBeacon.WriterLease.Native]::GetFileIdentity($destinationDirectoryHandle) -ne $ExpectedDestinationDirectory.Identity) {
            throw 'WRITER_LEASE_CONCURRENT_ARCHIVE_DIRECTORY_DRIFT_BLOCKED'
        }
        $currentBytes = [TabBeacon.WriterLease.Native]::ReadExactBytes($handle)
        if ((Get-Sha256Hex -Bytes $currentBytes) -ne $Snapshot.Sha256) {
            throw 'WRITER_LEASE_CONCURRENT_DRIFT_BLOCKED'
        }
        [TabBeacon.WriterLease.Native]::MoveExactHandle($handle, $destinationDirectoryHandle, (Split-Path -Leaf $DestinationPath))
        return $currentBytes
    } finally {
        if ($null -ne $handle) {
            $handle.Dispose()
        }
        if ($null -ne $destinationDirectoryHandle) {
            $destinationDirectoryHandle.Dispose()
        }
    }
}

function Assert-WorktreeBinding {
    param([string]$Path, [string]$ExpectedBranch, [string]$SourceCommit)

    Assert-SafeExistingDirectory -Path $Path
    $actualTopLevel = (& git -C $Path rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0) {
        throw 'WRITER_LEASE_WORKTREE_NOT_GIT'
    }
    $actualFull = Get-FullPath -Path $actualTopLevel.Trim()
    $expectedFull = Get-FullPath -Path $Path
    if (-not [string]::Equals($actualFull, $expectedFull, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WRITER_LEASE_WORKTREE_IDENTITY_MISMATCH'
    }
    $actualBranch = (& git -C $Path branch --show-current 2>$null).Trim()
    if ($LASTEXITCODE -ne 0 -or $actualBranch -ne $ExpectedBranch) {
        throw 'WRITER_LEASE_WORKTREE_BRANCH_MISMATCH'
    }
    & git -C $Path rev-parse --verify ($SourceCommit + '^{commit}') 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'WRITER_LEASE_SOURCE_HEAD_NOT_A_COMMIT'
    }
    & git -C $Path merge-base --is-ancestor $SourceCommit HEAD 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw 'WRITER_LEASE_SOURCE_HEAD_NOT_ANCESTOR_OF_WORKTREE'
    }
}

function Assert-ActiveWriterProof {
    param(
        [string]$ProofPath,
        [string]$ExpectedProofSha256,
        [string]$ExpectedLeasePath,
        [string]$ExpectedLeaseSha256,
        [string]$ExpectedWorktreePath,
        [string]$ExpectedBranchName,
        [string]$ExpectedRepositoryId
    )

    Assert-RequiredString -Name 'ActiveWriterProofPath' -Value $ProofPath
    Assert-RequiredString -Name 'ExpectedActiveWriterProofSha256' -Value $ExpectedProofSha256
    $safeProofPath = Assert-SafeExistingFile -Path $ProofPath
    $proofBytes = [IO.File]::ReadAllBytes($safeProofPath)
    $proofSha256 = Get-Sha256Hex -Bytes $proofBytes
    if (-not [string]::Equals($proofSha256, $ExpectedProofSha256.ToLowerInvariant(), [StringComparison]::Ordinal)) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_DIGEST_MISMATCH'
    }
    $proofText = [Text.UTF8Encoding]::new($false).GetString($proofBytes)
    $fields = @{}
    foreach ($line in ($proofText -split "`r?`n")) {
        if ($line -match '^(?<name>[A-Z0-9_]+)=(?<value>.*)$') {
            $fields[$Matches.name] = $Matches.value
        }
    }
    if ($fields['PROOF_SCHEMA'] -ne 'tabbeacon-writer-active-proof.v1') {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_SCHEMA_MISMATCH'
    }
    if ($fields['ACTIVE_WRITER_COUNT'] -ne '0') {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_COUNT_NOT_ZERO'
    }
    if ($fields['ACTIVE_LEASE_HOLDER_PROVEN'] -ne 'false') {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_HOLDER_STATE_MISMATCH'
    }
    if ($fields['OBSERVATION_SCOPE'] -ne 'bounded-process-and-worktree-inspection') {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_SCOPE_MISMATCH'
    }
    if ([string]::IsNullOrWhiteSpace($fields['OBSERVER_ID'])) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_OBSERVER_MISSING'
    }
    if ($fields['REPOSITORY'] -ne $ExpectedRepositoryId) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_REPOSITORY_MISMATCH'
    }
    if (-not [string]::Equals($fields['WORKTREE'], (Get-FullPath -Path $ExpectedWorktreePath), [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_WORKTREE_MISMATCH'
    }
    if ($fields['BRANCH'] -ne $ExpectedBranchName) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_BRANCH_MISMATCH'
    }
    $observedAt = [DateTimeOffset]::MinValue
    $expiresAt = [DateTimeOffset]::MinValue
    if (-not [DateTimeOffset]::TryParse($fields['OBSERVED_AT_UTC'], [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AssumeUniversal, [ref]$observedAt) -or -not [DateTimeOffset]::TryParse($fields['EXPIRES_AT_UTC'], [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AssumeUniversal, [ref]$expiresAt)) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_TIMESTAMP_INVALID'
    }
    $now = [DateTimeOffset]::UtcNow
    if ($observedAt -gt $now -or ($now - $observedAt) -gt [TimeSpan]::FromMinutes(5) -or $expiresAt -le $now -or ($expiresAt - $observedAt) -gt [TimeSpan]::FromMinutes(5)) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_STALE_OR_INVALID'
    }
    if (-not [string]::Equals($fields['LEASE_PATH'], (Get-FullPath -Path $ExpectedLeasePath), [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_LEASE_PATH_MISMATCH'
    }
    if (-not [string]::Equals($fields['LEASE_SHA256'], $ExpectedLeaseSha256.ToLowerInvariant(), [StringComparison]::Ordinal)) {
        throw 'WRITER_LEASE_ACTIVE_WRITER_PROOF_LEASE_DIGEST_MISMATCH'
    }
    return [pscustomobject]@{
        Path = $safeProofPath
        Sha256 = $proofSha256
    }
}

function Get-ReceiptText {
    param(
        [string]$ReceiptDisposition,
        [string]$ReceiptOperation,
        $Snapshot,
        [string]$ArchivedPath,
        [string]$ArchivedSha256,
        [string]$FinalState,
        [string]$WriterCount,
        [string]$WriterProofPath = '',
        [string]$WriterProofSha256 = ''
    )

    $lines = @(
        "DISPOSITION=$ReceiptDisposition",
        "OPERATION=$ReceiptOperation",
        "ORIGINAL_LEASE_PATH=$($Snapshot.Path)",
        "ARCHIVED_LEASE_PATH=$ArchivedPath",
        "ARCHIVED_LEASE_SHA256=$ArchivedSha256",
        "ORIGINAL_LEASE_SHA256=$($Snapshot.Sha256)",
        "SCHEMA=$(Get-LeaseProperty -Lease $Snapshot.Lease -Name 'schema')",
        "GOAL=$(Get-LeaseProperty -Lease $Snapshot.Lease -Name 'goal_id')",
        "PHASE=$(Get-LeaseProperty -Lease $Snapshot.Lease -Name 'state')",
        "FINAL_PHASE=$FinalState",
        "ACTIVE_WRITER_COUNT=$WriterCount",
        'LEASE_CONTENT_MODIFIED=false'
    )
    if (-not [string]::IsNullOrWhiteSpace($WriterProofPath)) {
        $lines += "ACTIVE_WRITER_PROOF_PATH=$WriterProofPath"
        $lines += "ACTIVE_WRITER_PROOF_SHA256=$WriterProofSha256"
    }
    return ($lines -join [Environment]::NewLine) + [Environment]::NewLine
}

function Get-PreparedReceiptMetadata {
    param([string]$Path)

    $safePath = Assert-SafeExistingFile -Path $Path
    $fields = @{}
    foreach ($line in ([Text.UTF8Encoding]::new($false).GetString([IO.File]::ReadAllBytes($safePath)) -split "`r?`n")) {
        if ($line -match '^(?<name>[A-Z0-9_]+)=(?<value>.*)$') {
            $fields[$Matches.name] = $Matches.value
        }
    }
    if ($fields['TRANSACTION'] -ne 'PREPARED' -or [string]::IsNullOrWhiteSpace($fields['OPERATION']) -or [string]::IsNullOrWhiteSpace($fields['ORIGINAL_LEASE_SHA256']) -or [string]::IsNullOrWhiteSpace($fields['ARCHIVE_PATH'])) {
        throw 'WRITER_LEASE_PREPARED_RECEIPT_INVALID'
    }
    return [pscustomobject]@{
        Path = $safePath
        OriginalLeaseSha256 = $fields['ORIGINAL_LEASE_SHA256']
        ArchivePath = $fields['ARCHIVE_PATH']
        Operation = $fields['OPERATION']
        Disposition = $fields['DISPOSITION']
        FinalPhase = $fields['FINAL_PHASE']
        WriterCount = $fields['ACTIVE_WRITER_COUNT']
        WriterProofPath = if ($null -eq $fields['ACTIVE_WRITER_PROOF_PATH']) { '' } else { $fields['ACTIVE_WRITER_PROOF_PATH'] }
        WriterProofSha256 = if ($null -eq $fields['ACTIVE_WRITER_PROOF_SHA256']) { '' } else { $fields['ACTIVE_WRITER_PROOF_SHA256'] }
        LegacyFormat = [string]::IsNullOrWhiteSpace($fields['DISPOSITION']) -or [string]::IsNullOrWhiteSpace($fields['FINAL_PHASE']) -or [string]::IsNullOrWhiteSpace($fields['ACTIVE_WRITER_COUNT'])
    }
}

function Invoke-ArchiveLease {
    param(
        [string]$ArchiveOperation,
        $Snapshot,
        [string]$ArchiveRootPath,
        [string]$ArchiveLeasePath,
        [string]$SettlementReceiptPath,
        [string]$ReceiptDisposition,
        [string]$SettlementFinalPhase,
        [string]$WriterCount,
        [string]$WriterProofPath = '',
        [string]$WriterProofSha256 = '',
        [switch]$UseExistingPreparedReceipt,
        [switch]$AllowLegacyPreparedReceipt
    )

    Assert-SafeExistingDirectory -Path $ArchiveRootPath
    $safeArchivePath = Assert-PathInsideRoot -Root $ArchiveRootPath -Child $ArchiveLeasePath -Name 'ARCHIVE_PATH'
    $safeReceiptPath = Assert-PathInsideRoot -Root $ArchiveRootPath -Child $SettlementReceiptPath -Name 'RECEIPT_PATH'
    Assert-SafeExistingDirectory -Path (Split-Path -Parent $safeArchivePath)
    Assert-SafeExistingDirectory -Path (Split-Path -Parent $safeReceiptPath)
    if (-not [string]::Equals((Split-Path -Parent $safeArchivePath), (Split-Path -Parent $safeReceiptPath), [StringComparison]::OrdinalIgnoreCase)) {
        throw 'WRITER_LEASE_RECEIPT_MUST_SHARE_ARCHIVE_DIRECTORY'
    }
    Assert-SameVolume -First $Snapshot.Path -Second $safeArchivePath
    if (Test-Path -LiteralPath $safeArchivePath) {
        throw 'WRITER_LEASE_ARCHIVE_COLLISION'
    }
    if (Test-Path -LiteralPath $safeReceiptPath) {
        if (-not $UseExistingPreparedReceipt) {
            throw 'WRITER_LEASE_RECEIPT_COLLISION'
        }
        $preparedReceipt = Get-PreparedReceiptMetadata -Path $safeReceiptPath
        if ($preparedReceipt.OriginalLeaseSha256 -ne $Snapshot.Sha256 -or -not [string]::Equals((Get-FullPath -Path $preparedReceipt.ArchivePath), $safeArchivePath, [StringComparison]::OrdinalIgnoreCase) -or $preparedReceipt.Operation -ne $ArchiveOperation) {
            throw 'WRITER_LEASE_PREPARED_RECEIPT_IDENTITY_MISMATCH'
        }
        if ($preparedReceipt.LegacyFormat) {
            if (-not $AllowLegacyPreparedReceipt) {
                throw 'WRITER_LEASE_PREPARED_RECEIPT_LEGACY_REQUIRES_RECOVER_OPERATION'
            }
        } elseif ($preparedReceipt.Disposition -ne $ReceiptDisposition -or $preparedReceipt.FinalPhase -ne $SettlementFinalPhase -or $preparedReceipt.WriterCount -ne $WriterCount -or $preparedReceipt.WriterProofPath -ne $WriterProofPath -or $preparedReceipt.WriterProofSha256 -ne $WriterProofSha256) {
            throw 'WRITER_LEASE_PREPARED_RECEIPT_IDENTITY_MISMATCH'
        }
    } elseif ($UseExistingPreparedReceipt) {
        throw 'WRITER_LEASE_PREPARED_RECEIPT_MISSING'
    }

    if (-not $UseExistingPreparedReceipt) {
        $prepared = @(
            'TRANSACTION=PREPARED',
            "OPERATION=$ArchiveOperation",
            "ORIGINAL_LEASE_SHA256=$($Snapshot.Sha256)",
            "ARCHIVE_PATH=$safeArchivePath",
            "DISPOSITION=$ReceiptDisposition",
            "FINAL_PHASE=$SettlementFinalPhase",
            "ACTIVE_WRITER_COUNT=$WriterCount"
        ) -join [Environment]::NewLine
        if (-not [string]::IsNullOrWhiteSpace($WriterProofPath)) {
            $prepared += [Environment]::NewLine + "ACTIVE_WRITER_PROOF_PATH=$WriterProofPath"
            $prepared += [Environment]::NewLine + "ACTIVE_WRITER_PROOF_SHA256=$WriterProofSha256"
        }
        Write-NewUtf8File -Path $safeReceiptPath -Content ($prepared + [Environment]::NewLine)
    }

    $expectedArchiveDirectory = Get-ExactDirectoryDescriptor -Path (Split-Path -Parent $safeArchivePath)
    [void](Move-ExactLeaseByHandle -Snapshot $Snapshot -DestinationPath $safeArchivePath -ExpectedSource $Snapshot.SourceDescriptor -ExpectedDestinationDirectory $expectedArchiveDirectory)

    if (Test-Path -LiteralPath $Snapshot.Path) {
        throw 'WRITER_LEASE_SOURCE_REMAINS_AFTER_ARCHIVE'
    }
    $archived = Get-LeaseSnapshot -Path $safeArchivePath
    if ($archived.Sha256 -ne $Snapshot.Sha256) {
        throw 'WRITER_LEASE_ARCHIVE_INTEGRITY_FAILURE'
    }

    $receipt = Get-ReceiptText -ReceiptDisposition $ReceiptDisposition -ReceiptOperation $ArchiveOperation -Snapshot $Snapshot -ArchivedPath $safeArchivePath -ArchivedSha256 $archived.Sha256 -FinalState $SettlementFinalPhase -WriterCount $WriterCount -WriterProofPath $WriterProofPath -WriterProofSha256 $WriterProofSha256
    Write-ExistingUtf8File -Path $safeReceiptPath -Content $receipt

    return [pscustomobject]@{
        archived_lease_path = $safeArchivePath
        archived_lease_sha256 = $archived.Sha256
        receipt_path = $safeReceiptPath
    }
}

function Write-MachineResult {
    param($Result)
    $Result | ConvertTo-Json -Compress -Depth 5
}

switch ($Operation) {
    'Status' {
        $fullLeasePath = Get-FullPath -Path $LeasePath
        if (-not (Test-Path -LiteralPath $fullLeasePath -PathType Leaf)) {
            Write-MachineResult ([ordered]@{
                operation = 'status'
                lease_path = $fullLeasePath
                exists = $false
                active_holderless = $false
            })
            break
        }
        $snapshot = Get-LeaseSnapshot -Path $fullLeasePath
        Write-MachineResult ([ordered]@{
            operation = 'status'
            lease_path = $snapshot.Path
            exists = $true
            sha256 = $snapshot.Sha256
            schema = Get-LeaseProperty -Lease $snapshot.Lease -Name 'schema'
            goal = Get-LeaseProperty -Lease $snapshot.Lease -Name 'goal_id'
            phase = Get-LeaseProperty -Lease $snapshot.Lease -Name 'state'
            repository = Get-LeaseProperty -Lease $snapshot.Lease -Name 'repository'
            source_head = Get-LeaseProperty -Lease $snapshot.Lease -Name 'start_remote_main'
            worktree = Get-LeaseProperty -Lease $snapshot.Lease -Name 'worktree'
            branch = Get-LeaseProperty -Lease $snapshot.Lease -Name 'branch'
            holderless = Test-HolderlessLease -Lease $snapshot.Lease
            active_holderless = ((Get-LeaseProperty -Lease $snapshot.Lease -Name 'state') -like 'ACTIVE*') -and (Test-HolderlessLease -Lease $snapshot.Lease)
        })
    }
    'Acquire' {
        foreach ($required in @(
            @{ Name = 'Goal'; Value = $Goal },
            @{ Name = 'Phase'; Value = $Phase },
            @{ Name = 'Repository'; Value = $Repository },
            @{ Name = 'SourceHead'; Value = $SourceHead },
            @{ Name = 'Worktree'; Value = $Worktree },
            @{ Name = 'Branch'; Value = $Branch }
        )) {
            Assert-RequiredString -Name $required.Name -Value $required.Value
        }
        if ($Phase -notlike 'ACTIVE*') {
            throw 'WRITER_LEASE_ACQUIRE_PHASE_MUST_BE_ACTIVE'
        }
        if ($SourceHead -notmatch '^[0-9a-fA-F]{40}$') {
            throw 'WRITER_LEASE_SOURCE_HEAD_MUST_BE_FULL_SHA'
        }

        $canonicalRegistryRoot = Get-CanonicalLeaseRegistryRoot -RepositoryId $Repository
        if (-not [string]::IsNullOrWhiteSpace($LeaseRegistryRoot) -and -not [string]::Equals((Get-FullPath -Path $LeaseRegistryRoot), $canonicalRegistryRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'WRITER_LEASE_NONCANONICAL_REGISTRY_ROOT'
        }
        $fullLeasePath = Assert-CanonicalLeasePath -Path $LeasePath -RegistryRoot $canonicalRegistryRoot
        Assert-SafeExistingDirectory -Path (Split-Path -Parent $fullLeasePath)
        if (Test-Path -LiteralPath $fullLeasePath) {
            throw 'WRITER_LEASE_ACQUIRE_BLOCKED_ACTIVE_LEASE_EXISTS'
        }
        Assert-WorktreeBinding -Path $Worktree -ExpectedBranch $Branch -SourceCommit $SourceHead

        $scopeMutexes = Enter-WriterScopeMutexes -RepositoryId $Repository -WorktreePath $Worktree -BranchName $Branch
        try {
            Assert-NoActiveScopeConflict -RegistryRoot $canonicalRegistryRoot -LeaseFilePath $fullLeasePath -RepositoryId $Repository -WorktreePath $Worktree -BranchName $Branch
            $lease = [ordered]@{
                schema = $LeaseSchema
                goal_id = $Goal
                repository = $Repository
                writer_role = 'implementer'
                worktree = Get-FullPath -Path $Worktree
                branch = $Branch
                start_remote_main = $SourceHead.ToLowerInvariant()
                state = $Phase
                owner_config_mutation = $false
                hook_trust_mutation = $false
                public_release = $false
            }
            $json = ($lease | ConvertTo-Json -Depth 3) + [Environment]::NewLine
            try {
                Publish-NewLeaseAtomically -Path $fullLeasePath -Content $json
            } catch [IO.IOException] {
                throw 'WRITER_LEASE_ACQUIRE_BLOCKED_ACTIVE_LEASE_EXISTS'
            }
            $snapshot = Get-LeaseSnapshot -Path $fullLeasePath
            if ((Get-LeaseProperty -Lease $snapshot.Lease -Name 'schema') -ne $LeaseSchema -or (Get-LeaseProperty -Lease $snapshot.Lease -Name 'goal_id') -ne $Goal -or (Get-LeaseProperty -Lease $snapshot.Lease -Name 'state') -ne $Phase) {
                throw 'WRITER_LEASE_ACQUIRE_POSTCONDITION_FAILED'
            }
            Write-MachineResult ([ordered]@{
                operation = 'acquire'
                lease_path = $snapshot.Path
                sha256 = $snapshot.Sha256
                goal = $Goal
                phase = $Phase
                holder = 'unsupported-by-v1-schema'
                expiry = 'unsupported-by-v1-schema'
            })
        } finally {
            Exit-WriterScopeMutexes -Mutexes $scopeMutexes
        }
    }
    'Settle' {
        Assert-RequiredString -Name 'FinalPhase' -Value $FinalPhase
        Assert-RequiredString -Name 'Disposition' -Value $Disposition
        Assert-RequiredString -Name 'ArchiveRoot' -Value $ArchiveRoot
        Assert-RequiredString -Name 'ArchivePath' -Value $ArchivePath
        Assert-RequiredString -Name 'ReceiptPath' -Value $ReceiptPath

        $mutex = Enter-LeaseMutex -Path $LeasePath
        try {
            $snapshot = Get-LeaseSnapshot -Path $LeasePath
            Assert-LeaseIdentity -Snapshot $snapshot -Sha256 $ExpectedLeaseSha256 -Schema $ExpectedSchema -GoalId $ExpectedGoal -State $ExpectedPhase -RepositoryId $ExpectedRepository -StartRemoteMain $ExpectedSourceHead -WorktreePath $ExpectedWorktree -BranchName $ExpectedBranch
            if (-not (Test-HolderlessLease -Lease $snapshot.Lease)) {
                throw 'WRITER_LEASE_SETTLE_NONEMPTY_HOLDER_BLOCKED'
            }
            $archive = Invoke-ArchiveLease -ArchiveOperation 'settle' -Snapshot $snapshot -ArchiveRootPath $ArchiveRoot -ArchiveLeasePath $ArchivePath -SettlementReceiptPath $ReceiptPath -ReceiptDisposition $Disposition -SettlementFinalPhase $FinalPhase -WriterCount 'N/A'
            Write-MachineResult ([ordered]@{
                operation = 'settle'
                disposition = $Disposition
                active_lease_exists = $false
                active_holderless_lease = $false
                final_phase = $FinalPhase
                archived_lease_path = $archive.archived_lease_path
                archived_lease_sha256 = $archive.archived_lease_sha256
                receipt_path = $archive.receipt_path
            })
        } finally {
            $mutex.ReleaseMutex()
            $mutex.Dispose()
        }
    }
    'ReclaimOrphan' {
        if (-not $ExpectedHolderless) {
            throw 'WRITER_LEASE_RECLAIM_REQUIRES_EXPLICIT_HOLDERLESS_PROOF'
        }
        if ($ExpectedPhase -notlike 'ACTIVE*') {
            throw 'WRITER_LEASE_RECLAIM_PHASE_MUST_BE_ACTIVE'
        }
        Assert-RequiredString -Name 'ArchiveRoot' -Value $ArchiveRoot
        Assert-RequiredString -Name 'ArchivePath' -Value $ArchivePath
        Assert-RequiredString -Name 'ReceiptPath' -Value $ReceiptPath
        Assert-RequiredString -Name 'ActiveWriterProofPath' -Value $ActiveWriterProofPath
        Assert-RequiredString -Name 'ExpectedActiveWriterProofSha256' -Value $ExpectedActiveWriterProofSha256

        $mutex = Enter-LeaseMutex -Path $LeasePath
        try {
            $snapshot = Get-LeaseSnapshot -Path $LeasePath
            Assert-LeaseIdentity -Snapshot $snapshot -Sha256 $ExpectedLeaseSha256 -Schema $ExpectedSchema -GoalId $ExpectedGoal -State $ExpectedPhase -RepositoryId $ExpectedRepository -StartRemoteMain $ExpectedSourceHead -WorktreePath $ExpectedWorktree -BranchName $ExpectedBranch
            if (-not (Test-HolderlessLease -Lease $snapshot.Lease)) {
                throw 'WRITER_LEASE_RECLAIM_NONEMPTY_HOLDER_BLOCKED'
            }
            $writerProof = Assert-ActiveWriterProof -ProofPath $ActiveWriterProofPath -ExpectedProofSha256 $ExpectedActiveWriterProofSha256 -ExpectedLeasePath $snapshot.Path -ExpectedLeaseSha256 $snapshot.Sha256 -ExpectedWorktreePath $ExpectedWorktree -ExpectedBranchName $ExpectedBranch -ExpectedRepositoryId $ExpectedRepository
            $archive = Invoke-ArchiveLease -ArchiveOperation 'reclaim-orphan' -Snapshot $snapshot -ArchiveRootPath $ArchiveRoot -ArchiveLeasePath $ArchivePath -SettlementReceiptPath $ReceiptPath -ReceiptDisposition 'ORPHAN_RECLAIMED' -SettlementFinalPhase 'RECLAIMED_ORPHAN' -WriterCount '0' -WriterProofPath $writerProof.Path -WriterProofSha256 $writerProof.Sha256
            Write-MachineResult ([ordered]@{
                operation = 'reclaim-orphan'
                disposition = 'ORPHAN_RECLAIMED'
                active_lease_exists = $false
                active_holderless_lease = $false
                archived_lease_path = $archive.archived_lease_path
                archived_lease_sha256 = $archive.archived_lease_sha256
                receipt_path = $archive.receipt_path
            })
        } finally {
            $mutex.ReleaseMutex()
            $mutex.Dispose()
        }
    }
    'RecoverPrepared' {
        Assert-RequiredString -Name 'PreparedOperation' -Value $PreparedOperation
        Assert-RequiredString -Name 'ArchiveRoot' -Value $ArchiveRoot
        Assert-RequiredString -Name 'ArchivePath' -Value $ArchivePath
        Assert-RequiredString -Name 'ReceiptPath' -Value $ReceiptPath
        Assert-RequiredString -Name 'ExpectedLeaseSha256' -Value $ExpectedLeaseSha256
        Assert-RequiredString -Name 'ExpectedSchema' -Value $ExpectedSchema
        Assert-RequiredString -Name 'ExpectedGoal' -Value $ExpectedGoal
        Assert-RequiredString -Name 'ExpectedPhase' -Value $ExpectedPhase
        Assert-RequiredString -Name 'ExpectedRepository' -Value $ExpectedRepository
        Assert-RequiredString -Name 'ExpectedSourceHead' -Value $ExpectedSourceHead
        Assert-RequiredString -Name 'ExpectedWorktree' -Value $ExpectedWorktree
        Assert-RequiredString -Name 'ExpectedBranch' -Value $ExpectedBranch
        if ($PreparedOperation -eq 'ReclaimOrphan' -and -not $ExpectedHolderless) {
            throw 'WRITER_LEASE_RECLAIM_REQUIRES_EXPLICIT_HOLDERLESS_PROOF'
        }
        if ($PreparedOperation -eq 'ReclaimOrphan' -and $ExpectedPhase -notlike 'ACTIVE*') {
            throw 'WRITER_LEASE_RECLAIM_PHASE_MUST_BE_ACTIVE'
        }

        Assert-SafeExistingDirectory -Path $ArchiveRoot
        $safeArchivePath = Assert-PathInsideRoot -Root $ArchiveRoot -Child $ArchivePath -Name 'ARCHIVE_PATH'
        $safeReceiptPath = Assert-PathInsideRoot -Root $ArchiveRoot -Child $ReceiptPath -Name 'RECEIPT_PATH'
        Assert-SafeExistingDirectory -Path (Split-Path -Parent $safeArchivePath)
        Assert-SafeExistingDirectory -Path (Split-Path -Parent $safeReceiptPath)
        if (-not [string]::Equals((Split-Path -Parent $safeArchivePath), (Split-Path -Parent $safeReceiptPath), [StringComparison]::OrdinalIgnoreCase)) {
            throw 'WRITER_LEASE_RECEIPT_MUST_SHARE_ARCHIVE_DIRECTORY'
        }
        Assert-SameVolume -First $LeasePath -Second $safeArchivePath

        $mutex = Enter-LeaseMutex -Path $LeasePath
        try {
            $preparedReceipt = Get-PreparedReceiptMetadata -Path $safeReceiptPath
            $expectedArchiveOperation = if ($PreparedOperation -eq 'Settle') { 'settle' } else { 'reclaim-orphan' }
            if ($preparedReceipt.Operation -ne $expectedArchiveOperation -or $preparedReceipt.OriginalLeaseSha256 -ne $ExpectedLeaseSha256.ToLowerInvariant() -or -not [string]::Equals((Get-FullPath -Path $preparedReceipt.ArchivePath), $safeArchivePath, [StringComparison]::OrdinalIgnoreCase)) {
                throw 'WRITER_LEASE_PREPARED_RECEIPT_IDENTITY_MISMATCH'
            }
            if ($preparedReceipt.LegacyFormat) {
                Assert-RequiredString -Name 'FinalPhase' -Value $FinalPhase
                Assert-RequiredString -Name 'Disposition' -Value $Disposition
                $receiptDisposition = $Disposition
                $settlementFinalPhase = $FinalPhase
                $writerCount = if ($PreparedOperation -eq 'ReclaimOrphan') { '0' } else { 'N/A' }
            } else {
                $receiptDisposition = $preparedReceipt.Disposition
                $settlementFinalPhase = $preparedReceipt.FinalPhase
                $writerCount = $preparedReceipt.WriterCount
            }

            $sourcePath = Get-FullPath -Path $LeasePath
            $sourceExists = Test-Path -LiteralPath $sourcePath -PathType Leaf
            $archiveExists = Test-Path -LiteralPath $safeArchivePath -PathType Leaf
            if ($sourceExists -and $archiveExists) {
                throw 'WRITER_LEASE_PREPARED_TRANSACTION_INCONSISTENT_BOTH_PATHS_EXIST'
            }
            if (-not $sourceExists -and -not $archiveExists) {
                throw 'WRITER_LEASE_PREPARED_TRANSACTION_UNRECOVERABLE_NO_LEASE'
            }

            if ($PreparedOperation -eq 'ReclaimOrphan') {
                Assert-RequiredString -Name 'ActiveWriterProofPath' -Value $ActiveWriterProofPath
                Assert-RequiredString -Name 'ExpectedActiveWriterProofSha256' -Value $ExpectedActiveWriterProofSha256
                if (-not $preparedReceipt.LegacyFormat -and (-not [string]::Equals((Get-FullPath -Path $preparedReceipt.WriterProofPath), (Get-FullPath -Path $ActiveWriterProofPath), [StringComparison]::OrdinalIgnoreCase) -or $preparedReceipt.WriterProofSha256 -ne $ExpectedActiveWriterProofSha256.ToLowerInvariant())) {
                    throw 'WRITER_LEASE_PREPARED_RECEIPT_WRITER_PROOF_MISMATCH'
                }
            } elseif (-not $preparedReceipt.LegacyFormat -and (-not [string]::IsNullOrWhiteSpace($preparedReceipt.WriterProofPath) -or -not [string]::IsNullOrWhiteSpace($preparedReceipt.WriterProofSha256))) {
                throw 'WRITER_LEASE_PREPARED_RECEIPT_WRITER_PROOF_MISMATCH'
            }

            if ($sourceExists) {
                $snapshot = Get-LeaseSnapshot -Path $sourcePath
                Assert-LeaseIdentity -Snapshot $snapshot -Sha256 $ExpectedLeaseSha256 -Schema $ExpectedSchema -GoalId $ExpectedGoal -State $ExpectedPhase -RepositoryId $ExpectedRepository -StartRemoteMain $ExpectedSourceHead -WorktreePath $ExpectedWorktree -BranchName $ExpectedBranch
                if (-not (Test-HolderlessLease -Lease $snapshot.Lease)) {
                    throw 'WRITER_LEASE_PREPARED_RECOVERY_NONEMPTY_HOLDER_BLOCKED'
                }
                if ($PreparedOperation -eq 'ReclaimOrphan') {
                    $writerProof = Assert-ActiveWriterProof -ProofPath $ActiveWriterProofPath -ExpectedProofSha256 $ExpectedActiveWriterProofSha256 -ExpectedLeasePath $snapshot.Path -ExpectedLeaseSha256 $snapshot.Sha256 -ExpectedWorktreePath $ExpectedWorktree -ExpectedBranchName $ExpectedBranch -ExpectedRepositoryId $ExpectedRepository
                    $writerProofPath = $writerProof.Path
                    $writerProofSha256 = $writerProof.Sha256
                } else {
                    $writerProofPath = ''
                    $writerProofSha256 = ''
                }
                $archiveArguments = @{
                    ArchiveOperation = $expectedArchiveOperation
                    Snapshot = $snapshot
                    ArchiveRootPath = $ArchiveRoot
                    ArchiveLeasePath = $safeArchivePath
                    SettlementReceiptPath = $safeReceiptPath
                    ReceiptDisposition = $receiptDisposition
                    SettlementFinalPhase = $settlementFinalPhase
                    WriterCount = $writerCount
                    WriterProofPath = $writerProofPath
                    WriterProofSha256 = $writerProofSha256
                    UseExistingPreparedReceipt = $true
                    AllowLegacyPreparedReceipt = $preparedReceipt.LegacyFormat
                }
                $archive = Invoke-ArchiveLease @archiveArguments
                Write-MachineResult ([ordered]@{
                    operation = 'recover-prepared'
                    recovery_state = 'resumed-archive'
                    active_lease_exists = $false
                    active_holderless_lease = $false
                    archived_lease_path = $archive.archived_lease_path
                    archived_lease_sha256 = $archive.archived_lease_sha256
                    receipt_path = $archive.receipt_path
                })
                break
            }

            $archivedSnapshot = Get-LeaseSnapshot -Path $safeArchivePath
            Assert-LeaseIdentity -Snapshot $archivedSnapshot -Sha256 $ExpectedLeaseSha256 -Schema $ExpectedSchema -GoalId $ExpectedGoal -State $ExpectedPhase -RepositoryId $ExpectedRepository -StartRemoteMain $ExpectedSourceHead -WorktreePath $ExpectedWorktree -BranchName $ExpectedBranch
            if (-not (Test-HolderlessLease -Lease $archivedSnapshot.Lease)) {
                throw 'WRITER_LEASE_PREPARED_RECOVERY_NONEMPTY_HOLDER_BLOCKED'
            }
            if ($PreparedOperation -eq 'ReclaimOrphan') {
                $writerProof = Assert-ActiveWriterProof -ProofPath $ActiveWriterProofPath -ExpectedProofSha256 $ExpectedActiveWriterProofSha256 -ExpectedLeasePath $sourcePath -ExpectedLeaseSha256 $ExpectedLeaseSha256 -ExpectedWorktreePath $ExpectedWorktree -ExpectedBranchName $ExpectedBranch -ExpectedRepositoryId $ExpectedRepository
                $writerProofPath = $writerProof.Path
                $writerProofSha256 = $writerProof.Sha256
            } else {
                $writerProofPath = ''
                $writerProofSha256 = ''
            }
            $historicalSnapshot = [pscustomobject]@{
                Path = $sourcePath
                Bytes = $archivedSnapshot.Bytes
                Sha256 = $archivedSnapshot.Sha256
                Lease = $archivedSnapshot.Lease
                SourceDescriptor = $null
            }
            $receipt = Get-ReceiptText -ReceiptDisposition $receiptDisposition -ReceiptOperation $preparedReceipt.Operation -Snapshot $historicalSnapshot -ArchivedPath $safeArchivePath -ArchivedSha256 $archivedSnapshot.Sha256 -FinalState $settlementFinalPhase -WriterCount $writerCount -WriterProofPath $writerProofPath -WriterProofSha256 $writerProofSha256
            Write-ExistingUtf8File -Path $safeReceiptPath -Content $receipt
            Write-MachineResult ([ordered]@{
                operation = 'recover-prepared'
                recovery_state = 'finalized-existing-archive'
                active_lease_exists = $false
                active_holderless_lease = $false
                archived_lease_path = $safeArchivePath
                archived_lease_sha256 = $archivedSnapshot.Sha256
                receipt_path = $safeReceiptPath
            })
        } finally {
            $mutex.ReleaseMutex()
            $mutex.Dispose()
        }
    }
}
