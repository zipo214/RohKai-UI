# Validate RohKai's dependency policy for SVG import work.
$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

$cargoToml = Get-Content -Raw -Path "Cargo.toml"
$blockedDirectDeps = @("resvg", "usvg", "tiny-skia")

foreach ($dep in $blockedDirectDeps) {
    $pattern = "(?m)^\s*$([regex]::Escape($dep))\s*="
    if ($cargoToml -match $pattern) {
        Write-Error "Blocked direct dependency '$dep' found in Cargo.toml. SVG import must stay zero-new-dependency."
        exit 1
    }
}

$sourceMatches = rg --line-number --glob '!docs/DEVLOG.md' --glob '!target/**' '\b(resvg|usvg|tiny[_-]skia)\b' src Cargo.toml 2>$null
if ($LASTEXITCODE -eq 0 -and $sourceMatches) {
    Write-Error "Blocked SVG dependency reference found outside historical devlog:"
    $sourceMatches
    exit 1
}

Write-Host "Dependency policy check passed."
