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

    [ValidateRange(30, 500)]
    [int]$WarmSamples = 100,

    [ValidateRange(10, 100)]
    [int]$ConcurrencyRounds = 25,

    [ValidateRange(10, 100)]
    [int]$SessionEndSamples = 30
)

$ErrorActionPreference = 'Stop'

$checkedOutHead = (git -C $Workspace rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $checkedOutHead -ne $ExpectedHead) {
    throw "Expected head $ExpectedHead does not match benchmark workspace head $checkedOutHead"
}

$resolvedBinary = (Resolve-Path -LiteralPath $Binary).Path
$binarySha256 = (Get-FileHash -LiteralPath $resolvedBinary -Algorithm SHA256).Hash
$comspec = [Environment]::GetEnvironmentVariable('COMSPEC')
if ([string]::IsNullOrWhiteSpace($comspec)) { $comspec = 'cmd.exe' }
$resolvedComspec = (Resolve-Path -LiteralPath $comspec -ErrorAction Stop).Path

function Get-Percentile {
    param([double[]]$Values, [double]$Percentile)

    if ($Values.Count -eq 0) { throw 'Cannot calculate a percentile without samples.' }
    $ordered = @($Values | Sort-Object)
    $index = [Math]::Min($ordered.Count - 1, [Math]::Max(0, [Math]::Ceiling($Percentile * $ordered.Count) - 1))
    return [Math]::Round([double]$ordered[$index], 3)
}

function Get-Statistics {
    param([object[]]$Samples)

    $values = [double[]]@($Samples | ForEach-Object { $_.milliseconds })
    return [ordered]@{
        samples = $values.Count
        p50_ms = Get-Percentile -Values $values -Percentile 0.50
        p95_ms = Get-Percentile -Values $values -Percentile 0.95
        p99_ms = Get-Percentile -Values $values -Percentile 0.99
        max_ms = [Math]::Round([double](($values | Measure-Object -Maximum).Maximum), 3)
        failures = @($Samples | Where-Object { -not $_.success }).Count
        timeout_failures = @($Samples | Where-Object { $_.timeout }).Count
    }
}

function New-McpRequest {
    param([int]$Id, [string]$Event, [string]$Session, [string]$Turn, [string]$Cwd)

    $arguments = [ordered]@{
        hook_event_name = $Event
        session_id = $Session
        cwd = $Cwd
    }
    if ($Event -eq 'SessionStart') {
        $arguments.source = 'startup'
    }
    elseif (-not [string]::IsNullOrWhiteSpace($Turn)) {
        $arguments.turn_id = $Turn
    }
    return [ordered]@{
        jsonrpc = '2.0'
        id = $Id
        method = 'tools/call'
        params = [ordered]@{
            name = 'tabbeacon_hook_event'
            arguments = $arguments
        }
    }
}

function Start-McpServer {
    param([string]$State, [string]$TerminalToken)

    [System.IO.Directory]::CreateDirectory($State) | Out-Null
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $resolvedBinary
    $info.Arguments = '__mcp-hook-stdio-v1'
    $info.WorkingDirectory = $Workspace
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.EnvironmentVariables['LOCALAPPDATA'] = $State
    $info.EnvironmentVariables['WT_SESSION'] = $TerminalToken
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $info
    if (-not $process.Start()) { throw 'MCP server did not start.' }
    return [pscustomobject]@{
        process = $process
        input = $process.StandardInput
        output = $process.StandardOutput
        error = $process.StandardError
        next_id = 1
    }
}

function Start-McpCall {
    param($Server, [string]$Event, [string]$Session, [string]$Turn, [string]$Cwd)

    $id = $Server.next_id
    $Server.next_id++
    $request = New-McpRequest -Id $id -Event $Event -Session $Session -Turn $Turn -Cwd $Cwd
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $Server.input.WriteLine(($request | ConvertTo-Json -Compress -Depth 6))
    $Server.input.Flush()
    return [pscustomobject]@{
        server = $Server
        id = $id
        stopwatch = $watch
    }
}

function Complete-McpCall {
    param($Pending, [int]$TimeoutMilliseconds = 1000)

    $task = $Pending.server.output.ReadLineAsync()
    $completed = $task.Wait($TimeoutMilliseconds)
    if ($Pending.stopwatch.IsRunning) { $Pending.stopwatch.Stop() }
    if (-not $completed) {
        return [pscustomobject]@{ milliseconds = 1000.0; success = $false; timeout = $true }
    }
    $line = $task.GetAwaiter().GetResult()
    $response = $null
    try { $response = $line | ConvertFrom-Json -ErrorAction Stop } catch { }
    $success = $null -ne $response -and $response.id -eq $Pending.id -and
        $null -ne $response.result -and $response.result.isError -ne $true
    return [pscustomobject]@{
        milliseconds = [Math]::Round($Pending.stopwatch.Elapsed.TotalMilliseconds, 3)
        success = $success
        timeout = $false
    }
}

function Initialize-McpSession {
    param($Server, [string]$Session, [string]$Turn, [string]$Cwd, [bool]$PrepareWorking = $true)

    $id = $Server.next_id
    $Server.next_id++
    $initialize = [ordered]@{
        jsonrpc = '2.0'
        id = $id
        method = 'initialize'
        params = [ordered]@{ protocolVersion = '2025-06-18'; capabilities = [ordered]@{} }
    }
    $Server.input.WriteLine(($initialize | ConvertTo-Json -Compress -Depth 6))
    $Server.input.Flush()
    $initialization = Complete-McpCall -Pending ([pscustomobject]@{
        server = $Server; id = $id; stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    })
    if (-not $initialization.success) { throw 'MCP initialize did not succeed.' }
    $events = @('SessionStart')
    if ($PrepareWorking) { $events += 'UserPromptSubmit' }
    foreach ($event in $events) {
        $result = Complete-McpCall -Pending (Start-McpCall -Server $Server -Event $event -Session $Session -Turn $Turn -Cwd $Cwd)
        if (-not $result.success) { throw "MCP $event did not succeed." }
    }
}

