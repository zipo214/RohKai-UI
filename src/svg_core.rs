//! Shared SVG microsyntax helpers.
//!
//! Keep this module dependency-free and deterministic. Importer and rasterizer
//! code should prefer these helpers over growing separate parsers for the same
//! SVG syntax.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    pub fn rgb(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

pub fn parse_color(value: &str) -> Option<Rgba> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("currentcolor")
        || value.starts_with("url(")
    {
        return None;
    }

    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }

    if let Some(body) = strip_ascii_function(value, "rgb") {
        return parse_rgb_body(body);
    }

    named_color(value)
}

pub fn parse_rgb(value: &str) -> Option<[u8; 3]> {
    parse_color(value).map(Rgba::rgb)
}

pub fn parse_numbers(value: &str) -> Vec<f64> {
    number_spans(value)
        .into_iter()
        .filter_map(|token| token.parse::<f64>().ok())
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvgStyleDeclaration {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvgCssSelector {
    pub element: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

impl SvgCssSelector {
    pub fn specificity(&self) -> [u16; 3] {
        [
            u16::from(self.id.is_some()),
            self.classes.len().min(u16::MAX as usize) as u16,
            u16::from(self.element.is_some()),
        ]
    }

    pub fn matches(&self, element: &str, id: Option<&str>, classes: &str) -> bool {
        if self
            .element
            .as_deref()
            .is_some_and(|expected| !expected.eq_ignore_ascii_case(element))
        {
            return false;
        }
        if self
            .id
            .as_deref()
            .is_some_and(|expected| id != Some(expected))
        {
            return false;
        }
        self.classes.iter().all(|expected| {
            classes
                .split_ascii_whitespace()
                .any(|class| class == expected)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvgCssRule {
    pub selectors: Vec<SvgCssSelector>,
    pub declarations: Vec<SvgStyleDeclaration>,
    pub source_order: usize,
}

impl SvgCssRule {
    pub fn matching_specificity(
        &self,
        element: &str,
        id: Option<&str>,
        classes: &str,
    ) -> Option<[u16; 3]> {
        self.selectors
            .iter()
            .filter(|selector| selector.matches(element, id, classes))
            .map(SvgCssSelector::specificity)
            .max()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SvgCssStyleSheet {
    pub rules: Vec<SvgCssRule>,
    pub unsupported_selector_count: usize,
    pub malformed_rule_count: usize,
    pub dropped_rule_count: usize,
    pub dropped_declaration_count: usize,
}

pub fn parse_style_declarations(
    input: &str,
    max_declarations: usize,
) -> (Vec<SvgStyleDeclaration>, usize) {
    let mut declarations = Vec::new();
    let mut dropped = 0;
    for declaration in input.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            if !declaration.trim().is_empty() {
                dropped += 1;
            }
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name.is_empty() || value.is_empty() || value.contains("!important") {
            dropped += 1;
            continue;
        }
        if declarations.len() >= max_declarations {
            dropped += 1;
            continue;
        }
        declarations.push(SvgStyleDeclaration {
            name,
            value: value.to_owned(),
        });
    }
    (declarations, dropped)
}

pub fn parse_css_stylesheet(
    input: &str,
    max_rules: usize,
    max_declarations: usize,
) -> SvgCssStyleSheet {
    let mut sheet = SvgCssStyleSheet::default();
    let mut source_order = 0;
    let mut rest = input;
    while let Some(open) = rest.find('{') {
        let selector_text = rest[..open].trim();
        let body = &rest[open + 1..];
        let Some(close) = body.find('}') else {
            sheet.malformed_rule_count += 1;
            break;
        };
        rest = &body[close + 1..];
        let declarations_text = &body[..close];
        if selector_text.is_empty() {
            sheet.malformed_rule_count += 1;
            continue;
        }
        let mut selectors = Vec::new();
        for selector in selector_text.split(',') {
            match parse_tier1_selector(selector.trim()) {
                Some(selector) => selectors.push(selector),
                None => sheet.unsupported_selector_count += 1,
            }
        }
        if selectors.is_empty() {
            continue;
        }
        let remaining = max_declarations.saturating_sub(
            sheet
                .rules
                .iter()
                .map(|rule| rule.declarations.len())
                .sum::<usize>(),
        );
        let (declarations, dropped) = parse_style_declarations(declarations_text, remaining);
        sheet.dropped_declaration_count += dropped;
        if declarations.is_empty() {
            continue;
        }
        if sheet.rules.len() >= max_rules {
            sheet.dropped_rule_count += 1;
            continue;
        }
        sheet.rules.push(SvgCssRule {
            selectors,
            declarations,
            source_order,
        });
        source_order += 1;
    }
    sheet
}

fn parse_tier1_selector(input: &str) -> Option<SvgCssSelector> {
    if input.is_empty()
        || input.chars().any(|c| {
            c.is_ascii_whitespace() || matches!(c, '>' | '+' | '~' | '[' | ']' | ':' | '*' | '|')
        })
    {
        return None;
    }
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut element = None;
    let mut id = None;
    let mut classes = Vec::new();
    if bytes
        .first()
        .is_some_and(|byte| is_css_identifier_start(*byte as char))
    {
        let end = scan_css_identifier(input, index);
        element = Some(input[index..end].to_ascii_lowercase());
        index = end;
    }
    while index < input.len() {
        let marker = bytes[index] as char;
        if marker != '.' && marker != '#' {
            return None;
        }
        index += 1;
        let end = scan_css_identifier(input, index);
        if end == index {
            return None;
        }
        let name = input[index..end].to_owned();
        if marker == '#' {
            if id.replace(name).is_some() {
                return None;
            }
        } else if !classes.contains(&name) {
            classes.push(name);
        }
        index = end;
    }
    (element.is_some() || id.is_some() || !classes.is_empty()).then_some(SvgCssSelector {
        element,
        id,
        classes,
    })
}

fn scan_css_identifier(input: &str, mut index: usize) -> usize {
    while index < input.len() {
        let c = input.as_bytes()[index] as char;
        if !is_css_identifier_char(c) {
            break;
        }
        index += 1;
    }
    index
}

fn is_css_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '_' | '-')
}

fn is_css_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvgLengthUnit {
    Number,
    Px,
    Percent,
    In,
    Cm,
    Mm,
    Q,
    Pt,
    Pc,
    Em,
    Ex,
    Rem,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SvgLength {
    pub value: f64,
    pub unit: SvgLengthUnit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SvgLengthContext {
    pub percent_base: f64,
    pub font_size: f64,
    pub x_height: f64,
    pub root_font_size: f64,
    pub dpi: f64,
}

impl SvgLengthContext {
    pub fn user_units(percent_base: f64) -> Self {
        Self {
            percent_base,
            font_size: 16.0,
            x_height: 8.0,
            root_font_size: 16.0,
            dpi: 96.0,
        }
    }
}

impl SvgLength {
    pub fn resolve(self, context: SvgLengthContext) -> Option<f64> {
        let value = match self.unit {
            SvgLengthUnit::Number | SvgLengthUnit::Px => self.value,
            SvgLengthUnit::Percent => self.value * context.percent_base / 100.0,
            SvgLengthUnit::In => self.value * context.dpi,
            SvgLengthUnit::Cm => self.value * context.dpi / 2.54,
            SvgLengthUnit::Mm => self.value * context.dpi / 25.4,
            SvgLengthUnit::Q => self.value * context.dpi / 101.6,
            SvgLengthUnit::Pt => self.value * context.dpi / 72.0,
            SvgLengthUnit::Pc => self.value * context.dpi / 6.0,
            SvgLengthUnit::Em => self.value * context.font_size,
            SvgLengthUnit::Ex => self.value * context.x_height,
            SvgLengthUnit::Rem => self.value * context.root_font_size,
        };
        value.is_finite().then_some(value)
    }
}

pub fn parse_length(value: &str) -> Option<SvgLength> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let number_end = scan_number_end(trimmed, 0);
    if number_end == 0 {
        return None;
    }
    let number = trimmed[..number_end].parse::<f64>().ok()?;
    if !number.is_finite() {
        return None;
    }

    let unit = match trimmed[number_end..].trim().to_ascii_lowercase().as_str() {
        "" => SvgLengthUnit::Number,
        "px" => SvgLengthUnit::Px,
        "%" => SvgLengthUnit::Percent,
        "in" => SvgLengthUnit::In,
        "cm" => SvgLengthUnit::Cm,
        "mm" => SvgLengthUnit::Mm,
        "q" => SvgLengthUnit::Q,
        "pt" => SvgLengthUnit::Pt,
        "pc" => SvgLengthUnit::Pc,
        "em" => SvgLengthUnit::Em,
        "ex" => SvgLengthUnit::Ex,
        "rem" => SvgLengthUnit::Rem,
        _ => return None,
    };
    Some(SvgLength {
        value: number,
        unit,
    })
}

pub fn resolve_length(value: &str, context: SvgLengthContext) -> Option<f64> {
    parse_length(value)?.resolve(context)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SvgAlign {
    Min,
    #[default]
    Mid,
    Max,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SvgMeetOrSlice {
    #[default]
    Meet,
    Slice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvgPreserveAspectRatio {
    pub defer: bool,
    pub align_x: Option<SvgAlign>,
    pub align_y: Option<SvgAlign>,
    pub meet_or_slice: SvgMeetOrSlice,
}

impl Default for SvgPreserveAspectRatio {
    fn default() -> Self {
        Self {
            defer: false,
            align_x: Some(SvgAlign::Mid),
            align_y: Some(SvgAlign::Mid),
            meet_or_slice: SvgMeetOrSlice::Meet,
        }
    }
}

pub fn parse_preserve_aspect_ratio(value: &str) -> SvgPreserveAspectRatio {
    let mut out = SvgPreserveAspectRatio::default();
    let mut tokens = value.split_ascii_whitespace();
    let first = tokens.next();
    let alignment = if first.is_some_and(|token| token.eq_ignore_ascii_case("defer")) {
        out.defer = true;
        tokens.next()
    } else {
        first
    };

    if let Some(alignment) = alignment {
        if alignment.eq_ignore_ascii_case("none") {
            out.align_x = None;
            out.align_y = None;
        } else if let Some((x, y)) = parse_alignment(alignment) {
            out.align_x = Some(x);
            out.align_y = Some(y);
        }
    }

    if let Some(mode) = tokens.next() {
        out.meet_or_slice = if mode.eq_ignore_ascii_case("slice") {
            SvgMeetOrSlice::Slice
        } else {
            SvgMeetOrSlice::Meet
        };
    }
    out
}

fn parse_alignment(value: &str) -> Option<(SvgAlign, SvgAlign)> {
    let lower = value.to_ascii_lowercase();
    let y_index = lower.find('y')?;
    let x = match &lower[..y_index] {
        "xmin" => SvgAlign::Min,
        "xmid" => SvgAlign::Mid,
        "xmax" => SvgAlign::Max,
        _ => return None,
    };
    let y = match &lower[y_index..] {
        "ymin" => SvgAlign::Min,
        "ymid" => SvgAlign::Mid,
        "ymax" => SvgAlign::Max,
        _ => return None,
    };
    Some((x, y))
}

pub fn viewbox_transform(
    view_box: [f64; 4],
    viewport: [f64; 4],
    aspect_ratio: SvgPreserveAspectRatio,
) -> Option<Affine2D> {
    let [view_x, view_y, view_width, view_height] = view_box;
    let [port_x, port_y, port_width, port_height] = viewport;
    if !view_box.into_iter().all(f64::is_finite)
        || !viewport.into_iter().all(f64::is_finite)
        || view_width <= 0.0
        || view_height <= 0.0
        || port_width <= 0.0
        || port_height <= 0.0
    {
        return None;
    }

    let scale_x = port_width / view_width;
    let scale_y = port_height / view_height;
    let (scale_x, scale_y, offset_x, offset_y) = match (aspect_ratio.align_x, aspect_ratio.align_y)
    {
        (Some(align_x), Some(align_y)) => {
            let scale = match aspect_ratio.meet_or_slice {
                SvgMeetOrSlice::Meet => scale_x.min(scale_y),
                SvgMeetOrSlice::Slice => scale_x.max(scale_y),
            };
            let remaining_x = port_width - view_width * scale;
            let remaining_y = port_height - view_height * scale;
            (
                scale,
                scale,
                align_offset(remaining_x, align_x),
                align_offset(remaining_y, align_y),
            )
        }
        _ => (scale_x, scale_y, 0.0, 0.0),
    };

    Some(Affine2D {
        a: scale_x,
        b: 0.0,
        c: 0.0,
        d: scale_y,
        e: port_x + offset_x - view_x * scale_x,
        f: port_y + offset_y - view_y * scale_y,
    })
}

fn align_offset(remaining: f64, align: SvgAlign) -> f64 {
    match align {
        SvgAlign::Min => 0.0,
        SvgAlign::Mid => remaining / 2.0,
        SvgAlign::Max => remaining,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SvgPathToken {
    Command(char),
    Number(f64),
}

pub fn tokenize_path_data(data: &str) -> Vec<SvgPathToken> {
    let bytes = data.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let c = bytes[index] as char;
        if c.is_ascii_alphabetic() {
            out.push(SvgPathToken::Command(c));
            index += 1;
        } else if is_number_start(c) {
            let start = index;
            let end = scan_number_end(data, index);
            if end > start {
                if let Ok(num) = data[start..end].parse::<f64>() {
                    out.push(SvgPathToken::Number(num));
                }
                index = end;
            } else {
                index += 1;
            }
        } else {
            index += 1;
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2D {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine2D {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn identity() -> Self {
        Self::IDENTITY
    }

    /// self * other: apply `other` first, then `self`.
    pub fn multiply(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    pub fn concat(self, other: Self) -> Self {
        self.multiply(other)
    }

    pub fn translate(x: f64, y: f64) -> Self {
        Self {
            e: x,
            f: y,
            ..Self::IDENTITY
        }
    }

    pub fn scale(x: f64, y: f64) -> Self {
        Self {
            a: x,
            d: y,
            ..Self::IDENTITY
        }
    }

    pub fn rotate(deg: f64) -> Self {
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

    pub fn rotate_about(deg: f64, cx: f64, cy: f64) -> Self {
        Self::translate(cx, cy)
            .multiply(Self::rotate(deg))
            .multiply(Self::translate(-cx, -cy))
    }

    pub fn skew_x(deg: f64) -> Self {
        Self {
            c: deg.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    pub fn skew_y(deg: f64) -> Self {
        Self {
            b: deg.to_radians().tan(),
            ..Self::IDENTITY
        }
    }

    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    pub fn apply_f32(self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = self.apply(x as f64, y as f64);
        (x as f32, y as f32)
    }

    pub fn inverse(self) -> Option<Self> {
        let determinant = self.a * self.d - self.b * self.c;
        if !determinant.is_finite() || determinant.abs() <= 1.0e-15 {
            return None;
        }
        let inverse = 1.0 / determinant;
        let out = Self {
            a: self.d * inverse,
            b: -self.b * inverse,
            c: -self.c * inverse,
            d: self.a * inverse,
            e: (self.c * self.f - self.d * self.e) * inverse,
            f: (self.b * self.e - self.a * self.f) * inverse,
        };
        [out.a, out.b, out.c, out.d, out.e, out.f]
            .into_iter()
            .all(f64::is_finite)
            .then_some(out)
    }

    pub fn is_finite(self) -> bool {
        self.a.is_finite()
            && self.b.is_finite()
            && self.c.is_finite()
            && self.d.is_finite()
            && self.e.is_finite()
            && self.f.is_finite()
    }

    pub fn is_extreme(self) -> bool {
        const EXTREME: f64 = 1_000_000.0;
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .into_iter()
            .any(|value| value.abs() > EXTREME)
    }

    pub fn summary(self) -> String {
        format!(
            "matrix({:.4} {:.4} {:.4} {:.4} {:.4} {:.4})",
            self.a, self.b, self.c, self.d, self.e, self.f
        )
    }

    pub fn parse_transform_checked(value: &str) -> Option<Self> {
        let mut rest = value.trim();
        let mut out = Self::IDENTITY;

        if rest.is_empty() {
            return Some(out);
        }

        while !rest.is_empty() {
            let paren = rest.find('(')?;
            let name = rest[..paren].trim().to_ascii_lowercase();
            let after = &rest[paren + 1..];
            let end = after.find(')')?;
            let nums = parse_number_list_strict(&after[..end])?;
            let local = match name.as_str() {
                "matrix" if nums.len() == 6 => Self {
                    a: nums[0],
                    b: nums[1],
                    c: nums[2],
                    d: nums[3],
                    e: nums[4],
                    f: nums[5],
                },
                "translate" if matches!(nums.len(), 1 | 2) => {
                    Self::translate(nums[0], *nums.get(1).unwrap_or(&0.0))
                }
                "scale" if matches!(nums.len(), 1 | 2) => {
                    Self::scale(nums[0], *nums.get(1).unwrap_or(&nums[0]))
                }
                "rotate" if matches!(nums.len(), 1 | 3) => {
                    if nums.len() == 3 {
                        Self::rotate_about(nums[0], nums[1], nums[2])
                    } else {
                        Self::rotate(nums[0])
                    }
                }
                "skewx" if nums.len() == 1 => Self::skew_x(nums[0]),
                "skewy" if nums.len() == 1 => Self::skew_y(nums[0]),
                _ => return None,
            };
            if [local.a, local.b, local.c, local.d, local.e, local.f]
                .into_iter()
                .any(|number| !number.is_finite())
            {
                return None;
            }
            out = out.multiply(local);
            rest = after[end + 1..].trim_start();
            if let Some(after_comma) = rest.strip_prefix(',') {
                rest = after_comma.trim_start();
            }
        }

        Some(out)
    }

    pub fn parse_transform(value: &str) -> Self {
        Self::parse_transform_checked(value).unwrap_or(Self::IDENTITY)
    }

    pub fn parse_chained(value: &str) -> Self {
        Self::parse_transform(value)
    }
}

fn parse_number_list_strict(value: &str) -> Option<Vec<f64>> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len()
            && ((bytes[index] as char).is_ascii_whitespace() || bytes[index] == b',')
        {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        let start = index;
        if !is_number_start(bytes[index] as char) {
            return None;
        }
        index = scan_number_end(value, index);
        if index == start {
            return None;
        }
        let number = value[start..index].parse::<f64>().ok()?;
        if !number.is_finite() {
            return None;
        }
        out.push(number);
    }
    Some(out)
}

fn parse_hex_color(hex: &str) -> Option<Rgba> {
    match hex.len() {
        3 | 4 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            let a = if hex.len() == 4 {
                u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?
            } else {
                255
            };
            Some(Rgba { r, g, b, a })
        }
        6 | 8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16).ok()?
            } else {
                255
            };
            Some(Rgba { r, g, b, a })
        }
        _ => None,
    }
}

fn parse_rgb_body(body: &str) -> Option<Rgba> {
    let parts: Vec<&str> = body
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|part| !part.is_empty() && *part != "/")
        .collect();
    if parts.len() < 3 {
        return None;
    }
    Some(Rgba {
        r: parse_color_component(parts[0])?,
        g: parse_color_component(parts[1])?,
        b: parse_color_component(parts[2])?,
        a: 255,
    })
}

fn parse_color_component(part: &str) -> Option<u8> {
    let part = part.trim();
    if let Some(percent) = part.strip_suffix('%') {
        let value: f64 = percent.parse().ok()?;
        Some((value / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8)
    } else {
        part.parse::<f64>()
            .ok()
            .map(|value| value.round().clamp(0.0, 255.0) as u8)
    }
}

fn strip_ascii_function<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let value = value.trim();
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    (close > open && value[..open].eq_ignore_ascii_case(name)).then_some(&value[open + 1..close])
}

fn named_color(name: &str) -> Option<Rgba> {
    let c = |r, g, b| Some(Rgba { r, g, b, a: 255 });
    match name.trim().to_ascii_lowercase().as_str() {
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

fn number_spans(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && !is_number_start(bytes[index] as char) {
            index += 1;
        }
        let start = index;
        let end = scan_number_end(value, index);
        if end > start {
            out.push(&value[start..end]);
            index = end;
        } else {
            index += 1;
        }
    }
    out
}

pub fn is_number_start(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '-' | '+' | '.')
}

fn scan_number_end(value: &str, mut index: usize) -> usize {
    let bytes = value.as_bytes();
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
    if digits > 0 {
        index
    } else {
        start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_svg_colors_shared_by_importer_and_renderer() {
        assert_eq!(parse_color("#0f8").unwrap().rgb(), [0, 255, 136]);
        assert_eq!(parse_color("#33669980").unwrap().a, 128);
        assert_eq!(
            parse_color("rgb(100%, 0%, 50%)").unwrap().rgb(),
            [255, 0, 128]
        );
        assert_eq!(parse_color("gold").unwrap().rgb(), [255, 215, 0]);
        assert!(parse_color("url(#g)").is_none());
    }

    #[test]
    fn parses_compact_svg_number_lists() {
        assert_eq!(parse_numbers("M10-20L.5.6e2"), vec![10.0, -20.0, 0.5, 60.0]);
    }

    #[test]
    fn tier1_css_parses_grouped_compound_selectors_and_specificity() {
        let sheet = parse_css_stylesheet(
            "rect, .hot { fill: red; } rect.hot#hero { stroke: blue; }",
            16,
            32,
        );

        assert_eq!(sheet.unsupported_selector_count, 0);
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(
            sheet.rules[1].matching_specificity("rect", Some("hero"), "hot"),
            Some([1, 1, 1])
        );
        assert_eq!(
            sheet.rules[0].matching_specificity("rect", Some("hero"), "hot"),
            Some([0, 1, 0])
        );
    }

    #[test]
    fn tier1_css_rejects_complex_selectors_and_important() {
        let sheet = parse_css_stylesheet(
            "g > rect { fill: red; } .ok { stroke: blue !important; fill: green; }",
            16,
            32,
        );

        assert_eq!(sheet.unsupported_selector_count, 1);
        assert_eq!(sheet.rules.len(), 1);
        assert_eq!(sheet.rules[0].declarations.len(), 1);
        assert_eq!(sheet.dropped_declaration_count, 1);
    }

    #[test]
    fn tier1_css_honors_rule_and_declaration_budgets() {
        let sheet = parse_css_stylesheet(".a { fill:red; stroke:blue; } .b { fill:green; }", 1, 1);

        assert_eq!(sheet.rules.len(), 1);
        assert!(sheet.dropped_rule_count > 0 || sheet.dropped_declaration_count > 0);
    }

    #[test]
    fn parses_and_resolves_svg_lengths() {
        let context = SvgLengthContext::user_units(200.0);
        assert_eq!(resolve_length("25%", context), Some(50.0));
        assert_eq!(resolve_length("2.54cm", context), Some(96.0));
        assert_eq!(resolve_length("1Q", context), Some(96.0 / 101.6));
        assert_eq!(resolve_length("2em", context), Some(32.0));
        assert_eq!(resolve_length("2ex", context), Some(16.0));
        assert_eq!(resolve_length("2rem", context), Some(32.0));
    }

    #[test]
    fn rejects_unknown_or_malformed_svg_lengths() {
        let context = SvgLengthContext::user_units(100.0);
        assert_eq!(resolve_length("", context), None);
        assert_eq!(resolve_length("12frobs", context), None);
        assert_eq!(resolve_length("e3px", context), None);
        assert_eq!(resolve_length("NaN", context), None);
        assert_eq!(resolve_length("1e309px", context), None);
    }

    #[test]
    fn parses_all_preserve_aspect_ratio_modes() {
        assert_eq!(
            parse_preserve_aspect_ratio("none"),
            SvgPreserveAspectRatio {
                align_x: None,
                align_y: None,
                ..Default::default()
            }
        );
        assert_eq!(
            parse_preserve_aspect_ratio("defer xMaxYMin slice"),
            SvgPreserveAspectRatio {
                defer: true,
                align_x: Some(SvgAlign::Max),
                align_y: Some(SvgAlign::Min),
                meet_or_slice: SvgMeetOrSlice::Slice,
            }
        );
        assert_eq!(
            parse_preserve_aspect_ratio("nonsense"),
            SvgPreserveAspectRatio::default()
        );
    }

    #[test]
    fn parses_every_preserve_aspect_ratio_alignment() {
        for (token, x, y) in [
            ("xMinYMin", SvgAlign::Min, SvgAlign::Min),
            ("xMidYMin", SvgAlign::Mid, SvgAlign::Min),
            ("xMaxYMin", SvgAlign::Max, SvgAlign::Min),
            ("xMinYMid", SvgAlign::Min, SvgAlign::Mid),
            ("xMidYMid", SvgAlign::Mid, SvgAlign::Mid),
            ("xMaxYMid", SvgAlign::Max, SvgAlign::Mid),
            ("xMinYMax", SvgAlign::Min, SvgAlign::Max),
            ("xMidYMax", SvgAlign::Mid, SvgAlign::Max),
            ("xMaxYMax", SvgAlign::Max, SvgAlign::Max),
        ] {
            let parsed = parse_preserve_aspect_ratio(token);
            assert_eq!(parsed.align_x, Some(x), "{token}");
            assert_eq!(parsed.align_y, Some(y), "{token}");
        }
    }

    #[test]
    fn maps_viewboxes_for_none_meet_and_slice() {
        let none = viewbox_transform(
            [0.0, 0.0, 100.0, 50.0],
            [10.0, 20.0, 200.0, 200.0],
            parse_preserve_aspect_ratio("none"),
        )
        .unwrap();
        assert_eq!(none.apply(100.0, 50.0), (210.0, 220.0));

        let meet = viewbox_transform(
            [0.0, 0.0, 100.0, 50.0],
            [0.0, 0.0, 200.0, 200.0],
            parse_preserve_aspect_ratio("xMidYMid meet"),
        )
        .unwrap();
        assert_eq!(meet.apply(0.0, 0.0), (0.0, 50.0));
        assert_eq!(meet.apply(100.0, 50.0), (200.0, 150.0));

        let slice = viewbox_transform(
            [-10.0, -20.0, 100.0, 50.0],
            [0.0, 0.0, 200.0, 200.0],
            parse_preserve_aspect_ratio("xMaxYMin slice"),
        )
        .unwrap();
        assert_eq!(slice.apply(-10.0, -20.0), (-200.0, 0.0));
        assert_eq!(slice.apply(90.0, 30.0), (200.0, 200.0));
    }

    #[test]
    fn tokenizes_compact_path_syntax() {
        assert_eq!(
            tokenize_path_data("M10-20L.5.6"),
            vec![
                SvgPathToken::Command('M'),
                SvgPathToken::Number(10.0),
                SvgPathToken::Number(-20.0),
                SvgPathToken::Command('L'),
                SvgPathToken::Number(0.5),
                SvgPathToken::Number(0.6),
            ]
        );
    }

    #[test]
    fn tokenizes_path_exponents_and_unknown_commands() {
        assert_eq!(
            tokenize_path_data("M1e-3-2E+4R5"),
            vec![
                SvgPathToken::Command('M'),
                SvgPathToken::Number(0.001),
                SvgPathToken::Number(-20_000.0),
                SvgPathToken::Command('R'),
                SvgPathToken::Number(5.0),
            ]
        );
    }

    #[test]
    fn tokenizes_path_data_without_panicking_on_malformed_numbers() {
        assert_eq!(
            tokenize_path_data("M+. .e2 L2"),
            vec![
                SvgPathToken::Command('M'),
                SvgPathToken::Command('e'),
                SvgPathToken::Number(2.0),
                SvgPathToken::Command('L'),
                SvgPathToken::Number(2.0),
            ]
        );
    }

    #[test]
    fn parses_affine_transform_lists() {
        let matrix = Affine2D::parse_transform("translate(10, 5) scale(2) rotate(90)");
        let (x, y) = matrix.apply(1.0, 0.0);

        assert!((x - 10.0).abs() < 0.001);
        assert!((y - 7.0).abs() < 0.001);
    }

    #[test]
    fn parses_rotate_about_origin_point() {
        let matrix = Affine2D::parse_transform("rotate(90 10 10)");
        let (x, y) = matrix.apply(20.0, 10.0);

        assert!((x - 10.0).abs() < 0.001);
        assert!((y - 20.0).abs() < 0.001);
    }

    #[test]
    fn affine_inverse_round_trips_points_and_rejects_singular_matrices() {
        let matrix = Affine2D::translate(10.0, -3.0)
            .multiply(Affine2D::rotate(27.0))
            .multiply(Affine2D::scale(2.0, 0.5));
        let point = matrix.apply(4.0, 9.0);
        let restored = matrix.inverse().unwrap().apply(point.0, point.1);

        assert!((restored.0 - 4.0).abs() < 1.0e-9);
        assert!((restored.1 - 9.0).abs() < 1.0e-9);
        assert!(Affine2D::scale(0.0, 1.0).inverse().is_none());
    }

    #[test]
    fn checked_transform_parser_rejects_malformed_or_unknown_functions() {
        assert!(Affine2D::parse_transform_checked("translate(2 3) rotate(45)").is_some());
        assert!(Affine2D::parse_transform_checked("translate(nope)").is_none());
        assert!(Affine2D::parse_transform_checked("matrix(1 0 0 1 2)").is_none());
        assert!(Affine2D::parse_transform_checked("warp(1 2)").is_none());
        assert!(Affine2D::parse_transform_checked("scale(2) trailing").is_none());
    }
}
