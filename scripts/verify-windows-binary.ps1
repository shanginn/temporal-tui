param(
    [Parameter(Mandatory = $true)]
    [string]$Binary
)

$ErrorActionPreference = "Stop"
$Binary = (Resolve-Path $Binary).Path
$DumpbinCommand = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
$Dumpbin = if ($null -ne $DumpbinCommand) {
    $DumpbinCommand.Source
}
else {
    $VsWhere = Join-Path ${env:ProgramFiles(x86)} (
        "Microsoft Visual Studio/Installer/vswhere.exe"
    )
    if (-not (Test-Path $VsWhere -PathType Leaf)) {
        throw "dumpbin.exe and vswhere.exe are unavailable"
    }
    $VisualStudio = (
        & $VsWhere `
            -latest `
            -products * `
            -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
            -property installationPath |
            Out-String
    ).Trim()
    if ([string]::IsNullOrWhiteSpace($VisualStudio)) {
        throw "Visual Studio C++ tools are unavailable"
    }
    $Candidates = Get-ChildItem `
        (Join-Path $VisualStudio "VC/Tools/MSVC") `
        -Filter dumpbin.exe `
        -File `
        -Recurse |
        Where-Object {
            $_.FullName -match 'Hostx64[\\/]x64[\\/]dumpbin\.exe$'
        } |
        Sort-Object FullName -Descending
    if ($Candidates.Count -eq 0) {
        throw "dumpbin.exe was not found under $VisualStudio"
    }
    $Candidates[0].FullName
}

$Dependencies = (& $Dumpbin /DEPENDENTS $Binary 2>&1 | Out-String)
if ($LASTEXITCODE -ne 0) {
    throw "dumpbin failed for $Binary`n$Dependencies"
}
if ($Dependencies -notmatch '(?im)^\s*KERNEL32\.dll\s*$') {
    throw "dumpbin output does not contain the expected PE dependencies"
}
if (
    $Dependencies -match
    '(?im)^\s*(?:VCRUNTIME|MSVCP|UCRTBASE)[^\s]*\.dll\s*$'
) {
    throw "Windows release binary requires an external VC/UCRT runtime`n$Dependencies"
}

Write-Output "Windows static CRT dependency check passed"
