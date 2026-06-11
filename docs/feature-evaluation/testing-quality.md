# Testing And Quality Evaluation

## Scope

This covers unit tests, fixture tests, generated-code tests, SVG validation,
encoding/dependency policy scripts, smoke tests, manual QA, performance testing,
and future CI expectations.

## Top-Class Expectation

Top-class quality for a GUI builder requires more than unit tests. RohKai needs
proof that features work through their whole pipeline: schema, canvas, properties,
codegen, export, save/load, and user workflow.

## Current State

| Test/Quality Area | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Unit tests | 3-4 | Broad Rust unit coverage; 412 tests currently pass. | Needs more cross-module integration fixtures. |
| Codegen tests | 3 | String-level generated code assertions. | Needs generated project compile tests. |
| SVG tests | 3-4 | Import/rasterizer/golden/security fixtures. | Needs larger fixture corpus and visual review artifacts. |
| Clippy/fmt | 4 | `cargo fmt --check`, `cargo clippy -- -D warnings` clean. | `--all-targets` still has known historical lints unless fixed separately. |
| Encoding policy | 4 | Script catches mojibake patterns. | Needs CI enforcement and Rust equivalent. |
| Dependency policy | 3-4 | SVG forbidden dependency checks. | Needs central policy report for all dependencies. |
| GUI smoke | 2 | Manual/short launch smoke. | Needs automated UI screenshots or egui harness where practical. |
| Performance | 1-2 | Limited explicit performance measurement. | Needs benchmarks and frame-time instrumentation. |

## Utility

- Safety utility: very high.
- Runtime utility: high for export compile tests.
- Developer utility: high for multi-agent confidence.

## Ideal Quality Bar By Feature Depth

| Feature Depth | Required Tests |
|---|---|
| Level 1 Surface | Docs say incomplete; no false "done" claims. |
| Level 2 Functional MVP | Unit test plus generated output or visible behavior assertion. |
| Level 3 Usable Product | Save/load, properties, codegen/export, and error path tests. |
| Level 4 Competitive | Fixtures, integration tests, manual QA script, performance target. |
| Level 5 Top-Class | Golden tests, fuzz/property tests where appropriate, CI matrix, stress tests. |

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Export compile fixtures | Every representative generated project compiles automatically. |
| UI regression | Key windows/canvas states captured and compared with tolerances. |
| Performance dashboard | Widget count, codegen time, SVG render time, export time measured. |
| Policy gates | Dependency, encoding, SVG security, and docs honesty checks in CI. |
| Feature depth manifest | Machine-readable list of feature depth and required evidence. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Test coverage by pipeline | Every widget exercises schema -> canvas -> properties -> codegen -> export. |
| Generated compile proof | Supported export fixtures compile with zero warnings. |
| Regression clarity | Failed test names tell which feature contract broke. |
| Performance budget | 100, 500, and 1000-widget projects have measured responsiveness targets. |
| Multi-agent safety | Preflight shows dirty files, latest handoff, encoding, dependency status. |

## Recommended Next Work

1. Add export compile integration tests with temp generated projects.
2. Fix known `cargo clippy --all-targets -- -D warnings` historical lints.
3. Add `xtask check` as cross-platform wrapper for fmt/check/test/clippy/scripts.
4. Add feature-depth manifest that maps each roadmap item to tests/docs.
5. Add simple screenshot-based UI regression harness for left rail, canvas, and preferences.

