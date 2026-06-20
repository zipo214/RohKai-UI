//! Visual Widget Maker panel — floating window for the primitive mini-canvas.
//!
//! `show_widget_maker_window` is the entry point called from `app.rs`.
//! The mini-canvas renders normalised `MakerPrimitive` shapes; corner handles
//! allow interactive resize.  The inspector on the right edits the selected
//! primitive.  "Save" serialises via `doc_to_descriptor` → JSON → `.rkwd` file.

use crate::canvas::widget_maker::{
    MakerPrimKind, MakerPrimitive, PrimAnchor, PrimState, SlotDef, StyleTokens, WidgetMakerDoc,
    doc_to_descriptor, gen_export_template, gen_live_preview, is_group_kind,
};
use egui::Color32;

const CANVAS_W: f32 = 280.0;
const CANVAS_H: f32 = 140.0;
const PRIM_MIN: f32 = 0.05; // minimum normalised size enforced by drag-value UI

/// Render the Visual Widget Maker window.
///
/// Returns the generated descriptor as JSON if the user clicked "Save Descriptor",
/// and the window close request if the user dismissed the window.
pub struct WidgetMakerResult {
    pub save_json: Option<String>,
    pub close_requested: bool,
}

pub fn show_widget_maker_window(
    ctx: &egui::Context,
    doc: &mut WidgetMakerDoc,
    open: &mut bool,
) -> WidgetMakerResult {
    let mut result = WidgetMakerResult {
        save_json: None,
        close_requested: false,
    };

    let mut keep_open = *open;
    let bounds = crate::panels::window_bounds::authoring_window_bounds(
        ctx.content_rect(),
        egui::vec2(960.0, 620.0),
        egui::vec2(700.0, 460.0),
    );
    egui::Window::new("Visual Widget Maker")
        .id(egui::Id::new("widget_maker_window"))
        .open(&mut keep_open)
        .default_pos(bounds.default_pos)
        .default_size(bounds.default_size)
        .min_size(bounds.min_size)
        .max_size(bounds.max_size)
        .resizable(true)
        .constrain(true)
        .show(ctx, |ui| {
            egui::ScrollArea::both()
                .id_salt("widget_maker_window_scroll")
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // ---- Left: mini-canvas + primitive list + toolbar ----
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Canvas").strong());
                            show_mini_canvas(ui, doc);
                            ui.separator();
                            show_toolbar(ui, doc);
                            ui.separator();
                            // Item 2: Primitive list with z-order ↑/↓ buttons
                            show_primitive_list(ui, doc);
                        });

                        ui.separator();

                        // ---- Right: tabs (Properties | Code Preview) ----
                        ui.vertical(|ui| {
                            ui.set_min_width(300.0);

                            // Tab bar
                            let active_tab = ui.data_mut(|d| {
                                *d.get_temp_mut_or_default::<WmTab>(egui::Id::new("wm_active_tab"))
                            });
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(active_tab == WmTab::Properties, "Properties")
                                    .clicked()
                                {
                                    ui.data_mut(|d| {
                                        *d.get_temp_mut_or_default::<WmTab>(egui::Id::new(
                                            "wm_active_tab",
                                        )) = WmTab::Properties;
                                    });
                                }
                                if ui
                                    .selectable_label(
                                        active_tab == WmTab::CodePreview,
                                        "Code Preview",
                                    )
                                    .clicked()
                                {
                                    ui.data_mut(|d| {
                                        *d.get_temp_mut_or_default::<WmTab>(egui::Id::new(
                                            "wm_active_tab",
                                        )) = WmTab::CodePreview;
                                    });
                                }
                            });
                            ui.separator();

                            // Re-read after possible mutation above
                            let active_tab = ui.data_mut(|d| {
                                *d.get_temp_mut_or_default::<WmTab>(egui::Id::new("wm_active_tab"))
                            });
                            match active_tab {
                                WmTab::Properties => {
                                    show_properties_tab(ui, doc, &mut result);
                                }
                                WmTab::CodePreview => {
                                    show_code_preview_tab(ui, doc);
                                }
                            }
                        });
                    });
                });
        });

    if !keep_open {
        *open = false;
        result.close_requested = true;
    }

    result
}

