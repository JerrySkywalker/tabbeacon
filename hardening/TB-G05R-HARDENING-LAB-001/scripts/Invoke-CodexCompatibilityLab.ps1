[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path,
    [string[]]$Versions = @('0.147.0', '0.146.0', '0.145.0')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$tabBeacon = Join-Path $RepositoryRoot 'target\debug\tabbeacon.exe'
$packageRoot = Join-Path $RepositoryRoot 'target\g05r-lab\npm'
$runKey = Get-Date -Format 'yyyyMMdd-HHmmss'
$outputRoot = Join-Path $RepositoryRoot "target\g05r-lab\compatibility\$runKey"
$null = New-Item -ItemType Directory -Force -Path $outputRoot

if (-not (Test-Path -LiteralPath $tabBeacon -PathType Leaf)) {
    throw "TabBeacon binary is missing: $tabBeacon"
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string]$FilePath,
        [string[]]$ArgumentList = @(),
        [hashtable]$Environment = @{},
        [int]$TimeoutMilliseconds = 30000
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
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
    $startedAt = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw "failed to start $FilePath"
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutMilliseconds)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw "process timed out after ${TimeoutMilliseconds}ms: $FilePath"
    }
    $startedAt.Stop()
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
        DurationMilliseconds = $startedAt.ElapsedMilliseconds
    }
}

function Start-CodexAppServer {
    param(
        [Parameter(Mandatory)] [string]$CodexExecutable,
        [Parameter(Mandatory)] [hashtable]$Environment
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $CodexExecutable
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $null = $startInfo.ArgumentList.Add('app-server')
    $null = $startInfo.ArgumentList.Add('--listen')
    $null = $startInfo.ArgumentList.Add('stdio://')
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "failed to start Codex app-server: $CodexExecutable"
    }
    $process
}

function Send-RpcMessage {
    param(
        [Parameter(Mandatory)] [System.Diagnostics.Process]$Process,
        [Parameter(Mandatory)] [object]$Message
    )

    $json = $Message | ConvertTo-Json -Compress -Depth 40
    $Process.StandardInput.WriteLine($json)
    $Process.StandardInput.Flush()
}

function Read-RpcResponse {
    param(
        [Parameter(Mandatory)] [System.Diagnostics.Process]$Process,
        [Parameter(Mandatory)] [int]$Id,
        [int]$TimeoutMilliseconds = 15000
    )

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $remaining = [Math]::Max(1, [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds)
        $readTask = $Process.StandardOutput.ReadLineAsync()
        if (-not $readTask.Wait($remaining)) {
            throw "timed out waiting for JSON-RPC response id $Id"
        }
        $line = $readTask.GetAwaiter().GetResult()
        if ($null -eq $line) {
            $stderr = $Process.StandardError.ReadToEnd()
            throw "Codex app-server exited before response id ${Id}: $stderr"
        }
        try {
            $message = $line | ConvertFrom-Json -Depth 40
        } catch {
            continue
        }
        if ($message.PSObject.Properties.Name -contains 'id' -and [int]$message.id -eq $Id) {
            return $message
        }
    }
    throw "timed out waiting for JSON-RPC response id $Id"
}

function Stop-CodexAppServer {
    param([System.Diagnostics.Process]$Process)
    if ($null -eq $Process) {
        return
    }
    try {
        $Process.StandardInput.Close()
        if (-not $Process.WaitForExit(3000)) {
            $Process.Kill($true)
            $Process.WaitForExit()
        }
    } finally {
        $Process.Dispose()
    }
}

function Invoke-HooksList {
    param(
        [Parameter(Mandatory)] [System.Diagnostics.Process]$Process,
        [Parameter(Mandatory)] [string]$Cwd,
        [int]$Id
    )
    Send-RpcMessage -Process $Process -Message ([ordered]@{
        method = 'hooks/list'
        id = $Id
        params = @{ cwds = @($Cwd) }
    })
    Read-RpcResponse -Process $Process -Id $Id
}

$results = @()
foreach ($version in $Versions) {
    $caseRoot = Join-Path $outputRoot $version
    $labCodexHome = Join-Path $caseRoot 'codex-home'
    $labLocalAppData = Join-Path $caseRoot 'local-app-data'
    $null = New-Item -ItemType Directory -Force -Path $labCodexHome, $labLocalAppData

    $codexExecutable = Get-ChildItem -LiteralPath (Join-Path $packageRoot "$version\node_modules\@openai") -Recurse -Filter codex.exe |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $codexExecutable) {
        $results += [pscustomobject]@{
            version = $version
            disposition = 'UNOBTAINABLE'
            error = 'native codex.exe was not found'
        }
        continue
    }

    $configPath = Join-Path $labCodexHome 'config.toml'
    $hooksPath = Join-Path $labCodexHome 'hooks.json'
    $seedConfig = @'
