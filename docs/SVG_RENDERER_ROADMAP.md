# RohKai SVG Renderer Roadmap

Purpose: derive the work needed for RohKai's zero-new-dependency SVG renderer
by comparing the current in-repo importer/rasterizer against the combined
capability profile of mature SVG engines. This is not a promise to implement
all of SVG at once. It is the inventory, gap map, and staged work plan.

## Authority And Scope

This file is the **single detailed source of truth** for SVG import maturity,
SVG Image rasterization, SVG text import, renderer diagnostics, conformance,
and SVG-facing editor UX.

- `docs/ROADMAP.md` records only strategic summaries and historical stage
  snapshots. If its SVG wording conflicts with this file, this file wins.
- `docs/TEXT_IMPORT_PLAN.md` is the design note for Phase R6 and does not define
  an independent schedule.
- `docs/SVG_IMPORT.md` describes current shipped behavior, not future closure.
- Stage 15 "Own Renderer" is separate. It concerns replacing egui as RohKai's
  general UI/runtime renderer; completing SVG R0-R8 does not activate or
  complete Stage 15.

## Current Execution Order

1. R0-R8 are complete for their documented subsets (R5: PNG and baseline JPEG
   `data:` images; R6: editable text import — chunked multi-label with
   anchor/baseline diagnostics; R7: alpha/luminance masks + filter tier-1 on the
   R4 offscreen pipeline; R8: in-app report UI + source viewer, golden corpus,
   benchmark, and dev-only oracle). **The SVG renderer roadmap is closed.**
2. Deferred follow-ons (tracked, runtime-diagnosed): progressive JPEG, the R6
   vector-outline snapshot / raster text, and filter tier 2/3.

Unchecked derivative-backlog entries are implementation notes for these phases,
not separate roadmap phases.

## Ground Rules

- No `resvg`, `usvg`, `tiny-skia`, browser embedding, or substitute renderer
  dependency chain.
- No hollow features: every checked item needs parser behavior, renderer output,
  diagnostics for unsupported cases, tests, and documentation.
- Preserve the original SVG source whenever fidelity is partial.
- Prefer a secure static-image profile first. Animation, scripting, external
  network resources, and browser DOM behavior remain out of scope unless the
  user explicitly opens that lane.

## Comparison Targets

The "best combined" feature inventory below is drawn from:

