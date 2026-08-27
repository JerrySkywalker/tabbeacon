[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateScript({ Test-Path -LiteralPath $_ -PathType Leaf })]
    [string]$Binary,

    [Parameter(Mandatory = $true)]
    [ValidateScript({ Test-Path -LiteralPath $_ -PathType Container })]
    [string]$Workspace,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string]$ExpectedHead,

    [Parameter(Mandatory = $true)]
    [ValidateScript({ -not (Test-Path -LiteralPath $_) })]
    [string]$StateRoot,

    [Parameter(Mandatory = $true)]
    [ValidateScript({ -not (Test-Path -LiteralPath $_) })]
    [string]$OutputPath,

    # When supplied, execute the exact generated Hook declaration through the
    # selected Codex 0.149 Windows command-runner shell mode.
    [AllowEmptyString()]
    [string]$HookCommand = '',

    # Codex 0.149 passes a configured non-empty TurnEnvironment shell through
    # to `commandWindows`; COMSPEC is only the fallback when that program is
    # empty. Measure both modes without changing Owner configuration.
    [ValidateSet('ComspecFallback', 'Pwsh7')]
    [string]$HookShell = 'ComspecFallback',

    [ValidateRange(10, 200)]
    [int]$ColdSamples = 30,

    [ValidateRange(30, 500)]
    [int]$WarmSamples = 100,

    [ValidateRange(10, 100)]
    [int]$ConcurrencyRounds = 25
)

$ErrorActionPreference = 'Stop'

$checkedOutHead = (git -C $Workspace rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $checkedOutHead -ne $ExpectedHead) {
    throw "Expected head $ExpectedHead does not match benchmark workspace head $checkedOutHead"
}
$binarySha256 = (Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash
$resolvedBinary = (Resolve-Path -LiteralPath $Binary).Path
$comspec = [Environment]::GetEnvironmentVariable('COMSPEC')
if ([string]::IsNullOrWhiteSpace($comspec)) {
    $comspec = 'cmd.exe'
}
$resolvedComspec = (Resolve-Path -LiteralPath $comspec -ErrorAction Stop).Path
$comspecSha256 = (Get-FileHash -LiteralPath $resolvedComspec -Algorithm SHA256).Hash
$resolvedPwsh = $null
$pwshSha256 = $null
if ($HookShell -eq 'Pwsh7') {
    $resolvedPwsh = (Get-Command pwsh.exe -CommandType Application -ErrorAction Stop).Source
    $pwshSha256 = (Get-FileHash -LiteralPath $resolvedPwsh -Algorithm SHA256).Hash
}

function Get-Percentile {
    param(
        [Parameter(Mandatory = $true)]
        [double[]]$Values,
        [Parameter(Mandatory = $true)]
        [double]$Percentile
    )

    if ($Values.Count -eq 0) {
        throw 'Cannot calculate a percentile without samples.'
    }
    $ordered = @($Values | Sort-Object)
    $index = [Math]::Min($ordered.Count - 1, [Math]::Max(0, [Math]::Ceiling($Percentile * $ordered.Count) - 1))
    return [Math]::Round([double]$ordered[$index], 3)
}

function Get-Statistics {
    param([Parameter(Mandatory = $true)][object[]]$Samples)

    $values = [double[]]@($Samples | ForEach-Object { $_.end_to_end_ms })
    return [ordered]@{
        samples = $values.Count
        p50_ms = Get-Percentile -Values $values -Percentile 0.50
        p95_ms = Get-Percentile -Values $values -Percentile 0.95
        p99_ms = Get-Percentile -Values $values -Percentile 0.99
        max_ms = [Math]::Round([double](($values | Measure-Object -Maximum).Maximum), 3)
        timeout_failures = @($Samples | Where-Object {
            $_.end_to_end_ms -ge 1000 -or $_.root_process_timeout -or $_.stream_eof_timeout
        }).Count
        hook_failures = @($Samples | Where-Object { -not $_.success }).Count
        root_process_timeouts = @($Samples | Where-Object { $_.root_process_timeout }).Count
        stream_eof_timeouts = @($Samples | Where-Object { $_.stream_eof_timeout }).Count
    }
}

function Parse-TimingLine {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$StandardError)

    $match = [regex]::Match(
        $StandardError,
        'TABBEACON_HOOK_TIMING_V1 total_ms=(?<total>\d+) outcome=(?<outcome>[a-z_]+) phases=(?<phases>[^\r\n]*)'
    )
    if (-not $match.Success) {
        return $null
    }
    $phases = [ordered]@{}
    foreach ($part in $match.Groups['phases'].Value.Split(',', [System.StringSplitOptions]::RemoveEmptyEntries)) {
        $pair = $part.Split('=', 2)
        if ($pair.Count -eq 2 -and $pair[0] -match '^[a-z_]+$' -and $pair[1] -match '^\d+$') {
            $phases[$pair[0]] = [int]$pair[1]
        }
    }
    return [pscustomobject]@{
        total_ms = [int]$match.Groups['total'].Value
        outcome = $match.Groups['outcome'].Value
        phases = $phases
    }
}

