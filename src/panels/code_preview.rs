use crate::codegen::source_map::{GeneratedCodeDocument, SourceSpan, WidgetSourceSpan};
use crate::codegen::{egui_emitter, parser, state_emitter};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

const CODE_EDITOR_GUTTER_X: f32 = 10.0;
const CODE_EDITOR_GUTTER_Y: f32 = 8.0;
const CODE_HIGHLIGHT_PADDING: f32 = 4.0;
const CODE_HIGHLIGHT_STROKE_WIDTH: f32 = 1.25;

#[derive(Default, PartialEq)]
pub enum CodeStatus {
    #[default]
    Generated,
    ValidEdit,
    InvalidEdit(String),
}

pub struct CodePreviewArgs<'a> {
    pub selected_ids: &'a mut Vec<Uuid>,
    pub navigation_target: &'a mut Option<Uuid>,
    /// Tracé: if Some(name), insert handler stub and clear after consuming.
    pub scroll_to_handler: &'a mut Option<String>,
    pub code_buffer: &'a mut String,
    pub code_status: &'a mut CodeStatus,
    pub last_generated: &'a mut String,
    pub split_ratio: &'a mut f32,
    pub wrap_code: &'a mut bool,
    pub editor_has_focus: &'a mut bool,
    pub code_font_size: f32,
    /// Ctrl+F search query for in-panel find (empty = no active search).
    pub search_query: &'a mut String,
    /// Whether the search bar is currently shown.
    pub search_open: &'a mut bool,
    /// Current match index for Prev/Next navigation.
    pub search_match_idx: &'a mut usize,
}

struct CodeEditorSurface<'a> {
    text: &'a mut String,
    font_size: f32,
    height: f32,
    wrap: bool,
    highlights: &'a [SourceSpan],
    navigation: Option<&'a SourceSpan>,
}

struct CodeEditorSurfaceOutput {
    response: egui::Response,
    outline_rects: Vec<egui::Rect>,
}

impl CodeEditorSurface<'_> {
    fn show(self, ui: &mut egui::Ui) -> CodeEditorSurfaceOutput {
        let outer_size = egui::vec2(ui.available_width().max(80.0), self.height.max(60.0));
        let (outer_rect, _) = ui.allocate_exact_size(outer_size, egui::Sense::hover());
        let inner_rect = outer_rect.shrink2(egui::vec2(CODE_EDITOR_GUTTER_X, CODE_EDITOR_GUTTER_Y));
        let parent_clip = ui.clip_rect();
        let decoration_clip = outer_rect.intersect(parent_clip);

        let mut editor_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        editor_ui.set_clip_rect(inner_rect.intersect(parent_clip));

        let scroll_output = egui::ScrollArea::new([!self.wrap, true])
            .id_salt("lazare_code_editor_scroll")
            .max_width(inner_rect.width())
            .max_height(inner_rect.height())
            .min_scrolled_width(inner_rect.width())
            .min_scrolled_height(inner_rect.height())
            .auto_shrink([false, false])
            .show(&mut editor_ui, |ui| {
                let wrap_width = inner_rect.width().max(24.0);
                let mut layouter = |ui: &egui::Ui, text: &str, requested_width: f32| {
                    let width = if self.wrap {
                        requested_width.min(wrap_width)
                    } else {
                        f32::INFINITY
                    };
                    let job = code_layout_job(ui, text, self.font_size, width);
                    ui.fonts(|fonts| fonts.layout_job(job))
                };

                let output = egui::TextEdit::multiline(self.text)
                    .font(egui::FontId::monospace(self.font_size))
                    .desired_width(wrap_width)
                    .desired_rows(code_rows_for_height(inner_rect.height(), self.font_size))
                    .min_size(egui::vec2(wrap_width, inner_rect.height()))
                    .code_editor()
                    .layouter(&mut layouter)
                    .frame(false)
                    .show(ui);

                let block_rects: Vec<egui::Rect> = self
                    .highlights
                    .iter()
                    .filter_map(|span| source_span_rect(&output, self.text, span))
                    .collect();

                if let Some(navigation) = self.navigation {
                    if let Some(rect) = source_span_rect(&output, self.text, navigation) {
                        let viewport = ui.clip_rect();
                        let align = if rect.width() <= viewport.width()
                            && rect.height() <= viewport.height()
                        {
                            egui::Align::Center
                        } else {
                            egui::Align::Min
                        };
                        ui.scroll_to_rect(rect.expand(2.0), Some(align));
                    }
                }

                (output.response.clone(), block_rects)
            });

        let stroke = egui::Stroke::new(
            CODE_HIGHLIGHT_STROKE_WIDTH,
            egui::Color32::from_rgb(52, 211, 153),
        );
        let painter = ui.painter_at(decoration_clip);
        let outline_rects: Vec<egui::Rect> = scroll_output
            .inner
            .1
            .into_iter()
            .filter_map(|rect| highlight_outline_rect(rect, inner_rect, decoration_clip))
            .collect();
        for rect in &outline_rects {
            painter.rect_stroke(*rect, 3.0, stroke);
        }

        CodeEditorSurfaceOutput {
            response: scroll_output.inner.0,
            outline_rects,
        }
    }
}

