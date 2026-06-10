# SVG Import

RohKai imports SVGs as editable template placeholders. It is intentionally not
a full SVG renderer: the generated `.rktp` gives RohKai canvas geometry to work
with, while the original `.svg` is preserved beside it as the source of truth.

The parser is pure Rust and adds no crates.

Importer and rasterizer share `src/svg_core.rs` for dependency-free SVG
microsyntax helpers. The shared module currently owns color parsing and
numeric-list scanning, affine transform math and transform-list parsing, and
path data tokenization, SVG length/unit parsing and resolution, plus
`preserveAspectRatio`/viewBox-to-viewport mapping, so those behaviors do not
drift between editable placeholder import and image-mode rasterization.

## Public API

- `import_svg_template(svg, SvgImportOptions::default())` returns widgets plus an
  `SvgImportReport`.
- `parse_svg_template(svg)` remains a compatibility wrapper that returns only
  `Vec<WidgetInstance>`.

The report includes imported/skipped counts, warnings, unsupported features, and
an approximate fidelity level: `High`, `Medium`, or `Low`.

Fidelity is intentionally conservative. SVGs with masks, filters,
unavailable paint-server references, many unsupported features, or text-heavy
placeholder imports are downgraded so RohKai does not imply pixel-perfect
reproduction. (clipPath clipping renders as of R4 and is not a downgrade.)

## Import Modes

`SvgImportOptions::default()` uses component mode: supported SVG geometry and
text become editable RohKai widgets.

Image mode creates one `WidgetKind::Image` with the original SVG source stored
on the widget. This is still zero-new-dependency: the canvas preview and export
path use RohKai's own in-repo software rasterizer for the supported visual
subset. It is not full browser/SVG rasterization, and it does not add renderer
crates.

The rasterizer now has a richer diagnostic API:

- `rasterize_with_report(svg, width, height)` returns `SvgRenderOutput` with
  pixels and an `SvgRenderReport`.
- `rasterize(svg, width, height)` remains a compatibility wrapper returning
  only the `ColorImage`.
- `rasterize_or_fallback(svg, width, height)` remains the caller-friendly
  fallback path for canvas/export rendering.

The report records requested/output raster dimensions, rendered/skipped element
counts, warnings, unsupported feature diagnostics, a conservative fidelity
level, and stable node ID/byte-span provenance for node-level renderer
diagnostics.

Renderer diagnostics for known unsupported elements and attributes are parsed
from SVG nodes/attributes rather than raw comment-sensitive source scans. This
reduces false positives and makes skipped counts more meaningful. Raster
execution now consumes an owned display list containing lowered geometry,
resolved style/transform state, pending diagnostics, and source provenance; it
does not traverse XML nodes while writing pixels.

The generated standalone app embeds the same RohKai-owned rasterizer module when
Image widgets are present. This removes the older gray-frame placeholder export
path; exported Image widgets now load the preserved SVG source into egui
textures at runtime.

## Supported Placeholder Elements

- `rect`
- `circle`
- `ellipse`
- `line`
- `polyline`
- `polygon`
- `path`
- `image` as a placeholder only
- `use` for local `#id` references
- `text`

## Supported Parsing Subset

- XML-style tags and attributes.
- Quoted and unquoted attribute values.
- Safe built-in XML entities only: `amp`, `lt`, `gt`, `quot`, `apos`.
- `width`, `height`, `viewBox`, and full `preserveAspectRatio` parsing:
  `none`, all nine alignments, and `meet`/`slice`.
- Units: unitless/`px`, `%`, `in`, `cm`, `mm`, `Q`, `pt`, `pc`, `em`, `ex`,
  `rem`.
- Transforms: `matrix`, `translate`, `scale`, `rotate`, `skewX`, `skewY`.
- Nested groups and transform stack.
- Local `symbol` / `use` expansion with cycle and depth protection.
- Shared bounded style resolution:
  - presentation attributes
  - inline `style=""`
  - tier-1 element, class, ID, compound, and grouped rules from `<style>`
  - specificity and source-order precedence
  - inherited `currentColor`
  - inherited visibility/display/opacity/font basics
- Solid paint approximation for simple `fill`, `stroke`, named colors,
  `#rgb`, `#rrggbb`, and `rgb(...)`.
- Opacity is approximated into solid RGB placeholder colors because RohKai
  widgets currently store RGB, not alpha.
