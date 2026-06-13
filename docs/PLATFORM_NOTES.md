# Platform Notes

RohKai itself is a Rust/egui app. The app workflow is cross-platform through
Cargo:

```text
cargo run
cargo test
cargo clippy --all-targets -- -D warnings
```

## Rust And Dependency Contract

- RohKai source uses Rust edition 2024.
- `rust-version = "1.92"` is the minimum supported Rust version.
- `rust-toolchain.toml` and CI pin Rust 1.96.0 with Clippy and rustfmt.
- Generated projects intentionally use edition 2021 for downstream
  compatibility, declare Rust 1.92, and share RohKai's egui/eframe/rfd
  versions.
- Direct dependency versions are explicit in `Cargo.toml`; `Cargo.lock` is the
  reproducible transitive dependency source of truth.

Check the offline version invariants after changing Cargo, CI, or export:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-toolchain-alignment.ps1
```

Audit crates.io and rustup for newer releases:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\audit-dependency-updates.ps1
```

## Windows

The current agent automation scripts are PowerShell-first, prefer PowerShell 7
(`pwsh`), and were written for the active Windows workspace at `D:\dev\rohkai`.

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\preflight-context.ps1
```

PowerShell 7 is required for the safest Unicode path. Windows PowerShell 5.1 is
fallback-only because its legacy console and file encodings can corrupt
UTF-8-only text when agents write repo files.

Repo PowerShell scripts set UTF-8 input/output explicitly. Agents and humans
should still avoid Windows PowerShell 5.1 text-writing commands for repo files.
If a script must read or write text, use explicit `-Encoding utf8`; for source
edits, prefer patch-based edits. `scripts/check-text-encoding.ps1` guards
against common mojibake markers and replacement characters.

## macOS And Linux

Mac and Linux users can build and run RohKai with Cargo. The `.ps1` scripts are
not required for normal app development, but they are currently the richest
agent preflight/validation path.

Options today:

- Use Cargo directly for app development.
- Install PowerShell 7 (`pwsh`) and run the same script commands used on Windows.
- Read the low-token markdown guidance manually: `AGENTS.md` or `CLAUDE.md`,
  latest `docs/CODE_COOP.md`, and relevant skills. Add `docs/ROADMAP.md`,
  `docs/CODE_INDEX.md`, `docs/ARCHITECTURE.md`, or `docs/DEVLOG.md` only when
  the task needs scope, orientation, structure, or history.

Future cleanup should add one cross-platform command path, preferably a small
Rust `xtask` or `cargo run --bin rohkai-dev -- ...` helper, so preflight and SVG
validation do not depend on Windows PowerShell.
