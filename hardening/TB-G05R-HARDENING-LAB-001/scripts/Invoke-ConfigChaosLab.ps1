[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$tabBeacon = Join-Path $RepositoryRoot 'target\debug\tabbeacon.exe'
$runKey = Get-Date -Format 'yyyyMMdd-HHmmss'
$outputRoot = Join-Path $RepositoryRoot "target\g05r-lab\config-chaos\$runKey"
$null = New-Item -ItemType Directory -Force -Path $outputRoot
$utf8 = [System.Text.UTF8Encoding]::new($false)

function New-CaseEnvironment {
    param([Parameter(Mandatory)] [string]$Name)
    $caseRoot = Join-Path $outputRoot $Name
    $labCodexHome = Join-Path $caseRoot 'codex-home'
    $labLocalAppData = Join-Path $caseRoot 'local-app-data'
    $null = New-Item -ItemType Directory -Force -Path $labCodexHome, $labLocalAppData
    [pscustomobject]@{
        Name = $Name
        Root = $caseRoot
        CodexHome = $labCodexHome
        LocalAppData = $labLocalAppData
        StateRoot = Join-Path $labLocalAppData 'TabBeacon\codex-integration'
        Manifest = Join-Path $labLocalAppData 'TabBeacon\codex-integration\integration-v1.json'
        Environment = @{
            CODEX_HOME = $labCodexHome
            LOCALAPPDATA = $labLocalAppData
            NO_COLOR = '1'
        }
    }
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string[]]$ArgumentList,
        [Parameter(Mandatory)] [hashtable]$Environment,
        [int]$TimeoutMilliseconds = 15000
    )
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
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw 'failed to start TabBeacon'
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutMilliseconds)) {
        $process.Kill($true)
        $process.WaitForExit()
        return [pscustomobject]@{
            ExitCode = $null
            Stdout = $stdoutTask.GetAwaiter().GetResult()
            Stderr = $stderrTask.GetAwaiter().GetResult()
            TimedOut = $true
        }
    }
    [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.GetAwaiter().GetResult()
        Stderr = $stderrTask.GetAwaiter().GetResult()
        TimedOut = $false
    }
}

function Start-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string[]]$ArgumentList,
        [Parameter(Mandatory)] [hashtable]$Environment
    )
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
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string]$entry.Value
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw 'failed to start TabBeacon'
    }
    [pscustomobject]@{
        Process = $process
        StdoutTask = $process.StandardOutput.ReadToEndAsync()
        StderrTask = $process.StandardError.ReadToEndAsync()
    }
}

function Wait-CapturedProcess {
    param([Parameter(Mandatory)] [object]$Capture)
    if (-not $Capture.Process.WaitForExit(20000)) {
        $Capture.Process.Kill($true)
        $Capture.Process.WaitForExit()
        $timedOut = $true
    } else {
        $timedOut = $false
    }
    [pscustomobject]@{
        ExitCode = if ($timedOut) { $null } else { $Capture.Process.ExitCode }
        Stdout = $Capture.StdoutTask.GetAwaiter().GetResult()
        Stderr = $Capture.StderrTask.GetAwaiter().GetResult()
        TimedOut = $timedOut
    }
}

function Get-FileDigestOrAbsent {
    param([Parameter(Mandatory)] [string]$Path)
    if (Test-Path -LiteralPath $Path -PathType Leaf) {
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    } else {
        'ABSENT'
    }
}

