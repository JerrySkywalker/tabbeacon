[CmdletBinding()]
param(
    [ValidateSet('Capture', 'Encode')]
    [string]$Mode = 'Capture',
    [string]$ExpectedHead = (git rev-parse HEAD).Trim(),
    [string]$RunId = ("TB-V072-PROMO-" + (Get-Date -Format 'yyyyMMddHHmmss')),
    [string]$EvidenceRoot,
    [string]$PrivacyReviewReceipt
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-ExactPngDimensions([string]$Path) {
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Image]::FromFile($Path)
    try { return @($image.Width, $image.Height) } finally { $image.Dispose() }
}

function Resolve-ExactOwnedEvidenceRoot([string]$SafeRunId, [string]$RequestedRoot) {
    $ownedBuildRoot = [System.IO.Path]::GetFullPath('V:\build\tabbeacon')
    $expectedRoot = [System.IO.Path]::GetFullPath((Join-Path $ownedBuildRoot $SafeRunId))
    $actualRoot = [System.IO.Path]::GetFullPath($RequestedRoot)
    if (-not $actualRoot.Equals($expectedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "EvidenceRoot must be the exact owned path: $expectedRoot"
    }
    $buildItem = Get-Item -LiteralPath $ownedBuildRoot -Force -ErrorAction Stop
    if ($buildItem.LinkType) { throw "Refusing reparse-point build root: $ownedBuildRoot" }
    return $expectedRoot
}

function Assert-NormalExistingPath([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if ($item.LinkType) { throw "Refusing reparse-point path: $($item.FullName)" }
    return $item
}

function Resolve-FfmpegTools {
    $ffmpegCommand = Get-Command ffmpeg -ErrorAction SilentlyContinue | Select-Object -First 1
    $ffprobeCommand = Get-Command ffprobe -ErrorAction SilentlyContinue | Select-Object -First 1
    $source = 'existing'
    if ($null -eq $ffmpegCommand -or $null -eq $ffprobeCommand) {
        $winget = Get-Command winget -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $winget) { throw 'FFmpeg/ffprobe are unavailable and winget is unavailable for the authorized Gyan.FFmpeg install.' }
        & $winget.Source install --id Gyan.FFmpeg -e --source winget --accept-package-agreements --accept-source-agreements
        if ($LASTEXITCODE -ne 0) { throw "Authorized Gyan.FFmpeg winget install failed: $LASTEXITCODE" }
        $ffmpegCommand = Get-Command ffmpeg -ErrorAction SilentlyContinue | Select-Object -First 1
        $ffprobeCommand = Get-Command ffprobe -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $ffmpegCommand -or $null -eq $ffprobeCommand) {
            throw 'Gyan.FFmpeg completed but ffmpeg/ffprobe are not resolvable in this process; do not mutate PATH.'
        }
        $source = 'winget:Gyan.FFmpeg'
    }
    return [pscustomobject]@{ Ffmpeg = $ffmpegCommand.Source; Ffprobe = $ffprobeCommand.Source; Source = $source }
}

function Read-FixtureReceipt([string]$Path, [string]$Head, [string]$FramesPath) {
    Assert-NormalExistingPath $Path | Out-Null
    $fixtureData = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($fixtureData.source_sha -ne $Head -or $fixtureData.target_window_match_count -ne 1 -or
        -not $fixtureData.real_windows_terminal -or -not $fixtureData.real_tabbeacon_renderer -or
        $fixtureData.real_model_session -or $fixtureData.desktop_capture -or -not $fixtureData.controlled_fixture_only) {
        throw 'Promo fixture receipt violates the exact-window, renderer, controlled-fixture, or no-model contract.'
    }
    $frameFiles = @(Get-ChildItem -LiteralPath $FramesPath -Filter 'frame-*.png' -File | Sort-Object Name)
    if ($frameFiles.Count -ne $fixtureData.frame_count) { throw "Frame count mismatch: $($frameFiles.Count) vs $($fixtureData.frame_count)" }
    if ($fixtureData.fps -ne 10 -or $fixtureData.duration_ms -lt 8000 -or $fixtureData.duration_ms -gt 12000) {
        throw 'Promo fixture receipt violates the 8-12 second, 10 FPS timing contract.'
    }
    return [pscustomobject]@{ Receipt = $fixtureData; Frames = $frameFiles }
}

