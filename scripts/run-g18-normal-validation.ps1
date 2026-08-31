[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string]$ExpectedHead,

    [string]$EvidenceRoot = 'V:\build\tabbeacon\TB-G18-FAST-LANE-CLOSEOUT-001',

    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9-]{0,63}$')]
    [string]$RunId = "TB-G18-NORMAL-$([Guid]::NewGuid().ToString('N').Substring(0, 12))"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-CompactReceipt {
    param([System.Collections.IDictionary]$Receipt, [string]$RunRoot)

    $receiptPath = Join-Path $RunRoot 'g18-normal-validation-receipt.json'
    $Receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $receiptPath -Encoding utf8
    $Receipt.GetEnumerator() | ForEach-Object { '{0}={1}' -f $_.Key, $_.Value }
}

function Stop-Validation {
    param([string]$Code)

    throw [System.InvalidOperationException]::new($Code)
}

function Get-SafeFailureCode {
    param([System.Management.Automation.ErrorRecord]$ErrorRecord)

    $message = $ErrorRecord.Exception.Message
    if ($message -match '^(BLOCKED|FAIL)_[A-Z0-9_]+$') { return $message }
    return 'FAIL_NORMAL_VALIDATION'
}

function ConvertTo-PowerShellLiteral {
    param([string]$Value)

    return "'$($Value.Replace("'", "''"))'"
}

function Get-ExactTabTitleMatches {
    param([string[]]$ExpectedTitles)

    $condition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::TabItem
    )
    $tabs = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        $condition
    )
    $matches = @(
        foreach ($tab in $tabs) {
            try {
                $name = $tab.Current.Name
                if ($ExpectedTitles -contains $name) { $name }
            }
            catch {
                # UIA elements may expire while the owned child exits; omit only
                # that unreadable element and keep the observation bounded.
            }
        }
    )
    return $matches
}

function Get-ExactTabTitle {
    param([string[]]$ExpectedTitles)

    $matches = @(Get-ExactTabTitleMatches -ExpectedTitles $ExpectedTitles)
    if ($matches.Count -eq 1) { return $matches[0] }
    return $null
}

function Wait-ForExactTabTitle {
    param([string[]]$ExpectedTitles, [int]$TimeoutMilliseconds)

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    do {
        $match = Get-ExactTabTitle -ExpectedTitles $ExpectedTitles
        if ($null -ne $match) { return $match }
        Start-Sleep -Milliseconds 75
    } while ([DateTime]::UtcNow -lt $deadline)
    return $null
}

function Observe-WorkingFrames {
    param([string[]]$ExpectedTitles)

    $first = Wait-ForExactTabTitle -ExpectedTitles $ExpectedTitles -TimeoutMilliseconds 8000
    if ($null -eq $first) { return @() }

    $frames = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    [void]$frames.Add($first)
    $deadline = [DateTime]::UtcNow.AddMilliseconds(1000)
    while ([DateTime]::UtcNow -lt $deadline -and $frames.Count -lt 3) {
        $match = Get-ExactTabTitle -ExpectedTitles $ExpectedTitles
        if ($null -ne $match) { [void]$frames.Add($match) }
        Start-Sleep -Milliseconds 73
    }
    return @($frames)
}

