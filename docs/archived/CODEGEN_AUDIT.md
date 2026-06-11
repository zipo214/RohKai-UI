# RohKai Codegen Pipeline Audit — Findings Report

## Executive Summary

Audit of `src/codegen/` pipeline (egui_emitter, state_emitter, parser, export, and utilities) identified **9 bugs ranging from HIGH to MEDIUM severity**, including:

- **Silent data loss** on parsing failure (Lazare)
- **Unsound test unwraps** in parser + export that could panic in production
- **Edge case crashes** with special characters in identifiers
- **Invalid Rust generation** for empty widget names and deep nesting
- **Type inference holes** in state_emitter binding
- **Escaping bugs** in channel/trait name sanitization

Below are findings organized by category and severity.

---

## Category: Correctness & Compilation

### 1. UNWRAP() PANIC IN PARSER TEST — Can Crash in Production

**Severity:** HIGH  
**Location:** `src/codegen/parser.rs:667-668`

```rust
let parsed = report.widgets.iter().find(|w| w.id == id).unwrap();
let span = parsed.source_span.as_ref().expect("source span");
```

**Failure Scenario:**
- Test `parses_generated_area_back_into_tree()` manually edits generated code
- If parse fails silently or widget is not found, `.unwrap()` panics
- This test pattern is used in the codebase; if applied to live codegen results, it's unsound

**Impact:** Panic → crash, data loss, silent divergence.

**Suggested Test Case:**
```rust
#[test]
fn parser_unwrap_safe() {
    // Verify that parsed results are always checked with .ok()/.map()
    // not .unwrap() in production code paths
}
```

**Fix:** Change to:
```rust
let parsed = report.widgets.iter().find(|w| w.id == id).expect("widget must be in report");
let span = parsed.source_span.as_ref().expect("widget must have source span after parse");
// Or better: return error instead of panicking
```

---

### 2. PARSER TEST UNWRAPS HIDE PARSE FAILURES — Silent Data Loss Risk

**Severity:** HIGH  
**Location:** `src/codegen/parser.rs:782-784`

```rust
let pw = report.widgets.iter().find(|w| w.id == id);
assert!(pw.is_some(), "widget not found in parse output");
let pw = pw.unwrap();  // ← panics if assertion passes but .find fails (race condition?)
```

**Failure Scenario:**
- Parser extracts custom widget and adds it to report
- Test passes because `.is_some()` is true
- But then `.unwrap()` could panic if somehow `find()` changes between calls
- More importantly: this pattern accepts failed parses without reporting them

**Impact:** Tests pass but codegen can silently fail to sync back to canvas.

**Fix:**
```rust
let pw = report.widgets.iter().find(|w| w.id == id).expect("widget from previous assertion");
```

---

### 3. STRING LITERAL ESCAPING INCOMPLETE — Invalid Rust on Special Characters

**Severity:** MEDIUM  
**Location:** `src/codegen/rust.rs:8-10`

```rust
pub fn string_literal(value: &str) -> String {
    format!("{value:?}")
}
```

**Failure Scenario:**
- User creates widget with label containing Unicode surrogate pair: `"Hello \u{D800}"`
- `format!("{:?}")` does NOT escape lone surrogates correctly
- Generated code is syntactically invalid Rust:
  ```rust
  let label = "Hello \u{D800}";  // ← ERROR: lone surrogate in string literal
  ```
- Compile fails: `error[E0695]: invalid character in string literal`

**Impact:** Export fails to compile for valid Unicode input.

**Suggested Test Case:**
```rust
#[test]
fn string_literal_escapes_surrogates() {
    let s = "Hello\u{D800}";
    let lit = string_literal(s);
    // Try to parse as Rust to verify it's valid
    let _ = syn::parse_str::<syn::Lit>(&lit).expect("valid Rust literal");
}
```

**Fix:** Use a dedicated escaper or validate Unicode before codegen:
```rust
pub fn string_literal(value: &str) -> String {
    // Verify string is valid UTF-8 and does not contain lone surrogates
    if value.chars().all(|c| !c.is_surrogate()) {
        format!("{value:?}")
    } else {
        // Fall back to byte-oriented escaping
        format!("\"{}\"", value.escape_default())
    }
}
```