# g05r-preserve-comment
model = "fixture-model"
unknown_future_scalar = "preserve-me"

[features]
hooks = true

[tui]
animations = false
terminal_title = ["project"]
status_line = ["model-with-reasoning", "current-dir"]

[profiles.g05r]
model = "fixture-profile-model"

[mcp_servers.g05r_fixture]
command = "missing-g05r-mcp-command"
args = ["--never-run"]

[notice]
hide_full_access_warning = true

[future_g05r]
opaque = "preserve-me-too"
'@
    $seedHooks = @'
{
  "description": "G05R preservation fixture",
  "hooks": {
    "PreCompact": [
      {
        "matcher": "manual",
        "hooks": [
          {
            "type": "command",
            "command": "exit 0",
            "commandWindows": "cmd.exe /d /s /c exit /b 0",
            "timeout": 1
          }
        ]
      }
    ]
  }
}
'@
    [System.IO.File]::WriteAllText($configPath, $seedConfig, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($hooksPath, $seedHooks, [System.Text.UTF8Encoding]::new($false))

    $codexBinDirectory = Split-Path -Parent $codexExecutable
    $caseEnvironment = @{
        CODEX_HOME = $labCodexHome
        LOCALAPPDATA = $labLocalAppData
        PATH = "$codexBinDirectory;$env:PATH"
        NO_COLOR = '1'
    }
    $case = [ordered]@{
        version = $version
        codexExecutable = $codexExecutable
        versionProbe = $null
        setup = $null
        repeatedSetup = $null
        discovery = 'NOT_EXECUTED'
        discoveredCount = 0
        tabBeaconHookCount = 0
        tabBeaconTrust = @()
        trustWrite = 'NOT_EXECUTED'
        trustedTabBeaconHookCount = 0
        unrelatedHookTrust = $null
        doctorAfterTrust = $null
        uninstall = $null
        preservation = 'UNPROVEN'
        reinstall = $null
        titleConfigAccepted = $false
        disposition = 'UNPROVEN'
        error = $null
    }

    try {
        $case.versionProbe = Invoke-CapturedProcess -FilePath $codexExecutable -ArgumentList @('--version') -Environment $caseEnvironment
        $case.setup = Invoke-CapturedProcess -FilePath $tabBeacon -ArgumentList @('setup', 'codex') -Environment $caseEnvironment
        $case.repeatedSetup = Invoke-CapturedProcess -FilePath $tabBeacon -ArgumentList @('setup', 'codex') -Environment $caseEnvironment

        $appServer = $null
        try {
            $appServer = Start-CodexAppServer -CodexExecutable $codexExecutable -Environment $caseEnvironment
            Send-RpcMessage -Process $appServer -Message ([ordered]@{
                method = 'initialize'
                id = 1
                params = @{
                    clientInfo = @{ name = 'tabbeacon_g05r_lab'; title = 'TabBeacon G05R Lab'; version = '0.1.0' }
                    capabilities = @{ experimentalApi = $true }
                }
            })
            $initialize = Read-RpcResponse -Process $appServer -Id 1
            if ($initialize.PSObject.Properties.Name -contains 'error') {
                throw "initialize failed: $($initialize.error | ConvertTo-Json -Compress -Depth 10)"
            }
            Send-RpcMessage -Process $appServer -Message @{ method = 'initialized' }

            $list = Invoke-HooksList -Process $appServer -Cwd $RepositoryRoot -Id 2
            if ($list.PSObject.Properties.Name -contains 'error') {
                throw "hooks/list failed: $($list.error | ConvertTo-Json -Compress -Depth 10)"
            }
            $hooks = @($list.result.data[0].hooks)
            $case.discovery = 'PASS'
            $case.discoveredCount = $hooks.Count
            $ownedHooks = @($hooks | Where-Object { $_.command -like '*tabbeacon.exe*' })
            $case.tabBeaconHookCount = $ownedHooks.Count
            $case.tabBeaconTrust = @($ownedHooks | Select-Object key, eventName, currentHash, trustStatus, enabled, command)
            $case.titleConfigAccepted = (Get-Content -LiteralPath $configPath -Raw) -match 'terminal_title\s*=\s*\[\s*\]'

            $state = [ordered]@{}
            foreach ($hook in $ownedHooks) {
                $state[$hook.key] = @{ trusted_hash = $hook.currentHash }
            }
            Send-RpcMessage -Process $appServer -Message ([ordered]@{
                method = 'config/batchWrite'
                id = 3
                params = @{
                    edits = @(@{
                        keyPath = 'hooks.state'
                        value = $state
                        mergeStrategy = 'upsert'
                    })
                    reloadUserConfig = $true
                }
            })
            $trustWrite = Read-RpcResponse -Process $appServer -Id 3
            if ($trustWrite.PSObject.Properties.Name -contains 'error') {
                throw "config/batchWrite failed: $($trustWrite.error | ConvertTo-Json -Compress -Depth 10)"
            }
            $case.trustWrite = 'PASS'

            $trustedList = Invoke-HooksList -Process $appServer -Cwd $RepositoryRoot -Id 4
            if ($trustedList.PSObject.Properties.Name -contains 'error') {
                throw "trusted hooks/list failed: $($trustedList.error | ConvertTo-Json -Compress -Depth 10)"
            }
            $trustedHooks = @($trustedList.result.data[0].hooks)
            $case.trustedTabBeaconHookCount = @($trustedHooks | Where-Object {
                $_.command -like '*tabbeacon.exe*' -and $_.trustStatus -eq 'trusted'
            }).Count
            $case.unrelatedHookTrust = ($trustedHooks | Where-Object {
                $_.command -eq 'cmd.exe /d /s /c exit /b 0'
            } | Select-Object -First 1 -ExpandProperty trustStatus)
        } finally {
            Stop-CodexAppServer -Process $appServer
        }

        $case.doctorAfterTrust = Invoke-CapturedProcess -FilePath $tabBeacon -ArgumentList @('doctor') -Environment $caseEnvironment
        $case.uninstall = Invoke-CapturedProcess -FilePath $tabBeacon -ArgumentList @('uninstall', 'codex') -Environment $caseEnvironment

        $afterUninstallConfig = Get-Content -LiteralPath $configPath -Raw
        $afterUninstallHooks = Get-Content -LiteralPath $hooksPath -Raw | ConvertFrom-Json -Depth 40
        $preservedComment = $afterUninstallConfig.Contains('# g05r-preserve-comment')
        $preservedUnknown = $afterUninstallConfig.Contains('unknown_future_scalar = "preserve-me"')
        $restoredTitle = $afterUninstallConfig -match 'terminal_title\s*=\s*\[\s*"project"\s*\]'
        $preservedMcp = $afterUninstallConfig.Contains('[mcp_servers.g05r_fixture]')
        $remainingCommands = @($afterUninstallHooks.hooks.PreCompact[0].hooks | ForEach-Object { $_.commandWindows })
        $unrelatedHookPreserved = $remainingCommands -contains 'cmd.exe /d /s /c exit /b 0'
        $ownedHookRemoved = -not ((Get-Content -LiteralPath $hooksPath -Raw).Contains('tabbeacon.exe'))
        if ($preservedComment -and $preservedUnknown -and $restoredTitle -and $preservedMcp -and $unrelatedHookPreserved -and $ownedHookRemoved) {
            $case.preservation = 'PASS'
        } else {
            $case.preservation = 'FAIL'
        }

        $case.reinstall = Invoke-CapturedProcess -FilePath $tabBeacon -ArgumentList @('setup', 'codex') -Environment $caseEnvironment
        if ($case.versionProbe.ExitCode -eq 0 -and
            $case.setup.ExitCode -eq 0 -and
            $case.repeatedSetup.ExitCode -eq 0 -and
            $case.discovery -eq 'PASS' -and
            $case.tabBeaconHookCount -eq 7 -and
            $case.trustWrite -eq 'PASS' -and
            $case.trustedTabBeaconHookCount -eq 7 -and
            $case.unrelatedHookTrust -eq 'untrusted' -and
            $case.preservation -eq 'PASS' -and
            $case.reinstall.ExitCode -eq 0) {
            $case.disposition = if ($version -eq '0.147.0') { 'PASS' } else { 'OUTSIDE_DECLARED_SUPPORT' }
        } else {
            $case.disposition = 'FAIL'
        }
    } catch {
        $case.error = $_.Exception.Message
        if ($version -ne '0.147.0') {
            $case.disposition = 'OUTSIDE_DECLARED_SUPPORT'
        } else {
            $case.disposition = 'FAIL'
        }
    }

    $resultPath = Join-Path $caseRoot 'result.json'
    [System.IO.File]::WriteAllText(
        $resultPath,
        ($case | ConvertTo-Json -Depth 30),
        [System.Text.UTF8Encoding]::new($false)
    )
    $results += [pscustomobject]$case
}

$summary = [ordered]@{
    runKey = $runKey
    outputRoot = $outputRoot
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    results = $results
}
$summaryPath = Join-Path $outputRoot 'summary.json'
[System.IO.File]::WriteAllText(
    $summaryPath,
    ($summary | ConvertTo-Json -Depth 40),
    [System.Text.UTF8Encoding]::new($false)
)
$summary | ConvertTo-Json -Depth 12