// ---------------------------------------------------------------------------
// Tab enum (stored in egui temp storage per-frame)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum WmTab {
    #[default]
    Properties,
    CodePreview,
}

// ---------------------------------------------------------------------------
// Properties tab (original right panel content)
// ---------------------------------------------------------------------------

fn show_properties_tab(
    ui: &mut egui::Ui,
    doc: &mut WidgetMakerDoc,
    result: &mut WidgetMakerResult,
) {
    ui.label(egui::RichText::new("Widget Properties").strong());
    show_widget_meta(ui, doc);
    ui.separator();
    if let Some(idx) = doc.selected {
        if idx < doc.primitives.len() {
            ui.label(egui::RichText::new("Primitive").strong());
            show_primitive_props(ui, &mut doc.primitives[idx]);
        }
    } else {
        ui.label(
            egui::RichText::new("Select a primitive to edit its properties")
                .small()
                .weak(),
        );
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .button("💾 Save Descriptor")
            .on_hover_text("Export as .rkwd and reload palette")
            .clicked()
        {
            let descriptor = doc_to_descriptor(doc);
            if let Ok(json) = serde_json::to_string_pretty(&descriptor) {
                result.save_json = Some(json);
            }
        }
        if ui.button("Reset").clicked() {
            *doc = WidgetMakerDoc::new_with_defaults();
        }
    });
}

// ---------------------------------------------------------------------------
// Item 1: Code Preview tab
// ---------------------------------------------------------------------------

fn show_code_preview_tab(ui: &mut egui::Ui, doc: &WidgetMakerDoc) {
    ui.label(egui::RichText::new("Code Preview").strong());
    ui.label(
        egui::RichText::new("Live-updated from current primitives. Read-only.")
            .small()
            .weak(),
    );
    ui.separator();

    let live = gen_live_preview(doc);
    let export = gen_export_template(doc);

    ui.label(egui::RichText::new("live_preview").small().strong());
    egui::ScrollArea::vertical()
        .id_salt("wm_code_live_scroll")
        .max_height(140.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut live.as_str())
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            );
        });

    ui.add_space(4.0);
    ui.label(egui::RichText::new("export template").small().strong());
    egui::ScrollArea::vertical()
        .id_salt("wm_code_export_scroll")
        .max_height(140.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut export.as_str())
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            );
        });
}

// ---------------------------------------------------------------------------
// Item 2: Primitive list with ↑/↓ reorder buttons
// ---------------------------------------------------------------------------

fn show_primitive_list(ui: &mut egui::Ui, doc: &mut WidgetMakerDoc) {
    ui.label(egui::RichText::new("Primitives").small().strong());
    let n = doc.primitives.len();
    if n == 0 {
        ui.label(
            egui::RichText::new("No primitives — add one above")
                .small()
                .weak(),
        );
        return;
    }

    // Collect reorder actions to apply after the loop (avoids borrow conflict).
    let mut move_up: Option<usize> = None;
    let mut move_down: Option<usize> = None;

    for i in 0..n {
        ui.horizontal(|ui| {
            let label = format!(
                "{i}: {}",
                match doc.primitives[i].kind {
                    MakerPrimKind::Rect => "Rect",
                    MakerPrimKind::Outline => "Outline",
                    MakerPrimKind::Ellipse => "Ellipse",
                    MakerPrimKind::Text => "Text",
                    MakerPrimKind::HitRegion => "HitRgn",
                    MakerPrimKind::HGroup => "HGroup",
                    MakerPrimKind::VGroup => "VGroup",
                    MakerPrimKind::Grid => "Grid",
                    MakerPrimKind::Stack => "Stack",
                }
            );
            let selected = doc.selected == Some(i);
            if ui
                .selectable_label(selected, egui::RichText::new(&label).small())
                .clicked()
            {
                doc.selected = Some(i);
            }
            // ↑ moves earlier in the list (lower index = rendered first = behind)
            ui.add_enabled_ui(i > 0, |ui| {
                if ui
                    .small_button("↑")
                    .on_hover_text("Move up (render earlier / further back)")
                    .clicked()
                {
                    move_up = Some(i);
                }
            });
            // ↓ moves later in the list (higher index = rendered last = in front)
            ui.add_enabled_ui(i + 1 < n, |ui| {
                if ui
                    .small_button("↓")
                    .on_hover_text("Move down (render later / further forward)")
                    .clicked()
                {
                    move_down = Some(i);
                }
            });
        });
    }

    if let Some(i) = move_up {
        doc.primitives.swap(i, i - 1);
        if doc.selected == Some(i) {
            doc.selected = Some(i - 1);
        } else if doc.selected == Some(i - 1) {
            doc.selected = Some(i);
        }
    }
    if let Some(i) = move_down {
        doc.primitives.swap(i, i + 1);
        if doc.selected == Some(i) {
            doc.selected = Some(i + 1);
        } else if doc.selected == Some(i + 1) {
            doc.selected = Some(i);
        }
    }
}

