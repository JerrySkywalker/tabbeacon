[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$sourceBinary = Join-Path $RepositoryRoot 'target\debug\tabbeacon.exe'
$runKey = Get-Date -Format 'yyyyMMdd-HHmmss'
$outputRoot = Join-Path $RepositoryRoot "target\g05r-lab\windows-quoting\$runKey"
$null = New-Item -ItemType Directory -Force -Path $outputRoot

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string]$FilePath,
        [string[]]$ArgumentList = @(),
        [hashtable]$Environment = @{},
        [AllowEmptyString()] [string]$InputText = '',
        [int]$TimeoutMilliseconds = 10000
    )
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $ArgumentList) {
        $null = $startInfo.ArgumentList.Add($argument)
    }
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw "failed to start $FilePath"
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if ($InputText.Length -gt 0) {
        $process.StandardInput.Write($InputText)
    }
    $process.StandardInput.Close()
    if (-not $process.WaitForExit($TimeoutMilliseconds)) {
        $process.Kill($true)
        $process.WaitForExit()
        $timer.Stop()
        return [pscustomobject]@{
            ExitCode = $null
            Stdout = $stdoutTask.GetAwaiter().GetResult()
            Stderr = $stderrTask.GetAwaiter().GetResult()
            DurationMilliseconds = $timer.ElapsedMilliseconds
            TimedOut = $true
        }
    }
    $timer.Stop()
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
        DurationMilliseconds = $timer.ElapsedMilliseconds
        TimedOut = $false
    }
}

