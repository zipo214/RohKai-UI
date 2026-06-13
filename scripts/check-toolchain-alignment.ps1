# Verify that RohKai's compiler and generated-project version contract stays aligned.
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom
$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

function Require-Match {
    param(
        [string]$Text,
        [string]$Pattern,
        [string]$Description
    )

    $match = [regex]::Match($Text, $Pattern, [System.Text.RegularExpressions.RegexOptions]::Multiline)
    if (-not $match.Success) {
        throw "Unable to read $Description."
    }
    return $match.Groups[1].Value
}

$cargo = Get-Content -Raw -Encoding utf8 -Path "Cargo.toml"
$toolchain = Get-Content -Raw -Encoding utf8 -Path "rust-toolchain.toml"
$workflow = Get-Content -Raw -Encoding utf8 -Path ".github\workflows\windows-build.yml"
$export = Get-Content -Raw -Encoding utf8 -Path "src\codegen\export.rs"

$edition = Require-Match $cargo '^\s*edition\s*=\s*"([^"]+)"' "Cargo edition"
$msrv = Require-Match $cargo '^\s*rust-version\s*=\s*"([^"]+)"' "Cargo rust-version"
$toolchainVersion = Require-Match $toolchain '^\s*channel\s*=\s*"([^"]+)"' "pinned toolchain"
$ciVersion = Require-Match $workflow 'dtolnay/rust-toolchain@([0-9]+\.[0-9]+\.[0-9]+)' "CI toolchain"
$designerEframe = Require-Match $cargo '^\s*eframe\s*=\s*\{\s*version\s*=\s*"([^"]+)"' "designer eframe version"
$designerEgui = Require-Match $cargo '^\s*egui\s*=\s*"([^"]+)"' "designer egui version"
$designerRfd = Require-Match $cargo '^\s*rfd\s*=\s*"([^"]+)"' "designer rfd version"
$exportEframe = Require-Match $export 'const EXPORTED_EFRAME_VERSION: &str = "([^"]+)";' "export eframe version"
$exportEgui = Require-Match $export 'const EXPORTED_EGUI_VERSION: &str = "([^"]+)";' "export egui version"
$exportRfd = Require-Match $export 'const EXPORTED_RFD_VERSION: &str = "([^"]+)";' "export rfd version"
$exportMsrv = Require-Match $export 'const EXPORTED_RUST_VERSION: &str = "([^"]+)";' "export rust-version"

$errors = [System.Collections.Generic.List[string]]::new()
if ($edition -ne "2024") {
    $errors.Add("Cargo edition is $edition; expected 2024.")
}
if ($toolchainVersion -ne $ciVersion) {
    $errors.Add("rust-toolchain.toml pins $toolchainVersion but CI pins $ciVersion.")
}
if ($msrv -ne $exportMsrv) {
    $errors.Add("Designer MSRV $msrv differs from generated-project MSRV $exportMsrv.")
}
if ($designerEframe -ne $exportEframe) {
    $errors.Add("Designer eframe $designerEframe differs from generated eframe $exportEframe.")
}
if ($designerEgui -ne $exportEgui) {
    $errors.Add("Designer egui $designerEgui differs from generated egui $exportEgui.")
}
if ($designerRfd -ne $exportRfd) {
    $errors.Add("Designer rfd $designerRfd differs from generated rfd $exportRfd.")
}
if ($export -notmatch 'edition = "2021"') {
    $errors.Add("Generated projects must explicitly retain edition 2021.")
}
if ($export -notmatch 'rust-version = "\{EXPORTED_RUST_VERSION\}"') {
    $errors.Add("Generated Cargo.toml output is not sourced from EXPORTED_RUST_VERSION.")
}

if ($errors.Count -gt 0) {
    $errors | ForEach-Object { Write-Error $_ }
    exit 1
}

Write-Host "Toolchain alignment check passed."
Write-Host "  Designer: edition $edition, MSRV $msrv, tested Rust $toolchainVersion"
Write-Host "  Generated: edition 2021, MSRV $exportMsrv, egui/eframe $exportEgui"
