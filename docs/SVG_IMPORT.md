# SVG Import

RohKai imports SVGs as editable template placeholders. It is intentionally not
a full SVG renderer: the generated `.rktp` gives RohKai canvas geometry to work
with, while the original `.svg` is preserved beside it as the source of truth.

The parser is pure Rust and adds no crates.

Importer and rasterizer share `src/svg_core.rs` for dependency-free SVG
microsyntax helpers. The shared module currently owns color parsing and
numeric-list scanning, affine transform math and transform-list parsing, and
path data tokenization, so those behaviors do not drift between editable
placeholder import and image-mode rasterization.

## Public API

- `import_svg_template(svg, SvgImportOptions::default())` returns widgets plus an
  `SvgImportReport`.
- `parse_svg_template(svg)` remains a compatibility wrapper that returns only
  `Vec<WidgetInstance>`.

The report includes imported/skipped counts, warnings, unsupported features, and
an approximate fidelity level: `High`, `Medium`, or `Low`.

Fidelity is intentionally conservative. SVGs with masks, clips, filters,
paint-server references, many unsupported features, or text-heavy placeholder
imports are downgraded so RohKai does not imply pixel-perfect reproduction.

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
counts, warnings, unsupported feature diagnostics, and a conservative fidelity
level.

Renderer diagnostics for known unsupported elements and attributes are parsed
from SVG nodes/attributes rather than raw comment-sensitive source scans. This
reduces false positives and makes skipped counts more meaningful, though a full
scene/display-list IR is still planned before the report becomes a complete UI
surface.

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
- `width`, `height`, and `viewBox`.
- Units: `px`, `%`, `in`, `cm`, `mm`, `pt`, `pc`, `em`, `rem`.
- Transforms: `matrix`, `translate`, `scale`, `rotate`, `skewX`, `skewY`.
- Nested groups and transform stack.
- Local `symbol` / `use` expansion with cycle and depth protection.
- Minimal style resolution:
  - presentation attributes
  - inline `style=""`
  - simple `.class { key: value; }` rules from `<style>`
  - inherited visibility/display/opacity/font basics
- Solid paint approximation for simple `fill`, `stroke`, named colors,
  `#rgb`, `#rrggbb`, and `rgb(...)`.
- Opacity is approximated into solid RGB placeholder colors because RohKai
  widgets currently store RGB, not alpha.
- Image-mode rasterization supports solid fills/strokes, inherited
  display/visibility, viewBox mapping, transforms, basic shapes, and path
  flattening for the current supported subset.
- Image-mode rasterization now builds an internal scene item list before
  drawing. Scene items carry inherited style and accumulated transforms, so
  both group-level and shape-level `transform` attributes affect output.
- Image-mode rasterization emits structured diagnostics for known unsupported
  renderer buckets such as gradients, patterns, clips, masks, filters, markers,
  text, images, use/symbol, stroke dash/cap/join hints, fill-rule, and
  paint-server references.
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
  `http(s)`/`file` hrefs, excessive tag count, excessive path commands, or
  excessive raster dimensions.
- `filter`, animation, `foreignObject`, `textPath`, masks, clips, gradients,
  patterns, paint-server references, and complex CSS selectors are reported as
  unsupported or approximated with structured diagnostics.
- Clip/mask/filter attributes are diagnosed separately from their definitions.

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

Current behavior keeps text editable:

- Simple `<text>` becomes a RohKai `Label`.
- Simple `<tspan>` content is flattened into the label text.
- Positioned, adjusted, or styled `tspan` content is imported approximately with
  warnings.
- Text-heavy SVGs are not reported as high fidelity, even when imported
  successfully.

Future text work is planned in `docs/TEXT_IMPORT_PLAN.md`. The intended path is
robust `tspan` parsing and editable multi-label groups before any optional
vector-outline or owned shaping engine work.

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
cargo clippy -- -D warnings
```

## Known Unsupported Features

- Full SVG rendering
- Full XML DOM
- Full CSS cascade
- Filters
- Animations
- Scripting
- External resources
- Browser layout behavior
- Text shaping and font metrics
- Masks and clips
- Gradient/pattern conversion
- Full Image export parity for unsupported SVG features
- Full `tspan` positioning and per-span styling
- Text-on-path layout
