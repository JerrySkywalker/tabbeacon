[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$CargoHome,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9_-]+$')]
    [string]$Package,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$')]
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$maxMetadataBytes = 1MB
$maxCandidates = 128

function Get-BoundedText {
    param([Parameter(Mandatory = $true)][string]$Path)

    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    if ($item.Length -gt $maxMetadataBytes) {
        throw 'Cargo metadata exceeds the bounded verifier input size.'
    }
    [IO.File]::ReadAllText($item.FullName, [Text.Encoding]::UTF8)
}

function Add-MatchingSource {
    param(
        [Parameter(Mandatory = $true)][string]$Entry,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][System.Collections.Generic.List[string]]$Sources
    )

    $escapedPackage = [Regex]::Escape($Package)
    $escapedVersion = [Regex]::Escape($Version)
    $pattern = "^$escapedPackage\s+$escapedVersion\s+\((?<source>[^)]+)\)$"
    $match = [Regex]::Match($Entry, $pattern, [Text.RegularExpressions.RegexOptions]::CultureInvariant)
    if (-not $match.Success) { return }
    if ($Sources.Count -ge $maxCandidates) {
        throw 'Too many matching Cargo installation records.'
    }
    $Sources.Add($match.Groups['source'].Value)
}

function Get-TomlSources {
    param([Parameter(Mandatory = $true)][string]$Path)

    $sources = [System.Collections.Generic.List[string]]::new()
    $text = Get-BoundedText -Path $Path
    foreach ($line in $text -split "`r?`n") {
        $match = [Regex]::Match($line, '^\s*"(?<entry>[^"]+)"\s*=')
        if ($match.Success) {
            Add-MatchingSource -Entry $match.Groups['entry'].Value -Sources $sources
        }
    }
    $sources
}

function Get-JsonSources {
    param([Parameter(Mandatory = $true)][string]$Path)

    $sources = [System.Collections.Generic.List[string]]::new()
    $text = Get-BoundedText -Path $Path
    $document = $text | ConvertFrom-Json -AsHashtable -Depth 16
    $installs = $document['installs']
    if ($null -eq $installs -or -not ($installs -is [Collections.IDictionary])) {
        return $sources
    }
    foreach ($entry in $installs.Keys) {
        Add-MatchingSource -Entry ([string]$entry) -Sources $sources
    }
    $sources
}

function Get-InstallSourceClass {
    param([Parameter(Mandatory = $true)][string]$Source)

    if ($Source -like 'git+*') { return 'GIT_REVISION' }
    if ($Source -like 'path+*') { return 'LOCAL_PATH' }
    if ($Source -eq 'registry+https://github.com/rust-lang/crates.io-index' -or
        $Source -eq 'registry+sparse+https://index.crates.io/') {
        return 'REGISTRY_OFFICIAL'
    }
    'UNKNOWN_OR_UNPROVEN'
}

$sources = [System.Collections.Generic.List[string]]::new()
foreach ($metadataName in '.crates.toml', '.crates2.json') {
    $metadataPath = Join-Path $CargoHome $metadataName
    if (-not (Test-Path -LiteralPath $metadataPath -PathType Leaf)) { continue }
    $fromFile = if ($metadataName -eq '.crates.toml') {
        Get-TomlSources -Path $metadataPath
    } else {
        Get-JsonSources -Path $metadataPath
    }
    foreach ($source in $fromFile) {
        if (-not $sources.Contains($source)) { $sources.Add($source) }
    }
}

$classification = if ($sources.Count -eq 1) {
    Get-InstallSourceClass -Source $sources[0]
} else {
    'UNKNOWN_OR_UNPROVEN'
}

Write-Output "OWNER_INSTALL_SOURCE=$classification"
Write-Output "OWNER_INSTALL_SOURCE_PROVEN=$(($classification -ne 'UNKNOWN_OR_UNPROVEN').ToString().ToLowerInvariant())"
Write-Output "OWNER_GIT_REV_INSTALL=$(($classification -eq 'GIT_REVISION').ToString().ToLowerInvariant())"
Write-Output 'OWNER_OFFICIAL_CHANNEL=crates.io'
