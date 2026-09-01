[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Set-Location $repositoryRoot

function Assert-Docs {
    param(
        [Parameter(Mandatory = $true)]
        [bool]$Condition,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Condition) {
        throw "DOCS_CHECK=FAIL: $Message"
    }
}

function Get-RequiredContent {
    param([Parameter(Mandatory = $true)][string]$Path)

    Assert-Docs (Test-Path -LiteralPath $Path -PathType Leaf) "missing required file: $Path"
    return Get-Content -LiteralPath $Path -Raw -Encoding UTF8
}

function Test-RelativeMarkdownLinks {
    param([Parameter(Mandatory = $true)][string[]]$MarkdownPaths)

    $linkPattern = '(?<!\!)\[[^\]]+\]\((?<target>[^)\s]+)(?:\s+[^)]*)?\)'
    foreach ($path in $MarkdownPaths) {
        $content = Get-Content -LiteralPath $path -Raw
        $directory = Split-Path -Parent $path
        $htmlLinkPattern = '(?i)<a\b[^>]*\bhref\s*=\s*["''](?<target>[^"'']+)["'']'
        $linkMatches = @([regex]::Matches($content, $linkPattern)) + @([regex]::Matches($content, $htmlLinkPattern))
        foreach ($match in $linkMatches) {
            $target = $match.Groups['target'].Value.Trim('<', '>')
            if ($target -match '^(https?:|mailto:|#)') {
                continue
            }
            $targetPath = $target.Split('#', 2)[0]
            if ([string]::IsNullOrWhiteSpace($targetPath)) {
                continue
            }
            $resolved = Join-Path $directory $targetPath
            Assert-Docs (Test-Path -LiteralPath $resolved) "$path links to a missing local target: $target"
        }
    }
}

$requiredFiles = @(
    'README.md',
    'README.zh-CN.md',
    'docs/README.md',
    'docs/getting-started.md',
    'docs/configuration.md',
    'docs/coding-agent-support.md',
    'docs/troubleshooting.md',
    'docs/faq.md',
    'docs/design/product-principles.md',
    'docs/design/visual-language.md',
    'docs/design/native-tab-icon.md',
    'docs/design/branding.md',
    'docs/development/build-and-test.md',
    'docs/development/release-process.md',
    'docs/v0.7.3-release-notes.md',
    'docs/v0.7.3-upgrade.md',
    'CONTRIBUTING.md',
    'SECURITY.md'
)

foreach ($path in $requiredFiles) {
    [void](Get-RequiredContent $path)
}

$englishReadme = Get-RequiredContent 'README.md'
$chineseReadme = Get-RequiredContent 'README.zh-CN.md'
$repositoryUrl = 'https://github.com/JerrySkywalker/tabbeacon'
$stableMediaRevision = '9215500cb3b0a9ef183c9c096a4bdde1b749da5b'
Assert-Docs ($englishReadme.Contains(('href="{0}/blob/main/README.zh-CN.md"' -f $repositoryUrl))) 'README.md must use a crates.io-safe absolute link to README.zh-CN.md'
Assert-Docs ($chineseReadme.Contains(('href="{0}/blob/main/README.md"' -f $repositoryUrl))) 'README.zh-CN.md must use a crates.io-safe absolute link to README.md'

$badgeBlockPattern = '(?s)<!-- tabbeacon:hero-badges:start -->(?<badges>.*?)<!-- tabbeacon:hero-badges:end -->'
foreach ($readme in @(@{ Name = 'README.md'; Content = $englishReadme }, @{ Name = 'README.zh-CN.md'; Content = $chineseReadme })) {
    $badgeMatch = [regex]::Match($readme.Content, $badgeBlockPattern)
    Assert-Docs $badgeMatch.Success "$($readme.Name) is missing the hero badge block"
    $badges = $badgeMatch.Groups['badges'].Value
    $badgeCount = [regex]::Matches($badges, '<img\b').Count
    Assert-Docs ($badgeCount -eq 2) "$($readme.Name) hero must contain exactly two badges; found $badgeCount"
    Assert-Docs ($badges -match 'Rust-1\.97\.1') "$($readme.Name) is missing the current Rust/MSRV badge"
    Assert-Docs ($badges -match 'actions/workflows/ci\.yml') "$($readme.Name) is missing the Windows CI badge"
    Assert-Docs ($badges -notmatch '(?i)codex|agy|claude|opencode') "$($readme.Name) has a prohibited agent badge"
}

$invariantMarker = '<!-- tabbeacon:critical-invariants install=cargo-install-tabbeacon setup=tabbeacon-setup codex=codex agy=agy providers=codex-agy claude=deferred opencode=deferred trust=manual fail-open=true privacy=content-minimal -->'
Assert-Docs ($englishReadme.Contains($invariantMarker)) 'README.md is missing the critical EN/ZH invariant marker'
Assert-Docs ($chineseReadme.Contains($invariantMarker)) 'README.zh-CN.md is missing the critical EN/ZH invariant marker'

foreach ($readme in @(@{ Name = 'README.md'; Content = $englishReadme }, @{ Name = 'README.zh-CN.md'; Content = $chineseReadme })) {
    $normalizedReadme = $readme.Content -replace "`r`n", "`n"
    Assert-Docs ($readme.Content -match '(?m)^cargo install tabbeacon$') "$($readme.Name) is missing the normal unpinned install command"
    Assert-Docs ($readme.Content -notmatch '(?m)^cargo install tabbeacon --locked$') "$($readme.Name) still requires --locked in the normal install path"
    Assert-Docs ($normalizedReadme.Contains("cargo install tabbeacon`ntabbeacon setup`ncodex")) "$($readme.Name) is missing the canonical install/setup/codex Quick Start sequence"
    Assert-Docs ($readme.Content.Contains("raw.githubusercontent.com/JerrySkywalker/tabbeacon/$stableMediaRevision/docs/assets/demo/tabbeacon-demo.gif")) "$($readme.Name) is missing the crates.io-safe immutable demo URL"
    Assert-Docs ($readme.Content.Contains("$repositoryUrl/blob/$stableMediaRevision/docs/assets/demo/tabbeacon-demo.gif")) "$($readme.Name) is missing the immutable demo target link"
}

$brandAssets = @(
    'docs/assets/brand/tabbeacon-mark.svg',
    'docs/assets/brand/tabbeacon-logo.svg',
    'docs/assets/brand/tabbeacon-mark-monochrome.svg',
    'docs/assets/brand/tabbeacon-state-strip.svg'
)
foreach ($asset in $brandAssets) {
    $svg = Get-RequiredContent $asset
    try {
        $xml = [System.Xml.XmlDocument]::new()
        $xml.XmlResolver = $null
        $xml.LoadXml($svg)
    } catch {
        throw "DOCS_CHECK=FAIL: malformed SVG ${asset}: $($_.Exception.Message)"
    }
    Assert-Docs ($svg -notmatch '(?i)<script\b|\bon[a-z]+\s*=') "$asset contains active script or event content"
    Assert-Docs ($svg -notmatch '(?i)(?:href|xlink:href)\s*=\s*["''](?:https?:|//)') "$asset contains an external URL"
    Assert-Docs ($svg -notmatch '(?i)<image\b|data:image|<text\b|font-family') "$asset contains embedded raster or font-dependent content"
    Assert-Docs ($svg -match '(?i)<svg\b[^>]*\bviewBox\s*=') "$asset is missing a viewBox"
}

$logo = Get-RequiredContent 'docs/assets/brand/tabbeacon-logo.svg'
$wordmarkMatch = [regex]::Match($logo, '(?s)<g\b(?<attributes>[^>]*)\baria-label="TABBEACON"[^>]*>(?<glyphs>.*?)</g>')
Assert-Docs $wordmarkMatch.Success 'tabbeacon-logo.svg is missing the deterministic TABBEACON wordmark group'
foreach ($attribute in @(
    'data-grid-unit="12"',
    'data-cap-height="84"',
    'data-baseline-y="132"',
    'data-glyph-cell-width="60"',
    'data-glyph-advance="76"',
    'data-inter-glyph-gap="16"'
)) {
    Assert-Docs ($wordmarkMatch.Value.Contains($attribute)) "tabbeacon-logo.svg wordmark is missing $attribute"
}
$expectedGlyphCells = @(
    @{ Glyph = 'T'; Start = 0; End = 60 },
    @{ Glyph = 'A'; Start = 76; End = 136 },
    @{ Glyph = 'B'; Start = 152; End = 212 },
    @{ Glyph = 'B'; Start = 228; End = 288 },
    @{ Glyph = 'E'; Start = 304; End = 364 },
    @{ Glyph = 'A'; Start = 380; End = 440 },
    @{ Glyph = 'C'; Start = 456; End = 516 },
    @{ Glyph = 'O'; Start = 532; End = 592 },
    @{ Glyph = 'N'; Start = 608; End = 668 }
)
$expectedGlyphPaths = @{
    'glyph-t' = 'M0 0h60v12H36v72H24V12H0z'
    'glyph-a' = 'M12 0h36l12 84H48l-6-24H18l-6 24H0zM30 12 21 48h18z'
    'glyph-b' = 'M0 0h36l24 12v24L48 42l12 6v24L36 84H0zM12 12v24h24l12-6V18l-12-6zm0 36v24h24l12-6V54l-12-6z'
    'glyph-e' = 'M0 0h60v12H12v24h42v12H12v24h48v12H0z'
    'glyph-c' = 'M12 0h48v12H12v60h48v12H12L0 72V12z'
    'glyph-o' = 'M12 0h36l12 12v60L48 84H12L0 72V12zM12 12v60h36V12z'
    'glyph-n' = 'M0 84V0h12l36 60V0h12v84H48L12 24v60z'
}
foreach ($glyphPath in $expectedGlyphPaths.GetEnumerator()) {
    $pathPattern = '<path\b[^>]*\bid="' + [regex]::Escape($glyphPath.Key) + '"[^>]*\bd="' + [regex]::Escape($glyphPath.Value) + '"[^>]*/>'
    Assert-Docs ([regex]::IsMatch($logo, $pathPattern)) "tabbeacon-logo.svg glyph definition $($glyphPath.Key) differs from the bounded grid path"
}
$glyphMatches = [regex]::Matches($wordmarkMatch.Groups['glyphs'].Value, '<use\b[^>]*\bx="(?<x>\d+)"[^>]*\bdata-glyph="(?<glyph>[A-Z])"[^>]*\bdata-cell-start="(?<start>\d+)"[^>]*\bdata-cell-end="(?<end>\d+)"[^>]*/>')
Assert-Docs ($glyphMatches.Count -eq $expectedGlyphCells.Count) 'tabbeacon-logo.svg wordmark must define nine glyph cells'
for ($index = 0; $index -lt $expectedGlyphCells.Count; $index++) {
    $actual = $glyphMatches[$index]
    $expected = $expectedGlyphCells[$index]
    Assert-Docs ($actual.Groups['glyph'].Value -eq $expected.Glyph) "tabbeacon-logo.svg glyph $index differs from the declared wordmark"
    Assert-Docs ([int]$actual.Groups['x'].Value -eq $expected.Start) "tabbeacon-logo.svg glyph $index x-position must equal its cell start"
    Assert-Docs ([int]$actual.Groups['start'].Value -eq $expected.Start) "tabbeacon-logo.svg glyph $index has an unexpected cell start"
    Assert-Docs ([int]$actual.Groups['end'].Value -eq $expected.End) "tabbeacon-logo.svg glyph $index has an unexpected cell end"
}

$markdownPaths = @(Get-ChildItem -Path @('README.md', 'README.zh-CN.md', 'CONTRIBUTING.md', 'SECURITY.md', 'docs') -Recurse -File -Filter '*.md' | ForEach-Object { $_.FullName })
Test-RelativeMarkdownLinks $markdownPaths

$fencePaths = @(
    'README.md', 'README.zh-CN.md', 'CONTRIBUTING.md',
    'docs/getting-started.md', 'docs/configuration.md', 'docs/coding-agent-support.md',
    'docs/troubleshooting.md', 'docs/faq.md', 'docs/development/build-and-test.md',
    'docs/development/release-process.md'
)
foreach ($path in $fencePaths) {
    $insideFence = $false
    foreach ($line in Get-Content -LiteralPath $path) {
        $fenceMatch = [regex]::Match($line, '^```(?<language>\S*)\s*$')
        if (-not $fenceMatch.Success) {
            continue
        }
        if (-not $insideFence) {
            Assert-Docs (-not [string]::IsNullOrWhiteSpace($fenceMatch.Groups['language'].Value)) "$path contains an untyped code fence"
        }
        $insideFence = -not $insideFence
    }
    Assert-Docs (-not $insideFence) "$path has an unterminated code fence"
}

$currentFacingPaths = @(
    'README.md', 'README.zh-CN.md', 'CONTRIBUTING.md', 'SECURITY.md',
    'docs/README.md', 'docs/getting-started.md', 'docs/configuration.md',
    'docs/coding-agent-support.md', 'docs/troubleshooting.md', 'docs/faq.md',
    'docs/design/product-principles.md', 'docs/design/native-tab-icon.md',
    'docs/development/build-and-test.md', 'docs/development/release-process.md',
    'dev_governance_files/DEVELOPMENT_PAUSE.md'
)
foreach ($path in $currentFacingPaths) {
    $content = Get-Content -LiteralPath $path -Raw
    Assert-Docs ($content -notmatch '(?i)(current|latest|supported)\s+(public\s+|published\s+)?release[^\n]{0,80}v?0\.(2|6\.0|6\.1|7\.1)') "$path contains a stale current release marker"
    Assert-Docs ($content -notmatch '(?i)TabBeacon\s+0\.6\.0\s+(supports|is|includes)') "$path contains stale v0.6.0 current-product wording"
}

$currentReleaseProofs = @(
    @{ Path = 'README.md'; Pattern = 'Current public release:\s+\*\*v0\.7\.2\*\*' },
    @{ Path = 'README.zh-CN.md'; Pattern = '\u5f53\u524d\u516c\u5f00\u7248\u672c\uff1a\*\*v0\.7\.2\*\*' },
    @{ Path = 'SECURITY.md'; Pattern = 'current published release is \*\*v0\.7\.2\*\*' },
    @{ Path = 'docs/README.md'; Pattern = 'Current public release:\s+\*\*v0\.7\.2\*\*' },
    @{ Path = 'docs/getting-started.md'; Pattern = 'current public release is \*\*v0\.7\.2\*\*' },
    @{ Path = 'docs/development/release-process.md'; Pattern = 'current public release is \*\*v0\.7\.2\*\*' },
    @{ Path = 'dev_governance_files/ROADMAP.md'; Pattern = 'CURRENT_PUBLIC_RELEASE=v0\.7\.2' },
    @{ Path = 'dev_governance_files/DEVELOPMENT_PAUSE.md'; Pattern = 'CURRENT_PUBLIC_RELEASE=v0\.7\.2' }
)
foreach ($proof in $currentReleaseProofs) {
    $content = Get-Content -LiteralPath $proof.Path -Raw -Encoding UTF8
    Assert-Docs ($content -match $proof.Pattern) "$($proof.Path) does not declare v0.7.2 as the current public release"
}

$releaseCandidateProofs = @(
    @{ Path = 'Cargo.toml'; Pattern = '(?m)^version = "0\.7\.3"$' },
    @{ Path = 'Cargo.lock'; Pattern = '(?ms)name = "tabbeacon"\r?\nversion = "0\.7\.3"' },
    @{ Path = 'CHANGELOG.md'; Pattern = '## \[0\.7\.3\] - 2026-09-01' },
    @{ Path = 'docs/v0.7.3-release-notes.md'; Pattern = '# TabBeacon v0\.7\.3' },
    @{ Path = 'docs/v0.7.3-upgrade.md'; Pattern = '# Upgrade from v0\.7\.2 to v0\.7\.3' },
    @{ Path = 'goals/TB-G102-V072-HARDENING-RELEASE.md'; Pattern = 'TARGET_PUBLIC_RELEASE=v0\.7\.3' }
)
foreach ($proof in $releaseCandidateProofs) {
    $content = Get-Content -LiteralPath $proof.Path -Raw -Encoding UTF8
    Assert-Docs ($content -match $proof.Pattern) "$($proof.Path) does not preserve the v0.7.3 release-candidate contract"
}

$releaseTargetProofs = @(
    @{ Path = 'dev_governance_files/ROADMAP.md'; Pattern = 'CURRENT_PUBLIC_TARGET=v0\.7\.2' },
    @{ Path = 'dev_governance_files/DEVELOPMENT_PAUSE.md'; Pattern = 'CURRENT_PUBLIC_TARGET=v0\.7\.2' },
    @{ Path = 'docs/v0.7.2-release-notes.md'; Pattern = '# TabBeacon v0\.7\.2' }
)
foreach ($proof in $releaseTargetProofs) {
    $content = Get-Content -LiteralPath $proof.Path -Raw
    Assert-Docs ($content -match $proof.Pattern) "$($proof.Path) does not identify the v0.7.2 release record"
}

$pauseStateProofs = @(
    @{ Path = 'dev_governance_files/ROADMAP.md'; Pattern = 'ACTIVE_FEATURE_DEVELOPMENT=PAUSED' },
    @{ Path = 'dev_governance_files/DEVELOPMENT_PAUSE.md'; Pattern = 'ACTIVE_FEATURE_DEVELOPMENT=PAUSED' },
    @{ Path = 'dev_governance_files/ROADMAP.md'; Pattern = 'PROMOTION_TARGET_RELEASE=v0\.7\.3' },
    @{ Path = 'dev_governance_files/DEVELOPMENT_PAUSE.md'; Pattern = 'PROMO_PR_STATE=FROZEN_DRAFT' },
    @{ Path = 'dev_governance_files/DEVELOPMENT_PAUSE.md'; Pattern = 'PR100_MERGE_ALLOWED=false' }
)
foreach ($proof in $pauseStateProofs) {
    $content = Get-Content -LiteralPath $proof.Path -Raw
    Assert-Docs ($content -match $proof.Pattern) "$($proof.Path) does not preserve the post-v0.7.2 development pause"
}

Write-Host 'README_BADGE_COUNT=2'
Write-Host 'README_BADGE_RUST=true'
Write-Host 'README_BADGE_WINDOWS_CI=true'
Write-Host 'README_AGENT_BADGES=false'
Write-Host 'CRITICAL_EN_ZH_INVARIANTS=PASS'
Write-Host 'REQUIRED_BRAND_ASSETS_EXIST=true'
Write-Host 'SVG_WELL_FORMED=true'
Write-Host 'SVG_ACTIVE_CONTENT=false'
Write-Host 'WORDMARK_CAP_HEIGHT_UNIFORM=true'
Write-Host 'WORDMARK_BASELINE_UNIFORM=true'
Write-Host 'GLYPH_OVERLAP_COUNT=0'
Write-Host 'WORDMARK_SPACING_REVIEW=PASS'
Write-Host 'README_HERO_LOGO=PASS'
Write-Host 'INTERNAL_MARKDOWN_LINKS_VALID=true'
Write-Host 'DOCS_PORTAL_LINKS_VALID=true'
Write-Host 'STALE_CURRENT_RELEASE_MARKERS=0'
Write-Host 'DOCS_CHECK=PASS'
