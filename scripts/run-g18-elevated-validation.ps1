[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string]$ExpectedHead,

    [string]$EvidenceRoot = 'V:\build\tabbeacon\TB-G18-FAST-LANE-CLOSEOUT-001'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-CompactReceipt {
    param([System.Collections.IDictionary]$Receipt)

    $receiptPath = Join-Path $EvidenceRoot 'g18-elevated-validation-receipt.json'
    $Receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $receiptPath -Encoding utf8
    $Receipt.GetEnumerator() | ForEach-Object { '{0}={1}' -f $_.Key, $_.Value }
}

function Invoke-OwnedVisualFixture {
    param(
        [string]$Fixture,
        [string]$FixtureExecutable,
        [string]$RunIdentifier
    )

    $output = & $FixtureExecutable run --expected-head $ExpectedHead --run-id $RunIdentifier `
        --evidence-root $EvidenceRoot --fixture $Fixture 2>&1
    $exitCode = $LASTEXITCODE
    $summaryLine = @($output | ForEach-Object { $_.ToString() } | Where-Object {
        $_.TrimStart().StartsWith('{')
    }) | Select-Object -Last 1
    if ($null -eq $summaryLine) {
        return [pscustomobject]@{ ExitCode = $exitCode; Summary = $null }
    }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Summary = ($summaryLine | ConvertFrom-Json)
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    New-Item -ItemType Directory -Force -Path $EvidenceRoot | Out-Null
    $receipt = [ordered]@{
        RUN_ID = 'TB-G18-FAST-LANE-CLOSEOUT-001'
        EXPECTED_HEAD = $ExpectedHead
        ACTUAL_ELEVATED_TOKEN = $false
        ADMIN_POWERSHELL = 'FAIL_NOT_ELEVATED'
        TITLE_AUTHORITY = 'NOT_RUN'
        VISIBLE_CONVERGENCE = 'NOT_RUN'
        WORKING_FRAMES = 0
        FINAL_STATIC_STATE = 'NOT_RUN'
        CLEANUP = 'NOT_RUN'
        OWNER_CONFIG_MUTATED = $false
        OWNER_SHELL_PROFILE_MUTATED = $false
        OWNER_WINDOWS_TERMINAL_SETTINGS_MUTATED = $false
    }

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $isElevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    $receipt.ACTUAL_ELEVATED_TOKEN = $isElevated
    if (-not $isElevated) {
        Write-CompactReceipt $receipt
        exit 2
    }
    $receipt.ADMIN_POWERSHELL = 'PASS_ACTUAL_ELEVATED'

    $gitCommand = Get-Command git -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $gitCommand) {
        $receipt.VISIBLE_CONVERGENCE = 'BLOCKED_REQUIRED_TOOL_MISSING'
        Write-CompactReceipt $receipt
        exit 3
    }

    $actualHead = (& $gitCommand.Source rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $actualHead -ne $ExpectedHead) {
        $receipt.VISIBLE_CONVERGENCE = 'BLOCKED_HEAD_MISMATCH'
        Write-CompactReceipt $receipt
        exit 3
    }

    $candidateStatus = & $gitCommand.Source status --porcelain
    if ($LASTEXITCODE -ne 0 -or -not [string]::IsNullOrWhiteSpace(($candidateStatus -join "`n"))) {
        $receipt.VISIBLE_CONVERGENCE = 'BLOCKED_DIRTY_CANDIDATE'
        Write-CompactReceipt $receipt
        exit 3
    }

    $cargoCommand = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $rustupCommand = Get-Command rustup -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $toolchainLine = Select-String -LiteralPath (Join-Path $repoRoot 'rust-toolchain.toml') `
        -Pattern '^\s*channel\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($null -eq $cargoCommand -or $null -eq $rustupCommand -or $null -eq $toolchainLine) {
        $receipt.VISIBLE_CONVERGENCE = 'BLOCKED_REQUIRED_TOOL_MISSING'
        Write-CompactReceipt $receipt
        exit 3
    }

    $pinnedToolchain = $toolchainLine.Matches[0].Groups[1].Value
    $cargoBin = Split-Path -Parent $cargoCommand.Source
    if ((Split-Path -Leaf $cargoBin) -eq 'bin') {
        $derivedCargoHome = Split-Path -Parent $cargoBin
        $derivedRustupHome = Join-Path (Split-Path -Parent $derivedCargoHome) '.rustup'
        if ((Test-Path -LiteralPath $derivedCargoHome -PathType Container) -and `
            (Test-Path -LiteralPath $derivedRustupHome -PathType Container)) {
            $env:CARGO_HOME = $derivedCargoHome
            $env:RUSTUP_HOME = $derivedRustupHome
        }
    }
    $env:RUSTUP_TOOLCHAIN = $pinnedToolchain
    $availableToolchains = & $rustupCommand.Source toolchain list 2>&1
    $toolchainPattern = '^{0}(?:-|\s)' -f [regex]::Escape($pinnedToolchain)
    if ($LASTEXITCODE -ne 0 -or -not (@($availableToolchains) | Where-Object {
        $_.ToString().TrimStart() -match $toolchainPattern
    })) {
        $receipt.VISIBLE_CONVERGENCE = 'BLOCKED_PINNED_TOOLCHAIN_UNAVAILABLE'
        Write-CompactReceipt $receipt
        exit 3
    }

    # The repository-pinned toolchain and any derived Rust homes above exist
    # only in this process; neither profiles nor persistent environment state change.
    & $cargoCommand.Source build --locked --features visual-fixture --bin tabbeacon-visual-fixture --bin tabbeacon
    if ($LASTEXITCODE -ne 0) {
        $receipt.VISIBLE_CONVERGENCE = 'BLOCKED_FIXTURE_BUILD'
        Write-CompactReceipt $receipt
        exit 3
    }

    $fixtureExecutable = Join-Path $repoRoot 'target\debug\tabbeacon-visual-fixture.exe'
    $runPrefix = "TB-G18-ELEVATED-$PID"
    $working = Invoke-OwnedVisualFixture -Fixture 'working' -FixtureExecutable $fixtureExecutable `
        -RunIdentifier "$runPrefix-working"
    $result = Invoke-OwnedVisualFixture -Fixture 'result-ready' -FixtureExecutable $fixtureExecutable `
        -RunIdentifier "$runPrefix-result"

    $framePath = Join-Path (Join-Path $EvidenceRoot "$runPrefix-working") 'title-frames-working.json'
    if (Test-Path -LiteralPath $framePath) {
        $receipt.WORKING_FRAMES = @((Get-Content -LiteralPath $framePath -Raw | ConvertFrom-Json)).Count
    }
    $workingPass = $null -ne $working.Summary -and $working.Summary.uia -eq 'pass' -and `
        $working.Summary.title -eq 'pass' -and $working.Summary.animation -eq 'pass' -and `
        $receipt.WORKING_FRAMES -ge 3
    $resultPass = $null -ne $result.Summary -and $result.Summary.uia -eq 'pass' -and `
        $result.Summary.title -eq 'pass'
    $receipt.FINAL_STATIC_STATE = if ($resultPass) { 'PASS' } else { 'UNPROVEN' }

    # The G15 probe uses a separate owned anchor/probe pair and refuses a
    # cleanup claim unless the owned sibling tab retired within its deadline.
    $probeOutput = & (Join-Path $repoRoot 'target\debug\tabbeacon.exe') doctor --probe-title --json 2>&1
    $probeLine = @($probeOutput | ForEach-Object { $_.ToString() } | Where-Object {
        $_.TrimStart().StartsWith('{')
    }) | Select-Object -Last 1
    $probe = if ($null -eq $probeLine) { $null } else { $probeLine | ConvertFrom-Json }
    if ($null -ne $probe) {
        $receipt.TITLE_AUTHORITY = $probe.title.authority
        $receipt.CLEANUP = if ($probe.title.probe_boundary -eq 'complete') { 'PASS' } else { 'UNPROVEN' }
    }

    $allPass = $workingPass -and $resultPass -and $receipt.CLEANUP -eq 'PASS' -and `
        $receipt.TITLE_AUTHORITY -eq 'healthy'
    $receipt.VISIBLE_CONVERGENCE = if ($allPass) { 'PASS' } else { 'UNPROVEN' }
    Write-CompactReceipt $receipt
    if (-not $allPass) { exit 3 }
    exit 0
}
finally {
    Pop-Location
}
