param([Parameter(Mandatory = $true)][string]$RunnerRoot)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot $RunnerRoot
$marker = Assert-TabBeaconOwnedRunner $RunnerRoot
$sessionId = Assert-TabBeaconInteractiveSession

if (-not ('TabBeaconExecutionState' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class TabBeaconExecutionState {
  [DllImport("kernel32.dll", SetLastError=true)] public static extern uint SetThreadExecutionState(uint flags);
}
'@
}

$continuous = 0x80000000
$systemRequired = 0x00000001
$displayRequired = 0x00000002
$listener = $null
try {
    [void][TabBeaconExecutionState]::SetThreadExecutionState($continuous -bor $systemRequired -bor $displayRequired)
    $listenerPath = Join-Path $RunnerRoot 'bin\Runner.Listener.exe'
    $listener = Start-Process -FilePath $listenerPath -ArgumentList 'run' -WorkingDirectory $RunnerRoot -WindowStyle Hidden -PassThru
    Write-TabBeaconJson -Path (Get-TabBeaconStatePath $RunnerRoot) -Value ([ordered]@{
        schema = 'tabbeacon-visual-runner-state-v1'
        owner = 'TB-G03R'
        runner_name = $marker.runner_name
        host_process_id = $PID
        listener_process_id = $listener.Id
        session_id = $sessionId
        started_utc = [DateTime]::UtcNow.ToString('o')
        sleep_prevention = 'SetThreadExecutionState(ES_CONTINUOUS|ES_SYSTEM_REQUIRED|ES_DISPLAY_REQUIRED)'
    })
    $stopPath = Get-TabBeaconStopPath $RunnerRoot
    while (-not $listener.HasExited -and -not (Test-Path -LiteralPath $stopPath)) { Start-Sleep -Seconds 1 }
    if (-not $listener.HasExited -and (Test-Path -LiteralPath $stopPath)) { Stop-Process -Id $listener.Id -ErrorAction SilentlyContinue }
} finally {
    if ($null -ne $listener -and -not $listener.HasExited) { $listener.WaitForExit(5000) | Out-Null }
    $statePath = Get-TabBeaconStatePath $RunnerRoot
    $stopPath = Get-TabBeaconStopPath $RunnerRoot
    if (Test-Path -LiteralPath $statePath) { Remove-Item -LiteralPath $statePath -Force }
    if (Test-Path -LiteralPath $stopPath) { Remove-Item -LiteralPath $stopPath -Force }
    [void][TabBeaconExecutionState]::SetThreadExecutionState($continuous)
}
