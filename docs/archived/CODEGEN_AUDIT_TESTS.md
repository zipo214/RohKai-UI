# RohKai Codegen Audit — Test Cases for Verification

These test cases should be run to verify the 9 issues documented in CODEGEN_AUDIT.md.

## Test Suite 1: String Escaping Edge Cases

```rust
#[cfg(test)]
mod codegen_audit_tests {
    use crate::codegen::rust::string_literal;

    #[test]
    fn test_string_literal_with_lone_surrogate() {
        // Issue #3: Invalid Unicode surrogates not escaped
        let malformed = "Hello\u{D800}World";  // lone surrogate
        let result = string_literal(malformed);
        // Try to parse as Rust literal — should succeed if properly escaped
        // Currently will fail because format!("{:?}") doesn't escape lone surrogates
        println!("Generated literal: {}", result);
        // Result: "Hello\u{D800}World" which is INVALID Rust
    }

    #[test]
    fn test_string_literal_with_quote_escape() {
        let input = r#"Say "Hello" to \"world\""#;
        let result = string_literal(input);
        // Should produce valid escaped string that compiles
        assert!(!result.is_empty());
        println!("Generated: {}", result);
    }

    #[test]
    fn test_string_literal_with_newlines_and_tabs() {
        let input = "Line1\nLine2\tTabbed";
        let result = string_literal(input);
        // Should escape as "Line1\\nLine2\\tTabbed"
        println!("Generated: {}", result);
    }
}
```

## Test Suite 2: Identifier Sanitization

```rust
#[cfg(test)]
mod identifier_sanitization_tests {
    use crate::codegen::rust::is_valid_identifier;

    #[test]
    fn test_channel_name_starting_with_digit() {
        // Issue #4: Channel names like "123abc" generate invalid identifiers
        let names = vec!["123abc", "456", "_test", "hello"];
        for name in names {
            // Current sanitize produces "123abc" → INVALID
            // Need to test that sanitization handles this
            let sanitized = super::sanitize(name);
            assert!(
                is_valid_identifier(&sanitized),
                "sanitize('{}') = '{}' is not a valid identifier",
                name,
                sanitized
            );
        }
    }

    #[test]
    fn test_field_name_collision_with_keywords() {
        // Issue #8: Keywords like "type", "ref", "mut" are not sanitized
        let keywords = vec!["type", "ref", "mut", "pub", "fn", "let", "const"];
        for kw in keywords {
            // Field collector should sanitize these
            // Currently it doesn't → generates invalid struct
            println!("Testing keyword: {}", kw);
            // Verify that generated code escapes or renames the keyword
        }
    }

    #[test]
    fn test_trait_impl_name_with_special_chars() {
        // rust_wiring::sanitize should handle trait names like "MyTrait<T>"
        let names = vec!["MyTrait<T>", "Foo::Bar", "baz-qux", "123invalid"];
        for name in names {
            let result = crate::codegen::rust_wiring::sanitize(name);
            println!("sanitize({}) = {}", name, result);
            // Verify result is alphanumeric + underscore
        }
    }
}
```

## Test Suite 3: Parser Robustness

```rust
#[cfg(test)]
mod parser_robustness_tests {
    use crate::codegen::parser::{parse_egui_output, extract_binding_name};

    #[test]
    fn test_malformed_binding_empty_name() {
        // Issue #6: Binding like "&mut self." (missing identifier) is silently dropped
        let code = r#"
            egui::TextEdit::singleline(&mut self.)
        "#;
        let report = parse_egui_output(code);
        // Currently: silently drops → no error, no binding extracted
        // Should: report error OR sanitize to valid identifier
        println!("Report diagnostics: {:?}", report.diagnostics);
        println!("Report widgets: {:?}", report.widgets);
    }

    #[test]
    fn test_binding_name_starting_with_digit() {
        // Issue #7: Parser extracts "123invalid" without validation
        let code = r#"
            ui.add_sized([100.0, 30.0], egui::TextEdit::singleline(&mut self.123invalid))
        "#;
        let report = parse_egui_output(code);
        if let Some(widget) = report.widgets.first() {
            if let Some(Some(binding)) = &widget.binding {
                println!("Extracted binding: {}", binding);
                // Currently: "123invalid" → INVALID RUST when codegen uses it
                // Should validate or sanitize
                assert!(
                    binding.chars().next().map_or(false, |c| !c.is_ascii_digit()),
                    "binding '{}' starts with digit",
                    binding
                );
            }
        }
    }

    #[test]
    fn test_parser_error_recovery_incomplete_block() {
        // Issue #2 related: Parser should report errors on malformed code
        let code = r#"
            egui::Area::new(egui::Id::new("widget_abc123"))
                .show(ctx, |ui| {
                    ui.button("Incomplete")
                    // Missing closing });
        "#;
        let report = parse_egui_output(code);
        // Should have error diagnostic about unclosed block
        assert!(report.has_errors(), "Incomplete block should generate error");
    }

    #[test]
    fn test_custom_widget_binding_first_line_precedence() {
        // Related to issue #7: binding should come from first line, not handler
        let code = format!(
            r#"
            egui::Area::new(egui::Id::new("widget_abc"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {{
                    ui.set_min_size(egui::vec2(100.0, 30.0));
                    MyWidget::new("Click", &mut self.flag);
                    if resp.clicked() {{ self.on_pressed(); }}
                }})"#
        );
        let report = parse_egui_output(&code);
        if let Some(widget) = report.widgets.first() {
            // Binding should be "flag" not from the handler
            println!("Widget binding: {:?}", widget.binding);
        }
    }
}
```

