[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$tabBeacon = Join-Path $RepositoryRoot 'target\debug\tabbeacon.exe'
$codexExecutable = Get-ChildItem -LiteralPath (Join-Path $RepositoryRoot 'target\g05r-lab\npm\0.147.0\node_modules\@openai') -Recurse -Filter codex.exe |
    Select-Object -First 1 -ExpandProperty FullName
$runKey = Get-Date -Format 'yyyyMMdd-HHmmss'
$outputRoot = Join-Path $RepositoryRoot "target\g05r-lab\trust-forensics\$runKey"
$labCodexHome = Join-Path $outputRoot 'codex-home'
$labLocalAppData = Join-Path $outputRoot 'local-app-data'
$null = New-Item -ItemType Directory -Force -Path $labCodexHome, $labLocalAppData
$utf8 = [System.Text.UTF8Encoding]::new($false)
$caseEnvironment = @{
    CODEX_HOME = $labCodexHome
    LOCALAPPDATA = $labLocalAppData
    PATH = "$(Split-Path -Parent $codexExecutable);$env:PATH"
    NO_COLOR = '1'
}

function Invoke-CapturedProcess {
    param([string[]]$ArgumentList)
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $tabBeacon
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $ArgumentList) {
        $null = $startInfo.ArgumentList.Add($argument)
    }
    foreach ($entry in $caseEnvironment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) { throw 'failed to start TabBeacon' }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(15000)) {
        $process.Kill($true)
        $process.WaitForExit()
        throw 'TabBeacon process timed out'
    }
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
    }
}

function Start-AppServer {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $codexExecutable
    $startInfo.WorkingDirectory = $RepositoryRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $null = $startInfo.ArgumentList.Add('app-server')
    $null = $startInfo.ArgumentList.Add('--listen')
    $null = $startInfo.ArgumentList.Add('stdio://')
    foreach ($entry in $caseEnvironment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) { throw 'failed to start Codex app-server' }
    $process
}

function Send-Rpc {
    param([System.Diagnostics.Process]$Process, [object]$Message)
    $Process.StandardInput.WriteLine(($Message | ConvertTo-Json -Compress -Depth 40))
    $Process.StandardInput.Flush()
}

function Read-Rpc {
    param([System.Diagnostics.Process]$Process, [int]$Id)
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        $remaining = [Math]::Max(1, [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds)
        $task = $Process.StandardOutput.ReadLineAsync()
        if (-not $task.Wait($remaining)) { throw "RPC $Id timed out" }
        $line = $task.GetAwaiter().GetResult()
        if ($null -eq $line) { throw "app-server exited before RPC $Id" }
        try { $message = $line | ConvertFrom-Json -Depth 40 } catch { continue }
        if ($message.PSObject.Properties.Name -contains 'id' -and [int]$message.id -eq $Id) {
            return $message
        }
    }
    throw "RPC $Id timed out"
}

function Initialize-AppServer {
    param([System.Diagnostics.Process]$Process)
    Send-Rpc $Process ([ordered]@{
        method = 'initialize'
        id = 1
        params = @{
            clientInfo = @{ name = 'tabbeacon_g05r_trust_lab'; title = 'TabBeacon G05R Trust Lab'; version = '0.1.0' }
            capabilities = @{ experimentalApi = $true }
        }
    })
    $response = Read-Rpc $Process 1
    if ($response.PSObject.Properties.Name -contains 'error') {
        throw "initialize failed: $($response.error | ConvertTo-Json -Compress)"
    }
    Send-Rpc $Process @{ method = 'initialized' }
}