function New-HookPayload {
    param(
        [Parameter(Mandatory = $true)][string]$Event,
        [Parameter(Mandatory = $true)][string]$Session,
        [AllowEmptyString()][string]$Turn
    )

    $payload = [ordered]@{
        hook_event_name = $Event
        session_id = $Session
        cwd = $Workspace
    }
    if ($Event -eq 'SessionStart') {
        $payload.source = 'startup'
    }
    elseif (-not [string]::IsNullOrEmpty($Turn)) {
        $payload.turn_id = $Turn
    }
    return ($payload | ConvertTo-Json -Compress)
}

function Start-ProductionHook {
    param(
        [Parameter(Mandatory = $true)][string]$State,
        [Parameter(Mandatory = $true)][string]$Event,
        [Parameter(Mandatory = $true)][string]$Session,
        [Parameter(Mandatory = $true)][string]$TerminalToken,
        [AllowEmptyString()][string]$Turn
    )

    [System.IO.Directory]::CreateDirectory($State) | Out-Null
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    if ([string]::IsNullOrWhiteSpace($HookCommand)) {
        $info.FileName = $Binary
        $info.Arguments = 'hook codex'
    }
    else {
        $directDeclarationMatch = [regex]::Match(
            $HookCommand,
            '\A(?<executable>[^\r\n]+) hook codex\z'
        )
        $encodedPowerShellMatch = [regex]::Match(
            $HookCommand,
            '^powershell\.exe -NoProfile -NonInteractive -EncodedCommand [A-Za-z0-9+/]+={0,2}$'
        )
        if (-not $directDeclarationMatch.Success -and -not $encodedPowerShellMatch.Success) {
            throw 'HookCommand is not an admitted synchronous Windows declaration.'
        }
        if ($directDeclarationMatch.Success) {
            $declaredBinary = (Resolve-Path -LiteralPath $directDeclarationMatch.Groups['executable'].Value).Path
            if ($declaredBinary -ne $resolvedBinary) {
                throw 'Direct Hook declaration does not bind to the measured binary.'
            }
        }
        else {
            $encodedScript = [Text.Encoding]::Unicode.GetString(
                [Convert]::FromBase64String($HookCommand.Split(' ')[-1])
            )
            $encodedMatch = [regex]::Match(
                $encodedScript,
                '^\$ErrorActionPreference = ''SilentlyContinue''; & ''(?<executable>(?:''''|[^''])*)'' hook codex 1>\$null 2>\$null; exit 0$'
            )
            if (-not $encodedMatch.Success) {
                throw 'Encoded PowerShell Hook declaration is not the admitted fixed script.'
            }
            $declaredBinary = (Resolve-Path -LiteralPath ($encodedMatch.Groups['executable'].Value -replace "''", "'")).Path
            if ($declaredBinary -ne $resolvedBinary) {
                throw 'Encoded PowerShell Hook declaration does not bind to the measured binary.'
            }
        }
        if ($HookShell -eq 'Pwsh7') {
            # Mirror the Codex 0.149 non-empty TurnEnvironment branch: each
            # argument is forwarded normally to pwsh's `-Command` entrypoint.
            $info.FileName = $resolvedPwsh
            $info.ArgumentList.Add('-NoProfile')
            $info.ArgumentList.Add('-Command')
            $info.ArgumentList.Add($HookCommand)
        }
        else {
            # Mirror the admitted empty-shell fallback: the effective COMSPEC
            # executable receives `/C` plus one raw outer-quoted declaration.
            $info.FileName = $resolvedComspec
            $info.Arguments = '/C "' + $HookCommand + '"'
        }
    }
    $info.WorkingDirectory = $Workspace
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.CreateNoWindow = $true
    $info.EnvironmentVariables['LOCALAPPDATA'] = $State
    # This non-Owner terminal token exercises the production activity
    # coordinator without reading or modifying a real terminal profile.
    $info.EnvironmentVariables['WT_SESSION'] = $TerminalToken
    $timingFile = Join-Path $State ("timing-{0}.txt" -f [guid]::NewGuid().ToString('N'))
    $info.EnvironmentVariables['TABBEACON_HOOK_TIMING_FILE'] = $timingFile
    # CreateNoWindow does not provide the owned console that a real Windows
    # Terminal activity worker inherits. Keep this isolated fixture's worker
    # alive through the production probe seam, which renders to NUL while
    # retaining the real worker/observer lifecycle and explicit detached stdio.
    $info.EnvironmentVariables['TABBEACON_ACTIVITY_WORKER_PROBE_RECEIPT'] =
        Join-Path $State 'activity-worker-probe.json'

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $info
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw 'TabBeacon Hook process did not start.'
    }
    $standardOutput = $process.StandardOutput.ReadToEndAsync()
    $standardError = $process.StandardError.ReadToEndAsync()
    $process.StandardInput.Write((New-HookPayload -Event $Event -Session $Session -Turn $Turn))
    $process.StandardInput.Close()
    return [pscustomobject]@{
        process = $process
        stopwatch = $stopwatch
        event = $Event
        timing_file = $timingFile
        standard_output = $standardOutput
        standard_error = $standardError
        standard_output_reader = $process.StandardOutput
        standard_error_reader = $process.StandardError
        root_process_timeout = $false
        stream_eof_timeout = $false
    }
}

function Stop-OwnedHookProcessTree {
    param([Parameter(Mandatory = $true)]$Process)

    if ($Process.HasExited) {
        return $true
    }
    try {
        # The process was started by this measurement run. Tree termination is
        # intentionally limited to that direct owned root, mirroring Codex's
        # one-second Hook timeout without touching unrelated processes.
        $Process.Kill($true)
    }
    catch {
        return $false
    }
    return $Process.WaitForExit(250)
}

function Complete-HookStream {
    param(
        [Parameter(Mandatory = $true)]$Task,
        [Parameter(Mandatory = $true)]$Reader,
        [Parameter(Mandatory = $true)]$Pending
    )

    try {
        $remainingMs = [Math]::Max(0, [int][Math]::Floor(1000 - $Pending.stopwatch.Elapsed.TotalMilliseconds))
        if ($remainingMs -gt 0 -and $Task.Wait($remainingMs)) {
            return [pscustomobject]@{ text = $Task.GetAwaiter().GetResult(); eof_timeout = $false }
        }
    }
    catch {
        return [pscustomobject]@{ text = ''; eof_timeout = $true }
    }
    # A detached descendant must not turn a completed synchronous Hook into an
    # unbounded runner wait. Close only this owned pipe, then report the
    # missing EOF as a strict benchmark failure.
    $Reader.Close()
    return [pscustomobject]@{ text = ''; eof_timeout = $true }
}

function Complete-ProductionHook {
    param([Parameter(Mandatory = $true)]$Pending)

    $remainingMs = [Math]::Max(0, [int][Math]::Floor(1000 - $Pending.stopwatch.Elapsed.TotalMilliseconds))
    if (-not $Pending.process.HasExited -and ($remainingMs -le 0 -or -not $Pending.process.WaitForExit($remainingMs))) {
        $Pending.root_process_timeout = $true
        $null = Stop-OwnedHookProcessTree -Process $Pending.process
    }
    $outerOutput = Complete-HookStream -Task $Pending.standard_output -Reader $Pending.standard_output_reader -Pending $Pending
    $outerError = Complete-HookStream -Task $Pending.standard_error -Reader $Pending.standard_error_reader -Pending $Pending
    $Pending.stream_eof_timeout = $Pending.stream_eof_timeout -or $outerOutput.eof_timeout -or $outerError.eof_timeout
    if ($Pending.stopwatch.IsRunning) { $Pending.stopwatch.Stop() }
    $standardError = ''
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        if (Test-Path -LiteralPath $Pending.timing_file -PathType Leaf) {
            $standardError = [System.IO.File]::ReadAllText($Pending.timing_file)
            break
        }
        Start-Sleep -Milliseconds 10
    }
    $timing = Parse-TimingLine -StandardError $standardError
    $exitCode = if ($Pending.process.HasExited) { $Pending.process.ExitCode } else { -1 }
    $success = -not $Pending.root_process_timeout -and -not $Pending.stream_eof_timeout -and
        $exitCode -eq 0 -and $null -ne $timing -and
        $timing.outcome -eq 'applied' -and $outerOutput.text.Length -eq 0 -and
        $outerError.text.Length -eq 0
    $phases = if ($null -eq $timing) {
        [ordered]@{}
    }
    else {
        $captured = [ordered]@{}
        foreach ($phase in $timing.phases.Keys) {
            $captured[$phase] = $timing.phases[$phase]
        }
        # The inner diagnostic starts inside the product process. Keep its
        # complement explicit so a declaration benchmark attributes the
        # shell, process creation, and exit cost instead of hiding it.
        $captured['shell_process_start_and_exit'] = [Math]::Round(
            [Math]::Max(0, $Pending.stopwatch.Elapsed.TotalMilliseconds - $timing.total_ms),
            3
        )
        $captured
    }
    return [pscustomobject]@{
        event = $Pending.event
        end_to_end_ms = [Math]::Round(
            [Math]::Max($Pending.stopwatch.Elapsed.TotalMilliseconds, $(if ($Pending.root_process_timeout -or $Pending.stream_eof_timeout) { 1000 } else { 0 })),
            3
        )
        process_total_ms = if ($null -eq $timing) { $null } else { $timing.total_ms }
        phases = $phases
        success = $success
        exit_code = $exitCode
        stdout_bytes = [System.Text.Encoding]::UTF8.GetByteCount($outerOutput.text)
        stderr_bytes = [System.Text.Encoding]::UTF8.GetByteCount($outerError.text)
        stderr_timing_present = $null -ne $timing
        root_process_timeout = $Pending.root_process_timeout
        stream_eof_timeout = $Pending.stream_eof_timeout
    }
}

function Wait-ConcurrentProductionHooks {
    param([Parameter(Mandatory = $true)][object[]]$Pending)

    $remaining = [System.Collections.Generic.List[object]]::new()
    foreach ($hook in $Pending) {
        $remaining.Add($hook)
    }
    $completed = [System.Collections.Generic.List[object]]::new()
    while ($remaining.Count -gt 0) {
        for ($index = $remaining.Count - 1; $index -ge 0; $index--) {
            $hook = $remaining[$index]
            if ($hook.process.HasExited -and $hook.standard_output.IsCompleted -and $hook.standard_error.IsCompleted) {
                # Stop each sample only after the root and both production
                # streams complete, preventing concurrent runner queueing while
                # retaining Codex's one-second end-to-end budget.
                if ($hook.stopwatch.IsRunning) {
                    $hook.stopwatch.Stop()
                }
                $completed.Add($hook)
                $remaining.RemoveAt($index)
            }
            elseif ($hook.stopwatch.Elapsed.TotalMilliseconds -ge 1000) {
                if (-not $hook.process.HasExited) {
                    $hook.root_process_timeout = $true
                    $null = Stop-OwnedHookProcessTree -Process $hook.process
                }
                if (-not $hook.standard_output.IsCompleted -or -not $hook.standard_error.IsCompleted) {
                    $hook.stream_eof_timeout = $true
                    $hook.standard_output_reader.Close()
                    $hook.standard_error_reader.Close()
                }
                if ($hook.stopwatch.IsRunning) {
                    $hook.stopwatch.Stop()
                }
                $completed.Add($hook)
                $remaining.RemoveAt($index)
            }
        }
        if ($remaining.Count -gt 0) {
            Start-Sleep -Milliseconds 1
        }
    }
    return $completed
}

function Invoke-ProductionHook {
    param(
        [Parameter(Mandatory = $true)][string]$State,
        [Parameter(Mandatory = $true)][string]$Event,
        [Parameter(Mandatory = $true)][string]$Session,
        [Parameter(Mandatory = $true)][string]$TerminalToken,
        [AllowEmptyString()][string]$Turn
    )

    return Complete-ProductionHook -Pending (Start-ProductionHook -State $State -Event $Event -Session $Session -TerminalToken $TerminalToken -Turn $Turn)
}

function Get-PhaseAttribution {
    param([Parameter(Mandatory = $true)][object[]]$Samples)

    $phaseNames = @('shell_process_start_and_exit', 'state_root', 'runtime_initialization', 'console_open', 'normalization', 'generation_admission', 'workspace_anchor', 'presentation_and_activity', 'terminal_write')
    $result = [ordered]@{}
    foreach ($phaseName in $phaseNames) {
        $values = [double[]]@($Samples | ForEach-Object {
            if ($_.phases.Contains($phaseName)) { [double]$_.phases[$phaseName] }
        })
        if ($values.Count -gt 0) {
            $result[$phaseName] = [ordered]@{
                samples = $values.Count
                p50_ms = Get-Percentile -Values $values -Percentile 0.50
                p95_ms = Get-Percentile -Values $values -Percentile 0.95
                p99_ms = Get-Percentile -Values $values -Percentile 0.99
                max_ms = [Math]::Round([double](($values | Measure-Object -Maximum).Maximum), 3)
            }
        }
    }
    return $result
}

[System.IO.Directory]::CreateDirectory($StateRoot) | Out-Null
$cold = @()
for ($index = 1; $index -le $ColdSamples; $index++) {
    $state = Join-Path $StateRoot "cold-$index"
    $session = "cold-session-$index"
    $terminal = "00000000-0000-0000-0000-{0:D12}" -f $index
    $cold += Invoke-ProductionHook -State $state -Event 'SessionStart' -Session $session -TerminalToken $terminal -Turn ''
    # Cold sessions are measured once, then explicitly retired. Leaving their
    # detached cleanup observers active would add background process load to
    # the next sample and turn a declaration benchmark into self-interference.
    $null = Invoke-ProductionHook -State $state -Event 'SessionEnd' -Session $session -TerminalToken $terminal -Turn ''
}

$warmState = Join-Path $StateRoot 'warm'
$warmSession = 'warm-session'
$warmTurn = 'warm-turn'
$warmTerminal = '00000000-0000-0000-0000-000000000052'
$null = Invoke-ProductionHook -State $warmState -Event 'SessionStart' -Session $warmSession -TerminalToken $warmTerminal -Turn ''
$null = Invoke-ProductionHook -State $warmState -Event 'UserPromptSubmit' -Session $warmSession -TerminalToken $warmTerminal -Turn $warmTurn
$warm = @()
for ($index = 1; $index -le $WarmSamples; $index++) {
    $warm += Invoke-ProductionHook -State $warmState -Event 'PostToolUse' -Session $warmSession -TerminalToken $warmTerminal -Turn $warmTurn
}
$null = Invoke-ProductionHook -State $warmState -Event 'SessionEnd' -Session $warmSession -TerminalToken $warmTerminal -Turn ''

function Measure-ConcurrentNormalHooks {
    param(
        [Parameter(Mandatory = $true)][ValidateRange(1, 8)][int]$Slots,
        [Parameter(Mandatory = $true)][int]$Rounds
    )

    $state = Join-Path $StateRoot "concurrency-$Slots"
    $samples = @()
    for ($slot = 1; $slot -le $Slots; $slot++) {
        $session = "concurrency-$Slots-session-$slot"
        $turn = "concurrency-$Slots-turn-$slot"
        $terminal = "00000000-0000-0000-0000-{0:D12}" -f (100 + $slot)
        $null = Invoke-ProductionHook -State $state -Event 'SessionStart' -Session $session -TerminalToken $terminal -Turn ''
        $null = Invoke-ProductionHook -State $state -Event 'UserPromptSubmit' -Session $session -TerminalToken $terminal -Turn $turn
        # Establish the same post-anchor activity transition as a real turn
        # before timing the steady ordinary Hook path. This keeps worker
        # creation/supersession out of the warm multi-Codex distribution.
        $null = Invoke-ProductionHook -State $state -Event 'PostToolUse' -Session $session -TerminalToken $terminal -Turn $turn
    }
    # Let the detached observer perform its one identity admission before the
    # steady multi-session sample. This is intentionally outside the normal
    # Hook distribution and does not mask any synchronous Hook work.
    Start-Sleep -Milliseconds 3000
    for ($round = 1; $round -le $Rounds; $round++) {
        $pending = @()
        for ($slot = 1; $slot -le $Slots; $slot++) {
            $terminal = "00000000-0000-0000-0000-{0:D12}" -f (100 + $slot)
            $pending += Start-ProductionHook -State $state -Event 'PostToolUse' -Session "concurrency-$Slots-session-$slot" -TerminalToken $terminal -Turn "concurrency-$Slots-turn-$slot"
        }
        foreach ($hook in @(Wait-ConcurrentProductionHooks -Pending $pending)) {
            $samples += Complete-ProductionHook -Pending $hook
        }
    }
    for ($slot = 1; $slot -le $Slots; $slot++) {
        $terminal = "00000000-0000-0000-0000-{0:D12}" -f (100 + $slot)
        $null = Invoke-ProductionHook -State $state -Event 'SessionEnd' -Session "concurrency-$Slots-session-$slot" -TerminalToken $terminal -Turn ''
    }
    return $samples
}

$concurrency4 = @(Measure-ConcurrentNormalHooks -Slots 4 -Rounds $ConcurrencyRounds)
$concurrency8 = @(Measure-ConcurrentNormalHooks -Slots 8 -Rounds $ConcurrencyRounds)

$report = [ordered]@{
    schema = 'tabbeacon-codex-hook-runtime-measurement-v1'
    expected_head = $ExpectedHead
    checked_out_head = $checkedOutHead
    binary = [System.IO.Path]::GetFileName($Binary)
    binary_sha256 = $binarySha256
    comspec_basename = [System.IO.Path]::GetFileName($resolvedComspec)
    comspec_sha256 = $comspecSha256
    hook_shell = $HookShell
    hook_shell_basename = if ($HookShell -eq 'Pwsh7') { [System.IO.Path]::GetFileName($resolvedPwsh) } else { [System.IO.Path]::GetFileName($resolvedComspec) }
    hook_shell_sha256 = if ($HookShell -eq 'Pwsh7') { $pwshSha256 } else { $comspecSha256 }
    workspace_kind = 'git_worktree'
    invocation = if ([string]::IsNullOrWhiteSpace($HookCommand)) { 'direct_binary' } else { "generated_command_windows_$HookShell" }
    hook_declaration_mode = if ([string]::IsNullOrWhiteSpace($HookCommand)) {
        'direct_binary'
    } elseif ($HookCommand.StartsWith('powershell.exe -NoProfile -NonInteractive -EncodedCommand ')) {
        'encoded_powershell_compatibility_fallback'
    } else {
        'direct_native_shell_neutral'
    }
    hook_declaration_sha256 = if ([string]::IsNullOrWhiteSpace($HookCommand)) {
        $null
    } else {
        ([System.Security.Cryptography.SHA256]::HashData(
            [System.Text.Encoding]::UTF8.GetBytes($HookCommand)
        ) | ForEach-Object { $_.ToString('x2') }) -join ''
    }
    terminal_binding = 'isolated_synthetic_per_session_windows_terminal_tokens'
    activity_worker_probe = 'isolated_NUL_backed_long_lived_worker'
    declaration_timeout_ms = 1000
    cold = Get-Statistics -Samples $cold
    warm = Get-Statistics -Samples $warm
    concurrency_1 = Get-Statistics -Samples $warm
    concurrency_4 = Get-Statistics -Samples $concurrency4
    concurrency_8 = Get-Statistics -Samples $concurrency8
    phase_attribution = [ordered]@{
        cold = Get-PhaseAttribution -Samples $cold
        warm = Get-PhaseAttribution -Samples $warm
        concurrency_1 = Get-PhaseAttribution -Samples $warm
        concurrency_4 = Get-PhaseAttribution -Samples $concurrency4
        concurrency_8 = Get-PhaseAttribution -Samples $concurrency8
    }
}

$outputParent = Split-Path -Parent $OutputPath
if (-not [string]::IsNullOrWhiteSpace($outputParent)) {
    [System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
}
[System.IO.File]::WriteAllText(
    $OutputPath,
    ($report | ConvertTo-Json -Depth 8),
    [System.Text.UTF8Encoding]::new($false)
)
$report | ConvertTo-Json -Depth 8
