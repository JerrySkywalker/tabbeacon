[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Verifier
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Equal {
    param([string]$Actual, [string]$Expected, [string]$Label)
    if ($Actual -ne $Expected) {
        throw "$Label expected '$Expected' but received '$Actual'."
    }
}

function Invoke-Fixture {
    param([string]$Name, [string]$Source, [bool]$Json)

    $fixtureCargoHome = Join-Path $script:root $Name
    $null = New-Item -ItemType Directory -Path $fixtureCargoHome
    $entry = "tabbeacon 0.5.2 ($Source)"
    if ($Json) {
        @{ installs = @{ $entry = @{ bins = @('tabbeacon.exe') } } } |
            ConvertTo-Json -Depth 8 |
            Set-Content -LiteralPath (Join-Path $fixtureCargoHome '.crates2.json') -Encoding utf8NoBOM
    } else {
        "[v1]`n`"$entry`" = [`"tabbeacon.exe`"]" |
            Set-Content -LiteralPath (Join-Path $fixtureCargoHome '.crates.toml') -Encoding utf8NoBOM
    }
    @(& $Verifier -CargoHome $fixtureCargoHome -Package tabbeacon -Version 0.5.2)
}

$script:root = Join-Path ([IO.Path]::GetTempPath()) ("tabbeacon-cargo-source-fixtures-" + [Guid]::NewGuid())
$null = New-Item -ItemType Directory -Path $script:root
try {
    $cases = @(
        @{ Name = 'registry'; Source = 'registry+https://github.com/rust-lang/crates.io-index'; Json = $true; Expected = 'REGISTRY_OFFICIAL' },
        @{ Name = 'git'; Source = 'git+https://example.invalid/tabbeacon?rev=abc#abc'; Json = $false; Expected = 'GIT_REVISION' },
        @{ Name = 'path'; Source = 'path+file:///fixture/tabbeacon'; Json = $false; Expected = 'LOCAL_PATH' },
        @{ Name = 'unknown'; Source = 'registry+https://example.invalid/index'; Json = $false; Expected = 'UNKNOWN_OR_UNPROVEN' }
    )
    foreach ($case in $cases) {
        $output = Invoke-Fixture -Name $case.Name -Source $case.Source -Json $case.Json
        Assert-Equal -Actual ($output | Where-Object { $_ -like 'OWNER_INSTALL_SOURCE=*' }) -Expected "OWNER_INSTALL_SOURCE=$($case.Expected)" -Label $case.Name
        Assert-Equal -Actual ($output | Where-Object { $_ -like 'OWNER_INSTALL_SOURCE_PROVEN=*' }) -Expected "OWNER_INSTALL_SOURCE_PROVEN=$(($case.Expected -ne 'UNKNOWN_OR_UNPROVEN').ToString().ToLowerInvariant())" -Label "$($case.Name) proof"
        Assert-Equal -Actual ($output | Where-Object { $_ -like 'OWNER_GIT_REV_INSTALL=*' }) -Expected "OWNER_GIT_REV_INSTALL=$(($case.Expected -eq 'GIT_REVISION').ToString().ToLowerInvariant())" -Label "$($case.Name) git"
        Assert-Equal -Actual ($output | Where-Object { $_ -like 'OWNER_OFFICIAL_CHANNEL=*' }) -Expected 'OWNER_OFFICIAL_CHANNEL=crates.io' -Label "$($case.Name) channel"
    }
    Write-Output 'CARGO_INSTALL_SOURCE_VERIFIER=PASS'
} finally {
    if (Test-Path -LiteralPath $script:root) { Remove-Item -LiteralPath $script:root -Recurse -Force }
}
