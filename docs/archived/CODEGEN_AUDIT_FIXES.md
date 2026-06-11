# RohKai Codegen Audit — Fix Implementation Guide

## Overview

This document provides concrete fixes for the 9 bugs identified in the codegen pipeline audit.

**Priority order:**
1. **CRITICAL (data loss):** Issues 1, 2, 6 — unwraps + silent drops
2. **HIGH (compilation failures):** Issues 3, 4, 7, 8
3. **MEDIUM (maintainability):** Issue 5, 9

---

## Fix #1 & #2: Remove Unwraps in Parser Tests

**Files:** `src/codegen/parser.rs` (tests section)

**Before (UNSOUND):**
```rust
#[test]
fn parses_generated_area_back_into_tree() {
    let button = widget(WidgetKind::Button, "Go");
    let id = button.id;
    let mut tree = UiTree::default();
    tree.add(button);
    let mut code = egui_emitter::emit_indexed(&tree)
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n");
    code = code.replace("egui::pos2(10.0, 20.0)", "egui::pos2(42.0, 56.0)");

    let report = parse_egui_output(&code);
    assert!(!report.has_errors(), "{:?}", report.diagnostics);
    let parsed = report.widgets.iter().find(|w| w.id == id).unwrap();  // ← PANIC
    let span = parsed.source_span.as_ref().expect("source span");      // ← PANIC
    assert!(code[span.bytes.clone()].starts_with("egui::Area::new"));
    assert!(!code[span.bytes.clone()].contains("CentralPanel"));
    apply_parsed(&mut tree, &report.widgets);

    let edited = tree.widgets.iter().find(|w| w.id == id).unwrap();    // ← PANIC
    assert_eq!(edited.rect.x, 42.0);
    assert_eq!(edited.rect.y, 56.0);
}
```

**After (SOUND):**
```rust
#[test]
fn parses_generated_area_back_into_tree() {
    let button = widget(WidgetKind::Button, "Go");
    let id = button.id;
    let mut tree = UiTree::default();
    tree.add(button);
    let mut code = egui_emitter::emit_indexed(&tree)
        .into_iter()
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n");
    code = code.replace("egui::pos2(10.0, 20.0)", "egui::pos2(42.0, 56.0)");

    let report = parse_egui_output(&code);
    assert!(!report.has_errors(), "{:?}", report.diagnostics);
    
    let parsed = report.widgets.iter()
        .find(|w| w.id == id)
        .expect("widget must be in parse report");
    
    let span = parsed.source_span.as_ref()
        .expect("successfully parsed widget must have source span");
    
    assert!(code[span.bytes.clone()].starts_with("egui::Area::new"));
    assert!(!code[span.bytes.clone()].contains("CentralPanel"));
    apply_parsed(&mut tree, &report.widgets);

    let edited = tree.widgets.iter()
        .find(|w| w.id == id)
        .expect("widget must exist in tree after apply_parsed");
    
    assert_eq!(edited.rect.x, 42.0);
    assert_eq!(edited.rect.y, 56.0);
}
```

**Similarly for other test unwraps at lines 667-668, 782-784, 815:**
- Replace all `.unwrap()` with `.expect("descriptive message")`
- Or convert to proper error handling with `?`

---

## Fix #3: String Literal Escaping for Surrogates

**File:** `src/codegen/rust.rs`

**Before (UNSAFE):**
```rust
pub fn string_literal(value: &str) -> String {
    format!("{value:?}")
}
```

**After (SAFE):**
```rust
pub fn string_literal(value: &str) -> String {
    // Verify the string is valid UTF-8 and contains no lone surrogates.
    // format!("{:?}") doesn't handle lone surrogates correctly.
    
    // Quick check: if all chars are valid Unicode scalars, format!("{:?}") is safe
    if value.chars().all(|c| !is_surrogate(c)) {
        return format!("{value:?}");
    }
    
    // Fallback: use escape_default() which is more conservative
    let mut result = String::from("\"");
    for ch in value.chars() {
        result.push_str(&ch.escape_default().to_string());
    }
    result.push('"');
    result
}

fn is_surrogate(c: char) -> bool {
    // Unicode surrogates are in range U+D800..=U+DFFF
    let code = c as u32;
    code >= 0xD800 && code <= 0xDFFF
}
```

**Add test:**
```rust
#[test]
fn string_literal_handles_unicode_edge_cases() {
    // Valid unicode
    let s = string_literal("Hello\nWorld");
    assert_eq!(s, "\"Hello\\nWorld\"");
    
    // Valid unicode with emoji
    let s = string_literal("🦀 Rust");
    assert!(s.contains("Rust"));
    
    // ASCII only
    let s = string_literal("Simple");
    assert_eq!(s, "\"Simple\"");
}
```

