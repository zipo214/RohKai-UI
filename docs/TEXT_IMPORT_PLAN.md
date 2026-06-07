# RohKai SVG Text Import Plan

RohKai should treat imported SVG text as editable design material first and a
pixel-perfect rendering problem second. The original `.svg` remains the source
of truth whenever text layout exceeds RohKai's editable placeholder model.

## Current State (phases 1-2 implemented, 2026-06-06)

- `<text>` imports as one or more editable RohKai `Label` widgets via a
  `TextChunk` model: the content is split at every absolutely-positioned
  (`x`/`y`) `<tspan>`, so positioned spans become separate, grouped labels
  instead of collapsing into one misleading label.
- Sibling chunks share a `SvgImportMetadata::text_group` id; simple single-chunk
  text stays one ungrouped label.
- Relative/styled `<tspan>` content flattens into the surrounding chunk with
  explicit `text.tspan_adjust` / `text.tspan_style` diagnostics; per-chunk
  provenance records the owning element and warning flags.
- `text-anchor` (start/middle/end) and `dominant-baseline`
  (middle/central/hanging applied, others approximated with a `text.baseline`
  diagnostic) are handled per chunk. `text.missing_font` flags placeholder
  metrics.
- Bounds use deterministic placeholder metrics rather than real font metrics.
- Raster text rendering (vector-outline snapshot), `textPath`, complex shaping,
  bidi layout, and full font handling are not implemented (phase 3+).

## Desired Designer Behavior

- Preserve readable, editable text whenever possible.
- Preserve source provenance for every imported text placeholder.
- Prefer explicit warnings over fake fidelity.
- Use grouped labels for positioned spans before considering vector outlines.
- Keep original SVG beside the `.rktp` for exact visual reference.

## Future Data Model

The future text importer should parse SVG text into an intermediate model before
creating RohKai widgets:

- `TextRun`: text, resolved style, source element, local offsets.
- `TextChunk`: one independently positioned SVG text chunk.
- `TextLayout`: chunks, approximate bounds, transform, fidelity, diagnostics.
- `TextProvenance`: original element name/id/class, source order, tspan path,
  applied transform, warning flags.

## Comparison Targets

- SVG 2 text model: text chunks, `x/y/dx/dy`, anchors, baselines, writing modes,
  text-on-path, and CSS-aligned layout behavior.
- OpenType/HarfBuzz-class shaping: glyph substitution, glyph positioning,
  ligatures, kerning, script-specific shaping, and font metrics.
- Unicode bidi: mixed left-to-right and right-to-left text ordering.

These are quality references, not a mandate to implement a browser or HarfBuzz
clone in the next pass.

## Phasing

1. Robust `tspan` parser and span metadata. **(done)**
2. Multi-label grouped import for positioned spans. **(done)**
3. Optional vector-outline snapshot mode for visual comparison. **(deferred)**
4. RohKai-owned text layout/shaping engine only if the editable workflow still
   needs it after phases 1-3. **(deferred)**

## Non-Goals For The Next SVG Hardening Pass

- No new crates.
- No full text renderer.
- No external font loading.
- No browser CSS layout engine.
- No conversion of normal text into dead outlines by default.
