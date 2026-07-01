param(
    [switch]$Full,
    [switch]$Snapshot,
    [switch]$DocsSnapshot,
    [switch]$UpdateChangelog,
    [switch]$FailOnDirty,
    [switch]$FailOnDrift,
    [string]$VersionTag
)

$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = $Utf8NoBom
[Console]::InputEncoding = $Utf8NoBom
[Console]::OutputEncoding = $Utf8NoBom
$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectRoot

function Invoke-Collect {
    param(
        [string]$Label,
        [scriptblock]$Command,
        [switch]$AllowFailure
    )

    $out = @()
    $code = 0
    try {
        $out = & $Command 2>&1
        $code = if ($global:LASTEXITCODE -is [int]) { $global:LASTEXITCODE } else { 0 }
    } catch {
        $out = @($_.Exception.Message)
        $code = 1
    }

    if ($code -ne 0 -and -not $AllowFailure) {
        throw "$Label failed:`n$($out -join "`n")"
    }

    [PSCustomObject]@{
        Label = $Label
        ExitCode = $code
        Output = @($out | ForEach-Object { "$_" })
    }
}

function Append-Block {
    param(
        [System.Text.StringBuilder]$Builder,
        [string]$Title,
        [string[]]$Lines
    )

    [void]$Builder.AppendLine("")
    [void]$Builder.AppendLine("## $Title")
    if ($Lines.Count -eq 0) {
        [void]$Builder.AppendLine("_No output._")
        return
    }
    [void]$Builder.AppendLine('```text')
    foreach ($line in $Lines) {
        [void]$Builder.AppendLine($line)
    }
    [void]$Builder.AppendLine('```')
}

function Get-RawFile {
    param([string]$Path)
    if (-not (Test-Path $Path)) {
        return ""
    }
    Get-Content -Raw -Encoding utf8 -Path $Path
}

$timestamp = (Get-Date).ToString("o")
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$branch = (git branch --show-current).Trim()
$statusShort = @(git status --short --branch)
$dirty = @(git status --porcelain)
$head = (git log --oneline -1).Trim()
$originAhead = @(git log --oneline origin/$branch..HEAD 2>$null)
$latestTag = (git describe --tags --abbrev=0 2>$null)
if (-not $latestTag) {
    $latestTag = "(none)"
}

$cargoToml = Get-RawFile "Cargo.toml"
$versionMatch = [regex]::Match($cargoToml, '(?m)^\s*version\s*=\s*"([^"]+)"')
$cargoVersion = if ($versionMatch.Success) { $versionMatch.Groups[1].Value } else { "(unknown)" }

$phase2 = Get-RawFile "docs\ROADMAP_PHASE2.md"
$coop = Get-RawFile "docs\CODE_COOP.md"
$drift = [System.Collections.Generic.List[string]]::new()
if ($coop -match 'S2 Item 1: Canvas widget search' -and $phase2 -match '\[ \] TODO\s+— Search in canvas') {
    $drift.Add("S2 canvas search appears shipped in CODE_COOP but TODO in ROADMAP_PHASE2.")
}
if ($coop -match 'S2 Item 2: Canvas clipboard' -and $phase2 -match '\[ \] TODO\s+— Clipboard enhancements') {
    $drift.Add("S2 clipboard appears shipped in CODE_COOP but TODO in ROADMAP_PHASE2.")
}
if ($phase2 -match '^\s*-\s*\[ \]\s*TODO\s+—\s*' -and $phase2 -notmatch '## S2') {
    $drift.Add("ROADMAP_PHASE2 has TODO entries but S2 marker was not found; inspect backlog format.")
}

$checks = [System.Collections.Generic.List[object]]::new()
$checks.Add((Invoke-Collect "text encoding" {
    pwsh -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\check-text-encoding.ps1"
}))
$checks.Add((Invoke-Collect "toolchain alignment" {
    pwsh -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\check-toolchain-alignment.ps1"
}))
$checks.Add((Invoke-Collect "dependency policy" {
    pwsh -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\check-dependency-policy.ps1"
}))
if ($Full) {
    $checks.Add((Invoke-Collect "cargo fmt --check" { cargo fmt --check }))
    $checks.Add((Invoke-Collect "cargo check" { cargo check }))
    $checks.Add((Invoke-Collect "cargo test" { cargo test }))
    $checks.Add((Invoke-Collect "cargo clippy --all-targets -- -D warnings" {
        cargo clippy --all-targets -- -D warnings
    }))
}

$cliffPreview = Invoke-Collect "git-cliff --unreleased" { git-cliff --unreleased } -AllowFailure
if ($UpdateChangelog) {
    if (Test-Path "cliff.toml") {
        & git-cliff --config cliff.toml -o CHANGELOG.md
    } else {
        & git-cliff -o CHANGELOG.md
    }
    if ($LASTEXITCODE -ne 0) {
        throw "git-cliff changelog generation failed."
    }
}

if ($VersionTag) {
    if ($dirty.Count -gt 0) {
        throw "refusing to tag $VersionTag because the worktree is dirty"
    }
    $existing = git tag --list $VersionTag
    if ($existing) {
        throw "tag $VersionTag already exists"
    }
    git tag -a $VersionTag -m "release: $VersionTag"
    if ($LASTEXITCODE -ne 0) {
        throw "failed to create tag $VersionTag"
    }
}

if ($FailOnDirty -and $dirty.Count -gt 0) {
    throw "worktree is dirty"
}
if ($FailOnDrift -and $drift.Count -gt 0) {
    throw "documentation drift detected: $($drift -join '; ')"
}

$builder = [System.Text.StringBuilder]::new()
[void]$builder.AppendLine("# RohKai Housekeeping Snapshot")
[void]$builder.AppendLine("")
[void]$builder.AppendLine("- Timestamp: $timestamp")
[void]$builder.AppendLine("- Branch: $branch")
[void]$builder.AppendLine("- HEAD: $head")
[void]$builder.AppendLine("- Cargo version: $cargoVersion")
[void]$builder.AppendLine("- Latest tag: $latestTag")
[void]$builder.AppendLine("- Ahead of origin/${branch}: $($originAhead.Count) commit(s)")
[void]$builder.AppendLine("- Dirty files: $($dirty.Count)")
[void]$builder.AppendLine("- Drift findings: $($drift.Count)")

Append-Block $builder "Git Status" $statusShort
Append-Block $builder "Unpushed Commits" $originAhead
$worktrees = @(git worktree list)
Append-Block $builder "Worktrees" $worktrees

if ($drift.Count -gt 0) {
    Append-Block $builder "Documentation Drift" @($drift)
} else {
    Append-Block $builder "Documentation Drift" @("No known drift patterns detected.")
}

foreach ($check in $checks) {
    $lines = @("exit=$($check.ExitCode)") + $check.Output
    Append-Block $builder $check.Label $lines
}

$previewLines = @("exit=$($cliffPreview.ExitCode)") + @($cliffPreview.Output | Select-Object -First 120)
Append-Block $builder "Unreleased Changelog Preview" $previewLines

$report = $builder.ToString()
Write-Host $report

if ($Snapshot) {
    $dir = Join-Path $ProjectRoot ".housekeeping\snapshots"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $path = Join-Path $dir "$stamp-housekeeping.md"
    [System.IO.File]::WriteAllText($path, $report, $Utf8NoBom)
    Write-Host "Local snapshot written: $path"
}

if ($DocsSnapshot) {
    $dir = Join-Path $ProjectRoot "docs\status-snapshots"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $path = Join-Path $dir "$stamp-housekeeping.md"
    [System.IO.File]::WriteAllText($path, $report, $Utf8NoBom)
    Write-Host "Docs snapshot written: $path"
}
