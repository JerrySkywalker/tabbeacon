Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:TabBeaconRepository = 'JerrySkywalker/tabbeacon'
$script:TabBeaconRunnerMarker = 'tabbeacon-visual-runner.json'
$script:TabBeaconRunnerState = 'tabbeacon-visual-runner-state.json'
$script:TabBeaconRunnerStop = 'tabbeacon-visual-runner-stop.request'

function Get-TabBeaconRunnerRoot {
    param([string]$RunnerRoot)

    if ([string]::IsNullOrWhiteSpace($RunnerRoot)) {
        return (Join-Path $env:LOCALAPPDATA 'TabBeacon\visual-runner')
    }
    return [IO.Path]::GetFullPath($RunnerRoot)
}

function Assert-TabBeaconRunnerRoot {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)

    $base = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'TabBeacon'))
    $root = [IO.Path]::GetFullPath($RunnerRoot)
    if ($root -eq $base -or -not $root.StartsWith($base + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Runner root must stay below the current user's TabBeacon state root: $base"
    }
    return $root
}

function Get-TabBeaconRunnerName {
    param([string]$RunnerName)

    if (-not [string]::IsNullOrWhiteSpace($RunnerName)) {
        return $RunnerName
    }
    $machine = ($env:COMPUTERNAME.ToLowerInvariant() -replace '[^a-z0-9-]', '-')
    return "tabbeacon-visual-$machine"
}

function Get-TabBeaconMarkerPath {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)
    return (Join-Path $RunnerRoot $script:TabBeaconRunnerMarker)
}

function Get-TabBeaconStatePath {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)
    return (Join-Path $RunnerRoot $script:TabBeaconRunnerState)
}

function Get-TabBeaconStopPath {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)
    return (Join-Path $RunnerRoot $script:TabBeaconRunnerStop)
}

function Read-TabBeaconJson {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    return (Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json)
}

function Write-TabBeaconJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][object]$Value
    )

    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $temporary = Join-Path $parent ('.' + [IO.Path]::GetFileName($Path) + '.' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        $json = $Value | ConvertTo-Json -Depth 8
        [IO.File]::WriteAllText($temporary, $json, [Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $temporary -Destination $Path -Force
    } finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
    }
}

function Get-TabBeaconMarker {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)
    return (Read-TabBeaconJson (Get-TabBeaconMarkerPath $RunnerRoot))
}

function Assert-TabBeaconOwnedRunner {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)

    $marker = Get-TabBeaconMarker $RunnerRoot
    if ($null -eq $marker -or $marker.schema -ne 'tabbeacon-visual-runner-v1' -or $marker.repository -ne $script:TabBeaconRepository -or $marker.owner -ne 'TB-G03R') {
        throw "Runner root is not positively owned by TabBeacon G03R: $RunnerRoot"
    }
    return $marker
}

function Assert-TabBeaconInteractiveSession {
    $sessionId = [Diagnostics.Process]::GetCurrentProcess().SessionId
    if ($sessionId -eq 0 -or -not [Environment]::UserInteractive) {
        throw "TabBeacon visual runner must run in an interactive nonzero user session; observed session=$sessionId interactive=$([Environment]::UserInteractive)"
    }
    return $sessionId
}

function Get-TabBeaconRepositoryRunner {
    param([Parameter(Mandatory = $true)][string]$RunnerName)

    $response = gh api "repos/$script:TabBeaconRepository/actions/runners" | ConvertFrom-Json
    return @($response.runners | Where-Object { $_.name -eq $RunnerName })
}

function Wait-TabBeaconRunnerOnline {
    param(
        [Parameter(Mandatory = $true)][string]$RunnerName,
        [int]$TimeoutSeconds = 30
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $matches = @(Get-TabBeaconRepositoryRunner $RunnerName)
        if ($matches.Count -eq 1 -and $matches[0].status -eq 'online') { return $matches[0] }
        Start-Sleep -Seconds 1
    } while ([DateTime]::UtcNow -lt $deadline)
    return $null
}

function Wait-TabBeaconRunnerOffline {
    param(
        [Parameter(Mandatory = $true)][string]$RunnerName,
        [int]$TimeoutSeconds = 60
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $matches = @(Get-TabBeaconRepositoryRunner $RunnerName)
        if ($matches.Count -eq 1 -and $matches[0].status -eq 'offline') { return $matches[0] }
        Start-Sleep -Seconds 1
    } while ([DateTime]::UtcNow -lt $deadline)
    return $null
}

function Get-TabBeaconRunnerVersion {
    param([Parameter(Mandatory = $true)][string]$RunnerRoot)

    $listener = Join-Path $RunnerRoot 'bin\Runner.Listener.exe'
    if (-not (Test-Path -LiteralPath $listener -PathType Leaf)) { return 'UNAVAILABLE' }
    $version = & $listener '--version' 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace(($version -join ''))) { return 'UNAVAILABLE' }
    return (($version -join ' ').Trim())
}

function Write-TabBeaconRedactedLog {
    param(
        [Parameter(Mandatory = $true)][string]$RunnerRoot,
        [Parameter(Mandatory = $true)][string]$Name,
        [string[]]$Lines,
        [AllowNull()][string]$SensitiveValue
    )

    $safeName = $Name -replace '[^A-Za-z0-9._-]', '-'
    $path = Join-Path $RunnerRoot ("$safeName.log")
    $redacted = $Lines | ForEach-Object {
        $line = [string]$_
        if (-not [string]::IsNullOrEmpty($SensitiveValue)) { $line = $line.Replace($SensitiveValue, '[REDACTED]') }
        $line
    }
    [IO.File]::WriteAllLines($path, [string[]]$redacted, [Text.UTF8Encoding]::new($false))
    return $path
}
