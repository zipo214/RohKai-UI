//! Stage 11 — Rust Wiring editor window.
//!
//! Edits `AppProps.rust_wiring`: mpsc channels, iterator pipelines, and trait
//! impls. A live preview of the generated code for each section reuses the
//! `codegen::rust_wiring` emitters, so what the user sees is what export writes.

use crate::project::schema::{ChannelDef, IterOp, IteratorPipeline, RustWiring, TraitImpl};
use uuid::Uuid;

/// Render the Rust Wiring window. Mutates `wiring` in place. Returns while open.
pub fn show(ctx: &egui::Context, open: &mut bool, wiring: &mut RustWiring) -> bool {
    if !*open {
        return false;
    }
    let mut keep_open = true;

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(
        (screen.center().x - 320.0).max(screen.min.x + 20.0),
        (screen.center().y - 260.0).max(screen.min.y + 20.0),
    );

    egui::Window::new("Rust Wiring")
        .id(egui::Id::new("rust_wiring_window"))
        .open(&mut keep_open)
        .default_pos(default_pos)
        .default_size([640.0, 520.0])
        .min_size([460.0, 320.0])
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("rust_wiring_scroll")
                .show(ui, |ui| {
                    show_channels(ui, wiring);
                    ui.add_space(8.0);
                    show_iterators(ui, wiring);
                    ui.add_space(8.0);
                    show_trait_impls(ui, wiring);
                });
        });

    *open = keep_open;
    keep_open
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

fn show_channels(ui: &mut egui::Ui, wiring: &mut RustWiring) {
    section(ui, "Channels (std::sync::mpsc)");
    let mut remove: Option<usize> = None;
    for (i, ch) in wiring.channels.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("name").small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut ch.name)
                    .hint_text("progress")
                    .desired_width(110.0),
            );
            ui.label(egui::RichText::new("type").small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut ch.ty)
                    .hint_text("f32")
                    .desired_width(90.0),
            );
            if ui.small_button("✕").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        wiring.channels.remove(i);
    }
    if ui.small_button("+ Add channel").clicked() {
        wiring.channels.push(ChannelDef {
            id: Uuid::new_v4(),
            name: format!("chan_{}", wiring.channels.len() + 1),
            ty: "f32".to_owned(),
        });
    }
    if !wiring.channels.is_empty() {
        let pairs = crate::codegen::rust_wiring::channel_field_pairs(wiring);
        let preview: String = pairs
            .iter()
            .map(|(d, _)| d.trim().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        code_preview(ui, "rwch_prev", &preview);
    }
}

// ---------------------------------------------------------------------------
// Iterator pipelines
// ---------------------------------------------------------------------------

fn show_iterators(ui: &mut egui::Ui, wiring: &mut RustWiring) {
    section(ui, "Iterator Pipelines");
    let mut remove: Option<usize> = None;
    for (i, p) in wiring.iterators.iter_mut().enumerate() {
        ui.push_id(p.id, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("fn").small().weak());
                ui.add(
                    egui::TextEdit::singleline(&mut p.name)
                        .hint_text("active_users")
                        .desired_width(120.0),
                );
                if ui.small_button("✕ remove").clicked() {
                    remove = Some(i);
                }
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("source").small().weak());
                ui.add(
                    egui::TextEdit::singleline(&mut p.source)
                        .hint_text("self.state.items")
                        .desired_width(220.0),
                );
            });

            let mut op_remove: Option<usize> = None;
            for (j, op) in p.ops.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let (tag, expr) = match op {
                        IterOp::Map(e) => ("map", e),
                        IterOp::Filter(e) => ("filter", e),
                    };
                    ui.label(
                        egui::RichText::new(format!(".{tag}(|x|"))
                            .small()
                            .monospace(),
                    );
                    ui.add(
                        egui::TextEdit::singleline(expr)
                            .hint_text("x.active")
                            .desired_width(180.0),
                    );
                    ui.label(egui::RichText::new(")").small().monospace());
                    if ui.small_button("✕").clicked() {
                        op_remove = Some(j);
                    }
                });
            }
            if let Some(j) = op_remove {
                p.ops.remove(j);
            }
            ui.horizontal(|ui| {
                if ui.small_button("+ map").clicked() {
                    p.ops.push(IterOp::Map("x".to_owned()));
                }
                if ui.small_button("+ filter").clicked() {
                    p.ops.push(IterOp::Filter("true".to_owned()));
                }
            });
        });
        ui.separator();
    }
    if let Some(i) = remove {
        wiring.iterators.remove(i);
    }
    if ui.small_button("+ Add pipeline").clicked() {
        wiring.iterators.push(IteratorPipeline {
            id: Uuid::new_v4(),
            name: format!("pipeline_{}", wiring.iterators.len() + 1),
            source: "self.state.items".to_owned(),
            ops: vec![],
        });
    }
    if !wiring.iterators.is_empty() {
        let preview = crate::codegen::rust_wiring::iterator_methods(wiring, "");
        code_preview(ui, "rwiter_prev", &preview);
    }
}

// ---------------------------------------------------------------------------
// Trait impls
// ---------------------------------------------------------------------------

fn show_trait_impls(ui: &mut egui::Ui, wiring: &mut RustWiring) {
    section(ui, "Trait Impls");
    let mut remove: Option<usize> = None;
    for (i, t) in wiring.trait_impls.iter_mut().enumerate() {
        ui.push_id(t.id, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("impl").small().weak());
                ui.add(
                    egui::TextEdit::singleline(&mut t.trait_name)
                        .hint_text("MyBehavior")
                        .desired_width(130.0),
                );
                ui.label(egui::RichText::new("for ExportedApp").small().weak());
                if ui.small_button("✕ remove").clicked() {
                    remove = Some(i);
                }
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("method").small().weak());
                ui.add(
                    egui::TextEdit::singleline(&mut t.method)
                        .hint_text("fn run(&mut self)")
                        .desired_width(240.0),
                );
            });
            ui.label(egui::RichText::new("body").small().weak());
            ui.add(
                egui::TextEdit::multiline(&mut t.body)
                    .font(egui::FontId::monospace(11.0))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
        });
        ui.separator();
    }
    if let Some(i) = remove {
        wiring.trait_impls.remove(i);
    }
    if ui.small_button("+ Add trait impl").clicked() {
        wiring.trait_impls.push(TraitImpl {
            id: Uuid::new_v4(),
            trait_name: "MyTrait".to_owned(),
            method: "fn run(&mut self)".to_owned(),
            body: "// body".to_owned(),
        });
    }
    if !wiring.trait_impls.is_empty() {
        let preview = crate::codegen::rust_wiring::trait_impl_blocks(wiring);
        code_preview(ui, "rwtrait_prev", &preview);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title).strong());
    ui.separator();
}

fn code_preview(ui: &mut egui::Ui, salt: &str, code: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new("generated:").small().weak());
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .max_height(80.0)
        .show(ui, |ui| {
            let mut s = code.to_owned();
            ui.add(
                egui::TextEdit::multiline(&mut s)
                    .font(egui::FontId::monospace(10.5))
                    .desired_width(f32::INFINITY)
                    .interactive(false),
            );
        });
}
