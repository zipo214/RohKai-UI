use crate::project::schema::{Rect, SvgImportMetadata, WidgetInstance, WidgetKind, WidgetProps};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

const MIN_PLACEHOLDER_SIZE: f64 = 20.0;
const ARC_TOLERANCE_PX: f64 = 0.5;

#[derive(Debug, Clone, Default)]
pub struct SvgImportOptions {
    pub limits: SvgImportLimits,
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

    fn finalize(&mut self) {
        self.warning_count = self.warnings.len();
        self.unsupported_feature_count = self.unsupported_features.len();
        self.fidelity = if self.imported_element_count == 0
            || self.skipped_element_count > self.imported_element_count
            || self.unsupported_feature_count > 5
        {
            SvgFidelity::Low
        } else if self.warning_count > 0 || self.unsupported_feature_count > 0 {
            SvgFidelity::Medium
        } else {
            SvgFidelity::High
        };
    }

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

#[derive(Debug, Clone)]
pub struct SvgImportWarning {
    pub code: String,
    pub message: String,
    pub element_name: Option<String>,
    pub original_id: Option<String>,
    pub source_order: Option<usize>,
    pub severity: SvgWarningSeverity,
}

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

    let mut ctx = ImportContext::new(options.limits);
    let nodes = scan_svg(svg, &mut ctx)?;
    let styles = collect_style_rules(&nodes, &mut ctx)?;
    let mut id_index = HashMap::new();
    for node in &nodes {
        if let Some(id) = attr(&node.tag, "id").filter(|id| !id.is_empty()) {
            id_index.entry(id.to_owned()).or_insert(node.index);
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
    ctx.report.finalize();
    Ok(SvgImportOutput {
        widgets,
        report: ctx.report,
    })
}

struct ImportContext {
    limits: SvgImportLimits,
    report: SvgImportReport,
    tag_count: usize,
}

impl ImportContext {
    fn new(limits: SvgImportLimits) -> Self {
        Self {
            limits,
            report: SvgImportReport::new(),
            tag_count: 0,
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
            format!("unsupported SVG feature ignored: {feature}"),
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

#[derive(Clone, Copy, Debug)]
struct Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Matrix {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn multiply(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    fn translate(x: f64, y: f64) -> Self {
        Self {
            e: x,
            f: y,
            ..Self::IDENTITY
        }
    }

    fn scale(x: f64, y: f64) -> Self {
        Self {
            a: x,
            d: y,
            ..Self::IDENTITY
        }
    }

    fn rotate(deg: f64) -> Self {
        let (sin, cos) = deg.to_radians().sin_cos();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: 0.0,
            f: 0.0,
        }
    }

    fn skew_x(deg: f64) -> Self {
        Self {
            c: deg.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    fn skew_y(deg: f64) -> Self {
        Self {
            b: deg.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn summary(self) -> String {
        format!(
            "matrix({:.4} {:.4} {:.4} {:.4} {:.4} {:.4})",
            self.a, self.b, self.c, self.d, self.e, self.f
        )
    }
}

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
    fill: Option<String>,
    stroke: Option<String>,
}

struct ClassRule {
    class_name: String,
    decls: Vec<(String, String)>,
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
) -> Result<Vec<ClassRule>, SvgImportError> {
    let mut out = Vec::new();
    for node in nodes.iter().filter(|n| n.tag.name == "style") {
        if node.text.len() > ctx.limits.max_style_bytes {
            return Err(SvgImportError::new(
                "limit.style_bytes",
                format!("style block exceeded {} bytes", ctx.limits.max_style_bytes),
            ));
        }
        for block in node.text.split('}') {
            let Some((selector, body)) = block.split_once('{') else {
                continue;
            };
            let selector = selector.trim();
            if selector.starts_with('.') && selector[1..].chars().all(is_css_ident_char) {
                out.push(ClassRule {
                    class_name: selector[1..].to_owned(),
                    decls: parse_decls(body),
                });
            } else if !selector.is_empty() {
                ctx.unsupported("complex CSS selector", Some(node));
            }
        }
    }
    Ok(out)
}

fn is_css_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}

fn parse_decls(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect()
}

fn resolve_style(
    node: &Node,
    inherited: &Style,
    rules: &[ClassRule],
    ctx: &mut ImportContext,
) -> Style {
    let mut style = inherited.clone();

    for key in [
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

    if let Some(classes) = attr(&node.tag, "class") {
        for class_name in classes.split_whitespace() {
            for rule in rules.iter().filter(|r| r.class_name == class_name) {
                for (key, value) in &rule.decls {
                    apply_style_decl(&mut style, key, value);
                }
            }
        }
    }

    if let Some(inline) = attr(&node.tag, "style") {
        for (key, value) in parse_decls(inline) {
            apply_style_decl(&mut style, &key, &value);
        }
    }

    for paint in [&style.fill, &style.stroke].into_iter().flatten() {
        if paint.trim().starts_with("url(") {
            ctx.unsupported("gradient or pattern paint", Some(node));
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
        "fill" => style.fill = Some(value.trim().to_owned()),
        "stroke" => style.stroke = Some(value.trim().to_owned()),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn import_node(
    node_id: usize,
    nodes: &[Node],
    rules: &[ClassRule],
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
    state.transform = state
        .transform
        .multiply(parse_transform(attr(&node.tag, "transform").unwrap_or("")));

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
            if let Some(widget) = shape_widget(node, state, ctx)? {
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
        "lineargradient" | "radialgradient" | "pattern" => {
            ctx.unsupported("gradient or pattern definition", Some(node))
        }
        _ => {}
    }

    for key in ["filter", "mask", "clip-path"] {
        if attr(&node.tag, key).is_some() {
            ctx.unsupported(key, Some(node));
        }
    }
}

fn update_viewport(node: &Node, state: &mut ParseState) {
    let width = attr(&node.tag, "width")
        .and_then(|v| parse_length(v, state.viewport_w))
        .unwrap_or(state.viewport_w);
    let height = attr(&node.tag, "height")
        .and_then(|v| parse_length(v, state.viewport_h))
        .unwrap_or(state.viewport_h);
    state.viewport_w = width.max(MIN_PLACEHOLDER_SIZE);
    state.viewport_h = height.max(MIN_PLACEHOLDER_SIZE);

    if let Some(view_box) = attr(&node.tag, "viewBox").or_else(|| attr(&node.tag, "viewbox")) {
        if let Some(nums) = parse_numbers(view_box).filter(|n| n.len() >= 4) {
            let sx = state.viewport_w / nums[2].abs().max(1.0);
            let sy = state.viewport_h / nums[3].abs().max(1.0);
            let view_transform =
                Matrix::scale(sx, sy).multiply(Matrix::translate(-nums[0], -nums[1]));
            state.transform = state.transform.multiply(view_transform);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn expand_use(
    node: &Node,
    nodes: &[Node],
    rules: &[ClassRule],
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

    Ok(Some(WidgetInstance {
        id: deterministic_uuid(node, &rect),
        kind: WidgetKind::Frame,
        rect,
        props: WidgetProps {
            label,
            ..Default::default()
        },
        state_binding: None,
        children: Vec::new(),
        import_metadata: Some(metadata_for(node, state, warning_flags)),
        tooltip: None,
        enabled: None,
        fg_color: None,
        corner_radius: None,
        label_binding: None,
        custom_props: Vec::new(),
        event_handler: None,
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

    Some(WidgetInstance {
        id: deterministic_uuid(node, &rect),
        kind: WidgetKind::Label,
        rect,
        props: WidgetProps {
            label,
            ..Default::default()
        },
        state_binding: None,
        children: Vec::new(),
        import_metadata: Some(metadata_for(node, state, warning_flags)),
        tooltip: None,
        enabled: None,
        fg_color: None,
        corner_radius: None,
        label_binding: None,
        custom_props: Vec::new(),
        event_handler: None,
    })
}

fn flatten_text(node_id: usize, nodes: &[Node], ctx: &mut ImportContext) -> String {
    let node = &nodes[node_id];
    let mut text = node.text.clone();
    for &child_id in &node.children {
        let child = &nodes[child_id];
        match child.tag.name.as_str() {
            "tspan" => {
                if attr(&child.tag, "x").is_some() || attr(&child.tag, "y").is_some() {
                    ctx.warn(
                        "text.complex_tspan",
                        "positioned tspan flattened approximately",
                        Some(child),
                        SvgWarningSeverity::Warning,
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
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let end = trimmed
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(trimmed.len());
    let number = trimmed[..end].parse::<f64>().ok()?;
    let unit = trimmed[end..].trim();
    let value = match unit {
        "" | "px" => number,
        "%" => number * percent_base / 100.0,
        "in" => number * 96.0,
        "cm" => number * 96.0 / 2.54,
        "mm" => number * 96.0 / 25.4,
        "pt" => number * 96.0 / 72.0,
        "pc" => number * 16.0,
        "em" | "rem" => number * 16.0,
        _ => number,
    };
    value.is_finite().then_some(value)
}

fn parse_transform(value: &str) -> Matrix {
    let mut rest = value.trim();
    let mut out = Matrix::IDENTITY;

    while let Some(paren) = rest.find('(') {
        let name = rest[..paren].trim().to_ascii_lowercase();
        let after = &rest[paren + 1..];
        let Some(end) = after.find(')') else {
            break;
        };
        let nums = parse_numbers(&after[..end]).unwrap_or_default();
        let local = match name.as_str() {
            "matrix" if nums.len() >= 6 => Matrix {
                a: nums[0],
                b: nums[1],
                c: nums[2],
                d: nums[3],
                e: nums[4],
                f: nums[5],
            },
            "translate" if !nums.is_empty() => {
                Matrix::translate(nums[0], *nums.get(1).unwrap_or(&0.0))
            }
            "scale" if !nums.is_empty() => Matrix::scale(nums[0], *nums.get(1).unwrap_or(&nums[0])),
            "rotate" if !nums.is_empty() => {
                if nums.len() >= 3 {
                    Matrix::translate(nums[1], nums[2])
                        .multiply(Matrix::rotate(nums[0]))
                        .multiply(Matrix::translate(-nums[1], -nums[2]))
                } else {
                    Matrix::rotate(nums[0])
                }
            }
            "skewx" if !nums.is_empty() => Matrix::skew_x(nums[0]),
            "skewy" if !nums.is_empty() => Matrix::skew_y(nums[0]),
            _ => Matrix::IDENTITY,
        };
        out = out.multiply(local);
        rest = &after[end + 1..];
    }

    out
}

fn parse_numbers(value: &str) -> Option<Vec<f64>> {
    let mut nums = Vec::new();
    for token in number_spans(value) {
        if let Ok(num) = token.parse::<f64>() {
            nums.push(num);
        }
    }
    (!nums.is_empty()).then_some(nums)
}

fn number_spans(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && !is_number_start(bytes[index] as char) {
            index += 1;
        }
        let start = index;
        if index < bytes.len() && matches!(bytes[index] as char, '+' | '-') {
            index += 1;
        }
        let mut digits = 0usize;
        while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
            digits += 1;
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
                digits += 1;
                index += 1;
            }
        }
        if digits > 0 && index < bytes.len() && matches!(bytes[index] as char, 'e' | 'E') {
            let exp = index;
            index += 1;
            if index < bytes.len() && matches!(bytes[index] as char, '+' | '-') {
                index += 1;
            }
            let exp_digits = index;
            while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
                index += 1;
            }
            if exp_digits == index {
                index = exp;
            }
        }
        if start < index && digits > 0 {
            out.push(&value[start..index]);
        } else if start == index {
            index += 1;
        }
    }
    out
}

fn is_number_start(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '-' | '+' | '.')
}

#[derive(Clone, Copy, Debug)]
enum PathToken {
    Command(char),
    Number(f64),
}

fn parse_path_points(
    data: &str,
    max_commands: usize,
    ctx: &mut ImportContext,
    node: &Node,
    warning_flags: &mut Vec<String>,
) -> Result<Vec<(f64, f64)>, SvgImportError> {
    let tokens = path_tokens(data);
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
        if let PathToken::Command(c) = tokens[index] {
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

fn path_tokens(data: &str) -> Vec<PathToken> {
    let mut out = Vec::new();
    let bytes = data.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let c = bytes[index] as char;
        if c.is_ascii_alphabetic() {
            out.push(PathToken::Command(c));
            index += 1;
        } else if is_number_start(c) {
            let start = index;
            if matches!(bytes[index] as char, '+' | '-') {
                index += 1;
            }
            let mut digits = 0usize;
            while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
                index += 1;
                digits += 1;
            }
            if index < bytes.len() && bytes[index] == b'.' {
                index += 1;
                while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
                    index += 1;
                    digits += 1;
                }
            }
            if digits > 0 && index < bytes.len() && matches!(bytes[index] as char, 'e' | 'E') {
                let exp = index;
                index += 1;
                if index < bytes.len() && matches!(bytes[index] as char, '+' | '-') {
                    index += 1;
                }
                let exp_digits = index;
                while index < bytes.len() && (bytes[index] as char).is_ascii_digit() {
                    index += 1;
                }
                if exp_digits == index {
                    index = exp;
                }
            }
            if digits > 0 {
                if let Ok(num) = data[start..index].parse::<f64>() {
                    out.push(PathToken::Number(num));
                }
            } else {
                index += 1;
            }
        } else {
            index += 1;
        }
    }
    out
}

fn read_number(tokens: &[PathToken], index: &mut usize) -> Option<f64> {
    match tokens.get(*index)? {
        PathToken::Number(num) => {
            *index += 1;
            Some(*num)
        }
        PathToken::Command(_) => None,
    }
}

fn read_pair(tokens: &[PathToken], index: &mut usize) -> Option<(f64, f64)> {
    Some((read_number(tokens, index)?, read_number(tokens, index)?))
}

fn read_numbers(tokens: &[PathToken], index: &mut usize, count: usize) -> Option<Vec<f64>> {
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

fn skip_until_next_command(tokens: &[PathToken], index: &mut usize) {
    while *index < tokens.len() && !matches!(tokens[*index], PathToken::Command(_)) {
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
        let err = import_svg_template(svg, SvgImportOptions { limits }).unwrap_err();
        assert_eq!(err.code, "limit.nesting_depth");

        let limits = SvgImportLimits {
            max_attribute_value_length: 4,
            ..SvgImportLimits::default()
        };
        let err = import_svg_template(
            "<svg><rect id=\"abcde\" width=\"10\" height=\"10\"/></svg>",
            SvgImportOptions { limits },
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
}