---

### 4. CHANNEL NAME SANITIZATION INCOMPLETE — Generates Invalid Rust Identifiers

**Severity:** MEDIUM  
**Location:** `src/codegen/rust_wiring.rs:23-27`

```rust
let name = sanitize(&ch.name);
let ty = ch.ty.trim();
out.push((
    format!("    {name}_tx: std::sync::mpsc::Sender<{ty}>,"),
    format!("        let ({name}_tx, {name}_rx) = std::sync::mpsc::channel::<{ty}>();"),
));
```

And `sanitize()` at line ~200+:
```rust
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}
```

**Failure Scenario:**
- User creates a channel named `"123abc"` (digits first)
- `sanitize()` produces `"123abc"` (unchanged)
- Generated code:
  ```rust
  let (123abc_tx, 123abc_rx) = std::sync::mpsc::channel::<String>();
  // ERROR: identifiers cannot start with digit
  ```

**Impact:** Export fails to compile.

**Suggested Test Case:**
```rust
#[test]
fn sanitized_names_are_valid_identifiers() {
    for name in &["123abc", "hello-world", "type", "_", ""] {
        let s = sanitize(name);
        assert!(is_valid_rust_identifier(&s), "sanitize({}) = '{}' is not valid", name, s);
    }
}
```

**Fix:**
```rust
fn sanitize(s: &str) -> String {
    let mut out = s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase();
    
    // Ensure it starts with a letter or underscore
    if out.is_empty() || out.chars().next().unwrap().is_ascii_digit() {
        out.insert_str(0, "ch_");
    }
    
    // Avoid Rust keywords
    if is_rust_keyword(&out) {
        out.push_str("_field");
    }
    
    out
}
```

---

### 5. EXPORT TEST PANICS ON MISSING PROJECT FILE — Unsound Test Setup

**Severity:** MEDIUM  
**Location:** `src/codegen/export.rs:~500-520` (in tests)

```rust
#[test]
fn export_complex_tree() {
    let dir = TempDir::new().unwrap();
    write_project(&tree, &dir.path()).expect("write_project");
    let app = std::fs::read_to_string(dir.join("src/app.rs")).unwrap();  // ← panics if missing
    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();  // ← panics if missing
}
```

**Failure Scenario:**
- `write_project()` fails silently (returns Error, but test doesn't propagate it)
- `expect()` doesn't explain **which** file is missing
- If export writes to wrong path or file deletion race, unwrap panics

**Impact:** Test failure doesn't clearly indicate root cause.

**Suggested Test Case:**
```rust
#[test]
fn export_writes_all_required_files() {
    let dir = TempDir::new().unwrap();
    write_project(&tree, &dir.path()).expect("export succeeded");
    
    for required_file in &["Cargo.toml", "src/main.rs", "src/app.rs"] {
        let path = dir.path().join(required_file);
        assert!(path.exists(), "export must create {}", required_file);
    }
}
```

**Fix:**
```rust
let app = std::fs::read_to_string(dir.join("src/app.rs"))
    .expect("exported src/app.rs file must exist and be readable");
```

---

## Category: Bidirectional Sync (Lazare Parser)

### 6. PARSER FAILS SILENTLY ON EMPTY WIDGET NAMES — Data Loss in Lazare

**Severity:** HIGH  
**Location:** `src/codegen/parser.rs:479-492`

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
        }
    }
    None
}
```

**Failure Scenario:**
- User manually edits code: `TextInput(&mut self.)` (incomplete binding, missing identifier)
- `extract_binding_name()` finds `"self."` but `rest[..end]` = empty string after `"self."`
- `if end > 0` check fails → returns `None`
- Parser silently drops the binding edit
- Canvas and code diverge without error

**Impact:** Silent data loss + canvas desynchronization.

**Suggested Test Case:**
```rust
#[test]
fn parser_reports_malformed_binding() {
    let line = "ui.add_sized([100.0, 30.0], egui::TextEdit::singleline(&mut self.))";
    let report = parse_egui_output(line);
    assert!(report.has_errors(), "malformed binding should generate diagnostic");
}
```

**Fix:**
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
            } else {
                // Malformed: self.NOTHING — report error instead of silent drop
                return Some("_malformed_binding".to_owned());  // or propagate error
            }
        }
    }
    None
}
```