function Stop-AppServer {
    param([System.Diagnostics.Process]$Process)
    if ($null -eq $Process) { return }
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

function Get-HooksList {
    $process = $null
    try {
        $process = Start-AppServer
        Initialize-AppServer $process
        Send-Rpc $process ([ordered]@{ method = 'hooks/list'; id = 2; params = @{ cwds = @($RepositoryRoot) } })
        $response = Read-Rpc $process 2
        if ($response.PSObject.Properties.Name -contains 'error') {
            throw "hooks/list failed: $($response.error | ConvertTo-Json -Compress)"
        }
        @($response.result.data[0].hooks)
    } finally {
        Stop-AppServer $process
    }
}

function Get-OwnedPromptHook {
    param([object[]]$Hooks)
    $Hooks | Where-Object { $_.eventName -eq 'userPromptSubmit' -and $_.command -like '*tabbeacon.exe*' } | Select-Object -First 1
}

function Write-HooksDocument {
    param([object]$Document)
    [System.IO.File]::WriteAllText(
        (Join-Path $labCodexHome 'hooks.json'),
        ($Document | ConvertTo-Json -Depth 40),
        $utf8
    )
}

$setup = Invoke-CapturedProcess @('setup', 'codex')
$hooksPath = Join-Path $labCodexHome 'hooks.json'
$originalHooksBytes = [System.IO.File]::ReadAllBytes($hooksPath)
$initialHooks = Get-HooksList
$ownedHooks = @($initialHooks | Where-Object command -like '*tabbeacon.exe*')
if ($ownedHooks.Count -ne 7) { throw "expected seven owned hooks, observed $($ownedHooks.Count)" }

# Use Codex's documented app-server write surface to simulate the exact trust
# state that the TUI writes. This remains inside the isolated home.
$process = $null
try {
    $process = Start-AppServer
    Initialize-AppServer $process
    Send-Rpc $process ([ordered]@{ method = 'hooks/list'; id = 2; params = @{ cwds = @($RepositoryRoot) } })
    $list = Read-Rpc $process 2
    $state = [ordered]@{}
    foreach ($hook in @($list.result.data[0].hooks | Where-Object command -like '*tabbeacon.exe*')) {
        $state[$hook.key] = @{ trusted_hash = $hook.currentHash }
    }
    Send-Rpc $process ([ordered]@{
        method = 'config/batchWrite'
        id = 3
        params = @{
            edits = @(@{ keyPath = 'hooks.state'; value = $state; mergeStrategy = 'upsert' })
            reloadUserConfig = $true
        }
    })
    $write = Read-Rpc $process 3
    if ($write.PSObject.Properties.Name -contains 'error') {
        throw "trust write failed: $($write.error | ConvertTo-Json -Compress)"
    }
} finally {
    Stop-AppServer $process
}

$baseline = Get-OwnedPromptHook (Get-HooksList)
$results = @()
$results += [pscustomobject]@{
    name = 'baseline-trusted'
    key = $baseline.key
    hash = $baseline.currentHash
    trustStatus = $baseline.trustStatus
    enabled = $baseline.enabled
    disposition = if ($baseline.trustStatus -eq 'trusted' -and $baseline.enabled) { 'PASS' } else { 'FAIL' }
}

# Whitespace/property formatting does not enter the normalized trust identity.
$document = Get-Content -LiteralPath $hooksPath -Raw | ConvertFrom-Json -Depth 40
Write-HooksDocument $document
$formatted = Get-OwnedPromptHook (Get-HooksList)
$results += [pscustomobject]@{
    name = 'json-formatting-only'
    key = $formatted.key
    hash = $formatted.currentHash
    trustStatus = $formatted.trustStatus
    enabled = $formatted.enabled
    disposition = if ($formatted.currentHash -eq $baseline.currentHash -and $formatted.trustStatus -eq 'trusted') { 'PASS' } else { 'FAIL' }
}

# Windows selects commandWindows before hashing. Changing only the Unix command
# retains Codex trust, while TabBeacon's ownership doctor must still refuse the
# modified owned declaration.
$document.hooks.UserPromptSubmit[0].hooks[0].command = 'unix-only-command-changed'
Write-HooksDocument $document
$unixChanged = Get-OwnedPromptHook (Get-HooksList)
$doctorUnixChanged = Invoke-CapturedProcess @('doctor')
$results += [pscustomobject]@{
    name = 'unix-command-only-on-windows'
    key = $unixChanged.key
    hash = $unixChanged.currentHash
    trustStatus = $unixChanged.trustStatus
    enabled = $unixChanged.enabled
    disposition = if ($unixChanged.currentHash -eq $baseline.currentHash -and $unixChanged.trustStatus -eq 'trusted' -and $doctorUnixChanged.Stdout -match 'hooks.declarations STATUS=FAIL') { 'PASS' } else { 'FAIL' }
}
[System.IO.File]::WriteAllBytes($hooksPath, $originalHooksBytes)

# commandWindows is the executable identity on Windows and must invalidate trust.
$document = Get-Content -LiteralPath $hooksPath -Raw | ConvertFrom-Json -Depth 40
$document.hooks.UserPromptSubmit[0].hooks[0].commandWindows += ' '
Write-HooksDocument $document
$windowsChanged = Get-OwnedPromptHook (Get-HooksList)
$results += [pscustomobject]@{
    name = 'windows-command-change'
    key = $windowsChanged.key
    hash = $windowsChanged.currentHash
    trustStatus = $windowsChanged.trustStatus
    enabled = $windowsChanged.enabled
    disposition = if ($windowsChanged.currentHash -ne $baseline.currentHash -and $windowsChanged.trustStatus -eq 'modified') { 'PASS' } else { 'FAIL' }
}
[System.IO.File]::WriteAllBytes($hooksPath, $originalHooksBytes)

# A duplicate handler receives a new positional key and no inherited trust.
$document = Get-Content -LiteralPath $hooksPath -Raw | ConvertFrom-Json -Depth 40
$clone = $document.hooks.UserPromptSubmit[0].hooks[0] | ConvertTo-Json -Depth 20 | ConvertFrom-Json -Depth 20
$document.hooks.UserPromptSubmit[0].hooks = @($document.hooks.UserPromptSubmit[0].hooks) + @($clone)
Write-HooksDocument $document
$duplicates = @(Get-HooksList | Where-Object { $_.eventName -eq 'userPromptSubmit' -and $_.command -like '*tabbeacon.exe*' })
$results += [pscustomobject]@{
    name = 'duplicate-handler'
    key = ($duplicates.key -join ';')
    hash = ($duplicates.currentHash -join ';')
    trustStatus = ($duplicates.trustStatus -join ';')
    enabled = ($duplicates.enabled -join ';')
    disposition = if ($duplicates.Count -eq 2 -and $duplicates[0].trustStatus -eq 'trusted' -and $duplicates[1].trustStatus -eq 'untrusted') { 'PASS' } else { 'FAIL' }
}
[System.IO.File]::WriteAllBytes($hooksPath, $originalHooksBytes)

# Inserting a matcher group before an owned group changes positional keys. The
# new group is modified against the old index; the moved owned group is new.
$document = Get-Content -LiteralPath $hooksPath -Raw | ConvertFrom-Json -Depth 40
$newGroup = [pscustomobject]@{
    matcher = 'inserted'
    hooks = @([pscustomobject]@{
        type = 'command'
        command = 'exit 0'
        commandWindows = 'cmd.exe /d /s /c exit /b 0'
        timeout = 1
        async = $false
    })
}
$document.hooks.UserPromptSubmit = @($newGroup) + @($document.hooks.UserPromptSubmit)
Write-HooksDocument $document
$reordered = @(Get-HooksList | Where-Object eventName -eq 'userPromptSubmit')
$movedOwned = $reordered | Where-Object command -like '*tabbeacon.exe*' | Select-Object -First 1
$inserted = $reordered | Where-Object command -eq 'cmd.exe /d /s /c exit /b 0' | Select-Object -First 1
$results += [pscustomobject]@{
    name = 'matcher-group-insertion'
    key = "$($inserted.key);$($movedOwned.key)"
    hash = "$($inserted.currentHash);$($movedOwned.currentHash)"
    trustStatus = "$($inserted.trustStatus);$($movedOwned.trustStatus)"
    enabled = "$($inserted.enabled);$($movedOwned.enabled)"
    disposition = if ($inserted.trustStatus -eq 'modified' -and $movedOwned.trustStatus -eq 'untrusted') { 'PASS' } else { 'FAIL' }
}
[System.IO.File]::WriteAllBytes($hooksPath, $originalHooksBytes)

# Disable one already trusted hook through the supported state surface. Trust
# remains a content claim; enabled=false prevents execution.
$process = $null
try {
    $process = Start-AppServer
    Initialize-AppServer $process
    Send-Rpc $process ([ordered]@{
        method = 'config/batchWrite'
        id = 2
        params = @{
            edits = @(@{ keyPath = 'hooks.state'; value = @{ $($baseline.key) = @{ enabled = $false } }; mergeStrategy = 'upsert' })
            reloadUserConfig = $true
        }
    })
    $disableWrite = Read-Rpc $process 2
    if ($disableWrite.PSObject.Properties.Name -contains 'error') { throw 'disable write failed' }
} finally {
    Stop-AppServer $process
}
$disabled = Get-OwnedPromptHook (Get-HooksList)
$doctorDisabled = Invoke-CapturedProcess @('doctor')
$results += [pscustomobject]@{
    name = 'trusted-but-disabled'
    key = $disabled.key
    hash = $disabled.currentHash
    trustStatus = $disabled.trustStatus
    enabled = $disabled.enabled
    doctorExitCode = $doctorDisabled.ExitCode
    doctorOutput = $doctorDisabled.Stdout.Trim()
    disposition = if (
        $disabled.trustStatus -eq 'trusted' -and
        -not $disabled.enabled -and
        $doctorDisabled.ExitCode -ne 0 -and
        $doctorDisabled.Stdout -match 'hooks.trust STATUS=FAIL'
    ) { 'PASS' } else { 'FAIL' }
}

# Removing the hook removes its discovery entry but does not silently delete
# user trust state from config.toml.
$document = Get-Content -LiteralPath $hooksPath -Raw | ConvertFrom-Json -Depth 40
$document.hooks.PSObject.Properties.Remove('UserPromptSubmit')
Write-HooksDocument $document
$afterRemoval = @(Get-HooksList | Where-Object eventName -eq 'userPromptSubmit')
$configAfterRemoval = Get-Content -LiteralPath (Join-Path $labCodexHome 'config.toml') -Raw
$results += [pscustomobject]@{
    name = 'removed-hook-state-retained'
    key = $baseline.key
    hash = $baseline.currentHash
    trustStatus = 'not-discovered'
    enabled = $null
    disposition = if ($afterRemoval.Count -eq 0 -and $configAfterRemoval.Contains($baseline.key)) { 'PASS' } else { 'FAIL' }
}

$helpStartInfo = [System.Diagnostics.ProcessStartInfo]::new()
$helpStartInfo.FileName = $codexExecutable
$helpStartInfo.UseShellExecute = $false
$helpStartInfo.CreateNoWindow = $true
$helpStartInfo.RedirectStandardOutput = $true
$helpStartInfo.RedirectStandardError = $true
$null = $helpStartInfo.ArgumentList.Add('--help')
$helpProcess = [System.Diagnostics.Process]::Start($helpStartInfo)
$helpText = $helpProcess.StandardOutput.ReadToEnd() + $helpProcess.StandardError.ReadToEnd()
$helpProcess.WaitForExit()
$bypassDocumentedByBinary = $helpText.Contains('--dangerously-bypass-hook-trust')

$summary = [ordered]@{
    runKey = $runKey
    outputRoot = $outputRoot
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    supportedPersistentTrustWriteTested = $true
    oneOffBypassAdvertisedByBinary = $bypassDocumentedByBinary
    ownerTrustBypassed = $false
    results = $results
    overall = if (@($results | Where-Object disposition -ne 'PASS').Count -eq 0) { 'PASS' } else { 'FAIL' }
}
$summaryPath = Join-Path $outputRoot 'summary.json'
[System.IO.File]::WriteAllText($summaryPath, ($summary | ConvertTo-Json -Depth 20), $utf8)
$summary | ConvertTo-Json -Depth 12
