[CmdletBinding()]
param([string]$RunnerRoot)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot (Get-TabBeaconRunnerRoot $RunnerRoot)
$marker = Assert-TabBeaconOwnedRunner $RunnerRoot
$state = Read-TabBeaconJson (Get-TabBeaconStatePath $RunnerRoot)
$remote = @(Get-TabBeaconRepositoryRunner $marker.runner_name)
$hostAlive = $false
if ($null -ne $state) { $hostAlive = $null -ne (Get-Process -Id $state.host_process_id -ErrorAction SilentlyContinue) }
$labels = if ($remote.Count -eq 1) { @($remote[0].labels | ForEach-Object { $_.name }) } else { @() }
$disposition = if ($remote.Count -eq 1 -and $remote[0].status -eq 'online' -and $hostAlive -and $labels -contains 'tabbeacon-visual') { 'READY' } elseif ($remote.Count -le 1) { 'CONFIGURED_OR_OFFLINE' } else { 'AMBIGUOUS_REMOTE_REGISTRATION' }
[ordered]@{ disposition = $disposition; runner_name = $marker.runner_name; runner_root = $RunnerRoot; repository = $marker.repository; mode = $marker.mode; configured_session_id = $marker.configured_session_id; current_session_id = [Diagnostics.Process]::GetCurrentProcess().SessionId; runner_version = Get-TabBeaconRunnerVersion $RunnerRoot; host_alive = $hostAlive; remote_count = $remote.Count; remote_status = if ($remote.Count -eq 1) { $remote[0].status } else { 'N/A' }; labels = $labels } | ConvertTo-Json -Compress