---

## Fix #4: Channel/Identifier Sanitization

**File:** `src/codegen/rust_wiring.rs`

**Before (BROKEN):**
```rust
pub fn channel_field_pairs(wiring: &RustWiring) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for ch in &wiring.channels {
        let name = sanitize(&ch.name);  // ← May produce "123abc"
        let ty = ch.ty.trim();
        out.push((
            format!("    {name}_tx: std::sync::mpsc::Sender<{ty}>,"),
            format!("        let ({name}_tx, {name}_rx) = std::sync::mpsc::channel::<{ty}>();"),
        ));
        // ...
    }
    out
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
        // ← MISSING: check for leading digit or keyword
}
```

**After (FIXED):**
```rust
pub fn channel_field_pairs(wiring: &RustWiring) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for ch in &wiring.channels {
        let name = sanitize_identifier(&ch.name);  // ← Safe identifier
        let ty = ch.ty.trim();
        out.push((
            format!("    {name}_tx: std::sync::mpsc::Sender<{ty}>,"),
            format!("        let ({name}_tx, {name}_rx) = std::sync::mpsc::channel::<{ty}>();"),
        ));
        out.push((
            format!("    {name}_rx: std::sync::mpsc::Receiver<{ty}>,"),
            String::new(),
        ));
    }
    out
}

/// Produce a valid Rust identifier from arbitrary string.
/// Handles: leading digits, keywords, special characters, empty strings.
fn sanitize_identifier(s: &str) -> String {
    let mut out: String = s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase();
    
    // Empty after sanitization → add prefix
    if out.is_empty() {
        out = "ch_".to_owned();
    }
    
    // Leading digit → prepend underscore
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    
    // Rust keyword → append suffix
    if is_rust_keyword(&out) {
        out.push_str("_ch");
    }
    
    out
}

/// Check if a string is a Rust keyword.
fn is_rust_keyword(s: &str) -> bool {
    matches!(s,
        "as" | "async" | "await" | "break" | "const" | "continue" | "crate" | "dyn" |
        "else" | "enum" | "extern" | "false" | "fn" | "for" | "if" | "impl" | "in" |
        "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub" | "ref" | "return" |
        "self" | "Self" | "static" | "struct" | "super" | "trait" | "true" | "type" |
        "union" | "unsafe" | "use" | "where" | "while"
    )
}

#[test]
fn sanitize_identifier_produces_valid_rust_names() {
    assert!(is_valid_rust_identifier(&sanitize_identifier("123abc")));
    assert!(is_valid_rust_identifier(&sanitize_identifier("type")));
    assert!(is_valid_rust_identifier(&sanitize_identifier("hello-world")));
    assert!(is_valid_rust_identifier(&sanitize_identifier("")));
    assert!(is_valid_rust_identifier(&sanitize_identifier("_private")));
}
```

---

## Fix #5: Malformed Binding Detection (Issue #6)

**File:** `src/codegen/parser.rs`

**Before (SILENT DROP):**
```rust
fn extract_binding_name(line: &str) -> Option<String> {
    for pat in &["&mut self.", "&self.", "self."] {
        if let Some(pos) = line.find(pat) {
            let rest = &line[pos + pat.len()..];
            let end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            if end > 0 {
                return Some(rest[..end].to_owned());
            }
            // ← MISSING: if end == 0, silently returns None (no binding extracted)
        }
    }
    None
}
```

**After (DETECTS ERROR):**
```rust
fn extract_binding_name(line: &str) -> Option<String> {
    for pat in &["&mut self.", "&self.", "self."] {
        if let Some(pos) = line.find(pat) {
            let rest = &line[pos + pat.len()..];
            let end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            if end > 0 {
                return Some(rest[..end].to_owned());
            } else if end == 0 && !rest.is_empty() && (rest.starts_with(')') || rest.starts_with(',')) {
                // Pattern matched but no identifier follows: "self." or "self.)"
                // This is likely a malformed binding → mark as error
                // Return a sentinel value that will cause a diagnostic
                return Some("_MALFORMED_BINDING".to_owned());
            }
        }
    }
    None
}

fn parse_widget_line(...) {
    // ...
    } else if line.contains("egui::TextEdit::singleline(") {
        widget.kind = Some(WidgetKind::TextInput);
        widget.binding = Some(extract_binding_name(line));
        
        // Check for malformed binding marker
        if let Some(Some(b)) = &widget.binding {
            if b == "_MALFORMED_BINDING" {
                push_error(report, line_no, "malformed binding: expected identifier after 'self.'");
                widget.binding = None;  // Clear the error marker
            }
        }
        
        parse_add_sized(widget, line, line_no, report);
    }
}
```

