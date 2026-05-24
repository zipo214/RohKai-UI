# Platform Notes

RohKai itself is a Rust/egui app. The app workflow is cross-platform through
Cargo:

```text
cargo run
cargo test
cargo clippy -- -D warnings
```

## Windows

The current agent automation scripts are PowerShell-first and were written for
the active Windows workspace at `D:\dev\rohkai`.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1
```

## macOS And Linux

Mac and Linux users can build and run RohKai with Cargo. The `.ps1` scripts are
not required for normal app development, but they are currently the richest
agent preflight/validation path.

Options today:

- Use Cargo directly for app development.
- Install PowerShell Core (`pwsh`) and adapt script invocation paths.
- Read the markdown guidance manually: `AGENTS.md`, `CLAUDE.md`,
  `docs/ROADMAP.md`, `docs/DEVLOG.md`, `docs/CODE_INDEX.md`,
  `docs/CODE_COOP.md`, and relevant skills.

Future cleanup should add one cross-platform command path, preferably a small
Rust `xtask` or `cargo run --bin rohkai-dev -- ...` helper, so preflight and SVG
validation do not depend on Windows PowerShell.
