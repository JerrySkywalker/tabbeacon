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
    [string]$RunId = 'TB-G61-REAL-WT-SMOKE',
    [ValidateRange(10, 120)]
    [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'

function Read-ReceiptMap {
    param([Parameter(Mandatory = $true)][string]$Path)

    $receipt = @{}
    foreach ($line in Get-Content -LiteralPath $Path) {
        $separator = $line.IndexOf('=')
        if ($separator -lt 1) {
            throw "Malformed receipt line in $Path"
        }
        $name = $line.Substring(0, $separator)
        if ($receipt.ContainsKey($name)) {
            throw "Duplicate receipt field $name in $Path"
        }
        $receipt[$name] = $line.Substring($separator + 1)
    }
    return $receipt
}

function Test-ReceiptValue {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Receipt,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Expected
    )

    return $Receipt.ContainsKey($Name) -and $Receipt[$Name] -eq $Expected
}

function Write-AtomicLines {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Lines
    )

    if (Test-Path -LiteralPath $Path) {
        throw "Refusing to overwrite existing evidence artifact $Path"
    }
    $temporaryPath = "$Path.$PID.$([Guid]::NewGuid().ToString('N')).tmp"
    try {
        [System.IO.File]::WriteAllLines($temporaryPath, $Lines)
        [System.IO.File]::Move($temporaryPath, $Path)
    }
    finally {
        Remove-Item -LiteralPath $temporaryPath -Force -ErrorAction SilentlyContinue
    }
}

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
$completionToken = [Guid]::NewGuid().ToString('N')
$windowTitle = "TabBeacon-G61-$token"
$sentinelPath = Join-Path $resolvedEvidence "sentinel-$token.txt"
$fixtureResultPath = Join-Path $resolvedEvidence "fixture-$token.txt"
$completionReceiptPath = Join-Path $resolvedEvidence "completion-$token.txt"
$receiptPath = Join-Path $resolvedEvidence 'g61-real-wt-smoke.txt'
foreach ($path in @($sentinelPath, $fixtureResultPath, $completionReceiptPath, $receiptPath)) {
    if (Test-Path -LiteralPath $path) {
        throw "Refusing to reuse existing smoke evidence artifact $path"
    }
}

