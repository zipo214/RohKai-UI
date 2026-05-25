// Pure-Rust software SVG rasterizer.
//
// Covers: rect (rx/ry), circle, ellipse, line, polyline, polygon, path (all
// standard commands), and <g> groups with transforms + style inheritance.
// Outputs egui::ColorImage (straight RGBA). Zero new Cargo dependencies.
//
// Text elements are skipped (decorative in design-tool context).
// Unsupported features (gradients, filters, masks, <use>): shape renders with
// fill/stroke color only.

use egui::ColorImage;

const MAX_SVG_BYTES: usize = 5_000_000;
const MAX_TAGS: usize = 10_000;
const MAX_PATH_TOKENS: usize = 20_000;
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

/// Rasterize an SVG string to a pixel buffer of the given dimensions.
///
/// Returns `Err` if the SVG fails security checks or cannot be parsed.
/// On success the returned `ColorImage` is straight RGBA.
pub fn rasterize(svg_text: &str, width: u32, height: u32) -> Result<ColorImage, SvgRasterError> {
    let (w, h) = raster_size(width, height);
    let mut buf = vec![0u8; w * h * 4]; // transparent black

    if !svg_text_allowed(svg_text) {
        return Err(SvgRasterError::ForbiddenContent);
    }

    let doc = SvgDoc::parse(svg_text).ok_or(SvgRasterError::ParseFailed)?;

    let vb_xform = viewbox_to_pixel_transform(&doc, w, h);
    let default_style = Style::default();
    render_nodes(&doc.nodes, &vb_xform, &default_style, &mut buf, w, h);

    Ok(ColorImage::from_rgba_unmultiplied([w, h], &buf))
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

#[derive(Clone, Copy, Default)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
}

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
        match parse_color(s) {
            Some(c) => Paint::Color(c),
            None => Paint::Color(Rgba::BLACK),
        }
    }
}

fn parse_color(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        match hex.len() {
            3 | 4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Rgba { r, g, b, a: 255 })
            }
            6 | 8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Rgba { r, g, b, a: 255 })
            }
            _ => None,
        }
    } else if let Some(inner) = s
        .strip_prefix("rgb(")
        .or_else(|| s.strip_prefix("RGB("))
        .and_then(|t| t.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() < 3 {
            return None;
        }
        let parse_c = |p: &str| -> Option<u8> {
            let p = p.trim();
            if let Some(pct) = p.strip_suffix('%') {
                let f: f32 = pct.parse().ok()?;
                Some((f / 100.0 * 255.0).round() as u8)
            } else {
                p.parse::<f32>().ok().map(|v| v.clamp(0.0, 255.0) as u8)
            }
        };
        Some(Rgba {
            r: parse_c(parts[0])?,
            g: parse_c(parts[1])?,
            b: parse_c(parts[2])?,
            a: 255,
        })
    } else {
        named_color(s)
    }
}

