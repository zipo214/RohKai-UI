# Optional local helper: sync another working copy into this checkout, then run.
param(
    [Parameter(Mandatory = $true)]
    [string]$SourcePath,

    [switch]$AllowOverwrite
)

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$ExcludeFile = Join-Path $PSScriptRoot "xcopy-exclude.txt"

if (-not $AllowOverwrite) {
    Write-Error "sync-and-run.ps1 overwrites this checkout. Re-run with -AllowOverwrite only after confirming the source tree is authoritative."
    exit 1
}

xcopy $SourcePath $ProjectRoot /E /I /H /Y /EXCLUDE:$ExcludeFile
Set-Location $ProjectRoot
cargo run
