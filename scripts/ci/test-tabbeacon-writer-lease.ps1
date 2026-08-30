[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$tool = Join-Path $repoRoot 'tools\governance\Invoke-TabBeaconWriterLease.ps1'
if (-not (Test-Path -LiteralPath $tool -PathType Leaf)) {
    throw 'WRITER_LEASE_TEST_TOOL_MISSING'
}

$worktree = (Resolve-Path -LiteralPath $repoRoot).Path
$branch = (& git -C $worktree branch --show-current).Trim()
$sourceHead = (& git -C $worktree rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($branch) -or $sourceHead -notmatch '^[0-9a-f]{40}$') {
    throw 'WRITER_LEASE_TEST_WORKTREE_ADMISSION_FAILED'
}

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('tabbeacon-writer-lease-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot -ErrorAction Stop | Out-Null

function Assert-True {
    param([bool]$Condition, [string]$Message)

    if (-not $Condition) {
        throw "WRITER_LEASE_TEST_ASSERTION_FAILED=$Message"
    }
}

function Get-Digest {
    param([string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function ConvertTo-ToolSplat {
    param([string[]]$ToolArguments)

    $splat = @{}
    for ($index = 0; $index -lt $ToolArguments.Count; $index++) {
        $token = $ToolArguments[$index]
        if (-not $token.StartsWith('-', [StringComparison]::Ordinal)) {
            throw "WRITER_LEASE_TEST_INVALID_TOOL_ARGUMENT=$token"
        }
        $name = $token.Substring(1)
        if (($index + 1) -ge $ToolArguments.Count -or $ToolArguments[$index + 1].StartsWith('-', [StringComparison]::Ordinal)) {
            $splat[$name] = $true
            continue
        }
        $index++
        $splat[$name] = $ToolArguments[$index]
    }
    return $splat
}

function Invoke-ToolJson {
    param([string[]]$ToolArguments)

    $splat = ConvertTo-ToolSplat -ToolArguments $ToolArguments
    $output = & $tool @splat
    if (-not $?) {
        throw 'WRITER_LEASE_TEST_TOOL_UNEXPECTED_FAILURE'
    }
    return (($output | Select-Object -Last 1) | ConvertFrom-Json -ErrorAction Stop)
}

function Assert-ToolFails {
    param([string[]]$ToolArguments, [string]$ExpectedToken)

    $failed = $false
    $message = ''
    try {
        $splat = ConvertTo-ToolSplat -ToolArguments $ToolArguments
        $output = & $tool @splat 2>&1
        $message = ($output | Out-String)
        if (-not $?) {
            $failed = $true
        }
    } catch {
        $failed = $true
        $message = $_ | Out-String
    }
    Assert-True -Condition $failed -Message "expected tool failure: $ExpectedToken"
    Assert-True -Condition ($message.Contains($ExpectedToken, [StringComparison]::Ordinal)) -Message "unexpected tool failure: $message"
}

function New-FixtureLease {
    param(
        [string]$Name,
        [string]$State = 'ACTIVE_TEST_LEASE',
        [string]$Holder = ''
    )

    $root = Join-Path $testRoot $Name
    New-Item -ItemType Directory -Path $root -ErrorAction Stop | Out-Null
    $leasePath = Join-Path $root 'writer-lease.json'
    $lease = [ordered]@{
        schema = 'tabbeacon-writer-lease.v1'
        goal_id = "test-$Name"
        repository = 'JerrySkywalker/tabbeacon'
        writer_role = 'implementer'
        worktree = $worktree
        branch = $branch
        start_remote_main = $sourceHead
        state = $State
        owner_config_mutation = $false
        hook_trust_mutation = $false
        public_release = $false
    }
    if (-not [string]::IsNullOrWhiteSpace($Holder)) {
        $lease.holder = $Holder
    }
    [IO.File]::WriteAllText($leasePath, (($lease | ConvertTo-Json -Depth 3) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
    return [pscustomobject]@{
        Root = $root
        LeasePath = $leasePath
        ArchiveRoot = (Join-Path $root 'archive')
        ArchivePath = (Join-Path $root 'archive\writer-lease.archived.v1.json')
        ReceiptPath = (Join-Path $root 'archive\lease-settlement-receipt.txt')
        Goal = "test-$Name"
        Phase = $State
    }
}

function Get-IdentityArguments {
    param($Fixture, [string]$Digest, [string]$ExpectedPhase = $Fixture.Phase)

    return @(
        '-ExpectedLeaseSha256', $Digest,
        '-ExpectedSchema', 'tabbeacon-writer-lease.v1',
        '-ExpectedGoal', $Fixture.Goal,
        '-ExpectedPhase', $ExpectedPhase,
        '-ExpectedRepository', 'JerrySkywalker/tabbeacon',
        '-ExpectedSourceHead', $sourceHead,
        '-ExpectedWorktree', $worktree,
        '-ExpectedBranch', $branch
    )
}

function Get-ReclaimArguments {
    param($Fixture, [string]$Digest, [string]$ExpectedPhase = $Fixture.Phase)

    $identity = Get-IdentityArguments -Fixture $Fixture -Digest $Digest -ExpectedPhase $ExpectedPhase
    return @('-Operation', 'ReclaimOrphan', '-LeasePath', $Fixture.LeasePath) + $identity + @(
        '-ExpectedHolderless',
        '-ActiveWriterCount', '0',
        '-ArchiveRoot', $Fixture.ArchiveRoot,
        '-ArchivePath', $Fixture.ArchivePath,
        '-ReceiptPath', $Fixture.ReceiptPath
    )
}

function New-ArchiveRoot {
    param($Fixture)
    New-Item -ItemType Directory -Path $Fixture.ArchiveRoot -ErrorAction Stop | Out-Null
}

try {
    # 1-3 and 14: atomic acquire, second acquire refusal, exact settle, and no active holderless normal lease.
    $normalRoot = Join-Path $testRoot 'normal'
    New-Item -ItemType Directory -Path $normalRoot -ErrorAction Stop | Out-Null
    $normalLease = Join-Path $normalRoot 'writer-lease.json'
    $normalArchiveRoot = Join-Path $normalRoot 'archive'
    New-Item -ItemType Directory -Path $normalArchiveRoot -ErrorAction Stop | Out-Null
    $normalAcquire = @(
        '-Operation', 'Acquire', '-LeasePath', $normalLease,
        '-Goal', 'test-normal-acquire', '-Phase', 'ACTIVE_TEST_NORMAL',
        '-Repository', 'JerrySkywalker/tabbeacon', '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    )
    $acquired = Invoke-ToolJson -ToolArguments $normalAcquire
    Assert-True -Condition ($acquired.operation -eq 'acquire') -Message 'normal acquire did not report acquire'
    $normalBytes = [IO.File]::ReadAllBytes($normalLease)
    $normalDigest = Get-Digest -Path $normalLease
    Assert-ToolFails -ToolArguments $normalAcquire -ExpectedToken 'WRITER_LEASE_ACQUIRE_BLOCKED_ACTIVE_LEASE_EXISTS'

    $normalFixture = [pscustomobject]@{
        LeasePath = $normalLease
        ArchiveRoot = $normalArchiveRoot
        ArchivePath = (Join-Path $normalArchiveRoot 'writer-lease.settled.v1.json')
        ReceiptPath = (Join-Path $normalArchiveRoot 'lease-settlement-receipt.txt')
        Goal = 'test-normal-acquire'
        Phase = 'ACTIVE_TEST_NORMAL'
    }
    $normalIdentity = Get-IdentityArguments -Fixture $normalFixture -Digest $normalDigest
    $settled = Invoke-ToolJson -ToolArguments (@('-Operation', 'Settle', '-LeasePath', $normalLease) + $normalIdentity + @(
        '-ArchiveRoot', $normalFixture.ArchiveRoot,
        '-ArchivePath', $normalFixture.ArchivePath,
        '-ReceiptPath', $normalFixture.ReceiptPath,
        '-FinalPhase', 'SETTLED_TEST_NORMAL',
        '-Disposition', 'SETTLED_TEST_NORMAL'
    ))
    Assert-True -Condition ($settled.active_holderless_lease -eq $false) -Message 'settle reported an active holderless lease'
    Assert-True -Condition ([Linq.Enumerable]::SequenceEqual[byte]($normalBytes, [IO.File]::ReadAllBytes($normalFixture.ArchivePath))) -Message 'normal settle archive changed bytes'
    Assert-True -Condition ((Get-Content -LiteralPath $normalFixture.ReceiptPath -Raw).Contains('DISPOSITION=SETTLED_TEST_NORMAL', [StringComparison]::Ordinal)) -Message 'normal settle receipt missing'
    $normalStatus = Invoke-ToolJson -ToolArguments @('-Operation', 'Status', '-LeasePath', $normalLease)
    Assert-True -Condition ((-not $normalStatus.exists) -and (-not $normalStatus.active_holderless)) -Message 'normal lifecycle left active holderless lease'
    'WRITER_LEASE_TEST_NORMAL_LIFECYCLE=PASS'

    # 4, 11, 12, and 13: exact orphan reclaim preserves bytes, produces a receipt, and releases a path for a fresh acquire.
    $reclaimFixture = New-FixtureLease -Name 'reclaim-success'
    New-ArchiveRoot -Fixture $reclaimFixture
    $reclaimBytes = [IO.File]::ReadAllBytes($reclaimFixture.LeasePath)
    $reclaimDigest = Get-Digest -Path $reclaimFixture.LeasePath
    $reclaimed = Invoke-ToolJson -ToolArguments (Get-ReclaimArguments -Fixture $reclaimFixture -Digest $reclaimDigest)
    Assert-True -Condition ($reclaimed.disposition -eq 'ORPHAN_RECLAIMED') -Message 'orphan reclaim disposition mismatch'
    Assert-True -Condition ([Linq.Enumerable]::SequenceEqual[byte]($reclaimBytes, [IO.File]::ReadAllBytes($reclaimFixture.ArchivePath))) -Message 'orphan archive changed bytes'
    Assert-True -Condition ((Get-Content -LiteralPath $reclaimFixture.ReceiptPath -Raw).Contains('DISPOSITION=ORPHAN_RECLAIMED', [StringComparison]::Ordinal)) -Message 'orphan receipt missing'
    $postReclaimAcquire = Invoke-ToolJson -ToolArguments @(
        '-Operation', 'Acquire', '-LeasePath', $reclaimFixture.LeasePath,
        '-Goal', 'test-post-reclaim-acquire', '-Phase', 'ACTIVE_TEST_POST_RECLAIM',
        '-Repository', 'JerrySkywalker/tabbeacon', '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    )
    Assert-True -Condition ($postReclaimAcquire.operation -eq 'acquire') -Message 'fresh acquire after reclaim failed'
    'WRITER_LEASE_TEST_RECLAIM_ARCHIVE_AND_FRESH_ACQUIRE=PASS'

    # 5: stale expected digest must block reclaim before any archive mutation.
    $wrongDigestFixture = New-FixtureLease -Name 'wrong-digest'
    New-ArchiveRoot -Fixture $wrongDigestFixture
    $zeroDigest = ('0' * 64 -join '')
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $wrongDigestFixture -Digest $zeroDigest) -ExpectedToken 'WRITER_LEASE_EXPECTED_DIGEST_MISMATCH'
    Assert-True -Condition (Test-Path -LiteralPath $wrongDigestFixture.LeasePath) -Message 'wrong-digest reclaim removed source lease'
    'WRITER_LEASE_TEST_WRONG_DIGEST=PASS'

    # 6: wrong phase must block reclaim.
    $wrongPhaseFixture = New-FixtureLease -Name 'wrong-phase'
    New-ArchiveRoot -Fixture $wrongPhaseFixture
    $wrongPhaseDigest = Get-Digest -Path $wrongPhaseFixture.LeasePath
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $wrongPhaseFixture -Digest $wrongPhaseDigest -ExpectedPhase 'ACTIVE_WRONG_PHASE') -ExpectedToken 'WRITER_LEASE_EXPECTED_PHASE_MISMATCH'
    'WRITER_LEASE_TEST_WRONG_PHASE=PASS'

    # 7: a non-empty holder must block reclaim even when every other identifier matches.
    $holderFixture = New-FixtureLease -Name 'nonempty-holder' -Holder 'active-writer'
    New-ArchiveRoot -Fixture $holderFixture
    $holderDigest = Get-Digest -Path $holderFixture.LeasePath
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $holderFixture -Digest $holderDigest) -ExpectedToken 'WRITER_LEASE_RECLAIM_NONEMPTY_HOLDER_BLOCKED'
    'WRITER_LEASE_TEST_NONEMPTY_HOLDER=PASS'

    # 8: a changed lease after its caller's recorded digest is concurrent drift and must block reclaim.
    $driftFixture = New-FixtureLease -Name 'concurrent-drift'
    New-ArchiveRoot -Fixture $driftFixture
    $driftDigest = Get-Digest -Path $driftFixture.LeasePath
    $driftLease = Get-Content -LiteralPath $driftFixture.LeasePath -Raw | ConvertFrom-Json
    $driftLease.owner_config_mutation = $true
    [IO.File]::WriteAllText($driftFixture.LeasePath, (($driftLease | ConvertTo-Json -Depth 3) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $driftFixture -Digest $driftDigest) -ExpectedToken 'WRITER_LEASE_EXPECTED_DIGEST_MISMATCH'
    'WRITER_LEASE_TEST_CONCURRENT_DRIFT=PASS'

    # 10: neither an existing archive target nor its source lease can be silently replaced.
    $collisionFixture = New-FixtureLease -Name 'archive-collision'
    New-ArchiveRoot -Fixture $collisionFixture
    [IO.File]::WriteAllText($collisionFixture.ArchivePath, 'existing archive marker', [Text.UTF8Encoding]::new($false))
    $collisionDigest = Get-Digest -Path $collisionFixture.LeasePath
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $collisionFixture -Digest $collisionDigest) -ExpectedToken 'WRITER_LEASE_ARCHIVE_COLLISION'
    Assert-True -Condition ((Get-Content -LiteralPath $collisionFixture.ArchivePath -Raw) -eq 'existing archive marker') -Message 'archive collision overwrote existing archive'
    Assert-True -Condition (Test-Path -LiteralPath $collisionFixture.LeasePath) -Message 'archive collision removed source lease'
    'WRITER_LEASE_TEST_ARCHIVE_COLLISION=PASS'

    # 9: Windows junctions are reparse points; archive roots must refuse them.
    $reparseFixture = New-FixtureLease -Name 'reparse-root'
    $safeArchiveTarget = Join-Path $reparseFixture.Root 'safe-archive-target'
    $reparseArchiveRoot = Join-Path $reparseFixture.Root 'archive-link'
    New-Item -ItemType Directory -Path $safeArchiveTarget -ErrorAction Stop | Out-Null
    try {
        New-Item -ItemType Junction -Path $reparseArchiveRoot -Value $safeArchiveTarget -ErrorAction Stop | Out-Null
        $reparseFixture.ArchiveRoot = $reparseArchiveRoot
        $reparseFixture.ArchivePath = Join-Path $reparseArchiveRoot 'writer-lease.archived.v1.json'
        $reparseFixture.ReceiptPath = Join-Path $reparseArchiveRoot 'lease-settlement-receipt.txt'
        $reparseDigest = Get-Digest -Path $reparseFixture.LeasePath
        Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $reparseFixture -Digest $reparseDigest) -ExpectedToken 'WRITER_LEASE_UNSAFE_REPARSE_DIRECTORY'
        'WRITER_LEASE_TEST_REPARSE_TARGET=PASS'
    } catch [System.UnauthorizedAccessException] {
        'WRITER_LEASE_TEST_REPARSE_TARGET=N_A_PLATFORM_PERMISSION'
    } catch [System.NotSupportedException] {
        'WRITER_LEASE_TEST_REPARSE_TARGET=N_A_PLATFORM_UNSUPPORTED'
    }

    "WRITER_LEASE_TEST_ARTIFACT_ROOT=$testRoot"
    'WRITER_LEASE_FOCUSED_TESTS=PASS'
} catch {
    "WRITER_LEASE_TEST_ARTIFACT_ROOT=$testRoot"
    throw
}
