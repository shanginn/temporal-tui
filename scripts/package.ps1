param(
    [Parameter(Mandatory = $true)]
    [string]$Target,
    [string]$OutputDirectory = "dist",
    [string]$BinaryPath = "target/release/temporal-tui.exe"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Manifest = Get-Content (Join-Path $ProjectRoot "Cargo.toml") -Raw
if ($Manifest -notmatch '(?ms)^\[package\].*?^version = "([^"]+)"') {
    throw "could not read package version"
}
$Version = $Matches[1]
$ResolvedBinary = Join-Path $ProjectRoot $BinaryPath
if (-not (Test-Path $ResolvedBinary -PathType Leaf)) {
    throw "release binary is missing: $ResolvedBinary"
}
$ReportedVersion = & $ResolvedBinary --version
if ($ReportedVersion -ne "temporal-tui $Version") {
    throw "release binary version does not match Cargo.toml"
}

$PackageName = "temporal-tui-v$Version-$Target"
$TemporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
$PackageRoot = Join-Path $TemporaryRoot $PackageName
$ResolvedOutput = Join-Path $ProjectRoot $OutputDirectory

try {
    New-Item -ItemType Directory -Force (Join-Path $PackageRoot "completions") | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $PackageRoot "man") | Out-Null
    Copy-Item $ResolvedBinary (Join-Path $PackageRoot "temporal-tui.exe")
    Copy-Item (Join-Path $ProjectRoot "README.md") $PackageRoot
    Copy-Item (Join-Path $ProjectRoot "LICENSE") $PackageRoot
    Copy-Item (Join-Path $ProjectRoot "assets/man/*.1") (Join-Path $PackageRoot "man")
    Copy-Item (Join-Path $ProjectRoot "assets/completions/*") (Join-Path $PackageRoot "completions")

    New-Item -ItemType Directory -Force $ResolvedOutput | Out-Null
    $Archive = Join-Path $ResolvedOutput "$PackageName.zip"
    Compress-Archive -Path $PackageRoot -DestinationPath $Archive -Force
    Write-Output $Archive
}
finally {
    Remove-Item -Recurse -Force $TemporaryRoot -ErrorAction SilentlyContinue
}
