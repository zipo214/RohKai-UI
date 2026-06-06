// Pure-Rust software SVG rasterizer.
//
// Covers: rect (rx/ry), circle, ellipse, line, polyline, polygon, path (all
// standard commands), and <g> groups with transforms + style inheritance.
// Outputs egui::ColorImage (straight RGBA). Zero new Cargo dependencies.
//
// Text elements are skipped (decorative in design-tool context).
// Unsupported features (gradients, filters, masks, <use>): shape renders with
// fill/stroke color only.

use crate::svg_core::{self, Rgba};
use egui::ColorImage;
use std::collections::HashMap;

const MAX_SVG_BYTES: usize = 5_000_000;
const MAX_TAGS: usize = 10_000;
const MAX_PATH_TOKENS: usize = 20_000;
const MAX_LOCAL_IDS: usize = 4_096;
const MAX_LOCAL_REFERENCE_USES: usize = 8_192;
/// Maximum flattened points in a single sub-path.  A 20 000-cubic-command
/// path at ~40 pts/command would otherwise allocate ~800 k (f32,f32) pairs.
const MAX_FLAT_PTS: usize = 50_000;
const MAX_RASTER_DIM: u32 = 4096;
const MAX_RASTER_PIXELS: usize = 16_777_216;

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
struct PendingDiagnostic {
    feature: &'static str,
    message: &'static str,
}

fn unsupported_attr_diagnostics(attrs: &[(String, String)]) -> Vec<PendingDiagnostic> {
    let mut diagnostics = Vec::new();
    for (key, value) in attrs {
        let diagnostic = match key.as_str() {
            "clip-path" => Some(PendingDiagnostic {
                feature: "clip-path attribute",
                message: "clip-path attributes are diagnosed but not applied yet",
            }),
            "mask" => Some(PendingDiagnostic {
                feature: "mask attribute",
                message: "mask attributes are diagnosed but not applied yet",
            }),
            "filter" => Some(PendingDiagnostic {
                feature: "filter attribute",
                message: "filter attributes are diagnosed but not applied yet",
            }),
            "stroke-dasharray" => Some(PendingDiagnostic {
                feature: "stroke dasharray",
                message: "stroke dash arrays are not rasterized yet",
            }),
            "stroke-linecap" => Some(PendingDiagnostic {
                feature: "stroke linecap",
                message: "stroke line caps use the current simple stroke fallback",
            }),
            "stroke-linejoin" => Some(PendingDiagnostic {
                feature: "stroke linejoin",
                message: "stroke line joins use the current simple stroke fallback",
            }),
            "fill-rule" => Some(PendingDiagnostic {
                feature: "fill-rule",
                message: "fill-rule is diagnosed; current path fill uses even-odd behavior",
            }),
            _ => None,
        };
        if let Some(diagnostic) = diagnostic {
            diagnostics.push(diagnostic);
        }
        if value.to_ascii_lowercase().contains("url(#") {
            diagnostics.push(PendingDiagnostic {
                feature: "paint server reference",
                message:
                    "paint-server references are diagnosed; gradients/patterns are not rasterized yet",
            });
        }
    }
    diagnostics
}

fn emit_diagnostics(
    diagnostics: &[PendingDiagnostic],
    source: SvgRenderSource,
    report: &mut SvgRenderReport,
) {
    for diagnostic in diagnostics {
        report.unsupported_at(diagnostic.feature, diagnostic.message, Some(source));
    }
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
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Color(Rgba::BLACK)
    }
}

// ---------------------------------------------------------------------------
// Style
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Style {
    fill: Paint,
    stroke: Paint,
    stroke_width: f32,
    opacity: f32,
    fill_opacity: f32,
    stroke_opacity: f32,
    visible: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fill: Paint::Color(Rgba::BLACK),
            stroke: Paint::None,
            stroke_width: 1.0,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            visible: true,
        }
    }
}