// ---------------------------------------------------------------------------
// Mini-canvas
// ---------------------------------------------------------------------------

fn show_mini_canvas(ui: &mut egui::Ui, doc: &mut WidgetMakerDoc) {
    let (canvas_rect, canvas_resp) = ui.allocate_exact_size(
        egui::vec2(CANVAS_W, CANVAS_H),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(canvas_rect);

    // Background
    painter.rect_filled(canvas_rect, 2.0, Color32::from_gray(30));
    painter.rect_stroke(
        canvas_rect,
        2.0,
        egui::Stroke::new(1.0, Color32::from_gray(80)),
        egui::StrokeKind::Inside,
    );

    // Draw primitives (bottom to top)
    for (idx, prim) in doc.primitives.iter().enumerate() {
        let pr = prim_canvas_rect(prim, canvas_rect);
        draw_primitive(&painter, prim, pr, &doc.style_tokens);

        // Selection outline
        if doc.selected == Some(idx) {
            painter.rect_stroke(
                pr,
                2.0,
                egui::Stroke::new(1.5, Color32::from_rgb(52, 211, 153)),
                egui::StrokeKind::Inside,
            );
            draw_resize_handles(&painter, pr);
        }
    }

    // Interaction: click to select
    if canvas_resp.clicked() {
        let click_pos = canvas_resp.interact_pointer_pos().unwrap_or_default();
        let mut hit: Option<usize> = None;
        for (idx, prim) in doc.primitives.iter().enumerate().rev() {
            let pr = prim_canvas_rect(prim, canvas_rect);
            if pr.contains(click_pos) {
                hit = Some(idx);
                break;
            }
        }
        doc.selected = hit;
    }

    // On drag start: check if pointer is on a corner handle → switch to resize mode
    if canvas_resp.drag_started() {
        doc.resize_corner = None;
        if let Some(idx) = doc.selected
            && idx < doc.primitives.len()
            && let Some(pos) = canvas_resp.interact_pointer_pos()
        {
            let pr = prim_canvas_rect(&doc.primitives[idx], canvas_rect);
            doc.resize_corner = corner_hit(pos, pr);
        }
    }

    // Drag: resize or move depending on whether a corner handle was grabbed
    if canvas_resp.dragged() {
        let delta = canvas_resp.drag_delta();
        if let Some(corner) = doc.resize_corner {
            if let Some(idx) = doc.selected
                && idx < doc.primitives.len()
            {
                apply_corner_resize(
                    &mut doc.primitives[idx],
                    corner,
                    delta.x / CANVAS_W,
                    delta.y / CANVAS_H,
                );
            }
        } else if let Some(idx) = doc.selected
            && idx < doc.primitives.len()
        {
            let p = &mut doc.primitives[idx];
            p.x = (p.x + delta.x / CANVAS_W).clamp(0.0, 1.0 - p.w);
            p.y = (p.y + delta.y / CANVAS_H).clamp(0.0, 1.0 - p.h);
        }
    }

    if canvas_resp.drag_stopped() {
        doc.resize_corner = None;
    }
}

fn show_toolbar(ui: &mut egui::Ui, doc: &mut WidgetMakerDoc) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Add:").small().weak());
        if ui.small_button("▬ Rect").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::Rect,
                x: 0.1,
                y: 0.1,
                w: 0.8,
                h: 0.8,
                fill: [80, 80, 200],
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("○ Outline").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::Outline,
                x: 0.05,
                y: 0.05,
                w: 0.9,
                h: 0.9,
                fill: [180, 180, 180],
                corner_radius: 3.0,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("● Ellipse").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::Ellipse,
                x: 0.25,
                y: 0.1,
                w: 0.5,
                h: 0.8,
                fill: [200, 100, 80],
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("T Text").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::Text,
                x: 0.05,
                y: 0.2,
                w: 0.9,
                h: 0.6,
                fill: [240, 240, 240],
                use_label_token: true,
                font_size: 14.0,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("⊡ HitRgn").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::HitRegion,
                sense_click: true,
                x: 0.1,
                y: 0.1,
                w: 0.8,
                h: 0.8,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("↔ HGroup").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::HGroup,
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                group_gap: 4.0,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("↕ VGroup").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::VGroup,
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                group_gap: 4.0,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("⊞ Grid").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::Grid,
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                group_gap: 4.0,
                grid_cols: 2,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }
        if ui.small_button("⬛ Stack").clicked() {
            doc.primitives.push(MakerPrimitive {
                kind: MakerPrimKind::Stack,
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
                ..Default::default()
            });
            doc.selected = Some(doc.primitives.len() - 1);
        }

        let can_remove = doc
            .selected
            .map(|i| i < doc.primitives.len())
            .unwrap_or(false);
        ui.add_enabled_ui(can_remove, |ui| {
            if ui.small_button("✕ Remove").clicked()
                && let Some(idx) = doc.selected.take()
            {
                doc.primitives.remove(idx);
            }
        });

        // Z-order (existing toolbar buttons kept for discoverability)
        if let Some(idx) = doc.selected {
            let n = doc.primitives.len();
            ui.add_enabled_ui(idx > 0, |ui| {
                if ui.small_button("▼ Back").clicked() {
                    doc.primitives.swap(idx, idx - 1);
                    doc.selected = Some(idx - 1);
                }
            });
            ui.add_enabled_ui(idx + 1 < n, |ui| {
                if ui.small_button("▲ Front").clicked() {
                    doc.primitives.swap(idx, idx + 1);
                    doc.selected = Some(idx + 1);
                }
            });
        }
    });
}

