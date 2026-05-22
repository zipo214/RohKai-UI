# SVG Import

RohKai imports SVGs as editable template placeholders. It is intentionally not
a full SVG renderer: the generated `.rktp` gives RohKai canvas geometry to work
with, while the original `.svg` is preserved beside it as the source of truth.

The parser is pure Rust and adds no crates.

## Public API

- `import_svg_template(svg, SvgImportOptions::default())` returns widgets plus an
  `SvgImportReport`.
- `parse_svg_template(svg)` remains a compatibility wrapper that returns only
  `Vec<WidgetInstance>`.

The report includes imported/skipped counts, warnings, unsupported features, and
an approximate fidelity level: `High`, `Medium`, or `Low`.

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
- Text flattening for simple `tspan` content.
- Path bounds for `M`, `L`, `H`, `V`, `C`, `S`, `Q`, `T`, `A`, and `Z`.
- Relative and absolute path commands.
- Compact path syntax such as `M10-20L.5.6`.

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
- `filter`, animation, `foreignObject`, `textPath`, masks, clips, gradients, and
  patterns are reported as unsupported or approximated.

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

## Validation

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\validate-svg-import.ps1
```

That runs:

- `cargo fmt --check`
- SVG importer tests
- SVG source-preservation tests
- deterministic output tests
- `cargo clippy -- -D warnings`

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
