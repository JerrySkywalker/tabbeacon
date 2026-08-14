[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$tabBeacon = Join-Path $RepositoryRoot 'target\debug\tabbeacon.exe'
$runKey = Get-Date -Format 'yyyyMMdd-HHmmss'
$outputRoot = Join-Path $RepositoryRoot "target\g05r-lab\hook-fuzz\$runKey"
$labLocalAppData = Join-Path $outputRoot 'local-app-data'
$null = New-Item -ItemType Directory -Force -Path $labLocalAppData

function ConvertTo-Utf8Bytes {
    param([AllowEmptyString()] [string]$Text)
    [System.Text.UTF8Encoding]::new($false).GetBytes($Text)
}

function ConvertTo-JsonBytes {
    param([object]$Value)
    ConvertTo-Utf8Bytes (($Value | ConvertTo-Json -Compress -Depth 100) + "`n")
}

function New-ValidPayload {
    param([hashtable]$Overrides = @{})
    $payload = @{}
    foreach ($entry in $validBase.GetEnumerator()) {
        $payload[$entry.Key] = $entry.Value
    }
    foreach ($entry in $Overrides.GetEnumerator()) {
        $payload[$entry.Key] = $entry.Value
    }
    $payload
}

function Invoke-HookBytes {
    param(
        [Parameter(Mandatory)] [AllowEmptyCollection()] [byte[]]$Bytes,
        [hashtable]$Environment = @{},
        [int]$TimeoutMilliseconds = 10000
    )
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $tabBeacon
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $null = $startInfo.ArgumentList.Add('hook')
    $null = $startInfo.ArgumentList.Add('codex')
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw 'failed to start TabBeacon hook process'
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if ($Bytes.Length -gt 0) {
        $process.StandardInput.BaseStream.Write($Bytes, 0, $Bytes.Length)
    }
    $process.StandardInput.Close()
    if (-not $process.WaitForExit($TimeoutMilliseconds)) {
        $process.Kill($true)
        $process.WaitForExit()
        $timer.Stop()
        return [pscustomobject]@{
            ExitCode = $null
            StdoutLength = $stdoutTask.GetAwaiter().GetResult().Length
            StderrLength = $stderrTask.GetAwaiter().GetResult().Length
            DurationMilliseconds = $timer.ElapsedMilliseconds
            TimedOut = $true
        }
    }
    $timer.Stop()
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        StdoutLength = $stdoutTask.GetAwaiter().GetResult().Length
        StderrLength = $stderrTask.GetAwaiter().GetResult().Length
        DurationMilliseconds = $timer.ElapsedMilliseconds
        TimedOut = $false
    }
}

$validBase = @{
    hook_event_name = 'UserPromptSubmit'
    session_id = 'g05r-fuzz'
    cwd = $RepositoryRoot
    prompt = 'hello'
}
$deletedCwd = Join-Path $outputRoot 'does-not-exist'
$deepJson = ('[' * 256) + '0' + (']' * 256)
$largePrompt = 'p' * (800 * 1024)
$overLimit = 'x' * ((1024 * 1024) + 1)