fn show_widget_meta(ui: &mut egui::Ui, doc: &mut WidgetMakerDoc) {
    egui::Grid::new("wm_meta")
        .num_columns(2)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Name").small());
            ui.add(
                egui::TextEdit::singleline(&mut doc.widget_name)
                    .hint_text("Widget name")
                    .desired_width(160.0),
            );
            ui.end_row();
            ui.label(egui::RichText::new("ID").small());
            ui.add(
                egui::TextEdit::singleline(&mut doc.widget_id)
                    .hint_text("e.g. mylib.button")
                    .desired_width(160.0),
            );
            ui.end_row();
            ui.label(egui::RichText::new("Category").small());
            ui.add(
                egui::TextEdit::singleline(&mut doc.category)
                    .hint_text("Palette category")
                    .desired_width(160.0),
            );
            ui.end_row();
            ui.label(egui::RichText::new("Size").small());
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut doc.default_size[0])
                        .range(20.0..=800.0_f32)
                        .suffix("w")
                        .speed(1.0),
                );
                ui.label("×");
                ui.add(
                    egui::DragValue::new(&mut doc.default_size[1])
                        .range(10.0..=600.0_f32)
                        .suffix("h")
                        .speed(1.0),
                );
            });
            ui.end_row();
        });

    // Style Tokens collapsible group
    show_style_tokens(ui, &mut doc.style_tokens);

    // Slots collapsible group
    show_slots(ui, &mut doc.slots);
}

