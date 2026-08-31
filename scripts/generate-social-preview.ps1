[CmdletBinding()]
param(
    [string]$SourcePath = (Join-Path $PSScriptRoot '..\docs\assets\social\tabbeacon-social-preview.svg'),
    [string]$OutputPath = (Join-Path $PSScriptRoot '..\docs\assets\social\tabbeacon-social-preview.png'),
    [string]$ExpectedHead = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $repositoryRoot
$expectedSource = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot 'docs\assets\social\tabbeacon-social-preview.svg'))
$expectedOutput = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot 'docs\assets\social\tabbeacon-social-preview.png'))
$requestedSource = [System.IO.Path]::GetFullPath($SourcePath)
$requestedOutput = [System.IO.Path]::GetFullPath($OutputPath)
if ([string]::IsNullOrWhiteSpace($ExpectedHead)) {
    $ExpectedHead = (git rev-parse HEAD).Trim()
}
if (-not $ExpectedHead -match '^[0-9a-f]{40}$') {
    throw 'ExpectedHead must be a lowercase full SHA.'
}
if (-not $requestedSource.Equals($expectedSource, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not $requestedOutput.Equals($expectedOutput, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Social preview generation is limited to the admitted repository asset paths.'
}
$checkedOutHead = (git rev-parse HEAD).Trim()
if ($checkedOutHead -ne $ExpectedHead) {
    throw "EXPECTED_HEAD mismatch: expected $ExpectedHead, checked out $checkedOutHead"
}
$dirtyPaths = @(git status --porcelain)
if ($dirtyPaths.Count -ne 0) {
    throw 'Social preview generation requires a clean committed candidate.'
}

function Resolve-EdgeExecutable {
    $candidates = @(@(
        (Get-Command msedge.exe -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source),
        (Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe'),
        (Join-Path $env:ProgramFiles 'Microsoft\Edge\Application\msedge.exe')
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) })

    if ($candidates.Count -eq 0) {
        throw 'Microsoft Edge headless is required to render the social preview.'
    }
    return $candidates[0]
}

function Get-PngDimensions([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 24 -or
        $bytes[0] -ne 137 -or $bytes[1] -ne 80 -or $bytes[2] -ne 78 -or $bytes[3] -ne 71 -or
        $bytes[4] -ne 13 -or $bytes[5] -ne 10 -or $bytes[6] -ne 26 -or $bytes[7] -ne 10) {
        throw "Not a PNG file: $Path"
    }
    $width = ([uint32]$bytes[16] -shl 24) -bor ([uint32]$bytes[17] -shl 16) -bor ([uint32]$bytes[18] -shl 8) -bor [uint32]$bytes[19]
    $height = ([uint32]$bytes[20] -shl 24) -bor ([uint32]$bytes[21] -shl 16) -bor ([uint32]$bytes[22] -shl 8) -bor [uint32]$bytes[23]
    return @([int]$width, [int]$height)
}

$source = (Resolve-Path -LiteralPath $expectedSource).Path
$output = $expectedOutput
$temporaryOutput = "$output.$PID.render.png"
$svg = Get-Content -LiteralPath $source -Raw

if (Test-Path -LiteralPath $output) {
    throw "Refusing to overwrite an existing social-preview asset: $output"
}

if ($svg -match '(?is)<script|<foreignObject|<text\b|\son[a-z]+\s*=|@font-face|font-family|url\(|\b(?:xlink:)?href\s*=\s*["''](?!#)') {
    throw 'Social preview SVG contains active content, remote content, or a font dependency.'
}

$document = [System.Xml.XmlDocument]::new()
$document.XmlResolver = $null
$document.LoadXml($svg)
if ($document.DocumentElement.LocalName -ne 'svg' -or $document.DocumentElement.GetAttribute('width') -ne '1280' -or $document.DocumentElement.GetAttribute('height') -ne '640') {
    throw 'Social preview SVG must declare exact 1280x640 dimensions.'
}

$outputDirectory = Split-Path -Parent $output
[System.IO.Directory]::CreateDirectory($outputDirectory) | Out-Null
if (Test-Path -LiteralPath $temporaryOutput) { throw "Refusing to reuse temporary social-preview output: $temporaryOutput" }

$edge = Resolve-EdgeExecutable
$uri = [System.Uri]::new($source).AbsoluteUri
$profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) "tabbeacon-social-preview-$PID"
if (Test-Path -LiteralPath $profileRoot) { throw "Refusing to reuse Edge profile root: $profileRoot" }
[System.IO.Directory]::CreateDirectory($profileRoot) | Out-Null
$rendered = $false
try {
    $global:LASTEXITCODE = 0
    & $edge --headless=new --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check "--user-data-dir=$profileRoot" "--window-size=1280,640" "--screenshot=$temporaryOutput" $uri
    if ($LASTEXITCODE -ne 0) { throw "Edge render failed with exit code $LASTEXITCODE." }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while (-not (Test-Path -LiteralPath $temporaryOutput -PathType Leaf) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 250
    }
    $rendered = Test-Path -LiteralPath $temporaryOutput -PathType Leaf
    if (-not $rendered) { throw "Edge did not create the social-preview PNG within 30 seconds: $temporaryOutput" }
}
finally {
    if (Test-Path -LiteralPath $profileRoot) {
        $profileItem = Get-Item -LiteralPath $profileRoot -Force
        if ($profileItem.LinkType) { throw "Refusing to remove reparse-point Edge profile root: $profileRoot" }
        try {
            Remove-Item -LiteralPath $profileRoot -Recurse -Force
        }
        catch {
            Write-Warning "Preserved exact-owned Edge profile root after render because it is still in use: $profileRoot"
        }
    }
}

$dimensions = Get-PngDimensions $temporaryOutput
if ($dimensions[0] -ne 1280 -or $dimensions[1] -ne 640) {
    throw "Rendered social preview dimensions are $($dimensions[0])x$($dimensions[1]), expected 1280x640."
}
Move-Item -LiteralPath $temporaryOutput -Destination $output

Write-Output "SOCIAL_PREVIEW_SVG=PASS"
Write-Output "SOCIAL_PREVIEW_PNG=PASS"
Write-Output "SOCIAL_PREVIEW_DIMENSIONS=$($dimensions[0])x$($dimensions[1])"