$cases = @(
    @{ Name = 'empty-stdin'; Bytes = [byte[]]@() },
    @{ Name = 'invalid-utf8'; Bytes = [byte[]]@(0xff, 0xfe, 0xfd, 0x00) },
    @{ Name = 'malformed-json'; Bytes = ConvertTo-Utf8Bytes '{not-json' },
    @{ Name = 'over-limit-input'; Bytes = ConvertTo-Utf8Bytes $overLimit },
    @{ Name = 'deeply-nested-json'; Bytes = ConvertTo-Utf8Bytes $deepJson },
    @{ Name = 'missing-event-name'; Bytes = ConvertTo-JsonBytes @{ session_id = 'x'; cwd = $RepositoryRoot } },
    @{ Name = 'unknown-event'; Bytes = ConvertTo-JsonBytes @{ hook_event_name = 'FutureStableEvent'; session_id = 'x'; cwd = $RepositoryRoot } },
    @{ Name = 'unknown-future-fields'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ future = @{ nested = @(1, 2, 3); opaque = 'yes' } }) },
    @{ Name = 'null-fields'; Bytes = ConvertTo-JsonBytes @{ hook_event_name = 'Stop'; session_id = $null; cwd = $null } },
    @{ Name = 'wrong-field-types'; Bytes = ConvertTo-JsonBytes @{ hook_event_name = 17; session_id = @('x'); cwd = @{ path = 'x' } } },
    @{ Name = 'empty-session-id'; Bytes = ConvertTo-JsonBytes @{ hook_event_name = 'Stop'; session_id = ''; cwd = $RepositoryRoot } },
    @{ Name = 'very-long-session-id'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ session_id = ('s' * (128 * 1024)) }) },
    @{ Name = 'unicode-session-id'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ session_id = '会話-🚀-сессия' }) },
    @{ Name = 'deleted-cwd'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ cwd = $deletedCwd }) },
    @{ Name = 'embedded-nul-cwd'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ cwd = "bad`0cwd" }) },
    @{ Name = 'control-cwd'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ cwd = "bad`e]0;owned`a`ncwd" }) },
    @{ Name = 'hostile-transcript-path'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ transcript_path = "`e]0;owned`a`0path" }) },
    @{ Name = 'very-long-prompt'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ prompt = $largePrompt }) },
    @{ Name = 'duplicate-looking-event-id'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ event_id = '0001'; hook_event_id = '0001'; id = '0001' }) },
    @{ Name = 'timestamp-order-anomaly'; Bytes = ConvertTo-JsonBytes (New-ValidPayload @{ timestamp = '1900-01-01T00:00:00Z'; sequence = -1 }) }
)

$baseEnvironment = @{
    LOCALAPPDATA = $labLocalAppData
    NO_COLOR = '1'
}
$results = @()
foreach ($case in $cases) {
    $probe = Invoke-HookBytes -Bytes $case.Bytes -Environment $baseEnvironment
    $disposition = if (
        -not $probe.TimedOut -and
        $probe.ExitCode -eq 0 -and
        $probe.StdoutLength -eq 0 -and
        $probe.StderrLength -eq 0 -and
        $probe.DurationMilliseconds -lt 10000
    ) { 'PASS' } else { 'FAIL' }
    $results += [pscustomobject]@{
        name = $case.Name
        inputBytes = $case.Bytes.Length
        exitCode = $probe.ExitCode
        stdoutLength = $probe.StdoutLength
        stderrLength = $probe.StderrLength
        durationMilliseconds = $probe.DurationMilliseconds
        timedOut = $probe.TimedOut
        disposition = $disposition
    }
}

# Git lookup is an optional decoration dependency. Removing Git from PATH must
# degrade repository identity without changing the Codex-facing exit contract.
$noGitEnvironment = @{
    LOCALAPPDATA = Join-Path $outputRoot 'no-git-local-app-data'
    PATH = "$env:SystemRoot\System32"
    NO_COLOR = '1'
}
$null = New-Item -ItemType Directory -Force -Path $noGitEnvironment.LOCALAPPDATA
$noGitProbe = Invoke-HookBytes -Bytes (ConvertTo-JsonBytes $validBase) -Environment $noGitEnvironment
$results += [pscustomobject]@{
    name = 'git-command-unavailable'
    inputBytes = (ConvertTo-JsonBytes $validBase).Length
    exitCode = $noGitProbe.ExitCode
    stdoutLength = $noGitProbe.StdoutLength
    stderrLength = $noGitProbe.StderrLength
    durationMilliseconds = $noGitProbe.DurationMilliseconds
    timedOut = $noGitProbe.TimedOut
    disposition = if (-not $noGitProbe.TimedOut -and $noGitProbe.ExitCode -eq 0 -and $noGitProbe.StdoutLength -eq 0 -and $noGitProbe.StderrLength -eq 0) { 'PASS' } else { 'FAIL' }
}

$summary = [ordered]@{
    runKey = $runKey
    outputRoot = $outputRoot
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    maximumInputBytes = 1024 * 1024
    results = $results
    overall = if (@($results | Where-Object disposition -ne 'PASS').Count -eq 0) { 'PASS' } else { 'FAIL' }
}
$summaryPath = Join-Path $outputRoot 'summary.json'
[System.IO.File]::WriteAllText(
    $summaryPath,
    ($summary | ConvertTo-Json -Depth 20),
    [System.Text.UTF8Encoding]::new($false)
)
$summary | ConvertTo-Json -Depth 12
