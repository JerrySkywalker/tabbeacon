[CmdletBinding()]
param(
    [ValidateSet('Plan', 'DirectVersion', 'TitleState', 'HookState', 'TitleCallback')]
    [string]$Mode = 'Plan',
    [ValidateSet('pre-tool-use', 'post-tool-use', 'pre-invocation', 'post-invocation', 'stop')]
    [string]$HookEvent = 'post-tool-use',
    [string]$InputPath,
    [string]$DocumentedVersion = '1.1.14',
    [switch]$OwnerPresent
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Require-OwnerPresent {
    if (-not $OwnerPresent) {
        throw 'OWNER_PRESENT_REQUIRED: this sample path is reserved for the real G64 admission spike.'
    }
}

function Require-DisposableInput {
    Require-OwnerPresent
    if ([string]::IsNullOrWhiteSpace($InputPath)) {
        throw 'OWNER_INPUT_REQUIRED: provide one explicitly approved disposable capture path.'
    }
    if (-not (Test-Path -LiteralPath $InputPath -PathType Leaf)) {
        throw 'OWNER_INPUT_REQUIRED: the explicitly approved disposable capture is unavailable.'
    }
}

switch ($Mode) {
    'Plan' {
        & tabbeacon agy plan --json
        exit $LASTEXITCODE
    }
    'DirectVersion' {
        [void](Get-Command agy -CommandType Application -ErrorAction Stop)
        $version = & agy --version
        if ($LASTEXITCODE -ne 0) {
            throw 'AGY_DIRECT_VERSION_FAILED'
        }
        & tabbeacon agy version --observed $version --documented $DocumentedVersion --json
        exit $LASTEXITCODE
    }
    'TitleState' {
        Require-DisposableInput
        Get-Content -LiteralPath $InputPath -Raw | & tabbeacon agy title-state --json
        exit $LASTEXITCODE
    }
    'HookState' {
        Require-DisposableInput
        Get-Content -LiteralPath $InputPath -Raw | & tabbeacon agy hook-state $HookEvent --json
        exit $LASTEXITCODE
    }
    'TitleCallback' {
        Require-DisposableInput
        Get-Content -LiteralPath $InputPath -Raw | & tabbeacon agy __title-callback-v1
        exit $LASTEXITCODE
    }
}
