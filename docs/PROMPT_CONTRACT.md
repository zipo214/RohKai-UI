# Prompt Contract

Reusable skeleton for Codex-to-Claude (and Claude-to-Codex) implementation goals.
Use this when a feature has multiple code paths, output forms, or a history of
agents fixing the obvious surface while missing hidden consequences.

## Why This Exists

Recent async/event parity work showed a repeated failure mode: prompts said
"full parity" or "any other widgets", but the implementation stopped at the
nearest local surface, such as primary events or top-level export only. This
contract makes the agent prove the feature boundary before coding.

The rule is simple:

> A feature is not done because one path works. It is done when every user-visible
> control has a real output form in every required path, or that path is explicitly
> removed from the UI before implementation begins.

## Prompt Skeleton

```text
/goal /caveman ultra
Read AGENTS.md / CLAUDE.md. Work only in D:\dev\rohkai.

Run:
pwsh -NoProfile -ExecutionPolicy Bypass -File D:\dev\rohkai\scripts\preflight-context.ps1

Read before editing:
- <source-of-truth file>
- <UI surface file>
- <runtime/canvas file>
- <export/codegen file>
- <relevant docs>

Goal:
<Feature X> must work globally, not only in the common/top-level path.

Before coding, derive and report this checklist from code:
1. Source of truth:
2. UI surfaces that expose this feature:
3. Runtime/canvas paths:
4. Export/codegen paths:
5. Nested/child/template/custom paths:
6. Tests needed for each path:

If any discovered path will not be fixed now, stop and report before editing.
Do not proceed by documenting a required path as a "remaining gap".

Implementation is incomplete if:
- only the top-level/common path works,
- tests only check the listed examples,
- docs call an unfixed required path a remaining gap,
- any user-visible control writes state that runtime/export ignores,
- a UI requirement is satisfied by duplicating content elsewhere instead of
  changing the actual surface named in the task,
- a narrowing abstraction is introduced unless the goal explicitly asks for it.

Required implementation:
1. Derive the complete feature set from the source of truth, not this prompt.
2. Enumerate every consumer/output path and update every required path.
3. Add invariant tests that iterate the derived set across required paths.
4. Add focused regression tests for the known misses.
5. Route behavior through the central helper/API when one exists.
6. Update docs honestly.

Verification:
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\check-text-encoding.ps1

Final report:
- derived feature matrix
- paths changed
- tests added
- verification results
- remaining gaps, if and only if they were explicitly excluded before editing

Success:
No user-visible control for <Feature X> is ignored by any required runtime or
export path.
```

## Example: Event Export Parity

```text
Goal:
Every event returned by WidgetKind::supported_events() must either export real
handler code through rust_wiring::handler_call(), or not appear in Properties.

Before coding:
1. Derive the complete (WidgetKind, WidgetEvent) matrix from supported_events().
2. Find every place a widget can be exported: top-level export, nested/frame child
   export, template/custom export, and live preview if in scope.
3. If nested export will not be fixed, stop before editing. Do not call top-level
   export "full parity".

Required tests:
1. Invariant test over every (WidgetKind, WidgetEvent) pair and every required
   export path.
2. Focused tests for known misses.
3. Conflict test for shared handler names across different event fields.

Failure:
Primary-only parity is failure. Top-level-only parity is failure unless nested
paths were explicitly excluded before coding.
```

## Example: Inline Code Highlight

```text
Goal:
Selected widget code must be highlighted inline inside the editable code panel.

Bad fallback:
Copying the selected code block into a preview card above the editor. That is a
duplicate preview, not an inline highlight.

Required:
- Highlight the actual editable TextEdit content.
- Keep normal readable monospace text; use a subtle background, not low-contrast
  recolored code.
- Preserve editability and Lazare parse/apply behavior.
- Add tests for highlight range/block detection.
```

## Compile Proof vs String Proof

When a feature emits generated code (codegen, export, templates), string-substring
tests are necessary but not sufficient — they prove markers exist, not that the
output compiles. If the goal asks for "compile proof", deliver a real toolchain
check, not more substring asserts:

- Generate the project to a unique temp dir with std only
  (`std::env::temp_dir()` + pid/nanos; no `tempfile` crate).
- Run the real tool (`std::process::Command::new("cargo").args(["check"])`) against
  the generated dir; fail the test on non-zero exit.
- Share `CARGO_TARGET_DIR` so cached deps make reruns fast.
- If the check is too slow for the default suite, mark it `#[ignore]` and add a
  fast always-run smoke that generates the fixture and asserts the feature matrix.
- Worked example: `codegen::export::tests::export_compile_fixture_cargo_check`
  (real `cargo check` on a generated crate covering event+async, FilePicker/rfd,
  channel fields, iterator methods, simple local trait binding, and state
  bindings) + the matrix smoke beside it.

"String-level proof remains" is an acceptable honest gap ONLY if a real compile
check is genuinely impractical — say so explicitly and say why.

## Language That Works Better

- "Derive the complete set from `<file/function>`."
- "Enumerate all consumer/output paths before editing."
- "If any path is excluded, stop and report before coding."
- "Add an invariant test that fails when the source-of-truth set and output paths drift."
- "Do not introduce a narrowing abstraction unless this goal explicitly asks for it."

## Language To Avoid

- "and any other..."
- "where practical"
- "strongest proof available"
- "fix parity" without naming all output paths
- "from Codex review" without pasting the review summary
- "remaining risk" for a path the prompt defined as required
