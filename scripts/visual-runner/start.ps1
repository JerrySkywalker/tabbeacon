[CmdletBinding()]
param([string]$RunnerRoot)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot (Get-TabBeaconRunnerRoot $RunnerRoot)
$marker = Assert-TabBeaconOwnedRunner $RunnerRoot
$sessionId = Assert-TabBeaconInteractiveSession
$remoteBeforeStart = @(Get-TabBeaconRepositoryRunner $marker.runner_name)
$remoteStatus = if ($remoteBeforeStart.Count -eq 1) { $remoteBeforeStart[0].status } else { 'N/A' }
if ($remoteBeforeStart.Count -ne 1 -or $remoteStatus -ne 'offline') {
    throw "Refusing to start while the named remote runner is not proven offline: count=$($remoteBeforeStart.Count) status=$remoteStatus"
}
$statePath = Get-TabBeaconStatePath $RunnerRoot
$state = Read-TabBeaconJson $statePath
if ($null -ne $state) {
    $runnerHostProcess = Get-Process -Id $state.host_process_id -ErrorAction SilentlyContinue
    if ($null -ne $runnerHostProcess) { throw "Owned visual runner host is already active: PID=$($runnerHostProcess.Id)" }
    Remove-Item -LiteralPath $statePath -Force
}

$hostStdout = Join-Path $RunnerRoot 'runner-host.stdout.log'
$hostStderr = Join-Path $RunnerRoot 'runner-host.stderr.log'
$runnerHostProcess = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'runner-host.ps1'), '-RunnerRoot', $RunnerRoot) -WindowStyle Hidden -RedirectStandardOutput $hostStdout -RedirectStandardError $hostStderr -PassThru
$remote = Wait-TabBeaconRunnerOnline -RunnerName $marker.runner_name -TimeoutSeconds 120
if ($null -eq $remote) { throw "Owned visual runner did not become online within the bounded 120-second wait; host PID=$($runnerHostProcess.Id)" }
$labels = @($remote.labels | ForEach-Object { $_.name })
if ($labels -notcontains 'tabbeacon-visual') { throw "Online runner lacks required tabbeacon-visual label: $($labels -join ',')" }
[ordered]@{ status = 'ONLINE'; runner_name = $remote.name; runner_id = $remote.id; labels = $labels; session_id = $sessionId; host_process_id = $runnerHostProcess.Id; mode = 'INTERACTIVE_USER_SESSION' } | ConvertTo-Json -Compress
