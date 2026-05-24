# /preflight

Run this before planning or editing RohKai code.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1
```

Then read, in order:

1. `AGENTS.md`
2. `CLAUDE.md`
3. `docs/ROADMAP.md`
4. `docs/CODE_INDEX.md`
5. Latest note in `docs/CODE_COOP.md`
6. Latest entry in `docs/DEVLOG.md`
7. `git status --short --branch`
8. Relevant `.agents/skills/*/SKILL.md` or `.claude/skills/*/SKILL.md`

At the start of a meaningful planning or coding session, append a 3-4 sentence
entry to `docs/CODE_COOP.md` describing what you are about to do, what context
matters, and any hazard the next agent should know.

Do not add `CONTRIBUTING.md` to this preflight/prep checklist. It is not part
of the agent execution context unless the user explicitly asks for contribution
policy work.

Only plan or edit after the preflight context is current.
