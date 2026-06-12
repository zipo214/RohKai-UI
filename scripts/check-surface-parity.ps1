#!/usr/bin/env pwsh
# check-surface-parity.ps1 — caveman-review parity auditor for RohKai.
#
# Catches the class of error this repo keeps producing: a capability implemented
# on ONE surface (schema field + Properties UI, or a roadmap checkbox) that never
# reaches the others (codegen / export / a test), and a "done" claim that the code
# does not back up.
#
# WHY this exists (root cause): RohKai already guards ENUM drift with exhaustive
# `match` arms (a new `WidgetKind` will not compile until codegen handles it) and
# `#[cfg(test)]` parity lists. But STRUCT FIELDS have no forcing function — a new
# `WidgetInstance` field can be serialized, shown in Properties, and silently
# ignored by codegen, and everything still compiles + "works". P2.4 shipped
# `child_flex`/`grid_col_span` exactly that way. This script is the static,
# human-review supplement to the `tests/fidelity_audit.rs` cross-surface harness.
#
# Output is caveman-review style: `<where>: <problem>. <fix>.` One line per find.
# Exit 0 by default (advisory). With -Strict, exit 1 if any DONE-overclaim is
# found (a roadmap [x] naming a symbol that does not exist in src/ or tests/).
#
# Usage:
#   pwsh -NoProfile -File scripts/check-surface-parity.ps1
#   pwsh -NoProfile -File scripts/check-surface-parity.ps1 -Strict

[CmdletBinding()]
param(
    [switch]$Strict
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$src = Join-Path $root 'src'
$schema = Join-Path $src 'project/schema.rs'
$codegenDir = Join-Path $src 'codegen'
$fidelity = Join-Path $root 'tests/fidelity_audit.rs'
$roadmap = Join-Path $root 'docs/ROADMAP_PHASE2.md'

$findings = [System.Collections.Generic.List[string]]::new()
$overclaims = 0

function Add-Find([string]$line) { $findings.Add($line) | Out-Null }

# Pre-load all Rust source text once (src + tests) for symbol existence checks.
$rustFiles = Get-ChildItem -Path $src, (Join-Path $root 'tests') -Recurse -Filter *.rs -ErrorAction SilentlyContinue
$rustText = [System.Text.StringBuilder]::new()
foreach ($f in $rustFiles) { [void]$rustText.Append((Get-Content -LiteralPath $f.FullName -Raw)) ; [void]$rustText.Append("`n") }
$allRust = $rustText.ToString()
$codegenText = (Get-ChildItem -Path $codegenDir -Recurse -Filter *.rs |
    ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw }) -join "`n"

function Test-SymbolExists([string]$sym) {
    # Word-boundary match anywhere in src+tests.
    return [regex]::IsMatch($allRust, "(?<![A-Za-z0-9_])$([regex]::Escape($sym))(?![A-Za-z0-9_])")
}

# ---------------------------------------------------------------------------
# CHECK 1 — WidgetInstance fields with no codegen surface.
# A field present on the schema + Properties but never read by any src/codegen/
# emitter is a candidate parity hole (the P2.4 class).
# ---------------------------------------------------------------------------
# Fields known to be positional / non-codegen (read by canvas or io, not the
# Rust emitter) — annotate so they are not noise.
$positional = @('id', 'import_metadata')
$schemaText = Get-Content -LiteralPath $schema -Raw
# Isolate the `pub struct WidgetInstance { ... }` body.
$m = [regex]::Match($schemaText, 'pub struct WidgetInstance \{(.+?)\n\}', 'Singleline')
if ($m.Success) {
    $body = $m.Groups[1].Value
    $fieldMatches = [regex]::Matches($body, '(?m)^\s*pub\s+([a-z_][a-z0-9_]*)\s*:')
    foreach ($fm in $fieldMatches) {
        $field = $fm.Groups[1].Value
        if ($positional -contains $field) { continue }
        $inCodegen = [regex]::IsMatch($codegenText, "(?<![A-Za-z0-9_])$([regex]::Escape($field))(?![A-Za-z0-9_])")
        if (-not $inCodegen) {
            Add-Find "risk: schema.rs WidgetInstance.$field has no reference in src/codegen/. Confirm it is geometry/canvas-only, or wire codegen + a fidelity_audit parity test (the P2.4 class)."
        }
    }
}
else {
    Add-Find "bug: schema.rs - could not locate the WidgetInstance struct body. Update this script's struct regex."
}

