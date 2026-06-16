pub const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while",
];

pub fn string_literal(value: &str) -> String {
    // &str is valid UTF-8; format!("{:?}") produces a valid Rust string literal.
    // Characters that need escaping (newlines, tabs, quotes, backslashes) are
    // handled correctly by the Debug formatter for str.
    format!("{value:?}")
}

pub fn is_valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && !RUST_KEYWORDS.contains(&value)
}

pub fn field_binding(value: Option<&str>) -> Option<&str> {
    value.filter(|binding| is_valid_identifier(binding))
}

/// Map a raw binding name to its effective Rust field name.
///
/// Rust keywords (e.g. `"type"`) are remapped to `"{raw}_value"` so every
/// consumer (emitter, field collector, overlays) uses the same sanitized name.
pub fn effective_binding(raw: &str) -> String {
    if RUST_KEYWORDS.contains(&raw) {
        format!("{raw}_value")
    } else {
        raw.to_owned()
    }
}

/// Resolve a raw state binding to the field identifier codegen actually emits:
/// apply the keyword remap (`effective_binding`) then validate. Returns `None`
/// for invalid identifiers (leading digit, empty, …).
///
/// Every state-binding → `self.<field>` site (field collector, emitter, export,
/// canvas overlays, descriptor templates) must route through this so the canvas,
/// preview, and exported project agree on one field name. Using raw
/// `field_binding` instead silently drops keyword bindings on one surface while
/// the field collector keeps them as `<kw>_value`, desyncing the surfaces.
pub fn effective_field_binding(value: Option<&str>) -> Option<String> {
    let eff = effective_binding(value?);
    is_valid_identifier(&eff).then_some(eff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_escapes_special_chars() {
        assert_eq!(string_literal("hello"), r#""hello""#);
        let nl = string_literal("a\nb");
        assert!(nl.starts_with('"') && nl.ends_with('"'));
        assert!(
            !nl.contains('\n'),
            "raw newline must not appear in generated literal"
        );
        let emoji = string_literal("🦀");
        assert!(emoji.starts_with('"') && emoji.ends_with('"') && emoji.contains("🦀"));
    }
}
