param(
    [string]$ExpectedHead = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Label,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Command
    )

    Write-Host "`n===== $Label ====="
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE."
    }
}

if ($ExpectedHead) {
    $ActualHead = (git rev-parse HEAD).Trim()
    if ($ActualHead -ne $ExpectedHead) {
        throw "Exact-head mismatch: expected=$ExpectedHead actual=$ActualHead"
    }
    Write-Host "CODE_HEAD=$ActualHead"
}

Invoke-Checked 'GIT DIFF CHECK' { git diff --check }
Invoke-Checked 'GIT COMMIT WHITESPACE CHECK' { git diff-tree --check --root HEAD }

$BadEol = @(
    git ls-files --eol |
        Where-Object {
            $_ -match '\bw/crlf\b' -and
            $_ -match '\beol=lf\b' -and
            $_ -notmatch '\.(cmd|bat)$'
        }
)

if ($BadEol.Count -gt 0) {
    $BadEol | ForEach-Object { Write-Host $_ }
    throw 'Line-ending policy failed: tracked LF files are checked out as CRLF.'
}
Write-Host 'EOL_GATE=PASS'

Invoke-Checked 'RUSTC VERSION' { rustc --version }
Invoke-Checked 'CARGO VERSION' { cargo --version }
Invoke-Checked 'FORMAT' { cargo fmt --all -- --check }
Invoke-Checked 'CLIPPY' { cargo clippy --all-targets --all-features -- -D warnings }
Invoke-Checked 'TEST' { cargo test --all-targets --all-features }
Invoke-Checked 'CODEX PRODUCTION HOOK SLA' {
    cargo test --release --locked --test codex_hooks anchored_default_windows_hook_declaration_finishes_within_the_one_second_sla -- --exact
}
Invoke-Checked 'LOCKED BUILD' { cargo build --locked --all-targets }
Invoke-Checked 'AGY PREADMISSION SCRIPT CONTRACT' { pwsh -NoProfile -File ./scripts/ci/test-agy-preadmission-contract.ps1 }

Write-Host "`nLOCAL_CI=PASS"
