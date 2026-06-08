# SVG Goal Plan Prompts

One paste-ready `/goal` per remaining SVG renderer phase. Each file follows
`docs/PROMPT_CONTRACT.md`: it forces the agent to derive the feature set and
enumerate every render/export path before coding, and to render visibly in BOTH
the in-app rasterizer and the export-embedded copy (no in-app-only support).

Authoritative scope/acceptance lives in `docs/SVG_RENDERER_ROADMAP.md`. These
prompts are execution wrappers, not a second source of truth — if they drift,
the roadmap wins.

## Status (updated 2026-06-06)

- **R0–R8 complete — the SVG renderer roadmap is closed.** R4 (clip/overflow/
  premultiplied/group opacity), R5 (zero-dependency PNG **and baseline JPEG**
  `data:` decode), R6 (editable chunked multi-label text import), R7
  (alpha/luminance masks + filter tier-1), and R8 (in-app report UI + source
  viewer, golden corpus, benchmark, dev-only oracle) are done and verified.
- Deferred follow-ons (tracked in `SVG_RENDERER_ROADMAP.md`, runtime-diagnosed):
  progressive JPEG (`image.unsupported_jpeg`); R6 vector-outline snapshot /
  raster text; filter tier 2/3 (`filter.unsupported_primitive`); broader
  conformance corpus + fuzzing.
- These goal prompts remain as the authored execution record; each phase shipped.

## Run order

| Phase | File | Theme |
|---|---|---|
| R4 | [R4-clipping-compositing.goal.md](R4-clipping-compositing.goal.md) | clipPath, nested-viewport overflow, premultiplied-alpha buffer, isolated group opacity |
| R5 | [R5-embedded-raster-images.goal.md](R5-embedded-raster-images.goal.md) | `data:` PNG/JPEG decode decision + implementation or explicit non-support |
| R6 | [R6-text-import-rendering.goal.md](R6-text-import-rendering.goal.md) | robust tspan runs/chunks/anchors, grouped multi-label import, optional vector snapshot |
| R7 | [R7-masks-filters.goal.md](R7-masks-filters.goal.md) | masks (alpha/luminance) + filters tier 1 on R4 offscreen pipeline |
| R8 | [R8-conformance-benchmarks-ux.goal.md](R8-conformance-benchmarks-ux.goal.md) | reference harness, golden corpus, benchmarks, report UI + source viewer |

### Post-R8 lanes (current run order)

R0–R8 are shipped. Remaining lanes — execute **in this order**, reading each
prompt before starting (per the SVG roadmap step protocol in CLAUDE.md/AGENTS.md):

| Lane | File | Theme |
|---|---|---|
| R8.1 ✅ | [R8.1-conformance-security-hardening.goal.md](R8.1-conformance-security-hardening.goal.md) | in-repo fuzz harness, W3C-subset corpus, benchmark methodology, precision policy |
| R9 | [R9-markers-vector-effect-patterns.goal.md](R9-markers-vector-effect-patterns.goal.md) | markers (start/mid/end, orient), non-scaling-stroke, pattern tiling |
| R10 | [R10-filter-correctness-tier2.goal.md](R10-filter-correctness-tier2.goal.md) | linearRGB filters, precise filter region, tier-2 primitives, blend modes |
| R11 | [R11-raster-text-textpath.goal.md](R11-raster-text-textpath.goal.md) | opt-in raster text + textPath via a bundled zero-dep vector glyph set |
| R12 | [R12-namespace-recovery-a11y.goal.md](R12-namespace-recovery-a11y.goal.md) | bounded namespace model, malformed-document recovery, title/desc a11y metadata |

**Optional hardening (parallel — does not block the order above):**

| Lane | File | Theme |
|---|---|---|
| R8.2 | [R8.2-deep-fuzz-ci-coverage.goal.md](R8.2-deep-fuzz-ci-coverage.goal.md) | structure-aware mutators, directory seed corpus, release/nightly deep-fuzz + coverage workflow (all zero-dep) |

R8.2 extends R8.1's fuzz harness into a deeper continuous capability. It depends
on R8.1 but is **optional** and can run anytime — the main lane order stays
R9 → R10 → R11 → R12. The R8.1 harness already ships a fixed-seed sweep whose
depth is env-configurable (`ROHKAI_FUZZ_ITERS`); R8.2 is only needed when a
release nightly / coverage-tracked deep-fuzz job is wanted.

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
