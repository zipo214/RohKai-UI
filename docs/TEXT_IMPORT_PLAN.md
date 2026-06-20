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
- Raster text rendering (vector-outline snapshot) and `textPath` landed in SVG
  renderer lane R11: Image mode renders text via a bundled public-domain Hershey
  simplex stroked font (ASCII coverage, honest `text.raster_snapshot`
  diagnostics). Complex shaping, bidi layout, and real font-file handling remain
  not implemented (diagnosed tofu, never silently wrong).

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
3. Optional vector-outline snapshot mode for visual comparison. **(done — R11:
   Image-mode rasterizer renders text via the bundled Hershey simplex vector
   font with textPath; editable import stays the component-import default.)**
4. RohKai-owned text layout/shaping engine. **(scheduled — master backlog S3
   builds the `ShaperEngine` HarfBuzz-class port; S9 integrates real `.ttf`/
   `.otf` glyphs + shaping/bidi into the rasterizer.)** No longer conditional.

## Posture For The Next SVG Hardening Pass (formerly "Non-Goals")

Deferral is no longer an option (`docs/ROADMAP_PHASE2.md`). These are **default
postures and one invariant**, not parked capabilities:

- **Invariant:** no new SVG/text renderer crate — the shaper is a pure-Rust,
  zero-C port in `src/canvas/shaper/` (rustybuzz is the interim main-app engine
  only, never embedded in `svg_rasterizer.rs`).
- Full text renderer → **S3** (shaper) + **S9** (rasterizer glyphs).
- External / custom font loading → **S9** (designer font load) + **S16**
  (opt-in external resources). Default stays no-external-load.
- Browser CSS layout for text → **S10** / **S16**.
- Editable-first stays the **default**: normal text is not converted to dead
  outlines unless the user picks Image/snapshot mode.
