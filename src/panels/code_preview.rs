use crate::codegen::{egui_emitter, parser, state_emitter};
use crate::project::ui_tree::UiTree;
use uuid::Uuid;

#[derive(Default, PartialEq)]
pub enum CodeStatus {
    #[default]
    Live,
    /// TextEdit has focus — user is editing
    Pending,
    Error(String),
}

pub struct CodePreviewArgs<'a> {
    pub highlighted_id: Option<Uuid>,
    pub scroll_to: &'a mut bool,
    /// Tracé: if Some(name), insert handler stub and clear after consuming.
    pub scroll_to_handler: &'a mut Option<String>,
    pub code_buffer: &'a mut String,
    pub code_status: &'a mut CodeStatus,
    pub last_generated: &'a mut String,
    pub split_ratio: &'a mut f32,
    pub code_font_size: f32,
}

fn highlighted_block(code: &str, id: Uuid) -> Option<(usize, String)> {
    let needle = format!("widget_{id}");
    let line_index = code.lines().position(|line| line.contains(&needle))?;
    let start = line_index.saturating_sub(1);
    let block = code
        .lines()
        .skip(start)
        .take(8)
        .collect::<Vec<_>>()
        .join("\n");
    Some((line_index + 1, block))
}

pub fn show(ctx: &egui::Context, tree: &mut UiTree, args: CodePreviewArgs<'_>) {
    let CodePreviewArgs {
        highlighted_id,
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

            if let Some(id) = highlighted_id {
                if let Some((line, block)) = highlighted_block(code_buffer, id) {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(52, 211, 153, 60))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(52, 211, 153),
                        ))
                        .inner_margin(egui::Margin::same(6.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Selected widget block near line {line}"
                                ))
                                .small()
                                .color(egui::Color32::from_rgb(52, 211, 153)),
                            );
                            ui.label(
                                egui::RichText::new(block)
                                    .monospace()
                                    .size((code_font_size - 1.0).max(10.0)),
                            );
                        });
                }
            }

            // ---- always-editable code panel + draggable vertical split ----
            *split_ratio = split_ratio.clamp(0.2, 0.85);
            let total_h = (ui.available_height() - 28.0).max(160.0);
            let divider_h = 7.0;
            let code_h = (total_h * *split_ratio).max(80.0);
            let state_h = (total_h - code_h - divider_h).max(60.0);

            let te_resp = ui.add_sized(
                [ui.available_width(), code_h],
                egui::TextEdit::multiline(code_buffer)
                    .font(egui::FontId::monospace(code_font_size))
                    .desired_width(f32::INFINITY)
                    .frame(false),
            );

            if te_resp.changed() {
                let report = parser::parse_egui_output(code_buffer);
                if report.has_errors() {
                    *code_status = CodeStatus::Error(report.summary());
                } else if !report.widgets.is_empty() {
                    parser::apply_parsed(tree, &report.widgets);
                    // Update last_generated so canvas-change detection
                    // doesn't clobber the buffer next frame.
                    *last_generated = egui_emitter::emit_indexed(tree)
                        .iter()
                        .map(|(_, l)| l.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !matches!(code_status, CodeStatus::Pending) {
                        *code_status = CodeStatus::Pending;
                    }
                } else if code_buffer.contains("widget_") || code_buffer.contains("egui::") {
                    *code_status =
                        CodeStatus::Error("no supported RohKai widget edits found".to_owned());
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
