[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string]$ExpectedHead,
    [Parameter(Mandatory = $true)]
    [string]$EvidenceDirectory,
    [string]$RunId = 'TB-G55-REAL-WT-SMOKE',
    [ValidateRange(10, 120)]
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'
$resolvedBinary = (Resolve-Path -LiteralPath $BinaryPath).Path
$resolvedRepository = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$checkedOutHead = git -C $resolvedRepository rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $checkedOutHead -ne $ExpectedHead) {
    throw "Expected head $ExpectedHead does not match checked-out head $checkedOutHead"
}
$worktreeState = git -C $resolvedRepository status --porcelain
if ($LASTEXITCODE -ne 0 -or $worktreeState) {
    throw 'The real-terminal smoke requires a clean settled candidate worktree'
}
$childScript = Join-Path $PSScriptRoot 'invoke-windows-terminal-tui-smoke-child.ps1'
$resolvedChildScript = (Resolve-Path -LiteralPath $childScript).Path
$wtCommand = Get-Command wt.exe -CommandType Application -ErrorAction Stop |
    Select-Object -First 1

New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$resolvedEvidence = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$token = [Guid]::NewGuid().ToString('N')
$windowTitle = "TabBeacon-G55-$token"
$sentinelPath = Join-Path $resolvedEvidence "sentinel-$token.txt"
$processReceiptPath = Join-Path $resolvedEvidence "child-$token.pid"
$fixtureResultPath = Join-Path $resolvedEvidence "fixture-$token.txt"
$receiptPath = Join-Path $resolvedEvidence 'g51-real-wt-smoke.txt'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class TabBeaconSmokeWindows {
    public sealed class Record {
        public IntPtr Handle { get; set; }
        public uint ProcessId { get; set; }
        public string Title { get; set; }
    }

    private delegate bool EnumWindowsProc(IntPtr handle, IntPtr state);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr state);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr handle, StringBuilder text, int capacity);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr handle, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr handle, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool IsWindow(IntPtr handle);

    public static Record GetRecord(IntPtr handle) {
        if (!IsWindow(handle)) {
            return null;
        }
        var title = new StringBuilder(512);
        GetWindowText(handle, title, title.Capacity);
        uint processId;
        GetWindowThreadProcessId(handle, out processId);
        return new Record { Handle = handle, ProcessId = processId, Title = title.ToString() };
    }

    public static Record[] FindExactTitle(string expectedTitle) {
        var records = new List<Record>();
        EnumWindows(delegate (IntPtr handle, IntPtr state) {
            var title = new StringBuilder(512);
            GetWindowText(handle, title, title.Capacity);
            if (String.Equals(title.ToString(), expectedTitle, StringComparison.Ordinal)) {
                uint processId;
                GetWindowThreadProcessId(handle, out processId);
                records.Add(new Record { Handle = handle, ProcessId = processId, Title = title.ToString() });
            }
            return true;
        }, IntPtr.Zero);
        return records.ToArray();
    }
}
'@

if ([TabBeaconSmokeWindows]::FindExactTitle($windowTitle).Count -ne 0) {
    throw 'The unique disposable Windows Terminal title already exists before launch'
}

function Test-ProcessAncestor {
    param(
        [int]$ProcessId,
        [int]$ExpectedAncestorId
    )
    $currentId = $ProcessId
    for ($depth = 0; $depth -lt 8 -and $currentId -gt 0; $depth++) {
        $record = Get-CimInstance Win32_Process -Filter "ProcessId = $currentId" -ErrorAction SilentlyContinue
        if ($null -eq $record) {
            return $false
        }
        $parentId = [int]$record.ParentProcessId
        if ($parentId -eq $ExpectedAncestorId) {
            return $true
        }
        $currentId = $parentId
    }
    return $false
}

function Get-ProcessStartTimeUtcTicks {
    param(
        [int]$ProcessId
    )
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $null
    }
    try {
        return $process.StartTime.ToUniversalTime().Ticks
    } catch {
        return $null
    }
}

