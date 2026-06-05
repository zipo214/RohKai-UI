use crate::codegen::{egui_emitter, parser, state_emitter};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

const CODE_EDITOR_PAD_X: f32 = 9.0;
const CODE_EDITOR_PAD_Y: f32 = 6.0;
const CODE_HIGHLIGHT_PADDING: f32 = 6.0;
const CODE_HIGHLIGHT_STROKE_WIDTH: f32 = 1.25;

#[derive(Default, PartialEq)]
pub enum CodeStatus {
    #[default]
    Live,
    /// TextEdit has focus — user is editing
    Pending,
    Error(String),
}

pub struct CodePreviewArgs<'a> {
    pub highlighted_ids: &'a [Uuid],
    pub scroll_to: &'a mut bool,
    /// Tracé: if Some(name), insert handler stub and clear after consuming.
    pub scroll_to_handler: &'a mut Option<String>,
    pub code_buffer: &'a mut String,
    pub code_status: &'a mut CodeStatus,
    pub last_generated: &'a mut String,
    pub split_ratio: &'a mut f32,
    pub code_font_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HighlightRange {
    line: usize,
    start: usize,
    end: usize,
}

fn line_spans(code: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0;
    for line in code.split_inclusive('\n') {
        let end = start + line.len();
        spans.push((start, end));
        start = end;
    }
    spans
}

fn highlighted_range(code: &str, id: Uuid) -> Option<HighlightRange> {
    let needle = format!("widget_{id}");
    let spans = line_spans(code);
    let line_index = spans
        .iter()
        .position(|(start, end)| code[*start..*end].contains(&needle))?;
    let start_line = line_index;
    let mut end_line = line_index;
    for (idx, (start, end)) in spans.iter().enumerate().skip(line_index) {
        end_line = idx;
        if code[*start..*end].trim() == "});" {
            break;
        }
        if idx > line_index
            && (code[*start..*end].contains("egui::Area::new(")
                || code[*start..*end].trim_start().starts_with("// widget_"))
        {
            end_line = idx.saturating_sub(1);
            break;
        }
    }
    Some(HighlightRange {
        line: line_index + 1,
        start: spans[start_line].0,
        end: spans[end_line].1,
    })
}

fn highlighted_ranges(code: &str, ids: &[Uuid]) -> Vec<HighlightRange> {
    let mut ranges: Vec<HighlightRange> = ids
        .iter()
        .filter_map(|&id| highlighted_range(code, id))
        .collect();
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges.dedup_by_key(|range| (range.start, range.end));
    ranges
}

fn code_layout_job(
    ui: &egui::Ui,
    text: &str,
    font_size: f32,
    _highlights: &[HighlightRange],
    wrap_width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let base = egui::TextFormat {
        font_id: egui::FontId::monospace(font_size),
        color: ui.visuals().text_color(),
        ..Default::default()
    };

    job.append(text, 0.0, base);
    job
}

