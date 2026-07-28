param(
    [Parameter(Mandatory = $true)]
    [string]$Archive
)

$ErrorActionPreference = "Stop"
$TemporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
$ExtractDirectory = Join-Path $TemporaryRoot "extract"
$Prefix = Join-Path $TemporaryRoot "prefix"
$Config = Join-Path $TemporaryRoot "config.toml"
$ConfigV2 = Join-Path $TemporaryRoot "config-v2.toml"

try {
    New-Item -ItemType Directory -Force $ExtractDirectory | Out-Null
    New-Item -ItemType Directory -Force (Join-Path $Prefix "bin") | Out-Null
    Expand-Archive -Path $Archive -DestinationPath $ExtractDirectory
    $PackageRoot = Get-ChildItem $ExtractDirectory -Directory | Select-Object -First 1
    if ($null -eq $PackageRoot) {
        throw "archive does not contain a package directory"
    }

    $PackagedBinary = Join-Path $PackageRoot.FullName "temporal-tui.exe"
    @(
        $PackagedBinary,
        (Join-Path $PackageRoot.FullName "man/temporal-tui.1"),
        (Join-Path $PackageRoot.FullName "man/temporal-tui-auth.1"),
        (Join-Path $PackageRoot.FullName "man/temporal-tui-auth-login.1"),
        (Join-Path $PackageRoot.FullName "completions/temporal-tui.bash"),
        (Join-Path $PackageRoot.FullName "completions/_temporal-tui"),
        (Join-Path $PackageRoot.FullName "completions/temporal-tui.fish"),
        (Join-Path $PackageRoot.FullName "completions/_temporal-tui.ps1"),
        (Join-Path $PackageRoot.FullName "completions/temporal-tui.elv")
    ) | ForEach-Object {
        if (-not (Test-Path $_ -PathType Leaf)) {
            throw "package file is missing: $_"
        }
    }

    $InstalledBinary = Join-Path $Prefix "bin/temporal-tui.exe"
    Copy-Item $PackagedBinary $InstalledBinary
    & $InstalledBinary --version
    & $InstalledBinary --help | Out-Null

    [System.IO.File]::WriteAllText($Config, "schema_version = 1`n")
    $env:TEMPORAL_TUI_CONFIG = $Config
    & $InstalledBinary filter list
    if ((Get-Content $Config -Raw) -notmatch '(?m)^schema_version = 3$') {
        throw "schema-1 config was not migrated"
    }
    if ((Get-Content "$Config.v1.bak" -Raw) -ne "schema_version = 1`n") {
        throw "schema-1 backup is not byte-identical"
    }

    [System.IO.File]::WriteAllText($ConfigV2, "schema_version = 2`n")
    $env:TEMPORAL_TUI_CONFIG = $ConfigV2
    & $InstalledBinary filter list
    if ((Get-Content $ConfigV2 -Raw) -notmatch '(?m)^schema_version = 3$') {
        throw "schema-2 config was not migrated"
    }
    if ((Get-Content "$ConfigV2.v2.bak" -Raw) -ne "schema_version = 2`n") {
        throw "schema-2 backup is not byte-identical"
    }

    Remove-Item $InstalledBinary
    if (Test-Path $InstalledBinary) {
        throw "binary remains after uninstall"
    }
    if (
        -not (Test-Path $Config) -or
        -not (Test-Path "$Config.v1.bak") -or
        -not (Test-Path $ConfigV2) -or
        -not (Test-Path "$ConfigV2.v2.bak")
    ) {
        throw "uninstall removed recoverable user configuration"
    }
}
finally {
    Remove-Item Env:TEMPORAL_TUI_CONFIG -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $TemporaryRoot -ErrorAction SilentlyContinue
}
