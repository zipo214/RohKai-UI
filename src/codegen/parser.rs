use crate::codegen::source_map::{line_byte_spans, SourceSpan, WidgetSourceSpan};
use crate::project::schema::WidgetKind;
use crate::project::ui_tree::UiTree;
use std::collections::HashSet;
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

    pub fn widget_spans(&self) -> Vec<WidgetSourceSpan> {
        self.widgets
            .iter()
            .filter_map(|widget| {
                widget.source_span.clone().map(|span| WidgetSourceSpan {
                    widget_id: widget.id,
                    span,
                })
            })
            .collect()
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
    pub children: Vec<Uuid>,
    pub source_span: Option<SourceSpan>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub created_widgets: usize,
    pub created_widget_ids: Vec<Uuid>,
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
            children: Vec::new(),
            source_span: None,
        }
    }

    fn touch_source(&mut self, byte_start: usize, byte_end: usize, line: usize) {
        if let Some(span) = &mut self.source_span {
            span.extend_to(byte_end, line);
        } else {
            self.source_span = Some(SourceSpan::new(byte_start..byte_end, line..=line));
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
            || !self.children.is_empty()
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

    for (idx, byte_span) in line_byte_spans(code).into_iter().enumerate() {
        let line_no = idx + 1;
        let line = &code[byte_span.clone()];
        let t = line.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(id) = extract_widget_uuid(t) {
            if t.starts_with("// widget_") {
                if let Some(parent) = builder.as_mut().and_then(|b| b.widget.as_mut()) {
                    if !parent.children.contains(&id) {
                        parent.children.push(id);
                    }
                    parent.touch_source(byte_span.start, byte_span.end, line_no);
                }
                flush_pending_child(&mut pending_child, &mut report);
                let mut child = ParsedWidget::new(id);
                child.touch_source(byte_span.start, byte_span.end, line_no);
                pending_child = Some(child);
                continue;
            }

            if t.starts_with("egui::Area::new(egui::Id::new(") {
                flush_pending_child(&mut pending_child, &mut report);
                if builder.is_some() {
                    report.diagnostics.push(ParseDiagnostic {
                        severity: ParseSeverity::Error,
                        line: line_no,
                        message: "previous widget block was not closed".to_owned(),
                    });
                }
                if let Some(done) = builder.take().and_then(AreaBuilder::emit) {
                    report.widgets.push(done);
                }
                let mut widget = ParsedWidget::new(id);
                widget.touch_source(byte_span.start, byte_span.end, line_no);
                builder = Some(AreaBuilder {
                    widget: Some(widget),
                    depth: 0,
                });
                continue;
            }
        }

        if let Some(b) = builder.as_mut() {
            if let Some(widget) = b.widget.as_mut() {
                widget.touch_source(byte_span.start, byte_span.end, line_no);
            }

            if t.ends_with('{') && (t.contains(".show(") || t.contains("|ui|")) {
                b.depth += 1;
            }

            if t == "});" {
                if b.depth > 1 {
                    b.depth -= 1;
                } else {
                    flush_pending_child(&mut pending_child, &mut report);
                    if let Some(done) = builder.take().and_then(AreaBuilder::emit) {
                        report.widgets.push(done);
                    }
                    continue;
                }
            }

            if let Some(child) = pending_child.as_mut() {
                child.touch_source(byte_span.start, byte_span.end, line_no);
                parse_widget_line(child, t, line_no, &mut report);
            } else if let Some(widget) = b.widget.as_mut() {
                parse_widget_line(widget, t, line_no, &mut report);
            }
        } else if looks_like_widget_edit(t) {
            let mut orphan = ParsedWidget::new(Uuid::new_v4());
            orphan.touch_source(byte_span.start, byte_span.end, line_no);
            parse_widget_line(&mut orphan, t, line_no, &mut report);
            if orphan.has_any_edit() {
                report.widgets.push(orphan);
            } else {
                report.diagnostics.push(ParseDiagnostic {
                    severity: ParseSeverity::Warning,
                    line: line_no,
                    message: "ignored widget code outside a RohKai widget block".to_owned(),
                });
            }
        }
    }

    flush_pending_child(&mut pending_child, &mut report);
    if builder.is_some() {
        report.diagnostics.push(ParseDiagnostic {
            severity: ParseSeverity::Error,
            line: code.lines().count().max(1),
            message: "widget block is missing its closing `});`".to_owned(),
        });
    }
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

/// Assign `widget.binding` from a parsed line, intercepting the malformed-binding
/// sentinel returned by `extract_binding_name`.  When malformed, emits a diagnostic
/// and leaves `widget.binding` unchanged so no prior valid binding is clobbered.
fn apply_binding(widget: &mut ParsedWidget, line: &str, line_no: usize, report: &mut ParseReport) {
    match extract_binding_name(line) {
        Some(b) if b == "_malformed_binding_" => {
            push_error(
                report,
                line_no,
                "malformed binding: expected identifier after 'self.'",
            );
        }
        b => {
            widget.binding = Some(b);
        }
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
        apply_binding(widget, line, line_no, report);
        if widget.binding.as_ref().and_then(|b| b.as_ref()).is_none() {
            widget.label = extract_string_literal(line);
        }
    } else if line.contains("egui::TextEdit::singleline(") {
        widget.kind = Some(WidgetKind::TextInput);
        apply_binding(widget, line, line_no, report);
        parse_add_sized(widget, line, line_no, report);
    } else if line.contains("egui::Slider::new(") {
        widget.kind = Some(WidgetKind::Slider);
        apply_binding(widget, line, line_no, report);
        widget.label = extract_string_literal(line);
        if let Some((min, max)) = extract_range(line) {
            widget.min = Some(min);
            widget.max = Some(max);
        }
        parse_add_sized(widget, line, line_no, report);
    } else if line.contains("egui::Checkbox::new(") {
        widget.kind = Some(WidgetKind::Checkbox);
        apply_binding(widget, line, line_no, report);
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
        apply_binding(widget, line, line_no, report);
        widget.label = extract_string_literal(line);
    } else if line.contains("egui::ProgressBar::new(") {
        widget.kind = Some(WidgetKind::ProgressBar);
        widget.label = extract_string_literal(line);
        widget.binding = Some(extract_progress_binding(line));
        parse_add_sized(widget, line, line_no, report);
    } else if line.starts_with("egui::Frame::group(") {
        widget.kind = Some(WidgetKind::Frame);
    } else if line.starts_with("ui.vertical(|ui|") {
        widget.kind = Some(WidgetKind::VLayout);
    } else if line.starts_with("ui.horizontal(|ui|") {
        widget.kind = Some(WidgetKind::HLayout);
    } else if line.starts_with("egui::Grid::new(") {
        widget.kind = Some(WidgetKind::GridLayout);
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
            match extract_binding_name(line) {
                Some(b) if b == "_malformed_binding_" => {
                    push_error(
                        report,
                        line_no,
                        "malformed binding: expected identifier after 'self.'",
                    );
                }
                Some(b) => {
                    widget.binding = Some(Some(b));
                }
                None => {}
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
                let name = rest[..end].to_owned();
                if crate::codegen::rust::is_valid_identifier(&name) {
                    return Some(name);
                }
                // Invalid (keyword, leading digit, etc.): drop rather than
                // salvage — a prefixed name could collide with an existing
                // binding and the canvas UI already blocks invalid input at
                // edit time, so rejection is the safer default.
                return None;
            } else if !rest.is_empty() && (rest.starts_with(')') || rest.starts_with(',')) {
                // Matched "self." but nothing follows — malformed; signal caller
                return Some("_malformed_binding_".to_owned());
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

pub fn apply_parsed(tree: &mut UiTree, widgets: &[ParsedWidget]) -> ApplyOutcome {
    let mut outcome = ApplyOutcome::default();
    let mut seen = HashSet::new();
    for pw in widgets {
        let duplicate_block = !seen.insert(pw.id);
        if !duplicate_block {
            if let Some(w) = tree.get_mut(pw.id) {
                apply_fields(w, pw, false);
                continue;
            }
        }

        if let Some(mut widget) = tree.widgets.iter().find(|w| w.id == pw.id).cloned() {
            widget.id = Uuid::new_v4();
            widget.children.clear();
            apply_fields(&mut widget, pw, true);
            let id = widget.id;
            tree.add(widget);
            outcome.created_widgets += 1;
            outcome.created_widget_ids.push(id);
        } else if let Some(mut widget) = instance_from_parsed(pw) {
            apply_fields(&mut widget, pw, false);
            let id = widget.id;
            tree.add(widget);
            outcome.created_widgets += 1;
            outcome.created_widget_ids.push(id);
        }
    }
    tree.validate_and_repair();
    outcome
}

fn apply_fields(
    w: &mut crate::project::schema::WidgetInstance,
    pw: &ParsedWidget,
    offset_duplicate: bool,
) {
    if let Some(x) = pw.x {
        w.rect.x = x;
    }
    if let Some(y) = pw.y {
        w.rect.y = y;
    }
    if offset_duplicate && (pw.x.is_some() || pw.y.is_some()) {
        w.rect.x += 16.0;
        w.rect.y += 16.0;
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
    if !offset_duplicate && !pw.children.is_empty() {
        w.children = pw.children.clone();
    }
}

fn instance_from_parsed(pw: &ParsedWidget) -> Option<crate::project::schema::WidgetInstance> {
    let kind = pw.kind.clone()?;
    Some(crate::project::schema::WidgetInstance {
        id: pw.id,
        kind,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::egui_emitter;
    use crate::project::schema::{Rect, WidgetInstance, WidgetProps};
    use crate::widgets;

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
        let parsed = report
            .widgets
            .iter()
            .find(|w| w.id == id)
            .expect("widget must be in parse report after successful parse");
        let span = parsed.source_span.as_ref().expect("source span");
        assert!(code[span.bytes.clone()].starts_with("egui::Area::new"));
        assert!(!code[span.bytes.clone()].contains("CentralPanel"));
        apply_parsed(&mut tree, &report.widgets);

        let edited = tree
            .widgets
            .iter()
            .find(|w| w.id == id)
            .expect("widget must remain in tree after apply_parsed");
        assert_eq!(edited.rect.x, 42.0);
        assert_eq!(edited.rect.y, 56.0);
    }

    #[test]
    fn every_generated_builtin_block_is_a_complete_parse() {
        for kind in widgets::ALL_KINDS {
            let mut tree = UiTree::default();
            tree.add(widget(kind.clone(), &format!("{kind:?}")));
            let document = egui_emitter::emit_document(&tree);
            let report = parse_egui_output(&document.text);

            assert!(
                !report.has_errors(),
                "{kind:?} canonical output must parse without structural errors: {:?}\n{}",
                report.diagnostics,
                document.text
            );
        }
    }

    #[test]
    fn duplicate_pasted_widget_block_creates_new_instance_with_fresh_id() {
        let button = widget(WidgetKind::Button, "Go");
        let id = button.id;
        let mut tree = UiTree::default();
        tree.add(button);
        let block = egui_emitter::emit_indexed(&tree)
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");
        let code = format!("{block}\n{block}");

        let report = parse_egui_output(&code);
        assert_eq!(report.widgets.iter().filter(|w| w.id == id).count(), 2);
        let outcome = apply_parsed(&mut tree, &report.widgets);

        assert_eq!(outcome.created_widgets, 1);
        assert_eq!(outcome.created_widget_ids.len(), 1);
        assert_ne!(outcome.created_widget_ids[0], id);
        assert_eq!(tree.widgets.len(), 2);
        assert_eq!(tree.widgets.iter().filter(|w| w.id == id).count(), 1);
        assert!(tree.widgets.iter().any(|w| w.id != id));
    }

    #[test]
    fn pasted_orphan_button_line_creates_widget() {
        let code = r#"ui.add_sized([100.0, 30.0], egui::Button::new("Paste Me"));"#;
        let report = parse_egui_output(code);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        assert_eq!(report.widgets.len(), 1);

        let mut tree = UiTree::default();
        let outcome = apply_parsed(&mut tree, &report.widgets);

        assert_eq!(outcome.created_widgets, 1);
        assert_eq!(outcome.created_widget_ids, vec![report.widgets[0].id]);
        assert_eq!(tree.widgets.len(), 1);
        assert_eq!(tree.widgets[0].kind, WidgetKind::Button);
        assert_eq!(tree.widgets[0].props.label, "Paste Me");
        assert_eq!(tree.widgets[0].rect.w, 100.0);
        assert_eq!(tree.widgets[0].rect.h, 30.0);
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
    fn reports_unclosed_widget_block_as_incomplete_parse() {
        let id = Uuid::new_v4();
        let code = format!(
            "egui::Area::new(egui::Id::new(\"widget_{id}\"))\n    .show(ctx, |ui| {{\n        ui.button(\"Broken\");"
        );

        let report = parse_egui_output(&code);
        assert!(report.has_errors());
        assert!(report.summary().contains("closing"));
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
        let pw = report
            .widgets
            .iter()
            .find(|w| w.id == id)
            .expect("widget must be in parse output");
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
        let pw = report
            .widgets
            .iter()
            .find(|w| w.id == id)
            .expect("widget must be in parse report");
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

    #[test]
    fn parses_layout_owned_hierarchy_from_generated_code() {
        for kind in [
            WidgetKind::VLayout,
            WidgetKind::HLayout,
            WidgetKind::GridLayout,
        ] {
            let mut parent = widget(kind.clone(), "Layout");
            parent.state_binding = None;
            let parent_id = parent.id;
            let mut child = widget(WidgetKind::Button, "Child");
            child.state_binding = None;
            let child_id = child.id;
            parent.children.push(child_id);
            let source_tree = UiTree {
                widgets: vec![parent, child.clone()],
                ..Default::default()
            };
            let code = egui_emitter::emit_indexed(&source_tree)
                .into_iter()
                .map(|(_, line)| line)
                .collect::<Vec<_>>()
                .join("\n");
            let report = parse_egui_output(&code);
            assert!(
                !report.has_errors(),
                "{kind:?} diagnostics: {:?}",
                report.diagnostics
            );
            let parsed_parent = report.widgets.iter().find(|w| w.id == parent_id).unwrap();
            assert_eq!(parsed_parent.kind.as_ref(), Some(&kind));
            assert_eq!(parsed_parent.children, vec![child_id]);

            let mut target_tree = UiTree {
                widgets: vec![
                    WidgetInstance {
                        id: parent_id,
                        kind,
                        children: Vec::new(),
                        ..Default::default()
                    },
                    child,
                ],
                ..Default::default()
            };
            apply_parsed(&mut target_tree, &report.widgets);
            let restored = target_tree
                .widgets
                .iter()
                .find(|w| w.id == parent_id)
                .unwrap();
            assert_eq!(restored.children, vec![child_id]);
        }
    }

    #[test]
    fn leading_digit_binding_is_rejected_not_salvaged() {
        // "self.1abc" starts with a digit; salvage (prefix "_") is rejected
        // because the resulting "_1abc" could collide with a real binding.
        let code =
            "            ui.add_sized([100.0, 30.0], egui::TextEdit::singleline(&mut self.1abc))";
        let report = parse_egui_output(code);
        if let Some(w) = report.widgets.first() {
            let extracted = w.binding.as_ref().and_then(|b| b.as_deref());
            assert_ne!(
                extracted,
                Some("_1abc"),
                "leading-digit binding must not be salvaged"
            );
            // Binding must be absent (None) or not start with "_1"
            assert!(
                extracted.is_none_or(|b| !b.starts_with("_1")),
                "salvaged _1... name must not appear"
            );
        }
    }

    #[test]
    fn malformed_binding_emits_error_not_sentinel() {
        // "self." with nothing after it (followed by ')') must produce a diagnostic
        // and must NOT store the "_malformed_binding_" sentinel in widget.binding.
        let code =
            "            ui.add_sized([100.0, 30.0], egui::TextEdit::singleline(&mut self.))";
        let report = parse_egui_output(code);
        assert!(
            report.has_errors(),
            "malformed 'self.' must emit a parse error; diagnostics: {:?}",
            report.diagnostics
        );
        if let Some(w) = report.widgets.first() {
            assert_ne!(
                w.binding.as_ref().and_then(|b| b.as_deref()),
                Some("_malformed_binding_"),
                "sentinel must not leak into widget.binding"
            );
        }
    }
}