fn named_color(name: &str) -> Option<Rgba> {
    let c = |r, g, b| Some(Rgba { r, g, b, a: 255 });
    match name.trim().to_lowercase().as_str() {
        "black" => c(0, 0, 0),
        "white" => c(255, 255, 255),
        "red" => c(255, 0, 0),
        "green" => c(0, 128, 0),
        "lime" => c(0, 255, 0),
        "blue" => c(0, 0, 255),
        "yellow" => c(255, 255, 0),
        "orange" => c(255, 165, 0),
        "purple" => c(128, 0, 128),
        "fuchsia" | "magenta" => c(255, 0, 255),
        "cyan" | "aqua" => c(0, 255, 255),
        "gray" | "grey" => c(128, 128, 128),
        "darkgray" | "darkgrey" => c(169, 169, 169),
        "lightgray" | "lightgrey" => c(211, 211, 211),
        "silver" => c(192, 192, 192),
        "maroon" => c(128, 0, 0),
        "navy" => c(0, 0, 128),
        "olive" => c(128, 128, 0),
        "teal" => c(0, 128, 128),
        "transparent" => Some(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }),
        "brown" => c(165, 42, 42),
        "coral" => c(255, 127, 80),
        "crimson" => c(220, 20, 60),
        "gold" => c(255, 215, 0),
        "indigo" => c(75, 0, 130),
        "ivory" => c(255, 255, 240),
        "khaki" => c(240, 230, 140),
        "lavender" => c(230, 230, 250),
        "pink" => c(255, 192, 203),
        "salmon" => c(250, 128, 114),
        "tan" => c(210, 180, 140),
        "violet" => c(238, 130, 238),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 2D Affine Transform
// ---------------------------------------------------------------------------

/// SVG affine matrix: x' = a*x + c*y + e,  y' = b*x + d*y + f
#[derive(Clone, Copy)]
struct Transform {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl Transform {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// self * other  (apply other first, then self)
    fn concat(self, other: Transform) -> Transform {
        Transform {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    fn parse_single(s: &str) -> Option<Self> {
        let s = s.trim();
        let lo = s.to_lowercase();
        if lo.starts_with("matrix(") {
            let inner = &s[7..s.rfind(')')?];
            let v = parse_floats(inner);
            if v.len() >= 6 {
                return Some(Transform {
                    a: v[0],
                    b: v[1],
                    c: v[2],
                    d: v[3],
                    e: v[4],
                    f: v[5],
                });
            }
        } else if lo.starts_with("translate(") {
            let inner = &s[10..s.rfind(')')?];
            let v = parse_floats(inner);
            let tx = v.first().copied().unwrap_or(0.0);
            let ty = v.get(1).copied().unwrap_or(0.0);
            return Some(Transform {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: tx,
                f: ty,
            });
        } else if lo.starts_with("scale(") {
            let inner = &s[6..s.rfind(')')?];
            let v = parse_floats(inner);
            let sx = v.first().copied().unwrap_or(1.0);
            let sy = v.get(1).copied().unwrap_or(sx);
            return Some(Transform {
                a: sx,
                b: 0.0,
                c: 0.0,
                d: sy,
                e: 0.0,
                f: 0.0,
            });
        } else if lo.starts_with("rotate(") {
            let inner = &s[7..s.rfind(')')?];
            let v = parse_floats(inner);
            let angle = v.first().copied().unwrap_or(0.0).to_radians();
            let cx = v.get(1).copied().unwrap_or(0.0);
            let cy = v.get(2).copied().unwrap_or(0.0);
            let (sin_a, cos_a) = angle.sin_cos();
            return Some(Transform {
                a: cos_a,
                b: sin_a,
                c: -sin_a,
                d: cos_a,
                e: cx - cx * cos_a + cy * sin_a,
                f: cy - cx * sin_a - cy * cos_a,
            });
        }
        None
    }

    fn parse_chained(s: &str) -> Transform {
        let mut result = Transform::identity();
        let s = s.trim();
        let mut i = 0;
        let bytes = s.as_bytes();

        while i < s.len() {
            // skip whitespace/commas
            while i < s.len()
                && (bytes[i] == b' ' || bytes[i] == b',' || bytes[i] == b'\t' || bytes[i] == b'\n')
            {
                i += 1;
            }
            if i >= s.len() {
                break;
            }

            // find opening paren
            let start = i;
            while i < s.len() && bytes[i] != b'(' {
                i += 1;
            }
            if i >= s.len() {
                break;
            }
            // find closing paren
            while i < s.len() && bytes[i] != b')' {
                i += 1;
            }
            if i >= s.len() {
                break;
            }
            let func_str = &s[start..i + 1];
            if let Some(t) = Transform::parse_single(func_str) {
                result = result.concat(t);
            }
            i += 1; // skip ')'
        }
        result
    }
}

fn parse_floats(s: &str) -> Vec<f32> {
    s.split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect()
}

// ---------------------------------------------------------------------------
// SVG Document model
// ---------------------------------------------------------------------------

enum SvgNode {
    Group {
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    },
    Rect {
        attrs: Vec<(String, String)>,
    },
    Circle {
        attrs: Vec<(String, String)>,
    },
    Ellipse {
        attrs: Vec<(String, String)>,
    },
    Line {
        attrs: Vec<(String, String)>,
    },
    Polyline {
        attrs: Vec<(String, String)>,
    },
    Polygon {
        attrs: Vec<(String, String)>,
    },
    Path {
        attrs: Vec<(String, String)>,
    },
    Text {
        // Skipped in rendering; kept for parse completeness
        #[allow(dead_code)]
        attrs: Vec<(String, String)>,
    },
}

struct SvgDoc {
    viewbox: Option<[f32; 4]>,
    width: f32,
    height: f32,
    nodes: Vec<SvgNode>,
}

impl SvgDoc {
    fn parse(svg_text: &str) -> Option<Self> {
        let mut parser = XmlParser {
            s: svg_text,
            pos: 0,
        };
        let all_nodes = parser.parse_nodes();

        // Find the SVG root node and extract its attributes
        let (root_attrs, root_children) = all_nodes.into_iter().find_map(|n| {
            if let SvgNode::Group { attrs, children } = n {
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
            let v: Vec<f32> = s
                .split([' ', ','])
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse().ok())
                .collect();
            if v.len() >= 4 {
                Some([v[0], v[1], v[2], v[3]])
            } else {
                None
            }
        });

        let parse_dim = |s: &str| -> f32 {
            s.trim_end_matches(|c: char| c.is_alphabetic() || c == '%')
                .parse()
                .unwrap_or(0.0)
        };

        let width = attr("width").map(parse_dim).unwrap_or(0.0);
        let height = attr("height").map(parse_dim).unwrap_or(0.0);

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

        let is_container = matches!(
            tag.as_str(),
            "g" | "svg" | "defs" | "symbol" | "clippath" | "mask"
        );
        let is_text = tag == "text" || tag == "tspan";

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

        self.make_node(&tag, raw_attrs, children)
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
        tag: &str,
        attrs: Vec<(String, String)>,
        children: Vec<SvgNode>,
    ) -> Option<SvgNode> {
        match tag {
            "svg" | "g" => Some(SvgNode::Group { attrs, children }),
            "rect" => Some(SvgNode::Rect { attrs }),
            "circle" => Some(SvgNode::Circle { attrs }),
            "ellipse" => Some(SvgNode::Ellipse { attrs }),
            "line" => Some(SvgNode::Line { attrs }),
            "polyline" => Some(SvgNode::Polyline { attrs }),
            "polygon" => Some(SvgNode::Polygon { attrs }),
            "path" => Some(SvgNode::Path { attrs }),
            "text" | "tspan" => Some(SvgNode::Text { attrs }),
            _ => None,
        }
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

fn attr_f32(attrs: &[(String, String)], key: &str, default: f32) -> f32 {
    attr_get(attrs, key)
        .and_then(|v| v.trim_end_matches(|c: char| c.is_alphabetic()).parse().ok())
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// ViewBox → pixel transform
// ---------------------------------------------------------------------------

fn viewbox_to_pixel_transform(doc: &SvgDoc, pw: usize, ph: usize) -> Transform {
    let (vbx, vby, vbw, vbh) = match doc.viewbox {
        Some([x, y, w, h]) if w > 0.0 && h > 0.0 => (x, y, w, h),
        _ => {
            // Fall back to width/height if present
            if doc.width > 0.0 && doc.height > 0.0 {
                (0.0, 0.0, doc.width, doc.height)
            } else {
                return Transform::identity();
            }
        }
    };

    let scale = (pw as f32 / vbw).min(ph as f32 / vbh);
    let tx = (pw as f32 - vbw * scale) / 2.0 - vbx * scale;
    let ty = (ph as f32 - vbh * scale) / 2.0 - vby * scale;

    Transform {
        a: scale,
        b: 0.0,
        c: 0.0,
        d: scale,
        e: tx,
        f: ty,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_nodes(
    nodes: &[SvgNode],
    xform: &Transform,
    style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    for node in nodes {
        render_node(node, xform, style, buf, w, h);
    }
}

fn render_node(
    node: &SvgNode,
    xform: &Transform,
    style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    match node {
        SvgNode::Group { attrs, children } => {
            let attr_pairs: Vec<(&str, &str)> = attrs
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let child_style = style.inherit(&attr_pairs);
            let local_xform = attr_get(attrs, "transform")
                .map(Transform::parse_chained)
                .unwrap_or_else(Transform::identity);
            let combined = xform.concat(local_xform);
            render_nodes(children, &combined, &child_style, buf, w, h);
        }
        SvgNode::Rect { attrs } => render_rect(attrs, xform, style, buf, w, h),
        SvgNode::Circle { attrs } => render_circle(attrs, xform, style, buf, w, h),
        SvgNode::Ellipse { attrs } => render_ellipse(attrs, xform, style, buf, w, h),
        SvgNode::Line { attrs } => render_line(attrs, xform, style, buf, w, h),
        SvgNode::Polyline { attrs } => render_poly(attrs, xform, style, buf, w, h, false),
        SvgNode::Polygon { attrs } => render_poly(attrs, xform, style, buf, w, h, true),
        SvgNode::Path { attrs } => render_path(attrs, xform, style, buf, w, h),
        SvgNode::Text { attrs: _ } => {} // not rendered
    }
}

// ---------------------------------------------------------------------------
// Shape renderers
// ---------------------------------------------------------------------------

fn render_rect(
    attrs: &[(String, String)],
    xform: &Transform,
    parent_style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    let attr_pairs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let style = parent_style.inherit(&attr_pairs);

    let x = attr_f32(attrs, "x", 0.0);
    let y = attr_f32(attrs, "y", 0.0);
    let rw = attr_f32(attrs, "width", 0.0);
    let rh = attr_f32(attrs, "height", 0.0);
    if rw <= 0.0 || rh <= 0.0 {
        return;
    }
    let mut rx = attr_f32(attrs, "rx", 0.0);
    let mut ry = attr_f32(attrs, "ry", 0.0);
    // SVG: if only rx or only ry is given, use the same for both
    if rx > 0.0 && ry == 0.0 {
        ry = rx;
    }
    if ry > 0.0 && rx == 0.0 {
        rx = ry;
    }

    let pts = rounded_rect_pts(x, y, rw, rh, rx, ry);
    let transformed: Vec<(f32, f32)> = pts.iter().map(|&(px, py)| xform.apply(px, py)).collect();

    if let Some(fill) = style.effective_fill() {
        fill_polygon(buf, w, h, &transformed, fill);
    }
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        stroke_polyline(buf, w, h, &transformed, stroke_w, stroke_color, true);
    }
}

fn render_circle(
    attrs: &[(String, String)],
    xform: &Transform,
    parent_style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    let attr_pairs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let style = parent_style.inherit(&attr_pairs);

    let cx = attr_f32(attrs, "cx", 0.0);
    let cy = attr_f32(attrs, "cy", 0.0);
    let r = attr_f32(attrs, "r", 0.0);
    if r <= 0.0 {
        return;
    }

    let pts = ellipse_pts(cx, cy, r, r);
    let transformed: Vec<(f32, f32)> = pts.iter().map(|&(px, py)| xform.apply(px, py)).collect();

    if let Some(fill) = style.effective_fill() {
        fill_polygon(buf, w, h, &transformed, fill);
    }
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        stroke_polyline(buf, w, h, &transformed, stroke_w, stroke_color, true);
    }
}

fn render_ellipse(
    attrs: &[(String, String)],
    xform: &Transform,
    parent_style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    let attr_pairs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let style = parent_style.inherit(&attr_pairs);

    let cx = attr_f32(attrs, "cx", 0.0);
    let cy = attr_f32(attrs, "cy", 0.0);
    let erx = attr_f32(attrs, "rx", 0.0);
    let ery = attr_f32(attrs, "ry", 0.0);
    if erx <= 0.0 || ery <= 0.0 {
        return;
    }

    let pts = ellipse_pts(cx, cy, erx, ery);
    let transformed: Vec<(f32, f32)> = pts.iter().map(|&(px, py)| xform.apply(px, py)).collect();

    if let Some(fill) = style.effective_fill() {
        fill_polygon(buf, w, h, &transformed, fill);
    }
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        stroke_polyline(buf, w, h, &transformed, stroke_w, stroke_color, true);
    }
}

fn render_line(
    attrs: &[(String, String)],
    xform: &Transform,
    parent_style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    let attr_pairs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let style = parent_style.inherit(&attr_pairs);

    let x1 = attr_f32(attrs, "x1", 0.0);
    let y1 = attr_f32(attrs, "y1", 0.0);
    let x2 = attr_f32(attrs, "x2", 0.0);
    let y2 = attr_f32(attrs, "y2", 0.0);

    let pts = vec![xform.apply(x1, y1), xform.apply(x2, y2)];
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        stroke_polyline(buf, w, h, &pts, stroke_w, stroke_color, false);
    }
}

fn render_poly(
    attrs: &[(String, String)],
    xform: &Transform,
    parent_style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
    closed: bool,
) {
    let attr_pairs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let style = parent_style.inherit(&attr_pairs);

    let pts_raw = attr_get(attrs, "points").unwrap_or("");
    let local_pts = parse_point_list(pts_raw);
    if local_pts.len() < 2 {
        return;
    }

    let pts: Vec<(f32, f32)> = local_pts
        .iter()
        .map(|&(px, py)| xform.apply(px, py))
        .collect();

    if closed {
        if let Some(fill) = style.effective_fill() {
            fill_polygon(buf, w, h, &pts, fill);
        }
    }
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        stroke_polyline(buf, w, h, &pts, stroke_w, stroke_color, closed);
    }
}

fn render_path(
    attrs: &[(String, String)],
    xform: &Transform,
    parent_style: &Style,
    buf: &mut [u8],
    w: usize,
    h: usize,
) {
    let attr_pairs: Vec<(&str, &str)> = attrs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let style = parent_style.inherit(&attr_pairs);

    let d = attr_get(attrs, "d").unwrap_or("");
    if d.is_empty() {
        return;
    }

    let sub_paths = parse_path_d(d);

    // Fill: all sub-paths as a combined even-odd fill
    if let Some(fill) = style.effective_fill() {
        fill_path_subpaths(buf, w, h, &sub_paths, xform, fill);
    }

    // Stroke: each sub-path individually
    if let Some((stroke_color, stroke_w)) = style.effective_stroke() {
        for sub in &sub_paths {
            if sub.len() < 2 {
                continue;
            }
            let closed = sub.first() == sub.last() && sub.len() > 2;
            let pts: Vec<(f32, f32)> = sub.iter().map(|&(px, py)| xform.apply(px, py)).collect();
            stroke_polyline(buf, w, h, &pts, stroke_w, stroke_color, closed);
        }
    }
}

// ---------------------------------------------------------------------------
// Path data parser
// ---------------------------------------------------------------------------

enum PathToken {
    Cmd(char),
    Num(f32),
}

fn tokenize_path(d: &str) -> Vec<PathToken> {
    let mut tokens = Vec::new();
    let mut chars = d.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' | ',' => {
                chars.next();
            }
            'M' | 'm' | 'L' | 'l' | 'H' | 'h' | 'V' | 'v' | 'C' | 'c' | 'Q' | 'q' | 'S' | 's'
            | 'T' | 't' | 'A' | 'a' | 'Z' | 'z' => {
                tokens.push(PathToken::Cmd(c));
                chars.next();
            }
            '0'..='9' | '.' | '-' | '+' => {
                let mut num = String::new();
                if c == '-' || c == '+' {
                    num.push(c);
                    chars.next();
                }
                // Integer part
                while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    num.push(chars.next().unwrap());
                }
                // Optional decimal
                if chars.peek() == Some(&'.') {
                    num.push(chars.next().unwrap());
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        num.push(chars.next().unwrap());
                    }
                }
                // Optional exponent
                if matches!(chars.peek(), Some(&'e') | Some(&'E')) {
                    num.push(chars.next().unwrap());
                    if matches!(chars.peek(), Some(&'+') | Some(&'-')) {
                        num.push(chars.next().unwrap());
                    }
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        num.push(chars.next().unwrap());
                    }
                }
                if let Ok(n) = num.parse::<f32>() {
                    tokens.push(PathToken::Num(n));
                }
            }
            _ => {
                chars.next();
            }
        }
    }
    tokens
}

