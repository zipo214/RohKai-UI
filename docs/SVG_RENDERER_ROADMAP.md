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

1. Complete R1 geometry quality now that the R0 IR boundary is closed.
2. Complete R2 style/reference resolution, including raster `<use>`.
3. Continue R3-R8 in order, with R6 owning all `tspan`/text-plan execution and
   R8 owning source/report UI.

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
  presentation attributes, inline `style=""`, simple `.class { key: value; }`,
  inherited display/visibility/opacity/font basics.
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
- ViewBox-to-pixel mapping.
- XML-ish parser for common SVG files.
- Style inheritance:
  solid `fill`, `stroke`, `stroke-width`, opacity, display/visibility, inline
  `style=""`.
- Closed scene/display-list boundary:
  parsed nodes receive stable preorder `SvgNodeId` values and exact byte spans;
  a bounded first-id-wins local reference table records resolved/unresolved
  fragment uses; scene items accumulate transforms and resolved inherited
  style; the owned display list lowers lengths, shape geometry, path geometry,
  diagnostics, and provenance before raster execution.
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
  even-odd scanline polygon fill, stroke-as-quad expansion, basic alpha
  compositing.
- Cache integration:
  canvas previews use texture caching and bounded raster dimensions.

Current limits:

- No real XML namespace model.
- No full DOM/defs expansion for raster output. R0 now has bounded local-id and
  reference metadata, but R2 still owns actual reference expansion.
- No `<use>` or `<symbol>` expansion in raster mode.
- No `<image>` decoding.
- No gradients, patterns, clips, masks, filters, markers, blend modes, or
  compositing groups.
- No text rendering.
- No full CSS cascade/specificity/media/import/pseudo/class/tag selector model.
- No anti-aliased area coverage, gamma-correct blending, stroke joins/caps,
  miter limits, dash arrays, or vector effects.
- No golden-image conformance suite against reference renderers.

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
| Render IR | Scene/display-list separate from XML | Stable source-spanned node IDs, bounded local references, flattened scene items, and an owned geometry/diagnostic display list | Add R2 reference expansion and later reusable paint/compositing IR | P0/P1 |
| Diagnostics | Structured per-node render/import diagnostics | Importer rich; rasterizer reports warnings, unsupported buckets, counts, fidelity, and node ID/byte-span provenance | Add R8 report/source UI | P1 |
| ViewBox/preserveAspectRatio | Full modes and nested viewport behavior | Basic meet-style mapping | Implement full `preserveAspectRatio`, nested SVG | P1 |
| CSS cascade | Specificity/order/inheritance subset | Importer simple classes; rasterizer inline/presentation | Shared style engine with selector tiers | P1 |
| Basic shapes | Full geometry and transforms | Mostly supported | Add exact bounds and tests | P0 |
| Paths | Full grammar, fill rules, stroke geometry | Commands supported; even-odd fill only; simple stroke quads | Nonzero fill, joins/caps/dashes/miter | P0/P1 |
| Gradients | Linear/radial with units, transforms, spread, href | Unsupported/diagnosed | Implement paint server model | P2 |
| Patterns | Tiled nested content | Unsupported/diagnosed | Implement after scene IR | P3 |
| Images | Embedded PNG/JPEG and secure external policy | Import placeholder only; raster no decode | Add zero-dep PNG/JPEG decision or explicit non-support | P2 |
| `defs`/`use` | Id resolution and expansion | Importer supports local use; raster does not | Shared reference resolver | P1 |
| Clips | Actual clipping stack | Diagnostics only | Clip masks in raster pipeline | P2 |
| Masks | Alpha/luminance masks | Diagnostics only | Offscreen mask buffers | P3 |
| Filters | Primitive graph | Diagnostics only | Start with drop shadow/blur/offset only if worth it | P4 |
| Text | Full layout/shaping | Import flatten; raster skips | Implement text plan in phases | P3/P4 |
| Markers | Arrowheads and symbols on paths | Unsupported | Add after stroke geometry | P3 |
| Antialiasing | High-quality coverage | Hard scanline/quad edges | Area coverage or supersampling | P1 |
| Compositing | Correct premultiplied alpha/groups | Basic over blend | Premultiplied pipeline + group opacity | P1 |
| Tests | Golden corpus + fuzz + oracles | Unit/fixture tests, not full golden | Add golden/reference harness | P0 |

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
  accumulated transforms. Source spans, reference table, stable node ids, and
  richer diagnostic provenance remain follow-up work.
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

- Implement full `preserveAspectRatio`.
- Add nonzero fill rule plus `fill-rule` support.
- Replace stroke-as-quad with stroke tessellation:
  caps, joins, miter limit, dasharray, dashoffset.
- Add anti-aliased fill/stroke coverage:
  either deterministic supersampling or analytic edge coverage.
- Add exact path/shape bounds where feasible.
- Add transform edge-case tests: rotation, skew, nested viewports, negative
  viewBox, tiny/huge coordinates.

