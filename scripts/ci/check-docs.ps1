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
    return Get-Content -LiteralPath $Path -Raw
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
    'CONTRIBUTING.md',
    'SECURITY.md'
)

foreach ($path in $requiredFiles) {
    [void](Get-RequiredContent $path)
}

$englishReadme = Get-RequiredContent 'README.md'
$chineseReadme = Get-RequiredContent 'README.zh-CN.md'
Assert-Docs ($englishReadme -match 'href="README\.zh-CN\.md"') 'README.md must link to README.zh-CN.md'
Assert-Docs ($chineseReadme -match 'href="README\.md"') 'README.zh-CN.md must link to README.md'

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

$invariantMarker = '<!-- tabbeacon:critical-invariants install=cargo-install-tabbeacon-locked setup=tabbeacon-setup codex=codex agy=agy providers=codex-agy claude=deferred opencode=deferred trust=manual fail-open=true privacy=content-minimal -->'
Assert-Docs ($englishReadme.Contains($invariantMarker)) 'README.md is missing the critical EN/ZH invariant marker'
Assert-Docs ($chineseReadme.Contains($invariantMarker)) 'README.zh-CN.md is missing the critical EN/ZH invariant marker'

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
    'docs/development/build-and-test.md', 'docs/development/release-process.md'
)
foreach ($path in $currentFacingPaths) {
    $content = Get-Content -LiteralPath $path -Raw
    Assert-Docs ($content -notmatch '(?i)(current|latest|supported)\s+(public\s+|published\s+)?release[^\n]{0,80}v?0\.(2|6\.0|6\.1)') "$path contains a stale current release marker"
    Assert-Docs ($content -notmatch '(?i)TabBeacon\s+0\.6\.0\s+(supports|is|includes)') "$path contains stale v0.6.0 current-product wording"
}

$currentReleaseProofs = @(
    @{ Path = 'README.md'; Pattern = 'Current public release:\s+\*\*v0\.7\.0\*\*' },
    @{ Path = 'README.zh-CN.md'; Pattern = '当前公开版本：\*\*v0\.7\.0\*\*' },
    @{ Path = 'SECURITY.md'; Pattern = 'current published release is \*\*v0\.7\.0\*\*' },
    @{ Path = 'docs/README.md'; Pattern = 'Current public release:\s+\*\*v0\.7\.0\*\*' },
    @{ Path = 'docs/getting-started.md'; Pattern = 'current public release is \*\*v0\.7\.0\*\*' },
    @{ Path = 'docs/development/release-process.md'; Pattern = 'current public release is \*\*v0\.7\.0\*\*' }
)
foreach ($proof in $currentReleaseProofs) {
    $content = Get-Content -LiteralPath $proof.Path -Raw
    Assert-Docs ($content -match $proof.Pattern) "$($proof.Path) does not declare v0.7.0 as the current public release"
}

Write-Host 'README_BADGE_COUNT=2'
Write-Host 'README_BADGE_RUST=true'
Write-Host 'README_BADGE_WINDOWS_CI=true'
Write-Host 'README_AGENT_BADGES=false'
Write-Host 'CRITICAL_EN_ZH_INVARIANTS=PASS'
Write-Host 'REQUIRED_BRAND_ASSETS_EXIST=true'
Write-Host 'SVG_WELL_FORMED=true'
Write-Host 'SVG_ACTIVE_CONTENT=false'
Write-Host 'INTERNAL_MARKDOWN_LINKS_VALID=true'
Write-Host 'DOCS_PORTAL_LINKS_VALID=true'
Write-Host 'STALE_CURRENT_RELEASE_MARKERS=0'
Write-Host 'DOCS_CHECK=PASS'
