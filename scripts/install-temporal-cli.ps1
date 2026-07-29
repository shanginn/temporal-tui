param(
    [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"
$Version = if ([string]::IsNullOrWhiteSpace($env:TEMPORAL_CLI_VERSION)) {
    "1.8.1"
}
else {
    $env:TEMPORAL_CLI_VERSION
}
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    if (-not [string]::IsNullOrWhiteSpace($env:TEMPORAL_CLI_OUTPUT)) {
        $OutputPath = $env:TEMPORAL_CLI_OUTPUT
    }
    else {
        $OutputPath = Join-Path $ProjectRoot ".tools/bin/temporal.exe"
    }
}
if (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $ProjectRoot $OutputPath
}

$Architecture = switch (
    [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
) {
    "X64" { "amd64" }
    "Arm64" { "arm64" }
    default {
        throw "unsupported architecture: $(
            [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
        )"
    }
}

$Asset = "temporal_cli_${Version}_windows_${Architecture}.zip"
$ReleaseUrl = "https://github.com/temporalio/cli/releases/download/v$Version"
$TemporaryRoot = Join-Path (
    [System.IO.Path]::GetTempPath()
) ([System.Guid]::NewGuid())

try {
    New-Item -ItemType Directory -Force $TemporaryRoot | Out-Null
    $Archive = Join-Path $TemporaryRoot $Asset
    $Checksums = Join-Path $TemporaryRoot "checksums.txt"
    Invoke-WebRequest -Uri "$ReleaseUrl/$Asset" -OutFile $Archive
    Invoke-WebRequest -Uri "$ReleaseUrl/checksums.txt" -OutFile $Checksums

    $ExpectedSha = ""
    foreach ($Line in Get-Content $Checksums) {
        $Parts = $Line -split '\s+'
        if (
            $Parts.Count -ge 2 -and
            $Parts[-1].TrimStart("*") -eq $Asset
        ) {
            $ExpectedSha = $Parts[0].ToLowerInvariant()
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($ExpectedSha)) {
        throw "release checksum for $Asset was not found"
    }

    $ActualSha = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualSha -ne $ExpectedSha) {
        throw "checksum mismatch for $Asset"
    }

    $ExtractDirectory = Join-Path $TemporaryRoot "extract"
    Expand-Archive -Path $Archive -DestinationPath $ExtractDirectory
    $ExtractedBinary = Join-Path $ExtractDirectory "temporal.exe"
    if (-not (Test-Path $ExtractedBinary -PathType Leaf)) {
        throw "$Asset does not contain temporal.exe"
    }

    New-Item -ItemType Directory -Force (
        Split-Path -Parent $OutputPath
    ) | Out-Null
    Copy-Item $ExtractedBinary $OutputPath -Force

    $ReportedVersion = (& $OutputPath --version | Out-String).Trim()
    $EscapedVersion = [System.Text.RegularExpressions.Regex]::Escape($Version)
    if (
        $LASTEXITCODE -ne 0 -or
        $ReportedVersion -notmatch "\b$EscapedVersion\b"
    ) {
        throw "unexpected Temporal CLI version: $ReportedVersion"
    }
    Write-Output $ReportedVersion
}
finally {
    Remove-Item -Recurse -Force $TemporaryRoot -ErrorAction SilentlyContinue
}
