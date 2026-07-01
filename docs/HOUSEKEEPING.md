# RohKai Housekeeping, Snapshots, And Versioning

This is the lightweight ritual for keeping RohKai's source of truth aligned
across local work, Claude worktrees, Codex worktrees, roadmap docs, and release
notes.

## Daily / Session Snapshot

Use this when asking "where are we?" or before handing work to another agent:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\project-housekeeping.ps1 -Snapshot
```

Default behavior is non-destructive:

- prints branch, HEAD, dirty status, unpushed commits, worktrees, known doc drift;
- runs encoding, toolchain-alignment, and dependency-policy checks;
- previews unreleased changelog text with `git-cliff --unreleased`;
- writes a local ignored snapshot under `.housekeeping/snapshots/`.

The `.housekeeping/` folder is intentionally ignored so normal snapshots do not
dirty the repo.

## Full Gate Snapshot

Use before PR, merge, release candidate, or a major handoff:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\project-housekeeping.ps1 -Full -Snapshot
```

`-Full` adds:

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

## Committed Audit Snapshot

Use sparingly when a snapshot should be reviewed or preserved in git:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\project-housekeeping.ps1 -Full -DocsSnapshot
```

This writes `docs/status-snapshots/<timestamp>-housekeeping.md`, which is a real
repo artifact and should be committed only when useful for a release or audit.

## Changelog

`git-cliff` is installed globally and must stay a standalone CLI tool, never a
project dependency.

Preview unreleased notes:

```powershell
git-cliff --unreleased
```

Generate/update the changelog:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\project-housekeeping.ps1 -UpdateChangelog
```

This runs `git-cliff -o CHANGELOG.md`, or `git-cliff --config cliff.toml` if the
repo later adds its own config.

## Version Tags

Tag creation is explicit and requires a clean worktree:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\project-housekeeping.ps1 -VersionTag v0.2.1
```

The script creates the annotated local tag only. Pushing tags remains a separate
human-reviewed action:

```powershell
git push origin v0.2.1
```

## Failure Modes To Watch

- **Dirty source of truth:** run with `-FailOnDirty` in CI-style checks.
- **Roadmap/CoOp drift:** run with `-FailOnDrift`; extend the script when new
  recurring drift patterns appear.
- **Old worktrees:** inspect the Worktrees section and prune only after checking
  whether Claude/Codex still owns the work.
- **Changelog omissions:** use Conventional Commit subjects; non-conforming
  commits are dropped by `git-cliff`.
