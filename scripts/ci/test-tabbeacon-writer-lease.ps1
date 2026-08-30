[CmdletBinding()]
param(
    [switch]$InjectFailureAfterNormalAcquire
)

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

$testRoot = 'V:\build\tabbeacon'
$testPrefix = 'TB-WRITER-LEASE-TEST-' + [Guid]::NewGuid().ToString('N')
$script:fixtureLedger = @()

function Assert-True {
    param([bool]$Condition, [string]$Message)

    if (-not $Condition) {
        throw "WRITER_LEASE_TEST_ASSERTION_FAILED=$Message"
    }
}

function Test-ExactBytes {
    param([byte[]]$First, [byte[]]$Second)

    if ($First.Length -ne $Second.Length) {
        return $false
    }
    for ($index = 0; $index -lt $First.Length; $index++) {
        if ($First[$index] -ne $Second[$index]) {
            return $false
        }
    }
    return $true
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
    Assert-True -Condition ($message.IndexOf($ExpectedToken, [StringComparison]::Ordinal) -ge 0) -Message "unexpected tool failure: $message"
}

function New-FixtureLease {
    param(
        [string]$Name,
        [string]$State = 'ACTIVE_TEST_LEASE',
        [string]$Holder = '',
        [string]$Schema = 'tabbeacon-writer-lease.v1'
    )

    $root = Join-Path $testRoot ($testPrefix + '-' + $Name)
    New-Item -ItemType Directory -Path $root -ErrorAction Stop | Out-Null
    $leasePath = Join-Path $root 'writer-lease.json'
    $fixtureRepository = 'test/tabbeacon-writer-lease-contract'
    $fixtureWorktree = $root
    $fixtureBranch = 'test/writer-lease-contract'
    $lease = [ordered]@{
        schema = $Schema
        goal_id = "test-$Name"
        repository = $fixtureRepository
        writer_role = 'implementer'
        worktree = $fixtureWorktree
        branch = $fixtureBranch
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
    $fixture = [pscustomobject]@{
        Root = $root
        LeasePath = $leasePath
        ArchiveRoot = (Join-Path $root 'archive')
        ArchivePath = (Join-Path $root 'archive\writer-lease.archived.v1.json')
        ReceiptPath = (Join-Path $root 'archive\lease-settlement-receipt.txt')
        Goal = "test-$Name"
        Phase = $State
        Schema = $Schema
        Repository = $fixtureRepository
        Worktree = $fixtureWorktree
        Branch = $fixtureBranch
        SourceHead = $sourceHead
    }
    $script:fixtureLedger += $fixture
    return $fixture
}

function Get-IdentityArguments {
    param($Fixture, [string]$Digest, [string]$ExpectedPhase = $Fixture.Phase)

    return @(
        '-ExpectedLeaseSha256', $Digest,
        '-ExpectedSchema', $Fixture.Schema,
        '-ExpectedGoal', $Fixture.Goal,
        '-ExpectedPhase', $ExpectedPhase,
        '-ExpectedRepository', $Fixture.Repository,
        '-ExpectedSourceHead', $Fixture.SourceHead,
        '-ExpectedWorktree', $Fixture.Worktree,
        '-ExpectedBranch', $Fixture.Branch
    )
}

function Get-ReclaimArguments {
    param($Fixture, [string]$Digest, [string]$ExpectedPhase = $Fixture.Phase)

    $identity = Get-IdentityArguments -Fixture $Fixture -Digest $Digest -ExpectedPhase $ExpectedPhase
    $proof = New-ActiveWriterProof -Fixture $Fixture -LeaseDigest $Digest
    return @('-Operation', 'ReclaimOrphan', '-LeasePath', $Fixture.LeasePath) + $identity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $Fixture.ArchiveRoot,
        '-ArchivePath', $Fixture.ArchivePath,
        '-ReceiptPath', $Fixture.ReceiptPath,
        '-ActiveWriterProofPath', $proof.Path,
        '-ExpectedActiveWriterProofSha256', $proof.Sha256
    )
}