#[allow(clippy::while_let_loop)]
fn parse_path_d(d: &str) -> Vec<Vec<(f32, f32)>> {
    let tokens = tokenize_path(d);
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
            PathToken::Cmd(c) => {
                i += 1;
                *c
            }
            PathToken::Num(_) => {
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
                if let Some(PathToken::Num(n)) = tokens.get(i) {
                    i += 1;
                    *n
                } else {
                    break;
                }
            };
        }

        match cmd {
            'M' | 'm' => loop {
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
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
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                let (nx, ny) = if cmd == 'l' { (cx + x, cy + y) } else { (x, y) };
                cx = nx;
                cy = ny;
                current.push((cx, cy));
            },
            'H' | 'h' => loop {
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                cx = if cmd == 'h' { cx + x } else { x };
                current.push((cx, cy));
            },
            'V' | 'v' => loop {
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let y = get_num!();
                cy = if cmd == 'v' { cy + y } else { y };
                current.push((cx, cy));
            },
            'C' | 'c' => loop {
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x1 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let y1 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x2 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let y2 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
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
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x2 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let y2 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
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
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x1 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let y1 = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
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
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
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
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let rx = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let ry = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x_rot = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let large = get_num!() != 0.0;
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let sweep = get_num!() != 0.0;
                let Some(PathToken::Num(_)) = tokens.get(i) else {
                    break;
                };
                let x = get_num!();
                let Some(PathToken::Num(_)) = tokens.get(i) else {
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
            _ => {}
        }
    }

    if !current.is_empty() {
        sub_paths.push(current);
    }
    sub_paths
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
        .map(|s| s.iter().map(|&(px, py)| xform.apply(px, py)).collect())
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
}
