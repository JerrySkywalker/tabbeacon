[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Status', 'Acquire', 'Settle', 'ReclaimOrphan')]
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
    [string]$FinalPhase,
    [string]$Disposition = 'SETTLED',
    [switch]$ExpectedHolderless,
    [int]$ActiveWriterCount = -1
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$LeaseSchema = 'tabbeacon-writer-lease.v1'

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
        private const uint FILE_SHARE_READ = 0x00000001;
        private const uint OPEN_EXISTING = 3;
        private const uint FILE_ATTRIBUTE_NORMAL = 0x00000080;
        private const int FileRenameInfo = 3;

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct FILE_RENAME_INFO {
            [MarshalAs(UnmanagedType.Bool)] public bool ReplaceIfExists;
            public IntPtr RootDirectory;
            public uint FileNameLength;
            public char FileName;
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

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool SetFileInformationByHandle(
            SafeFileHandle file,
            int fileInformationClass,
            IntPtr fileInformation,
            uint bufferSize);

        public static SafeFileHandle OpenForExactMove(string path) {
            SafeFileHandle handle = CreateFile(
                path,
                GENERIC_READ | DELETE,
                FILE_SHARE_READ,
                IntPtr.Zero,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                IntPtr.Zero);
            if (handle.IsInvalid) {
                throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateFile exact-move open failed");
            }
            return handle;
        }

        public static void MoveExactHandle(SafeFileHandle handle, string destinationPath) {
            byte[] destination = System.Text.Encoding.Unicode.GetBytes(destinationPath);
            int nameOffset = Marshal.OffsetOf(typeof(FILE_RENAME_INFO), "FileName").ToInt32();
            int size = checked(nameOffset + destination.Length);
            IntPtr buffer = Marshal.AllocHGlobal(size);
            try {
                FILE_RENAME_INFO rename = new FILE_RENAME_INFO {
                    ReplaceIfExists = false,
                    RootDirectory = IntPtr.Zero,
                    FileNameLength = checked((uint)destination.Length),
                    FileName = '\0'
                };
                Marshal.StructureToPtr(rename, buffer, false);
                Marshal.Copy(destination, 0, IntPtr.Add(buffer, nameOffset), destination.Length);
                if (!SetFileInformationByHandle(handle, FileRenameInfo, buffer, checked((uint)size))) {
                    throw new Win32Exception(Marshal.GetLastWin32Error(), "SetFileInformationByHandle exact rename failed");
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
    $files = Get-ChildItem -LiteralPath $RegistryRoot -Filter 'writer-lease.json' -File -Recurse -Force -ErrorAction Stop
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
            throw "WRITER_LEASE_REGISTRY_ENTRY_UNSUPPORTED_SCHEMA=$existingPath"
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

function Move-ExactLeaseByHandle {
    param($Snapshot, [string]$DestinationPath)

    $handle = $null
    $stream = $null
    $memory = $null
    try {
        $handle = [TabBeacon.WriterLease.Native]::OpenForExactMove($Snapshot.Path)
        $stream = [IO.FileStream]::new($handle, [IO.FileAccess]::Read)
        $memory = [IO.MemoryStream]::new()
        $stream.CopyTo($memory)
        $currentBytes = $memory.ToArray()
        if ((Get-Sha256Hex -Bytes $currentBytes) -ne $Snapshot.Sha256) {
            throw 'WRITER_LEASE_CONCURRENT_DRIFT_BLOCKED'
        }
        [TabBeacon.WriterLease.Native]::MoveExactHandle($handle, $DestinationPath)
        return $currentBytes
    } finally {
        if ($null -ne $memory) {
            $memory.Dispose()
        }
        if ($null -ne $stream) {
            $stream.Dispose()
        } elseif ($null -ne $handle) {
            $handle.Dispose()
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
        [string]$ExpectedLeaseSha256
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
        [string]$WriterProofSha256 = ''
    )

    Assert-SafeExistingDirectory -Path $ArchiveRootPath
    $safeArchivePath = Assert-PathInsideRoot -Root $ArchiveRootPath -Child $ArchiveLeasePath -Name 'ARCHIVE_PATH'
    $safeReceiptPath = Assert-PathInsideRoot -Root $ArchiveRootPath -Child $SettlementReceiptPath -Name 'RECEIPT_PATH'
    Assert-SafeExistingDirectory -Path (Split-Path -Parent $safeArchivePath)
    Assert-SafeExistingDirectory -Path (Split-Path -Parent $safeReceiptPath)
    Assert-SameVolume -First $Snapshot.Path -Second $safeArchivePath
    if (Test-Path -LiteralPath $safeArchivePath) {
        throw 'WRITER_LEASE_ARCHIVE_COLLISION'
    }
    if (Test-Path -LiteralPath $safeReceiptPath) {
        throw 'WRITER_LEASE_RECEIPT_COLLISION'
    }

    $prepared = @(
        'TRANSACTION=PREPARED',
        "OPERATION=$ArchiveOperation",
        "ORIGINAL_LEASE_SHA256=$($Snapshot.Sha256)",
        "ARCHIVE_PATH=$safeArchivePath"
    ) -join [Environment]::NewLine
    Write-NewUtf8File -Path $safeReceiptPath -Content ($prepared + [Environment]::NewLine)

    [void](Move-ExactLeaseByHandle -Snapshot $Snapshot -DestinationPath $safeArchivePath)

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
            @{ Name = 'Branch'; Value = $Branch },
            @{ Name = 'LeaseRegistryRoot'; Value = $LeaseRegistryRoot }
        )) {
            Assert-RequiredString -Name $required.Name -Value $required.Value
        }
        if ($Phase -notlike 'ACTIVE*') {
            throw 'WRITER_LEASE_ACQUIRE_PHASE_MUST_BE_ACTIVE'
        }
        if ($SourceHead -notmatch '^[0-9a-fA-F]{40}$') {
            throw 'WRITER_LEASE_SOURCE_HEAD_MUST_BE_FULL_SHA'
        }

        $fullLeasePath = Get-FullPath -Path $LeasePath
        Assert-SafeExistingDirectory -Path (Split-Path -Parent $fullLeasePath)
        Assert-SafeExistingDirectory -Path $LeaseRegistryRoot
        if (Test-Path -LiteralPath $fullLeasePath) {
            throw 'WRITER_LEASE_ACQUIRE_BLOCKED_ACTIVE_LEASE_EXISTS'
        }
        Assert-WorktreeBinding -Path $Worktree -ExpectedBranch $Branch -SourceCommit $SourceHead

        $scopeMutexes = Enter-WriterScopeMutexes -RepositoryId $Repository -WorktreePath $Worktree -BranchName $Branch
        try {
            Assert-NoActiveScopeConflict -RegistryRoot $LeaseRegistryRoot -LeaseFilePath $fullLeasePath -RepositoryId $Repository -WorktreePath $Worktree -BranchName $Branch
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
        if ($ActiveWriterCount -ne 0) {
            throw 'WRITER_LEASE_RECLAIM_REQUIRES_ACTIVE_WRITER_COUNT_ZERO'
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
            $writerProof = Assert-ActiveWriterProof -ProofPath $ActiveWriterProofPath -ExpectedProofSha256 $ExpectedActiveWriterProofSha256 -ExpectedLeasePath $snapshot.Path -ExpectedLeaseSha256 $snapshot.Sha256
            $archive = Invoke-ArchiveLease -ArchiveOperation 'reclaim-orphan' -Snapshot $snapshot -ArchiveRootPath $ArchiveRoot -ArchiveLeasePath $ArchivePath -SettlementReceiptPath $ReceiptPath -ReceiptDisposition 'ORPHAN_RECLAIMED' -SettlementFinalPhase 'RECLAIMED_ORPHAN' -WriterCount $ActiveWriterCount -WriterProofPath $writerProof.Path -WriterProofSha256 $writerProof.Sha256
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
}