function Invoke-RawCmdCommand {
    param(
        [Parameter(Mandatory)] [string]$CommandLine,
        [Parameter(Mandatory)] [hashtable]$Environment,
        [Parameter(Mandatory)] [string]$Payload,
        [switch]$DisableAutoRun
    )
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = 'cmd.exe'
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $prefix = if ($DisableAutoRun) { '/D /S /C' } else { '/S /C' }
    # Match Codex's Windows raw-argument shape: cmd.exe /C "<configured command>".
    $startInfo.Arguments = "$prefix `"$CommandLine`""
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw 'failed to start cmd.exe'
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $process.StandardInput.Write($Payload)
    $process.StandardInput.Close()
    if (-not $process.WaitForExit(5000)) {
        $process.Kill($true)
        $process.WaitForExit()
        $timer.Stop()
        return [pscustomobject]@{
            ExitCode = $null
            Stdout = $stdoutTask.GetAwaiter().GetResult()
            Stderr = $stderrTask.GetAwaiter().GetResult()
            DurationMilliseconds = $timer.ElapsedMilliseconds
            TimedOut = $true
        }
    }
    $timer.Stop()
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
        DurationMilliseconds = $timer.ElapsedMilliseconds
        TimedOut = $false
    }
}

$longPart = 'long-' + ('x' * 30)
$overlongPart = 'over-' + ('y' * 42)
$cases = @(
    @{ Name = 'spaces'; Segment = 'binary with spaces'; Expected = 'PASS' },
    @{ Name = 'parentheses'; Segment = 'binary(paren)'; Expected = 'PASS' },
    @{ Name = 'ampersand'; Segment = 'amp&echo TB_G05R_INJECTED&'; Expected = 'PASS' },
    @{ Name = 'caret'; Segment = 'caret^segment'; Expected = 'PASS' },
    @{ Name = 'bang'; Segment = 'bang!segment'; Expected = 'PASS' },
    @{ Name = 'apostrophe'; Segment = "apostrophe'segment"; Expected = 'PASS' },
    @{ Name = 'unicode'; Segment = 'ユニコード-路径-таб'; Expected = 'PASS' },
    @{ Name = 'percent-rejected'; Segment = 'percent%PATH%segment'; Expected = 'REJECT' },
    @{ Name = 'long'; Segment = "$longPart\$longPart\$longPart"; Expected = 'PASS' },
    @{ Name = 'overlong-host-boundary'; Segment = "$overlongPart\$overlongPart\$overlongPart\$overlongPart"; Expected = 'ENVIRONMENT_LIMIT' }
)

$payload = (@{
    hook_event_name = 'UserPromptSubmit'
    session_id = 'g05r-windows-quoting'
    cwd = $RepositoryRoot
    prompt = 'quoting probe'
} | ConvertTo-Json -Compress) + "`n"

$results = @()
foreach ($case in $cases) {
    $caseRoot = Join-Path $outputRoot $case.Name
    $binaryDirectory = Join-Path $caseRoot $case.Segment
    $labCodexHome = Join-Path $caseRoot "codex home & config%$($case.Name)!"
    $labLocalAppData = Join-Path $caseRoot "local app data (g05r)^$($case.Name)'"
    $result = [ordered]@{
        name = $case.Name
        expected = $case.Expected
        binaryDirectory = $binaryDirectory
        executableLength = $null
        setup = $null
        commandWindows = $null
        defaultCmd = $null
        noAutoRunCmd = $null
        injectionObserved = $false
        disposition = 'UNPROVEN'
        error = $null
    }
    try {
        $null = New-Item -ItemType Directory -Force -Path $binaryDirectory, $labCodexHome, $labLocalAppData
        $caseBinary = Join-Path $binaryDirectory 'tabbeacon.exe'
        Copy-Item -LiteralPath $sourceBinary -Destination $caseBinary
        $result.executableLength = $caseBinary.Length
        $caseEnvironment = @{
            CODEX_HOME = $labCodexHome
            LOCALAPPDATA = $labLocalAppData
            NO_COLOR = '1'
        }
        $result.setup = Invoke-CapturedProcess -FilePath $caseBinary -ArgumentList @('setup', 'codex') -Environment $caseEnvironment
        if ($case.Expected -eq 'REJECT') {
            $hooksCreated = Test-Path -LiteralPath (Join-Path $labCodexHome 'hooks.json')
            $result.disposition = if ($result.setup.ExitCode -ne 0 -and -not $hooksCreated) { 'PASS' } else { 'FAIL' }
        } elseif ($result.setup.ExitCode -ne 0) {
            $result.disposition = 'FAIL'
        } else {
            $hooks = Get-Content -LiteralPath (Join-Path $labCodexHome 'hooks.json') -Raw | ConvertFrom-Json -Depth 30
            $result.commandWindows = $hooks.hooks.UserPromptSubmit[0].hooks[0].commandWindows
            $result.defaultCmd = Invoke-RawCmdCommand -CommandLine $result.commandWindows -Environment $caseEnvironment -Payload $payload
            $result.noAutoRunCmd = Invoke-RawCmdCommand -CommandLine $result.commandWindows -Environment $caseEnvironment -Payload $payload -DisableAutoRun
            $combinedOutput = $result.defaultCmd.Stdout + $result.defaultCmd.Stderr + $result.noAutoRunCmd.Stdout + $result.noAutoRunCmd.Stderr
            $result.injectionObserved = $combinedOutput.Contains('TB_G05R_INJECTED')
            $exactExecutableQuoted = $result.commandWindows.StartsWith('"' + $caseBinary + '" hook codex')
            $bounded = $result.defaultCmd.DurationMilliseconds -lt 5000 -and $result.noAutoRunCmd.DurationMilliseconds -lt 5000
            $result.disposition = if (
                $exactExecutableQuoted -and
                -not $result.defaultCmd.TimedOut -and
                -not $result.noAutoRunCmd.TimedOut -and
                $result.defaultCmd.ExitCode -eq 0 -and
                $result.noAutoRunCmd.ExitCode -eq 0 -and
                -not $result.injectionObserved -and
                $bounded
            ) { 'PASS' } else { 'FAIL' }
        }
    } catch {
        $result.error = $_.Exception.Message
        $result.disposition = if ($case.Expected -eq 'ENVIRONMENT_LIMIT') { 'WINDOWS_ENVIRONMENT' } else { 'FAIL' }
    }
    $results += [pscustomobject]$result
}

$unrepresentable = @(
    [pscustomobject]@{ name = 'pipe'; character = '|'; disposition = 'FILESYSTEM_UNREPRESENTABLE' },
    [pscustomobject]@{ name = 'double-quote'; character = '"'; disposition = 'FILESYSTEM_UNREPRESENTABLE' }
)

$summary = [ordered]@{
    runKey = $runKey
    outputRoot = $outputRoot
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    results = $results
    unrepresentableWindowsPathCharacters = $unrepresentable
}
$summaryPath = Join-Path $outputRoot 'summary.json'
[System.IO.File]::WriteAllText(
    $summaryPath,
    ($summary | ConvertTo-Json -Depth 30),
    [System.Text.UTF8Encoding]::new($false)
)
$summary | ConvertTo-Json -Depth 12