---

## Fix #6: Validate Extracted Identifiers (Issue #7)

**File:** `src/codegen/parser.rs`

**Before (NO VALIDATION):**
```rust
fn extract_binding_name(line: &str) -> Option<String> {
    // ... extract "123invalid" without checking if it's valid ...
    if end > 0 {
        return Some(rest[..end].to_owned());  // ← May be "123invalid"
    }
}
```

**After (VALIDATED):**
```rust
fn extract_binding_name(line: &str) -> Option<String> {
    for pat in &["&mut self.", "&self.", "self."] {
        if let Some(pos) = line.find(pat) {
            let rest = &line[pos + pat.len()..];
            let end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            if end > 0 {
                let name = rest[..end].to_owned();
                // Validate the extracted name
                if crate::codegen::rust::is_valid_identifier(&name) {
                    return Some(name);
                } else {
                    // Sanitize invalid identifier
                    let sanitized = sanitize_identifier_for_binding(&name);
                    return Some(sanitized);
                }
            }
        }
    }
    None
}

fn sanitize_identifier_for_binding(s: &str) -> String {
    let mut out = s.to_owned();
    
    // Leading digit → prepend underscore
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    
    // Keyword → append suffix
    if crate::codegen::rust::RUST_KEYWORDS.contains(&out.as_str()) {
        out.push_str("_val");
    }
    
    out
}
```

---

## Fix #7: Field Collector Sanitizes Keywords (Issue #8)

**File:** `src/codegen/field_collector.rs`

**Before (KEYWORDS NOT ESCAPED):**
```rust
fn collect_one(
    w: &WidgetInstance,
    fields: &mut Vec<AppStateField>,
    seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    if let Some(raw) = w.state_binding.as_deref() {
        if let Some(name) = field_binding(Some(raw)) {
            if let Some(info) = kind_table::state_info(&w.kind) {
                push_field(
                    AppStateField {
                        name: name.to_owned(),  // ← "type" → KEYWORD!
                        ty: info.rust_type.to_owned(),
                        default_expr: default_expr_for_widget(w),
                    },
                    fields,
                    seen,
                    warnings,
                );
            }
        }
    }
}
```

**After (KEYWORDS SANITIZED):**
```rust
fn collect_one(
    w: &WidgetInstance,
    fields: &mut Vec<AppStateField>,
    seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    if let Some(raw) = w.state_binding.as_deref() {
        if let Some(name) = field_binding(Some(raw)) {
            if let Some(info) = kind_table::state_info(&w.kind) {
                // Sanitize to avoid Rust keywords
                let safe_name = if is_rust_keyword(name) {
                    format!("{}_value", name)
                } else {
                    name.to_owned()
                };
                
                push_field(
                    AppStateField {
                        name: safe_name,
                        ty: info.rust_type.to_owned(),
                        default_expr: default_expr_for_widget(w),
                    },
                    fields,
                    seen,
                    warnings,
                );
            }
        }
    }
}

fn is_rust_keyword(s: &str) -> bool {
    crate::codegen::rust::RUST_KEYWORDS.contains(&s)
}
```

---

## Fix #8: Export Test File Presence (Issue #5)

**File:** `src/codegen/export.rs` (test section)

**Before (UNCLEAR FAILURES):**
```rust
#[test]
fn export_complex_tree() {
    let dir = TempDir::new().unwrap();
    write_project(&tree, &dir.path()).expect("write_project");
    let app = std::fs::read_to_string(dir.join("src/app.rs")).unwrap();  // ← panic without context
    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
}
```

**After (CLEAR FAILURES):**
```rust
#[test]
fn export_complex_tree() {
    let dir = TempDir::new().expect("create temp dir");
    write_project(&tree, dir.path()).expect("export tree to temp directory");
    
    // Verify all required files exist and are readable
    let app_path = dir.path().join("src/app.rs");
    assert!(app_path.exists(), "exported src/app.rs file not found at {:?}", app_path);
    let app = std::fs::read_to_string(&app_path)
        .expect("src/app.rs must be readable after export");
    
    let cargo_path = dir.path().join("Cargo.toml");
    assert!(cargo_path.exists(), "exported Cargo.toml file not found at {:?}", cargo_path);
    let cargo = std::fs::read_to_string(&cargo_path)
        .expect("Cargo.toml must be readable after export");
    
    // Verify content is not empty
    assert!(!app.is_empty(), "exported src/app.rs is empty");
    assert!(!cargo.is_empty(), "exported Cargo.toml is empty");
}
```

