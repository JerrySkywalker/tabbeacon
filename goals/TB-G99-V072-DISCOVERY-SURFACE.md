# TB-G99 — v0.7.2 GitHub Discovery Surface

## Purpose

Improve TabBeacon's GitHub-native discoverability without changing product
runtime behavior. A user who encounters the repository through search, Topics,
or a shared GitHub link should understand the product quickly and accurately.

## Preconditions

Start from the accepted v0.7.2 planning baseline and current public v0.7.1.
Re-read repository metadata and current README before mutation.

```text
CURRENT_PUBLIC_RELEASE=v0.7.1
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
NEW_PROVIDER_ADDED=false
```

## A. Repository description

Audit the current description for user value, search discoverability, and
truthfulness. Prefer a compact value proposition over internal architecture
language.

A candidate meaning is:

> See what your coding-agent tabs are doing in Windows Terminal — without
> changing how you launch them.

The exact wording is an implementation-time decision after checking GitHub
length/display behavior and consistency with README truth.

Required:

```text
DESCRIPTION_TRUTHFUL=true
DESCRIPTION_COMPACT=true
DESCRIPTION_PROVIDER_OVERCLAIM=false
```

## B. Topics

Research current GitHub Topic usage for relevant concepts and choose **6–10**
semantically accurate topics. Do not blindly apply every candidate and do not
keyword-stuff.

Candidate families include:

- coding agents / agentic coding;
- Codex CLI;
- Windows Terminal;
- terminal / CLI developer tools;
- Rust;
- Windows.

Record why each chosen topic is relevant and reject candidates that are stale,
unused, misleading, or too broad.

Required:

```text
GITHUB_TOPICS_COUNT=6..10
GITHUB_TOPICS_RELEVANT=true
TOPIC_KEYWORD_STUFFING=false
```

Apply description/topics through supported GitHub APIs or `gh`; no browser UI
automation is needed for these metadata fields.

## C. Social-preview source and render

Create deterministic TabBeacon-owned social-preview assets:

```text
docs/assets/social/tabbeacon-social-preview.svg
docs/assets/social/tabbeacon-social-preview.png
```

PNG contract:

```text
WIDTH=1280
HEIGHT=640
```

Design requirements:

- use the repaired TabBeacon mark/logo and visual language;
- use a compact user-value tagline;
- include only bounded synthetic state examples if useful;
- no OpenAI/Codex/Agy trademark-logo imitation;
- no external font/runtime font dependency;
- no script, remote image, external SVG, or active content;
- remain legible at social-card scale.

Prefer a deterministic local SVG/HTML render sheet and installed Edge headless
for PNG production. Add a focused generator such as:

```text
scripts/generate-social-preview.ps1
```

if doing so makes the render reproducible and low-maintenance.

## D. Social-preview upload boundary

Re-check GitHub's current supported API surface. If a supported write API exists,
use it. If not, do **not** automate the Settings UI with cookies, browser
sessions, DOM selectors, Playwright, or similar hacks.

Allowed final classification:

```text
SOCIAL_PREVIEW_UPLOAD=PASS
```

or

```text
SOCIAL_PREVIEW_UPLOAD=WAITING_OWNER_UI
```

The latter does not block the code/asset portion of G99 when the final 1280x640
asset is present and validated.

## E. Validation

At minimum verify:

```text
GITHUB_DESCRIPTION=PASS
GITHUB_TOPICS_COUNT=6..10
GITHUB_TOPICS_RELEVANT=true
SOCIAL_PREVIEW_SVG=PASS
SOCIAL_PREVIEW_PNG=PASS
SOCIAL_PREVIEW_DIMENSIONS=1280x640
SVG_WELL_FORMED=true
SVG_ACTIVE_CONTENT=false
EXTERNAL_FONT_DEPENDENCY=false
RUNTIME_BEHAVIOR_CHANGED=false
PROVIDER_BEHAVIOR_CHANGED=false
```

Run relevant docs/asset checks and fresh hosted exact-head CI if the repository
policy requires it for the final G99 PR head.

## Risk vector

```text
CODE_CHANGED=docs_scripts_metadata_only_expected
PRESENTATION_CHANGED=repository_public_surface
PROVIDER_CHANGED=false
USER_PERSISTENT_CONFIG_CHANGED=false
SECURITY_OR_PRIVACY_CHANGED=public_asset_review_only
RELEASE_BOUNDARY=false
```

## Exit

G99 is complete when GitHub metadata is truthfully improved, the social-preview
asset is reproducibly generated and validated, and no runtime/provider behavior
has changed.

Next: `TB-G100-V072-AUTOMATED-PROMO-DEMO.md`.