fn code_layout_job(
    ui: &egui::Ui,
    text: &str,
    font_size: f32,
    wrap_width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::monospace(font_size),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    job
}

fn byte_to_char_index(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

fn source_span_rect(
    output: &egui::widgets::text_edit::TextEditOutput,
    text: &str,
    span: &SourceSpan,
) -> Option<egui::Rect> {
    if span.bytes.start >= span.bytes.end || span.bytes.end > text.len() {
        return None;
    }

    let char_range =
        byte_to_char_index(text, span.bytes.start)..byte_to_char_index(text, span.bytes.end);
    let mut row_start = 0usize;
    let mut block_rect: Option<egui::Rect> = None;

    for row in &output.galley.rows {
        let row_char_count = row.char_count_excluding_newline();
        let row_end = row_start + row.char_count_including_newline();
        if char_range.start < row_end && char_range.end > row_start {
            let local_start = char_range
                .start
                .saturating_sub(row_start)
                .min(row_char_count);
            let local_end = char_range.end.saturating_sub(row_start).min(row_char_count);
            let left = row.x_offset(local_start);
            let right = row.x_offset(local_end.max(local_start));
            let row_rect = egui::Rect::from_min_max(
                egui::pos2(left.min(right), row.rect.top()),
                egui::pos2(left.max(right), row.rect.bottom()),
            )
            .translate(output.galley_pos.to_vec2());
            block_rect = Some(match block_rect {
                Some(existing) => existing.union(row_rect),
                None => row_rect,
            });
        }
        row_start = row_end;
    }

    block_rect.filter(egui::Rect::is_positive)
}

fn highlight_outline_rect(
    block_rect: egui::Rect,
    text_viewport: egui::Rect,
    decoration_clip: egui::Rect,
) -> Option<egui::Rect> {
    let visible_text = block_rect.intersect(text_viewport);
    if !visible_text.is_positive() {
        return None;
    }

    let stroke_inset = CODE_HIGHLIGHT_STROKE_WIDTH + 1.0;
    let safe_clip = decoration_clip.shrink(stroke_inset);
    let outlined = visible_text
        .expand(CODE_HIGHLIGHT_PADDING)
        .intersect(safe_clip);
    outlined.is_positive().then_some(outlined)
}

fn selected_widget_spans(all_spans: &[WidgetSourceSpan], selected_ids: &[Uuid]) -> Vec<SourceSpan> {
    selected_ids
        .iter()
        .flat_map(|selected| {
            all_spans
                .iter()
                .filter(move |entry| entry.widget_id == *selected)
                .map(|entry| entry.span.clone())
        })
        .collect()
}

fn source_spans_for_buffer(code: &str, generated: &GeneratedCodeDocument) -> Vec<WidgetSourceSpan> {
    if code == generated.text {
        generated.widget_spans.clone()
    } else {
        parser::parse_egui_output(code).widget_spans()
    }
}

fn handler_source_span(code: &str, handler_name: &str) -> Option<SourceSpan> {
    let needle = format!("fn {handler_name}(");
    let start = code.find(&needle)?;
    let line_start = code[..start].bytes().filter(|byte| *byte == b'\n').count() + 1;
    let end = code[start..]
        .find("\n}")
        .map(|offset| start + offset + 2)
        .unwrap_or(code.len());
    let line_end = code[..end].bytes().filter(|byte| *byte == b'\n').count() + 1;
    Some(SourceSpan::new(start..end, line_start..=line_end))
}

/// Collect all `fn name(` user-defined handler names from the code buffer.
/// Skips `fn update(` and `fn new(` which are framework methods, not handlers.
fn collect_handler_names(code: &str) -> Vec<String> {
    let skip = ["fn update(", "fn new(", "fn default("];
    code.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name: String =
                    rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !name.is_empty() && !skip.iter().any(|s| trimmed.starts_with(s)) {
                    return Some(name);
                }
            }
            None
        })
        .collect()
}

