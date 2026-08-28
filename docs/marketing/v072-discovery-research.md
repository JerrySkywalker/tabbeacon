# v0.7.2 discovery-surface research

This note records the bounded research used for TB-G99. Counts are GitHub
repository-search observations captured on 2026-08-29; they guide relevance,
not popularity claims about TabBeacon.

## Repository description

```text
DESCRIPTION_BEFORE=Live identity and status beacons for coding-agent tabs in Windows Terminal.
DESCRIPTION_AFTER=Live status for coding-agent tabs in Windows Terminal — no launcher required.
```

The proposed description is shorter, retains the product's Windows Terminal and
coding-agent scope, and truthfully states the no-launcher invariant without
claiming support for deferred providers.

## Topics

| Topic | Observed repositories | Decision | Reason |
| --- | ---: | --- | --- |
| `coding-agents` | 3,426 | select | Directly describes the tabs TabBeacon helps distinguish. |
| `codex-cli` | 2,417 | select | Codex CLI is a production-supported integration. |
| `ai-coding` | 5,758 | select | Common, user-facing discovery language for this product category. |
| `windows-terminal` | 543 | select | Exact presentation surface; it prevents a cross-terminal overclaim. |
| `terminal` | 28,003 | select | Broad but accurate terminal-tool discovery category. |
| `cli` | 120,875 | select | TabBeacon is a Rust command-line tool. |
| `rust` | 119,476 | select | Accurate implementation and crates.io ecosystem signal. |
| `windows` | 62,683 | select | Accurate supported platform signal. |
| `agentic-coding` | 1,703 | reject | Semantically overlapping with `coding-agents`; omit rather than stuff. |
| `developer-tools` | 60,255 | reject | Accurate but too broad once the more precise tool topics are present. |

The selected set has eight topics and is deliberately limited to product scope,
platform, delivery form, and one admitted provider integration.

## Social-preview upload boundary

GitHub's documented social-preview workflow remains a repository Settings UI
upload. No supported repository social-preview write endpoint was identified in
the official REST documentation during this research. The committed 1280x640
PNG is therefore the final code asset; upload is an Owner UI action rather than
browser-session automation.
