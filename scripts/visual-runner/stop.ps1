[CmdletBinding()]
param([string]$RunnerRoot, [int]$TimeoutSeconds = 30)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot (Get-TabBeaconRunnerRoot $RunnerRoot)
$null = Assert-TabBeaconOwnedRunner $RunnerRoot
$statePath = Get-TabBeaconStatePath $RunnerRoot
$state = Read-TabBeaconJson $statePath
if ($null -eq $state) { [ordered]@{ status = 'ALREADY_STOPPED'; runner_root = $RunnerRoot } | ConvertTo-Json -Compress; return }
New-Item -ItemType File -Path (Get-TabBeaconStopPath $RunnerRoot) -Force | Out-Null
$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
do {
    $runnerHostProcess = Get-Process -Id $state.host_process_id -ErrorAction SilentlyContinue
    if ($null -eq $runnerHostProcess) {
        $remote = Wait-TabBeaconRunnerOffline -RunnerName $state.runner_name
        if ($null -eq $remote) { throw 'Owned runner host stopped but GitHub did not report the runner offline within the bounded wait' }
        [ordered]@{ status = 'STOPPED'; runner_root = $RunnerRoot; runner_name = $state.runner_name; remote_status = $remote.status } | ConvertTo-Json -Compress
        return
    }
    Start-Sleep -Seconds 1
} while ([DateTime]::UtcNow -lt $deadline)
throw "Owned visual runner host did not stop within $TimeoutSeconds seconds; no process was force-terminated"