- Image-mode rasterization supports solid fills/strokes, inherited
  display/visibility, root and nested SVG viewport mapping, per-viewport
  percentage bases, transforms, basic shapes, path flattening, and inherited
  `fill-rule` values (`nonzero` by default and explicit `evenodd`) for the
  current supported subset. Nested `<svg>` viewport overflow is clipped to the
  viewport rect (R4).
- Image-mode fills and strokes use deterministic 8x8 subpixel coverage. Fill
  samples preserve nonzero/evenodd winding semantics; stroke samples union all
  tessellated pieces before compositing so translucent joins do not darken.
- Image-mode stroke geometry supports local-space width under affine
  transforms, butt/round/square caps, miter/miter-clip/round/bevel joins,
  miter limits, dash arrays and signed offsets, closed seams, zero-length
  round/square caps, and positive `pathLength` dash calibration. `arcs` joins
  are diagnosed and approximated with bounded miter-clip geometry.
- Image-mode rasterization assigns stable preorder node IDs and byte spans,
  builds a bounded first-id-wins local reference table, then lowers scene items
  into an owned display list before drawing. Display commands carry inherited
  style, accumulated transforms, resolved shape/path geometry, diagnostics, and
  source provenance.
- Image-mode rasterization expands bounded local `defs`/`symbol`/`use`
  references with cycle/depth/node guards and duplicate-ID diagnostics.
- Image-mode rasterization renders linear/radial gradient fills and strokes,
  including stop opacity, CSS/currentColor stops, both gradient units,
  gradient transforms, pad/reflect/repeat spread, bounded href inheritance,
  deterministic interpolation, and malformed-value diagnostics.