impl Style {
    fn inherit(&self, attrs: &[(&str, &str)]) -> Style {
        let mut s = self.clone();
        // Inline style="" overrides individual presentation attrs in CSS
        // but we parse both; apply presentation attrs first, then style="" on top.
        for &(k, v) in attrs {
            match k {
                "fill" => s.fill = parse_paint(v),
                "stroke" => s.stroke = parse_paint(v),
                "stroke-width" => {
                    s.stroke_width = v
                        .trim_end_matches(|c: char| c.is_alphabetic())
                        .parse()
                        .unwrap_or(s.stroke_width);
                }
                "opacity" => {
                    s.opacity = v.parse().unwrap_or(s.opacity);
                }
                "fill-opacity" => {
                    s.fill_opacity = v.parse().unwrap_or(s.fill_opacity);
                }
                "stroke-opacity" => {
                    s.stroke_opacity = v.parse().unwrap_or(s.stroke_opacity);
                }
                "display" if v.trim() == "none" => {
                    s.visible = false;
                }
                "visibility" => {
                    if matches!(v.trim(), "hidden" | "collapse") {
                        s.visible = false;
                    }
                }
                _ => {}
            }
        }
        if let Some(style_val) = attrs.iter().find(|&&(k, _)| k == "style").map(|&(_, v)| v) {
            s.apply_css(style_val);
        }
        s
    }

