# Build an optimized release executable from this repository checkout.
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

cargo build --release

$Binary = Join-Path $ProjectRoot "target\release\rohkai.exe"
Write-Host "Binary at: $Binary"
