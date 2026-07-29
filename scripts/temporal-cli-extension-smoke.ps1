param(
    [Parameter(Mandatory = $true)]
    [string]$TemporalCli,

    [Parameter(Mandatory = $true)]
    [string]$TuiBinary,

    [Parameter(Mandatory = $true)]
    [string]$ExpectedVersion
)

$ErrorActionPreference = "Stop"
$ExpectedVersion = $ExpectedVersion.TrimStart("v")
$TemporalCli = (Resolve-Path $TemporalCli).Path
$TuiBinary = (Resolve-Path $TuiBinary).Path
$TemporaryRoot = Join-Path (
    [System.IO.Path]::GetTempPath()
) ([System.Guid]::NewGuid())
$OriginalPath = $env:PATH
$OriginalPathExt = $env:PATHEXT
$OriginalConfig = $env:TEMPORAL_TUI_CONFIG
$OriginalHome = $env:HOME
$OriginalUserProfile = $env:USERPROFILE
$OriginalAppData = $env:APPDATA
$OriginalLocalAppData = $env:LOCALAPPDATA
$OriginalNativeErrorPreference = $PSNativeCommandUseErrorActionPreference
$PSNativeCommandUseErrorActionPreference = $false

try {
    $ExtensionDirectory = Join-Path $TemporaryRoot "bin"
    New-Item -ItemType Directory -Force $ExtensionDirectory | Out-Null
    $IsolatedHome = Join-Path $TemporaryRoot "home"
    $IsolatedAppData = Join-Path $TemporaryRoot "appdata"
    $IsolatedLocalAppData = Join-Path $TemporaryRoot "local-appdata"
    New-Item -ItemType Directory -Force $IsolatedHome | Out-Null
    New-Item -ItemType Directory -Force $IsolatedAppData | Out-Null
    New-Item -ItemType Directory -Force $IsolatedLocalAppData | Out-Null
    $env:HOME = $IsolatedHome
    $env:USERPROFILE = $IsolatedHome
    $env:APPDATA = $IsolatedAppData
    $env:LOCALAPPDATA = $IsolatedLocalAppData
    $InstalledBinary = Join-Path $ExtensionDirectory "temporal-tui.exe"
    Copy-Item $TuiBinary $InstalledBinary
    $env:PATH = "$ExtensionDirectory$(
        [System.IO.Path]::PathSeparator
    )$OriginalPath"
    if ([string]::IsNullOrWhiteSpace($env:PATHEXT)) {
        $env:PATHEXT = ".EXE;.BAT"
    }

    $CliVersion = (& $TemporalCli --version | Out-String).Trim()
    if (
        $LASTEXITCODE -ne 0 -or
        $CliVersion -notmatch "^temporal version 1\.8\.1\b"
    ) {
        throw "expected Temporal CLI 1.8.1, got: $CliVersion"
    }

    $VersionOutput = (& $TemporalCli tui --version | Out-String).Trim()
    if (
        $LASTEXITCODE -ne 0 -or
        $VersionOutput -ne "temporal-tui $ExpectedVersion"
    ) {
        throw "unexpected extension version: $VersionOutput"
    }

    $TimeoutVersion = (
        & $TemporalCli tui --command-timeout 5s --version |
            Out-String
    ).Trim()
    if (
        $LASTEXITCODE -ne 0 -or
        $TimeoutVersion -ne "temporal-tui $ExpectedVersion"
    ) {
        throw "Temporal CLI command timeout was not accepted: $TimeoutVersion"
    }

    $TimeoutConfig = Join-Path $TemporaryRoot "timeout-config.toml"
    $TimeoutConfigOutput = (
        & $TemporalCli tui --command-timeout 5s `
            --config $TimeoutConfig config-path |
            Out-String
    ).Trim()
    if ($LASTEXITCODE -ne 0 -or $TimeoutConfigOutput -ne $TimeoutConfig) {
        throw "Temporal CLI command timeout blocked a non-interactive command"
    }

    $HelpOutput = (& $TemporalCli tui --help | Out-String)
    if (
        $LASTEXITCODE -ne 0 -or
        $HelpOutput -notmatch "terminal dashboard and control plane for Temporal"
    ) {
        throw "Temporal CLI did not forward extension help"
    }

    $AllHelpOutput = (& $TemporalCli help --all | Out-String)
    if (
        $LASTEXITCODE -ne 0 -or
        $AllHelpOutput -notmatch "(?m)^\s{2}tui\s"
    ) {
        throw "Temporal CLI did not discover the tui extension"
    }

    $Config = Join-Path $TemporaryRoot "config.toml"
    $ConfigOutput = (
        & $TemporalCli tui --config $Config config-path |
            Out-String
    ).Trim()
    if ($LASTEXITCODE -ne 0 -or $ConfigOutput -ne $Config) {
        throw "Temporal CLI did not forward extension arguments"
    }

    $ProfileConfigOutput = (
        & $TemporalCli tui --profile rubase --config $Config config-path |
            Out-String
    ).Trim()
    if ($LASTEXITCODE -ne 0 -or $ProfileConfigOutput -ne $Config) {
        throw "Temporal CLI did not forward its --profile flag"
    }

    $EnvConfig = Join-Path $TemporaryRoot "env-config.toml"
    $env:TEMPORAL_TUI_CONFIG = $EnvConfig
    $EnvConfigOutput = (& $TemporalCli tui config-path | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $EnvConfigOutput -ne $EnvConfig) {
        throw "Temporal CLI did not preserve the extension environment"
    }

    $AuthHelp = (
        & $TemporalCli tui --profile rubase auth whoami --help |
            Out-String
    )
    if (
        $LASTEXITCODE -ne 0 -or
        $AuthHelp -notmatch "current signed-in identity and session status"
    ) {
        throw "Temporal CLI did not forward the auth subcommand"
    }

    $InvalidOutput = (
        & $TemporalCli tui --definitely-invalid 2>&1 |
            Out-String
    )
    $InvalidExitCode = $LASTEXITCODE
    if ($InvalidExitCode -ne 2) {
        throw "extension exit code was not preserved: $InvalidExitCode"
    }
    if ($InvalidOutput -notmatch "unexpected argument '--definitely-invalid'") {
        throw "extension stderr was not preserved"
    }

    $InvalidTimeoutOutput = (
        & $TemporalCli tui --command-timeout invalid --version 2>&1 |
            Out-String
    )
    $InvalidTimeoutExitCode = $LASTEXITCODE
    if ($InvalidTimeoutExitCode -ne 1) {
        throw (
            "invalid parent timeout returned $InvalidTimeoutExitCode, " +
            "expected 1"
        )
    }
    if (
        $InvalidTimeoutOutput -notmatch
        'invalid argument "invalid" for "--command-timeout"'
    ) {
        throw "Temporal CLI did not validate an invalid command timeout"
    }

    $InteractiveTimeoutOutput = (
        & $TemporalCli tui --command-timeout 5s `
            --config $Config 2>&1 |
            Out-String
    )
    $InteractiveTimeoutExitCode = $LASTEXITCODE
    if ($InteractiveTimeoutExitCode -ne 1) {
        throw (
            "interactive timeout returned $InteractiveTimeoutExitCode, " +
            "expected 1"
        )
    }
    if (
        $InteractiveTimeoutOutput -notmatch
        "cannot safely interrupt the dashboard"
    ) {
        throw "interactive command timeout did not fail safely"
    }

    $global:LASTEXITCODE = 0
    Write-Output (
        "Temporal CLI extension smoke test passed for temporal-tui " +
        $ExpectedVersion
    )
}
finally {
    $PSNativeCommandUseErrorActionPreference = $OriginalNativeErrorPreference
    $env:PATH = $OriginalPath
    if ($null -eq $OriginalPathExt) {
        Remove-Item Env:PATHEXT -ErrorAction SilentlyContinue
    }
    else {
        $env:PATHEXT = $OriginalPathExt
    }
    if ($null -eq $OriginalConfig) {
        Remove-Item Env:TEMPORAL_TUI_CONFIG -ErrorAction SilentlyContinue
    }
    else {
        $env:TEMPORAL_TUI_CONFIG = $OriginalConfig
    }
    foreach ($Variable in @(
        @{ Name = "HOME"; Value = $OriginalHome },
        @{ Name = "USERPROFILE"; Value = $OriginalUserProfile },
        @{ Name = "APPDATA"; Value = $OriginalAppData },
        @{ Name = "LOCALAPPDATA"; Value = $OriginalLocalAppData }
    )) {
        if ($null -eq $Variable.Value) {
            Remove-Item "Env:$($Variable.Name)" -ErrorAction SilentlyContinue
        }
        else {
            Set-Item "Env:$($Variable.Name)" $Variable.Value
        }
    }
    Remove-Item -Recurse -Force $TemporaryRoot -ErrorAction SilentlyContinue
}
