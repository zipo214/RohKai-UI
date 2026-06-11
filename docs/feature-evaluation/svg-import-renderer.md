# SVG Import And Renderer Evaluation

## Scope

This covers SVG import as editable RohKai templates, SVG Image preview/rendering,
security policy, diagnostics, source preservation, and future text/rendering work.

## Top-Class Expectation

Top-class SVG support has two different goals:

1. Import mode: convert SVG into editable RohKai template placeholders with clear
   diagnostics and source preservation.
2. Render mode: show SVG Image widgets accurately enough for design and export.

These are not the same product. Import prioritizes editability. Render prioritizes
visual fidelity. Mature SVG engines handle enormous spec surface; RohKai must be
truthful about its supported subset while building its own zero-new-dependency
capabilities where required.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Security gate | 4 | Rejects DOCTYPE/entities and guards unsupported risky input; R8.1 deterministic fuzz harness (XML/path/PNG/JPEG/inflate, seed corpus) + memory-cap regressions assert no-panic/bounded output. | Broader licensed corpus + size/resource telemetry remain. |
| Import parser | 4 | Common shapes/paths/groups/style/transforms/`use`/metadata; bounded xmlns namespace model with foreign-ns skip + malformed-markup recovery + `<title>`/`<desc>` a11y extraction (R12). | Real DOM/full recovery (out of scope); richer CSS tiers as fixtures justify. |
| Diagnostics | 4 | Structured warnings/errors/fidelity across buckets; raster node diagnostics carry stable node IDs + source byte spans; namespace/recovery/a11y surfaced (R12) in the report panel. | Per-element remediation suggestions. |
| Source preservation | 4 | Original SVG preserved beside imported template and in Image widgets. | Needs source diff/viewer integration polish. |
| Shared microsyntax | 4 | Shared colors, numbers, lengths, checked transforms, path tokenization, preserveAspectRatio/viewBox mapping, declarations, and bounded tier-1 CSS selectors/cascade. | Needs broader fuzz/property tests and only those later CSS tiers justified by fixtures. |
| Raster IR | 4 | Stable source-spanned node IDs, bounded expanded local references, flattened scene items, owned lowered display commands (incl. clip geometry + BeginLayer/EndLayer compositing scopes), and reusable solid/linear/radial paint-server IR. | Later phases add image/text/effects IR. |
| Raster renderer | 4 | Own software rasterizer for supported subset; retained paths; viewport mapping; nonzero/evenodd fills; affine caps/joins/miters/dashes; deterministic 8x8 coverage; bounded use/symbol expansion; linear/radial gradient fills/strokes; clipPath clipping + nested-`<svg>` overflow + premultiplied-alpha group compositing (R4); PNG/JPEG images (R5); masks + filter tier-1 (R7); markers, pattern tiling, and `vector-effect: non-scaling-stroke` (R9); linearRGB filter tier-2 + precise filter regions + `mix-blend-mode` (R10); and raster text/textPath via the bundled Hershey simplex vector font (R11). | Still lacks tier-3 filter primitives, exact arcs joins, and real font-file glyphs/shaping (R12 / out of scope). |
| Text/tspan | 4 | Chunked multi-label import (R6: positioned spans → grouped labels with provenance + per-chunk diagnostics) plus Image-mode raster snapshot (R11: bundled Hershey simplex vector font, anchored runs, x/y/dx/dy tspans, arc-length textPath, honest `text.raster_snapshot`/tofu/bidi diagnostics). | Real font-file glyphs, shaping/bidi, per-glyph position lists. |

## Utility

- Authoring utility: high for turning diagrams/wireframes into editable starts.
- Inspection utility: high because diagnostics explain fidelity risk.
- Runtime utility: medium-high for Image widget preview/export.
- Safety utility: very high because SVG is hostile input.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Import report UI | User sees fidelity score, skipped features, approximations, source links, and suggested fixes. |
| Editable conversion | Shapes/text become meaningful RohKai widgets when possible, grouped with provenance. |
| Visual fallback | Original SVG Image remains source-of-truth fallback when editability loses fidelity. |
| Text engine | Robust tspan chunks, style runs, anchors, baselines, bidi/shaping limitations documented and tested. |
| Renderer parity subset | For supported SVG subset, output is golden-tested and close to mature engines. |
| Security | Hard parser limits, no external refs by default, deterministic failure modes, fuzz corpus. |

