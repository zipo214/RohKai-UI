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
| Shared microsyntax | 3-4 | Shared colors, numbers, transforms, path tokenization, length/unit resolution, and preserveAspectRatio/viewBox mapping. | Needs shared style declarations plus broader fuzz/property tests. |
| Raster IR | 3-4 | Stable source-spanned node IDs, bounded local references, flattened scene items, and owned lowered display commands. | R2 still needs actual reference expansion; later phases need reusable paint/compositing IR. |
| Raster renderer | 2-3 | Own software rasterizer for supported subset; golden fixtures; full root/nested preserveAspectRatio mapping with per-viewport percentage bases. | Far from full SVG 1.1/2, text, filters, gradients, masks, compositing, and nested viewport clipping. |
| Text/tspan | 1-2 | Simple text import/flattening; text renderer planned. | Needs robust span model, bidi/shaping decisions, editable multi-span output. |

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
| Paths | Shared tokenizer and bounds/render subset | Full path semantics, fill rules, markers, stroke joins/caps. |
| Paint | Solid colors/opacity | Gradients, patterns, paint servers with diagnostics/fallbacks. |
| Text | Simple labels/flattened spans | Text runs, font selection, bidi, shaping, fallback, editable grouping. |
| Clipping/masking | Diagnostics awareness | Correct render where supported, import warnings where not editable. |
| Filters | Unsupported diagnostics | Optional safe subset or clear visual fallback. |
| Images | Placeholder/source preservation | Data URI policy, image decode strategy, export/runtime handling. |
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

1. Complete R1 geometry/stroke/antialiasing quality; R0 metadata and IR closure
   are now complete.
2. Complete R2 shared styles and local reference expansion.
3. Continue paint, clipping, images, text, effects, and conformance through
   R3-R8. Robust `tspan` work belongs to R6; report UI belongs to R8.