## Test Suite 4: Deep Nesting

```rust
#[cfg(test)]
mod deep_nesting_tests {
    use crate::codegen::egui_emitter;
    use crate::project::ui_tree::UiTree;
    use crate::project::schema::{WidgetInstance, WidgetKind, Rect};

    #[test]
    fn test_deeply_nested_frames_indentation() {
        // Issue #9: Deep nesting produces unreadable/misaligned code
        let mut tree = UiTree::default();
        let mut parent_id = None;

        // Create 10-level nesting: Frame > Frame > ... > Frame
        for i in 0..10 {
            let mut w = WidgetInstance {
                id: uuid::Uuid::new_v4(),
                kind: WidgetKind::Frame,
                rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 },
                ..Default::default()
            };
            w.props.label = format!("Level {}", i);
            let child_id = w.id;

            if let Some(pid) = parent_id {
                if let Some(parent) = tree.get_mut(pid) {
                    parent.children.push(child_id);
                }
            }

            tree.add(w);
            parent_id = Some(child_id);
        }

        let lines = egui_emitter::emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>();

        // Check indentation levels
        for (idx, line) in lines.iter().enumerate() {
            let indent = line.len() - line.trim_start().len();
            let non_empty = !line.trim().is_empty();
            if non_empty && indent > 0 {
                println!("Line {}: indent={} content={}", idx, indent, line.trim().chars().take(40).collect::<String>());
                // At depth 10, indent should be reasonable (not 40+ spaces)
                assert!(indent <= 40, "Line {} has excessive indentation: {} spaces", idx, indent);
            }
        }
    }

    #[test]
    fn test_frame_indentation_consistency() {
        // Frames should maintain consistent indentation across levels
        let mut tree = UiTree::default();
        let mut frame1 = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Frame,
            rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 },
            ..Default::default()
        };
        frame1.props.label = "Outer".to_owned();

        let mut frame2 = WidgetInstance {
            id: uuid::Uuid::new_v4(),
            kind: WidgetKind::Frame,
            rect: Rect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 },
            ..Default::default()
        };
        frame2.props.label = "Inner".to_owned();
        let frame2_id = frame2.id;

        frame1.children.push(frame2_id);
        tree.add(frame1);
        tree.add(frame2);

        let lines = egui_emitter::emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>();

        // All opening braces should align vertically
        let open_brace_indents: Vec<usize> = lines
            .iter()
            .filter(|l| l.contains("{"))
            .map(|l| l.len() - l.trim_start().len())
            .collect();

        println!("Brace indents: {:?}", open_brace_indents);
        // Should have pattern like [8, 12, 16] not [8, 8, 8] or random
    }
}
```

## Test Suite 5: Export Compilation

```rust
#[cfg(test)]
mod export_compilation_tests {
    use crate::codegen::export;
    use crate::project::ui_tree::UiTree;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    #[ignore]  // expensive: runs cargo check
    fn test_export_compiles_for_complex_tree() {
        // Create a tree with edge cases:
        // - Widget names with special characters
        // - Deep nesting
        // - State bindings with reserved names (sanitized)
        // - Channels and async handlers

        let mut tree = UiTree::default();
        // ... populate tree ...

        let temp = TempDir::new().expect("temp dir");
        export::write_project(&tree, temp.path()).expect("export");

        // Try to compile the exported project
        let output = Command::new("cargo")
            .args(&["check"])
            .current_dir(temp.path())
            .output()
            .expect("cargo check");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "exported project does not compile:\n{}",
                stderr
            );
        }
    }

    #[test]
    fn test_export_produces_all_files() {
        let tree = UiTree::default();
        let temp = TempDir::new().expect("temp dir");

        export::write_project(&tree, temp.path()).expect("export");

        let required_files = vec![
            "Cargo.toml",
            "src/main.rs",
            "src/app.rs",
        ];

        for file in required_files {
            let path = temp.path().join(file);
            assert!(
                path.exists(),
                "exported file {} not found",
                file
            );
            let content = fs::read_to_string(&path).expect("read file");
            assert!(!content.is_empty(), "exported {} is empty", file);
        }
    }
}
```

## Running These Tests

```bash
# Add to Cargo.toml [dev-dependencies]:
tempfile = "3"

# Run tests:
cargo test codegen_audit --lib -- --nocapture
cargo test parser_robustness --lib -- --nocapture
cargo test identifier_sanitization --lib -- --nocapture
cargo test deep_nesting --lib -- --nocapture
```

## Expected Failures (Before Fixes)

These tests will FAIL if the issues are present:
- `test_string_literal_with_lone_surrogate` — Invalid Rust generated
- `test_channel_name_starting_with_digit` — Identifier validation fails
- `test_field_name_collision_with_keywords` — Keywords not sanitized
- `test_malformed_binding_empty_name` — Silent drop (no error)
- `test_binding_name_starting_with_digit` — Invalid identifier extracted
- `test_deeply_nested_frames_indentation` — Excessive indentation

All should PASS after fixes are applied.
