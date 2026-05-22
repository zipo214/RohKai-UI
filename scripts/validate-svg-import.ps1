# Validate the zero-dependency SVG import pipeline.
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

Write-Host "Checking formatting..."
cargo fmt --check
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Running SVG parser tests..."
cargo test svg_import
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

Write-Host "Checking clippy..."
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "SVG import validation passed."