/// Compute `SourceSpan`s for all occurrences of `query` (lowercase) in `code`.
/// Returns an empty vec when query is empty.
fn compute_search_spans(code: &str, query: &str) -> Vec<SourceSpan> {
    if query.is_empty() {
        return Vec::new();
    }
    let code_lower = code.to_lowercase();
    let mut spans = Vec::new();
    let mut search_from = 0usize;
    while let Some(byte_offset) = code_lower[search_from..].find(query) {
        let abs_byte = search_from + byte_offset;
        let line_num = code[..abs_byte].chars().filter(|&c| c == '\n').count() + 1;
        spans.push(SourceSpan::new(
            abs_byte..(abs_byte + query.len()),
            line_num..=line_num,
        ));
        search_from = abs_byte + query.len().max(1);
    }
    spans
}

fn navigation_span(target: Option<Uuid>, spans: &[WidgetSourceSpan]) -> Option<SourceSpan> {
    target.and_then(|id| {
        spans
            .iter()
            .find(|entry| entry.widget_id == id)
            .map(|entry| entry.span.clone())
    })
}

fn first_selected_line(spans: &[SourceSpan]) -> Option<usize> {
    spans.iter().map(|span| *span.lines.start()).min()
}

fn code_rows_for_height(height: f32, font_size: f32) -> usize {
    ((height / (font_size * 1.35)).floor() as usize).max(4)
}

fn canonical_widget_free_code(tree: &UiTree) -> String {
    let mut widget_free = tree.clone();
    widget_free.clear_widgets();
    egui_emitter::emit_document(&widget_free).text
}

fn is_canonical_widget_free_edit(code: &str, tree: &UiTree, report: &parser::ParseReport) -> bool {
    !report.has_errors()
        && report.widgets.is_empty()
        && code.trim() == canonical_widget_free_code(tree).trim()
}

