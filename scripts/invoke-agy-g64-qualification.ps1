[CmdletBinding()]
param(
    [ValidateSet('Plan', 'DirectVersion', 'TitleState', 'HookState', 'TitleCallback')]
    [string]$Mode = 'Plan',
    [ValidateSet('pre-tool-use', 'post-tool-use', 'pre-invocation', 'post-invocation', 'stop')]
    [string]$HookEvent = 'post-tool-use',
    [ValidatePattern('^v?\d{1,5}\.\d{1,5}\.\d{1,5}$')]
    [string]$DocumentedVersion = '1.1.14',
    [ValidateRange(1, 30)]
    [int]$TimeoutSeconds = 10,
    [ValidateRange(1, 65536)]
    [int]$MaxInputBytes = 65536,
    [string]$TabBeaconExecutablePath,
    [ValidatePattern('^[A-Fa-f0-9]{64}$')]
    [string]$TabBeaconExecutableSha256,
    [string]$AgyExecutablePath,
    [ValidatePattern('^[A-Fa-f0-9]{64}$')]
    [string]$AgyExecutableSha256,
    [string]$InputPath,
    [string]$DisposableRoot,
    [switch]$OwnerPresent
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Throw-QualificationError {
    param([Parameter(Mandatory = $true)][string]$Code)
    throw $Code
}

function Require-OwnerPresent {
    if (-not $OwnerPresent) {
        Throw-QualificationError 'OWNER_PRESENT_REQUIRED'
    }
}

function Resolve-VerifiedExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if ([string]::IsNullOrWhiteSpace($Path) -or [string]::IsNullOrWhiteSpace($ExpectedSha256)) {
        Throw-QualificationError "$Label`_EXECUTABLE_IDENTITY_REQUIRED"
    }
    if (-not [IO.Path]::IsPathFullyQualified($Path)) {
        Throw-QualificationError "$Label`_EXECUTABLE_PATH_MUST_BE_ABSOLUTE"
    }
    try {
        $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    } catch {
        Throw-QualificationError "$Label`_EXECUTABLE_UNAVAILABLE"
    }
    if ($item.PSIsContainer -or (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
        Throw-QualificationError "$Label`_EXECUTABLE_NOT_DIRECT"
    }
    if ($item.Extension -ine '.exe') {
        Throw-QualificationError "$Label`_EXECUTABLE_NOT_NATIVE"
    }
    try {
        $actualSha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256 -ErrorAction Stop).Hash
    } catch {
        Throw-QualificationError "$Label`_EXECUTABLE_UNREADABLE"
    }
    if ($actualSha256 -ine $ExpectedSha256) {
        Throw-QualificationError "$Label`_EXECUTABLE_IDENTITY_MISMATCH"
    }
    [IO.Path]::GetFullPath($item.FullName)
}

function Resolve-DisposableInput {
    Require-OwnerPresent
    if ([string]::IsNullOrWhiteSpace($InputPath) -or [string]::IsNullOrWhiteSpace($DisposableRoot)) {
        Throw-QualificationError 'OWNER_INPUT_REQUIRED'
    }
    if (-not [IO.Path]::IsPathFullyQualified($InputPath) -or -not [IO.Path]::IsPathFullyQualified($DisposableRoot)) {
        Throw-QualificationError 'DISPOSABLE_PATH_MUST_BE_ABSOLUTE'
    }
    try {
        $root = Get-Item -LiteralPath $DisposableRoot -Force -ErrorAction Stop
        $input = Get-Item -LiteralPath $InputPath -Force -ErrorAction Stop
    } catch {
        Throw-QualificationError 'OWNER_INPUT_REQUIRED'
    }
    if (-not $root.PSIsContainer -or $input.PSIsContainer) {
        Throw-QualificationError 'DISPOSABLE_PATH_KIND_INVALID'
    }
    $rootPath = [IO.Path]::GetFullPath($root.FullName)
    $inputPath = [IO.Path]::GetFullPath($input.FullName)
    $relative = [IO.Path]::GetRelativePath($rootPath, $inputPath)
    if ($relative -eq '..' -or $relative.StartsWith("..$([IO.Path]::DirectorySeparatorChar)") -or [IO.Path]::IsPathFullyQualified($relative)) {
        Throw-QualificationError 'DISPOSABLE_INPUT_OUTSIDE_ROOT'
    }
    $cursor = $inputPath
    while ($true) {
        try {
            $item = Get-Item -LiteralPath $cursor -Force -ErrorAction Stop
        } catch {
            Throw-QualificationError 'DISPOSABLE_INPUT_UNAVAILABLE'
        }
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            Throw-QualificationError 'DISPOSABLE_PATH_REPARSE_FORBIDDEN'
        }
        if ([string]::Equals($cursor, $rootPath, [StringComparison]::OrdinalIgnoreCase)) {
            break
        }
        $cursor = [IO.Path]::GetDirectoryName($cursor)
        if ([string]::IsNullOrEmpty($cursor)) {
            Throw-QualificationError 'DISPOSABLE_INPUT_OUTSIDE_ROOT'
        }
    }
    if ($input.Length -gt $MaxInputBytes) {
        Throw-QualificationError 'DISPOSABLE_INPUT_OVERSIZED'
    }
    $input
}

