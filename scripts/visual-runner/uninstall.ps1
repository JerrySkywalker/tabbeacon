[CmdletBinding()]
param([string]$RunnerRoot, [switch]$Execute)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot (Get-TabBeaconRunnerRoot $RunnerRoot)
$marker = Assert-TabBeaconOwnedRunner $RunnerRoot
$state = Read-TabBeaconJson (Get-TabBeaconStatePath $RunnerRoot)
if ($null -ne $state) { throw 'Stop the positively owned runner before uninstalling it' }
if (-not $Execute) { [ordered]@{ status = 'DRY_RUN'; runner_name = $marker.runner_name; runner_root = $RunnerRoot; remote_remove = 'would request ephemeral remove token and run config.cmd remove'; local_remove = 'would remove only marker-proven runner root' } | ConvertTo-Json -Compress; return }

$removeToken = $null
try {
    $removeToken = (gh api --method POST "repos/$script:TabBeaconRepository/actions/runners/remove-token" | ConvertFrom-Json).token
    if ([string]::IsNullOrWhiteSpace($removeToken)) { throw 'GitHub did not return a remove token' }
    $output = @(& (Join-Path $RunnerRoot 'config.cmd') remove --token $removeToken 2>&1)
    $log = Write-TabBeaconRedactedLog -RunnerRoot $RunnerRoot -Name 'uninstall' -Lines $output -SensitiveValue $removeToken
    if ($LASTEXITCODE -ne 0) { throw "Owned runner removal failed; inspect redacted log: $log" }
} finally {
    $removeToken = $null
}
Remove-Item -LiteralPath $RunnerRoot -Recurse -Force
[ordered]@{ status = 'REMOVED'; runner_name = $marker.runner_name; runner_root = $RunnerRoot } | ConvertTo-Json -Compress