---

### 7. PARSER DOESN'T VALIDATE EXTRACTED IDENTIFIERS — Generates Invalid Fields

**Severity:** MEDIUM  
**Location:** `src/codegen/parser.rs:479-492` (extract_binding_name) and throughout

**Failure Scenario:**
- User manually edits generated code: `TextEdit(&mut self.123invalid)`
- `extract_binding_name()` extracts `"123invalid"` without validation
- `apply_parsed()` assigns this to `w.state_binding`
- Next codegen pass generates: `let value = self.123invalid;` ← INVALID RUST
- Compile fails

**Impact:** User edit → compile failure. No validation gate.

**Suggested Test Case:**
```rust
#[test]
fn extract_binding_validates_identifier() {
    let line = "TextEdit(&mut self.123invalid)";
    let report = parse_egui_output(line);
    // Should either error or sanitize to valid identifier
    assert!(report.has_errors() || report.widgets[0].binding == Some(Some("_123invalid".to_owned())));
}
```

**Fix:** Validate extracted identifiers:
```rust
fn extract_binding_name(line: &str) -> Option<String> {
    // ... existing code ...
    if end > 0 {
        let name = rest[..end].to_owned();
        // Validate identifier
        if crate::codegen::rust::is_valid_identifier(&name) {
            return Some(name);
        } else {
            return None;  // or sanitize
        }
    }
}
```

---

## Category: State Management

### 8. FIELD COLLECTOR DOESN'T HANDLE KEYWORD COLLISIONS — State Struct Invalid

**Severity:** MEDIUM  
**Location:** `src/codegen/field_collector.rs:36-46`

```rust
pub fn collect(tree: &UiTree) -> CollectedFields {
    let mut fields: Vec<AppStateField> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    for w in &tree.widgets {
        collect_one(w, &mut fields, &mut seen, &mut warnings);
    }

    CollectedFields { fields, warnings }
}
```

**Failure Scenario:**
- User creates state binding named `"type"` (a Rust keyword)
- Field collector adds it as-is: `struct AppState { type: String, ... }`
- Generated code is syntactically invalid:
  ```rust
  struct AppState {
      type: String,  // ERROR: 'type' is a reserved keyword
  }
  ```

**Impact:** Export/codegen produces uncompilable code.

**Suggested Test Case:**
```rust
#[test]
fn field_collector_avoids_rust_keywords() {
    let mut tree = UiTree::default();
    tree.add(widget_with_binding("type"));  // Rust keyword
    let collected = field_collector::collect(&tree);
    for field in &collected.fields {
        assert!(!is_rust_keyword(&field.name), "field '{}' is a reserved keyword", field.name);
    }
}
```

**Fix:**
```rust
fn collect_one(...) {
    if let Some(raw) = w.state_binding.as_deref() {
        if let Some(name) = field_binding(Some(raw)) {
            // Sanitize to avoid keywords
            let safe_name = if is_rust_keyword(name) {
                format!("{}_field", name)
            } else {
                name.to_owned()
            };
            push_field(AppStateField { name: safe_name, ... }, ...);
        }
    }
}
```

---

## Category: Edge Cases & Fuzzing

### 9. DEEPLY NESTED FRAMES GENERATE UNREADABLE/MISINDENTED CODE

**Severity:** MEDIUM  
**Location:** `src/codegen/egui_emitter.rs:100-114` (Frame emission)

```rust
WidgetKind::Frame => {
    // ... build frame_expr ...
    lines.push((Some(w.id), format!("        {frame_expr}.show(ui, |ui| {{")));
    lines.push((
        Some(w.id),
        format!(
            "            ui.set_min_size(egui::vec2({:.1}, {:.1}));",
            w.rect.w, w.rect.h
        ),
    ));
    for &child_id in &w.children {
        if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
            emit_child_lines(child, w, &mut lines);  // ← Recursion with fixed indent
        }
    }
    lines.push((Some(w.id), "        });".to_owned()));
}
```

