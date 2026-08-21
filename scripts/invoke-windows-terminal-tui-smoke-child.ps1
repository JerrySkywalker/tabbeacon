[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,
    [Parameter(Mandatory = $true)]
    [string]$SentinelPath,
    [Parameter(Mandatory = $true)]
    [string]$FixtureResultPath,
    [Parameter(Mandatory = $true)]
    [string]$CompletionReceiptPath,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{32}$')]
    [string]$CompletionToken,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string]$ExpectedHead,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-F0-9]{64}$')]
    [string]$BinarySha256,
    [Parameter(Mandatory = $true)]
    [string]$RunId
)

$ErrorActionPreference = 'Stop'

function Write-AtomicLines {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Lines
    )

    if (Test-Path -LiteralPath $Path) {
        throw "Refusing to overwrite existing child evidence artifact $Path"
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

$env:TABBEACON_TUI_SMOKE_RESULT_PATH = $FixtureResultPath
& $BinaryPath
$fixtureExitCode = $LASTEXITCODE
$fixtureResultPresent = Test-Path -LiteralPath $FixtureResultPath

# This write deliberately occurs in the same PowerShell process after the TUI
# returns. Reaching it proves that the shell can execute commands after raw mode
# and the alternate screen have been restored.
$sentinel = @(
    "RUN_ID=$RunId"
    "FIXTURE_EXIT_CODE=$fixtureExitCode"
    'SHELL_USABLE_AFTER_TUI=true'
)
Write-AtomicLines -Path $SentinelPath -Lines $sentinel

$completion = @(
    'COMPLETION_SCHEMA=tabbeacon-wt-child-completion-v1'
    "COMPLETION_TOKEN=$CompletionToken"
    "RUN_ID=$RunId"
    "EXPECTED_HEAD=$ExpectedHead"
    "BINARY_SHA256=$BinarySha256"
    "CHILD_PROCESS_ID=$PID"
    "FIXTURE_EXIT_CODE=$fixtureExitCode"
    "FIXTURE_RESULT_PRESENT=$($fixtureResultPresent.ToString().ToLowerInvariant())"
    'SENTINEL_WRITTEN=true'
    'SHELL_USABLE_AFTER_TUI=true'
    'COMPLETED=true'
)
Write-AtomicLines -Path $CompletionReceiptPath -Lines $completion
exit $fixtureExitCode
