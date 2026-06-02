# Feature Depth Model

This model gives agents and humans a shared way to talk about feature depth.
It is intended to prevent hollow checkboxes: a feature is not "done" merely
because a menu item or enum variant exists.

## Depth Scale

| Level | Name | Meaning | Evidence Required |
|---:|---|---|---|
| 0 | Planned | Idea exists in roadmap/docs only. | Roadmap item or design note. |
| 1 | Surface | UI or schema exists, but output is missing, hollow, or comment-only. | Visible control or type, plus explicit docs saying incomplete. |
| 2 | Functional MVP | Works for a narrow happy path and produces real output. | Code path, tests, generated output or user-visible behavior. |
| 3 | Usable Product Feature | Handles common workflows, errors, persistence, docs, and tests. | Manual workflow can be completed repeatedly without code edits. |
| 4 | Competitive | Comparable to mature tools for the supported subset; has good UX, diagnostics, and integration. | Fixture tests, edge cases, round-trip behavior, clear unsupported cases. |
| 5 | Top-Class | Best-in-class or distinctive: deep behavior, excellent UX, performance, extensibility, and safety. | Quantitative targets, qualitative review, regression harness, stress tests. |

## Depth Axes

Each feature should be evaluated on these axes. The total score is less useful
than the shape of the score.

| Axis | Measures |
|---|---|
| Functional behavior | Does it actually perform the user's task? |
| Output form | Does it affect canvas, properties, codegen, export, save/load, or runtime as promised? |
| State integration | Is `UiTree` or project schema the source of truth? |
| UX clarity | Can a beginner discover and use it? Can a power user inspect it? |
| Error handling | Are invalid states prevented, repaired, or diagnosed? |
| Persistence | Does it save/load correctly without dirtying unrelated data? |
| Round-trip | Can canvas, properties, code panel, and export remain aligned? |
| Test depth | Are unit, fixture, generated-code, and smoke tests present where needed? |
| Performance | Does it stay responsive at realistic project size? |
| Security/dependency policy | Does it honor project constraints and avoid risky input handling? |

## Utility Classes

| Utility | Description |
|---|---|
| Authoring utility | Helps users create or arrange UI. |
| Inspection utility | Helps users understand structure, state, code, or errors. |
| Runtime utility | Affects behavior of exported apps. |
| Extensibility utility | Lets users add new capabilities without editing RohKai source. |
| Safety utility | Prevents data loss, invalid code, bad imports, or dependency drift. |

## Competitor Baseline Categories

RohKai should not copy one tool exactly. It should combine strengths:

- Qt Designer: widget catalog depth, object inspector, layouts, container editing.
- Lazarus/Delphi: visual event wiring, object inspector immediacy, component tray.
- Figma-like tools: precise canvas, guides, hierarchy, reusable components.
- Visual Studio/Blend-like tools: design/code relationship, data binding, project assets.
- Retool/Appsmith-like builders: data integrations, state, actions, live preview.
- IDEs: navigation, refactoring, diagnostics, generated code trust.

## Definition Of "No Hollow Feature"

A feature is hollow if any of these are true:

- It has a button/menu/enum but no real output.
- It changes canvas only, but not codegen/export when users expect runtime behavior.
- It generates comments while docs claim runtime behavior.
- It serializes data that no panel can edit or no code path consumes.
- It passes only a snapshot/string test that does not prove the promised behavior.
- It silently discards unsupported input without diagnostics.

## Anti-Misread Gap Closure Format

When a feature evaluation says RohKai "has" a feature, agents must distinguish
between the **current implementation contract** and the **desired closure
contract**. Existing surface area does not satisfy a gap unless the closure
criteria are met.

Use this section's vocabulary whenever a feature already exists in MVP form but
the roadmap or evaluation is asking for competitor-depth behavior.

### Required Terminology

| Term | Meaning |
|---|---|
| Current Implementation Contract | The narrow behavior the repo actually implements today. This must be written in implementation-specific terms: enum/type, panel, code path, state field, generated output, tests. |
| Desired Closure Contract | The behavior required before the roadmap/evaluation gap is considered closed. This must name missing data models, UI, codegen/export, runtime behavior, diagnostics, and tests. |
| Insufficient Existing Surface | A feature shell or MVP that must not be treated as satisfying the desired closure contract. |
| Acceptance Delta | The exact technical difference between current behavior and desired behavior. Agents should implement the delta, not re-affirm the current surface. |
| Closure Criteria | Tests, generated output, UX behavior, persistence, and docs that prove the desired closure contract is met. |

### Required Block

Every partially implemented feature should add a block like this before agents
start implementation:

```md
### Implementation Status

Status: Functional MVP
Roadmap Closure: Not closed for competitor-depth work.
Current Implementation Contract:
- `WidgetKind::Example` exists.
- Properties expose only `label` and `binding`.
- Export emits a simple one-line egui widget.
- Tests prove only the narrow generated string.

Insufficient Existing Surface:
- The presence of `WidgetKind::Example` does not satisfy `Example System v1`.
- The current code path is useful but incomplete.

Desired Closure Contract:
- Add a typed data model for ...
- Add Properties UI for ...
- Add canvas/preview behavior for ...
- Add export/runtime behavior for ...
- Add diagnostics for ...

Acceptance Delta:
- Replace comment/static/MVP behavior with ...
- Preserve existing MVP behavior while extending it to ...
- Add tests proving ...

Closure Criteria:
- `cargo test example_feature` proves ...
- Exported project compiles with ...
- Manual smoke: user can ...
```

### Agent Rule

An agent must not close, mark complete, or skip a gap merely because:

- an enum variant exists,
- a palette item exists,
- a properties field exists,
- canvas draws a placeholder,
- codegen emits a comment,
- docs say "MVP",
- a string test proves only that a token appears.

The agent may treat the gap as closed only when the **Desired Closure Contract**
and **Closure Criteria** are satisfied. If those are absent, the agent should
write them first, then implement against them.

### Example: Chart

Current Implementation Contract:

- `WidgetKind::Chart` exists.
- Canvas draws a chart-like preview.
- Codegen/export emit a minimal `Vec<f32>` bar painter.
- Tests prove the painter code is emitted.

Insufficient Existing Surface:

- This is not a charting system.
- It does not close a roadmap item asking for competitor-depth charts.

Desired Closure Contract:

- Add a chart data model for one or more named series.
- Add axis, scale, label, legend, color, empty-state, and formatting properties.
- Add preview/canvas behavior that reflects those properties.
- Add export helper code that renders the same chart in generated apps.
- Add fixture tests for multiple series, empty data, and generated compile.

Closure Criteria:

- A chart with two named series renders both series in preview and export.
- Axis labels and legend visibility are editable from Properties.
- Empty data renders an explicit empty state.
- Generated project containing the chart compiles without manual edits.