function New-ActiveWriterProof {
    param(
        $Fixture,
        [string]$LeaseDigest,
        [string]$ProofFileName = 'active-writer-proof.txt',
        [int]$AgeMinutes = 0
    )

    $proofPath = Join-Path $Fixture.Root $ProofFileName
    $observedAt = [DateTimeOffset]::UtcNow.AddMinutes(-$AgeMinutes)
    $expiresAt = $observedAt.AddMinutes(2)
    $content = @(
        'PROOF_SCHEMA=tabbeacon-writer-active-proof.v1',
        'ACTIVE_WRITER_COUNT=0',
        'ACTIVE_LEASE_HOLDER_PROVEN=false',
        'OBSERVATION_SCOPE=bounded-process-and-worktree-inspection',
        'OBSERVER_ID=focused-contract-test',
        ('REPOSITORY=' + $Fixture.Repository),
        ('WORKTREE=' + $Fixture.Worktree),
        ('BRANCH=' + $Fixture.Branch),
        ('OBSERVED_AT_UTC=' + $observedAt.ToString('o')),
        ('EXPIRES_AT_UTC=' + $expiresAt.ToString('o')),
        ('LEASE_PATH=' + $Fixture.LeasePath),
        ('LEASE_SHA256=' + $LeaseDigest)
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($proofPath, $content + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    return [pscustomobject]@{ Path = $proofPath; Sha256 = Get-Digest -Path $proofPath }
}

function New-ArchiveRoot {
    param($Fixture)
    New-Item -ItemType Directory -Path $Fixture.ArchiveRoot -ErrorAction Stop | Out-Null
}

function Get-TestReceiptFields {
    param([string]$Path)

    $fields = @{}
    foreach ($line in ((Get-Content -LiteralPath $Path -Raw -ErrorAction Stop -Encoding UTF8) -split "`r?`n")) {
        if ($line -match '^(?<name>[A-Z0-9_]+)=(?<value>.*)$') {
            $fields[$Matches.name] = $Matches.value
        }
    }
    return $fields
}

function Complete-FixturePreparedTransactionForTestCleanup {
    param($Fixture, [hashtable]$Fields)

    $archivePath = [string]$Fields['ARCHIVE_PATH']
    if ([string]::IsNullOrWhiteSpace($archivePath)) {
        throw 'WRITER_LEASE_TEST_CLEANUP_PREPARED_ARCHIVE_PATH_MISSING'
    }
    $operation = [string]$Fields['OPERATION']
    if ($operation -eq 'settle') {
        $preparedOperation = 'Settle'
    } elseif ($operation -eq 'reclaim-orphan') {
        $preparedOperation = 'ReclaimOrphan'
    } else {
        throw 'WRITER_LEASE_TEST_CLEANUP_PREPARED_OPERATION_INVALID'
    }
    $cleanupReceipt = $Fixture.PSObject.Properties['CleanupFinalReceiptContent']
    if ($null -ne $cleanupReceipt -and -not [string]::IsNullOrWhiteSpace([string]$cleanupReceipt.Value)) {
        [IO.File]::WriteAllText($Fixture.ReceiptPath, [string]$cleanupReceipt.Value, [Text.UTF8Encoding]::new($false))
    }
    $leaseMetadataPath = if (Test-Path -LiteralPath $Fixture.LeasePath -PathType Leaf) { $Fixture.LeasePath } else { $archivePath }
    if (-not (Test-Path -LiteralPath $leaseMetadataPath -PathType Leaf)) {
        throw 'WRITER_LEASE_TEST_CLEANUP_PREPARED_LEASE_MISSING'
    }
    $lease = Get-Content -LiteralPath $leaseMetadataPath -Raw | ConvertFrom-Json -ErrorAction Stop
    $cleanupFixture = [pscustomobject]@{
        Root = $Fixture.Root
        LeasePath = $Fixture.LeasePath
        ArchiveRoot = $Fixture.ArchiveRoot
        ArchivePath = $archivePath
        ReceiptPath = $Fixture.ReceiptPath
        Goal = $lease.goal_id
        Phase = $lease.state
        Schema = $lease.schema
        Repository = $lease.repository
        Worktree = $lease.worktree
        Branch = $lease.branch
        SourceHead = $lease.start_remote_main
    }
    $identity = Get-IdentityArguments -Fixture $cleanupFixture -Digest ([string]$Fields['ORIGINAL_LEASE_SHA256'])
    $recoverArguments = @('-Operation', 'RecoverPrepared', '-PreparedOperation', $preparedOperation, '-LeasePath', $cleanupFixture.LeasePath) + $identity + @(
        '-ArchiveRoot', $cleanupFixture.ArchiveRoot,
        '-ArchivePath', $cleanupFixture.ArchivePath,
        '-ReceiptPath', $cleanupFixture.ReceiptPath
    )
    $holderProperty = $lease.PSObject.Properties['holder']
    if ($preparedOperation -eq 'Settle' -and $null -ne $holderProperty -and -not [string]::IsNullOrWhiteSpace([string]$holderProperty.Value)) {
        $recoverArguments += @('-ExpectedHolder', [string]$holderProperty.Value)
    }
    if ($preparedOperation -eq 'ReclaimOrphan') {
        $recordedProofPath = [string]$Fields['ACTIVE_WRITER_PROOF_PATH']
        $recordedProofSha256 = [string]$Fields['ACTIVE_WRITER_PROOF_SHA256']
        if (-not (Test-Path -LiteralPath $cleanupFixture.LeasePath -PathType Leaf) -and -not [string]::IsNullOrWhiteSpace($recordedProofPath) -and -not [string]::IsNullOrWhiteSpace($recordedProofSha256) -and (Test-Path -LiteralPath $recordedProofPath -PathType Leaf) -and (Get-Digest -Path $recordedProofPath) -eq $recordedProofSha256) {
            $proof = [pscustomobject]@{ Path = $recordedProofPath; Sha256 = $recordedProofSha256 }
        } else {
            $proof = New-ActiveWriterProof -Fixture $cleanupFixture -LeaseDigest ([string]$Fields['ORIGINAL_LEASE_SHA256'])
        }
        $recoverArguments += @('-ExpectedHolderless', '-ActiveWriterProofPath', $proof.Path, '-ExpectedActiveWriterProofSha256', $proof.Sha256)
    }
    [void](Invoke-ToolJson -ToolArguments $recoverArguments)
}

function Settle-FixtureForTestCleanup {
    param($Fixture)

    $transactionPath = Join-Path $Fixture.Root 'writer-lease.transaction.v1.txt'
    $preparedFields = $null
    if (Test-Path -LiteralPath $transactionPath -PathType Leaf) {
        $fields = Get-TestReceiptFields -Path $transactionPath
        if ($fields['TRANSACTION'] -eq 'PREPARED') {
            $preparedFields = $fields
        }
    }
    if ($null -eq $preparedFields -and (Test-Path -LiteralPath $Fixture.ReceiptPath -PathType Leaf)) {
        $fields = Get-TestReceiptFields -Path $Fixture.ReceiptPath
        if ($fields['TRANSACTION'] -eq 'PREPARED') {
            $preparedFields = $fields
        }
    }
    if ($null -ne $preparedFields) {
        Complete-FixturePreparedTransactionForTestCleanup -Fixture $Fixture -Fields $preparedFields
        return
    }
    if (-not (Test-Path -LiteralPath $Fixture.LeasePath -PathType Leaf)) {
        return
    }
    $lease = Get-Content -LiteralPath $Fixture.LeasePath -Raw | ConvertFrom-Json -ErrorAction Stop
    if ($lease.schema -ne 'tabbeacon-writer-lease.v1') {
        return
    }
    $cleanupRoot = Join-Path $Fixture.Root 'cleanup-archive'
    if (-not (Test-Path -LiteralPath $cleanupRoot -PathType Container)) {
        New-Item -ItemType Directory -Path $cleanupRoot -ErrorAction Stop | Out-Null
    }
    $cleanupFixture = [pscustomobject]@{
        LeasePath = $Fixture.LeasePath
        ArchiveRoot = $cleanupRoot
        ArchivePath = (Join-Path $cleanupRoot 'writer-lease.test-cleanup.v1.json')
        ReceiptPath = (Join-Path $cleanupRoot 'test-cleanup-receipt.txt')
        Goal = $lease.goal_id
        Phase = $lease.state
        Schema = $lease.schema
        Repository = $lease.repository
        Worktree = $lease.worktree
        Branch = $lease.branch
        SourceHead = $lease.start_remote_main
    }
    $identity = Get-IdentityArguments -Fixture $cleanupFixture -Digest (Get-Digest -Path $cleanupFixture.LeasePath)
    $settleArguments = @('-Operation', 'Settle', '-LeasePath', $cleanupFixture.LeasePath) + $identity + @(
        '-ArchiveRoot', $cleanupFixture.ArchiveRoot,
        '-ArchivePath', $cleanupFixture.ArchivePath,
        '-ReceiptPath', $cleanupFixture.ReceiptPath,
        '-FinalPhase', 'SETTLED_TEST_FIXTURE_CLEANUP',
        '-Disposition', 'SETTLED_TEST_FIXTURE_CLEANUP'
    )
    $holderProperty = $lease.PSObject.Properties['holder']
    if ($null -ne $holderProperty -and -not [string]::IsNullOrWhiteSpace([string]$holderProperty.Value)) {
        $settleArguments += @('-ExpectedHolder', [string]$holderProperty.Value)
    }
    [void](Invoke-ToolJson -ToolArguments $settleArguments)
}

try {
    # 1-3 and 14: atomic acquire, second acquire refusal, exact settle, and no active holderless normal lease.
    $normalRoot = Join-Path $testRoot ($testPrefix + '-normal')
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
    $normalFixture = [pscustomobject]@{
        Root = $normalRoot
        LeasePath = $normalLease
        ArchiveRoot = $normalArchiveRoot
        ArchivePath = (Join-Path $normalArchiveRoot 'writer-lease.settled.v1.json')
        ReceiptPath = (Join-Path $normalArchiveRoot 'lease-settlement-receipt.txt')
        Goal = 'test-normal-acquire'
        Phase = 'ACTIVE_TEST_NORMAL'
        Schema = 'tabbeacon-writer-lease.v1'
        Repository = 'JerrySkywalker/tabbeacon'
        Worktree = $worktree
        Branch = $branch
        SourceHead = $sourceHead
    }
    $script:fixtureLedger += $normalFixture
    $acquired = Invoke-ToolJson -ToolArguments $normalAcquire
    Assert-True -Condition ($acquired.operation -eq 'acquire') -Message 'normal acquire did not report acquire'
    if ($InjectFailureAfterNormalAcquire) {
        throw 'WRITER_LEASE_TEST_INJECTED_FAILURE_AFTER_NORMAL_ACQUIRE'
    }
    $normalBytes = [IO.File]::ReadAllBytes($normalLease)
    $normalDigest = Get-Digest -Path $normalLease
    Assert-ToolFails -ToolArguments $normalAcquire -ExpectedToken 'WRITER_LEASE_ACQUIRE_BLOCKED_ACTIVE_LEASE_EXISTS'
    $scopeConflictDirectory = Join-Path $testRoot ($testPrefix + '-second-lease-path')
    New-Item -ItemType Directory -Path $scopeConflictDirectory -ErrorAction Stop | Out-Null
    $scopeConflictAcquire = @(
        '-Operation', 'Acquire', '-LeasePath', (Join-Path $scopeConflictDirectory 'writer-lease.json'),
        '-Goal', 'test-scope-conflict', '-Phase', 'ACTIVE_TEST_SCOPE_CONFLICT',
        '-Repository', 'JerrySkywalker/tabbeacon', '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    )
    Assert-ToolFails -ToolArguments $scopeConflictAcquire -ExpectedToken 'WRITER_LEASE_ACQUIRE_BLOCKED_SCOPE_CONFLICT'

    $normalIdentity = Get-IdentityArguments -Fixture $normalFixture -Digest $normalDigest
    $settled = Invoke-ToolJson -ToolArguments (@('-Operation', 'Settle', '-LeasePath', $normalLease) + $normalIdentity + @(
        '-ArchiveRoot', $normalFixture.ArchiveRoot,
        '-ArchivePath', $normalFixture.ArchivePath,
        '-ReceiptPath', $normalFixture.ReceiptPath,
        '-FinalPhase', 'SETTLED_TEST_NORMAL',
        '-Disposition', 'SETTLED_TEST_NORMAL'
    ))
    Assert-True -Condition ($settled.active_holderless_lease -eq $false) -Message 'settle reported an active holderless lease'
    Assert-True -Condition (Test-ExactBytes -First $normalBytes -Second ([IO.File]::ReadAllBytes($normalFixture.ArchivePath))) -Message 'normal settle archive changed bytes'
    Assert-True -Condition ((Get-Content -LiteralPath $normalFixture.ReceiptPath -Raw).IndexOf('DISPOSITION=SETTLED_TEST_NORMAL', [StringComparison]::Ordinal) -ge 0) -Message 'normal settle receipt missing'
    $normalStatus = Invoke-ToolJson -ToolArguments @('-Operation', 'Status', '-LeasePath', $normalLease)
    Assert-True -Condition ((-not $normalStatus.exists) -and (-not $normalStatus.active_holderless)) -Message 'normal lifecycle left active holderless lease'
    $normalTransaction = Join-Path $normalRoot 'writer-lease.transaction.v1.txt'
    Assert-True -Condition ((Get-Content -LiteralPath $normalTransaction -Raw).IndexOf('TRANSACTION=FINALIZED', [StringComparison]::Ordinal) -ge 0) -Message 'normal settlement did not finalize its task transaction marker'

    # Exercise the catch/finally path in a child process. The intentionally
    # failing child must settle the lease it acquired before it exits.
    $failureCleanupOutput = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath -InjectFailureAfterNormalAcquire 2>&1
    $failureCleanupExitCode = $LASTEXITCODE
    $failureCleanupText = $failureCleanupOutput | Out-String
    Assert-True -Condition ($failureCleanupExitCode -ne 0) -Message 'injected cleanup child unexpectedly succeeded'
    Assert-True -Condition ($failureCleanupText.IndexOf('WRITER_LEASE_TEST_INJECTED_FAILURE_AFTER_NORMAL_ACQUIRE', [StringComparison]::Ordinal) -ge 0) -Message 'injected cleanup child did not reach failure point'
    Assert-True -Condition ($failureCleanupText.IndexOf('WRITER_LEASE_TEST_ACTIVE_FIXTURE_LEAKS=0', [StringComparison]::Ordinal) -ge 0) -Message 'injected cleanup child left active fixture lease'
    Assert-True -Condition ($failureCleanupText.IndexOf('WRITER_LEASE_TEST_PREPARED_MARKER_LEAKS=0', [StringComparison]::Ordinal) -ge 0) -Message 'injected cleanup child left prepared marker'
    'WRITER_LEASE_TEST_FAILURE_CLEANUP=PASS'

    # A valid durable transaction blocks Acquire before any second writer can
    # reach the source path; cleanup completes it through RecoverPrepared.
    $pendingRoot = Join-Path $testRoot ($testPrefix + '-prepared-acquire-block')
    New-Item -ItemType Directory -Path $pendingRoot -ErrorAction Stop | Out-Null
    $pendingLease = Join-Path $pendingRoot 'writer-lease.json'
    $pendingArchiveRoot = Join-Path $pendingRoot 'archive'
    New-Item -ItemType Directory -Path $pendingArchiveRoot -ErrorAction Stop | Out-Null
    $pendingTransaction = Join-Path $pendingRoot 'writer-lease.transaction.v1.txt'
    $pendingFixture = [pscustomobject]@{
        Root = $pendingRoot
        LeasePath = $pendingLease
        ArchiveRoot = $pendingArchiveRoot
        ArchivePath = (Join-Path $pendingArchiveRoot 'writer-lease.prepared.v1.json')
        ReceiptPath = (Join-Path $pendingArchiveRoot 'lease-settlement-receipt.txt')
        Goal = 'test-prepared-acquire-block'
        Phase = 'ACTIVE_TEST_PREPARED_BLOCK'
        Schema = 'tabbeacon-writer-lease.v1'
        Repository = 'JerrySkywalker/tabbeacon'
        Worktree = $worktree
        Branch = $branch
        SourceHead = $sourceHead
    }
    $script:fixtureLedger += $pendingFixture
    $pendingAcquire = @(
        '-Operation', 'Acquire', '-LeasePath', $pendingLease,
        '-Goal', 'test-prepared-acquire-block', '-Phase', 'ACTIVE_TEST_PREPARED_BLOCK',
        '-Repository', 'JerrySkywalker/tabbeacon', '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    )
    [void](Invoke-ToolJson -ToolArguments $pendingAcquire)
    $pendingDigest = Get-Digest -Path $pendingLease
    $pendingPrepared = @(
        'TRANSACTION=PREPARED',
        'OPERATION=settle',
        ('ORIGINAL_LEASE_SHA256=' + $pendingDigest),
        ('ARCHIVE_PATH=' + $pendingFixture.ArchivePath),
        ('RECEIPT_PATH=' + $pendingFixture.ReceiptPath),
        'REPOSITORY=JerrySkywalker/tabbeacon',
        ('WORKTREE=' + $worktree),
        ('BRANCH=' + $branch),
        'DISPOSITION=SETTLED_TEST_PENDING',
        'FINAL_PHASE=SETTLED_TEST_PENDING',
        'ACTIVE_WRITER_COUNT=N/A'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($pendingTransaction, $pendingPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($pendingFixture.ReceiptPath, $pendingPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $pendingLease -Destination $pendingFixture.ArchivePath -ErrorAction Stop
    Assert-ToolFails -ToolArguments $pendingAcquire -ExpectedToken 'WRITER_LEASE_ACQUIRE_BLOCKED_PREPARED_TRANSACTION'
    $pendingIdentity = Get-IdentityArguments -Fixture $pendingFixture -Digest $pendingDigest
    [void](Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'Settle', '-LeasePath', $pendingLease) + $pendingIdentity + @(
        '-ArchiveRoot', $pendingFixture.ArchiveRoot,
        '-ArchivePath', $pendingFixture.ArchivePath,
        '-ReceiptPath', $pendingFixture.ReceiptPath
    )))
    'WRITER_LEASE_TEST_PREPARED_ACQUIRE_BLOCK=PASS'

    # A prepared transaction in a different task root still owns its recorded
    # repository/worktree scope after its exact source has moved; Acquire must
    # not bypass it by choosing a new path.
    $crossScopeMarkerRoot = Join-Path $testRoot ($testPrefix + '-prepared-cross-scope-marker')
    $crossScopeLeaseRoot = Join-Path $testRoot ($testPrefix + '-prepared-cross-scope-candidate')
    New-Item -ItemType Directory -Path $crossScopeMarkerRoot -ErrorAction Stop | Out-Null
    New-Item -ItemType Directory -Path $crossScopeLeaseRoot -ErrorAction Stop | Out-Null
    $crossScopeMarker = Join-Path $crossScopeMarkerRoot 'writer-lease.transaction.v1.txt'
    $crossScopeArchiveRoot = Join-Path $crossScopeMarkerRoot 'archive'
    New-Item -ItemType Directory -Path $crossScopeArchiveRoot -ErrorAction Stop | Out-Null
    $crossScopeFixture = [pscustomobject]@{
        Root = $crossScopeMarkerRoot
        LeasePath = (Join-Path $crossScopeMarkerRoot 'writer-lease.json')
        ArchiveRoot = $crossScopeArchiveRoot
        ArchivePath = (Join-Path $crossScopeArchiveRoot 'writer-lease.prepared.v1.json')
        ReceiptPath = (Join-Path $crossScopeArchiveRoot 'lease-settlement-receipt.txt')
        Goal = 'test-prepared-cross-scope'
        Phase = 'ACTIVE_TEST_PREPARED_CROSS_SCOPE'
        Schema = 'tabbeacon-writer-lease.v1'
        Repository = 'JerrySkywalker/tabbeacon'
        Worktree = $worktree
        Branch = $branch
        SourceHead = $sourceHead
    }
    $script:fixtureLedger += $crossScopeFixture
    [void](Invoke-ToolJson -ToolArguments @(
        '-Operation', 'Acquire', '-LeasePath', $crossScopeFixture.LeasePath,
        '-Goal', $crossScopeFixture.Goal, '-Phase', $crossScopeFixture.Phase,
        '-Repository', $crossScopeFixture.Repository, '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    ))
    $crossScopeDigest = Get-Digest -Path $crossScopeFixture.LeasePath
    $crossScopeContent = @(
        'TRANSACTION=PREPARED',
        'OPERATION=settle',
        ('ORIGINAL_LEASE_SHA256=' + $crossScopeDigest),
        ('ARCHIVE_PATH=' + $crossScopeFixture.ArchivePath),
        ('RECEIPT_PATH=' + $crossScopeFixture.ReceiptPath),
        'REPOSITORY=JerrySkywalker/tabbeacon',
        ('WORKTREE=' + $worktree),
        ('BRANCH=' + $branch),
        'DISPOSITION=SETTLED_TEST_CROSS_SCOPE',
        'FINAL_PHASE=SETTLED_TEST_CROSS_SCOPE',
        'ACTIVE_WRITER_COUNT=N/A'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($crossScopeMarker, $crossScopeContent + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($crossScopeFixture.ReceiptPath, $crossScopeContent + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $crossScopeFixture.LeasePath -Destination $crossScopeFixture.ArchivePath -ErrorAction Stop
    $crossScopeAcquire = @(
        '-Operation', 'Acquire', '-LeasePath', (Join-Path $crossScopeLeaseRoot 'writer-lease.json'),
        '-Goal', 'test-prepared-cross-scope', '-Phase', 'ACTIVE_TEST_PREPARED_CROSS_SCOPE',
        '-Repository', 'JerrySkywalker/tabbeacon', '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    )
    Assert-ToolFails -ToolArguments $crossScopeAcquire -ExpectedToken 'WRITER_LEASE_ACQUIRE_BLOCKED_PREPARED_SCOPE_CONFLICT'
    $crossScopeIdentity = Get-IdentityArguments -Fixture $crossScopeFixture -Digest $crossScopeDigest
    [void](Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'Settle', '-LeasePath', $crossScopeFixture.LeasePath) + $crossScopeIdentity + @(
        '-ArchiveRoot', $crossScopeFixture.ArchiveRoot,
        '-ArchivePath', $crossScopeFixture.ArchivePath,
        '-ReceiptPath', $crossScopeFixture.ReceiptPath
    )))
    'WRITER_LEASE_TEST_PREPARED_CROSS_SCOPE_BLOCK=PASS'
    'WRITER_LEASE_TEST_NORMAL_LIFECYCLE=PASS'

    # A durable PREPARED receipt either resumes its exact source archive or, after a simulated
    # post-move crash, finalizes the exact archived bytes.  These fixtures model only the
    # interrupted transaction boundary; normal lifecycle moves always go through the tool.
    $preparedFixture = New-FixtureLease -Name 'prepared-resume'
    New-ArchiveRoot -Fixture $preparedFixture
    $preparedBytes = [IO.File]::ReadAllBytes($preparedFixture.LeasePath)
    $preparedDigest = Get-Digest -Path $preparedFixture.LeasePath
    $preparedContent = @(
        'TRANSACTION=PREPARED',
        'OPERATION=settle',
        ('ORIGINAL_LEASE_SHA256=' + $preparedDigest),
        ('ARCHIVE_PATH=' + $preparedFixture.ArchivePath),
        'DISPOSITION=SETTLED_TEST_PREPARED',
        'FINAL_PHASE=SETTLED_TEST_PREPARED',
        'ACTIVE_WRITER_COUNT=N/A'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($preparedFixture.ReceiptPath, $preparedContent + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $preparedIdentity = Get-IdentityArguments -Fixture $preparedFixture -Digest $preparedDigest
    $preparedRecovered = Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'Settle', '-LeasePath', $preparedFixture.LeasePath) + $preparedIdentity + @(
        '-ArchiveRoot', $preparedFixture.ArchiveRoot,
        '-ArchivePath', $preparedFixture.ArchivePath,
        '-ReceiptPath', $preparedFixture.ReceiptPath
    ))
    Assert-True -Condition ($preparedRecovered.recovery_state -eq 'resumed-archive') -Message 'prepared recovery did not resume archive'
    Assert-True -Condition (Test-ExactBytes -First $preparedBytes -Second ([IO.File]::ReadAllBytes($preparedFixture.ArchivePath))) -Message 'prepared recovery archive changed bytes'

    $finalizeFixture = New-FixtureLease -Name 'prepared-finalize'
    New-ArchiveRoot -Fixture $finalizeFixture
    $finalizeBytes = [IO.File]::ReadAllBytes($finalizeFixture.LeasePath)
    $finalizeDigest = Get-Digest -Path $finalizeFixture.LeasePath
    $finalizeContent = @(
        'TRANSACTION=PREPARED',
        'OPERATION=settle',
        ('ORIGINAL_LEASE_SHA256=' + $finalizeDigest),
        ('ARCHIVE_PATH=' + $finalizeFixture.ArchivePath),
        'DISPOSITION=SETTLED_TEST_FINALIZE',
        'FINAL_PHASE=SETTLED_TEST_FINALIZE',
        'ACTIVE_WRITER_COUNT=N/A'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($finalizeFixture.ReceiptPath, $finalizeContent + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $finalizeFixture.LeasePath -Destination $finalizeFixture.ArchivePath -ErrorAction Stop
    $finalizeIdentity = Get-IdentityArguments -Fixture $finalizeFixture -Digest $finalizeDigest
    $finalizeRecovered = Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'Settle', '-LeasePath', $finalizeFixture.LeasePath) + $finalizeIdentity + @(
        '-ArchiveRoot', $finalizeFixture.ArchiveRoot,
        '-ArchivePath', $finalizeFixture.ArchivePath,
        '-ReceiptPath', $finalizeFixture.ReceiptPath
    ))
    Assert-True -Condition ($finalizeRecovered.recovery_state -eq 'finalized-existing-archive') -Message 'prepared recovery did not finalize existing archive'
    Assert-True -Condition (Test-ExactBytes -First $finalizeBytes -Second ([IO.File]::ReadAllBytes($finalizeFixture.ArchivePath))) -Message 'prepared finalize archive changed bytes'
    Assert-True -Condition ((Get-Content -LiteralPath $finalizeFixture.ReceiptPath -Raw).IndexOf('DISPOSITION=SETTLED_TEST_FINALIZE', [StringComparison]::Ordinal) -ge 0) -Message 'prepared finalize receipt missing'

    # If power fails after the external receipt is made final but before the
    # task marker is finalized, RecoverPrepared deterministically finalizes
    # that marker without rewriting the archived lease.
    $finalReceiptFixture = New-FixtureLease -Name 'prepared-final-receipt'
    New-ArchiveRoot -Fixture $finalReceiptFixture
    $finalReceiptBytes = [IO.File]::ReadAllBytes($finalReceiptFixture.LeasePath)
    $finalReceiptDigest = Get-Digest -Path $finalReceiptFixture.LeasePath
    $finalReceiptMarker = Join-Path $finalReceiptFixture.Root 'writer-lease.transaction.v1.txt'
    $finalReceiptPrepared = @(
        'TRANSACTION=PREPARED',
        'OPERATION=settle',
        ('ORIGINAL_LEASE_SHA256=' + $finalReceiptDigest),
        ('ARCHIVE_PATH=' + $finalReceiptFixture.ArchivePath),
        ('RECEIPT_PATH=' + $finalReceiptFixture.ReceiptPath),
        ('REPOSITORY=' + $finalReceiptFixture.Repository),
        ('WORKTREE=' + $finalReceiptFixture.Worktree),
        ('BRANCH=' + $finalReceiptFixture.Branch),
        'DISPOSITION=SETTLED_TEST_FINAL_RECEIPT',
        'FINAL_PHASE=SETTLED_TEST_FINAL_RECEIPT',
        'ACTIVE_WRITER_COUNT=N/A'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($finalReceiptMarker, $finalReceiptPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $finalReceiptFixture.LeasePath -Destination $finalReceiptFixture.ArchivePath -ErrorAction Stop
    $finalReceiptContent = @(
        'DISPOSITION=SETTLED_TEST_FINAL_RECEIPT',
        'OPERATION=settle',
        ('ORIGINAL_LEASE_PATH=' + $finalReceiptFixture.LeasePath),
        ('ORIGINAL_LEASE_SHA256=' + $finalReceiptDigest),
        ('ARCHIVED_LEASE_PATH=' + $finalReceiptFixture.ArchivePath),
        ('ARCHIVED_LEASE_SHA256=' + $finalReceiptDigest),
        ('SCHEMA=' + $finalReceiptFixture.Schema),
        ('GOAL=' + $finalReceiptFixture.Goal),
        ('PHASE=' + $finalReceiptFixture.Phase),
        'FINAL_PHASE=SETTLED_TEST_FINAL_RECEIPT',
        'ACTIVE_WRITER_COUNT=N/A',
        'LEASE_CONTENT_MODIFIED=false'
    ) -join [Environment]::NewLine
    $finalReceiptFixture | Add-Member -NotePropertyName CleanupFinalReceiptContent -NotePropertyValue ($finalReceiptContent + [Environment]::NewLine)
    [IO.File]::WriteAllText($finalReceiptFixture.ReceiptPath, $finalReceiptContent + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $finalReceiptIdentity = Get-IdentityArguments -Fixture $finalReceiptFixture -Digest $finalReceiptDigest
    $finalReceiptRecoveryArguments = @('-Operation', 'RecoverPrepared', '-PreparedOperation', 'Settle', '-LeasePath', $finalReceiptFixture.LeasePath) + $finalReceiptIdentity + @(
        '-ArchiveRoot', $finalReceiptFixture.ArchiveRoot,
        '-ArchivePath', $finalReceiptFixture.ArchivePath,
        '-ReceiptPath', $finalReceiptFixture.ReceiptPath
    )
    $tamperedFinalReceipt = $finalReceiptContent.Replace('DISPOSITION=SETTLED_TEST_FINAL_RECEIPT', 'DISPOSITION=TAMPERED_TEST_FINAL_RECEIPT')
    [IO.File]::WriteAllText($finalReceiptFixture.ReceiptPath, $tamperedFinalReceipt + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Assert-ToolFails -ToolArguments $finalReceiptRecoveryArguments -ExpectedToken 'WRITER_LEASE_FINAL_RECEIPT_IDENTITY_MISMATCH'
    [IO.File]::WriteAllText($finalReceiptFixture.ReceiptPath, $finalReceiptContent + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $finalReceiptRecovered = Invoke-ToolJson -ToolArguments $finalReceiptRecoveryArguments
    Assert-True -Condition ($finalReceiptRecovered.recovery_state -eq 'finalized-existing-receipt') -Message 'final receipt recovery did not finalize marker'
    Assert-True -Condition (Test-ExactBytes -First $finalReceiptBytes -Second ([IO.File]::ReadAllBytes($finalReceiptFixture.ArchivePath))) -Message 'final receipt recovery changed archive bytes'
    Assert-True -Condition ((Get-Content -LiteralPath $finalReceiptMarker -Raw).IndexOf('TRANSACTION=FINALIZED', [StringComparison]::Ordinal) -ge 0) -Message 'final receipt recovery did not finalize task marker'
    'WRITER_LEASE_TEST_FINAL_RECEIPT_MARKER_RECOVERY=PASS'

    # A resumed orphan reclaim can require a newer zero-writer proof. Its final
    # receipt must retain the original prepared-proof binding as provenance, so
    # a second recovery can finish a simulated post-receipt crash.
    $refreshProofFixture = New-FixtureLease -Name 'prepared-refresh-proof-crash'
    New-ArchiveRoot -Fixture $refreshProofFixture
    $refreshProofDigest = Get-Digest -Path $refreshProofFixture.LeasePath
    $originalRefreshProof = New-ActiveWriterProof -Fixture $refreshProofFixture -LeaseDigest $refreshProofDigest
    $refreshProofMarker = Join-Path $refreshProofFixture.Root 'writer-lease.transaction.v1.txt'
    $refreshProofPrepared = @(
        'TRANSACTION=PREPARED',
        'OPERATION=reclaim-orphan',
        ('ORIGINAL_LEASE_SHA256=' + $refreshProofDigest),
        ('ARCHIVE_PATH=' + $refreshProofFixture.ArchivePath),
        ('RECEIPT_PATH=' + $refreshProofFixture.ReceiptPath),
        ('REPOSITORY=' + $refreshProofFixture.Repository),
        ('WORKTREE=' + $refreshProofFixture.Worktree),
        ('BRANCH=' + $refreshProofFixture.Branch),
        'DISPOSITION=ORPHAN_RECLAIMED',
        'FINAL_PHASE=RECLAIMED_ORPHAN',
        'ACTIVE_WRITER_COUNT=0',
        ('ACTIVE_WRITER_PROOF_PATH=' + $originalRefreshProof.Path),
        ('ACTIVE_WRITER_PROOF_SHA256=' + $originalRefreshProof.Sha256)
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($refreshProofMarker, $refreshProofPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($refreshProofFixture.ReceiptPath, $refreshProofPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $freshRefreshProof = New-ActiveWriterProof -Fixture $refreshProofFixture -LeaseDigest $refreshProofDigest
    Assert-True -Condition ($freshRefreshProof.Sha256 -ne $originalRefreshProof.Sha256) -Message 'fresh reclaim proof did not replace prior proof bytes'
    $refreshProofIdentity = Get-IdentityArguments -Fixture $refreshProofFixture -Digest $refreshProofDigest
    [void](Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'ReclaimOrphan', '-LeasePath', $refreshProofFixture.LeasePath) + $refreshProofIdentity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $refreshProofFixture.ArchiveRoot,
        '-ArchivePath', $refreshProofFixture.ArchivePath,
        '-ReceiptPath', $refreshProofFixture.ReceiptPath,
        '-ActiveWriterProofPath', $freshRefreshProof.Path,
        '-ExpectedActiveWriterProofSha256', $freshRefreshProof.Sha256
    )))
    $refreshProofFinalReceipt = Get-Content -LiteralPath $refreshProofFixture.ReceiptPath -Raw
    Assert-True -Condition ($refreshProofFinalReceipt.IndexOf('RECOVERY_PROOF_REFRESH=true', [StringComparison]::Ordinal) -ge 0) -Message 'fresh reclaim receipt omitted refresh provenance marker'
    Assert-True -Condition ($refreshProofFinalReceipt.IndexOf(('PREPARED_WRITER_PROOF_SHA256=' + $originalRefreshProof.Sha256), [StringComparison]::Ordinal) -ge 0) -Message 'fresh reclaim receipt omitted original prepared proof'
    Assert-True -Condition ($refreshProofFinalReceipt.IndexOf(('ACTIVE_WRITER_PROOF_SHA256=' + $freshRefreshProof.Sha256), [StringComparison]::Ordinal) -ge 0) -Message 'fresh reclaim receipt omitted recovery proof'
    $refreshProofFixture | Add-Member -NotePropertyName CleanupFinalReceiptContent -NotePropertyValue ($refreshProofFinalReceipt)
    $refreshProofPreparedForFinalization = $refreshProofPrepared.Replace(('ACTIVE_WRITER_PROOF_SHA256=' + $originalRefreshProof.Sha256), ('ACTIVE_WRITER_PROOF_SHA256=' + $freshRefreshProof.Sha256)) + [Environment]::NewLine + ('PREPARED_WRITER_PROOF_PATH=' + $originalRefreshProof.Path) + [Environment]::NewLine + ('PREPARED_WRITER_PROOF_SHA256=' + $originalRefreshProof.Sha256) + [Environment]::NewLine + 'RECOVERY_PROOF_REFRESH=true'
    [IO.File]::WriteAllText($refreshProofMarker, $refreshProofPreparedForFinalization + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $tamperedRefreshProofFinalReceipt = $refreshProofFinalReceipt.Replace(('ACTIVE_WRITER_PROOF_SHA256=' + $freshRefreshProof.Sha256), ('ACTIVE_WRITER_PROOF_SHA256=' + ('0' * 64 -join '')))
    [IO.File]::WriteAllText($refreshProofFixture.ReceiptPath, $tamperedRefreshProofFinalReceipt, [Text.UTF8Encoding]::new($false))
    Assert-ToolFails -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'ReclaimOrphan', '-LeasePath', $refreshProofFixture.LeasePath) + $refreshProofIdentity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $refreshProofFixture.ArchiveRoot,
        '-ArchivePath', $refreshProofFixture.ArchivePath,
        '-ReceiptPath', $refreshProofFixture.ReceiptPath,
        '-ActiveWriterProofPath', $freshRefreshProof.Path,
        '-ExpectedActiveWriterProofSha256', $freshRefreshProof.Sha256
    )) -ExpectedToken 'WRITER_LEASE_FINAL_RECEIPT_IDENTITY_MISMATCH'
    [IO.File]::WriteAllText($refreshProofFixture.ReceiptPath, $refreshProofFinalReceipt, [Text.UTF8Encoding]::new($false))
    $refreshProofRecovered = Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'ReclaimOrphan', '-LeasePath', $refreshProofFixture.LeasePath) + $refreshProofIdentity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $refreshProofFixture.ArchiveRoot,
        '-ArchivePath', $refreshProofFixture.ArchivePath,
        '-ReceiptPath', $refreshProofFixture.ReceiptPath,
        '-ActiveWriterProofPath', $freshRefreshProof.Path,
        '-ExpectedActiveWriterProofSha256', $freshRefreshProof.Sha256
    ))
    Assert-True -Condition ($refreshProofRecovered.recovery_state -eq 'finalized-existing-receipt') -Message 'fresh proof final receipt crash state did not recover'
    'WRITER_LEASE_TEST_REFRESHED_PROOF_FINAL_RECEIPT_RECOVERY=PASS'

    # A second interruption can leave an expired bound proof before the archive
    # move. A fresh C proof must replace it while retaining original A as
    # provenance; otherwise PREPARED permanently blocks the scope.
    $expiredRefreshFixture = New-FixtureLease -Name 'prepared-expired-refresh-rotation'
    New-ArchiveRoot -Fixture $expiredRefreshFixture
    $expiredRefreshDigest = Get-Digest -Path $expiredRefreshFixture.LeasePath
    $expiredRefreshOriginalProof = New-ActiveWriterProof -Fixture $expiredRefreshFixture -LeaseDigest $expiredRefreshDigest -ProofFileName 'proof-a.txt'
    $expiredRefreshBoundProof = New-ActiveWriterProof -Fixture $expiredRefreshFixture -LeaseDigest $expiredRefreshDigest -ProofFileName 'proof-b-expired.txt' -AgeMinutes 10
    $expiredRefreshMarker = Join-Path $expiredRefreshFixture.Root 'writer-lease.transaction.v1.txt'
    $expiredRefreshPrepared = @(
        'TRANSACTION=PREPARED',
        'OPERATION=reclaim-orphan',
        ('ORIGINAL_LEASE_SHA256=' + $expiredRefreshDigest),
        ('ARCHIVE_PATH=' + $expiredRefreshFixture.ArchivePath),
        ('RECEIPT_PATH=' + $expiredRefreshFixture.ReceiptPath),
        ('REPOSITORY=' + $expiredRefreshFixture.Repository),
        ('WORKTREE=' + $expiredRefreshFixture.Worktree),
        ('BRANCH=' + $expiredRefreshFixture.Branch),
        'DISPOSITION=ORPHAN_RECLAIMED',
        'FINAL_PHASE=RECLAIMED_ORPHAN',
        'ACTIVE_WRITER_COUNT=0',
        ('ACTIVE_WRITER_PROOF_PATH=' + $expiredRefreshBoundProof.Path),
        ('ACTIVE_WRITER_PROOF_SHA256=' + $expiredRefreshBoundProof.Sha256),
        ('PREPARED_WRITER_PROOF_PATH=' + $expiredRefreshOriginalProof.Path),
        ('PREPARED_WRITER_PROOF_SHA256=' + $expiredRefreshOriginalProof.Sha256),
        'RECOVERY_PROOF_REFRESH=true'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($expiredRefreshMarker, $expiredRefreshPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($expiredRefreshFixture.ReceiptPath, $expiredRefreshPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $expiredRefreshReplacementProof = New-ActiveWriterProof -Fixture $expiredRefreshFixture -LeaseDigest $expiredRefreshDigest -ProofFileName 'proof-c.txt'
    $expiredRefreshIdentity = Get-IdentityArguments -Fixture $expiredRefreshFixture -Digest $expiredRefreshDigest
    $expiredRefreshRecovered = Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'ReclaimOrphan', '-LeasePath', $expiredRefreshFixture.LeasePath) + $expiredRefreshIdentity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $expiredRefreshFixture.ArchiveRoot,
        '-ArchivePath', $expiredRefreshFixture.ArchivePath,
        '-ReceiptPath', $expiredRefreshFixture.ReceiptPath,
        '-ActiveWriterProofPath', $expiredRefreshReplacementProof.Path,
        '-ExpectedActiveWriterProofSha256', $expiredRefreshReplacementProof.Sha256
    ))
    Assert-True -Condition ($expiredRefreshRecovered.recovery_state -eq 'resumed-archive') -Message 'expired bound proof did not rotate before archive'
    $expiredRefreshFinalReceipt = Get-Content -LiteralPath $expiredRefreshFixture.ReceiptPath -Raw
    Assert-True -Condition ($expiredRefreshFinalReceipt.IndexOf(('ACTIVE_WRITER_PROOF_SHA256=' + $expiredRefreshReplacementProof.Sha256), [StringComparison]::Ordinal) -ge 0) -Message 'expired bound proof recovery did not bind replacement proof'
    Assert-True -Condition ($expiredRefreshFinalReceipt.IndexOf(('PREPARED_WRITER_PROOF_SHA256=' + $expiredRefreshOriginalProof.Sha256), [StringComparison]::Ordinal) -ge 0) -Message 'expired bound proof recovery lost original provenance'
    'WRITER_LEASE_TEST_EXPIRED_BOUND_PROOF_ROTATION=PASS'

    # Once the archive and final receipt exist, their marker-bound proof is
    # historical evidence. Marker-only finalization must not demand that it is
    # still within the five-minute observation window.
    $expiredFinalFixture = New-FixtureLease -Name 'prepared-expired-final-receipt'
    New-ArchiveRoot -Fixture $expiredFinalFixture
    $expiredFinalDigest = Get-Digest -Path $expiredFinalFixture.LeasePath
    $expiredFinalOriginalProof = New-ActiveWriterProof -Fixture $expiredFinalFixture -LeaseDigest $expiredFinalDigest -ProofFileName 'proof-a.txt'
    $expiredFinalBoundProof = New-ActiveWriterProof -Fixture $expiredFinalFixture -LeaseDigest $expiredFinalDigest -ProofFileName 'proof-b-expired.txt' -AgeMinutes 10
    $expiredFinalMarker = Join-Path $expiredFinalFixture.Root 'writer-lease.transaction.v1.txt'
    $expiredFinalPrepared = @(
        'TRANSACTION=PREPARED',
        'OPERATION=reclaim-orphan',
        ('ORIGINAL_LEASE_SHA256=' + $expiredFinalDigest),
        ('ARCHIVE_PATH=' + $expiredFinalFixture.ArchivePath),
        ('RECEIPT_PATH=' + $expiredFinalFixture.ReceiptPath),
        ('REPOSITORY=' + $expiredFinalFixture.Repository),
        ('WORKTREE=' + $expiredFinalFixture.Worktree),
        ('BRANCH=' + $expiredFinalFixture.Branch),
        'DISPOSITION=ORPHAN_RECLAIMED',
        'FINAL_PHASE=RECLAIMED_ORPHAN',
        'ACTIVE_WRITER_COUNT=0',
        ('ACTIVE_WRITER_PROOF_PATH=' + $expiredFinalBoundProof.Path),
        ('ACTIVE_WRITER_PROOF_SHA256=' + $expiredFinalBoundProof.Sha256),
        ('PREPARED_WRITER_PROOF_PATH=' + $expiredFinalOriginalProof.Path),
        ('PREPARED_WRITER_PROOF_SHA256=' + $expiredFinalOriginalProof.Sha256),
        'RECOVERY_PROOF_REFRESH=true'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($expiredFinalMarker, $expiredFinalPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $expiredFinalFixture.LeasePath -Destination $expiredFinalFixture.ArchivePath -ErrorAction Stop
    $expiredFinalReceipt = @(
        'DISPOSITION=ORPHAN_RECLAIMED',
        'OPERATION=reclaim-orphan',
        ('ORIGINAL_LEASE_PATH=' + $expiredFinalFixture.LeasePath),
        ('ORIGINAL_LEASE_SHA256=' + $expiredFinalDigest),
        ('ARCHIVED_LEASE_PATH=' + $expiredFinalFixture.ArchivePath),
        ('ARCHIVED_LEASE_SHA256=' + $expiredFinalDigest),
        ('SCHEMA=' + $expiredFinalFixture.Schema),
        ('GOAL=' + $expiredFinalFixture.Goal),
        ('PHASE=' + $expiredFinalFixture.Phase),
        'FINAL_PHASE=RECLAIMED_ORPHAN',
        'ACTIVE_WRITER_COUNT=0',
        'LEASE_CONTENT_MODIFIED=false',
        ('ACTIVE_WRITER_PROOF_PATH=' + $expiredFinalBoundProof.Path),
        ('ACTIVE_WRITER_PROOF_SHA256=' + $expiredFinalBoundProof.Sha256),
        ('PREPARED_WRITER_PROOF_PATH=' + $expiredFinalOriginalProof.Path),
        ('PREPARED_WRITER_PROOF_SHA256=' + $expiredFinalOriginalProof.Sha256),
        'RECOVERY_PROOF_REFRESH=true'
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($expiredFinalFixture.ReceiptPath, $expiredFinalReceipt + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $expiredFinalIdentity = Get-IdentityArguments -Fixture $expiredFinalFixture -Digest $expiredFinalDigest
    $expiredFinalRecovered = Invoke-ToolJson -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'ReclaimOrphan', '-LeasePath', $expiredFinalFixture.LeasePath) + $expiredFinalIdentity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $expiredFinalFixture.ArchiveRoot,
        '-ArchivePath', $expiredFinalFixture.ArchivePath,
        '-ReceiptPath', $expiredFinalFixture.ReceiptPath
    ))
    Assert-True -Condition ($expiredFinalRecovered.recovery_state -eq 'finalized-existing-receipt') -Message 'expired historical proof did not finalize marker-only recovery'
    Assert-True -Condition ((Get-Content -LiteralPath $expiredFinalMarker -Raw).IndexOf('TRANSACTION=FINALIZED', [StringComparison]::Ordinal) -ge 0) -Message 'expired historical proof did not finalize marker'
    'WRITER_LEASE_TEST_EXPIRED_FINAL_PROOF_MARKER_RECOVERY=PASS'

    # A final orphan receipt must carry precisely the proof binding recorded by
    # its prepared transaction; a substituted proof cannot finalize the marker.
    $proofFinalFixture = New-FixtureLease -Name 'prepared-final-proof-mismatch'
    New-ArchiveRoot -Fixture $proofFinalFixture
    $proofFinalDigest = Get-Digest -Path $proofFinalFixture.LeasePath
    $proofFinalProof = New-ActiveWriterProof -Fixture $proofFinalFixture -LeaseDigest $proofFinalDigest
    $proofFinalMarker = Join-Path $proofFinalFixture.Root 'writer-lease.transaction.v1.txt'
    $proofFinalPrepared = @(
        'TRANSACTION=PREPARED',
        'OPERATION=reclaim-orphan',
        ('ORIGINAL_LEASE_SHA256=' + $proofFinalDigest),
        ('ARCHIVE_PATH=' + $proofFinalFixture.ArchivePath),
        ('RECEIPT_PATH=' + $proofFinalFixture.ReceiptPath),
        ('REPOSITORY=' + $proofFinalFixture.Repository),
        ('WORKTREE=' + $proofFinalFixture.Worktree),
        ('BRANCH=' + $proofFinalFixture.Branch),
        'DISPOSITION=ORPHAN_RECLAIMED',
        'FINAL_PHASE=RECLAIMED_ORPHAN',
        'ACTIVE_WRITER_COUNT=0',
        ('ACTIVE_WRITER_PROOF_PATH=' + $proofFinalProof.Path),
        ('ACTIVE_WRITER_PROOF_SHA256=' + $proofFinalProof.Sha256)
    ) -join [Environment]::NewLine
    [IO.File]::WriteAllText($proofFinalMarker, $proofFinalPrepared + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $proofFinalFixture.LeasePath -Destination $proofFinalFixture.ArchivePath -ErrorAction Stop
    $proofFinalValidReceipt = @(
        'DISPOSITION=ORPHAN_RECLAIMED',
        'OPERATION=reclaim-orphan',
        ('ORIGINAL_LEASE_PATH=' + $proofFinalFixture.LeasePath),
        ('ORIGINAL_LEASE_SHA256=' + $proofFinalDigest),
        ('ARCHIVED_LEASE_PATH=' + $proofFinalFixture.ArchivePath),
        ('ARCHIVED_LEASE_SHA256=' + $proofFinalDigest),
        ('SCHEMA=' + $proofFinalFixture.Schema),
        ('GOAL=' + $proofFinalFixture.Goal),
        ('PHASE=' + $proofFinalFixture.Phase),
        'FINAL_PHASE=RECLAIMED_ORPHAN',
        'ACTIVE_WRITER_COUNT=0',
        'LEASE_CONTENT_MODIFIED=false',
        ('ACTIVE_WRITER_PROOF_PATH=' + $proofFinalProof.Path),
        ('ACTIVE_WRITER_PROOF_SHA256=' + $proofFinalProof.Sha256)
    ) -join [Environment]::NewLine
    $proofFinalFixture | Add-Member -NotePropertyName CleanupFinalReceiptContent -NotePropertyValue ($proofFinalValidReceipt + [Environment]::NewLine)
    $proofFinalReceipt = $proofFinalValidReceipt.Replace(('ACTIVE_WRITER_PROOF_SHA256=' + $proofFinalProof.Sha256), ('ACTIVE_WRITER_PROOF_SHA256=' + ('0' * 64 -join '')))
    [IO.File]::WriteAllText($proofFinalFixture.ReceiptPath, $proofFinalReceipt + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    $proofFinalIdentity = Get-IdentityArguments -Fixture $proofFinalFixture -Digest $proofFinalDigest
    Assert-ToolFails -ToolArguments (@('-Operation', 'RecoverPrepared', '-PreparedOperation', 'ReclaimOrphan', '-LeasePath', $proofFinalFixture.LeasePath) + $proofFinalIdentity + @(
        '-ExpectedHolderless',
        '-ArchiveRoot', $proofFinalFixture.ArchiveRoot,
        '-ArchivePath', $proofFinalFixture.ArchivePath,
        '-ReceiptPath', $proofFinalFixture.ReceiptPath,
        '-ActiveWriterProofPath', $proofFinalProof.Path,
        '-ExpectedActiveWriterProofSha256', $proofFinalProof.Sha256
    )) -ExpectedToken 'WRITER_LEASE_FINAL_RECEIPT_IDENTITY_MISMATCH'
    'WRITER_LEASE_TEST_FINAL_RECEIPT_PROOF_TAMPER=PASS'
    'WRITER_LEASE_TEST_PREPARED_RECOVERY=PASS'

    # 4, 11, 12, and 13: exact orphan reclaim preserves bytes, produces a receipt, and releases a path for a fresh acquire.
    $reclaimFixture = New-FixtureLease -Name 'reclaim-success'
    New-ArchiveRoot -Fixture $reclaimFixture
    $reclaimBytes = [IO.File]::ReadAllBytes($reclaimFixture.LeasePath)
    $reclaimDigest = Get-Digest -Path $reclaimFixture.LeasePath
    $reclaimed = Invoke-ToolJson -ToolArguments (Get-ReclaimArguments -Fixture $reclaimFixture -Digest $reclaimDigest)
    Assert-True -Condition ($reclaimed.disposition -eq 'ORPHAN_RECLAIMED') -Message 'orphan reclaim disposition mismatch'
    Assert-True -Condition (Test-ExactBytes -First $reclaimBytes -Second ([IO.File]::ReadAllBytes($reclaimFixture.ArchivePath))) -Message 'orphan archive changed bytes'
    Assert-True -Condition ((Get-Content -LiteralPath $reclaimFixture.ReceiptPath -Raw).IndexOf('DISPOSITION=ORPHAN_RECLAIMED', [StringComparison]::Ordinal) -ge 0) -Message 'orphan receipt missing'
    $postReclaimFixture = [pscustomobject]@{
        Root = $reclaimFixture.Root
        LeasePath = $reclaimFixture.LeasePath
        ArchiveRoot = $reclaimFixture.ArchiveRoot
        ArchivePath = (Join-Path $reclaimFixture.ArchiveRoot 'writer-lease.post-reclaim.settled.v1.json')
        ReceiptPath = (Join-Path $reclaimFixture.ArchiveRoot 'post-reclaim-settlement-receipt.txt')
        Goal = 'test-post-reclaim-acquire'
        Phase = 'ACTIVE_TEST_POST_RECLAIM'
        Schema = 'tabbeacon-writer-lease.v1'
        Repository = 'JerrySkywalker/tabbeacon'
        Worktree = $worktree
        Branch = $branch
        SourceHead = $sourceHead
    }
    $script:fixtureLedger += $postReclaimFixture
    $postReclaimAcquire = Invoke-ToolJson -ToolArguments @(
        '-Operation', 'Acquire', '-LeasePath', $reclaimFixture.LeasePath,
        '-Goal', 'test-post-reclaim-acquire', '-Phase', 'ACTIVE_TEST_POST_RECLAIM',
        '-Repository', 'JerrySkywalker/tabbeacon', '-SourceHead', $sourceHead,
        '-Worktree', $worktree, '-Branch', $branch
    )
    Assert-True -Condition ($postReclaimAcquire.operation -eq 'acquire') -Message 'fresh acquire after reclaim failed'
    $postReclaimDigest = Get-Digest -Path $postReclaimFixture.LeasePath
    $postReclaimIdentity = Get-IdentityArguments -Fixture $postReclaimFixture -Digest $postReclaimDigest
    [void](Invoke-ToolJson -ToolArguments (@('-Operation', 'Settle', '-LeasePath', $postReclaimFixture.LeasePath) + $postReclaimIdentity + @(
        '-ArchiveRoot', $postReclaimFixture.ArchiveRoot,
        '-ArchivePath', $postReclaimFixture.ArchivePath,
        '-ReceiptPath', $postReclaimFixture.ReceiptPath,
        '-FinalPhase', 'SETTLED_TEST_POST_RECLAIM',
        '-Disposition', 'SETTLED_TEST_POST_RECLAIM'
    )))
    Assert-True -Condition (-not (Test-Path -LiteralPath $postReclaimFixture.LeasePath -PathType Leaf)) -Message 'fresh acquire cleanup left an active lease'
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

    # Unsupported schemas cannot be reclaimed just because a caller repeats their string.
    $unsupportedFixture = New-FixtureLease -Name 'unsupported-schema' -State 'CLOSED_TEST_UNSUPPORTED_SCHEMA' -Schema 'tabbeacon-writer-lease.future'
    New-ArchiveRoot -Fixture $unsupportedFixture
    $unsupportedDigest = Get-Digest -Path $unsupportedFixture.LeasePath
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $unsupportedFixture -Digest $unsupportedDigest) -ExpectedToken 'WRITER_LEASE_UNSUPPORTED_SCHEMA'
    'WRITER_LEASE_TEST_UNSUPPORTED_SCHEMA=PASS'

    # The zero-writer assertion must be a hash-bound evidence record, not a loose command-line value.
    $proofFixture = New-FixtureLease -Name 'writer-proof'
    New-ArchiveRoot -Fixture $proofFixture
    $proofDigest = Get-Digest -Path $proofFixture.LeasePath
    $proofArguments = [Collections.Generic.List[string]](Get-ReclaimArguments -Fixture $proofFixture -Digest $proofDigest)
    $proofPathIndex = $proofArguments.IndexOf('-ActiveWriterProofPath') + 1
    $proofHashIndex = $proofArguments.IndexOf('-ExpectedActiveWriterProofSha256') + 1
    $proofContent = (Get-Content -LiteralPath $proofArguments[$proofPathIndex] -Raw).Replace('ACTIVE_WRITER_COUNT=0', 'ACTIVE_WRITER_COUNT=1')
    [IO.File]::WriteAllText($proofArguments[$proofPathIndex], $proofContent, [Text.UTF8Encoding]::new($false))
    $proofArguments[$proofHashIndex] = Get-Digest -Path $proofArguments[$proofPathIndex]
    Assert-ToolFails -ToolArguments $proofArguments.ToArray() -ExpectedToken 'WRITER_LEASE_ACTIVE_WRITER_PROOF_COUNT_NOT_ZERO'
    'WRITER_LEASE_TEST_ACTIVE_WRITER_PROOF=PASS'

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

    # A receipt collision must be rejected before the task transaction marker
    # is created, so it cannot poison a later acquire or recovery.
    $receiptCollisionFixture = New-FixtureLease -Name 'receipt-collision'
    New-ArchiveRoot -Fixture $receiptCollisionFixture
    [IO.File]::WriteAllText($receiptCollisionFixture.ReceiptPath, 'existing settlement receipt', [Text.UTF8Encoding]::new($false))
    $receiptCollisionDigest = Get-Digest -Path $receiptCollisionFixture.LeasePath
    Assert-ToolFails -ToolArguments (Get-ReclaimArguments -Fixture $receiptCollisionFixture -Digest $receiptCollisionDigest) -ExpectedToken 'WRITER_LEASE_RECEIPT_COLLISION'
    Assert-True -Condition (-not (Test-Path -LiteralPath (Join-Path $receiptCollisionFixture.Root 'writer-lease.transaction.v1.txt') -PathType Leaf)) -Message 'receipt collision created a transaction marker'
    Assert-True -Condition (Test-Path -LiteralPath $receiptCollisionFixture.LeasePath) -Message 'receipt collision removed source lease'
    'WRITER_LEASE_TEST_RECEIPT_COLLISION=PASS'

    # Normal settlement with a declared holder requires explicit caller
    # confirmation, while orphan reclaim always rejects that holder.
    $settlementHolderFixture = New-FixtureLease -Name 'settlement-holder-confirmation' -Holder 'test-confirmed-holder'
    New-ArchiveRoot -Fixture $settlementHolderFixture
    $settlementHolderDigest = Get-Digest -Path $settlementHolderFixture.LeasePath
    $settlementHolderIdentity = Get-IdentityArguments -Fixture $settlementHolderFixture -Digest $settlementHolderDigest
    $settlementHolderArguments = @('-Operation', 'Settle', '-LeasePath', $settlementHolderFixture.LeasePath) + $settlementHolderIdentity + @(
        '-ArchiveRoot', $settlementHolderFixture.ArchiveRoot,
        '-ArchivePath', $settlementHolderFixture.ArchivePath,
        '-ReceiptPath', $settlementHolderFixture.ReceiptPath,
        '-FinalPhase', 'SETTLED_TEST_HOLDER_CONFIRMATION',
        '-Disposition', 'SETTLED_TEST_HOLDER_CONFIRMATION'
    )
    Assert-ToolFails -ToolArguments $settlementHolderArguments -ExpectedToken 'WRITER_LEASE_SETTLE_HOLDER_CONFIRMATION_REQUIRED'
    [void](Invoke-ToolJson -ToolArguments ($settlementHolderArguments + @('-ExpectedHolder', 'test-confirmed-holder')))
    Assert-True -Condition (-not (Test-Path -LiteralPath $settlementHolderFixture.LeasePath -PathType Leaf)) -Message 'holder-confirmed settlement left source lease'
    'WRITER_LEASE_TEST_SETTLEMENT_HOLDER_CONFIRMATION=PASS'

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

    "WRITER_LEASE_TEST_ARTIFACT_PREFIX=$testPrefix"
    'WRITER_LEASE_FOCUSED_TESTS=PASS'
} catch {
    "WRITER_LEASE_TEST_ARTIFACT_PREFIX=$testPrefix"
    throw
} finally {
    foreach ($fixture in $script:fixtureLedger) {
        Settle-FixtureForTestCleanup -Fixture $fixture
    }
    $residualActiveV1Fixtures = @(
        foreach ($taskRoot in Get-ChildItem -LiteralPath $testRoot -Directory -Force -ErrorAction Stop) {
            if (-not $taskRoot.Name.StartsWith($testPrefix, [StringComparison]::Ordinal)) {
                continue
            }
            $leasePath = Join-Path $taskRoot.FullName 'writer-lease.json'
            if (-not (Test-Path -LiteralPath $leasePath -PathType Leaf)) {
                continue
            }
            $lease = Get-Content -LiteralPath $leasePath -Raw | ConvertFrom-Json -ErrorAction Stop
            if ($lease.schema -eq 'tabbeacon-writer-lease.v1' -and $lease.state -like 'ACTIVE*') {
                $leasePath
            }
        }
    )
    Assert-True -Condition ($residualActiveV1Fixtures.Count -eq 0) -Message 'focused test left active v1 fixture leases'
    $residualPreparedMarkers = @(
        foreach ($taskRoot in Get-ChildItem -LiteralPath $testRoot -Directory -Force -ErrorAction Stop) {
            if (-not $taskRoot.Name.StartsWith($testPrefix, [StringComparison]::Ordinal)) {
                continue
            }
            $markerPath = Join-Path $taskRoot.FullName 'writer-lease.transaction.v1.txt'
            if ((Test-Path -LiteralPath $markerPath -PathType Leaf) -and (Get-Content -LiteralPath $markerPath -Raw).IndexOf('TRANSACTION=PREPARED', [StringComparison]::Ordinal) -ge 0) {
                $markerPath
            }
        }
    )
    Assert-True -Condition ($residualPreparedMarkers.Count -eq 0) -Message 'focused test left prepared transaction markers'
    'WRITER_LEASE_TEST_ACTIVE_FIXTURE_LEAKS=0'
    'WRITER_LEASE_TEST_PREPARED_MARKER_LEAKS=0'
}