function Get-HookCounts {
    param([Parameter(Mandatory)] [string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return [pscustomobject]@{ Total = 0; TabBeacon = 0 }
    }
    $document = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json -Depth 40
    $total = 0
    $owned = 0
    foreach ($eventProperty in $document.hooks.PSObject.Properties) {
        foreach ($group in @($eventProperty.Value)) {
            foreach ($handler in @($group.hooks)) {
                $total++
                if (($handler.commandWindows -as [string]) -like '*tabbeacon.exe*') {
                    $owned++
                }
            }
        }
    }
    [pscustomobject]@{ Total = $total; TabBeacon = $owned }
}

$results = @()

# Clean install and exact lifecycle.
$case = New-CaseEnvironment 'clean-roundtrip'
$setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$repeat = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$doctor = Invoke-CapturedProcess -ArgumentList @('doctor') -Environment $case.Environment
$uninstall = Invoke-CapturedProcess -ArgumentList @('uninstall', 'codex') -Environment $case.Environment
$results += [pscustomobject]@{
    name = $case.Name
    class = 'NORMAL'
    setupExit = $setup.ExitCode
    repeatedSetupExit = $repeat.ExitCode
    doctorExit = $doctor.ExitCode
    uninstallExit = $uninstall.ExitCode
    disposition = if ($setup.ExitCode -eq 0 -and $repeat.ExitCode -eq 0 -and $doctor.ExitCode -eq 0 -and $uninstall.ExitCode -eq 0 -and -not (Test-Path -LiteralPath $case.Manifest)) { 'PASS' } else { 'FAIL' }
    detail = 'empty config and hooks lifecycle'
}

# Corrupt hooks must fail before a manifest or mutation exists.
$case = New-CaseEnvironment 'corrupt-hooks'
$hooksPath = Join-Path $case.CodexHome 'hooks.json'
[System.IO.File]::WriteAllText($hooksPath, '{not-json', $utf8)
$before = Get-FileDigestOrAbsent $hooksPath
$setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$after = Get-FileDigestOrAbsent $hooksPath
$results += [pscustomobject]@{
    name = $case.Name
    class = 'FILESYSTEM'
    setupExit = $setup.ExitCode
    repeatedSetupExit = $null
    doctorExit = $null
    uninstallExit = $null
    disposition = if ($setup.ExitCode -ne 0 -and $before -eq $after -and -not (Test-Path -LiteralPath $case.Manifest)) { 'PASS' } else { 'FAIL' }
    detail = 'corrupt hooks rejected without mutation'
}

# A lookalike declaration cannot be claimed.
$case = New-CaseEnvironment 'unowned-lookalike'
$hooksPath = Join-Path $case.CodexHome 'hooks.json'
$lookalike = '{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"tabbeacon hook codex"}]}]}}'
[System.IO.File]::WriteAllText($hooksPath, $lookalike, $utf8)
$before = Get-FileDigestOrAbsent $hooksPath
$setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$after = Get-FileDigestOrAbsent $hooksPath
$results += [pscustomobject]@{
    name = $case.Name
    class = 'SECURITY_BOUNDARY'
    setupExit = $setup.ExitCode
    repeatedSetupExit = $null
    doctorExit = $null
    uninstallExit = $null
    disposition = if ($setup.ExitCode -ne 0 -and $before -eq $after -and -not (Test-Path -LiteralPath $case.Manifest)) { 'PASS' } else { 'FAIL' }
    detail = 'unowned matching declaration refused'
}

# A wrong pre-existing backup at the content-addressed path stops before writes.
$case = New-CaseEnvironment 'backup-collision'
$hooksPath = Join-Path $case.CodexHome 'hooks.json'
$originalHooks = '{"description":"owner","hooks":{}}'
[System.IO.File]::WriteAllText($hooksPath, $originalHooks, $utf8)
$digest = (Get-FileHash -LiteralPath $hooksPath -Algorithm SHA256).Hash.ToLowerInvariant()
$null = New-Item -ItemType Directory -Force -Path $case.StateRoot
[System.IO.File]::WriteAllText((Join-Path $case.StateRoot "before-hooks-$digest"), 'wrong-content', $utf8)
$before = Get-FileDigestOrAbsent $hooksPath
$setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$results += [pscustomobject]@{
    name = $case.Name
    class = 'SECURITY_BOUNDARY'
    setupExit = $setup.ExitCode
    repeatedSetupExit = $null
    doctorExit = $null
    uninstallExit = $null
    disposition = if ($setup.ExitCode -ne 0 -and $before -eq (Get-FileDigestOrAbsent $hooksPath) -and -not (Test-Path -LiteralPath $case.Manifest)) { 'PASS' } else { 'FAIL' }
    detail = 'content-addressed backup collision refused'
}

# Locked destination: backups and Installing manifest may exist, but external
# config must remain exact and doctor must report the invalid ownership phase.
$case = New-CaseEnvironment 'locked-hooks'
$hooksPath = Join-Path $case.CodexHome 'hooks.json'
[System.IO.File]::WriteAllText($hooksPath, '{"description":"locked","hooks":{}}', $utf8)
$before = Get-FileDigestOrAbsent $hooksPath
$lock = [System.IO.File]::Open($hooksPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::Read)
try {
    $setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
} finally {
    $lock.Dispose()
}
$doctor = Invoke-CapturedProcess -ArgumentList @('doctor') -Environment $case.Environment
$manifestPhase = if (Test-Path -LiteralPath $case.Manifest) { (Get-Content -LiteralPath $case.Manifest -Raw | ConvertFrom-Json).phase } else { 'absent' }
$results += [pscustomobject]@{
    name = $case.Name
    class = 'FILESYSTEM'
    setupExit = $setup.ExitCode
    repeatedSetupExit = $null
    doctorExit = $doctor.ExitCode
    uninstallExit = $null
    disposition = if ($setup.ExitCode -ne 0 -and $before -eq (Get-FileDigestOrAbsent $hooksPath) -and $manifestPhase -eq 'installing' -and $doctor.Stdout -match 'ownership.manifest STATUS=FAIL') { 'PASS' } else { 'FAIL' }
    detail = 'locked replacement leaves typed Installing recovery boundary'
}

# Corrupt, missing, and interrupted manifests refuse mutation and surface doctor.
foreach ($variant in @('corrupt', 'missing', 'installing')) {
    $case = New-CaseEnvironment "manifest-$variant"
    $setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
    $hooksPath = Join-Path $case.CodexHome 'hooks.json'
    $configPath = Join-Path $case.CodexHome 'config.toml'
    $hooksBefore = Get-FileDigestOrAbsent $hooksPath
    $configBefore = Get-FileDigestOrAbsent $configPath
    if ($variant -eq 'corrupt') {
        [System.IO.File]::WriteAllText($case.Manifest, '{partial', $utf8)
    } elseif ($variant -eq 'missing') {
        Move-Item -LiteralPath $case.Manifest -Destination ($case.Manifest + '.saved')
    } else {
        $manifest = Get-Content -LiteralPath $case.Manifest -Raw | ConvertFrom-Json -Depth 30
        $manifest.phase = 'installing'
        [System.IO.File]::WriteAllText($case.Manifest, ($manifest | ConvertTo-Json -Depth 30), $utf8)
    }
    $doctor = Invoke-CapturedProcess -ArgumentList @('doctor') -Environment $case.Environment
    $retry = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
    $uninstall = Invoke-CapturedProcess -ArgumentList @('uninstall', 'codex') -Environment $case.Environment
    $unchanged = $hooksBefore -eq (Get-FileDigestOrAbsent $hooksPath) -and $configBefore -eq (Get-FileDigestOrAbsent $configPath)
    $safeRetry = if ($variant -eq 'missing') {
        $retry.ExitCode -ne 0 -and $uninstall.Stdout -match 'CODEX_INTEGRATION=NOT_INSTALLED'
    } else {
        $retry.ExitCode -ne 0 -and $uninstall.ExitCode -ne 0
    }
    $results += [pscustomobject]@{
        name = $case.Name
        class = 'TRUST_BOUNDARY'
        setupExit = $setup.ExitCode
        repeatedSetupExit = $retry.ExitCode
        doctorExit = $doctor.ExitCode
        uninstallExit = $uninstall.ExitCode
        disposition = if ($setup.ExitCode -eq 0 -and $doctor.Stdout -match 'ownership.manifest STATUS=FAIL' -and $safeRetry -and $unchanged) { 'PASS' } else { 'FAIL' }
        detail = 'manifest damage is typed and does not guess ownership'
    }
}

# An abandoned unrelated temporary file must not affect setup.
$case = New-CaseEnvironment 'abandoned-temp'
[System.IO.File]::WriteAllText((Join-Path $case.CodexHome 'hooks.json.abandoned.tmp'), 'partial', $utf8)
$setup = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$counts = Get-HookCounts (Join-Path $case.CodexHome 'hooks.json')
$results += [pscustomobject]@{
    name = $case.Name
    class = 'FILESYSTEM'
    setupExit = $setup.ExitCode
    repeatedSetupExit = $null
    doctorExit = $null
    uninstallExit = $null
    disposition = if ($setup.ExitCode -eq 0 -and $counts.TabBeacon -eq 7 -and (Test-Path -LiteralPath (Join-Path $case.CodexHome 'hooks.json.abandoned.tmp'))) { 'PASS' } else { 'FAIL' }
    detail = 'unowned abandoned temp ignored and preserved'
}

# Ten simultaneous first-use setup processes serialize under the integration lock.
$case = New-CaseEnvironment 'concurrent-setup'
$captures = @()
1..10 | ForEach-Object {
    $captures += Start-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
}
$processResults = @($captures | ForEach-Object { Wait-CapturedProcess $_ })
$counts = Get-HookCounts (Join-Path $case.CodexHome 'hooks.json')
$results += [pscustomobject]@{
    name = $case.Name
    class = 'FILESYSTEM'
    setupExit = (@($processResults | Where-Object ExitCode -ne 0).Count)
    repeatedSetupExit = $null
    doctorExit = $null
    uninstallExit = $null
    disposition = if (@($processResults | Where-Object { $_.ExitCode -ne 0 -or $_.TimedOut }).Count -eq 0 -and $counts.TabBeacon -eq 7 -and (Test-Path -LiteralPath $case.Manifest)) { 'PASS' } else { 'FAIL' }
    detail = '10 concurrent first-use writers serialize to one exact integration'
}

# Setup versus uninstall race must finish in one complete state, never a partial mix.
$case = New-CaseEnvironment 'setup-uninstall-race'
$initial = Invoke-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$setupCapture = Start-CapturedProcess -ArgumentList @('setup', 'codex') -Environment $case.Environment
$uninstallCapture = Start-CapturedProcess -ArgumentList @('uninstall', 'codex') -Environment $case.Environment
$raceSetup = Wait-CapturedProcess $setupCapture
$raceUninstall = Wait-CapturedProcess $uninstallCapture
$manifestExists = Test-Path -LiteralPath $case.Manifest
$counts = Get-HookCounts (Join-Path $case.CodexHome 'hooks.json')
$completeInstalled = $manifestExists -and $counts.TabBeacon -eq 7
$completeRemoved = -not $manifestExists -and $counts.TabBeacon -eq 0
$results += [pscustomobject]@{
    name = $case.Name
    class = 'FILESYSTEM'
    setupExit = $raceSetup.ExitCode
    repeatedSetupExit = $null
    doctorExit = $null
    uninstallExit = $raceUninstall.ExitCode
    disposition = if ($initial.ExitCode -eq 0 -and $raceSetup.ExitCode -eq 0 -and $raceUninstall.ExitCode -eq 0 -and ($completeInstalled -or $completeRemoved)) { 'PASS' } else { 'FAIL' }
    detail = if ($completeInstalled) { 'race ended fully installed' } elseif ($completeRemoved) { 'race ended fully removed' } else { 'race ended partial' }
}

$summary = [ordered]@{
    runKey = $runKey
    outputRoot = $outputRoot
    generatedAtUtc = [DateTime]::UtcNow.ToString('o')
    results = $results
    overall = if (@($results | Where-Object disposition -ne 'PASS').Count -eq 0) { 'PASS' } else { 'FAIL' }
}
$summaryPath = Join-Path $outputRoot 'summary.json'
[System.IO.File]::WriteAllText($summaryPath, ($summary | ConvertTo-Json -Depth 20), $utf8)
$summary | ConvertTo-Json -Depth 12