fn show_style_tokens(ui: &mut egui::Ui, tokens: &mut StyleTokens) {
    ui.collapsing(egui::RichText::new("Style Tokens").small().strong(), |ui| {
        egui::Grid::new("wm_tokens")
            .num_columns(4)
            .spacing([4.0, 2.0])
            .show(ui, |ui| {
                // Accent
                ui.label(egui::RichText::new("Accent").small());
                ui.add(egui::DragValue::new(&mut tokens.accent[0]).range(0..=255_u8));
                ui.add(egui::DragValue::new(&mut tokens.accent[1]).range(0..=255_u8));
                ui.add(egui::DragValue::new(&mut tokens.accent[2]).range(0..=255_u8));
                ui.end_row();
                // Border
                ui.label(egui::RichText::new("Border").small());
                ui.add(egui::DragValue::new(&mut tokens.border[0]).range(0..=255_u8));
                ui.add(egui::DragValue::new(&mut tokens.border[1]).range(0..=255_u8));
                ui.add(egui::DragValue::new(&mut tokens.border[2]).range(0..=255_u8));
                ui.end_row();
                // Text color
                ui.label(egui::RichText::new("Text").small());
                ui.add(egui::DragValue::new(&mut tokens.text_color[0]).range(0..=255_u8));
                ui.add(egui::DragValue::new(&mut tokens.text_color[1]).range(0..=255_u8));
                ui.add(egui::DragValue::new(&mut tokens.text_color[2]).range(0..=255_u8));
                ui.end_row();
                // Corner radius
                ui.label(egui::RichText::new("Radius").small());
                ui.add(
                    egui::DragValue::new(&mut tokens.corner_radius)
                        .range(0.0..=32.0_f32)
                        .speed(0.5),
                );
                ui.end_row();
                // Spacing
                ui.label(egui::RichText::new("Spacing").small());
                ui.add(
                    egui::DragValue::new(&mut tokens.spacing)
                        .range(0.0..=32.0_f32)
                        .speed(0.5),
                );
                ui.end_row();
            });
    });
}

fn show_slots(ui: &mut egui::Ui, slots: &mut Vec<SlotDef>) {
    ui.collapsing(egui::RichText::new("Slots").small().strong(), |ui| {
        let mut remove = None;
        for (i, slot) in slots.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut slot.name)
                        .hint_text("slot name")
                        .desired_width(80.0),
                );
                ui.label(egui::RichText::new("x").small().weak());
                ui.add(
                    egui::DragValue::new(&mut slot.x)
                        .range(0.0..=1.0_f32)
                        .speed(0.005),
                );
                ui.label(egui::RichText::new("y").small().weak());
                ui.add(
                    egui::DragValue::new(&mut slot.y)
                        .range(0.0..=1.0_f32)
                        .speed(0.005),
                );
                ui.label(egui::RichText::new("w").small().weak());
                ui.add(
                    egui::DragValue::new(&mut slot.w)
                        .range(0.01..=1.0_f32)
                        .speed(0.005),
                );
                ui.label(egui::RichText::new("h").small().weak());
                ui.add(
                    egui::DragValue::new(&mut slot.h)
                        .range(0.01..=1.0_f32)
                        .speed(0.005),
                );
                if ui.small_button("✕").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            slots.remove(i);
        }
        if ui.small_button("+ Add Slot").clicked() {
            slots.push(SlotDef::default());
        }
    });
}

