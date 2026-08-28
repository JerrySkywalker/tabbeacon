[CmdletBinding()]
param(
    [string]$ExpectedHead = (git rev-parse HEAD).Trim(),
    [string]$RunId = ("TB-V072-PROMO-" + (Get-Date -Format 'yyyyMMddHHmmss')),
    [string]$EvidenceRoot = (Join-Path 'V:\build\tabbeacon' ("TB-V072-PROMO-" + (Get-Date -Format 'yyyyMMddHHmmss')))
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Require-Command([string]$Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $command) { throw "Required command is unavailable: $Name" }
    return $command.Source
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-ExactPngDimensions([string]$Path) {
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Image]::FromFile($Path)
    try { return @($image.Width, $image.Height) } finally { $image.Dispose() }
}

if ($ExpectedHead -notmatch '^[0-9a-f]{40}$') { throw 'ExpectedHead must be a lowercase full SHA.' }
if ($RunId -notmatch '^[A-Za-z0-9-]{1,64}$') { throw 'RunId must be a safe bounded identifier.' }
if (Test-Path -LiteralPath $EvidenceRoot) { throw "Refusing to reuse evidence root: $EvidenceRoot" }

$checkedOutHead = (git rev-parse HEAD).Trim()
if ($checkedOutHead -ne $ExpectedHead) { throw "EXPECTED_HEAD mismatch: expected $ExpectedHead, checked out $checkedOutHead" }

$ffmpeg = Require-Command 'ffmpeg'
$ffprobe = Require-Command 'ffprobe'
$ffmpegVersion = (& $ffmpeg -version | Select-Object -First 1).Trim()

[System.IO.Directory]::CreateDirectory($EvidenceRoot) | Out-Null
$frames = Join-Path $EvidenceRoot 'frames'
$fixtureReceipt = Join-Path $EvidenceRoot 'promo-fixture-receipt.json'
$palette = Join-Path $EvidenceRoot 'palette.png'
$mediaReceipt = Join-Path $EvidenceRoot 'promo-media-receipt.json'
$gif = Join-Path $PSScriptRoot '..\docs\assets\demo\tabbeacon-demo.gif'
$poster = Join-Path $PSScriptRoot '..\docs\assets\demo\tabbeacon-demo-poster.png'
[System.IO.Directory]::CreateDirectory((Split-Path -Parent $gif)) | Out-Null

$env:CARGO_TARGET_DIR = 'V:\build\tabbeacon\codex-target'
cargo build --locked --features visual-fixture --bin tabbeacon-visual-fixture
if ($LASTEXITCODE -ne 0) { throw "visual fixture build failed: $LASTEXITCODE" }

$fixture = Join-Path $env:CARGO_TARGET_DIR 'debug\tabbeacon-visual-fixture.exe'
if (-not (Test-Path -LiteralPath $fixture -PathType Leaf)) { throw "visual fixture binary missing: $fixture" }

& $fixture promo --expected-head $ExpectedHead --run-id $RunId --frames-dir $frames --receipt $fixtureReceipt
if ($LASTEXITCODE -ne 0) { throw "controlled promotional capture failed: $LASTEXITCODE" }

$fixtureData = Get-Content -LiteralPath $fixtureReceipt -Raw | ConvertFrom-Json
if ($fixtureData.source_sha -ne $ExpectedHead -or $fixtureData.target_window_match_count -ne 1 -or
    -not $fixtureData.real_windows_terminal -or -not $fixtureData.real_tabbeacon_renderer -or
    $fixtureData.real_model_session -or $fixtureData.desktop_capture) {
    throw 'Promo fixture receipt violates the exact-window, renderer, or no-model contract.'
}
$frameFiles = @(Get-ChildItem -LiteralPath $frames -Filter 'frame-*.png' -File | Sort-Object Name)
if ($frameFiles.Count -ne $fixtureData.frame_count) { throw "Frame count mismatch: $($frameFiles.Count) vs $($fixtureData.frame_count)" }

$cropHeight = [Math]::Min(320, [int]$fixtureData.frame_height)
$scaleFilter = "fps=10,crop=iw:$cropHeight:0:0,scale=960:-2:flags=lanczos"
& $ffmpeg -hide_banner -loglevel error -framerate 10 -i (Join-Path $frames 'frame-%04d.png') -vf "$scaleFilter,palettegen=stats_mode=diff" -y $palette
if ($LASTEXITCODE -ne 0) { throw "FFmpeg palette generation failed: $LASTEXITCODE" }
& $ffmpeg -hide_banner -loglevel error -framerate 10 -i (Join-Path $frames 'frame-%04d.png') -i $palette -filter_complex "[0:v]$scaleFilter[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" -loop 0 -y $gif
if ($LASTEXITCODE -ne 0) { throw "FFmpeg GIF encoding failed: $LASTEXITCODE" }
& $ffmpeg -hide_banner -loglevel error -i $gif -frames:v 1 -y $poster
if ($LASTEXITCODE -ne 0) { throw "FFmpeg poster extraction failed: $LASTEXITCODE" }

$probe = & $ffprobe -v error -select_streams v:0 -show_entries stream=width,height,avg_frame_rate,nb_frames,duration -of json $gif | ConvertFrom-Json
$stream = $probe.streams[0]
$gifBytes = (Get-Item -LiteralPath $gif).Length
if ($gifBytes -gt 6MB) { throw "Promo GIF exceeds the 6 MiB hard limit: $gifBytes bytes" }
$posterDimensions = Get-ExactPngDimensions $poster
$receipt = [ordered]@{
    source_sha = $ExpectedHead
    windows_terminal_version = $fixtureData.windows_terminal_version
    ffmpeg_version = $ffmpegVersion
    frame_count = $fixtureData.frame_count
    fps = $fixtureData.fps
    duration_ms = $fixtureData.duration_ms
    output_dimensions = "$($stream.width)x$($stream.height)"
    gif_bytes = $gifBytes
    gif_sha256 = Get-Sha256 $gif
    target_window_match_count = $fixtureData.target_window_match_count
    promo_real_windows_terminal = $fixtureData.real_windows_terminal
    promo_real_tabbeacon_renderer = $fixtureData.real_tabbeacon_renderer
    promo_real_model_session = $fixtureData.real_model_session
    no_desktop_capture = -not $fixtureData.desktop_capture
    poster_dimensions = "$($posterDimensions[0])x$($posterDimensions[1])"
}
$receipt | ConvertTo-Json | Set-Content -LiteralPath $mediaReceipt -Encoding utf8NoBOM

Write-Output 'PROMO_GIF=PASS'
Write-Output 'PROMO_POSTER=PASS'
Write-Output "PROMO_GIF_BYTES=$gifBytes"
Write-Output "PROMO_GIF_SHA256=$($receipt.gif_sha256)"
Write-Output "PROMO_GIF_DIMENSIONS=$($receipt.output_dimensions)"
Write-Output "PROMO_MEDIA_RECEIPT=$mediaReceipt"