Acceptance:

- Basic icons and diagrams render without jagged obvious failures.
- Stroke-heavy fixtures match golden outputs.
- Pathological paths remain bounded.

### Phase R2 — Shared Style And References

Goal: remove duplicated importer/rasterizer understanding.

Tasks:

- Extract shared SVG microsyntax modules:
  numbers, lengths, colors, transforms, paths, style declarations.
- Implement selector tier 1:
  presentation attrs, inline style, element selectors, class selectors,
  id selectors, grouped selectors, specificity/order.
- Implement reference resolver:
  `defs`, `symbol`, `use`, local paint refs, cycle/depth limits.
- Add currentColor and basic inherited color behavior.
- Add duplicate-id diagnostics.

Acceptance:

- Importer and rasterizer agree on colors/transforms/paths.
- `<use>` renders in image mode and imports in component mode.
- Complex CSS stays diagnosed instead of silently ignored.

### Phase R3 — Paint Servers

Goal: cover the biggest visual gap in real SVG art.

Tasks:

- Add paint server IR:
  solid, linear gradient, radial gradient, pattern placeholder.
- Linear gradients:
  stops, opacity, units, transform, spread methods, href inheritance.
- Radial gradients:
  center/focal/radius, units, transform, spread methods, href inheritance.
- Gradient sampling with deterministic interpolation.
- Pattern diagnostics upgraded:
  report exact unsupported pattern attributes until real pattern rendering lands.

Acceptance:

- Gradient-heavy fixtures no longer fall to flat color.
- Fidelity scoring distinguishes fully rendered gradients from approximated
  patterns.

### Phase R4 — Clipping, Viewport Overflow, And Group Compositing

Goal: make exported design-tool SVGs stop leaking outside their intended masks.

Tasks:

- Add clip stack.
- Implement `clipPath` with transforms and clip-rule.
- Implement nested viewport overflow clipping.
- Add premultiplied-alpha internal buffer.
- Add group opacity and isolated offscreen compositing.

Acceptance:

- Clip fixtures render visually clipped, not just diagnosed.
- Alpha edges remain deterministic and not haloed.

### Phase R5 — Embedded Raster Images

Goal: decide and implement real image policy without dependency creep.

Tasks:

- Decide whether to implement zero-dependency PNG and JPEG decoders in RohKai or
  keep image decode explicitly unsupported.
- If implemented:
  parse PNG chunks, zlib/deflate, filters, color types, alpha, interlace policy;
  parse JPEG baseline DCT, Huffman, quantization, YCbCr conversion, EXIF policy.
- If not implemented:
  keep placeholder behavior but make diagnostics/UI clearer.

Acceptance:

- No "image supported" claim unless pixels really render.
- Security caps cover image decode memory and CPU.

### Phase R6 — Text Import And Optional Rendering

Goal: keep text editable first, then add fidelity modes.

Tasks:

- Execute `docs/TEXT_IMPORT_PLAN.md` phase 1:
  robust `tspan` runs, chunks, provenance, anchors, baselines diagnostics.
- Add multi-label grouped import for positioned spans.
- Add optional vector-outline snapshot mode only after source preservation and
  editable text remain intact.
- Defer owned shaping engine until the product proves it needs one.

Acceptance:

- Text-heavy SVGs no longer collapse into misleading single labels without
  detailed warnings.
- Users can choose editable approximation vs visual snapshot mode.

### Phase R7 — Masks And Filters

Goal: add expensive visual effects only after core geometry/paint is trustworthy.

Tasks:

- Masks:
  maskUnits, maskContentUnits, alpha/luminance mode, offscreen buffers.
- Filters tier 1:
  drop shadow, blur, offset, flood, merge, color matrix.
- Filters tier 2:
  blend/composite/component transfer/morphology.
- Filters tier 3:
  turbulence, displacement, convolution, lighting if still justified.

Acceptance:

- Filter region limits prevent memory bombs.
- Unsupported primitives show partial-output diagnostics.

### Phase R8 — Conformance, Benchmarks, And Editor UX

Goal: make the renderer dependable, measurable, and visible in RohKai.

Tasks:

- Add reference comparison harness:
  browser/librsvg/resvg may be optional developer-only or CI-artifact tools, not
  runtime dependencies.
- Add golden-image corpus:
  geometry, paint, text, clips, masks, filters, malicious inputs.
- Add renderer benchmark suite:
  parse time, scene build time, raster time, peak allocations.
- Add SVG report UI:
  fidelity, warnings, unsupported features, source node ids.
- Add source viewer and "rendered vs editable approximation" toggle.

Acceptance:

- Every supported feature has visual tests.
- Roadmap claims match tests and diagnostics.

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
- Add antialiasing strategy doc and prototype.

### Later Tasks

- Implement gradients.
- Implement clip stack.
- Implement `<use>` in raster mode.
- Implement text import phase 1 from `docs/TEXT_IMPORT_PLAN.md`.
- Implement renderer report UI in RohKai.

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
