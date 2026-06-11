// Pure-Rust software SVG rasterizer.
//
// Covers: rect (rx/ry), circle, ellipse, line, polyline, polygon, path (all
// standard commands), and <g> groups with transforms + style inheritance.
// Outputs egui::ColorImage (straight RGBA). Zero new Cargo dependencies.
//
// Text elements are currently diagnosed rather than shaped. Linear/radial
// gradients and bounded local <use>/<symbol> references are rendered; patterns,
// filters, masks, and clips remain explicit unsupported diagnostics.

use crate::svg_core::{self, Rgba};
use egui::ColorImage;
use std::collections::HashMap;

const MAX_SVG_BYTES: usize = 5_000_000;
const MAX_TAGS: usize = 10_000;
const MAX_PATH_TOKENS: usize = 20_000;
const MAX_LOCAL_IDS: usize = 4_096;
const MAX_LOCAL_REFERENCE_USES: usize = 8_192;
const MAX_USE_EXPANSION_DEPTH: usize = 32;
const MAX_EXPANDED_USE_NODES: usize = 20_000;
const MAX_STYLE_BYTES: usize = 262_144;
const MAX_CSS_RULES: usize = 4_096;
const MAX_CSS_DECLARATIONS: usize = 16_384;
/// Maximum flattened points in a single sub-path.  A 20 000-cubic-command
/// path at ~40 pts/command would otherwise allocate ~800 k (f32,f32) pairs.
const MAX_FLAT_PTS: usize = 50_000;
const MAX_RASTER_DIM: u32 = 4096;
const MAX_RASTER_PIXELS: usize = 16_777_216;
const COVERAGE_GRID: usize = 8;
const COVERAGE_SAMPLES: u32 = (COVERAGE_GRID * COVERAGE_GRID) as u32;
const COVERAGE_TILE_ROWS: usize = 64;
const MAX_DASH_ENTRIES: usize = 256;
const MAX_DASH_RUNS: usize = 50_000;
const MAX_STROKE_PRIMITIVES: usize = 100_000;
const MAX_STROKE_VERTICES: usize = 200_000;
/// Maximum nested clip-path reference / clipPath-of-clipPath chain depth.
const MAX_CLIP_DEPTH: usize = 16;
/// Maximum fillable shapes lowered from a single clipPath subtree.
const MAX_CLIP_SHAPES: usize = 4_096;
/// Maximum total bytes of simultaneously live isolated-group offscreen buffers.
const MAX_OFFSCREEN_BYTES: usize = 134_217_728; // 128 MiB
/// Maximum simultaneously live isolated-group offscreen buffers (nesting depth).
const MAX_OFFSCREEN_DEPTH: usize = 8;
/// Maximum shapes lowered from a single `<mask>` subtree (R7).
const MAX_MASK_ITEMS: usize = 4_096;
/// Maximum primitives executed from a single `<filter>` (R7).
const MAX_FILTER_PRIMITIVES: usize = 64;
/// Maximum Gaussian-blur box radius in device pixels (R7), to bound filter CPU.
const MAX_BLUR_RADIUS: usize = 200;
/// Maximum `feMorphology` radius in device pixels (R10); the window is O(r^2).
const MAX_MORPH_RADIUS: usize = 100;
/// Maximum marker placements (start/mid/end vertices) drawn per stroked shape (R9).
const MAX_MARKER_PLACEMENTS: usize = 10_000;
/// Maximum shapes lowered from a single `<marker>` subtree (R9).
const MAX_MARKER_CONTENT_ITEMS: usize = 1_024;
/// Maximum shapes lowered from a single `<pattern>` subtree (R9).
const MAX_PATTERN_CONTENT_ITEMS: usize = 4_096;
/// Maximum pixels in one rendered pattern tile buffer (R9), to bound tile memory.
const MAX_PATTERN_TILE_PIXELS: usize = 1_048_576; // 1024 x 1024
/// Maximum `<pattern>` href-inheritance chain depth (R9).
const MAX_PATTERN_REFERENCE_DEPTH: usize = 32;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Reasons a rasterization attempt can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgRasterError {
    /// SVG source exceeded size limits or contained forbidden content.
    ForbiddenContent,
    /// The SVG XML could not be parsed into a renderable document.
    ParseFailed,
}

/// Render result with diagnostics. Prefer this when callers need to explain
/// partial SVG fidelity to users.
#[allow(dead_code)]
pub struct SvgRenderOutput {
    pub image: ColorImage,
    pub report: SvgRenderReport,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SvgRenderReport {
    pub requested_width: u32,
    pub requested_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub rendered_element_count: usize,
    pub skipped_element_count: usize,
    pub warning_count: usize,
    pub unsupported_feature_count: usize,
    pub warnings: Vec<SvgRenderWarning>,
    pub unsupported_features: Vec<SvgRenderUnsupportedFeature>,
    pub fidelity: SvgRenderFidelity,
    /// R12 accessibility metadata: `<title>` text (bounded length), if any.
    pub title: Option<String>,
    /// R12 accessibility metadata: `<desc>` text (bounded length), if any.
    pub desc: Option<String>,
    /// R12: count of malformed constructs the parser recovered from
    /// (mismatched/unclosed tags, stray junk) rather than hard-failing.
    pub recovered_error_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgRenderFidelity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgRenderWarning {
    pub code: String,
    pub message: String,
    pub source: Option<SvgRenderSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgRenderUnsupportedFeature {
    pub feature: String,
    pub message: String,
    pub source: Option<SvgRenderSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgRenderSource {
    pub node_id: u32,
    pub byte_start: usize,
    pub byte_end: usize,
}

impl SvgRenderReport {
    fn new(
        requested_width: u32,
        requested_height: u32,
        output_width: u32,
        output_height: u32,
    ) -> Self {
        Self {
            requested_width,
            requested_height,
            output_width,
            output_height,
            rendered_element_count: 0,
            skipped_element_count: 0,
            warning_count: 0,
            unsupported_feature_count: 0,
            warnings: Vec::new(),
            unsupported_features: Vec::new(),
            fidelity: SvgRenderFidelity::High,
            title: None,
            desc: None,
            recovered_error_count: 0,
        }
    }

    fn warning(&mut self, code: impl Into<String>, message: impl Into<String>) {
        self.warning_at(code, message, None);
    }

    fn warning_at(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        source: Option<SvgRenderSource>,
    ) {
        self.warnings.push(SvgRenderWarning {
            code: code.into(),
            message: message.into(),
            source,
        });
    }

    fn unsupported_at(
        &mut self,
        feature: impl Into<String>,
        message: impl Into<String>,
        source: Option<SvgRenderSource>,
    ) {
        self.unsupported_features.push(SvgRenderUnsupportedFeature {
            feature: feature.into(),
            message: message.into(),
            source,
        });
    }

    fn rendered(&mut self) {
        self.rendered_element_count += 1;
    }

    fn skipped(&mut self) {
        self.skipped_element_count += 1;
    }

    fn finalize(&mut self) {
        self.warning_count = self.warnings.len();
        self.unsupported_feature_count = self.unsupported_features.len();
        let severe_unsupported = self.unsupported_feature_count > 3
            || self
                .unsupported_features
                .iter()
                .any(|u| matches!(u.feature.as_str(), "clipPath" | "mask" | "filter"));
        self.fidelity = if self.rendered_element_count == 0 || severe_unsupported {
            SvgRenderFidelity::Low
        } else if self.warning_count > 0 || self.unsupported_feature_count > 0 {
            SvgRenderFidelity::Medium
        } else {
            SvgRenderFidelity::High
        };
    }
}

impl std::fmt::Display for SvgRasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForbiddenContent => {
                write!(f, "SVG contains forbidden content or exceeds size limits")
            }
            Self::ParseFailed => write!(f, "SVG could not be parsed"),
        }
    }
}

/// Rasterize an SVG string and return pixels plus structured diagnostics.
pub fn rasterize_with_report(
    svg_text: &str,
    width: u32,
    height: u32,
) -> Result<SvgRenderOutput, SvgRasterError> {
    let (w, h) = raster_size(width, height);
    let mut report = SvgRenderReport::new(width, height, w as u32, h as u32);
    let mut buf = vec![0u8; w * h * 4]; // transparent black
    if w as u32 != width || h as u32 != height {
        report.warning(
            "limit.raster_size",
            "requested raster size was clamped to renderer safety limits",
        );
    }

    if !svg_text_allowed(svg_text) {
        return Err(SvgRasterError::ForbiddenContent);
    }

    let scene = SvgScene::parse(svg_text).ok_or(SvgRasterError::ParseFailed)?;
    if scene.references.external_reference_count > 0 {
        return Err(SvgRasterError::ForbiddenContent);
    }
    scene.references.report_into(&mut report);
    report_stylesheet(&scene.stylesheet, &mut report);
    scene.paint_servers.report_into(&mut report);
    if scene.expanded_use_limit_hit {
        report.warning(
            "limit.use_expansion",
            "local use expansion exceeded renderer depth or node limits",
        );
    }
    if scene.use_cycle_count > 0 {
        report.warning(
            "reference.use_cycle",
            format!(
                "{} cyclic local use reference(s) were skipped",
                scene.use_cycle_count
            ),
        );
    }
    // R12: accessibility metadata + namespace/recovery diagnostics.
    report.title = scene.title.clone();
    report.desc = scene.desc.clone();
    report.recovered_error_count = scene.recovered;
    if scene.foreign_count > 0 {
        report.warning(
            "namespace.foreign_element",
            format!(
                "{} foreign-namespace element(s) were skipped (not in the SVG namespace)",
                scene.foreign_count
            ),
        );
    }
    if scene.recovered > 0 {
        report.warning(
            "recovery.malformed_markup",
            format!(
                "recovered from {} malformed-markup construct(s) (mismatched/unclosed tags); output is partial",
                scene.recovered
            ),
        );
    }

    let vb_xform = viewbox_to_pixel_transform(&scene, w, h);
    // Scene graph → display list IR (build phase: classify + resolve transforms),
    // then execute the flat command stream (raster phase).
    let display_list = DisplayList::build(&scene, &vb_xform);
    display_list.execute(&mut buf, w, h, &mut report);
    report.finalize();

    Ok(SvgRenderOutput {
        image: ColorImage::from_rgba_unmultiplied([w, h], &buf),
        report,
    })
}

/// Rasterize an SVG string to a pixel buffer of the given dimensions.
///
/// Returns `Err` if the SVG fails security checks or cannot be parsed.
/// On success the returned `ColorImage` is straight RGBA.
pub fn rasterize(svg_text: &str, width: u32, height: u32) -> Result<ColorImage, SvgRasterError> {
    rasterize_with_report(svg_text, width, height).map(|output| output.image)
}

/// Rasterize an SVG string, returning a grey fallback image on any error.
///
/// Callers that just need pixels and don't need to distinguish failure
/// reasons should prefer this over calling `rasterize` directly.
pub fn rasterize_or_fallback(svg_text: &str, width: u32, height: u32) -> ColorImage {
    let (w, h) = raster_size(width, height);
    rasterize(svg_text, width, height).unwrap_or_else(|_| fallback_image(w, h))
}

fn raster_size(width: u32, height: u32) -> (usize, usize) {
    let mut w = width.clamp(1, MAX_RASTER_DIM) as usize;
    let mut h = height.clamp(1, MAX_RASTER_DIM) as usize;
    let pixels = w.saturating_mul(h);
    if pixels > MAX_RASTER_PIXELS {
        let scale = (MAX_RASTER_PIXELS as f64 / pixels as f64).sqrt();
        w = ((w as f64 * scale).floor() as usize).max(1);
        h = ((h as f64 * scale).floor() as usize).max(1);
    }
    (w, h)
}

fn svg_text_allowed(svg_text: &str) -> bool {
    if svg_text.len() > MAX_SVG_BYTES {
        return false;
    }
    if svg_text.matches('<').count() > MAX_TAGS {
        return false;
    }

    let lower = svg_text.to_ascii_lowercase();
    if lower.contains("<!doctype") || lower.contains("<!entity") || lower.contains("<script") {
        return false;
    }
    if lower.contains("href=\"http:")
        || lower.contains("href='http:")
        || lower.contains("href=\"https:")
        || lower.contains("href='https:")
        || lower.contains("href=\"file:")
        || lower.contains("href='file:")
    {
        return false;
    }

    let mut search = lower.as_str();
    while let Some(idx) = search.find("<?") {
        let rest = &search[idx..];
        if !rest.starts_with("<?xml") {
            return false;
        }
        search = &rest[2..];
    }

    true
}

#[derive(Clone)]
enum PendingDiagnostic {
    Warning {
        code: &'static str,
        message: &'static str,
    },
}

fn unsupported_attr_diagnostics(attrs: &[(String, String)]) -> Vec<PendingDiagnostic> {
    let mut diagnostics = Vec::new();
    // mask/filter attributes are applied via the R7 layer pipeline (see
    // shape_layer / layer_for_group), so they are no longer diagnosed here.
    if final_style_property(attrs, "fill-rule")
        .is_some_and(|value| parse_fill_rule(value).is_none())
    {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "style.invalid_fill_rule",
            message: "invalid fill-rule value was ignored; inherited or default nonzero behavior was used",
        });
    }
    if final_style_property(attrs, "stroke-width")
        .is_some_and(|value| svg_core::parse_length(value).is_none_or(|length| length.value < 0.0))
    {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "style.invalid_stroke_width",
            message: "invalid stroke-width was ignored; the inherited stroke width was used",
        });
    }
    if final_style_property(attrs, "stroke-linecap")
        .is_some_and(|value| parse_stroke_linecap(value).is_none())
    {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "style.invalid_stroke_linecap",
            message: "invalid stroke-linecap was ignored; the inherited line cap was used",
        });
    }
    if let Some(value) = final_style_property(attrs, "stroke-linejoin") {
        if parse_stroke_linejoin(value).is_none() {
            diagnostics.push(PendingDiagnostic::Warning {
                code: "style.invalid_stroke_linejoin",
                message: "invalid stroke-linejoin was ignored; the inherited line join was used",
            });
        } else if value.trim().eq_ignore_ascii_case("arcs") {
            diagnostics.push(PendingDiagnostic::Warning {
                code: "style.stroke_linejoin_arcs_approximated",
                message: "stroke-linejoin arcs is approximated with bounded miter-clip geometry",
            });
        }
    }
    if final_style_property(attrs, "stroke-miterlimit").is_some_and(|value| {
        value
            .parse::<f64>()
            .map_or(true, |limit| !limit.is_finite() || limit < 1.0)
    }) {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "style.invalid_stroke_miterlimit",
            message: "invalid stroke-miterlimit was ignored; the inherited miter limit was used",
        });
    }
    if final_style_property(attrs, "stroke-dasharray")
        .is_some_and(|value| parse_dasharray(value).is_err())
    {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "style.invalid_stroke_dasharray",
            message: "invalid stroke-dasharray was ignored; the inherited dash pattern was used",
        });
    }
    if final_style_property(attrs, "stroke-dashoffset")
        .is_some_and(|value| svg_core::parse_length(value).is_none())
    {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "style.invalid_stroke_dashoffset",
            message: "invalid stroke-dashoffset was ignored; the inherited dash offset was used",
        });
    }
    if attr_get(attrs, "pathlength").is_some_and(|value| {
        value
            .parse::<f64>()
            .map_or(true, |length| !length.is_finite() || length <= 0.0)
    }) {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "geometry.invalid_path_length",
            message: "invalid pathLength was ignored; uncalibrated geometric length was used",
        });
    }
    diagnostics
}

fn emit_diagnostics(
    diagnostics: &[PendingDiagnostic],
    source: SvgRenderSource,
    report: &mut SvgRenderReport,
) {
    for diagnostic in diagnostics {
        let PendingDiagnostic::Warning { code, message } = diagnostic;
        report.warning_at(*code, *message, Some(source));
    }
}

fn final_style_property<'a>(attrs: &'a [(String, String)], property: &str) -> Option<&'a str> {
    let presentation = attr_get(attrs, property);
    let inline = attr_get(attrs, "style").and_then(|style| {
        style.split(';').rev().find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case(property)
                .then_some(value.trim())
        })
    });
    inline.or(presentation)
}

fn fallback_image(w: usize, h: usize) -> ColorImage {
    let mut buf = vec![50u8; w * h * 4];
    for i in (3..buf.len()).step_by(4) {
        buf[i] = 180;
    }
    ColorImage::from_rgba_unmultiplied([w, h], &buf)
}

// ---------------------------------------------------------------------------
// RGBA + Paint
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Paint {
    None,
    Color(Rgba),
    Server(String),
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Color(Rgba::BLACK)
    }
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FillRule {
    #[default]
    Nonzero,
    Evenodd,
}

fn parse_fill_rule(value: &str) -> Option<FillRule> {
    match value.trim().to_ascii_lowercase().as_str() {
        "nonzero" => Some(FillRule::Nonzero),
        "evenodd" => Some(FillRule::Evenodd),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StrokeLineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StrokeLineJoin {
    #[default]
    Miter,
    MiterClip,
    Round,
    Bevel,
    Arcs,
}

fn parse_stroke_linecap(value: &str) -> Option<StrokeLineCap> {
    match value.trim().to_ascii_lowercase().as_str() {
        "butt" => Some(StrokeLineCap::Butt),
        "round" => Some(StrokeLineCap::Round),
        "square" => Some(StrokeLineCap::Square),
        _ => None,
    }
}

fn parse_stroke_linejoin(value: &str) -> Option<StrokeLineJoin> {
    match value.trim().to_ascii_lowercase().as_str() {
        "miter" => Some(StrokeLineJoin::Miter),
        "miter-clip" => Some(StrokeLineJoin::MiterClip),
        "round" => Some(StrokeLineJoin::Round),
        "bevel" => Some(StrokeLineJoin::Bevel),
        "arcs" => Some(StrokeLineJoin::Arcs),
        _ => None,
    }
}

/// `vector-effect` (R9). Only `non-scaling-stroke` is supported; every other
/// value is diagnosed and treated as `none`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum VectorEffect {
    #[default]
    None,
    NonScalingStroke,
}

/// Parse a `vector-effect` value. Returns `None` for unrecognized values so the
/// caller can emit a diagnostic; recognized values map to a `VectorEffect`.
fn parse_vector_effect(value: &str) -> Option<VectorEffect> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(VectorEffect::None),
        "non-scaling-stroke" => Some(VectorEffect::NonScalingStroke),
        _ => None,
    }
}

fn default_stroke_width() -> svg_core::SvgLength {
    svg_core::SvgLength {
        value: 1.0,
        unit: svg_core::SvgLengthUnit::Number,
    }
}

fn zero_stroke_length() -> svg_core::SvgLength {
    svg_core::SvgLength {
        value: 0.0,
        unit: svg_core::SvgLengthUnit::Number,
    }
}

#[derive(Clone)]
struct Style {
    color: Rgba,
    fill: Paint,
    fill_rule: FillRule,
    stroke: Paint,
    stroke_width: svg_core::SvgLength,
    stroke_linecap: StrokeLineCap,
    stroke_linejoin: StrokeLineJoin,
    stroke_miterlimit: f64,
    stroke_dasharray: Option<Vec<svg_core::SvgLength>>,
    stroke_dashoffset: svg_core::SvgLength,
    opacity: f32,
    fill_opacity: f32,
    stroke_opacity: f32,
    stop_color: Rgba,
    stop_opacity: f32,
    visible: bool,
    vector_effect: VectorEffect,
    /// Inherited `font-size` (R11 raster text); resolved at text lowering.
    font_size: svg_core::SvgLength,
    /// Inherited `text-anchor` (R11 raster text).
    text_anchor: TextAnchor,
}

/// `text-anchor` (R11). Applied to the whole laid-out run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

fn parse_text_anchor(value: &str) -> Option<TextAnchor> {
    match value.trim().to_ascii_lowercase().as_str() {
        "start" => Some(TextAnchor::Start),
        "middle" => Some(TextAnchor::Middle),
        "end" => Some(TextAnchor::End),
        _ => None,
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: Rgba::BLACK,
            fill: Paint::Color(Rgba::BLACK),
            fill_rule: FillRule::Nonzero,
            stroke: Paint::None,
            stroke_width: default_stroke_width(),
            stroke_linecap: StrokeLineCap::Butt,
            stroke_linejoin: StrokeLineJoin::Miter,
            stroke_miterlimit: 4.0,
            stroke_dasharray: None,
            stroke_dashoffset: zero_stroke_length(),
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            stop_color: Rgba::BLACK,
            stop_opacity: 1.0,
            visible: true,
            vector_effect: VectorEffect::None,
            font_size: svg_core::SvgLength {
                value: 16.0,
                unit: svg_core::SvgLengthUnit::Number,
            },
            text_anchor: TextAnchor::Start,
        }
    }
}

impl Style {
    fn inherit(&self, node: &SvgNode, sheet: &svg_core::SvgCssStyleSheet) -> Style {
        self.inherit_parts(node.tag_name(), node.attrs(), sheet)
    }

    fn inherit_parts(
        &self,
        tag_name: &str,
        attrs: &[(String, String)],
        sheet: &svg_core::SvgCssStyleSheet,
    ) -> Style {
        let mut s = self.clone();
        // `opacity` is a non-inherited property: it applies to the element (or,
        // for a group, to the group as a composited whole) and must not cascade
        // to children, or overlapping children in a translucent group would
        // double-darken.  Reset before applying this element's own declarations.
        s.opacity = 1.0;
        // `vector-effect` is likewise non-inherited: it applies only to the
        // element that declares it.
        s.vector_effect = VectorEffect::None;
        for (key, value) in attrs {
            if key != "style" {
                s.apply_declaration(key, value);
            }
        }

        let id = attr_get(attrs, "id");
        let classes = attr_get(attrs, "class").unwrap_or("");
        let mut matches: Vec<_> = sheet
            .rules
            .iter()
            .filter_map(|rule| {
                rule.matching_specificity(tag_name, id, classes)
                    .map(|specificity| (specificity, rule.source_order, rule))
            })
            .collect();
        matches.sort_by_key(|(specificity, order, _)| (*specificity, *order));
        for (_, _, rule) in matches {
            for declaration in &rule.declarations {
                s.apply_declaration(&declaration.name, &declaration.value);
            }
        }

        if let Some(style_val) = attr_get(attrs, "style") {
            s.apply_css(style_val);
        }
        s
    }

    fn apply_declaration(&mut self, key: &str, value: &str) {
        match key {
            "color" => {
                if let Some(color) = svg_core::parse_color(value) {
                    self.color = color;
                }
            }
            "fill" => self.fill = parse_paint(value, self.color),
            "fill-rule" => {
                if let Some(fill_rule) = parse_fill_rule(value) {
                    self.fill_rule = fill_rule;
                }
            }
            "stroke" => self.stroke = parse_paint(value, self.color),
            "stroke-width" => {
                if let Some(width) =
                    svg_core::parse_length(value).filter(|width| width.value >= 0.0)
                {
                    self.stroke_width = width;
                }
            }
            "stroke-linecap" => {
                if let Some(linecap) = parse_stroke_linecap(value) {
                    self.stroke_linecap = linecap;
                }
            }
            "stroke-linejoin" => {
                if let Some(linejoin) = parse_stroke_linejoin(value) {
                    self.stroke_linejoin = linejoin;
                }
            }
            "stroke-miterlimit" => {
                if let Ok(limit) = value.parse::<f64>() {
                    if limit.is_finite() && limit >= 1.0 {
                        self.stroke_miterlimit = limit;
                    }
                }
            }
            "stroke-dasharray" => {
                if let Ok(array) = parse_dasharray(value) {
                    self.stroke_dasharray = array;
                }
            }
            "stroke-dashoffset" => {
                if let Some(offset) = svg_core::parse_length(value) {
                    self.stroke_dashoffset = offset;
                }
            }
            "opacity" => {
                self.opacity = value.parse().unwrap_or(self.opacity);
            }
            "fill-opacity" => {
                self.fill_opacity = value.parse().unwrap_or(self.fill_opacity);
            }
            "stroke-opacity" => {
                self.stroke_opacity = value.parse().unwrap_or(self.stroke_opacity);
            }
            "stop-color" => {
                if value.trim().eq_ignore_ascii_case("currentcolor") {
                    self.stop_color = self.color;
                } else if let Some(color) = svg_core::parse_color(value) {
                    self.stop_color = color;
                }
            }
            "stop-opacity" => {
                self.stop_opacity = value.parse().unwrap_or(self.stop_opacity);
            }
            "vector-effect" => {
                // Recognized values set the effect; unrecognized values fall back
                // to None and are diagnosed at lowering time.
                if let Some(effect) = parse_vector_effect(value) {
                    self.vector_effect = effect;
                }
            }
            "font-size" => {
                if let Some(size) = svg_core::parse_length(value)
                    .filter(|size| size.value.is_finite() && size.value > 0.0)
                {
                    self.font_size = size;
                }
            }
            "text-anchor" => {
                if let Some(anchor) = parse_text_anchor(value) {
                    self.text_anchor = anchor;
                }
            }
            "display" if value.trim() == "none" => {
                self.visible = false;
            }
            "visibility" => {
                if matches!(value.trim(), "hidden" | "collapse") {
                    self.visible = false;
                }
            }
            _ => {}
        }
    }

    fn apply_css(&mut self, css: &str) {
        let (declarations, _) = svg_core::parse_style_declarations(css, MAX_CSS_DECLARATIONS);
        for declaration in declarations {
            self.apply_declaration(&declaration.name, &declaration.value);
        }
    }

    fn effective_fill(&self) -> Option<ResolvedPaint> {
        if !self.visible {
            return None;
        }
        match &self.fill {
            Paint::None => None,
            Paint::Color(c) => Some(ResolvedPaint {
                source: ResolvedPaintSource::Solid(*c),
                opacity: self.fill_opacity * self.opacity,
            }),
            Paint::Server(id) => Some(ResolvedPaint {
                source: ResolvedPaintSource::Server(id.clone()),
                opacity: self.fill_opacity * self.opacity,
            }),
        }
    }

    fn effective_stroke(&self, length_bases: SvgLengthBases) -> Option<ResolvedStroke> {
        if !self.visible {
            return None;
        }
        let width = resolve_stroke_length(self.stroke_width, length_bases)?;
        if width <= 0.0 {
            return None;
        }
        match &self.stroke {
            Paint::None => None,
            Paint::Color(_) | Paint::Server(_) => {
                let dash_array = self.stroke_dasharray.as_ref().and_then(|array| {
                    let resolved: Option<Vec<f64>> = array
                        .iter()
                        .map(|length| resolve_stroke_length(*length, length_bases))
                        .collect();
                    resolved.filter(|values| values.iter().any(|value| *value > 0.0))
                });
                Some(ResolvedStroke {
                    paint: ResolvedPaint {
                        source: match &self.stroke {
                            Paint::Color(color) => ResolvedPaintSource::Solid(*color),
                            Paint::Server(id) => ResolvedPaintSource::Server(id.clone()),
                            Paint::None => unreachable!(),
                        },
                        opacity: self.stroke_opacity * self.opacity,
                    },
                    width,
                    linecap: self.stroke_linecap,
                    linejoin: self.stroke_linejoin,
                    miterlimit: self.stroke_miterlimit,
                    dash_array,
                    dash_offset: resolve_stroke_length(self.stroke_dashoffset, length_bases)
                        .unwrap_or(0.0),
                })
            }
        }
    }

    fn paint_server_references(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            match &self.fill {
                Paint::Server(id) => Some(("fill", id.as_str())),
                Paint::None | Paint::Color(_) => None,
            },
            match &self.stroke {
                Paint::Server(id) => Some(("stroke", id.as_str())),
                Paint::None | Paint::Color(_) => None,
            },
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Clone)]
struct ResolvedStroke {
    paint: ResolvedPaint,
    width: f64,
    linecap: StrokeLineCap,
    linejoin: StrokeLineJoin,
    miterlimit: f64,
    dash_array: Option<Vec<f64>>,
    dash_offset: f64,
}

#[derive(Clone)]
struct ResolvedPaint {
    source: ResolvedPaintSource,
    opacity: f32,
}

#[derive(Clone)]
enum ResolvedPaintSource {
    Solid(Rgba),
    Server(String),
}

fn resolve_stroke_length(length: svg_core::SvgLength, length_bases: SvgLengthBases) -> Option<f64> {
    length.resolve(svg_core::SvgLengthContext::user_units(length_bases.other))
}

fn parse_dasharray(value: &str) -> Result<Option<Vec<svg_core::SvgLength>>, ()> {
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let mut values = Vec::new();
    for token in value.split(|c: char| c == ',' || c.is_ascii_whitespace()) {
        if token.is_empty() {
            continue;
        }
        let length = svg_core::parse_length(token).ok_or(())?;
        if length.value < 0.0 || !length.value.is_finite() {
            return Err(());
        }
        values.push(length);
    }
    if values.is_empty() || values.len() > MAX_DASH_ENTRIES {
        return Err(());
    }
    if values.len() % 2 == 1 {
        let repeated = values.clone();
        values.extend(repeated);
    }
    Ok(Some(values))
}

// ---------------------------------------------------------------------------
// Color parsing
// ---------------------------------------------------------------------------

fn parse_paint(s: &str, current_color: Rgba) -> Paint {
    let s = s.trim();
    if s.eq_ignore_ascii_case("currentcolor") {
        Paint::Color(current_color)
    } else if let Some(id) = local_url_reference(s) {
        Paint::Server(id.to_owned())
    } else if s == "none" || s == "transparent" || s.is_empty() || s.starts_with("url(") {
        Paint::None
    } else {
        match svg_core::parse_color(s) {
            Some(c) => Paint::Color(c),
            None => Paint::Color(Rgba::BLACK),
        }
    }
}

fn local_url_reference(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let body = trimmed.strip_prefix("url(")?.strip_suffix(')')?.trim();
    body.trim_matches(['"', '\'']).strip_prefix('#')
}

const MAX_GRADIENT_STOPS: usize = 4_096;
const MAX_GRADIENT_REFERENCE_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GradientUnits {
    UserSpaceOnUse,
    #[default]
    ObjectBoundingBox,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GradientSpread {
    #[default]
    Pad,
    Reflect,
    Repeat,
}

#[derive(Clone)]
struct GradientStop {
    offset: f64,
    color: Rgba,
}

#[derive(Clone)]
struct GradientCommon {
    units: GradientUnits,
    transform: Transform,
    spread: GradientSpread,
    stops: Vec<GradientStop>,
}

#[derive(Clone)]
enum PaintServer {
    Linear {
        common: GradientCommon,
        x1: svg_core::SvgLength,
        y1: svg_core::SvgLength,
        x2: svg_core::SvgLength,
        y2: svg_core::SvgLength,
    },
    Radial {
        common: GradientCommon,
        cx: svg_core::SvgLength,
        cy: svg_core::SvgLength,
        r: svg_core::SvgLength,
        fx: svg_core::SvgLength,
        fy: svg_core::SvgLength,
    },
}

#[derive(Clone)]
struct PaintServerWarning {
    code: &'static str,
    message: String,
    source: SvgRenderSource,
}

#[derive(Clone, Default)]
struct PaintServerTable {
    servers: HashMap<String, PaintServer>,
    /// Resolved `<pattern>` paint servers (R9), tiled lazily per fill at paint
    /// time.  Kept separate from gradient `servers` so the gradient enum stays
    /// untouched.
    patterns: HashMap<String, PatternDef>,
    warnings: Vec<PaintServerWarning>,
}

impl PaintServerTable {
    fn build(
        references: &SvgReferenceTable,
        stylesheet: &svg_core::SvgCssStyleSheet,
        root_style: &Style,
    ) -> Self {
        let mut table = Self::default();
        for id in references
            .ordered_ids
            .iter()
            .map(|local| local.xml_id.clone())
        {
            let mut stack = Vec::new();
            let _ = table.resolve(&id, references, stylesheet, root_style, &mut stack);
        }
        // R9: resolve `<pattern>` definitions (separate pass; patterns are not
        // gradients and inherit attributes/content via their own href chain).
        for local in &references.ordered_ids {
            let is_pattern = references.nodes_by_id.get(&local.node_id).is_some_and(
                |node| matches!(node, SvgNode::Unsupported { tag, .. } if tag == "pattern"),
            );
            if !is_pattern || table.patterns.contains_key(&local.xml_id) {
                continue;
            }
            let mut stack = Vec::new();
            if let Some(def) = build_pattern_def(
                &local.xml_id,
                references,
                stylesheet,
                &mut table.warnings,
                &mut stack,
            ) {
                table.patterns.insert(local.xml_id.clone(), def);
            }
        }
        table
    }

    fn resolve(
        &mut self,
        id: &str,
        references: &SvgReferenceTable,
        stylesheet: &svg_core::SvgCssStyleSheet,
        root_style: &Style,
        stack: &mut Vec<SvgNodeId>,
    ) -> Option<PaintServer> {
        if let Some(server) = self.servers.get(id) {
            return Some(server.clone());
        }
        if stack.len() >= MAX_GRADIENT_REFERENCE_DEPTH {
            if let Some(node) = references
                .by_xml_id
                .get(id)
                .and_then(|node_id| references.nodes_by_id.get(node_id))
            {
                self.warnings.push(PaintServerWarning {
                    code: "limit.gradient_reference_depth",
                    message: "gradient href inheritance exceeded its depth limit".to_owned(),
                    source: node.source(),
                });
            }
            return None;
        }
        let node = references
            .by_xml_id
            .get(id)
            .and_then(|node_id| references.nodes_by_id.get(node_id))?;
        if !matches!(
            node,
            SvgNode::LinearGradient { .. } | SvgNode::RadialGradient { .. }
        ) {
            return None;
        }
        if stack.contains(&node.id()) {
            self.warnings.push(PaintServerWarning {
                code: "reference.gradient_cycle",
                message: "cyclic gradient href inheritance was ignored".to_owned(),
                source: node.source(),
            });
            return None;
        }
        stack.push(node.id());
        let inherited = attr_get(node.attrs(), "href")
            .and_then(|href| href.trim().strip_prefix('#'))
            .and_then(|base| self.resolve(base, references, stylesheet, root_style, stack));
        let server = self.resolve_node(node, inherited, stylesheet, root_style);
        stack.pop();
        if let Some(server) = &server {
            self.servers.insert(id.to_owned(), server.clone());
        }
        server
    }

    fn resolve_node(
        &mut self,
        node: &SvgNode,
        inherited: Option<PaintServer>,
        stylesheet: &svg_core::SvgCssStyleSheet,
        root_style: &Style,
    ) -> Option<PaintServer> {
        let attrs = node.attrs();
        let units_attr = attr_get(attrs, "gradientunits");
        if units_attr.is_some_and(|value| parse_gradient_units(value).is_none()) {
            self.warn(
                node,
                "paint.invalid_gradient_units",
                "invalid gradientUnits was ignored; inherited or objectBoundingBox units were used",
            );
        }
        let units = units_attr
            .and_then(parse_gradient_units)
            .or_else(|| {
                inherited
                    .as_ref()
                    .map(PaintServer::common)
                    .map(|common| common.units)
            })
            .unwrap_or_default();
        let transform_attr = attr_get(attrs, "gradienttransform");
        if transform_attr.is_some_and(|value| Transform::parse_transform_checked(value).is_none()) {
            self.warn(
                node,
                "paint.invalid_gradient_transform",
                "invalid gradientTransform was ignored; inherited or identity transform was used",
            );
        }
        let transform = transform_attr
            .and_then(Transform::parse_transform_checked)
            .or_else(|| {
                inherited
                    .as_ref()
                    .map(PaintServer::common)
                    .map(|common| common.transform)
            })
            .unwrap_or_else(Transform::identity);
        let spread_attr = attr_get(attrs, "spreadmethod");
        if spread_attr.is_some_and(|value| parse_gradient_spread(value).is_none()) {
            self.warn(
                node,
                "paint.invalid_spread_method",
                "invalid spreadMethod was ignored; inherited or pad behavior was used",
            );
        }
        let spread = spread_attr
            .and_then(parse_gradient_spread)
            .or_else(|| {
                inherited
                    .as_ref()
                    .map(PaintServer::common)
                    .map(|common| common.spread)
            })
            .unwrap_or_default();
        self.warn_for_invalid_stops(node);
        let mut stops = gradient_stops(node, stylesheet, root_style);
        if stops.is_empty() {
            stops = inherited
                .as_ref()
                .map(PaintServer::common)
                .map(|common| common.stops.clone())
                .unwrap_or_default();
        }
        if stops.len() > MAX_GRADIENT_STOPS {
            stops.truncate(MAX_GRADIENT_STOPS);
            self.warnings.push(PaintServerWarning {
                code: "limit.gradient_stops",
                message: "gradient stops exceeded the renderer safety limit".to_owned(),
                source: node.source(),
            });
        }
        if stops.is_empty() {
            self.warnings.push(PaintServerWarning {
                code: "paint.gradient_without_stops",
                message: "gradient has no usable stops and was left transparent".to_owned(),
                source: node.source(),
            });
        }
        let common = GradientCommon {
            units,
            transform,
            spread,
            stops,
        };

        match node {
            SvgNode::LinearGradient { .. } => {
                let inherited = match inherited {
                    Some(PaintServer::Linear { x1, y1, x2, y2, .. }) => Some((x1, y1, x2, y2)),
                    _ => None,
                };
                Some(PaintServer::Linear {
                    common,
                    x1: self.gradient_length_or(
                        node,
                        "x1",
                        inherited.as_ref().map(|values| values.0),
                        percent_length(0.0),
                        false,
                    ),
                    y1: self.gradient_length_or(
                        node,
                        "y1",
                        inherited.as_ref().map(|values| values.1),
                        percent_length(0.0),
                        false,
                    ),
                    x2: self.gradient_length_or(
                        node,
                        "x2",
                        inherited.as_ref().map(|values| values.2),
                        percent_length(100.0),
                        false,
                    ),
                    y2: self.gradient_length_or(
                        node,
                        "y2",
                        inherited.as_ref().map(|values| values.3),
                        percent_length(0.0),
                        false,
                    ),
                })
            }
            SvgNode::RadialGradient { .. } => {
                let inherited = match inherited {
                    Some(PaintServer::Radial {
                        cx, cy, r, fx, fy, ..
                    }) => Some((cx, cy, r, fx, fy)),
                    _ => None,
                };
                let cx = self.gradient_length_or(
                    node,
                    "cx",
                    inherited.as_ref().map(|values| values.0),
                    percent_length(50.0),
                    false,
                );
                let cy = self.gradient_length_or(
                    node,
                    "cy",
                    inherited.as_ref().map(|values| values.1),
                    percent_length(50.0),
                    false,
                );
                Some(PaintServer::Radial {
                    common,
                    cx,
                    cy,
                    r: self.gradient_length_or(
                        node,
                        "r",
                        inherited.as_ref().map(|values| values.2),
                        percent_length(50.0),
                        true,
                    ),
                    fx: self.gradient_length_or(
                        node,
                        "fx",
                        inherited.as_ref().map(|values| values.3),
                        cx,
                        false,
                    ),
                    fy: self.gradient_length_or(
                        node,
                        "fy",
                        inherited.as_ref().map(|values| values.4),
                        cy,
                        false,
                    ),
                })
            }
            _ => None,
        }
    }

    fn gradient_length_or(
        &mut self,
        node: &SvgNode,
        key: &str,
        inherited: Option<svg_core::SvgLength>,
        default: svg_core::SvgLength,
        require_nonnegative: bool,
    ) -> svg_core::SvgLength {
        if let Some(value) = attr_get(node.attrs(), key) {
            if let Some(length) = svg_core::parse_length(value)
                .filter(|length| !require_nonnegative || length.value >= 0.0)
            {
                return length;
            }
            self.warn(
                node,
                "paint.invalid_gradient_length",
                format!(
                    "invalid {key} gradient length was ignored; inherited or default geometry was used"
                ),
            );
        }
        inherited.unwrap_or(default)
    }

    fn warn_for_invalid_stops(&mut self, node: &SvgNode) {
        for child in node.children().unwrap_or_default() {
            let SvgNode::Stop { attrs, .. } = child else {
                continue;
            };
            if attr_get(attrs, "offset").is_some_and(|value| parse_stop_offset(value).is_none()) {
                self.warn(
                    child,
                    "paint.invalid_stop_offset",
                    "invalid stop offset was clamped to the preceding valid offset",
                );
            }
            if final_style_property(attrs, "stop-opacity").is_some_and(|value| {
                value
                    .parse::<f32>()
                    .map_or(true, |opacity| !opacity.is_finite())
            }) {
                self.warn(
                    child,
                    "paint.invalid_stop_opacity",
                    "invalid stop-opacity was ignored; inherited opacity was used",
                );
            }
            if final_style_property(attrs, "stop-color").is_some_and(|value| {
                !value.trim().eq_ignore_ascii_case("currentcolor")
                    && svg_core::parse_color(value).is_none()
            }) {
                self.warn(
                    child,
                    "paint.invalid_stop_color",
                    "invalid stop-color was ignored; inherited color was used",
                );
            }
        }
    }

    fn warn(&mut self, node: &SvgNode, code: &'static str, message: impl Into<String>) {
        self.warnings.push(PaintServerWarning {
            code,
            message: message.into(),
            source: node.source(),
        });
    }

    fn report_into(&self, report: &mut SvgRenderReport) {
        for warning in &self.warnings {
            report.warning_at(warning.code, warning.message.clone(), Some(warning.source));
        }
    }
}

impl PaintServer {
    fn common(&self) -> &GradientCommon {
        match self {
            Self::Linear { common, .. } | Self::Radial { common, .. } => common,
        }
    }
}

fn gradient_stops(
    node: &SvgNode,
    stylesheet: &svg_core::SvgCssStyleSheet,
    root_style: &Style,
) -> Vec<GradientStop> {
    let gradient_style = root_style.inherit(node, stylesheet);
    let mut stops = Vec::new();
    for child in node.children().unwrap_or_default() {
        let SvgNode::Stop { attrs, .. } = child else {
            continue;
        };
        let style = gradient_style.inherit(child, stylesheet);
        let offset = attr_get(attrs, "offset")
            .and_then(parse_stop_offset)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
            .max(stops.last().map_or(0.0, |stop: &GradientStop| stop.offset));
        let mut color = style.stop_color;
        color.a = (color.a as f32 * style.stop_opacity.clamp(0.0, 1.0))
            .round()
            .clamp(0.0, 255.0) as u8;
        stops.push(GradientStop { offset, color });
    }
    stops
}

fn parse_stop_offset(value: &str) -> Option<f64> {
    let length = svg_core::parse_length(value)?;
    match length.unit {
        svg_core::SvgLengthUnit::Percent => Some(length.value / 100.0),
        svg_core::SvgLengthUnit::Number => Some(length.value),
        _ => None,
    }
}

fn parse_gradient_units(value: &str) -> Option<GradientUnits> {
    match value.trim().to_ascii_lowercase().as_str() {
        "userspaceonuse" => Some(GradientUnits::UserSpaceOnUse),
        "objectboundingbox" => Some(GradientUnits::ObjectBoundingBox),
        _ => None,
    }
}

fn parse_gradient_spread(value: &str) -> Option<GradientSpread> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pad" => Some(GradientSpread::Pad),
        "reflect" => Some(GradientSpread::Reflect),
        "repeat" => Some(GradientSpread::Repeat),
        _ => None,
    }
}

fn percent_length(value: f64) -> svg_core::SvgLength {
    svg_core::SvgLength {
        value,
        unit: svg_core::SvgLengthUnit::Percent,
    }
}

// ---------------------------------------------------------------------------
// R9: pattern paint servers (tiled nested content)
// ---------------------------------------------------------------------------

/// `patternUnits` / `patternContentUnits` coordinate system (R9).
#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternUnits {
    UserSpaceOnUse,
    ObjectBoundingBox,
}

fn parse_pattern_units(value: &str) -> Option<PatternUnits> {
    match value.trim().to_ascii_lowercase().as_str() {
        "userspaceonuse" => Some(PatternUnits::UserSpaceOnUse),
        "objectboundingbox" => Some(PatternUnits::ObjectBoundingBox),
        _ => None,
    }
}

/// A resolved `<pattern>` paint server (R9). Attributes are stored post-href
/// merge as `Option`s; defaults (`objectBoundingBox` tile units,
/// `userSpaceOnUse` content units) are applied when a tile is built so that
/// `href`-chained patterns inherit cleanly. The tile itself is rendered lazily
/// per fill in `build_pattern_sampler`, mirroring `MaskDef::build_alpha`.
#[derive(Clone)]
struct PatternDef {
    units: Option<PatternUnits>,
    content_units: Option<PatternUnits>,
    x: Option<svg_core::SvgLength>,
    y: Option<svg_core::SvgLength>,
    width: Option<svg_core::SvgLength>,
    height: Option<svg_core::SvgLength>,
    view_box: Option<[f64; 4]>,
    aspect: Option<svg_core::SvgPreserveAspectRatio>,
    pattern_transform: Option<Transform>,
    items: Vec<MaskItem>,
}

/// Resolve a `<pattern>` definition, merging `href`-inherited attributes and
/// content. Bounded by `MAX_PATTERN_REFERENCE_DEPTH`; cyclic href chains are
/// diagnosed and dropped (never panic).
fn build_pattern_def(
    id: &str,
    references: &SvgReferenceTable,
    stylesheet: &svg_core::SvgCssStyleSheet,
    warnings: &mut Vec<PaintServerWarning>,
    stack: &mut Vec<SvgNodeId>,
) -> Option<PatternDef> {
    if stack.len() >= MAX_PATTERN_REFERENCE_DEPTH {
        return None;
    }
    let node = references
        .by_xml_id
        .get(id)
        .and_then(|nid| references.nodes_by_id.get(nid))?;
    let SvgNode::Unsupported {
        tag,
        attrs,
        children,
        ..
    } = node
    else {
        return None;
    };
    if tag != "pattern" {
        return None;
    }
    if stack.contains(&node.id()) {
        warnings.push(PaintServerWarning {
            code: "reference.pattern_cycle",
            message: "cyclic pattern href inheritance was ignored".to_owned(),
            source: node.source(),
        });
        return None;
    }
    stack.push(node.id());
    let inherited = attr_get(attrs, "href")
        .or_else(|| attr_get(attrs, "xlink:href"))
        .and_then(|href| href.trim().strip_prefix('#'))
        .and_then(|base| build_pattern_def(base, references, stylesheet, warnings, stack));
    stack.pop();

    let inherit = |pick: &dyn Fn(&PatternDef) -> Option<svg_core::SvgLength>, key: &str| {
        attr_get(attrs, key)
            .and_then(svg_core::parse_length)
            .or_else(|| inherited.as_ref().and_then(pick))
    };
    let units = attr_get(attrs, "patternunits")
        .and_then(parse_pattern_units)
        .or_else(|| inherited.as_ref().and_then(|p| p.units));
    let content_units = attr_get(attrs, "patterncontentunits")
        .and_then(parse_pattern_units)
        .or_else(|| inherited.as_ref().and_then(|p| p.content_units));
    let x = inherit(&|p| p.x, "x");
    let y = inherit(&|p| p.y, "y");
    let width = inherit(&|p| p.width, "width");
    let height = inherit(&|p| p.height, "height");
    let view_box = parse_view_box(attrs).or_else(|| inherited.as_ref().and_then(|p| p.view_box));
    let aspect = attr_get(attrs, "preserveaspectratio")
        .map(svg_core::parse_preserve_aspect_ratio)
        .or_else(|| inherited.as_ref().and_then(|p| p.aspect));
    let pattern_transform = attr_get(attrs, "patterntransform")
        .map(Transform::parse_chained)
        .or_else(|| inherited.as_ref().and_then(|p| p.pattern_transform));

    let content_bases = match view_box {
        Some([_, _, w, h]) if w > 0.0 && h > 0.0 => SvgLengthBases::new(w.abs(), h.abs()),
        _ => SvgLengthBases::new(0.0, 0.0),
    };
    let root_style = Style::default();
    let mut items = Vec::new();
    collect_mask_items(
        children,
        Transform::identity(),
        content_bases,
        &root_style,
        stylesheet,
        &mut items,
    );
    if items.is_empty() {
        if let Some(base) = &inherited {
            items = base.items.clone();
        }
    }
    items.truncate(MAX_PATTERN_CONTENT_ITEMS);

    Some(PatternDef {
        units,
        content_units,
        x,
        y,
        width,
        height,
        view_box,
        aspect,
        pattern_transform,
        items,
    })
}

#[derive(Clone)]
enum PaintSampler {
    Solid([u8; 4]),
    Linear {
        device_to_gradient: Transform,
        from: (f64, f64),
        to: (f64, f64),
        spread: GradientSpread,
        stops: Vec<GradientStop>,
        opacity: f32,
    },
    Radial {
        device_to_gradient: Transform,
        center: (f64, f64),
        focal: (f64, f64),
        radius: f64,
        spread: GradientSpread,
        stops: Vec<GradientStop>,
        opacity: f32,
    },
    /// R9 tiled `<pattern>`: a pre-rendered straight-RGBA tile repeated across
    /// the fill via the device→pattern mapping.
    Pattern {
        tile: Vec<u8>,
        tile_w: usize,
        tile_h: usize,
        device_to_pattern: Transform,
        origin: (f64, f64),
        size: (f64, f64),
        opacity: f32,
    },
    Transparent,
}

impl PaintSampler {
    fn from_resolved(
        paint: &ResolvedPaint,
        servers: &PaintServerTable,
        local_bounds: [f64; 4],
        object_transform: Transform,
        length_bases: SvgLengthBases,
    ) -> Self {
        match &paint.source {
            ResolvedPaintSource::Solid(color) => {
                let mut color = [color.r, color.g, color.b, color.a];
                color[3] = (color[3] as f32 * paint.opacity.clamp(0.0, 1.0))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                Self::Solid(color)
            }
            ResolvedPaintSource::Server(id) => {
                if let Some(server) = servers.servers.get(id) {
                    Self::from_server(
                        server,
                        paint.opacity,
                        local_bounds,
                        object_transform,
                        length_bases,
                    )
                } else if let Some(pattern) = servers.patterns.get(id) {
                    build_pattern_sampler(
                        pattern,
                        id,
                        paint.opacity,
                        local_bounds,
                        object_transform,
                        length_bases,
                        servers,
                    )
                } else {
                    Self::Transparent
                }
            }
        }
    }

    fn from_server(
        server: &PaintServer,
        opacity: f32,
        bounds: [f64; 4],
        object_transform: Transform,
        length_bases: SvgLengthBases,
    ) -> Self {
        let common = server.common();
        if common.stops.is_empty() {
            return Self::Transparent;
        }
        let units_transform = match common.units {
            GradientUnits::UserSpaceOnUse => Transform::identity(),
            GradientUnits::ObjectBoundingBox => {
                let width = bounds[2] - bounds[0];
                let height = bounds[3] - bounds[1];
                if width.abs() <= 1.0e-15 || height.abs() <= 1.0e-15 {
                    return Self::Transparent;
                }
                Transform::translate(bounds[0], bounds[1]).multiply(Transform::scale(width, height))
            }
        };
        let Some(device_to_gradient) = object_transform
            .multiply(units_transform)
            .multiply(common.transform)
            .inverse()
        else {
            return Self::Transparent;
        };
        let resolve = |length: svg_core::SvgLength, axis: GradientAxis| {
            resolve_gradient_length(length, common.units, axis, length_bases)
        };
        match server {
            PaintServer::Linear { x1, y1, x2, y2, .. } => {
                let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                    resolve(*x1, GradientAxis::Horizontal),
                    resolve(*y1, GradientAxis::Vertical),
                    resolve(*x2, GradientAxis::Horizontal),
                    resolve(*y2, GradientAxis::Vertical),
                ) else {
                    return Self::Transparent;
                };
                Self::Linear {
                    device_to_gradient,
                    from: (x1, y1),
                    to: (x2, y2),
                    spread: common.spread,
                    stops: common.stops.clone(),
                    opacity,
                }
            }
            PaintServer::Radial {
                cx, cy, r, fx, fy, ..
            } => {
                let (Some(cx), Some(cy), Some(radius), Some(mut fx), Some(mut fy)) = (
                    resolve(*cx, GradientAxis::Horizontal),
                    resolve(*cy, GradientAxis::Vertical),
                    resolve(*r, GradientAxis::Other),
                    resolve(*fx, GradientAxis::Horizontal),
                    resolve(*fy, GradientAxis::Vertical),
                ) else {
                    return Self::Transparent;
                };
                if radius <= 0.0 {
                    return Self::Transparent;
                }
                let focal_distance = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                if focal_distance >= radius {
                    let scale = radius * (1.0 - 1.0e-9) / focal_distance.max(1.0e-15);
                    fx = cx + (fx - cx) * scale;
                    fy = cy + (fy - cy) * scale;
                }
                Self::Radial {
                    device_to_gradient,
                    center: (cx, cy),
                    focal: (fx, fy),
                    radius,
                    spread: common.spread,
                    stops: common.stops.clone(),
                    opacity,
                }
            }
        }
    }

    fn sample(&self, device_x: f64, device_y: f64) -> [u8; 4] {
        match self {
            Self::Solid(color) => *color,
            Self::Linear {
                device_to_gradient,
                from,
                to,
                spread,
                stops,
                opacity,
            } => {
                let point = device_to_gradient.apply(device_x, device_y);
                let vector = (to.0 - from.0, to.1 - from.1);
                let length_squared = vector.0 * vector.0 + vector.1 * vector.1;
                let t = if length_squared <= 1.0e-15 {
                    1.0
                } else {
                    ((point.0 - from.0) * vector.0 + (point.1 - from.1) * vector.1) / length_squared
                };
                sample_gradient(stops, spread_value(t, *spread), *opacity)
            }
            Self::Radial {
                device_to_gradient,
                center,
                focal,
                radius,
                spread,
                stops,
                opacity,
            } => {
                let point = device_to_gradient.apply(device_x, device_y);
                let direction = (point.0 - focal.0, point.1 - focal.1);
                let a = direction.0 * direction.0 + direction.1 * direction.1;
                let t = if a <= 1.0e-15 {
                    0.0
                } else {
                    let fc = (focal.0 - center.0, focal.1 - center.1);
                    let b = 2.0 * (fc.0 * direction.0 + fc.1 * direction.1);
                    let c = fc.0 * fc.0 + fc.1 * fc.1 - radius * radius;
                    let discriminant = (b * b - 4.0 * a * c).max(0.0).sqrt();
                    let boundary = (-b + discriminant) / (2.0 * a);
                    if boundary > 1.0e-15 {
                        1.0 / boundary
                    } else {
                        0.0
                    }
                };
                sample_gradient(stops, spread_value(t, *spread), *opacity)
            }
            Self::Pattern {
                tile,
                tile_w,
                tile_h,
                device_to_pattern,
                origin,
                size,
                opacity,
            } => {
                let (tw, th) = *size;
                if tw <= 0.0 || th <= 0.0 || *tile_w == 0 || *tile_h == 0 {
                    return [0, 0, 0, 0];
                }
                let (px, py) = device_to_pattern.apply(device_x, device_y);
                // Wrap into the tile rect (rem_euclid keeps negatives in range).
                let u = (px - origin.0).rem_euclid(tw);
                let v = (py - origin.1).rem_euclid(th);
                let ix = (((u / tw) * *tile_w as f64) as usize).min(*tile_w - 1);
                let iy = (((v / th) * *tile_h as f64) as usize).min(*tile_h - 1);
                let i = (iy * *tile_w + ix) * 4;
                let a = (tile[i + 3] as f32 * opacity.clamp(0.0, 1.0))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                [tile[i], tile[i + 1], tile[i + 2], a]
            }
            Self::Transparent => [0, 0, 0, 0],
        }
    }

    fn is_transparent(&self) -> bool {
        matches!(self, Self::Transparent | Self::Solid([_, _, _, 0]))
    }
}

/// Render a pattern tile once and produce a tiling sampler (R9). The tile is
/// bounded to `MAX_PATTERN_TILE_PIXELS`; nested content is rendered through the
/// shape renderer with this pattern removed from the server table, so a pattern
/// whose content references itself (directly or via a cycle) terminates with a
/// transparent paint instead of recursing forever.
#[allow(clippy::too_many_arguments)]
fn build_pattern_sampler(
    def: &PatternDef,
    pattern_id: &str,
    opacity: f32,
    bounds: [f64; 4],
    object_transform: Transform,
    length_bases: SvgLengthBases,
    servers: &PaintServerTable,
) -> PaintSampler {
    let units = def.units.unwrap_or(PatternUnits::ObjectBoundingBox);
    let content_units = def.content_units.unwrap_or(PatternUnits::UserSpaceOnUse);
    let (bw, bh) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);

    // Resolve the tile rect (origin + size) into pattern user space.
    let frac = |len: Option<svg_core::SvgLength>| -> f64 {
        match len {
            Some(l) => match l.unit {
                svg_core::SvgLengthUnit::Percent => l.value / 100.0,
                _ => l.value,
            },
            None => 0.0,
        }
    };
    let user = |len: Option<svg_core::SvgLength>, base: f64| -> f64 {
        len.and_then(|l| l.resolve(svg_core::SvgLengthContext::user_units(base)))
            .unwrap_or(0.0)
    };
    let (tx, ty, tw, th) = match units {
        PatternUnits::ObjectBoundingBox => {
            if bw.abs() <= 1.0e-12 || bh.abs() <= 1.0e-12 {
                return PaintSampler::Transparent;
            }
            (
                bounds[0] + frac(def.x) * bw,
                bounds[1] + frac(def.y) * bh,
                frac(def.width) * bw,
                frac(def.height) * bh,
            )
        }
        PatternUnits::UserSpaceOnUse => (
            user(def.x, length_bases.horizontal),
            user(def.y, length_bases.vertical),
            user(def.width, length_bases.horizontal),
            user(def.height, length_bases.vertical),
        ),
    };
    if tw <= 0.0 || th <= 0.0 {
        return PaintSampler::Transparent;
    }

    let pattern_to_device =
        object_transform.multiply(def.pattern_transform.unwrap_or_else(Transform::identity));
    let Some(device_to_pattern) = pattern_to_device.inverse() else {
        return PaintSampler::Transparent;
    };

    // Tile pixel size: scale the tile rect by the per-axis device scale, capped.
    let sx = (pattern_to_device.a.powi(2) + pattern_to_device.b.powi(2)).sqrt();
    let sy = (pattern_to_device.c.powi(2) + pattern_to_device.d.powi(2)).sqrt();
    let mut tpw = ((tw * sx).round() as usize).clamp(1, MAX_RASTER_DIM as usize);
    let mut tph = ((th * sy).round() as usize).clamp(1, MAX_RASTER_DIM as usize);
    if tpw.saturating_mul(tph) > MAX_PATTERN_TILE_PIXELS {
        let scale = (MAX_PATTERN_TILE_PIXELS as f64 / (tpw as f64 * tph as f64)).sqrt();
        tpw = ((tpw as f64 * scale).floor() as usize).max(1);
        tph = ((tph as f64 * scale).floor() as usize).max(1);
    }

    // content user space -> tile pixel space.
    let content_to_tile_user = if let Some(vb) = def.view_box {
        svg_core::viewbox_transform(vb, [0.0, 0.0, tw, th], def.aspect.unwrap_or_default())
            .unwrap_or_else(Transform::identity)
    } else if content_units == PatternUnits::ObjectBoundingBox {
        Transform::scale(bw, bh)
    } else {
        Transform::identity()
    };
    let content_to_tile_px =
        Transform::scale(tpw as f64 / tw, tph as f64 / th).multiply(content_to_tile_user);

    // Render content into the tile with this pattern removed to break cycles.
    let mut reduced = servers.clone();
    reduced.patterns.remove(pattern_id);
    let mut tile = vec![0u8; tpw * tph * 4];
    {
        let mut target = RasterTarget {
            buf: &mut tile,
            width: tpw,
            height: tph,
            premultiplied: false,
            clip: None,
        };
        render_content_items(&def.items, content_to_tile_px, &reduced, &mut target);
    }

    PaintSampler::Pattern {
        tile,
        tile_w: tpw,
        tile_h: tph,
        device_to_pattern,
        origin: (tx, ty),
        size: (tw, th),
        opacity: opacity.clamp(0.0, 1.0),
    }
}

#[derive(Clone, Copy)]
enum GradientAxis {
    Horizontal,
    Vertical,
    Other,
}

fn resolve_gradient_length(
    length: svg_core::SvgLength,
    units: GradientUnits,
    axis: GradientAxis,
    bases: SvgLengthBases,
) -> Option<f64> {
    if units == GradientUnits::ObjectBoundingBox {
        return match length.unit {
            svg_core::SvgLengthUnit::Number => Some(length.value),
            svg_core::SvgLengthUnit::Percent => Some(length.value / 100.0),
            _ => None,
        };
    }
    let base = match axis {
        GradientAxis::Horizontal => bases.horizontal,
        GradientAxis::Vertical => bases.vertical,
        GradientAxis::Other => bases.other,
    };
    length.resolve(svg_core::SvgLengthContext::user_units(base))
}

fn spread_value(value: f64, spread: GradientSpread) -> f64 {
    match spread {
        GradientSpread::Pad => value.clamp(0.0, 1.0),
        GradientSpread::Repeat => value.rem_euclid(1.0),
        GradientSpread::Reflect => {
            let repeated = value.rem_euclid(2.0);
            if repeated <= 1.0 {
                repeated
            } else {
                2.0 - repeated
            }
        }
    }
}

fn sample_gradient(stops: &[GradientStop], value: f64, opacity: f32) -> [u8; 4] {
    let Some(first) = stops.first() else {
        return [0, 0, 0, 0];
    };
    let (left, right) = if value <= first.offset {
        (first, first)
    } else if let Some(last) = stops.last().filter(|last| value >= last.offset) {
        (last, last)
    } else {
        let right_index = stops
            .partition_point(|stop| stop.offset < value)
            .min(stops.len() - 1);
        (&stops[right_index - 1], &stops[right_index])
    };
    let span = right.offset - left.offset;
    let mix = if span <= 1.0e-15 {
        0.0
    } else {
        ((value - left.offset) / span).clamp(0.0, 1.0)
    };
    let interpolate = |a: u8, b: u8| {
        (a as f64 + (b as f64 - a as f64) * mix)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [
        interpolate(left.color.r, right.color.r),
        interpolate(left.color.g, right.color.g),
        interpolate(left.color.b, right.color.b),
        (interpolate(left.color.a, right.color.a) as f32 * opacity.clamp(0.0, 1.0))
            .round()
            .clamp(0.0, 255.0) as u8,
    ]
}

type Transform = svg_core::Affine2D;

// ---------------------------------------------------------------------------
// SVG Document model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SvgNodeId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SvgSourceSpan {
    start: usize,
    end: usize,
}

impl SvgSourceSpan {
    fn render_source(self, node_id: SvgNodeId) -> SvgRenderSource {
        SvgRenderSource {
            node_id: node_id.0,
            byte_start: self.start,
            byte_end: self.end,
        }
    }
}

#[derive(Clone)]
enum SvgNode {
    Group {
        id: SvgNodeId,
        span: SvgSourceSpan,
        is_viewport: bool,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
    Rect {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Circle {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Ellipse {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Line {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Polyline {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Polygon {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Path {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Text {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
        /// Raw inner markup of the `<text>` element (plain text plus nested
        /// `<tspan>` runs), scanned into glyph runs by the R11 text renderer.
        content: String,
    },
    Definition {
        id: SvgNodeId,
        span: SvgSourceSpan,
        is_symbol: bool,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
    Use {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    StyleSheet {
        id: SvgNodeId,
        span: SvgSourceSpan,
        css: String,
    },
    LinearGradient {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
    RadialGradient {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
    Stop {
        id: SvgNodeId,
        span: SvgSourceSpan,
        attrs: Vec<(String, String)>,
    },
    Unsupported {
        id: SvgNodeId,
        span: SvgSourceSpan,
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
}

struct SvgDoc {
    root_attrs: Vec<(String, String)>,
    viewbox: Option<[f32; 4]>,
    preserve_aspect_ratio: svg_core::SvgPreserveAspectRatio,
    width: f32,
    height: f32,
    nodes: Vec<SvgNode>,
    /// R12 accessibility + recovery metadata gathered during parse.
    title: Option<String>,
    desc: Option<String>,
    foreign_count: usize,
    recovered: usize,
}

struct SvgScene {
    viewbox: Option<[f32; 4]>,
    preserve_aspect_ratio: svg_core::SvgPreserveAspectRatio,
    width: f32,
    height: f32,
    items: Vec<SvgSceneItem>,
    references: SvgReferenceTable,
    stylesheet: svg_core::SvgCssStyleSheet,
    paint_servers: PaintServerTable,
    expanded_use_limit_hit: bool,
    use_cycle_count: usize,
    /// R12 a11y + namespace/recovery metadata surfaced into the render report.
    title: Option<String>,
    desc: Option<String>,
    foreign_count: usize,
    recovered: usize,
}

struct SvgSceneItem {
    node: SvgNode,
    transform: Transform,
    style: Style,
    length_bases: SvgLengthBases,
    skipped_by_unsupported_ancestor: bool,
    /// When `Some`, this group/viewport item opens a compositing/clip layer that
    /// encloses the following items up to a matching `is_layer_end` marker.
    layer: Option<LayerRaw>,
    /// Synthetic marker that closes the most recently opened layer.  Carries no
    /// renderable geometry; the `node`/`transform`/`style` fields are unused.
    is_layer_end: bool,
}

/// Pre-lowering layer description recorded during scene flattening, before the
/// view transform and clip references are resolved into device geometry (which
/// happens in `DisplayList::build`).
#[derive(Clone)]
struct LayerRaw {
    /// `clip-path="url(#id)"` target, if any.
    clip_ref: Option<String>,
    /// `mask="url(#id)"` target, if any (R7).
    mask_ref: Option<String>,
    /// `filter="url(#id)"` target, if any (R7).
    filter_ref: Option<String>,
    /// Scene-space CTM of the element (view transform applied later).
    element_transform: Transform,
    /// Length bases of the element's user space (for clip percentages/objbb).
    length_bases: SvgLengthBases,
    /// `true` when the element is a `<g>` (no single object bounding box for
    /// objectBoundingBox clip units); shapes set this `false`.
    is_group: bool,
    /// Nested-`<svg>` overflow rectangle (x, y, w, h) in parent user space plus
    /// the scene-space transform mapping it to device.
    overflow: Option<([f64; 4], Transform)>,
    /// Group opacity in `0..=1`; `< 1` forces an isolated offscreen.
    opacity: f32,
    /// `isolation: isolate` requested an offscreen regardless of opacity.
    isolate: bool,
    /// `mix-blend-mode` (R10); non-`Normal` composites the isolated offscreen
    /// with the parent using the selected separable blend.
    blend: BlendMode,
    source: SvgRenderSource,
}

#[derive(Clone, Copy)]
struct SvgLengthBases {
    horizontal: f64,
    vertical: f64,
    other: f64,
}

impl SvgLengthBases {
    fn new(horizontal: f64, vertical: f64) -> Self {
        Self {
            horizontal,
            vertical,
            other: ((horizontal * horizontal + vertical * vertical) / 2.0).sqrt(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SvgLocalId {
    xml_id: String,
    node_id: SvgNodeId,
    span: SvgSourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SvgReferenceUse {
    source_id: SvgNodeId,
    source_span: SvgSourceSpan,
    target_id: String,
    resolved: Option<SvgNodeId>,
}

#[derive(Default)]
struct SvgReferenceTable {
    by_xml_id: HashMap<String, SvgNodeId>,
    nodes_by_id: HashMap<SvgNodeId, SvgNode>,
    ordered_ids: Vec<SvgLocalId>,
    uses: Vec<SvgReferenceUse>,
    duplicate_id_count: usize,
    dropped_id_count: usize,
    dropped_use_count: usize,
    external_reference_count: usize,
}

impl SvgScene {
    fn parse(svg_text: &str) -> Option<Self> {
        SvgDoc::parse(svg_text).map(Self::from_doc)
    }

    fn from_doc(doc: SvgDoc) -> Self {
        let references = SvgReferenceTable::build(&doc.nodes);
        let stylesheet = collect_stylesheet(&doc.nodes);
        let root_style = Style::default().inherit_parts("svg", &doc.root_attrs, &stylesheet);
        let paint_servers = PaintServerTable::build(&references, &stylesheet, &root_style);
        let root_length_bases = match doc.viewbox {
            Some([_, _, width, height]) if width > 0.0 && height > 0.0 => {
                SvgLengthBases::new(width as f64, height as f64)
            }
            _ => SvgLengthBases::new(doc.width.max(0.0) as f64, doc.height.max(0.0) as f64),
        };
        let mut items = Vec::new();
        let mut expansion = UseExpansionState::default();
        let mut build = SceneBuildTarget {
            stylesheet: &stylesheet,
            references: &references,
            expansion: &mut expansion,
            items: &mut items,
        };
        Self::build_items(
            &doc.nodes,
            Transform::identity(),
            root_style,
            root_length_bases,
            false,
            &mut build,
        );
        Self {
            viewbox: doc.viewbox,
            preserve_aspect_ratio: doc.preserve_aspect_ratio,
            width: doc.width,
            height: doc.height,
            items,
            references,
            stylesheet,
            paint_servers,
            expanded_use_limit_hit: expansion.limit_hit,
            use_cycle_count: expansion.cycle_count,
            title: doc.title,
            desc: doc.desc,
            foreign_count: doc.foreign_count,
            recovered: doc.recovered,
        }
    }

    fn build_items(
        nodes: &[SvgNode],
        inherited_transform: Transform,
        inherited_style: Style,
        inherited_length_bases: SvgLengthBases,
        skipped_by_unsupported_ancestor: bool,
        build: &mut SceneBuildTarget<'_>,
    ) {
        for node in nodes {
            let attrs = node.attrs();
            let local_style = inherited_style.inherit(node, build.stylesheet);
            let local_transform = attr_get(attrs, "transform")
                .map(Transform::parse_chained)
                .unwrap_or_else(Transform::identity);
            let mut combined_transform = inherited_transform.concat(local_transform);
            let mut child_length_bases = inherited_length_bases;

            match node {
                SvgNode::Group {
                    is_viewport,
                    children,
                    ..
                } => {
                    let pre_viewport_transform = combined_transform;
                    let mut overflow = None;
                    if *is_viewport {
                        let (viewport_transform, viewport_bases) =
                            nested_viewport(attrs, inherited_length_bases);
                        overflow = Some((
                            viewport_rect(attrs, inherited_length_bases),
                            pre_viewport_transform,
                        ));
                        combined_transform = combined_transform.concat(viewport_transform);
                        child_length_bases = viewport_bases;
                    }
                    let layer = layer_for_group(
                        attrs,
                        &local_style,
                        combined_transform,
                        child_length_bases,
                        overflow,
                        node.source(),
                        skipped_by_unsupported_ancestor,
                    );
                    let has_layer = layer.is_some();
                    build.items.push(SvgSceneItem {
                        node: node.shallow(),
                        transform: combined_transform,
                        style: local_style.clone(),
                        length_bases: child_length_bases,
                        skipped_by_unsupported_ancestor,
                        layer,
                        is_layer_end: false,
                    });
                    Self::build_items(
                        children,
                        combined_transform,
                        local_style,
                        child_length_bases,
                        skipped_by_unsupported_ancestor,
                        build,
                    );
                    if has_layer {
                        build.items.push(SvgSceneItem {
                            node: node.shallow(),
                            transform: combined_transform,
                            style: Style::default(),
                            length_bases: child_length_bases,
                            skipped_by_unsupported_ancestor,
                            layer: None,
                            is_layer_end: true,
                        });
                    }
                }
                SvgNode::Definition { children, .. } => {
                    Self::build_items(
                        children,
                        combined_transform,
                        local_style,
                        inherited_length_bases,
                        true,
                        build,
                    );
                }
                // clipPath/mask/filter/marker/pattern definitions never render
                // directly; their children are consumed only when an element
                // references them (R9 flips marker/pattern from diagnosed to
                // rendered, so they no longer emit an unsupported-node command).
                SvgNode::Unsupported { tag, .. }
                    if tag == "clippath"
                        || tag == "mask"
                        || tag == "filter"
                        || tag == "marker"
                        || tag == "pattern" => {}
                SvgNode::StyleSheet { .. }
                | SvgNode::LinearGradient { .. }
                | SvgNode::RadialGradient { .. }
                | SvgNode::Stop { .. } => {}
                SvgNode::Use { .. } => {
                    Self::expand_use(
                        node,
                        combined_transform,
                        local_style,
                        inherited_length_bases,
                        build,
                    );
                }
                SvgNode::Unsupported { children, .. } => {
                    build.items.push(SvgSceneItem {
                        node: node.shallow(),
                        transform: combined_transform,
                        style: local_style.clone(),
                        length_bases: inherited_length_bases,
                        skipped_by_unsupported_ancestor,
                        layer: None,
                        is_layer_end: false,
                    });
                    Self::build_items(
                        children,
                        combined_transform,
                        local_style,
                        inherited_length_bases,
                        true,
                        build,
                    );
                }
                _ => {
                    // Shapes with mask/filter need an isolated offscreen (R7):
                    // wrap them in a layer so EndLayer can post-process.
                    let layer = if skipped_by_unsupported_ancestor {
                        None
                    } else {
                        shape_layer(
                            node.attrs(),
                            combined_transform,
                            inherited_length_bases,
                            node.source(),
                        )
                    };
                    let has_layer = layer.is_some();
                    build.items.push(SvgSceneItem {
                        node: node.shallow(),
                        transform: combined_transform,
                        style: local_style,
                        length_bases: inherited_length_bases,
                        skipped_by_unsupported_ancestor,
                        layer,
                        is_layer_end: false,
                    });
                    if has_layer {
                        build.items.push(SvgSceneItem {
                            node: node.shallow(),
                            transform: combined_transform,
                            style: Style::default(),
                            length_bases: inherited_length_bases,
                            skipped_by_unsupported_ancestor,
                            layer: None,
                            is_layer_end: true,
                        });
                    }
                }
            }
        }
    }

    fn expand_use(
        use_node: &SvgNode,
        inherited_transform: Transform,
        inherited_style: Style,
        inherited_length_bases: SvgLengthBases,
        build: &mut SceneBuildTarget<'_>,
    ) {
        if build.expansion.stack.len() >= MAX_USE_EXPANSION_DEPTH
            || build.expansion.expanded_nodes >= MAX_EXPANDED_USE_NODES
        {
            build.expansion.limit_hit = true;
            return;
        }
        let Some(target) = use_node
            .attrs()
            .iter()
            .find_map(|(key, value)| (key == "href").then_some(value))
            .and_then(|href| href.trim().strip_prefix('#'))
            .and_then(|id| build.references.by_xml_id.get(id))
            .and_then(|id| build.references.nodes_by_id.get(id))
        else {
            return;
        };
        if build.expansion.stack.contains(&target.id()) {
            build.expansion.cycle_count += 1;
            return;
        }

        let x = attr_f32(
            use_node.attrs(),
            "x",
            inherited_length_bases.horizontal,
            0.0,
        ) as f64;
        let y = attr_f32(use_node.attrs(), "y", inherited_length_bases.vertical, 0.0) as f64;
        let mut transform = inherited_transform.concat(Transform::translate(x, y));
        let mut child_bases = inherited_length_bases;
        if let SvgNode::Definition {
            is_symbol: true,
            attrs,
            ..
        } = target
        {
            let width = attr_f32(
                use_node.attrs(),
                "width",
                inherited_length_bases.horizontal,
                parse_view_box(attrs).map_or(0.0, |view| view[2] as f32),
            ) as f64;
            let height = attr_f32(
                use_node.attrs(),
                "height",
                inherited_length_bases.vertical,
                parse_view_box(attrs).map_or(0.0, |view| view[3] as f32),
            ) as f64;
            if let Some(view_box) = parse_view_box(attrs) {
                if let Some(viewport) = svg_core::viewbox_transform(
                    view_box,
                    [0.0, 0.0, width, height],
                    svg_core::parse_preserve_aspect_ratio(
                        attr_get(attrs, "preserveaspectratio").unwrap_or(""),
                    ),
                ) {
                    transform = transform.concat(viewport);
                    child_bases = SvgLengthBases::new(view_box[2], view_box[3]);
                }
            }
        }

        build.expansion.stack.push(target.id());
        build.expansion.expanded_nodes += 1;
        match target {
            SvgNode::Definition { children, .. } | SvgNode::Group { children, .. } => {
                Self::build_items(
                    children,
                    transform,
                    inherited_style,
                    child_bases,
                    false,
                    build,
                );
            }
            _ => Self::build_items(
                std::slice::from_ref(target),
                transform,
                inherited_style,
                child_bases,
                false,
                build,
            ),
        }
        build.expansion.stack.pop();
    }
}

struct SceneBuildTarget<'a> {
    stylesheet: &'a svg_core::SvgCssStyleSheet,
    references: &'a SvgReferenceTable,
    expansion: &'a mut UseExpansionState,
    items: &'a mut Vec<SvgSceneItem>,
}

#[derive(Default)]
struct UseExpansionState {
    stack: Vec<SvgNodeId>,
    expanded_nodes: usize,
    limit_hit: bool,
    cycle_count: usize,
}

impl SvgNode {
    fn id(&self) -> SvgNodeId {
        match self {
            SvgNode::Group { id, .. }
            | SvgNode::Rect { id, .. }
            | SvgNode::Circle { id, .. }
            | SvgNode::Ellipse { id, .. }
            | SvgNode::Line { id, .. }
            | SvgNode::Polyline { id, .. }
            | SvgNode::Polygon { id, .. }
            | SvgNode::Path { id, .. }
            | SvgNode::Text { id, .. }
            | SvgNode::Definition { id, .. }
            | SvgNode::Use { id, .. }
            | SvgNode::StyleSheet { id, .. }
            | SvgNode::LinearGradient { id, .. }
            | SvgNode::RadialGradient { id, .. }
            | SvgNode::Stop { id, .. }
            | SvgNode::Unsupported { id, .. } => *id,
        }
    }

    fn span(&self) -> SvgSourceSpan {
        match self {
            SvgNode::Group { span, .. }
            | SvgNode::Rect { span, .. }
            | SvgNode::Circle { span, .. }
            | SvgNode::Ellipse { span, .. }
            | SvgNode::Line { span, .. }
            | SvgNode::Polyline { span, .. }
            | SvgNode::Polygon { span, .. }
            | SvgNode::Path { span, .. }
            | SvgNode::Text { span, .. }
            | SvgNode::Definition { span, .. }
            | SvgNode::Use { span, .. }
            | SvgNode::StyleSheet { span, .. }
            | SvgNode::LinearGradient { span, .. }
            | SvgNode::RadialGradient { span, .. }
            | SvgNode::Stop { span, .. }
            | SvgNode::Unsupported { span, .. } => *span,
        }
    }

    fn source(&self) -> SvgRenderSource {
        self.span().render_source(self.id())
    }

    fn attrs(&self) -> &[(String, String)] {
        match self {
            SvgNode::Group { attrs, .. }
            | SvgNode::Rect { attrs, .. }
            | SvgNode::Circle { attrs, .. }
            | SvgNode::Ellipse { attrs, .. }
            | SvgNode::Line { attrs, .. }
            | SvgNode::Polyline { attrs, .. }
            | SvgNode::Polygon { attrs, .. }
            | SvgNode::Path { attrs, .. }
            | SvgNode::Text { attrs, .. }
            | SvgNode::Definition { attrs, .. }
            | SvgNode::Use { attrs, .. }
            | SvgNode::LinearGradient { attrs, .. }
            | SvgNode::RadialGradient { attrs, .. }
            | SvgNode::Stop { attrs, .. }
            | SvgNode::Unsupported { attrs, .. } => attrs,
            SvgNode::StyleSheet { .. } => &[],
        }
    }

    fn children(&self) -> Option<&[SvgNode]> {
        match self {
            SvgNode::Group { children, .. }
            | SvgNode::Definition { children, .. }
            | SvgNode::LinearGradient { children, .. }
            | SvgNode::RadialGradient { children, .. }
            | SvgNode::Unsupported { children, .. } => Some(children),
            _ => None,
        }
    }

    fn tag_name(&self) -> &str {
        match self {
            SvgNode::Group {
                is_viewport: true, ..
            } => "svg",
            SvgNode::Group { .. } => "g",
            SvgNode::Rect { .. } => "rect",
            SvgNode::Circle { .. } => "circle",
            SvgNode::Ellipse { .. } => "ellipse",
            SvgNode::Line { .. } => "line",
            SvgNode::Polyline { .. } => "polyline",
            SvgNode::Polygon { .. } => "polygon",
            SvgNode::Path { .. } => "path",
            SvgNode::Text { .. } => "text",
            SvgNode::Definition {
                is_symbol: true, ..
            } => "symbol",
            SvgNode::Definition { .. } => "defs",
            SvgNode::Use { .. } => "use",
            SvgNode::StyleSheet { .. } => "style",
            SvgNode::LinearGradient { .. } => "lineargradient",
            SvgNode::RadialGradient { .. } => "radialgradient",
            SvgNode::Stop { .. } => "stop",
            SvgNode::Unsupported { tag, .. } => tag,
        }
    }

    fn shallow(&self) -> Self {
        match self {
            SvgNode::Group {
                id,
                span,
                is_viewport,
                attrs,
                ..
            } => SvgNode::Group {
                id: *id,
                span: *span,
                is_viewport: *is_viewport,
                attrs: attrs.clone(),
                children: Vec::new(),
            },
            SvgNode::Rect { id, span, attrs } => SvgNode::Rect {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Circle { id, span, attrs } => SvgNode::Circle {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Ellipse { id, span, attrs } => SvgNode::Ellipse {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Line { id, span, attrs } => SvgNode::Line {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Polyline { id, span, attrs } => SvgNode::Polyline {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Polygon { id, span, attrs } => SvgNode::Polygon {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Path { id, span, attrs } => SvgNode::Path {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Text {
                id,
                span,
                attrs,
                content,
            } => SvgNode::Text {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
                content: content.clone(),
            },
            SvgNode::Definition {
                id,
                span,
                is_symbol,
                attrs,
                ..
            } => SvgNode::Definition {
                id: *id,
                span: *span,
                is_symbol: *is_symbol,
                attrs: attrs.clone(),
                children: Vec::new(),
            },
            SvgNode::Use { id, span, attrs } => SvgNode::Use {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::StyleSheet { id, span, css } => SvgNode::StyleSheet {
                id: *id,
                span: *span,
                css: css.clone(),
            },
            SvgNode::LinearGradient {
                id, span, attrs, ..
            } => SvgNode::LinearGradient {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
                children: Vec::new(),
            },
            SvgNode::RadialGradient {
                id, span, attrs, ..
            } => SvgNode::RadialGradient {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
                children: Vec::new(),
            },
            SvgNode::Stop { id, span, attrs } => SvgNode::Stop {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
            },
            SvgNode::Unsupported {
                id,
                span,
                tag,
                attrs,
                ..
            } => SvgNode::Unsupported {
                id: *id,
                span: *span,
                tag: tag.clone(),
                attrs: attrs.clone(),
                children: Vec::new(),
            },
        }
    }
}

impl SvgReferenceTable {
    fn build(nodes: &[SvgNode]) -> Self {
        let mut table = Self::default();
        table.collect_ids(nodes);
        table.collect_uses(nodes);
        table
    }

    fn collect_ids(&mut self, nodes: &[SvgNode]) {
        for node in nodes {
            self.nodes_by_id.insert(node.id(), node.clone());
            if let Some(xml_id) = attr_get(node.attrs(), "id").filter(|id| !id.is_empty()) {
                if self.by_xml_id.contains_key(xml_id) {
                    self.duplicate_id_count += 1;
                } else if self.ordered_ids.len() >= MAX_LOCAL_IDS {
                    self.dropped_id_count += 1;
                } else {
                    let local = SvgLocalId {
                        xml_id: xml_id.to_owned(),
                        node_id: node.id(),
                        span: node.span(),
                    };
                    self.by_xml_id.insert(local.xml_id.clone(), local.node_id);
                    self.ordered_ids.push(local);
                }
            }
            if let Some(children) = node.children() {
                self.collect_ids(children);
            }
        }
    }

    fn collect_uses(&mut self, nodes: &[SvgNode]) {
        for node in nodes {
            self.external_reference_count += external_reference_count(node.attrs());
            for target_id in local_reference_targets(node.attrs()) {
                if self.uses.len() >= MAX_LOCAL_REFERENCE_USES {
                    self.dropped_use_count += 1;
                    continue;
                }
                self.uses.push(SvgReferenceUse {
                    source_id: node.id(),
                    source_span: node.span(),
                    resolved: self.by_xml_id.get(&target_id).copied(),
                    target_id,
                });
            }
            if let Some(children) = node.children() {
                self.collect_uses(children);
            }
        }
    }

    fn report_into(&self, report: &mut SvgRenderReport) {
        if self.duplicate_id_count > 0 {
            report.warning(
                "reference.duplicate_id",
                format!(
                    "{} duplicate SVG id value(s) ignored; first occurrence wins",
                    self.duplicate_id_count
                ),
            );
        }
        if self.dropped_id_count > 0 {
            report.warning(
                "limit.reference_ids",
                format!(
                    "{} SVG id value(s) exceeded the bounded local-id table",
                    self.dropped_id_count
                ),
            );
        }
        if self.dropped_use_count > 0 {
            report.warning(
                "limit.reference_uses",
                format!(
                    "{} local reference use(s) exceeded the bounded reference-use table",
                    self.dropped_use_count
                ),
            );
        }
        for reference in self
            .uses
            .iter()
            .filter(|reference| reference.resolved.is_none())
        {
            report.warning_at(
                "reference.unresolved",
                format!(
                    "local SVG reference '#{}' did not resolve",
                    reference.target_id
                ),
                Some(reference.source_span.render_source(reference.source_id)),
            );
        }
    }
}

fn collect_stylesheet(nodes: &[SvgNode]) -> svg_core::SvgCssStyleSheet {
    fn visit(nodes: &[SvgNode], css: &mut String, truncated: &mut bool) {
        for node in nodes {
            if let SvgNode::StyleSheet { css: block, .. } = node {
                if css.len() + block.len() <= MAX_STYLE_BYTES {
                    css.push_str(block);
                    css.push('\n');
                } else {
                    *truncated = true;
                }
            }
            if let Some(children) = node.children() {
                visit(children, css, truncated);
            }
        }
    }

    let mut css = String::new();
    let mut truncated = false;
    visit(nodes, &mut css, &mut truncated);
    let mut sheet = svg_core::parse_css_stylesheet(&css, MAX_CSS_RULES, MAX_CSS_DECLARATIONS);
    if truncated {
        sheet.dropped_rule_count += 1;
    }
    sheet
}

fn report_stylesheet(sheet: &svg_core::SvgCssStyleSheet, report: &mut SvgRenderReport) {
    if sheet.unsupported_selector_count > 0 {
        report.unsupported_at(
            "complex CSS selector",
            format!(
                "{} selector(s) exceeded the supported element/class/id/grouped tier",
                sheet.unsupported_selector_count
            ),
            None,
        );
    }
    if sheet.malformed_rule_count > 0 || sheet.dropped_declaration_count > 0 {
        report.warning(
            "css.malformed_rule",
            "malformed or unsupported CSS declarations were ignored",
        );
    }
    if sheet.dropped_rule_count > 0 {
        report.warning(
            "limit.css_rules",
            "CSS rules exceeded renderer byte, rule, or declaration limits",
        );
    }
}

fn local_reference_targets(attrs: &[(String, String)]) -> Vec<String> {
    let mut targets = Vec::new();
    for (key, value) in attrs {
        let value = value.trim();
        if key == "href" {
            if let Some(target) = value.strip_prefix('#').filter(|target| !target.is_empty()) {
                targets.push(target.to_owned());
            }
        }

        let lower = value.to_ascii_lowercase();
        let mut offset = 0;
        while let Some(start) = lower[offset..].find("url(#") {
            let start = offset + start;
            let rest = &value[start..];
            let after = &rest[5..];
            let Some(end) = after.find(')') else {
                break;
            };
            let target = after[..end].trim().trim_matches(['"', '\'']);
            if !target.is_empty() {
                targets.push(target.to_owned());
            }
            offset = start + 5 + end + 1;
        }
    }
    targets
}

fn external_reference_count(attrs: &[(String, String)]) -> usize {
    let mut count = 0;
    for (key, value) in attrs {
        let value = value.trim();
        if key == "href"
            && !value.is_empty()
            && !value.starts_with('#')
            && !value.to_ascii_lowercase().starts_with("data:")
        {
            count += 1;
        }

        let lower = value.to_ascii_lowercase();
        let mut offset = 0;
        while let Some(start) = lower[offset..].find("url(") {
            let start = offset + start;
            let rest = &value[start..];
            let after = &rest[4..];
            let Some(end) = after.find(')') else {
                break;
            };
            let target = after[..end].trim().trim_matches(['"', '\'']);
            if !target.is_empty() && !target.starts_with('#') {
                count += 1;
            }
            offset = start + 4 + end + 1;
        }
    }
    count
}

impl SvgDoc {
    fn parse(svg_text: &str) -> Option<Self> {
        let mut parser = XmlParser {
            s: svg_text,
            pos: 0,
            next_node_id: 0,
            ns_stack: Vec::new(),
            foreign_count: 0,
            recovered: 0,
            title: None,
            desc: None,
        };
        let all_nodes = parser.parse_nodes();
        let title = parser.title.clone();
        let desc = parser.desc.clone();
        let foreign_count = parser.foreign_count;
        let recovered = parser.recovered;

        // Find the SVG root node and extract its attributes
        let (root_attrs, root_children) = all_nodes.into_iter().find_map(|n| {
            if let SvgNode::Group {
                attrs, children, ..
            } = n
            {
                Some((attrs, children))
            } else {
                None
            }
        })?;

        let attr = |key: &str| -> Option<&str> {
            root_attrs
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v.as_str())
        };

        let viewbox = attr("viewbox").and_then(|s| {
            let v = svg_core::parse_numbers(s);
            if v.len() >= 4 {
                Some([v[0] as f32, v[1] as f32, v[2] as f32, v[3] as f32])
            } else {
                None
            }
        });
        let preserve_aspect_ratio =
            svg_core::parse_preserve_aspect_ratio(attr("preserveaspectratio").unwrap_or(""));

        let width = attr("width")
            .and_then(|value| {
                svg_core::resolve_length(value, svg_core::SvgLengthContext::user_units(100.0))
            })
            .unwrap_or(0.0) as f32;
        let height = attr("height")
            .and_then(|value| {
                svg_core::resolve_length(value, svg_core::SvgLengthContext::user_units(100.0))
            })
            .unwrap_or(0.0) as f32;

        // Root aria-label is an a11y fallback when no <title> child is present.
        let title = title.or_else(|| {
            attr("aria-label")
                .filter(|v| !v.trim().is_empty())
                .map(bounded_a11y_text)
        });

        Some(SvgDoc {
            root_attrs,
            viewbox,
            preserve_aspect_ratio,
            width,
            height,
            nodes: root_children,
            title,
            desc,
            foreign_count,
            recovered,
        })
    }
}

// ---------------------------------------------------------------------------
// Minimal XML parser
// ---------------------------------------------------------------------------

/// R12: maximum a11y metadata text length kept (`<title>`/`<desc>`).
const MAX_A11Y_TEXT: usize = 1_024;
/// R12: maximum nested xmlns scope frames tracked (bounds the namespace stack).
const MAX_NS_DEPTH: usize = 256;

/// One xmlns scope frame (R12). `default` is the no-prefix namespace in effect;
/// `prefixes` maps declared `xmlns:p` prefixes to their namespace token.
#[derive(Clone, Default)]
struct NsFrame {
    default: Namespace,
    prefixes: Vec<(String, Namespace)>,
}

/// The small set of namespaces the renderer understands (R12). Everything else
/// is `Foreign` and skipped-with-diagnostic rather than mis-parsed.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Namespace {
    #[default]
    Svg,
    Xlink,
    Foreign,
}

fn classify_namespace(uri: &str) -> Namespace {
    let u = uri.trim();
    if u == "http://www.w3.org/2000/svg" || u.is_empty() {
        Namespace::Svg
    } else if u == "http://www.w3.org/1999/xlink" {
        Namespace::Xlink
    } else {
        Namespace::Foreign
    }
}

/// Apply any `xmlns` / `xmlns:prefix` declarations in a raw open-tag header to a
/// namespace scope frame (R12). Bounded: a malformed header simply stops the
/// scan; prefix names are case-sensitive, the `xmlns` keyword is not.
fn apply_xmlns(header: &str, frame: &mut NsFrame) {
    let bytes = header.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len()
            && (bytes[i].is_ascii_whitespace() || bytes[i] == b'<' || bytes[i] == b'/')
        {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'=' || b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                break;
            }
            i += 1;
        }
        if i == key_start {
            i += 1;
            continue;
        }
        let key = &header[key_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = match bytes.get(i) {
            Some(&q @ (b'"' | b'\'')) => {
                i += 1;
                let vs = i;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
                let v = &header[vs..i.min(header.len())];
                if i < bytes.len() {
                    i += 1;
                }
                v
            }
            _ => "",
        };
        let key_lower = key.to_ascii_lowercase();
        if key_lower == "xmlns" {
            frame.default = classify_namespace(value);
        } else if let Some(pfx) = key.get(..6).map(|h| h.eq_ignore_ascii_case("xmlns:")) {
            if pfx {
                let prefix = key[6..].to_owned();
                let ns = classify_namespace(value);
                if frame.prefixes.len() < MAX_NS_DEPTH {
                    frame.prefixes.push((prefix, ns));
                }
            }
        }
    }
}

/// Collapse whitespace and truncate a11y text to `MAX_A11Y_TEXT` chars (R12).
fn bounded_a11y_text(raw: &str) -> String {
    let collapsed = collapse_text_whitespace(&unescape_xml(raw));
    if collapsed.chars().count() > MAX_A11Y_TEXT {
        collapsed.chars().take(MAX_A11Y_TEXT).collect()
    } else {
        collapsed
    }
}

struct XmlParser<'a> {
    s: &'a str,
    pos: usize,
    next_node_id: u32,
    /// R12 xmlns scope stack (innermost last).
    ns_stack: Vec<NsFrame>,
    /// R12 count of foreign-namespace elements skipped.
    foreign_count: usize,
    /// R12 count of recovered malformed constructs (mismatched/unclosed tags).
    recovered: usize,
    /// R12 first `<title>` / `<desc>` text encountered (bounded length).
    title: Option<String>,
    desc: Option<String>,
}

impl<'a> XmlParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s.as_bytes()[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.pos).copied()
    }

    fn consume(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.s.len());
    }

    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.pos..].starts_with(pat)
    }

    fn find_from(&self, pat: &str) -> Option<usize> {
        self.s[self.pos..].find(pat).map(|i| self.pos + i)
    }

    fn consume_until(&mut self, pat: &str) {
        if let Some(idx) = self.s[self.pos..].find(pat) {
            self.pos += idx;
        } else {
            self.pos = self.s.len();
        }
    }

    fn parse_nodes(&mut self) -> Vec<SvgNode> {
        let mut nodes = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.s.len() {
                break;
            }
            // End tag — let parent consume it
            if self.starts_with("</") {
                break;
            }
            // Comment
            if self.starts_with("<!--") {
                self.consume(4);
                self.consume_until("-->");
                self.consume(3);
                continue;
            }
            // Processing instruction or DOCTYPE
            if self.starts_with("<?") || self.starts_with("<!") {
                self.consume_until(">");
                self.consume(1);
                continue;
            }
            if self.starts_with("<") {
                if let Some(node) = self.parse_element() {
                    nodes.push(node);
                }
                continue;
            }
            // Text node — skip
            self.consume_until("<");
        }
        nodes
    }

    fn parse_element(&mut self) -> Option<SvgNode> {
        let element_start = self.pos;
        self.consume(1); // '<'
        self.skip_ws();

        // Tag name
        let name_start = self.pos;
        while self.pos < self.s.len() {
            let b = self.s.as_bytes()[self.pos];
            if b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                break;
            }
            self.pos += 1;
        }
        let tag_raw = &self.s[name_start..self.pos];
        // Split a namespace prefix (e.g. "svg:rect" → prefix "svg", local "rect").
        let (prefix, local) = match tag_raw.rfind(':') {
            Some(i) => (Some(tag_raw[..i].to_owned()), &tag_raw[i + 1..]),
            None => (None, tag_raw),
        };
        let tag = local.to_lowercase();
        let header_start = self.pos;

        // Attributes
        let mut raw_attrs: Vec<(String, String)> = Vec::new();
        let mut self_closing = false;
        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(b'>') => {
                    self.consume(1);
                    break;
                }
                Some(b'/') => {
                    self.consume(1);
                    self.skip_ws();
                    if self.peek() == Some(b'>') {
                        self.consume(1);
                    }
                    self_closing = true;
                    break;
                }
                _ => {
                    if let Some((k, v)) = self.parse_attr() {
                        raw_attrs.push((k, v));
                    } else {
                        // Skip unexpected char
                        self.consume(1);
                    }
                }
            }
        }

        // R12: resolve this element's namespace within the xmlns scope, then
        // push a scope frame (with any xmlns declared here) for its children.
        let header = &self.s[header_start..self.pos];
        let mut frame = self.ns_stack.last().cloned().unwrap_or_default();
        apply_xmlns(header, &mut frame);
        let elem_ns = match &prefix {
            None => frame.default,
            Some(p) if p.eq_ignore_ascii_case("svg") => frame
                .prefixes
                .iter()
                .rev()
                .find(|(k, _)| k == p)
                .map(|(_, ns)| *ns)
                .unwrap_or(Namespace::Svg),
            Some(p) => frame
                .prefixes
                .iter()
                .rev()
                .find(|(k, _)| k == p)
                .map(|(_, ns)| *ns)
                .unwrap_or(Namespace::Foreign),
        };
        let foreign = elem_ns != Namespace::Svg;
        if self.ns_stack.len() < MAX_NS_DEPTH {
            self.ns_stack.push(frame);
        }

        let is_meta = tag == "title" || tag == "desc";
        let is_container = is_container_tag(&tag) && !foreign;
        let is_text = (tag == "text" || tag == "tspan") && !foreign;
        let is_style = tag == "style" && !foreign;
        let id = SvgNodeId(self.next_node_id);
        self.next_node_id = self.next_node_id.saturating_add(1);

        let mut text_content = String::new();
        // R12: foreign-namespace element or <title>/<desc> — consume the subtree
        // (balanced) and produce no renderable node.
        if (foreign || is_meta) && !self_closing && !is_text && !is_style {
            let inner_start = self.pos;
            // Balance nested elements so we land on the matching close tag.
            let _ = self.parse_nodes();
            let inner = self.s[inner_start..self.pos].to_owned();
            self.consume_close_tag(&tag);
            self.ns_stack.pop();
            if foreign {
                self.foreign_count += 1;
            } else if tag == "title" && self.title.is_none() {
                self.title = Some(bounded_a11y_text(&strip_tags(&inner)));
            } else if tag == "desc" && self.desc.is_none() {
                self.desc = Some(bounded_a11y_text(&strip_tags(&inner)));
            }
            return None;
        }
        if (foreign || is_meta) && self_closing {
            self.ns_stack.pop();
            if foreign {
                self.foreign_count += 1;
            }
            return None;
        }

        let children = if !self_closing && is_style {
            if let Some(relative) = self.s[self.pos..].to_ascii_lowercase().find("</style") {
                let end = self.pos + relative;
                text_content = self.s[self.pos..end].to_owned();
                self.pos = end;
                self.consume(2);
                self.consume_until(">");
                self.consume(1);
            } else {
                self.pos = self.s.len();
            }
            Vec::new()
        } else if !self_closing && is_container {
            let ch = self.parse_nodes();
            // R12: consume the close tag, recovering from mismatch/unclosed.
            if self.starts_with("</") {
                self.consume_close_tag(&tag);
            } else {
                self.recovered += 1; // unclosed container element
            }
            ch
        } else if !self_closing && is_text {
            // Capture the raw inner markup (plain text + nested <tspan> runs)
            // until the MATCHING close tag, so `<text>a<tspan>b</tspan></text>`
            // is not cut at the first `</` (R11 raster text needs the content).
            let close = format!("</{tag}");
            if let Some(relative) = self.s[self.pos..].to_ascii_lowercase().find(&close) {
                let end = self.pos + relative;
                text_content = self.s[self.pos..end].to_owned();
                self.pos = end;
                self.consume(2 + tag.len());
                self.consume_until(">");
                self.consume(1);
            } else {
                // Unterminated text element — consume the rest, render nothing.
                self.pos = self.s.len();
            }
            Vec::new()
        } else if !self_closing {
            // Non-container element — consume until close tag or next element
            // (SVG elements like rect/circle may be written without self-closing slash)
            if let Some(end_pos) = self.find_from(&format!("</{tag}")) {
                if let Some(next_open) = self.find_from("<") {
                    if next_open < end_pos {
                        // Another element starts before our close tag — don't consume
                    } else {
                        self.pos = end_pos;
                        self.consume(2 + tag.len()); // </tagname
                        self.skip_ws();
                        if self.peek() == Some(b'>') {
                            self.consume(1);
                        }
                    }
                }
            }
            Vec::new()
        } else {
            Vec::new()
        };

        self.ns_stack.pop();
        let span = SvgSourceSpan {
            start: element_start,
            end: self.pos,
        };
        self.make_node(id, span, &tag, raw_attrs, children, text_content)
    }

    /// R12: consume a `</name>` close tag, counting a recovery when its name
    /// does not match the element it is closing.
    fn consume_close_tag(&mut self, expected: &str) {
        self.consume(2); // "</"
        self.skip_ws();
        let name_start = self.pos;
        while self.pos < self.s.len() {
            let b = self.s.as_bytes()[self.pos];
            if b.is_ascii_whitespace() || b == b'>' {
                break;
            }
            self.pos += 1;
        }
        let raw = &self.s[name_start..self.pos];
        let local = raw.rsplit(':').next().unwrap_or(raw).to_ascii_lowercase();
        if local != expected {
            self.recovered += 1;
        }
        self.consume_until(">");
        self.consume(1);
    }

    fn parse_attr(&mut self) -> Option<(String, String)> {
        self.skip_ws();
        // Read key
        let key_start = self.pos;
        while self.pos < self.s.len() {
            let b = self.s.as_bytes()[self.pos];
            if b == b'=' || b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                break;
            }
            self.pos += 1;
        }
        let key_raw = self.s[key_start..self.pos].to_lowercase();
        if key_raw.is_empty() {
            return None;
        }
        // Strip namespace (xmlns:xlink → xlink, xlink:href → href, etc.)
        let key = key_raw
            .rfind(':')
            .map(|i| key_raw[i + 1..].to_string())
            .unwrap_or(key_raw);

        self.skip_ws();
        if self.peek() != Some(b'=') {
            // Boolean attr
            return Some((key, String::new()));
        }
        self.consume(1); // '='
        self.skip_ws();

        let quote = self.peek()?;
        if quote == b'"' || quote == b'\'' {
            self.consume(1);
            let val_start = self.pos;
            while self.pos < self.s.len() && self.s.as_bytes()[self.pos] != quote {
                self.pos += 1;
            }
            let val = unescape_xml(&self.s[val_start..self.pos]);
            self.consume(1);
            Some((key, val))
        } else {
            let val_start = self.pos;
            while self.pos < self.s.len() {
                let b = self.s.as_bytes()[self.pos];
                if b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                    break;
                }
                self.pos += 1;
            }
            Some((key, self.s[val_start..self.pos].to_string()))
        }
    }

    fn make_node(
        &self,
        id: SvgNodeId,
        span: SvgSourceSpan,
        tag: &str,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
        text_content: String,
    ) -> Option<SvgNode> {
        match tag {
            "svg" | "g" => Some(SvgNode::Group {
                id,
                span,
                is_viewport: tag == "svg",
                attrs,
                children,
            }),
            "rect" => Some(SvgNode::Rect { id, span, attrs }),
            "circle" => Some(SvgNode::Circle { id, span, attrs }),
            "ellipse" => Some(SvgNode::Ellipse { id, span, attrs }),
            "line" => Some(SvgNode::Line { id, span, attrs }),
            "polyline" => Some(SvgNode::Polyline { id, span, attrs }),
            "polygon" => Some(SvgNode::Polygon { id, span, attrs }),
            "path" => Some(SvgNode::Path { id, span, attrs }),
            "text" | "tspan" => Some(SvgNode::Text {
                id,
                span,
                attrs,
                content: text_content,
            }),
            "defs" | "symbol" => Some(SvgNode::Definition {
                id,
                span,
                is_symbol: tag == "symbol",
                attrs,
                children,
            }),
            "use" => Some(SvgNode::Use { id, span, attrs }),
            "style" => Some(SvgNode::StyleSheet {
                id,
                span,
                css: text_content,
            }),
            "lineargradient" => Some(SvgNode::LinearGradient {
                id,
                span,
                attrs,
                children,
            }),
            "radialgradient" => Some(SvgNode::RadialGradient {
                id,
                span,
                attrs,
                children,
            }),
            "stop" => Some(SvgNode::Stop { id, span, attrs }),
            tag if unsupported_tag_feature(tag).is_some() => Some(SvgNode::Unsupported {
                id,
                span,
                tag: tag.to_owned(),
                attrs,
                children,
            }),
            _ => None,
        }
    }
}

fn is_container_tag(tag: &str) -> bool {
    matches!(
        tag,
        "g" | "svg"
            | "defs"
            | "symbol"
            | "clippath"
            | "mask"
            | "filter"
            | "marker"
            | "lineargradient"
            | "radialgradient"
            | "pattern"
            | "foreignobject"
            | "style"
            | "switch"
            | "femerge"
            | "fecomponenttransfer"
    )
}

fn unsupported_tag_feature(tag: &str) -> Option<(&'static str, &'static str)> {
    match tag {
        "clippath" => Some((
            "clipPath",
            "clip paths are diagnosed but not applied in raster output yet",
        )),
        "mask" => Some(("mask", "masks are diagnosed but not applied in raster output yet")),
        "filter" => Some((
            "filter",
            "filters are diagnosed but not evaluated in raster output yet",
        )),
        "marker" => Some((
            "marker",
            "markers are diagnosed but not drawn on stroked paths yet",
        )),
        "pattern" => Some(("pattern", "patterns are diagnosed but not rasterized yet")),
        "image" => Some((
            "image",
            "PNG data: images are decoded; JPEG and other formats or external sources are diagnosed",
        )),
        "textpath" => Some(("textPath", "textPath is preserved in source but not rasterized yet")),
        "foreignobject" => Some((
            "foreignObject",
            "foreignObject content is rejected from the secure static renderer profile",
        )),
        "animate" | "animatetransform" | "animatemotion" | "set" | "mpath" => {
            Some(("animation", "animation elements are ignored by the static renderer"))
        }
        "switch" => Some((
            "switch",
            "switch conditional processing is not implemented in raster mode yet",
        )),
        // Filter primitives are retained in the tree so a referenced <filter> can
        // read them; they render only inside a supported filter (R7).
        t if t.starts_with("fe") => Some((
            "filter primitive",
            "filter primitive elements render only inside a supported filter",
        )),
        _ => None,
    }
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn attr_get<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn attr_f32(attrs: &[(String, String)], key: &str, percent_base: f64, default: f32) -> f32 {
    attr_get(attrs, key)
        .and_then(|value| {
            svg_core::resolve_length(value, svg_core::SvgLengthContext::user_units(percent_base))
        })
        .map(|value| value as f32)
        .unwrap_or(default)
}

fn parse_view_box(attrs: &[(String, String)]) -> Option<[f64; 4]> {
    let values = svg_core::parse_numbers(attr_get(attrs, "viewbox")?);
    (values.len() >= 4).then_some([values[0], values[1], values[2], values[3]])
}

fn nested_viewport(
    attrs: &[(String, String)],
    parent_bases: SvgLengthBases,
) -> (Transform, SvgLengthBases) {
    let x = attr_f32(attrs, "x", parent_bases.horizontal, 0.0) as f64;
    let y = attr_f32(attrs, "y", parent_bases.vertical, 0.0) as f64;
    let width = attr_f32(
        attrs,
        "width",
        parent_bases.horizontal,
        parent_bases.horizontal as f32,
    )
    .max(0.0) as f64;
    let height = attr_f32(
        attrs,
        "height",
        parent_bases.vertical,
        parent_bases.vertical as f32,
    )
    .max(0.0) as f64;

    if let Some(view_box) = parse_view_box(attrs) {
        let aspect_ratio = svg_core::parse_preserve_aspect_ratio(
            attr_get(attrs, "preserveaspectratio").unwrap_or(""),
        );
        if let Some(transform) =
            svg_core::viewbox_transform(view_box, [x, y, width, height], aspect_ratio)
        {
            return (
                transform,
                SvgLengthBases::new(view_box[2].abs(), view_box[3].abs()),
            );
        }
    }

    (
        Transform::translate(x, y),
        SvgLengthBases::new(width, height),
    )
}

/// Nested-`<svg>` overflow clip rectangle (x, y, width, height) in parent
/// coordinates, matching the geometry `nested_viewport` maps.
fn viewport_rect(attrs: &[(String, String)], parent_bases: SvgLengthBases) -> [f64; 4] {
    let x = attr_f32(attrs, "x", parent_bases.horizontal, 0.0) as f64;
    let y = attr_f32(attrs, "y", parent_bases.vertical, 0.0) as f64;
    let width = attr_f32(
        attrs,
        "width",
        parent_bases.horizontal,
        parent_bases.horizontal as f32,
    )
    .max(0.0) as f64;
    let height = attr_f32(
        attrs,
        "height",
        parent_bases.vertical,
        parent_bases.vertical as f32,
    )
    .max(0.0) as f64;
    [x, y, width, height]
}

/// Resolved `clip-path="url(#id)"` reference id, if present and local.
fn clip_path_ref(attrs: &[(String, String)]) -> Option<String> {
    final_style_property(attrs, "clip-path")
        .and_then(local_url_reference)
        .map(str::to_owned)
}

/// Whether `isolation: isolate` requests an isolated compositing group.
fn parse_isolation(attrs: &[(String, String)]) -> bool {
    final_style_property(attrs, "isolation")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("isolate"))
}

/// Decide whether a `<g>`/nested-`<svg>` needs a compositing/clip layer, and
/// describe it.  Returns `None` for plain containers so the common path stays a
/// flat draw with no offscreen or clip overhead.
#[allow(clippy::too_many_arguments)]
fn layer_for_group(
    attrs: &[(String, String)],
    style: &Style,
    element_transform: Transform,
    length_bases: SvgLengthBases,
    overflow: Option<([f64; 4], Transform)>,
    source: SvgRenderSource,
    skipped_by_unsupported_ancestor: bool,
) -> Option<LayerRaw> {
    if skipped_by_unsupported_ancestor || !style.visible {
        return None;
    }
    let clip_ref = clip_path_ref(attrs);
    let mask_ref = local_attr_ref(attrs, "mask");
    let filter_ref = local_attr_ref(attrs, "filter");
    let opacity = style.opacity.clamp(0.0, 1.0);
    let isolate = parse_isolation(attrs);
    let blend = parse_mix_blend_mode(attrs);
    if clip_ref.is_none()
        && mask_ref.is_none()
        && filter_ref.is_none()
        && overflow.is_none()
        && opacity >= 1.0
        && !isolate
        && blend == BlendMode::Normal
    {
        return None;
    }
    Some(LayerRaw {
        clip_ref,
        mask_ref,
        filter_ref,
        element_transform,
        length_bases,
        is_group: true,
        overflow,
        opacity,
        isolate,
        blend,
        source,
    })
}

/// `mix-blend-mode` (R10); unrecognised values fall back to `Normal`.
fn parse_mix_blend_mode(attrs: &[(String, String)]) -> BlendMode {
    final_style_property(attrs, "mix-blend-mode")
        .and_then(parse_blend_mode)
        .unwrap_or(BlendMode::Normal)
}

/// A `<g>`/shape `mask`/`filter` reference resolved to a local `url(#id)` target.
fn local_attr_ref(attrs: &[(String, String)], key: &str) -> Option<String> {
    local_url_reference(attr_get(attrs, key)?).map(ToOwned::to_owned)
}

/// Build a LayerRaw for a *shape* that carries `mask`/`filter` (which need an
/// isolated offscreen). Clip and opacity stay on the shape's own draw path.
fn shape_layer(
    attrs: &[(String, String)],
    element_transform: Transform,
    length_bases: SvgLengthBases,
    source: SvgRenderSource,
) -> Option<LayerRaw> {
    let mask_ref = local_attr_ref(attrs, "mask");
    let filter_ref = local_attr_ref(attrs, "filter");
    let blend = parse_mix_blend_mode(attrs);
    if mask_ref.is_none() && filter_ref.is_none() && blend == BlendMode::Normal {
        return None;
    }
    Some(LayerRaw {
        clip_ref: None,
        mask_ref,
        filter_ref,
        element_transform,
        length_bases,
        is_group: false,
        overflow: None,
        opacity: 1.0,
        isolate: false,
        blend,
        source,
    })
}

// ---------------------------------------------------------------------------
// R11: raster text (vector-outline snapshot via a bundled stroked font)
// ---------------------------------------------------------------------------
//
// Image-mode text rendering uses an embedded public-domain stroked vector font
// (Hershey "simplex", Allen V. Hershey, US Naval Weapons Laboratory — public
// domain).  Coverage is ASCII 32..=126 only; every other character renders as
// a tofu box with a diagnostic.  Glyph metrics: y-up, baseline at 0, capital
// height 21 units, descender to -7; we treat 30 units as one em (cap height =
// 0.70 em).  Each glyph is a set of polylines that are laid out in user space
// and stroked through the existing stroke pipeline (so clips, masks, filters,
// opacity, and gradient paint all apply to text exactly like to shapes).
//
// This is the *visual-fidelity snapshot* path: component import (svg_import.rs,
// R6) keeps producing editable labels and is unchanged; choosing Image mode is
// the opt-in to this raster snapshot, with the original source preserved.

/// One em in glyph units (capital height 21 → 0.70 em).
const HERSHEY_EM_UNITS: f64 = 30.0;
/// Glyph stroke width in glyph units (2 units ≈ font_size / 15).
const HERSHEY_STROKE_UNITS: f64 = 2.0;
/// Maximum glyphs laid out per `<text>` element (R11).
const MAX_TEXT_GLYPHS: usize = 4_096;

/// Hershey simplex strokes for ASCII 32..=126.  Each entry is
/// `[advance_width, x0, y0, x1, y1, ...]` with `(-1, -1)` pairs as pen-up
/// markers (no real vertex has x = -1).  `^` is a simplified caret.
const HERSHEY_SIMPLEX: [&[i8]; 95] = [
    &[16],                                                    // space
    &[10, 5, 21, 5, 7, -1, -1, 5, 2, 4, 1, 5, 0, 6, 1, 5, 2], // !
    &[16, 4, 21, 4, 14, -1, -1, 12, 21, 12, 14],              // "
    &[
        21, 11, 25, 4, -7, -1, -1, 17, 25, 10, -7, -1, -1, 4, 12, 18, 12, -1, -1, 3, 6, 17, 6,
    ], // #
    &[
        20, 8, 25, 8, -4, -1, -1, 12, 25, 12, -4, -1, -1, 17, 18, 15, 20, 12, 21, 8, 21, 5, 20, 3,
        18, 3, 16, 4, 14, 5, 13, 7, 12, 13, 10, 15, 9, 16, 8, 17, 6, 17, 3, 15, 1, 12, 0, 8, 0, 5,
        1, 3, 3,
    ], // $
    &[
        24, 21, 21, 3, 0, -1, -1, 8, 21, 10, 19, 10, 17, 9, 15, 7, 14, 5, 14, 3, 16, 3, 18, 4, 20,
        6, 21, 8, 21, 10, 20, 13, 19, 16, 19, 19, 20, 21, 21, -1, -1, 17, 7, 15, 6, 14, 4, 14, 2,
        16, 0, 18, 0, 20, 1, 21, 3, 21, 5, 19, 7, 17, 7,
    ], // %
    &[
        26, 23, 12, 23, 13, 22, 14, 21, 14, 20, 13, 19, 11, 17, 6, 15, 3, 13, 1, 11, 0, 7, 0, 5, 1,
        4, 2, 3, 4, 3, 6, 4, 8, 5, 9, 12, 13, 13, 14, 14, 16, 14, 18, 13, 20, 11, 21, 9, 20, 8, 18,
        8, 16, 9, 13, 11, 10, 16, 3, 18, 1, 20, 0, 22, 0, 23, 1, 23, 2,
    ], // &
    &[10, 5, 19, 4, 20, 5, 21, 6, 20, 6, 18, 5, 16, 4, 15],   // '
    &[
        14, 11, 25, 9, 23, 7, 20, 5, 16, 4, 11, 4, 7, 5, 2, 7, -2, 9, -5, 11, -7,
    ], // (
    &[
        14, 3, 25, 5, 23, 7, 20, 9, 16, 10, 11, 10, 7, 9, 2, 7, -2, 5, -5, 3, -7,
    ], // )
    &[
        16, 8, 21, 8, 9, -1, -1, 3, 18, 13, 12, -1, -1, 13, 18, 3, 12,
    ], // *
    &[26, 13, 18, 13, 0, -1, -1, 4, 9, 22, 9],                // +
    &[10, 6, 1, 5, 0, 4, 1, 5, 2, 6, 1, 6, -1, 5, -3, 4, -4], // ,
    &[26, 4, 9, 22, 9],                                       // -
    &[10, 5, 2, 4, 1, 5, 0, 6, 1, 5, 2],                      // .
    &[22, 20, 25, 2, -7],                                     // /
    &[
        20, 9, 21, 6, 20, 4, 17, 3, 12, 3, 9, 4, 4, 6, 1, 9, 0, 11, 0, 14, 1, 16, 4, 17, 9, 17, 12,
        16, 17, 14, 20, 11, 21, 9, 21,
    ], // 0
    &[20, 6, 17, 8, 18, 11, 21, 11, 0],                       // 1
    &[
        20, 4, 16, 4, 17, 5, 19, 6, 20, 8, 21, 12, 21, 14, 20, 15, 19, 16, 17, 16, 15, 15, 13, 13,
        10, 3, 0, 17, 0,
    ], // 2
    &[
        20, 5, 21, 16, 21, 10, 13, 13, 13, 15, 12, 16, 11, 17, 8, 17, 6, 16, 3, 14, 1, 11, 0, 8, 0,
        5, 1, 4, 2, 3, 4,
    ], // 3
    &[20, 13, 21, 3, 7, 18, 7, -1, -1, 13, 21, 13, 0],        // 4
    &[
        20, 15, 21, 5, 21, 4, 12, 5, 13, 8, 14, 11, 14, 14, 13, 16, 11, 17, 8, 17, 6, 16, 3, 14, 1,
        11, 0, 8, 0, 5, 1, 4, 2, 3, 4,
    ], // 5
    &[
        20, 16, 18, 15, 20, 12, 21, 10, 21, 7, 20, 5, 17, 4, 12, 4, 7, 5, 3, 7, 1, 10, 0, 11, 0,
        14, 1, 16, 3, 17, 6, 17, 7, 16, 10, 14, 12, 11, 13, 10, 13, 7, 12, 5, 10, 4, 7,
    ], // 6
    &[20, 17, 21, 7, 0, -1, -1, 3, 21, 17, 21],               // 7
    &[
        20, 8, 21, 5, 20, 4, 18, 4, 16, 5, 14, 7, 13, 11, 12, 14, 11, 16, 9, 17, 7, 17, 4, 16, 2,
        15, 1, 12, 0, 8, 0, 5, 1, 4, 2, 3, 4, 3, 7, 4, 9, 6, 11, 9, 12, 13, 13, 15, 14, 16, 16, 16,
        18, 15, 20, 12, 21, 8, 21,
    ], // 8
    &[
        20, 16, 14, 15, 11, 13, 9, 10, 8, 9, 8, 6, 9, 4, 11, 3, 14, 3, 15, 4, 18, 6, 20, 9, 21, 10,
        21, 13, 20, 15, 18, 16, 14, 16, 9, 15, 4, 13, 1, 10, 0, 8, 0, 5, 1, 4, 3,
    ], // 9
    &[
        10, 5, 14, 4, 13, 5, 12, 6, 13, 5, 14, -1, -1, 5, 2, 4, 1, 5, 0, 6, 1, 5, 2,
    ], // :
    &[
        10, 5, 14, 4, 13, 5, 12, 6, 13, 5, 14, -1, -1, 6, 1, 5, 0, 4, 1, 5, 2, 6, 1, 6, -1, 5, -3,
        4, -4,
    ], // ;
    &[24, 20, 18, 4, 9, 20, 0],                               // <
    &[26, 4, 12, 22, 12, -1, -1, 4, 6, 22, 6],                // =
    &[24, 4, 18, 20, 9, 4, 0],                                // >
    &[
        18, 3, 16, 3, 17, 4, 19, 5, 20, 7, 21, 11, 21, 13, 20, 14, 19, 15, 17, 15, 15, 14, 13, 13,
        12, 9, 10, 9, 7, -1, -1, 9, 2, 8, 1, 9, 0, 10, 1, 9, 2,
    ], // ?
    &[
        27, 18, 13, 17, 15, 15, 16, 12, 16, 10, 15, 9, 14, 8, 11, 8, 8, 9, 6, 11, 5, 14, 5, 16, 6,
        17, 8, -1, -1, 12, 16, 10, 14, 9, 11, 9, 8, 10, 6, 11, 5, -1, -1, 18, 16, 17, 8, 17, 6, 19,
        5, 21, 5, 23, 7, 24, 10, 24, 12, 23, 15, 22, 17, 20, 19, 18, 20, 15, 21, 12, 21, 9, 20, 7,
        19, 5, 17, 4, 15, 3, 12, 3, 9, 4, 6, 5, 4, 7, 2, 9, 1, 12, 0, 15, 0, 18, 1, 20, 2, 21, 3,
        -1, -1, 19, 16, 18, 8, 18, 6, 19, 5,
    ], // @
    &[18, 9, 21, 1, 0, -1, -1, 9, 21, 17, 0, -1, -1, 4, 7, 14, 7], // A
    &[
        21, 4, 21, 4, 0, -1, -1, 4, 21, 13, 21, 16, 20, 17, 19, 18, 17, 18, 15, 17, 13, 16, 12, 13,
        11, -1, -1, 4, 11, 13, 11, 16, 10, 17, 9, 18, 7, 18, 4, 17, 2, 16, 1, 13, 0, 4, 0,
    ], // B
    &[
        21, 18, 16, 17, 18, 15, 20, 13, 21, 9, 21, 7, 20, 5, 18, 4, 16, 3, 13, 3, 8, 4, 5, 5, 3, 7,
        1, 9, 0, 13, 0, 15, 1, 17, 3, 18, 5,
    ], // C
    &[
        21, 4, 21, 4, 0, -1, -1, 4, 21, 11, 21, 14, 20, 16, 18, 17, 16, 18, 13, 18, 8, 17, 5, 16,
        3, 14, 1, 11, 0, 4, 0,
    ], // D
    &[
        19, 4, 21, 4, 0, -1, -1, 4, 21, 17, 21, -1, -1, 4, 11, 12, 11, -1, -1, 4, 0, 17, 0,
    ], // E
    &[
        18, 4, 21, 4, 0, -1, -1, 4, 21, 17, 21, -1, -1, 4, 11, 12, 11,
    ], // F
    &[
        21, 18, 16, 17, 18, 15, 20, 13, 21, 9, 21, 7, 20, 5, 18, 4, 16, 3, 13, 3, 8, 4, 5, 5, 3, 7,
        1, 9, 0, 13, 0, 15, 1, 17, 3, 18, 5, 18, 8, -1, -1, 13, 8, 18, 8,
    ], // G
    &[
        22, 4, 21, 4, 0, -1, -1, 18, 21, 18, 0, -1, -1, 4, 11, 18, 11,
    ], // H
    &[8, 4, 21, 4, 0],                                        // I
    &[
        16, 12, 21, 12, 5, 11, 2, 10, 1, 8, 0, 6, 0, 4, 1, 3, 2, 2, 5, 2, 7,
    ], // J
    &[21, 4, 21, 4, 0, -1, -1, 18, 21, 4, 7, -1, -1, 9, 12, 18, 0], // K
    &[17, 4, 21, 4, 0, -1, -1, 4, 0, 16, 0],                  // L
    &[
        24, 4, 21, 4, 0, -1, -1, 4, 21, 12, 0, -1, -1, 20, 21, 12, 0, -1, -1, 20, 21, 20, 0,
    ], // M
    &[22, 4, 21, 4, 0, -1, -1, 4, 21, 18, 0, -1, -1, 18, 21, 18, 0], // N
    &[
        22, 9, 21, 7, 20, 5, 18, 4, 16, 3, 13, 3, 8, 4, 5, 5, 3, 7, 1, 9, 0, 13, 0, 15, 1, 17, 3,
        18, 5, 19, 8, 19, 13, 18, 16, 17, 18, 15, 20, 13, 21, 9, 21,
    ], // O
    &[
        21, 4, 21, 4, 0, -1, -1, 4, 21, 13, 21, 16, 20, 17, 19, 18, 17, 18, 14, 17, 12, 16, 11, 13,
        10, 4, 10,
    ], // P
    &[
        22, 9, 21, 7, 20, 5, 18, 4, 16, 3, 13, 3, 8, 4, 5, 5, 3, 7, 1, 9, 0, 13, 0, 15, 1, 17, 3,
        18, 5, 19, 8, 19, 13, 18, 16, 17, 18, 15, 20, 13, 21, 9, 21, -1, -1, 12, 4, 18, -2,
    ], // Q
    &[
        21, 4, 21, 4, 0, -1, -1, 4, 21, 13, 21, 16, 20, 17, 19, 18, 17, 18, 15, 17, 13, 16, 12, 13,
        11, 4, 11, -1, -1, 11, 11, 18, 0,
    ], // R
    &[
        20, 17, 18, 15, 20, 12, 21, 8, 21, 5, 20, 3, 18, 3, 16, 4, 14, 5, 13, 7, 12, 13, 10, 15, 9,
        16, 8, 17, 6, 17, 3, 15, 1, 12, 0, 8, 0, 5, 1, 3, 3,
    ], // S
    &[16, 8, 21, 8, 0, -1, -1, 1, 21, 15, 21],                // T
    &[
        22, 4, 21, 4, 6, 5, 3, 7, 1, 10, 0, 12, 0, 15, 1, 17, 3, 18, 6, 18, 21,
    ], // U
    &[18, 1, 21, 9, 0, -1, -1, 17, 21, 9, 0],                 // V
    &[
        24, 2, 21, 7, 0, -1, -1, 12, 21, 7, 0, -1, -1, 12, 21, 17, 0, -1, -1, 22, 21, 17, 0,
    ], // W
    &[20, 3, 21, 17, 0, -1, -1, 17, 21, 3, 0],                // X
    &[18, 1, 21, 9, 11, 9, 0, -1, -1, 17, 21, 9, 11],         // Y
    &[20, 17, 21, 3, 0, -1, -1, 3, 21, 17, 21, -1, -1, 3, 0, 17, 0], // Z
    &[
        14, 4, 25, 4, -7, -1, -1, 5, 25, 5, -7, -1, -1, 4, 25, 11, 25, -1, -1, 4, -7, 11, -7,
    ], // [
    &[14, 0, 21, 14, -3],                                     // backslash
    &[
        14, 9, 25, 9, -7, -1, -1, 10, 25, 10, -7, -1, -1, 3, 25, 10, 25, -1, -1, 3, -7, 10, -7,
    ], // ]
    &[16, 4, 14, 8, 21, 12, 14],                              // ^ (simplified caret)
    &[16, 0, -2, 16, -2],                                     // _
    &[10, 6, 21, 5, 20, 4, 18, 4, 16, 5, 15, 6, 16, 5, 17],   // `
    &[
        19, 15, 14, 15, 0, -1, -1, 15, 11, 13, 13, 11, 14, 8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3,
        6, 1, 8, 0, 11, 0, 13, 1, 15, 3,
    ], // a
    &[
        19, 4, 21, 4, 0, -1, -1, 4, 11, 6, 13, 8, 14, 11, 14, 13, 13, 15, 11, 16, 8, 16, 6, 15, 3,
        13, 1, 11, 0, 8, 0, 6, 1, 4, 3,
    ], // b
    &[
        18, 15, 11, 13, 13, 11, 14, 8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3, 6, 1, 8, 0, 11, 0, 13,
        1, 15, 3,
    ], // c
    &[
        19, 15, 21, 15, 0, -1, -1, 15, 11, 13, 13, 11, 14, 8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3,
        6, 1, 8, 0, 11, 0, 13, 1, 15, 3,
    ], // d
    &[
        18, 3, 8, 15, 8, 15, 10, 14, 12, 13, 13, 11, 14, 8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3, 6,
        1, 8, 0, 11, 0, 13, 1, 15, 3,
    ], // e
    &[12, 10, 21, 8, 21, 6, 20, 5, 17, 5, 0, -1, -1, 2, 14, 9, 14], // f
    &[
        19, 15, 14, 15, -2, 14, -5, 13, -6, 11, -7, 8, -7, 6, -6, -1, -1, 15, 11, 13, 13, 11, 14,
        8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3, 6, 1, 8, 0, 11, 0, 13, 1, 15, 3,
    ], // g
    &[
        19, 4, 21, 4, 0, -1, -1, 4, 10, 7, 13, 9, 14, 12, 14, 14, 13, 15, 10, 15, 0,
    ], // h
    &[8, 3, 21, 4, 20, 5, 21, 4, 22, 3, 21, -1, -1, 4, 14, 4, 0], // i
    &[
        10, 5, 21, 6, 20, 7, 21, 6, 22, 5, 21, -1, -1, 6, 14, 6, -3, 5, -6, 3, -7, 1, -7,
    ], // j
    &[17, 4, 21, 4, 0, -1, -1, 14, 14, 4, 4, -1, -1, 8, 8, 15, 0], // k
    &[8, 4, 21, 4, 0],                                        // l
    &[
        30, 4, 14, 4, 0, -1, -1, 4, 10, 7, 13, 9, 14, 12, 14, 14, 13, 15, 10, 15, 0, -1, -1, 15,
        10, 18, 13, 20, 14, 23, 14, 25, 13, 26, 10, 26, 0,
    ], // m
    &[
        19, 4, 14, 4, 0, -1, -1, 4, 10, 7, 13, 9, 14, 12, 14, 14, 13, 15, 10, 15, 0,
    ], // n
    &[
        19, 8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3, 6, 1, 8, 0, 11, 0, 13, 1, 15, 3, 16, 6, 16, 8,
        15, 11, 13, 13, 11, 14, 8, 14,
    ], // o
    &[
        19, 4, 14, 4, -7, -1, -1, 4, 11, 6, 13, 8, 14, 11, 14, 13, 13, 15, 11, 16, 8, 16, 6, 15, 3,
        13, 1, 11, 0, 8, 0, 6, 1, 4, 3,
    ], // p
    &[
        19, 15, 14, 15, -7, -1, -1, 15, 11, 13, 13, 11, 14, 8, 14, 6, 13, 4, 11, 3, 8, 3, 6, 4, 3,
        6, 1, 8, 0, 11, 0, 13, 1, 15, 3,
    ], // q
    &[13, 4, 14, 4, 0, -1, -1, 4, 8, 5, 11, 7, 13, 9, 14, 12, 14], // r
    &[
        17, 14, 11, 13, 13, 10, 14, 7, 14, 4, 13, 3, 11, 4, 9, 6, 8, 11, 7, 13, 6, 14, 4, 14, 3,
        13, 1, 10, 0, 7, 0, 4, 1, 3, 3,
    ], // s
    &[12, 5, 21, 5, 4, 6, 1, 8, 0, 10, 0, -1, -1, 2, 14, 9, 14], // t
    &[
        19, 4, 14, 4, 4, 5, 1, 7, 0, 10, 0, 12, 1, 15, 4, -1, -1, 15, 14, 15, 0,
    ], // u
    &[16, 2, 14, 8, 0, -1, -1, 14, 14, 8, 0],                 // v
    &[
        22, 3, 14, 7, 0, -1, -1, 11, 14, 7, 0, -1, -1, 11, 14, 15, 0, -1, -1, 19, 14, 15, 0,
    ], // w
    &[17, 3, 14, 14, 0, -1, -1, 14, 14, 3, 0],                // x
    &[
        16, 2, 14, 8, 0, -1, -1, 14, 14, 8, 0, 6, -4, 4, -6, 2, -7, 1, -7,
    ], // y
    &[17, 14, 14, 3, 0, -1, -1, 3, 14, 14, 14, -1, -1, 3, 0, 14, 0], // z
    &[
        14, 9, 25, 7, 24, 6, 23, 5, 21, 5, 19, 6, 17, 7, 16, 8, 14, 8, 12, 6, 10, -1, -1, 7, 24, 6,
        22, 6, 20, 7, 18, 8, 17, 9, 15, 9, 13, 8, 11, 4, 9, 8, 7, 9, 5, 9, 3, 8, 1, 7, 0, 6, -2, 6,
        -4, 7, -6, -1, -1, 6, 8, 8, 6, 8, 4, 7, 2, 6, 1, 5, -1, 5, -3, 6, -5, 7, -6, 9, -7,
    ], // {
    &[8, 4, 25, 4, -7],                                       // |
    &[
        14, 5, 25, 7, 24, 8, 23, 9, 21, 9, 19, 8, 17, 7, 16, 6, 14, 6, 12, 8, 10, -1, -1, 7, 24, 8,
        22, 8, 20, 7, 18, 6, 17, 5, 15, 5, 13, 6, 11, 10, 9, 6, 7, 5, 5, 5, 3, 6, 1, 7, 0, 8, -2,
        8, -4, 7, -6, -1, -1, 8, 8, 6, 6, 6, 4, 7, 2, 8, 1, 9, -1, 9, -3, 8, -5, 7, -6, 5, -7,
    ], // }
    &[
        24, 3, 6, 3, 8, 4, 11, 6, 12, 8, 12, 10, 11, 14, 8, 16, 7, 18, 7, 20, 8, 21, 10, -1, -1, 3,
        8, 4, 10, 6, 11, 8, 11, 10, 10, 14, 7, 16, 6, 18, 6, 20, 7, 21, 9, 21, 11,
    ], // ~
];

/// Look up the stroke set for an ASCII character.
fn hershey_glyph(c: char) -> Option<&'static [i8]> {
    let code = c as usize;
    if !(32..=126).contains(&code) {
        return None;
    }
    Some(HERSHEY_SIMPLEX[code - 32])
}

/// Tofu box strokes (drawn for any character outside the bundled coverage).
const TOFU_STROKES: &[i8] = &[16, 3, 0, 13, 0, 13, 21, 3, 21, 3, 0];

/// Advance width (glyph units) for a character, tofu included.
fn glyph_advance_units(c: char) -> f64 {
    hershey_glyph(c).unwrap_or(TOFU_STROKES)[0] as f64
}

/// One laid-out run of characters from a `<text>` element's inner markup.
struct TextRun {
    text: String,
    /// Absolute reposition from a `<tspan x= y=>`, in user units.
    x: Option<f32>,
    y: Option<f32>,
    /// Relative offsets from `<tspan dx= dy=>`, in user units.
    dx: f32,
    dy: f32,
}

/// Flags accumulated while scanning text content (each becomes a diagnostic).
#[derive(Default)]
struct TextScanFlags {
    nested_tspan: bool,
    styled_tspan: bool,
    text_path: Option<TextPathRef>,
}

/// A `<textPath>` reference found inside a `<text>` element.
struct TextPathRef {
    href: Option<String>,
    start_offset: Option<svg_core::SvgLength>,
    text: String,
}

/// Collapse XML whitespace runs to single spaces (default `xml:space`).
fn collapse_text_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Extract the attribute list from a raw `<tag attr="v">` fragment.
fn scan_tag_attrs(tag_body: &str) -> Vec<(String, String)> {
    let mut parser = XmlParser {
        s: tag_body,
        pos: 0,
        next_node_id: 0,
        ns_stack: Vec::new(),
        foreign_count: 0,
        recovered: 0,
        title: None,
        desc: None,
    };
    let mut attrs = Vec::new();
    while parser.pos < tag_body.len() {
        if let Some((k, v)) = parser.parse_attr() {
            attrs.push((k, v));
        } else {
            parser.consume(1);
        }
    }
    attrs
}

/// Scan a `<text>` element's raw inner markup into flat character runs plus an
/// optional `<textPath>` payload.  One level of `<tspan>` is honored
/// (x/y/dx/dy); deeper nesting and other child tags are flattened to their
/// text with a diagnostic flag.
fn scan_text_runs(content: &str) -> (Vec<TextRun>, TextScanFlags) {
    let mut runs = Vec::new();
    let mut flags = TextScanFlags::default();
    let lower = content.to_ascii_lowercase();
    let mut pos = 0usize;

    let push_plain = |text: &str, runs: &mut Vec<TextRun>| {
        let collapsed = collapse_text_whitespace(&unescape_xml(text));
        if !collapsed.is_empty() {
            runs.push(TextRun {
                text: collapsed,
                x: None,
                y: None,
                dx: 0.0,
                dy: 0.0,
            });
        }
    };

    while pos < content.len() {
        let Some(open_rel) = lower[pos..].find('<') else {
            push_plain(&content[pos..], &mut runs);
            break;
        };
        let open = pos + open_rel;
        push_plain(&content[pos..open], &mut runs);
        let Some(gt_rel) = lower[open..].find('>') else {
            break; // malformed tail — stop scanning
        };
        let tag_end = open + gt_rel;
        let tag_body = &content[open + 1..tag_end];
        let tag_lower = &lower[open + 1..tag_end];
        if tag_lower.starts_with("tspan") {
            let attrs = scan_tag_attrs(&tag_body["tspan".len()..]);
            if attrs
                .iter()
                .any(|(k, _)| matches!(k.as_str(), "font-size" | "fill" | "stroke" | "style"))
            {
                flags.styled_tspan = true;
            }
            let self_closing = tag_body.trim_end().ends_with('/');
            let (inner, after) = if self_closing {
                ("", tag_end + 1)
            } else if let Some(close_rel) = lower[tag_end..].find("</tspan") {
                let close = tag_end + close_rel;
                let inner = &content[tag_end + 1..close];
                let after = lower[close..]
                    .find('>')
                    .map(|r| close + r + 1)
                    .unwrap_or(content.len());
                (inner, after)
            } else {
                (&content[tag_end + 1..], content.len())
            };
            if inner.to_ascii_lowercase().contains("<tspan") {
                flags.nested_tspan = true;
            }
            // Flatten any nested markup inside the tspan to its text.
            let inner_text = strip_tags(inner);
            let collapsed = collapse_text_whitespace(&unescape_xml(&inner_text));
            if !collapsed.is_empty() {
                let num = |key: &str| {
                    attr_get(&attrs, key)
                        .and_then(svg_core::parse_length)
                        .map(|l| l.value as f32)
                };
                runs.push(TextRun {
                    text: collapsed,
                    x: num("x"),
                    y: num("y"),
                    dx: num("dx").unwrap_or(0.0),
                    dy: num("dy").unwrap_or(0.0),
                });
            }
            pos = after;
        } else if tag_lower.starts_with("textpath") {
            let attrs = scan_tag_attrs(&tag_body["textpath".len()..]);
            let (inner, after) = if let Some(close_rel) = lower[tag_end..].find("</textpath") {
                let close = tag_end + close_rel;
                let inner = &content[tag_end + 1..close];
                let after = lower[close..]
                    .find('>')
                    .map(|r| close + r + 1)
                    .unwrap_or(content.len());
                (inner, after)
            } else {
                (&content[tag_end + 1..], content.len())
            };
            flags.text_path = Some(TextPathRef {
                href: attr_get(&attrs, "href")
                    .and_then(|v| v.trim().strip_prefix('#'))
                    .map(ToOwned::to_owned),
                start_offset: attr_get(&attrs, "startoffset").and_then(svg_core::parse_length),
                text: collapse_text_whitespace(&unescape_xml(&strip_tags(inner))),
            });
            pos = after;
        } else {
            // Unknown child tag — skip the tag itself, keep scanning after it.
            pos = tag_end + 1;
        }
    }
    (runs, flags)
}

/// Remove markup tags, keeping text content.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Character classes the bundled font cannot honestly render.
fn char_needs_bidi(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x200E..=0x200F | 0x202A..=0x202E)
}

fn char_needs_shaping(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x20D0..=0x20FF)
}

/// Append one glyph's strokes to `data`, placing each glyph-space vertex via
/// `place` (which bakes scale, y-flip, pen position, and any rotation).
fn append_glyph_strokes(
    data: &mut PathData,
    strokes: &[i8],
    place: &dyn Fn(f64, f64) -> (f64, f64),
) {
    let mut current: Option<PathSubpath> = None;
    let mut i = 1;
    while i + 1 < strokes.len() {
        let (gx, gy) = (strokes[i], strokes[i + 1]);
        i += 2;
        if gx == -1 && gy == -1 {
            if let Some(sub) = current.take() {
                if !sub.segments.is_empty() {
                    data.subpaths.push(sub);
                }
            }
            continue;
        }
        let to = place(gx as f64, gy as f64);
        match current.as_mut() {
            None => {
                current = Some(PathSubpath {
                    start: to,
                    segments: Vec::new(),
                    closed: false,
                });
            }
            Some(sub) => sub.segments.push(PathSegment::Line { to }),
        }
    }
    if let Some(sub) = current.take() {
        if !sub.segments.is_empty() {
            data.subpaths.push(sub);
        }
    }
}

/// Total advance of a string in glyph units.
fn text_advance_units(text: &str) -> f64 {
    text.chars().map(glyph_advance_units).sum()
}

/// Arc-length table over a flattened user-space path (R11 textPath).
struct ArcLengthPath {
    points: Vec<(f64, f64)>,
    cumulative: Vec<f64>,
}

impl ArcLengthPath {
    fn build(subpaths: &[FlattenedSubpath]) -> Option<Self> {
        let mut points: Vec<(f64, f64)> = Vec::new();
        for sub in subpaths {
            points.extend(sub.points.iter().map(|&(x, y)| (x as f64, y as f64)));
        }
        if points.len() < 2 {
            return None;
        }
        let mut cumulative = Vec::with_capacity(points.len());
        let mut total = 0.0;
        cumulative.push(0.0);
        for pair in points.windows(2) {
            total += (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1);
            cumulative.push(total);
        }
        Some(Self { points, cumulative })
    }

    fn total(&self) -> f64 {
        *self.cumulative.last().unwrap_or(&0.0)
    }

    /// Point + tangent angle at arc distance `d`; `None` beyond the path end
    /// (glyphs past the end are not rendered, per SVG).
    fn at(&self, d: f64) -> Option<((f64, f64), f64)> {
        if d < 0.0 || d > self.total() {
            return None;
        }
        let idx = match self
            .cumulative
            .binary_search_by(|probe| probe.partial_cmp(&d).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i.min(self.points.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.points.len() - 2),
        };
        let seg = self.cumulative[idx + 1] - self.cumulative[idx];
        let t = if seg > 1.0e-12 {
            (d - self.cumulative[idx]) / seg
        } else {
            0.0
        };
        let (p0, p1) = (self.points[idx], self.points[idx + 1]);
        let pos = (p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t);
        let angle = (p1.1 - p0.1).atan2(p1.0 - p0.0);
        Some((pos, angle))
    }
}

/// Flatten a referenced geometry into user-space polylines for textPath.
fn user_space_subpaths(geometry: &ShapeGeometry) -> Vec<FlattenedSubpath> {
    match geometry {
        ShapeGeometry::Path { data } => flatten_path_data(data, &Transform::identity(), 0.25),
        ShapeGeometry::Poly { points, closed } => vec![FlattenedSubpath {
            points: points.clone(),
            closed: *closed,
        }],
        ShapeGeometry::Line { from, to } => vec![FlattenedSubpath {
            points: vec![*from, *to],
            closed: false,
        }],
        ShapeGeometry::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => vec![FlattenedSubpath {
            points: rounded_rect_pts(*x, *y, *width, *height, *rx, *ry),
            closed: true,
        }],
        ShapeGeometry::Ellipse { cx, cy, rx, ry } => vec![FlattenedSubpath {
            points: ellipse_pts(*cx, *cy, *rx, *ry),
            closed: true,
        }],
    }
}

/// Lower a `<text>` element into a stroked-glyph `Shape` command (R11).  All
/// glyph layout happens in user space; the resulting `PathData` flows through
/// the normal shape render path, so clips, masks, filters, opacity, and
/// gradient paint apply to text exactly like to shapes.
fn lower_text_command(
    scene: &SvgScene,
    item: &SvgSceneItem,
    node_xform: Transform,
    mut diagnostics: Vec<PendingDiagnostic>,
    source: SvgRenderSource,
) -> DrawCommand {
    let SvgNode::Text { attrs, content, .. } = &item.node else {
        return DrawCommand::SkippedShape {
            diagnostics,
            source,
        };
    };
    let lb = item.length_bases;
    let font_size = item
        .style
        .font_size
        .resolve(svg_core::SvgLengthContext::user_units(lb.other))
        .filter(|v| *v > 0.0)
        .unwrap_or(16.0);
    let scale = font_size / HERSHEY_EM_UNITS;

    // First value of a possibly-listed coordinate attribute, in user units.
    let mut position_list = false;
    let mut first_coord = |key: &str, base: f64| -> f32 {
        match attr_get(attrs, key) {
            None => 0.0,
            Some(value) => {
                let nums = svg_core::parse_numbers(value);
                if nums.len() > 1 {
                    position_list = true;
                }
                match nums.first() {
                    Some(n) => *n as f32,
                    None => attr_f32(attrs, key, base, 0.0),
                }
            }
        }
    };
    let origin_x = first_coord("x", lb.horizontal) as f64;
    let origin_y = first_coord("y", lb.vertical) as f64;
    if position_list {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "text.position_list_approximated",
            message: "per-glyph x/y position lists are approximated by their first value",
        });
    }

    let (runs, flags) = scan_text_runs(content);
    if flags.nested_tspan {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "text.tspan_nested_flattened",
            message: "tspan nesting beyond one level was flattened to plain text",
        });
    }
    if flags.styled_tspan {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "text.tspan_style_ignored",
            message: "per-tspan font/paint styling is ignored; the text element style is used",
        });
    }

    let mut data = PathData::default();
    let mut glyph_count = 0usize;
    let mut truncated = false;
    let mut tofu = false;
    let mut bidi = false;
    let mut shaping = false;

    // Resolve the strokes for one character, recording honesty flags.
    let mut strokes_for = |c: char| -> &'static [i8] {
        if char_needs_bidi(c) {
            bidi = true;
            TOFU_STROKES
        } else if char_needs_shaping(c) {
            shaping = true;
            TOFU_STROKES
        } else {
            hershey_glyph(c).unwrap_or_else(|| {
                tofu = true;
                TOFU_STROKES
            })
        }
    };

    if let Some(text_path) = &flags.text_path {
        // --- textPath: glyphs along a referenced path, arc-length sampled ---
        let resolved = text_path
            .href
            .as_ref()
            .and_then(|id| scene.references.by_xml_id.get(id))
            .and_then(|node_id| scene.references.nodes_by_id.get(node_id))
            .and_then(|node| lower_shape_geometry(node, lb))
            .map(|geometry| user_space_subpaths(&geometry))
            .and_then(|subs| ArcLengthPath::build(&subs));
        match resolved {
            None => {
                diagnostics.push(PendingDiagnostic::Warning {
                    code: "textpath.unresolved",
                    message:
                        "textPath references an unavailable or empty local path; the text was not rendered",
                });
            }
            Some(arc) => {
                let total_advance = text_advance_units(&text_path.text) * scale;
                let start = match text_path.start_offset {
                    Some(len) if len.unit == svg_core::SvgLengthUnit::Percent => {
                        arc.total() * len.value / 100.0
                    }
                    Some(len) => len.value,
                    None => 0.0,
                } + match item.style.text_anchor {
                    TextAnchor::Start => 0.0,
                    TextAnchor::Middle => -total_advance / 2.0,
                    TextAnchor::End => -total_advance,
                };
                let mut pen = start;
                for c in text_path.text.chars() {
                    if glyph_count >= MAX_TEXT_GLYPHS {
                        truncated = true;
                        break;
                    }
                    let strokes = strokes_for(c);
                    let advance = strokes[0] as f64 * scale;
                    // Sample position at the glyph origin and the tangent at the
                    // glyph midpoint, so rotation follows the curve smoothly.
                    if let Some((pos, _)) = arc.at(pen) {
                        let angle = arc
                            .at(pen + advance * 0.5)
                            .map(|(_, a)| a)
                            .unwrap_or_else(|| arc.at(pen).map(|(_, a)| a).unwrap_or(0.0));
                        let (sin, cos) = angle.sin_cos();
                        append_glyph_strokes(&mut data, strokes, &|gx, gy| {
                            let (lx, ly) = (gx * scale, -gy * scale);
                            (pos.0 + lx * cos - ly * sin, pos.1 + lx * sin + ly * cos)
                        });
                        glyph_count += 1;
                    }
                    pen += advance;
                }
            }
        }
    } else {
        // --- plain text: horizontal pen, x/y/dx/dy runs, whole-run anchor ---
        let total_advance: f64 = runs
            .iter()
            .map(|run| text_advance_units(&run.text))
            .sum::<f64>()
            * scale;
        let anchor_shift = match item.style.text_anchor {
            TextAnchor::Start => 0.0,
            TextAnchor::Middle => -total_advance / 2.0,
            TextAnchor::End => -total_advance,
        };
        let mut pen_x = origin_x + anchor_shift;
        let mut pen_y = origin_y;
        'runs: for run in &runs {
            if let Some(x) = run.x {
                pen_x = x as f64 + anchor_shift;
            }
            if let Some(y) = run.y {
                pen_y = y as f64;
            }
            pen_x += run.dx as f64;
            pen_y += run.dy as f64;
            for c in run.text.chars() {
                if glyph_count >= MAX_TEXT_GLYPHS {
                    truncated = true;
                    break 'runs;
                }
                let strokes = strokes_for(c);
                let (gx0, gy0) = (pen_x, pen_y);
                append_glyph_strokes(&mut data, strokes, &|gx, gy| {
                    (gx0 + gx * scale, gy0 - gy * scale)
                });
                pen_x += strokes[0] as f64 * scale;
                glyph_count += 1;
            }
        }
    }

    if truncated {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "limit.text_glyphs",
            message: "text exceeded the renderer glyph limit; remaining glyphs skipped",
        });
    }
    if tofu {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "text.glyph_unsupported",
            message:
                "characters outside the bundled ASCII glyph set were rendered as placeholder boxes",
        });
    }
    if bidi {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "text.bidi_unsupported",
            message: "bidirectional text is not supported; affected characters render as placeholder boxes",
        });
    }
    if shaping {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "text.shaping_unsupported",
            message: "combining marks / complex shaping are not supported; affected characters render as placeholder boxes",
        });
    }

    if data.subpaths.is_empty() {
        return DrawCommand::SkippedShape {
            diagnostics,
            source,
        };
    }

    diagnostics.push(PendingDiagnostic::Warning {
        code: "text.raster_snapshot",
        message:
            "text rendered with the bundled stroked vector font (font-family substituted, approximate metrics)",
    });

    // Text is painted with the element's *fill* through the stroke pipeline
    // (a stroked font has no fillable outline); stroke styling is not applied.
    let glyph_style = Style {
        fill: Paint::None,
        stroke: item.style.fill.clone(),
        stroke_width: svg_core::SvgLength {
            value: font_size * HERSHEY_STROKE_UNITS / HERSHEY_EM_UNITS,
            unit: svg_core::SvgLengthUnit::Number,
        },
        stroke_linecap: StrokeLineCap::Round,
        stroke_linejoin: StrokeLineJoin::Round,
        stroke_miterlimit: 4.0,
        stroke_dasharray: None,
        stroke_dashoffset: zero_stroke_length(),
        stroke_opacity: item.style.fill_opacity,
        ..item.style.clone()
    };

    let geometry = ShapeGeometry::Path { data };
    let clip = clip_path_ref(attrs).and_then(|id| {
        let bbox = Some(geometry_local_bounds(&geometry));
        let mut visited = Vec::new();
        resolve_clip(
            scene,
            &id,
            node_xform,
            bbox,
            lb,
            &mut visited,
            &mut diagnostics,
        )
    });
    DrawCommand::Shape {
        geometry: Some(geometry),
        transform: node_xform,
        style: Box::new(glyph_style),
        length_bases: lb,
        path_length: None,
        clip,
        markers: None,
        diagnostics,
        source,
    }
}

// ---------------------------------------------------------------------------
// ViewBox → pixel transform
// ---------------------------------------------------------------------------

fn viewbox_to_pixel_transform(scene: &SvgScene, pw: usize, ph: usize) -> Transform {
    let view_box = match scene.viewbox {
        Some([x, y, w, h]) if w > 0.0 && h > 0.0 => [x as f64, y as f64, w as f64, h as f64],
        _ => {
            // Fall back to width/height if present
            if scene.width > 0.0 && scene.height > 0.0 {
                [0.0, 0.0, scene.width as f64, scene.height as f64]
            } else {
                return Transform::identity();
            }
        }
    };
    svg_core::viewbox_transform(
        view_box,
        [0.0, 0.0, pw as f64, ph as f64],
        scene.preserve_aspect_ratio,
    )
    .unwrap_or_else(Transform::identity)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Display list IR
//
// The scene graph (`SvgScene` — flattened items still tied to parse nodes) is
// lowered into a `DisplayList`: a flat, render-ready stream of `DrawCommand`s
// with final transforms computed and skip/unsupported classification resolved.
// `build()` is the lowering pass (no pixels touched); `execute()` is the raster
// pass (no classification logic).  This mirrors mature renderers (scene tree →
// display list → raster) and makes the render-ready IR independently testable.
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum ShapeGeometry {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rx: f32,
        ry: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    Line {
        from: (f32, f32),
        to: (f32, f32),
    },
    Poly {
        points: Vec<(f32, f32)>,
        closed: bool,
    },
    Path {
        data: PathData,
    },
}

type PathPoint = (f64, f64);

#[derive(Clone, Debug, PartialEq)]
enum PathSegment {
    Line {
        to: PathPoint,
    },
    Quadratic {
        ctrl: PathPoint,
        to: PathPoint,
    },
    Cubic {
        ctrl1: PathPoint,
        ctrl2: PathPoint,
        to: PathPoint,
    },
    Arc {
        rx: f64,
        ry: f64,
        x_axis_rotation: f64,
        large_arc: bool,
        sweep: bool,
        to: PathPoint,
    },
}

impl PathSegment {
    fn end(&self) -> PathPoint {
        match *self {
            Self::Line { to }
            | Self::Quadratic { to, .. }
            | Self::Cubic { to, .. }
            | Self::Arc { to, .. } => to,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PathSubpath {
    start: PathPoint,
    segments: Vec<PathSegment>,
    closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PathData {
    subpaths: Vec<PathSubpath>,
}

// ---------------------------------------------------------------------------
// Clip geometry (resolved to device space during display-list build)
// ---------------------------------------------------------------------------

/// One fillable clip contour set, already in device pixel coordinates.
#[derive(Clone)]
struct ClipShape {
    device_subpaths: Vec<Vec<(f32, f32)>>,
    fill_rule: FillRule,
}

/// A resolved clip: the union of `shapes`, intersected with `nested` (used for
/// clipPath-of-clipPath chains and overflow ∩ clip-path combinations).
#[derive(Clone)]
struct ClipDef {
    shapes: Vec<ClipShape>,
    nested: Option<Box<ClipDef>>,
}

impl ClipDef {
    /// Rasterize this clip into an alpha mask of the requested raster size.
    fn build_mask(&self, w: usize, h: usize) -> ClipMask {
        let mut mask = ClipMask::transparent(w, h);
        for shape in &self.shapes {
            let refs: Vec<&[(f32, f32)]> =
                shape.device_subpaths.iter().map(Vec::as_slice).collect();
            mask.add_shape(&refs, shape.fill_rule);
        }
        if let Some(nested) = &self.nested {
            mask.intersect(&nested.build_mask(w, h));
        }
        mask
    }
}

fn geometry_local_bounds(geometry: &ShapeGeometry) -> [f64; 4] {
    match geometry {
        ShapeGeometry::Rect {
            x,
            y,
            width,
            height,
            ..
        } => [
            *x as f64,
            *y as f64,
            (*x + *width) as f64,
            (*y + *height) as f64,
        ],
        ShapeGeometry::Ellipse { cx, cy, rx, ry } => [
            (*cx - *rx) as f64,
            (*cy - *ry) as f64,
            (*cx + *rx) as f64,
            (*cy + *ry) as f64,
        ],
        ShapeGeometry::Line { from, to } => [
            from.0.min(to.0) as f64,
            from.1.min(to.1) as f64,
            from.0.max(to.0) as f64,
            from.1.max(to.1) as f64,
        ],
        ShapeGeometry::Poly { points, .. } => local_bounds(points),
        ShapeGeometry::Path { data } => {
            let flat = flatten_path_data(data, &Transform::identity(), 0.25);
            let points: Vec<(f32, f32)> = flat
                .iter()
                .flat_map(|subpath| subpath.points.iter())
                .copied()
                .collect();
            local_bounds(&points)
        }
    }
}

/// Lower a shape's geometry to device-space polygon rings under `transform`.
/// Lines produce no area and yield an empty set (clips fill, not stroke).
fn geometry_device_subpaths(
    geometry: &ShapeGeometry,
    transform: &Transform,
) -> Vec<Vec<(f32, f32)>> {
    let apply = |points: Vec<(f32, f32)>| -> Vec<(f32, f32)> {
        points
            .into_iter()
            .map(|(x, y)| transform.apply_f32(x, y))
            .collect()
    };
    match geometry {
        ShapeGeometry::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => vec![apply(rounded_rect_pts(*x, *y, *width, *height, *rx, *ry))],
        ShapeGeometry::Ellipse { cx, cy, rx, ry } => vec![apply(ellipse_pts(*cx, *cy, *rx, *ry))],
        ShapeGeometry::Line { .. } => Vec::new(),
        ShapeGeometry::Poly { points, .. } => vec![apply(points.clone())],
        ShapeGeometry::Path { data } => flatten_path_data(data, transform, 0.25)
            .into_iter()
            .map(|subpath| subpath.points)
            .collect(),
    }
}

/// Collect fillable clip contours from a clipPath subtree, honoring per-child
/// transforms, `<g>` nesting, and inherited `clip-rule`.
fn collect_clip_shapes(
    nodes: &[SvgNode],
    base: Transform,
    length_bases: SvgLengthBases,
    inherited_rule: FillRule,
    out: &mut Vec<ClipShape>,
) {
    for node in nodes {
        if out.len() >= MAX_CLIP_SHAPES {
            break;
        }
        let local_transform = attr_get(node.attrs(), "transform")
            .map(Transform::parse_chained)
            .unwrap_or_else(Transform::identity);
        let transform = base.concat(local_transform);
        let rule = final_style_property(node.attrs(), "clip-rule")
            .and_then(parse_fill_rule)
            .unwrap_or(inherited_rule);
        match node {
            SvgNode::Group { children, .. } => {
                collect_clip_shapes(children, transform, length_bases, rule, out);
            }
            _ => {
                if let Some(geometry) = lower_shape_geometry(node, length_bases) {
                    let subpaths = geometry_device_subpaths(&geometry, &transform);
                    if !subpaths.is_empty() {
                        out.push(ClipShape {
                            device_subpaths: subpaths,
                            fill_rule: rule,
                        });
                    }
                }
            }
        }
    }
}

/// Resolve a `clip-path="url(#id)"` reference into device-space clip geometry,
/// reusing the shared first-id-wins reference table.  Bounded by depth and
/// cycle detection; pushes structured diagnostics for every degraded path.
fn resolve_clip(
    scene: &SvgScene,
    clip_id: &str,
    element_ctm: Transform,
    bbox: Option<[f64; 4]>,
    length_bases: SvgLengthBases,
    visited: &mut Vec<SvgNodeId>,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> Option<ClipDef> {
    if visited.len() >= MAX_CLIP_DEPTH {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "limit.clip_depth",
            message: "clip-path nesting exceeded the renderer depth limit; clip skipped",
        });
        return None;
    }
    let Some(node) = scene
        .references
        .by_xml_id
        .get(clip_id)
        .and_then(|id| scene.references.nodes_by_id.get(id))
    else {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "clip.unresolved",
            message: "clip-path references an unavailable local id; no clip was applied",
        });
        return None;
    };
    if !matches!(node, SvgNode::Unsupported { tag, .. } if tag == "clippath") {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "clip.unresolved",
            message: "clip-path target is not a clipPath element; no clip was applied",
        });
        return None;
    }
    if visited.contains(&node.id()) {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "reference.clip_cycle",
            message: "cyclic clip-path reference was ignored",
        });
        return None;
    }
    visited.push(node.id());

    let units_obb = attr_get(node.attrs(), "clippathunits")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("objectBoundingBox"));
    let units_transform = if units_obb {
        match bbox {
            Some([x0, y0, x1, y1]) if (x1 - x0).abs() > 1e-12 && (y1 - y0).abs() > 1e-12 => {
                Transform::translate(x0, y0).multiply(Transform::scale(x1 - x0, y1 - y0))
            }
            _ => {
                diagnostics.push(PendingDiagnostic::Warning {
                    code: "clip.object_bounding_box",
                    message:
                        "objectBoundingBox clip units require a single shape bounding box; clip skipped",
                });
                visited.pop();
                return None;
            }
        }
    } else {
        Transform::identity()
    };

    let base = element_ctm.concat(units_transform);
    let mut shapes = Vec::new();
    if let Some(children) = node.children() {
        collect_clip_shapes(children, base, length_bases, FillRule::Nonzero, &mut shapes);
    }
    let nested = clip_path_ref(node.attrs()).and_then(|nested_id| {
        resolve_clip(
            scene,
            &nested_id,
            element_ctm,
            bbox,
            length_bases,
            visited,
            diagnostics,
        )
        .map(Box::new)
    });
    visited.pop();
    Some(ClipDef { shapes, nested })
}

/// Build a rectangular clip (device space) for nested-`<svg>` overflow.
fn overflow_clip_shape(rect: [f64; 4], scene_transform: Transform, view: &Transform) -> ClipShape {
    let [x, y, w, h] = rect;
    let device = view.concat(scene_transform);
    let corners = vec![
        device.apply_f32(x as f32, y as f32),
        device.apply_f32((x + w) as f32, y as f32),
        device.apply_f32((x + w) as f32, (y + h) as f32),
        device.apply_f32(x as f32, (y + h) as f32),
    ];
    ClipShape {
        device_subpaths: vec![corners],
        fill_rule: FillRule::Nonzero,
    }
}

// ---------------------------------------------------------------------------
// R7: masks
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum MaskMode {
    Luminance,
    Alpha,
}

/// One renderable shape lowered from a `<mask>` subtree.  Reused for `<marker>`
/// and `<pattern>` content lowering (R9): the transform is content-relative and a
/// base transform is applied at render time.
#[derive(Clone)]
struct MaskItem {
    geometry: ShapeGeometry,
    transform: Transform,
    style: Box<Style>,
    length_bases: SvgLengthBases,
    path_length: Option<f64>,
}

/// A resolved `<mask>`: its content shapes plus luminance/alpha mode.
struct MaskDef {
    items: Vec<MaskItem>,
    mode: MaskMode,
}

impl MaskDef {
    /// Render the mask content to a premultiplied buffer, then reduce to an
    /// alpha coverage mask (luminance or alpha). Reuses the shape renderer so
    /// gradient/solid mask content works identically to normal painting.
    fn build_alpha(&self, w: usize, h: usize, paint_servers: &PaintServerTable) -> ClipMask {
        let mut buf = vec![0u8; w * h * 4];
        let mut target = RasterTarget {
            buf: &mut buf,
            width: w,
            height: h,
            premultiplied: true,
            clip: None,
        };
        render_content_items(
            &self.items,
            Transform::identity(),
            paint_servers,
            &mut target,
        );
        let mut alpha = vec![0u8; w * h];
        for (i, slot) in alpha.iter_mut().enumerate() {
            let r = buf[i * 4] as f32;
            let g = buf[i * 4 + 1] as f32;
            let b = buf[i * 4 + 2] as f32;
            let a = buf[i * 4 + 3] as f32;
            // buf is premultiplied, so luminance(premult rgb) == luminance(straight)*alpha.
            *slot = match self.mode {
                MaskMode::Alpha => a.round().clamp(0.0, 255.0) as u8,
                MaskMode::Luminance => (0.2125 * r + 0.7154 * g + 0.0721 * b)
                    .round()
                    .clamp(0.0, 255.0) as u8,
            };
        }
        ClipMask {
            width: w,
            height: h,
            alpha,
        }
    }
}

fn resolve_mask(
    scene: &SvgScene,
    mask_id: &str,
    element_ctm: Transform,
    length_bases: SvgLengthBases,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> Option<MaskDef> {
    let node = scene
        .references
        .by_xml_id
        .get(mask_id)
        .and_then(|id| scene.references.nodes_by_id.get(id));
    let Some(SvgNode::Unsupported {
        tag,
        attrs,
        children,
        ..
    }) = node
    else {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "mask.unresolved",
            message: "mask references an unavailable local id; no mask was applied",
        });
        return None;
    };
    if tag != "mask" {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "mask.unresolved",
            message: "mask target is not a mask element; no mask was applied",
        });
        return None;
    }
    let mode = if final_style_property(attrs, "mask-type")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("alpha"))
    {
        MaskMode::Alpha
    } else {
        MaskMode::Luminance
    };
    if attr_get(attrs, "maskcontentunits")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("objectBoundingBox"))
    {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "mask.content_units",
            message: "maskContentUnits=objectBoundingBox is approximated in user space",
        });
    }
    let mut items = Vec::new();
    let root_style = Style::default();
    collect_mask_items(
        children,
        element_ctm,
        length_bases,
        &root_style,
        &scene.stylesheet,
        &mut items,
    );
    if items.is_empty() {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "mask.empty",
            message: "mask has no renderable content; the masked element is hidden",
        });
    }
    Some(MaskDef { items, mode })
}

fn collect_mask_items(
    nodes: &[SvgNode],
    base: Transform,
    length_bases: SvgLengthBases,
    inherited: &Style,
    sheet: &svg_core::SvgCssStyleSheet,
    out: &mut Vec<MaskItem>,
) {
    for node in nodes {
        if out.len() >= MAX_MASK_ITEMS {
            break;
        }
        let style = inherited.inherit(node, sheet);
        let local = attr_get(node.attrs(), "transform")
            .map(Transform::parse_chained)
            .unwrap_or_else(Transform::identity);
        let transform = base.concat(local);
        match node {
            SvgNode::Group { children, .. } => {
                collect_mask_items(children, transform, length_bases, &style, sheet, out)
            }
            _ => {
                if let Some(geometry) = lower_shape_geometry(node, length_bases) {
                    out.push(MaskItem {
                        geometry,
                        transform,
                        style: Box::new(style),
                        length_bases,
                        path_length: parsed_path_length(node.attrs()),
                    });
                }
            }
        }
    }
}

/// Render a list of content items (shapes lowered from a `<mask>`, `<marker>`,
/// or `<pattern>` subtree) under `base` into `target`, reusing the shape
/// renderer so gradient/solid/pattern paint behaves identically to normal
/// painting.  Each item's transform is content-relative; `base` maps content
/// space into the target's device (or tile) space (R7/R9).
fn render_content_items(
    items: &[MaskItem],
    base: Transform,
    paint_servers: &PaintServerTable,
    target: &mut RasterTarget<'_>,
) {
    for item in items {
        render_shape(
            &item.geometry,
            &base.concat(item.transform),
            &item.style,
            item.length_bases,
            item.path_length,
            paint_servers,
            target,
        );
    }
}

/// Multiply a premultiplied RGBA buffer in place by a coverage mask (all four
/// channels scale, preserving premultiplication).
fn apply_mask_to_offscreen(buf: &mut [u8], mask: &ClipMask, w: usize, h: usize) {
    if mask.width != w || mask.height != h {
        return;
    }
    for i in 0..(w * h) {
        let m = mask.alpha[i] as u16;
        for c in 0..4 {
            let idx = i * 4 + c;
            buf[idx] = ((buf[idx] as u16 * m + 127) / 255) as u8;
        }
    }
}

// ---------------------------------------------------------------------------
// R9: markers (start/mid/end symbols placed on path vertices)
// ---------------------------------------------------------------------------

/// `markerUnits` (R9). `strokeWidth` scales marker content by the referencing
/// element's stroke width; `userSpaceOnUse` leaves it at 1:1.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkerUnits {
    StrokeWidth,
    UserSpaceOnUse,
}

/// `orient` (R9). `auto` aligns to the vertex tangent, `auto-start-reverse`
/// additionally flips the start marker, and a fixed angle is in radians.
#[derive(Clone, Copy)]
enum MarkerOrient {
    Auto,
    AutoStartReverse,
    Angle(f64),
}

/// Which path vertex a marker sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkerRole {
    Start,
    Mid,
    End,
}

/// A resolved `<marker>` definition: its lowered content plus viewport/orient
/// parameters (R9).
struct MarkerDef {
    items: Vec<MaskItem>,
    marker_w: f64,
    marker_h: f64,
    ref_x: f64,
    ref_y: f64,
    view_box: Option<[f64; 4]>,
    aspect: svg_core::SvgPreserveAspectRatio,
    units: MarkerUnits,
    orient: MarkerOrient,
    overflow_hidden: bool,
}

/// A raw marker vertex during extraction: position plus optional incoming and
/// outgoing tangent directions (before role assignment / angle resolution).
type MarkerRawVertex = ((f64, f64), Option<(f64, f64)>, Option<(f64, f64)>);

/// One path vertex with its auto-orient tangent angle and role.
struct MarkerVertex {
    pos: (f64, f64),
    angle: f64,
    role: MarkerRole,
}

/// One placed marker instance: the content→device transform, the device-space
/// viewport rect (for overflow clipping), and which resolved def to draw.
struct MarkerPlacement {
    def_index: usize,
    content_to_device: Transform,
    viewport_corners: Vec<(f32, f32)>,
    overflow_hidden: bool,
}

/// All markers resolved for one shape.
struct MarkerSet {
    defs: Vec<MarkerDef>,
    placements: Vec<MarkerPlacement>,
}

fn parse_marker_orient(value: Option<&str>) -> MarkerOrient {
    match value.map(str::trim) {
        Some("auto") => MarkerOrient::Auto,
        Some("auto-start-reverse") => MarkerOrient::AutoStartReverse,
        Some(v) => MarkerOrient::Angle(parse_angle_radians(v).unwrap_or(0.0)),
        None => MarkerOrient::Angle(0.0),
    }
}

/// Parse an SVG `<angle>` (number with optional deg/grad/rad unit; bare numbers
/// are degrees) into radians.
fn parse_angle_radians(value: &str) -> Option<f64> {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    let (num, to_rad): (&str, fn(f64) -> f64) = if let Some(n) = lower.strip_suffix("grad") {
        (n, |g| g * std::f64::consts::PI / 200.0)
    } else if let Some(n) = lower.strip_suffix("rad") {
        (n, |r| r)
    } else if let Some(n) = lower.strip_suffix("deg") {
        (n, f64::to_radians)
    } else {
        (lower.as_str(), f64::to_radians)
    };
    num.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .map(to_rad)
}

fn marker_ref(attrs: &[(String, String)], which: &str) -> Option<String> {
    final_style_property(attrs, which)
        .and_then(local_url_reference)
        .map(str::to_owned)
        .or_else(|| {
            final_style_property(attrs, "marker")
                .and_then(local_url_reference)
                .map(str::to_owned)
        })
}

fn resolve_marker(
    scene: &SvgScene,
    marker_id: &str,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> Option<MarkerDef> {
    let node = scene
        .references
        .by_xml_id
        .get(marker_id)
        .and_then(|id| scene.references.nodes_by_id.get(id));
    let Some(SvgNode::Unsupported {
        tag,
        attrs,
        children,
        ..
    }) = node
    else {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "marker.unresolved",
            message: "marker references an unavailable local id; no marker was drawn",
        });
        return None;
    };
    if tag != "marker" {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "marker.unresolved",
            message: "marker target is not a marker element; no marker was drawn",
        });
        return None;
    }
    let len = |key: &str| attr_get(attrs, key).and_then(svg_core::parse_length);
    let marker_w = len("markerwidth")
        .map(|l| l.value)
        .filter(|v| *v > 0.0)
        .unwrap_or(3.0);
    let marker_h = len("markerheight")
        .map(|l| l.value)
        .filter(|v| *v > 0.0)
        .unwrap_or(3.0);
    let ref_x = len("refx").map(|l| l.value).unwrap_or(0.0);
    let ref_y = len("refy").map(|l| l.value).unwrap_or(0.0);
    let view_box = parse_view_box(attrs);
    let aspect =
        svg_core::parse_preserve_aspect_ratio(attr_get(attrs, "preserveaspectratio").unwrap_or(""));
    let units = match attr_get(attrs, "markerunits").map(|v| v.trim().to_ascii_lowercase()) {
        Some(s) if s == "userspaceonuse" => MarkerUnits::UserSpaceOnUse,
        _ => MarkerUnits::StrokeWidth,
    };
    let orient = parse_marker_orient(attr_get(attrs, "orient"));
    // SVG markers default to overflow:hidden (content clipped to the viewport).
    let overflow_hidden = !attr_get(attrs, "overflow").is_some_and(|v| {
        let v = v.trim();
        v.eq_ignore_ascii_case("visible") || v.eq_ignore_ascii_case("auto")
    });
    let content_bases = match view_box {
        Some([_, _, w, h]) if w > 0.0 && h > 0.0 => SvgLengthBases::new(w.abs(), h.abs()),
        _ => SvgLengthBases::new(marker_w, marker_h),
    };
    let root_style = Style::default();
    let mut items = Vec::new();
    collect_mask_items(
        children,
        Transform::identity(),
        content_bases,
        &root_style,
        &scene.stylesheet,
        &mut items,
    );
    items.truncate(MAX_MARKER_CONTENT_ITEMS);
    Some(MarkerDef {
        items,
        marker_w,
        marker_h,
        ref_x,
        ref_y,
        view_box,
        aspect,
        units,
        orient,
        overflow_hidden,
    })
}

/// `Some(v)` if `v` has non-negligible length, else `None`.
fn nonzero_dir(d: (f64, f64)) -> Option<(f64, f64)> {
    if d.0.hypot(d.1) > 1.0e-9 {
        Some(d)
    } else {
        None
    }
}

/// Tangent directions (initial, final) and endpoint of one path segment.
fn segment_tangents(from: (f64, f64), seg: &PathSegment) -> ((f64, f64), (f64, f64), (f64, f64)) {
    let sub = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0, a.1 - b.1);
    let pick = |cands: &[(f64, f64)]| {
        cands
            .iter()
            .copied()
            .find(|d| nonzero_dir(*d).is_some())
            .unwrap_or(*cands.last().unwrap())
    };
    match seg {
        PathSegment::Line { to } => {
            let d = sub(*to, from);
            (d, d, *to)
        }
        PathSegment::Cubic { ctrl1, ctrl2, to } => {
            let init = pick(&[sub(*ctrl1, from), sub(*ctrl2, from), sub(*to, from)]);
            let finl = pick(&[sub(*to, *ctrl2), sub(*to, *ctrl1), sub(*to, from)]);
            (init, finl, *to)
        }
        PathSegment::Quadratic { ctrl, to } => {
            let init = pick(&[sub(*ctrl, from), sub(*to, from)]);
            let finl = pick(&[sub(*to, *ctrl), sub(*to, from)]);
            (init, finl, *to)
        }
        PathSegment::Arc { to, .. } => {
            let d = sub(*to, from);
            (d, d, *to)
        }
    }
}

/// Auto-orient angle (radians) at a vertex from its incoming/outgoing tangents.
fn auto_marker_angle(
    in_dir: Option<(f64, f64)>,
    out_dir: Option<(f64, f64)>,
    role: MarkerRole,
) -> f64 {
    let angle = |d: (f64, f64)| d.1.atan2(d.0);
    let unit = |d: (f64, f64)| {
        let l = d.0.hypot(d.1);
        (d.0 / l, d.1 / l)
    };
    match role {
        MarkerRole::Start => out_dir.or(in_dir).map(angle).unwrap_or(0.0),
        MarkerRole::End => in_dir.or(out_dir).map(angle).unwrap_or(0.0),
        MarkerRole::Mid => match (in_dir, out_dir) {
            (Some(i), Some(o)) => {
                let (i, o) = (unit(i), unit(o));
                let s = (i.0 + o.0, i.1 + o.1);
                nonzero_dir(s).map(angle).unwrap_or_else(|| angle(o))
            }
            (Some(i), None) => angle(i),
            (None, Some(o)) => angle(o),
            (None, None) => 0.0,
        },
    }
}

/// Assign Start/Mid/End roles by global position and compute auto angles.
fn finalize_marker_vertices(raw: Vec<MarkerRawVertex>) -> Vec<MarkerVertex> {
    let n = raw.len();
    raw.into_iter()
        .enumerate()
        .map(|(i, (pos, in_dir, out_dir))| {
            let role = if i == 0 {
                MarkerRole::Start
            } else if i + 1 == n {
                MarkerRole::End
            } else {
                MarkerRole::Mid
            };
            MarkerVertex {
                pos,
                angle: auto_marker_angle(in_dir, out_dir, role),
                role,
            }
        })
        .collect()
}

/// Extract marker vertices (with tangents) from a markable geometry. Markers
/// apply only to `line`/`polyline`/`polygon`/`path`; other shapes yield none.
fn marker_vertices(geometry: &ShapeGeometry) -> Vec<MarkerVertex> {
    let mut raw: Vec<MarkerRawVertex> = Vec::new();
    match geometry {
        ShapeGeometry::Line { from, to } => {
            let from = (from.0 as f64, from.1 as f64);
            let to = (to.0 as f64, to.1 as f64);
            let d = nonzero_dir((to.0 - from.0, to.1 - from.1));
            raw.push((from, None, d));
            raw.push((to, d, None));
        }
        ShapeGeometry::Poly { points, closed } => {
            let pts: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (x as f64, y as f64)).collect();
            push_polyline_vertices(&pts, *closed, &mut raw);
        }
        ShapeGeometry::Path { data } => {
            for sub in &data.subpaths {
                if raw.len() >= MAX_MARKER_PLACEMENTS {
                    break;
                }
                let mut sv: Vec<MarkerRawVertex> = vec![(sub.start, None, None)];
                let mut from = sub.start;
                for seg in &sub.segments {
                    let (init, finl, end) = segment_tangents(from, seg);
                    if let Some(last) = sv.last_mut() {
                        last.2 = last.2.or(nonzero_dir(init));
                    }
                    sv.push((end, nonzero_dir(finl), None));
                    from = end;
                }
                if sub.closed {
                    let d = nonzero_dir((sub.start.0 - from.0, sub.start.1 - from.1));
                    if let Some(last) = sv.last_mut() {
                        last.2 = last.2.or(d);
                    }
                    if let Some(first) = sv.first_mut() {
                        first.1 = first.1.or(d);
                    }
                }
                raw.extend(sv);
            }
        }
        _ => {}
    }
    raw.truncate(MAX_MARKER_PLACEMENTS);
    finalize_marker_vertices(raw)
}

fn push_polyline_vertices(pts: &[(f64, f64)], closed: bool, raw: &mut Vec<MarkerRawVertex>) {
    if pts.is_empty() {
        return;
    }
    let mut sv: Vec<MarkerRawVertex> = vec![(pts[0], None, None)];
    for pair in pts.windows(2) {
        let d = nonzero_dir((pair[1].0 - pair[0].0, pair[1].1 - pair[0].1));
        if let Some(last) = sv.last_mut() {
            last.2 = d;
        }
        sv.push((pair[1], d, None));
    }
    if closed {
        if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
            let d = nonzero_dir((first.0 - last.0, first.1 - last.1));
            if let Some(v) = sv.last_mut() {
                v.2 = v.2.or(d);
            }
            if let Some(v) = sv.first_mut() {
                v.1 = v.1.or(d);
            }
        }
    }
    raw.extend(sv);
}

fn push_marker_def(
    scene: &SvgScene,
    opt_id: Option<String>,
    defs: &mut Vec<MarkerDef>,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> Option<usize> {
    let id = opt_id?;
    let def = resolve_marker(scene, &id, diagnostics)?;
    defs.push(def);
    Some(defs.len() - 1)
}

fn marker_placement(
    def: &MarkerDef,
    def_index: usize,
    vertex: &MarkerVertex,
    stroke_w: f64,
    node_xform: &Transform,
) -> MarkerPlacement {
    let angle = match def.orient {
        MarkerOrient::Angle(a) => a,
        MarkerOrient::Auto => vertex.angle,
        MarkerOrient::AutoStartReverse => {
            if vertex.role == MarkerRole::Start {
                vertex.angle + std::f64::consts::PI
            } else {
                vertex.angle
            }
        }
    };
    let mut m = Transform::translate(vertex.pos.0, vertex.pos.1)
        .multiply(Transform::rotate(angle.to_degrees()));
    if def.units == MarkerUnits::StrokeWidth {
        m = m.multiply(Transform::scale(stroke_w, stroke_w));
    }
    let (content_to_user, viewport_to_user) = if let Some(vb) = def.view_box {
        let vb_ts =
            svg_core::viewbox_transform(vb, [0.0, 0.0, def.marker_w, def.marker_h], def.aspect)
                .unwrap_or_else(Transform::identity);
        let (rx, ry) = vb_ts.apply(def.ref_x, def.ref_y);
        let m = m.multiply(Transform::translate(-rx, -ry));
        (m.multiply(vb_ts), m)
    } else {
        let m = m.multiply(Transform::translate(-def.ref_x, -def.ref_y));
        (m, m)
    };
    let content_to_device = node_xform.concat(content_to_user);
    let viewport_to_device = node_xform.concat(viewport_to_user);
    let corners = vec![
        viewport_to_device.apply_f32(0.0, 0.0),
        viewport_to_device.apply_f32(def.marker_w as f32, 0.0),
        viewport_to_device.apply_f32(def.marker_w as f32, def.marker_h as f32),
        viewport_to_device.apply_f32(0.0, def.marker_h as f32),
    ];
    MarkerPlacement {
        def_index,
        content_to_device,
        viewport_corners: corners,
        overflow_hidden: def.overflow_hidden,
    }
}

/// Resolve and place start/mid/end markers for a shape. Returns `None` when the
/// shape is not markable or no marker references resolve. Bounded by
/// `MAX_MARKER_PLACEMENTS` with a `limit.marker_count` diagnostic on truncation.
fn build_markers(
    scene: &SvgScene,
    attrs: &[(String, String)],
    style: &Style,
    length_bases: SvgLengthBases,
    geometry: &ShapeGeometry,
    node_xform: &Transform,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> Option<Box<MarkerSet>> {
    let start_ref = marker_ref(attrs, "marker-start");
    let mid_ref = marker_ref(attrs, "marker-mid");
    let end_ref = marker_ref(attrs, "marker-end");
    if start_ref.is_none() && mid_ref.is_none() && end_ref.is_none() {
        return None;
    }
    let vertices = marker_vertices(geometry);
    if vertices.is_empty() {
        return None;
    }
    let mut defs = Vec::new();
    let start_i = push_marker_def(scene, start_ref, &mut defs, diagnostics);
    let mid_i = push_marker_def(scene, mid_ref, &mut defs, diagnostics);
    let end_i = push_marker_def(scene, end_ref, &mut defs, diagnostics);
    if defs.is_empty() {
        return None;
    }
    let stroke_w = resolve_stroke_length(style.stroke_width, length_bases)
        .filter(|w| *w > 0.0)
        .unwrap_or(1.0);
    let mut placements = Vec::new();
    let mut truncated = false;
    for vertex in &vertices {
        let def_index = match vertex.role {
            MarkerRole::Start => start_i,
            MarkerRole::Mid => mid_i,
            MarkerRole::End => end_i,
        };
        let Some(def_index) = def_index else {
            continue;
        };
        if placements.len() >= MAX_MARKER_PLACEMENTS {
            truncated = true;
            break;
        }
        placements.push(marker_placement(
            &defs[def_index],
            def_index,
            vertex,
            stroke_w,
            node_xform,
        ));
    }
    if truncated {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "limit.marker_count",
            message: "marker placements exceeded the renderer limit; remaining markers skipped",
        });
    }
    if placements.is_empty() {
        return None;
    }
    Some(Box::new(MarkerSet { defs, placements }))
}

// ---------------------------------------------------------------------------
// R7: filters (tier 1)
// ---------------------------------------------------------------------------

enum FilterInput {
    SourceGraphic,
    SourceAlpha,
    Reference(String),
    Previous,
}

enum FilterKind {
    GaussianBlur {
        sx: f64,
        sy: f64,
    },
    Offset {
        dx: f64,
        dy: f64,
    },
    Flood {
        color: [u8; 4],
    },
    Merge {
        inputs: Vec<FilterInput>,
    },
    ColorMatrix {
        m: [f32; 20],
    },
    DropShadow {
        dx: f64,
        dy: f64,
        sx: f64,
        sy: f64,
        color: [u8; 4],
    },
    /// R10 `feComposite` — Porter-Duff + arithmetic, on premultiplied pixels.
    Composite {
        op: CompositeOp,
        input2: FilterInput,
    },
    /// R10 `feBlend` — separable blend of `in` over `in2`.
    Blend {
        mode: BlendMode,
        input2: FilterInput,
    },
    /// R10 `feComponentTransfer` — per-channel transfer functions (R, G, B, A).
    ComponentTransfer {
        funcs: [TransferFunc; 4],
    },
    /// R10 `feMorphology` — dilate/erode over a bounded radius.
    Morphology {
        dilate: bool,
        rx: usize,
        ry: usize,
    },
    /// Tier-3: `feTile` — tile the input over the filter region.
    Tile,
    /// Tier-3: `feDisplacementMap` — displace pixels using a second input's channels.
    DisplacementMap {
        scale: f32,
        x_channel: u8,
        y_channel: u8,
        input2: FilterInput,
    },
    /// Tier-3: `feConvolveMatrix` — general convolution kernel.
    ConvolveMatrix {
        order_x: usize,
        order_y: usize,
        kernel: Vec<f32>,
        divisor: f32,
        bias: f32,
        target_x: usize,
        target_y: usize,
        edge_wrap: bool,
        preserve_alpha: bool,
    },
    /// Tier-3: `feTurbulence` / `feFractalNoise` — Perlin noise / fractal noise.
    Turbulence {
        base_freq_x: f64,
        base_freq_y: f64,
        num_octaves: u32,
        seed: i32,
        fractal_noise: bool,
        stitch: bool,
    },
    /// Tier-3: `feDiffuseLighting` — Lambertian diffuse shading from a bump map.
    DiffuseLighting {
        surface_scale: f32,
        diffuse_constant: f32,
        light: LightSource,
        lighting_color: [u8; 4],
    },
    /// Tier-3: `feSpecularLighting` — specular (Phong) shading from a bump map.
    SpecularLighting {
        surface_scale: f32,
        specular_constant: f32,
        specular_exponent: f32,
        light: LightSource,
        lighting_color: [u8; 4],
    },
    /// Tier-3: `feImage` with decoded RGBA pixels (data URI only; external diagnosed).
    Image {
        pixels: Vec<u8>,
        img_w: usize,
        img_h: usize,
    },
    /// Unsupported primitive passed through (partial output) with a diagnostic.
    Identity,
}

/// Light source for `feDiffuseLighting` / `feSpecularLighting`.
#[allow(dead_code)]
enum LightSource {
    Distant {
        azimuth: f32,
        elevation: f32,
    },
    Point {
        x: f32,
        y: f32,
        z: f32,
    },
    Spot {
        x: f32,
        y: f32,
        z: f32,
        px: f32,
        py: f32,
        pz: f32,
        limiting_cone_angle: f32,
        specular_exponent: f32,
    },
}

/// `feComposite` operator (R10). Inputs are premultiplied.
#[derive(Clone, Copy)]
enum CompositeOp {
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
}

/// Separable blend, shared by `feBlend` and `mix-blend-mode` (R10).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Darken,
    Lighten,
}

fn parse_blend_mode(value: &str) -> Option<BlendMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(BlendMode::Normal),
        "multiply" => Some(BlendMode::Multiply),
        "screen" => Some(BlendMode::Screen),
        "darken" => Some(BlendMode::Darken),
        "lighten" => Some(BlendMode::Lighten),
        _ => None,
    }
}

/// Per-channel `feComponentTransfer` transfer function (R10).
#[derive(Clone)]
enum TransferFunc {
    Identity,
    Table(Vec<f32>),
    Discrete(Vec<f32>),
    Linear {
        slope: f32,
        intercept: f32,
    },
    Gamma {
        amplitude: f32,
        exponent: f32,
        offset: f32,
    },
}

impl TransferFunc {
    /// Map a straight channel value in `[0,1]` through this function.
    fn apply(&self, c: f32) -> f32 {
        let c = c.clamp(0.0, 1.0);
        let out = match self {
            TransferFunc::Identity => c,
            TransferFunc::Linear { slope, intercept } => slope * c + intercept,
            TransferFunc::Gamma {
                amplitude,
                exponent,
                offset,
            } => amplitude * c.powf(*exponent) + offset,
            TransferFunc::Table(values) => {
                if values.is_empty() {
                    c
                } else if values.len() == 1 {
                    values[0]
                } else {
                    let n = values.len() - 1;
                    let scaled = c * n as f32;
                    let k = (scaled.floor() as usize).min(n - 1);
                    let frac = scaled - k as f32;
                    values[k] + frac * (values[k + 1] - values[k])
                }
            }
            TransferFunc::Discrete(values) => {
                if values.is_empty() {
                    c
                } else {
                    let n = values.len();
                    let k = ((c * n as f32).floor() as usize).min(n - 1);
                    values[k]
                }
            }
        };
        out.clamp(0.0, 1.0)
    }
}

struct FilterPrimitive {
    kind: FilterKind,
    input: FilterInput,
    result: Option<String>,
}

struct FilterGraph {
    primitives: Vec<FilterPrimitive>,
    /// `color-interpolation-filters` (R10): `true` = run in linearRGB (the SVG
    /// default), `false` = sRGB (`color-interpolation-filters: sRGB`).
    linear: bool,
    /// Filter region (R10): the result is clipped to this rect.
    region: FilterRegion,
}

/// Filter region from `filterUnits` + filter `x/y/width/height` (R10). The
/// result of the primitive graph is clipped to this rect.
#[derive(Clone, Copy)]
enum FilterRegion {
    /// `filterUnits="userSpaceOnUse"`: an explicit device-space rect
    /// `[x0, y0, x1, y1]`.
    UserSpace([f64; 4]),
    /// `filterUnits="objectBoundingBox"` (default): fractions of the element
    /// bounding box (default `-10% -10% 120% 120%`). Resolved in `apply` against
    /// the source content's device-space alpha extent.
    ObjectBoundingBox { fx: f64, fy: f64, fw: f64, fh: f64 },
}

impl Default for FilterRegion {
    fn default() -> Self {
        FilterRegion::ObjectBoundingBox {
            fx: -0.1,
            fy: -0.1,
            fw: 1.2,
            fh: 1.2,
        }
    }
}

impl FilterGraph {
    /// Run the primitive graph over a premultiplied source-graphic buffer,
    /// returning the premultiplied result. Bounded by buffer size and primitive
    /// count; never panics.
    ///
    /// R10: when `self.linear`, the source is converted sRGB->linearRGB
    /// (premultiplied-aware) before the graph and back to sRGB after, and any
    /// sRGB-specified primitive colour (`feFlood`/`feDropShadow`) is linearised
    /// so all per-channel math runs in linear light.
    fn apply(&self, source_srgb: &[u8], w: usize, h: usize) -> Vec<u8> {
        let owned_linear;
        let source: &[u8] = if self.linear {
            owned_linear = srgb_to_linear_premul(source_srgb);
            &owned_linear
        } else {
            source_srgb
        };
        let source_alpha = source_alpha_buffer(source);
        let mut named: HashMap<String, Vec<u8>> = HashMap::new();
        let mut previous = source.to_vec();
        for prim in &self.primitives {
            let input = resolve_filter_input(&prim.input, source, &source_alpha, &named, &previous);
            let out = match &prim.kind {
                FilterKind::GaussianBlur { sx, sy } => gaussian_blur(&input, w, h, *sx, *sy),
                FilterKind::Offset { dx, dy } => offset_buffer(&input, w, h, *dx, *dy),
                FilterKind::Flood { color } => {
                    flood_buffer(linearize_color(*color, self.linear), w, h)
                }
                FilterKind::ColorMatrix { m } => color_matrix(&input, m),
                FilterKind::Merge { inputs } => {
                    let mut acc = vec![0u8; w * h * 4];
                    for fin in inputs {
                        let layer =
                            resolve_filter_input(fin, source, &source_alpha, &named, &previous);
                        composite_premultiplied_over(&mut acc, &layer);
                    }
                    acc
                }
                FilterKind::DropShadow {
                    dx,
                    dy,
                    sx,
                    sy,
                    color,
                } => {
                    let mut shadow =
                        flood_masked(&source_alpha, linearize_color(*color, self.linear));
                    shadow = gaussian_blur(&shadow, w, h, *sx, *sy);
                    shadow = offset_buffer(&shadow, w, h, *dx, *dy);
                    composite_premultiplied_over(&mut shadow, source);
                    shadow
                }
                FilterKind::Composite { op, input2 } => {
                    let in2 =
                        resolve_filter_input(input2, source, &source_alpha, &named, &previous);
                    composite_filter(&input, &in2, *op)
                }
                FilterKind::Blend { mode, input2 } => {
                    let in2 =
                        resolve_filter_input(input2, source, &source_alpha, &named, &previous);
                    blend_filter(&input, &in2, *mode)
                }
                FilterKind::ComponentTransfer { funcs } => component_transfer(&input, funcs),
                FilterKind::Morphology { dilate, rx, ry } => {
                    morphology(&input, w, h, *dilate, *rx, *ry)
                }
                FilterKind::Tile => filter_tile(&input, w, h),
                FilterKind::DisplacementMap {
                    scale,
                    x_channel,
                    y_channel,
                    input2,
                } => {
                    let map =
                        resolve_filter_input(input2, source, &source_alpha, &named, &previous);
                    displacement_map(&input, &map, w, h, *scale, *x_channel, *y_channel)
                }
                FilterKind::ConvolveMatrix {
                    order_x,
                    order_y,
                    kernel,
                    divisor,
                    bias,
                    target_x,
                    target_y,
                    edge_wrap,
                    preserve_alpha,
                } => convolve_matrix(
                    &input,
                    w,
                    h,
                    *order_x,
                    *order_y,
                    kernel,
                    *divisor,
                    *bias,
                    *target_x,
                    *target_y,
                    *edge_wrap,
                    *preserve_alpha,
                ),
                FilterKind::Turbulence {
                    base_freq_x,
                    base_freq_y,
                    num_octaves,
                    seed,
                    fractal_noise,
                    stitch,
                } => turbulence_buffer(
                    w,
                    h,
                    *base_freq_x,
                    *base_freq_y,
                    *num_octaves,
                    *seed,
                    *fractal_noise,
                    *stitch,
                ),
                FilterKind::DiffuseLighting {
                    surface_scale,
                    diffuse_constant,
                    light,
                    lighting_color,
                } => diffuse_lighting(
                    &input,
                    w,
                    h,
                    *surface_scale,
                    *diffuse_constant,
                    light,
                    *lighting_color,
                ),
                FilterKind::SpecularLighting {
                    surface_scale,
                    specular_constant,
                    specular_exponent,
                    light,
                    lighting_color,
                } => specular_lighting(
                    &input,
                    w,
                    h,
                    *surface_scale,
                    *specular_constant,
                    *specular_exponent,
                    light,
                    *lighting_color,
                ),
                FilterKind::Image {
                    pixels,
                    img_w,
                    img_h,
                } => scale_image_to(pixels, *img_w, *img_h, w, h),
                FilterKind::Identity => input.clone(),
            };
            if let Some(name) = &prim.result {
                named.insert(name.clone(), out.clone());
            }
            previous = out;
        }
        let mut result = if self.linear {
            linear_to_srgb_premul(&previous)
        } else {
            previous
        };
        clip_to_filter_region(&mut result, w, h, self.region, source_srgb);
        result
    }
}

/// Device-space alpha bounding box `[minx, miny, maxx, maxy]` of a buffer, or
/// `None` when fully transparent.
fn alpha_extent(buf: &[u8], w: usize, h: usize) -> Option<[usize; 4]> {
    let (mut minx, mut miny, mut maxx, mut maxy) = (usize::MAX, usize::MAX, 0usize, 0usize);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if buf[(y * w + x) * 4 + 3] != 0 {
                any = true;
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            }
        }
    }
    any.then_some([minx, miny, maxx, maxy])
}

/// Zero every pixel of `buf` outside the resolved filter region (R10). For
/// objectBoundingBox units the element bbox is taken from the source content's
/// device-space alpha extent (a deterministic proxy that also works for groups,
/// which have no single geometric bbox).
fn clip_to_filter_region(
    buf: &mut [u8],
    w: usize,
    h: usize,
    region: FilterRegion,
    source_srgb: &[u8],
) {
    let rect = match region {
        FilterRegion::UserSpace(rect) => rect,
        FilterRegion::ObjectBoundingBox { fx, fy, fw, fh } => {
            let Some([minx, miny, maxx, maxy]) = alpha_extent(source_srgb, w, h) else {
                return; // no source content — nothing to bound
            };
            let (bx, by) = (minx as f64, miny as f64);
            let bw = (maxx - minx + 1) as f64;
            let bh = (maxy - miny + 1) as f64;
            [
                bx + fx * bw,
                by + fy * bh,
                bx + (fx + fw) * bw,
                by + (fy + fh) * bh,
            ]
        }
    };
    let (x0, y0, x1, y1) = (rect[0], rect[1], rect[2], rect[3]);
    for y in 0..h {
        let py = y as f64 + 0.5;
        for x in 0..w {
            let px = x as f64 + 0.5;
            if px < x0 || px >= x1 || py < y0 || py >= y1 {
                let idx = (y * w + x) * 4;
                buf[idx..idx + 4].fill(0);
            }
        }
    }
}

/// sRGB transfer (component in `[0,1]`) -> linear light.
#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light (component in `[0,1]`) -> sRGB transfer.
#[inline]
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Convert a premultiplied sRGB buffer to premultiplied linearRGB (alpha kept).
fn srgb_to_linear_premul(buf: &[u8]) -> Vec<u8> {
    convert_premul(buf, srgb_to_linear)
}

/// Convert a premultiplied linearRGB buffer back to premultiplied sRGB.
fn linear_to_srgb_premul(buf: &[u8]) -> Vec<u8> {
    convert_premul(buf, linear_to_srgb)
}

/// Apply a colour-transfer function to each RGB channel of a premultiplied
/// buffer (unpremultiply -> transfer -> re-premultiply); alpha is unchanged.
fn convert_premul(buf: &[u8], transfer: fn(f32) -> f32) -> Vec<u8> {
    let mut out = vec![0u8; buf.len()];
    for (px, dst) in buf.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        let a = px[3] as f32 / 255.0;
        for c in 0..3 {
            let straight = if a > 0.0 {
                px[c] as f32 / 255.0 / a
            } else {
                0.0
            };
            dst[c] = (transfer(straight.clamp(0.0, 1.0)) * a * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        dst[3] = px[3];
    }
    out
}

/// Linearise an sRGB-specified `[u8;4]` colour (RGB only) when running a filter
/// graph in linearRGB; a no-op in sRGB mode.
fn linearize_color(color: [u8; 4], linear: bool) -> [u8; 4] {
    if !linear {
        return color;
    }
    let conv = |c: u8| {
        (srgb_to_linear(c as f32 / 255.0) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [conv(color[0]), conv(color[1]), conv(color[2]), color[3]]
}

fn resolve_filter_input(
    input: &FilterInput,
    source: &[u8],
    source_alpha: &[u8],
    named: &HashMap<String, Vec<u8>>,
    previous: &[u8],
) -> Vec<u8> {
    match input {
        FilterInput::SourceGraphic => source.to_vec(),
        FilterInput::SourceAlpha => source_alpha.to_vec(),
        FilterInput::Reference(name) => named
            .get(name)
            .cloned()
            .unwrap_or_else(|| previous.to_vec()),
        FilterInput::Previous => previous.to_vec(),
    }
}

/// SourceAlpha = the source's alpha with zero RGB (premultiplied black).
fn source_alpha_buffer(source: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; source.len()];
    let mut i = 3;
    while i < source.len() {
        out[i] = source[i];
        i += 4;
    }
    out
}

/// Recolor a premultiplied alpha buffer with a flood colour (for drop shadow).
fn flood_masked(alpha_buf: &[u8], color: [u8; 4]) -> Vec<u8> {
    let mut out = vec![0u8; alpha_buf.len()];
    let ca = color[3] as u32;
    let mut i = 0;
    while i + 3 < alpha_buf.len() {
        let a = (alpha_buf[i + 3] as u32 * ca + 127) / 255; // combined alpha
        out[i] = ((color[0] as u32 * a + 127) / 255) as u8;
        out[i + 1] = ((color[1] as u32 * a + 127) / 255) as u8;
        out[i + 2] = ((color[2] as u32 * a + 127) / 255) as u8;
        out[i + 3] = a as u8;
        i += 4;
    }
    out
}

fn flood_buffer(color: [u8; 4], w: usize, h: usize) -> Vec<u8> {
    let a = color[3] as u32;
    let pr = ((color[0] as u32 * a + 127) / 255) as u8;
    let pg = ((color[1] as u32 * a + 127) / 255) as u8;
    let pb = ((color[2] as u32 * a + 127) / 255) as u8;
    let mut out = vec![0u8; w * h * 4];
    for px in out.chunks_exact_mut(4) {
        px[0] = pr;
        px[1] = pg;
        px[2] = pb;
        px[3] = color[3];
    }
    out
}

fn offset_buffer(src: &[u8], w: usize, h: usize, dx: f64, dy: f64) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let dxi = dx.round() as i64;
    let dyi = dy.round() as i64;
    for y in 0..h as i64 {
        let sy = y - dyi;
        if sy < 0 || sy >= h as i64 {
            continue;
        }
        for x in 0..w as i64 {
            let sx = x - dxi;
            if sx < 0 || sx >= w as i64 {
                continue;
            }
            let di = ((y as usize) * w + x as usize) * 4;
            let si = ((sy as usize) * w + sx as usize) * 4;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

/// Separable triple box blur approximating a Gaussian on a premultiplied buffer.
fn gaussian_blur(src: &[u8], w: usize, h: usize, sx: f64, sy: f64) -> Vec<u8> {
    let rx = blur_radius(sx);
    let ry = blur_radius(sy);
    let mut buf = src.to_vec();
    if rx > 0 {
        for _ in 0..3 {
            buf = box_blur_h(&buf, w, h, rx);
        }
    }
    if ry > 0 {
        for _ in 0..3 {
            buf = box_blur_v(&buf, w, h, ry);
        }
    }
    buf
}

fn blur_radius(std_dev: f64) -> usize {
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return 0;
    }
    // d ~= stdDev * 3 * sqrt(2*pi) / 4, per the SVG box-blur approximation note.
    let d = (std_dev * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0).round() as usize;
    (d / 2).min(MAX_BLUR_RADIUS)
}

fn box_blur_h(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    let span = (2 * r + 1) as u32;
    for y in 0..h {
        for c in 0..4 {
            let mut sum: u32 = 0;
            for x in 0..(r.min(w)) {
                sum += src[(y * w + x) * 4 + c] as u32;
            }
            for x in 0..w {
                let add = x + r;
                if add < w {
                    sum += src[(y * w + add) * 4 + c] as u32;
                }
                let sub = x as i64 - r as i64 - 1;
                if sub >= 0 {
                    sum -= src[(y * w + sub as usize) * 4 + c] as u32;
                }
                out[(y * w + x) * 4 + c] = ((sum + span / 2) / span) as u8;
            }
        }
    }
    out
}

fn box_blur_v(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    let span = (2 * r + 1) as u32;
    for x in 0..w {
        for c in 0..4 {
            let mut sum: u32 = 0;
            for y in 0..(r.min(h)) {
                sum += src[(y * w + x) * 4 + c] as u32;
            }
            for y in 0..h {
                let add = y + r;
                if add < h {
                    sum += src[(add * w + x) * 4 + c] as u32;
                }
                let sub = y as i64 - r as i64 - 1;
                if sub >= 0 {
                    sum -= src[(sub as usize * w + x) * 4 + c] as u32;
                }
                out[(y * w + x) * 4 + c] = ((sum + span / 2) / span) as u8;
            }
        }
    }
    out
}

/// Apply a 4x5 colour matrix. Operates on straight RGBA (unpremultiply → matrix
/// → clamp → premultiply) so it matches the SVG definition.
fn color_matrix(src: &[u8], m: &[f32; 20]) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for (px, dst) in src.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        let a = px[3] as f32 / 255.0;
        let (r, g, b) = if a > 0.0 {
            (
                px[0] as f32 / 255.0 / a,
                px[1] as f32 / 255.0 / a,
                px[2] as f32 / 255.0 / a,
            )
        } else {
            (0.0, 0.0, 0.0)
        };
        let nr = (m[0] * r + m[1] * g + m[2] * b + m[3] * a + m[4]).clamp(0.0, 1.0);
        let ng = (m[5] * r + m[6] * g + m[7] * b + m[8] * a + m[9]).clamp(0.0, 1.0);
        let nb = (m[10] * r + m[11] * g + m[12] * b + m[13] * a + m[14]).clamp(0.0, 1.0);
        let na = (m[15] * r + m[16] * g + m[17] * b + m[18] * a + m[19]).clamp(0.0, 1.0);
        dst[0] = (nr * na * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[1] = (ng * na * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[2] = (nb * na * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[3] = (na * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// src-over compositing of one premultiplied layer onto a premultiplied accumulator.
fn composite_premultiplied_over(dst: &mut [u8], src: &[u8]) {
    let n = dst.len().min(src.len());
    let mut i = 0;
    while i + 3 < n {
        let sa = src[i + 3] as u32;
        let inv = 255 - sa;
        for c in 0..4 {
            dst[i + c] = (src[i + c] as u32 + (dst[i + c] as u32 * inv + 127) / 255).min(255) as u8;
        }
        i += 4;
    }
}

/// R10 `feComposite`: Porter-Duff and arithmetic compositing of premultiplied
/// `in` (i) and `in2` (i2). Channels are treated in `[0,1]` premultiplied.
fn composite_filter(i: &[u8], i2: &[u8], op: CompositeOp) -> Vec<u8> {
    let n = i.len().min(i2.len());
    let mut out = vec![0u8; n];
    let mut p = 0;
    while p + 3 < n {
        let sa = i[p + 3] as f32 / 255.0;
        let da = i2[p + 3] as f32 / 255.0;
        for c in 0..4 {
            let s = i[p + c] as f32 / 255.0;
            let d = i2[p + c] as f32 / 255.0;
            let v = match op {
                CompositeOp::Over => s + d * (1.0 - sa),
                CompositeOp::In => s * da,
                CompositeOp::Out => s * (1.0 - da),
                CompositeOp::Atop => s * da + d * (1.0 - sa),
                CompositeOp::Xor => s * (1.0 - da) + d * (1.0 - sa),
                CompositeOp::Arithmetic { k1, k2, k3, k4 } => k1 * s * d + k2 * s + k3 * d + k4,
            };
            out[p + c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        // Keep alpha consistent with the premultiplied colour (clamp rgb <= a).
        let a = out[p + 3];
        for c in 0..3 {
            if out[p + c] > a {
                out[p + c] = a;
            }
        }
        p += 4;
    }
    out
}

/// R10 separable blend `B(cb, cs)` for `feBlend` / `mix-blend-mode`.
#[inline]
fn blend_channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => cb + cs - cb * cs,
        BlendMode::Darken => cb.min(cs),
        BlendMode::Lighten => cb.max(cs),
    }
}

/// R10 `feBlend`: blend premultiplied source `i` over backdrop `i2`.
/// Uses the premultiplied separable-blend formula:
///   Co = (1-ab)*Cs + (1-as)*Cb + as*ab*B(Cs/as, Cb/ab)
///   Ao = as + ab - as*ab
fn blend_filter(i: &[u8], i2: &[u8], mode: BlendMode) -> Vec<u8> {
    let n = i.len().min(i2.len());
    let mut out = vec![0u8; n];
    let mut p = 0;
    while p + 3 < n {
        let asrc = i[p + 3] as f32 / 255.0;
        let aback = i2[p + 3] as f32 / 255.0;
        let ao = asrc + aback - asrc * aback;
        for c in 0..3 {
            let scp = i[p + c] as f32 / 255.0; // premultiplied source channel
            let bcp = i2[p + c] as f32 / 255.0; // premultiplied backdrop channel
            let sc = if asrc > 0.0 { scp / asrc } else { 0.0 };
            let bc = if aback > 0.0 { bcp / aback } else { 0.0 };
            let co = (1.0 - aback) * scp
                + (1.0 - asrc) * bcp
                + asrc * aback * blend_channel(mode, bc, sc);
            out[p + c] = (co.clamp(0.0, ao.max(0.0)).min(1.0) * 255.0).round() as u8;
        }
        out[p + 3] = (ao.clamp(0.0, 1.0) * 255.0).round() as u8;
        p += 4;
    }
    out
}

/// R10 `feComponentTransfer`: per-channel transfer on straight (unpremultiplied)
/// colour, re-premultiplied at the end.
fn component_transfer(src: &[u8], funcs: &[TransferFunc; 4]) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for (px, dst) in src.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        let a = px[3] as f32 / 255.0;
        let straight = |c: usize| {
            if a > 0.0 {
                px[c] as f32 / 255.0 / a
            } else {
                0.0
            }
        };
        let nr = funcs[0].apply(straight(0));
        let ng = funcs[1].apply(straight(1));
        let nb = funcs[2].apply(straight(2));
        let na = funcs[3].apply(a);
        dst[0] = (nr * na * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[1] = (ng * na * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[2] = (nb * na * 255.0).round().clamp(0.0, 255.0) as u8;
        dst[3] = (na * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// R10 `feMorphology`: dilate (max) or erode (min) each premultiplied channel
/// over a `(2rx+1) x (2ry+1)` window. Radii are capped by the caller.
fn morphology(src: &[u8], w: usize, h: usize, dilate: bool, rx: usize, ry: usize) -> Vec<u8> {
    if (rx == 0 && ry == 0) || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        let y0 = y.saturating_sub(ry);
        let y1 = (y + ry).min(h - 1);
        for x in 0..w {
            let x0 = x.saturating_sub(rx);
            let x1 = (x + rx).min(w - 1);
            for c in 0..4 {
                let mut acc: u8 = if dilate { 0 } else { 255 };
                for yy in y0..=y1 {
                    let row = yy * w;
                    for xx in x0..=x1 {
                        let v = src[(row + xx) * 4 + c];
                        acc = if dilate { acc.max(v) } else { acc.min(v) };
                    }
                }
                out[(y * w + x) * 4 + c] = acc;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tier-3 filter primitive implementations
// ---------------------------------------------------------------------------

/// feTile: repeat the input buffer over the filter region by tiling.
fn filter_tile(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    if w == 0 || h == 0 || src.len() < w * h * 4 {
        return src.to_vec();
    }
    let tw = w.max(1);
    let th = h.max(1);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let sy = y % th;
        for x in 0..w {
            let sx = x % tw;
            let dst = (y * w + x) * 4;
            let s = (sy * tw + sx) * 4;
            if s + 3 < src.len() {
                out[dst..dst + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
    }
    out
}

/// feDisplacementMap: move each pixel by amounts derived from a displacement map.
/// Channels: 0=R, 1=G, 2=B, 3=A.
fn displacement_map(
    src: &[u8],
    map: &[u8],
    w: usize,
    h: usize,
    scale: f32,
    x_ch: u8,
    y_ch: u8,
) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let half = scale / 2.0;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if i + 3 >= map.len() {
                continue;
            }
            let dx = (map[i + x_ch as usize] as f32 / 255.0) * scale - half;
            let dy = (map[i + y_ch as usize] as f32 / 255.0) * scale - half;
            let sx = x as f32 - dx;
            let sy = y as f32 - dy;
            let sx0 = (sx as isize).clamp(0, w as isize - 1) as usize;
            let sy0 = (sy as isize).clamp(0, h as isize - 1) as usize;
            let s = (sy0 * w + sx0) * 4;
            if s + 3 < src.len() {
                let d = (y * w + x) * 4;
                out[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
    }
    out
}

/// feConvolveMatrix: general NxM convolution kernel with edge clamping or wrap.
#[allow(clippy::too_many_arguments)]
fn convolve_matrix(
    src: &[u8],
    w: usize,
    h: usize,
    kw: usize,
    kh: usize,
    kernel: &[f32],
    divisor: f32,
    bias: f32,
    target_x: usize,
    target_y: usize,
    edge_wrap: bool,
    preserve_alpha: bool,
) -> Vec<u8> {
    if kw == 0 || kh == 0 || w == 0 || h == 0 || kernel.len() < kw * kh {
        return src.to_vec();
    }
    let divisor = if divisor == 0.0 { 1.0 } else { divisor };
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for ky in 0..kh {
                for kx in 0..kw {
                    let ki = ky * kw + kx;
                    let k = kernel[ki];
                    let sx = x as isize + kx as isize - target_x as isize;
                    let sy = y as isize + ky as isize - target_y as isize;
                    let (sx, sy) = if edge_wrap {
                        (
                            sx.rem_euclid(w as isize) as usize,
                            sy.rem_euclid(h as isize) as usize,
                        )
                    } else {
                        (
                            sx.clamp(0, w as isize - 1) as usize,
                            sy.clamp(0, h as isize - 1) as usize,
                        )
                    };
                    let si = (sy * w + sx) * 4;
                    if si + 3 < src.len() {
                        for c in 0..4 {
                            acc[c] += src[si + c] as f32 * k;
                        }
                    }
                }
            }
            let dst = (y * w + x) * 4;
            let src_alpha = src[dst + 3];
            for c in 0..4 {
                if preserve_alpha && c == 3 {
                    out[dst + 3] = src_alpha;
                } else {
                    let v = (acc[c] / divisor + bias * 255.0).round().clamp(0.0, 255.0) as u8;
                    out[dst + c] = v;
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// feTurbulence — SVG spec §15.20 Perlin/fractal noise (no external deps)
// ---------------------------------------------------------------------------

const TURBULENCE_TABLE_SIZE: usize = 256;
const TURBULENCE_B: usize = TURBULENCE_TABLE_SIZE;
const TURBULENCE_N: i32 = 0x1000;

struct TurbulenceState {
    lattice: [i32; TURBULENCE_B + TURBULENCE_B + 2],
    gradient: [[f64; 2]; TURBULENCE_B + TURBULENCE_B + 2],
}

fn turbulence_setup(seed: i32) -> TurbulenceState {
    let mut b = [0i32; TURBULENCE_B + TURBULENCE_B + 2];
    let mut g = [[0.0f64; 2]; TURBULENCE_B + TURBULENCE_B + 2];
    let mut s = TurbulenceRng::new(seed);
    for i in 0..TURBULENCE_B {
        b[i] = i as i32;
        for g_ch in &mut g[i] {
            loop {
                let v = (s.next() & (2 * TURBULENCE_N + 1) as u32) as f64 - TURBULENCE_N as f64;
                if v != 0.0 {
                    *g_ch = v;
                    break;
                }
            }
        }
        let mag = (g[i][0] * g[i][0] + g[i][1] * g[i][1]).sqrt();
        if mag > 0.0 {
            g[i][0] /= mag;
            g[i][1] /= mag;
        }
    }
    for i in (1..TURBULENCE_B).rev() {
        let j = (s.next() % TURBULENCE_B as u32) as usize;
        b.swap(i, j);
    }
    for i in 0..TURBULENCE_B + 2 {
        b[TURBULENCE_B + i] = b[i];
        g[TURBULENCE_B + i] = g[i];
    }
    TurbulenceState {
        lattice: b,
        gradient: g,
    }
}

struct TurbulenceRng(i32);
impl TurbulenceRng {
    fn new(seed: i32) -> Self {
        const LOW_BITS: i32 = 0xffff;
        let seed = if seed <= 0 {
            -(seed % (i32::MAX - 1)) + 1
        } else {
            seed
        };
        let mut s = TurbulenceRng(seed & LOW_BITS);
        let _ = s.next();
        s
    }
    fn next(&mut self) -> u32 {
        const RAND_M: i32 = 2147483647;
        const RAND_A: i32 = 16807;
        const RAND_Q: i32 = 127773;
        const RAND_R: i32 = 2836;
        let hi = self.0 / RAND_Q;
        let lo = self.0 % RAND_Q;
        let test = RAND_A * lo - RAND_R * hi;
        self.0 = if test > 0 { test } else { test + RAND_M };
        self.0 as u32
    }
}

fn turbulence_noise2(state: &TurbulenceState, tx: f64, ty: f64) -> f64 {
    #[inline(always)]
    fn s_curve(t: f64) -> f64 {
        t * t * (3.0 - 2.0 * t)
    }
    #[inline(always)]
    fn lerp(t: f64, a: f64, b: f64) -> f64 {
        a + t * (b - a)
    }
    let bx0 = (tx as i64).rem_euclid(TURBULENCE_B as i64) as usize;
    let bx1 = (bx0 + 1) % TURBULENCE_B;
    let by0 = (ty as i64).rem_euclid(TURBULENCE_B as i64) as usize;
    let by1 = (by0 + 1) % TURBULENCE_B;
    let rx0 = tx - tx.floor();
    let rx1 = rx0 - 1.0;
    let ry0 = ty - ty.floor();
    let ry1 = ry0 - 1.0;
    let sx = s_curve(rx0);
    let sy = s_curve(ry0);
    let i = state.lattice[bx0];
    let j = state.lattice[bx1];
    let b00 = state.lattice[(i + by0 as i32) as usize & (TURBULENCE_B - 1)];
    let b10 = state.lattice[(j + by0 as i32) as usize & (TURBULENCE_B - 1)];
    let b01 = state.lattice[(i + by1 as i32) as usize & (TURBULENCE_B - 1)];
    let b11 = state.lattice[(j + by1 as i32) as usize & (TURBULENCE_B - 1)];
    let g00 = state.gradient[b00 as usize];
    let g10 = state.gradient[b10 as usize];
    let g01 = state.gradient[b01 as usize];
    let g11 = state.gradient[b11 as usize];
    let u = rx0 * g00[0] + ry0 * g00[1];
    let v = rx1 * g10[0] + ry0 * g10[1];
    let a = lerp(sx, u, v);
    let u = rx0 * g01[0] + ry1 * g01[1];
    let v = rx1 * g11[0] + ry1 * g11[1];
    let b = lerp(sx, u, v);
    lerp(sy, a, b)
}

/// Compute one channel of feTurbulence/feFractalNoise per the SVG spec.
fn turbulence_channel(
    state: &TurbulenceState,
    x: f64,
    y: f64,
    bfx: f64,
    bfy: f64,
    num_octaves: u32,
    fractal_noise: bool,
) -> f64 {
    let mut sum = 0.0f64;
    let mut freq_x = bfx;
    let mut freq_y = bfy;
    let mut amp = 1.0f64;
    for _ in 0..num_octaves {
        let n = turbulence_noise2(state, x * freq_x, y * freq_y);
        sum += if fractal_noise { n } else { n.abs() };
        freq_x *= 2.0;
        freq_y *= 2.0;
        amp *= 0.5;
        let _ = amp;
    }
    sum
}

/// Build a feTurbulence/fractalNoise RGBA buffer (premultiplied sRGB).
/// Each channel is an independent noise call. Alpha is always 255 (opaque).
#[allow(clippy::too_many_arguments)]
fn turbulence_buffer(
    w: usize,
    h: usize,
    bfx: f64,
    bfy: f64,
    num_octaves: u32,
    seed: i32,
    fractal_noise: bool,
    _stitch: bool,
) -> Vec<u8> {
    let num_octaves = num_octaves.clamp(1, 8);
    let mut out = vec![255u8; w * h * 4];
    // Four independent states per the spec: one per channel R/G/B/A.
    let states: Vec<TurbulenceState> = (0..4).map(|ch| turbulence_setup(seed + ch)).collect();
    for y in 0..h {
        for x in 0..w {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            for ch in 0..4 {
                let raw =
                    turbulence_channel(&states[ch], px, py, bfx, bfy, num_octaves, fractal_noise);
                let val = if fractal_noise {
                    // fractalNoise: [-1, 1] → [0, 1]
                    ((raw + 1.0) * 0.5).clamp(0.0, 1.0)
                } else {
                    // turbulence: [0, n_octaves] → [0, 1]
                    raw.clamp(0.0, 1.0)
                };
                out[(y * w + x) * 4 + ch] = (val * 255.0).round() as u8;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Lighting filters (feDiffuseLighting / feSpecularLighting)
// ---------------------------------------------------------------------------

fn surface_normal(src: &[u8], w: usize, h: usize, x: usize, y: usize, scale: f32) -> [f32; 3] {
    let a = |cx: isize, cy: isize| -> f32 {
        let cx = cx.clamp(0, w as isize - 1) as usize;
        let cy = cy.clamp(0, h as isize - 1) as usize;
        src[(cy * w + cx) * 4] as f32 / 255.0
    };
    let xi = x as isize;
    let yi = y as isize;
    let nx = -(a(xi - 1, yi - 1) + 2.0 * a(xi - 1, yi) + a(xi - 1, yi + 1))
        + (a(xi + 1, yi - 1) + 2.0 * a(xi + 1, yi) + a(xi + 1, yi + 1));
    let ny = -(a(xi - 1, yi - 1) + 2.0 * a(xi, yi - 1) + a(xi + 1, yi - 1))
        + (a(xi - 1, yi + 1) + 2.0 * a(xi, yi + 1) + a(xi + 1, yi + 1));
    let nz = 1.0 / scale.max(0.001);
    let mag = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
    [nx / mag, ny / mag, nz / mag]
}

fn light_vector(light: &LightSource, x: f32, y: f32) -> [f32; 3] {
    match light {
        LightSource::Distant { azimuth, elevation } => {
            let az = azimuth.to_radians();
            let el = elevation.to_radians();
            [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()]
        }
        LightSource::Point {
            x: lx,
            y: ly,
            z: lz,
        }
        | LightSource::Spot {
            x: lx,
            y: ly,
            z: lz,
            ..
        } => {
            let dx = lx - x;
            let dy = ly - y;
            let mag = (dx * dx + dy * dy + lz * lz).sqrt().max(1e-6);
            [dx / mag, dy / mag, lz / mag]
        }
    }
}

fn diffuse_lighting(
    src: &[u8],
    w: usize,
    h: usize,
    surface_scale: f32,
    diffuse_constant: f32,
    light: &LightSource,
    color: [u8; 4],
) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let n = surface_normal(src, w, h, x, y, surface_scale);
            let l = light_vector(light, x as f32, y as f32);
            let dot = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.0);
            let factor = (diffuse_constant * dot).clamp(0.0, 1.0);
            let d = (y * w + x) * 4;
            out[d] = (color[0] as f32 * factor).round() as u8;
            out[d + 1] = (color[1] as f32 * factor).round() as u8;
            out[d + 2] = (color[2] as f32 * factor).round() as u8;
            out[d + 3] = 255;
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn specular_lighting(
    src: &[u8],
    w: usize,
    h: usize,
    surface_scale: f32,
    specular_constant: f32,
    specular_exponent: f32,
    light: &LightSource,
    color: [u8; 4],
) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    // Eye direction is always +Z in SVG spec.
    let eye = [0.0f32, 0.0, 1.0];
    for y in 0..h {
        for x in 0..w {
            let n = surface_normal(src, w, h, x, y, surface_scale);
            let l = light_vector(light, x as f32, y as f32);
            let h_vec = {
                let hx = l[0] + eye[0];
                let hy = l[1] + eye[1];
                let hz = l[2] + eye[2];
                let mag = (hx * hx + hy * hy + hz * hz).sqrt().max(1e-6);
                [hx / mag, hy / mag, hz / mag]
            };
            let n_dot_h = (n[0] * h_vec[0] + n[1] * h_vec[1] + n[2] * h_vec[2]).max(0.0);
            let factor = (specular_constant * n_dot_h.powf(specular_exponent)).clamp(0.0, 1.0);
            let d = (y * w + x) * 4;
            out[d] = (color[0] as f32 * factor).round() as u8;
            out[d + 1] = (color[1] as f32 * factor).round() as u8;
            out[d + 2] = (color[2] as f32 * factor).round() as u8;
            out[d + 3] = 255;
        }
    }
    out
}

/// Bilinear scale an RGBA image to target dimensions.
fn scale_image_to(pixels: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        for dx in 0..dw {
            let sx = (dx as f32 * (sw as f32 / dw as f32)).clamp(0.0, sw as f32 - 1.0) as usize;
            let sy = (dy as f32 * (sh as f32 / dh as f32)).clamp(0.0, sh as f32 - 1.0) as usize;
            let s = (sy * sw + sx) * 4;
            if s + 3 < pixels.len() {
                let d = (dy * dw + dx) * 4;
                out[d..d + 4].copy_from_slice(&pixels[s..s + 4]);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tier-3 filter parsers
// ---------------------------------------------------------------------------

fn parse_channel_selector(value: Option<&str>) -> u8 {
    match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("r") => 0,
        Some("g") => 1,
        Some("b") => 2,
        Some("a") => 3,
        _ => 0,
    }
}

fn parse_convolve_matrix(attrs: &[(String, String)]) -> FilterKind {
    let (kw, kh) = {
        let order = attr_get(attrs, "order").unwrap_or("3");
        let parts: Vec<f64> = order
            .split_ascii_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        let kw = parts.first().copied().unwrap_or(3.0).round() as usize;
        let kh = parts.get(1).copied().unwrap_or(kw as f64).round() as usize;
        (kw.min(25), kh.min(25))
    };
    let kernel: Vec<f32> = attr_get(attrs, "kernelmatrix")
        .unwrap_or("")
        .split_ascii_whitespace()
        .filter_map(|v| v.parse().ok())
        .take(kw * kh)
        .collect();
    let kernel_sum: f32 = kernel.iter().sum();
    let divisor = attr_get(attrs, "divisor")
        .and_then(parse_f64)
        .unwrap_or(if kernel_sum == 0.0 {
            1.0
        } else {
            kernel_sum as f64
        }) as f32;
    let bias = attr_get(attrs, "bias").and_then(parse_f64).unwrap_or(0.0) as f32;
    let target_x = attr_get(attrs, "targetx")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(kw / 2);
    let target_y = attr_get(attrs, "targety")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(kh / 2);
    let edge_wrap = attr_get(attrs, "edgemode")
        .map(|v| v.trim().eq_ignore_ascii_case("wrap"))
        .unwrap_or(false);
    let preserve_alpha = attr_get(attrs, "preservealpha")
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    FilterKind::ConvolveMatrix {
        order_x: kw,
        order_y: kh,
        kernel,
        divisor,
        bias,
        target_x,
        target_y,
        edge_wrap,
        preserve_alpha,
    }
}

fn parse_light_source(children: Option<&[SvgNode]>) -> LightSource {
    let Some(children) = children else {
        return LightSource::Distant {
            azimuth: 0.0,
            elevation: 0.0,
        };
    };
    for child in children {
        let (tag, attrs) = match child {
            SvgNode::Unsupported { tag, attrs, .. } => (tag.as_str(), attrs.as_slice()),
            _ => continue,
        };
        match tag {
            "fedistantlight" => {
                return LightSource::Distant {
                    azimuth: attr_get(attrs, "azimuth")
                        .and_then(parse_f64)
                        .unwrap_or(0.0) as f32,
                    elevation: attr_get(attrs, "elevation")
                        .and_then(parse_f64)
                        .unwrap_or(0.0) as f32,
                };
            }
            "fepointlight" => {
                return LightSource::Point {
                    x: attr_get(attrs, "x").and_then(parse_f64).unwrap_or(0.0) as f32,
                    y: attr_get(attrs, "y").and_then(parse_f64).unwrap_or(0.0) as f32,
                    z: attr_get(attrs, "z").and_then(parse_f64).unwrap_or(0.0) as f32,
                };
            }
            "fespotlight" => {
                return LightSource::Spot {
                    x: attr_get(attrs, "x").and_then(parse_f64).unwrap_or(0.0) as f32,
                    y: attr_get(attrs, "y").and_then(parse_f64).unwrap_or(0.0) as f32,
                    z: attr_get(attrs, "z").and_then(parse_f64).unwrap_or(0.0) as f32,
                    px: attr_get(attrs, "pointsatx")
                        .and_then(parse_f64)
                        .unwrap_or(0.0) as f32,
                    py: attr_get(attrs, "pointsaty")
                        .and_then(parse_f64)
                        .unwrap_or(0.0) as f32,
                    pz: attr_get(attrs, "pointsatz")
                        .and_then(parse_f64)
                        .unwrap_or(0.0) as f32,
                    limiting_cone_angle: attr_get(attrs, "limitingconeangle")
                        .and_then(parse_f64)
                        .unwrap_or(f64::INFINITY) as f32,
                    specular_exponent: attr_get(attrs, "specularexponent")
                        .and_then(parse_f64)
                        .unwrap_or(1.0) as f32,
                };
            }
            _ => {}
        }
    }
    LightSource::Distant {
        azimuth: 0.0,
        elevation: 0.0,
    }
}

fn parse_diffuse_lighting(child: &SvgNode, attrs: &[(String, String)]) -> FilterKind {
    let light = parse_light_source(child.children());
    FilterKind::DiffuseLighting {
        surface_scale: attr_get(attrs, "surfacescale")
            .and_then(parse_f64)
            .unwrap_or(1.0) as f32,
        diffuse_constant: attr_get(attrs, "diffuseconstant")
            .and_then(parse_f64)
            .unwrap_or(1.0) as f32,
        light,
        lighting_color: lighting_color(attrs),
    }
}

fn parse_specular_lighting(child: &SvgNode, attrs: &[(String, String)]) -> FilterKind {
    let light = parse_light_source(child.children());
    FilterKind::SpecularLighting {
        surface_scale: attr_get(attrs, "surfacescale")
            .and_then(parse_f64)
            .unwrap_or(1.0) as f32,
        specular_constant: attr_get(attrs, "specularconstant")
            .and_then(parse_f64)
            .unwrap_or(1.0) as f32,
        specular_exponent: (attr_get(attrs, "specularexponent")
            .and_then(parse_f64)
            .unwrap_or(1.0) as f32)
            .clamp(1.0, 128.0),
        light,
        lighting_color: lighting_color(attrs),
    }
}

fn lighting_color(attrs: &[(String, String)]) -> [u8; 4] {
    attr_get(attrs, "lighting-color")
        .and_then(|v| svg_core::parse_color(v.trim()))
        .map(|c| [c.r, c.g, c.b, c.a])
        .unwrap_or([255, 255, 255, 255])
}

fn parse_feimage(
    attrs: &[(String, String)],
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> FilterKind {
    let href = attr_get(attrs, "xlink:href")
        .or_else(|| attr_get(attrs, "href"))
        .unwrap_or("");
    if href.starts_with("data:") {
        match decode_image_href(href) {
            Ok(img) => {
                return FilterKind::Image {
                    pixels: img.rgba,
                    img_w: img.width,
                    img_h: img.height,
                };
            }
            Err(_) => {
                diagnostics.push(PendingDiagnostic::Warning {
                    code: "filter.image_decode_failed",
                    message: "feImage data URI could not be decoded; filter not applied",
                });
            }
        }
    } else if !href.is_empty() {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "filter.image_external",
            message: "feImage external URI not supported; filter not applied",
        });
    }
    FilterKind::Identity
}

fn parse_filter(
    scene: &SvgScene,
    filter_id: &str,
    ctm: Transform,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> Option<FilterGraph> {
    let node = scene
        .references
        .by_xml_id
        .get(filter_id)
        .and_then(|id| scene.references.nodes_by_id.get(id));
    let Some(SvgNode::Unsupported {
        tag,
        attrs: filter_attrs,
        children,
        ..
    }) = node
    else {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "filter.unresolved",
            message: "filter references an unavailable local id; no filter was applied",
        });
        return None;
    };
    if tag != "filter" {
        diagnostics.push(PendingDiagnostic::Warning {
            code: "filter.unresolved",
            message: "filter target is not a filter element; no filter was applied",
        });
        return None;
    }
    // `color-interpolation-filters` defaults to linearRGB; only an explicit
    // `sRGB` opts out (auto/linearRGB both stay linear).
    let linear = !final_style_property(filter_attrs, "color-interpolation-filters")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("sRGB"));
    let scale = affine_max_scale(ctm).max(1.0e-6);
    let mut primitives = Vec::new();
    for child in children {
        if primitives.len() >= MAX_FILTER_PRIMITIVES {
            diagnostics.push(PendingDiagnostic::Warning {
                code: "limit.filter_primitives",
                message:
                    "filter exceeded the renderer primitive limit; remaining primitives skipped",
            });
            break;
        }
        let SvgNode::Unsupported {
            tag: ptag, attrs, ..
        } = child
        else {
            continue;
        };
        let input = parse_filter_input(attr_get(attrs, "in"));
        let result = attr_get(attrs, "result").map(ToOwned::to_owned);
        let kind = match ptag.as_str() {
            "fegaussianblur" => {
                let (sx, sy) = parse_std_deviation(attr_get(attrs, "stddeviation"));
                FilterKind::GaussianBlur {
                    sx: sx * scale,
                    sy: sy * scale,
                }
            }
            "feoffset" => FilterKind::Offset {
                dx: attr_get(attrs, "dx").and_then(parse_f64).unwrap_or(0.0) * scale,
                dy: attr_get(attrs, "dy").and_then(parse_f64).unwrap_or(0.0) * scale,
            },
            "feflood" => FilterKind::Flood {
                color: flood_color(attrs),
            },
            "fecolormatrix" => FilterKind::ColorMatrix {
                m: parse_color_matrix(attrs),
            },
            "femerge" => FilterKind::Merge {
                inputs: child
                    .children()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|n| match n {
                        SvgNode::Unsupported { tag, attrs, .. } if tag == "femergenode" => {
                            Some(parse_filter_input(attr_get(attrs, "in")))
                        }
                        _ => None,
                    })
                    .collect(),
            },
            "fedropshadow" => {
                let (sx, sy) = parse_std_deviation(attr_get(attrs, "stddeviation"));
                FilterKind::DropShadow {
                    dx: attr_get(attrs, "dx").and_then(parse_f64).unwrap_or(2.0) * scale,
                    dy: attr_get(attrs, "dy").and_then(parse_f64).unwrap_or(2.0) * scale,
                    sx: sx * scale,
                    sy: sy * scale,
                    color: flood_color(attrs),
                }
            }
            "fecomposite" => FilterKind::Composite {
                op: parse_composite_op(attrs),
                input2: parse_filter_input(attr_get(attrs, "in2")),
            },
            "feblend" => {
                let mode = attr_get(attrs, "mode")
                    .and_then(parse_blend_mode)
                    .unwrap_or(BlendMode::Normal);
                FilterKind::Blend {
                    mode,
                    input2: parse_filter_input(attr_get(attrs, "in2")),
                }
            }
            "fecomponenttransfer" => FilterKind::ComponentTransfer {
                funcs: parse_component_transfer(child),
            },
            "femorphology" => {
                let dilate = attr_get(attrs, "operator")
                    .map(|v| v.trim().eq_ignore_ascii_case("dilate"))
                    .unwrap_or(false);
                let (rx, ry) = parse_std_deviation(attr_get(attrs, "radius"));
                FilterKind::Morphology {
                    dilate,
                    rx: ((rx * scale).round().max(0.0) as usize).min(MAX_MORPH_RADIUS),
                    ry: ((ry * scale).round().max(0.0) as usize).min(MAX_MORPH_RADIUS),
                }
            }
            "fetile" => FilterKind::Tile,
            "fedisplacementmap" => FilterKind::DisplacementMap {
                scale: attr_get(attrs, "scale").and_then(parse_f64).unwrap_or(0.0) as f32,
                x_channel: parse_channel_selector(attr_get(attrs, "xchannelselector")),
                y_channel: parse_channel_selector(attr_get(attrs, "ychannelselector")),
                input2: parse_filter_input(attr_get(attrs, "in2")),
            },
            "feconvolvematrix" => parse_convolve_matrix(attrs),
            "feturbulence" | "fefractalnoise" => {
                let (bfx, bfy) = parse_std_deviation(attr_get(attrs, "basefrequency"));
                FilterKind::Turbulence {
                    base_freq_x: bfx,
                    base_freq_y: bfy,
                    num_octaves: attr_get(attrs, "numoctaves")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(1),
                    seed: attr_get(attrs, "seed")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0),
                    fractal_noise: attr_get(attrs, "type")
                        .map(|v| v.trim().eq_ignore_ascii_case("fractalNoise"))
                        .unwrap_or(ptag == "fefractalnoise"),
                    stitch: attr_get(attrs, "stitchtiles")
                        .map(|v| v.trim().eq_ignore_ascii_case("stitch"))
                        .unwrap_or(false),
                }
            }
            "fediffuselighting" => parse_diffuse_lighting(child, attrs),
            "fespecularlighting" => parse_specular_lighting(child, attrs),
            "feimage" => parse_feimage(attrs, diagnostics),
            other => {
                diagnostics.push(PendingDiagnostic::Warning {
                    code: "filter.unsupported_primitive",
                    message: "an unsupported filter primitive was passed through (partial output)",
                });
                let _ = other;
                FilterKind::Identity
            }
        };
        primitives.push(FilterPrimitive {
            kind,
            input,
            result,
        });
    }
    if primitives.is_empty() {
        return None;
    }
    let region = parse_filter_region(filter_attrs, ctm, diagnostics);
    Some(FilterGraph {
        primitives,
        linear,
        region,
    })
}

/// Parse the filter region from `filterUnits` + filter `x/y/width/height` (R10).
/// objectBoundingBox (default) yields bbox fractions; userSpaceOnUse yields an
/// explicit device rect (percentage lengths there are approximated + diagnosed).
fn parse_filter_region(
    attrs: &[(String, String)],
    ctm: Transform,
    diagnostics: &mut Vec<PendingDiagnostic>,
) -> FilterRegion {
    let obbox = !attr_get(attrs, "filterunits")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("userSpaceOnUse"));
    if obbox {
        // Fractions of the bounding box: number as-is, percent / 100.
        let frac = |key: &str, default: f64| {
            attr_get(attrs, key)
                .and_then(svg_core::parse_length)
                .map(|l| match l.unit {
                    svg_core::SvgLengthUnit::Percent => l.value / 100.0,
                    _ => l.value,
                })
                .unwrap_or(default)
        };
        FilterRegion::ObjectBoundingBox {
            fx: frac("x", -0.1),
            fy: frac("y", -0.1),
            fw: frac("width", 1.2),
            fh: frac("height", 1.2),
        }
    } else {
        // userSpaceOnUse: explicit user lengths -> device rect via the CTM.
        let parse_len = |key: &str| attr_get(attrs, key).and_then(svg_core::parse_length);
        if ["x", "y", "width", "height"]
            .iter()
            .any(|k| matches!(parse_len(k), Some(l) if l.unit == svg_core::SvgLengthUnit::Percent))
        {
            diagnostics.push(PendingDiagnostic::Warning {
                code: "filter.region_percent_approximated",
                message:
                    "percentage filter region with userSpaceOnUse is approximated as a user-unit value",
            });
        }
        let num = |key: &str| parse_len(key).map(|l| l.value);
        let (x, y) = (num("x").unwrap_or(0.0), num("y").unwrap_or(0.0));
        let (wd, ht) = (num("width"), num("height"));
        match (wd, ht) {
            (Some(wd), Some(ht)) if wd > 0.0 && ht > 0.0 => {
                let (dx0, dy0) = ctm.apply(x, y);
                let (dx1, dy1) = ctm.apply(x + wd, y + ht);
                FilterRegion::UserSpace([dx0.min(dx1), dy0.min(dy1), dx0.max(dx1), dy0.max(dy1)])
            }
            // No usable explicit region — fall back to the bbox default.
            _ => FilterRegion::default(),
        }
    }
}

fn parse_filter_input(value: Option<&str>) -> FilterInput {
    match value.map(str::trim) {
        Some("SourceGraphic") => FilterInput::SourceGraphic,
        Some("SourceAlpha") => FilterInput::SourceAlpha,
        Some(name) if !name.is_empty() => FilterInput::Reference(name.to_owned()),
        _ => FilterInput::Previous,
    }
}

fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

fn parse_std_deviation(value: Option<&str>) -> (f64, f64) {
    let nums = svg_core::parse_numbers(value.unwrap_or(""));
    match nums.as_slice() {
        [x] => (*x, *x),
        [x, y, ..] => (*x, *y),
        _ => (0.0, 0.0),
    }
}

fn flood_color(attrs: &[(String, String)]) -> [u8; 4] {
    let color = final_style_property(attrs, "flood-color")
        .and_then(svg_core::parse_color)
        .unwrap_or(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        });
    let opacity = final_style_property(attrs, "flood-opacity")
        .and_then(parse_f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    [
        color.r,
        color.g,
        color.b,
        ((color.a as f64 * opacity).round().clamp(0.0, 255.0)) as u8,
    ]
}

fn parse_color_matrix(attrs: &[(String, String)]) -> [f32; 20] {
    let kind = attr_get(attrs, "type")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "matrix".to_owned());
    let values = svg_core::parse_numbers(attr_get(attrs, "values").unwrap_or(""));
    let identity = {
        let mut m = [0.0f32; 20];
        m[0] = 1.0;
        m[6] = 1.0;
        m[12] = 1.0;
        m[18] = 1.0;
        m
    };
    match kind.as_str() {
        "matrix" if values.len() == 20 => {
            let mut m = [0.0f32; 20];
            for (i, slot) in m.iter_mut().enumerate() {
                *slot = values[i] as f32;
            }
            m
        }
        "saturate" => {
            let s = values.first().copied().unwrap_or(1.0) as f32;
            [
                0.213 + 0.787 * s,
                0.715 - 0.715 * s,
                0.072 - 0.072 * s,
                0.0,
                0.0,
                0.213 - 0.213 * s,
                0.715 + 0.285 * s,
                0.072 - 0.072 * s,
                0.0,
                0.0,
                0.213 - 0.213 * s,
                0.715 - 0.715 * s,
                0.072 + 0.928 * s,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ]
        }
        "luminancetoalpha" => [
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.2125,
            0.7154, 0.0721, 0.0, 0.0,
        ],
        _ => identity,
    }
}

/// Parse a `feComposite` `operator` (+ arithmetic k1..k4) (R10).
fn parse_composite_op(attrs: &[(String, String)]) -> CompositeOp {
    let k = |name: &str| attr_get(attrs, name).and_then(parse_f64).unwrap_or(0.0) as f32;
    match attr_get(attrs, "operator")
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("in") => CompositeOp::In,
        Some("out") => CompositeOp::Out,
        Some("atop") => CompositeOp::Atop,
        Some("xor") => CompositeOp::Xor,
        Some("arithmetic") => CompositeOp::Arithmetic {
            k1: k("k1"),
            k2: k("k2"),
            k3: k("k3"),
            k4: k("k4"),
        },
        _ => CompositeOp::Over,
    }
}

/// Parse `feFuncR/G/B/A` children into per-channel transfer functions (R10).
fn parse_component_transfer(node: &SvgNode) -> [TransferFunc; 4] {
    let mut funcs = [
        TransferFunc::Identity,
        TransferFunc::Identity,
        TransferFunc::Identity,
        TransferFunc::Identity,
    ];
    for child in node.children().unwrap_or_default() {
        let SvgNode::Unsupported { tag, attrs, .. } = child else {
            continue;
        };
        let idx = match tag.as_str() {
            "fefuncr" => 0,
            "fefuncg" => 1,
            "fefuncb" => 2,
            "fefunca" => 3,
            _ => continue,
        };
        funcs[idx] = parse_transfer_func(attrs);
    }
    funcs
}

fn parse_transfer_func(attrs: &[(String, String)]) -> TransferFunc {
    let f = |name: &str, default: f32| {
        attr_get(attrs, name)
            .and_then(parse_f64)
            .map(|v| v as f32)
            .unwrap_or(default)
    };
    let table = || {
        svg_core::parse_numbers(attr_get(attrs, "tablevalues").unwrap_or(""))
            .into_iter()
            .map(|v| v as f32)
            .collect::<Vec<f32>>()
    };
    match attr_get(attrs, "type")
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("table") => TransferFunc::Table(table()),
        Some("discrete") => TransferFunc::Discrete(table()),
        Some("linear") => TransferFunc::Linear {
            slope: f("slope", 1.0),
            intercept: f("intercept", 0.0),
        },
        Some("gamma") => TransferFunc::Gamma {
            amplitude: f("amplitude", 1.0),
            exponent: f("exponent", 1.0),
            offset: f("offset", 0.0),
        },
        _ => TransferFunc::Identity,
    }
}

/// Fully-resolved layer payload carried by `DrawCommand::BeginLayer`.
struct ResolvedLayer {
    clip: Option<ClipDef>,
    mask: Option<MaskDef>,
    filter: Option<FilterGraph>,
    opacity: f32,
    blend: BlendMode,
    needs_offscreen: bool,
    diagnostics: Vec<PendingDiagnostic>,
    source: SvgRenderSource,
}

impl ResolvedLayer {
    fn resolve(raw: &LayerRaw, scene: &SvgScene, view: &Transform) -> Self {
        let mut diagnostics = Vec::new();
        let element_ctm = view.concat(raw.element_transform);
        let bbox = (!raw.is_group).then_some([0.0; 4]); // groups have no single bbox
        let clip_path = raw.clip_ref.as_ref().and_then(|id| {
            let mut visited = Vec::new();
            resolve_clip(
                scene,
                id,
                element_ctm,
                bbox,
                raw.length_bases,
                &mut visited,
                &mut diagnostics,
            )
        });
        let clip = match (&raw.overflow, clip_path) {
            (Some((rect, scene_transform)), clip_path) => {
                let shape = overflow_clip_shape(*rect, *scene_transform, view);
                Some(ClipDef {
                    shapes: vec![shape],
                    nested: clip_path.map(Box::new),
                })
            }
            (None, clip_path) => clip_path,
        };
        let mask = raw.mask_ref.as_ref().and_then(|id| {
            resolve_mask(scene, id, element_ctm, raw.length_bases, &mut diagnostics)
        });
        let filter = raw
            .filter_ref
            .as_ref()
            .and_then(|id| parse_filter(scene, id, element_ctm, &mut diagnostics));
        let needs_offscreen = raw.opacity < 1.0
            || raw.isolate
            || mask.is_some()
            || filter.is_some()
            || raw.blend != BlendMode::Normal;
        ResolvedLayer {
            clip,
            mask,
            filter,
            opacity: raw.opacity.clamp(0.0, 1.0),
            blend: raw.blend,
            needs_offscreen,
            diagnostics,
            source: raw.source,
        }
    }
}

/// One render-ready command in the display list.
enum DrawCommand {
    /// Open a compositing/clip layer enclosing following commands until `EndLayer`.
    BeginLayer(Box<ResolvedLayer>),
    /// Close the most recently opened layer.
    EndLayer,
    /// A renderable shape with its final (view ∘ item) transform and resolved style.
    Shape {
        geometry: Option<ShapeGeometry>,
        transform: Transform,
        style: Box<Style>,
        length_bases: SvgLengthBases,
        path_length: Option<f64>,
        clip: Option<ClipDef>,
        /// R9 start/mid/end markers resolved + placed on this shape's vertices.
        markers: Option<Box<MarkerSet>>,
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
    /// A decoded embedded raster image (R5), placed by `img_to_device` and
    /// clipped to its destination rect (so preserveAspectRatio `slice` overflow
    /// is trimmed) plus any `clip-path`.
    Image {
        image: Box<DecodedImage>,
        img_to_device: Transform,
        dest_device_rect: Vec<(f32, f32)>,
        opacity: f32,
        clip: Option<ClipDef>,
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
    /// An `<image>` that could not be decoded — diagnosed and skipped, leaving
    /// the source preserved (the import-side placeholder remains the fallback).
    ImageSkipped {
        code: &'static str,
        message: &'static str,
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
    /// A shape under an unsupported ancestor — counted as skipped, not drawn.
    SkippedShape {
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
    /// A group container — carried only so attribute diagnostics still fire.
    GroupDiagnostics {
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
    /// Unsupported element — diagnostics + skip.
    UnsupportedNode {
        tag: String,
        message: String,
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
}

struct DisplayList {
    commands: Vec<DrawCommand>,
    paint_servers: PaintServerTable,
}

impl DisplayList {
    /// Lowering pass: scene items → flat draw-command stream.
    fn build(scene: &SvgScene, view_xform: &Transform) -> Self {
        let mut commands = Vec::with_capacity(scene.items.len());
        for item in &scene.items {
            if item.is_layer_end {
                commands.push(DrawCommand::EndLayer);
                continue;
            }
            if let Some(raw) = &item.layer {
                commands.push(DrawCommand::BeginLayer(Box::new(ResolvedLayer::resolve(
                    raw, scene, view_xform,
                ))));
            }
            let node_xform = view_xform.concat(item.transform);
            let source = item.node.source();
            let mut diagnostics = unsupported_attr_diagnostics(item.node.attrs());
            let cmd = match &item.node {
                SvgNode::Group { .. } => DrawCommand::GroupDiagnostics {
                    diagnostics,
                    source,
                },
                SvgNode::Text { .. } => {
                    if item.skipped_by_unsupported_ancestor {
                        DrawCommand::SkippedShape {
                            diagnostics,
                            source,
                        }
                    } else {
                        // R11: raster text snapshot via the bundled vector font.
                        lower_text_command(scene, item, node_xform, diagnostics, source)
                    }
                }
                SvgNode::Unsupported { tag, attrs, .. } if tag == "image" => {
                    let lb = item.length_bases;
                    let x = attr_f32(attrs, "x", lb.horizontal, 0.0) as f64;
                    let y = attr_f32(attrs, "y", lb.vertical, 0.0) as f64;
                    let width = (attr_f32(attrs, "width", lb.horizontal, 0.0) as f64).max(0.0);
                    let height = (attr_f32(attrs, "height", lb.vertical, 0.0) as f64).max(0.0);
                    let href = attr_get(attrs, "href")
                        .or_else(|| attr_get(attrs, "xlink:href"))
                        .unwrap_or("");
                    match decode_image_href(href) {
                        Ok(image) if width > 0.0 && height > 0.0 => {
                            let aspect = svg_core::parse_preserve_aspect_ratio(
                                attr_get(attrs, "preserveaspectratio").unwrap_or(""),
                            );
                            let place = svg_core::viewbox_transform(
                                [0.0, 0.0, image.width as f64, image.height as f64],
                                [x, y, width, height],
                                aspect,
                            )
                            .unwrap_or_else(Transform::identity);
                            let dest = [
                                node_xform.apply_f32(x as f32, y as f32),
                                node_xform.apply_f32((x + width) as f32, y as f32),
                                node_xform.apply_f32((x + width) as f32, (y + height) as f32),
                                node_xform.apply_f32(x as f32, (y + height) as f32),
                            ];
                            let clip = clip_path_ref(attrs).and_then(|id| {
                                let mut visited = Vec::new();
                                resolve_clip(
                                    scene,
                                    &id,
                                    node_xform,
                                    Some([x, y, x + width, y + height]),
                                    item.length_bases,
                                    &mut visited,
                                    &mut diagnostics,
                                )
                            });
                            DrawCommand::Image {
                                image: Box::new(image),
                                img_to_device: node_xform.concat(place),
                                dest_device_rect: dest.to_vec(),
                                opacity: item.style.opacity.clamp(0.0, 1.0),
                                clip,
                                diagnostics,
                                source,
                            }
                        }
                        Ok(_) => DrawCommand::ImageSkipped {
                            code: "image.zero_size",
                            message: "image has zero width or height; not rendered",
                            diagnostics,
                            source,
                        },
                        Err(err) => DrawCommand::ImageSkipped {
                            code: err.code(),
                            message: err.message(),
                            diagnostics,
                            source,
                        },
                    }
                }
                SvgNode::Unsupported { tag, .. } => DrawCommand::UnsupportedNode {
                    tag: tag.clone(),
                    message: unsupported_node_message(tag, item.node.attrs()),
                    diagnostics,
                    source,
                },
                shape_node => {
                    if item.skipped_by_unsupported_ancestor {
                        DrawCommand::SkippedShape {
                            diagnostics,
                            source,
                        }
                    } else {
                        let geometry = lower_shape_geometry(shape_node, item.length_bases);
                        if let Some(value) = attr_get(shape_node.attrs(), "vector-effect") {
                            if parse_vector_effect(value).is_none() {
                                diagnostics.push(PendingDiagnostic::Warning {
                                    code: "vector_effect.unsupported",
                                    message: "unsupported vector-effect value; treated as none",
                                });
                            }
                        }
                        let clip = clip_path_ref(shape_node.attrs()).and_then(|id| {
                            let bbox = geometry.as_ref().map(geometry_local_bounds);
                            let mut visited = Vec::new();
                            resolve_clip(
                                scene,
                                &id,
                                node_xform,
                                bbox,
                                item.length_bases,
                                &mut visited,
                                &mut diagnostics,
                            )
                        });
                        let markers = geometry.as_ref().and_then(|geometry| {
                            build_markers(
                                scene,
                                shape_node.attrs(),
                                &item.style,
                                item.length_bases,
                                geometry,
                                &node_xform,
                                &mut diagnostics,
                            )
                        });
                        DrawCommand::Shape {
                            geometry,
                            transform: node_xform,
                            style: Box::new(item.style.clone()),
                            length_bases: item.length_bases,
                            path_length: parsed_path_length(shape_node.attrs()),
                            clip,
                            markers,
                            diagnostics,
                            source,
                        }
                    }
                }
            };
            commands.push(cmd);
        }
        Self {
            commands,
            paint_servers: scene.paint_servers.clone(),
        }
    }

    /// Raster pass: execute each command, writing pixels and updating the report.
    ///
    /// Maintains a layer stack so clip masks and isolated-group offscreens scope
    /// to their subtree.  The base `buf` is straight RGBA; isolated offscreens
    /// are premultiplied and composited back once per group at group opacity.
    fn execute(&self, buf: &mut [u8], w: usize, h: usize, report: &mut SvgRenderReport) {
        let mut offscreens: Vec<Offscreen> = Vec::new();
        let mut offscreen_bytes: usize = 0;
        let mut frames: Vec<LayerFrame> = Vec::new();
        let mut effective_clip: Option<ClipMask> = None;

        for cmd in &self.commands {
            match cmd {
                DrawCommand::BeginLayer(layer) => {
                    emit_diagnostics(&layer.diagnostics, layer.source, report);
                    let prev_effective = effective_clip.clone();
                    if let Some(clip_def) = &layer.clip {
                        let mask = clip_def.build_mask(w, h);
                        effective_clip = combine_clips(effective_clip.as_ref(), Some(&mask));
                    }
                    let mut pushed_offscreen = false;
                    if layer.needs_offscreen {
                        let bytes = w.saturating_mul(h).saturating_mul(4);
                        if offscreens.len() < MAX_OFFSCREEN_DEPTH
                            && offscreen_bytes.saturating_add(bytes) <= MAX_OFFSCREEN_BYTES
                        {
                            offscreens.push(Offscreen {
                                buf: vec![0u8; bytes],
                                opacity: layer.opacity,
                                blend: layer.blend,
                            });
                            offscreen_bytes += bytes;
                            pushed_offscreen = true;
                        } else {
                            report.warning_at(
                                "limit.offscreen_buffer",
                                "isolated group offscreen exceeded the renderer memory or depth cap; group composited without isolation",
                                Some(layer.source),
                            );
                        }
                    }
                    frames.push(LayerFrame {
                        prev_effective,
                        pushed_offscreen,
                        layer,
                    });
                }
                DrawCommand::EndLayer => {
                    if let Some(frame) = frames.pop() {
                        if frame.pushed_offscreen {
                            if let Some(mut off) = offscreens.pop() {
                                offscreen_bytes = offscreen_bytes.saturating_sub(off.buf.len());
                                // R7: filter then mask post-process the isolated offscreen
                                // before it composites back into its parent.
                                if let Some(filter) = &frame.layer.filter {
                                    off.buf = filter.apply(&off.buf, w, h);
                                }
                                if let Some(mask) = &frame.layer.mask {
                                    let alpha = mask.build_alpha(w, h, &self.paint_servers);
                                    apply_mask_to_offscreen(&mut off.buf, &alpha, w, h);
                                }
                                match offscreens.last_mut() {
                                    Some(parent) => {
                                        composite_offscreen(&mut parent.buf, true, &off)
                                    }
                                    None => composite_offscreen(buf, false, &off),
                                }
                            }
                        }
                        effective_clip = frame.prev_effective;
                    }
                }
                DrawCommand::GroupDiagnostics {
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                }
                DrawCommand::Shape {
                    geometry,
                    transform,
                    style,
                    length_bases,
                    path_length,
                    clip,
                    markers,
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    for (property, id) in style.paint_server_references() {
                        if !self.paint_servers.servers.contains_key(id)
                            && !self.paint_servers.patterns.contains_key(id)
                        {
                            report.warning_at(
                                "paint.unresolved_server",
                                format!(
                                    "{property} references unavailable local paint server #{id}; the affected paint was left transparent"
                                ),
                                Some(*source),
                            );
                        }
                    }
                    // Combine the active layer clip with this shape's own clip-path.
                    let shape_mask = clip.as_ref().map(|c| c.build_mask(w, h));
                    let combined_clip;
                    let draw_clip: Option<&ClipMask> = if let Some(mask) = &shape_mask {
                        combined_clip = combine_clips(effective_clip.as_ref(), Some(mask));
                        combined_clip.as_ref()
                    } else {
                        effective_clip.as_ref()
                    };
                    let outcome = match (geometry.as_ref(), offscreens.last_mut()) {
                        (None, _) => RenderOutcome::default(),
                        (Some(geometry), Some(off)) => {
                            let mut target = RasterTarget {
                                buf: &mut off.buf,
                                width: w,
                                height: h,
                                premultiplied: true,
                                clip: draw_clip,
                            };
                            render_shape(
                                geometry,
                                transform,
                                style,
                                *length_bases,
                                *path_length,
                                &self.paint_servers,
                                &mut target,
                            )
                        }
                        (Some(geometry), None) => {
                            let mut target = RasterTarget {
                                buf,
                                width: w,
                                height: h,
                                premultiplied: false,
                                clip: draw_clip,
                            };
                            render_shape(
                                geometry,
                                transform,
                                style,
                                *length_bases,
                                *path_length,
                                &self.paint_servers,
                                &mut target,
                            )
                        }
                    };
                    if outcome.stroke_limit_hit {
                        report.warning_at(
                            "limit.stroke_complexity",
                            "stroke geometry exceeded a renderer safety limit and was truncated",
                            Some(*source),
                        );
                    }
                    if outcome.drawn {
                        report.rendered();
                    } else {
                        report.skipped();
                    }
                    // R9: draw resolved markers on this shape's vertices, each
                    // clipped to its viewport rect (overflow:hidden) intersected
                    // with the ancestor clip.
                    if let Some(set) = markers {
                        for placement in &set.placements {
                            let def = &set.defs[placement.def_index];
                            let marker_clip = placement.overflow_hidden.then(|| {
                                ClipDef {
                                    shapes: vec![ClipShape {
                                        device_subpaths: vec![placement.viewport_corners.clone()],
                                        fill_rule: FillRule::Nonzero,
                                    }],
                                    nested: None,
                                }
                                .build_mask(w, h)
                            });
                            let combined =
                                combine_clips(effective_clip.as_ref(), marker_clip.as_ref());
                            match offscreens.last_mut() {
                                Some(off) => {
                                    let mut target = RasterTarget {
                                        buf: &mut off.buf,
                                        width: w,
                                        height: h,
                                        premultiplied: true,
                                        clip: combined.as_ref(),
                                    };
                                    render_content_items(
                                        &def.items,
                                        placement.content_to_device,
                                        &self.paint_servers,
                                        &mut target,
                                    );
                                }
                                None => {
                                    let mut target = RasterTarget {
                                        buf,
                                        width: w,
                                        height: h,
                                        premultiplied: false,
                                        clip: combined.as_ref(),
                                    };
                                    render_content_items(
                                        &def.items,
                                        placement.content_to_device,
                                        &self.paint_servers,
                                        &mut target,
                                    );
                                }
                            }
                        }
                    }
                }
                DrawCommand::Image {
                    image,
                    img_to_device,
                    dest_device_rect,
                    opacity,
                    clip,
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    let dest_clip = ClipDef {
                        shapes: vec![ClipShape {
                            device_subpaths: vec![dest_device_rect.clone()],
                            fill_rule: FillRule::Nonzero,
                        }],
                        nested: clip.clone().map(Box::new),
                    };
                    let mask = dest_clip.build_mask(w, h);
                    let combined = combine_clips(effective_clip.as_ref(), Some(&mask));
                    let drawn = if let Some(inverse) = img_to_device.inverse() {
                        let bounds = device_rect_bounds(dest_device_rect, w, h);
                        match offscreens.last_mut() {
                            Some(off) => {
                                let mut target = RasterTarget {
                                    buf: &mut off.buf,
                                    width: w,
                                    height: h,
                                    premultiplied: true,
                                    clip: combined.as_ref(),
                                };
                                draw_image_samples(&mut target, image, inverse, *opacity, bounds);
                            }
                            None => {
                                let mut target = RasterTarget {
                                    buf,
                                    width: w,
                                    height: h,
                                    premultiplied: false,
                                    clip: combined.as_ref(),
                                };
                                draw_image_samples(&mut target, image, inverse, *opacity, bounds);
                            }
                        }
                        true
                    } else {
                        false
                    };
                    if drawn {
                        report.rendered();
                    } else {
                        report.skipped();
                    }
                }
                DrawCommand::ImageSkipped {
                    code,
                    message,
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    report.unsupported_at(
                        "image",
                        "embedded raster image was not rendered",
                        Some(*source),
                    );
                    report.warning_at(*code, *message, Some(*source));
                    report.skipped();
                }
                DrawCommand::SkippedShape {
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    report.skipped();
                }
                DrawCommand::UnsupportedNode {
                    tag,
                    message,
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    if let Some((feature, _)) = unsupported_tag_feature(tag) {
                        report.unsupported_at(feature, message.clone(), Some(*source));
                    }
                    report.skipped();
                }
            }
        }
    }
}

fn unsupported_node_message(tag: &str, attrs: &[(String, String)]) -> String {
    if tag == "pattern" {
        let present: Vec<&str> = [
            "patternunits",
            "patterncontentunits",
            "patterntransform",
            "x",
            "y",
            "width",
            "height",
            "viewbox",
            "preserveaspectratio",
            "href",
        ]
        .into_iter()
        .filter(|name| attr_get(attrs, name).is_some())
        .collect();
        if present.is_empty() {
            return "pattern paint servers are diagnosed but not rasterized yet".to_owned();
        }
        return format!(
            "pattern paint server is not rasterized; preserved attributes: {}",
            present.join(", ")
        );
    }
    unsupported_tag_feature(tag)
        .map(|(_, message)| message.to_owned())
        .unwrap_or_else(|| "unsupported SVG element was skipped".to_owned())
}

// ---------------------------------------------------------------------------
// Shape renderers
// ---------------------------------------------------------------------------

fn parsed_path_length(attrs: &[(String, String)]) -> Option<f64> {
    attr_get(attrs, "pathlength")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn lower_shape_geometry(node: &SvgNode, length_bases: SvgLengthBases) -> Option<ShapeGeometry> {
    let attrs = node.attrs();
    match node {
        SvgNode::Rect { .. } => {
            let x = attr_f32(attrs, "x", length_bases.horizontal, 0.0);
            let y = attr_f32(attrs, "y", length_bases.vertical, 0.0);
            let width = attr_f32(attrs, "width", length_bases.horizontal, 0.0);
            let height = attr_f32(attrs, "height", length_bases.vertical, 0.0);
            if width <= 0.0 || height <= 0.0 {
                return None;
            }
            let mut rx = attr_f32(attrs, "rx", length_bases.horizontal, 0.0);
            let mut ry = attr_f32(attrs, "ry", length_bases.vertical, 0.0);
            if rx > 0.0 && ry == 0.0 {
                ry = rx;
            }
            if ry > 0.0 && rx == 0.0 {
                rx = ry;
            }
            Some(ShapeGeometry::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
            })
        }
        SvgNode::Circle { .. } => {
            let cx = attr_f32(attrs, "cx", length_bases.horizontal, 0.0);
            let cy = attr_f32(attrs, "cy", length_bases.vertical, 0.0);
            let radius = attr_f32(attrs, "r", length_bases.other, 0.0);
            (radius > 0.0).then_some(ShapeGeometry::Ellipse {
                cx,
                cy,
                rx: radius,
                ry: radius,
            })
        }
        SvgNode::Ellipse { .. } => {
            let cx = attr_f32(attrs, "cx", length_bases.horizontal, 0.0);
            let cy = attr_f32(attrs, "cy", length_bases.vertical, 0.0);
            let rx = attr_f32(attrs, "rx", length_bases.horizontal, 0.0);
            let ry = attr_f32(attrs, "ry", length_bases.vertical, 0.0);
            (rx > 0.0 && ry > 0.0).then_some(ShapeGeometry::Ellipse { cx, cy, rx, ry })
        }
        SvgNode::Line { .. } => Some(ShapeGeometry::Line {
            from: (
                attr_f32(attrs, "x1", length_bases.horizontal, 0.0),
                attr_f32(attrs, "y1", length_bases.vertical, 0.0),
            ),
            to: (
                attr_f32(attrs, "x2", length_bases.horizontal, 0.0),
                attr_f32(attrs, "y2", length_bases.vertical, 0.0),
            ),
        }),
        SvgNode::Polyline { .. } | SvgNode::Polygon { .. } => {
            let points = parse_point_list(attr_get(attrs, "points").unwrap_or(""));
            (points.len() >= 2).then_some(ShapeGeometry::Poly {
                points,
                closed: matches!(node, SvgNode::Polygon { .. }),
            })
        }
        SvgNode::Path { .. } => {
            let data = parse_path_d(attr_get(attrs, "d").unwrap_or(""));
            (!data.subpaths.is_empty()).then_some(ShapeGeometry::Path { data })
        }
        _ => None,
    }
}

/// A pixel-aligned coverage/alpha mask in device space (one byte per pixel,
/// 0 = fully clipped out, 255 = fully visible).  Used for `clipPath` clipping
/// and nested-`<svg>` overflow clipping.  Independent of color, so the same
/// mask machinery serves both clip kinds.
#[derive(Clone)]
struct ClipMask {
    width: usize,
    height: usize,
    alpha: Vec<u8>,
}

impl ClipMask {
    fn transparent(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            alpha: vec![0u8; width * height],
        }
    }

    #[inline]
    fn at(&self, x: usize, y: usize) -> u8 {
        self.alpha[y * self.width + x]
    }

    /// Union (max) one shape's coverage into the mask, mirroring the fill
    /// coverage scan so clip edges anti-alias identically to painted edges.
    fn add_shape(&mut self, sub_paths: &[&[(f32, f32)]], fill_rule: FillRule) {
        let (w, h) = (self.width, self.height);
        coverage_scan(w, h, sub_paths, fill_rule, |x, y, samples| {
            let alpha = ((samples * 255 + COVERAGE_SAMPLES / 2) / COVERAGE_SAMPLES) as u8;
            let slot = &mut self.alpha[y * w + x];
            *slot = (*slot).max(alpha);
        });
    }

    /// Intersect (multiply) another mask into this one.  Out-of-range masks are
    /// treated as fully transparent so intersection stays conservative.
    fn intersect(&mut self, other: &ClipMask) {
        if other.width != self.width || other.height != self.height {
            self.alpha.iter_mut().for_each(|a| *a = 0);
            return;
        }
        for (slot, &m) in self.alpha.iter_mut().zip(other.alpha.iter()) {
            *slot = ((*slot as u16 * m as u16 + 127) / 255) as u8;
        }
    }
}

/// Combine an optional ancestor clip with an optional local clip into one mask.
fn combine_clips(ancestor: Option<&ClipMask>, local: Option<&ClipMask>) -> Option<ClipMask> {
    match (ancestor, local) {
        (None, None) => None,
        (Some(a), None) => Some(a.clone()),
        (None, Some(l)) => Some(l.clone()),
        (Some(a), Some(l)) => {
            let mut combined = a.clone();
            combined.intersect(l);
            Some(combined)
        }
    }
}

/// An isolated-group offscreen accumulation buffer in *premultiplied* RGBA.
struct Offscreen {
    buf: Vec<u8>,
    opacity: f32,
    /// `mix-blend-mode` used when compositing this layer back into its parent.
    blend: BlendMode,
}

/// One open layer scope in `DisplayList::execute`'s layer stack.
struct LayerFrame<'a> {
    /// Effective clip to restore when this layer closes.
    prev_effective: Option<ClipMask>,
    /// Whether this layer allocated an isolated offscreen (vs. clip-only).
    pushed_offscreen: bool,
    /// The resolved layer, so `EndLayer` can apply R7 filter/mask post-processing.
    layer: &'a ResolvedLayer,
}

struct RasterTarget<'a> {
    buf: &'a mut [u8],
    width: usize,
    height: usize,
    /// `true` when `buf` stores premultiplied RGBA (isolated offscreen layers);
    /// `false` for the straight-RGBA output buffer.
    premultiplied: bool,
    /// Active clip mask (already combined across ancestors); `None` = no clip.
    clip: Option<&'a ClipMask>,
}

impl RasterTarget<'_> {
    /// Composite one straight-RGBA source sample at (x, y), honoring the active
    /// clip mask and the target's premultiplied/straight pixel format.
    #[inline]
    fn composite(&mut self, x: usize, y: usize, mut src: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        if let Some(clip) = self.clip {
            let m = clip.at(x, y) as u16;
            if m == 0 {
                return;
            }
            src[3] = ((src[3] as u16 * m + 127) / 255) as u8;
        }
        if self.premultiplied {
            blend_pixel_premultiplied(self.buf, self.width, x, y, src);
        } else {
            blend_pixel(self.buf, self.width, x, y, src);
        }
    }
}

#[derive(Default)]
struct RenderOutcome {
    drawn: bool,
    stroke_limit_hit: bool,
}

/// Resolve the stroke for a shape, applying `vector-effect: non-scaling-stroke`.
///
/// The stroke mesh is built in local space and then scaled by the CTM, so a
/// constant device-space width is achieved by dividing the user-space width
/// (and dash metrics) by the CTM scale before meshing — the later device
/// transform restores exactly the requested pixel width regardless of zoom.
/// Non-uniform scales use the bounded `affine_max_scale` approximation.
fn effective_device_stroke(
    style: &Style,
    xform: &Transform,
    length_bases: SvgLengthBases,
) -> Option<ResolvedStroke> {
    let mut stroke = style.effective_stroke(length_bases)?;
    if style.vector_effect == VectorEffect::NonScalingStroke {
        let inv = 1.0 / affine_max_scale(*xform).max(1.0e-6);
        stroke.width *= inv;
        if let Some(dashes) = stroke.dash_array.as_mut() {
            for d in dashes.iter_mut() {
                *d *= inv;
            }
        }
        stroke.dash_offset *= inv;
    }
    Some(stroke)
}

fn render_shape(
    geometry: &ShapeGeometry,
    xform: &Transform,
    style: &Style,
    length_bases: SvgLengthBases,
    path_length: Option<f64>,
    paint_servers: &PaintServerTable,
    target: &mut RasterTarget<'_>,
) -> RenderOutcome {
    match geometry {
        ShapeGeometry::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => render_closed_points(
            &rounded_rect_pts(*x, *y, *width, *height, *rx, *ry),
            xform,
            style,
            length_bases,
            path_length,
            paint_servers,
            target,
        ),
        ShapeGeometry::Ellipse { cx, cy, rx, ry } => render_closed_points(
            &ellipse_pts(*cx, *cy, *rx, *ry),
            xform,
            style,
            length_bases,
            path_length,
            paint_servers,
            target,
        ),
        ShapeGeometry::Line { from, to } => {
            if let Some(stroke) = effective_device_stroke(style, xform, length_bases) {
                let bounds = local_bounds(&[*from, *to]);
                let sampler = PaintSampler::from_resolved(
                    &stroke.paint,
                    paint_servers,
                    bounds,
                    *xform,
                    length_bases,
                );
                let stroke = stroke_polyline(
                    target,
                    &[*from, *to],
                    xform,
                    &stroke,
                    &sampler,
                    false,
                    path_length,
                );
                RenderOutcome {
                    drawn: stroke.drawn,
                    stroke_limit_hit: stroke.limit_hit,
                }
            } else {
                RenderOutcome::default()
            }
        }
        ShapeGeometry::Poly { points, closed } => {
            let transformed: Vec<_> = points.iter().map(|&(x, y)| xform.apply_f32(x, y)).collect();
            let bounds = local_bounds(points);
            let mut outcome = RenderOutcome::default();
            if *closed {
                if let Some(fill) = style.effective_fill() {
                    let sampler = PaintSampler::from_resolved(
                        &fill,
                        paint_servers,
                        bounds,
                        *xform,
                        length_bases,
                    );
                    fill_polygon(target, &transformed, style.fill_rule, &sampler);
                    outcome.drawn |= !sampler.is_transparent();
                }
            }
            if let Some(stroke) = effective_device_stroke(style, xform, length_bases) {
                let sampler = PaintSampler::from_resolved(
                    &stroke.paint,
                    paint_servers,
                    bounds,
                    *xform,
                    length_bases,
                );
                let stroke = stroke_polyline(
                    target,
                    points,
                    xform,
                    &stroke,
                    &sampler,
                    *closed,
                    path_length,
                );
                outcome.drawn |= stroke.drawn;
                outcome.stroke_limit_hit |= stroke.limit_hit;
            }
            outcome
        }
        ShapeGeometry::Path { data } => {
            let subpaths = flatten_path_data(data, xform, 0.25);
            let local_points: Vec<(f32, f32)> = subpaths
                .iter()
                .flat_map(|subpath| subpath.points.iter())
                .copied()
                .collect();
            let bounds = local_bounds(&local_points);
            let mut outcome = RenderOutcome::default();
            if let Some(fill) = style.effective_fill() {
                let sampler =
                    PaintSampler::from_resolved(&fill, paint_servers, bounds, *xform, length_bases);
                let device_paths: Vec<Vec<(f32, f32)>> = subpaths
                    .iter()
                    .map(|subpath| {
                        subpath
                            .points
                            .iter()
                            .map(|&(x, y)| xform.apply_f32(x, y))
                            .collect()
                    })
                    .collect();
                let refs: Vec<&[(f32, f32)]> = device_paths.iter().map(Vec::as_slice).collect();
                rasterize_fill_coverage(target, &refs, style.fill_rule, &sampler);
                outcome.drawn |= !sampler.is_transparent();
            }
            if let Some(stroke) = effective_device_stroke(style, xform, length_bases) {
                let sampler = PaintSampler::from_resolved(
                    &stroke.paint,
                    paint_servers,
                    bounds,
                    *xform,
                    length_bases,
                );
                for subpath in &subpaths {
                    if subpath.points.len() < 2 {
                        continue;
                    }
                    let stroke = stroke_polyline(
                        target,
                        &subpath.points,
                        xform,
                        &stroke,
                        &sampler,
                        subpath.closed,
                        path_length,
                    );
                    outcome.drawn |= stroke.drawn;
                    outcome.stroke_limit_hit |= stroke.limit_hit;
                }
            }
            outcome
        }
    }
}

fn render_closed_points(
    points: &[(f32, f32)],
    xform: &Transform,
    style: &Style,
    length_bases: SvgLengthBases,
    path_length: Option<f64>,
    paint_servers: &PaintServerTable,
    target: &mut RasterTarget<'_>,
) -> RenderOutcome {
    let transformed: Vec<_> = points.iter().map(|&(x, y)| xform.apply_f32(x, y)).collect();
    let bounds = local_bounds(points);
    let mut outcome = RenderOutcome::default();
    if let Some(fill) = style.effective_fill() {
        let sampler =
            PaintSampler::from_resolved(&fill, paint_servers, bounds, *xform, length_bases);
        fill_polygon(target, &transformed, style.fill_rule, &sampler);
        outcome.drawn |= !sampler.is_transparent();
    }
    if let Some(stroke) = style.effective_stroke(length_bases) {
        let sampler =
            PaintSampler::from_resolved(&stroke.paint, paint_servers, bounds, *xform, length_bases);
        let stroke = stroke_polyline(target, points, xform, &stroke, &sampler, true, path_length);
        outcome.drawn |= stroke.drawn;
        outcome.stroke_limit_hit |= stroke.limit_hit;
    }
    outcome
}

// ---------------------------------------------------------------------------
// Path data parser
// ---------------------------------------------------------------------------

fn parse_path_d(d: &str) -> PathData {
    let tokens = svg_core::tokenize_path_data(d);
    if tokens.len() > MAX_PATH_TOKENS {
        return PathData::default();
    }
    let mut data = PathData::default();
    let mut active: Option<PathSubpath> = None;
    let mut current = (0.0, 0.0);
    let mut repeat_cmd = 'M';
    let mut last_cubic_ctrl: Option<PathPoint> = None;
    let mut last_quadratic_ctrl: Option<PathPoint> = None;

    let mut i = 0;
    while i < tokens.len() {
        let cmd = match tokens[i] {
            svg_core::SvgPathToken::Command(c) => {
                i += 1;
                c
            }
            svg_core::SvgPathToken::Number(_) => match repeat_cmd {
                'M' => 'L',
                'm' => 'l',
                c => c,
            },
        };
        repeat_cmd = cmd;

        match cmd {
            'M' | 'm' => {
                let mut first = true;
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([x, y]) = read_path_values(&tokens, &mut i) else {
                        break;
                    };
                    let to = if cmd == 'm' {
                        (current.0 + x, current.1 + y)
                    } else {
                        (x, y)
                    };
                    if first {
                        finish_subpath(&mut active, &mut data);
                        active = Some(PathSubpath {
                            start: to,
                            segments: Vec::new(),
                            closed: false,
                        });
                        first = false;
                    } else if let Some(subpath) = active.as_mut() {
                        subpath.segments.push(PathSegment::Line { to });
                    }
                    current = to;
                }
                repeat_cmd = if cmd == 'm' { 'l' } else { 'L' };
                last_cubic_ctrl = None;
                last_quadratic_ctrl = None;
            }
            'L' | 'l' => {
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([x, y]) = read_path_values(&tokens, &mut i) else {
                        break;
                    };
                    let to = if cmd == 'l' {
                        (current.0 + x, current.1 + y)
                    } else {
                        (x, y)
                    };
                    push_path_segment(&mut active, PathSegment::Line { to });
                    current = to;
                }
                last_cubic_ctrl = None;
                last_quadratic_ctrl = None;
            }
            'H' | 'h' => {
                while let Some(x) = read_path_number(&tokens, &mut i) {
                    let to = (if cmd == 'h' { current.0 + x } else { x }, current.1);
                    push_path_segment(&mut active, PathSegment::Line { to });
                    current = to;
                }
                last_cubic_ctrl = None;
                last_quadratic_ctrl = None;
            }
            'V' | 'v' => {
                while let Some(y) = read_path_number(&tokens, &mut i) {
                    let to = (current.0, if cmd == 'v' { current.1 + y } else { y });
                    push_path_segment(&mut active, PathSegment::Line { to });
                    current = to;
                }
                last_cubic_ctrl = None;
                last_quadratic_ctrl = None;
            }
            'C' | 'c' => {
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([x1, y1, x2, y2, x, y]) = read_path_values(&tokens, &mut i) else {
                        break;
                    };
                    let (ctrl1, ctrl2, to) = if cmd == 'c' {
                        (
                            (current.0 + x1, current.1 + y1),
                            (current.0 + x2, current.1 + y2),
                            (current.0 + x, current.1 + y),
                        )
                    } else {
                        ((x1, y1), (x2, y2), (x, y))
                    };
                    push_path_segment(&mut active, PathSegment::Cubic { ctrl1, ctrl2, to });
                    current = to;
                    last_cubic_ctrl = Some(ctrl2);
                    last_quadratic_ctrl = None;
                }
            }
            'S' | 's' => {
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([x2, y2, x, y]) = read_path_values(&tokens, &mut i) else {
                        break;
                    };
                    let ctrl1 = last_cubic_ctrl
                        .map(|ctrl| (2.0 * current.0 - ctrl.0, 2.0 * current.1 - ctrl.1))
                        .unwrap_or(current);
                    let (ctrl2, to) = if cmd == 's' {
                        (
                            (current.0 + x2, current.1 + y2),
                            (current.0 + x, current.1 + y),
                        )
                    } else {
                        ((x2, y2), (x, y))
                    };
                    push_path_segment(&mut active, PathSegment::Cubic { ctrl1, ctrl2, to });
                    current = to;
                    last_cubic_ctrl = Some(ctrl2);
                    last_quadratic_ctrl = None;
                }
            }
            'Q' | 'q' => {
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([x1, y1, x, y]) = read_path_values(&tokens, &mut i) else {
                        break;
                    };
                    let (ctrl, to) = if cmd == 'q' {
                        (
                            (current.0 + x1, current.1 + y1),
                            (current.0 + x, current.1 + y),
                        )
                    } else {
                        ((x1, y1), (x, y))
                    };
                    push_path_segment(&mut active, PathSegment::Quadratic { ctrl, to });
                    current = to;
                    last_quadratic_ctrl = Some(ctrl);
                    last_cubic_ctrl = None;
                }
            }
            'T' | 't' => {
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([x, y]) = read_path_values(&tokens, &mut i) else {
                        break;
                    };
                    let ctrl = last_quadratic_ctrl
                        .map(|point| (2.0 * current.0 - point.0, 2.0 * current.1 - point.1))
                        .unwrap_or(current);
                    let to = if cmd == 't' {
                        (current.0 + x, current.1 + y)
                    } else {
                        (x, y)
                    };
                    push_path_segment(&mut active, PathSegment::Quadratic { ctrl, to });
                    current = to;
                    last_quadratic_ctrl = Some(ctrl);
                    last_cubic_ctrl = None;
                }
            }
            'A' | 'a' => {
                while matches!(tokens.get(i), Some(svg_core::SvgPathToken::Number(_))) {
                    let Some([rx, ry, rotation, large, sweep, x, y]) =
                        read_path_values(&tokens, &mut i)
                    else {
                        break;
                    };
                    let to = if cmd == 'a' {
                        (current.0 + x, current.1 + y)
                    } else {
                        (x, y)
                    };
                    push_path_segment(
                        &mut active,
                        PathSegment::Arc {
                            rx: rx.abs(),
                            ry: ry.abs(),
                            x_axis_rotation: rotation,
                            large_arc: large != 0.0,
                            sweep: sweep != 0.0,
                            to,
                        },
                    );
                    current = to;
                    last_cubic_ctrl = None;
                    last_quadratic_ctrl = None;
                }
            }
            'Z' | 'z' => {
                if let Some(subpath) = active.as_mut() {
                    subpath.closed = true;
                    current = subpath.start;
                }
                finish_subpath(&mut active, &mut data);
                last_cubic_ctrl = None;
                last_quadratic_ctrl = None;
            }
            _ => {
                skip_path_numbers(&tokens, &mut i);
                last_cubic_ctrl = None;
                last_quadratic_ctrl = None;
            }
        }
    }

    finish_subpath(&mut active, &mut data);
    data
}

fn read_path_number(tokens: &[svg_core::SvgPathToken], index: &mut usize) -> Option<f64> {
    match tokens.get(*index) {
        Some(svg_core::SvgPathToken::Number(value)) => {
            *index += 1;
            value.is_finite().then_some(*value)
        }
        _ => None,
    }
}

fn read_path_values<const N: usize>(
    tokens: &[svg_core::SvgPathToken],
    index: &mut usize,
) -> Option<[f64; N]> {
    let mut values = [0.0; N];
    for value in &mut values {
        *value = read_path_number(tokens, index)?;
    }
    Some(values)
}

fn push_path_segment(active: &mut Option<PathSubpath>, segment: PathSegment) {
    if let Some(subpath) = active.as_mut() {
        subpath.segments.push(segment);
    }
}

fn finish_subpath(active: &mut Option<PathSubpath>, data: &mut PathData) {
    if let Some(subpath) = active.take() {
        data.subpaths.push(subpath);
    }
}

fn skip_path_numbers(tokens: &[svg_core::SvgPathToken], index: &mut usize) {
    while *index < tokens.len() && !matches!(tokens[*index], svg_core::SvgPathToken::Command(_)) {
        *index += 1;
    }
}

// ---------------------------------------------------------------------------
// Transform-aware path flattening
// ---------------------------------------------------------------------------

struct FlattenedSubpath {
    points: Vec<(f32, f32)>,
    closed: bool,
}

fn flatten_path_data(
    data: &PathData,
    transform: &Transform,
    tolerance_px: f64,
) -> Vec<FlattenedSubpath> {
    data.subpaths
        .iter()
        .filter_map(|subpath| {
            let mut points = Vec::new();
            points.push(subpath.start);
            let mut from = subpath.start;
            for segment in &subpath.segments {
                flatten_path_segment(&mut points, from, segment, transform, tolerance_px);
                from = segment.end();
                if points.len() >= MAX_FLAT_PTS {
                    break;
                }
            }
            if subpath.closed && points.last().copied() != Some(subpath.start) {
                if points.len() < MAX_FLAT_PTS {
                    points.push(subpath.start);
                } else if let Some(last) = points.last_mut() {
                    *last = subpath.start;
                }
            }
            (points.len() >= 2).then(|| FlattenedSubpath {
                points: points
                    .into_iter()
                    .map(|(x, y)| (x as f32, y as f32))
                    .collect(),
                closed: subpath.closed,
            })
        })
        .collect()
}

fn flatten_path_segment(
    points: &mut Vec<PathPoint>,
    from: PathPoint,
    segment: &PathSegment,
    transform: &Transform,
    tolerance_px: f64,
) {
    match *segment {
        PathSegment::Line { to } => push_flat_point(points, to),
        PathSegment::Quadratic { ctrl, to } => {
            let ctrl1 = (
                from.0 + (ctrl.0 - from.0) * 2.0 / 3.0,
                from.1 + (ctrl.1 - from.1) * 2.0 / 3.0,
            );
            let ctrl2 = (
                to.0 + (ctrl.0 - to.0) * 2.0 / 3.0,
                to.1 + (ctrl.1 - to.1) * 2.0 / 3.0,
            );
            flatten_cubic_device(points, from, ctrl1, ctrl2, to, transform, tolerance_px, 0);
        }
        PathSegment::Cubic { ctrl1, ctrl2, to } => {
            flatten_cubic_device(points, from, ctrl1, ctrl2, to, transform, tolerance_px, 0);
        }
        PathSegment::Arc {
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            to,
        } => flatten_arc_device(
            points,
            from,
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            to,
            transform,
            tolerance_px,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn flatten_cubic_device(
    points: &mut Vec<PathPoint>,
    p0: PathPoint,
    p1: PathPoint,
    p2: PathPoint,
    p3: PathPoint,
    transform: &Transform,
    tolerance_px: f64,
    depth: u32,
) {
    if depth >= 24 || points.len() >= MAX_FLAT_PTS {
        push_flat_point(points, p3);
        return;
    }
    let tp0 = transform.apply(p0.0, p0.1);
    let tp1 = transform.apply(p1.0, p1.1);
    let tp2 = transform.apply(p2.0, p2.1);
    let tp3 = transform.apply(p3.0, p3.1);
    let d1 = distance_to_line(tp1, tp0, tp3);
    let d2 = distance_to_line(tp2, tp0, tp3);
    if d1.max(d2) <= tolerance_px {
        push_flat_point(points, p3);
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let middle = midpoint(p012, p123);
    flatten_cubic_device(
        points,
        p0,
        p01,
        p012,
        middle,
        transform,
        tolerance_px,
        depth + 1,
    );
    flatten_cubic_device(
        points,
        middle,
        p123,
        p23,
        p3,
        transform,
        tolerance_px,
        depth + 1,
    );
}

fn midpoint(a: PathPoint, b: PathPoint) -> PathPoint {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

fn distance_to_line(point: PathPoint, start: PathPoint, end: PathPoint) -> f64 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        return ((point.0 - start.0).powi(2) + (point.1 - start.1).powi(2)).sqrt();
    }
    (dx * (start.1 - point.1) - dy * (start.0 - point.0)).abs() / len
}

fn push_flat_point(points: &mut Vec<PathPoint>, point: PathPoint) {
    if points.len() < MAX_FLAT_PTS {
        points.push(point);
    } else if let Some(last) = points.last_mut() {
        *last = point;
    }
}

// ---------------------------------------------------------------------------
// SVG arc → line approximation
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn flatten_arc_device(
    points: &mut Vec<PathPoint>,
    from: PathPoint,
    rx: f64,
    ry: f64,
    x_rot_deg: f64,
    large_arc: bool,
    sweep: bool,
    to: PathPoint,
    transform: &Transform,
    tolerance_px: f64,
) {
    if rx <= 0.0 || ry <= 0.0 || from == to {
        push_flat_point(points, to);
        return;
    }
    let phi = x_rot_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    let dx = (from.0 - to.0) * 0.5;
    let dy = (from.1 - to.1) * 0.5;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    let mut rx = rx.abs();
    let mut ry = ry.abs();
    let lambda = (x1p / rx).powi(2) + (y1p / ry).powi(2);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }

    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;

    let num = (rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2).max(0.0);
    let den = rx2 * y1p2 + ry2 * x1p2;
    let sq = if den.abs() < 1e-9 {
        0.0
    } else {
        (num / den).sqrt()
    };
    let sq = if large_arc == sweep { -sq } else { sq };

    let cxp = sq * rx * y1p / ry;
    let cyp = -sq * ry * x1p / rx;

    let cx = cos_phi * cxp - sin_phi * cyp + (from.0 + to.0) * 0.5;
    let cy = sin_phi * cxp + cos_phi * cyp + (from.1 + to.1) * 0.5;

    let theta1 = angle_between(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut d_theta = angle_between(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );

    if !sweep && d_theta > 0.0 {
        d_theta -= std::f64::consts::TAU;
    } else if sweep && d_theta < 0.0 {
        d_theta += std::f64::consts::TAU;
    }

    let radius_px = rx.max(ry) * affine_max_scale(*transform);
    let max_angle = if radius_px > tolerance_px && tolerance_px > 0.0 {
        (2.0 * (1.0 - tolerance_px / radius_px).clamp(-1.0, 1.0).acos())
            .clamp(0.01, std::f64::consts::FRAC_PI_2)
    } else {
        std::f64::consts::FRAC_PI_2
    };
    let remaining = MAX_FLAT_PTS.saturating_sub(points.len()).max(1);
    let n = ((d_theta.abs() / max_angle).ceil() as usize)
        .clamp(4, 4096)
        .min(remaining);
    for k in 1..=n {
        let t = theta1 + d_theta * (k as f64 / n as f64);
        let px = cos_phi * rx * t.cos() - sin_phi * ry * t.sin() + cx;
        let py = sin_phi * rx * t.cos() + cos_phi * ry * t.sin() + cy;
        push_flat_point(points, (px, py));
    }
}

fn affine_max_scale(transform: Transform) -> f64 {
    let aa = transform.a * transform.a + transform.b * transform.b;
    let bb = transform.c * transform.c + transform.d * transform.d;
    let cross = transform.a * transform.c + transform.b * transform.d;
    let discriminant = ((aa - bb) * (aa - bb) + 4.0 * cross * cross).sqrt();
    ((aa + bb + discriminant) * 0.5).max(0.0).sqrt()
}

fn angle_between(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
    let dot = ux * vx + uy * vy;
    let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
    if len < 1e-12 {
        return 0.0;
    }
    let angle = (dot / len).clamp(-1.0, 1.0).acos();
    if ux * vy - uy * vx < 0.0 {
        -angle
    } else {
        angle
    }
}

// ---------------------------------------------------------------------------
// Shape polygon helpers
// ---------------------------------------------------------------------------

fn ellipse_pts(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<(f32, f32)> {
    let n = ((rx.max(ry) * 3.0) as usize).clamp(16, 128);
    (0..n)
        .map(|k| {
            let t = (k as f32 / n as f32) * std::f32::consts::TAU;
            (cx + t.cos() * rx, cy + t.sin() * ry)
        })
        .collect()
}

fn rounded_rect_pts(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> Vec<(f32, f32)> {
    if rx <= 0.0 && ry <= 0.0 {
        return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    }
    let rx = rx.min(w * 0.5);
    let ry = ry.min(h * 0.5);
    let n = 8usize; // per quarter arc
    let mut pts = Vec::with_capacity(n * 4 + 4);

    // top-left arc: π → 3π/2
    for k in 0..n {
        let t = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * (k as f32 / n as f32);
        pts.push((x + rx + rx * t.cos(), y + ry + ry * t.sin()));
    }
    // top-right arc: 3π/2 → 2π
    for k in 0..n {
        let t =
            3.0 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * (k as f32 / n as f32);
        pts.push((x + w - rx + rx * t.cos(), y + ry + ry * t.sin()));
    }
    // bottom-right arc: 0 → π/2
    for k in 0..n {
        let t = std::f32::consts::FRAC_PI_2 * (k as f32 / n as f32);
        pts.push((x + w - rx + rx * t.cos(), y + h - ry + ry * t.sin()));
    }
    // bottom-left arc: π/2 → π  (include endpoint)
    for k in 0..=n {
        let t = std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * (k as f32 / n as f32);
        pts.push((x + rx + rx * t.cos(), y + h - ry + ry * t.sin()));
    }
    pts
}

fn parse_point_list(s: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = s
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect();
    nums.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect()
}

fn local_bounds(points: &[(f32, f32)]) -> [f64; 4] {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for &(x, y) in points {
        let (x, y) = (x as f64, y as f64);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    }
    bounds
}

// ---------------------------------------------------------------------------
// Pixel fill algorithms
// ---------------------------------------------------------------------------

fn fill_polygon(
    target: &mut RasterTarget<'_>,
    points: &[(f32, f32)],
    fill_rule: FillRule,
    paint: &PaintSampler,
) {
    rasterize_fill_coverage(target, &[points], fill_rule, paint);
}

#[derive(Clone, Copy)]
struct ScanCrossing {
    x: f64,
    winding_delta: i32,
}

fn rasterize_fill_coverage(
    target: &mut RasterTarget<'_>,
    sub_paths: &[&[(f32, f32)]],
    fill_rule: FillRule,
    paint: &PaintSampler,
) {
    rasterize_coverage(target, sub_paths, fill_rule, paint);
}

fn rasterize_stroke_union_coverage(
    target: &mut RasterTarget<'_>,
    polygons: &[&[(f32, f32)]],
    paint: &PaintSampler,
) {
    rasterize_coverage(target, polygons, FillRule::Nonzero, paint);
}

fn rasterize_coverage(
    target: &mut RasterTarget<'_>,
    sub_paths: &[&[(f32, f32)]],
    fill_rule: FillRule,
    paint: &PaintSampler,
) {
    if paint.is_transparent() || sub_paths.is_empty() {
        return;
    }
    let (w, h) = (target.width, target.height);
    coverage_scan(w, h, sub_paths, fill_rule, |x, y, samples| {
        let mut covered = paint.sample(x as f64 + 0.5, y as f64 + 0.5);
        covered[3] =
            ((covered[3] as u32 * samples + COVERAGE_SAMPLES / 2) / COVERAGE_SAMPLES) as u8;
        target.composite(x, y, covered);
    });
}

/// Deterministic 8x8 subpixel coverage scan shared by fill, stroke-union, and
/// clip-mask rasterization.  Invokes `emit(col, row, samples)` once per touched
/// pixel, where `samples` is the covered subsample count (1..=COVERAGE_SAMPLES).
/// No pixels are written here; the caller decides how to use the coverage
/// (composite a paint, or accumulate a clip alpha).
fn coverage_scan(
    w: usize,
    h: usize,
    sub_paths: &[&[(f32, f32)]],
    fill_rule: FillRule,
    mut emit: impl FnMut(usize, usize, u32),
) {
    if sub_paths.is_empty() {
        return;
    }

    let min_x = sub_paths
        .iter()
        .flat_map(|path| path.iter().map(|point| point.0 as f64))
        .fold(f64::INFINITY, f64::min);
    let max_x = sub_paths
        .iter()
        .flat_map(|path| path.iter().map(|point| point.0 as f64))
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = sub_paths
        .iter()
        .flat_map(|path| path.iter().map(|point| point.1 as f64))
        .fold(f64::INFINITY, f64::min);
    let max_y = sub_paths
        .iter()
        .flat_map(|path| path.iter().map(|point| point.1 as f64))
        .fold(f64::NEG_INFINITY, f64::max);
    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return;
    }
    let col_start = min_x.floor().max(0.0).min(w as f64) as usize;
    let col_end = max_x.ceil().max(0.0).min(w as f64) as usize;
    let row_start = min_y.floor().max(0.0).min(h as f64) as usize;
    let row_end = max_y.ceil().max(0.0).min(h as f64) as usize;
    if col_start >= col_end || row_start >= row_end {
        return;
    }

    for tile_start in (row_start..row_end).step_by(COVERAGE_TILE_ROWS) {
        let tile_end = (tile_start + COVERAGE_TILE_ROWS).min(row_end);
        let tile_height = tile_end - tile_start;
        let tile_width = col_end - col_start;
        let mut coverage = vec![0u64; tile_width * tile_height];

        for row in tile_start..tile_end {
            for sample_y in 0..COVERAGE_GRID {
                let yf = row as f64 + (sample_y as f64 + 0.5) / COVERAGE_GRID as f64;
                let mut crossings = Vec::new();
                for sub in sub_paths {
                    let n = sub.len();
                    if n < 2 {
                        continue;
                    }
                    for k in 0..n {
                        let (x0, y0) = (sub[k].0 as f64, sub[k].1 as f64);
                        let next = sub[(k + 1) % n];
                        let (x1, y1) = (next.0 as f64, next.1 as f64);
                        if (y0 <= yf && y1 > yf) || (y1 <= yf && y0 > yf) {
                            let t = (yf - y0) / (y1 - y0);
                            crossings.push(ScanCrossing {
                                x: x0 + t * (x1 - x0),
                                winding_delta: if y1 > y0 { 1 } else { -1 },
                            });
                        }
                    }
                }
                crossings.sort_by(|left, right| left.x.total_cmp(&right.x));

                let mut index = 0;
                let mut winding = 0;
                let mut parity = false;
                while index < crossings.len() {
                    let x = crossings[index].x;
                    let mut next = index;
                    let mut winding_delta = 0;
                    let mut crossing_count = 0;
                    while next < crossings.len() && crossing_x_eq(crossings[next].x, x) {
                        winding_delta += crossings[next].winding_delta;
                        crossing_count += 1;
                        next += 1;
                    }
                    winding += winding_delta;
                    if crossing_count % 2 == 1 {
                        parity = !parity;
                    }

                    if let Some(next_crossing) = crossings.get(next) {
                        let inside = match fill_rule {
                            FillRule::Nonzero => winding != 0,
                            FillRule::Evenodd => parity,
                        };
                        if inside {
                            mark_coverage_interval(
                                &mut coverage,
                                tile_width,
                                row - tile_start,
                                sample_y,
                                col_start,
                                col_end,
                                x,
                                next_crossing.x,
                            );
                        }
                    }
                    index = next;
                }
            }
        }

        for local_row in 0..tile_height {
            for local_col in 0..tile_width {
                let samples = coverage[local_row * tile_width + local_col].count_ones();
                if samples == 0 {
                    continue;
                }
                emit(col_start + local_col, tile_start + local_row, samples);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn mark_coverage_interval(
    coverage: &mut [u64],
    tile_width: usize,
    local_row: usize,
    sample_y: usize,
    col_start: usize,
    col_end: usize,
    left: f64,
    right: f64,
) {
    let sample_limit = col_end.saturating_mul(COVERAGE_GRID);
    let first = (left * COVERAGE_GRID as f64 - 0.5).ceil() as i64;
    let last = (right * COVERAGE_GRID as f64 - 0.5).ceil() as i64;
    let first = first.clamp((col_start * COVERAGE_GRID) as i64, sample_limit as i64) as usize;
    let last = last.clamp((col_start * COVERAGE_GRID) as i64, sample_limit as i64) as usize;
    for sample in first..last {
        let col = sample / COVERAGE_GRID;
        let sample_x = sample % COVERAGE_GRID;
        let bit = sample_y * COVERAGE_GRID + sample_x;
        coverage[local_row * tile_width + (col - col_start)] |= 1u64 << bit;
    }
}

fn crossing_x_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1.0e-5 * left.abs().max(right.abs()).max(1.0)
}

fn stroke_polyline(
    target: &mut RasterTarget<'_>,
    pts: &[(f32, f32)],
    transform: &Transform,
    stroke: &ResolvedStroke,
    paint: &PaintSampler,
    closed: bool,
    path_length: Option<f64>,
) -> StrokeRenderOutcome {
    if pts.is_empty() || paint.is_transparent() || stroke.width <= 0.0 {
        return StrokeRenderOutcome::default();
    }
    let local: Vec<PathPoint> = pts
        .iter()
        .filter_map(|&(x, y)| {
            let point = (x as f64, y as f64);
            (point.0.is_finite() && point.1.is_finite()).then_some(point)
        })
        .collect();
    let (runs, mut limit_hit) = if let Some(pattern) = stroke.dash_array.as_deref() {
        dash_polyline(&local, closed, pattern, stroke.dash_offset, path_length)
    } else {
        (
            vec![StrokeRun {
                points: local,
                closed,
            }],
            false,
        )
    };
    let mut polygons = Vec::new();
    let mut vertex_count = 0;
    let solid_stroke = ResolvedStroke {
        dash_array: None,
        dash_offset: 0.0,
        ..stroke.clone()
    };
    'runs: for run in runs {
        let mesh = build_stroke_mesh(&run.points, transform, &solid_stroke, run.closed);
        for polygon in mesh.polygons {
            if polygons.len() >= MAX_STROKE_PRIMITIVES
                || vertex_count + polygon.len() > MAX_STROKE_VERTICES
            {
                limit_hit = true;
                break 'runs;
            }
            vertex_count += polygon.len();
            polygons.push(polygon);
        }
    }
    let mesh = finish_stroke_mesh(polygons, transform);
    if mesh.polygons.is_empty() {
        return StrokeRenderOutcome {
            drawn: false,
            limit_hit,
        };
    }
    debug_assert!(mesh.local_bounds.is_finite());
    debug_assert!(mesh.device_bounds.is_finite());

    let transformed: Vec<Vec<(f32, f32)>> = mesh
        .polygons
        .iter()
        .map(|polygon| {
            polygon
                .iter()
                .map(|&(x, y)| transform.apply_f32(x as f32, y as f32))
                .collect()
        })
        .collect();
    let refs: Vec<&[(f32, f32)]> = transformed.iter().map(Vec::as_slice).collect();
    rasterize_stroke_union_coverage(target, &refs, paint);
    StrokeRenderOutcome {
        drawn: true,
        limit_hit,
    }
}

#[derive(Default)]
struct StrokeRenderOutcome {
    drawn: bool,
    limit_hit: bool,
}

struct StrokeRun {
    points: Vec<PathPoint>,
    closed: bool,
}

fn dash_polyline(
    input: &[PathPoint],
    closed: bool,
    pattern: &[f64],
    dash_offset: f64,
    path_length: Option<f64>,
) -> (Vec<StrokeRun>, bool) {
    let mut points = normalized_polyline(input, closed);
    if points.is_empty() || pattern.is_empty() {
        return (Vec::new(), false);
    }
    if closed && points.len() > 1 {
        points.push(points[0]);
    }
    let total_length: f64 = points
        .windows(2)
        .map(|segment| distance(segment[0], segment[1]))
        .sum();
    if total_length <= 1e-12 {
        return (
            vec![StrokeRun {
                points: vec![points[0]],
                closed: false,
            }],
            false,
        );
    }

    let calibration = path_length
        .filter(|length| *length > 0.0 && length.is_finite())
        .map_or(1.0, |length| total_length / length);
    let pattern: Vec<f64> = pattern.iter().map(|value| value * calibration).collect();
    let pattern_length: f64 = pattern.iter().sum();
    if pattern_length <= 1e-12 || !pattern_length.is_finite() {
        return (vec![StrokeRun { points, closed }], false);
    }

    let mut phase = (-dash_offset * calibration).rem_euclid(pattern_length);
    let mut pattern_index = 0usize;
    while phase > pattern[pattern_index] && pattern_index + 1 < pattern.len() {
        phase -= pattern[pattern_index];
        pattern_index += 1;
    }
    let mut remaining = (pattern[pattern_index] - phase).max(0.0);
    let mut painted = pattern_index.is_multiple_of(2);
    let mut runs = Vec::new();
    let mut active = Vec::new();
    let mut limit_hit = false;

    'segments: for segment in points.windows(2) {
        let mut cursor = segment[0];
        let endpoint = segment[1];
        let Some(direction) = unit_vector(cursor, endpoint) else {
            continue;
        };
        let mut segment_remaining = distance(cursor, endpoint);
        while segment_remaining > 1e-12 {
            if runs.len() >= MAX_DASH_RUNS {
                limit_hit = true;
                break 'segments;
            }
            advance_zero_dash_entries(
                &pattern,
                &mut pattern_index,
                &mut remaining,
                &mut painted,
                cursor,
                &mut runs,
            );
            if runs.len() >= MAX_DASH_RUNS {
                limit_hit = true;
                break 'segments;
            }
            let step = segment_remaining.min(remaining);
            let next = add(cursor, mul(direction, step));
            if painted {
                if active.is_empty() {
                    active.push(cursor);
                }
                if active
                    .last()
                    .is_none_or(|last| distance(*last, next) > 1e-12)
                {
                    active.push(next);
                }
            }
            cursor = next;
            segment_remaining = (segment_remaining - step).max(0.0);
            remaining = (remaining - step).max(0.0);
            if remaining <= 1e-12 {
                if painted && !active.is_empty() {
                    runs.push(StrokeRun {
                        points: std::mem::take(&mut active),
                        closed: false,
                    });
                }
                pattern_index = (pattern_index + 1) % pattern.len();
                painted = pattern_index.is_multiple_of(2);
                remaining = pattern[pattern_index];
            }
        }
    }
    if painted && !active.is_empty() {
        if runs.len() < MAX_DASH_RUNS {
            runs.push(StrokeRun {
                points: active,
                closed: false,
            });
        } else {
            limit_hit = true;
        }
    }

    if closed && runs.len() == 1 {
        let run = &mut runs[0];
        if run.points.len() > 2
            && run
                .points
                .last()
                .is_some_and(|last| distance(run.points[0], *last) <= 1e-10)
        {
            run.closed = true;
        }
    } else if closed && runs.len() >= 2 {
        let seam = points[0];
        let merge = runs
            .first()
            .and_then(|first| first.points.first())
            .is_some_and(|point| distance(*point, seam) <= 1e-10)
            && runs
                .last()
                .and_then(|last| last.points.last())
                .is_some_and(|point| distance(*point, seam) <= 1e-10);
        if merge {
            let first = runs.remove(0);
            if let Some(mut last) = runs.pop() {
                last.points.extend(first.points.into_iter().skip(1));
                runs.insert(0, last);
            }
        }
    }
    (runs, limit_hit)
}

fn advance_zero_dash_entries(
    pattern: &[f64],
    pattern_index: &mut usize,
    remaining: &mut f64,
    painted: &mut bool,
    cursor: PathPoint,
    runs: &mut Vec<StrokeRun>,
) {
    let mut guard = 0;
    while *remaining <= 1e-12 && guard < pattern.len() {
        if *painted && runs.len() < MAX_DASH_RUNS {
            runs.push(StrokeRun {
                points: vec![cursor],
                closed: false,
            });
        }
        *pattern_index = (*pattern_index + 1) % pattern.len();
        *painted = (*pattern_index).is_multiple_of(2);
        *remaining = pattern[*pattern_index];
        guard += 1;
    }
}

fn normalized_polyline(input: &[PathPoint], closed: bool) -> Vec<PathPoint> {
    let mut points = Vec::with_capacity(input.len());
    for &point in input {
        if !point.0.is_finite() || !point.1.is_finite() {
            continue;
        }
        if points
            .last()
            .is_none_or(|last| distance(*last, point) > 1e-10)
        {
            points.push(point);
        }
    }
    if closed
        && points.len() > 1
        && points
            .last()
            .is_some_and(|last| distance(points[0], *last) <= 1e-10)
    {
        points.pop();
    }
    points
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds2D {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds2D {
    fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn include(&mut self, point: PathPoint) {
        if !point.0.is_finite() || !point.1.is_finite() {
            return;
        }
        self.min_x = self.min_x.min(point.0);
        self.min_y = self.min_y.min(point.1);
        self.max_x = self.max_x.max(point.0);
        self.max_y = self.max_y.max(point.1);
    }

    fn is_finite(self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
            && self.min_x <= self.max_x
            && self.min_y <= self.max_y
    }
}

struct StrokeMesh {
    polygons: Vec<Vec<PathPoint>>,
    local_bounds: Bounds2D,
    device_bounds: Bounds2D,
}

#[derive(Clone, Copy)]
struct StrokeEdge {
    tangent: PathPoint,
    normal: PathPoint,
}

fn build_stroke_mesh(
    input: &[PathPoint],
    transform: &Transform,
    stroke: &ResolvedStroke,
    closed: bool,
) -> StrokeMesh {
    let mut points = Vec::with_capacity(input.len());
    for &point in input {
        if !point.0.is_finite() || !point.1.is_finite() {
            continue;
        }
        if points
            .last()
            .is_none_or(|last| distance(*last, point) > 1e-10)
        {
            points.push(point);
        }
    }
    if closed
        && points.len() > 1
        && points
            .last()
            .is_some_and(|last| distance(points[0], *last) <= 1e-10)
    {
        points.pop();
    }

    let half = stroke.width * 0.5;
    let round_steps = ((half * affine_max_scale(*transform) * 1.5).ceil() as usize).clamp(8, 128);
    let mut polygons = Vec::new();

    if points.len() == 1 {
        add_zero_length_cap(&mut polygons, points[0], half, stroke.linecap, round_steps);
        return finish_stroke_mesh(polygons, transform);
    }

    let use_closed = closed && points.len() >= 3;
    let edge_count = if use_closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    let mut edges = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let from = points[index];
        let to = points[(index + 1) % points.len()];
        let Some(tangent) = unit_vector(from, to) else {
            continue;
        };
        let normal = (-tangent.1 * half, tangent.0 * half);
        edges.push(StrokeEdge { tangent, normal });
        add_polygon(
            &mut polygons,
            vec![
                add(from, normal),
                sub(from, normal),
                sub(to, normal),
                add(to, normal),
            ],
        );
    }

    if edges.is_empty() {
        add_zero_length_cap(&mut polygons, points[0], half, stroke.linecap, round_steps);
        return finish_stroke_mesh(polygons, transform);
    }

    if use_closed {
        for index in 0..points.len() {
            let previous = edges[(index + edges.len() - 1) % edges.len()];
            let next = edges[index % edges.len()];
            add_stroke_join(
                &mut polygons,
                points[index],
                previous,
                next,
                half,
                stroke.linejoin,
                stroke.miterlimit,
                round_steps,
            );
        }
    } else {
        for index in 1..points.len() - 1 {
            add_stroke_join(
                &mut polygons,
                points[index],
                edges[index - 1],
                edges[index],
                half,
                stroke.linejoin,
                stroke.miterlimit,
                round_steps,
            );
        }
        add_line_cap(
            &mut polygons,
            points[0],
            edges[0],
            half,
            stroke.linecap,
            true,
            round_steps,
        );
        add_line_cap(
            &mut polygons,
            *points.last().unwrap(),
            *edges.last().unwrap(),
            half,
            stroke.linecap,
            false,
            round_steps,
        );
    }

    debug_assert!(
        stroke.dash_array.is_none()
            || stroke
                .dash_array
                .as_ref()
                .is_some_and(|values| !values.is_empty())
    );
    debug_assert!(stroke.dash_offset.is_finite());
    finish_stroke_mesh(polygons, transform)
}

fn finish_stroke_mesh(polygons: Vec<Vec<PathPoint>>, transform: &Transform) -> StrokeMesh {
    let mut local_bounds = Bounds2D::empty();
    let mut device_bounds = Bounds2D::empty();
    for polygon in &polygons {
        for &point in polygon {
            local_bounds.include(point);
            device_bounds.include(transform.apply(point.0, point.1));
        }
    }
    StrokeMesh {
        polygons,
        local_bounds,
        device_bounds,
    }
}

fn add_line_cap(
    polygons: &mut Vec<Vec<PathPoint>>,
    center: PathPoint,
    edge: StrokeEdge,
    half: f64,
    cap: StrokeLineCap,
    start: bool,
    round_steps: usize,
) {
    match cap {
        StrokeLineCap::Butt => {}
        StrokeLineCap::Round => add_circle(polygons, center, half, round_steps * 2),
        StrokeLineCap::Square => {
            let extension = if start {
                mul(edge.tangent, -half)
            } else {
                mul(edge.tangent, half)
            };
            add_polygon(
                polygons,
                vec![
                    add(center, edge.normal),
                    sub(center, edge.normal),
                    add(sub(center, edge.normal), extension),
                    add(add(center, edge.normal), extension),
                ],
            );
        }
    }
}

fn add_zero_length_cap(
    polygons: &mut Vec<Vec<PathPoint>>,
    center: PathPoint,
    half: f64,
    cap: StrokeLineCap,
    round_steps: usize,
) {
    match cap {
        StrokeLineCap::Butt => {}
        StrokeLineCap::Round => add_circle(polygons, center, half, round_steps * 2),
        StrokeLineCap::Square => add_polygon(
            polygons,
            vec![
                (center.0 - half, center.1 - half),
                (center.0 + half, center.1 - half),
                (center.0 + half, center.1 + half),
                (center.0 - half, center.1 + half),
            ],
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn add_stroke_join(
    polygons: &mut Vec<Vec<PathPoint>>,
    center: PathPoint,
    previous: StrokeEdge,
    next: StrokeEdge,
    half: f64,
    join: StrokeLineJoin,
    miterlimit: f64,
    round_steps: usize,
) {
    let turn = cross(previous.tangent, next.tangent);
    let direction_dot = dot(previous.tangent, next.tangent);
    if turn.abs() <= 1e-10 {
        if direction_dot < 0.0 {
            match join {
                StrokeLineJoin::Round => add_circle(polygons, center, half, round_steps * 2),
                _ => {
                    add_zero_length_cap(polygons, center, half, StrokeLineCap::Square, round_steps)
                }
            }
        }
        return;
    }

    let side = if turn > 0.0 { -1.0 } else { 1.0 };
    let outer_previous = add(center, mul(previous.normal, side));
    let outer_next = add(center, mul(next.normal, side));

    match join {
        StrokeLineJoin::Bevel => {
            add_polygon(polygons, vec![center, outer_previous, outer_next]);
        }
        StrokeLineJoin::Round => {
            add_round_join(
                polygons,
                center,
                outer_previous,
                outer_next,
                turn,
                round_steps,
            );
        }
        StrokeLineJoin::Miter | StrokeLineJoin::MiterClip | StrokeLineJoin::Arcs => {
            let Some(miter) =
                line_intersection(outer_previous, previous.tangent, outer_next, next.tangent)
            else {
                add_polygon(polygons, vec![center, outer_previous, outer_next]);
                return;
            };
            let ratio = distance(center, miter) / half.max(1e-12);
            if ratio <= miterlimit {
                add_polygon(polygons, vec![outer_previous, miter, outer_next]);
            } else if join == StrokeLineJoin::Miter {
                add_polygon(polygons, vec![center, outer_previous, outer_next]);
            } else {
                let bisector = unit_vector(center, miter).unwrap_or((0.0, 0.0));
                let clip_distance = half * miterlimit;
                let clip_previous = intersect_line_with_projection(
                    outer_previous,
                    previous.tangent,
                    center,
                    bisector,
                    clip_distance,
                )
                .unwrap_or(outer_previous);
                let clip_next = intersect_line_with_projection(
                    outer_next,
                    next.tangent,
                    center,
                    bisector,
                    clip_distance,
                )
                .unwrap_or(outer_next);
                add_polygon(
                    polygons,
                    vec![outer_previous, clip_previous, clip_next, outer_next],
                );
            }
        }
    }
}

fn add_round_join(
    polygons: &mut Vec<Vec<PathPoint>>,
    center: PathPoint,
    from: PathPoint,
    to: PathPoint,
    turn: f64,
    round_steps: usize,
) {
    let start = (from.1 - center.1).atan2(from.0 - center.0);
    let mut end = (to.1 - center.1).atan2(to.0 - center.0);
    if turn > 0.0 {
        while end < start {
            end += std::f64::consts::TAU;
        }
    } else {
        while end > start {
            end -= std::f64::consts::TAU;
        }
    }
    let delta = end - start;
    let count = ((delta.abs() / std::f64::consts::PI * round_steps as f64).ceil() as usize)
        .clamp(1, round_steps);
    let radius = distance(center, from);
    let mut polygon = Vec::with_capacity(count + 2);
    polygon.push(center);
    for index in 0..=count {
        let angle = start + delta * index as f64 / count as f64;
        polygon.push((
            center.0 + radius * angle.cos(),
            center.1 + radius * angle.sin(),
        ));
    }
    add_polygon(polygons, polygon);
}

fn add_circle(polygons: &mut Vec<Vec<PathPoint>>, center: PathPoint, radius: f64, steps: usize) {
    let count = steps.clamp(8, 256);
    let polygon = (0..count)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / count as f64;
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect();
    add_polygon(polygons, polygon);
}

fn add_polygon(polygons: &mut Vec<Vec<PathPoint>>, mut polygon: Vec<PathPoint>) {
    if polygon.len() < 3
        || polygon
            .iter()
            .any(|point| !point.0.is_finite() || !point.1.is_finite())
    {
        return;
    }
    if signed_area(&polygon) < 0.0 {
        polygon.reverse();
    }
    polygons.push(polygon);
}

fn signed_area(polygon: &[PathPoint]) -> f64 {
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
        .map(|(&(x0, y0), &(x1, y1))| x0 * y1 - y0 * x1)
        .sum::<f64>()
        * 0.5
}

fn line_intersection(
    first: PathPoint,
    first_direction: PathPoint,
    second: PathPoint,
    second_direction: PathPoint,
) -> Option<PathPoint> {
    let denominator = cross(first_direction, second_direction);
    if denominator.abs() <= 1e-12 {
        return None;
    }
    let delta = sub(second, first);
    let distance = cross(delta, second_direction) / denominator;
    Some(add(first, mul(first_direction, distance)))
}

fn intersect_line_with_projection(
    line_point: PathPoint,
    line_direction: PathPoint,
    origin: PathPoint,
    projection_direction: PathPoint,
    projection: f64,
) -> Option<PathPoint> {
    let denominator = dot(line_direction, projection_direction);
    if denominator.abs() <= 1e-12 {
        return None;
    }
    let current = dot(sub(line_point, origin), projection_direction);
    let distance = (projection - current) / denominator;
    Some(add(line_point, mul(line_direction, distance)))
}

fn unit_vector(from: PathPoint, to: PathPoint) -> Option<PathPoint> {
    let delta = sub(to, from);
    let length = (delta.0 * delta.0 + delta.1 * delta.1).sqrt();
    (length > 1e-12).then_some((delta.0 / length, delta.1 / length))
}

fn distance(a: PathPoint, b: PathPoint) -> f64 {
    let delta = sub(a, b);
    (delta.0 * delta.0 + delta.1 * delta.1).sqrt()
}

fn add(a: PathPoint, b: PathPoint) -> PathPoint {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: PathPoint, b: PathPoint) -> PathPoint {
    (a.0 - b.0, a.1 - b.1)
}

fn mul(point: PathPoint, scalar: f64) -> PathPoint {
    (point.0 * scalar, point.1 * scalar)
}

fn dot(a: PathPoint, b: PathPoint) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(a: PathPoint, b: PathPoint) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// Porter-Duff "src over dst" alpha compositing.
fn blend_pixel(buf: &mut [u8], w: usize, x: usize, y: usize, src: [u8; 4]) {
    let idx = (y * w + x) * 4;
    if idx + 3 >= buf.len() {
        return;
    }
    let sa = src[3] as f32 / 255.0;
    if sa <= 0.0 {
        return;
    }
    let da = buf[idx + 3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a > 0.0 {
        let inv = da * (1.0 - sa);
        buf[idx] = ((src[0] as f32 * sa + buf[idx] as f32 * inv) / out_a).round() as u8;
        buf[idx + 1] = ((src[1] as f32 * sa + buf[idx + 1] as f32 * inv) / out_a).round() as u8;
        buf[idx + 2] = ((src[2] as f32 * sa + buf[idx + 2] as f32 * inv) / out_a).round() as u8;
        buf[idx + 3] = (out_a * 255.0).round() as u8;
    }
}

/// "src over dst" into a *premultiplied* RGBA buffer.  `src` arrives straight
/// (non-premultiplied); it is premultiplied here before accumulation.  Working
/// in premultiplied space keeps isolated-group compositing halo-free: the color
/// of fully-transparent destination samples never leaks into the result.
fn blend_pixel_premultiplied(buf: &mut [u8], w: usize, x: usize, y: usize, src: [u8; 4]) {
    let idx = (y * w + x) * 4;
    if idx + 3 >= buf.len() {
        return;
    }
    let sa = src[3] as f32 / 255.0;
    if sa <= 0.0 {
        return;
    }
    let inv = 1.0 - sa;
    // Destination is already premultiplied.
    buf[idx] = (src[0] as f32 * sa + buf[idx] as f32 * inv)
        .round()
        .clamp(0.0, 255.0) as u8;
    buf[idx + 1] = (src[1] as f32 * sa + buf[idx + 1] as f32 * inv)
        .round()
        .clamp(0.0, 255.0) as u8;
    buf[idx + 2] = (src[2] as f32 * sa + buf[idx + 2] as f32 * inv)
        .round()
        .clamp(0.0, 255.0) as u8;
    buf[idx + 3] = (src[3] as f32 + buf[idx + 3] as f32 * inv)
        .round()
        .clamp(0.0, 255.0) as u8;
}

/// Composite a finished isolated-group offscreen (premultiplied RGBA) into its
/// parent buffer at the group `opacity`, applied once for the whole group so
/// overlapping children do not double-darken.  The parent may itself be a
/// premultiplied offscreen or the straight-RGBA output buffer.
fn composite_offscreen(parent: &mut [u8], parent_premultiplied: bool, offscreen: &Offscreen) {
    let opacity = offscreen.opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    if offscreen.blend != BlendMode::Normal {
        composite_offscreen_blended(parent, parent_premultiplied, offscreen, opacity);
        return;
    }
    let src = &offscreen.buf;
    let count = parent.len().min(src.len()) / 4;
    for pixel in 0..count {
        let idx = pixel * 4;
        // Offscreen is premultiplied; scaling all four channels by group opacity
        // is the correct premultiplied "fade" of the whole layer.
        let sr = src[idx] as f32 * opacity;
        let sg = src[idx + 1] as f32 * opacity;
        let sb = src[idx + 2] as f32 * opacity;
        let sa = src[idx + 3] as f32 * opacity / 255.0;
        if sa <= 0.0 {
            continue;
        }
        let inv = 1.0 - sa;
        if parent_premultiplied {
            parent[idx] = (sr + parent[idx] as f32 * inv).round().clamp(0.0, 255.0) as u8;
            parent[idx + 1] = (sg + parent[idx + 1] as f32 * inv)
                .round()
                .clamp(0.0, 255.0) as u8;
            parent[idx + 2] = (sb + parent[idx + 2] as f32 * inv)
                .round()
                .clamp(0.0, 255.0) as u8;
            parent[idx + 3] = (sa * 255.0 + parent[idx + 3] as f32 * inv)
                .round()
                .clamp(0.0, 255.0) as u8;
        } else {
            // Parent is straight RGBA: src premultiplied rgb over straight dst.
            let da = parent[idx + 3] as f32 / 255.0;
            let out_a = sa + da * inv;
            if out_a > 0.0 {
                let dinv = da * inv;
                parent[idx] = ((sr + parent[idx] as f32 * dinv) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                parent[idx + 1] = ((sg + parent[idx + 1] as f32 * dinv) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                parent[idx + 2] = ((sb + parent[idx + 2] as f32 * dinv) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                parent[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// R10 `mix-blend-mode` composite: blend an isolated group offscreen
/// (premultiplied, faded by `opacity`) into its parent with a separable blend.
/// Works in premultiplied space; converts to/from a straight parent at the edge.
fn composite_offscreen_blended(
    parent: &mut [u8],
    parent_premultiplied: bool,
    offscreen: &Offscreen,
    opacity: f32,
) {
    let src = &offscreen.buf;
    let count = parent.len().min(src.len()) / 4;
    for pixel in 0..count {
        let idx = pixel * 4;
        let sa = src[idx + 3] as f32 * opacity / 255.0;
        // Source premultiplied channels, faded by group opacity.
        let s = [
            src[idx] as f32 * opacity / 255.0,
            src[idx + 1] as f32 * opacity / 255.0,
            src[idx + 2] as f32 * opacity / 255.0,
        ];
        let ab = parent[idx + 3] as f32 / 255.0;
        // Backdrop premultiplied channels.
        let d = if parent_premultiplied {
            [
                parent[idx] as f32 / 255.0,
                parent[idx + 1] as f32 / 255.0,
                parent[idx + 2] as f32 / 255.0,
            ]
        } else {
            [
                parent[idx] as f32 / 255.0 * ab,
                parent[idx + 1] as f32 / 255.0 * ab,
                parent[idx + 2] as f32 / 255.0 * ab,
            ]
        };
        if sa <= 0.0 && ab <= 0.0 {
            continue;
        }
        let ao = sa + ab - sa * ab;
        let mut co = [0.0f32; 3];
        for c in 0..3 {
            let sc = if sa > 0.0 { s[c] / sa } else { 0.0 };
            let bc = if ab > 0.0 { d[c] / ab } else { 0.0 };
            co[c] = (1.0 - ab) * s[c]
                + (1.0 - sa) * d[c]
                + sa * ab * blend_channel(offscreen.blend, bc, sc);
        }
        if parent_premultiplied {
            for c in 0..3 {
                parent[idx + c] = (co[c] * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            parent[idx + 3] = (ao * 255.0).round().clamp(0.0, 255.0) as u8;
        } else if ao > 0.0 {
            for c in 0..3 {
                parent[idx + c] = (co[c] / ao * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            parent[idx + 3] = (ao * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

// ---------------------------------------------------------------------------
// R5: embedded raster image decode (zero-dependency PNG + base64)
// ---------------------------------------------------------------------------
//
// Only inline `data:` images are decoded; external references are rejected by
// `svg_text_allowed` (http/https/file) or this module (anything else).  PNG is
// decoded with a from-scratch DEFLATE inflater, scanline unfilter, and pixel
// expansion to straight RGBA8.  Baseline JPEG decode is a tracked R5 follow-on
// and is reported with a specific diagnostic rather than a generic failure.

/// Maximum decoded image pixels (RGBA8 → 4 bytes each).  Shares the raster
/// pixel budget so a decoded image can never out-allocate the canvas cap.
const MAX_IMAGE_PIXELS: usize = MAX_RASTER_PIXELS;
/// Hard cap on a single inflate output (filtered scanline stream).
const MAX_IMAGE_DECODE_BYTES: usize = 96 * 1024 * 1024;

/// A decoded raster image in straight (non-premultiplied) RGBA8, row-major.
struct DecodedImage {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ImageDecodeError {
    Empty,
    NotDataUri,
    ExternalRejected,
    UnsupportedEncoding,
    BadBase64,
    NotImage,
    MalformedPng,
    UnsupportedPng,
    InflateFailed,
    MalformedJpeg,
    UnsupportedJpeg,
    TooLarge,
}

impl ImageDecodeError {
    fn code(self) -> &'static str {
        match self {
            Self::Empty => "image.decode_failed",
            Self::NotDataUri => "image.unsupported_source",
            Self::ExternalRejected => "image.external_rejected",
            Self::UnsupportedEncoding => "image.unsupported_encoding",
            Self::BadBase64 => "image.decode_failed",
            Self::NotImage => "image.unsupported_format",
            Self::MalformedPng => "image.decode_failed",
            Self::UnsupportedPng => "image.unsupported_png",
            Self::InflateFailed => "image.decode_failed",
            Self::MalformedJpeg => "image.decode_failed",
            Self::UnsupportedJpeg => "image.unsupported_jpeg",
            Self::TooLarge => "limit.image_pixels",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Empty => "embedded image had no data; placeholder kept",
            Self::NotDataUri => {
                "image href is not a supported inline data: URI; placeholder kept"
            }
            Self::ExternalRejected => {
                "external image references are blocked for security; placeholder kept"
            }
            Self::UnsupportedEncoding => {
                "only base64-encoded data: images are decoded; placeholder kept"
            }
            Self::BadBase64 => "image data: payload was not valid base64; placeholder kept",
            Self::NotImage => {
                "embedded image is not a supported PNG or JPEG; placeholder kept"
            }
            Self::MalformedPng => "embedded PNG was malformed or truncated; placeholder kept",
            Self::UnsupportedPng => {
                "embedded PNG uses an unsupported feature (interlace, bit depth, or color type); placeholder kept"
            }
            Self::InflateFailed => "embedded PNG compressed data was invalid; placeholder kept",
            Self::MalformedJpeg => "embedded JPEG was malformed or truncated; placeholder kept",
            Self::UnsupportedJpeg => {
                "embedded JPEG uses an unsupported feature (arithmetic coding, lossless, or 12-bit precision); placeholder kept"
            }
            Self::TooLarge => "embedded image exceeds the renderer pixel budget; placeholder kept",
        }
    }
}

/// Decode an `<image>` href.  Only inline base64 `data:` PNG is rendered.
fn decode_image_href(href: &str) -> Result<DecodedImage, ImageDecodeError> {
    let href = href.trim();
    if href.is_empty() {
        return Err(ImageDecodeError::Empty);
    }
    let Some(rest) = href.strip_prefix("data:") else {
        let lower = href.to_ascii_lowercase();
        if lower.starts_with("http:")
            || lower.starts_with("https:")
            || lower.starts_with("file:")
            || lower.starts_with("//")
            || href.contains('/')
            || href.contains('\\')
        {
            return Err(ImageDecodeError::ExternalRejected);
        }
        return Err(ImageDecodeError::NotDataUri);
    };
    let Some((meta, payload)) = rest.split_once(',') else {
        return Err(ImageDecodeError::NotDataUri);
    };
    if !meta.to_ascii_lowercase().contains("base64") {
        return Err(ImageDecodeError::UnsupportedEncoding);
    }
    let bytes = base64_decode(payload).ok_or(ImageDecodeError::BadBase64)?;
    if bytes.is_empty() {
        return Err(ImageDecodeError::Empty);
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return decode_jpeg(&bytes);
    }
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err(ImageDecodeError::NotImage);
    }
    decode_png(&bytes)
}

/// Standard base64 decode, ignoring `=` padding and ASCII whitespace.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &c in input.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// PNG chunk parse → zlib inflate → scanline unfilter → straight RGBA8.
/// Supports non-interlaced bit depth 8 (color types 0/2/3/4/6) and bit depth 16
/// (0/2/4/6, truncated to 8).  Interlace and sub-byte depths are rejected with a
/// diagnostic rather than mis-decoded.
fn decode_png(bytes: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    let mut pos = 8usize; // past the signature
    let mut ihdr: Option<(usize, usize, u8, u8)> = None; // w, h, bitdepth, colortype
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();

    loop {
        if pos + 8 > bytes.len() {
            return Err(ImageDecodeError::MalformedPng);
        }
        let len = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start
            .checked_add(len)
            .ok_or(ImageDecodeError::MalformedPng)?;
        if data_end + 4 > bytes.len() {
            return Err(ImageDecodeError::MalformedPng);
        }
        let data = &bytes[data_start..data_end];
        match ctype {
            b"IHDR" => {
                if len != 13 {
                    return Err(ImageDecodeError::MalformedPng);
                }
                let w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
                let h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
                let bit_depth = data[8];
                let color_type = data[9];
                let compression = data[10];
                let filter = data[11];
                let interlace = data[12];
                if w == 0 || h == 0 {
                    return Err(ImageDecodeError::MalformedPng);
                }
                if compression != 0 || filter != 0 {
                    return Err(ImageDecodeError::MalformedPng);
                }
                if interlace != 0 {
                    return Err(ImageDecodeError::UnsupportedPng);
                }
                if w.checked_mul(h).is_none_or(|px| px > MAX_IMAGE_PIXELS) {
                    return Err(ImageDecodeError::TooLarge);
                }
                let supported = match color_type {
                    0 | 2 | 4 | 6 => bit_depth == 8 || bit_depth == 16,
                    3 => bit_depth == 8,
                    _ => false,
                };
                if !supported {
                    return Err(ImageDecodeError::UnsupportedPng);
                }
                ihdr = Some((w, h, bit_depth, color_type));
            }
            b"PLTE" => {
                for chunk in data.chunks_exact(3) {
                    palette.push([chunk[0], chunk[1], chunk[2]]);
                }
            }
            b"tRNS" => {
                trns = data.to_vec();
            }
            b"IDAT" => {
                if idat.len().saturating_add(data.len()) > MAX_IMAGE_DECODE_BYTES {
                    return Err(ImageDecodeError::TooLarge);
                }
                idat.extend_from_slice(data);
            }
            b"IEND" => break,
            _ => {}
        }
        pos = data_end + 4; // skip CRC
    }

    let (width, height, bit_depth, color_type) = ihdr.ok_or(ImageDecodeError::MalformedPng)?;
    let channels: usize = match color_type {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Err(ImageDecodeError::UnsupportedPng),
    };
    let sample_bytes = (bit_depth / 8) as usize; // 1 or 2
    let bpp = channels * sample_bytes;
    let stride = width.checked_mul(bpp).ok_or(ImageDecodeError::TooLarge)?;
    let expected = height
        .checked_mul(stride + 1)
        .filter(|n| *n <= MAX_IMAGE_DECODE_BYTES)
        .ok_or(ImageDecodeError::TooLarge)?;

    let raw = zlib_inflate(&idat, expected).ok_or(ImageDecodeError::InflateFailed)?;
    if raw.len() < expected {
        return Err(ImageDecodeError::MalformedPng);
    }

    // Unfilter scanlines in place into `lines` (stride bytes per row, no filter byte).
    let mut lines = vec![0u8; height * stride];
    for row in 0..height {
        let src = &raw[row * (stride + 1)..row * (stride + 1) + stride + 1];
        let filter = src[0];
        let src = &src[1..];
        let (prev_row, cur_row) = lines.split_at_mut(row * stride);
        let prev = if row > 0 {
            &prev_row[(row - 1) * stride..row * stride]
        } else {
            &[][..]
        };
        let cur = &mut cur_row[..stride];
        for i in 0..stride {
            let a = if i >= bpp { cur[i - bpp] as i32 } else { 0 };
            let b = if row > 0 { prev[i] as i32 } else { 0 };
            let c = if row > 0 && i >= bpp {
                prev[i - bpp] as i32
            } else {
                0
            };
            let x = src[i] as i32;
            let value = match filter {
                0 => x,
                1 => x + a,
                2 => x + b,
                3 => x + (a + b) / 2,
                4 => x + paeth_predictor(a, b, c),
                _ => return Err(ImageDecodeError::MalformedPng),
            };
            cur[i] = (value & 0xff) as u8;
        }
    }

    // Expand to straight RGBA8.
    let mut rgba = vec![0u8; width * height * 4];
    let gray_key = single_color_trns(color_type, bit_depth, &trns);
    for row in 0..height {
        let line = &lines[row * stride..(row + 1) * stride];
        for col in 0..width {
            let sample = |channel: usize| -> u8 {
                let base = (col * channels + channel) * sample_bytes;
                line[base] // high byte for 16-bit, only byte for 8-bit
            };
            let raw16 = |channel: usize| -> u16 {
                let base = (col * channels + channel) * sample_bytes;
                if sample_bytes == 2 {
                    u16::from_be_bytes([line[base], line[base + 1]])
                } else {
                    line[base] as u16
                }
            };
            let out = &mut rgba[(row * width + col) * 4..(row * width + col) * 4 + 4];
            match color_type {
                0 => {
                    let g = sample(0);
                    let transparent = gray_key == Some([raw16(0), raw16(0), raw16(0)]);
                    out.copy_from_slice(&[g, g, g, if transparent { 0 } else { 255 }]);
                }
                2 => {
                    let (r, g, b) = (sample(0), sample(1), sample(2));
                    let transparent = gray_key == Some([raw16(0), raw16(1), raw16(2)]);
                    out.copy_from_slice(&[r, g, b, if transparent { 0 } else { 255 }]);
                }
                3 => {
                    let idx = line[col] as usize;
                    let color = palette.get(idx).copied().unwrap_or([0, 0, 0]);
                    let alpha = trns.get(idx).copied().unwrap_or(255);
                    out.copy_from_slice(&[color[0], color[1], color[2], alpha]);
                }
                4 => {
                    let g = sample(0);
                    let a = sample(1);
                    out.copy_from_slice(&[g, g, g, a]);
                }
                6 => {
                    out.copy_from_slice(&[sample(0), sample(1), sample(2), sample(3)]);
                }
                _ => return Err(ImageDecodeError::UnsupportedPng),
            }
        }
    }

    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

/// The single transparent colour from a `tRNS` chunk for grayscale (type 0) and
/// truecolor (type 2) images, expressed as 16-bit samples for exact comparison.
fn single_color_trns(color_type: u8, bit_depth: u8, trns: &[u8]) -> Option<[u16; 3]> {
    let read = |i: usize| -> u16 {
        let hi = *trns.get(i * 2).unwrap_or(&0) as u16;
        let lo = *trns.get(i * 2 + 1).unwrap_or(&0) as u16;
        let v = (hi << 8) | lo;
        if bit_depth == 16 {
            v
        } else {
            v & 0xff
        }
    };
    match color_type {
        0 if trns.len() >= 2 => {
            let g = read(0);
            Some([g, g, g])
        }
        2 if trns.len() >= 6 => Some([read(0), read(1), read(2)]),
        _ => None,
    }
}

fn paeth_predictor(a: i32, b: i32, c: i32) -> i32 {
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// zlib wrapper: skip the 2-byte header, reject preset dictionaries, inflate the
/// DEFLATE body, and ignore the trailing adler32.
fn zlib_inflate(data: &[u8], max_out: usize) -> Option<Vec<u8>> {
    if data.len() < 2 {
        return None;
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0f != 8 {
        return None; // not DEFLATE
    }
    if flg & 0x20 != 0 {
        return None; // preset dictionary unsupported
    }
    inflate(&data[2..], max_out)
}

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn bit(&mut self) -> Option<u32> {
        let byte = *self.data.get(self.byte_pos)?;
        let value = (byte >> self.bit_pos) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(value as u32)
    }

    fn bits(&mut self, count: u32) -> Option<u32> {
        let mut value = 0u32;
        for i in 0..count {
            value |= self.bit()? << i;
        }
        Some(value)
    }

    fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    fn read_byte(&mut self) -> Option<u8> {
        let byte = *self.data.get(self.byte_pos)?;
        self.byte_pos += 1;
        Some(byte)
    }
}

/// Canonical Huffman decoder (puff-style: counts per length + sorted symbols).
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Option<Self> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            if len as usize > 15 {
                return None;
            }
            counts[len as usize] += 1;
        }
        counts[0] = 0;
        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        let mut next = offsets;
        for (symbol, &len) in lengths.iter().enumerate() {
            if len != 0 {
                let slot = next[len as usize] as usize;
                if slot >= symbols.len() {
                    return None;
                }
                symbols[slot] = symbol as u16;
                next[len as usize] += 1;
            }
        }
        Some(Self { counts, symbols })
    }

    fn decode(&self, br: &mut BitReader<'_>) -> Option<u16> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for len in 1..=15usize {
            code |= br.bit()? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return self.symbols.get((index + (code - first)) as usize).copied();
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        None
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Inflate a raw DEFLATE stream, bounded by `max_out` output bytes.
fn inflate(data: &[u8], max_out: usize) -> Option<Vec<u8>> {
    let mut br = BitReader::new(data);
    let mut out: Vec<u8> = Vec::new();
    loop {
        let bfinal = br.bit()?;
        let btype = br.bits(2)?;
        match btype {
            0 => {
                br.align_byte();
                let len = br.read_byte()? as usize | ((br.read_byte()? as usize) << 8);
                let nlen = br.read_byte()? as usize | ((br.read_byte()? as usize) << 8);
                if len != (!nlen & 0xffff) {
                    return None;
                }
                if out.len() + len > max_out {
                    return None;
                }
                for _ in 0..len {
                    out.push(br.read_byte()?);
                }
            }
            1 => {
                let (lit, dist) = fixed_huffman();
                inflate_block(&mut br, &mut out, &lit, &dist, max_out)?;
            }
            2 => {
                let (lit, dist) = dynamic_huffman(&mut br)?;
                inflate_block(&mut br, &mut out, &lit, &dist, max_out)?;
            }
            _ => return None,
        }
        if bfinal == 1 {
            break;
        }
    }
    Some(out)
}

fn inflate_block(
    br: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    max_out: usize,
) -> Option<()> {
    loop {
        let symbol = lit.decode(br)?;
        match symbol {
            0..=255 => {
                if out.len() >= max_out {
                    return None;
                }
                out.push(symbol as u8);
            }
            256 => return Some(()),
            257..=285 => {
                let s = (symbol - 257) as usize;
                let length = LEN_BASE[s] as usize + br.bits(LEN_EXTRA[s] as u32)? as usize;
                let dsym = dist.decode(br)? as usize;
                if dsym >= DIST_BASE.len() {
                    return None;
                }
                let distance =
                    DIST_BASE[dsym] as usize + br.bits(DIST_EXTRA[dsym] as u32)? as usize;
                if distance == 0 || distance > out.len() || out.len() + length > max_out {
                    return None;
                }
                let start = out.len() - distance;
                for i in 0..length {
                    let byte = out[start + i];
                    out.push(byte);
                }
            }
            _ => return None,
        }
    }
}

fn fixed_huffman() -> (Huffman, Huffman) {
    let mut lit_lengths = [0u8; 288];
    for (i, slot) in lit_lengths.iter_mut().enumerate() {
        *slot = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    let dist_lengths = [5u8; 30];
    (
        Huffman::new(&lit_lengths).expect("valid fixed literal table"),
        Huffman::new(&dist_lengths).expect("valid fixed distance table"),
    )
}

fn dynamic_huffman(br: &mut BitReader<'_>) -> Option<(Huffman, Huffman)> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let hlit = br.bits(5)? as usize + 257;
    let hdist = br.bits(5)? as usize + 1;
    let hclen = br.bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return None;
    }
    let mut cl_lengths = [0u8; 19];
    for &slot in ORDER.iter().take(hclen) {
        cl_lengths[slot] = br.bits(3)? as u8;
    }
    let cl = Huffman::new(&cl_lengths)?;
    let mut lengths: Vec<u8> = Vec::with_capacity(hlit + hdist);
    while lengths.len() < hlit + hdist {
        match cl.decode(br)? {
            len @ 0..=15 => lengths.push(len as u8),
            16 => {
                let prev = *lengths.last()?;
                for _ in 0..(br.bits(2)? + 3) {
                    lengths.push(prev);
                }
            }
            17 => {
                let count = (br.bits(3)? + 3) as usize;
                lengths.resize(lengths.len() + count, 0);
            }
            18 => {
                let count = (br.bits(7)? + 11) as usize;
                lengths.resize(lengths.len() + count, 0);
            }
            _ => return None,
        }
    }
    if lengths.len() != hlit + hdist {
        return None;
    }
    let lit = Huffman::new(&lengths[..hlit])?;
    let dist = Huffman::new(&lengths[hlit..])?;
    Some((lit, dist))
}

/// Device-space half-open bounding box of a destination polygon, clamped to the
/// raster size.
fn device_rect_bounds(corners: &[(f32, f32)], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for &(x, y) in corners {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let x0 = (min_x.floor().max(0.0) as usize).min(w);
    let y0 = (min_y.floor().max(0.0) as usize).min(h);
    let x1 = (max_x.ceil().max(0.0) as usize).min(w);
    let y1 = (max_y.ceil().max(0.0) as usize).min(h);
    (x0, y0, x1, y1)
}

/// Nearest-neighbour sample a decoded image into the target, honouring the
/// active clip and element opacity.  Deterministic across platforms.
fn draw_image_samples(
    target: &mut RasterTarget<'_>,
    image: &DecodedImage,
    device_to_image: Transform,
    opacity: f32,
    bounds: (usize, usize, usize, usize),
) {
    let (x0, y0, x1, y1) = bounds;
    let alpha_scale = opacity.clamp(0.0, 1.0);
    for py in y0..y1 {
        for px in x0..x1 {
            let (fx, fy) = device_to_image.apply(px as f64 + 0.5, py as f64 + 0.5);
            if fx < 0.0 || fy < 0.0 {
                continue;
            }
            let (ix, iy) = (fx as usize, fy as usize);
            if ix >= image.width || iy >= image.height {
                continue;
            }
            let base = (iy * image.width + ix) * 4;
            let mut src = [
                image.rgba[base],
                image.rgba[base + 1],
                image.rgba[base + 2],
                image.rgba[base + 3],
            ];
            if alpha_scale < 1.0 {
                src[3] = (src[3] as f32 * alpha_scale).round().clamp(0.0, 255.0) as u8;
            }
            target.composite(px, py, src);
        }
    }
}

// ---------------------------------------------------------------------------
// R5 follow-on: zero-dependency baseline JPEG decode
// ---------------------------------------------------------------------------
//
// Supports baseline / extended-sequential Huffman JPEG (SOF0/SOF1), 8-bit, 1 or
// 3 components (grayscale or YCbCr), arbitrary integer chroma subsampling
// (4:4:4 / 4:2:2 / 4:2:0 …) with restart markers and byte-stuffing.  Progressive,
// arithmetic, lossless, 12-bit, and CMYK/4-component are diagnosed as unsupported
// rather than mis-decoded.  See `docs/jpegdecoder roadmap.md`.

const JPEG_ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Canonical JPEG Huffman table (spec Annex F decode procedure).
#[derive(Clone, Default)]
struct JpegHuffTable {
    mincode: [i32; 17],
    maxcode: [i32; 17],
    valptr: [usize; 17],
    values: Vec<u8>,
}

impl JpegHuffTable {
    fn build(counts: &[u8; 16], values: Vec<u8>) -> Self {
        let mut sizes: Vec<u8> = Vec::new();
        for (l, &count) in counts.iter().enumerate() {
            for _ in 0..count {
                sizes.push((l + 1) as u8);
            }
        }
        let mut codes = vec![0u16; sizes.len()];
        let mut code = 0u16;
        let mut k = 0;
        let mut si = sizes.first().copied().unwrap_or(0);
        while k < sizes.len() {
            while k < sizes.len() && sizes[k] == si {
                codes[k] = code;
                code = code.wrapping_add(1);
                k += 1;
            }
            code <<= 1;
            si += 1;
        }
        let mut table = Self {
            values,
            ..Self::default()
        };
        let mut p = 0usize;
        for l in 1..=16usize {
            if counts[l - 1] == 0 {
                table.maxcode[l] = -1;
            } else {
                table.valptr[l] = p;
                table.mincode[l] = codes[p] as i32;
                p += counts[l - 1] as usize;
                table.maxcode[l] = codes[p - 1] as i32;
            }
        }
        table
    }

    fn decode(&self, br: &mut JpegBits<'_>) -> u8 {
        let mut code = 0i32;
        for l in 1..=16usize {
            code = (code << 1) | br.bit() as i32;
            if self.maxcode[l] >= 0 && code <= self.maxcode[l] {
                let idx = self.valptr[l] + (code - self.mincode[l]) as usize;
                return self.values.get(idx).copied().unwrap_or(0);
            }
        }
        0
    }
}

/// MSB-first entropy bit reader with `0xFF00` de-stuffing; stops at any marker.
struct JpegBits<'a> {
    data: &'a [u8],
    pos: usize,
    cur: u32,
    bit_count: u32,
    eod: bool,
}

impl<'a> JpegBits<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self {
            data,
            pos,
            cur: 0,
            bit_count: 0,
            eod: false,
        }
    }

    fn bit(&mut self) -> u8 {
        if self.bit_count == 0 {
            if self.eod || self.pos >= self.data.len() {
                self.eod = true;
                return 0;
            }
            let b = self.data[self.pos];
            if b == 0xFF {
                let next = self.data.get(self.pos + 1).copied().unwrap_or(0xD9);
                if next == 0x00 {
                    self.pos += 2;
                    self.cur = 0xFF;
                } else {
                    // Marker reached: stop feeding bits, leave pos at the 0xFF.
                    self.eod = true;
                    return 0;
                }
            } else {
                self.pos += 1;
                self.cur = b as u32;
            }
            self.bit_count = 8;
        }
        self.bit_count -= 1;
        ((self.cur >> self.bit_count) & 1) as u8
    }

    fn receive(&mut self, n: u32) -> i32 {
        let mut v = 0i32;
        for _ in 0..n {
            v = (v << 1) | self.bit() as i32;
        }
        v
    }

    /// Byte-align and consume the next RSTn marker, tolerating fill bytes.
    fn restart(&mut self) {
        self.bit_count = 0;
        self.eod = false;
        while self.pos + 1 < self.data.len() {
            if self.data[self.pos] == 0xFF {
                let m = self.data[self.pos + 1];
                if (0xD0..=0xD7).contains(&m) {
                    self.pos += 2;
                    return;
                }
                if m == 0x00 {
                    self.pos += 2;
                    continue;
                }
                return;
            }
            self.pos += 1;
        }
    }
}

/// JPEG signed-magnitude extension (spec Figure F.12).
fn jpeg_extend(v: i32, n: u32) -> i32 {
    if n == 0 {
        0
    } else if v < (1 << (n - 1)) {
        v + (-1i32 << n) + 1
    } else {
        v
    }
}

#[derive(Clone)]
struct JpegComponent {
    id: u8,
    h: usize,
    v: usize,
    quant: usize,
    dc_table: usize,
    ac_table: usize,
    pred: i32,
}

struct SosCompEntry {
    comp_idx: usize,
    dc_table: usize,
    ac_table: usize,
}

struct SosScanParams {
    entries: Vec<SosCompEntry>,
    ss: u8,
    se: u8,
    ah: u8,
    al: u8,
}

struct ProgCoeff {
    /// Quantized coefficients in zigzag order: [component][block][zigzag_pos].
    coeff: Vec<Vec<[i32; 64]>>,
    blocks_x: Vec<usize>,
    blocks_y: Vec<usize>,
    max_h: usize,
    max_v: usize,
    mcus_x: usize,
    mcus_y: usize,
    dc_pred: Vec<i32>,
    eob_run: usize,
}

impl ProgCoeff {
    fn new(
        components: &[JpegComponent],
        max_h: usize,
        max_v: usize,
        mcus_x: usize,
        mcus_y: usize,
    ) -> Self {
        let nc = components.len();
        let mut coeff = Vec::with_capacity(nc);
        let mut blocks_x = Vec::with_capacity(nc);
        let mut blocks_y = Vec::with_capacity(nc);
        for c in components {
            let bx = mcus_x * c.h;
            let by = mcus_y * c.v;
            coeff.push(vec![[0i32; 64]; bx * by]);
            blocks_x.push(bx);
            blocks_y.push(by);
        }
        Self {
            coeff,
            blocks_x,
            blocks_y,
            max_h,
            max_v,
            mcus_x,
            mcus_y,
            dc_pred: vec![0i32; nc],
            eob_run: 0,
        }
    }
}

fn decode_jpeg(bytes: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    let mut pos = 2; // past SOI (FF D8)
    let mut qtables: [[u16; 64]; 4] = [[0; 64]; 4];
    let mut dc_tables: [Option<JpegHuffTable>; 4] = Default::default();
    let mut ac_tables: [Option<JpegHuffTable>; 4] = Default::default();
    let mut width = 0usize;
    let mut height = 0usize;
    let mut components: Vec<JpegComponent> = Vec::new();
    let mut restart_interval = 0usize;
    let mut is_progressive = false;
    // Adobe APP14 ColorTransform=1 means YCCK for 4-component images.
    let mut app14_ycck = false;
    let mut prog_coeff: Option<ProgCoeff> = None;

    loop {
        if pos + 1 >= bytes.len() || bytes[pos] != 0xFF {
            if is_progressive {
                break; // truncated progressive JPEG — produce best-effort output
            }
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let marker = bytes[pos + 1];
        pos += 2;
        match marker {
            0xD9 => {
                if is_progressive {
                    break; // EOI — finalize
                }
                return Err(ImageDecodeError::MalformedJpeg); // EOI before SOS
            }
            0x01 | 0xD0..=0xD7 => continue, // standalone markers
            _ => {}
        }
        if pos + 2 > bytes.len() {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let seg_len = ((bytes[pos] as usize) << 8 | bytes[pos + 1] as usize)
            .checked_sub(2)
            .ok_or(ImageDecodeError::MalformedJpeg)?;
        let seg_start = pos + 2;
        let seg_end = seg_start
            .checked_add(seg_len)
            .filter(|end| *end <= bytes.len())
            .ok_or(ImageDecodeError::MalformedJpeg)?;
        let seg = &bytes[seg_start..seg_end];
        match marker {
            0xDB => parse_dqt(seg, &mut qtables)?,
            0xC4 => parse_dht(seg, &mut dc_tables, &mut ac_tables)?,
            0xDD if seg.len() >= 2 => {
                restart_interval = (seg[0] as usize) << 8 | seg[1] as usize;
            }
            0xC0 | 0xC1 => {
                let (w, h, comps) = parse_sof(seg)?;
                width = w;
                height = h;
                components = comps;
                is_progressive = false;
            }
            0xC2 => {
                // SOF2: progressive DCT
                let (w, h, comps) = parse_sof(seg)?;
                width = w;
                height = h;
                is_progressive = true;
                let max_h = comps.iter().map(|c| c.h).max().unwrap_or(1);
                let max_v = comps.iter().map(|c| c.v).max().unwrap_or(1);
                let mcus_x = width.div_ceil(max_h * 8);
                let mcus_y = height.div_ceil(max_v * 8);
                prog_coeff = Some(ProgCoeff::new(&comps, max_h, max_v, mcus_x, mcus_y));
                components = comps;
            }
            0xC3 | 0xC5..=0xCB | 0xCD..=0xCF => return Err(ImageDecodeError::UnsupportedJpeg),
            // Adobe APP14: detect YCCK color transform for 4-component images.
            0xEE if seg.len() >= 12 && &seg[..5] == b"Adobe" && seg[11] == 1 => {
                app14_ycck = true;
            }
            0xDA => {
                if !is_progressive {
                    parse_sos(seg, &mut components, &dc_tables, &ac_tables)?;
                    return decode_jpeg_scan(
                        bytes,
                        seg_end,
                        width,
                        height,
                        &components,
                        &qtables,
                        &dc_tables,
                        &ac_tables,
                        restart_interval,
                        app14_ycck,
                    );
                }
                // Progressive scan: accumulate into coefficient arrays.
                let params = parse_sos_params(seg, &components)?;
                let prog = prog_coeff.as_mut().ok_or(ImageDecodeError::MalformedJpeg)?;
                // Reset DC predictors at the start of each new DC first-pass scan.
                if params.ah == 0 && params.ss == 0 {
                    for dc in prog.dc_pred.iter_mut() {
                        *dc = 0;
                    }
                }
                prog.eob_run = 0;
                let after = decode_progressive_scan(
                    bytes,
                    seg_end,
                    &components,
                    &params,
                    prog,
                    &dc_tables,
                    &ac_tables,
                    restart_interval,
                )?;
                pos = after;
                continue; // skip pos = seg_end at bottom
            }
            _ => {} // APPn / COM / other: skip
        }
        pos = seg_end;
    }

    // Reached only in progressive mode (baseline always returns from within the loop).
    let prog = prog_coeff.ok_or(ImageDecodeError::MalformedJpeg)?;
    decode_progressive_finish(prog, width, height, &components, &qtables, app14_ycck)
}

fn parse_dqt(seg: &[u8], qtables: &mut [[u16; 64]; 4]) -> Result<(), ImageDecodeError> {
    let mut i = 0;
    while i < seg.len() {
        let pq = seg[i] >> 4;
        let tq = (seg[i] & 0x0f) as usize;
        i += 1;
        if tq > 3 {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        match pq {
            0 => {
                if i + 64 > seg.len() {
                    return Err(ImageDecodeError::MalformedJpeg);
                }
                for (k, slot) in qtables[tq].iter_mut().enumerate() {
                    *slot = seg[i + k] as u16;
                }
                i += 64;
            }
            1 => {
                if i + 128 > seg.len() {
                    return Err(ImageDecodeError::MalformedJpeg);
                }
                for (k, slot) in qtables[tq].iter_mut().enumerate() {
                    *slot = (seg[i + 2 * k] as u16) << 8 | seg[i + 2 * k + 1] as u16;
                }
                i += 128;
            }
            _ => return Err(ImageDecodeError::MalformedJpeg),
        }
    }
    Ok(())
}

fn parse_dht(
    seg: &[u8],
    dc: &mut [Option<JpegHuffTable>; 4],
    ac: &mut [Option<JpegHuffTable>; 4],
) -> Result<(), ImageDecodeError> {
    let mut i = 0;
    while i < seg.len() {
        if i + 17 > seg.len() {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let tc = seg[i] >> 4;
        let th = (seg[i] & 0x0f) as usize;
        i += 1;
        if th > 3 {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let mut counts = [0u8; 16];
        counts.copy_from_slice(&seg[i..i + 16]);
        i += 16;
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        if i + total > seg.len() {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let table = JpegHuffTable::build(&counts, seg[i..i + total].to_vec());
        i += total;
        match tc {
            0 => dc[th] = Some(table),
            1 => ac[th] = Some(table),
            _ => return Err(ImageDecodeError::MalformedJpeg),
        }
    }
    Ok(())
}

fn parse_sof(seg: &[u8]) -> Result<(usize, usize, Vec<JpegComponent>), ImageDecodeError> {
    if seg.len() < 6 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    if seg[0] != 8 {
        return Err(ImageDecodeError::UnsupportedJpeg); // only 8-bit precision
    }
    let h = (seg[1] as usize) << 8 | seg[2] as usize;
    let w = (seg[3] as usize) << 8 | seg[4] as usize;
    let nc = seg[5] as usize;
    if w == 0 || h == 0 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    if w.checked_mul(h).is_none_or(|px| px > MAX_IMAGE_PIXELS) {
        return Err(ImageDecodeError::TooLarge);
    }
    if nc == 0 || nc > 4 {
        return Err(ImageDecodeError::UnsupportedJpeg);
    }
    if seg.len() < 6 + nc * 3 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    let mut comps = Vec::with_capacity(nc);
    for i in 0..nc {
        let o = 6 + i * 3;
        let sampling = seg[o + 1];
        let hh = (sampling >> 4) as usize;
        let vv = (sampling & 0x0f) as usize;
        let quant = (seg[o + 2]) as usize;
        if hh == 0 || vv == 0 || hh > 4 || vv > 4 || quant > 3 {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        comps.push(JpegComponent {
            id: seg[o],
            h: hh,
            v: vv,
            quant,
            dc_table: 0,
            ac_table: 0,
            pred: 0,
        });
    }
    Ok((w, h, comps))
}

fn parse_sos(
    seg: &[u8],
    components: &mut [JpegComponent],
    dc: &[Option<JpegHuffTable>; 4],
    ac: &[Option<JpegHuffTable>; 4],
) -> Result<(), ImageDecodeError> {
    if seg.is_empty() {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    let ns = seg[0] as usize;
    if seg.len() < 1 + ns * 2 + 3 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    if ns != components.len() {
        // Non-interleaved / multi-scan (progressive-style) not supported.
        return Err(ImageDecodeError::UnsupportedJpeg);
    }
    for i in 0..ns {
        let cs = seg[1 + i * 2];
        let td_ta = seg[1 + i * 2 + 1];
        let dct = (td_ta >> 4) as usize;
        let act = (td_ta & 0x0f) as usize;
        if dct > 3 || act > 3 {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let comp = components
            .iter_mut()
            .find(|c| c.id == cs)
            .ok_or(ImageDecodeError::MalformedJpeg)?;
        comp.dc_table = dct;
        comp.ac_table = act;
    }
    for c in components.iter() {
        if dc[c.dc_table].is_none() || ac[c.ac_table].is_none() {
            return Err(ImageDecodeError::MalformedJpeg);
        }
    }
    Ok(())
}

fn parse_sos_params(
    seg: &[u8],
    components: &[JpegComponent],
) -> Result<SosScanParams, ImageDecodeError> {
    if seg.is_empty() {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    let ns = seg[0] as usize;
    if ns == 0 || seg.len() < 1 + ns * 2 + 3 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    let mut entries = Vec::with_capacity(ns);
    for i in 0..ns {
        let cs = seg[1 + i * 2];
        let td_ta = seg[1 + i * 2 + 1];
        let dc_table = (td_ta >> 4) as usize;
        let ac_table = (td_ta & 0x0f) as usize;
        if dc_table > 3 || ac_table > 3 {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let comp_idx = components
            .iter()
            .position(|c| c.id == cs)
            .ok_or(ImageDecodeError::MalformedJpeg)?;
        entries.push(SosCompEntry {
            comp_idx,
            dc_table,
            ac_table,
        });
    }
    let offset = 1 + ns * 2;
    let ss = seg[offset];
    let se = seg[offset + 1];
    let ah_al = seg[offset + 2];
    let ah = ah_al >> 4;
    let al = ah_al & 0x0f;
    if ss > se || se > 63 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    // AC scans must be non-interleaved.
    if ss > 0 && ns > 1 {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    Ok(SosScanParams {
        entries,
        ss,
        se,
        ah,
        al,
    })
}

#[allow(clippy::too_many_arguments)]
fn decode_progressive_scan(
    bytes: &[u8],
    entropy_start: usize,
    components: &[JpegComponent],
    params: &SosScanParams,
    prog: &mut ProgCoeff,
    dc_tables: &[Option<JpegHuffTable>; 4],
    ac_tables: &[Option<JpegHuffTable>; 4],
    restart_interval: usize,
) -> Result<usize, ImageDecodeError> {
    let mut br = JpegBits::new(bytes, entropy_start);
    let ss = params.ss as usize;
    let se = params.se as usize;
    let ah = params.ah;
    let al = params.al;

    if ss == 0 {
        // DC scan (interleaved or non-interleaved).
        let mut mcu_index = 0usize;
        for my in 0..prog.mcus_y {
            for mx in 0..prog.mcus_x {
                if restart_interval > 0
                    && mcu_index > 0
                    && mcu_index.is_multiple_of(restart_interval)
                {
                    br.restart();
                    for dc in prog.dc_pred.iter_mut() {
                        *dc = 0;
                    }
                }
                for entry in &params.entries {
                    let ci = entry.comp_idx;
                    let comp = &components[ci];
                    let dc_table = dc_tables[entry.dc_table]
                        .as_ref()
                        .ok_or(ImageDecodeError::MalformedJpeg)?;
                    for by in 0..comp.v {
                        for bx in 0..comp.h {
                            let block_x = mx * comp.h + bx;
                            let block_y = my * comp.v + by;
                            if block_x >= prog.blocks_x[ci] || block_y >= prog.blocks_y[ci] {
                                continue;
                            }
                            let bidx = block_y * prog.blocks_x[ci] + block_x;
                            if ah == 0 {
                                // DC first pass: receive and accumulate.
                                let t = dc_table.decode(&mut br) as u32;
                                let diff = jpeg_extend(br.receive(t), t);
                                prog.dc_pred[ci] += diff;
                                prog.coeff[ci][bidx][0] = prog.dc_pred[ci] << al;
                            } else {
                                // DC refinement: add one correction bit.
                                let bit = br.receive(1);
                                prog.coeff[ci][bidx][0] |= bit << al;
                            }
                        }
                    }
                }
                mcu_index += 1;
            }
        }
    } else {
        // AC scan: always non-interleaved.
        if params.entries.len() != 1 {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let entry = &params.entries[0];
        let ci = entry.comp_idx;
        let ac_table = ac_tables[entry.ac_table]
            .as_ref()
            .ok_or(ImageDecodeError::MalformedJpeg)?;
        let total_bx = prog.blocks_x[ci];
        let total_by = prog.blocks_y[ci];
        let mut block_count = 0usize;
        for by in 0..total_by {
            for bx in 0..total_bx {
                if restart_interval > 0
                    && block_count > 0
                    && block_count.is_multiple_of(restart_interval)
                {
                    br.restart();
                    prog.eob_run = 0;
                }
                let bidx = by * total_bx + bx;
                if ah == 0 {
                    // AC first pass.
                    if prog.eob_run > 0 {
                        prog.eob_run -= 1;
                    } else {
                        let mut k = ss;
                        while k <= se {
                            let rs = ac_table.decode(&mut br);
                            let r = (rs >> 4) as usize;
                            let s = (rs & 0x0f) as u32;
                            if s == 0 {
                                if r == 15 {
                                    k += 16; // ZRL: skip 16 zeros
                                } else {
                                    prog.eob_run = if r > 0 {
                                        ((1usize << r) + br.receive(r as u32) as usize)
                                            .saturating_sub(1)
                                    } else {
                                        0
                                    };
                                    break;
                                }
                            } else {
                                k += r;
                                if k > se {
                                    break;
                                }
                                let coeff = jpeg_extend(br.receive(s), s);
                                prog.coeff[ci][bidx][k] = coeff << al;
                                k += 1;
                            }
                        }
                    }
                } else {
                    // AC refinement.
                    decode_ac_refinement(
                        &mut br,
                        &mut prog.coeff[ci][bidx],
                        ac_table,
                        ss,
                        se,
                        al,
                        &mut prog.eob_run,
                    );
                }
                block_count += 1;
            }
        }
    }

    Ok(br.pos)
}

fn decode_ac_refinement(
    br: &mut JpegBits<'_>,
    block: &mut [i32; 64],
    ac_table: &JpegHuffTable,
    ss: usize,
    se: usize,
    al: u8,
    eob_run: &mut usize,
) {
    let delta = 1i32 << al;

    if *eob_run > 0 {
        // Refine non-zero coefficients in this block and decrement the run.
        for coeff in &mut block[ss..=se] {
            if *coeff != 0 {
                let bit = br.receive(1);
                if bit != 0 {
                    if *coeff > 0 {
                        *coeff += delta;
                    } else {
                        *coeff -= delta;
                    }
                }
            }
        }
        *eob_run -= 1;
        return;
    }

    let mut k = ss;
    while k <= se {
        let rs = ac_table.decode(br);
        let r = (rs >> 4) as usize;
        let s = rs & 0x0f;
        if s == 0 {
            if r == 15 {
                // ZRL: advance 16 zero slots, refining non-zeros along the way.
                let mut zeros = 16usize;
                while zeros > 0 && k <= se {
                    if block[k] != 0 {
                        let bit = br.receive(1);
                        if bit != 0 {
                            if block[k] > 0 {
                                block[k] += delta;
                            } else {
                                block[k] -= delta;
                            }
                        }
                    } else {
                        zeros -= 1;
                    }
                    k += 1;
                }
            } else {
                // EOBrun: refine rest of band, set counter for subsequent blocks.
                *eob_run = if r > 0 {
                    ((1usize << r) + br.receive(r as u32) as usize).saturating_sub(1)
                } else {
                    0
                };
                while k <= se {
                    if block[k] != 0 {
                        let bit = br.receive(1);
                        if bit != 0 {
                            if block[k] > 0 {
                                block[k] += delta;
                            } else {
                                block[k] -= delta;
                            }
                        }
                    }
                    k += 1;
                }
                return;
            }
        } else {
            // s == 1: new significant coefficient; r zeros to skip first.
            let sign_bit = br.receive(1);
            let new_coeff = if sign_bit != 0 { delta } else { -delta };
            let mut zeros = r;
            while zeros > 0 && k <= se {
                if block[k] != 0 {
                    let bit = br.receive(1);
                    if bit != 0 {
                        if block[k] > 0 {
                            block[k] += delta;
                        } else {
                            block[k] -= delta;
                        }
                    }
                } else {
                    zeros -= 1;
                }
                k += 1;
            }
            if k <= se {
                block[k] = new_coeff;
                k += 1;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_progressive_finish(
    prog: ProgCoeff,
    width: usize,
    height: usize,
    components: &[JpegComponent],
    qtables: &[[u16; 64]; 4],
    app14_ycck: bool,
) -> Result<DecodedImage, ImageDecodeError> {
    if width == 0 || height == 0 || components.is_empty() {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    // Precompute the 8-point IDCT cosine basis (same as decode_jpeg_scan).
    let mut cos_t = [[0f32; 8]; 8];
    for (u, row) in cos_t.iter_mut().enumerate() {
        let cu = if u == 0 { 1.0 / 2f32.sqrt() } else { 1.0 };
        for (x, slot) in row.iter_mut().enumerate() {
            *slot = cu * ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }

    let nc = components.len();
    let mut planes: Vec<JpegPlane> = Vec::with_capacity(nc);
    for (ci, comp) in components.iter().enumerate() {
        let bx = prog.blocks_x[ci];
        let by = prog.blocks_y[ci];
        let pw = bx * 8;
        let ph = by * 8;
        let mut plane_data = vec![0u8; pw * ph];
        for block_y in 0..by {
            for block_x in 0..bx {
                let bidx = block_y * bx + block_x;
                let coeffs = &prog.coeff[ci][bidx];
                let qt = &qtables[comp.quant];
                // Dequantize: coefficients are stored in zigzag order.
                let mut dequant = [0f32; 64];
                for (k, &c) in coeffs.iter().enumerate() {
                    dequant[JPEG_ZIGZAG[k]] = c as f32 * qt[k] as f32;
                }
                let mut spatial = [0f32; 64];
                idct_8x8(&dequant, &cos_t, &mut spatial);
                let px0 = block_x * 8;
                let py0 = block_y * 8;
                for yy in 0..8 {
                    for xx in 0..8 {
                        let val = (spatial[yy * 8 + xx] + 128.0).round().clamp(0.0, 255.0) as u8;
                        plane_data[(py0 + yy) * pw + (px0 + xx)] = val;
                    }
                }
            }
        }
        planes.push(JpegPlane {
            width: pw,
            data: plane_data,
        });
    }

    let max_h = prog.max_h;
    let max_v = prog.max_v;
    let sample = |plane: &JpegPlane, comp: &JpegComponent, x: usize, y: usize| -> i32 {
        let cx = (x * comp.h / max_h).min(plane.width.saturating_sub(1));
        let ph = plane.data.len().checked_div(plane.width).unwrap_or(0);
        let cy = (y * comp.v / max_v).min(ph.saturating_sub(1));
        plane.data[cy * plane.width + cx] as i32
    };

    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let out = &mut rgba[(y * width + x) * 4..(y * width + x) * 4 + 4];
            match nc {
                1 => {
                    let g = sample(&planes[0], &components[0], x, y) as u8;
                    out.copy_from_slice(&[g, g, g, 255]);
                }
                3 => {
                    let yv = sample(&planes[0], &components[0], x, y);
                    let cb = sample(&planes[1], &components[1], x, y);
                    let cr = sample(&planes[2], &components[2], x, y);
                    let rgb = jpeg_ycbcr_to_rgb(yv, cb, cr);
                    out.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
                4 => {
                    let c0 = sample(&planes[0], &components[0], x, y);
                    let c1 = sample(&planes[1], &components[1], x, y);
                    let c2 = sample(&planes[2], &components[2], x, y);
                    let c3 = sample(&planes[3], &components[3], x, y);
                    let rgb = jpeg_cmyk_to_rgb(c0, c1, c2, c3, app14_ycck);
                    out.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
                _ => out.copy_from_slice(&[0, 0, 0, 255]),
            }
        }
    }
    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

fn decode_block(
    br: &mut JpegBits<'_>,
    dc_table: &JpegHuffTable,
    ac_table: &JpegHuffTable,
    qt: &[u16; 64],
    pred: &mut i32,
    block: &mut [f32; 64],
) {
    let t = dc_table.decode(br) as u32;
    *pred += jpeg_extend(br.receive(t), t);
    block[0] = *pred as f32 * qt[0] as f32;
    let mut k = 1usize;
    while k < 64 {
        let rs = ac_table.decode(br);
        let r = (rs >> 4) as usize;
        let s = (rs & 0x0f) as u32;
        if s == 0 {
            if r == 15 {
                k += 16; // ZRL: skip 16 zeros
                continue;
            }
            break; // EOB
        }
        k += r;
        if k >= 64 {
            break;
        }
        let coeff = jpeg_extend(br.receive(s), s);
        block[JPEG_ZIGZAG[k]] = coeff as f32 * qt[k] as f32;
        k += 1;
    }
}

fn idct_8x8(block: &[f32; 64], cos_t: &[[f32; 8]; 8], out: &mut [f32; 64]) {
    let mut tmp = [0f32; 64];
    for v in 0..8 {
        for x in 0..8 {
            let mut sum = 0f32;
            for u in 0..8 {
                sum += block[v * 8 + u] * cos_t[u][x];
            }
            tmp[v * 8 + x] = sum * 0.5;
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let mut sum = 0f32;
            for v in 0..8 {
                sum += tmp[v * 8 + x] * cos_t[v][y];
            }
            out[y * 8 + x] = sum * 0.5;
        }
    }
}

fn jpeg_ycbcr_to_rgb(y: i32, cb: i32, cr: i32) -> [u8; 3] {
    let cb = cb - 128;
    let cr = cr - 128;
    let r = y + ((91881 * cr) >> 16);
    let g = y - ((22554 * cb + 46802 * cr) >> 16);
    let b = y + ((116130 * cb) >> 16);
    [
        r.clamp(0, 255) as u8,
        g.clamp(0, 255) as u8,
        b.clamp(0, 255) as u8,
    ]
}

fn jpeg_cmyk_to_rgb(c0: i32, c1: i32, c2: i32, c3: i32, ycck: bool) -> [u8; 3] {
    if ycck {
        // YCCK: first three are YCbCr, fourth is inverted K ink.
        let [r, g, b] = jpeg_ycbcr_to_rgb(c0, c1, c2);
        let k = 255 - c3;
        [
            (r as i32 * k / 255).clamp(0, 255) as u8,
            (g as i32 * k / 255).clamp(0, 255) as u8,
            (b as i32 * k / 255).clamp(0, 255) as u8,
        ]
    } else {
        // Direct CMYK: each channel is ink density; K multiplies the others.
        let k = 255 - c3;
        [
            ((255 - c0) * k / 255).clamp(0, 255) as u8,
            ((255 - c1) * k / 255).clamp(0, 255) as u8,
            ((255 - c2) * k / 255).clamp(0, 255) as u8,
        ]
    }
}

struct JpegPlane {
    width: usize,
    data: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
fn decode_jpeg_scan(
    bytes: &[u8],
    entropy_start: usize,
    width: usize,
    height: usize,
    components: &[JpegComponent],
    qtables: &[[u16; 64]; 4],
    dc_tables: &[Option<JpegHuffTable>; 4],
    ac_tables: &[Option<JpegHuffTable>; 4],
    restart_interval: usize,
    app14_ycck: bool,
) -> Result<DecodedImage, ImageDecodeError> {
    if width == 0 || height == 0 || components.is_empty() {
        return Err(ImageDecodeError::MalformedJpeg);
    }
    let max_h = components.iter().map(|c| c.h).max().unwrap_or(1);
    let max_v = components.iter().map(|c| c.v).max().unwrap_or(1);
    let mcus_x = width.div_ceil(max_h * 8);
    let mcus_y = height.div_ceil(max_v * 8);

    let mut planes: Vec<JpegPlane> = components
        .iter()
        .map(|c| {
            let pw = mcus_x * c.h * 8;
            let ph = mcus_y * c.v * 8;
            JpegPlane {
                width: pw,
                data: vec![0u8; pw * ph],
            }
        })
        .collect();

    // Precompute the 8-point IDCT cosine basis (Cu folded in).
    let mut cos_t = [[0f32; 8]; 8];
    for (u, row) in cos_t.iter_mut().enumerate() {
        let cu = if u == 0 { 1.0 / 2f32.sqrt() } else { 1.0 };
        for (x, slot) in row.iter_mut().enumerate() {
            *slot = cu * ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }

    let mut comps = components.to_vec();
    let mut br = JpegBits::new(bytes, entropy_start);
    let mut mcu_index = 0usize;

    for my in 0..mcus_y {
        for mx in 0..mcus_x {
            if restart_interval > 0 && mcu_index > 0 && mcu_index.is_multiple_of(restart_interval) {
                br.restart();
                for c in comps.iter_mut() {
                    c.pred = 0;
                }
            }
            for (ci, comp) in comps.iter_mut().enumerate() {
                let dc_table = dc_tables[comp.dc_table].as_ref().unwrap();
                let ac_table = ac_tables[comp.ac_table].as_ref().unwrap();
                let qt = &qtables[comp.quant];
                for by in 0..comp.v {
                    for bx in 0..comp.h {
                        let mut block = [0f32; 64];
                        decode_block(&mut br, dc_table, ac_table, qt, &mut comp.pred, &mut block);
                        let mut spatial = [0f32; 64];
                        idct_8x8(&block, &cos_t, &mut spatial);
                        let px0 = (mx * comp.h + bx) * 8;
                        let py0 = (my * comp.v + by) * 8;
                        let plane = &mut planes[ci];
                        for yy in 0..8 {
                            for xx in 0..8 {
                                let value =
                                    (spatial[yy * 8 + xx] + 128.0).round().clamp(0.0, 255.0) as u8;
                                plane.data[(py0 + yy) * plane.width + (px0 + xx)] = value;
                            }
                        }
                    }
                }
            }
            mcu_index += 1;
        }
    }

    let sample = |plane: &JpegPlane, comp: &JpegComponent, x: usize, y: usize| -> i32 {
        let cx = x * comp.h / max_h;
        let cy = y * comp.v / max_v;
        plane.data[cy * plane.width + cx] as i32
    };

    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let out = &mut rgba[(y * width + x) * 4..(y * width + x) * 4 + 4];
            match comps.len() {
                1 => {
                    let g = sample(&planes[0], &comps[0], x, y) as u8;
                    out.copy_from_slice(&[g, g, g, 255]);
                }
                3 => {
                    let yv = sample(&planes[0], &comps[0], x, y);
                    let cb = sample(&planes[1], &comps[1], x, y);
                    let cr = sample(&planes[2], &comps[2], x, y);
                    let rgb = jpeg_ycbcr_to_rgb(yv, cb, cr);
                    out.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
                4 => {
                    let c0 = sample(&planes[0], &comps[0], x, y);
                    let c1 = sample(&planes[1], &comps[1], x, y);
                    let c2 = sample(&planes[2], &comps[2], x, y);
                    let c3 = sample(&planes[3], &comps[3], x, y);
                    let rgb = jpeg_cmyk_to_rgb(c0, c1, c2, c3, app14_ycck);
                    out.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
                _ => out.copy_from_slice(&[0, 0, 0, 255]),
            }
        }
    }

    Ok(DecodedImage {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(image: &ColorImage, x: usize, y: usize) -> [u8; 4] {
        image.pixels[y * image.size[0] + x].to_array()
    }

    fn alpha_bounds(image: &ColorImage) -> Option<[usize; 4]> {
        let mut min_x = image.size[0];
        let mut min_y = image.size[1];
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found = false;
        for y in 0..image.size[1] {
            for x in 0..image.size[0] {
                if pixel(image, x, y)[3] == 0 {
                    continue;
                }
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        found.then_some([min_x, min_y, max_x, max_y])
    }

    fn test_stroke(
        width: f64,
        linecap: StrokeLineCap,
        linejoin: StrokeLineJoin,
        miterlimit: f64,
    ) -> ResolvedStroke {
        ResolvedStroke {
            paint: ResolvedPaint {
                source: ResolvedPaintSource::Solid(Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                }),
                opacity: 1.0,
            },
            width,
            linecap,
            linejoin,
            miterlimit,
            dash_array: None,
            dash_offset: 0.0,
        }
    }

    #[test]
    fn rejects_doctype_before_rendering() {
        let svg = r##"<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#ff0000"/></svg>"##;
        assert_eq!(
            rasterize(svg, 10, 10).unwrap_err(),
            SvgRasterError::ForbiddenContent
        );
    }

    #[test]
    fn rejects_non_local_references_after_structured_parse() {
        for svg in [
            r#"<svg><use href="javascript:alert(1)"/></svg>"#,
            r#"<svg><rect fill="url(//example.invalid/paint)"/></svg>"#,
        ] {
            assert_eq!(
                rasterize(svg, 10, 10).unwrap_err(),
                SvgRasterError::ForbiddenContent
            );
        }
    }

    #[test]
    fn defs_and_masks_do_not_render_as_visible_content() {
        let svg = r##"<svg viewBox="0 0 10 10">
<defs><rect width="10" height="10" fill="#ff0000"/></defs>
<mask id="m"><rect width="10" height="10" fill="#00ff00"/></mask>
</svg>"##;
        let image = rasterize(svg, 10, 10).unwrap();

        assert_eq!(pixel(&image, 5, 5), [0, 0, 0, 0]);
    }

    #[test]
    fn display_none_and_gradient_url_do_not_render_black_boxes() {
        let svg = r##"<svg viewBox="0 0 20 10">
<rect width="10" height="10" fill="#ff0000" display="none"/>
<rect x="10" width="10" height="10" fill="url(#g)"/>
</svg>"##;
        let image = rasterize(svg, 20, 10).unwrap();

        assert_eq!(pixel(&image, 5, 5), [0, 0, 0, 0]);
        assert_eq!(pixel(&image, 15, 5), [0, 0, 0, 0]);
    }

    #[test]
    fn render_report_counts_rendered_skipped_and_text_limitations() {
        // R11: <text> now renders via the bundled vector font (with an honest
        // text.raster_snapshot approximation warning) instead of being skipped.
        let svg = r##"<svg viewBox="0 0 20 20">
<rect width="10" height="10" fill="#ff0000"/>
<rect x="12" width="0" height="5" fill="#00ff00"/>
<text x="1" y="18">Hi</text>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 20).unwrap();

        assert_eq!(output.report.rendered_element_count, 2);
        assert_eq!(output.report.skipped_element_count, 1);
        assert_eq!(output.report.unsupported_feature_count, 0);
        let snapshot = output
            .report
            .warnings
            .iter()
            .find(|w| w.code == "text.raster_snapshot")
            .expect("raster snapshot warning");
        let source = snapshot.source.unwrap();
        assert!(svg[source.byte_start..source.byte_end].starts_with("<text"));
        assert_eq!(output.report.fidelity, SvgRenderFidelity::Medium);
        assert_eq!(pixel(&output.image, 5, 5), [255, 0, 0, 255]);
    }

    #[test]
    fn render_report_flags_unsupported_feature_buckets() {
        // clipPath (R4), filter (R7), and pattern (R9) all render now — none
        // should appear as unsupported feature buckets.
        let svg = r##"<svg viewBox="0 0 20 20">
<defs>
  <clipPath id="c"><rect width="10" height="10"/></clipPath>
  <pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="2" height="2" fill="#ff0000"/></pattern>
  <filter id="f"><feGaussianBlur stdDeviation="1"/></filter>
</defs>
<rect width="20" height="20" fill="url(#p)" clip-path="url(#c)" filter="url(#f)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 20).unwrap();
        let features: Vec<&str> = output
            .report
            .unsupported_features
            .iter()
            .map(|u| u.feature.as_str())
            .collect();

        // clipPath (R4), filter (R7), and pattern (R9) render → no unsupported.
        assert!(!features.contains(&"clipPath"));
        assert!(!features.contains(&"clip-path attribute"));
        assert!(!features.contains(&"filter"));
        assert!(!features.contains(&"filter attribute"));
        assert!(!features.contains(&"pattern"));
        // No `paint.unresolved_server` warning for a resolvable pattern.
        assert!(!output
            .report
            .warnings
            .iter()
            .any(|w| w.code == "paint.unresolved_server"));
        assert!(output.report.rendered_element_count >= 1);
    }

    #[test]
    fn comments_do_not_create_unsupported_diagnostics() {
        let svg = r##"<svg viewBox="0 0 10 10">
<!-- <filter id="not-real"/><text>not real</text><use href="#x"/> -->
<rect width="10" height="10" fill="#00ff00"/>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).unwrap();

        assert_eq!(output.report.rendered_element_count, 1);
        assert_eq!(output.report.skipped_element_count, 0);
        assert_eq!(output.report.unsupported_feature_count, 0);
        assert_eq!(output.report.fidelity, SvgRenderFidelity::High);
    }

    #[test]
    fn scene_flattens_group_style_and_element_transform() {
        let svg = r##"<svg viewBox="0 0 20 10">
<g fill="#ff0000" transform="translate(2,3)">
  <rect width="4" height="4" transform="translate(8,0)"/>
</g>
</svg>"##;
        let scene = SvgScene::parse(svg).unwrap();
        let renderable: Vec<&SvgSceneItem> = scene
            .items
            .iter()
            .filter(|item| matches!(item.node, SvgNode::Rect { .. }))
            .collect();

        assert_eq!(renderable.len(), 1);
        let (x, y) = renderable[0].transform.apply(0.0, 0.0);
        assert!((x - 10.0).abs() < 0.001);
        assert!((y - 3.0).abs() < 0.001);
        assert!(matches!(
            renderable[0].style.fill,
            Paint::Color(Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            })
        ));
    }

    #[test]
    fn scene_node_ids_and_source_spans_are_stable_preorder_metadata() {
        let svg = r##"<svg viewBox="0 0 20 10"><g id="group"><rect id="box" width="4" height="4"/></g><text>Hi</text></svg>"##;
        let first = SvgScene::parse(svg).unwrap();
        let second = SvgScene::parse(svg).unwrap();

        let first_meta: Vec<_> = first
            .items
            .iter()
            .map(|item| (item.node.id(), item.node.span()))
            .collect();
        let second_meta: Vec<_> = second
            .items
            .iter()
            .map(|item| (item.node.id(), item.node.span()))
            .collect();

        assert_eq!(first_meta, second_meta);
        assert_eq!(
            first_meta.iter().map(|(id, _)| id.0).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        for (_, span) in first_meta {
            let source = &svg[span.start..span.end];
            assert!(source.starts_with('<'));
            assert!(source.ends_with('>'));
        }
    }

    #[test]
    fn local_reference_table_is_deterministic_bounded_and_first_id_wins() {
        let svg = r##"<svg>
<defs><rect id="shape" width="2" height="2"/><circle id="shape" r="1"/></defs>
<use href="#shape"/><rect fill="url(#missing)" width="2" height="2"/>
</svg>"##;
        let scene = SvgScene::parse(svg).unwrap();

        assert_eq!(scene.references.ordered_ids.len(), 1);
        assert_eq!(scene.references.ordered_ids[0].xml_id, "shape");
        assert_eq!(scene.references.duplicate_id_count, 1);
        assert_eq!(scene.references.uses.len(), 2);
        assert!(scene.references.uses[0].resolved.is_some());
        assert!(scene.references.uses[1].resolved.is_none());

        let output = rasterize_with_report(svg, 10, 10).unwrap();
        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "reference.duplicate_id"));
        let unresolved = output
            .report
            .warnings
            .iter()
            .find(|warning| warning.code == "reference.unresolved")
            .unwrap();
        assert!(unresolved.source.is_some());
    }

    #[test]
    fn local_reference_tables_enforce_independent_caps() {
        let mut ids = String::from("<svg>");
        for index in 0..=MAX_LOCAL_IDS {
            ids.push_str(&format!(r#"<rect id="id{index}"/>"#));
        }
        ids.push_str("</svg>");
        let id_scene = SvgScene::parse(&ids).unwrap();
        assert_eq!(id_scene.references.ordered_ids.len(), MAX_LOCAL_IDS);
        assert_eq!(id_scene.references.dropped_id_count, 1);

        let mut uses = String::from(r#"<svg><rect id="target"/>"#);
        for _ in 0..=MAX_LOCAL_REFERENCE_USES {
            uses.push_str(r##"<use href="#target"/>"##);
        }
        uses.push_str("</svg>");
        let use_scene = SvgScene::parse(&uses).unwrap();
        assert_eq!(use_scene.references.uses.len(), MAX_LOCAL_REFERENCE_USES);
        assert_eq!(use_scene.references.dropped_use_count, 1);
    }

    #[test]
    fn display_list_owns_render_data_after_scene_is_dropped() {
        let svg =
            r##"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#ff0000"/></svg>"##;
        let display_list = {
            let scene = SvgScene::parse(svg).unwrap();
            let view = viewbox_to_pixel_transform(&scene, 10, 10);
            DisplayList::build(&scene, &view)
        };
        let mut pixels = vec![0; 10 * 10 * 4];
        let mut report = SvgRenderReport::new(10, 10, 10, 10);

        display_list.execute(&mut pixels, 10, 10, &mut report);

        assert_eq!(
            &pixels[(5 * 10 + 5) * 4..(5 * 10 + 5) * 4 + 4],
            &[255, 0, 0, 255]
        );
        assert_eq!(report.rendered_element_count, 1);
    }

    #[test]
    fn renderer_resolves_shared_absolute_and_percentage_lengths() {
        let svg = r##"<svg viewBox="0 0 100 100">
<rect width="50%" height="50%" fill="#ff0000"/>
</svg>"##;
        let image = rasterize(svg, 100, 100).unwrap();
        let physical = SvgScene::parse(r#"<svg width="1in" height="2.54cm"/>"#).unwrap();

        assert_eq!(pixel(&image, 25, 25), [255, 0, 0, 255]);
        assert_eq!(pixel(&image, 75, 75), [0, 0, 0, 0]);
        assert!((physical.width - 96.0).abs() < 0.001);
        assert!((physical.height - 96.0).abs() < 0.001);
    }

    #[test]
    fn root_viewbox_honors_meet_none_and_max_alignment() {
        let meet = rasterize(
            r##"<svg viewBox="0 0 100 50"><rect width="100" height="50" fill="#ff0000"/></svg>"##,
            200,
            200,
        )
        .unwrap();
        assert_eq!(alpha_bounds(&meet), Some([0, 50, 199, 149]));
        assert_eq!(pixel(&meet, 10, 10), [0, 0, 0, 0]);
        assert_eq!(pixel(&meet, 10, 60), [255, 0, 0, 255]);

        let none = rasterize(
            r##"<svg viewBox="0 0 100 50" preserveAspectRatio="none"><rect width="100" height="50" fill="#ff0000"/></svg>"##,
            200,
            200,
        )
        .unwrap();
        assert_eq!(alpha_bounds(&none), Some([0, 0, 199, 199]));
        assert_eq!(pixel(&none, 10, 10), [255, 0, 0, 255]);
        assert_eq!(pixel(&none, 10, 190), [255, 0, 0, 255]);

        let max = rasterize(
            r##"<svg viewBox="0 0 100 50" preserveAspectRatio="xMaxYMax meet"><rect width="100" height="50" fill="#ff0000"/></svg>"##,
            200,
            200,
        )
        .unwrap();
        assert_eq!(alpha_bounds(&max), Some([0, 100, 199, 199]));
        assert_eq!(pixel(&max, 10, 50), [0, 0, 0, 0]);
        assert_eq!(pixel(&max, 10, 150), [255, 0, 0, 255]);
    }

    #[test]
    fn nested_svg_viewports_honor_aspect_ratio_and_percentage_geometry() {
        let meet = rasterize(
            r##"<svg viewBox="0 0 100 100">
<svg x="20" y="10" width="40" height="60" viewBox="0 0 10 10">
  <rect width="10" height="10" fill="#ff0000"/>
</svg>
</svg>"##,
            100,
            100,
        )
        .unwrap();
        assert_eq!(alpha_bounds(&meet), Some([20, 20, 59, 59]));
        assert_eq!(pixel(&meet, 25, 15), [0, 0, 0, 0]);
        assert_eq!(pixel(&meet, 25, 25), [255, 0, 0, 255]);

        let none = rasterize(
            r##"<svg viewBox="0 0 100 100">
<svg x="20" y="10" width="40" height="60" viewBox="0 0 10 10" preserveAspectRatio="none">
  <rect width="10" height="10" fill="#00ff00"/>
</svg>
</svg>"##,
            100,
            100,
        )
        .unwrap();
        assert_eq!(alpha_bounds(&none), Some([20, 10, 59, 69]));
        assert_eq!(pixel(&none, 25, 15), [0, 255, 0, 255]);
        assert_eq!(pixel(&none, 25, 65), [0, 255, 0, 255]);

        let percentage = rasterize(
            r##"<svg viewBox="0 0 100 100">
<svg x="10%" y="20%" width="50%" height="40%" viewBox="0 0 10 10">
  <rect width="100%" height="100%" fill="#0000ff"/>
</svg>
</svg>"##,
            100,
            100,
        )
        .unwrap();
        assert_eq!(alpha_bounds(&percentage), Some([15, 20, 54, 59]));
        assert_eq!(pixel(&percentage, 12, 25), [0, 0, 0, 0]);
        assert_eq!(pixel(&percentage, 20, 25), [0, 0, 255, 255]);
        assert_eq!(pixel(&percentage, 50, 55), [0, 0, 255, 255]);
    }

    #[test]
    fn root_percentage_dimensions_keep_a_usable_viewport_fallback() {
        let scene = SvgScene::parse(
            r#"<svg width="100%" height="50%"><rect width="100%" height="100%"/></svg>"#,
        )
        .unwrap();

        assert_eq!(scene.width, 100.0);
        assert_eq!(scene.height, 50.0);
        let geometry = scene
            .items
            .iter()
            .find_map(|item| lower_shape_geometry(&item.node, item.length_bases))
            .unwrap();
        assert!(matches!(
            geometry,
            ShapeGeometry::Rect {
                width: 100.0,
                height: 50.0,
                ..
            }
        ));
    }

    #[test]
    fn element_transform_affects_rendered_pixels() {
        let svg = r##"<svg viewBox="0 0 20 10">
<rect width="5" height="5" fill="#ff0000" transform="translate(10,0)"/>
</svg>"##;
        let image = rasterize(svg, 20, 10).unwrap();

        assert_eq!(pixel(&image, 2, 2), [0, 0, 0, 0]);
        assert_eq!(pixel(&image, 12, 2), [255, 0, 0, 255]);
    }

    #[test]
    fn compact_path_syntax_renders_visible_pixels() {
        let svg = r##"<svg viewBox="0 0 10 10">
<path d="M2 2L8.5.5L8 8L2 8Z" fill="#ff0000"/>
</svg>"##;
        let image = rasterize(svg, 10, 10).unwrap();

        assert_eq!(pixel(&image, 5, 5), [255, 0, 0, 255]);
    }

    #[test]
    fn diagonal_fill_produces_partial_alpha_coverage() {
        let svg = r##"<svg viewBox="0 0 16 16">
<path d="M1 1 L15 6 L3 15 Z" fill="#ff0000"/>
</svg>"##;
        let image = rasterize(svg, 16, 16).unwrap();

        assert!(image
            .pixels
            .iter()
            .map(|color| color.a())
            .any(|alpha| alpha > 0 && alpha < 255));
    }

    #[test]
    fn retained_path_preserves_explicit_close_and_open_subpaths() {
        let path = parse_path_d("M0 0 L4 0 Z M10 0 L14 0");

        assert_eq!(path.subpaths.len(), 2);
        assert!(path.subpaths[0].closed);
        assert!(!path.subpaths[1].closed);
        assert_eq!(path.subpaths[0].segments.last().unwrap().end(), (4.0, 0.0));
    }

    #[test]
    fn retained_path_reflects_smooth_cubic_control_point() {
        let path = parse_path_d("M0 0 C0 0 10 0 10 10 S20 20 30 10");
        let second = &path.subpaths[0].segments[1];

        assert!(matches!(
            second,
            PathSegment::Cubic {
                ctrl1: (10.0, 20.0),
                ctrl2: (20.0, 20.0),
                to: (30.0, 10.0)
            }
        ));
    }

    #[test]
    fn path_flattening_uses_final_device_transform_for_tolerance() {
        let path = parse_path_d("M0 0 C0 10 10 10 10 0");
        let identity = flatten_path_data(&path, &Transform::identity(), 0.25);
        let enlarged = flatten_path_data(&path, &Transform::scale(100.0, 100.0), 0.25);

        assert!(enlarged[0].points.len() > identity[0].points.len());
        assert!(enlarged[0].points.len() <= MAX_FLAT_PTS);
    }

    #[test]
    fn nonzero_and_evenodd_distinguish_same_winding_contours() {
        let path = "M1 1 H9 V9 H1 Z M3 3 H7 V7 H3 Z";
        let nonzero = rasterize(
            &format!(r##"<svg viewBox="0 0 10 10"><path d="{path}" fill="#ff0000"/></svg>"##),
            10,
            10,
        )
        .unwrap();
        let evenodd = rasterize_with_report(
            &format!(
                r##"<svg viewBox="0 0 10 10"><path d="{path}" fill="#ff0000" fill-rule="evenodd"/></svg>"##
            ),
            10,
            10,
        )
        .unwrap();

        assert_eq!(pixel(&nonzero, 5, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(&evenodd.image, 5, 5), [0, 0, 0, 0]);
        assert_eq!(pixel(&evenodd.image, 2, 5), [255, 0, 0, 255]);
        assert_eq!(evenodd.report.warning_count, 0);
        assert_eq!(evenodd.report.unsupported_feature_count, 0);
    }

    #[test]
    fn self_intersecting_evenodd_fill_keeps_pentagram_center_open() {
        let path = "M10 1 L15.3 18 L1.4 7.5 L18.6 7.5 L4.7 18 Z";
        let nonzero = rasterize(
            &format!(r##"<svg viewBox="0 0 20 20"><path d="{path}" fill="#ff0000"/></svg>"##),
            20,
            20,
        )
        .unwrap();
        let evenodd = rasterize(
            &format!(
                r##"<svg viewBox="0 0 20 20"><path d="{path}" fill="#ff0000" fill-rule="evenodd"/></svg>"##
            ),
            20,
            20,
        )
        .unwrap();

        assert_eq!(pixel(&nonzero, 10, 10), [255, 0, 0, 255]);
        assert_eq!(pixel(&evenodd, 10, 10), [0, 0, 0, 0]);
    }

    #[test]
    fn stroke_union_does_not_stack_opacity_at_joins() {
        let svg = r##"<svg viewBox="0 0 20 20">
<path d="M2 17 L10 3 L18 17" fill="none" stroke="#ff0000"
      stroke-width="6" stroke-linejoin="round" stroke-opacity="0.5"/>
</svg>"##;
        let image = rasterize(svg, 20, 20).unwrap();
        let alphas: Vec<u8> = image.pixels.iter().map(|color| color.a()).collect();

        assert!(alphas.contains(&128));
        assert!(alphas.iter().all(|alpha| *alpha <= 128));
    }

    #[test]
    fn scaled_stroke_bounds_match_transformed_outline() {
        let svg = r##"<svg viewBox="0 0 10 10">
<line x1="2" y1="5" x2="8" y2="5" stroke="#ff0000" stroke-width="2"/>
</svg>"##;
        let image = rasterize(svg, 100, 100).unwrap();

        assert_eq!(alpha_bounds(&image), Some([20, 40, 79, 59]));
    }

    #[test]
    fn cap_and_miter_limits_have_geometric_bounds() {
        let line = [(0.0, 0.0), (10.0, 0.0)];
        let butt = build_stroke_mesh(
            &line,
            &Transform::identity(),
            &test_stroke(2.0, StrokeLineCap::Butt, StrokeLineJoin::Miter, 4.0),
            false,
        );
        let square = build_stroke_mesh(
            &line,
            &Transform::identity(),
            &test_stroke(2.0, StrokeLineCap::Square, StrokeLineJoin::Miter, 4.0),
            false,
        );
        let round = build_stroke_mesh(
            &line,
            &Transform::identity(),
            &test_stroke(2.0, StrokeLineCap::Round, StrokeLineJoin::Miter, 4.0),
            false,
        );
        assert_eq!(
            butt.local_bounds,
            Bounds2D {
                min_x: 0.0,
                min_y: -1.0,
                max_x: 10.0,
                max_y: 1.0,
            }
        );
        assert_eq!(
            square.local_bounds,
            Bounds2D {
                min_x: -1.0,
                min_y: -1.0,
                max_x: 11.0,
                max_y: 1.0,
            }
        );
        assert!((round.local_bounds.min_x + 1.0).abs() < 1.0e-9);
        assert!((round.local_bounds.max_x - 11.0).abs() < 1.0e-9);

        let corner = [(0.0, 10.0), (5.0, 0.0), (10.0, 10.0)];
        let clipped = build_stroke_mesh(
            &corner,
            &Transform::identity(),
            &test_stroke(2.0, StrokeLineCap::Butt, StrokeLineJoin::Miter, 1.0),
            false,
        );
        let extended = build_stroke_mesh(
            &corner,
            &Transform::identity(),
            &test_stroke(2.0, StrokeLineCap::Butt, StrokeLineJoin::Miter, 8.0),
            false,
        );
        assert!(extended.local_bounds.min_y < clipped.local_bounds.min_y);
    }

    #[test]
    fn nonuniform_transform_applies_to_complete_stroke_outline() {
        let transform = Transform::translate(20.0, 10.0)
            .multiply(Transform::rotate(30.0))
            .multiply(Transform::scale(3.0, 0.5));
        let mesh = build_stroke_mesh(
            &[(0.0, 0.0), (10.0, 0.0)],
            &transform,
            &test_stroke(4.0, StrokeLineCap::Square, StrokeLineJoin::Miter, 4.0),
            false,
        );
        let transformed_corners = [
            transform.apply(-2.0, -2.0),
            transform.apply(-2.0, 2.0),
            transform.apply(12.0, -2.0),
            transform.apply(12.0, 2.0),
        ];

        for point in transformed_corners {
            assert!(point.0 >= mesh.device_bounds.min_x - 1.0e-9);
            assert!(point.0 <= mesh.device_bounds.max_x + 1.0e-9);
            assert!(point.1 >= mesh.device_bounds.min_y - 1.0e-9);
            assert!(point.1 <= mesh.device_bounds.max_y + 1.0e-9);
        }
    }

    #[test]
    fn dash_pattern_phase_and_path_length_change_visible_runs() {
        let base = r##"<svg viewBox="0 0 20 10">
<line x1="1" y1="5.5" x2="19" y2="5.5" stroke="#ff0000"
      stroke-width="1" stroke-dasharray="2 2" {extra}/>
</svg>"##;
        let plain = rasterize(&base.replace("{extra}", ""), 20, 10).unwrap();
        let shifted =
            rasterize(&base.replace("{extra}", r#"stroke-dashoffset="1""#), 20, 10).unwrap();
        let calibrated = rasterize(&base.replace("{extra}", r#"pathLength="9""#), 20, 10).unwrap();

        assert!(pixel(&plain, 1, 5)[3] > 0);
        assert_eq!(pixel(&plain, 3, 5)[3], 0);
        assert_ne!(plain.pixels, shifted.pixels);
        assert_eq!(pixel(&plain, 4, 5)[3], 0);
        assert!(pixel(&calibrated, 4, 5)[3] > 0);
    }

    #[test]
    fn odd_dash_array_repeats_and_zero_length_round_dash_draws_dot() {
        let parsed = parse_dasharray("2 1 3").unwrap().unwrap();
        assert_eq!(parsed.len(), 6);
        assert_eq!(parsed[0], parsed[3]);
        assert_eq!(parsed[1], parsed[4]);
        assert_eq!(parsed[2], parsed[5]);

        let svg = r##"<svg viewBox="0 0 10 10">
<line x1="5" y1="5" x2="5" y2="5" stroke="#ff0000"
      stroke-width="4" stroke-linecap="round"/>
</svg>"##;
        let image = rasterize(svg, 10, 10).unwrap();
        assert!(pixel(&image, 5, 5)[3] > 0);
        assert!(alpha_bounds(&image).is_some());
    }

    #[test]
    fn closed_dash_pattern_merges_painted_run_across_seam() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let (runs, limit_hit) = dash_polyline(&square, true, &[6.0, 2.0], -1.0, None);

        assert!(!limit_hit);
        assert_eq!(runs.len(), 5);
        let seam_run = &runs[0];
        assert!(seam_run.points.len() >= 3);
        assert!(seam_run
            .points
            .windows(2)
            .any(|pair| distance(pair[0], pair[1]) > 0.0));
    }

    #[test]
    fn valid_stroke_features_are_supported_and_invalid_values_warn() {
        let valid = r##"<svg viewBox="0 0 20 10">
<path d="M1 5 L10 1 L19 5" fill="none" stroke="#ff0000"
      stroke-width="2" stroke-linecap="round" stroke-linejoin="miter-clip"
      stroke-miterlimit="3" stroke-dasharray="3 1" stroke-dashoffset="-2"
      pathLength="18"/>
</svg>"##;
        let valid_output = rasterize_with_report(valid, 20, 10).unwrap();
        assert_eq!(valid_output.report.warning_count, 0);
        assert_eq!(valid_output.report.unsupported_feature_count, 0);

        let invalid = r##"<svg viewBox="0 0 20 10">
<line x1="1" y1="5" x2="19" y2="5" stroke="#ff0000"
      stroke-dasharray="3 -1" pathLength="0"/>
</svg>"##;
        let invalid_output = rasterize_with_report(invalid, 20, 10).unwrap();
        let codes: Vec<&str> = invalid_output
            .report
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect();
        assert!(codes.contains(&"style.invalid_stroke_dasharray"));
        assert!(codes.contains(&"geometry.invalid_path_length"));
    }

    #[test]
    fn dash_run_limit_is_reported_instead_of_silently_truncating() {
        let svg = r##"<svg viewBox="0 0 1 1">
<line x1="0" y1=".5" x2="1" y2=".5" stroke="#ff0000"
      stroke-width=".1" stroke-dasharray="0 .00001"/>
</svg>"##;
        let output = rasterize_with_report(svg, 2, 2).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "limit.stroke_complexity"));
    }

    #[test]
    fn dashed_stroke_rendering_is_deterministic() {
        let svg = r##"<svg viewBox="0 0 20 20">
<path d="M2 18 L10 2 L18 18 Z" fill="none" stroke="#123456"
      stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"
      stroke-dasharray="3 1 0" stroke-dashoffset="-2.25"/>
</svg>"##;
        let first = rasterize(svg, 64, 64).unwrap();
        let second = rasterize(svg, 64, 64).unwrap();

        assert_eq!(first.pixels, second.pixels);
    }

    #[test]
    fn nonzero_creates_hole_for_opposite_winding_contours() {
        let svg = r##"<svg viewBox="0 0 10 10">
<path d="M1 1 H9 V9 H1 Z M3 3 V7 H7 V3 Z" fill="#00ff00"/>
</svg>"##;
        let image = rasterize(svg, 10, 10).unwrap();

        assert_eq!(pixel(&image, 5, 5), [0, 0, 0, 0]);
        assert_eq!(pixel(&image, 2, 5), [0, 255, 0, 255]);
    }

    #[test]
    fn fill_rule_inherits_and_inline_style_overrides_presentation_attribute() {
        let path = "M1 1 H9 V9 H1 Z M3 3 H7 V7 H3 Z";
        let inherited = rasterize(
            &format!(
                r##"<svg viewBox="0 0 10 10"><g fill-rule="evenodd"><path d="{path}"/></g></svg>"##
            ),
            10,
            10,
        )
        .unwrap();
        let inline_override = rasterize(
            &format!(
                r##"<svg viewBox="0 0 10 10"><path d="{path}" fill-rule="nonzero" style="fill-rule: evenodd"/></svg>"##
            ),
            10,
            10,
        )
        .unwrap();

        assert_eq!(pixel(&inherited, 5, 5), [0, 0, 0, 0]);
        assert_eq!(pixel(&inline_override, 5, 5), [0, 0, 0, 0]);
    }

    #[test]
    fn invalid_fill_rule_warns_and_keeps_inherited_rule() {
        let svg = r##"<svg viewBox="0 0 10 10">
<g fill-rule="evenodd">
  <path d="M1 1 H9 V9 H1 Z M3 3 H7 V7 H3 Z" fill-rule="sideways"/>
</g>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).unwrap();

        assert_eq!(pixel(&output.image, 5, 5), [0, 0, 0, 0]);
        let warning = output
            .report
            .warnings
            .iter()
            .find(|warning| warning.code == "style.invalid_fill_rule")
            .expect("invalid fill-rule warning");
        assert!(warning.source.is_some());
        assert!(!output
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "fill-rule"));
    }

    #[test]
    fn unknown_path_command_does_not_stall_renderer() {
        let path = parse_path_d("M1 1 R5 5 L8 1 L8 8 Z");

        assert!(!path.subpaths.is_empty());
        assert!(path
            .subpaths
            .iter()
            .flat_map(|subpath| &subpath.segments)
            .any(|segment| segment.end() == (8.0, 8.0)));
    }

    #[test]
    fn supported_defs_and_linear_gradient_render_without_unsupported_claims() {
        let svg = r##"<svg viewBox="0 0 10 10">
<defs><linearGradient id="g"><stop offset="0%" stop-color="red"/></linearGradient></defs>
<rect width="10" height="10" fill="url(#g)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).unwrap();
        let features: Vec<&str> = output
            .report
            .unsupported_features
            .iter()
            .map(|u| u.feature.as_str())
            .collect();

        assert_eq!(output.report.rendered_element_count, 1);
        assert!(!features.contains(&"defs"));
        assert!(!features.contains(&"linearGradient"));
        assert_eq!(pixel(&output.image, 5, 5), [255, 0, 0, 255]);
        assert_eq!(output.report.fidelity, SvgRenderFidelity::High);
    }

    #[test]
    fn linear_gradient_interpolates_object_bounding_box_stops() {
        let svg = r##"<svg viewBox="0 0 20 4">
<defs><linearGradient id="g">
  <stop offset="0%" stop-color="#ff0000"/>
  <stop offset="100%" stop-color="#0000ff"/>
</linearGradient></defs>
<rect width="20" height="4" fill="url(#g)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 4).unwrap();
        let left = pixel(&output.image, 1, 2);
        let middle = pixel(&output.image, 10, 2);
        let right = pixel(&output.image, 18, 2);

        assert!(left[0] > left[2]);
        assert!((middle[0] as i16 - middle[2] as i16).abs() < 20);
        assert!(right[2] > right[0]);
        assert_eq!(output.report.unsupported_feature_count, 0);
    }

    #[test]
    fn linear_gradient_honors_transform_units_and_spread_methods() {
        let vertical = r##"<svg viewBox="0 0 10 10">
<defs><linearGradient id="g" gradientTransform="rotate(90 .5 .5)">
<stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/>
</linearGradient></defs><rect width="10" height="10" fill="url(#g)"/></svg>"##;
        let image = rasterize(vertical, 10, 10).unwrap();
        assert!(pixel(&image, 5, 1)[0] > pixel(&image, 5, 1)[2]);
        assert!(pixel(&image, 5, 8)[2] > pixel(&image, 5, 8)[0]);

        let repeated = r##"<svg viewBox="0 0 20 4">
<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" x2="5"
 spreadMethod="repeat"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/>
</linearGradient></defs><rect width="20" height="4" fill="url(#g)"/></svg>"##;
        let image = rasterize(repeated, 20, 4).unwrap();
        assert_eq!(pixel(&image, 2, 2), pixel(&image, 7, 2));
        assert_eq!(pixel(&image, 7, 2), pixel(&image, 12, 2));
    }

    #[test]
    fn radial_gradient_uses_focal_point_and_stop_opacity() {
        let svg = r##"<svg viewBox="0 0 20 20" color="#00ff00">
<style>.edge { stop-color: currentColor; stop-opacity: .5; }</style>
<defs><radialGradient id="g" cx="50%" cy="50%" r="50%" fx="35%" fy="35%">
<stop offset="0" stop-color="white"/>
<stop class="edge" offset="1"/>
</radialGradient></defs>
<circle cx="10" cy="10" r="10" fill="url(#g)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 20).unwrap();
        let focal = pixel(&output.image, 7, 7);
        let edge = pixel(&output.image, 18, 10);

        assert!(focal[0] > 200 && focal[1] > 200 && focal[2] > 200);
        assert!(edge[1] > edge[0] && edge[1] > edge[2]);
        assert!(edge[3] < 255);
        assert_eq!(output.report.unsupported_feature_count, 0);
    }

    #[test]
    fn gradient_href_inherits_stops_and_overrides_geometry() {
        let svg = r##"<svg viewBox="0 0 20 4">
<defs>
<linearGradient id="base"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
<linearGradient id="derived" href="#base" x1="100%" x2="0%"/>
</defs>
<rect width="20" height="4" fill="url(#derived)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 4).unwrap();

        assert!(pixel(&output.image, 1, 2)[2] > pixel(&output.image, 1, 2)[0]);
        assert!(pixel(&output.image, 18, 2)[0] > pixel(&output.image, 18, 2)[2]);
        assert_eq!(output.report.warning_count, 0);
    }

    #[test]
    fn gradient_strokes_sample_through_existing_union_coverage() {
        let svg = r##"<svg viewBox="0 0 20 10">
<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" x2="20">
<stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/>
</linearGradient></defs>
<path d="M1 8 L10 2 L19 8" fill="none" stroke="url(#g)" stroke-width="3" stroke-linejoin="round"/>
</svg>"##;
        let image = rasterize(svg, 20, 10).unwrap();

        assert!(pixel(&image, 3, 7)[0] > pixel(&image, 3, 7)[2]);
        assert!(pixel(&image, 17, 7)[2] > pixel(&image, 17, 7)[0]);
    }

    #[test]
    fn gradient_cycles_are_diagnosed_and_patterns_resolve() {
        // The gradient href cycle is still diagnosed; the pattern (R9) now
        // resolves as a paint server rather than landing in the unsupported
        // bucket, even when it has no renderable content.
        let svg = r##"<svg viewBox="0 0 10 10">
<defs>
<linearGradient id="a" href="#b"/><linearGradient id="b" href="#a"/>
<pattern id="p" patternUnits="userSpaceOnUse" patternTransform="rotate(20)" width="4" height="4"/>
</defs>
<rect width="5" height="10" fill="url(#a)"/>
<rect x="5" width="5" height="10" fill="url(#p)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "reference.gradient_cycle"));
        // Pattern is resolved: not an unsupported feature, no unresolved-server.
        assert!(!output
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "pattern"));
        assert!(!output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "paint.unresolved_server"
                && warning.message.contains("#p")));
    }

    #[test]
    fn malformed_gradient_values_report_exact_fallbacks() {
        let svg = r##"<svg viewBox="0 0 10 10">
<defs>
  <linearGradient id="bad" gradientUnits="screen" gradientTransform="warp(2)"
      spreadMethod="bounce" x1="nope">
    <stop offset="wrong" stop-color="not-a-color" stop-opacity="opaque"/>
    <stop offset="1" stop-color="blue"/>
  </linearGradient>
  <radialGradient id="radius" r="-2">
    <stop offset="0" stop-color="red"/>
  </radialGradient>
</defs>
<rect width="5" height="10" fill="url(#bad)"/>
<rect x="5" width="5" height="10" fill="url(#radius)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).expect("gradient fallback render");
        let codes: Vec<_> = output
            .report
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect();
        for expected in [
            "paint.invalid_gradient_units",
            "paint.invalid_gradient_transform",
            "paint.invalid_spread_method",
            "paint.invalid_gradient_length",
            "paint.invalid_stop_offset",
            "paint.invalid_stop_color",
            "paint.invalid_stop_opacity",
        ] {
            assert!(codes.contains(&expected), "missing diagnostic {expected}");
        }
    }

    #[test]
    fn gradients_and_patterns_render_deterministically_and_high_fidelity() {
        let gradient = r##"<svg viewBox="0 0 12 4"><defs>
<linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
</defs><rect width="12" height="4" fill="url(#g)"/></svg>"##;
        let first = rasterize_with_report(gradient, 12, 4).unwrap();
        let second = rasterize_with_report(gradient, 12, 4).unwrap();
        assert_eq!(first.image.pixels, second.image.pixels);
        assert_eq!(first.report.fidelity, SvgRenderFidelity::High);
        assert_eq!(first.report.unsupported_feature_count, 0);

        // R9: patterns now tile and render (not diagnosed-transparent), so the
        // fill is no longer flagged unsupported and produces real pixels.
        let pattern = r##"<svg viewBox="0 0 12 4"><defs>
<pattern id="p" width="2" height="2" patternUnits="userSpaceOnUse"><rect width="1" height="1" fill="#ff0000"/></pattern>
</defs><rect width="12" height="4" fill="url(#p)"/></svg>"##;
        let pfirst = rasterize_with_report(pattern, 12, 4).unwrap();
        let psecond = rasterize_with_report(pattern, 12, 4).unwrap();
        assert_eq!(pfirst.image.pixels, psecond.image.pixels);
        assert!(!pfirst
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "pattern"));
        assert!(!pfirst
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "paint.unresolved_server"));
        // The tile (a red square in the top-left of each 2x2 cell) actually paints.
        assert_eq!(pixel(&pfirst.image, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&pfirst.image, 1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn tier1_css_specificity_and_current_color_render_consistently() {
        let svg = r##"<svg viewBox="0 0 30 10" color="#112233">
<style>
  rect, .base { fill: #ff0000; }
  rect.hot { fill: #00ff00; }
  #hero { fill: currentColor; }
</style>
<rect class="base" width="10" height="10"/>
<rect class="hot" x="10" width="10" height="10"/>
<rect id="hero" class="hot" x="20" width="10" height="10"/>
</svg>"##;
        let output = rasterize_with_report(svg, 30, 10).unwrap();

        assert_eq!(pixel(&output.image, 5, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(&output.image, 15, 5), [0, 255, 0, 255]);
        assert_eq!(pixel(&output.image, 25, 5), [17, 34, 51, 255]);
        assert!(!output
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "style"));
    }

    #[test]
    fn complex_css_is_diagnosed_without_discarding_supported_rules() {
        let svg = r##"<svg viewBox="0 0 10 10">
<style>g > rect { fill: red; } rect { fill: #00ff00; }</style>
<rect width="10" height="10"/>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).unwrap();

        assert_eq!(pixel(&output.image, 5, 5), [0, 255, 0, 255]);
        assert!(output
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "complex CSS selector"));
    }

    #[test]
    fn local_use_and_symbol_viewbox_render_with_inherited_style() {
        let svg = r##"<svg viewBox="0 0 30 10" color="#00ff00">
<defs>
  <rect id="box" width="5" height="5" fill="currentColor"/>
  <symbol id="icon" viewBox="0 0 5 5"><circle cx="2.5" cy="2.5" r="2.5"/></symbol>
</defs>
<use href="#box" x="2" y="2"/>
<use href="#icon" x="15" width="10" height="10" fill="#0000ff"/>
</svg>"##;
        let output = rasterize_with_report(svg, 30, 10).unwrap();

        assert_eq!(pixel(&output.image, 4, 4), [0, 255, 0, 255]);
        assert_eq!(pixel(&output.image, 20, 5), [0, 0, 255, 255]);
        assert!(!output
            .report
            .unsupported_features
            .iter()
            .any(|feature| matches!(feature.feature.as_str(), "use" | "symbol" | "defs")));
    }

    #[test]
    fn use_cycles_and_duplicate_ids_are_bounded_and_reported() {
        let svg = r##"<svg viewBox="0 0 10 10">
<defs>
  <g id="a"><use href="#b"/></g>
  <g id="b"><use href="#a"/></g>
  <rect id="dup" width="2" height="2"/>
  <rect id="dup" width="8" height="8"/>
</defs>
<use href="#a"/>
<use href="#dup"/>
</svg>"##;
        let output = rasterize_with_report(svg, 10, 10).unwrap();
        let codes: Vec<&str> = output
            .report
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect();

        assert!(codes.contains(&"reference.duplicate_id"));
        assert!(codes.contains(&"reference.use_cycle"));
        assert_eq!(alpha_bounds(&output.image), Some([0, 0, 1, 1]));
    }

    #[test]
    fn render_output_is_deterministic_for_same_svg_and_size() {
        let svg = r##"<svg viewBox="-5 -5 20 20">
<g transform="translate(2,3) scale(0.5)">
  <path d="M0 0 L10 0 L10 10 Z" fill="#123456" opacity="0.8"/>
</g>
</svg>"##;
        let first = rasterize_with_report(svg, 32, 32).unwrap();
        let second = rasterize_with_report(svg, 32, 32).unwrap();

        assert_eq!(first.image.size, second.image.size);
        assert_eq!(first.image.pixels, second.image.pixels);
        assert_eq!(
            first.report.rendered_element_count,
            second.report.rendered_element_count
        );
        assert_eq!(
            first.report.unsupported_features,
            second.report.unsupported_features
        );
        assert_eq!(first.report.fidelity, second.report.fidelity);
    }

    #[test]
    fn render_report_records_raster_size_clamp() {
        let svg = r##"<svg viewBox="0 0 1 1"><rect width="1" height="1"/></svg>"##;
        let output = rasterize_with_report(svg, 5000, 1).unwrap();

        assert_eq!(output.report.requested_width, 5000);
        assert_eq!(output.report.requested_height, 1);
        assert_eq!(output.report.output_width, MAX_RASTER_DIM);
        assert_eq!(output.report.output_height, 1);
        assert_eq!(output.report.warning_count, 1);
        assert_eq!(output.report.warnings[0].code, "limit.raster_size");
    }

    #[test]
    #[ignore = "coarse local performance budget; run explicitly for renderer profiling"]
    fn antialiased_fill_performance_smoke() {
        let mut path = String::from("M256 8");
        for index in 1usize..256 {
            let angle = index as f64 / 256.0 * std::f64::consts::TAU;
            let radius = if index.is_multiple_of(2) { 248.0 } else { 96.0 };
            let x = 256.0 + angle.sin() * radius;
            let y = 256.0 - angle.cos() * radius;
            path.push_str(&format!(" L{x:.3} {y:.3}"));
        }
        path.push_str(" Z");
        let svg =
            format!(r##"<svg viewBox="0 0 512 512"><path d="{path}" fill="#336699"/></svg>"##);
        let started = std::time::Instant::now();
        let output = rasterize(&svg, 512, 512).unwrap();

        assert!(output.pixels.iter().any(|color| color.a() > 0));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "512px supersampled fill exceeded the 5-second debug budget"
        );
    }

    // -----------------------------------------------------------------------
    // R4: clipping, viewport overflow, premultiplied compositing, group opacity
    // -----------------------------------------------------------------------

    #[test]
    fn clip_path_renders_visibly_clipped_high_fidelity() {
        let svg = r##"<svg viewBox="0 0 4 4">
<clipPath id="c"><rect width="2" height="4"/></clipPath>
<rect width="4" height="4" fill="#ff0000" clip-path="url(#c)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 4, 4).unwrap();

        assert_eq!(pixel(&output.image, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&output.image, 3, 0), [0, 0, 0, 0]);
        assert_eq!(output.report.rendered_element_count, 1);
        // Clip is rendered, not approximated: no clip/clip-path unsupported buckets.
        assert!(!output
            .report
            .unsupported_features
            .iter()
            .any(|f| f.feature.contains("clip")));
        assert_eq!(output.report.fidelity, SvgRenderFidelity::High);
    }

    #[test]
    fn nested_svg_overflow_is_clipped_to_viewport() {
        let svg = r##"<svg viewBox="0 0 4 4">
<svg x="0" y="0" width="2" height="2"><rect width="4" height="4" fill="#ff0000"/></svg>
</svg>"##;
        let image = rasterize(svg, 4, 4).unwrap();

        assert_eq!(pixel(&image, 0, 0), [255, 0, 0, 255]);
        // Content beyond the 2x2 nested viewport is clipped away.
        assert_eq!(pixel(&image, 3, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&image, 0, 3), [0, 0, 0, 0]);
        assert_eq!(alpha_bounds(&image), Some([0, 0, 1, 1]));
    }

    #[test]
    fn translucent_group_does_not_double_darken_overlap() {
        // Two overlapping opaque fills inside a 50% group: the overlap must
        // match the non-overlap alpha (composited once), not stack to 0.75.
        let svg = r##"<svg viewBox="0 0 6 4">
<g opacity="0.5"><rect width="4" height="4" fill="#ff0000"/><rect x="2" width="4" height="4" fill="#ff0000"/></g>
</svg>"##;
        let image = rasterize(svg, 6, 4).unwrap();
        let only_first = pixel(&image, 1, 2);
        let overlap = pixel(&image, 3, 2);
        let only_second = pixel(&image, 5, 2);

        assert_eq!(only_first[3], 128);
        assert_eq!(overlap[3], 128, "overlap must not double-darken");
        assert_eq!(only_second[3], 128);
        assert_eq!(overlap, only_first);
    }

    #[test]
    fn isolated_group_differs_from_flat_per_element_opacity() {
        let isolated = rasterize(
            r##"<svg viewBox="0 0 6 4"><g opacity="0.5"><rect width="4" height="4" fill="#ff0000"/><rect x="2" width="4" height="4" fill="#ff0000"/></g></svg>"##,
            6,
            4,
        )
        .unwrap();
        let flat = rasterize(
            r##"<svg viewBox="0 0 6 4"><rect width="4" height="4" fill="#ff0000" opacity="0.5"/><rect x="2" width="4" height="4" fill="#ff0000" opacity="0.5"/></svg>"##,
            6,
            4,
        )
        .unwrap();

        // Outside the overlap the two agree; inside the overlap the flat version
        // double-composites (higher alpha) while the isolated group does not.
        assert_eq!(pixel(&isolated, 1, 2)[3], pixel(&flat, 1, 2)[3]);
        assert_eq!(pixel(&isolated, 3, 2)[3], 128);
        assert!(pixel(&flat, 3, 2)[3] > 128);
        assert_ne!(isolated.pixels, flat.pixels);
    }

    #[test]
    fn clip_and_composite_are_deterministic_and_halo_free() {
        let svg = r##"<svg viewBox="0 0 8 8">
<clipPath id="c"><circle cx="4" cy="4" r="3"/></clipPath>
<g opacity="0.6" clip-path="url(#c)">
  <rect width="8" height="8" fill="#3366cc"/>
  <rect width="8" height="8" fill="#cc3366"/>
</g>
</svg>"##;
        let first = rasterize(svg, 8, 8).unwrap();
        let second = rasterize(svg, 8, 8).unwrap();
        assert_eq!(first.pixels, second.pixels);

        // Fully clipped-out corners stay pure transparent — no color halo leaks
        // from premultiplied compositing.
        assert_eq!(pixel(&first, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&first, 7, 7), [0, 0, 0, 0]);
        assert!(pixel(&first, 4, 4)[3] > 0);
    }

    #[test]
    fn object_bounding_box_clip_on_group_is_diagnosed_not_applied() {
        let svg = r##"<svg viewBox="0 0 4 4">
<clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="0.5" height="1"/></clipPath>
<g clip-path="url(#c)"><rect width="4" height="4" fill="#ff0000"/></g>
</svg>"##;
        let output = rasterize_with_report(svg, 4, 4).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|w| w.code == "clip.object_bounding_box"));
        // With clip skipped the group still renders fully.
        assert_eq!(pixel(&output.image, 3, 3), [255, 0, 0, 255]);
    }

    #[test]
    fn missing_clip_reference_warns_and_keeps_element_visible() {
        let svg = r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000" clip-path="url(#nope)"/></svg>"##;
        let output = rasterize_with_report(svg, 4, 4).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|w| w.code == "clip.unresolved"));
        assert_eq!(pixel(&output.image, 2, 2), [255, 0, 0, 255]);
    }

    #[test]
    fn clip_reference_cycle_is_bounded_and_reported() {
        let svg = r##"<svg viewBox="0 0 4 4">
<clipPath id="a" clip-path="url(#b)"><rect width="4" height="4"/></clipPath>
<clipPath id="b" clip-path="url(#a)"><rect width="2" height="4"/></clipPath>
<rect width="4" height="4" fill="#ff0000" clip-path="url(#a)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 4, 4).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|w| w.code == "reference.clip_cycle"));
    }

    #[test]
    fn clip_depth_limit_is_reported() {
        let mut svg = String::from(r##"<svg viewBox="0 0 4 4">"##);
        let chain = MAX_CLIP_DEPTH + 4;
        for index in 0..chain {
            let next = index + 1;
            svg.push_str(&format!(
                r##"<clipPath id="c{index}" clip-path="url(#c{next})"><rect width="4" height="4"/></clipPath>"##
            ));
        }
        svg.push_str(&format!(
            r##"<clipPath id="c{chain}"><rect width="4" height="4"/></clipPath>"##
        ));
        svg.push_str(r##"<rect width="4" height="4" fill="#ff0000" clip-path="url(#c0)"/></svg>"##);
        let output = rasterize_with_report(&svg, 4, 4).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|w| w.code == "limit.clip_depth"));
    }

    #[test]
    fn isolated_group_offscreen_depth_cap_is_reported() {
        let mut svg = String::from(r##"<svg viewBox="0 0 4 4">"##);
        let depth = MAX_OFFSCREEN_DEPTH + 1;
        for _ in 0..depth {
            svg.push_str(r##"<g opacity="0.5">"##);
        }
        svg.push_str(r##"<rect width="4" height="4" fill="#ff0000"/>"##);
        for _ in 0..depth {
            svg.push_str("</g>");
        }
        svg.push_str("</svg>");
        let output = rasterize_with_report(&svg, 4, 4).unwrap();

        assert!(output
            .report
            .warnings
            .iter()
            .any(|w| w.code == "limit.offscreen_buffer"));
        // Despite truncation the render still produces visible pixels.
        assert!(output.image.pixels.iter().any(|c| c.a() > 0));
    }

    #[test]
    fn empty_clip_path_hides_referencing_element() {
        let svg = r##"<svg viewBox="0 0 4 4">
<clipPath id="c"></clipPath>
<rect width="4" height="4" fill="#ff0000" clip-path="url(#c)"/>
</svg>"##;
        let image = rasterize(svg, 4, 4).unwrap();
        assert!(image.pixels.iter().all(|c| c.a() == 0));
    }

    // --- R5: embedded raster images -----------------------------------------

    // 2x2 PNGs minted with real zlib (level 9 → dynamic-Huffman inflate path).
    const PNG_RGBA_2X2: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAE0lEQVR42mP4z8DwHwyBNIhgAAA/0gX7f+ZqKwAAAABJRU5ErkJggg==";
    const PNG_RGB_2X2: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR42mP4z8DAAMIM/4EAAB/uBfvxq7p3AAAAAElFTkSuQmCC";
    const PNG_PALETTE_2X2: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAMAAABFaP0WAAAADFBMVEX/AAAA/wAAAP8AAAD7vkbkAAAABHRSTlP///8AQCqp9AAAAA5JREFUeNpjYGBkYGIGAAARAAeDymRkAAAAAElFTkSuQmCC";
    const PNG_GRAY_2X2: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAAAAABX3VL4AAAADklEQVR42mNgCGVY9R8AA60B/3qThH8AAAAASUVORK5CYII=";
    const PNG_INTERLACED: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAAEFsT2yAAAAE0lEQVR42mP4z8DwHwyBNIhgAAA/0gX7f+ZqKwAAAABJRU5ErkJggg==";
    const PNG_OVERSIZE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgABhqAAAYagCAYAAACoUgvIAAAACElEQVR42gMAAAAAAW/dyZEAAAAASUVORK5CYII=";

    fn image_svg(href: &str) -> String {
        format!(
            r##"<svg viewBox="0 0 2 2"><image x="0" y="0" width="2" height="2" href="{href}"/></svg>"##
        )
    }

    #[test]
    fn png_rgba_data_uri_renders_pixels() {
        let out = rasterize_with_report(&image_svg(PNG_RGBA_2X2), 2, 2).unwrap();
        assert_eq!(pixel(&out.image, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&out.image, 1, 0), [0, 255, 0, 255]);
        assert_eq!(pixel(&out.image, 0, 1), [0, 0, 255, 255]);
        assert_eq!(pixel(&out.image, 1, 1)[3], 0);
        assert_eq!(out.report.fidelity, SvgRenderFidelity::High);
        assert_eq!(out.report.unsupported_feature_count, 0);
        assert!(!out
            .report
            .warnings
            .iter()
            .any(|w| w.code.starts_with("image.")));
    }

    #[test]
    fn png_rgb_data_uri_renders_opaque() {
        let out = rasterize(&image_svg(PNG_RGB_2X2), 2, 2).unwrap();
        assert_eq!(pixel(&out, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&out, 1, 0), [0, 255, 0, 255]);
        assert_eq!(pixel(&out, 0, 1), [0, 0, 255, 255]);
        assert_eq!(pixel(&out, 1, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn png_palette_with_trns_renders() {
        let out = rasterize(&image_svg(PNG_PALETTE_2X2), 2, 2).unwrap();
        assert_eq!(pixel(&out, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&out, 1, 0), [0, 255, 0, 255]);
        assert_eq!(pixel(&out, 0, 1), [0, 0, 255, 255]);
        assert_eq!(pixel(&out, 1, 1)[3], 0);
    }

    #[test]
    fn png_grayscale_renders() {
        let out = rasterize(&image_svg(PNG_GRAY_2X2), 2, 2).unwrap();
        assert_eq!(pixel(&out, 0, 0), [0, 0, 0, 255]);
        assert_eq!(pixel(&out, 1, 0), [85, 85, 85, 255]);
        assert_eq!(pixel(&out, 0, 1), [170, 170, 170, 255]);
        assert_eq!(pixel(&out, 1, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn image_scales_with_nearest_sampling() {
        let out = rasterize(&image_svg(PNG_RGB_2X2), 4, 4).unwrap();
        assert_eq!(pixel(&out, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&out, 3, 0), [0, 255, 0, 255]);
        assert_eq!(pixel(&out, 0, 3), [0, 0, 255, 255]);
        assert_eq!(pixel(&out, 3, 3), [255, 255, 255, 255]);
    }

    #[test]
    fn embedded_image_is_clipped_by_clip_path() {
        let svg = format!(
            r##"<svg viewBox="0 0 2 2"><clipPath id="c"><rect width="1" height="2"/></clipPath><image x="0" y="0" width="2" height="2" href="{PNG_RGB_2X2}" clip-path="url(#c)"/></svg>"##
        );
        let out = rasterize(&svg, 2, 2).unwrap();
        assert_eq!(pixel(&out, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&out, 1, 0)[3], 0);
    }

    #[test]
    fn embedded_image_opacity_scales_alpha() {
        let svg = format!(
            r##"<svg viewBox="0 0 2 2"><image x="0" y="0" width="2" height="2" opacity="0.5" href="{PNG_RGB_2X2}"/></svg>"##
        );
        let out = rasterize(&svg, 2, 2).unwrap();
        let a = pixel(&out, 0, 0)[3] as i32;
        assert!((a - 128).abs() <= 2, "alpha was {a}");
    }

    #[test]
    fn image_decode_is_deterministic() {
        let a = rasterize(&image_svg(PNG_RGBA_2X2), 2, 2).unwrap();
        let b = rasterize(&image_svg(PNG_RGBA_2X2), 2, 2).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    #[test]
    fn external_href_image_is_rejected_at_document_gate() {
        // A plain external `href` trips the fail-closed document gate (no fetch).
        let svg = r##"<svg viewBox="0 0 2 2"><image x="0" y="0" width="2" height="2" href="pics/logo.png"/></svg>"##;
        assert!(matches!(
            rasterize_with_report(svg, 2, 2),
            Err(SvgRasterError::ForbiddenContent)
        ));
    }

    // Baseline JPEGs minted with ffmpeg's mjpeg encoder (real ground truth).
    const JPEG_RED_444: &str = "data:image/jpeg;base64,/9j/4AAQSkZJRgABAgAAAQABAAD//gAQTGF2YzYyLjI4LjEwMQD/2wBDAAgEBAQEBAUFBQUFBQYGBgYGBgYGBgYGBgYHBwcICAgHBwcGBgcHCAgICAkJCQgICAgJCQoKCgwMCwsODg4RERT/xABNAAEBAAAAAAAAAAAAAAAAAAAABgEBAQEAAAAAAAAAAAAAAAAAAAYHEAEAAAAAAAAAAAAAAAAAAAAAEQEAAAAAAAAAAAAAAAAAAAAA/8AAEQgACAAIAwESAAISAAMSAP/aAAwDAQACEQMRAD8AixJjfx//2Q==";
    const JPEG_RB_420: &str = "data:image/jpeg;base64,/9j/4AAQSkZJRgABAgAAAQABAAD//gAQTGF2YzYyLjI4LjEwMQD/2wBDAAgEBAQEBAUFBQUFBQYGBgYGBgYGBgYGBgYHBwcICAgHBwcGBgcHCAgICAkJCQgICAgJCQoKCgwMCwsODg4RERT/xABOAAEAAAAAAAAAAAAAAAAAAAAGAQEAAAAAAAAAAAAAAAAAAAAGEAEAAAAAAAAAAAAAAAAAAAAAEQADAQEAAAAAAAAAAAAAAAAACEbEhf/AABEIABAAEAMBIgACEQADEQD/2gAMAwEAAhEDEQA/ABYgXiBUslXx941YaZ6uI//Z";
    // Hand-encoded flat-128 single-component (grayscale) baseline JPEG.
    const JPEG_GRAY: &str = "data:image/jpeg;base64,/9j/2wBDAAEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQH/wAALCAAIAAgBAREA/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/9oACAEBAAA/ACv/2Q==";

    fn sized_image_svg(href: &str, w: u32, h: u32) -> String {
        format!(
            r##"<svg viewBox="0 0 {w} {h}"><image x="0" y="0" width="{w}" height="{h}" href="{href}"/></svg>"##
        )
    }

    fn near(actual: u8, expected: u8, tol: i32) -> bool {
        (actual as i32 - expected as i32).abs() <= tol
    }

    #[test]
    fn jpeg_baseline_444_renders_color() {
        let out = rasterize_with_report(&sized_image_svg(JPEG_RED_444, 8, 8), 8, 8).unwrap();
        let p = pixel(&out.image, 4, 4);
        assert!(
            near(p[0], 255, 12) && near(p[1], 0, 12) && near(p[2], 0, 12),
            "px {p:?}"
        );
        assert_eq!(p[3], 255);
        assert_eq!(out.report.fidelity, SvgRenderFidelity::High);
        assert_eq!(out.report.unsupported_feature_count, 0);
    }

    #[test]
    fn jpeg_chroma_subsampled_420_decodes_two_regions() {
        let out = rasterize(&sized_image_svg(JPEG_RB_420, 16, 16), 16, 16).unwrap();
        let left = pixel(&out, 2, 8);
        let right = pixel(&out, 13, 8);
        assert!(
            near(left[0], 255, 24) && near(left[2], 0, 32),
            "left {left:?}"
        );
        assert!(
            near(right[2], 255, 24) && near(right[0], 0, 32),
            "right {right:?}"
        );
    }

    #[test]
    fn jpeg_grayscale_single_component_renders() {
        // True 1-component JPEG: every channel equals the luma, exactly 128 here.
        let out = rasterize(&sized_image_svg(JPEG_GRAY, 8, 8), 8, 8).unwrap();
        let p = pixel(&out, 4, 4);
        assert_eq!(p, [128, 128, 128, 255], "grayscale px {p:?}");
    }

    #[test]
    fn jpeg_decode_is_deterministic() {
        let a = rasterize(&sized_image_svg(JPEG_RED_444, 8, 8), 8, 8).unwrap();
        let b = rasterize(&sized_image_svg(JPEG_RED_444, 8, 8), 8, 8).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    /// Build a minimal valid SOF2 progressive JPEG: 8×8 grayscale, DC-only scan,
    /// all DC diffs = 0, so every pixel decodes to 128 (grey).
    fn make_progressive_gray_jpeg() -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[0xFF, 0xD8]); // SOI
                                            // DQT: 8-bit, table 0, all values = 1
        v.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
        v.extend(std::iter::repeat_n(1u8, 64));
        // SOF2: precision=8, 8×8, 1 component (h1v1, qt=0)
        v.extend_from_slice(&[
            0xFF, 0xC2, 0x00, 0x0B, 0x08, 0x00, 0x08, 0x00, 0x08, 0x01, 0x01, 0x11, 0x00,
        ]);
        // DHT: DC table 0 — single code of length 1 (0b0) for category 0
        v.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x14, 0x00]);
        v.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        v.push(0x00); // huffval: category 0
                      // SOS: DC-only scan, Ss=0 Se=0 Ah=0 Al=0
        v.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00]);
        // Entropy: 1-bit code 0b0 padded to byte → 0x7F
        v.push(0x7F);
        v.extend_from_slice(&[0xFF, 0xD9]); // EOI
        v
    }

    /// Build a minimal valid SOF0 CMYK JPEG: 8×8, 4-component, all DC=0 (samples→128).
    /// CMYK(128,128,128,128) → R≈G≈B≈63.
    fn make_cmyk_baseline_jpeg() -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(&[0xFF, 0xD8]); // SOI
        v.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
        v.extend(std::iter::repeat_n(1u8, 64));
        // SOF0: 8×8, 4 components (ids 1–4, h1v1, qt=0)
        v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x14, 0x08, 0x00, 0x08, 0x00, 0x08, 0x04]);
        v.extend_from_slice(&[
            0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00, 0x04, 0x11, 0x00,
        ]);
        // DHT: DC table 0 (same single-symbol table)
        v.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x14, 0x00]);
        v.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        v.push(0x00);
        // DHT: AC table 0 — single code of length 1 (0b0) for EOB (0x00)
        v.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x14, 0x10]);
        v.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        v.push(0x00);
        // SOS: 4 components, Ss=0 Se=63 Ah=0 Al=0
        v.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x0E, 0x04]);
        v.extend_from_slice(&[0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00]);
        v.extend_from_slice(&[0x00, 0x3F, 0x00]);
        // Entropy: 4 × (1 DC bit + 1 AC-EOB bit) = 8 bits = 0x00
        v.push(0x00);
        v.extend_from_slice(&[0xFF, 0xD9]); // EOI
        v
    }

    #[test]
    fn progressive_jpeg_decodes_to_gray_pixels() {
        let bytes = make_progressive_gray_jpeg();
        let img = decode_jpeg(&bytes).unwrap();
        assert_eq!(img.width, 8);
        assert_eq!(img.height, 8);
        // DC=0 after dequant → IDCT → all spatial = 0 → +128 = 128 gray
        assert_eq!(
            &img.rgba[0..4],
            &[128, 128, 128, 255],
            "center pixel should be gray-128"
        );
    }

    #[test]
    fn cmyk_baseline_jpeg_decodes_to_dark_gray() {
        let bytes = make_cmyk_baseline_jpeg();
        let img = decode_jpeg(&bytes).unwrap();
        assert_eq!(img.width, 8);
        assert_eq!(img.height, 8);
        // CMYK(128,128,128,128) → (255-128)*(255-128)/255 = 127*127/255 = 63
        let p = &img.rgba[0..4];
        assert_eq!(p[0], 63, "CMYK R channel");
        assert_eq!(p[1], 63, "CMYK G channel");
        assert_eq!(p[2], 63, "CMYK B channel");
        assert_eq!(p[3], 255);
    }

    #[test]
    fn progressive_jpeg_truncated_sof2_is_malformed_not_unsupported() {
        // SOF2 with a zero-length body → MalformedJpeg, not UnsupportedJpeg.
        assert!(matches!(
            decode_jpeg(&[0xFF, 0xD8, 0xFF, 0xC2, 0x00, 0x02]),
            Err(ImageDecodeError::MalformedJpeg)
        ));
    }

    #[test]
    fn malformed_jpeg_is_diagnosed_not_panicking() {
        // A bogus SOF segment length is a bounded error.
        assert!(decode_jpeg(&[0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0xFF]).is_err());
        // A truncated real JPEG must not panic (lenient entropy decode is allowed).
        let payload = JPEG_RED_444
            .strip_prefix("data:image/jpeg;base64,")
            .unwrap();
        let bytes = base64_decode(&payload[..payload.len() / 2]).unwrap();
        let _ = decode_jpeg(&bytes);
    }

    #[test]
    fn interlaced_png_is_diagnosed_unsupported() {
        let out = rasterize_with_report(&image_svg(PNG_INTERLACED), 2, 2).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "image.unsupported_png"));
    }

    #[test]
    fn oversized_png_dimensions_are_bounded() {
        let out = rasterize_with_report(&image_svg(PNG_OVERSIZE), 2, 2).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "limit.image_pixels"));
    }

    #[test]
    fn truncated_png_is_diagnosed_not_panicking() {
        let truncated = &PNG_RGBA_2X2[..PNG_RGBA_2X2.len() - 16];
        let out = rasterize_with_report(&image_svg(truncated), 2, 2).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code.starts_with("image.")));
    }

    #[test]
    fn inflate_handles_stored_blocks() {
        // BFINAL=1, BTYPE=00 → 0x01; LEN=5, NLEN=~5; literal "hello".
        let data = [0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(inflate(&data, 64).as_deref(), Some(&b"hello"[..]));
    }

    // --- R7: masks + filters ------------------------------------------------

    #[test]
    fn alpha_mask_uses_mask_alpha_channel() {
        // mask-type:alpha with 50% black content → element drops to ~50% alpha.
        let svg = r##"<svg viewBox="0 0 4 4"><mask id="m" mask-type="alpha"><rect width="4" height="4" fill="#000000" fill-opacity="0.5"/></mask><rect width="4" height="4" fill="#ff0000" mask="url(#m)"/></svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        let p = pixel(&out.image, 2, 2);
        assert!((p[3] as i32 - 128).abs() <= 4, "alpha {p:?}");
        assert!(p[0] > 0, "still red {p:?}");
        assert!(!out
            .report
            .unsupported_features
            .iter()
            .any(|f| f.feature == "mask"));
    }

    #[test]
    fn missing_mask_reference_is_diagnosed_and_element_visible() {
        let svg = r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000" mask="url(#nope)"/></svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "mask.unresolved"));
        // No mask applied → element renders fully.
        assert_eq!(pixel(&out.image, 2, 2), [255, 0, 0, 255]);
    }

    #[test]
    fn gaussian_blur_softens_a_hard_edge() {
        // Explicit region so the blur halo survives R10 filter-region clipping.
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f" x="-25%" y="-25%" width="150%" height="150%"><feGaussianBlur stdDeviation="1.2"/></filter><rect x="2" y="2" width="4" height="4" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let out = rasterize(svg, 8, 8).unwrap();
        // Blur bleeds partial alpha outside the original sharp 4x4 rect.
        assert!(out.pixels.iter().any(|c| c.a() > 0 && c.a() < 255));
        // And bleeds into a pixel that was empty before the blur.
        assert!(pixel(&out, 1, 4)[3] > 0);
    }

    #[test]
    fn fecolormatrix_saturate_zero_grayscales() {
        let svg = r##"<svg viewBox="0 0 4 4"><filter id="f"><feColorMatrix type="saturate" values="0"/></filter><rect width="4" height="4" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let out = rasterize(svg, 4, 4).unwrap();
        let p = pixel(&out, 2, 2);
        assert_eq!(p[0], p[1], "grayscale r==g {p:?}");
        assert_eq!(p[1], p[2], "grayscale g==b {p:?}");
        assert!(p[0] > 0 && p[3] == 255, "non-empty {p:?}");
    }

    #[test]
    fn fedropshadow_adds_offset_shadow() {
        // Explicit region so the offset shadow survives R10 region clipping.
        let svg = r##"<svg viewBox="0 0 6 6"><filter id="f" x="0" y="0" width="250%" height="250%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-color="#000000"/></filter><rect width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let out = rasterize(svg, 6, 6).unwrap();
        // Source stays red at the origin; a dark shadow appears at the offset.
        assert_eq!(pixel(&out, 0, 0), [255, 0, 0, 255]);
        let shadow = pixel(&out, 3, 3);
        assert!(shadow[3] > 0 && shadow[0] < 64, "shadow {shadow:?}");
    }

    #[test]
    fn unsupported_filter_primitive_is_partial_with_diagnostic() {
        // Use a truly unrecognised element name so the catch-all fires.
        let svg = r##"<svg viewBox="0 0 4 4"><filter id="f"><feUnknownXYZ/></filter><rect width="4" height="4" fill="#00ff00" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        assert!(
            out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "unknown filter element must produce unsupported_primitive diagnostic"
        );
        // Partial output: the source still renders (identity passthrough).
        assert!(out.image.pixels.iter().any(|c| c.a() > 0));
    }

    #[test]
    fn feturbulence_generates_non_transparent_output() {
        // feTurbulence is now a real implementation — verify it generates noise pixels.
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f"><feTurbulence baseFrequency="0.05" numOctaves="2"/></filter><rect width="8" height="8" fill="#000000" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        // Should NOT produce an unsupported_primitive diagnostic.
        assert!(
            !out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "feTurbulence must not fall through to unsupported catch-all"
        );
        // Output should have some non-zero pixels from the noise.
        assert!(
            out.image.pixels.iter().any(|p| p.r() > 0 || p.g() > 0),
            "feTurbulence output should contain non-zero noise pixels"
        );
    }

    #[test]
    fn fetile_repeats_input_pixels() {
        // A simple feTile on a 2x2 flood should tile to fill the region.
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f" x="0" y="0" width="100%" height="100%" filterUnits="userSpaceOnUse"><feFlood flood-color="#ff0000" result="r"/><feTile in="r"/></filter><rect width="8" height="8" fill="#0000ff" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(
            !out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "feTile must not fall through to unsupported catch-all"
        );
        // feTile with feFlood input should produce red pixels.
        assert!(
            out.image.pixels.iter().any(|p| p.r() > 0),
            "feTile of a red flood should produce red pixels"
        );
    }

    #[test]
    fn fedisplacementmap_accepts_without_unsupported_diagnostic() {
        // feDisplacementMap with SourceGraphic as map: verify it parses and runs.
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f"><feDisplacementMap scale="5" xChannelSelector="R" yChannelSelector="G" in2="SourceGraphic"/></filter><rect width="8" height="8" fill="#ff8800" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(
            !out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "feDisplacementMap must not fall through to unsupported"
        );
    }

    #[test]
    fn feconvolvematrix_identity_kernel_preserves_source() {
        // A 3x3 identity kernel (centre=1, rest=0) should preserve the source.
        let svg = r##"<svg viewBox="0 0 4 4"><filter id="f"><feConvolveMatrix order="3" kernelMatrix="0 0 0 0 1 0 0 0 0"/></filter><rect width="4" height="4" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        assert!(
            !out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "feConvolveMatrix must not fall through to unsupported"
        );
        // Output should be red (identity kernel).
        let c = &out.image.pixels[out.image.pixels.len() / 2];
        assert!(
            c.r() > 128,
            "identity convolution should preserve red, got {c:?}"
        );
    }

    #[test]
    fn fediffuselighting_accepts_without_unsupported_diagnostic() {
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f"><feDiffuseLighting lighting-color="white" diffuseConstant="1" surfaceScale="4"><feDistantLight azimuth="45" elevation="60"/></feDiffuseLighting></filter><rect width="8" height="8" fill="#888" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(
            !out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "feDiffuseLighting must not fall through to unsupported"
        );
    }

    #[test]
    fn fespecularlighting_accepts_without_unsupported_diagnostic() {
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f"><feSpecularLighting lighting-color="white" specularConstant="1" specularExponent="20" surfaceScale="4"><fePointLight x="4" y="4" z="20"/></feSpecularLighting></filter><rect width="8" height="8" fill="#888" filter="url(#f)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(
            !out.report
                .warnings
                .iter()
                .any(|w| w.code == "filter.unsupported_primitive"),
            "feSpecularLighting must not fall through to unsupported"
        );
    }

    #[test]
    fn huge_blur_is_bounded_not_a_bomb() {
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f"><feGaussianBlur stdDeviation="100000"/></filter><rect width="8" height="8" fill="#ff0000" filter="url(#f)"/></svg>"##;
        // Must complete (radius is capped) and not panic.
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert_eq!(out.report.fidelity, SvgRenderFidelity::High);
    }

    #[test]
    fn mask_and_filter_render_deterministically() {
        let svg = r##"<svg viewBox="0 0 6 6"><mask id="m"><rect width="3" height="6" fill="#fff"/></mask><filter id="f"><feGaussianBlur stdDeviation="0.8"/></filter><rect width="6" height="6" fill="#0000ff" mask="url(#m)" filter="url(#f)"/></svg>"##;
        let a = rasterize(svg, 6, 6).unwrap();
        let b = rasterize(svg, 6, 6).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    // --- R8: conformance / benchmark harness --------------------------------

    /// Build a multi-feature SVG with `n` gradient-filled, clipped, stroked rects.
    fn benchmark_svg(n: usize) -> String {
        let mut s = String::from(
            r##"<svg viewBox="0 0 256 256"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><clipPath id="c"><rect width="256" height="200"/></clipPath></defs>"##,
        );
        for i in 0..n {
            let x = (i * 7) % 240;
            let y = (i * 11) % 240;
            s.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="16" height="16" fill="url(#g)" stroke="#000" stroke-width="1" clip-path="url(#c)"/>"##
            ));
        }
        s.push_str("</svg>");
        s
    }

    // Parse/scene-build/raster/peak-alloc budgets + methodology:
    // docs/SVG_PRECISION_AND_BENCH.md (measure-not-gate; budgets are targets).
    #[test]
    #[ignore = "perf benchmark; run with --ignored to measure parse+scene+raster time."]
    fn raster_benchmark_complex_scene_within_budget() {
        let svg = benchmark_svg(200);
        let start = std::time::Instant::now();
        let out = rasterize_with_report(&svg, 256, 256).expect("benchmark scene renders");
        let elapsed = start.elapsed();
        eprintln!("raster_benchmark: 200 gradient/clip/stroke rects @256px in {elapsed:?} (debug)");
        // Produced pixels and stayed bounded; generous guard catches only hangs
        // (debug builds are slow — this measures, it does not gate fidelity).
        assert!(out.image.pixels.iter().any(|c| c.a() > 0));
        assert!(
            elapsed.as_secs_f64() < 30.0,
            "raster benchmark unexpectedly slow: {elapsed:?}"
        );
    }

    /// Dev-only reference-oracle workflow. Comparison against external reference
    /// renderers is a CI-artifact / developer-only step and MUST NOT become a
    /// runtime or Cargo dependency (zero-dependency policy). As an in-repo
    /// stand-in this asserts the renderer is deterministic for a representative
    /// multi-feature scene so any external oracle diff is reproducible.
    #[test]
    #[ignore = "dev-only reference-oracle workflow; external renderers are CI artifacts, never runtime deps"]
    fn reference_oracle_scene_is_deterministic() {
        let svg = benchmark_svg(64);
        let a = rasterize(&svg, 256, 256).unwrap();
        let b = rasterize(&svg, 256, 256).unwrap();
        assert_eq!(a.pixels, b.pixels);
    }

    // --- R9: vector-effect non-scaling-stroke -------------------------------

    /// Count image columns containing at least one opaque pixel.
    fn opaque_columns(img: &egui::ColorImage) -> usize {
        let [w, h] = img.size;
        (0..w)
            .filter(|&x| (0..h).any(|y| img.pixels[y * w + x].a() > 0))
            .count()
    }

    #[test]
    fn non_scaling_stroke_keeps_device_width_constant() {
        // Same geometry under a 4x group scale: the plain stroke scales with the
        // CTM (~8px wide); non-scaling-stroke stays ~2px in device space.
        let nss = r##"<svg viewBox="0 0 16 16"><g transform="scale(4)"><line x1="2" y1="0" x2="2" y2="4" stroke="#000000" stroke-width="2" vector-effect="non-scaling-stroke"/></g></svg>"##;
        let plain = r##"<svg viewBox="0 0 16 16"><g transform="scale(4)"><line x1="2" y1="0" x2="2" y2="4" stroke="#000000" stroke-width="2"/></g></svg>"##;
        let nss_cols = opaque_columns(&rasterize(nss, 16, 16).unwrap());
        let plain_cols = opaque_columns(&rasterize(plain, 16, 16).unwrap());
        assert!(
            plain_cols >= nss_cols + 3,
            "non-scaling-stroke must stay narrow: nss={nss_cols} plain={plain_cols}"
        );
        assert!(
            (1..=4).contains(&nss_cols),
            "non-scaling device width bounded near 2px: {nss_cols}"
        );
    }

    #[test]
    fn unsupported_vector_effect_value_is_diagnosed() {
        let svg = r##"<svg viewBox="0 0 8 8"><line x1="0" y1="4" x2="8" y2="4" stroke="#000000" stroke-width="2" vector-effect="fixed-position"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(
            out.report
                .warnings
                .iter()
                .any(|w| w.code == "vector_effect.unsupported"),
            "unsupported vector-effect value must be diagnosed"
        );
    }

    // --- R9: markers ---------------------------------------------------------

    #[test]
    fn markers_render_on_start_mid_end_vertices() {
        // 2x2 red markers centred (refX/refY=1) on every vertex of a 3-point
        // polyline; check a red pixel lands at each vertex.
        let svg = r##"<svg viewBox="0 0 12 4"><defs><marker id="m" markerWidth="2" markerHeight="2" refX="1" refY="1" markerUnits="userSpaceOnUse"><rect width="2" height="2" fill="#ff0000"/></marker></defs><polyline points="1,2 6,2 11,2" fill="none" stroke="#0000ff" stroke-width="1" marker-start="url(#m)" marker-mid="url(#m)" marker-end="url(#m)"/></svg>"##;
        let out = rasterize_with_report(svg, 12, 4).unwrap();
        for vx in [1usize, 6, 11] {
            assert_eq!(
                pixel(&out.image, vx.min(11), 1),
                [255, 0, 0, 255],
                "marker missing at vertex x={vx}"
            );
        }
        assert!(out
            .report
            .warnings
            .iter()
            .all(|w| w.code != "marker.unresolved"));
    }

    #[test]
    fn auto_orient_marker_renders_and_is_deterministic() {
        let svg = r##"<svg viewBox="0 0 8 8"><defs><marker id="a" markerWidth="3" markerHeight="3" refX="0" refY="1.5" orient="auto" markerUnits="userSpaceOnUse"><path d="M0 0 L3 1.5 L0 3 Z" fill="#00ff00"/></marker></defs><line x1="1" y1="4" x2="5" y2="4" stroke="#000000" stroke-width="1" marker-end="url(#a)"/></svg>"##;
        let a = rasterize(svg, 8, 8).unwrap();
        let b = rasterize(svg, 8, 8).unwrap();
        assert_eq!(a.pixels, b.pixels, "marker render must be deterministic");
        // The green arrowhead tip lands near the line end (x≈5, y≈4).
        assert_eq!(pixel(&a, 5, 4), [0, 255, 0, 255]);
    }

    #[test]
    fn missing_marker_reference_is_diagnosed_not_fatal() {
        let svg = r##"<svg viewBox="0 0 8 8"><line x1="0" y1="4" x2="8" y2="4" stroke="#0000ff" stroke-width="2" marker-end="url(#nope)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "marker.unresolved"));
        // The line itself still renders.
        assert!(out.report.rendered_element_count >= 1);
    }

    #[test]
    fn marker_units_userspaceonuse_ignores_stroke_width() {
        // userSpaceOnUse markers are NOT scaled by stroke-width; a 2x2 marker
        // stays 2 device px wide regardless of a thick stroke.
        let svg = r##"<svg viewBox="0 0 8 8"><defs><marker id="m" markerWidth="2" markerHeight="2" refX="0" refY="0" markerUnits="userSpaceOnUse"><rect width="2" height="2" fill="#ff0000"/></marker></defs><line x1="1" y1="1" x2="6" y2="1" stroke="#0000ff" stroke-width="4" marker-start="url(#m)"/></svg>"##;
        let out = rasterize(svg, 8, 8).unwrap();
        // Marker placed at (1,1), spanning device x 1..3, y 1..3 (2px, not 8px).
        assert_eq!(pixel(&out, 1, 1), [255, 0, 0, 255]);
        assert_eq!(pixel(&out, 2, 2), [255, 0, 0, 255]);
        assert_eq!(pixel(&out, 4, 1), [0, 0, 255, 255]); // beyond marker → blue stroke
    }

    // --- R9: pattern robustness ----------------------------------------------

    #[test]
    fn pattern_href_cycle_is_bounded_and_diagnosed() {
        let svg = r##"<svg viewBox="0 0 8 8"><defs>
<pattern id="a" href="#b" width="2" height="2" patternUnits="userSpaceOnUse"/>
<pattern id="b" href="#a" width="2" height="2" patternUnits="userSpaceOnUse"><rect width="1" height="1" fill="#ff0000"/></pattern>
</defs><rect width="8" height="8" fill="url(#a)"/></svg>"##;
        let out = rasterize_with_report(svg, 8, 8).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "reference.pattern_cycle"));
    }

    #[test]
    fn self_referential_pattern_content_terminates() {
        // A pattern whose own content paints with itself must not recurse
        // forever — the inner reference is dropped to transparent.
        let svg = r##"<svg viewBox="0 0 8 8"><defs>
<pattern id="p" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="4" height="4" fill="url(#p)"/></pattern>
</defs><rect width="8" height="8" fill="url(#p)"/></svg>"##;
        let out = rasterize(svg, 8, 8).unwrap();
        // Fully transparent (the only content paints with the removed pattern).
        assert_eq!(pixel(&out, 4, 4), [0, 0, 0, 0]);
    }

    #[test]
    fn oversized_pattern_tile_is_capped_without_panic() {
        // A huge tile under a large scale would blow memory if uncapped; the
        // tile pixel budget clamps it and the render still completes.
        let svg = r##"<svg viewBox="0 0 64 64"><defs>
<pattern id="p" width="5000" height="5000" patternUnits="userSpaceOnUse"><rect width="2500" height="2500" fill="#00ff00"/></pattern>
</defs><rect width="64" height="64" fill="url(#p)"/></svg>"##;
        let out = rasterize(svg, 64, 64).unwrap();
        assert_eq!(out.size, [64, 64]);
    }

    #[test]
    fn pattern_object_bounding_box_tiles_render() {
        let svg = r##"<svg viewBox="0 0 4 4"><defs>
<pattern id="p" width="0.5" height="0.5" patternUnits="objectBoundingBox" patternContentUnits="objectBoundingBox"><rect width="0.25" height="0.5" fill="#0000ff"/></pattern>
</defs><rect width="4" height="4" fill="url(#p)"/></svg>"##;
        let out = rasterize(svg, 4, 4).unwrap();
        assert_eq!(pixel(&out, 0, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(&out, 1, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&out, 2, 0), [0, 0, 255, 255]);
    }

    // --- R10: tier-2 filter primitives --------------------------------------

    #[test]
    fn feblend_multiply_and_screen_differ_and_are_deterministic() {
        let multiply = r##"<svg viewBox="0 0 4 4"><filter id="f"><feFlood flood-color="#00ff00" result="b"/><feBlend mode="multiply" in="SourceGraphic" in2="b"/></filter><rect width="4" height="4" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let screen = r##"<svg viewBox="0 0 4 4"><filter id="f"><feFlood flood-color="#00ff00" result="b"/><feBlend mode="screen" in="SourceGraphic" in2="b"/></filter><rect width="4" height="4" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let m1 = rasterize(multiply, 4, 4).unwrap();
        let m2 = rasterize(multiply, 4, 4).unwrap();
        assert_eq!(m1.pixels, m2.pixels, "feBlend must be deterministic");
        // multiply(red, green) = black; screen(red, green) = yellow.
        assert_eq!(pixel(&m1, 0, 0), [0, 0, 0, 255]);
        let s = rasterize(screen, 4, 4).unwrap();
        assert_eq!(pixel(&s, 0, 0), [255, 255, 0, 255]);
    }

    #[test]
    fn fecomposite_arithmetic_and_porterduff_render() {
        // arithmetic add of red over green flood = yellow.
        let add = r##"<svg viewBox="0 0 2 2"><filter id="f"><feFlood flood-color="#00ff00" result="g"/><feComposite operator="arithmetic" k1="0" k2="1" k3="1" k4="0" in="SourceGraphic" in2="g"/></filter><rect width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##;
        assert_eq!(
            pixel(&rasterize(add, 2, 2).unwrap(), 0, 0),
            [255, 255, 0, 255]
        );
        // operator="in" keeps source only where the backdrop (flood) is present.
        let op_in = r##"<svg viewBox="0 0 2 2"><filter id="f"><feFlood flood-color="#0000ff" result="g"/><feComposite operator="in" in="SourceGraphic" in2="g"/></filter><rect width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##;
        assert_eq!(
            pixel(&rasterize(op_in, 2, 2).unwrap(), 0, 0),
            [255, 0, 0, 255]
        );
    }

    #[test]
    fn fecomponent_transfer_gamma_and_linear_are_applied() {
        // gamma exponent=2 on a mid grey (0.5) -> 0.25.
        assert_eq!(
            TransferFunc::Gamma {
                amplitude: 1.0,
                exponent: 2.0,
                offset: 0.0,
            }
            .apply(0.5),
            0.25
        );
        // linear slope=0 intercept=1 forces the channel to full.
        assert_eq!(
            TransferFunc::Linear {
                slope: 0.0,
                intercept: 1.0,
            }
            .apply(0.0),
            1.0
        );
        // table inversion renders (red -> cyan) end to end.
        let svg = r##"<svg viewBox="0 0 2 2"><filter id="f"><feComponentTransfer><feFuncR type="table" tableValues="1 0"/></feComponentTransfer></filter><rect width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##;
        assert_eq!(pixel(&rasterize(svg, 2, 2).unwrap(), 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn femorphology_dilate_grows_and_huge_radius_is_bounded() {
        let svg = r##"<svg viewBox="0 0 8 8"><filter id="f" x="-50%" y="-50%" width="200%" height="200%"><feMorphology operator="dilate" radius="1"/></filter><rect x="3" y="3" width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let out = rasterize(svg, 8, 8).unwrap();
        // Dilation reaches the pixel just outside the original 2x2 square.
        assert_eq!(pixel(&out, 2, 2), [255, 0, 0, 255]);
        // A pathological radius must complete (capped) without panicking.
        let bomb = r##"<svg viewBox="0 0 8 8"><filter id="f"><feMorphology operator="dilate" radius="100000"/></filter><rect width="8" height="8" fill="#ff0000" filter="url(#f)"/></svg>"##;
        assert_eq!(rasterize(bomb, 8, 8).unwrap().size, [8, 8]);
    }

    #[test]
    fn color_interpolation_filters_linear_default_lightens_blur_midpoint() {
        // Blur a black|white seam. In linearRGB (default) the blurred midpoint is
        // lighter than in sRGB, because averaging happens in linear light.
        let linear = r##"<svg viewBox="0 0 8 4"><filter id="f"><feGaussianBlur stdDeviation="1.2"/></filter><g filter="url(#f)"><rect width="4" height="4" fill="#000000"/><rect x="4" width="4" height="4" fill="#ffffff"/></g></svg>"##;
        let srgb = r##"<svg viewBox="0 0 8 4"><filter id="f" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="1.2"/></filter><g filter="url(#f)"><rect width="4" height="4" fill="#000000"/><rect x="4" width="4" height="4" fill="#ffffff"/></g></svg>"##;
        let lin = rasterize(linear, 8, 4).unwrap();
        let srg = rasterize(srgb, 8, 4).unwrap();
        // Same seam pixel, just-left-of-centre (still inside the black half).
        let lp = pixel(&lin, 3, 2);
        let sp = pixel(&srg, 3, 2);
        assert!(
            lp[0] > sp[0] + 10,
            "linearRGB blur must lighten the midpoint vs sRGB: linear={lp:?} srgb={sp:?}"
        );
        // Determinism.
        assert_eq!(lin.pixels, rasterize(linear, 8, 4).unwrap().pixels);
    }

    #[test]
    fn filter_region_clips_default_and_userspace() {
        // Default objectBoundingBox region clips an feFlood to the bbox+margin,
        // not the whole canvas.
        let default_region = r##"<svg viewBox="0 0 6 6"><filter id="f"><feFlood flood-color="#0000ff"/></filter><rect x="2" y="2" width="2" height="2" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let out = rasterize(default_region, 6, 6).unwrap();
        assert_eq!(pixel(&out, 2, 2), [0, 0, 255, 255]); // inside region
        assert_eq!(pixel(&out, 0, 0), [0, 0, 0, 0]); // corner clipped out
                                                     // userSpaceOnUse region with an explicit rect bounds the flood exactly.
        let user = r##"<svg viewBox="0 0 6 6"><filter id="f" filterUnits="userSpaceOnUse" x="1" y="1" width="2" height="2"><feFlood flood-color="#0000ff"/></filter><rect width="6" height="6" fill="#ff0000" filter="url(#f)"/></svg>"##;
        let u = rasterize(user, 6, 6).unwrap();
        assert_eq!(pixel(&u, 1, 1), [0, 0, 255, 255]); // inside [1,3)
        assert_eq!(pixel(&u, 4, 4), [0, 0, 0, 0]); // outside the explicit region
    }

    // --- R11: raster text ----------------------------------------------------

    #[test]
    fn raster_text_renders_deterministically_and_reports_snapshot() {
        let svg = r##"<svg viewBox="0 0 16 16"><text x="1" y="12" font-size="12" fill="#ff0000">Hi</text></svg>"##;
        let a = rasterize_with_report(svg, 16, 16).unwrap();
        let b = rasterize_with_report(svg, 16, 16).unwrap();
        assert_eq!(a.image.pixels, b.image.pixels, "text render deterministic");
        // Pixels actually land (H left stem near x=2).
        assert!(a.image.pixels.iter().any(|c| c.r() > 200 && c.a() > 200));
        assert!(a
            .report
            .warnings
            .iter()
            .any(|w| w.code == "text.raster_snapshot"));
        assert_eq!(a.report.rendered_element_count, 1);
        // No "text" unsupported bucket anymore.
        assert!(!a
            .report
            .unsupported_features
            .iter()
            .any(|f| f.feature == "text"));
    }

    #[test]
    fn unknown_glyphs_render_tofu_with_diagnostic() {
        let svg = r##"<svg viewBox="0 0 24 24"><text x="2" y="18" font-size="16" fill="#000000">日</text></svg>"##;
        let out = rasterize_with_report(svg, 24, 24).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "text.glyph_unsupported"));
        // The tofu box paints something.
        assert!(out.image.pixels.iter().any(|c| c.a() > 200));
    }

    #[test]
    fn bidi_text_is_diagnosed_not_silently_wrong() {
        let svg = r##"<svg viewBox="0 0 24 24"><text x="2" y="18" font-size="16" fill="#000000">ש</text></svg>"##;
        let out = rasterize_with_report(svg, 24, 24).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "text.bidi_unsupported"));
    }

    #[test]
    fn tspan_runs_offset_and_unresolved_textpath_is_diagnosed() {
        // dy-shifted tspan lands lower than the base run.
        let svg = r##"<svg viewBox="0 0 32 32"><text x="2" y="10" font-size="10" fill="#ff0000">l<tspan dy="12">l</tspan></text></svg>"##;
        let out = rasterize(svg, 32, 32).unwrap();
        // Threshold 50: the 0.67px glyph stroke can straddle a pixel boundary
        // and split its anti-aliased coverage across two columns.
        let inked_rows: Vec<usize> = (0..32)
            .filter(|&y| (0..32).any(|x| pixel(&out, x, y)[3] > 50))
            .collect();
        // Two stems: one ending near y=10, one near y=22.
        assert!(inked_rows.iter().any(|&y| y < 11), "rows: {inked_rows:?}");
        assert!(inked_rows.iter().any(|&y| y > 16), "rows: {inked_rows:?}");

        let missing = r##"<svg viewBox="0 0 16 16"><text font-size="10"><textPath href="#nope">x</textPath></text></svg>"##;
        let rep = rasterize_with_report(missing, 16, 16).unwrap();
        assert!(rep
            .report
            .warnings
            .iter()
            .any(|w| w.code == "textpath.unresolved"));
    }

    #[test]
    fn text_glyph_limit_is_bounded_with_diagnostic() {
        let long: String = "x".repeat(MAX_TEXT_GLYPHS + 50);
        let svg = format!(
            r##"<svg viewBox="0 0 64 64"><text x="1" y="32" font-size="8">{long}</text></svg>"##
        );
        let out = rasterize_with_report(&svg, 64, 64).unwrap();
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "limit.text_glyphs"));
    }

    #[test]
    fn mix_blend_mode_group_blends_with_backdrop() {
        // Green group with mix-blend-mode:multiply over a red backdrop -> black.
        let blended = r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000"/><g style="mix-blend-mode: multiply"><rect width="4" height="4" fill="#00ff00"/></g></svg>"##;
        let out = rasterize(blended, 4, 4).unwrap();
        assert_eq!(pixel(&out, 1, 1), [0, 0, 0, 255]);
        // Without the blend, the green group is plain src-over -> green wins.
        let normal = r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000"/><g><rect width="4" height="4" fill="#00ff00"/></g></svg>"##;
        assert_eq!(
            pixel(&rasterize(normal, 4, 4).unwrap(), 1, 1),
            [0, 255, 0, 255]
        );
    }

    // --- R12: namespace model + malformed recovery + a11y metadata ----------

    #[test]
    fn foreign_namespace_element_is_skipped_not_misrendered() {
        // A custom-namespace <c:rect> must NOT render as an SVG <rect>; the real
        // svg rect still renders, and xlink:href on <use> still resolves.
        let svg = r##"<svg viewBox="0 0 10 10" xmlns:c="urn:custom">
<c:rect width="10" height="10" fill="#ff0000"/>
<rect x="0" y="0" width="4" height="4" fill="#00ff00"/>
</svg>"##;
        let out = rasterize_with_report(svg, 10, 10).unwrap();
        // green rect rendered...
        assert_eq!(pixel(&out.image, 1, 1), [0, 255, 0, 255]);
        // ...but the foreign c:rect did not paint red over the rest.
        assert_eq!(pixel(&out.image, 8, 8), [0, 0, 0, 0]);
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "namespace.foreign_element"));
    }

    #[test]
    fn xlink_href_use_still_resolves() {
        let svg = r##"<svg viewBox="0 0 4 4" xmlns:xlink="http://www.w3.org/1999/xlink"><defs><rect id="r" width="2" height="4" fill="#ff0000"/></defs><use xlink:href="#r" x="2"/></svg>"##;
        let out = rasterize(svg, 4, 4).unwrap();
        assert_eq!(pixel(&out, 3, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn malformed_markup_recovers_with_partial_render_and_diagnostic() {
        // Mismatched close tag + stray junk: should still render the rect and
        // report a recovery, never ParseFailed or panic.
        let svg = r##"<svg viewBox="0 0 4 4"><g><rect width="4" height="4" fill="#ff0000"/></span> junk text <</svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        assert_eq!(pixel(&out.image, 2, 2), [255, 0, 0, 255]);
        assert!(out.report.recovered_error_count > 0);
        assert!(out
            .report
            .warnings
            .iter()
            .any(|w| w.code == "recovery.malformed_markup"));
        // Determinism.
        let again = rasterize(svg, 4, 4).unwrap();
        assert_eq!(out.image.pixels, again.pixels);
    }

    #[test]
    fn title_and_desc_are_extracted_and_bounded() {
        let svg = r##"<svg viewBox="0 0 4 4"><title>My Chart</title><desc>A red square</desc><rect width="4" height="4" fill="#ff0000"/></svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        assert_eq!(out.report.title.as_deref(), Some("My Chart"));
        assert_eq!(out.report.desc.as_deref(), Some("A red square"));
        // <title> text does not render as glyphs.
        assert_eq!(out.report.rendered_element_count, 1);
        // Length bound.
        let long = "x".repeat(MAX_A11Y_TEXT + 500);
        let big = format!(
            r##"<svg viewBox="0 0 4 4"><title>{long}</title><rect width="4" height="4"/></svg>"##
        );
        let out2 = rasterize_with_report(&big, 4, 4).unwrap();
        assert_eq!(out2.report.title.unwrap().chars().count(), MAX_A11Y_TEXT);
    }

    #[test]
    fn aria_label_is_a11y_title_fallback() {
        let svg = r##"<svg viewBox="0 0 4 4" aria-label="Icon"><rect width="4" height="4" fill="#ff0000"/></svg>"##;
        let out = rasterize_with_report(svg, 4, 4).unwrap();
        assert_eq!(out.report.title.as_deref(), Some("Icon"));
    }

    #[test]
    fn security_gates_survive_recovery_policy() {
        // Recovery must NOT weaken the secure-static profile.
        for bad in [
            r##"<!DOCTYPE svg><svg viewBox="0 0 4 4"><rect width="4" height="4"/></svg>"##,
            r##"<svg viewBox="0 0 4 4"><script>x()</script><rect width="4" height="4"/></svg>"##,
            r##"<svg viewBox="0 0 4 4"><image href="https://evil.invalid/a.png" width="4" height="4"/></svg>"##,
        ] {
            assert_eq!(
                rasterize(bad, 4, 4),
                Err(SvgRasterError::ForbiddenContent),
                "must stay rejected: {bad}"
            );
        }
    }

    // --- R8.1: in-repo fuzz harness + memory/CPU cap regressions ------------
    //
    // The decoders below all consume untrusted bytes (SVG text, path data,
    // base64/DEFLATE, PNG, JPEG). The harness mutates a fixed, checked-in seed
    // corpus with a deterministic PRNG (no `rand`/clock dependency) so any CI
    // run is byte-for-byte reproducible, and asserts the invariant every parser
    // must hold: no panic, Err-or-bounded-Ok, bounded output.

    /// Deterministic xorshift64* PRNG.
    fn fuzz_rng(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Mutate a seed buffer with a bounded number of byte edits.
    fn fuzz_mutate(seed: &[u8], rng: &mut u64) -> Vec<u8> {
        let mut out = seed.to_vec();
        let edits = (fuzz_rng(rng) % 8) as usize + 1;
        for _ in 0..edits {
            if out.is_empty() {
                out.push((fuzz_rng(rng) & 0xff) as u8);
                continue;
            }
            let idx = (fuzz_rng(rng) as usize) % out.len();
            match fuzz_rng(rng) % 5 {
                0 => out[idx] ^= 1u8 << (fuzz_rng(rng) % 8),  // bit flip
                1 => out[idx] = (fuzz_rng(rng) & 0xff) as u8, // byte replace
                2 => out.insert(idx, (fuzz_rng(rng) & 0xff) as u8), // insert
                3 => {
                    out.remove(idx); // delete
                }
                _ => out.truncate(idx), // truncate tail
            }
        }
        out
    }

    /// The checked-in seed corpus: a feature-dense SVG + a path string from
    /// `tests/fixtures/svg_fuzz/`, plus the canonical PNG/JPEG payloads.
    fn fuzz_seed_corpus() -> Vec<Vec<u8>> {
        let svg = include_str!("../../tests/fixtures/svg_fuzz/seed.svg");
        let path = include_str!("../../tests/fixtures/svg_fuzz/seed_path.txt");
        let png = base64_decode(PNG_RGBA_2X2.strip_prefix("data:image/png;base64,").unwrap())
            .expect("seed png decodes");
        let jpeg = base64_decode(
            JPEG_RED_444
                .strip_prefix("data:image/jpeg;base64,")
                .unwrap(),
        )
        .expect("seed jpeg decodes");
        vec![svg.as_bytes().to_vec(), path.as_bytes().to_vec(), png, jpeg]
    }

    /// Feed one mutated buffer through every untrusted-input decoder, asserting
    /// the no-panic / bounded-output contract for each path.
    fn fuzz_drive(buf: &[u8]) {
        if let Ok(text) = std::str::from_utf8(buf) {
            // rasterize_or_fallback never errors and is bounded to the canvas.
            let img = rasterize_or_fallback(text, 24, 24);
            assert!(img.pixels.len() <= 24 * 24, "raster output bounded");
            // The path tokenizer must not panic and stays under its token cap.
            let pd = parse_path_d(text);
            assert!(pd.subpaths.len() <= MAX_PATH_TOKENS, "path output bounded");
        }
        if let Ok(img) = decode_png(buf) {
            assert!(
                img.width * img.height <= MAX_IMAGE_PIXELS,
                "png pixels bounded"
            );
            assert_eq!(
                img.rgba.len(),
                img.width * img.height * 4,
                "png buffer exact"
            );
        }
        if let Ok(img) = decode_jpeg(buf) {
            assert!(
                img.width * img.height <= MAX_IMAGE_PIXELS,
                "jpeg pixels bounded"
            );
            assert_eq!(
                img.rgba.len(),
                img.width * img.height * 4,
                "jpeg buffer exact"
            );
        }
        if let Some(out) = inflate(buf, 4096) {
            assert!(out.len() <= 4096, "inflate output bounded");
        }
    }

    fn fuzz_run(iterations: usize) {
        let corpus = fuzz_seed_corpus();
        let mut rng: u64 = 0x9E37_79B9_7F4A_7C15;
        for i in 0..iterations {
            let seed = &corpus[i % corpus.len()];
            let buf = fuzz_mutate(seed, &mut rng);
            fuzz_drive(&buf);
        }
    }

    /// Iteration count for the ignored sweep. Configurable via `ROHKAI_FUZZ_ITERS`
    /// so the SAME harness covers the smoke/sweep/deep tiers without recompiling:
    /// default `default`; `ROHKAI_FUZZ_ITERS=8000`/`50000` for a deeper run. The
    /// fixed PRNG seed keeps any count byte-for-byte reproducible. Debug rasterize
    /// is slow (~ms/iter) — run deep tiers under `--release`. See
    /// docs/SVG_PRECISION_AND_BENCH.md.
    fn fuzz_iters_from_env(default: usize) -> usize {
        std::env::var("ROHKAI_FUZZ_ITERS")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(default)
    }

    #[test]
    fn fuzz_smoke_decoders_never_panic() {
        // Always-run: a few deterministic iterations across each seed.
        fuzz_run(64);
    }

    #[test]
    #[ignore = "fuzz: deterministic sweep over the seed corpus; run with --ignored (ROHKAI_FUZZ_ITERS to deepen)"]
    fn fuzz_decoders_no_panic_bounded() {
        // Default 1k keeps the debug ignored run bounded; raise via env + --release
        // for an 8k/50k sweep (all reproducible from the fixed seed).
        fuzz_run(fuzz_iters_from_env(1_000));
    }

    #[test]
    fn oversized_canvas_request_is_clamped_not_allocated() {
        // A 100k x 100k request (10^10 px) is clamped to the pixel cap rather
        // than allocating the raw buffer.
        let (w, h) = raster_size(100_000, 100_000);
        assert!(w * h <= MAX_RASTER_PIXELS, "{w}x{h} exceeds raster cap");
        let img = rasterize_or_fallback(
            r##"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="red"/></svg>"##,
            100_000,
            100_000,
        );
        assert!(
            img.pixels.len() <= MAX_RASTER_PIXELS,
            "raster output bounded"
        );
    }

    #[test]
    fn oversized_svg_document_is_rejected_bounded() {
        let huge = format!(
            "<svg viewBox=\"0 0 4 4\">{}</svg>",
            "z".repeat(MAX_SVG_BYTES)
        );
        assert!(huge.len() > MAX_SVG_BYTES);
        assert!(
            !svg_text_allowed(&huge),
            "oversized document must be rejected"
        );
        // And the public entry falls back instead of processing it.
        let img = rasterize_or_fallback(&huge, 8, 8);
        assert!(!img.pixels.is_empty());
    }

    #[test]
    fn path_token_flood_collapses_to_default() {
        let flood = format!("M{}", "1 1 ".repeat(MAX_PATH_TOKENS + 50));
        let pd = parse_path_d(&flood);
        assert!(
            pd.subpaths.is_empty(),
            "token flood must collapse to default"
        );
    }

    #[test]
    fn inflate_respects_output_ceiling() {
        // Stored-block stream: "hello" (5 bytes).
        let data = [0x01, 0x05, 0x00, 0xfa, 0xff, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(inflate(&data, 64).as_deref(), Some(&b"hello"[..]));
        // A ceiling below the payload returns None, not a full allocation.
        assert!(
            inflate(&data, 2).is_none(),
            "inflate must honor the ceiling"
        );
    }
}
