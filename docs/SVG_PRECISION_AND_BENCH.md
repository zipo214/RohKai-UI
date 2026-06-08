# SVG Renderer — Precision Policy & Benchmark Methodology

Zero-dependency rasterizer reference doc (R8.1). Two parts: the numeric
precision policy the renderer commits to, and the budget methodology behind the
ignored benchmark. No external renderer, no new crates — see `svg-zero-dep`.

Authoritative source paths:
- `src/canvas/svg_rasterizer.rs` — rasterizer, decoders, caps, benchmark/oracle.
- `src/svg_core.rs` — shared color/length/transform microsyntax.
- `src/canvas/svg_golden.rs` — ASCII golden corpus + signature buckets.

---

## 1. Rendering-precision policy

### Coverage / anti-aliasing
- Fills and strokes are scan-converted into a **per-pixel coverage grid** in
  `[0.0, 1.0]`, then written as 8-bit alpha. Edges bucket to the golden `o`
  (partial) char; interiors to a solid color/`W`/`K` char.
- Coverage is **deterministic**: same input + same target size ⇒ identical
  pixels (asserted by `signature_is_deterministic`, `*_deterministically`).
- Geometry is evaluated in `f64`; results are quantized to `u8` only at the
  final premultiplied store.

### Sampling
- Embedded raster images (`<image>` PNG/JPEG) use **nearest-neighbor** sampling
  when mapped through the element transform. No bilinear/bicubic — chosen for
  determinism and zero-cost; documented as a known fidelity limit, not a bug.

### Color/alpha boundary (the important one)
- The offscreen compositing buffer is **premultiplied alpha** (R4). Group
  opacity, masks, and `over` compositing all operate premultiplied so there is
  no double-darkening (`translucent_group_no_double_darken`).
- Working color space for compositing is **sRGB** (8-bit, non-linear).
- **Filters currently run in sRGB**, not linearRGB. Per SVG 1.1,
  `color-interpolation-filters` defaults to **linearRGB**, so blur/color-matrix
  output is *approximate* vs a spec-exact engine. This is the **R10 conformance
  gap** — converting the filter graph to linearRGB at the boundary and honoring
  `color-interpolation-filters: sRGB` is R10's job. Flagged here so the gap is
  honest and tracked, not silently wrong.
- Masks: luminance masks use Rec.709 coefficients
  (`0.2125 R + 0.7154 G + 0.0721 B`); `mask-type="alpha"` keys on the alpha
  channel. Both covered by goldens (`luminance_mask_left_half`,
  `w3c_alpha_mask_left_half`).

### Numeric caps (precision ↔ safety)
| Cap | Const | Purpose |
|---|---|---|
| Raster pixels | `MAX_RASTER_PIXELS` (16,777,216) | clamp canvas; oversize requests are scaled down, never allocated raw |
| Raster dim | `MAX_RASTER_DIM` | per-axis clamp |
| SVG bytes | `MAX_SVG_BYTES` (5 MB) | reject oversized documents pre-parse |
| Path tokens | `MAX_PATH_TOKENS` (20,000) | path-data flood ⇒ empty default |
| Image pixels | `MAX_IMAGE_PIXELS` | decoded `<image>` dimension bomb guard |
| Image decode bytes | `MAX_IMAGE_DECODE_BYTES` (96 MB) | inflate/IDAT accumulation guard |
| Blur radius | `MAX_BLUR_RADIUS` | blur bomb guard |
| Offscreen depth/bytes | `MAX_OFFSCREEN_*` | nested isolated-group guard |

Each cap has a regression test (`oversized_*`, `path_token_flood_*`,
`inflate_respects_output_ceiling`, `huge_blur_is_bounded_not_a_bomb`,
`oversized_png_dimensions_are_bounded`).

---

## 2. Benchmark methodology

The benchmark is **measure-not-gate**: it prints timings and only fails on a
hang (a 30 s guard), because debug builds vary too much to assert a hard
wall-clock budget. Run it explicitly:

```
cargo test --bin rohkai -- --ignored raster_benchmark_complex_scene_within_budget
```

Scene: `benchmark_svg(200)` — 200 gradient-filled, stroked, clipped rects on a
256×256 canvas, exercising paint-server lookup, stroking, clip masks, and the
premultiplied offscreen path together.

### Budgets (targets, release build, reference dev machine)
These are **design targets** for the four pipeline stages, not asserted gates:

| Stage | What it covers | Target (256², 200 rects, release) |
|---|---|---|
| Parse | XML scan → token stream | < 1 ms |
| Scene build | tokens → `SvgScene` → `DisplayList` | < 2 ms |
| Raster | display-list execute → pixels | < 15 ms |
| Peak alloc | offscreen + paint tables | bounded by the caps above (no unbounded growth) |

Methodology notes:
- Timings come from `std::time::Instant` around `rasterize_with_report`.
- To attribute time per stage, temporarily split the call; the harness measures
  the whole pipeline by default to track end-to-end regressions.
- Peak allocation is bounded structurally by the cap table, not sampled — the
  cap regression tests are the allocation guard.
- Determinism is a precondition for benchmarking: the ignored
  `reference_oracle_scene_is_deterministic` proves the scene is stable so an
  external-oracle pixel diff (a **dev-only CI artifact**, never a runtime/Cargo
  dependency) is reproducible.

### Fuzz reproducibility & depth tiers
`fuzz_decoders_no_panic_bounded` (ignored) mutates the checked-in
`tests/fixtures/svg_fuzz/` corpus with a **fixed-seed** xorshift PRNG, so any run
is byte-for-byte reproducible regardless of iteration count.
`fuzz_smoke_decoders_never_panic` runs 64 iterations on every `cargo test`.

The per-iteration cost is dominated by `rasterize_or_fallback` on the dense seed
SVG (≈ms/iter in **debug**, ~10–20× faster in **release**), so the sweep depth is
a single env-configurable knob — one harness, three tiers, no recompile:

| Tier | Iterations | How to run |
|---|---|---|
| Smoke (always-run) | 64 | `cargo test` |
| Sweep (default ignored) | 1,000 | `cargo test --bin rohkai -- --ignored fuzz_decoders_no_panic_bounded` |
| Deep | 8,000 | `ROHKAI_FUZZ_ITERS=8000 cargo test --release --bin rohkai -- --ignored fuzz_decoders_no_panic_bounded` |
| Deep+ (debug-budget reference) | 50,000 | `ROHKAI_FUZZ_ITERS=50000 cargo test --release --bin rohkai -- --ignored fuzz_decoders_no_panic_bounded` |

Run deep tiers under `--release` — debug at 50k is multi-minute and not a useful
gate. All counts share the fixed seed, so a failure at any tier reproduces at
that exact `ROHKAI_FUZZ_ITERS`. Wiring a release nightly deep-fuzz job + coverage
tracking + a larger corpus is tracked as a separate future lane (see
`docs/svg-goal-plan-prompts/`).
