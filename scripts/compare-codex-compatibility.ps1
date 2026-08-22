[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AdmittedSource,
    [Parameter(Mandatory = $true)]
    [string]$CandidateSource,
    [string]$Repository = (Get-Location).Path
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$SourceExtensions = @('.rs', '.ts', '.tsx', '.js', '.mjs', '.py', '.go', '.json', '.toml')
$RelevantPathPattern = '(?i)(hook|session|turn|subagent|compact|terminal.*title|title.*ownership)'
$BreakingPattern = '(?i)(hook[_ -]?event|session[_ -]?id|turn[_ -]?id|agent[_ -]?(id|type)|subagent|precompact|postcompact|\basync\b|\btimeout\b|terminal[_ -]?title|title[_ -]?ownership|commandwindows|trusted_hash|mcp_tool|schema)'
$ProtocolPattern = '(?i)(\bhooks\b|\btype\b|\bcommand\b|commandwindows|\btimeout\b|\basync\b|trusted_hash|terminal_title|mcp_tool|schema)'
$DeltaAuditSchema = 'tabbeacon-codex-hook-delta-v1'

function Get-DirectoryDiff {
    param(
        [Parameter(Mandatory = $true)][string]$Left,
        [Parameter(Mandatory = $true)][string]$Right
    )

    $output = @(& git diff --no-index --unified=0 -- $Left $Right 2>$null)
    if ($LASTEXITCODE -notin 0, 1) {
        throw 'Git could not compare the supplied source checkouts.'
    }
    return $output
}

function Get-ReferenceDiff {
    param(
        [Parameter(Mandatory = $true)][string]$Left,
        [Parameter(Mandatory = $true)][string]$Right,
        [Parameter(Mandatory = $true)][string]$GitRepository
    )

    $output = @(& git -C $GitRepository diff --unified=0 $Left $Right -- 2>$null)
    if ($LASTEXITCODE -ne 0) {
        throw 'Git could not compare the supplied source references.'
    }
    return $output
}

$admittedIsDirectory = Test-Path -LiteralPath $AdmittedSource -PathType Container
$candidateIsDirectory = Test-Path -LiteralPath $CandidateSource -PathType Container
if ($admittedIsDirectory -ne $candidateIsDirectory) {
    throw 'Both sources must be checkouts, or both must be Git references.'
}

$diff = if ($admittedIsDirectory) {
    Get-DirectoryDiff -Left (Resolve-Path -LiteralPath $AdmittedSource) -Right (Resolve-Path -LiteralPath $CandidateSource)
} else {
    Get-ReferenceDiff -Left $AdmittedSource -Right $CandidateSource -GitRepository $Repository
}
$candidateRoot = if ($candidateIsDirectory) {
    (Resolve-Path -LiteralPath $CandidateSource).Path
} else {
    $null
}

$changedSourceLines = @($diff | Where-Object {
    $_ -match '^[+-](?![+-])' -and $_ -notmatch '^[+-]{3}'
})
$changedPaths = @($diff | Where-Object { $_ -match '^\+\+\+\s+' } | ForEach-Object {
    $path = ($_ -replace '^\+\+\+\s+', '').Trim().Trim('"')
    $path -replace '^b/', ''
} | Where-Object {
    $SourceExtensions -contains [IO.Path]::GetExtension($_).ToLowerInvariant()
})
$relevantPaths = @($changedPaths | Where-Object { $_ -match $RelevantPathPattern } | Sort-Object -Unique)
$hasRelevantSurface = $relevantPaths.Count -gt 0 -or ($changedSourceLines -join "`n") -match $BreakingPattern
$protocolSignals = @($changedSourceLines | Where-Object { $_ -match $ProtocolPattern })

"DELTA_AUDIT_SCHEMA=$DeltaAuditSchema"
if ($candidateIsDirectory) {
    'SOURCE_MODE=checkout'
} else {
    'SOURCE_MODE=git_reference'
}

if (-not $hasRelevantSurface) {
    'CLASSIFICATION=SAFE_COMPATIBLE'
    'PROTOCOL_DELTA=NONE_RELEVANT'
    'EXACT_PRODUCTION_ADMISSION=NOT_GRANTED'
    'RELEVANT_FILES=0'
    'PROTOCOL_SIGNALS=0'
    exit 0
}

if (($changedSourceLines -join "`n") -match $BreakingPattern) {
    'CLASSIFICATION=BREAKING_OR_UNPROVEN'
    'PROTOCOL_DELTA=BREAKING_OR_UNPROVEN'
} else {
    'CLASSIFICATION=REQUIRES_REVIEW'
    'PROTOCOL_DELTA=REQUIRES_SOURCE_REVIEW'
}
"EXACT_PRODUCTION_ADMISSION=NOT_GRANTED"
"RELEVANT_FILES=$($relevantPaths.Count)"
"PROTOCOL_SIGNALS=$($protocolSignals.Count)"
if ($relevantPaths.Count -gt 0) {
    $displayPaths = @($relevantPaths | ForEach-Object {
        if ($candidateRoot -and [IO.Path]::IsPathFullyQualified($_)) {
            [IO.Path]::GetRelativePath($candidateRoot, $_)
        } else {
            $_
        }
    })
    "RELEVANT_PATHS=$($displayPaths -join ',')"
}
