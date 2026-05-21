# Build and run RohKai from this repository checkout.
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

cargo run
