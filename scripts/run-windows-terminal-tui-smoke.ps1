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

    public static IntPtr[] FindHandlesForProcess(uint expectedProcessId) {
        var handles = new List<IntPtr>();
        EnumWindows(delegate (IntPtr handle, IntPtr state) {
            uint processId;
            GetWindowThreadProcessId(handle, out processId);
            if (processId == expectedProcessId) {
                handles.Add(handle);
            }
            return true;
        }, IntPtr.Zero);
        return handles.ToArray();
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

function Get-ProcessObservation {
    param(
        [int]$ProcessId
    )
    try {
        $process = Get-Process -Id $ProcessId -ErrorAction Stop
    } catch {
        if ($_.CategoryInfo.Category -eq [System.Management.Automation.ErrorCategory]::ObjectNotFound) {
            return [pscustomobject]@{ State = 'exited'; StartTimeUtcTicks = $null }
        }
        return [pscustomobject]@{ State = 'unknown'; StartTimeUtcTicks = $null }
    }
    try {
        return [pscustomobject]@{
            State = 'live'
            StartTimeUtcTicks = $process.StartTime.ToUniversalTime().Ticks
        }
    } catch {
        # A process can exit between Get-Process and StartTime. Re-read only
        # to distinguish that ordinary completion race from an inaccessible
        # or otherwise unknown process; unknown never counts as completion.
        try {
            Get-Process -Id $ProcessId -ErrorAction Stop | Out-Null
        } catch {
            if ($_.CategoryInfo.Category -eq [System.Management.Automation.ErrorCategory]::ObjectNotFound) {
                return [pscustomobject]@{ State = 'exited'; StartTimeUtcTicks = $null }
            }
        }
        return [pscustomobject]@{ State = 'unknown'; StartTimeUtcTicks = $null }
    }
}

function Get-ProcessIdentityState {
    param(
        [int]$ProcessId,
        [long]$ExpectedStartTimeUtcTicks
    )
    $observation = Get-ProcessObservation -ProcessId $ProcessId
    if ($observation.State -ne 'live') {
        return $observation.State
    }
    if ($observation.StartTimeUtcTicks -eq $ExpectedStartTimeUtcTicks) {
        return 'live'
    }
    # A reused PID proves the admitted process has exited; it is never a
    # valid target for subsequent cleanup.
    return 'exited'
}

function Add-OwnedProcessTreeSnapshot {
    param(
        [int]$RootProcessId,
        [long]$RootStartTimeUtcTicks,
        [hashtable]$TrackedProcesses
    )
    $rootState = Get-ProcessIdentityState -ProcessId $RootProcessId -ExpectedStartTimeUtcTicks $RootStartTimeUtcTicks
    if ($rootState -ne 'live') {
        return $rootState
    }
    $pending = [System.Collections.Generic.Queue[object]]::new()
    $pending.Enqueue([pscustomobject]@{
        ProcessId = $RootProcessId
        StartTimeUtcTicks = $RootStartTimeUtcTicks
    })
    while ($pending.Count -gt 0) {
        $current = $pending.Dequeue()
        $currentState = Get-ProcessIdentityState -ProcessId $current.ProcessId -ExpectedStartTimeUtcTicks $current.StartTimeUtcTicks
        if ($currentState -eq 'unknown') {
            return 'unknown'
        }
        if ($currentState -ne 'live') {
            continue
        }
        $TrackedProcesses[[string]$current.ProcessId] = [long]$current.StartTimeUtcTicks
        try {
            $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($current.ProcessId)" -ErrorAction Stop)
        } catch {
            return 'unknown'
        }
        foreach ($child in $children) {
            $childProcessId = [int]$child.ProcessId
            if ($TrackedProcesses.ContainsKey([string]$childProcessId)) {
                continue
            }
            $childObservation = Get-ProcessObservation -ProcessId $childProcessId
            if ($childObservation.State -eq 'unknown') {
                return 'unknown'
            }
            if ($childObservation.State -eq 'live') {
                $pending.Enqueue([pscustomobject]@{
                    ProcessId = $childProcessId
                    StartTimeUtcTicks = $childObservation.StartTimeUtcTicks
                })
            }
        }
    }
    return 'live'
}

function Get-TrackedProcessTreeState {
    param(
        [hashtable]$TrackedProcesses
    )
    $unknown = $false
    foreach ($processId in $TrackedProcesses.Keys) {
        $state = Get-ProcessIdentityState -ProcessId ([int]$processId) -ExpectedStartTimeUtcTicks ([long]$TrackedProcesses[$processId])
        if ($state -eq 'live') {
            return 'live'
        }
        if ($state -eq 'unknown') {
            $unknown = $true
        }
    }
    if ($unknown) {
        return 'unknown'
    }
    return 'completed'
}

function Get-LiveTrackedProcessIds {
    param(
        [hashtable]$TrackedProcesses
    )
    foreach ($processId in $TrackedProcesses.Keys) {
        if ((Get-ProcessIdentityState -ProcessId ([int]$processId) -ExpectedStartTimeUtcTicks ([long]$TrackedProcesses[$processId])) -eq 'live') {
            [int]$processId
        }
    }
}

function Wait-ProcessIdentityExit {
    param(
        [int]$ProcessId,
        [long]$ExpectedStartTimeUtcTicks,
        [DateTimeOffset]$Deadline
    )
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        $state = Get-ProcessIdentityState -ProcessId $ProcessId -ExpectedStartTimeUtcTicks $ExpectedStartTimeUtcTicks
        if ($state -ne 'live') {
            return $state
        }
        Start-Sleep -Milliseconds 100
    }
    return (Get-ProcessIdentityState -ProcessId $ProcessId -ExpectedStartTimeUtcTicks $ExpectedStartTimeUtcTicks)
}