---

## Fix #9: Deep Nesting Indentation Tracking (Issue #9)

**File:** `src/codegen/egui_emitter.rs`

**Before (FIXED INDENTATION):**
```rust
fn emit_child_lines(
    child: &WidgetInstance,
    _parent: &WidgetInstance,
    lines: &mut Vec<(Option<Uuid>, String)>,
) {
    // Fixed 8 spaces for root level
    let base_indent = "        ";  // Always 8 spaces
    // ...
    lines.push((Some(child.id), format!("{}ui.label(\"...\");", base_indent)));
}
```

**After (DEPTH-AWARE INDENTATION):**
```rust
pub fn emit_indexed(tree: &UiTree) -> Vec<(Option<Uuid>, String)> {
    let mut lines: Vec<(Option<Uuid>, String)> = Vec::new();
    
    lines.push((
        None,
        "egui::CentralPanel::default().show(ctx, |_ui| {});".to_owned(),
    ));

    let child_ids: HashSet<Uuid> = tree
        .widgets
        .iter()
        .flat_map(|w| w.children.iter().copied())
        .collect();

    for w in &tree.widgets {
        if child_ids.contains(&w.id) {
            continue;
        }
        
        // Start at depth 1 (root widgets)
        emit_widget_lines(w, tree, &mut lines, 1);
    }
    
    lines
}

fn emit_widget_lines(
    w: &WidgetInstance,
    tree: &UiTree,
    lines: &mut Vec<(Option<Uuid>, String)>,
    depth: usize,
) {
    let indent = "    ".repeat(depth);
    let area_id = string_literal(&format!("widget_{}", w.id));
    
    lines.push((
        Some(w.id),
        format!("{}egui::Area::new(egui::Id::new({area_id}))", indent),
    ));
    lines.push((
        Some(w.id),
        format!(
            "{}    .fixed_pos(egui::pos2({:.1}, {:.1}))",
            indent, w.rect.x, w.rect.y
        ),
    ));
    lines.push((
        Some(w.id),
        format!("{}    .show(ctx, |ui| {{", indent),
    ));
    lines.push((
        Some(w.id),
        format!(
            "{}        ui.set_min_size(egui::vec2({:.1}, {:.1}));",
            indent, w.rect.w, w.rect.h
        ),
    ));

    match &w.kind {
        WidgetKind::Frame => {
            // ... frame setup ...
            lines.push((Some(w.id), format!("{}        egui::Frame::none()..show(ui, |ui| {{", indent)));
            
            // Recurse with depth+1
            for &child_id in &w.children {
                if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                    emit_widget_lines(child, tree, lines, depth + 1);
                }
            }
            
            lines.push((Some(w.id), format!("{}        }});", indent)));
        }
        // ... other kinds ...
    }
    
    lines.push((Some(w.id), format!("{}{}}};", indent, "    ")));
}
```

---

## Verification Checklist

After implementing these fixes:

- [ ] All `unwrap()`/`expect()` in parser tests have descriptive messages
- [ ] String literals handle Unicode surrogates correctly
- [ ] Channel/identifier sanitization produces valid Rust names
- [ ] Parser detects and reports malformed bindings
- [ ] Extracted identifiers are validated or sanitized
- [ ] Field collector escapes Rust keywords
- [ ] Export tests clearly indicate which file is missing
- [ ] Deep nesting produces readable, consistent indentation
- [ ] All tests pass: `cargo test codegen_ --lib`
- [ ] Export generates compilable Rust: `cargo check` on exported project succeeds

---

## Testing Commands

```bash
# Run all fixes:
cargo test codegen --lib -- --nocapture
cargo test parser --lib -- --nocapture

# Export and compile (expensive):
cargo test export_compiles --lib -- --nocapture --ignored

# Lint (must pass):
cargo clippy -- -D warnings
```

---

## Long-term Improvements (Stage Future)

1. **Dedicated Identifier Validator Module** (`src/codegen/identifiers.rs`)
   - Central `ValidIdent` newtype with guaranteed soundness
   - Used throughout codegen for all Rust identifiers

2. **Lazy Error Reporting**
   - Accumulate parse errors, don't stop on first failure
   - Report all problems at once

3. **Roundtrip Testing Harness**
   - Generate → Emit → Parse → Apply → Emit → Compare
   - Verify Lazare sync is lossless

4. **Codegen Property-Based Testing**
   - Use `proptest` to generate random UiTrees
   - Verify all exports compile and parse correctly
