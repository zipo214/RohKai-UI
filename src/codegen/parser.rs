use crate::project::schema::WidgetKind;
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct ParseDiagnostic {
    pub severity: ParseSeverity,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct ParseReport {
    pub widgets: Vec<ParsedWidget>,
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl ParseReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == ParseSeverity::Error)
    }

    pub fn summary(&self) -> String {
        if let Some(error) = self
            .diagnostics
            .iter()
            .find(|d| d.severity == ParseSeverity::Error)
        {
            format!("line {}: {}", error.line, error.message)
        } else if let Some(warning) = self.diagnostics.first() {
            format!("line {}: {}", warning.line, warning.message)
        } else {
            format!("{} widget edits applied", self.widgets.len())
        }
    }
}

/// One parsed widget extracted from egui_emitter output.
#[derive(Debug, Clone)]
pub struct ParsedWidget {
    pub id: Uuid,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub kind: Option<WidgetKind>,
    pub label: Option<String>,
    pub binding: Option<Option<String>>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl ParsedWidget {
    fn new(id: Uuid) -> Self {
        Self {
            id,
            x: None,
            y: None,
            w: None,
            h: None,
            kind: None,
            label: None,
            binding: None,
            min: None,
            max: None,
        }
    }

    fn has_geometry(&self) -> bool {
        self.x.is_some() || self.y.is_some() || self.w.is_some() || self.h.is_some()
    }

    fn has_any_edit(&self) -> bool {
        self.has_geometry()
            || self.kind.is_some()
            || self.label.is_some()
            || self.binding.is_some()
            || self.min.is_some()
            || self.max.is_some()
    }
}

#[derive(Default)]
struct AreaBuilder {
    widget: Option<ParsedWidget>,
    depth: i32,
}

impl AreaBuilder {
    fn emit(self) -> Option<ParsedWidget> {
        self.widget.filter(ParsedWidget::has_any_edit)
    }
}

pub fn parse_egui_output(code: &str) -> ParseReport {
    let mut report = ParseReport::default();
    let mut builder: Option<AreaBuilder> = None;
    let mut pending_child: Option<ParsedWidget> = None;

    for (idx, line) in code.lines().enumerate() {
        let line_no = idx + 1;
        let t = line.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(id) = extract_widget_uuid(t) {
            if t.starts_with("// widget_") {
                flush_pending_child(&mut pending_child, &mut report);
                pending_child = Some(ParsedWidget::new(id));
                continue;
            }

            if t.starts_with("egui::Area::new(egui::Id::new(") {
                if let Some(done) = builder.take().and_then(AreaBuilder::emit) {
                    report.widgets.push(done);
                }
                builder = Some(AreaBuilder {
                    widget: Some(ParsedWidget::new(id)),
                    depth: 0,
                });
                continue;
            }
        }

        if let Some(child) = pending_child.as_mut() {
            parse_widget_line(child, t, line_no, &mut report);
            flush_pending_child(&mut pending_child, &mut report);
            continue;
        }

        if let Some(b) = builder.as_mut() {
            if t.ends_with('{') && (t.contains(".show(") || t.contains("|ui|")) {
                b.depth += 1;
            }

            if t == "});" {
                if b.depth > 1 {
                    b.depth -= 1;
                } else {
                    if let Some(done) = builder.take().and_then(AreaBuilder::emit) {
                        report.widgets.push(done);
                    }
                    continue;
                }
            }

            if let Some(widget) = b.widget.as_mut() {
                parse_widget_line(widget, t, line_no, &mut report);
            }
        } else if looks_like_widget_edit(t) {
            report.diagnostics.push(ParseDiagnostic {
                severity: ParseSeverity::Warning,
                line: line_no,
                message: "ignored widget code outside a RohKai widget block".to_owned(),
            });
        }
    }

    flush_pending_child(&mut pending_child, &mut report);
    if let Some(done) = builder.take().and_then(AreaBuilder::emit) {
        report.widgets.push(done);
    }

    if report.widgets.is_empty() && code.contains("widget_") {
        report.diagnostics.push(ParseDiagnostic {
            severity: ParseSeverity::Error,
            line: 1,
            message: "no valid widget edits could be parsed".to_owned(),
        });
    }

    report
}

fn flush_pending_child(pending: &mut Option<ParsedWidget>, report: &mut ParseReport) {
    if let Some(widget) = pending.take().filter(ParsedWidget::has_any_edit) {
        report.widgets.push(widget);
    }
}

fn parse_widget_line(
    widget: &mut ParsedWidget,
    line: &str,
    line_no: usize,
    report: &mut ParseReport,
) {
    if line.contains(".fixed_pos(egui::pos2(") {
        match extract_f32_pair(line, "pos2(") {
            Some((x, y)) => {
                widget.x = Some(x);
                widget.y = Some(y);
            }
            None => push_error(report, line_no, "could not parse fixed_pos coordinates"),
        }
    } else if line.contains("ui.set_min_size(egui::vec2(") {
        match extract_f32_pair(line, "vec2(") {
            Some((w, h)) => set_size(widget, w, h, line_no, report),
            None => push_error(report, line_no, "could not parse minimum size"),
        }
    } else if line.contains("egui::Rect::from_min_size(") {
        parse_child_rect(widget, line, line_no, report);
    } else if line.contains("egui::Button::new(") {
        widget.kind = Some(WidgetKind::Button);
        widget.label = extract_string_literal(line);
        parse_add_sized(widget, line, line_no, report);
    } else if line.starts_with("ui.label(") || line.contains("egui::Label::new(") {
        widget.kind = Some(WidgetKind::Label);
        widget.binding = Some(extract_binding_name(line));
        if widget.binding.as_ref().and_then(|b| b.as_ref()).is_none() {
            widget.label = extract_string_literal(line);
        }
    } else if line.contains("egui::TextEdit::singleline(") {
        widget.kind = Some(WidgetKind::TextInput);
        widget.binding = Some(extract_binding_name(line));
        parse_add_sized(widget, line, line_no, report);
    } else if line.contains("egui::Slider::new(") {
        widget.kind = Some(WidgetKind::Slider);
        widget.binding = Some(extract_binding_name(line));
        widget.label = extract_string_literal(line);
        if let Some((min, max)) = extract_range(line) {
            widget.min = Some(min);
            widget.max = Some(max);
        }
        parse_add_sized(widget, line, line_no, report);
    } else if line.contains("egui::Checkbox::new(") {
        widget.kind = Some(WidgetKind::Checkbox);
        widget.binding = Some(extract_binding_name(line));
        widget.label = extract_string_literal(line);
        parse_add_sized(widget, line, line_no, report);
    } else if line.contains("egui::ComboBox::from_label(") {
        widget.kind = Some(WidgetKind::ComboBox);
        widget.label = extract_string_literal(line);
        widget.binding = Some(extract_combo_binding(line));
        if let Some(width) = extract_call_number(line, ".width(") {
            widget.w = Some(width.max(20.0));
        }
    } else if line.contains("ui.radio_value(") {
        widget.kind = Some(WidgetKind::RadioButton);
        widget.binding = Some(extract_binding_name(line));
        widget.label = extract_string_literal(line);
    } else if line.contains("egui::ProgressBar::new(") {
        widget.kind = Some(WidgetKind::ProgressBar);
        widget.label = extract_string_literal(line);
        widget.binding = Some(extract_progress_binding(line));
        parse_add_sized(widget, line, line_no, report);
    } else if line.starts_with("egui::Frame::group(") {
        widget.kind = Some(WidgetKind::Frame);
    } else {
        // Fallback for custom widget template lines.  Kind is intentionally
        // not set so apply_parsed cannot overwrite a Custom kind.  Each field
        // is extracted at most once: the guard prevents later lines in the same
        // block (e.g. a handler call `self.on_click()`) from clobbering the
        // value captured from the constructor line.
        if widget.label.is_none() {
            widget.label = extract_string_literal(line);
        }
        if widget.binding.is_none() {
            if let Some(b) = extract_binding_name(line) {
                widget.binding = Some(Some(b));
            }
        }
    }
}

fn parse_add_sized(
    widget: &mut ParsedWidget,
    line: &str,
    line_no: usize,
    report: &mut ParseReport,
) {
    if line.contains("add_sized([") {
        match extract_bracket_f32_pair(line, "add_sized([") {
            Some((w, h)) => set_size(widget, w, h, line_no, report),
            None => push_error(report, line_no, "could not parse add_sized dimensions"),
        }
    }
}

fn parse_child_rect(
    widget: &mut ParsedWidget,
    line: &str,
    line_no: usize,
    report: &mut ParseReport,
) {
    if let Some((rel_x, rel_y)) = extract_f32_pair(line, "vec2(") {
        widget.x = Some(rel_x);
        widget.y = Some(rel_y);
    } else {
        push_error(report, line_no, "could not parse child widget position");
    }

    if let Some(second_vec2) = nth_after(line, "vec2(", 2) {
        match extract_f32_pair(second_vec2, "") {
            Some((w, h)) => set_size(widget, w, h, line_no, report),
            None => push_error(report, line_no, "could not parse child widget size"),
        }
    }
}

fn set_size(widget: &mut ParsedWidget, w: f32, h: f32, line_no: usize, report: &mut ParseReport) {
    if w <= 0.0 || h <= 0.0 || !w.is_finite() || !h.is_finite() {
        push_error(
            report,
            line_no,
            "widget dimensions must be positive finite numbers",
        );
        return;
    }
    widget.w = Some(w);
    widget.h = Some(h);
}

fn push_error(report: &mut ParseReport, line: usize, message: &str) {
    report.diagnostics.push(ParseDiagnostic {
        severity: ParseSeverity::Error,
        line,
        message: message.to_owned(),
    });
}

fn looks_like_widget_edit(line: &str) -> bool {
    line.contains("egui::Button::new(")
        || line.contains("egui::TextEdit::singleline(")
        || line.contains("egui::Slider::new(")
        || line.contains("egui::Checkbox::new(")
        || line.contains("egui::ComboBox::from_label(")
        || line.contains("ui.radio_value(")
        || line.contains("egui::ProgressBar::new(")
}

fn extract_widget_uuid(line: &str) -> Option<Uuid> {
    let marker = "widget_";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
        .unwrap_or(rest.len());
    Uuid::parse_str(&rest[..end]).ok()
}

fn extract_f32_pair(line: &str, prefix: &str) -> Option<(f32, f32)> {
    let rest = if prefix.is_empty() {
        line
    } else {
        &line[line.find(prefix)? + prefix.len()..]
    };
    let close = rest.find(')')?;
    let content = &rest[..close];
    let mut parts = content.splitn(2, ',');
    let a: f32 = parts.next()?.trim().parse().ok()?;
    let b: f32 = parts.next()?.trim().parse().ok()?;
    Some((a, b))
}

fn extract_bracket_f32_pair(line: &str, prefix: &str) -> Option<(f32, f32)> {
    let rest = &line[line.find(prefix)? + prefix.len()..];
    let close = rest.find(']')?;
    let content = &rest[..close];
    let mut parts = content.splitn(2, ',');
    let a: f32 = parts.next()?.trim().parse().ok()?;
    let b: f32 = parts.next()?.trim().parse().ok()?;
    Some((a, b))
}

fn extract_string_literal(line: &str) -> Option<String> {
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut out = String::new();
        let mut escaped = false;
        for ch in chars.by_ref() {
            if escaped {
                match ch {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    other => out.push(other),
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Some(out);
            } else {
                out.push(ch);
            }
        }
    }
    None
}

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

fn extract_combo_binding(line: &str) -> Option<String> {
    let pos = line.find("selected_text(self.")?;
    let rest = &line[pos + "selected_text(self.".len()..];
    let end = rest.find('.').unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

fn extract_progress_binding(line: &str) -> Option<String> {
    let pos = line.find("ProgressBar::new(self.")?;
    let rest = &line[pos + "ProgressBar::new(self.".len()..];
    let end = rest.find(')').unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

fn extract_range(line: &str) -> Option<(f32, f32)> {
    let sep = line.find("..=")?;
    let before = &line[..sep];
    let min_start = before.rfind([',', '(']).map(|p| p + 1).unwrap_or(0);
    let min_str = before[min_start..].trim();
    let after = &line[sep + 3..];
    let max_end = after.find([')', ' ', ',']).unwrap_or(after.len());
    let max_str = after[..max_end].trim();
    Some((min_str.parse().ok()?, max_str.parse().ok()?))
}

fn extract_call_number(line: &str, prefix: &str) -> Option<f32> {
    let rest = &line[line.find(prefix)? + prefix.len()..];
    let end = rest.find(')')?;
    rest[..end].trim().parse().ok()
}

fn nth_after<'a>(line: &'a str, pat: &str, n: usize) -> Option<&'a str> {
    let mut count = 0;
    let mut search_start = 0;
    while let Some(pos) = line[search_start..].find(pat) {
        count += 1;
        let abs = search_start + pos + pat.len();
        if count == n {
            return Some(&line[abs..]);
        }
        search_start = abs;
    }
    None
}

pub fn apply_parsed(tree: &mut UiTree, widgets: &[ParsedWidget]) {
    for pw in widgets {
        if let Some(w) = tree.widgets.iter_mut().find(|w| w.id == pw.id) {
            if let Some(x) = pw.x {
                w.rect.x = x;
            }
            if let Some(y) = pw.y {
                w.rect.y = y;
            }
            if let Some(width) = pw.w {
                w.rect.w = width;
            }
            if let Some(height) = pw.h {
                w.rect.h = height;
            }
            if let Some(kind) = &pw.kind {
                // Never overwrite a Custom kind with a parser-inferred built-in.
                // Descriptor templates may contain egui-like patterns that would
                // otherwise be misidentified as a built-in widget kind.
                if !matches!(w.kind, WidgetKind::Custom(_)) {
                    w.kind = kind.clone();
                }
            }
            if let Some(label) = &pw.label {
                w.props.label = label.clone();
            }
            if let Some(binding) = &pw.binding {
                w.state_binding = binding.clone();
            }
            if let Some(min) = pw.min {
                w.props.min = min;
            }
            if let Some(max) = pw.max {
                w.props.max = max;
            }
        }
    }
    tree.validate_and_repair();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::egui_emitter;
    use crate::project::schema::{Rect, WidgetInstance, WidgetProps};

    fn widget(kind: WidgetKind, label: &str) -> WidgetInstance {
        WidgetInstance {
            id: Uuid::new_v4(),
            kind,
            rect: Rect {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 30.0,
            },
            props: WidgetProps {
                label: label.to_owned(),
                ..Default::default()
            },
            state_binding: Some("value".to_owned()),
            ..Default::default()
        }
    }

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
        apply_parsed(&mut tree, &report.widgets);

        let edited = tree.widgets.iter().find(|w| w.id == id).unwrap();
        assert_eq!(edited.rect.x, 42.0);
        assert_eq!(edited.rect.y, 56.0);
    }

