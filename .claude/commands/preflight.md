# /preflight

Run this before planning or editing RohKai code.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
```

Then read, in order:

1. `AGENTS.md`
2. `CLAUDE.md`
3. `docs/ROADMAP.md`
4. Latest entry in `docs/DEVLOG.md`
5. `git status --short --branch`
6. Relevant `.claude/skills/*/SKILL.md` files

Only plan or edit after the preflight context is current.