# ---------------------------------------------------------------------------
# CHECK 2 — Roadmap claim vs code drift.
# [x]/DONE naming a missing symbol = overclaim (dangerous). [ ] TODO naming a
# present symbol = stale underclaim (the P2.7 "claimed not-done but done" case).
# ---------------------------------------------------------------------------
if (Test-Path $roadmap) {
    # Tokens that look like backticked Rust identifiers but are not symbols.
    $denylist = @('S1', 'S2', 'S3', 'S4', 'S5', 'S6', 'S7', 'S8', 'S9', 'S10', 'S11',
        'S12', 'S13', 'S14', 'S15', 'S16', 'S17', 'S18', 'S19', 'S20', 'S21', 'S22',
        'params', 'data', 'file', 'https', 'http', 'var', 'crate', 'std', 'true', 'false')
    $lineNo = 0
    foreach ($line in (Get-Content -LiteralPath $roadmap)) {
        $lineNo++
        $state = $null
        if ($line -match '^\s*-\s*\[x\]\s*DONE') { $state = 'done' }
        elseif ($line -match '^\s*-\s*\[\s\]\s*TODO') { $state = 'todo' }
        else { continue }

        foreach ($bt in [regex]::Matches($line, '`([^`]+)`')) {
            $tok = $bt.Groups[1].Value
            # Extract a bare identifier candidate (drop ::, (), generics, etc.).
            $idm = [regex]::Match($tok, '^[A-Za-z_][A-Za-z0-9_]*')
            if (-not $idm.Success) { continue }
            $sym = $idm.Value
            if ($sym.Length -lt 4) { continue }
            if ($denylist -contains $sym) { continue }
            # Must look like a code identifier: snake_case fn/field or CamelCase type.
            if ($sym -notmatch '_' -and $sym -cnotmatch '^[A-Z][a-z]') { continue }

            $exists = Test-SymbolExists $sym
            if ($state -eq 'done' -and -not $exists) {
                Add-Find "bug: ROADMAP_PHASE2.md:$lineNo OVERCLAIM - [x] DONE names ``$sym`` but no such symbol in src/ or tests/. Implement it or downgrade the claim."
                $script:overclaims++
            }
            elseif ($state -eq 'todo' -and $exists) {
                Add-Find "risk: ROADMAP_PHASE2.md:$lineNo STALE? - [ ] TODO names ``$sym`` which already exists in code. Confirm it is an incidental mention, else mark [x]/[~]."
            }
        }
    }
}

# ---------------------------------------------------------------------------
# CHECK 3 — Hollow public API: #[allow(dead_code)] directly on a pub item.
# A pub fn/struct/enum that is dead = shipped but unsurfaced (validate_constraints
# class). Legit when it is a planned/test-only API — hence advisory.
# ---------------------------------------------------------------------------
foreach ($f in (Get-ChildItem -Path $src -Recurse -Filter *.rs)) {
    $lines = Get-Content -LiteralPath $f.FullName
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '#\[allow\(dead_code\)\]') {
            # Look ahead past further attributes/doc-comments to the declaration.
            $j = $i + 1
            while ($j -lt $lines.Count -and ($lines[$j] -match '^\s*(#\[|///|//!|$)')) { $j++ }
            if ($j -lt $lines.Count -and $lines[$j] -match '^\s*pub\s+(fn|struct|enum|const|static|trait)\s+([A-Za-z_][A-Za-z0-9_]*)') {
                $kind = $Matches[1]; $name = $Matches[2]
                $rel = $f.FullName.Substring($root.Length + 1).Replace('\', '/')
                Add-Find "risk: ${rel}:$($j+1) pub $kind ``$name`` is #[allow(dead_code)] - shipped but unsurfaced. Wire it into the UI/codegen or delete it (or confirm it is planned-API scaffolding)."
            }
        }
    }
}

# ---------------------------------------------------------------------------
# Report (caveman-review style).
# ---------------------------------------------------------------------------
"check-surface-parity — RohKai cross-surface drift audit"
"========================================================"
if ($findings.Count -eq 0) {
    "OK — no surface-parity drift detected."
    exit 0
}
foreach ($line in $findings) { $line }
""
"$($findings.Count) finding(s); $overclaims overclaim(s). [bug] broken  [risk] review  [nit] minor."
"Advisory by default. The hard gates are the exhaustive match arms + tests/fidelity_audit.rs."
if ($Strict -and $overclaims -gt 0) {
    "STRICT: failing on $overclaims DONE-overclaim(s)."
    exit 1
}
exit 0
