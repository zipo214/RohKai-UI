$ProjectRoot = "D:\dev\rohkai"
$StatePath = Join-Path $ProjectRoot ".codex\session-last-touch.txt"
$Now = Get-Date

try {
    $CurrentPath = (Resolve-Path ".").Path
} catch {
    $CurrentPath = (Get-Location).Path
}

if ($CurrentPath -ne $ProjectRoot) {
    Write-Host "[rohkai] WARNING: CWD is $CurrentPath, not $ProjectRoot"
}

$LastSeen = $null
if (Test-Path $StatePath) {
    $Raw = (Get-Content -LiteralPath $StatePath -Raw).Trim()
    if ($Raw.Length -gt 0) {
        try {
            $LastSeen = [DateTimeOffset]::Parse($Raw)
        } catch {
            $LastSeen = $null
        }
    }
}

if ($LastSeen -ne $null) {
    $HoursAway = ($Now - $LastSeen.LocalDateTime).TotalHours
    if ($HoursAway -ge 12) {
        $RoundedHours = [Math]::Round($HoursAway, 1)
        Write-Host "[rohkai] $RoundedHours hours since last Codex preflight. Re-read AGENTS.md, CLAUDE.md, and recent docs before editing."
    }
}

$Now.ToString("o") | Set-Content -LiteralPath $StatePath -NoNewline
