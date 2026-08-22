[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$qualificationScript = Join-Path $repoRoot 'scripts\invoke-agy-g64-qualification.ps1'
$tabbeacon = Join-Path $repoRoot 'target\debug\tabbeacon.exe'
if (-not (Test-Path -LiteralPath $tabbeacon -PathType Leaf)) {
    throw 'AGY_PREADMISSION_TEST_BINARY_UNAVAILABLE'
}

$source = Get-Content -LiteralPath $qualificationScript -Raw
foreach ($forbidden in @('Get-Command agy', '& agy', '& tabbeacon', 'Get-Content -LiteralPath $InputPath -Raw', 'ReadToEndAsync')) {
    if ($source.Contains($forbidden, [StringComparison]::Ordinal)) {
        throw 'AGY_PREADMISSION_PATH_BOUNDARY_VIOLATION'
    }
}
foreach ($required in @('Resolve-VerifiedExecutable', 'Resolve-DisposableInput', 'Invoke-BoundedDirectVersion', 'Kill($true)')) {
    if (-not $source.Contains($required, [StringComparison]::Ordinal)) {
        throw 'AGY_PREADMISSION_BOUNDARY_MISSING'
    }
}

$hash = (Get-FileHash -LiteralPath $tabbeacon -Algorithm SHA256).Hash
$savedPath = $env:PATH
try {
    $env:PATH = ''
    $plan = & $qualificationScript -Mode Plan -TabBeaconExecutablePath $tabbeacon -TabBeaconExecutableSha256 $hash 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw 'AGY_PREADMISSION_PATH_SHADOW_TEST_FAILED'
    }
} finally {
    $env:PATH = $savedPath
}

$plan = $plan | ConvertFrom-Json
if ($plan.admission -ne 'unadmitted' -or $plan.provider_enablement -ne 'disabled') {
    throw 'AGY_PREADMISSION_ADMISSION_BOUNDARY_VIOLATION'
}

'AGY_PREADMISSION_SCRIPT_CONTRACT=PASS'