function Test-ProcessIdentity {
    param(
        [int]$ProcessId,
        [long]$ExpectedStartTimeUtcTicks
    )
    $actualStartTimeUtcTicks = Get-ProcessStartTimeUtcTicks -ProcessId $ProcessId
    return $null -ne $actualStartTimeUtcTicks -and
        $actualStartTimeUtcTicks -eq $ExpectedStartTimeUtcTicks
}

function Add-OwnedProcessTreeSnapshot {
    param(
        [int]$RootProcessId,
        [long]$RootStartTimeUtcTicks,
        [hashtable]$TrackedProcesses
    )
    if (-not (Test-ProcessIdentity -ProcessId $RootProcessId -ExpectedStartTimeUtcTicks $RootStartTimeUtcTicks)) {
        return $false
    }
    $pending = [System.Collections.Generic.Queue[object]]::new()
    $pending.Enqueue([pscustomobject]@{
        ProcessId = $RootProcessId
        StartTimeUtcTicks = $RootStartTimeUtcTicks
    })
    while ($pending.Count -gt 0) {
        $current = $pending.Dequeue()
        if (-not (Test-ProcessIdentity -ProcessId $current.ProcessId -ExpectedStartTimeUtcTicks $current.StartTimeUtcTicks)) {
            continue
        }
        $TrackedProcesses[[string]$current.ProcessId] = [long]$current.StartTimeUtcTicks
        $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($current.ProcessId)" -ErrorAction SilentlyContinue)
        foreach ($child in $children) {
            $childProcessId = [int]$child.ProcessId
            if ($TrackedProcesses.ContainsKey([string]$childProcessId)) {
                continue
            }
            $childStartTimeUtcTicks = Get-ProcessStartTimeUtcTicks -ProcessId $childProcessId
            if ($null -ne $childStartTimeUtcTicks) {
                $pending.Enqueue([pscustomobject]@{
                    ProcessId = $childProcessId
                    StartTimeUtcTicks = $childStartTimeUtcTicks
                })
            }
        }
    }
    return $true
}

function Test-TrackedProcessTreeCompleted {
    param(
        [hashtable]$TrackedProcesses
    )
    foreach ($processId in $TrackedProcesses.Keys) {
        if (Test-ProcessIdentity -ProcessId ([int]$processId) -ExpectedStartTimeUtcTicks ([long]$TrackedProcesses[$processId])) {
            return $false
        }
    }
    return $true
}

function Get-LiveTrackedProcessIds {
    param(
        [hashtable]$TrackedProcesses
    )
    foreach ($processId in $TrackedProcesses.Keys) {
        if (Test-ProcessIdentity -ProcessId ([int]$processId) -ExpectedStartTimeUtcTicks ([long]$TrackedProcesses[$processId])) {
            [int]$processId
        }
    }
}

function Wait-ProcessExit {
    param(
        [int]$ProcessId,
        [DateTimeOffset]$Deadline
    )
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        if ($null -eq (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)) {
            return $true
        }
        Start-Sleep -Milliseconds 100
    }
    return $null -eq (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
}

function Stop-OwnedProcessTree {
    param(
        [int]$ProcessId,
        [long]$ExpectedStartTimeUtcTicks,
        [int]$TimeoutMilliseconds = 5000
    )
    if (-not (Test-ProcessIdentity -ProcessId $ProcessId -ExpectedStartTimeUtcTicks $ExpectedStartTimeUtcTicks)) {
        return $false
    }
    $taskkillPath = Join-Path $env:SystemRoot 'System32\taskkill.exe'
    if (-not (Test-Path -LiteralPath $taskkillPath)) {
        $taskkillPath = 'taskkill.exe'
    }
    # taskkill itself is isolated so a platform failure cannot consume the
    # harness deadline. The target PID is admitted from the owned terminal
    # child lineage before this function is called.
    $taskkill = Start-Process -FilePath $taskkillPath -ArgumentList @(
        '/PID', [string]$ProcessId, '/T', '/F'
    ) -PassThru -WindowStyle Hidden
    $deadline = [DateTimeOffset]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ($taskkill.HasExited) {
            return $taskkill.ExitCode -eq 0
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $taskkill.HasExited) {
        Stop-Process -Id $taskkill.Id -Force -ErrorAction SilentlyContinue
    }
    return $false
}