- Image-mode rasterization applies `clipPath` clipping (R4): `clip-rule`
  nonzero/evenodd, transformed clipPath children and nested `<g>`, clipPathUnits
  `userSpaceOnUse` and `objectBoundingBox` (against a shape's bounding box), and
  clipPath-of-clipPath intersection. Clip references reuse the shared first-id-wins
  reference table with cycle/depth bounds. Missing references, cycles, depth
  overflow, and objectBoundingBox-on-group (no single bbox) are diagnosed
  (`clip.unresolved`, `reference.clip_cycle`, `limit.clip_depth`,
  `clip.object_bounding_box`) and skipped rather than approximated.
- Image-mode rasterization composites `<g opacity>` / `isolation:isolate`
  groups through a premultiplied-alpha offscreen and composites once at group
  opacity, so overlapping children do not double-darken. Offscreen buffers are
  bounded by depth/byte caps with a `limit.offscreen_buffer` diagnostic on
  truncation. The internal compositing buffer is premultiplied; the emitted
  `ColorImage` remains straight RGBA and non-grouped output is byte-stable.
- Image-mode rasterization decodes inline base64 `data:` PNG and baseline JPEG
  images (R5) with zero-dependency decoders: PNG via a from-scratch zlib/DEFLATE
  inflater + scanline unfilter (color types 0/2/3/4/6 at 8/16-bit); JPEG via
  baseline/extended-sequential Huffman decode (marker parse, dequantization, 8×8
  IDCT, YCbCr→RGB, 4:4:4/4:2:2/4:2:0 chroma upsampling, restart markers). Both
  draw through the R4 clip/compositing pipeline with `preserveAspectRatio`
  placement, element opacity, and `clip-path`. Decode memory/CPU is bounded by
  pixel and inflate/output caps. External image references are rejected
  fail-closed; unsupported or malformed inputs are diagnosed
  (`image.unsupported_format`, `image.unsupported_png`, `image.unsupported_jpeg`,
  `image.decode_failed`, `limit.image_pixels`) — including progressive/arithmetic/
  CMYK/12-bit JPEG, which are explicit unsupported cases. Component import still
  keeps `<image>` as an editable placeholder with the source preserved.
- Image-mode rasterization renders `mask` (alpha + luminance) and filter tier-1
  (`feGaussianBlur`, `feOffset`, `feFlood`, `feMerge`, `feColorMatrix`,
  `feDropShadow`) through the R4 offscreen pipeline (R7), plus filter tier-2
  (`feComposite`, `feBlend`, `feComponentTransfer`, `feMorphology`), linearRGB
  `color-interpolation-filters` (default; `sRGB` opts out), precise filter-region
  clipping (`filterUnits` + `x/y/width/height`), and `mix-blend-mode` group
  blending (R10). Tier-3 primitives (turbulence/displacement/convolution/
  lighting/tile/image) pass through with a `filter.unsupported_primitive`
  partial-output diagnostic; missing mask/filter refs warn and leave the element
  rendered.
- Image-mode rasterization renders **markers** (`marker-start`/`marker-mid`/
  `marker-end` + `marker` shorthand, `orient` `auto`/`auto-start-reverse`/angle,
  `markerUnits`, `viewBox`/`refX`/`refY` with overflow clip), **patterns** (tiled
  via the offscreen pipeline — `patternUnits`/`patternContentUnits`/`viewBox`/
  `patternTransform`/`href`), and `vector-effect: non-scaling-stroke` (R9). Tile
  pixels and marker placements are bounded; cyclic/missing references diagnose
  (`reference.pattern_cycle`, `marker.unresolved`, `limit.marker_count`) and
  never panic.
- Image-mode rasterization renders **text** (`<text>`/`<tspan>`/`<textPath>`)
  as a vector-outline snapshot via a bundled public-domain Hershey simplex
  stroked font (R11): ASCII coverage, inherited font-size, whole-run
  `text-anchor`, x/y/dx/dy tspan runs, and arc-length `textPath` placement.
  Every rendered text carries an honest `text.raster_snapshot` warning
  (font-family substituted, approximate metrics); non-ASCII renders tofu boxes
  with `text.glyph_unsupported`; bidi/combining marks are diagnosed
  (`text.bidi_unsupported` / `text.shaping_unsupported`), never silently wrong.
  Glyph count is bounded (`limit.text_glyphs`).
- Image-mode rasterization tracks a bounded **xmlns namespace model** (R12):
  qualified names resolve to svg/xlink/foreign within an xmlns scope stack;
  foreign-namespace elements are skipped + diagnosed (`namespace.foreign_element`)
  rather than mis-parsed (so `<custom:rect>` is not drawn as a `<rect>`), while
  `xlink:href` still resolves. **Malformed markup** (mismatched/unclosed tags,
  stray junk) recovers to partial output + `recovery.malformed_markup` +
  `recovered_error_count` instead of a whole-document failure (the security gates
  stay hard-fail). **Accessibility metadata** `<title>`/`<desc>` (+ root
  `aria-label` fallback) is extracted (bounded length), surfaced on the render
  report + report panel (Title/Description rows), and preserved on export.
- Image-mode rasterization emits structured diagnostics for the remaining
  unsupported renderer buckets such as tier-3 filter primitives and
  unavailable paint-server references.
  Invalid fill-rule/stroke declarations produce source-spanned warnings and
  preserve inherited/default behavior. Stroke complexity limits report
  truncation explicitly.
- Text flattening for simple `tspan` content.
- Path bounds for `M`, `L`, `H`, `V`, `C`, `S`, `Q`, `T`, `A`, and `Z`.
- Relative and absolute path commands.
- Compact path syntax such as `M10-20L.5.6`.
- Shared path tokenization supports compact signs, adjacent decimals,
  exponents, and unknown command letters without panics. Importer and
  rasterizer keep separate semantics after tokenization: importer computes
  bounds/provenance and structured warnings, while rasterizer flattens paths
  into pixels.

## Limits

Defaults are deliberately conservative:

- max file bytes: `5_000_000`
- max tag count: `10_000`
- max attributes per tag: `64`
- max attribute value length: `65_536`
- max nesting depth: `64`
- max path command count: `20_000`
- max generated placeholder count: `2_000`
- max image data URI bytes: `1_000_000`
- max use expansion depth: `32`
- max style bytes: `262_144`

Limit failures return structured `SvgImportError` values and never panic.

## Security Policy

RohKai rejects or ignores unsafe SVG features:

- `DOCTYPE` is rejected.
- Custom/external entities are rejected.
- Unknown entities are left literal with a warning.
- Non-XML processing instructions are ignored with a warning.
- `script` is not executed.
- External network/file references are rejected for `use`, `image`, and related refs.
- Image-mode rasterization also refuses unsafe raw `svg_source` containing
  `DOCTYPE`, custom entities, scripts, non-XML processing instructions, external
  hrefs or non-local `url(...)` references, excessive tag count, excessive path
  commands, or excessive raster dimensions.
- Animation, `foreignObject`, tier-3 filter primitives,
  unavailable paint-server references, and complex CSS selectors are reported
  as unsupported or approximated with structured diagnostics. Linear/radial
  gradients, `clipPath` clipping, masks (alpha/luminance), filter tier-1 + tier-2
  (linearRGB, precise regions, `mix-blend-mode`) (R7/R10), markers, patterns,
  `vector-effect: non-scaling-stroke` (R9), and raster text + `textPath` via the
  bundled vector font (R11) are supported in Image-mode rendering
  but remain non-editable, diagnosed placeholders in component import mode.
  `clip-path` is applied in Image-mode rendering (R4); unresolved/cyclic/too-deep
  clip references and objectBoundingBox-on-group are diagnosed and skipped.

## Image Policy

Only embedded data URIs are accepted:

- `data:image/png;base64,...`
- `data:image/jpeg;base64,...`

External URLs, local file paths, unsupported MIME types, malformed data, invalid
base64 characters, and oversized data URIs are skipped with diagnostics. Images
still become placeholder frames because RohKai does not yet have a dedicated
image widget.

## Arc And Curve Bounds

Bezier curves are sampled deterministically for bounds. Elliptical arcs use SVG
endpoint-to-center conversion and deterministic sampling with a documented
`0.5px` tolerance, capped at 128 samples per arc.

## Text Policy

Current behavior keeps text editable (TEXT_IMPORT_PLAN phases 1-2):

- `<text>` is split into chunks at every absolutely-positioned (`x`/`y`) `<tspan>`;
  each non-empty chunk becomes its own editable RohKai `Label`.
- Sibling chunks share a `text_group` provenance id; a single-chunk `<text>` stays
  one ungrouped label.
- Relative/styled `<tspan>` content flattens into the surrounding chunk with
  explicit `text.tspan_adjust` / `text.tspan_style` diagnostics.
- `text-anchor` (start/middle/end) and `dominant-baseline` are applied per chunk;
  unsupported baselines are approximated with a `text.baseline` diagnostic.
- Image-mode rendering draws text via the bundled Hershey simplex vector font
  with `textPath` support (R11, TEXT_IMPORT_PLAN phase 3); the editable import
  above stays the component-import default. Bidi and shaping remain deferred
  (diagnosed tofu); text-heavy SVGs are still not reported as high fidelity
  (the `text.raster_snapshot` approximation warning keeps fidelity honest).

Remaining text work is in `docs/TEXT_IMPORT_PLAN.md` (phase 4): an owned
shaping engine only if the product proves it needs one.

## Validation

Run:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1
```

That runs:

- `cargo fmt --check`
- SVG dependency policy checks
- text encoding policy checks
- SVG importer tests
- SVG rasterizer tests
- SVG golden renderer fixtures
- stroke cap/join/dash/pathLength and transform-bound tests
- self-intersecting evenodd and non-darkening translucent-stroke tests
- an ignored coarse 512px anti-aliased fill performance smoke
- shared SVG core microsyntax tests
- SVG source-preservation tests
- deterministic output tests
- real-world-style fixture tests from `tests/fixtures/svg_import/real_world/`
- `cargo clippy -- -D warnings`

The fixture suite uses small checked-in SVGs that cover common real-world
import buckets: basic geometry, class styles, `tspan` flattening, paint servers,
clips/masks/filters, local `symbol`/`use`, external references, malformed
recovery, and embedded image placeholders. The tests assert deterministic
placeholder IDs and deterministic diagnostics as well as minimum import results.

Full project verification is still:

```powershell
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Known Unsupported Features

- Full SVG specification rendering
- Full XML DOM
- Full CSS cascade
- Filters
- Animations
- Scripting
- External resources
- Browser layout behavior
- Text shaping and font metrics
- Masks (clips are supported as of R4)
- objectBoundingBox clip units on a `<g>` (no single bounding box)
- Editable gradient/pattern conversion during component import (Image-mode
  pattern *rasterization* ships in R9; editable conversion is still future work)
- Full Image export parity for unsupported SVG features
- Full `tspan` positioning and per-span styling
- Text-on-path layout in *component import* mode (Image-mode `textPath`
  rendering ships in R11 via the bundled vector font)
