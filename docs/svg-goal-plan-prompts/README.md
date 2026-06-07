# SVG Goal Plan Prompts

One paste-ready `/goal` per remaining SVG renderer phase. Each file follows
`docs/PROMPT_CONTRACT.md`: it forces the agent to derive the feature set and
enumerate every render/export path before coding, and to render visibly in BOTH
the in-app rasterizer and the export-embedded copy (no in-app-only support).

Authoritative scope/acceptance lives in `docs/SVG_RENDERER_ROADMAP.md`. These
prompts are execution wrappers, not a second source of truth — if they drift,
the roadmap wins.

## Status (updated 2026-06-06)

- R0–R6 complete for documented subsets. R4 (clip/overflow/premultiplied/group
  opacity), R5 (zero-dependency PNG **and baseline JPEG** `data:` decode + render
  through the R4 pipeline), and R6 (editable chunked multi-label text import with
  anchor/baseline diagnostics) are done and verified.
- Deferred follow-ons (tracked in `SVG_RENDERER_ROADMAP.md`): progressive JPEG
  (`image.unsupported_jpeg`); the R6 vector-outline snapshot / raster text
  rendering; broader conformance corpus.
- R7–R8 remain. Run them in order; each is its own goal with its own gate.
  **Next: R7 masks + filters.**

## Run order

| Phase | File | Theme |
|---|---|---|
| R4 | [R4-clipping-compositing.goal.md](R4-clipping-compositing.goal.md) | clipPath, nested-viewport overflow, premultiplied-alpha buffer, isolated group opacity |
| R5 | [R5-embedded-raster-images.goal.md](R5-embedded-raster-images.goal.md) | `data:` PNG/JPEG decode decision + implementation or explicit non-support |
| R6 | [R6-text-import-rendering.goal.md](R6-text-import-rendering.goal.md) | robust tspan runs/chunks/anchors, grouped multi-label import, optional vector snapshot |
| R7 | [R7-masks-filters.goal.md](R7-masks-filters.goal.md) | masks (alpha/luminance) + filters tier 1 on R4 offscreen pipeline |
| R8 | [R8-conformance-benchmarks-ux.goal.md](R8-conformance-benchmarks-ux.goal.md) | reference harness, golden corpus, benchmarks, report UI + source viewer |

## Why one phase per goal

Finishing R4–R8 in a single goal produces the hollow-surface failure
`PROMPT_CONTRACT.md` exists to prevent (one path works, rest "documented as a
gap"). Each phase here has its own derive-paths step, tests, and zero-warning
verification gate. R7 deliberately depends on the R4 offscreen/premultiplied
pipeline, so do not reorder it ahead of R4.

## Hard invariants for every phase

- No new dependencies. Pure RohKai source (`svg-zero-dep` skill). No `resvg`,
  `usvg`, `tiny-skia`, or substitute renderer chain.
- Both embedded sources (`src/canvas/svg_rasterizer.rs`, `src/svg_core.rs`) stay
  std-only; the single-`crate::` export rewrite contract in
  `src/codegen/export.rs` must keep passing
  (`embedded_svg_sources_keep_single_import_rewrite_contract`).
- No hollow features: parser + scene/IR + visible render (or deliberate
  fallback) + diagnostics + tests + honest docs, or the feature is not done.
- Zero `cargo clippy -- -D warnings`. Goldens stay signature-stable unless a
  change is provably more correct and justified in the devlog.