$binarySha256 = (Get-FileHash -LiteralPath $resolvedBinary -Algorithm SHA256).Hash

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class TabBeaconSmokeProcessObservation {
    private const uint Synchronize = 0x00100000;
    private const uint WaitObject0 = 0;
    private const uint WaitTimeout = 258;

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(uint desiredAccess, bool inheritHandle, uint processId);

    [DllImport("kernel32.dll")]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);

    [DllImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CloseHandle(IntPtr handle);

    public static string ObserveExit(uint processId, uint timeoutMilliseconds) {
        IntPtr handle = OpenProcess(Synchronize, false, processId);
        if (handle == IntPtr.Zero) {
            return "not-observed";
        }
        try {
            uint result = WaitForSingleObject(handle, timeoutMilliseconds);
            if (result == WaitObject0) {
                return "exited";
            }
            if (result == WaitTimeout) {
                return "still-running";
            }
            return "unknown";
        }
        finally {
            CloseHandle(handle);
        }
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
    '-FixtureResultPath', $fixtureResultPath,
    '-CompletionReceiptPath', $completionReceiptPath,
    '-CompletionToken', $completionToken,
    '-ExpectedHead', $ExpectedHead,
    '-BinarySha256', $binarySha256,
    '-RunId', $RunId
)

# `wt.exe` is only a one-shot launcher. Completion is established by the
# receipt written by the owned terminal child, not by launcher lifetime,
# Windows title discovery, CIM, or PowerShell process queries.
$deadline = [DateTimeOffset]::UtcNow.AddSeconds($TimeoutSeconds)
$wtLauncher = Start-Process -FilePath $wtCommand.Source -ArgumentList $arguments -PassThru
$wtLauncherProcessId = $wtLauncher.Id
$wtLauncher.Dispose()

$completionReceiptObserved = $false
$completionReceiptInvalid = $false
$completionReceipt = $null
while ([DateTimeOffset]::UtcNow -lt $deadline) {
    if (Test-Path -LiteralPath $completionReceiptPath) {
        $completionReceiptObserved = $true
        try {
            $completionReceipt = Read-ReceiptMap -Path $completionReceiptPath
        }
        catch {
            $completionReceiptInvalid = $true
        }
        break
    }
    Start-Sleep -Milliseconds 100
}
$watchdogExpired = -not $completionReceiptObserved

$completionSchemaValid = $false
$completionRunBound = $false
$completionHeadBound = $false
$completionBinaryBound = $false
$completionTokenBound = $false
$completionFixtureBound = $false
$completionSentinelBound = $false
$completionCompleted = $false
$childProcessId = $null
if ($null -ne $completionReceipt -and -not $completionReceiptInvalid) {
    $completionSchemaValid = Test-ReceiptValue -Receipt $completionReceipt -Name 'COMPLETION_SCHEMA' -Expected 'tabbeacon-wt-child-completion-v1'
    $completionRunBound = Test-ReceiptValue -Receipt $completionReceipt -Name 'RUN_ID' -Expected $RunId
    $completionHeadBound = Test-ReceiptValue -Receipt $completionReceipt -Name 'EXPECTED_HEAD' -Expected $ExpectedHead
    $completionBinaryBound = Test-ReceiptValue -Receipt $completionReceipt -Name 'BINARY_SHA256' -Expected $binarySha256
    $completionTokenBound = Test-ReceiptValue -Receipt $completionReceipt -Name 'COMPLETION_TOKEN' -Expected $completionToken
    $completionFixtureBound = Test-ReceiptValue -Receipt $completionReceipt -Name 'FIXTURE_RESULT_PRESENT' -Expected 'true'
    $completionSentinelBound = Test-ReceiptValue -Receipt $completionReceipt -Name 'SENTINEL_WRITTEN' -Expected 'true'
    $completionCompleted = Test-ReceiptValue -Receipt $completionReceipt -Name 'COMPLETED' -Expected 'true'
    if ($completionReceipt.ContainsKey('CHILD_PROCESS_ID')) {
        try {
            $childProcessId = [int]$completionReceipt['CHILD_PROCESS_ID']
        }
        catch {
            $completionReceiptInvalid = $true
        }
    }
}
$durableCompletionProven = $completionSchemaValid -and $completionRunBound -and
    $completionHeadBound -and $completionBinaryBound -and $completionTokenBound -and
    $completionFixtureBound -and $completionSentinelBound -and $completionCompleted -and
    $null -ne $childProcessId -and $childProcessId -gt 0

# This native wait is bounded and diagnostic-only. It is attempted only after
# the receipt has bound the PID to this child; its result never changes PASS.
$residualOwnedProcessObservation = 'unavailable'
if ($durableCompletionProven) {
    try {
        $residualOwnedProcessObservation = [TabBeaconSmokeProcessObservation]::ObserveExit(
            [uint32]$childProcessId,
            750
        )
    }
    catch {
        $residualOwnedProcessObservation = 'unknown'
    }
}

$fixtureExitCode = $null
$shellUsable = $false
$sentinelObserved = Test-Path -LiteralPath $sentinelPath
if ($sentinelObserved) {
    try {
        $sentinel = Read-ReceiptMap -Path $sentinelPath
        if ($sentinel.ContainsKey('FIXTURE_EXIT_CODE')) {
            $fixtureExitCode = [int]$sentinel['FIXTURE_EXIT_CODE']
        }
        $shellUsable = Test-ReceiptValue -Receipt $sentinel -Name 'RUN_ID' -Expected $RunId
        $shellUsable = $shellUsable -and (Test-ReceiptValue -Receipt $sentinel -Name 'SHELL_USABLE_AFTER_TUI' -Expected 'true')
    }
    catch {
        $shellUsable = $false
    }
}

$liveRefresh = $false
$workspaceSessions = $false
$hookInventory = $false
$hookProviderAdapter = $false
$integrations = $false
$providerCapabilityMatrix = $false
$providerBadgeStaged = $false
$helpOverlay = $false
$titleExplanation = $false
$localeSwitched = $false
$interfaceReverted = $false
$interfaceApplyStaged = $false
if (Test-Path -LiteralPath $fixtureResultPath) {
    $fixtureResult = Get-Content -LiteralPath $fixtureResultPath
    $liveRefresh = $fixtureResult -contains 'TUI_LIVE_REFRESH=true'
    $workspaceSessions = $fixtureResult -contains 'TUI_WORKSPACE_SESSIONS=true'
    $hookInventory = $fixtureResult -contains 'TUI_HOOK_INVENTORY=true'
    $hookProviderAdapter = $fixtureResult -contains 'TUI_HOOK_PROVIDER_ADAPTER=true'
    $integrations = $fixtureResult -contains 'TUI_INTEGRATIONS=true'
    $providerCapabilityMatrix = $fixtureResult -contains 'TUI_PROVIDER_CAPABILITY_MATRIX=true'
    $providerBadgeStaged = $fixtureResult -contains 'TUI_PROVIDER_BADGE_STAGED=true'
    $helpOverlay = $fixtureResult -contains 'TUI_HELP_OVERLAY=true'
    $titleExplanation = $fixtureResult -contains 'TUI_TITLE_EXPLANATION=true'
    $localeSwitched = $fixtureResult -contains 'TUI_LANGUAGE_LIVE_SWITCH=true'
    $interfaceReverted = $fixtureResult -contains 'TUI_INTERFACE_REVERT=true'
    $interfaceApplyStaged = $fixtureResult -contains 'TUI_INTERFACE_STAGED_APPLY=true'
}

$passed = $durableCompletionProven -and $sentinelObserved -and $shellUsable -and
    $fixtureExitCode -eq 0 -and $liveRefresh -and $workspaceSessions -and $hookInventory -and
    $hookProviderAdapter -and $integrations -and $providerCapabilityMatrix -and $providerBadgeStaged -and
    $helpOverlay -and $titleExplanation -and $localeSwitched -and
    $interfaceReverted -and $interfaceApplyStaged
$visualDisposition = if ($passed) {
    'PASS'
}
elseif ($watchdogExpired) {
    'UNPROVEN'
}
else {
    'FAIL'
}
$failureClass = if ($passed) {
    'none'
}
elseif ($watchdogExpired) {
    'WATCHDOG_TIMEOUT'
}
elseif ($completionReceiptInvalid) {
    'COMPLETION_RECEIPT_INVALID'
}
else {
    'FIXTURE_OR_EVIDENCE_FAILURE'
}

$receipt = @(
    "RUN_ID=$RunId"
    "EXPECTED_HEAD=$ExpectedHead"
    "CHECKED_OUT_HEAD=$checkedOutHead"
    "BINARY_SHA256=$binarySha256"
    'SMOKE_METHOD=feature-gated deterministic app events in disposable real wt.exe with atomic child completion receipt'
    "WT_LAUNCHER_PROCESS_ID=$wtLauncherProcessId"
    'WINDOWS_TERMINAL_INTERACTIVE_BOUND_BY=WT_SESSION_AND_TTY'
    "CHILD_COMPLETION_RECEIPT_OBSERVED=$($completionReceiptObserved.ToString().ToLowerInvariant())"
    "COMPLETION_RECEIPT_VALID=$($completionSchemaValid.ToString().ToLowerInvariant())"
    "COMPLETION_RECEIPT_RUN_BOUND=$($completionRunBound.ToString().ToLowerInvariant())"
    "COMPLETION_RECEIPT_HEAD_BOUND=$($completionHeadBound.ToString().ToLowerInvariant())"
    "COMPLETION_RECEIPT_BINARY_BOUND=$($completionBinaryBound.ToString().ToLowerInvariant())"
    "COMPLETION_RECEIPT_TOKEN_BOUND=$($completionTokenBound.ToString().ToLowerInvariant())"
    "DURABLE_COMPLETION_PROVEN=$($durableCompletionProven.ToString().ToLowerInvariant())"
    "RESIDUAL_OWNED_PROCESS_OBSERVATION=$residualOwnedProcessObservation"
    'PROCESS_QUERY_DEPENDENCY=none'
    "WATCHDOG_EXPIRED=$($watchdogExpired.ToString().ToLowerInvariant())"
    "SENTINEL_OBSERVED=$($sentinelObserved.ToString().ToLowerInvariant())"
    "FIXTURE_EXIT_CODE=$fixtureExitCode"
    "TUI_LIVE_REFRESH=$($liveRefresh.ToString().ToLowerInvariant())"
    "TUI_WORKSPACE_SESSIONS=$($workspaceSessions.ToString().ToLowerInvariant())"
    "TUI_HOOK_INVENTORY=$($hookInventory.ToString().ToLowerInvariant())"
    "TUI_HOOK_PROVIDER_ADAPTER=$($hookProviderAdapter.ToString().ToLowerInvariant())"
    "TUI_INTEGRATIONS=$($integrations.ToString().ToLowerInvariant())"
    "TUI_PROVIDER_CAPABILITY_MATRIX=$($providerCapabilityMatrix.ToString().ToLowerInvariant())"
    "TUI_PROVIDER_BADGE_STAGED=$($providerBadgeStaged.ToString().ToLowerInvariant())"
    "TUI_HELP_OVERLAY=$($helpOverlay.ToString().ToLowerInvariant())"
    "TUI_TITLE_EXPLANATION=$($titleExplanation.ToString().ToLowerInvariant())"
    "TUI_LANGUAGE_LIVE_SWITCH=$($localeSwitched.ToString().ToLowerInvariant())"
    "TUI_INTERFACE_REVERT=$($interfaceReverted.ToString().ToLowerInvariant())"
    "TUI_INTERFACE_STAGED_APPLY=$($interfaceApplyStaged.ToString().ToLowerInvariant())"
    "VISUAL_OPERATION_DISPOSITION=$visualDisposition"
    "VISUAL_FAILURE_CLASS=$failureClass"
    "WINDOWS_TERMINAL_TUI_SMOKE=$visualDisposition"
    "WINDOWS_TERMINAL_SMOKE=$visualDisposition"
    "TUI_EXIT_RESTORES_TERMINAL=$($passed.ToString().ToLowerInvariant())"
    "SHELL_USABLE_AFTER_TUI=$($shellUsable.ToString().ToLowerInvariant())"
    'OWNER_MUTATIONS=none'
)
Write-AtomicLines -Path $receiptPath -Lines $receipt

$receipt | ForEach-Object { Write-Output $_ }
if (-not $passed) {
    exit 1
}
