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
    Unsupported {
        feature: &'static str,
        message: &'static str,
    },
    Warning {
        code: &'static str,
        message: &'static str,
    },
}

fn unsupported_attr_diagnostics(attrs: &[(String, String)]) -> Vec<PendingDiagnostic> {
    let mut diagnostics = Vec::new();
    for (key, _value) in attrs {
        let diagnostic = match key.as_str() {
            "mask" => Some(PendingDiagnostic::Unsupported {
                feature: "mask attribute",
                message: "mask attributes are diagnosed but not applied yet",
            }),
            "filter" => Some(PendingDiagnostic::Unsupported {
                feature: "filter attribute",
                message: "filter attributes are diagnosed but not applied yet",
            }),
            _ => None,
        };
        if let Some(diagnostic) = diagnostic {
            diagnostics.push(diagnostic);
        }
    }
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
        match diagnostic {
            PendingDiagnostic::Unsupported { feature, message } => {
                report.unsupported_at(*feature, *message, Some(source));
            }
            PendingDiagnostic::Warning { code, message } => {
                report.warning_at(*code, *message, Some(source));
            }
        }
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
                servers.servers.get(id).map_or(Self::Transparent, |server| {
                    Self::from_server(
                        server,
                        paint.opacity,
                        local_bounds,
                        object_transform,
                        length_bases,
                    )
                })
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
            Self::Transparent => [0, 0, 0, 0],
        }
    }

    fn is_transparent(&self) -> bool {
        matches!(self, Self::Transparent | Self::Solid([_, _, _, 0]))
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
        // Skipped in rendering; kept for parse completeness
        #[allow(dead_code)]
        attrs: Vec<(String, String)>,
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
                // A <clipPath> definition never renders directly; its children
                // are consumed only when an element references it via clip-path.
                SvgNode::Unsupported { tag, .. } if tag == "clippath" => {}
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
                _ => build.items.push(SvgSceneItem {
                    node: node.shallow(),
                    transform: combined_transform,
                    style: local_style,
                    length_bases: inherited_length_bases,
                    skipped_by_unsupported_ancestor,
                    layer: None,
                    is_layer_end: false,
                }),
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
            SvgNode::Text { id, span, attrs } => SvgNode::Text {
                id: *id,
                span: *span,
                attrs: attrs.clone(),
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
        };
        let all_nodes = parser.parse_nodes();

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

        Some(SvgDoc {
            root_attrs,
            viewbox,
            preserve_aspect_ratio,
            width,
            height,
            nodes: root_children,
        })
    }
}

// ---------------------------------------------------------------------------
// Minimal XML parser
// ---------------------------------------------------------------------------