    #[test]
    fn reports_invalid_dimensions_without_applying() {
        let id = Uuid::new_v4();
        let code = format!(
            "egui::Area::new(egui::Id::new(\"widget_{id}\"))\n    .fixed_pos(egui::pos2(1.0, 2.0))\n    .show(ctx, |ui| {{\n        ui.set_min_size(egui::vec2(-1.0, 20.0));\n    }});"
        );

        let report = parse_egui_output(&code);
        assert!(report.has_errors());
        assert!(report.summary().contains("dimensions"));
    }

    #[test]
    fn custom_widget_label_and_binding_extracted_from_template_line() {
        // A Custom widget whose template expansion contains a string literal
        // and a &mut self.field reference.  Parser must extract both without
        // inferring a built-in kind (so apply_parsed cannot overwrite Custom).
        let id = Uuid::new_v4();
        let code = format!(
            concat!(
                "egui::Area::new(egui::Id::new(\"widget_{id}\"))\n",
                "    .fixed_pos(egui::pos2(10.0, 20.0))\n",
                "    .show(ctx, |ui| {{\n",
                "        ui.set_min_size(egui::vec2(120.0, 36.0));\n",
                "        MyWidget::new(\"Hello World\", &mut self.counter);\n",
                "    }});\n"
            ),
            id = id
        );
        let report = parse_egui_output(&code);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        let pw = report.widgets.iter().find(|w| w.id == id);
        assert!(pw.is_some(), "widget not found in parse output");
        let pw = pw.unwrap();
        assert_eq!(pw.kind, None, "Custom kind must not be inferred");
        assert_eq!(pw.label.as_deref(), Some("Hello World"));
        assert_eq!(
            pw.binding
                .as_ref()
                .and_then(|b| b.as_ref())
                .map(|s| s.as_str()),
            Some("counter")
        );
    }

