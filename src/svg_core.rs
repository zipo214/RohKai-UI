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

    pub fn parse_transform(value: &str) -> Self {
        let mut rest = value.trim();
        let mut out = Self::IDENTITY;

        while let Some(paren) = rest.find('(') {
            let name = rest[..paren].trim().to_ascii_lowercase();
            let after = &rest[paren + 1..];
            let Some(end) = after.find(')') else {
                break;
            };
            let nums = parse_numbers(&after[..end]);
            let local = match name.as_str() {
                "matrix" if nums.len() >= 6 => Self {
                    a: nums[0],
                    b: nums[1],
                    c: nums[2],
                    d: nums[3],
                    e: nums[4],
                    f: nums[5],
                },
                "translate" if !nums.is_empty() => {
                    Self::translate(nums[0], *nums.get(1).unwrap_or(&0.0))
                }
                "scale" if !nums.is_empty() => {
                    Self::scale(nums[0], *nums.get(1).unwrap_or(&nums[0]))
                }
                "rotate" if !nums.is_empty() => {
                    if nums.len() >= 3 {
                        Self::rotate_about(nums[0], nums[1], nums[2])
                    } else {
                        Self::rotate(nums[0])
                    }
                }
                "skewx" if !nums.is_empty() => Self::skew_x(nums[0]),
                "skewy" if !nums.is_empty() => Self::skew_y(nums[0]),
                _ => Self::IDENTITY,
            };
            out = out.multiply(local);
            rest = &after[end + 1..];
        }

        out
    }

    pub fn parse_chained(value: &str) -> Self {
        Self::parse_transform(value)
    }
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
}