$arguments = @(
    '-w', 'new',
    'new-tab',
    '--title', $windowTitle,
    '--suppressApplicationTitle',
    'pwsh.exe',
    '-NoLogo',
    '-NoProfile',
    '-File', $resolvedChildScript,
    '-BinaryPath', $resolvedBinary,
    '-SentinelPath', $sentinelPath,
    '-ProcessReceiptPath', $processReceiptPath,
    '-FixtureResultPath', $fixtureResultPath,
    '-RunId', $RunId
)

$deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
$wtLauncher = Start-Process -FilePath $wtCommand.Source -ArgumentList $arguments -PassThru
$launchExitCode = $null
$windowObserved = $false
$windowHandle = [IntPtr]::Zero
$windowProcessId = $null
$windowProcessStartTimeUtcTicks = $null
$windowOwnerBound = $false
$windowChildLineageBound = $false
$childProcessId = $null
$childProcessStartTimeUtcTicks = $null
$ownedProcessTree = @{}
$sentinelObserved = $false
$childCompleted = $false
$childTreeCompleted = $false
$windowCompleted = $false
$ownedTreeTerminationAttempted = $false
$ownedTreeTerminationSucceeded = $false
$launcherTerminationAttempted = $false
$launcherCompleted = $false

while ([DateTimeOffset]::UtcNow -lt $deadline) {
    $windows = [TabBeaconSmokeWindows]::FindExactTitle($windowTitle)
    if (-not $windowObserved -and $windows.Count -eq 1) {
        $candidateOwner = Get-Process -Id $windows[0].ProcessId -ErrorAction SilentlyContinue
        $candidateOwnerStartTimeUtcTicks = Get-ProcessStartTimeUtcTicks -ProcessId $windows[0].ProcessId
        if ($null -ne $candidateOwner -and $candidateOwner.ProcessName -eq 'WindowsTerminal' -and
            $null -ne $candidateOwnerStartTimeUtcTicks) {
            $windowObserved = $true
            $windowOwnerBound = $true
            $windowHandle = $windows[0].Handle
            $windowProcessId = $windows[0].ProcessId
            $windowProcessStartTimeUtcTicks = $candidateOwnerStartTimeUtcTicks
        }
    }
    if ($null -eq $childProcessId -and (Test-Path -LiteralPath $processReceiptPath)) {
        $candidateChildProcessId = [int](Get-Content -LiteralPath $processReceiptPath -Raw)
        $candidateChildStartTimeUtcTicks = Get-ProcessStartTimeUtcTicks -ProcessId $candidateChildProcessId
        if ($null -ne $candidateChildStartTimeUtcTicks) {
            $childProcessId = $candidateChildProcessId
            $childProcessStartTimeUtcTicks = $candidateChildStartTimeUtcTicks
        }
    }
    if (-not $windowChildLineageBound -and $windowOwnerBound -and $null -ne $childProcessId -and
        $null -ne $windowProcessStartTimeUtcTicks -and $null -ne $childProcessStartTimeUtcTicks) {
        $windowChildLineageBound = Test-ProcessAncestor -ProcessId $childProcessId -ExpectedAncestorId $windowProcessId
    }
    if ($windowChildLineageBound -and $null -ne $childProcessId) {
        [void](Add-OwnedProcessTreeSnapshot -RootProcessId $childProcessId -RootStartTimeUtcTicks $childProcessStartTimeUtcTicks -TrackedProcesses $ownedProcessTree)
    }
    $sentinelObserved = Test-Path -LiteralPath $sentinelPath
    if ($null -ne $childProcessId) {
        $childCompleted = -not (Test-ProcessIdentity -ProcessId $childProcessId -ExpectedStartTimeUtcTicks $childProcessStartTimeUtcTicks)
    }
    $childTreeCompleted = $windowChildLineageBound -and (Test-TrackedProcessTreeCompleted -TrackedProcesses $ownedProcessTree)
    $boundWindow = if ($windowObserved) {
        [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    } else {
        $null
    }
    $boundWindowPresent = $null -ne $boundWindow -and
        $boundWindow.ProcessId -eq $windowProcessId -and
        (Test-ProcessIdentity -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks)
    $windowCompleted = $windowObserved -and -not $boundWindowPresent
    if ($sentinelObserved -and $childCompleted -and $childTreeCompleted -and $windowCompleted) {
        break
    }
    Start-Sleep -Milliseconds 100
}

if (-not $windowCompleted -and $windowOwnerBound) {
    $cleanupWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    $cleanupTargetExact = $null -ne $cleanupWindow -and
        $cleanupWindow.ProcessId -eq $windowProcessId -and
        $cleanupWindow.Title -eq $windowTitle
    # WM_CLOSE is sent only after the exact disposable handle, owner PID, and
    # unique title have all been revalidated immediately before cleanup.
    if ($cleanupTargetExact) {
        [void][TabBeaconSmokeWindows]::PostMessage(
            $windowHandle,
            0x0010,
            [IntPtr]::Zero,
            [IntPtr]::Zero
        )
    }
}

# A hung fixture (including its bounded adapter probe) remains contained in
# the exact owned Windows Terminal child tree. After an exact-handle close,
# wait briefly for normal shutdown, then terminate that admitted tree only if
# it remains live. No process without the verified owned lineage is targeted.
$cleanupDeadline = [DateTimeOffset]::UtcNow.AddSeconds(10)
if ($windowOwnerBound -and (-not $windowCompleted -or -not $childTreeCompleted)) {
    while ([DateTimeOffset]::UtcNow -lt $cleanupDeadline) {
        $boundWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
        $windowCompleted = $null -eq $boundWindow -or
            $boundWindow.ProcessId -ne $windowProcessId -or
            -not (Test-ProcessIdentity -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks)
        if ($null -ne $childProcessId) {
            if ($windowChildLineageBound) {
                [void](Add-OwnedProcessTreeSnapshot -RootProcessId $childProcessId -RootStartTimeUtcTicks $childProcessStartTimeUtcTicks -TrackedProcesses $ownedProcessTree)
            }
            $childCompleted = -not (Test-ProcessIdentity -ProcessId $childProcessId -ExpectedStartTimeUtcTicks $childProcessStartTimeUtcTicks)
        }
        $childTreeCompleted = $windowChildLineageBound -and (Test-TrackedProcessTreeCompleted -TrackedProcesses $ownedProcessTree)
        if ($windowCompleted -and $childTreeCompleted) {
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $childTreeCompleted -and $windowChildLineageBound -and $ownedProcessTree.Count -gt 0) {
        $ownedTreeTerminationAttempted = $true
        $ownedTreeTerminationSucceeded = $true
        foreach ($ownedProcessId in @(Get-LiveTrackedProcessIds -TrackedProcesses $ownedProcessTree)) {
            # PID/start-time revalidation occurs in Get-LiveTrackedProcessIds
            # immediately before every forced termination request.
            if (-not (Stop-OwnedProcessTree -ProcessId $ownedProcessId -ExpectedStartTimeUtcTicks ([long]$ownedProcessTree[[string]$ownedProcessId]))) {
                $ownedTreeTerminationSucceeded = $false
            }
        }
        $terminationDeadline = [DateTimeOffset]::UtcNow.AddSeconds(5)
        while ([DateTimeOffset]::UtcNow -lt $terminationDeadline) {
            if (Test-TrackedProcessTreeCompleted -TrackedProcesses $ownedProcessTree) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        $childTreeCompleted = Test-TrackedProcessTreeCompleted -TrackedProcesses $ownedProcessTree
        $childCompleted = -not (Test-ProcessIdentity -ProcessId $childProcessId -ExpectedStartTimeUtcTicks $childProcessStartTimeUtcTicks)
        $ownedTreeTerminationSucceeded = $ownedTreeTerminationSucceeded -and $childTreeCompleted
    }
    $boundWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    $windowCompleted = $null -eq $boundWindow -or $boundWindow.ProcessId -ne $windowProcessId -or
        -not (Test-ProcessIdentity -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks)
}

if (-not $wtLauncher.HasExited) {
    $launcherTerminationAttempted = $true
    Stop-Process -Id $wtLauncher.Id -Force -ErrorAction SilentlyContinue
}
$launcherCompleted = Wait-ProcessExit -ProcessId $wtLauncher.Id -Deadline ([DateTimeOffset]::UtcNow.AddSeconds(5))
$launchExitCode = if ($wtLauncher.HasExited) { $wtLauncher.ExitCode } else { $null }
$watchdogExpired = [DateTimeOffset]::UtcNow -ge $deadline

$fixtureExitCode = $null
$shellUsable = $false
if ($sentinelObserved) {
    $sentinel = Get-Content -LiteralPath $sentinelPath
    $fixtureLine = $sentinel | Where-Object { $_ -like 'FIXTURE_EXIT_CODE=*' } |
        Select-Object -First 1
    $fixtureExitCode = [int]($fixtureLine -replace '^FIXTURE_EXIT_CODE=', '')
    $shellUsable = $sentinel -contains 'SHELL_USABLE_AFTER_TUI=true'
}

$localeSwitched = $false
$interfaceReverted = $false
$interfaceApplyStaged = $false
$liveRefresh = $false
$workspaceSessions = $false
$hookInventory = $false
$hookProviderAdapter = $false
$helpOverlay = $false
if (Test-Path -LiteralPath $fixtureResultPath) {
    $fixtureResult = Get-Content -LiteralPath $fixtureResultPath
    $liveRefresh = $fixtureResult -contains 'TUI_LIVE_REFRESH=true'
    $workspaceSessions = $fixtureResult -contains 'TUI_WORKSPACE_SESSIONS=true'
    $hookInventory = $fixtureResult -contains 'TUI_HOOK_INVENTORY=true'
    $hookProviderAdapter = $fixtureResult -contains 'TUI_HOOK_PROVIDER_ADAPTER=true'
    $helpOverlay = $fixtureResult -contains 'TUI_HELP_OVERLAY=true'
    $localeSwitched = $fixtureResult -contains 'TUI_LANGUAGE_LIVE_SWITCH=true'
    $interfaceReverted = $fixtureResult -contains 'TUI_INTERFACE_REVERT=true'
    $interfaceApplyStaged = $fixtureResult -contains 'TUI_INTERFACE_STAGED_APPLY=true'
}

$cleanupBounded = -not $ownedTreeTerminationAttempted -or
    ($ownedTreeTerminationSucceeded -and $childTreeCompleted)
$passed = -not $watchdogExpired -and $launcherCompleted -and $cleanupBounded -and
    $windowObserved -and $windowOwnerBound -and $windowChildLineageBound -and
    $windowCompleted -and $childCompleted -and $childTreeCompleted -and $sentinelObserved -and $shellUsable -and
    $fixtureExitCode -eq 0 -and $liveRefresh -and $workspaceSessions -and $hookInventory -and $hookProviderAdapter -and $helpOverlay -and $localeSwitched -and
    $interfaceReverted -and $interfaceApplyStaged
$visualOperationDisposition = if ($passed) {
    'PASS'
} elseif ($watchdogExpired) {
    'UNPROVEN'
} else {
    'FAIL'
}
$receipt = @(
    "RUN_ID=$RunId"
    "EXPECTED_HEAD=$ExpectedHead"
    "CHECKED_OUT_HEAD=$checkedOutHead"
    'SMOKE_METHOD=feature-gated deterministic app events in disposable real wt.exe'
    "WINDOW_OBSERVED=$($windowObserved.ToString().ToLowerInvariant())"
    "WINDOW_OWNER_BOUND=$($windowOwnerBound.ToString().ToLowerInvariant())"
    "WINDOW_CHILD_LINEAGE_BOUND=$($windowChildLineageBound.ToString().ToLowerInvariant())"
    "WINDOW_COMPLETED=$($windowCompleted.ToString().ToLowerInvariant())"
    "CHILD_PROCESS_COMPLETED=$($childCompleted.ToString().ToLowerInvariant())"
    "OWNED_CHILD_TREE_COMPLETED=$($childTreeCompleted.ToString().ToLowerInvariant())"
    "WT_LAUNCHER_PROCESS_ID=$($wtLauncher.Id)"
    "WT_LAUNCHER_EXIT_CODE=$launchExitCode"
    "WT_LAUNCHER_COMPLETED=$($launcherCompleted.ToString().ToLowerInvariant())"
    "WT_LAUNCHER_TERMINATION_ATTEMPTED=$($launcherTerminationAttempted.ToString().ToLowerInvariant())"
    "OWNED_CHILD_TREE_TERMINATION_ATTEMPTED=$($ownedTreeTerminationAttempted.ToString().ToLowerInvariant())"
    "OWNED_CHILD_TREE_TERMINATION_SUCCEEDED=$($ownedTreeTerminationSucceeded.ToString().ToLowerInvariant())"
    "WATCHDOG_EXPIRED=$($watchdogExpired.ToString().ToLowerInvariant())"
    "VISUAL_OPERATION_DISPOSITION=$visualOperationDisposition"
    "SENTINEL_OBSERVED=$($sentinelObserved.ToString().ToLowerInvariant())"
    "FIXTURE_EXIT_CODE=$fixtureExitCode"
    "TUI_LIVE_REFRESH=$($liveRefresh.ToString().ToLowerInvariant())"
    "TUI_WORKSPACE_SESSIONS=$($workspaceSessions.ToString().ToLowerInvariant())"
    "TUI_HOOK_INVENTORY=$($hookInventory.ToString().ToLowerInvariant())"
    "TUI_HOOK_PROVIDER_ADAPTER=$($hookProviderAdapter.ToString().ToLowerInvariant())"
    "TUI_HELP_OVERLAY=$($helpOverlay.ToString().ToLowerInvariant())"
    "TUI_LANGUAGE_LIVE_SWITCH=$($localeSwitched.ToString().ToLowerInvariant())"
    "TUI_INTERFACE_REVERT=$($interfaceReverted.ToString().ToLowerInvariant())"
    "TUI_INTERFACE_STAGED_APPLY=$($interfaceApplyStaged.ToString().ToLowerInvariant())"
    "WINDOWS_TERMINAL_TUI_SMOKE=$(if ($passed) { 'PASS' } else { 'FAIL' })"
    "WINDOWS_TERMINAL_SMOKE=$(if ($passed) { 'PASS' } else { 'FAIL' })"
    "TUI_EXIT_RESTORES_TERMINAL=$($passed.ToString().ToLowerInvariant())"
    "SHELL_USABLE_AFTER_TUI=$($shellUsable.ToString().ToLowerInvariant())"
    'OWNER_MUTATIONS=none'
)
[System.IO.File]::WriteAllLines($receiptPath, $receipt)

Remove-Item -LiteralPath $processReceiptPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $sentinelPath -Force -ErrorAction SilentlyContinue

$receipt | ForEach-Object { Write-Output $_ }
if (-not $passed) {
    exit 1
}