fn byte_to_char_index(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

fn outline_clip_rect(clip_rect: egui::Rect) -> egui::Rect {
    let inset = clip_rect.shrink(CODE_HIGHLIGHT_STROKE_WIDTH + 1.0);
    if inset.is_positive() {
        inset
    } else {
        clip_rect
    }
}

fn clipped_outline_rect(rect: egui::Rect, clip_rect: egui::Rect) -> Option<egui::Rect> {
    let clipped = rect
        .expand(CODE_HIGHLIGHT_PADDING)
        .intersect(outline_clip_rect(clip_rect));
    clipped.is_positive().then_some(clipped)
}

fn highlighted_row_rect(
    row: &egui::epaint::text::Row,
    galley_pos: egui::Pos2,
    clip_rect: egui::Rect,
) -> Option<egui::Rect> {
    let mesh = row.visuals.mesh_bounds;
    let code_rect = if mesh.is_positive() {
        let row_rect = row.rect;
        egui::Rect::from_min_max(
            egui::pos2(mesh.left(), row_rect.top()),
            egui::pos2(mesh.right(), row_rect.bottom()),
        )
    } else {
        row.rect
    };
    let visible = code_rect
        .translate(galley_pos.to_vec2())
        .intersect(clip_rect);
    visible.is_positive().then_some(visible)
}

fn paint_highlight_outlines(
    ui: &egui::Ui,
    output: &egui::widgets::text_edit::TextEditOutput,
    text: &str,
    highlights: &[HighlightRange],
) {
    let mut valid_ranges: Vec<HighlightRange> = highlights
        .iter()
        .copied()
        .filter(|range| range.start < range.end && range.end <= text.len())
        .collect();
    valid_ranges.sort_by_key(|range| (range.start, range.end));

    if valid_ranges.is_empty() {
        return;
    }

    let mut char_ranges: Vec<std::ops::Range<usize>> = valid_ranges
        .iter()
        .map(|range| byte_to_char_index(text, range.start)..byte_to_char_index(text, range.end))
        .collect();
    char_ranges.sort_by_key(|range| (range.start, range.end));

    let painter = ui.painter_at(output.text_clip_rect);
    let outline_clip = outline_clip_rect(output.text_clip_rect);
    let stroke = egui::Stroke::new(
        CODE_HIGHLIGHT_STROKE_WIDTH,
        egui::Color32::from_rgb(52, 211, 153),
    );

    for char_range in char_ranges {
        let mut row_start = 0usize;
        let mut outline: Option<egui::Rect> = None;

        for row in &output.galley.rows {
            let row_text_end = row_start + row.glyphs.len();
            let row_end = row_text_end + usize::from(row.ends_with_newline);
            let intersects = char_range.start < row_end && char_range.end > row_start;

            if intersects {
                if let Some(visible) = highlighted_row_rect(row, output.galley_pos, outline_clip) {
                    outline = Some(match outline {
                        Some(accumulated) => accumulated.union(visible),
                        None => visible,
                    });
                }
            }

            row_start = row_end;
        }

        if let Some(rect) =
            outline.and_then(|rect| clipped_outline_rect(rect, output.text_clip_rect))
        {
            painter.rect_stroke(rect, 3.0, stroke);
        }
    }
}

fn code_rows_for_height(height: f32, font_size: f32) -> usize {
    ((height / (font_size * 1.35)).floor() as usize).max(4)
}

pub fn show(ctx: &egui::Context, tree: &mut UiTree, args: CodePreviewArgs<'_>) {
    let CodePreviewArgs {
        highlighted_ids,
        scroll_to,
        scroll_to_handler,
        code_buffer,
        code_status,
        last_generated,
        split_ratio,
        code_font_size,
    } = args;

    // Tracé — insert handler stub if absent, then consume the signal
    // Current canonical generated code
    let generated: String = egui_emitter::emit_indexed(tree)
        .iter()
        .map(|(_, l)| l.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // Canvas-change detection: tree changed externally → sync buffer, reset to Live
    if generated != *last_generated {
        *last_generated = generated.clone();
        *code_buffer = generated.clone();
        *code_status = CodeStatus::Live;
    }

    // Tracé: sync generated code first, then append the handler stub so the
    // canvas-change reset above cannot erase it on the same frame.
    if let Some(handler_name) = scroll_to_handler.take() {
        let needle = format!("fn {handler_name}(");
        if !code_buffer.contains(&needle) {
            let stub = format!("\nfn {handler_name}(&mut self) {{\n    // TODO: implement\n}}\n");
            code_buffer.push_str(&stub);
            *code_status = CodeStatus::Live;
        }
        *scroll_to = true;
    }

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
                    CodeStatus::Live => (egui::Color32::from_rgb(52, 211, 153), "live"),
                    CodeStatus::Pending => (egui::Color32::from_rgb(234, 179, 8), "pending"),
                    CodeStatus::Error(_) => (egui::Color32::RED, "error"),
                };
                ui.label(egui::RichText::new("●").color(dot_color).small());
                ui.label(egui::RichText::new(status_text).small().color(dot_color));

                if ui
                    .small_button("↺")
                    .on_hover_text("Reset to generated code")
                    .clicked()
                {
                    *code_buffer = generated.clone();
                    *code_status = CodeStatus::Live;
                }
            });

            if let CodeStatus::Error(msg) = code_status {
                ui.label(
                    egui::RichText::new(msg.as_str())
                        .small()
                        .color(egui::Color32::RED),
                );
            }

            let inline_highlights = highlighted_ranges(code_buffer, highlighted_ids);
            match inline_highlights.as_slice() {
                [range] => {
                    ui.label(
                        egui::RichText::new(format!(
                            "Selected widget block near line {}",
                            range.line
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(52, 211, 153)),
                    );
                }
                [] => {}
                ranges => {
                    let first_line = ranges.first().map(|range| range.line).unwrap_or_default();
                    ui.label(
                        egui::RichText::new(format!(
                            "{} selected widget blocks highlighted from line {}",
                            ranges.len(),
                            first_line
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(52, 211, 153)),
                    );
                }
            }

            let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let job = code_layout_job(ui, text, code_font_size, &inline_highlights, wrap_width);
                ui.fonts(|fonts| fonts.layout_job(job))
            };

            if inline_highlights.is_empty() && !highlighted_ids.is_empty() {
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

            let te_output = egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(
                    CODE_EDITOR_PAD_X,
                    CODE_EDITOR_PAD_Y,
                ))
                .show(ui, |ui| {
                    let inner_width = ui.available_width();
                    let inner_height = (code_h - CODE_EDITOR_PAD_Y * 2.0).max(60.0);
                    egui::TextEdit::multiline(code_buffer)
                        .font(egui::FontId::monospace(code_font_size))
                        .desired_width(inner_width)
                        .desired_rows(code_rows_for_height(inner_height, code_font_size))
                        .min_size(egui::vec2(inner_width, inner_height))
                        .code_editor()
                        .layouter(&mut layouter)
                        .frame(false)
                        .show(ui)
                })
                .inner;
            paint_highlight_outlines(ui, &te_output, code_buffer, &inline_highlights);
            let te_resp = te_output.response;

            if te_resp.changed() {
                if code_buffer.trim().is_empty() {
                    tree.clear_widgets();
                    *last_generated = egui_emitter::emit_indexed(tree)
                        .iter()
                        .map(|(_, l)| l.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    *code_buffer = last_generated.clone();
                    *code_status = CodeStatus::Live;
                } else {
                    let report = parser::parse_egui_output(code_buffer);
                    if report.has_errors() {
                        *code_status = CodeStatus::Error(report.summary());
                    } else if !report.widgets.is_empty() {
                        let outcome = parser::apply_parsed(tree, &report.widgets);
                        // Update last_generated so canvas-change detection
                        // doesn't clobber the buffer next frame.
                        *last_generated = egui_emitter::emit_indexed(tree)
                            .iter()
                            .map(|(_, l)| l.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        if outcome.created_widgets > 0 {
                            *code_buffer = last_generated.clone();
                        }
                        if !matches!(code_status, CodeStatus::Pending) {
                            *code_status = CodeStatus::Pending;
                        }
                    } else if code_buffer.contains("widget_") || code_buffer.contains("egui::") {
                        *code_status =
                            CodeStatus::Error("no supported RohKai widget edits found".to_owned());
                    }
                }
            }

            // Status tracks TextEdit focus.
            if te_resp.has_focus() {
                if !matches!(code_status, CodeStatus::Error(_)) {
                    *code_status = CodeStatus::Pending;
                }
            } else if !matches!(code_status, CodeStatus::Error(_)) {
                *code_status = CodeStatus::Live;
            }

            // Lazare scroll: consume signal. Exact TextEdit cursor control is
            // brittle across egui versions, so the visible block above is the
            // stable navigation fallback.
            if *scroll_to {
                *scroll_to = false;
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
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighted_range_finds_widget_block_without_copying_text() {
        let id = Uuid::from_u128(0xAB);
        let code = format!(
            "egui::CentralPanel::default().show(ctx, |_ui| {{}});\n\
             egui::Area::new(egui::Id::new(\"widget_{id}\"))\n\
             \x20   .fixed_pos(egui::pos2(10.0, 20.0))\n\
             \x20   .show(ctx, |ui| {{\n\
             \x20       ui.set_min_size(egui::vec2(100.0, 30.0));\n\
             \x20       ui.button(\"Button\");\n\
             \x20   }});\n\
             trailing\n"
        );

        let range = highlighted_range(&code, id).expect("range");
        assert_eq!(range.line, 2);
        let highlighted = &code[range.start..range.end];
        assert!(highlighted.contains("widget_"));
        assert!(highlighted.contains("ui.button"));
        assert!(!highlighted.contains("CentralPanel"));
        assert!(!highlighted.contains("trailing"));
    }

    #[test]
    fn highlighted_range_is_none_for_missing_widget() {
        assert_eq!(highlighted_range("ui.label(\"none\");", Uuid::nil()), None);
    }

    #[test]
    fn highlighted_ranges_finds_multiple_widget_blocks() {
        let first = Uuid::from_u128(0xA1);
        let second = Uuid::from_u128(0xB2);
        let code = format!(
            "egui::CentralPanel::default().show(ctx, |_ui| {{}});\n\
             egui::Area::new(egui::Id::new(\"widget_{first}\"))\n\
             \x20   .show(ctx, |ui| {{\n\
             \x20       ui.button(\"One\");\n\
             \x20   }});\n\
             egui::Area::new(egui::Id::new(\"widget_{second}\"))\n\
             \x20   .show(ctx, |ui| {{\n\
             \x20       ui.button(\"Two\");\n\
             \x20   }});\n"
        );

        let ranges = highlighted_ranges(&code, &[first, second]);
        assert_eq!(ranges.len(), 2);
        assert!(code[ranges[0].start..ranges[0].end].contains("One"));
        assert!(code[ranges[1].start..ranges[1].end].contains("Two"));
    }

    #[test]
    fn clipped_outline_rect_stays_inside_visible_text_clip() {
        let clip = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(110.0, 120.0));
        let raw = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(150.0, 200.0));

        let clipped = clipped_outline_rect(raw, clip).expect("visible outline");
        let expected = outline_clip_rect(clip);

        assert!(clip.contains(clipped.min));
        assert!(clip.contains(clipped.max));
        assert_eq!(clipped, expected);
    }
}