function Read-BoundedDisposableInput {
    param([Parameter(Mandatory = $true)][IO.FileInfo]$Input)

    $stream = $null
    try {
        $stream = [IO.File]::Open($Input.FullName, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        $buffer = New-Object byte[] ($MaxInputBytes + 1)
        $total = 0
        while ($total -lt $buffer.Length) {
            $read = $stream.Read($buffer, $total, $buffer.Length - $total)
            if ($read -eq 0) {
                break
            }
            $total += $read
        }
        if ($total -gt $MaxInputBytes) {
            Throw-QualificationError 'DISPOSABLE_INPUT_OVERSIZED'
        }
        [byte[]]$result = New-Object byte[] $total
        [Array]::Copy($buffer, $result, $total)
        return ,$result
    } catch {
        if ($_.Exception.Message -match '^(DISPOSABLE_INPUT_OVERSIZED)$') {
            throw
        }
        Throw-QualificationError 'DISPOSABLE_INPUT_UNREADABLE'
    } finally {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
    }
}

function Invoke-BoundedDirectVersion {
    param([Parameter(Mandatory = $true)][string]$ExecutablePath)

    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $ExecutablePath
    [void]$start.ArgumentList.Add('--version')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        $started = $process.Start()
    } catch {
        Throw-QualificationError 'AGY_DIRECT_VERSION_START_FAILED'
    }
    if (-not $started) {
        Throw-QualificationError 'AGY_DIRECT_VERSION_START_FAILED'
    }
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        try {
            $process.Kill($true)
        } catch {
            try {
                $process.Kill()
            } catch {
                Throw-QualificationError 'AGY_DIRECT_VERSION_STOP_FAILED'
            }
        }
        [void]$process.WaitForExit(1000)
        Throw-QualificationError 'AGY_DIRECT_VERSION_TIMEOUT'
    }
    [char[]]$buffer = New-Object char[] 129
    $total = 0
    while ($total -lt $buffer.Length) {
        $read = $process.StandardOutput.Read($buffer, $total, $buffer.Length - $total)
        if ($read -eq 0) {
            break
        }
        $total += $read
    }
    if ($total -gt 128) {
        Throw-QualificationError 'AGY_DIRECT_VERSION_OUTPUT_OVERSIZED'
    }
    $stdout = [string]::new($buffer, 0, $total).Trim()
    if ($process.ExitCode -ne 0 -or $stdout -notmatch '^v?\d{1,5}\.\d{1,5}\.\d{1,5}$') {
        Throw-QualificationError 'AGY_DIRECT_VERSION_INVALID'
    }
    $stdout
}

function Invoke-VerifiedTabBeacon {
    param(
        [Parameter(Mandatory = $true)][string]$ExecutablePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [byte[]]$InputBytes
    )

    try {
        if ($null -eq $InputBytes) {
            & $ExecutablePath @Arguments
        } else {
            $inputText = [Text.Encoding]::UTF8.GetString($InputBytes)
            $inputText | & $ExecutablePath @Arguments
        }
    } catch {
        Throw-QualificationError 'TABBEACON_QUALIFICATION_START_FAILED'
    }
    if ($LASTEXITCODE -ne 0) {
        Throw-QualificationError 'TABBEACON_QUALIFICATION_FAILED'
    }
}

$tabbeacon = Resolve-VerifiedExecutable -Path $TabBeaconExecutablePath -ExpectedSha256 $TabBeaconExecutableSha256 -Label 'TABBEACON'

switch ($Mode) {
    'Plan' {
        Invoke-VerifiedTabBeacon -ExecutablePath $tabbeacon -Arguments @('agy', 'plan', '--json')
    }
    'DirectVersion' {
        $agy = Resolve-VerifiedExecutable -Path $AgyExecutablePath -ExpectedSha256 $AgyExecutableSha256 -Label 'AGY'
        $version = Invoke-BoundedDirectVersion -ExecutablePath $agy
        Invoke-VerifiedTabBeacon -ExecutablePath $tabbeacon -Arguments @('agy', 'version', '--observed', $version, '--documented', $DocumentedVersion, '--json')
    }
    'TitleState' {
        $input = Resolve-DisposableInput
        Invoke-VerifiedTabBeacon -ExecutablePath $tabbeacon -Arguments @('agy', 'title-state', '--json') -InputBytes (Read-BoundedDisposableInput -Input $input)
    }
    'HookState' {
        $input = Resolve-DisposableInput
        Invoke-VerifiedTabBeacon -ExecutablePath $tabbeacon -Arguments @('agy', 'hook-state', $HookEvent, '--json') -InputBytes (Read-BoundedDisposableInput -Input $input)
    }
    'TitleCallback' {
        $input = Resolve-DisposableInput
        Invoke-VerifiedTabBeacon -ExecutablePath $tabbeacon -Arguments @('agy', '__title-callback-v1') -InputBytes (Read-BoundedDisposableInput -Input $input)
    }
}
