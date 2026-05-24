param()

$ErrorActionPreference = "Continue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$ProjectRoot = "D:\dev\rohkai"
$Cwd = (Get-Location).ProviderPath

Write-Host "== RohKai preflight =="
if ($Cwd -ne $ProjectRoot) {
    Write-Warning "CWD is '$Cwd', expected '$ProjectRoot'. Switch before editing."
} else {
    Write-Host "CWD: $Cwd"
}

Write-Host ""
Write-Host "== Git =="
Push-Location $ProjectRoot
try {
    git branch --show-current
    git status --short
} catch {
    Write-Warning "Unable to read git status: $_"
}

Write-Host ""
Write-Host "== Roadmap Current Stage =="
$Roadmap = Join-Path $ProjectRoot "docs\ROADMAP.md"
if (Test-Path $Roadmap) {
    $stage = Get-Content $Roadmap |
        Where-Object { $_ -match "^## Stage [0-9]" } |
        Select-Object -Last 1
    if ($stage) {
        Write-Host $stage
    } else {
        Write-Host "No stage heading found."
    }
} else {
    Write-Warning "Missing docs\ROADMAP.md"
}

Write-Host ""
Write-Host "== Latest Devlog Entry =="
$Devlog = Join-Path $ProjectRoot "docs\DEVLOG.md"
if (Test-Path $Devlog) {
    $lines = Get-Content $Devlog
    $heads = for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^## ") { $i }
    }
    if ($heads.Count -gt 0) {
        $start = $heads[0]
        $end = $lines.Count - 1
        for ($i = $start + 1; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match "^## ") { $end = $i - 1; break }
        }
        $lines[$start..([Math]::Min($end, $start + 24))]
    } else {
        Write-Host "No dated entries yet."
    }
} else {
    Write-Warning "Missing docs\DEVLOG.md"
}

Write-Host ""
Write-Host "== Code CoOp Latest Note =="
$CodeCoop = Join-Path $ProjectRoot "docs\CODE_COOP.md"
if (Test-Path $CodeCoop) {
    $lines = Get-Content $CodeCoop
    $heads = for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^## ") { $i }
    }
    if ($heads.Count -gt 0) {
        $start = $heads[0]
        $end = $lines.Count - 1
        for ($i = $start + 1; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match "^## ") { $end = $i - 1; break }
        }
        $lines[$start..([Math]::Min($end, $start + 12))]
    } else {
        Write-Host "No Code CoOp notes yet."
    }
} else {
    Write-Warning "Missing docs\CODE_COOP.md"
}

Write-Host ""
Write-Host "== Guidance Drift Check =="
$pairs = @(
    @(".agents\skills\project-model\SKILL.md", ".claude\skills\project-model\SKILL.md"),
    @(".agents\skills\canvas-patterns\SKILL.md", ".claude\skills\canvas-patterns\SKILL.md"),
    @(".agents\skills\codegen-rules\SKILL.md", ".claude\skills\codegen-rules\SKILL.md"),
    @(".agents\skills\svg-zero-dep\SKILL.md", ".claude\skills\svg-zero-dep\SKILL.md")
)
foreach ($pair in $pairs) {
    $left = Join-Path $ProjectRoot $pair[0]
    $right = Join-Path $ProjectRoot $pair[1]
    if ((Test-Path $left) -and (Test-Path $right)) {
        $lh = ((Get-Content -Raw $left) -replace "`r`n", "`n")
        $rh = ((Get-Content -Raw $right) -replace "`r`n", "`n")
        if ($lh -eq $rh) {
            Write-Host "OK: $($pair[0]) <-> $($pair[1])"
        } else {
            Write-Host "REVIEW: $($pair[0]) differs from $($pair[1])"
        }
    } else {
        Write-Host "MISSING PAIR: $($pair[0]) <-> $($pair[1])"
    }
}
Write-Host "INFO: AGENTS.md, CLAUDE.md, and role-specific agent files may differ by tool, but both must be read."

Write-Host ""
Write-Host "== SVG Dependency Policy =="
$DependencyPolicy = Join-Path $ProjectRoot "scripts\check-dependency-policy.ps1"
if (Test-Path $DependencyPolicy) {
    try {
        powershell -NoProfile -ExecutionPolicy Bypass -File $DependencyPolicy
    } catch {
        Write-Warning "Dependency policy check failed: $_"
    }
} else {
    Write-Warning "Missing scripts\check-dependency-policy.ps1"
}

Write-Host ""
Write-Host "Reminder: read AGENTS.md, CLAUDE.md, docs/ROADMAP.md, docs/CODE_INDEX.md, latest docs/CODE_COOP.md note, latest docs/DEVLOG.md entry, git status, and relevant skills/agents before planning or edits."
Pop-Location
