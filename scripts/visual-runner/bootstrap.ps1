[CmdletBinding()]
param(
    [string]$RunnerRoot,
    [string]$RunnerName,
    [switch]$Start
)

. (Join-Path $PSScriptRoot 'common.ps1')
$RunnerRoot = Assert-TabBeaconRunnerRoot (Get-TabBeaconRunnerRoot $RunnerRoot)
$RunnerName = Get-TabBeaconRunnerName $RunnerName
$sessionId = Assert-TabBeaconInteractiveSession
$marker = Get-TabBeaconMarker $RunnerRoot

if ($null -ne $marker) {
    $marker = Assert-TabBeaconOwnedRunner $RunnerRoot
    if ($marker.runner_name -ne $RunnerName) { throw "Owned runner name differs from requested name: $($marker.runner_name)" }
    & (Join-Path $PSScriptRoot 'doctor.ps1') -RunnerRoot $RunnerRoot | Write-Output
    if ($Start) { & (Join-Path $PSScriptRoot 'start.ps1') -RunnerRoot $RunnerRoot | Write-Output }
    return
}

if (Test-Path -LiteralPath $RunnerRoot) {
    $children = @(Get-ChildItem -LiteralPath $RunnerRoot -Force)
    if ($children.Count -ne 0) { throw "Refusing to adopt a nonempty unmarked runner root: $RunnerRoot" }
} else {
    New-Item -ItemType Directory -Path $RunnerRoot | Out-Null
}

$existingRemote = @(Get-TabBeaconRepositoryRunner $RunnerName)
if ($existingRemote.Count -ne 0) { throw "Refusing to replace an existing remote runner named $RunnerName without an owned local marker" }

$release = gh api repos/actions/runner/releases/latest | ConvertFrom-Json
$archive = @($release.assets | Where-Object { $_.name -match '^actions-runner-win-x64-[0-9.]+\.zip$' }) | Select-Object -First 1
$hashes = @($release.assets | Where-Object { $_.name -eq 'hashes.txt' }) | Select-Object -First 1
if ($null -eq $archive) { throw 'Official actions/runner release did not expose the required Windows x64 archive' }

$archivePath = Join-Path $RunnerRoot $archive.name
$hashPath = Join-Path $RunnerRoot 'hashes.txt'
try {
    Invoke-WebRequest -Uri $archive.browser_download_url -OutFile $archivePath
    $expectedHash = $null
    if (-not [string]::IsNullOrWhiteSpace($archive.digest) -and $archive.digest -match '^sha256:([0-9a-fA-F]{64})$') {
        $expectedHash = $Matches[1]
    } elseif ($null -ne $hashes) {
        Invoke-WebRequest -Uri $hashes.browser_download_url -OutFile $hashPath
        $hashLine = Get-Content -LiteralPath $hashPath | Where-Object { $_ -match ([regex]::Escape($archive.name) + '$') } | Select-Object -First 1
        if ($null -ne $hashLine) { $expectedHash = $hashLine.Split(' ', [StringSplitOptions]::RemoveEmptyEntries)[0] }
    }
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
    if ([string]::IsNullOrWhiteSpace($expectedHash) -or -not $actualHash.Equals($expectedHash, [StringComparison]::OrdinalIgnoreCase)) { throw "Runner archive SHA-256 mismatch or was unavailable for $($archive.name)" }
    Expand-Archive -LiteralPath $archivePath -DestinationPath $RunnerRoot -Force
} finally {
    if (Test-Path -LiteralPath $archivePath) { Remove-Item -LiteralPath $archivePath -Force }
    if (Test-Path -LiteralPath $hashPath) { Remove-Item -LiteralPath $hashPath -Force }
}

$config = Join-Path $RunnerRoot 'config.cmd'
if (-not (Test-Path -LiteralPath $config -PathType Leaf)) { throw "Verified runner archive did not contain config.cmd: $RunnerRoot" }
$registrationToken = $null
try {
    $registrationToken = (gh api --method POST "repos/$script:TabBeaconRepository/actions/runners/registration-token" | ConvertFrom-Json).token
    if ([string]::IsNullOrWhiteSpace($registrationToken)) { throw 'GitHub did not return a registration token' }
    $output = @(& $config --unattended --url "https://github.com/$script:TabBeaconRepository" --token $registrationToken --name $RunnerName --labels 'tabbeacon-visual' --work '_work' 2>&1)
    $log = Write-TabBeaconRedactedLog -RunnerRoot $RunnerRoot -Name 'bootstrap' -Lines $output -SensitiveValue $registrationToken
    if ($LASTEXITCODE -ne 0) { throw "GitHub runner configuration failed; inspect owned redacted log: $log" }
} finally {
    $registrationToken = $null
}

Write-TabBeaconJson -Path (Get-TabBeaconMarkerPath $RunnerRoot) -Value ([ordered]@{
    schema = 'tabbeacon-visual-runner-v1'
    owner = 'TB-G03R'
    repository = $script:TabBeaconRepository
    runner_name = $RunnerName
    root = $RunnerRoot
    configured_session_id = $sessionId
    configured_utc = [DateTime]::UtcNow.ToString('o')
    runner_version = Get-TabBeaconRunnerVersion $RunnerRoot
    mode = 'INTERACTIVE_USER_SESSION'
})

$summary = [ordered]@{ status = 'CONFIGURED'; runner_name = $RunnerName; runner_root = $RunnerRoot; session_id = $sessionId; runner_version = Get-TabBeaconRunnerVersion $RunnerRoot; labels_expected = @('self-hosted', 'Windows', 'X64', 'tabbeacon-visual') }
$summary | ConvertTo-Json -Compress
if ($Start) { & (Join-Path $PSScriptRoot 'start.ps1') -RunnerRoot $RunnerRoot | Write-Output }
