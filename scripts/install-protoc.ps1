param(
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
$Version = "35.1"
$Asset = "protoc-$Version-win64.zip"
$ExpectedSha = "5d3ff218d7d91eea95f7569bcb5a98f3030f8996d44151279d9772edcff76082"
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    if (-not [string]::IsNullOrWhiteSpace($env:TEMPORAL_PROTOC_OUTPUT)) {
        $OutputDirectory = $env:TEMPORAL_PROTOC_OUTPUT
    }
    else {
        $OutputDirectory = Join-Path $ProjectRoot ".tools/protoc-$Version"
    }
}
if (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
    $OutputDirectory = Join-Path $ProjectRoot $OutputDirectory
}

$Binary = Join-Path $OutputDirectory "bin/protoc.exe"
$ReportedVersion = ""
if (Test-Path $Binary -PathType Leaf) {
    $ReportedVersion = (& $Binary --version | Out-String).Trim()
}

if ($ReportedVersion -ne "libprotoc $Version") {
    New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
    $Archive = Join-Path $OutputDirectory $Asset
    Invoke-WebRequest `
        -Uri "https://github.com/protocolbuffers/protobuf/releases/download/v$Version/$Asset" `
        -OutFile $Archive
    $ActualSha = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualSha -ne $ExpectedSha) {
        throw "checksum mismatch for $Asset"
    }
    Expand-Archive -Path $Archive -DestinationPath $OutputDirectory -Force
    $ReportedVersion = (& $Binary --version | Out-String).Trim()
}

if ($ReportedVersion -ne "libprotoc $Version") {
    throw "unexpected protoc version: $ReportedVersion"
}
if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_PATH)) {
    Add-Content -Path $env:GITHUB_PATH -Value (Join-Path $OutputDirectory "bin")
}
Write-Output $ReportedVersion