    #[test]
    fn custom_widget_first_line_wins_over_later_handler_call() {
        // Handler call (`self.on_click()`) appears after the constructor line.
        // The binding extracted from the constructor must not be overwritten.
        let id = Uuid::new_v4();
        let code = format!(
            concat!(
                "egui::Area::new(egui::Id::new(\"widget_{id}\"))\n",
                "    .fixed_pos(egui::pos2(0.0, 0.0))\n",
                "    .show(ctx, |ui| {{\n",
                "        ui.set_min_size(egui::vec2(100.0, 30.0));\n",
                "        let resp = MyWidget::new(\"Btn\", &mut self.flag);\n",
                "        if resp.clicked() {{ self.on_pressed(); }}\n",
                "    }});\n"
            ),
            id = id
        );
        let report = parse_egui_output(&code);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        let pw = report.widgets.iter().find(|w| w.id == id).unwrap();
        assert_eq!(pw.label.as_deref(), Some("Btn"));
        assert_eq!(
            pw.binding
                .as_ref()
                .and_then(|b| b.as_ref())
                .map(|s| s.as_str()),
            Some("flag"),
            "binding must come from constructor, not handler call"
        );
    }

    #[test]
    fn parses_child_widget_marker_comments() {
        let mut parent = widget(WidgetKind::Frame, "Group");
        parent.state_binding = None;
        let child = widget(WidgetKind::Label, "Child");
        let child_id = child.id;
        parent.children.push(child_id);
        let mut tree = UiTree::default();
        tree.add(parent);
        tree.add(child);

        let code = egui_emitter::emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");
        let report = parse_egui_output(&code);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        assert!(report.widgets.iter().any(|w| w.id == child_id));
    }
}
