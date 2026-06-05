param(
    [switch]$Release,
    [switch]$CheckOnly
)

# Build and run RohKai from this repository checkout.
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

$branch = git rev-parse --abbrev-ref HEAD 2>$null
$commit = git rev-parse --short HEAD 2>$null
$dirty = git status --porcelain 2>$null

Write-Host "RohKai source: $ProjectRoot"
Write-Host "Branch: $branch"
Write-Host "Commit: $commit"
if ($dirty) {
    Write-Warning "Working tree has local changes; this launch includes uncommitted code."
} else {
    Write-Host "Working tree: clean"
}

if ($CheckOnly) {
    exit 0
}

if ($Release) {
    cargo run --release
} else {
    cargo run
}