function Assert-PrivacyReview([string]$Path, [string]$Head, [object]$FixtureData, [string]$FramesPath) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "PROMO_PRIVACY_REVIEW=REQUIRED: inspect representative and boundary frames, then create the content-minimal receipt at $Path"
    }
    Assert-NormalExistingPath $Path | Out-Null
    $privacy = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($privacy.source_sha -ne $Head -or $privacy.frame_count -ne $FixtureData.frame_count -or $privacy.review_disposition -ne 'PASS') {
        throw 'Privacy review receipt does not bind PASS to this exact capture.'
    }
    foreach ($name in @('owner_prompt_visible', 'owner_assistant_content_visible', 'owner_tool_content_visible', 'owner_username_visible', 'private_path_visible', 'private_repository_visible', 'unrelated_window_visible', 'private_content_visible')) {
        $property = $privacy.PSObject.Properties[$name]
        if ($null -eq $property -or $property.Value -ne $false) { throw "Privacy review receipt must prove $name=false." }
    }
    foreach ($name in @('first_frame', 'working_frame', 'attention_frame', 'loop_boundary_frame')) {
        $frameName = [string]$privacy.PSObject.Properties[$name].Value
        if ($frameName -notmatch '^frame-\d{4}\.png$' -or -not (Test-Path -LiteralPath (Join-Path $FramesPath $frameName) -PathType Leaf)) {
            throw "Privacy review receipt has no valid $name representative frame."
        }
    }
}

function Get-GifFrameRate([string]$Rate) {
    if ($Rate -notmatch '^(\d+)/(\d+)$') { throw "Unexpected GIF frame rate: $Rate" }
    $parts = $Rate.Split('/')
    if ([double]$parts[1] -eq 0) { throw "Invalid GIF frame rate: $Rate" }
    return [double]$parts[0] / [double]$parts[1]
}

