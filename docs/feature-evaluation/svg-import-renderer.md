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
| Security gate | 4 | Rejects DOCTYPE/entities and guards unsupported risky input. | Needs broader fuzz corpus and size/resource telemetry. |
| Import parser | 3 | Handles common shapes, paths, groups, style subset, transforms, `use`, metadata. | Needs richer CSS, text/tspan, clipping/masking behavior, and report UI. |
| Diagnostics | 3-4 | Structured warnings/errors/fidelity for many unsupported buckets; raster node diagnostics carry stable node IDs and source byte spans. | Needs R8 UI surfacing and per-element remediation suggestions. |
| Source preservation | 4 | Original SVG preserved beside imported template and in Image widgets. | Needs source diff/viewer integration polish. |
| Shared microsyntax | 4 | Shared colors, numbers, lengths, checked transforms, path tokenization, preserveAspectRatio/viewBox mapping, declarations, and bounded tier-1 CSS selectors/cascade. | Needs broader fuzz/property tests and only those later CSS tiers justified by fixtures. |
| Raster IR | 4 | Stable source-spanned node IDs, bounded expanded local references, flattened scene items, owned lowered display commands (incl. clip geometry + BeginLayer/EndLayer compositing scopes), and reusable solid/linear/radial paint-server IR. | Later phases add image/text/effects IR. |
| Raster renderer | 3-4 | Own software rasterizer for supported subset; retained paths; viewport mapping; nonzero/evenodd fills; affine caps/joins/miters/dashes; deterministic 8x8 coverage; bounded use/symbol expansion; and linear/radial gradient fills/strokes with units, transforms, spread, href, CSS/currentColor stops, diagnostics, goldens, and performance tests. | Still lacks patterns, text, filters, masks, vector effects, markers, and exact arcs joins. clipPath clipping, nested-`<svg>` overflow, premultiplied-alpha isolated group compositing, and group opacity landed in R4. |
| Text/tspan | 3 | Chunked multi-label import: positioned spans become separate grouped labels with provenance + per-chunk anchor/baseline diagnostics; relative/styled spans flatten with warnings. Raster text deferred. | Vector-outline snapshot / raster text, bidi/shaping, textPath. |

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
| XML/security | Hardened subset | Full safe XML gate with fuzzing and resource budgets. |
| CSS cascade | Inline/simple style subset | Selectors, inheritance, specificity for supported properties. |
| Paths | Shared tokenizer, retained command semantics, nonzero/evenodd fills, affine cap/join/miter/dash strokes, transformed stroke bounds, and anti-aliased coverage | Reusable exact curve/arc fill bounds, markers, vector effects, exact SVG `arcs` joins, and broader conformance corpus. |
| Paint | Solid colors/opacity plus deterministic linear/radial gradient fills and strokes; patterns are explicit transparent unsupported paint servers | Pattern tiling, color-space policy, and broader conformance corpus. |
| Text | Chunked multi-label editable import (positioned spans → grouped labels, anchor/baseline diagnostics) | Vector snapshot / raster text, font selection, bidi, shaping, fallback. |
| Clipping/masking | clipPath rendered (clip-rule, transforms, both units, nested intersection, nested-`<svg>` overflow); masks diagnosed | Mask offscreen buffers; objectBoundingBox-on-group; clip on text/image. |
| Filters | Unsupported diagnostics | Optional safe subset or clear visual fallback. |
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

1. R0-R6 are complete for their documented subsets (R4: clipPath clipping,
   nested-`<svg>` overflow, premultiplied-alpha compositing, isolated group
   opacity; R5: zero-dependency PNG and baseline JPEG `data:` decode + render
   through the R4 pipeline; R6: editable chunked multi-label text import with
   anchor/baseline diagnostics). Next is R7 masks/filters. Deferred follow-ons:
   progressive JPEG (`image.unsupported_jpeg`) and the R6 vector-outline snapshot
   / raster text rendering.
2. Keep R1-R3 quality under regression coverage while expanding real-world
   fixtures and reference comparisons.
3. Continue effects and conformance through R7-R8. Report UI belongs to R8.
