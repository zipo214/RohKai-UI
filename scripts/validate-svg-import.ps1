# Validate the zero-dependency SVG import pipeline.
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

function Get-PwshCommand {
    $pwsh = Get-Command pwsh -ErrorAction SilentlyContinue
    if ($pwsh) {
        return $pwsh.Source
    }
    $legacy = Get-Command powershell -ErrorAction SilentlyContinue
    if ($legacy) {
        return $legacy.Source
    }
    return $null
}

Write-Host "Checking formatting..."
cargo fmt --check
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Checking SVG dependency policy..."
$runner = Get-PwshCommand
if (-not $runner) {
    Write-Error "Neither pwsh nor powershell is available for script validation."
    exit 1
}
& $runner -NoProfile -ExecutionPolicy Bypass -File scripts\check-dependency-policy.ps1
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Checking text encoding policy..."
& $runner -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Running SVG parser tests..."
cargo test svg_import
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Running SVG rasterizer tests..."
cargo test svg_rasterizer
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Running SVG template preservation tests..."
cargo test imported_svg_preserves_original_source_next_to_template
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Running deterministic SVG output tests..."
cargo test preserves_source_order_metadata_and_deterministic_ids
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Running real-world SVG fixture suite..."
cargo test real_world_fixture_suite_imports_deterministically
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Checking clippy..."
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "SVG import validation passed."