function Assert-InfiniteGifLoop([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $needle = [System.Text.Encoding]::ASCII.GetBytes('NETSCAPE2.0')
    for ($offset = 0; $offset -le $bytes.Length - $needle.Length - 5; $offset++) {
        $matched = $true
        for ($index = 0; $index -lt $needle.Length; $index++) {
            if ($bytes[$offset + $index] -ne $needle[$index]) { $matched = $false; break }
        }
        if ($matched -and $bytes[$offset + $needle.Length] -eq 3 -and $bytes[$offset + $needle.Length + 1] -eq 1 -and
            $bytes[$offset + $needle.Length + 2] -eq 0 -and $bytes[$offset + $needle.Length + 3] -eq 0 -and
            $bytes[$offset + $needle.Length + 4] -eq 0) { return }
    }
    throw 'Promo GIF does not contain an infinite NETSCAPE2.0 loop extension.'
}

if ($ExpectedHead -notmatch '^[0-9a-f]{40}$') { throw 'ExpectedHead must be a lowercase full SHA.' }
if ($RunId -notmatch '^[A-Za-z0-9-]{1,64}$') { throw 'RunId must be a safe bounded identifier.' }
if ([string]::IsNullOrWhiteSpace($EvidenceRoot)) { $EvidenceRoot = Join-Path 'V:\build\tabbeacon' $RunId }
$EvidenceRoot = Resolve-ExactOwnedEvidenceRoot $RunId $EvidenceRoot
if ([string]::IsNullOrWhiteSpace($PrivacyReviewReceipt)) { $PrivacyReviewReceipt = Join-Path $EvidenceRoot 'promo-privacy-review.json' }
if ([System.IO.Path]::GetFullPath($PrivacyReviewReceipt) -ne [System.IO.Path]::GetFullPath((Join-Path $EvidenceRoot 'promo-privacy-review.json'))) {
    throw 'PrivacyReviewReceipt must remain inside the exact owned evidence root.'
}

$checkedOutHead = (git rev-parse HEAD).Trim()
if ($checkedOutHead -ne $ExpectedHead) { throw "EXPECTED_HEAD mismatch: expected $ExpectedHead, checked out $checkedOutHead" }

$frames = Join-Path $EvidenceRoot 'frames'
$fixtureReceipt = Join-Path $EvidenceRoot 'promo-fixture-receipt.json'
$palette = Join-Path $EvidenceRoot 'palette.png'
$mediaReceipt = Join-Path $EvidenceRoot 'promo-media-receipt.json'
$temporaryGif = Join-Path $EvidenceRoot 'tabbeacon-demo.gif'
$temporaryPoster = Join-Path $EvidenceRoot 'tabbeacon-demo-poster.png'
$gif = Join-Path $PSScriptRoot '..\docs\assets\demo\tabbeacon-demo.gif'
$poster = Join-Path $PSScriptRoot '..\docs\assets\demo\tabbeacon-demo-poster.png'

if ($Mode -eq 'Capture') {
    if (Test-Path -LiteralPath $EvidenceRoot) { throw "Refusing to reuse evidence root: $EvidenceRoot" }
    [System.IO.Directory]::CreateDirectory($EvidenceRoot) | Out-Null
    Assert-NormalExistingPath $EvidenceRoot | Out-Null
    $env:CARGO_TARGET_DIR = 'V:\build\tabbeacon\codex-target'
    cargo build --locked --features visual-fixture --bin tabbeacon-visual-fixture
    if ($LASTEXITCODE -ne 0) { throw "visual fixture build failed: $LASTEXITCODE" }
    $fixture = Join-Path $env:CARGO_TARGET_DIR 'debug\tabbeacon-visual-fixture.exe'
    if (-not (Test-Path -LiteralPath $fixture -PathType Leaf)) { throw "visual fixture binary missing: $fixture" }
    & $fixture promo --expected-head $ExpectedHead --run-id $RunId --frames-dir $frames --receipt $fixtureReceipt
    if ($LASTEXITCODE -ne 0) { throw "controlled promotional capture failed: $LASTEXITCODE" }
    $fixtureProof = Read-FixtureReceipt $fixtureReceipt $ExpectedHead $frames
    Write-Output "FRAME_COUNT=$($fixtureProof.Frames.Count)"
    Write-Output "PROMO_FRAME_REVIEW_REQUIRED=$PrivacyReviewReceipt"
    exit 0
}

Assert-NormalExistingPath $EvidenceRoot | Out-Null
$fixtureProof = Read-FixtureReceipt $fixtureReceipt $ExpectedHead $frames
Assert-PrivacyReview $PrivacyReviewReceipt $ExpectedHead $fixtureProof.Receipt $frames
if (Test-Path -LiteralPath $gif -PathType Leaf -or Test-Path -LiteralPath $poster -PathType Leaf) {
    throw 'Refusing to overwrite a committed promo asset; use a fresh source transaction instead.'
}
foreach ($path in @($palette, $temporaryGif, $temporaryPoster, $mediaReceipt)) {
    if (Test-Path -LiteralPath $path) { throw "Refusing to overwrite evidence artifact: $path" }
}

$ffmpegTools = Resolve-FfmpegTools
$ffmpegVersion = (& $ffmpegTools.Ffmpeg -version | Select-Object -First 1).Trim()
$cropHeight = [Math]::Min(320, [int]$fixtureProof.Receipt.frame_height)
$scaleFilter = "fps=10,crop=iw:${cropHeight}:0:0,scale=960:-2:flags=lanczos"
& $ffmpegTools.Ffmpeg -hide_banner -loglevel error -framerate 10 -i (Join-Path $frames 'frame-%04d.png') -vf "$scaleFilter,palettegen=stats_mode=diff" -y $palette
if ($LASTEXITCODE -ne 0) { throw "FFmpeg palette generation failed: $LASTEXITCODE" }
& $ffmpegTools.Ffmpeg -hide_banner -loglevel error -framerate 10 -i (Join-Path $frames 'frame-%04d.png') -i $palette -filter_complex "[0:v]$scaleFilter[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" -loop 0 -y $temporaryGif
if ($LASTEXITCODE -ne 0) { throw "FFmpeg GIF encoding failed: $LASTEXITCODE" }
& $ffmpegTools.Ffmpeg -hide_banner -loglevel error -i $temporaryGif -frames:v 1 -y $temporaryPoster
if ($LASTEXITCODE -ne 0) { throw "FFmpeg poster extraction failed: $LASTEXITCODE" }

$probe = & $ffmpegTools.Ffprobe -v error -select_streams v:0 -show_entries stream=width,height,avg_frame_rate,duration -of json $temporaryGif | ConvertFrom-Json
$stream = $probe.streams[0]
$gifFps = Get-GifFrameRate ([string]$stream.avg_frame_rate)
$durationMs = [Math]::Round(([double]$stream.duration) * 1000)
$gifBytes = (Get-Item -LiteralPath $temporaryGif).Length
$posterDimensions = Get-ExactPngDimensions $temporaryPoster
if ($stream.width -lt 960 -or $stream.width -gt 1100 -or $stream.height -lt 260 -or $stream.height -gt 360) { throw "Promo GIF dimensions violate the composition contract: $($stream.width)x$($stream.height)" }
if ([Math]::Abs($gifFps - 10) -gt 0.01) { throw "Promo GIF FPS is $gifFps, expected 10." }
if ($durationMs -lt 8000 -or $durationMs -gt 12000) { throw "Promo GIF duration is $durationMs ms, expected 8-12 seconds." }
if ($posterDimensions[0] -ne $stream.width -or $posterDimensions[1] -ne $stream.height) { throw 'Promo poster dimensions do not match the GIF.' }
if ($gifBytes -gt 6MB) { throw "Promo GIF exceeds the 6 MiB hard limit: $gifBytes bytes" }
if ($gifBytes -gt 4MB) { throw "Promo GIF misses the 4 MiB target: $gifBytes bytes" }
Assert-InfiniteGifLoop $temporaryGif

[System.IO.Directory]::CreateDirectory((Split-Path -Parent $gif)) | Out-Null
Copy-Item -LiteralPath $temporaryGif -Destination $gif -ErrorAction Stop
Copy-Item -LiteralPath $temporaryPoster -Destination $poster -ErrorAction Stop
$receipt = [ordered]@{
    source_sha = $ExpectedHead
    windows_terminal_version = $fixtureProof.Receipt.windows_terminal_version
    ffmpeg_version = $ffmpegVersion
    ffmpeg_source = $ffmpegTools.Source
    frame_count = $fixtureProof.Receipt.frame_count
    fps = $gifFps
    duration_ms = $durationMs
    output_dimensions = "$($stream.width)x$($stream.height)"
    gif_bytes = $gifBytes
    gif_sha256 = Get-Sha256 $gif
    gif_loop = $true
    target_window_match_count = $fixtureProof.Receipt.target_window_match_count
    promo_real_windows_terminal = $fixtureProof.Receipt.real_windows_terminal
    promo_real_tabbeacon_renderer = $fixtureProof.Receipt.real_tabbeacon_renderer
    promo_real_model_session = $fixtureProof.Receipt.real_model_session
    no_desktop_capture = -not $fixtureProof.Receipt.desktop_capture
    private_content_visible = $false
    promo_privacy_review = 'PASS'
    poster_dimensions = "$($posterDimensions[0])x$($posterDimensions[1])"
}
[System.IO.File]::WriteAllText($mediaReceipt, ($receipt | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))

Write-Output 'PROMO_GIF=PASS'
Write-Output 'PROMO_POSTER=PASS'
Write-Output "FFMPEG_SOURCE=$($ffmpegTools.Source)"
Write-Output "PROMO_GIF_BYTES=$gifBytes"
Write-Output "PROMO_GIF_SHA256=$($receipt.gif_sha256)"
Write-Output "PROMO_GIF_DURATION_MS=$durationMs"
Write-Output "PROMO_GIF_FPS=$gifFps"
Write-Output "PROMO_GIF_DIMENSIONS=$($receipt.output_dimensions)"
Write-Output "PROMO_MEDIA_RECEIPT=$mediaReceipt"
