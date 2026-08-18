[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,
    [Parameter(Mandatory = $true)]
    [string]$SentinelPath,
    [Parameter(Mandatory = $true)]
    [string]$ProcessReceiptPath,
    [Parameter(Mandatory = $true)]
    [string]$RunId
)

$ErrorActionPreference = 'Stop'
[System.IO.File]::WriteAllText($ProcessReceiptPath, [string]$PID)

& $BinaryPath
$fixtureExitCode = $LASTEXITCODE

# This write deliberately occurs in the same PowerShell process after the TUI
# returns. Reaching it proves that the shell can execute commands after raw mode
# and the alternate screen have been restored.
$sentinel = @(
    "RUN_ID=$RunId"
    "FIXTURE_EXIT_CODE=$fixtureExitCode"
    'SHELL_USABLE_AFTER_TUI=true'
)
[System.IO.File]::WriteAllLines($SentinelPath, $sentinel)

# Keep the disposable tab present briefly so the external observer can bind to
# its exact unique title before normal process/window completion.
Start-Sleep -Milliseconds 500
exit $fixtureExitCode
