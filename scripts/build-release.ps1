# Build an optimized release executable from this repository checkout.
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

cargo build --release

$Binary = Join-Path $ProjectRoot "target\release\rohkai.exe"
Write-Host "Binary at: $Binary"
