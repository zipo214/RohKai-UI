param(
    [switch]$IncludeDevlog
)

$ErrorActionPreference = "Continue"
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom

$ProjectRoot = "D:\dev\rohkai"
$Cwd = (Get-Location).ProviderPath

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
    $stage = Get-Content -Path $Roadmap -Encoding utf8 |
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
$Devlog = Join-Path $ProjectRoot "docs\DEVLOG.md"
if ($IncludeDevlog) {
    Write-Host "== Latest Devlog Entry =="
    if (Test-Path $Devlog) {
        $lines = Get-Content -Path $Devlog -Encoding utf8
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
} else {
    Write-Host "== Devlog =="
    if (Test-Path $Devlog) {
        Write-Host "Omitted by default to reduce agent token load. Re-run with -IncludeDevlog when investigating history or resuming a specific prior session."
    } else {
        Write-Warning "Missing docs\DEVLOG.md"
    }
}

Write-Host ""
Write-Host "== Code CoOp Latest Note =="
$CodeCoop = Join-Path $ProjectRoot "docs\CODE_COOP.md"
if (Test-Path $CodeCoop) {
    $lines = Get-Content -Path $CodeCoop -Encoding utf8
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
        $lh = ((Get-Content -Raw -Encoding utf8 -Path $left) -replace "`r`n", "`n")
        $rh = ((Get-Content -Raw -Encoding utf8 -Path $right) -replace "`r`n", "`n")
        if ($lh -eq $rh) {
            Write-Host "OK: $($pair[0]) <-> $($pair[1])"
        } else {
            Write-Host "REVIEW: $($pair[0]) differs from $($pair[1])"
        }
    } else {
        Write-Host "MISSING PAIR: $($pair[0]) <-> $($pair[1])"
    }
}
Write-Host "INFO: Read the entry doc for your tool. Cross-read AGENTS.md/CLAUDE.md only for guidance drift, handoff, or multi-agent coordination work."

Write-Host ""
Write-Host "== Text Encoding Policy =="
$TextEncodingPolicy = Join-Path $ProjectRoot "scripts\check-text-encoding.ps1"
if (Test-Path $TextEncodingPolicy) {
    try {
        $runner = Get-PwshCommand
        if ($runner) {
            & $runner -NoProfile -ExecutionPolicy Bypass -File $TextEncodingPolicy
        } else {
            Write-Warning "Neither pwsh nor powershell is available for text encoding policy check."
        }
    } catch {
        Write-Warning "Text encoding policy check failed: $_"
    }
} else {
    Write-Warning "Missing scripts\check-text-encoding.ps1"
}

Write-Host ""
Write-Host "== SVG Dependency Policy =="
$DependencyPolicy = Join-Path $ProjectRoot "scripts\check-dependency-policy.ps1"
if (Test-Path $DependencyPolicy) {
    try {
        $runner = Get-PwshCommand
        if ($runner) {
            & $runner -NoProfile -ExecutionPolicy Bypass -File $DependencyPolicy
        } else {
            Write-Warning "Neither pwsh nor powershell is available for dependency policy check."
        }
    } catch {
        Write-Warning "Dependency policy check failed: $_"
    }
} else {
    Write-Warning "Missing scripts\check-dependency-policy.ps1"
}

Write-Host ""
Write-Host "Reminder: low-token default is AGENTS/CLAUDE policy + this summary + latest Code CoOp + git status + relevant skills. Read ROADMAP/CODE_INDEX/ARCHITECTURE/DEVLOG only when the task needs planning, structure, or history."
Pop-Location
