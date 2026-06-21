# /preflight

Run this before planning or editing RohKai code. The script is the procedural
source of truth; AGENTS.md is the policy source.

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
```

Use the default low-token context: AGENTS.md, preflight output, latest
`docs/CODE_COOP.md` note, git status, and relevant skills.

Read heavier docs only when the task needs them:
- `docs/ROADMAP.md` for stage/scope decisions.
- `docs/CODE_INDEX.md` for orientation.
- `docs/ARCHITECTURE.md` for structural changes.
- `docs/DEVLOG.md` for history/regression work, or run preflight with
  `-IncludeDevlog`.

At the start of a meaningful planning or coding session, append a 3-4 sentence
newest-first entry to `docs/CODE_COOP.md`.

Only plan or edit after the preflight context is current.

Before coding, derive:
1. source-of-truth data model,
2. all ownership/topology cases,
3. all UI surfaces,
4. all codegen/export/parser paths,
5. invariant tests for every required path.

Do not mark complete from representative tests. If a required path is unknown or
excluded, stop and report before editing.

Encoding rule: prefer `pwsh`/PowerShell 7 for repo scripts. Do not use Windows
PowerShell 5.1 text-writing commands for repo files. Do not use `Set-Content`,
`Add-Content`, or `Out-File` without explicit `-Encoding utf8`; prefer
`apply_patch` for source edits.
