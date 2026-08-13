[CmdletBinding()]
param([string]$RunnerRoot)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot (Get-TabBeaconRunnerRoot $RunnerRoot)
$marker = Assert-TabBeaconOwnedRunner $RunnerRoot
$sessionId = Assert-TabBeaconInteractiveSession
$statePath = Get-TabBeaconStatePath $RunnerRoot
$state = Read-TabBeaconJson $statePath
if ($null -ne $state) {
    $host = Get-Process -Id $state.host_process_id -ErrorAction SilentlyContinue
    if ($null -ne $host) { throw "Owned visual runner host is already active: PID=$($host.Id)" }
    Remove-Item -LiteralPath $statePath -Force
}

$host = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'runner-host.ps1'), '-RunnerRoot', $RunnerRoot) -WindowStyle Hidden -PassThru
$remote = Wait-TabBeaconRunnerOnline -RunnerName $marker.runner_name
if ($null -eq $remote) { throw "Owned visual runner did not become online within the bounded wait; host PID=$($host.Id)" }
$labels = @($remote.labels | ForEach-Object { $_.name })
if ($labels -notcontains 'tabbeacon-visual') { throw "Online runner lacks required tabbeacon-visual label: $($labels -join ',')" }
[ordered]@{ status = 'ONLINE'; runner_name = $remote.name; runner_id = $remote.id; labels = $labels; session_id = $sessionId; host_process_id = $host.Id; mode = 'INTERACTIVE_USER_SESSION' } | ConvertTo-Json -Compress