- [W3C SVG 1.1](https://www.w3.org/TR/SVG11/) and
  [SVG 2 / SVG Native work](https://w3c.github.io/svgwg/specs/svg-native/index.html)
  for the standard feature surface and secure-static constraints.
- [MDN SVG element reference](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Element)
  for the broad element inventory used by web authors.
- [librsvg feature documentation](https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/features.html)
  as a mature static renderer profile: mostly SVG 1.1/SVG 2, no scripting or
  animation, with explicit security handling for referenced files.
- [Apache Batik](https://xmlgraphics.apache.org/batik/index) and
  [Batik implementation status](https://xmlgraphics.apache.org/batik/status)
  as a broad toolkit model: DOM, microsyntax parsers, rasterizer, transcoder,
  scripting/animation lanes, and detailed support levels.
- [Skia](https://skia.org/docs/) as the high-performance 2D rendering baseline:
  paths, curves, transforms, text, shaders, compositing, and platform rendering
  quality. Skia is not an SVG feature spec; it is the rendering-engine quality
  bar.

## Current RohKai Snapshot

### Importer: `src/svg_import.rs`

Current strengths:

- Public API:
  - `parse_svg_template(svg) -> Result<Vec<WidgetInstance>, String>`
  - `import_svg_template(svg, SvgImportOptions) -> Result<SvgImportOutput, SvgImportError>`
- Structured report:
  imported/skipped counts, warnings, unsupported features, fidelity level.
- Limits:
  file bytes, tag count, attributes per tag, attribute length, nesting depth,
  path command count, generated placeholders, image data URI bytes, use depth,
  style bytes.
- XML safety:
  rejects `DOCTYPE` and custom entities; decodes only safe built-in entities;
  rejects external image/use refs; diagnostics for unknown entities and
  processing instructions.
- Geometry import:
  `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `path`, `image`
  placeholder, local `use`, `symbol`, `text`.
- Style subset:
  presentation attributes, inline `style=""`, bounded tier-1 element/class/id
  compound and grouped selectors with specificity/source order, inherited
  display/visibility/opacity/font basics, and `currentColor`.
- Transforms:
  `matrix`, `translate`, `scale`, `rotate`, `skewX`, `skewY`; nested stack.
- Paths:
  `M/L/H/V/C/S/Q/T/A/Z`, relative and absolute, compact syntax, deterministic
  curve/arc bounds.
- Text:
  simple `<text>` becomes editable `Label`; simple `tspan` is flattened with
  diagnostics for positioned/styled spans.
- Determinism:
  source order metadata and deterministic placeholder IDs are tested.

Current limits:

- It imports editable placeholders, not pixel-perfect SVG.
- It approximates opacity into RGB widget colors.
- It diagnoses, but does not visually apply, gradients, patterns, masks, clips,
  filters, animation, textPath, complex CSS, or external resources.

### Rasterizer: `src/canvas/svg_rasterizer.rs`

Current strengths:

- Structured renderer API:
  `rasterize_with_report()` returns pixels plus `SvgRenderReport`; existing
  `rasterize()` and `rasterize_or_fallback()` wrappers remain stable.
- Pure Rust in-repo software rasterizer returning `egui::ColorImage`.
- Security gates:
  byte limit, tag count limit, forbidden `DOCTYPE`, entity/script checks,
  external href rejection, raster dimension/pixel caps.
- Diagnostics:
  rendered/skipped counts, parsed-node/attribute unsupported feature list,
  warnings, output raster dimensions, conservative fidelity score, and
  source-spanned node provenance on node-level diagnostics.
- Full root and nested viewport mapping for `preserveAspectRatio`: `none`, all
  nine alignments, and `meet`/`slice`. Nested viewport overflow clipping remains
  a later clipping-phase item.
- XML-ish parser for common SVG files.
- Style inheritance:
  presentation attributes, inline declarations, bounded tier-1
  element/class/id/grouped selectors, specificity/source order, solid
  fill/stroke, stroke geometry, opacity, display/visibility, and
  `currentColor`.
- Closed scene/display-list boundary:
  parsed nodes receive stable preorder `SvgNodeId` values and exact byte spans;
  a bounded first-id-wins local reference table records resolved/unresolved
  fragment uses; scene items accumulate transforms and resolved inherited
  style; the owned display list lowers lengths, shape geometry, path geometry,
  diagnostics, and provenance before raster execution.
- References:
  bounded first-id-wins `defs`/`symbol`/`use` expansion with source-order
  stability, cycle/depth/node limits, duplicate-ID diagnostics, symbol viewBox
  mapping, and inherited style.
- Paint servers:
  solid paint plus linear/radial gradients with stop color/opacity, CSS stop
  style, objectBoundingBox/userSpaceOnUse units, gradient transforms,
  pad/reflect/repeat spread, bounded href inheritance, deterministic sampling,
  and gradient strokes through the existing union-coverage backend. Invalid
  gradient values and unresolved paint references produce source-spanned
  warnings. Patterns preserve exact unsupported attributes and remain
  transparent.
- Shared lengths:
  importer and rasterizer use `svg_core` for unitless/px, percentages, absolute
  physical units, and font-relative unit parsing. Raster geometry resolves
  percentages against the active user viewport axes.
- Colors:
  `#rgb`, `#rrggbb`, `rgb(...)`, and a small named-color table.
- Geometry:
  `rect` with `rx/ry`, `circle`, `ellipse`, `line`, `polyline`, `polygon`,
  `path`.
- Paths:
  all standard commands including relative variants; cubic flattening has depth
  and point-count caps; arcs approximate to lines.
- Rendering:
  deterministic 8x8 subpixel coverage with separate fill and stroke modes.
  Fills evaluate inherited `nonzero`/`evenodd` winding or parity per sample;
  strokes union their tessellated pieces before one alpha composite so joins
  and overlapping pieces do not darken. Invalid fill-rule values keep the
  inherited/default rule and produce source-spanned warnings.
- Stroke geometry:
  local-space width expansion followed by the complete affine transform;
  butt/round/square caps; miter, miter-clip, round, and bevel joins; miter
  limits; dash arrays, signed dash offsets, closed-seam continuation,
  zero-length round/square strokes, and `pathLength` dash calibration. The SVG
  `arcs` join is accepted with a source-spanned warning and approximated by
  bounded miter-clip geometry.
- Bounds and budgets:
  stroke meshes retain local and transformed device bounds. Dash runs,
  primitives, vertices, path tokens, flattened points, raster dimensions, and
  total pixels are bounded; runtime stroke truncation emits
  `limit.stroke_complexity` rather than failing silently.
- Cache integration:
  canvas previews use texture caching and bounded raster dimensions.

Current limits:

- No real XML namespace model.
- PNG (zlib/DEFLATE + unfilter, color types 0/2/3/4/6 at 8/16-bit) and baseline
  JPEG (Huffman entropy, IDCT, YCbCr, 4:4:4/4:2:2/4:2:0 subsampling, restart
  markers) `data:` images are decoded and rendered through the R4 clip/compositing
  pipeline. Progressive/arithmetic/CMYK/12-bit JPEG and external sources are
  diagnosed rather than rendered.
- No pattern rendering, markers, or blend modes. (Alpha/luminance masks and
  filter tier-1 — blur/offset/flood/merge/colorMatrix/dropShadow — render via the
  R4 offscreen pipeline as of R7; tier 2/3 filter primitives are diagnosed.)
- Text imports as editable chunked labels (positioned spans become separate
  grouped labels with per-chunk anchor/baseline diagnostics); raster text
  rendering (vector-outline snapshot) is deferred, so the rasterizer still reports
  `<text>` in the `text` unsupported bucket.
- No full CSS media/import/pseudo/attribute/combinator selector model.
- No gamma-correct compositing pipeline. (Premultiplied-alpha compositing and
  isolated group opacity are implemented for layer compositing; the base buffer
  remains straight-RGBA, so non-grouped output is byte-stable with prior goldens.)
- `clipPath` is implemented (clip-rule, transforms, both clipPathUnits with
  shape bounding boxes, nested clip intersection, nested-`<svg>` overflow).
  objectBoundingBox clip units on a group element have no single bounding box and
  are diagnosed/skipped rather than approximated.
- No `vector-effect`, markers, exact SVG `arcs` line-join geometry, or SVG 2
  context-sensitive stroke semantics.
- No broad external reference-renderer conformance corpus. RohKai has a small,
  dependency-free ASCII golden suite for current fills, transforms, opacity,
  anti-aliased diagonals, compound fill rules, and dashed strokes.

## Combined Feature Inventory

This is the superset of what mature SVG engines tend to cover. RohKai does not
need all of it immediately, but this is the map.

### 1. Input, XML, And Security

- XML tokenizer/parser with comments, CDATA, processing instructions,
  namespaces, qualified names, entity policy, error recovery, and source spans.
- Bounded parse budgets: bytes, tags, attributes, attr value length, text length,
  nesting depth, path tokens, output pixels, filter buffers, reference depth.
- Secure static profile:
  no scripts, no external network/file loads, no unbounded entities, no
  recursive reference explosions.
- Strict diagnostics:
  rejected, ignored, approximated, fallback-rendered, and fully-rendered cases
  are different states.

### 2. Document Model And References

- Element tree / render tree separation.
- `id` map, duplicate-id policy, source order, provenance.
- `defs`, `symbol`, `use`, `view`, nested `svg`, `switch`.
- Reference resolution for `url(#id)` and `href`/`xlink:href`.
- Cycle detection and bounded expansion.
- Display-list or scene graph IR usable by importer, renderer, diagnostics, and
  future editor features.

### 3. Lengths, Units, Coordinates

- Unitless, px, pt, pc, mm, cm, in, em, ex, rem, percentages.
- Viewports, nested viewports, `viewBox`, `preserveAspectRatio`.
- `userSpaceOnUse` vs `objectBoundingBox`.
- Bounding boxes for geometry, stroke, markers, clips, masks, filters.
- Transform lists and CSS transforms.

### 4. CSS And Style Cascade

- Presentation attributes.
- Inline `style=""`.
- `<style>` blocks.
- Selector support tiers:
  simple element, class, id, descendant, child, grouped selectors.
- Specificity, order, inheritance, `currentColor`, `inherit`, `initial`,
  `unset`, CSS variables if allowed, and disabled external imports.
- Supported properties:
  fill/stroke paint, stroke geometry, opacity, display/visibility, clip/mask,
  filter, font/text, color-interpolation, vector-effect.

### 5. Geometry And Paths

- Basic shapes:
  `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`.
- Full path grammar:
  implicit repeats, compact numbers, invalid recovery, all command variants.
- Fill rules:
  nonzero and evenodd.
- Stroke:
  width, linecap, linejoin, miterlimit, dasharray, dashoffset.
- Markers:
  start/mid/end markers with orient, markerUnits, viewBox.
- Vector effects:
  especially non-scaling stroke.
- Robust path math:
  arc conversion, curve subdivision tolerance, extrema, exact bounds.

### 6. Paint Servers

- Solid colors and opacity.
- Linear gradients:
  stops, stop-opacity, spreadMethod, gradientUnits, gradientTransform, href
  inheritance.
- Radial gradients:
  focal point, radius, spread, transforms, inheritance.
- Patterns:
  patternUnits, patternContentUnits, viewBox, patternTransform, nested content.
- Color interpolation and gradient sampling quality.

### 7. Raster Images

- Embedded `data:` images.
- PNG and JPEG decoding if allowed.
- Size, aspect ratio, transforms, opacity, clipping/masking.
- Explicit policy for external image refs: reject for security unless a future
  local-only resource system is added.
- Color-space handling, alpha handling, and failure placeholders.

### 8. Text

- `text`, `tspan`, `textPath`, `altGlyph`-style legacy handling if desired.
- x/y/dx/dy lists, rotate lists, textLength, lengthAdjust.
- font-family fallback, style, weight, stretch, size, decoration.
- Unicode bidi and writing modes.
- Shaping/kerning/ligatures/OpenType feature behavior.
- Baselines:
  dominant-baseline, alignment-baseline, baseline-shift.
- Text anchors and chunk layout.
- Text-to-path/vector snapshot mode as an optional fidelity fallback.

### 9. Clipping, Masking, And Compositing

- `clipPath` with clip-rule and nested transforms.
- `mask` with maskUnits, maskContentUnits, luminance/alpha behavior.
- Group opacity and isolated compositing.
- Blend modes.
- Overflow clipping on nested SVG viewports.

### 10. Filters

- Filter region computation.
- Primitive graph execution and result buffers.
- Common primitives:
  `feGaussianBlur`, `feOffset`, `feBlend`, `feComposite`, `feColorMatrix`,
  `feFlood`, `feMerge`, `feDropShadow`, `feImage`.
- Broader primitives:
  component transfer, morphology, displacement map, turbulence, lighting,
  convolution, tile.
- Color-interpolation-filters and buffer precision policy.

### 11. Raster Quality

- Subpixel transforms.
- Anti-aliased fills and strokes by area coverage or supersampling.
- Premultiplied-alpha internal pipeline with correct straight-RGBA output.
- Gamma/color-space policy.
- Stroke joins/caps/dashes with high-quality edges.
- Deterministic output across platforms.

### 12. Performance And Memory

- Display-list compilation.
- Scene graph cache.
- Texture/raster cache by source hash, size, device scale, and style env.
- Incremental rerender by dirty region or node.
- Bounded temporary allocations.
- Path flattening caches.
- Filter/image buffer limits.
- Optional tile rendering for large SVGs.

### 13. Tests, Conformance, And Tooling

- Tiny malicious input tests.
- Fuzz-style malformed XML/path/style corpus.
- Golden-pixel fixtures for each supported feature.
- Reference comparison harness against browser/librsvg/resvg only as external
  test oracles, never as runtime dependencies.
- W3C-style feature matrix.
- Benchmark corpus and memory-limit tests.
- Diagnostics snapshots.

### 14. RohKai Editor Integration

- Source preservation and source viewer.
- Editable placeholder import and image-mode raster preview.
- Per-node provenance, warnings, and fidelity report UI.
- Round-trip behavior:
  source SVG stays intact; editable approximations are clearly labeled.
- Export parity:
  generated app embeds the same supported renderer or declares unsupported
  features accurately.

## Gap Matrix

| Capability | Mature renderer expectation | RohKai now | Gap | Priority |
|---|---|---|---|---|
| Secure parse budgets | Comprehensive limits across parse/render/filter/reference work | Basic importer + rasterizer limits | Add unified limit config/report for rasterizer too | P0 |
| XML model | Namespace-aware XML tree with source spans | Simple custom parser with stable byte spans | Add namespaces and better recovery | P1 |
| Render IR | Scene/display-list separate from XML | Stable source-spanned node IDs, bounded expanded local references, flattened scene items, owned geometry/diagnostic/clip commands, BeginLayer/EndLayer compositing scopes, and reusable paint-server IR | Extend for masks/filters (R7) | P0/P1 |
| Diagnostics | Structured per-node render/import diagnostics | Importer rich; rasterizer reports warnings, unsupported buckets, counts, fidelity, and node ID/byte-span provenance, surfaced in the R8 properties report panel + source viewer | Closed | P1 |
| ViewBox/preserveAspectRatio | Full modes and nested viewport behavior | Full `none`/alignment/meet/slice mapping for root and nested SVG viewports; nested viewport overflow is clipped (R4) | Closed | P1/P2 |
| CSS cascade | Specificity/order/inheritance subset | Shared bounded tier-1 element/class/id compound and grouped selectors, inline/presentation precedence, inheritance, and currentColor | Add only selector/property tiers justified by real fixtures | P1/P3 |
| Basic shapes | Full geometry and transforms | Supported subset with affine transforms, anti-aliased fill/stroke, and transformed stroke-bound tests | Add reusable exact fill/path bounds and more extreme-coordinate tests | P0/P1 |
| Paths | Full grammar, fill rules, stroke geometry | Full command grammar; retained curves/arcs; nonzero/evenodd fills; local-space cap/join/miter/dash stroke meshes | Exact arc extrema, SVG `arcs` join, vector effects, markers | P1/P3 |
| Gradients | Linear/radial with units, transforms, spread, href | Linear/radial fills and strokes support stops/opacity, both units, transforms, all spread modes, href inheritance, currentColor/CSS stops, deterministic sampling, limits, diagnostics, and goldens | Add color-space/gamma policy and broader conformance fixtures | P2 |
| Patterns | Tiled nested content | Unsupported/diagnosed | Implement after scene IR | P3 |
| Images | Embedded PNG/JPEG and secure external policy | PNG (zlib/unfilter, types 0/2/3/4/6, 8/16-bit) and baseline JPEG (Huffman/IDCT/YCbCr, 4:4:4/4:2:2/4:2:0, restart) `data:` decoded + rendered through R4 clip/compositing; external refs fail-closed | Progressive JPEG; broader format/corpus | P2 |
| `defs`/`use` | Id resolution and expansion | Importer and rasterizer support bounded local symbol/use expansion with cycles, depth/node limits, duplicate-ID diagnostics, and source-order stability | Extend references only as later phases require | P1 |
| Clips | Actual clipping stack | clipPath rendered: clip-rule, transforms, both clipPathUnits (shape bbox), nested intersection, cycle/depth caps, plus nested-`<svg>` overflow clipping (R4) | objectBoundingBox-on-group; clip on text/image when those land | P2 |
| Masks | Alpha/luminance masks | Alpha + luminance masks rendered through the R4 offscreen (shape/gradient mask content, `mask-type`, bounded by item/offscreen caps) | objectBoundingBox content units; mask region clipping | P3 |
| Filters | Primitive graph | Tier-1 graph on R4 offscreen (premultiplied, capped): blur/offset/flood/merge/colorMatrix/dropShadow; unsupported primitives partial + diagnosed | Tier 2/3 primitives; full filter-region clipping | P4 |
| Text | Full layout/shaping | Editable chunked multi-label import (positioned spans → grouped labels, per-chunk anchor/baseline diagnostics); raster text skipped | Vector-outline snapshot / raster text; shaping/bidi/textPath | P3/P4 |
| Markers | Arrowheads and symbols on paths | Unsupported | Add after stroke geometry | P3 |
| Antialiasing | High-quality coverage | Deterministic 8x8 coverage; winding/parity fills and unioned stroke coverage are separate | Tune quality/performance and add gamma-aware compositing | P1/P2 |
| Compositing | Correct premultiplied alpha/groups | Premultiplied-alpha offscreen compositing for isolated group opacity (no double-darken, halo-free); straight-RGBA base buffer + output (R4) | Gamma-correct base pipeline; blend modes | P1 |
| Tests | Golden corpus + fuzz + oracles | Per-feature ASCII golden corpus + pixel-exact unit tests across R0-R7, an ignored benchmark, and a dev-only deterministic reference-oracle stand-in | Broader licensed corpus + fuzzing (future) | P0/P1 |

## Roadmap

### Phase R0 — Truth, Harness, And IR Boundary

Goal: make current behavior impossible to overclaim.

Tasks:

- [x] Add `docs/SVG_RENDERER_ROADMAP.md` as this truth inventory.
- [x] Add `SvgRenderReport`, `SvgRenderWarning`, and
  `SvgRenderUnsupportedFeature`.
- [x] Add `SvgRenderOutput { image, report }`.
- [x] Preserve `rasterize()` and `rasterize_or_fallback()` wrappers.
- [x] Add first `SvgScene` IR:
  viewBox, parsed nodes, flattened scene items, resolved inherited styles, and
  accumulated transforms. The later R0 tasks below subsequently added source
  spans, stable node IDs, bounded references, and richer provenance.
- [x] Add first internal `SvgSceneItem` flattening boundary with resolved
  inherited style, accumulated transforms, and unsupported-subtree flags.
- [x] Split renderer into:
  parser -> scene builder -> render backend.
- [x] Add first report tests for rendered/skipped counts, unsupported buckets,
  determinism, and raster-size clamping.
- [x] Move known unsupported element/attribute diagnostics from raw source scans
  to parsed node/attribute reporting.
- [x] Add first fixture per known supported and unsupported feature bucket:
  rect/circle-equivalent fills, path fill, stroke, transform, opacity,
  unsupported gradient, unsupported clip, unsafe external reference.
- [x] Add golden tests for currently supported primitives.
- [x] Add stable source-spanned `SvgNodeId` values and a bounded local reference
  table to `SvgScene`.
- [x] Make raster execution consume only scene/display-list IR; no direct
  XML-node-to-pixel traversal may remain.
- [x] Move shared SVG length/unit parsing into `svg_core`.

Acceptance:

- Rasterizer still renders current images.
- Unsupported features produce structured reports, not silent fallback.
- Current feature matrix is test-backed.

### Phase R1 — Core Geometry Quality

Goal: make basic vectors look reliably good.

Tasks:

- [x] Implement full `preserveAspectRatio` for root and nested SVG viewports:
  `none`, all nine alignments, and `meet`/`slice`; propagate per-viewport
  percentage bases through importer and raster display-list lowering.
- [x] Add inherited `nonzero` and `evenodd` fill-rule support for compound
  paths and closed geometry, with inline-style precedence, invalid-value
  warnings, analytical winding tests, and golden fixtures.
- [x] Replace stroke-as-quad with local-space stroke tessellation:
  butt/round/square caps; miter, miter-clip, round, and bevel joins; miter
  limits; dasharray, signed dashoffset, closed seams, zero-length caps, and
  `pathLength` calibration. Exact SVG `arcs` joins remain deferred and are
  reported as an approximation.
- [x] Add deterministic 8x8 anti-aliased coverage with two explicit modes:
  fill winding/parity for nonzero/evenodd and union coverage for stroke
  primitives.
- [x] Add retained local/device stroke bounds and analytical cap, miter,
  affine-transform, and rendered-alpha bound tests.
- [x] Add transform edge-case tests for rotation and non-uniform scaling of
  complete stroke outlines. Broader tiny/huge-coordinate and nested clipping
  torture cases continue in R4/R9.
- [x] Add bounded dash/primitive/vertex work and visible
  `limit.stroke_complexity` diagnostics.
- [x] Add selected anti-aliased diagonal, dashed round-cap, and
  self-intersecting evenodd golden fixtures plus a coarse ignored 512px fill
  performance smoke.

Acceptance:

- Basic icons and diagrams render without jagged obvious failures.
- Stroke-heavy fixtures match golden outputs.
- Pathological paths remain bounded.

### Phase R2 — Shared Style And References

Goal: remove duplicated importer/rasterizer understanding.

Tasks:

- [x] Extract shared SVG microsyntax modules:
  numbers, lengths, colors, transforms, paths, style declarations.
- [x] Implement selector tier 1:
  presentation attrs, inline style, element selectors, class selectors,
  id selectors, grouped selectors, specificity/order.
- [x] Implement reference resolver:
  `defs`, `symbol`, `use`, local paint refs, cycle/depth limits.
- [x] Add currentColor and basic inherited color behavior.
- [x] Add duplicate-id diagnostics.

Acceptance:

- Importer and rasterizer agree on colors/transforms/paths.
- `<use>` renders in image mode and imports in component mode.
- Complex CSS stays diagnosed instead of silently ignored.

### Phase R3 — Paint Servers

Goal: cover the biggest visual gap in real SVG art.

Tasks:

- [x] Add paint server IR:
  solid, linear gradient, radial gradient, pattern placeholder.
- [x] Linear gradients:
  stops, opacity, units, transform, spread methods, href inheritance.
- [x] Radial gradients:
  center/focal/radius, units, transform, spread methods, href inheritance.
- [x] Gradient sampling with deterministic interpolation.
- [x] Pattern diagnostics upgraded:
  report exact unsupported pattern attributes until real pattern rendering lands.

Acceptance:

- Gradient-heavy fixtures no longer fall to flat color.
- Fidelity scoring distinguishes fully rendered gradients from approximated
  patterns.

### Phase R4 — Clipping, Viewport Overflow, And Group Compositing

Goal: make exported design-tool SVGs stop leaking outside their intended masks.

Tasks:

- [x] Add clip stack (layer scope threaded through display-list execution; clip =
  coverage mask intersected before compositing each primitive).
- [x] Implement `clipPath` with transforms and clip-rule. Supports nonzero and
  evenodd `clip-rule`, transformed clipPath children, nested `<g>` in clipPath,
  clipPathUnits `userSpaceOnUse` and `objectBoundingBox` (shape bbox), and
  clipPath-of-clipPath intersection with cycle/depth bounds. objectBoundingBox on
  a `<g>` (no single bbox) is diagnosed `clip.object_bounding_box` and skipped.
- [x] Implement nested viewport overflow clipping (nested `<svg>` content is
  clipped to its viewport rect using the existing preserveAspectRatio mapping).
- [x] Add premultiplied-alpha internal buffer (isolated group offscreens
  composite in premultiplied space; straight-RGBA `ColorImage` output unchanged
  at the boundary; halo-free clipped/composited edges).
- [x] Add group opacity and isolated offscreen compositing (`<g opacity>` /
  `isolation:isolate` render to an offscreen composited once at group opacity, so
  overlapping children do not double-darken; bounded by offscreen depth/byte caps
  with a `limit.offscreen_buffer` diagnostic on truncation).

Acceptance:

- [x] Clip fixtures render visually clipped, not just diagnosed (ASCII goldens:
  rect clip, path clip nonzero/evenodd, transformed clip, objectBoundingBox clip,
  nested-svg overflow, translucent overlap group).
- [x] Alpha edges remain deterministic and not haloed (determinism +
  halo-free unit tests).

### Phase R5 — Embedded Raster Images

Goal: decide and implement real image policy without dependency creep.

Decision (2026-06-06): implement zero-dependency PNG and baseline JPEG decoders
for inline `data:` images. PNG landed first (lossless DEFLATE + unfilter); the
baseline JPEG decoder (Huffman + IDCT + YCbCr + chroma upsampling) followed.
Both are pure RohKai source, `data:`-only, and bounded — no new crates.

Tasks:

- [x] Implement zero-dependency PNG decode for inline base64 `data:` images:
  signature/chunk parse (IHDR/PLTE/tRNS/IDAT/IEND), from-scratch zlib + DEFLATE
  inflate (stored/fixed/dynamic Huffman), scanline unfilter (None/Sub/Up/Average/
  Paeth), and expansion to straight RGBA8 for color types 0/2/3/4/6 at bit depth 8
  (and 16 truncated to 8). Drawn through the R4 clip/premultiplied pipeline with
  nearest-neighbour sampling, `preserveAspectRatio` placement (`slice` overflow
  trimmed to the destination rect), element opacity, and `clip-path`.
- [x] Security caps cover image decode memory and CPU: pixel budget
  (`MAX_IMAGE_PIXELS`), inflate output bound (`MAX_IMAGE_DECODE_BYTES`), and
  bounded chunk reads. Malformed/oversized/unsupported inputs are diagnosed, not
  mis-decoded.
- [x] Honest diagnostics for every degraded path: external references
  (`image.external_rejected` / fail-closed document gate), non-PNG
  (`image.unsupported_format`), interlace or unsupported bit depth/color type
  (`image.unsupported_png`), decode failures (`image.decode_failed`), and
  oversize (`limit.image_pixels`).
- [x] **Baseline JPEG decode** — zero-dependency decoder for baseline /
  extended-sequential Huffman JPEG (SOF0/SOF1), 8-bit, 1 or 3 components
  (grayscale or YCbCr), arbitrary integer chroma subsampling (4:4:4 / 4:2:2 /
  4:2:0 …) with restart markers and `0xFF00` byte-stuffing: marker parse,
  quant/Huffman tables, entropy decode (DC diff + AC RLE in zigzag), dequantize,
  separable 8×8 IDCT, chroma upsampling, and YCbCr→RGB. Drawn through the same R4
  clip/premultiplied image path as PNG. Progressive, arithmetic, lossless,
  12-bit, and CMYK/4-component are diagnosed `image.unsupported_jpeg`; malformed
  input is `image.decode_failed`. Design note: `docs/jpegdecoder roadmap.md`.
  Remaining JPEG follow-ups (deferred): progressive JPEG, integer/AAN IDCT for
  speed, and a broader reference corpus.

Acceptance:

- [x] No "image supported" claim unless pixels really render: PNG `data:` images
  render real pixels in both the in-app and export-embedded rasterizers (same
  embedded source); JPEG and other formats/sources are explicitly diagnosed.
- [x] Security caps cover image decode memory and CPU (pixel + inflate-byte caps;
  bounded, deterministic failures with diagnostics).

### Phase R6 — Text Import And Optional Rendering

Goal: keep text editable first, then add fidelity modes.

Tasks:

- [x] Execute `docs/TEXT_IMPORT_PLAN.md` phase 1: a `TextChunk` model splits
  `<text>`/`<tspan>` at every absolutely-positioned span (`x`/`y`), with per-chunk
  font size, anchor, baseline, fill, and provenance (`source_node`, warning
  flags). Relative/styled spans flatten into the surrounding chunk with explicit
  `text.tspan_adjust` / `text.tspan_style` diagnostics.
- [x] Multi-label grouped import for positioned spans: each non-empty chunk
  imports as its own editable `Label`; sibling chunks share a
  `SvgImportMetadata::text_group` id. Simple single-chunk text stays one
  ungrouped label (no regression). Placeholder bounds remain deterministic and
  documented as approximate.
- [x] Anchor + baseline handling: `text-anchor` start/middle/end applied per
  chunk; `dominant-baseline` middle/central/hanging applied, others approximated
  with a `text.baseline` diagnostic. `text.missing_font` flags placeholder
  metrics.
- [ ] **Deferred:** optional vector-outline snapshot mode (raster text rendering)
  — the rasterizer still reports `<text>` as the `text` unsupported bucket. Will
  be added only after editable text + source preservation are proven, per
  TEXT_IMPORT_PLAN phase 3. `textPath`, bidi, and full shaping/kerning/ligatures
  remain deferred with explicit diagnostics.
- Defer owned shaping engine until the product proves it needs one.

Acceptance:

- [x] Text-heavy SVGs no longer collapse into misleading single labels without
  detailed warnings: positioned spans become separate grouped labels with
  provenance and per-chunk diagnostics.
- [ ] Users can choose editable approximation vs visual snapshot mode — editable
  import is done; the visual snapshot toggle is deferred to the R6 follow-on / R8
  report UI.

### Phase R7 — Masks And Filters

Goal: add expensive visual effects only after core geometry/paint is trustworthy.

Tasks:

- [x] Masks: alpha and luminance modes, `mask-type`, rendered through the R4
  offscreen pipeline. Mask content (shapes/gradients) is rendered with the shape
  renderer to a premultiplied buffer, reduced to a coverage alpha
  (luminance = `0.2125R+0.7154G+0.0721B`, or the alpha channel), then multiplied
  into the masked element's isolated offscreen. Bounded by `MAX_MASK_ITEMS` and
  the existing offscreen caps. `maskContentUnits=objectBoundingBox` is approximated
  in user space with a diagnostic.
- [x] Filters tier 1: `feGaussianBlur` (separable triple box-blur,
  radius-capped), `feOffset`, `feFlood`, `feMerge` (+`feMergeNode`),
  `feColorMatrix` (matrix/saturate/luminanceToAlpha), and `feDropShadow`.
  Primitive graph with named results, `in`/`SourceGraphic`/`SourceAlpha` inputs,
  executed in premultiplied space on the element's isolated offscreen; output is
  composited back as straight RGBA. Both masks and filters work on shapes and
  groups (shapes with mask/filter get a synthetic layer).
- [ ] **Deferred — Filters tier 2/3:** `feComposite`, `feBlend`,
  `feComponentTransfer`, `feMorphology`, `feTile`, `feImage`,
  `feDisplacementMap`, `feTurbulence`, convolution, lighting. These are passed
  through as identity with a `filter.unsupported_primitive` partial-output
  diagnostic. Full filter-region clipping (currently the whole canvas, which is
  already bounded) is also a refinement.

Acceptance:

- [x] Filter region limits prevent memory bombs: buffers are bounded by the
  offscreen byte/depth caps and blur radius is capped (`MAX_BLUR_RADIUS`); a huge
  `stdDeviation` completes without hanging.
- [x] Unsupported primitives show partial-output diagnostics
  (`filter.unsupported_primitive`); missing/invalid mask or filter refs warn
  (`mask.unresolved` / `filter.unresolved`) and leave the element rendered.

### Phase R8 — Conformance, Benchmarks, And Editor UX

Goal: make the renderer dependable, measurable, and visible in RohKai.

Tasks:

- [x] SVG report UI: the properties panel surfaces the selected SVG widget's
  `SvgRenderReport` — fidelity (colour-coded), rendered/skipped counts,
  warning + unsupported counts, and collapsible per-diagnostic lists with
  byte-span provenance. The report→rows mapping (`panels::svg_report::report_summary`)
  is a pure, unit-tested function; the panel only renders it (no new report
  computation beyond the existing `rasterize_with_report`).
- [x] Source viewer + "rendered report / SVG source" toggle for SVG widgets
  (read-only source view; toggle state in egui temp memory).
- [x] Golden-image corpus across geometry/paint/clip/mask/filter/image plus a
  polygon-geometry golden; raster text stays import-only (no raster golden).
  Malicious/edge inputs covered by the security unit tests in each phase.
- [x] Renderer benchmark suite (`#[ignore]`): `raster_benchmark_complex_scene
  _within_budget` measures parse+scene+raster of a 200-rect gradient/clip/stroke
  scene, alongside the existing ignored 512px fill smoke.
- [x] Dev-only reference-oracle workflow (`#[ignore]`
  `reference_oracle_scene_is_deterministic`): external reference renderers are
  developer/CI-artifact tools only, never runtime/Cargo dependencies; the in-repo
  stand-in asserts deterministic output so any external diff is reproducible.

Acceptance:

- [x] Every supported feature has a visual test (golden or pixel-exact unit test).
- [x] Roadmap claims match tests and diagnostics; report + provenance are visible
  in-app.

**SVG renderer roadmap R0–R8 complete.** Remaining work is explicitly deferred:
baseline-JPEG follow-ups (progressive JPEG), the R6 vector-outline snapshot /
raster text rendering, and filter tier 2/3 — all tracked above and diagnosed at
runtime. Animation, scripting, and external network/file loading stay out of
scope per the secure-static profile.

## Derivative Task Backlog

### Immediate Tasks

- `src/canvas/svg_rasterizer.rs`
  - [x] Add `SvgRenderReport`.
  - [x] Return `SvgRenderOutput { image, report }`.
  - [x] Preserve `rasterize()` and `rasterize_or_fallback()` wrappers.
  - [x] Attach known unsupported diagnostics to parsed nodes/attributes.
  - [x] Add first internal `SvgSceneItem` flattening boundary before raster
    drawing.
  - [ ] Add `SvgRenderOptions` once the scene split needs caller-controlled
    budgets beyond existing hard caps.
- `src/svg_import.rs`
  - [x] Extract shared color parsing and numeric-list parsing into
    `src/svg_core.rs`.
  - [x] Extract shared affine transform math and transform-list parsing into
    `src/svg_core.rs`.
  - [x] Extract shared SVG path tokenization into `src/svg_core.rs` while
    preserving separate importer bounds logic and rasterizer flattening logic.
  - [x] Extract remaining shared microsyntax parsing candidate: lengths.
- `tests/fixtures/svg_render/`
  - [x] Add one golden fixture each for rect/basic fill, path fill, stroke,
    transform, opacity, unsupported gradient, unsupported clip, unsafe external
    reference. Current harness stores these inline in `src/canvas/svg_golden.rs`
    as dependency-free ASCII signatures rather than PNG files.
- `scripts/validate-svg-import.ps1`
  - [x] Add renderer roadmap/golden test command.

### Near Tasks

- [x] Add `src/svg_core.rs` module to avoid importer/rasterizer drift.
- [x] Move shared color, numeric-list, affine-transform, and path-token parsing
  into `src/svg_core.rs`.
- [x] Add source-spanned `SvgNodeId` and reference-table metadata on top of the
  current `SvgScene` / `SvgSceneItem` / `DisplayList` split.
- [x] Replace direct XML-to-render traversal with owned display-list traversal.
- [x] Add explicit unsupported-feature enum instead of stringly diagnostics.
- [x] Implement and document deterministic 8x8 coverage with separate fill
  winding/parity and stroke-union modes.

### Later Tasks (all complete — R0–R8)

- [x] Implement gradients. (R3)
- [x] Implement clip stack. (R4: clipPath, overflow, premultiplied compositing,
  isolated group opacity)
- [x] Implement `<use>` in raster mode. (R2)
- [x] Implement text import phase 1 from `docs/TEXT_IMPORT_PLAN.md`. (R6)
- [x] Implement renderer report UI in RohKai. (R8)

The original R0–R8 backlog is closed. New work lives in
**Post-R8 Gap Analysis And Future Lanes** below (R8.1, R9–R12), with paste-ready
goal prompts in `docs/svg-goal-plan-prompts/`.

## Non-Goals For The Next Pass

- Full browser DOM.
- Scripting.
- Animation.
- External network/file loading.
- Complete CSS.
- Claiming equivalence to Batik/librsvg/resvg without a golden test corpus.

## Definition Of Done For Any SVG Renderer Feature

A feature is not done until all of these are true:

- Parser recognizes it.
- Scene/IR represents it.
- Renderer produces a visible output or a deliberate fallback.
- Diagnostics say whether it was rendered, approximated, skipped, or rejected.
- Tests cover success, malformed input, security boundaries, and deterministic
  output.
- Docs list the feature in supported or unsupported form.

## Post-R8 Gap Analysis And Future Lanes (2026-06-06)

R0-R8 are closed. This section is a gap analysis against W3C SVG 1.1 / SVG 2 /
SVG Native and the mature static renderers (resvg/usvg, librsvg, Batik, browser
static SVG) used **only as comparison/oracle targets, never as dependencies**.
Every proposed lane below is achievable with original, dependency-free, secure,
deterministic, bounded, golden-testable in-repo work.

Terminology: **intra-roadmap** = was planned in R0-R8 and shipped or explicitly
deferred; **extra-roadmap** = a real gap the roadmap never represented.

### Post-R8 Gap Matrix

| Capability | RohKai now | Mature engines | Priority | Gap type | Origin | Recommendation |
|---|---|---|---|---|---|---|
| Filter color-interpolation (linearRGB) | filters run in sRGB premultiplied | filters default `color-interpolation-filters: linearRGB` | P1 | conformance | extra-roadmap | implement (R10) |
| Filter region precision | region = whole canvas | bbox-based `-10%..110%` + x/y/w/h filterUnits | P1 | implementation/conformance | partially (R7 noted approx) | implement (R10) |
| Filters tier 2/3 | identity passthrough + diagnostic | feComposite/feBlend/feTile/feMorphology/feComponentTransfer/feImage/feTurbulence/feDisplacementMap | P2/P3 | implementation | intra-roadmap deferred | tier-2 implement (R10); tier-3 defer |
| Patterns | diagnosed transparent | full tiling (patternUnits/contentUnits/viewBox/transform) | P1/P2 | implementation | intra-roadmap deferred | implement (R9/R10) |
| Markers | none | marker-start/mid/end, orient, markerUnits, viewBox | P1 | implementation | extra-roadmap | implement (R9) |
| vector-effect=non-scaling-stroke | none | supported | P2 | implementation | extra-roadmap | implement (R9) |
| Raster text / textPath | import-only chunks; raster skips | full glyph layout + textPath | P1 | implementation | intra-roadmap deferred (R6 ph3) | implement editable-first + vector snapshot (R11) |
| Namespace model | prefixes stripped (`xlink:href`→`href`) | real xmlns/qualified-name model | P1 | architecture/conformance | extra-roadmap | implement bounded model (R12) |
| Malformed-document recovery | hard-reject on several constructs | lenient recovery + diagnostics | P1 | robustness | extra-roadmap | implement recovery policy (R12) |
| Accessibility metadata | dropped | `title`/`desc`/`aria-*`/`role` exposed | P2 | editor/diagnostics | extra-roadmap | implement title/desc extraction (R12) |
| Blend modes (mix-blend-mode) | normal over only | full isolation/blend | P2/P3 | implementation | extra-roadmap | tier-2 with feBlend (R10) |
| CSS combinators/@media/vars | tier-1 selectors only | descendant/child/attr/pseudo, @media, custom props | P2/P3 | implementation | intra-roadmap (only justified tiers) | implement only fixture-justified tiers |
| Color management (ICC) | sRGB assumed | ICC/`color-interpolation` | P4 | conformance | extra-roadmap | reject (out of scope) |
| Conformance corpus validation | per-feature goldens; no W3C subset | W3C test-suite + reference oracles | P1 | conformance/testing | partially (R8 stand-in) | implement curated subset (R8.1) |
| Fuzzing | none | fuzzed parsers/decoders | P0/P1 | testing/security | extra-roadmap | implement fuzz harness (R8.1) |
| Benchmark methodology | one ignored bench + smoke | documented parse/scene/raster/alloc budgets | P2 | performance/testing | partially (R8) | document + expand (R8.1) |
| Rendering precision policy | 8x8 coverage; nearest image/chroma | documented AA/gamma policy | P2 | docs/fidelity | extra-roadmap | document policy (R8.1) |
| Sub-byte/interlaced PNG, progressive JPEG | diagnosed | supported | P3 | implementation | intra-roadmap deferred | defer (diagnosed) |

### Proposed Future Lanes (all dependency-free, secure, deterministic, bounded)

- **R8.1 — Conformance & security hardening** (P0/P1; depends R0-R8):
  in-repo fuzz harness for the XML parser, path tokenizer, PNG/JPEG decoders, and
  DEFLATE inflater (random + mutated corpus, asserts no panic / bounded output);
  a curated W3C-1.1-subset golden corpus with an optional dev-only external-oracle
  diff (CI artifact); documented benchmark methodology + memory-cap tests; a
  written rendering-precision/AA policy. No runtime deps.
- **R9 — Markers, vector-effect, patterns** (P1/P2; depends R1 stroke + R3 paint +
  R4 IR): marker placement on path vertices (start/mid/end, orient incl.
  `auto`/`auto-start-reverse`, markerUnits, marker viewBox); `vector-effect:
  non-scaling-stroke`; pattern tiling via the offscreen pipeline (patternUnits/
  patternContentUnits/viewBox/patternTransform, bounded tile count).
- **R10 — Filter correctness & tier-2** (P1/P2; depends R7): linearRGB
  color-interpolation-filters; precise filter-region computation + clipping;
  feComposite, feBlend (+ mix-blend-mode), feComponentTransfer, feMorphology;
  bounded buffers; goldens per primitive.
- **R11 — Raster text & textPath** (P1; depends R6): editable-first stays; add an
  optional vector-outline snapshot render path (own glyph-outline extraction, no
  external font/shaping crate) and textPath layout; explicit diagnostics for
  unshaped/bidi cases. Heavy — gate on real product need.
- **R12 — XML/namespace & robustness** (P1; depends R0): real bounded namespace
  model (qualified names, xmlns scoping, foreign-namespace skip with diagnostics);
  malformed-document recovery policy (recover-and-diagnose instead of hard reject
  where safe); accessibility metadata (`title`/`desc`) extraction surfaced in the
  report panel and preserved on export.

### Recommended Additions To R8 (conformance/testing) — fold into R8.1

- Fuzz targets (parser/path/PNG/JPEG/inflate) with a checked-in seed corpus.
- Curated W3C SVG 1.1 sub-corpus as goldens; dev-only oracle diff as a CI artifact.
- Documented benchmark budgets (parse / scene-build / raster / peak alloc) and
  memory-cap regression tests.
- A rendering-precision policy doc (coverage grid, sampling, premultiplied/sRGB
  vs linearRGB boundaries).

### Explicit Non-Goals (remain rejected)

- Scripting, SMIL/CSS animation, DOM, event handling.
- External network/file loading; non-`data:` references stay fail-closed.
- Full browser CSS layout engine; complete selector/cascade parity.
- ICC color management beyond sRGB.
- `foreignObject` content rendering.
- Progressive/arithmetic/CMYK/12-bit JPEG; full HarfBuzz-class shaping + Unicode
  bidi (editable-first text remains the contract).
- Any external renderer dependency (resvg/usvg/tiny-skia/librsvg/Skia/Cairo/
  browser) — comparison/oracle only.

### Maturity Assessment

- **Importer-grade: achieved (exceeds).** Editable widgets + per-node provenance +
  structured diagnostics + source preservation.
- **Editor-grade: achieved.** In-app report UI, source viewer, rendered/source
  toggle, round-trip source preservation, honest fidelity scoring.
- **Application-grade: approaching.** Deterministic, bounded, diagnosed rendering
  of most static SVG (geometry, gradients, clips, masks, filter tier-1, PNG+JPEG
  images). Blockers to "achieved": markers, patterns, vector-effect (R9) and
  raster text (R11) — features common in real-world UI/diagram SVGs.
- **Renderer-grade (resvg/librsvg-class): not yet.** Requires R9-R12 plus R8.1:
  linearRGB filters + precise regions (R10), namespace model + malformed recovery
  (R12), and a W3C-subset conformance corpus + fuzzing (R8.1). Until then,
  "renderer-grade" is not claimed.

Next maturity step (application → renderer grade): **R8.1 (conformance/fuzz) →
R9 (markers/patterns/vector-effect) → R10 (filter correctness) → R11 (raster
text) → R12 (namespace/robustness/a11y)**, in that order.