function Stop-OwnedProcessTree {
    param(
        [int]$ProcessId,
        [long]$ExpectedStartTimeUtcTicks,
        [int]$TimeoutMilliseconds = 5000
    )
    if ((Get-ProcessIdentityState -ProcessId $ProcessId -ExpectedStartTimeUtcTicks $ExpectedStartTimeUtcTicks) -ne 'live') {
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
$wtLauncherObservation = Get-ProcessObservation -ProcessId $wtLauncher.Id
$wtLauncherStartTimeUtcTicks = if ($wtLauncherObservation.State -eq 'live') {
    $wtLauncherObservation.StartTimeUtcTicks
} else {
    $null
}
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
$identityQueriesProven = $true
$treeEnumerationProven = $true
$windowCompleted = $false
$ownedTreeTerminationAttempted = $false
$ownedTreeTerminationSucceeded = $false
$ownedWindowTerminationAttempted = $false
$ownedWindowTerminationSucceeded = $false
$launcherTerminationAttempted = $false
$launcherCompleted = $false

while ([DateTimeOffset]::UtcNow -lt $deadline) {
    $windows = [TabBeaconSmokeWindows]::FindExactTitle($windowTitle)
    if (-not $windowObserved -and $windows.Count -eq 1) {
        $candidateOwner = Get-Process -Id $windows[0].ProcessId -ErrorAction SilentlyContinue
        $candidateOwnerObservation = Get-ProcessObservation -ProcessId $windows[0].ProcessId
        if ($candidateOwnerObservation.State -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $candidateOwnerStartTimeUtcTicks = if ($candidateOwnerObservation.State -eq 'live') {
            $candidateOwnerObservation.StartTimeUtcTicks
        } else {
            $null
        }
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
        $candidateChildObservation = Get-ProcessObservation -ProcessId $candidateChildProcessId
        if ($candidateChildObservation.State -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $candidateChildStartTimeUtcTicks = if ($candidateChildObservation.State -eq 'live') {
            $candidateChildObservation.StartTimeUtcTicks
        } else {
            $null
        }
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
        $treeSnapshotState = Add-OwnedProcessTreeSnapshot -RootProcessId $childProcessId -RootStartTimeUtcTicks $childProcessStartTimeUtcTicks -TrackedProcesses $ownedProcessTree
        if ($treeSnapshotState -eq 'unknown') {
            $treeEnumerationProven = $false
        }
    }
    $sentinelObserved = Test-Path -LiteralPath $sentinelPath
    if ($null -ne $childProcessId) {
        $childIdentityState = Get-ProcessIdentityState -ProcessId $childProcessId -ExpectedStartTimeUtcTicks $childProcessStartTimeUtcTicks
        if ($childIdentityState -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $childCompleted = $childIdentityState -eq 'exited'
    }
    $childTreeState = if ($windowChildLineageBound) {
        Get-TrackedProcessTreeState -TrackedProcesses $ownedProcessTree
    } else {
        'unknown'
    }
    if ($childTreeState -eq 'unknown') {
        $identityQueriesProven = $false
    }
    $childTreeCompleted = $childTreeState -eq 'completed'
    $boundWindow = if ($windowObserved) {
        [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    } else {
        $null
    }
    $windowIdentityState = if ($null -ne $boundWindow -and
        $boundWindow.ProcessId -eq $windowProcessId) {
        Get-ProcessIdentityState -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks
    } else {
        'exited'
    }
    if ($windowIdentityState -eq 'unknown') {
        $identityQueriesProven = $false
    }
    $boundWindowPresent = $null -ne $boundWindow -and
        $boundWindow.ProcessId -eq $windowProcessId -and $windowIdentityState -eq 'live'
    $windowCompleted = $windowObserved -and -not $boundWindowPresent -and
        $windowIdentityState -ne 'unknown'
    # Once the owned child has completed, close the exact observed fixture
    # promptly instead of spending the full watchdog interval waiting for a
    # Windows Terminal close-on-exit preference.
    if ($sentinelObserved -and $childCompleted -and $childTreeCompleted -and
        ($windowCompleted -or $windowOwnerBound)) {
        break
    }
    Start-Sleep -Milliseconds 100
}

if (-not $windowCompleted -and $windowOwnerBound) {
    $cleanupWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    $cleanupWindowIdentityState = Get-ProcessIdentityState -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks
    if ($cleanupWindowIdentityState -eq 'unknown') {
        $identityQueriesProven = $false
    }
    $cleanupTargetExact = $null -ne $cleanupWindow -and
        $cleanupWindow.ProcessId -eq $windowProcessId -and
        $cleanupWindow.Title -eq $windowTitle -and
        $cleanupWindowIdentityState -eq 'live'
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
        $windowIdentityState = if ($null -ne $boundWindow -and
            $boundWindow.ProcessId -eq $windowProcessId) {
            Get-ProcessIdentityState -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks
        } else {
            'exited'
        }
        if ($windowIdentityState -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $boundWindowPresent = $null -ne $boundWindow -and
            $boundWindow.ProcessId -eq $windowProcessId -and $windowIdentityState -eq 'live'
        $windowCompleted = -not $boundWindowPresent -and $windowIdentityState -ne 'unknown'
        if ($null -ne $childProcessId) {
            if ($windowChildLineageBound) {
                $treeSnapshotState = Add-OwnedProcessTreeSnapshot -RootProcessId $childProcessId -RootStartTimeUtcTicks $childProcessStartTimeUtcTicks -TrackedProcesses $ownedProcessTree
                if ($treeSnapshotState -eq 'unknown') {
                    $treeEnumerationProven = $false
                }
            }
            $childIdentityState = Get-ProcessIdentityState -ProcessId $childProcessId -ExpectedStartTimeUtcTicks $childProcessStartTimeUtcTicks
            if ($childIdentityState -eq 'unknown') {
                $identityQueriesProven = $false
            }
            $childCompleted = $childIdentityState -eq 'exited'
        }
        $childTreeState = if ($windowChildLineageBound) {
            Get-TrackedProcessTreeState -TrackedProcesses $ownedProcessTree
        } else {
            'unknown'
        }
        if ($childTreeState -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $childTreeCompleted = $childTreeState -eq 'completed'
        if ($windowCompleted -and $childTreeCompleted) {
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $childTreeCompleted -and $childTreeState -eq 'live' -and $windowChildLineageBound -and $ownedProcessTree.Count -gt 0) {
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
            $childTreeState = Get-TrackedProcessTreeState -TrackedProcesses $ownedProcessTree
            if ($childTreeState -eq 'unknown') {
                $identityQueriesProven = $false
                break
            }
            if ($childTreeState -eq 'completed') {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        $childTreeCompleted = $childTreeState -eq 'completed'
        $childIdentityState = Get-ProcessIdentityState -ProcessId $childProcessId -ExpectedStartTimeUtcTicks $childProcessStartTimeUtcTicks
        if ($childIdentityState -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $childCompleted = $childIdentityState -eq 'exited'
        $ownedTreeTerminationSucceeded = $ownedTreeTerminationSucceeded -and $childTreeCompleted
    }
    $boundWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    $windowIdentityState = if ($null -ne $boundWindow -and
        $boundWindow.ProcessId -eq $windowProcessId) {
        Get-ProcessIdentityState -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks
    } else {
        'exited'
    }
    if ($windowIdentityState -eq 'unknown') {
        $identityQueriesProven = $false
    }
    $boundWindowPresent = $null -ne $boundWindow -and
        $boundWindow.ProcessId -eq $windowProcessId -and $windowIdentityState -eq 'live'
    $windowCompleted = -not $boundWindowPresent -and $windowIdentityState -ne 'unknown'
}

# A WM_CLOSE that does not retire the disposable window cannot be treated as
# cleanup. A last-resort tree termination is admitted only when the exact
# owner PID/start time remains live and every top-level window of that process
# is this one run-token-bound fixture window; a shared Owner terminal is never
# an eligible target.
if (-not $windowCompleted -and $windowIdentityState -eq 'live') {
    $ownerWindowHandles = [TabBeaconSmokeWindows]::FindHandlesForProcess($windowProcessId)
    $revalidatedOwnedWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
    $ownedWindowProcessExact = $ownerWindowHandles.Count -eq 1 -and
        $ownerWindowHandles[0] -eq $windowHandle -and
        $null -ne $revalidatedOwnedWindow -and
        $revalidatedOwnedWindow.ProcessId -eq $windowProcessId -and
        $revalidatedOwnedWindow.Title -eq $windowTitle -and
        (Get-ProcessIdentityState -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks) -eq 'live'
    if ($ownedWindowProcessExact) {
        $ownedWindowTerminationAttempted = $true
        $ownedWindowTerminationSucceeded = Stop-OwnedProcessTree -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks
        $windowIdentityState = Wait-ProcessIdentityExit -ProcessId $windowProcessId -ExpectedStartTimeUtcTicks $windowProcessStartTimeUtcTicks -Deadline ([DateTimeOffset]::UtcNow.AddSeconds(5))
        if ($windowIdentityState -eq 'unknown') {
            $identityQueriesProven = $false
        }
        $boundWindow = [TabBeaconSmokeWindows]::GetRecord($windowHandle)
        $boundWindowPresent = $null -ne $boundWindow -and
            $boundWindow.ProcessId -eq $windowProcessId -and $windowIdentityState -eq 'live'
        $windowCompleted = -not $boundWindowPresent -and $windowIdentityState -ne 'unknown'
        $ownedWindowTerminationSucceeded = $ownedWindowTerminationSucceeded -and $windowCompleted
    }
}

if ($null -eq $wtLauncherStartTimeUtcTicks) {
    $launcherIdentityState = 'unknown'
    $identityQueriesProven = $false
} else {
    $launcherIdentityState = Get-ProcessIdentityState -ProcessId $wtLauncher.Id -ExpectedStartTimeUtcTicks $wtLauncherStartTimeUtcTicks
}
if ($launcherIdentityState -eq 'live') {
    $launcherTerminationAttempted = $true
    # Revalidate the exact short-lived launcher identity immediately before
    # terminating this train-owned process.
    if ((Get-ProcessIdentityState -ProcessId $wtLauncher.Id -ExpectedStartTimeUtcTicks $wtLauncherStartTimeUtcTicks) -eq 'live') {
        Stop-Process -Id $wtLauncher.Id -Force -ErrorAction SilentlyContinue
    } else {
        $identityQueriesProven = $false
    }
}
$launcherIdentityState = if ($null -eq $wtLauncherStartTimeUtcTicks) {
    'unknown'
} else {
    Wait-ProcessIdentityExit -ProcessId $wtLauncher.Id -ExpectedStartTimeUtcTicks $wtLauncherStartTimeUtcTicks -Deadline ([DateTimeOffset]::UtcNow.AddSeconds(5))
}
if ($launcherIdentityState -eq 'unknown') {
    $identityQueriesProven = $false
}
$launcherCompleted = $launcherIdentityState -eq 'exited'
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

# A transient polling uncertainty does not amend an ownership or cleanup
# decision. Only a final exact state can establish completion; no `unknown`
# observation is interpreted as `exited` or used to authorize termination.
$identityQueriesProven = $windowIdentityState -ne 'unknown' -and
    $childIdentityState -ne 'unknown' -and $childTreeState -ne 'unknown' -and
    $launcherIdentityState -ne 'unknown'
$ownedTreeTracked = $ownedProcessTree.Count -gt 0
$cleanupBounded = -not $ownedTreeTerminationAttempted -or
    ($ownedTreeTerminationSucceeded -and $childTreeCompleted)
$windowCleanupBounded = -not $ownedWindowTerminationAttempted -or
    ($ownedWindowTerminationSucceeded -and $windowCompleted)
$passed = -not $watchdogExpired -and $identityQueriesProven -and $treeEnumerationProven -and
    $ownedTreeTracked -and $launcherCompleted -and $cleanupBounded -and $windowCleanupBounded -and
    $windowObserved -and $windowOwnerBound -and $windowChildLineageBound -and
    $windowCompleted -and $childCompleted -and $childTreeCompleted -and $sentinelObserved -and $shellUsable -and
    $fixtureExitCode -eq 0 -and $liveRefresh -and $workspaceSessions -and $hookInventory -and $hookProviderAdapter -and $helpOverlay -and $localeSwitched -and
    $interfaceReverted -and $interfaceApplyStaged
$visualOperationDisposition = if ($passed) {
    'PASS'
} elseif ($watchdogExpired -or -not $identityQueriesProven -or -not $treeEnumerationProven) {
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
    "WINDOW_IDENTITY_STATE=$windowIdentityState"
    "CHILD_PROCESS_COMPLETED=$($childCompleted.ToString().ToLowerInvariant())"
    "CHILD_IDENTITY_STATE=$childIdentityState"
    "OWNED_CHILD_TREE_COMPLETED=$($childTreeCompleted.ToString().ToLowerInvariant())"
    "OWNED_CHILD_TREE_STATE=$childTreeState"
    "OWNED_CHILD_TREE_TRACKED=$($ownedTreeTracked.ToString().ToLowerInvariant())"
    "IDENTITY_QUERIES_PROVEN=$($identityQueriesProven.ToString().ToLowerInvariant())"
    "TREE_ENUMERATION_PROVEN=$($treeEnumerationProven.ToString().ToLowerInvariant())"
    "WT_LAUNCHER_PROCESS_ID=$($wtLauncher.Id)"
    "WT_LAUNCHER_EXIT_CODE=$launchExitCode"
    "WT_LAUNCHER_COMPLETED=$($launcherCompleted.ToString().ToLowerInvariant())"
    "WT_LAUNCHER_IDENTITY_STATE=$launcherIdentityState"
    "WT_LAUNCHER_TERMINATION_ATTEMPTED=$($launcherTerminationAttempted.ToString().ToLowerInvariant())"
    "OWNED_CHILD_TREE_TERMINATION_ATTEMPTED=$($ownedTreeTerminationAttempted.ToString().ToLowerInvariant())"
    "OWNED_CHILD_TREE_TERMINATION_SUCCEEDED=$($ownedTreeTerminationSucceeded.ToString().ToLowerInvariant())"
    "OWNED_WINDOW_TERMINATION_ATTEMPTED=$($ownedWindowTerminationAttempted.ToString().ToLowerInvariant())"
    "OWNED_WINDOW_TERMINATION_SUCCEEDED=$($ownedWindowTerminationSucceeded.ToString().ToLowerInvariant())"
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
