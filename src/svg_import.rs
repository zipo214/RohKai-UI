use crate::project::schema::{Rect, SvgImportMetadata, WidgetInstance, WidgetKind, WidgetProps};
use crate::svg_core;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

const MIN_PLACEHOLDER_SIZE: f64 = 20.0;
const ARC_TOLERANCE_PX: f64 = 0.5;
const MAX_CSS_RULES: usize = 4_096;
const MAX_CSS_DECLARATIONS: usize = 16_384;

#[derive(Debug, Clone, Default, PartialEq)]
pub enum SvgImportMode {
    /// Single source-backed node. SVG bytes are stored on the widget and
    /// previewed by RohKai's native placeholder painter.
    Image,
    /// Editable frame-per-shape (existing behaviour).
    #[default]
    Components,
}

#[derive(Debug, Clone, Default)]
pub struct SvgImportOptions {
    pub limits: SvgImportLimits,
    pub mode: SvgImportMode,
}

#[derive(Debug, Clone)]
pub struct SvgImportLimits {
    pub max_file_bytes: usize,
    pub max_tag_count: usize,
    pub max_attribute_count_per_tag: usize,
    pub max_attribute_value_length: usize,
    pub max_nesting_depth: usize,
    pub max_path_command_count: usize,
    pub max_generated_placeholder_count: usize,
    pub max_image_data_uri_bytes: usize,
    pub max_use_expansion_depth: usize,
    pub max_style_bytes: usize,
}

impl Default for SvgImportLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 5_000_000,
            max_tag_count: 10_000,
            max_attribute_count_per_tag: 64,
            max_attribute_value_length: 65_536,
            max_nesting_depth: 64,
            max_path_command_count: 20_000,
            max_generated_placeholder_count: 2_000,
            max_image_data_uri_bytes: 1_000_000,
            max_use_expansion_depth: 32,
            max_style_bytes: 262_144,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SvgImportOutput {
    pub widgets: Vec<WidgetInstance>,
    pub report: SvgImportReport,
}

#[derive(Debug, Clone)]
pub struct SvgImportReport {
    pub imported_element_count: usize,
    pub skipped_element_count: usize,
    pub warning_count: usize,
    pub unsupported_feature_count: usize,
    pub warnings: Vec<SvgImportWarning>,
    pub unsupported_features: Vec<SvgUnsupportedFeature>,
    pub fidelity: SvgFidelity,
}

impl SvgImportReport {
    fn new() -> Self {
        Self {
            imported_element_count: 0,
            skipped_element_count: 0,
            warning_count: 0,
            unsupported_feature_count: 0,
            warnings: Vec::new(),
            unsupported_features: Vec::new(),
            fidelity: SvgFidelity::High,
        }
    }

    fn finalize(&mut self, text_element_count: usize, text_character_count: usize) {
        self.warning_count = self.warnings.len();
        self.unsupported_feature_count = self.unsupported_features.len();
        let has_layout_loss = self.unsupported_features.iter().any(|u| {
            matches!(
                u.feature.as_str(),
                "clipPath"
                    | "clip-path attribute"
                    | "mask"
                    | "mask attribute"
                    | "filter"
                    | "filter attribute"
                    | "textPath"
                    | "foreignObject"
            )
        });
        let has_paint_loss = self.unsupported_features.iter().any(|u| {
            matches!(
                u.feature.as_str(),
                "linearGradient" | "radialGradient" | "pattern" | "paint server reference"
            )
        });
        let text_heavy = text_element_count >= 3 || text_character_count >= 80;
        self.fidelity = if self.imported_element_count == 0
            || self.skipped_element_count > self.imported_element_count
            || self.unsupported_feature_count > 5
            || (has_layout_loss && self.skipped_element_count > 0)
        {
            SvgFidelity::Low
        } else if self.warning_count > 0
            || self.unsupported_feature_count > 0
            || has_layout_loss
            || has_paint_loss
            || text_heavy
        {
            SvgFidelity::Medium
        } else {
            SvgFidelity::High
        };
    }

