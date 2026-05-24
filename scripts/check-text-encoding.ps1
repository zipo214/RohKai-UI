# check-text-encoding.ps1
# Scans tracked text files for common UTF-8 double-encoding (mojibake).
# Exit 0 = clean.  Exit 1 = found (prints file:line).
# Usage: pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

# Patterns in [char] codes so this source file stays pure ASCII.
$c3=[char]0xC3;$a9=[char]0xA9;$e2=[char]0xE2;$euro=[char]0x20AC
$rdq=[char]0x201D;$rsq=[char]0x2019;$ldq=[char]0x201C;$ell=[char]0x2026
$badPatterns = @(
    "Trac${c3}",
    "${c3}${a9}",
    "${e2}${euro}${rdq}",
    "${e2}${euro}${rsq}",
    "${e2}${euro}${ldq}",
    "${e2}${euro}${ell}"
)
$regex = ($badPatterns | ForEach-Object { [regex]::Escape($_) }) -join '|'
$ext = '*.rs','*.md','*.toml','*.ps1','*.json','*.txt','*.rkwd','*.rktp'
$findings = @()

Get-ChildItem -Path $repoRoot -Recurse -File -Include $ext |
    Where-Object { $_.FullName -notmatch 'target' } |
    ForEach-Object {
        $p = $_.FullName
        $lns = Get-Content $p -Encoding UTF8 -ErrorAction SilentlyContinue
        if ($null -eq $lns) { return }
        $n = 0
        foreach ($l in $lns) { $n++; if ($l -match $regex) { $findings += "${p}:${n}: $($l.Trim())" } }
    }

if ($findings.Count -gt 0) {
    Write-Host "MOJIBAKE FOUND ($($findings.Count) occurrence(s)):" -ForegroundColor Red
    $findings | ForEach-Object { Write-Host "  $_" -ForegroundColor Yellow }
    exit 1
} else {
    Write-Host "check-text-encoding: OK (no mojibake found)" -ForegroundColor Green
    exit 0
}