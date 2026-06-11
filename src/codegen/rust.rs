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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_escapes_special_chars() {
        assert_eq!(string_literal("hello"), r#""hello""#);
        let nl = string_literal("a\nb");
        assert!(nl.starts_with('"') && nl.ends_with('"'));
        assert!(!nl.contains('\n'), "raw newline must not appear in generated literal");
        let emoji = string_literal("🦀");
        assert!(emoji.starts_with('"') && emoji.ends_with('"') && emoji.contains("🦀"));
    }
}