    fn apply_css(&mut self, css: &str) {
        for decl in css.split(';') {
            if let Some(colon) = decl.find(':') {
                let prop = decl[..colon].trim();
                let val = decl[colon + 1..].trim();
                match prop {
                    "fill" => self.fill = parse_paint(val),
                    "stroke" => self.stroke = parse_paint(val),
                    "stroke-width" => {
                        self.stroke_width = val
                            .trim_end_matches(|c: char| c.is_alphabetic())
                            .parse()
                            .unwrap_or(self.stroke_width);
                    }
                    "opacity" => {
                        self.opacity = val.parse().unwrap_or(self.opacity);
                    }
                    "fill-opacity" => {
                        self.fill_opacity = val.parse().unwrap_or(self.fill_opacity);
                    }
                    "stroke-opacity" => {
                        self.stroke_opacity = val.parse().unwrap_or(self.stroke_opacity);
                    }
                    "display" if val == "none" => {
                        self.visible = false;
                    }
                    "visibility" => {
                        if matches!(val, "hidden" | "collapse") {
                            self.visible = false;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn effective_fill(&self) -> Option<[u8; 4]> {
        if !self.visible {
            return None;
        }
        match &self.fill {
            Paint::None => None,
            Paint::Color(c) => {
                let a = (c.a as f32 * self.fill_opacity * self.opacity)
                    .clamp(0.0, 255.0)
                    .round() as u8;
                Some([c.r, c.g, c.b, a])
            }
        }
    }

    fn effective_stroke(&self) -> Option<([u8; 4], f32)> {
        if !self.visible {
            return None;
        }
        if self.stroke_width <= 0.0 {
            return None;
        }
        match &self.stroke {
            Paint::None => None,
            Paint::Color(c) => {
                let a = (c.a as f32 * self.stroke_opacity * self.opacity)
                    .clamp(0.0, 255.0)
                    .round() as u8;
                Some(([c.r, c.g, c.b, a], self.stroke_width))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Color parsing
// ---------------------------------------------------------------------------

fn parse_paint(s: &str) -> Paint {
    let s = s.trim();
    if s == "none" || s == "transparent" || s.is_empty() || s.starts_with("url(") {
        Paint::None
    } else {
        match svg_core::parse_color(s) {
            Some(c) => Paint::Color(c),
            None => Paint::Color(Rgba::BLACK),
        }
    }
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
    Unsupported {
        id: SvgNodeId,
        span: SvgSourceSpan,
        tag: String,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
}

struct SvgDoc {
    viewbox: Option<[f32; 4]>,
    width: f32,
    height: f32,
    nodes: Vec<SvgNode>,
}

struct SvgScene {
    viewbox: Option<[f32; 4]>,
    width: f32,
    height: f32,
    items: Vec<SvgSceneItem>,
    references: SvgReferenceTable,
}

struct SvgSceneItem {
    node: SvgNode,
    transform: Transform,
    style: Style,
    skipped_by_unsupported_ancestor: bool,
}

#[derive(Clone, Copy)]
struct SvgLengthBases {
    horizontal: f64,
    vertical: f64,
    other: f64,
}

impl SvgScene {
    fn length_bases(&self) -> SvgLengthBases {
        let (width, height) = match self.viewbox {
            Some([_, _, width, height]) if width > 0.0 && height > 0.0 => {
                (width as f64, height as f64)
            }
            _ => (self.width.max(0.0) as f64, self.height.max(0.0) as f64),
        };
        SvgLengthBases {
            horizontal: width,
            vertical: height,
            other: ((width * width + height * height) / 2.0).sqrt(),
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
        let mut items = Vec::new();
        Self::build_items(
            &doc.nodes,
            Transform::identity(),
            Style::default(),
            false,
            &mut items,
        );
        Self {
            viewbox: doc.viewbox,
            width: doc.width,
            height: doc.height,
            items,
            references,
        }
    }

    fn build_items(
        nodes: &[SvgNode],
        inherited_transform: Transform,
        inherited_style: Style,
        skipped_by_unsupported_ancestor: bool,
        items: &mut Vec<SvgSceneItem>,
    ) {
        for node in nodes {
            let attrs = node.attrs();
            let attr_pairs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let local_style = inherited_style.inherit(&attr_pairs);
            let local_transform = attr_get(attrs, "transform")
                .map(Transform::parse_chained)
                .unwrap_or_else(Transform::identity);
            let combined_transform = inherited_transform.concat(local_transform);

            match node {
                SvgNode::Group { children, .. } => {
                    items.push(SvgSceneItem {
                        node: node.shallow(),
                        transform: combined_transform,
                        style: local_style.clone(),
                        skipped_by_unsupported_ancestor,
                    });
                    Self::build_items(
                        children,
                        combined_transform,
                        local_style,
                        skipped_by_unsupported_ancestor,
                        items,
                    );
                }
                SvgNode::Unsupported { children, .. } => {
                    items.push(SvgSceneItem {
                        node: node.shallow(),
                        transform: combined_transform,
                        style: local_style.clone(),
                        skipped_by_unsupported_ancestor,
                    });
                    Self::build_items(children, combined_transform, local_style, true, items);
                }
                _ => items.push(SvgSceneItem {
                    node: node.shallow(),
                    transform: combined_transform,
                    style: local_style,
                    skipped_by_unsupported_ancestor,
                }),
            }
        }
    }
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
            | SvgNode::Unsupported { attrs, .. } => attrs,
        }
    }

    fn children(&self) -> Option<&[SvgNode]> {
        match self {
            SvgNode::Group { children, .. } | SvgNode::Unsupported { children, .. } => {
                Some(children)
            }
            _ => None,
        }
    }

    fn shallow(&self) -> Self {
        match self {
            SvgNode::Group {
                id, span, attrs, ..
            } => SvgNode::Group {
                id: *id,
                span: *span,
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
            viewbox,
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
        let id = SvgNodeId(self.next_node_id);
        self.next_node_id = self.next_node_id.saturating_add(1);

        let children = if !self_closing && is_container {
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
        self.make_node(id, span, &tag, raw_attrs, children)
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
    ) -> Option<SvgNode> {
        match tag {
            "svg" | "g" => Some(SvgNode::Group {
                id,
                span,
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
        "defs" => Some((
            "defs",
            "defs content is preserved in source but not directly rendered",
        )),
        "symbol" => Some((
            "symbol",
            "symbols are not rendered until referenced use expansion is implemented",
        )),
        "use" => Some((
            "use",
            "use/symbol expansion is not implemented in raster mode yet",
        )),
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
        "lineargradient" => Some((
            "linearGradient",
            "linear gradients are diagnosed but not rasterized yet",
        )),
        "radialgradient" => Some((
            "radialGradient",
            "radial gradients are diagnosed but not rasterized yet",
        )),
        "pattern" => Some(("pattern", "patterns are diagnosed but not rasterized yet")),
        "image" => Some((
            "image",
            "embedded raster images are diagnosed but not decoded by the zero-dependency renderer yet",
        )),
        "textpath" => Some(("textPath", "textPath is preserved in source but not rasterized yet")),
        "foreignobject" => Some((
            "foreignObject",
            "foreignObject content is rejected from the secure static renderer profile",
        )),
        "animate" | "animatetransform" | "animatemotion" | "set" | "mpath" => {
            Some(("animation", "animation elements are ignored by the static renderer"))
        }
        "style" => Some((
            "style",
            "style blocks are not parsed by the current rasterizer style engine",
        )),
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

// ---------------------------------------------------------------------------
// ViewBox → pixel transform
// ---------------------------------------------------------------------------

fn viewbox_to_pixel_transform(scene: &SvgScene, pw: usize, ph: usize) -> Transform {
    let (vbx, vby, vbw, vbh) = match scene.viewbox {
        Some([x, y, w, h]) if w > 0.0 && h > 0.0 => (x, y, w, h),
        _ => {
            // Fall back to width/height if present
            if scene.width > 0.0 && scene.height > 0.0 {
                (0.0, 0.0, scene.width, scene.height)
            } else {
                return Transform::identity();
            }
        }
    };

    let scale = (pw as f32 / vbw).min(ph as f32 / vbh);
    let tx = (pw as f32 - vbw * scale) / 2.0 - vbx * scale;
    let ty = (ph as f32 - vbh * scale) / 2.0 - vby * scale;

    Transform {
        a: scale as f64,
        b: 0.0,
        c: 0.0,
        d: scale as f64,
        e: tx as f64,
        f: ty as f64,
    }
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
        sub_paths: Vec<Vec<(f32, f32)>>,
    },
}

/// One render-ready command in the display list.
enum DrawCommand {
    /// A renderable shape with its final (view ∘ item) transform and resolved style.
    Shape {
        geometry: Option<ShapeGeometry>,
        transform: Transform,
        style: Style,
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
        diagnostics: Vec<PendingDiagnostic>,
        source: SvgRenderSource,
    },
}

struct DisplayList {
    commands: Vec<DrawCommand>,
}

impl DisplayList {
    /// Lowering pass: scene items → flat draw-command stream.
    fn build(scene: &SvgScene, view_xform: &Transform) -> Self {
        let mut commands = Vec::with_capacity(scene.items.len());
        let length_bases = scene.length_bases();
        for item in &scene.items {
            let node_xform = view_xform.concat(item.transform);
            let source = item.node.source();
            let diagnostics = unsupported_attr_diagnostics(item.node.attrs());
            let cmd = match &item.node {
                SvgNode::Group { .. } => DrawCommand::GroupDiagnostics {
                    diagnostics,
                    source,
                },
                SvgNode::Text { .. } => DrawCommand::UnsupportedText {
                    diagnostics,
                    source,
                },
                SvgNode::Unsupported { tag, .. } => DrawCommand::UnsupportedNode {
                    tag: tag.clone(),
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
                        DrawCommand::Shape {
                            geometry: lower_shape_geometry(shape_node, length_bases),
                            transform: node_xform,
                            style: item.style.clone(),
                            diagnostics,
                            source,
                        }
                    }
                }
            };
            commands.push(cmd);
        }
        Self { commands }
    }

    /// Raster pass: execute each command, writing pixels and updating the report.
    fn execute(&self, buf: &mut [u8], w: usize, h: usize, report: &mut SvgRenderReport) {
        for cmd in &self.commands {
            match cmd {
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
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    let drawn = geometry.as_ref().is_some_and(|geometry| {
                        render_shape(geometry, transform, style, buf, w, h)
                    });
                    if drawn {
                        report.rendered();
                    } else {
                        report.skipped();
                    }
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
                    diagnostics,
                    source,
                } => {
                    emit_diagnostics(diagnostics, *source, report);
                    if let Some((feature, message)) = unsupported_tag_feature(tag) {
                        report.unsupported_at(feature, message, Some(*source));
                    }
                    report.skipped();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shape renderers
// ---------------------------------------------------------------------------

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
            let sub_paths = parse_path_d(attr_get(attrs, "d").unwrap_or(""));
            (!sub_paths.is_empty()).then_some(ShapeGeometry::Path { sub_paths })
        }
        _ => None,
    }
}

fn render_shape(
    geometry: &ShapeGeometry,
    xform: &Transform,
    style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) -> bool {
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
            buf,
            w,
            h,
        ),
        ShapeGeometry::Ellipse { cx, cy, rx, ry } => {
            render_closed_points(&ellipse_pts(*cx, *cy, *rx, *ry), xform, style, buf, w, h)
        }
        ShapeGeometry::Line { from, to } => {
            let points = vec![xform.apply_f32(from.0, from.1), xform.apply_f32(to.0, to.1)];
            if let Some((stroke_color, stroke_width)) = style.effective_stroke() {
                stroke_polyline(buf, w, h, &points, stroke_width, stroke_color, false);
                true
            } else {
                false
            }
        }
        ShapeGeometry::Poly { points, closed } => {
            let transformed: Vec<_> = points.iter().map(|&(x, y)| xform.apply_f32(x, y)).collect();
            let mut rendered = false;
            if *closed {
                if let Some(fill) = style.effective_fill() {
                    fill_polygon(buf, w, h, &transformed, fill);
                    rendered = true;
                }
            }
            if let Some((stroke_color, stroke_width)) = style.effective_stroke() {
                stroke_polyline(buf, w, h, &transformed, stroke_width, stroke_color, *closed);
                rendered = true;
            }
            rendered
        }
        ShapeGeometry::Path { sub_paths } => {
            let mut rendered = false;
            if let Some(fill) = style.effective_fill() {
                fill_path_subpaths(buf, w, h, sub_paths, xform, fill);
                rendered = true;
            }
            if let Some((stroke_color, stroke_width)) = style.effective_stroke() {
                for sub_path in sub_paths {
                    if sub_path.len() < 2 {
                        continue;
                    }
                    let closed = sub_path.first() == sub_path.last() && sub_path.len() > 2;
                    let points: Vec<_> = sub_path
                        .iter()
                        .map(|&(x, y)| xform.apply_f32(x, y))
                        .collect();
                    stroke_polyline(buf, w, h, &points, stroke_width, stroke_color, closed);
                    rendered = true;
                }
            }
            rendered
        }
    }
}

fn render_closed_points(
    points: &[(f32, f32)],
    xform: &Transform,
    style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) -> bool {
    let transformed: Vec<_> = points.iter().map(|&(x, y)| xform.apply_f32(x, y)).collect();
    let mut rendered = false;
    if let Some(fill) = style.effective_fill() {
        fill_polygon(buf, w, h, &transformed, fill);
        rendered = true;
    }
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        stroke_polyline(buf, w, h, &transformed, stroke_w, stroke_color, true);
        rendered = true;
    }
    rendered
}

// ---------------------------------------------------------------------------
// Path data parser
// ---------------------------------------------------------------------------

#[allow(clippy::while_let_loop)]
fn parse_path_d(d: &str) -> Vec<Vec<(f32, f32)>> {
    let tokens = svg_core::tokenize_path_data(d);
    if tokens.len() > MAX_PATH_TOKENS {
        return Vec::new();
    }
    let mut sub_paths: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut current: Vec<(f32, f32)> = Vec::new();
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    let mut start_x = 0.0f32;
    let mut start_y = 0.0f32;
    let mut last_ctrl_x = 0.0f32;
    let mut last_ctrl_y = 0.0f32;
    let mut last_cmd = 'M';

    let mut i = 0;
    while i < tokens.len() {
        let cmd = match &tokens[i] {
            svg_core::SvgPathToken::Command(c) => {
                i += 1;
                *c
            }
            svg_core::SvgPathToken::Number(_) => {
                // Implicit repetition: use last command (M→L, m→l)
                match last_cmd {
                    'M' => 'L',
                    'm' => 'l',
                    c => c,
                }
            }
        };
        last_cmd = cmd;

        macro_rules! get_num {
            () => {
                if let Some(svg_core::SvgPathToken::Number(n)) = tokens.get(i) {
                    i += 1;
                    *n as f32
                } else {
                    break;
                }
            };
        }

        match cmd {
            'M' | 'm' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let (nx, ny) = if cmd == 'm' { (cx + x, cy + y) } else { (x, y) };
                if !current.is_empty() {
                    sub_paths.push(std::mem::take(&mut current));
                }
                cx = nx;
                cy = ny;
                start_x = cx;
                start_y = cy;
                current.push((cx, cy));
                // After first M/m point, treat extra pairs as L/l
                last_cmd = if cmd == 'm' { 'l' } else { 'L' };
            },
            'L' | 'l' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let (nx, ny) = if cmd == 'l' { (cx + x, cy + y) } else { (x, y) };
                cx = nx;
                cy = ny;
                current.push((cx, cy));
            },
            'H' | 'h' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                cx = if cmd == 'h' { cx + x } else { x };
                current.push((cx, cy));
            },
            'V' | 'v' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                cy = if cmd == 'v' { cy + y } else { y };
                current.push((cx, cy));
            },
            'C' | 'c' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x1 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y1 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x2 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y2 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let (ax1, ay1, ax2, ay2, ax, ay) = if cmd == 'c' {
                    (cx + x1, cy + y1, cx + x2, cy + y2, cx + x, cy + y)
                } else {
                    (x1, y1, x2, y2, x, y)
                };
                last_ctrl_x = ax2;
                last_ctrl_y = ay2;
                flatten_cubic(&mut current, cx, cy, ax1, ay1, ax2, ay2, ax, ay, 0);
                cx = ax;
                cy = ay;
            },
            'S' | 's' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x2 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y2 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                // Reflected control point
                let ax1 = if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                    2.0 * cx - last_ctrl_x
                } else {
                    cx
                };
                let ay1 = if matches!(last_cmd, 'C' | 'c' | 'S' | 's') {
                    2.0 * cy - last_ctrl_y
                } else {
                    cy
                };
                let (ax2, ay2, ax, ay) = if cmd == 's' {
                    (cx + x2, cy + y2, cx + x, cy + y)
                } else {
                    (x2, y2, x, y)
                };
                last_ctrl_x = ax2;
                last_ctrl_y = ay2;
                flatten_cubic(&mut current, cx, cy, ax1, ay1, ax2, ay2, ax, ay, 0);
                cx = ax;
                cy = ay;
            },
            'Q' | 'q' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x1 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y1 = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let (ax1, ay1, ax, ay) = if cmd == 'q' {
                    (cx + x1, cy + y1, cx + x, cy + y)
                } else {
                    (x1, y1, x, y)
                };
                last_ctrl_x = ax1;
                last_ctrl_y = ay1;
                // Quadratic → cubic
                let cx1 = cx + 2.0 / 3.0 * (ax1 - cx);
                let cy1 = cy + 2.0 / 3.0 * (ay1 - cy);
                let cx2 = ax + 2.0 / 3.0 * (ax1 - ax);
                let cy2 = ay + 2.0 / 3.0 * (ay1 - ay);
                flatten_cubic(&mut current, cx, cy, cx1, cy1, cx2, cy2, ax, ay, 0);
                cx = ax;
                cy = ay;
            },
            'T' | 't' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let ctrl_x = if matches!(last_cmd, 'Q' | 'q' | 'T' | 't') {
                    2.0 * cx - last_ctrl_x
                } else {
                    cx
                };
                let ctrl_y = if matches!(last_cmd, 'Q' | 'q' | 'T' | 't') {
                    2.0 * cy - last_ctrl_y
                } else {
                    cy
                };
                let (ax, ay) = if cmd == 't' { (cx + x, cy + y) } else { (x, y) };
                last_ctrl_x = ctrl_x;
                last_ctrl_y = ctrl_y;
                let cx1 = cx + 2.0 / 3.0 * (ctrl_x - cx);
                let cy1 = cy + 2.0 / 3.0 * (ctrl_y - cy);
                let cx2 = ax + 2.0 / 3.0 * (ctrl_x - ax);
                let cy2 = ay + 2.0 / 3.0 * (ctrl_y - ay);
                flatten_cubic(&mut current, cx, cy, cx1, cy1, cx2, cy2, ax, ay, 0);
                cx = ax;
                cy = ay;
            },
            'A' | 'a' => loop {
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let rx = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let ry = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x_rot = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let large = get_num!() != 0.0;
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let sweep = get_num!() != 0.0;
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(svg_core::SvgPathToken::Number(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let (ax, ay) = if cmd == 'a' { (cx + x, cy + y) } else { (x, y) };
                arc_to_lines(&mut current, cx, cy, rx, ry, x_rot, large, sweep, ax, ay);
                cx = ax;
                cy = ay;
            },
            'Z' | 'z' => {
                if !current.is_empty() {
                    current.push((start_x, start_y));
                    sub_paths.push(std::mem::take(&mut current));
                }
                cx = start_x;
                cy = start_y;
            }
            _ => skip_path_numbers(&tokens, &mut i),
        }
    }

    if !current.is_empty() {
        sub_paths.push(current);
    }
    sub_paths
}

fn skip_path_numbers(tokens: &[svg_core::SvgPathToken], index: &mut usize) {
    while *index < tokens.len() && !matches!(tokens[*index], svg_core::SvgPathToken::Command(_)) {
        *index += 1;
    }
}

// ---------------------------------------------------------------------------
// Bezier flattening
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn flatten_cubic(
    pts: &mut Vec<(f32, f32)>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    depth: u32,
) {
    // Hard depth limit prevents stack overflow on pathological inputs.
    // Point-count limit caps memory on dense paths (e.g. 20 k cubic commands).
    if depth >= 32 || pts.len() >= MAX_FLAT_PTS {
        pts.push((x3, y3));
        return;
    }
    // Subdivide until chord distance is small enough.
    let d1 = dist_point_to_line(x1, y1, x0, y0, x3, y3);
    let d2 = dist_point_to_line(x2, y2, x0, y0, x3, y3);
    if d1 + d2 < 0.5 {
        pts.push((x3, y3));
        return;
    }
    let m01x = (x0 + x1) * 0.5;
    let m01y = (y0 + y1) * 0.5;
    let m12x = (x1 + x2) * 0.5;
    let m12y = (y1 + y2) * 0.5;
    let m23x = (x2 + x3) * 0.5;
    let m23y = (y2 + y3) * 0.5;
    let m012x = (m01x + m12x) * 0.5;
    let m012y = (m01y + m12y) * 0.5;
    let m123x = (m12x + m23x) * 0.5;
    let m123y = (m12y + m23y) * 0.5;
    let midx = (m012x + m123x) * 0.5;
    let midy = (m012y + m123y) * 0.5;
    flatten_cubic(pts, x0, y0, m01x, m01y, m012x, m012y, midx, midy, depth + 1);
    flatten_cubic(pts, midx, midy, m123x, m123y, m23x, m23y, x3, y3, depth + 1);
}

fn dist_point_to_line(px: f32, py: f32, lx0: f32, ly0: f32, lx1: f32, ly1: f32) -> f32 {
    let dx = lx1 - lx0;
    let dy = ly1 - ly0;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return ((px - lx0) * (px - lx0) + (py - ly0) * (py - ly0)).sqrt();
    }
    ((dx * (ly0 - py) - dy * (lx0 - px)).abs()) / len
}

// ---------------------------------------------------------------------------
// SVG arc → line approximation
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn arc_to_lines(
    pts: &mut Vec<(f32, f32)>,
    x0: f32,
    y0: f32,
    rx: f32,
    ry: f32,
    x_rot_deg: f32,
    large_arc: bool,
    sweep: bool,
    x1: f32,
    y1: f32,
) {
    if rx <= 0.0 || ry <= 0.0 {
        pts.push((x1, y1));
        return;
    }
    let phi = x_rot_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    let dx = (x0 - x1) * 0.5;
    let dy = (y0 - y1) * 0.5;
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

    let cx = cos_phi * cxp - sin_phi * cyp + (x0 + x1) * 0.5;
    let cy = sin_phi * cxp + cos_phi * cyp + (y0 + y1) * 0.5;

    let theta1 = angle_between(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut d_theta = angle_between(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );

    if !sweep && d_theta > 0.0 {
        d_theta -= std::f32::consts::TAU;
    } else if sweep && d_theta < 0.0 {
        d_theta += std::f32::consts::TAU;
    }

    let n = (d_theta.abs() / 0.1).ceil() as i32;
    let n = n.max(4);
    for k in 1..=n {
        let t = theta1 + d_theta * (k as f32 / n as f32);
        let px = cos_phi * rx * t.cos() - sin_phi * ry * t.sin() + cx;
        let py = sin_phi * rx * t.cos() + cos_phi * ry * t.sin() + cy;
        pts.push((px, py));
    }
}

fn angle_between(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
    let dot = ux * vx + uy * vy;
    let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
    if len < 1e-9 {
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

// ---------------------------------------------------------------------------
// Pixel fill algorithms
// ---------------------------------------------------------------------------

/// Even-odd scanline fill for a single polygon.
fn fill_polygon(buf: &mut [u8], w: usize, h: usize, pts: &[(f32, f32)], color: [u8; 4]) {
    if pts.len() < 3 || color[3] == 0 {
        return;
    }

    let min_y = pts
        .iter()
        .map(|p| p.1)
        .fold(f32::INFINITY, f32::min)
        .max(0.0) as usize;
    let max_y = pts
        .iter()
        .map(|p| p.1)
        .fold(f32::NEG_INFINITY, f32::max)
        .min(h as f32 - 1.0) as usize;

    let n = pts.len();
    for row in min_y..=max_y {
        let yf = row as f32 + 0.5;
        let mut xs: Vec<f32> = Vec::new();
        for k in 0..n {
            let (x0, y0) = pts[k];
            let (x1, y1) = pts[(k + 1) % n];
            if (y0 <= yf && y1 > yf) || (y1 <= yf && y0 > yf) {
                let t = (yf - y0) / (y1 - y0);
                xs.push(x0 + t * (x1 - x0));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut k = 0;
        while k + 1 < xs.len() {
            let x_start = (xs[k].ceil() as i32).max(0) as usize;
            let x_end = (xs[k + 1].floor() as i32).min(w as i32 - 1) as usize;
            for col in x_start..=x_end {
                blend_pixel(buf, w, col, row, color);
            }
            k += 2;
        }
    }
}

/// Combine multiple sub-paths into one even-odd fill.
fn fill_path_subpaths(
    buf: &mut [u8],
    w: usize,
    h: usize,
    sub_paths: &[Vec<(f32, f32)>],
    xform: &Transform,
    color: [u8; 4],
) {
    if color[3] == 0 || sub_paths.is_empty() {
        return;
    }

    // Bounding box
    let mut min_y = usize::MAX;
    let mut max_y = 0usize;
    let all: Vec<Vec<(f32, f32)>> = sub_paths
        .iter()
        .filter(|s| s.len() >= 2)
        .map(|s| s.iter().map(|&(px, py)| xform.apply_f32(px, py)).collect())
        .collect();

    for sub in &all {
        for &(_, py) in sub {
            let row = py as usize;
            min_y = min_y.min(row);
            max_y = max_y.max(row);
        }
    }
    let min_y = min_y.min(h.saturating_sub(1));
    let max_y = max_y.min(h.saturating_sub(1));

    for row in min_y..=max_y {
        let yf = row as f32 + 0.5;
        let mut xs: Vec<f32> = Vec::new();
        for sub in &all {
            let n = sub.len();
            for k in 0..n {
                let (x0, y0) = sub[k];
                let (x1, y1) = sub[(k + 1) % n];
                if (y0 <= yf && y1 > yf) || (y1 <= yf && y0 > yf) {
                    let t = (yf - y0) / (y1 - y0);
                    xs.push(x0 + t * (x1 - x0));
                }
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut k = 0;
        while k + 1 < xs.len() {
            let x_start = (xs[k].ceil() as i32).max(0) as usize;
            let x_end = (xs[k + 1].floor() as i32).min(w as i32 - 1) as usize;
            for col in x_start..=x_end {
                blend_pixel(buf, w, col, row, color);
            }
            k += 2;
        }
    }
}

/// Stroke a polyline by expanding each segment into a thin quad.
fn stroke_polyline(
    buf: &mut [u8],
    w: usize,
    h: usize,
    pts: &[(f32, f32)],
    stroke_w: f32,
    color: [u8; 4],
    closed: bool,
) {
    if pts.len() < 2 || color[3] == 0 || stroke_w <= 0.0 {
        return;
    }
    let half = stroke_w * 0.5;
    let n = if closed { pts.len() } else { pts.len() - 1 };
    for k in 0..n {
        let (x0, y0) = pts[k];
        let (x1, y1) = pts[(k + 1) % pts.len()];
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-4 {
            continue;
        }
        let nx = -dy / len * half;
        let ny = dx / len * half;
        let quad = [
            (x0 + nx, y0 + ny),
            (x0 - nx, y0 - ny),
            (x1 - nx, y1 - ny),
            (x1 + nx, y1 + ny),
        ];
        fill_polygon(buf, w, h, &quad, color);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(image: &ColorImage, x: usize, y: usize) -> [u8; 4] {
        image.pixels[y * image.size[0] + x].to_array()
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

        assert!(features.contains(&"linearGradient"));
        assert!(features.contains(&"clipPath"));
        assert!(features.contains(&"filter"));
        assert!(features.contains(&"clip-path attribute"));
        assert!(features.contains(&"filter attribute"));
        assert!(features.contains(&"paint server reference"));
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
            .find_map(|item| lower_shape_geometry(&item.node, scene.length_bases()))
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
    fn unknown_path_command_does_not_stall_renderer() {
        let paths = parse_path_d("M1 1 R5 5 L8 1 L8 8 Z");

        assert!(!paths.is_empty());
        assert!(paths.iter().flatten().any(|point| *point == (8.0, 8.0)));
    }

    #[test]
    fn unsupported_definition_children_are_counted_as_skipped() {
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

        assert_eq!(output.report.rendered_element_count, 0);
        assert!(output.report.skipped_element_count >= 3);
        assert!(features.contains(&"defs"));
        assert!(features.contains(&"linearGradient"));
        assert!(features.contains(&"paint server reference"));
        assert_eq!(output.report.fidelity, SvgRenderFidelity::Low);
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
}
