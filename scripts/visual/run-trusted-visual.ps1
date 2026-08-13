param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-f]{40}$')]
    [string]$ExpectedHead,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._/-]{0,255}$')]
    [string]$HeadBranch,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9-]{0,63}$')]
    [string]$RunId,

    [string]$EvidenceRoot = 'artifacts/visual'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($HeadBranch.Contains('..') -or $HeadBranch.EndsWith('/') -or $HeadBranch.StartsWith('-')) {
    throw "Unsafe trusted branch input: $HeadBranch"
}

$actualHead = (git rev-parse HEAD).Trim()
if ($actualHead -ne $ExpectedHead) {
    throw "Exact-head mismatch before visual harness: expected=$ExpectedHead actual=$actualHead"
}

New-Item -ItemType Directory -Path $EvidenceRoot -Force | Out-Null

& cargo run --locked --bin tabbeacon-visual-fixture -- run `
    --expected-head $ExpectedHead `
    --run-id $RunId `
    --evidence-root $EvidenceRoot
$visualExit = $LASTEXITCODE

if ($visualExit -eq 0) {
    "VISUAL_CI=PASS"
    exit 0
}

if ($visualExit -eq 78) {
    "VISUAL_CI=BLOCKED"
    exit 78
}

if ($visualExit -eq 3) {
    "VISUAL_CI=UNPROVEN"
    exit 3
}

"VISUAL_CI=FAIL"
exit $visualExit
