[CmdletBinding()]
param(
    [string]$SourcePath = (Join-Path $PSScriptRoot '..\docs\assets\social\tabbeacon-social-preview.svg'),
    [string]$OutputPath = (Join-Path $PSScriptRoot '..\docs\assets\social\tabbeacon-social-preview.png')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

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

$source = (Resolve-Path -LiteralPath $SourcePath).Path
$output = [System.IO.Path]::GetFullPath($OutputPath)
$svg = Get-Content -LiteralPath $source -Raw

if ($svg -match '(?is)<script|<foreignObject|\son[a-z]+\s*=|@font-face|url\(\s*["'']?https?://|\bxlink:href\s*=|\bhref\s*=') {
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
if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Force }

$edge = Resolve-EdgeExecutable
$uri = [System.Uri]::new($source).AbsoluteUri
$profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) "tabbeacon-social-preview-$PID"
if (Test-Path -LiteralPath $profileRoot) { throw "Refusing to reuse Edge profile root: $profileRoot" }
[System.IO.Directory]::CreateDirectory($profileRoot) | Out-Null
$rendered = $false
try {
    $global:LASTEXITCODE = 0
    & $edge --headless=new --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check "--user-data-dir=$profileRoot" "--window-size=1280,640" "--screenshot=$output" $uri
    if ($LASTEXITCODE -ne 0) { throw "Edge render failed with exit code $LASTEXITCODE." }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while (-not (Test-Path -LiteralPath $output -PathType Leaf) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 250
    }
    $rendered = Test-Path -LiteralPath $output -PathType Leaf
    if (-not $rendered) { throw "Edge did not create the social-preview PNG within 30 seconds: $output" }
}
finally {
    if ($rendered -and (Test-Path -LiteralPath $profileRoot)) {
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

$dimensions = Get-PngDimensions $output
if ($dimensions[0] -ne 1280 -or $dimensions[1] -ne 640) {
    throw "Rendered social preview dimensions are $($dimensions[0])x$($dimensions[1]), expected 1280x640."
}

Write-Output "SOCIAL_PREVIEW_SVG=PASS"
Write-Output "SOCIAL_PREVIEW_PNG=PASS"
Write-Output "SOCIAL_PREVIEW_DIMENSIONS=$($dimensions[0])x$($dimensions[1])"
