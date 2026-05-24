# Mojibake Remediation Plan - 2026-05-24

## Purpose

RohKai has seen UTF-8 text become mojibake during agent-assisted edits on
Windows. The likely mechanism is UTF-8 bytes being decoded through a legacy
Windows ANSI/PowerShell 5.1 path and then saved back as valid UTF-8.

## Trace Summary

- `docs/DEVLOG.md` gained corrupted `Tracé` text in commit `1d4a828`
  (`2026-05-22 16:40 -0700`). Git metadata does not identify the agent.
- `src/codegen/export.rs` gained a corrupted em dash in commit `b837061`
  (`2026-05-24 08:32 -0700`). That commit has a Claude co-author trailer.
- Later formatting commits preserved the corrupted bytes; `cargo fmt` did not
  create the corruption.

The stored bad forms were equivalent to:

- `Tracé` encoded as UTF-8, decoded as Windows-1252, then written as UTF-8.
- An em dash encoded as UTF-8, decoded as Windows-1252, then written as UTF-8.

## Remedy

- Prefer PowerShell 7 (`pwsh`) for all repo scripts.
- Bootstrap every `.ps1` script with explicit UTF-8 input/output encodings.
- Make script file reads and writes explicit with `-Encoding utf8`.
- Avoid Windows PowerShell 5.1 text-writing commands for repo files.
- Prefer `apply_patch` for source edits.
- Add `scripts/check-text-encoding.ps1` and wire it into preflight and SVG
  validation.

## 2026-05-24 Investigation Result

A byte-level scan of all `.rs`, `.md`, `.toml`, `.ps1`, `.json`, `.txt`,
`.rkwd`, and `.rktp` files confirmed every file is **valid UTF-8 without BOM**.
No live mojibake bytes found. The corrupted forms described in the Trace Summary
either were never committed, or were corrected by a later `cargo fmt` pass before
this investigation ran.

The `rg --encoding latin1` false-positive output that triggered this plan was
rg re-interpreting correct UTF-8 multibyte sequences (em dashes, arrows,
box-drawing) as their Windows-1252 code point representations — the files
themselves were fine.

Prevention infrastructure added:
- `scripts/check-text-encoding.ps1` — scans for the six most common
  double-encoding patterns; called automatically by preflight.

## Verification

Run:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```