pub fn show(ctx: &egui::Context, tree: &mut UiTree, args: CodePreviewArgs<'_>) {
    let CodePreviewArgs {
        selected_ids,
        navigation_target,
        scroll_to_handler,
        code_buffer,
        code_status,
        last_generated,
        split_ratio,
        wrap_code,
        editor_has_focus,
        code_font_size,
        search_query,
        search_open,
        search_match_idx,
    } = args;

    let generated = egui_emitter::emit_document(tree);

    // External tree changes resync the editor only when the user is not
    // actively typing. This preserves cursor/clipboard edits while keeping
    // UiTree authoritative once focus leaves the editor.
    if generated.text != *last_generated
        && !*editor_has_focus
        && !matches!(code_status, CodeStatus::InvalidEdit(_))
    {
        *last_generated = generated.text.clone();
        *code_buffer = generated.text.clone();
        *code_status = CodeStatus::Generated;
    }
    let tree_changed_while_invalid =
        generated.text != *last_generated && matches!(code_status, CodeStatus::InvalidEdit(_));

    let mut handler_navigation = None;
    if let Some(handler_name) = scroll_to_handler.take() {
        let needle = format!("fn {handler_name}(");
        if !code_buffer.contains(&needle) {
            let stub = format!("\nfn {handler_name}(&mut self) {{\n    // TODO: implement\n}}\n");
            code_buffer.push_str(&stub);
            *code_status = CodeStatus::ValidEdit;
        }
        handler_navigation = handler_source_span(code_buffer, &handler_name);
    }
    let requested_navigation = navigation_target.take();

    egui::SidePanel::right("code_output")
        .min_width(220.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Generated Code");
            ui.separator();

            // ---- status row ----
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("egui output").strong());

                let (dot_color, status_text) = match code_status {
                    CodeStatus::Generated => (egui::Color32::from_rgb(52, 211, 153), "generated"),
                    CodeStatus::ValidEdit => (egui::Color32::from_rgb(96, 165, 250), "valid edit"),
                    CodeStatus::InvalidEdit(_) => (egui::Color32::RED, "invalid edit"),
                };
                ui.label(egui::RichText::new("●").color(dot_color).small());
                ui.label(egui::RichText::new(status_text).small().color(dot_color));

                ui.toggle_value(wrap_code, "Wrap")
                    .on_hover_text("Wrap long code lines instead of horizontal scrolling");

                let search_btn = ui
                    .selectable_label(*search_open, egui::RichText::new("⌕").small())
                    .on_hover_text("Find in code (Ctrl+F)");
                if search_btn.clicked() {
                    *search_open = !*search_open;
                    if !*search_open {
                        search_query.clear();
                    }
                }

                if ui
                    .small_button("↺")
                    .on_hover_text("Reset to generated code")
                    .clicked()
                {
                    *code_buffer = generated.text.clone();
                    *last_generated = generated.text.clone();
                    *code_status = CodeStatus::Generated;
                }
            });

            // Ctrl+F shortcut to open/close search
            if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
                *search_open = !*search_open;
                if !*search_open {
                    search_query.clear();
                }
            }

            // Search bar
            let search_spans = if *search_open {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(search_query)
                            .hint_text("Find…")
                            .desired_width(ui.available_width() - 70.0),
                    );
                    // Focus the search field when it first opens
                    if response.gained_focus() || search_query.is_empty() && response.hovered() {
                        response.request_focus();
                    }
                    let query_lower = search_query.to_lowercase();
                    let spans = compute_search_spans(code_buffer, &query_lower);
                    let match_count = spans.len();
                    if !spans.is_empty() {
                        if ui.small_button("▲").on_hover_text("Previous match").clicked() {
                            *search_match_idx = search_match_idx.saturating_sub(1);
                        }
                        if ui.small_button("▼").on_hover_text("Next match").clicked() {
                            *search_match_idx = (*search_match_idx + 1).min(match_count.saturating_sub(1));
                        }
                        ui.label(
                            egui::RichText::new(format!("{}/{}", *search_match_idx + 1, match_count))
                                .small()
                                .color(egui::Color32::from_rgb(156, 163, 175)),
                        );
                    } else if !search_query.is_empty() {
                        ui.label(egui::RichText::new("No matches").small().color(egui::Color32::RED));
                    }
                    // Close on Escape
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        *search_open = false;
                        search_query.clear();
                    }
                    spans
                })
                .inner
            } else {
                search_query.clear();
                *search_match_idx = 0;
                Vec::new()
            };

            if let CodeStatus::InvalidEdit(msg) = code_status {
                ui.label(
                    egui::RichText::new(msg.as_str())
                        .small()
                        .color(egui::Color32::RED),
                );
                if tree_changed_while_invalid {
                    ui.label(
                        egui::RichText::new(
                            "Canvas changed while this edit is invalid; reset to generated code to resync",
                        )
                        .small()
                        .color(egui::Color32::from_rgb(251, 191, 36)),
                    );
                }
            }

            let source_spans = source_spans_for_buffer(code_buffer, &generated);
            let selected_spans = selected_widget_spans(&source_spans, selected_ids);
            match selected_spans.as_slice() {
                [span] => {
                    ui.label(
                        egui::RichText::new(format!(
                            "Selected widget block near line {}",
                            span.lines.start()
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(52, 211, 153)),
                    );
                }
                [] => {}
                spans => {
                    let first_line = first_selected_line(spans).unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!(
                            "{} selected widget blocks highlighted from line {}",
                            spans.len(),
                            first_line
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(52, 211, 153)),
                    );
                }
            }

            if selected_spans.is_empty() && !selected_ids.is_empty() {
                ui.label(
                    egui::RichText::new("Selected widgets are not present in editable code")
                        .small()
                        .color(egui::Color32::from_rgb(234, 179, 8)),
                );
            }

            // ---- always-editable code panel + draggable vertical split ----
            *split_ratio = split_ratio.clamp(0.2, 0.85);
            let total_h = (ui.available_height() - 28.0).max(160.0);
            let divider_h = 7.0;
            let code_h = (total_h * *split_ratio).max(80.0);
            let state_h = (total_h - code_h - divider_h).max(60.0);

            let widget_navigation = navigation_span(requested_navigation, &source_spans);
            // Search match navigation takes priority when search is active
            let search_nav = if !search_spans.is_empty() {
                search_spans.get(*search_match_idx).cloned()
            } else {
                None
            };
            let navigation = search_nav
                .as_ref()
                .or(handler_navigation.as_ref())
                .or(widget_navigation.as_ref());
            // Merge widget selection highlights with search match highlights
            let mut all_highlights = selected_spans.clone();
            all_highlights.extend_from_slice(&search_spans);
            let editor_output = CodeEditorSurface {
                text: code_buffer,
                font_size: code_font_size,
                height: code_h,
                wrap: *wrap_code,
                highlights: &all_highlights,
                navigation,
            }
            .show(ui);
            let _outline_count = editor_output.outline_rects.len();
            let te_resp = editor_output.response;
            *editor_has_focus = te_resp.has_focus();

            if te_resp.changed() {
                if code_buffer.trim().is_empty() {
                    tree.clear_widgets();
                    selected_ids.clear();
                    let canonical = egui_emitter::emit_document(tree);
                    *last_generated = canonical.text.clone();
                    *code_buffer = canonical.text;
                    *code_status = CodeStatus::Generated;
                } else {
                    let report = parser::parse_egui_output(code_buffer);
                    if report.has_errors() {
                        *code_status = CodeStatus::InvalidEdit(report.summary());
                    } else if is_canonical_widget_free_edit(code_buffer, tree, &report) {
                        tree.clear_widgets();
                        selected_ids.clear();
                        let canonical = egui_emitter::emit_document(tree);
                        *last_generated = canonical.text.clone();
                        *code_buffer = canonical.text;
                        *code_status = CodeStatus::Generated;
                    } else if !report.widgets.is_empty() {
                        let outcome = parser::apply_parsed(tree, &report.widgets);
                        let canonical = egui_emitter::emit_document(tree);
                        *last_generated = canonical.text.clone();
                        if outcome.created_widgets > 0 {
                            selected_ids.clear();
                            selected_ids.extend(outcome.created_widget_ids);
                            *code_buffer = canonical.text;
                            *code_status = CodeStatus::Generated;
                        } else {
                            *code_status = CodeStatus::ValidEdit;
                        }
                    } else if code_buffer.contains("widget_") || code_buffer.contains("egui::") {
                        *code_status = CodeStatus::InvalidEdit(
                            "no supported RohKai widget edits found".to_owned(),
                        );
                    } else {
                        *code_status = CodeStatus::ValidEdit;
                    }
                }
            }

            let (divider_rect, divider_resp) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), divider_h),
                egui::Sense::drag(),
            );
            let divider_color = if divider_resp.dragged() || divider_resp.hovered() {
                egui::Color32::from_rgb(52, 211, 153)
            } else {
                egui::Color32::from_gray(70)
            };
            ui.painter().line_segment(
                [divider_rect.left_center(), divider_rect.right_center()],
                egui::Stroke::new(1.0, divider_color),
            );
            if divider_resp.hovered() || divider_resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if divider_resp.dragged() {
                *split_ratio =
                    (*split_ratio + divider_resp.drag_delta().y / total_h).clamp(0.2, 0.85);
            }

            // ---- AppState (always read-only) ----
            ui.label(egui::RichText::new("AppState").strong());
            let state = state_emitter::emit(tree);
            egui::ScrollArea::vertical()
                .id_salt("state_scroll")
                .max_height(state_h)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&state).monospace().size(code_font_size));
                });

            // ---- Symbol list (collapsible) ----
            ui.separator();
            egui::CollapsingHeader::new(egui::RichText::new("Symbols").strong())
                .id_salt("symbol_list")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Widgets").small().weak(),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("symbol_widget_scroll")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for w in &tree.widgets {
                                if w.children.iter().any(|&c| {
                                    tree.widgets.iter().any(|tw| tw.id == c)
                                }) || tree.widgets.iter().any(|tw| tw.children.contains(&w.id))
                                    || !w.children.is_empty()
                                {
                                    // Layout parent
                                }
                                let display_name = if let Some(ref b) = w.state_binding {
                                    format!("{} ({})", w.props.label, b)
                                } else {
                                    w.props.label.clone()
                                };
                                let resp = ui.small_button(
                                    egui::RichText::new(&display_name).monospace(),
                                );
                                if resp.clicked() {
                                    *navigation_target = Some(w.id);
                                }
                            }
                        });

                    // Handler functions in the code buffer
                    let handlers = collect_handler_names(code_buffer);
                    if !handlers.is_empty() {
                        ui.label(egui::RichText::new("Handlers").small().weak());
                        for handler_name in handlers {
                            if ui
                                .small_button(egui::RichText::new(&handler_name).monospace())
                                .clicked()
                            {
                                *scroll_to_handler = Some(handler_name);
                            }
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance, WidgetKind, WidgetProps};

    fn button(id: Uuid, label: &str, x: f32) -> WidgetInstance {
        WidgetInstance {
            id,
            kind: WidgetKind::Button,
            rect: Rect {
                x,
                y: 20.0,
                w: 100.0,
                h: 30.0,
            },
            props: WidgetProps {
                label: label.to_owned(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn generated_source_span_excludes_preamble_and_neighbors() {
        let first = Uuid::from_u128(0xA1);
        let second = Uuid::from_u128(0xB2);
        let mut tree = UiTree::default();
        tree.add(button(first, "One", 10.0));
        tree.add(button(second, "Two", 140.0));

        let document = egui_emitter::emit_document(&tree);
        let first_span = document
            .widget_spans
            .iter()
            .find(|entry| entry.widget_id == first)
            .expect("first span");
        let selected = &document.text[first_span.span.bytes.clone()];

        assert!(selected.starts_with("egui::Area::new"));
        assert!(selected.contains("One"));
        assert!(!selected.contains("CentralPanel"));
        assert!(!selected.contains("Two"));
        assert_eq!(*first_span.span.lines.start(), 3);
    }

    #[test]
    fn selected_widget_spans_preserve_multi_selection() {
        let first = Uuid::from_u128(0xA1);
        let second = Uuid::from_u128(0xB2);
        let mut tree = UiTree::default();
        tree.add(button(first, "One", 10.0));
        tree.add(button(second, "Two", 140.0));
        let document = egui_emitter::emit_document(&tree);

        let spans = selected_widget_spans(&document.widget_spans, &[first, second]);
        assert_eq!(spans.len(), 2);
        assert!(document.text[spans[0].bytes.clone()].contains("One"));
        assert!(document.text[spans[1].bytes.clone()].contains("Two"));
    }

    #[test]
    fn edited_code_uses_parser_source_spans() {
        let id = Uuid::from_u128(0xAB);
        let mut tree = UiTree::default();
        tree.add(button(id, "Before", 10.0));
        let generated = egui_emitter::emit_document(&tree);
        let edited = generated.text.replace("\"Before\"", "\"After\"");

        let spans = source_spans_for_buffer(&edited, &generated);
        let span = spans
            .iter()
            .find(|entry| entry.widget_id == id)
            .expect("edited span");
        assert!(edited[span.span.bytes.clone()].contains("After"));
        assert!(!edited[span.span.bytes.clone()].contains("CentralPanel"));
    }

    #[test]
    fn canonical_preamble_only_code_represents_an_empty_canvas() {
        let id = Uuid::from_u128(0xC3);
        let mut tree = UiTree::default();
        tree.app_props.title = "Preserved".to_owned();
        tree.add(button(id, "Delete Me", 10.0));

        let widget_free = canonical_widget_free_code(&tree);
        let report = parser::parse_egui_output(&widget_free);

        assert!(is_canonical_widget_free_edit(&widget_free, &tree, &report));
        tree.clear_widgets();
        assert!(tree.widgets.is_empty());
        assert_eq!(tree.app_props.title, "Preserved");
    }

    #[test]
    fn outline_uses_gutter_without_touching_visible_text() {
        let decoration = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(120.0, 100.0));
        let text_viewport =
            egui::Rect::from_min_max(egui::pos2(10.0, 8.0), egui::pos2(110.0, 92.0));
        let visible_text = egui::Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(90.0, 70.0));

        let outline =
            highlight_outline_rect(visible_text, text_viewport, decoration).expect("outline");
        assert!(decoration.contains(outline.min));
        assert!(decoration.contains(outline.max));
        assert!(visible_text.left() - outline.left() >= CODE_HIGHLIGHT_PADDING);
        assert!(outline.right() - visible_text.right() >= CODE_HIGHLIGHT_PADDING);
        assert!(visible_text.top() - outline.top() >= CODE_HIGHLIGHT_PADDING);
        assert!(outline.bottom() - visible_text.bottom() >= CODE_HIGHLIGHT_PADDING);
    }

    #[test]
    fn clipped_wide_block_keeps_complete_perimeter_inside_editor() {
        let decoration = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 80.0));
        let text_viewport = egui::Rect::from_min_max(egui::pos2(10.0, 8.0), egui::pos2(90.0, 72.0));
        let wide_block = egui::Rect::from_min_max(egui::pos2(-80.0, 20.0), egui::pos2(240.0, 60.0));

        let outline =
            highlight_outline_rect(wide_block, text_viewport, decoration).expect("outline");
        let safe = decoration.shrink(CODE_HIGHLIGHT_STROKE_WIDTH + 1.0);
        assert!(safe.contains(outline.min));
        assert!(safe.contains(outline.max));
        assert!(outline.left() < text_viewport.left());
        assert!(outline.right() > text_viewport.right());
    }
}