## Mature Renderer Capability Inventory

| Capability | RohKai Current | Ideal |
|---|---|---|
| XML/security | Hardened subset + R8.1 deterministic fuzz harness over parsers/decoders | Full safe XML gate with fuzzing and resource budgets. |
| CSS cascade | Inline/simple style subset | Selectors, inheritance, specificity for supported properties. |
| Paths | Shared tokenizer, retained command semantics, nonzero/evenodd fills, affine cap/join/miter/dash strokes, transformed stroke bounds, anti-aliased coverage, markers (R9), and `vector-effect: non-scaling-stroke` (R9) | Reusable exact curve/arc fill bounds, exact SVG `arcs` joins, and broader conformance corpus. |
| Paint | Solid colors/opacity, deterministic linear/radial gradient fills and strokes, and tiled patterns (R9: patternUnits/contentUnits/viewBox/transform/href, bounded + cycle-safe) | Color-space policy and broader conformance corpus. |
| Text | Chunked multi-label editable import (R6) + Image-mode raster snapshot via bundled Hershey simplex vector font with textPath (R11), honest approximation diagnostics | Real font-file glyphs, font selection, bidi, shaping. |
| Clipping/masking | clipPath rendered (clip-rule, transforms, both units, nested intersection, nested-`<svg>` overflow); alpha + luminance masks rendered via the R4 offscreen (R7) | objectBoundingBox mask content units; mask region clipping. |
| Filters | Tier-1 (R7) + tier-2 (R10) graph: blur/offset/flood/merge/colorMatrix/dropShadow + composite/blend/componentTransfer/morphology on the R4 offscreen, premultiplied, capped, in linearRGB by default with precise filter-region clipping and `mix-blend-mode`; tier-3 primitives partial + diagnosed | Tier-3 primitives (turbulence/displacement/convolution/lighting/tile/image). |
| Images | PNG (zlib/unfilter, types 0/2/3/4/6, 8/16-bit) and baseline JPEG (Huffman/IDCT/YCbCr, 4:4:4/4:2:2/4:2:0, restart) `data:` decoded + rendered (R5) through R4 clip/compositing; external refs rejected | Progressive JPEG (deferred); broader format/corpus coverage. |
| Golden tests | Initial fixtures | Broad fixture suite plus differential tests against reference outputs where allowed. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Security corpus | Malicious/edge SVG fixtures produce deterministic safe failures. |
| Fidelity score calibration | Low score correlates with visible loss in fixture reviews. |
| Golden drift | Supported render fixtures are byte/signature stable. |
| Import editability | Common UI SVGs become editable widgets with useful grouping/provenance. |
| Unsupported clarity | Every skipped major feature has a specific diagnostic and fallback explanation. |

## Recommended Next Work

Detailed sequencing is authoritative in `docs/SVG_RENDERER_ROADMAP.md`:

1. R0-R8 are complete for their documented subsets — the SVG renderer roadmap is
   closed (R4: clipPath/overflow/premultiplied compositing/group opacity; R5:
   zero-dependency PNG + baseline JPEG `data:` decode + render; R6: editable
   chunked multi-label text import; R7: alpha/luminance masks + filter tier-1;
   R8: in-app report UI + source viewer, golden corpus, benchmark, dev-only
   oracle). Deferred, runtime-diagnosed follow-ons: progressive JPEG
   (`image.unsupported_jpeg`), the R6 vector-outline snapshot / raster text, and
   filter tier 2/3 (`filter.unsupported_primitive`).
2. Keep R1-R3 quality under regression coverage while expanding real-world
   fixtures and reference comparisons.
3. Future (post-roadmap): broader licensed conformance corpus + fuzzing, and the
   deferred follow-ons above.
