# RohKai SVG Text Import Plan

RohKai should treat imported SVG text as editable design material first and a
pixel-perfect rendering problem second. The original `.svg` remains the source
of truth whenever text layout exceeds RohKai's editable placeholder model.

## Current State

- Simple `<text>` imports as an editable RohKai `Label`.
- Simple `<tspan>` content is flattened into that label.
- Positioned or styled `tspan` content produces diagnostics and is approximated.
- Bounds use deterministic placeholder metrics rather than real font metrics.
- `textPath`, complex shaping, bidi layout, and full font handling are not
  implemented.

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

1. Robust `tspan` parser and span metadata.
2. Multi-label grouped import for positioned spans.
3. Optional vector-outline snapshot mode for visual comparison.
4. RohKai-owned text layout/shaping engine only if the editable workflow still
   needs it after phases 1-3.

## Non-Goals For The Next SVG Hardening Pass

- No new crates.
- No full text renderer.
- No external font loading.
- No browser CSS layout engine.
- No conversion of normal text into dead outlines by default.