function Stop-OwnedProcessTree {
    param($Process)

    if ($Process.HasExited) { return }
    try { $Process.Kill($true) } catch { }
    $null = $Process.WaitForExit(250)
}

function Invoke-SessionEndCommand {
    param([string]$State, [string]$Session, [string]$TerminalToken)

    # The current 0.149 declaration selects this shell-neutral commandWindows
    # shape for a safe native path. Execute through COMSPEC to reproduce the
    # empty-shell runner branch without touching Owner Codex configuration.
    $command = "$resolvedBinary hook codex"
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $resolvedComspec
    $info.Arguments = '/D /S /C "' + $command + '"'
    $info.WorkingDirectory = $Workspace
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.EnvironmentVariables['LOCALAPPDATA'] = $State
    $info.EnvironmentVariables['WT_SESSION'] = $TerminalToken
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $info
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) { throw 'SessionEnd command did not start.' }
    $payload = [ordered]@{ hook_event_name = 'SessionEnd'; session_id = $Session; cwd = $Workspace } |
        ConvertTo-Json -Compress
    $process.StandardInput.Write($payload)
    $process.StandardInput.Close()
    $exited = $process.WaitForExit(900)
    if (-not $exited) { Stop-OwnedProcessTree -Process $process }
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $watch.Stop()
    return [pscustomobject]@{
        milliseconds = [Math]::Round([Math]::Max($watch.Elapsed.TotalMilliseconds, $(if ($exited) { 0 } else { 900 })), 3)
        success = $exited -and $process.ExitCode -eq 0 -and $stdout.Length -eq 0 -and $stderr.Length -eq 0
        timeout = -not $exited
    }
}

[System.IO.Directory]::CreateDirectory($StateRoot) | Out-Null
$warmState = Join-Path $StateRoot 'warm'
$warmServer = Start-McpServer -State $warmState -TerminalToken '00000000-0000-0000-0000-000000000052'
Initialize-McpSession -Server $warmServer -Session 'warm-session' -Turn 'warm-turn' -Cwd $Workspace
$warm = @()
for ($index = 1; $index -le $WarmSamples; $index++) {
    $warm += Complete-McpCall -Pending (Start-McpCall -Server $warmServer -Event 'PostToolUse' -Session 'warm-session' -Turn 'warm-turn' -Cwd $Workspace)
}
Stop-OwnedProcessTree -Process $warmServer.process

$c8Servers = @()
for ($slot = 1; $slot -le 8; $slot++) {
    $state = Join-Path $StateRoot "c8-$slot"
    $token = '00000000-0000-0000-0000-{0:D12}' -f (1000 + $slot)
    $server = Start-McpServer -State $state -TerminalToken $token
    Initialize-McpSession -Server $server -Session "c8-session-$slot" -Turn "c8-turn-$slot" -Cwd $Workspace
    $c8Servers += $server
}
$c8 = @()
for ($round = 1; $round -le $ConcurrencyRounds; $round++) {
    $pending = @()
    for ($slot = 0; $slot -lt $c8Servers.Count; $slot++) {
        $pending += Start-McpCall -Server $c8Servers[$slot] -Event 'PostToolUse' -Session "c8-session-$($slot + 1)" -Turn "c8-turn-$($slot + 1)" -Cwd $Workspace
    }
    foreach ($call in $pending) { $c8 += Complete-McpCall -Pending $call }
}
foreach ($server in $c8Servers) { Stop-OwnedProcessTree -Process $server.process }

$sessionEnd = @()
for ($index = 1; $index -le $SessionEndSamples; $index++) {
    $state = Join-Path $StateRoot "session-end-$index"
    $token = '00000000-0000-0000-0000-{0:D12}' -f (2000 + $index)
    $session = "session-end-$index"
    $server = Start-McpServer -State $state -TerminalToken $token
    Initialize-McpSession -Server $server -Session $session -Turn "session-end-turn-$index" -Cwd $Workspace -PrepareWorking $false
    # Mirrors LocalStdioServerTransport::close: terminate child before its
    # transport/stdio close, then deliver SessionEnd independently.
    Stop-OwnedProcessTree -Process $server.process
    $sessionEnd += Invoke-SessionEndCommand -State $state -Session $session -TerminalToken $token
}

$report = [ordered]@{
    schema = 'tabbeacon-codex-mcp-hybrid-runtime-measurement-v1'
    expected_head = $ExpectedHead
    checked_out_head = $checkedOutHead
    binary_sha256 = $binarySha256
    transport = '10_mcp_plus_1_session_end_command'
    normal_event_shell_process_count = 0
    session_end_command_process_count = 1
    session_end_timeout_ms = 1000
    session_end_async = $false
    real_codex_terminate_before_eof_model = $true
    eof_authoritative = $false
    warm = Get-Statistics -Samples $warm
    concurrency_8 = Get-Statistics -Samples $c8
    session_end = Get-Statistics -Samples $sessionEnd
}

$outputParent = Split-Path -Parent $OutputPath
if (-not [string]::IsNullOrWhiteSpace($outputParent)) {
    [System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
}
[System.IO.File]::WriteAllText(
    $OutputPath,
    ($report | ConvertTo-Json -Depth 6),
    [System.Text.UTF8Encoding]::new($false)
)
$report | ConvertTo-Json -Depth 6