fn show_primitive_props(ui: &mut egui::Ui, prim: &mut MakerPrimitive) {
    // Kind selector
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Kind").small().weak());
        for (label, kind) in [
            ("Rect", MakerPrimKind::Rect),
            ("Outline", MakerPrimKind::Outline),
            ("Ellipse", MakerPrimKind::Ellipse),
            ("Text", MakerPrimKind::Text),
            ("⊡ HitRgn", MakerPrimKind::HitRegion),
            ("↔ HGroup", MakerPrimKind::HGroup),
            ("↕ VGroup", MakerPrimKind::VGroup),
            ("⊞ Grid", MakerPrimKind::Grid),
            ("⬛ Stack", MakerPrimKind::Stack),
        ] {
            if ui
                .selectable_label(prim.kind == kind, egui::RichText::new(label).small())
                .clicked()
            {
                prim.kind = kind;
            }
        }
    });

    egui::Grid::new("prim_props")
        .num_columns(4)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("X%").small());
            let mut xp = prim.x * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut xp)
                        .range(0.0..=95.0_f32)
                        .speed(0.5)
                        .suffix("%"),
                )
                .changed()
            {
                prim.x = xp / 100.0;
            }
            ui.label(egui::RichText::new("Y%").small());
            let mut yp = prim.y * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut yp)
                        .range(0.0..=95.0_f32)
                        .speed(0.5)
                        .suffix("%"),
                )
                .changed()
            {
                prim.y = yp / 100.0;
            }
            ui.end_row();
            ui.label(egui::RichText::new("W%").small());
            let mut wp = prim.w * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut wp)
                        .range((PRIM_MIN * 100.0)..=100.0_f32)
                        .speed(0.5)
                        .suffix("%"),
                )
                .changed()
            {
                prim.w = (wp / 100.0).max(prim.min_w);
            }
            ui.label(egui::RichText::new("H%").small());
            let mut hp = prim.h * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut hp)
                        .range((PRIM_MIN * 100.0)..=100.0_f32)
                        .speed(0.5)
                        .suffix("%"),
                )
                .changed()
            {
                prim.h = (hp / 100.0).max(prim.min_h);
            }
            ui.end_row();
            // Fill RGB only for non-HitRegion kinds
            if !matches!(prim.kind, MakerPrimKind::HitRegion) {
                ui.label(egui::RichText::new("R").small());
                ui.add(egui::DragValue::new(&mut prim.fill[0]).range(0..=255_u8));
                ui.label(egui::RichText::new("G").small());
                ui.add(egui::DragValue::new(&mut prim.fill[1]).range(0..=255_u8));
                ui.end_row();
                ui.label(egui::RichText::new("B").small());
                ui.add(egui::DragValue::new(&mut prim.fill[2]).range(0..=255_u8));
                if matches!(prim.kind, MakerPrimKind::Rect | MakerPrimKind::Outline) {
                    ui.label(egui::RichText::new("Rad").small());
                    ui.add(
                        egui::DragValue::new(&mut prim.corner_radius)
                            .range(0.0..=32.0_f32)
                            .speed(0.5),
                    );
                }
                ui.end_row();
            }
        });

    // Token fill checkbox (all non-HitRegion kinds)
    if !matches!(prim.kind, MakerPrimKind::HitRegion) {
        ui.checkbox(
            &mut prim.use_token_fill,
            egui::RichText::new("Use accent token fill").small(),
        );
    }

    if matches!(prim.kind, MakerPrimKind::Text) {
        ui.checkbox(
            &mut prim.use_token_text_color,
            egui::RichText::new("Use text-color token").small(),
        );
        ui.checkbox(
            &mut prim.use_label_token,
            egui::RichText::new("Use {{label}} token").small(),
        )
        .on_hover_text("Replace text with the widget's runtime Label property");
        if !prim.use_label_token {
            ui.add(
                egui::TextEdit::singleline(&mut prim.text_content)
                    .hint_text("Static text…")
                    .desired_width(f32::INFINITY),
            );
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Font size").small().weak());
            ui.add(
                egui::DragValue::new(&mut prim.font_size)
                    .range(6.0..=72.0_f32)
                    .speed(0.5),
            );
        });
    }

    // HitRegion sense flags
    if matches!(prim.kind, MakerPrimKind::HitRegion) {
        ui.separator();
        ui.label(egui::RichText::new("Hit Region").small().strong());
        ui.add(
            egui::TextEdit::singleline(&mut prim.prim_name)
                .hint_text("zone name (optional)")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut prim.sense_click, egui::RichText::new("Click").small());
            ui.checkbox(&mut prim.sense_hover, egui::RichText::new("Hover").small());
            ui.checkbox(&mut prim.sense_drag, egui::RichText::new("Drag").small());
        });
    }

    // Layout group properties
    if is_group_kind(&prim.kind) {
        ui.separator();
        ui.label(egui::RichText::new("Group").small().strong());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Gap").small().weak());
            ui.add(
                egui::DragValue::new(&mut prim.group_gap)
                    .range(0.0..=64.0_f32)
                    .speed(0.5),
            );
        });
        if matches!(prim.kind, MakerPrimKind::Grid) {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Cols").small().weak());
                ui.add(egui::DragValue::new(&mut prim.grid_cols).range(1..=16_u32));
            });
        }
    }

    // --- Item 3: Constraints ---
    ui.separator();
    ui.label(egui::RichText::new("Constraints").small().strong());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Anchor").small().weak());
        egui::ComboBox::from_id_salt("prim_anchor")
            .selected_text(prim.anchor.label())
            .show_ui(ui, |ui| {
                for &anchor in PrimAnchor::ALL {
                    ui.selectable_value(&mut prim.anchor, anchor, anchor.label());
                }
            });
    });
    egui::Grid::new("prim_constraints")
        .num_columns(4)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Min W%").small());
            let mut min_wp = prim.min_w * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut min_wp)
                        .range(0.0..=100.0_f32)
                        .speed(0.5)
                        .suffix("%"),
                )
                .changed()
            {
                prim.min_w = min_wp / 100.0;
            }
            ui.label(egui::RichText::new("Min H%").small());
            let mut min_hp = prim.min_h * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut min_hp)
                        .range(0.0..=100.0_f32)
                        .speed(0.5)
                        .suffix("%"),
                )
                .changed()
            {
                prim.min_h = min_hp / 100.0;
            }
            ui.end_row();
        });

    // State variants (non-group, non-HitRegion kinds only)
    if !is_group_kind(&prim.kind) && !matches!(prim.kind, MakerPrimKind::HitRegion) {
        ui.separator();
        ui.collapsing(
            egui::RichText::new("State Variants").small().strong(),
            |ui| {
                for &state in PrimState::ALL {
                    if matches!(state, PrimState::Normal) {
                        continue;
                    }
                    let enabled = prim.variants.get(state).is_some();
                    let mut is_enabled = enabled;
                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(&mut is_enabled, egui::RichText::new(state.label()).small())
                            .changed()
                        {
                            prim.variants.set_enabled(state, is_enabled);
                        }
                        if is_enabled && let Some(ov) = prim.variants.get_mut(state) {
                            let has_fill = ov.fill.is_some();
                            let mut fill_on = has_fill;
                            ui.checkbox(&mut fill_on, egui::RichText::new("Fill").small());
                            if fill_on && !has_fill {
                                ov.fill = Some([200, 200, 200]);
                            } else if !fill_on {
                                ov.fill = None;
                            }
                            if let Some(ref mut rgb) = ov.fill {
                                ui.add(egui::DragValue::new(&mut rgb[0]).range(0..=255_u8));
                                ui.add(egui::DragValue::new(&mut rgb[1]).range(0..=255_u8));
                                ui.add(egui::DragValue::new(&mut rgb[2]).range(0..=255_u8));
                            }
                        }
                    });
                }
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn prim_canvas_rect(prim: &MakerPrimitive, canvas: egui::Rect) -> egui::Rect {
    let min = egui::pos2(
        canvas.min.x + prim.x * canvas.width(),
        canvas.min.y + prim.y * canvas.height(),
    );
    let max = egui::pos2(
        canvas.min.x + (prim.x + prim.w) * canvas.width(),
        canvas.min.y + (prim.y + prim.h) * canvas.height(),
    );
    egui::Rect::from_min_max(min, max)
}

fn draw_primitive(
    painter: &egui::Painter,
    prim: &MakerPrimitive,
    rect: egui::Rect,
    tokens: &StyleTokens,
) {
    match prim.kind {
        MakerPrimKind::HitRegion => {
            // Dashed orange outline — no fill
            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.5, Color32::from_rgb(255, 165, 0)),
                egui::StrokeKind::Inside,
            );
        }
        _ => {
            let [r, g, b] = if prim.use_token_fill {
                tokens.accent
            } else {
                prim.fill
            };
            let color = Color32::from_rgb(r, g, b);
            match prim.kind {
                MakerPrimKind::Rect => {
                    painter.rect_filled(rect, prim.corner_radius, color);
                }
                MakerPrimKind::Outline => {
                    painter.rect_stroke(
                        rect,
                        prim.corner_radius,
                        egui::Stroke::new(1.5, color),
                        egui::StrokeKind::Inside,
                    );
                }
                MakerPrimKind::Ellipse => {
                    let cx = rect.center();
                    let rx = rect.width() * 0.5;
                    let ry = rect.height() * 0.5;
                    if (rx - ry).abs() < 2.0 {
                        painter.circle_filled(cx, rx, color);
                    } else {
                        // Approximate ellipse with 32 segments
                        let n = 32usize;
                        let points: Vec<egui::Pos2> = (0..=n)
                            .map(|i| {
                                let t = i as f32 * std::f32::consts::TAU / n as f32;
                                egui::pos2(cx.x + rx * t.cos(), cx.y + ry * t.sin())
                            })
                            .collect();
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            color,
                            egui::Stroke::NONE,
                        ));
                    }
                }
                MakerPrimKind::Text => {
                    let [tr, tg, tb] = if prim.use_token_text_color {
                        tokens.text_color
                    } else {
                        prim.fill
                    };
                    let text_color = Color32::from_rgb(tr, tg, tb);
                    let font_id = egui::FontId::proportional(prim.font_size);
                    let text = if prim.use_label_token {
                        "Label"
                    } else {
                        prim.text_content.as_str()
                    };
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        text,
                        font_id,
                        text_color,
                    );
                }
                MakerPrimKind::HitRegion => unreachable!(),
                // Groups render as a dashed outline in the mini-canvas
                MakerPrimKind::HGroup
                | MakerPrimKind::VGroup
                | MakerPrimKind::Grid
                | MakerPrimKind::Stack => {
                    painter.rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0, Color32::from_rgb(100, 180, 100)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
    }
}

