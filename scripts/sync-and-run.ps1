# Optional local helper: sync another working copy into this checkout, then run.
param(
    [Parameter(Mandatory = $true)]
    [string]$SourcePath
)

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$ExcludeFile = Join-Path $PSScriptRoot "xcopy-exclude.txt"

xcopy $SourcePath $ProjectRoot /E /I /H /Y /EXCLUDE:$ExcludeFile
Set-Location $ProjectRoot
cargo run
