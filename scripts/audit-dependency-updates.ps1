param(
    [switch]$FailOnUpdates
)

# Report current stable Rust and direct-crate updates without installing helper crates.
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom
$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

& pwsh -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\check-toolchain-alignment.ps1"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$cargo = Get-Content -Raw -Encoding utf8 -Path "Cargo.toml"
$dependencyNames = @(
    "ab_glyph",
    "eframe",
    "egui",
    "rayon",
    "rustybuzz",
    "rfd",
    "rusqlite",
    "serde",
    "serde_json",
    "uuid"
)

$updates = [System.Collections.Generic.List[string]]::new()
Write-Host ""
Write-Host "== Direct dependency registry audit =="
foreach ($name in $dependencyNames) {
    $escaped = [regex]::Escape($name)
    $currentMatch = [regex]::Match(
        $cargo,
        "(?m)^\s*$escaped\s*=\s*(?:`"([^`"]+)`"|\{\s*version\s*=\s*`"([^`"]+)`")"
    )
    if (-not $currentMatch.Success) {
        throw "Unable to read direct dependency '$name' from Cargo.toml."
    }
    $current = if ($currentMatch.Groups[1].Success) {
        $currentMatch.Groups[1].Value
    } else {
        $currentMatch.Groups[2].Value
    }

    $search = cargo search $name --limit 1 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "cargo search failed for '$name': $search"
    }
    $latestMatch = [regex]::Match(($search -join "`n"), "(?m)^$escaped\s*=\s*`"([^`"]+)`"")
    if (-not $latestMatch.Success) {
        throw "Unable to parse crates.io result for '$name'."
    }
    $latest = $latestMatch.Groups[1].Value
    $state = if ($current -eq $latest) { "current" } else { "UPDATE" }
    Write-Host ("  {0,-12} {1,-10} latest {2,-10} {3}" -f $name, $current, $latest, $state)
    if ($current -ne $latest) {
        $updates.Add("$name $current -> $latest")
    }
}

Write-Host ""
Write-Host "== Rust toolchain audit =="
$rustup = rustup check 2>&1
if ($LASTEXITCODE -ne 0 -and ($rustup -join "`n") -notmatch "(?i)update available") {
    throw "rustup check failed: $rustup"
}
$rustup | ForEach-Object { Write-Host "  $_" }
if (($rustup -join "`n") -match "(?i)update available") {
    $updates.Add("Rust toolchain update available")
}

Write-Host ""
if ($updates.Count -eq 0) {
    Write-Host "Dependency audit passed: direct crates and pinned Rust are current."
    exit 0
}

Write-Warning ("Updates found:`n  " + ($updates -join "`n  "))
if ($FailOnUpdates) {
    exit 1
}