/// Return the index (0=TL,1=TR,2=BL,3=BR) of the corner handle closest to `pos`
/// if it is within the hit radius, otherwise `None`.
fn corner_hit(pos: egui::Pos2, rect: egui::Rect) -> Option<u8> {
    const HIT_RADIUS: f32 = 8.0;
    let corners = [
        (0u8, rect.left_top()),
        (1u8, rect.right_top()),
        (2u8, rect.left_bottom()),
        (3u8, rect.right_bottom()),
    ];
    for (idx, corner) in corners {
        if pos.distance(corner) <= HIT_RADIUS {
            return Some(idx);
        }
    }
    None
}

/// Apply a normalised `(dx, dy)` resize delta for the given corner index to `p`.
/// Corners: 0=TL, 1=TR, 2=BL, 3=BR.
///
/// Respects `p.min_w` and `p.min_h` — the resulting width/height will never
/// drop below those thresholds (Item 3).
///
/// Exported `pub(crate)` so unit tests in `widget_maker.rs` can call it directly.
pub(crate) fn apply_corner_resize(
    p: &mut crate::canvas::widget_maker::MakerPrimitive,
    corner: u8,
    dx: f32,
    dy: f32,
) {
    // The hard absolute floor for a drag (independent of the user min_w/min_h).
    const ABS_MIN: f32 = 0.05;
    let floor_w = p.min_w.max(ABS_MIN);
    let floor_h = p.min_h.max(ABS_MIN);

    match corner {
        0 => {
            // Top-left: x and y move, w and h shrink/grow inversely
            let new_x = (p.x + dx).clamp(0.0, p.x + p.w - floor_w);
            let new_y = (p.y + dy).clamp(0.0, p.y + p.h - floor_h);
            p.w += p.x - new_x;
            p.h += p.y - new_y;
            p.x = new_x;
            p.y = new_y;
        }
        1 => {
            // Top-right: y moves, w and h change
            let new_y = (p.y + dy).clamp(0.0, p.y + p.h - floor_h);
            p.w = (p.w + dx).clamp(floor_w, 1.0 - p.x);
            p.h += p.y - new_y;
            p.y = new_y;
        }
        2 => {
            // Bottom-left: x moves, h grows/shrinks
            let new_x = (p.x + dx).clamp(0.0, p.x + p.w - floor_w);
            p.w += p.x - new_x;
            p.x = new_x;
            p.h = (p.h + dy).clamp(floor_h, 1.0 - p.y);
        }
        3 => {
            // Bottom-right: pure w/h change
            p.w = (p.w + dx).clamp(floor_w, 1.0 - p.x);
            p.h = (p.h + dy).clamp(floor_h, 1.0 - p.y);
        }
        _ => {}
    }
}

fn draw_resize_handles(painter: &egui::Painter, rect: egui::Rect) {
    let handle_color = Color32::from_rgb(52, 211, 153);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ];
    for corner in corners {
        painter.circle_filled(corner, 3.5, handle_color);
    }
}