struct XmlParser<'a> {
    s: &'a str,
    pos: usize,
    next_node_id: u32,
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
        // Strip namespace prefix (e.g. "svg:rect" → "rect")
        let tag = tag_raw
            .rfind(':')
            .map(|i| &tag_raw[i + 1..])
            .unwrap_or(tag_raw)
            .to_lowercase();

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

        let is_container = is_container_tag(&tag);
        let is_text = tag == "text" || tag == "tspan";
        let is_style = tag == "style";
        let id = SvgNodeId(self.next_node_id);
        self.next_node_id = self.next_node_id.saturating_add(1);

        let mut text_content = String::new();
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
            // Consume end tag
            if self.starts_with("</") {
                self.consume(2);
                self.consume_until(">");
                self.consume(1);
            }
            ch
        } else if !self_closing && is_text {
            // Consume until close tag (text content is skipped)
            self.consume_until("</");
            if self.starts_with("</") {
                self.consume(2);
                self.consume_until(">");
                self.consume(1);
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

        let span = SvgSourceSpan {
            start: element_start,
            end: self.pos,
        };
        self.make_node(id, span, &tag, raw_attrs, children, text_content)
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
            "text" | "tspan" => Some(SvgNode::Text { id, span, attrs }),
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
    let opacity = style.opacity.clamp(0.0, 1.0);
    let isolate = parse_isolation(attrs);
    if clip_ref.is_none() && overflow.is_none() && opacity >= 1.0 && !isolate {
        return None;
    }
    Some(LayerRaw {
        clip_ref,
        element_transform,
        length_bases,
        is_group: true,
        overflow,
        opacity,
        isolate,
        source,
    })
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

/// Fully-resolved layer payload carried by `DrawCommand::BeginLayer`.
struct ResolvedLayer {
    clip: Option<ClipDef>,
    opacity: f32,
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
        ResolvedLayer {
            clip,
            opacity: raw.opacity.clamp(0.0, 1.0),
            needs_offscreen: raw.opacity < 1.0 || raw.isolate,
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
    /// Text — preserved in source but not rasterized.
    UnsupportedText {
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
                SvgNode::Text { .. } => DrawCommand::UnsupportedText {
                    diagnostics,
                    source,
                },
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
                        DrawCommand::Shape {
                            geometry,
                            transform: node_xform,
                            style: Box::new(item.style.clone()),
                            length_bases: item.length_bases,
                            path_length: parsed_path_length(shape_node.attrs()),
                            clip,
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
                    });
                }
                DrawCommand::EndLayer => {
                    if let Some(frame) = frames.pop() {
                        if frame.pushed_offscreen {
                            if let Some(off) = offscreens.pop() {
                                offscreen_bytes = offscreen_bytes.saturating_sub(off.buf.len());
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
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    for (property, id) in style.paint_server_references() {
                        if !self.paint_servers.servers.contains_key(id) {
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
                DrawCommand::UnsupportedText {
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    report.unsupported_at(
                        "text",
                        "text elements are preserved in source but not rasterized yet",
                        Some(*source),
                    );
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
}

/// One open layer scope in `DisplayList::execute`'s layer stack.
struct LayerFrame {
    /// Effective clip to restore when this layer closes.
    prev_effective: Option<ClipMask>,
    /// Whether this layer allocated an isolated offscreen (vs. clip-only).
    pushed_offscreen: bool,
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
            if let Some(stroke) = style.effective_stroke(length_bases) {
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
            if let Some(stroke) = style.effective_stroke(length_bases) {
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
            if let Some(stroke) = style.effective_stroke(length_bases) {
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

#[derive(Clone, Copy)]
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
                "embedded JPEG uses an unsupported feature (progressive, arithmetic, CMYK, or 12-bit); placeholder kept"
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

fn decode_jpeg(bytes: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    let mut pos = 2; // past SOI (FF D8)
    let mut qtables: [[u16; 64]; 4] = [[0; 64]; 4];
    let mut dc_tables: [Option<JpegHuffTable>; 4] = Default::default();
    let mut ac_tables: [Option<JpegHuffTable>; 4] = Default::default();
    let mut width = 0usize;
    let mut height = 0usize;
    let mut components: Vec<JpegComponent> = Vec::new();
    let mut restart_interval = 0usize;

    loop {
        if pos + 1 >= bytes.len() || bytes[pos] != 0xFF {
            return Err(ImageDecodeError::MalformedJpeg);
        }
        let marker = bytes[pos + 1];
        pos += 2;
        match marker {
            0xD9 => return Err(ImageDecodeError::MalformedJpeg), // EOI before SOS
            0x01 | 0xD0..=0xD7 => continue,                      // standalone markers
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
            }
            0xC2 | 0xC3 | 0xC5..=0xCB | 0xCD..=0xCF => {
                return Err(ImageDecodeError::UnsupportedJpeg)
            }
            0xDA => {
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
                );
            }
            _ => {} // APPn / COM / other: skip
        }
        pos = seg_end;
    }
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
    if nc != 1 && nc != 3 {
        return Err(ImageDecodeError::UnsupportedJpeg); // grayscale or YCbCr only
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
            if comps.len() == 1 {
                let g = sample(&planes[0], &comps[0], x, y) as u8;
                out.copy_from_slice(&[g, g, g, 255]);
            } else {
                let yv = sample(&planes[0], &comps[0], x, y);
                let cb = sample(&planes[1], &comps[1], x, y);
                let cr = sample(&planes[2], &comps[2], x, y);
                let rgb = jpeg_ycbcr_to_rgb(yv, cb, cr);
                out.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
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
        let svg = r##"<svg viewBox="0 0 20 20">
<rect width="10" height="10" fill="#ff0000"/>
<rect x="12" width="0" height="5" fill="#00ff00"/>
<text x="1" y="18">Skipped text</text>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 20).unwrap();

        assert_eq!(output.report.rendered_element_count, 1);
        assert_eq!(output.report.skipped_element_count, 2);
        assert_eq!(output.report.warning_count, 0);
        assert_eq!(output.report.unsupported_feature_count, 1);
        assert_eq!(output.report.unsupported_features[0].feature, "text");
        let source = output.report.unsupported_features[0].source.unwrap();
        assert!(svg[source.byte_start..source.byte_end].starts_with("<text"));
        assert_eq!(output.report.fidelity, SvgRenderFidelity::Medium);
        assert_eq!(pixel(&output.image, 5, 5), [255, 0, 0, 255]);
    }

    #[test]
    fn render_report_flags_unsupported_feature_buckets() {
        let svg = r##"<svg viewBox="0 0 20 20">
<defs>
  <linearGradient id="g"/>
  <clipPath id="c"><rect width="10" height="10"/></clipPath>
  <filter id="f"/>
</defs>
<rect width="20" height="20" fill="url(#g)" clip-path="url(#c)" filter="url(#f)"/>
</svg>"##;
        let output = rasterize_with_report(svg, 20, 20).unwrap();
        let features: Vec<&str> = output
            .report
            .unsupported_features
            .iter()
            .map(|u| u.feature.as_str())
            .collect();

        assert!(!features.contains(&"linearGradient"));
        // R4: clipPath now renders, so it is no longer an unsupported feature.
        assert!(!features.contains(&"clipPath"));
        assert!(!features.contains(&"clip-path attribute"));
        assert!(features.contains(&"filter"));
        assert!(features.contains(&"filter attribute"));
        assert_eq!(output.report.fidelity, SvgRenderFidelity::Low);
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

        assert!(alphas.iter().any(|alpha| *alpha == 128));
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
    fn gradient_cycles_and_patterns_remain_explicit_diagnostics() {
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
        let pattern = output
            .report
            .unsupported_features
            .iter()
            .find(|feature| feature.feature == "pattern")
            .expect("pattern diagnostic");
        assert!(pattern.message.contains("patternunits"));
        assert!(pattern.message.contains("patterntransform"));
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
    fn gradients_are_deterministic_high_fidelity_while_patterns_are_not() {
        let gradient = r##"<svg viewBox="0 0 12 4"><defs>
<linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
</defs><rect width="12" height="4" fill="url(#g)"/></svg>"##;
        let first = rasterize_with_report(gradient, 12, 4).unwrap();
        let second = rasterize_with_report(gradient, 12, 4).unwrap();
        assert_eq!(first.image.pixels, second.image.pixels);
        assert_eq!(first.report.fidelity, SvgRenderFidelity::High);
        assert_eq!(first.report.unsupported_feature_count, 0);

        let pattern = r##"<svg viewBox="0 0 12 4"><defs>
<pattern id="p" width="2" height="2" patternUnits="userSpaceOnUse"><rect width="1" height="1"/></pattern>
</defs><rect width="12" height="4" fill="url(#p)"/></svg>"##;
        let output = rasterize_with_report(pattern, 12, 4).unwrap();
        assert_eq!(output.report.fidelity, SvgRenderFidelity::Low);
        assert!(output
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "pattern"));
        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "paint.unresolved_server"));
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

    #[test]
    fn progressive_jpeg_is_diagnosed_unsupported() {
        // SOI + SOF2 (progressive) marker → unsupported, not mis-decoded.
        assert!(matches!(
            decode_jpeg(&[0xFF, 0xD8, 0xFF, 0xC2, 0x00, 0x02]),
            Err(ImageDecodeError::UnsupportedJpeg)
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
}