function Test-StableTabTitle {
    param([string]$ExpectedTitle, [int]$TimeoutMilliseconds = 10000)

    $first = Wait-ForExactTabTitle -ExpectedTitles @($ExpectedTitle) -TimeoutMilliseconds $TimeoutMilliseconds
    if ($null -eq $first) { return $false }
    Start-Sleep -Milliseconds 300
    $second = Get-ExactTabTitle -ExpectedTitles @($ExpectedTitle)
    Start-Sleep -Milliseconds 300
    $third = Get-ExactTabTitle -ExpectedTitles @($ExpectedTitle)
    return $second -eq $ExpectedTitle -and $third -eq $ExpectedTitle
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $EvidenceRoot $RunId
New-Item -ItemType Directory -Force -Path $EvidenceRoot | Out-Null
if (Test-Path -LiteralPath $runRoot) {
    throw 'normal_validation_run_root_already_exists'
}
New-Item -ItemType Directory -Path $runRoot | Out-Null

$receipt = [ordered]@{
    RUN_ID = $RunId
    EXPECTED_HEAD = $ExpectedHead
    ACTUAL_ELEVATED_TOKEN = $false
    NORMAL_POWERSHELL = 'UNPROVEN'
    WORKSPACE_KIND = 'git'
    WORKING_FRAMES = 0
    WORKSPACE_ALIAS_STABLE = $false
    RESULT_READY = 'UNPROVEN'
    PERMISSION_REQUEST = 'UNPROVEN'
    TITLE_AUTHORITY = 'UNPROVEN'
    CLEANUP = 'UNPROVEN'
    TEMP_WT_CLEANUP = 'UNPROVEN'
    TEMP_WINDOWS_CREATED = 0
    TEMP_WINDOWS_CLOSED = 0
    OWNED_TEMP_WT_REMAINING = 0
    OWNER_WINDOWS_CLOSED = 0
    BROAD_WINDOW_KILL_USED = $false
    PIXEL_CAPTURE = 'REUSED_BLOCKER'
    OWNER_CONFIG_MUTATED = $false
    OWNER_SHELL_PROFILE_MUTATED = $false
    OWNER_WINDOWS_TERMINAL_SETTINGS_MUTATED = $false
}
$scratchRoot = $null
$ownershipPath = $null
$temporaryWindowCreated = $false
$exitCode = 3

Push-Location $repoRoot
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Stop-Validation 'BLOCKED_NOT_NORMAL_POWERSHELL'
    }

    $gitCommand = Get-Command git -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $cargoCommand = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $rustupCommand = Get-Command rustup -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $wtCommand = Get-Command wt.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $toolchainLine = Select-String -LiteralPath (Join-Path $repoRoot 'rust-toolchain.toml') `
        -Pattern '^\s*channel\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($null -eq $gitCommand -or $null -eq $cargoCommand -or $null -eq $rustupCommand `
        -or $null -eq $wtCommand -or $null -eq $toolchainLine) {
        Stop-Validation 'BLOCKED_REQUIRED_TOOL_MISSING'
    }

    $actualHead = (& $gitCommand.Source rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $actualHead -ne $ExpectedHead) {
        Stop-Validation 'BLOCKED_HEAD_MISMATCH'
    }
    $candidateStatus = & $gitCommand.Source status --porcelain
    if ($LASTEXITCODE -ne 0 -or -not [string]::IsNullOrWhiteSpace(($candidateStatus -join "`n"))) {
        Stop-Validation 'BLOCKED_DIRTY_CANDIDATE'
    }

    $pinnedToolchain = $toolchainLine.Matches[0].Groups[1].Value
    $cargoBin = Split-Path -Parent $cargoCommand.Source
    if ((Split-Path -Leaf $cargoBin) -eq 'bin') {
        $derivedCargoHome = Split-Path -Parent $cargoBin
        $derivedRustupHome = Join-Path (Split-Path -Parent $derivedCargoHome) '.rustup'
        if ((Test-Path -LiteralPath $derivedCargoHome -PathType Container) -and `
            (Test-Path -LiteralPath $derivedRustupHome -PathType Container)) {
            $env:CARGO_HOME = $derivedCargoHome
            $env:RUSTUP_HOME = $derivedRustupHome
        }
    }
    $env:RUSTUP_TOOLCHAIN = $pinnedToolchain
    $availableToolchains = & $rustupCommand.Source toolchain list 2>&1
    $toolchainPattern = '^{0}(?:-|\s)' -f [regex]::Escape($pinnedToolchain)
    if ($LASTEXITCODE -ne 0 -or -not (@($availableToolchains) | Where-Object {
        $_.ToString().TrimStart() -match $toolchainPattern
    })) {
        Stop-Validation 'BLOCKED_PINNED_TOOLCHAIN_UNAVAILABLE'
    }

    Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
    if ($null -eq [System.Windows.Automation.AutomationElement]::RootElement) {
        Stop-Validation 'BLOCKED_UIA_UNAVAILABLE'
    }

    & $cargoCommand.Source build --locked --bin tabbeacon
    if ($LASTEXITCODE -ne 0) {
        Stop-Validation 'BLOCKED_FIXTURE_BUILD'
    }
    $tabbeaconExecutable = Join-Path $repoRoot 'target\debug\tabbeacon.exe'
    if (-not (Test-Path -LiteralPath $tabbeaconExecutable -PathType Leaf)) {
        Stop-Validation 'BLOCKED_FIXTURE_BUILD'
    }
    & $tabbeaconExecutable '__temporary-wt-recover-v1' $EvidenceRoot | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Stop-Validation 'BLOCKED_STALE_TEMP_WT_RECOVERY'
    }

    $scratchRoot = Join-Path $runRoot 'scratch'
    $workspace = Join-Path $scratchRoot 'workspace'
    $localAppData = Join-Path $scratchRoot 'local-appdata'
    New-Item -ItemType Directory -Force -Path $workspace, $localAppData | Out-Null
    & $gitCommand.Source -C $workspace init --quiet
    if ($LASTEXITCODE -ne 0) { Stop-Validation 'BLOCKED_WORKSPACE_FIXTURE' }
    $identityToken = [Guid]::NewGuid().ToString('N')
    # With an empty isolated registry, the Git remote display name below maps
    # deterministically to this twelve-character production alias.
    $aliasToken = $identityToken.Substring(0, 11)
    $remoteName = 'g-' + ($aliasToken.ToCharArray() -join '-')
    $workspaceAlias = 'G' + $aliasToken.ToUpperInvariant()
    & $gitCommand.Source -C $workspace remote add origin "https://example.invalid/$remoteName.git"
    if ($LASTEXITCODE -ne 0) { Stop-Validation 'BLOCKED_WORKSPACE_FIXTURE' }

    $sessionId = "g18-$($RunId.ToLowerInvariant())"
    $turnId = "turn-$($identityToken.Substring(0, 12))"
    $env:LOCALAPPDATA = $localAppData
    $lifecycleRunId = "TBWT-$($identityToken.Substring(0, 32))"
    $anchorTitle = "TB-WT-ANCHOR-$lifecycleRunId"
    $windowName = "tabbeacon-$lifecycleRunId"
    $childTemplate = @'
$ErrorActionPreference = 'Stop'
$env:LOCALAPPDATA = __LOCALAPPDATA__
Set-Location -LiteralPath __WORKSPACE__
function Invoke-G18Hook {
    param([string]$EventName)
    $payload = [ordered]@{
        hook_event_name = $EventName
        session_id = __SESSION_ID__
        turn_id = __TURN_ID__
        cwd = __WORKSPACE__
    }
    $payload | ConvertTo-Json -Compress | & __TABBEACON__ hook codex
    if ($LASTEXITCODE -ne 0) { exit 89 }
}
Invoke-G18Hook 'UserPromptSubmit'
Start-Sleep -Seconds 12
Invoke-G18Hook 'Stop'
Start-Sleep -Seconds 6
Invoke-G18Hook 'PermissionRequest'
Start-Sleep -Seconds 6
Invoke-G18Hook 'SessionEnd'
'@
    $childCommand = $childTemplate.
        Replace('__LOCALAPPDATA__', (ConvertTo-PowerShellLiteral -Value $localAppData)).
        Replace('__WORKSPACE__', (ConvertTo-PowerShellLiteral -Value $workspace)).
        Replace('__SESSION_ID__', (ConvertTo-PowerShellLiteral -Value $sessionId)).
        Replace('__TURN_ID__', (ConvertTo-PowerShellLiteral -Value $turnId)).
        Replace('__TABBEACON__', (ConvertTo-PowerShellLiteral -Value $tabbeaconExecutable))
    $encodedChildCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($childCommand))
    $wtArguments = @(
        '-w', $windowName,
        'new-tab', '--title', $anchorTitle, '--suppressApplicationTitle',
        'powershell.exe', '-NoLogo', '-NoProfile', '-NonInteractive',
        '-Command', 'Start-Sleep -Milliseconds 60000',
        ';',
        'new-tab', 'powershell.exe', '-NoLogo', '-NoProfile', '-NonInteractive',
        '-EncodedCommand', $encodedChildCommand
    )
    & $wtCommand.Source @wtArguments
    if ($LASTEXITCODE -ne 0) { Stop-Validation 'BLOCKED_WINDOWS_TERMINAL_LAUNCH' }
    $temporaryWindowCreated = $true
    $receipt.TEMP_WINDOWS_CREATED = 1
    $registrationOutput = & $tabbeaconExecutable '__temporary-wt-register-v1' `
        $runRoot $lifecycleRunId $anchorTitle $windowName $PID 2>&1
    if ($LASTEXITCODE -ne 0) { Stop-Validation 'BLOCKED_TEMP_WT_REGISTRATION' }
    $ownershipPath = @($registrationOutput | ForEach-Object { $_.ToString().Trim() } |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) | Select-Object -Last 1
    if ([string]::IsNullOrWhiteSpace($ownershipPath) -or `
        -not (Test-Path -LiteralPath $ownershipPath -PathType Leaf)) {
        Stop-Validation 'BLOCKED_TEMP_WT_REGISTRATION'
    }

    $workingTitles = @(0x280B, 0x2819, 0x2839, 0x2838, 0x283C, 0x2834, 0x2826, 0x2827, 0x2807, 0x280F) |
        ForEach-Object { "$([char]$_) $workspaceAlias" }
    $workingFrames = @(Observe-WorkingFrames -ExpectedTitles $workingTitles)
    $receipt.WORKING_FRAMES = $workingFrames.Count
    $receipt.WORKSPACE_ALIAS_STABLE = $workingFrames.Count -ge 3 -and `
        @($workingFrames | Where-Object { $_ -notlike "* $workspaceAlias" }).Count -eq 0

    $approvalTitle = "! $workspaceAlias"
    $resultTitle = "$([char]0x2713) $workspaceAlias"
    $resultPass = Test-StableTabTitle -ExpectedTitle $resultTitle -TimeoutMilliseconds 16000
    $receipt.RESULT_READY = if ($resultPass) { 'PASS' } else { 'UNPROVEN' }
    $approvalPass = Test-StableTabTitle -ExpectedTitle $approvalTitle
    $receipt.PERMISSION_REQUEST = if ($approvalPass) { 'PASS' } else { 'UNPROVEN' }

    Start-Sleep -Seconds 7
    $windowRetired = @(Get-ExactTabTitleMatches -ExpectedTitles @($workingTitles + $approvalTitle + $resultTitle)).Count -eq 0
    $statusOutput = & $tabbeaconExecutable status --json 2>&1
    $statusLine = @($statusOutput | ForEach-Object { $_.ToString() } | Where-Object {
        $_.TrimStart().StartsWith('{')
    }) | Select-Object -Last 1
    $status = if ($null -eq $statusLine) { $null } else { $statusLine | ConvertFrom-Json }
    $leaseClean = $null -ne $status -and $status.activity.active_leases -eq 0 -and `
        $status.activity.stale_leases -eq 0
    $receipt.CLEANUP = if ($windowRetired -and $leaseClean) { 'PASS' } else { 'UNPROVEN' }
    $receipt.TITLE_AUTHORITY = if ($workingFrames.Count -ge 3 -and $approvalPass -and $resultPass) {
        'healthy'
    }
    else {
        'UNPROVEN'
    }

    $allPass = $workingFrames.Count -ge 3 -and $receipt.WORKSPACE_ALIAS_STABLE -and `
        $approvalPass -and $resultPass -and $receipt.CLEANUP -eq 'PASS' -and `
        $receipt.TITLE_AUTHORITY -eq 'healthy'
    $receipt.NORMAL_POWERSHELL = if ($allPass) { 'PASS' } else { 'UNPROVEN' }
    $exitCode = if ($allPass) { 0 } else { 3 }
}
catch {
    $receipt.NORMAL_POWERSHELL = Get-SafeFailureCode -ErrorRecord $_
    $exitCode = 3
}
finally {
    if ($null -ne $ownershipPath) {
        $productDisposition = if ($exitCode -eq 0) {
            'PASS'
        }
        elseif ($receipt.NORMAL_POWERSHELL -like 'BLOCKED_*') {
            'BLOCKED'
        }
        else {
            'FAIL'
        }
        try {
            $cleanupOutput = & $tabbeaconExecutable '__temporary-wt-cleanup-v1' `
                $ownershipPath $productDisposition 2>&1
            $cleanupExitCode = $LASTEXITCODE
            $cleanupLine = @($cleanupOutput | ForEach-Object { $_.ToString() } |
                Where-Object { $_.TrimStart().StartsWith('{') }) | Select-Object -Last 1
            if ($null -eq $cleanupLine) { throw 'temporary_wt_cleanup_receipt_missing' }
            $cleanupReceipt = $cleanupLine | ConvertFrom-Json
            if ($cleanupReceipt.temporary_wt_cleanup -ne 'PASS') {
                $retryOutput = & $tabbeaconExecutable '__temporary-wt-retry-cleanup-v1' `
                    $ownershipPath $PID 2>&1
                $retryLine = @($retryOutput | ForEach-Object { $_.ToString() } |
                    Where-Object { $_.TrimStart().StartsWith('{') }) | Select-Object -Last 1
                if ($null -eq $retryLine) { throw 'temporary_wt_cleanup_retry_receipt_missing' }
                $cleanupReceipt = $retryLine | ConvertFrom-Json
            }
            $receipt.TEMP_WT_CLEANUP = $cleanupReceipt.temporary_wt_cleanup
            $receipt.TEMP_WINDOWS_CREATED = $cleanupReceipt.temporary_windows_created
            $receipt.TEMP_WINDOWS_CLOSED = $cleanupReceipt.temporary_windows_closed
            $receipt.OWNED_TEMP_WT_REMAINING = $cleanupReceipt.owned_temporary_wt_remaining
            $receipt.OWNER_WINDOWS_CLOSED = $cleanupReceipt.owner_windows_closed
            $receipt.BROAD_WINDOW_KILL_USED = $cleanupReceipt.broad_window_kill_used
            if ($cleanupExitCode -ne 0 -or $receipt.TEMP_WT_CLEANUP -ne 'PASS') {
                $exitCode = 3
            }
        }
        catch {
            $receipt.TEMP_WT_CLEANUP = 'FAIL'
            $exitCode = 3
        }
    }
    elseif ($temporaryWindowCreated) {
        # Registration performs a fresh exact-anchor emergency close on error,
        # but without an immutable record cleanup cannot be claimed as proved.
        $receipt.TEMP_WT_CLEANUP = 'FAIL'
        $receipt.OWNED_TEMP_WT_REMAINING = 1
        $exitCode = 3
    }
    if ($null -ne $scratchRoot -and (Test-Path -LiteralPath $scratchRoot)) {
        try {
            $resolvedRunRoot = [IO.Path]::GetFullPath($runRoot).TrimEnd('\', '/')
            $resolvedScratch = [IO.Path]::GetFullPath($scratchRoot)
            if (-not $resolvedScratch.StartsWith("$resolvedRunRoot$([IO.Path]::DirectorySeparatorChar)", [StringComparison]::OrdinalIgnoreCase)) {
                throw 'unsafe_fixture_cleanup_target'
            }
            Remove-Item -LiteralPath $resolvedScratch -Recurse -Force
        }
        catch {
            $receipt.CLEANUP = 'FAIL'
            $exitCode = 3
        }
    }
    Pop-Location
    Write-CompactReceipt -Receipt $receipt -RunRoot $runRoot
}

exit $exitCode
