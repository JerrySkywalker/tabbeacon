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
    [string]$RunId = 'TB-G46-REAL-WT-SMOKE',
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
$windowTitle = "TabBeacon-G46-$token"
$sentinelPath = Join-Path $resolvedEvidence "sentinel-$token.txt"
$processReceiptPath = Join-Path $resolvedEvidence "child-$token.pid"
$receiptPath = Join-Path $resolvedEvidence 'g46-real-wt-smoke.txt'

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
    '-RunId', $RunId
)

& $wtCommand.Source @arguments
$launchExitCode = $LASTEXITCODE
if ($launchExitCode -ne 0) {
    throw "wt.exe launch failed with exit code $launchExitCode"
}

$deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
$windowObserved = $false
$windowHandle = [IntPtr]::Zero
$childProcessId = $null
$sentinelObserved = $false
$childCompleted = $false
$windowCompleted = $false

while ([DateTimeOffset]::UtcNow -lt $deadline) {
    $windows = [TabBeaconSmokeWindows]::FindExactTitle($windowTitle)
    if ($windows.Count -eq 1) {
        $windowObserved = $true
        $windowHandle = $windows[0].Handle
    }
    if ($null -eq $childProcessId -and (Test-Path -LiteralPath $processReceiptPath)) {
        $childProcessId = [int](Get-Content -LiteralPath $processReceiptPath -Raw)
    }
    $sentinelObserved = Test-Path -LiteralPath $sentinelPath
    if ($null -ne $childProcessId) {
        $childCompleted = $null -eq (Get-Process -Id $childProcessId -ErrorAction SilentlyContinue)
    }
    $windowCompleted = $windowObserved -and
        ([TabBeaconSmokeWindows]::FindExactTitle($windowTitle).Count -eq 0)
    if ($sentinelObserved -and $childCompleted -and $windowCompleted) {
        break
    }
    Start-Sleep -Milliseconds 100
}

if (-not $windowCompleted -and $windowHandle -ne [IntPtr]::Zero) {
    # WM_CLOSE is sent only to the exact disposable title-bound window.
    [void][TabBeaconSmokeWindows]::PostMessage(
        $windowHandle,
        0x0010,
        [IntPtr]::Zero,
        [IntPtr]::Zero
    )
}

$fixtureExitCode = $null
$shellUsable = $false
if ($sentinelObserved) {
    $sentinel = Get-Content -LiteralPath $sentinelPath
    $fixtureLine = $sentinel | Where-Object { $_ -like 'FIXTURE_EXIT_CODE=*' } |
        Select-Object -First 1
    $fixtureExitCode = [int]($fixtureLine -replace '^FIXTURE_EXIT_CODE=', '')
    $shellUsable = $sentinel -contains 'SHELL_USABLE_AFTER_TUI=true'
}

$passed = $windowObserved -and $windowCompleted -and $childCompleted -and
    $sentinelObserved -and $shellUsable -and $fixtureExitCode -eq 0
$receipt = @(
    "RUN_ID=$RunId"
    "EXPECTED_HEAD=$ExpectedHead"
    "CHECKED_OUT_HEAD=$checkedOutHead"
    'SMOKE_METHOD=feature-gated deterministic app events in disposable real wt.exe'
    "WINDOW_OBSERVED=$($windowObserved.ToString().ToLowerInvariant())"
    "WINDOW_COMPLETED=$($windowCompleted.ToString().ToLowerInvariant())"
    "CHILD_PROCESS_COMPLETED=$($childCompleted.ToString().ToLowerInvariant())"
    "SENTINEL_OBSERVED=$($sentinelObserved.ToString().ToLowerInvariant())"
    "FIXTURE_EXIT_CODE=$fixtureExitCode"
    "WINDOWS_TERMINAL_TUI_SMOKE=$(if ($passed) { 'PASS' } else { 'FAIL' })"
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