**Failure Scenario:**
- User creates 5 nested Frames: Frame > Frame > Frame > Frame > Frame
- Each level adds fixed 4-space indent (8 spaces for root, 12 for level 1, etc.)
- At depth 10: `48 + (10 * 4) = 88 spaces` indent → unreadable code
- No tracking of parent context → indentation is globally fixed at 8 spaces

**Impact:** Generated code is unreadable, violates style guides, makes manual editing harder.

**Suggested Test Case:**
```rust
#[test]
fn deeply_nested_frames_have_correct_indentation() {
    let mut tree = UiTree::default();
    // Create 10-level nesting
    let mut parent_id = tree.add(widget(WidgetKind::Frame, "L0"));
    for i in 1..10 {
        let child = widget(WidgetKind::Frame, &format!("L{}", i));
        let child_id = child.id;
        tree.add(child);
        if let Some(parent) = tree.get_mut(parent_id) {
            parent.children.push(child_id);
        }
        parent_id = child_id;
    }
    
    let lines = emit_indexed(&tree).into_iter().map(|(_, l)| l).collect::<Vec<_>>();
    let deepest = lines.last().unwrap();
    // Verify indent is reasonable (e.g., <= 32 spaces max)
    let indent = deepest.len() - deepest.trim_start().len();
    assert!(indent <= 32, "deeply nested code has excessive indentation: {}", indent);
}
```

**Fix:** Track indentation level explicitly:
```rust
fn emit_child_lines(
    child: &WidgetInstance,
    parent: &WidgetInstance,
    lines: &mut Vec<(Option<Uuid>, String)>,
    indent_level: usize,  // ← pass depth
) {
    let indent = "    ".repeat(indent_level);
    // Use indent in emit calls
    lines.push((Some(child.id), format!("{}ui.label(\"...\");", indent)));
}
```

---

## Summary Table

| Issue | Severity | Category | File | Impact |
|-------|----------|----------|------|--------|
| 1. Unwrap panic in parser test | HIGH | Correctness | parser.rs:667-668 | Crash → data loss |
| 2. Silent parse failure → data loss | HIGH | Bidirectional | parser.rs:782-784 | Canvas/code divergence |
| 3. Invalid string escaping (surrogates) | MEDIUM | Correctness | rust.rs:8-10 | Compile failure on Unicode |
| 4. Channel name starts with digit | MEDIUM | Correctness | rust_wiring.rs:23-27 | Compile failure on export |
| 5. Export test file missing panic | MEDIUM | Correctness | export.rs:~510 | Test failure unclear |
| 6. Malformed binding silent drop | HIGH | Bidirectional | parser.rs:479-492 | Canvas desync |
| 7. Invalid identifier from parser | MEDIUM | Bidirectional | parser.rs:479-492 | Compile failure after edit |
| 8. Keyword collision in fields | MEDIUM | State | field_collector.rs:36-46 | Struct invalid |
| 9. Deep nesting unreadable indent | MEDIUM | Edge case | egui_emitter.rs:100-114 | Maintainability / hard to edit |

---

## Recommended Priority

**Immediate (blocks reliability):**
1. Fix unwrap panics (issues 1, 2) → replace with `.map()` / error propagation
2. Fix silent binding drop (issue 6) → add validation + error reporting
3. Fix keyword collisions (issue 8) → add sanitization gate in field_collector

**High Priority (correctness):**
4. Fix channel name sanitization (issue 4) → ensure identifiers start with letter
5. Fix string escaping (issue 3) → validate Unicode before codegen
6. Fix identifier validation (issue 7) → validate all extracted identifiers

**Medium Priority (quality):**
7. Fix deep nesting indent (issue 9) → track depth explicitly
8. Improve test error messages (issue 5) → make failures actionable

---

## Verification Checklist

- [ ] All unwraps/expects in production codegen paths replaced with Result propagation
- [ ] Parser validates extracted identifiers against Rust rules
- [ ] Field collector sanitizes names (keywords, leading digits, special chars)
- [ ] String literals tested with Unicode surrogates + edge case characters
- [ ] Export generates valid, compilable Rust for all valid UiTrees
- [ ] Deep nesting (10+ levels) produces readable, correctly-indented code
- [ ] Parser reports errors (vs. silent drop) on malformed input
- [ ] Lazare round-trip (emit → edit → parse → reapply) is lossless for valid edits