    #[allow(dead_code)]
    pub fn diagnostics_digest(&self) -> usize {
        let warning_bits: usize = self
            .warnings
            .iter()
            .map(|w| {
                let severity = match w.severity {
                    SvgWarningSeverity::Info => 1,
                    SvgWarningSeverity::Warning => 2,
                };
                w.code.len()
                    + w.message.len()
                    + w.element_name.as_deref().unwrap_or("").len()
                    + w.original_id.as_deref().unwrap_or("").len()
                    + w.source_order.unwrap_or_default()
                    + severity
            })
            .sum();
        let unsupported_bits: usize = self
            .unsupported_features
            .iter()
            .map(|u| {
                u.feature.len()
                    + u.element_name.as_deref().unwrap_or("").len()
                    + u.original_id.as_deref().unwrap_or("").len()
                    + u.source_order.unwrap_or_default()
            })
            .sum();
        warning_bits + unsupported_bits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgWarningSeverity {
    Info,
    Warning,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SvgImportWarning {
    pub code: String,
    pub message: String,
    pub element_name: Option<String>,
    pub original_id: Option<String>,
    pub source_order: Option<usize>,
    pub severity: SvgWarningSeverity,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SvgUnsupportedFeature {
    pub feature: String,
    pub element_name: Option<String>,
    pub original_id: Option<String>,
    pub source_order: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgFidelity {
    High,
    Medium,
    Low,
}

impl fmt::Display for SvgFidelity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvgFidelity::High => write!(f, "High"),
            SvgFidelity::Medium => write!(f, "Medium"),
            SvgFidelity::Low => write!(f, "Low"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SvgImportError {
    pub code: String,
    pub message: String,
}

impl SvgImportError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for SvgImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[allow(dead_code)]
pub fn parse_svg_template(svg: &str) -> Result<Vec<WidgetInstance>, String> {
    import_svg_template(svg, SvgImportOptions::default())
        .map(|output| output.widgets)
        .map_err(|err| err.to_string())
}

pub fn import_svg_template(
    svg: &str,
    options: SvgImportOptions,
) -> Result<SvgImportOutput, SvgImportError> {
    if svg.len() > options.limits.max_file_bytes {
        return Err(SvgImportError::new(
            "limit.file_bytes",
            format!(
                "SVG is {} bytes; limit is {}",
                svg.len(),
                options.limits.max_file_bytes
            ),
        ));
    }

    let lower = svg.to_ascii_lowercase();
    if !lower.contains("<svg") {
        return Err(SvgImportError::new("not_svg", "not an SVG document"));
    }
    if lower.contains("<!doctype") {
        return Err(SvgImportError::new(
            "xml.doctype",
            "DOCTYPE is not allowed in SVG imports",
        ));
    }
    if lower.contains("<!entity") {
        return Err(SvgImportError::new(
            "xml.entity",
            "custom XML entities are not allowed in SVG imports",
        ));
    }

    if options.mode == SvgImportMode::Image {
        return import_svg_as_image(svg);
    }

    let mut ctx = ImportContext::new(options.limits);
    let nodes = scan_svg(svg, &mut ctx)?;
    let styles = collect_style_rules(&nodes, &mut ctx)?;
    let mut id_index = HashMap::new();
    for node in &nodes {
        if let Some(id) = attr(&node.tag, "id").filter(|id| !id.is_empty()) {
            if id_index.contains_key(id) {
                ctx.warn(
                    "id.duplicate",
                    format!(
                        "duplicate id '{id}' ignored for reference lookup; first occurrence wins"
                    ),
                    Some(node),
                    SvgWarningSeverity::Warning,
                );
            } else {
                id_index.insert(id.to_owned(), node.index);
            }
        }
    }

    let mut widgets = Vec::new();
    let root_state = ParseState::default();
    for child in nodes[0].children.clone() {
        import_node(
            child,
            &nodes,
            &styles,
            &id_index,
            root_state,
            Style::default(),
            &mut Vec::new(),
            &mut widgets,
            &mut ctx,
        )?;
    }

    if widgets.is_empty() {
        return Err(SvgImportError::new(
            "empty_import",
            "SVG parsed, but no supported visible shapes or text were found",
        ));
    }

    normalize_widgets(&mut widgets);
    ctx.report.imported_element_count = widgets.len();
    ctx.report
        .finalize(ctx.text_element_count, ctx.text_character_count);
    Ok(SvgImportOutput {
        widgets,
        report: ctx.report,
    })
}

fn import_svg_as_image(svg: &str) -> Result<SvgImportOutput, SvgImportError> {
    let (w, h) = parse_svg_dimensions(svg);
    let rect = crate::project::schema::Rect {
        x: 0.0,
        y: 0.0,
        w,
        h,
    };
    let widget = crate::project::schema::WidgetInstance {
        id: deterministic_source_uuid("svg-image", svg, &rect),
        kind: crate::project::schema::WidgetKind::Image,
        rect,
        props: crate::project::schema::WidgetProps {
            label: "SVG Image".to_owned(),
            ..Default::default()
        },
        svg_source: Some(svg.to_owned()),
        ..Default::default()
    };
    let mut report = SvgImportReport::new();
    report.imported_element_count = 1;
    report.finalize(0, 0);
    Ok(SvgImportOutput {
        widgets: vec![widget],
        report,
    })
}

fn deterministic_source_uuid(
    kind: &str,
    source: &str,
    rect: &crate::project::schema::Rect,
) -> Uuid {
    let mut hash = 0xcbf29ce484222325u64;
    fn feed(hash: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *hash ^= *b as u64;
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    feed(&mut hash, kind.as_bytes());
    feed(&mut hash, source.as_bytes());
    feed(&mut hash, rect.w.to_bits().to_le_bytes().as_slice());
    feed(&mut hash, rect.h.to_bits().to_le_bytes().as_slice());
    let second = hash.rotate_left(23) ^ 0xa0761d6478bd642f;
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hash.to_le_bytes());
    bytes[8..].copy_from_slice(&second.to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Extract natural width/height from SVG root element. Falls back to 400×300.
fn parse_svg_dimensions(svg: &str) -> (f32, f32) {
    // Try viewBox="min-x min-y width height"
    if let Some(vb) = extract_attr(svg, "viewBox").or_else(|| extract_attr(svg, "viewbox")) {
        let nums = svg_core::parse_numbers(vb);
        if nums.len() >= 4 {
            let w = nums[2].max(1.0) as f32;
            let h = nums[3].max(1.0) as f32;
            return (w, h);
        }
    }
    // Try width/height attributes
    let w = extract_attr(svg, "width")
        .and_then(|v| parse_length(v, 400.0))
        .map(|value| value as f32)
        .unwrap_or(400.0_f32)
        .max(1.0);
    let h = extract_attr(svg, "height")
        .and_then(|v| parse_length(v, 300.0))
        .map(|value| value as f32)
        .unwrap_or(300.0_f32)
        .max(1.0);
    (w, h)
}

/// Naive attribute value extractor for the SVG root tag. Scans for `name="value"` or `name='value'`.
fn extract_attr<'a>(svg: &'a str, name: &str) -> Option<&'a str> {
    // Only scan within the first 2 KB (root tag region)
    let region = &svg[..svg.len().min(2048)];
    let pattern = format!("{name}=");
    let start = region.find(pattern.as_str())?;
    let after_eq = start + pattern.len();
    let bytes = region.as_bytes();
    if after_eq >= bytes.len() {
        return None;
    }
    let quote = bytes[after_eq];
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let val_start = after_eq + 1;
    let val_end = region[val_start..].find(quote as char)? + val_start;
    Some(&region[val_start..val_end])
}

struct ImportContext {
    limits: SvgImportLimits,
    report: SvgImportReport,
    tag_count: usize,
    text_element_count: usize,
    text_character_count: usize,
}

impl ImportContext {
    fn new(limits: SvgImportLimits) -> Self {
        Self {
            limits,
            report: SvgImportReport::new(),
            tag_count: 0,
            text_element_count: 0,
            text_character_count: 0,
        }
    }

    fn warn(
        &mut self,
        code: &str,
        message: impl Into<String>,
        node: Option<&Node>,
        severity: SvgWarningSeverity,
    ) {
        self.report.warnings.push(SvgImportWarning {
            code: code.to_owned(),
            message: message.into(),
            element_name: node.map(|n| n.tag.name.clone()),
            original_id: node.and_then(|n| attr(&n.tag, "id").map(ToOwned::to_owned)),
            source_order: node.map(|n| n.source_order),
            severity,
        });
    }

    fn unsupported(&mut self, feature: &str, node: Option<&Node>) {
        self.report
            .unsupported_features
            .push(SvgUnsupportedFeature {
                feature: feature.to_owned(),
                element_name: node.map(|n| n.tag.name.clone()),
                original_id: node.and_then(|n| attr(&n.tag, "id").map(ToOwned::to_owned)),
                source_order: node.map(|n| n.source_order),
            });
        self.warn(
            "unsupported.feature",
            format!(
                "unsupported SVG feature ignored: {feature}; RohKai preserved the source SVG and imported editable placeholders for supported visible geometry"
            ),
            node,
            SvgWarningSeverity::Warning,
        );
    }

    fn skip(&mut self) {
        self.report.skipped_element_count += 1;
    }
}

#[derive(Clone, Debug)]
struct Node {
    index: usize,
    children: Vec<usize>,
    tag: Tag,
    text: String,
    source_order: usize,
}

#[derive(Clone, Debug)]
struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
    self_closing: bool,
}

type Matrix = svg_core::Affine2D;

#[derive(Clone, Copy)]
struct ParseState {
    transform: Matrix,
    hidden: bool,
    viewport_w: f64,
    viewport_h: f64,
    expanding_use: bool,
}

impl Default for ParseState {
    fn default() -> Self {
        Self {
            transform: Matrix::IDENTITY,
            hidden: false,
            viewport_w: 800.0,
            viewport_h: 600.0,
            expanding_use: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn new(x: f64, y: f64, w: f64, h: f64) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
            return None;
        }
        Some(Self {
            min_x: x,
            min_y: y,
            max_x: x + w.max(0.0),
            max_y: y + h.max(0.0),
        })
    }

    fn from_points(points: &[(f64, f64)]) -> Option<Self> {
        let first = points
            .iter()
            .find(|(x, y)| x.is_finite() && y.is_finite())?;
        let mut out = Self {
            min_x: first.0,
            min_y: first.1,
            max_x: first.0,
            max_y: first.1,
        };
        for &(x, y) in points {
            if x.is_finite() && y.is_finite() {
                out.include(x, y);
            }
        }
        Some(out)
    }

    fn include(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }

    fn transform(self, matrix: Matrix) -> Self {
        let corners = [
            matrix.apply(self.min_x, self.min_y),
            matrix.apply(self.max_x, self.min_y),
            matrix.apply(self.max_x, self.max_y),
            matrix.apply(self.min_x, self.max_y),
        ];
        Self::from_points(&corners).unwrap_or(self)
    }

    fn rect(self) -> Rect {
        Rect {
            x: self.min_x.max(0.0) as f32,
            y: self.min_y.max(0.0) as f32,
            w: (self.max_x - self.min_x).abs().max(MIN_PLACEHOLDER_SIZE) as f32,
            h: (self.max_y - self.min_y).abs().max(MIN_PLACEHOLDER_SIZE) as f32,
        }
    }
}

#[derive(Clone, Default)]
struct Style {
    display_none: bool,
    visibility_hidden: bool,
    opacity: Option<f64>,
    font_size: Option<f64>,
    text_anchor: Option<String>,
    dominant_baseline: Option<String>,
    color: Option<String>,
    fill: Option<String>,
    stroke: Option<String>,
}

fn scan_svg(svg: &str, ctx: &mut ImportContext) -> Result<Vec<Node>, SvgImportError> {
    let mut nodes = vec![Node {
        index: 0,
        children: Vec::new(),
        tag: Tag {
            name: "#root".to_owned(),
            attrs: Vec::new(),
            self_closing: false,
        },
        text: String::new(),
        source_order: 0,
    }];
    let mut stack = vec![0usize];
    let mut index = 0usize;
    let mut source_order = 0usize;

    while let Some(open_rel) = svg[index..].find('<') {
        let open = index + open_rel;
        if open > index {
            let text = decode_entities(&svg[index..open], ctx, None);
            if let Some(last) = stack.last().copied() {
                nodes[last].text.push_str(&text);
            }
        }

        let close = find_tag_close(svg, open)
            .ok_or_else(|| SvgImportError::new("xml.unterminated_tag", "unterminated SVG tag"))?;
        let raw = svg[open + 1..close].trim();
        index = close + 1;

        if raw.is_empty() {
            continue;
        }
        if raw.starts_with("!--") {
            continue;
        }
        if raw.starts_with('?') {
            if !raw.to_ascii_lowercase().starts_with("?xml") {
                ctx.warn(
                    "xml.processing_instruction",
                    "non-XML processing instruction ignored",
                    None,
                    SvgWarningSeverity::Warning,
                );
            }
            continue;
        }
        if raw.starts_with('!') {
            let decl = raw.to_ascii_lowercase();
            if decl.starts_with("!doctype") {
                return Err(SvgImportError::new(
                    "xml.doctype",
                    "DOCTYPE is not allowed in SVG imports",
                ));
            }
            if decl.starts_with("!entity") {
                return Err(SvgImportError::new(
                    "xml.entity",
                    "custom XML entities are not allowed in SVG imports",
                ));
            }
            ctx.warn(
                "xml.declaration",
                "unsupported declaration ignored",
                None,
                SvgWarningSeverity::Warning,
            );
            continue;
        }
        if raw.starts_with('/') {
            let name = raw
                .trim_start_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            while stack.len() > 1 {
                let popped = stack.pop().unwrap_or(0);
                if nodes[popped].tag.name == name {
                    break;
                }
            }
            continue;
        }

        ctx.tag_count += 1;
        if ctx.tag_count > ctx.limits.max_tag_count {
            return Err(SvgImportError::new(
                "limit.tag_count",
                format!("SVG exceeded {} tags", ctx.limits.max_tag_count),
            ));
        }
        if stack.len() > ctx.limits.max_nesting_depth {
            return Err(SvgImportError::new(
                "limit.nesting_depth",
                format!(
                    "SVG nesting depth exceeded {}",
                    ctx.limits.max_nesting_depth
                ),
            ));
        }

        let tag = parse_tag(raw, ctx)?;
        source_order += 1;
        let parent = stack.last().copied().unwrap_or(0);
        let node_index = nodes.len();
        nodes[parent].children.push(node_index);
        let self_closing = tag.self_closing;
        nodes.push(Node {
            index: node_index,
            children: Vec::new(),
            tag,
            text: String::new(),
            source_order,
        });
        if !self_closing {
            stack.push(node_index);
        }
    }

    if index < svg.len() {
        let text = decode_entities(&svg[index..], ctx, None);
        if let Some(last) = stack.last().copied() {
            nodes[last].text.push_str(&text);
        }
    }

    Ok(nodes)
}

fn find_tag_close(svg: &str, open: usize) -> Option<usize> {
    let bytes = svg.as_bytes();
    let mut quote: Option<u8> = None;
    let mut index = open + 1;
    while index < bytes.len() {
        match (bytes[index], quote) {
            (b'"' | b'\'', None) => quote = Some(bytes[index]),
            (c, Some(q)) if c == q => quote = None,
            (b'>', None) => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

fn parse_tag(raw: &str, ctx: &mut ImportContext) -> Result<Tag, SvgImportError> {
    let self_closing = raw.ends_with('/');
    let body = raw.trim_end_matches('/').trim();
    let name_end = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
    let name = body[..name_end].to_ascii_lowercase();
    if name.is_empty() {
        return Err(SvgImportError::new("xml.empty_tag", "empty SVG tag"));
    }
    Ok(Tag {
        name,
        attrs: parse_attrs(&body[name_end..], ctx)?,
        self_closing,
    })
}

fn parse_attrs(
    mut input: &str,
    ctx: &mut ImportContext,
) -> Result<Vec<(String, String)>, SvgImportError> {
    let mut out = Vec::new();
    while !input.trim_start().is_empty() {
        if out.len() >= ctx.limits.max_attribute_count_per_tag {
            return Err(SvgImportError::new(
                "limit.attribute_count",
                format!(
                    "tag exceeded {} attributes",
                    ctx.limits.max_attribute_count_per_tag
                ),
            ));
        }
        input = input.trim_start();
        let Some(eq) = input.find('=') else {
            break;
        };
        let key = input[..eq].trim().to_ascii_lowercase();
        input = input[eq + 1..].trim_start();
        if key.is_empty() || input.is_empty() {
            break;
        }

        let (value, rest) =
            if let Some(quote) = input.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let after_quote = &input[quote.len_utf8()..];
                match after_quote.find(quote) {
                    Some(end) => (&after_quote[..end], &after_quote[end + quote.len_utf8()..]),
                    None => (after_quote, ""),
                }
            } else {
                let end = input
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(input.len());
                (&input[..end], &input[end..])
            };

        if value.len() > ctx.limits.max_attribute_value_length {
            return Err(SvgImportError::new(
                "limit.attribute_value",
                format!(
                    "attribute value exceeded {} bytes",
                    ctx.limits.max_attribute_value_length
                ),
            ));
        }
        out.push((key, decode_entities(value, ctx, None)));
        input = rest;
    }
    Ok(out)
}

fn attr<'a>(tag: &'a Tag, key: &str) -> Option<&'a str> {
    tag.attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn collect_style_rules(
    nodes: &[Node],
    ctx: &mut ImportContext,
) -> Result<svg_core::SvgCssStyleSheet, SvgImportError> {
    let mut out = svg_core::SvgCssStyleSheet::default();
    for node in nodes.iter().filter(|n| n.tag.name == "style") {
        if node.text.len() > ctx.limits.max_style_bytes {
            return Err(SvgImportError::new(
                "limit.style_bytes",
                format!("style block exceeded {} bytes", ctx.limits.max_style_bytes),
            ));
        }
        let remaining_rules = MAX_CSS_RULES.saturating_sub(out.rules.len());
        let used_declarations = out
            .rules
            .iter()
            .map(|rule| rule.declarations.len())
            .sum::<usize>();
        let remaining_declarations = MAX_CSS_DECLARATIONS.saturating_sub(used_declarations);
        let mut parsed =
            svg_core::parse_css_stylesheet(&node.text, remaining_rules, remaining_declarations);
        let order_offset = out.rules.len();
        for rule in &mut parsed.rules {
            rule.source_order += order_offset;
        }
        if parsed.unsupported_selector_count > 0 {
            ctx.unsupported("complex CSS selector", Some(node));
        }
        if parsed.malformed_rule_count > 0 || parsed.dropped_declaration_count > 0 {
            ctx.warn(
                "css.malformed_rule",
                "malformed or unsupported CSS declarations were ignored",
                Some(node),
                SvgWarningSeverity::Warning,
            );
        }
        if parsed.dropped_rule_count > 0
            || out.rules.len() + parsed.rules.len() >= MAX_CSS_RULES
            || used_declarations
                + parsed
                    .rules
                    .iter()
                    .map(|rule| rule.declarations.len())
                    .sum::<usize>()
                >= MAX_CSS_DECLARATIONS
        {
            ctx.warn(
                "limit.css_rules",
                "CSS rules or declarations exceeded importer safety limits",
                Some(node),
                SvgWarningSeverity::Warning,
            );
        }
        out.unsupported_selector_count += parsed.unsupported_selector_count;
        out.malformed_rule_count += parsed.malformed_rule_count;
        out.dropped_rule_count += parsed.dropped_rule_count;
        out.dropped_declaration_count += parsed.dropped_declaration_count;
        out.rules.extend(parsed.rules);
    }
    Ok(out)
}

fn resolve_style(
    node: &Node,
    inherited: &Style,
    sheet: &svg_core::SvgCssStyleSheet,
    ctx: &mut ImportContext,
) -> Style {
    let mut style = inherited.clone();

    for key in [
        "color",
        "fill",
        "stroke",
        "stroke-width",
        "opacity",
        "display",
        "visibility",
        "font-size",
        "text-anchor",
        "dominant-baseline",
    ] {
        if let Some(value) = attr(&node.tag, key) {
            apply_style_decl(&mut style, key, value);
        }
    }

    let element = node.tag.name.as_str();
    let id = attr(&node.tag, "id");
    let classes = attr(&node.tag, "class").unwrap_or("");
    let mut matches: Vec<_> = sheet
        .rules
        .iter()
        .filter_map(|rule| {
            rule.matching_specificity(element, id, classes)
                .map(|specificity| (specificity, rule.source_order, rule))
        })
        .collect();
    matches.sort_by_key(|(specificity, order, _)| (*specificity, *order));
    for (_, _, rule) in matches {
        for declaration in &rule.declarations {
            apply_style_decl(&mut style, &declaration.name, &declaration.value);
        }
    }

    if let Some(inline) = attr(&node.tag, "style") {
        let (declarations, dropped) =
            svg_core::parse_style_declarations(inline, MAX_CSS_DECLARATIONS);
        if dropped > 0 {
            ctx.warn(
                "css.invalid_inline_declaration",
                "malformed or unsupported inline style declarations were ignored",
                Some(node),
                SvgWarningSeverity::Warning,
            );
        }
        for declaration in declarations {
            apply_style_decl(&mut style, &declaration.name, &declaration.value);
        }
    }

    let current_color = style.color.clone().unwrap_or_else(|| "black".to_owned());
    for paint in [&mut style.fill, &mut style.stroke] {
        if paint
            .as_deref()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("currentcolor"))
        {
            *paint = Some(current_color.clone());
        }
    }

    for paint in [&style.fill, &style.stroke].into_iter().flatten() {
        if paint.trim().starts_with("url(") {
            ctx.unsupported("paint server reference", Some(node));
        }
    }

    style
}

fn apply_style_decl(style: &mut Style, key: &str, value: &str) {
    match key {
        "display" => style.display_none = value.trim().eq_ignore_ascii_case("none"),
        "visibility" => style.visibility_hidden = value.trim().eq_ignore_ascii_case("hidden"),
        "opacity" => style.opacity = value.trim().parse::<f64>().ok(),
        "font-size" => style.font_size = parse_length(value, 16.0),
        "text-anchor" => style.text_anchor = Some(value.trim().to_owned()),
        "dominant-baseline" => style.dominant_baseline = Some(value.trim().to_owned()),
        "color" => style.color = Some(value.trim().to_owned()),
        "fill" => style.fill = Some(value.trim().to_owned()),
        "stroke" => style.stroke = Some(value.trim().to_owned()),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn import_node(
    node_id: usize,
    nodes: &[Node],
    rules: &svg_core::SvgCssStyleSheet,
    id_index: &HashMap<String, usize>,
    parent_state: ParseState,
    inherited_style: Style,
    use_stack: &mut Vec<usize>,
    widgets: &mut Vec<WidgetInstance>,
    ctx: &mut ImportContext,
) -> Result<(), SvgImportError> {
    if widgets.len() >= ctx.limits.max_generated_placeholder_count {
        return Err(SvgImportError::new(
            "limit.placeholder_count",
            format!(
                "generated placeholder count exceeded {}",
                ctx.limits.max_generated_placeholder_count
            ),
        ));
    }

    let node = &nodes[node_id];
    diagnose_unsupported(node, ctx);

    let mut state = parent_state;
    let next_transform = state
        .transform
        .multiply(parse_transform(attr(&node.tag, "transform").unwrap_or("")));
    if !next_transform.is_finite() {
        ctx.warn(
            "transform.invalid",
            "non-finite transform ignored for this node",
            Some(node),
            SvgWarningSeverity::Warning,
        );
    } else {
        if next_transform.is_extreme() {
            ctx.warn(
                "transform.extreme",
                "extreme transform approximated; placeholder bounds may be imprecise",
                Some(node),
                SvgWarningSeverity::Warning,
            );
        }
        state.transform = next_transform;
    }

    if node.tag.name == "svg" {
        update_viewport(node, &mut state);
    }

    let style = resolve_style(node, &inherited_style, rules, ctx);
    let hidden_container =
        is_hidden_container(&node.tag.name) && !(state.expanding_use && node.tag.name == "symbol");
    let hidden = state.hidden
        || hidden_container
        || style.display_none
        || style.visibility_hidden
        || style.opacity == Some(0.0);

    if hidden {
        diagnose_descendants(node, nodes, ctx);
        if is_supported_visual(&node.tag.name) {
            ctx.skip();
        }
        return Ok(());
    }

    match node.tag.name.as_str() {
        "use" => expand_use(
            node, nodes, rules, id_index, state, style, use_stack, widgets, ctx,
        )?,
        "text" => {
            if let Some(widget) = text_widget(node, nodes, state, &style, ctx) {
                widgets.push(widget);
            } else {
                ctx.skip();
            }
        }
        name if is_supported_shape(name) => {
            if let Some(widget) = shape_widget(node, state, &style, ctx)? {
                widgets.push(widget);
            } else {
                ctx.skip();
            }
        }
        name if is_container(name) => {
            for &child in &node.children {
                import_node(
                    child,
                    nodes,
                    rules,
                    id_index,
                    state,
                    style.clone(),
                    use_stack,
                    widgets,
                    ctx,
                )?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn diagnose_unsupported(node: &Node, ctx: &mut ImportContext) {
    match node.tag.name.as_str() {
        "script" => ctx.unsupported("script execution", Some(node)),
        "foreignobject" => ctx.unsupported("foreignObject", Some(node)),
        "textpath" => ctx.unsupported("textPath", Some(node)),
        "filter" => ctx.unsupported("filter", Some(node)),
        "animate" | "animatetransform" | "animatemotion" | "set" | "mpath" => {
            ctx.unsupported("animation", Some(node))
        }
        "mask" => ctx.unsupported("mask", Some(node)),
        "clippath" => ctx.unsupported("clipPath", Some(node)),
        "lineargradient" => ctx.unsupported("linearGradient", Some(node)),
        "radialgradient" => ctx.unsupported("radialGradient", Some(node)),
        "pattern" => ctx.unsupported("pattern", Some(node)),
        _ => {}
    }

    for (key, feature) in [
        ("filter", "filter attribute"),
        ("mask", "mask attribute"),
        ("clip-path", "clip-path attribute"),
    ] {
        if attr(&node.tag, key).is_some() {
            ctx.unsupported(feature, Some(node));
        }
    }
}

fn diagnose_descendants(node: &Node, nodes: &[Node], ctx: &mut ImportContext) {
    for &child_id in &node.children {
        let child = &nodes[child_id];
        diagnose_unsupported(child, ctx);
        diagnose_descendants(child, nodes, ctx);
    }
}

fn update_viewport(node: &Node, state: &mut ParseState) {
    let parent_w = state.viewport_w;
    let parent_h = state.viewport_h;
    let x = attr(&node.tag, "x")
        .and_then(|value| parse_length(value, parent_w))
        .unwrap_or(0.0);
    let y = attr(&node.tag, "y")
        .and_then(|value| parse_length(value, parent_h))
        .unwrap_or(0.0);
    let width = attr(&node.tag, "width")
        .and_then(|value| parse_length(value, parent_w))
        .unwrap_or(parent_w);
    let height = attr(&node.tag, "height")
        .and_then(|value| parse_length(value, parent_h))
        .unwrap_or(parent_h);
    let width = width.max(0.0);
    let height = height.max(0.0);

    if let Some(view_box) = attr(&node.tag, "viewBox").or_else(|| attr(&node.tag, "viewbox")) {
        if let Some(nums) = parse_numbers(view_box).filter(|n| n.len() >= 4) {
            let aspect_ratio = svg_core::parse_preserve_aspect_ratio(
                attr(&node.tag, "preserveaspectratio").unwrap_or(""),
            );
            if let Some(view_transform) = svg_core::viewbox_transform(
                [nums[0], nums[1], nums[2], nums[3]],
                [x, y, width, height],
                aspect_ratio,
            ) {
                state.transform = state.transform.multiply(view_transform);
                state.viewport_w = nums[2].abs();
                state.viewport_h = nums[3].abs();
                return;
            }
        }
    }

    state.transform = state.transform.multiply(Matrix::translate(x, y));
    state.viewport_w = width;
    state.viewport_h = height;
}

#[allow(clippy::too_many_arguments)]
fn expand_use(
    node: &Node,
    nodes: &[Node],
    rules: &svg_core::SvgCssStyleSheet,
    id_index: &HashMap<String, usize>,
    mut state: ParseState,
    style: Style,
    use_stack: &mut Vec<usize>,
    widgets: &mut Vec<WidgetInstance>,
    ctx: &mut ImportContext,
) -> Result<(), SvgImportError> {
    let Some(href) = attr(&node.tag, "href").or_else(|| attr(&node.tag, "xlink:href")) else {
        ctx.warn(
            "use.missing_href",
            "use element has no href",
            Some(node),
            SvgWarningSeverity::Warning,
        );
        ctx.skip();
        return Ok(());
    };
    if is_external_ref(href) {
        ctx.unsupported("external use reference", Some(node));
        ctx.skip();
        return Ok(());
    }
    let Some(local_id) = href.strip_prefix('#') else {
        ctx.unsupported("unsupported use reference", Some(node));
        ctx.skip();
        return Ok(());
    };
    let Some(&target_id) = id_index.get(local_id) else {
        ctx.warn(
            "use.missing_target",
            format!("use target #{local_id} was not found"),
            Some(node),
            SvgWarningSeverity::Warning,
        );
        ctx.skip();
        return Ok(());
    };
    if use_stack.contains(&target_id) {
        ctx.unsupported("use cycle", Some(node));
        ctx.skip();
        return Ok(());
    }
    if use_stack.len() >= ctx.limits.max_use_expansion_depth {
        return Err(SvgImportError::new(
            "limit.use_depth",
            format!(
                "use expansion exceeded {} levels",
                ctx.limits.max_use_expansion_depth
            ),
        ));
    }

    let x = attr(&node.tag, "x")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let y = attr(&node.tag, "y")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    state.transform = state.transform.multiply(Matrix::translate(x, y));
    state.expanding_use = true;
    use_stack.push(target_id);
    import_node(
        target_id, nodes, rules, id_index, state, style, use_stack, widgets, ctx,
    )?;
    use_stack.pop();
    Ok(())
}

fn is_supported_shape(name: &str) -> bool {
    matches!(
        name,
        "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" | "path" | "image"
    )
}

fn is_supported_visual(name: &str) -> bool {
    is_supported_shape(name) || matches!(name, "text" | "use")
}

fn is_container(name: &str) -> bool {
    matches!(
        name,
        "svg"
            | "g"
            | "a"
            | "symbol"
            | "defs"
            | "marker"
            | "mask"
            | "clippath"
            | "pattern"
            | "lineargradient"
            | "radialgradient"
    )
}

fn is_hidden_container(name: &str) -> bool {
    matches!(
        name,
        "defs"
            | "symbol"
            | "marker"
            | "mask"
            | "clippath"
            | "pattern"
            | "lineargradient"
            | "radialgradient"
            | "style"
            | "script"
    )
}

fn shape_widget(
    node: &Node,
    state: ParseState,
    style: &Style,
    ctx: &mut ImportContext,
) -> Result<Option<WidgetInstance>, SvgImportError> {
    let mut warning_flags = Vec::new();
    if node.tag.name == "image" {
        let href = attr(&node.tag, "href").or_else(|| attr(&node.tag, "xlink:href"));
        if !image_ref_allowed(href, node, ctx, &mut warning_flags) {
            return Ok(None);
        }
    }

    let bounds = match node.tag.name.as_str() {
        "rect" => rect_bounds(node, state),
        "circle" => circle_bounds(node, state),
        "ellipse" => ellipse_bounds(node, state),
        "line" => line_bounds(node, state),
        "polyline" | "polygon" => points_bounds(node, state),
        "path" => path_bounds(node, state, ctx, &mut warning_flags)?,
        "image" => rect_bounds(node, state),
        _ => None,
    };

    let Some(bounds) = bounds else {
        ctx.warn(
            "geometry.missing_bounds",
            "supported SVG element had no usable bounds",
            Some(node),
            SvgWarningSeverity::Warning,
        );
        return Ok(None);
    };

    let rect = bounds.rect();
    let mut label = format!("svg {}", node.tag.name);
    if let Some(id) = attr(&node.tag, "id").filter(|id| !id.trim().is_empty()) {
        label.push(' ');
        label.push_str(id.trim());
    }
    let fill_color = style_color(
        style.fill.as_deref(),
        style.opacity,
        node,
        ctx,
        &mut warning_flags,
    );
    let stroke_color = style_color(
        style.stroke.as_deref(),
        style.opacity,
        node,
        ctx,
        &mut warning_flags,
    );

    Ok(Some(WidgetInstance {
        id: deterministic_uuid(node, &rect),
        kind: WidgetKind::Frame,
        rect,
        props: WidgetProps {
            label,
            ..Default::default()
        },
        state_binding: None,
        import_metadata: Some(metadata_for(node, state, warning_flags)),
        fg_color: stroke_color.or(fill_color),
        bg_color: fill_color,
        ..Default::default()
    }))
}

fn image_ref_allowed(
    href: Option<&str>,
    node: &Node,
    ctx: &mut ImportContext,
    warning_flags: &mut Vec<String>,
) -> bool {
    let Some(href) = href else {
        ctx.warn(
            "image.missing_href",
            "image has no embedded data URI",
            Some(node),
            SvgWarningSeverity::Warning,
        );
        warning_flags.push("missing-image-href".to_owned());
        ctx.skip();
        return false;
    };
    let Some(data) = href.strip_prefix("data:") else {
        ctx.unsupported("external image reference", Some(node));
        warning_flags.push("external-image".to_owned());
        ctx.skip();
        return false;
    };
    let supported = data.starts_with("image/png;base64,") || data.starts_with("image/jpeg;base64,");
    if !supported {
        ctx.unsupported("unsupported image MIME type", Some(node));
        warning_flags.push("unsupported-image-mime".to_owned());
        ctx.skip();
        return false;
    }
    let Some((_, payload)) = href.split_once(',') else {
        ctx.warn(
            "image.malformed_data_uri",
            "image data URI has no payload",
            Some(node),
            SvgWarningSeverity::Warning,
        );
        warning_flags.push("malformed-image-data".to_owned());
        ctx.skip();
        return false;
    };
    if payload.len() > ctx.limits.max_image_data_uri_bytes {
        ctx.unsupported("oversized image data URI", Some(node));
        warning_flags.push("oversized-image-data".to_owned());
        ctx.skip();
        return false;
    }
    if !payload
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
    {
        ctx.warn(
            "image.invalid_base64",
            "image data URI contains non-base64 characters",
            Some(node),
            SvgWarningSeverity::Warning,
        );
        warning_flags.push("invalid-image-base64".to_owned());
        ctx.skip();
        return false;
    }
    true
}

fn text_widget(
    node: &Node,
    nodes: &[Node],
    state: ParseState,
    style: &Style,
    ctx: &mut ImportContext,
) -> Option<WidgetInstance> {
    let mut warning_flags = Vec::new();
    if attr(&node.tag, "font-family").is_none() {
        ctx.warn(
            "text.missing_font",
            "text import uses placeholder font metrics",
            Some(node),
            SvgWarningSeverity::Info,
        );
        warning_flags.push("missing-font".to_owned());
    }

    let label = collapse_ws(&flatten_text(node.index, nodes, ctx));
    if label.is_empty() {
        return None;
    }
    ctx.text_element_count += 1;
    ctx.text_character_count += label.chars().count();

    let mut x = attr(&node.tag, "x")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let mut y = attr(&node.tag, "y")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    x += attr(&node.tag, "dx")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    y += attr(&node.tag, "dy")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);

    let font_size = style
        .font_size
        .or_else(|| attr(&node.tag, "font-size").and_then(|v| parse_length(v, state.viewport_h)))
        .unwrap_or(16.0)
        .clamp(8.0, 96.0);
    let mut width = (label.chars().count() as f64 * font_size * 0.6).max(MIN_PLACEHOLDER_SIZE);
    let height = (font_size * 1.25).max(MIN_PLACEHOLDER_SIZE);

    match style.text_anchor.as_deref() {
        Some("middle") => x -= width / 2.0,
        Some("end") => x -= width,
        _ => {}
    }
    match style.dominant_baseline.as_deref() {
        Some("middle") | Some("central") => y -= height / 2.0,
        Some("hanging") => {}
        Some(_) => {
            ctx.warn(
                "text.baseline",
                "dominant-baseline approximated",
                Some(node),
                SvgWarningSeverity::Info,
            );
            warning_flags.push("baseline-approx".to_owned());
            y -= font_size;
        }
        None => y -= font_size,
    }

    if width < MIN_PLACEHOLDER_SIZE {
        width = MIN_PLACEHOLDER_SIZE;
    }
    let bounds = Bounds::new(x, y, width, height)?.transform(state.transform);
    let rect = bounds.rect();
    let fill_color = style_color(
        style.fill.as_deref(),
        style.opacity,
        node,
        ctx,
        &mut warning_flags,
    );

    Some(WidgetInstance {
        id: deterministic_uuid(node, &rect),
        kind: WidgetKind::Label,
        rect,
        props: WidgetProps {
            label,
            ..Default::default()
        },
        state_binding: None,
        import_metadata: Some(metadata_for(node, state, warning_flags)),
        fg_color: fill_color,
        ..Default::default()
    })
}

fn flatten_text(node_id: usize, nodes: &[Node], ctx: &mut ImportContext) -> String {
    let node = &nodes[node_id];
    let mut text = node.text.clone();
    for &child_id in &node.children {
        let child = &nodes[child_id];
        match child.tag.name.as_str() {
            "tspan" => {
                if ["x", "y", "dx", "dy", "rotate", "textlength", "lengthadjust"]
                    .into_iter()
                    .any(|key| attr(&child.tag, key).is_some())
                {
                    ctx.warn(
                        "text.complex_tspan",
                        "positioned or adjusted tspan flattened into editable placeholder text; source SVG preserved for exact layout",
                        Some(child),
                        SvgWarningSeverity::Warning,
                    );
                }
                if attr(&child.tag, "style").is_some() || attr(&child.tag, "class").is_some() {
                    ctx.warn(
                        "text.tspan_style",
                        "tspan style was flattened into one editable text placeholder",
                        Some(child),
                        SvgWarningSeverity::Info,
                    );
                }
                text.push(' ');
                text.push_str(&flatten_text(child_id, nodes, ctx));
            }
            "textpath" => {
                ctx.unsupported("textPath", Some(child));
            }
            _ => {}
        }
    }
    decode_entities(&text, ctx, Some(node))
}

fn metadata_for(node: &Node, state: ParseState, warning_flags: Vec<String>) -> SvgImportMetadata {
    SvgImportMetadata {
        element_name: node.tag.name.clone(),
        original_id: attr(&node.tag, "id").map(ToOwned::to_owned),
        original_class: attr(&node.tag, "class").map(ToOwned::to_owned),
        source_order: node.source_order,
        transform_summary: state.transform.summary(),
        warning_flags,
    }
}

fn rect_bounds(node: &Node, state: ParseState) -> Option<Bounds> {
    let x = attr(&node.tag, "x")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let y = attr(&node.tag, "y")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    let w = attr(&node.tag, "width").and_then(|v| parse_length(v, state.viewport_w))?;
    let h = attr(&node.tag, "height").and_then(|v| parse_length(v, state.viewport_h))?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Bounds::new(x, y, w, h).map(|b| b.transform(state.transform))
}

fn circle_bounds(node: &Node, state: ParseState) -> Option<Bounds> {
    let cx = attr(&node.tag, "cx")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let cy = attr(&node.tag, "cy")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    let r = attr(&node.tag, "r").and_then(|v| parse_length(v, state.viewport_w))?;
    if r <= 0.0 {
        return None;
    }
    Bounds::new(cx - r, cy - r, r * 2.0, r * 2.0).map(|b| b.transform(state.transform))
}

fn ellipse_bounds(node: &Node, state: ParseState) -> Option<Bounds> {
    let cx = attr(&node.tag, "cx")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let cy = attr(&node.tag, "cy")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    let rx = attr(&node.tag, "rx").and_then(|v| parse_length(v, state.viewport_w))?;
    let ry = attr(&node.tag, "ry").and_then(|v| parse_length(v, state.viewport_h))?;
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    Bounds::new(cx - rx, cy - ry, rx * 2.0, ry * 2.0).map(|b| b.transform(state.transform))
}

fn line_bounds(node: &Node, state: ParseState) -> Option<Bounds> {
    let x1 = attr(&node.tag, "x1")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let y1 = attr(&node.tag, "y1")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    let x2 = attr(&node.tag, "x2")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(0.0);
    let y2 = attr(&node.tag, "y2")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(0.0);
    Bounds::from_points(&[state.transform.apply(x1, y1), state.transform.apply(x2, y2)])
}

fn points_bounds(node: &Node, state: ParseState) -> Option<Bounds> {
    let values = parse_numbers(attr(&node.tag, "points")?)?;
    let points: Vec<_> = values
        .chunks_exact(2)
        .map(|pair| state.transform.apply(pair[0], pair[1]))
        .collect();
    Bounds::from_points(&points)
}

fn path_bounds(
    node: &Node,
    state: ParseState,
    ctx: &mut ImportContext,
    warning_flags: &mut Vec<String>,
) -> Result<Option<Bounds>, SvgImportError> {
    let Some(data) = attr(&node.tag, "d") else {
        return Ok(None);
    };
    let points = parse_path_points(
        data,
        ctx.limits.max_path_command_count,
        ctx,
        node,
        warning_flags,
    )?;
    let transformed: Vec<_> = points
        .into_iter()
        .map(|(x, y)| state.transform.apply(x, y))
        .collect();
    Ok(Bounds::from_points(&transformed))
}

fn parse_length(value: &str, percent_base: f64) -> Option<f64> {
    svg_core::resolve_length(value, svg_core::SvgLengthContext::user_units(percent_base))
}

fn parse_transform(value: &str) -> Matrix {
    Matrix::parse_transform(value)
}

fn parse_numbers(value: &str) -> Option<Vec<f64>> {
    let nums = svg_core::parse_numbers(value);
    (!nums.is_empty()).then_some(nums)
}

fn parse_path_points(
    data: &str,
    max_commands: usize,
    ctx: &mut ImportContext,
    node: &Node,
    warning_flags: &mut Vec<String>,
) -> Result<Vec<(f64, f64)>, SvgImportError> {
    let tokens = svg_core::tokenize_path_data(data);
    let mut points = Vec::new();
    let mut index = 0;
    let mut command = 'M';
    let mut current = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    let mut last_cubic_ctrl: Option<(f64, f64)> = None;
    let mut last_quad_ctrl: Option<(f64, f64)> = None;
    let mut command_count = 0usize;

    while index < tokens.len() {
        if command_count >= max_commands {
            return Err(SvgImportError::new(
                "limit.path_commands",
                format!("path exceeded {max_commands} commands"),
            ));
        }
        if let svg_core::SvgPathToken::Command(c) = tokens[index] {
            command = c;
            index += 1;
        }
        command_count += 1;
        let relative = command.is_ascii_lowercase();
        match command.to_ascii_uppercase() {
            'M' => {
                let mut first = true;
                while let Some(pair) = read_pair(&tokens, &mut index) {
                    current = apply_relative(pair, current, relative);
                    if first {
                        start = current;
                        first = false;
                    }
                    points.push(current);
                    command = if relative { 'l' } else { 'L' };
                }
            }
            'L' | 'T' => {
                while let Some(pair) = read_pair(&tokens, &mut index) {
                    if command.eq_ignore_ascii_case(&'T') {
                        let ctrl = last_quad_ctrl
                            .map(|c| reflect(c, current))
                            .unwrap_or(current);
                        let end = apply_relative(pair, current, relative);
                        sample_quad(current, ctrl, end, &mut points);
                        last_quad_ctrl = Some(ctrl);
                        current = end;
                    } else {
                        current = apply_relative(pair, current, relative);
                        points.push(current);
                    }
                }
            }
            'H' => {
                while let Some(x) = read_number(&tokens, &mut index) {
                    current.0 = if relative { current.0 + x } else { x };
                    points.push(current);
                }
            }
            'V' => {
                while let Some(y) = read_number(&tokens, &mut index) {
                    current.1 = if relative { current.1 + y } else { y };
                    points.push(current);
                }
            }
            'C' => {
                while let Some(nums) = read_numbers(&tokens, &mut index, 6) {
                    let c1 = apply_relative((nums[0], nums[1]), current, relative);
                    let c2 = apply_relative((nums[2], nums[3]), current, relative);
                    let end = apply_relative((nums[4], nums[5]), current, relative);
                    sample_cubic(current, c1, c2, end, &mut points);
                    last_cubic_ctrl = Some(c2);
                    current = end;
                }
            }
            'S' => {
                while let Some(nums) = read_numbers(&tokens, &mut index, 4) {
                    let c1 = last_cubic_ctrl
                        .map(|c| reflect(c, current))
                        .unwrap_or(current);
                    let c2 = apply_relative((nums[0], nums[1]), current, relative);
                    let end = apply_relative((nums[2], nums[3]), current, relative);
                    sample_cubic(current, c1, c2, end, &mut points);
                    last_cubic_ctrl = Some(c2);
                    current = end;
                }
            }
            'Q' => {
                while let Some(nums) = read_numbers(&tokens, &mut index, 4) {
                    let c1 = apply_relative((nums[0], nums[1]), current, relative);
                    let end = apply_relative((nums[2], nums[3]), current, relative);
                    sample_quad(current, c1, end, &mut points);
                    last_quad_ctrl = Some(c1);
                    current = end;
                }
            }
            'A' => {
                while let Some(nums) = read_numbers(&tokens, &mut index, 7) {
                    let end = apply_relative((nums[5], nums[6]), current, relative);
                    sample_arc(
                        current,
                        nums[0].abs(),
                        nums[1].abs(),
                        nums[2],
                        nums[3].abs() >= 0.5,
                        nums[4].abs() >= 0.5,
                        end,
                        &mut points,
                    );
                    current = end;
                }
            }
            'Z' => {
                current = start;
                points.push(current);
                command = 'L';
            }
            _ => {
                ctx.warn(
                    "path.unsupported_command",
                    format!("unsupported path command {command} skipped"),
                    Some(node),
                    SvgWarningSeverity::Warning,
                );
                warning_flags.push("path-unsupported-command".to_owned());
                skip_until_next_command(&tokens, &mut index);
            }
        }
    }

    if points.is_empty() && !tokens.is_empty() {
        ctx.warn(
            "path.malformed",
            "path had no recoverable geometry",
            Some(node),
            SvgWarningSeverity::Warning,
        );
        warning_flags.push("path-malformed".to_owned());
    }
    Ok(points)
}

fn read_number(tokens: &[svg_core::SvgPathToken], index: &mut usize) -> Option<f64> {
    match tokens.get(*index)? {
        svg_core::SvgPathToken::Number(num) => {
            *index += 1;
            Some(*num)
        }
        svg_core::SvgPathToken::Command(_) => None,
    }
}

fn read_pair(tokens: &[svg_core::SvgPathToken], index: &mut usize) -> Option<(f64, f64)> {
    Some((read_number(tokens, index)?, read_number(tokens, index)?))
}

fn read_numbers(
    tokens: &[svg_core::SvgPathToken],
    index: &mut usize,
    count: usize,
) -> Option<Vec<f64>> {
    let start = *index;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        match read_number(tokens, index) {
            Some(num) => out.push(num),
            None => {
                *index = start;
                return None;
            }
        }
    }
    Some(out)
}

fn skip_until_next_command(tokens: &[svg_core::SvgPathToken], index: &mut usize) {
    while *index < tokens.len() && !matches!(tokens[*index], svg_core::SvgPathToken::Command(_)) {
        *index += 1;
    }
}

fn apply_relative(point: (f64, f64), current: (f64, f64), relative: bool) -> (f64, f64) {
    if relative {
        (current.0 + point.0, current.1 + point.1)
    } else {
        point
    }
}

fn reflect(point: (f64, f64), around: (f64, f64)) -> (f64, f64) {
    (around.0 * 2.0 - point.0, around.1 * 2.0 - point.1)
}

fn sample_quad(p0: (f64, f64), c: (f64, f64), p1: (f64, f64), points: &mut Vec<(f64, f64)>) {
    for i in 1..=24 {
        let t = i as f64 / 24.0;
        let mt = 1.0 - t;
        points.push((
            mt * mt * p0.0 + 2.0 * mt * t * c.0 + t * t * p1.0,
            mt * mt * p0.1 + 2.0 * mt * t * c.1 + t * t * p1.1,
        ));
    }
}

fn sample_cubic(
    p0: (f64, f64),
    c1: (f64, f64),
    c2: (f64, f64),
    p1: (f64, f64),
    points: &mut Vec<(f64, f64)>,
) {
    for i in 1..=32 {
        let t = i as f64 / 32.0;
        let mt = 1.0 - t;
        points.push((
            mt.powi(3) * p0.0
                + 3.0 * mt * mt * t * c1.0
                + 3.0 * mt * t * t * c2.0
                + t.powi(3) * p1.0,
            mt.powi(3) * p0.1
                + 3.0 * mt * mt * t * c1.1
                + 3.0 * mt * t * t * c2.1
                + t.powi(3) * p1.1,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_arc(
    start: (f64, f64),
    mut rx: f64,
    mut ry: f64,
    x_axis_rotation: f64,
    large_arc: bool,
    sweep: bool,
    end: (f64, f64),
    points: &mut Vec<(f64, f64)>,
) {
    if rx <= 0.0 || ry <= 0.0 || start == end {
        points.push(end);
        return;
    }
    let phi = x_axis_rotation.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();
    let dx = (start.0 - end.0) / 2.0;
    let dy = (start.1 - end.1) / 2.0;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;
    let lambda = x1p.powi(2) / rx.powi(2) + y1p.powi(2) / ry.powi(2);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let num = rx.powi(2) * ry.powi(2) - rx.powi(2) * y1p.powi(2) - ry.powi(2) * x1p.powi(2);
    let den = rx.powi(2) * y1p.powi(2) + ry.powi(2) * x1p.powi(2);
    let coef = sign * (num / den.max(f64::EPSILON)).max(0.0).sqrt();
    let cxp = coef * (rx * y1p / ry);
    let cyp = coef * (-ry * x1p / rx);
    let cx = cos_phi * cxp - sin_phi * cyp + (start.0 + end.0) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (start.1 + end.1) / 2.0;

    let theta1 = angle((1.0, 0.0), ((x1p - cxp) / rx, (y1p - cyp) / ry));
    let mut delta = angle(
        ((x1p - cxp) / rx, (y1p - cyp) / ry),
        ((-x1p - cxp) / rx, (-y1p - cyp) / ry),
    );
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }
    let steps = ((delta.abs() * rx.max(ry) / ARC_TOLERANCE_PX).ceil() as usize).clamp(8, 128);
    for i in 1..=steps {
        let t = theta1 + delta * i as f64 / steps as f64;
        let x = cos_phi * rx * t.cos() - sin_phi * ry * t.sin() + cx;
        let y = sin_phi * rx * t.cos() + cos_phi * ry * t.sin() + cy;
        points.push((x, y));
    }
}

fn angle(u: (f64, f64), v: (f64, f64)) -> f64 {
    let dot = u.0 * v.0 + u.1 * v.1;
    let det = u.0 * v.1 - u.1 * v.0;
    det.atan2(dot)
}

fn normalize_widgets(widgets: &mut [WidgetInstance]) {
    let min_x = widgets
        .iter()
        .map(|w| w.rect.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = widgets
        .iter()
        .map(|w| w.rect.y)
        .fold(f32::INFINITY, f32::min);

    if !min_x.is_finite() || !min_y.is_finite() {
        return;
    }

    for widget in widgets {
        widget.rect.x = (widget.rect.x - min_x + 20.0).max(0.0);
        widget.rect.y = (widget.rect.y - min_y + 20.0).max(0.0);
    }
}

fn decode_entities(value: &str, ctx: &mut ImportContext, node: Option<&Node>) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        let Some(end) = after.find(';') else {
            out.push('&');
            rest = after;
            continue;
        };
        let entity = &after[..end];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                out.push('&');
                out.push_str(entity);
                out.push(';');
                ctx.warn(
                    "xml.unknown_entity",
                    format!("unknown XML entity &{entity}; left literal"),
                    node,
                    SvgWarningSeverity::Warning,
                );
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn style_color(
    paint: Option<&str>,
    opacity: Option<f64>,
    node: &Node,
    ctx: &mut ImportContext,
    warning_flags: &mut Vec<String>,
) -> Option<[u8; 3]> {
    let color = parse_color(paint?)?;
    let Some(opacity) = opacity else {
        return Some(color);
    };
    if (opacity - 1.0).abs() < f64::EPSILON {
        return Some(color);
    }
    ctx.warn(
        "style.opacity_approx",
        "opacity approximated into a solid RGB placeholder color; source SVG preserved for exact alpha",
        Some(node),
        SvgWarningSeverity::Info,
    );
    warning_flags.push("opacity-approx".to_owned());
    let factor = opacity.clamp(0.0, 1.0);
    Some([
        (color[0] as f64 * factor).round().clamp(0.0, 255.0) as u8,
        (color[1] as f64 * factor).round().clamp(0.0, 255.0) as u8,
        (color[2] as f64 * factor).round().clamp(0.0, 255.0) as u8,
    ])
}

fn parse_color(value: &str) -> Option<[u8; 3]> {
    svg_core::parse_rgb(value)
}

fn collapse_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_external_ref(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http:")
        || lower.starts_with("https:")
        || lower.starts_with("file:")
        || lower.starts_with("ftp:")
        || lower.starts_with("//")
        || lower.contains(":\\")
        || lower.starts_with('/')
        || (!lower.starts_with('#') && !lower.starts_with("data:"))
}

fn deterministic_uuid(node: &Node, rect: &Rect) -> Uuid {
    let mut hash = 0xcbf29ce484222325u64;
    fn feed(hash: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *hash ^= *b as u64;
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    feed(&mut hash, node.tag.name.as_bytes());
    feed(&mut hash, node.source_order.to_le_bytes().as_slice());
    if let Some(id) = attr(&node.tag, "id") {
        feed(&mut hash, id.as_bytes());
    }
    feed(&mut hash, rect.x.to_bits().to_le_bytes().as_slice());
    feed(&mut hash, rect.y.to_bits().to_le_bytes().as_slice());
    feed(&mut hash, rect.w.to_bits().to_le_bytes().as_slice());
    feed(&mut hash, rect.h.to_bits().to_le_bytes().as_slice());
    let second = hash.rotate_left(17) ^ 0x9e3779b97f4a7c15;
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hash.to_le_bytes());
    bytes[8..].copy_from_slice(&second.to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_basic_shapes_and_text() {
        let svg = r#"
            <svg width="200" height="100" viewBox="0 0 100 50">
                <rect id="panel" x="5" y="5" width="40" height="20"/>
                <circle cx="70" cy="25" r="10"/>
                <text x="10" y="45" font-size="8">Hello &amp; Rohkai</text>
            </svg>
        "#;

        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 3);
        assert_eq!(output.widgets[0].kind, WidgetKind::Frame);
        assert_eq!(output.widgets[2].kind, WidgetKind::Label);
        assert_eq!(output.widgets[2].props.label, "Hello & Rohkai");
        assert_eq!(output.report.fidelity, SvgFidelity::Medium);
    }

    #[test]
    fn nested_viewport_state_honors_meet_and_none_mapping() {
        let node = |preserve_aspect_ratio: &str| Node {
            index: 1,
            children: Vec::new(),
            tag: Tag {
                name: "svg".to_owned(),
                attrs: vec![
                    ("x".to_owned(), "20".to_owned()),
                    ("y".to_owned(), "10".to_owned()),
                    ("width".to_owned(), "40".to_owned()),
                    ("height".to_owned(), "60".to_owned()),
                    ("viewbox".to_owned(), "0 0 10 10".to_owned()),
                    (
                        "preserveaspectratio".to_owned(),
                        preserve_aspect_ratio.to_owned(),
                    ),
                ],
                self_closing: false,
            },
            text: String::new(),
            source_order: 1,
        };

        let mut meet = ParseState {
            viewport_w: 100.0,
            viewport_h: 100.0,
            ..Default::default()
        };
        update_viewport(&node("xMidYMid meet"), &mut meet);
        assert_eq!(meet.transform.apply(0.0, 0.0), (20.0, 20.0));
        assert_eq!(meet.transform.apply(10.0, 10.0), (60.0, 60.0));
        assert_eq!((meet.viewport_w, meet.viewport_h), (10.0, 10.0));

        let mut none = ParseState {
            viewport_w: 100.0,
            viewport_h: 100.0,
            ..Default::default()
        };
        update_viewport(&node("none"), &mut none);
        assert_eq!(none.transform.apply(0.0, 0.0), (20.0, 10.0));
        assert_eq!(none.transform.apply(10.0, 10.0), (60.0, 70.0));
        assert_eq!((none.viewport_w, none.viewport_h), (10.0, 10.0));
    }

    #[test]
    fn rejects_malicious_doctype_and_entities() {
        let svg = r#"<!DOCTYPE svg [ <!ENTITY xxe SYSTEM "file:///etc/passwd"> ]><svg/>"#;
        let err = import_svg_template(svg, SvgImportOptions::default()).unwrap_err();
        assert_eq!(err.code, "xml.doctype");
    }

    #[test]
    fn enforces_nesting_and_attribute_limits() {
        let limits = SvgImportLimits {
            max_nesting_depth: 2,
            ..SvgImportLimits::default()
        };
        let svg = "<svg><g><g><rect width=\"10\" height=\"10\"/></g></g></svg>";
        let err = import_svg_template(
            svg,
            SvgImportOptions {
                limits,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code, "limit.nesting_depth");

        let limits = SvgImportLimits {
            max_attribute_value_length: 4,
            ..SvgImportLimits::default()
        };
        let err = import_svg_template(
            "<svg><rect id=\"abcde\" width=\"10\" height=\"10\"/></svg>",
            SvgImportOptions {
                limits,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code, "limit.attribute_value");
    }

    #[test]
    fn honors_group_transform_hidden_and_style_classes() {
        let svg = r#"
            <svg width="100" height="100">
                <style>.hide { display:none } .show { visibility:visible }</style>
                <defs><rect width="1000" height="1000"/></defs>
                <rect class="hide" width="10" height="10"/>
                <g transform="translate(10,20) scale(2)">
                    <path class="show" d="M 0 0 L 10 0 L 10 10 Z"/>
                </g>
            </svg>
        "#;

        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 1);
        assert!(output.widgets[0].rect.w >= 20.0);
        assert!(output.report.skipped_element_count >= 1);
    }

    #[test]
    fn tier1_css_specificity_and_current_color_match_rasterizer_rules() {
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
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        let colors: Vec<Option<[u8; 3]>> = output
            .widgets
            .iter()
            .map(|widget| widget.fg_color)
            .collect();

        assert_eq!(
            colors,
            vec![Some([255, 0, 0]), Some([0, 255, 0]), Some([17, 34, 51])]
        );
        assert!(!output
            .report
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "complex CSS selector"));
    }

    #[test]
    fn expands_symbol_use_and_detects_cycles() {
        let svg = r##"
            <svg width="100" height="100">
                <symbol id="icon"><rect width="12" height="8"/></symbol>
                <use href="#icon" x="20" y="30"/>
            </svg>
        "##;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 1);
        assert_eq!(
            output.widgets[0]
                .import_metadata
                .as_ref()
                .unwrap()
                .original_id
                .as_deref(),
            None
        );

        let svg = r##"
            <svg><symbol id="a"><use href="#b"/></symbol><symbol id="b"><use href="#a"/></symbol><use href="#a"/></svg>
        "##;
        let err = import_svg_template(svg, SvgImportOptions::default()).unwrap_err();
        assert_eq!(err.code, "empty_import");
    }

    #[test]
    fn parses_compact_relative_paths_and_arcs() {
        let svg = r#"<svg><path d="M10-20l.5.6A5 5 0 0 1 30 30Z"/></svg>"#;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 1);
        assert!(output.widgets[0].rect.w >= MIN_PLACEHOLDER_SIZE as f32);
        assert!(output.widgets[0].rect.h >= MIN_PLACEHOLDER_SIZE as f32);
    }

    #[test]
    fn shared_path_tokenizer_preserves_unsupported_command_warning() {
        let svg = r#"<svg><path d="M0 0 R5 5 L10 10"/></svg>"#;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();

        assert_eq!(output.widgets.len(), 1);
        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "path.unsupported_command"));
    }

    #[test]
    fn warns_on_unsupported_features_and_image_policy() {
        let svg = r#"
            <svg>
                <foreignObject width="10" height="10"/>
                <image href="https://example.com/a.png" width="10" height="10"/>
                <image href="data:image/png;base64,QUJD" width="10" height="10"/>
            </svg>
        "#;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 1);
        assert!(output.report.unsupported_feature_count >= 2);
        assert_eq!(output.report.fidelity, SvgFidelity::Low);
    }

    #[test]
    fn preserves_source_order_metadata_and_deterministic_ids() {
        let svg = r#"<svg><rect id="a" width="10" height="10"/><rect id="b" x="20" width="10" height="10"/></svg>"#;
        let first = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        let second = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(first.widgets.len(), second.widgets.len());
        assert_eq!(first.widgets[0].id, second.widgets[0].id);
        assert_eq!(
            first.widgets[1]
                .import_metadata
                .as_ref()
                .unwrap()
                .source_order,
            3
        );
    }

    #[test]
    fn reports_malformed_text_entities_and_duplicate_ids() {
        let err = import_svg_template("<svg><rect width=\"10\"", SvgImportOptions::default())
            .unwrap_err();
        assert_eq!(err.code, "xml.unterminated_tag");

        let svg = r#"
            <svg>
                <rect id="dup" width="10" height="10"/>
                <rect id="dup" x="20" width="10" height="10"/>
                <text x="0" y="30" font-family="Noto">A &bogus; B</text>
            </svg>
        "#;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 3);
        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "id.duplicate"));
        assert!(output
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "xml.unknown_entity"));
    }

    #[test]
    fn diagnoses_paint_servers_clips_masks_and_filters() {
        let svg = r##"
            <svg>
                <defs>
                    <linearGradient id="g"/>
                    <pattern id="p"/>
                    <clipPath id="c"><rect width="5" height="5"/></clipPath>
                    <mask id="m"><rect width="5" height="5"/></mask>
                    <filter id="f"/>
                </defs>
                <rect width="20" height="20" fill="url(#g)" clip-path="url(#c)" mask="url(#m)" filter="url(#f)"/>
            </svg>
        "##;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 1);
        for feature in [
            "linearGradient",
            "pattern",
            "clipPath",
            "mask",
            "filter",
            "paint server reference",
            "clip-path attribute",
            "mask attribute",
            "filter attribute",
        ] {
            assert!(
                output
                    .report
                    .unsupported_features
                    .iter()
                    .any(|unsupported| unsupported.feature == feature),
                "missing unsupported feature diagnostic: {feature}"
            );
        }
        assert_eq!(output.report.fidelity, SvgFidelity::Low);
    }

    #[test]
    fn approximates_solid_paint_and_opacity_in_metadata() {
        let svg = r##"
            <svg>
                <rect id="painted" width="10" height="10" fill="#f00" stroke="rgb(0, 255, 0)" opacity="0.5"/>
            </svg>
        "##;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        let widget = &output.widgets[0];
        assert_eq!(widget.bg_color, Some([128, 0, 0]));
        assert_eq!(widget.fg_color, Some([0, 128, 0]));
        assert!(widget
            .import_metadata
            .as_ref()
            .unwrap()
            .warning_flags
            .contains(&"opacity-approx".to_owned()));
    }

    #[test]
    fn downgrades_text_heavy_svg_even_with_declared_fonts() {
        let svg = r#"
            <svg>
                <text x="0" y="20" font-family="Noto">One</text>
                <text x="0" y="40" font-family="Noto">Two</text>
                <text x="0" y="60" font-family="Noto">Three</text>
            </svg>
        "#;
        let output = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(output.widgets.len(), 3);
        assert_eq!(output.report.fidelity, SvgFidelity::Medium);
    }

    #[test]
    fn recovers_from_empty_geometry_and_extreme_transform_deterministically() {
        let svg = r#"
            <svg>
                <rect id="empty" width="0" height="10"/>
                <rect id="huge" width="10" height="10" transform="scale(10000000)"/>
                <rect id="ok" x="20" width="10" height="10"/>
            </svg>
        "#;
        let first = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        let second = import_svg_template(svg, SvgImportOptions::default()).unwrap();
        assert_eq!(first.widgets.len(), 2);
        assert_eq!(first.widgets[0].id, second.widgets[0].id);
        assert!(first
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "geometry.missing_bounds"));
        assert!(first
            .report
            .warnings
            .iter()
            .any(|warning| warning.code == "transform.extreme"));
    }

    #[test]
    fn image_mode_preserves_source_and_dimensions_without_renderer_dependencies() {
        let svg = r#"<svg width="128" height="64"><rect width="128" height="64"/></svg>"#;
        let opts = SvgImportOptions {
            mode: SvgImportMode::Image,
            ..Default::default()
        };
        let first = import_svg_template(svg, opts.clone()).unwrap();
        let second = import_svg_template(svg, opts).unwrap();

        assert_eq!(first.widgets.len(), 1);
        assert_eq!(
            first.widgets[0].kind,
            crate::project::schema::WidgetKind::Image
        );
        assert_eq!(first.widgets[0].rect.w, 128.0);
        assert_eq!(first.widgets[0].rect.h, 64.0);
        assert_eq!(first.widgets[0].svg_source.as_deref(), Some(svg));
        assert_eq!(first.widgets[0].id, second.widgets[0].id);
        assert_eq!(first.report.imported_element_count, 1);
        assert_eq!(first.report.fidelity, SvgFidelity::High);
    }

    #[test]
    fn image_mode_uses_viewbox_when_width_height_are_absent() {
        let svg = r#"<svg viewBox="0 0 320 180"><circle cx="50" cy="50" r="20"/></svg>"#;
        let output = import_svg_template(
            svg,
            SvgImportOptions {
                mode: SvgImportMode::Image,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(output.widgets.len(), 1);
        assert_eq!(output.widgets[0].rect.w, 320.0);
        assert_eq!(output.widgets[0].rect.h, 180.0);
    }

    struct FixtureCase {
        name: &'static str,
        svg: &'static str,
        min_widgets: usize,
        fidelity: SvgFidelity,
        unsupported: &'static [&'static str],
        warnings: &'static [&'static str],
    }

    #[test]
    fn real_world_fixture_suite_imports_deterministically() {
        let cases = [
            FixtureCase {
                name: "basic_shapes",
                svg: include_str!("../tests/fixtures/svg_import/real_world/basic_shapes.svg"),
                min_widgets: 6,
                fidelity: SvgFidelity::High,
                unsupported: &[],
                warnings: &[],
            },
            FixtureCase {
                name: "css_classes",
                svg: include_str!("../tests/fixtures/svg_import/real_world/css_classes.svg"),
                min_widgets: 2,
                fidelity: SvgFidelity::Medium,
                unsupported: &[],
                warnings: &["style.opacity_approx"],
            },
            FixtureCase {
                name: "tspan_text",
                svg: include_str!("../tests/fixtures/svg_import/real_world/tspan_text.svg"),
                min_widgets: 1,
                fidelity: SvgFidelity::Medium,
                unsupported: &[],
                warnings: &["text.complex_tspan"],
            },
            FixtureCase {
                name: "paint_servers",
                svg: include_str!("../tests/fixtures/svg_import/real_world/paint_servers.svg"),
                min_widgets: 3,
                fidelity: SvgFidelity::Low,
                unsupported: &[
                    "linearGradient",
                    "radialGradient",
                    "pattern",
                    "paint server reference",
                ],
                warnings: &[],
            },
            FixtureCase {
                name: "clip_mask_filter",
                svg: include_str!("../tests/fixtures/svg_import/real_world/clip_mask_filter.svg"),
                min_widgets: 2,
                fidelity: SvgFidelity::Low,
                unsupported: &["clipPath", "mask", "filter", "clip-path attribute"],
                warnings: &[],
            },
            FixtureCase {
                name: "symbol_use",
                svg: include_str!("../tests/fixtures/svg_import/real_world/symbol_use.svg"),
                min_widgets: 4,
                fidelity: SvgFidelity::High,
                unsupported: &[],
                warnings: &[],
            },
            FixtureCase {
                name: "external_refs",
                svg: include_str!("../tests/fixtures/svg_import/real_world/external_refs.svg"),
                min_widgets: 1,
                fidelity: SvgFidelity::Low,
                unsupported: &["external image reference", "external use reference"],
                warnings: &[],
            },
            FixtureCase {
                name: "malformed_recovery",
                svg: include_str!("../tests/fixtures/svg_import/real_world/malformed_recovery.svg"),
                min_widgets: 2,
                fidelity: SvgFidelity::Medium,
                unsupported: &[],
                warnings: &[
                    "id.duplicate",
                    "geometry.missing_bounds",
                    "xml.unknown_entity",
                ],
            },
            FixtureCase {
                name: "embedded_image",
                svg: include_str!("../tests/fixtures/svg_import/real_world/embedded_image.svg"),
                min_widgets: 1,
                fidelity: SvgFidelity::High,
                unsupported: &[],
                warnings: &[],
            },
        ];

        for case in cases {
            let first = import_svg_template(case.svg, SvgImportOptions::default())
                .unwrap_or_else(|err| panic!("fixture {} failed: {err}", case.name));
            let second = import_svg_template(case.svg, SvgImportOptions::default())
                .unwrap_or_else(|err| panic!("fixture {} failed on repeat: {err}", case.name));

            assert!(
                first.widgets.len() >= case.min_widgets,
                "fixture {} imported {} widgets; expected at least {}",
                case.name,
                first.widgets.len(),
                case.min_widgets
            );
            assert_eq!(first.report.fidelity, case.fidelity, "{}", case.name);
            assert_eq!(
                first
                    .widgets
                    .iter()
                    .map(|widget| widget.id)
                    .collect::<Vec<_>>(),
                second
                    .widgets
                    .iter()
                    .map(|widget| widget.id)
                    .collect::<Vec<_>>(),
                "fixture {} did not produce deterministic widget IDs",
                case.name
            );
            assert_eq!(
                first.report.diagnostics_digest(),
                second.report.diagnostics_digest(),
                "fixture {} did not produce deterministic diagnostics",
                case.name
            );

            for feature in case.unsupported {
                assert!(
                    first
                        .report
                        .unsupported_features
                        .iter()
                        .any(|unsupported| unsupported.feature == *feature),
                    "fixture {} missing unsupported feature {feature}",
                    case.name
                );
            }
            for code in case.warnings {
                assert!(
                    first
                        .report
                        .warnings
                        .iter()
                        .any(|warning| warning.code == *code),
                    "fixture {} missing warning {code}",
                    case.name
                );
            }
        }
    }
}
