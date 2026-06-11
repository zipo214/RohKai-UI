use crate::codegen::formula::{collect_variables, emit_formula_rust, parse_formula};
use crate::codegen::{
    field_collector,
    rust::{field_binding, string_literal},
};
use crate::project::{
    schema::{SizePolicy, WidgetEvent, WidgetInstance, WidgetKind},
    ui_tree::UiTree,
};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use uuid::Uuid;

const SVG_RASTERIZER_SOURCE: &str = include_str!("../canvas/svg_rasterizer.rs");
const SVG_CORE_SOURCE: &str = include_str!("../svg_core.rs");
const MAX_GRID_COLUMNS: usize = 12;

/// Write a complete compilable Rust project to `dest` folder.
pub fn write_project(tree: &UiTree, dest: &Path) -> Result<(), String> {
    let src_dir = dest.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| format!("create dirs: {e}"))?;

    let files = project_files(tree);

    // Ensure all parent directories exist before parallel writes.
    for (rel_path, _) in &files {
        let full = dest.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create dirs: {e}"))?;
        }
    }

    files
        .par_iter()
        .map(|(rel_path, content)| {
            fs::write(dest.join(rel_path), content).map_err(|e| format!("{rel_path}: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}

/// Generate the exported project as an in-memory list of
/// `(relative_path, file_contents)` pairs — the single source of truth for both
/// disk export (`write_project`) and the in-app project-tree viewer.
///
/// Asset entries declared in `AppProps.assets` are surfaced as an `assets/`
/// manifest so the viewer and the exported project agree on referenced files.
pub fn project_files(tree: &UiTree) -> Vec<(String, String)> {
    // Collect unique Cargo dependency lines from Custom widgets
    let mut extra_deps: Vec<String> = Vec::new();
    let mut seen_deps: HashSet<String> = HashSet::new();
    for w in &tree.widgets {
        for dep_line in &w.descriptor_cargo_deps {
            if seen_deps.insert(dep_line.clone()) {
                extra_deps.push(dep_line.clone());
            }
        }
    }
    if tree
        .widgets
        .iter()
        .any(|w| w.kind == WidgetKind::FilePicker)
    {
        let dep_line = String::from("rfd = \"0.14\"");
        if seen_deps.insert(dep_line.clone()) {
            extra_deps.push(dep_line);
        }
    }

    // Generate the three core output files in parallel.
    let ((cargo_toml, main_rs), app_rs) = rayon::join(
        || rayon::join(|| gen_cargo_toml(&extra_deps), || gen_main_rs(tree)),
        || gen_app_rs(tree),
    );
    let mut files = vec![
        ("Cargo.toml".to_owned(), cargo_toml),
        ("src/main.rs".to_owned(), main_rs),
        ("src/app.rs".to_owned(), app_rs),
    ];

    if !tree.app_props.assets.is_empty() {
        files.push(("assets/MANIFEST.txt".to_owned(), gen_asset_manifest(tree)));
    }

    files
}

/// Write a WASM-compatible Rust project to `dest` folder.
///
/// The generated project can be built with:
/// `cargo build --target wasm32-unknown-unknown --release`
/// or bundled with trunk: `trunk build`
///
/// FilePicker widgets are replaced with a static label stub since native
/// file dialogs are unavailable in WASM.
pub fn write_project_wasm(tree: &UiTree, dest: &Path, gen_index_html: bool) -> Result<(), String> {
    let src_dir = dest.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| format!("create dirs: {e}"))?;

    let files = project_files_wasm(tree, gen_index_html);

    for (rel_path, _) in &files {
        let full = dest.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create dirs: {e}"))?;
        }
    }

    files
        .par_iter()
        .map(|(rel_path, content)| {
            fs::write(dest.join(rel_path), content).map_err(|e| format!("{rel_path}: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}

/// Generate WASM-compatible project files in memory.
///
/// Returns `(relative_path, content)` pairs.  FilePicker widgets are replaced
/// with a static stub since `rfd` requires native OS dialogs.
pub fn project_files_wasm(tree: &UiTree, gen_index_html: bool) -> Vec<(String, String)> {
    let has_file_picker = tree
        .widgets
        .iter()
        .any(|w| w.kind == WidgetKind::FilePicker);

    let ((cargo_toml, lib_rs), app_rs) = rayon::join(
        || rayon::join(gen_cargo_toml_wasm, || gen_lib_rs_wasm(tree)),
        || gen_app_rs(tree),
    );

    let mut files = vec![
        ("Cargo.toml".to_owned(), cargo_toml),
        ("src/lib.rs".to_owned(), lib_rs),
        ("src/app.rs".to_owned(), app_rs),
    ];

    if gen_index_html {
        files.push(("index.html".to_owned(), gen_index_html_wasm(tree)));
        files.push(("Trunk.toml".to_owned(), gen_trunk_toml()));
    }

    if has_file_picker {
        files.push((
            "WASM_NOTES.txt".to_owned(),
            "FilePicker widgets are stubbed in this WASM build.\n\
             rfd native file dialogs are not available in the browser.\n\
             Replace FilePicker usages with <input type=\"file\"> via web_sys if needed.\n"
                .to_owned(),
        ));
    }

    files
}

fn gen_cargo_toml_wasm() -> String {
    r#"[package]
name = "exported_app"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
eframe = { version = "0.29", default-features = false, features = ["glow", "wasm-bindgen"] }
egui   = "0.29"
wasm-bindgen-futures = "0.4"

[profile.release]
opt-level = "s"
"#
    .to_owned()
}

fn gen_lib_rs_wasm(tree: &UiTree) -> String {
    let title = string_literal(&tree.app_props.title);
    format!(
        r#"mod app;
use app::ExportedApp;

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{{self, prelude::*}};

/// Entry point for the browser.  Called automatically by the bundler.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start() -> Result<(), wasm_bindgen::JsValue> {{
    let web_options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(
            "the_canvas_id",
            web_options,
            Box::new(|_cc| Ok(Box::new(ExportedApp::default()))),
        )
        .await
        .map_err(|e| e.into())
}}

/// Native entry point (non-WASM builds still compile as a library).
#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> eframe::Result<()> {{
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        {title},
        options,
        Box::new(|_cc| Ok(Box::new(ExportedApp::default()))),
    )
}}
"#
    )
}

fn gen_index_html_wasm(tree: &UiTree) -> String {
    let title = html_escape(&tree.app_props.title);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>{title}</title>
    <style>
      html, body {{
        overflow: hidden;
        width: 100%;
        height: 100%;
        margin: 0;
        padding: 0;
        background: #1a1a1a;
      }}
      canvas {{
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
      }}
    </style>
  </head>
  <body>
    <canvas id="the_canvas_id"></canvas>
  </body>
</html>
"#
    )
}

fn gen_trunk_toml() -> String {
    r#"[build]
target = "index.html"
dist = "dist"
"#
    .to_owned()
}

/// Minimal HTML-entity escaping for title text embedded in index.html.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build the `assets/MANIFEST.txt` listing declared asset references.
fn gen_asset_manifest(tree: &UiTree) -> String {
    let mut s = String::from("# Asset manifest — files referenced by this project.\n");
    s.push_str("# Copy the listed source files into this assets/ folder before building.\n\n");
    for a in &tree.app_props.assets {
        s.push_str(&format!("{}\t<- {}\n", a.name, a.source_path));
    }
    s
}

// ---------------------------------------------------------------------------

fn gen_cargo_toml(extra_deps: &[String]) -> String {
    let mut s = r#"[package]
name = "exported_app"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.29"
egui   = "0.29"
"#
    .to_owned();
    for dep in extra_deps {
        s.push_str(dep);
        s.push('\n');
    }
    s
}

fn gen_main_rs(tree: &UiTree) -> String {
    let title = string_literal(&tree.app_props.title);
    let w = tree.app_props.win_w as u32;
    let h = tree.app_props.win_h as u32;
    let resizable = tree.app_props.resizable;
    let min_chain = tree
        .app_props
        .min_size
        .map(|[mw, mh]| format!("\n            .with_min_inner_size([{mw:.0}.0, {mh:.0}.0])"))
        .unwrap_or_default();
    let max_chain = tree
        .app_props
        .max_size
        .map(|[mw, mh]| format!("\n            .with_max_inner_size([{mw:.0}.0, {mh:.0}.0])"))
        .unwrap_or_default();
    format!(
        r#"mod app;
use app::ExportedApp;

fn main() -> eframe::Result<()> {{
    let options = eframe::NativeOptions {{
        viewport: egui::ViewportBuilder::default()
            .with_title({title})
            .with_inner_size([{w}.0, {h}.0])
            .with_resizable({resizable}){min_chain}{max_chain},
        ..Default::default()
    }};
    eframe::run_native(
        {title},
        options,
        Box::new(|_cc| Ok(Box::new(ExportedApp::default()))),
    )
}}
"#
    )
}

// ---------------------------------------------------------------------------

fn gen_app_rs(tree: &UiTree) -> String {
    let has_images = tree.widgets.iter().any(|w| w.kind == WidgetKind::Image);
    let collected = field_collector::collect(tree);
    let fields = &collected.fields;

    // Collect unique handler names; detect conflicts where the same name is used
    // with different async/result modes across widgets.  First definition wins.
    // Done first so async task fields and the conflict header can be emitted.
    let mut handler_names: Vec<(String, crate::project::schema::HandlerResult, bool, bool)> =
        Vec::new(); // (name, result, is_async, has_conflict)
    let mut handler_index: HashMap<String, usize> = HashMap::new();
    for w in &tree.widgets {
        // Collect a handler for EVERY event Properties exposes for this kind (not
        // just the primary), reading each event's dedicated field.  This keeps
        // export in full parity with the schema event contract used by Properties.
        for &ev in w.kind.supported_events() {
            let Some(h) = event_field_handler(w, ev) else {
                continue;
            };
            if let Some(&idx) = handler_index.get(h) {
                let (_, ref existing_result, existing_async, ref mut has_conflict) =
                    handler_names[idx];
                if existing_async != w.async_handler || existing_result != &w.handler_result {
                    *has_conflict = true;
                }
            } else {
                let idx = handler_names.len();
                handler_index.insert(h.to_owned(), idx);
                handler_names.push((
                    h.to_owned(),
                    w.handler_result.clone(),
                    w.async_handler,
                    false,
                ));
            }
        }
    }
    // Call-site registry: maps each handler to its first-registered (result, async) mode.
    // All widgets sharing a handler name use this mode so call sites are consistent.
    let handler_registry: HashMap<String, (crate::project::schema::HandlerResult, bool)> =
        handler_names
            .iter()
            .map(|(h, r, is_async, _)| (h.clone(), (r.clone(), *is_async)))
            .collect();
    // Async handlers get a generated task contract (fields, launcher, worker, drain).
    let async_handlers: Vec<(String, crate::project::schema::HandlerResult)> = handler_names
        .iter()
        .filter(|(_, _, is_async, _)| *is_async)
        .map(|(h, r, _, _)| (h.clone(), r.clone()))
        .collect();
    let conflict_names: Vec<String> = handler_names
        .iter()
        .filter(|(_, _, _, has_conflict)| *has_conflict)
        .map(|(h, _, _, _)| h.clone())
        .collect();

    let mut s = String::from("// Generated by RohKai — do not edit manually\n");
    if !conflict_names.is_empty() {
        s.push_str("//\n// !! HANDLER CONFLICTS DETECTED — generated code may be inconsistent:\n");
        for name in &conflict_names {
            s.push_str(&format!(
                "//   - `{name}` is bound by multiple widgets with different async/error modes;\n\
                 //     the first definition wins. Align those widgets' \"Run async\" / \"Error mode\"\n\
                 //     settings to resolve.\n"
            ));
        }
        s.push_str("//\n");
    }
    s.push_str("\nuse eframe::egui;\n");
    if has_images {
        s.push_str("use std::collections::HashMap;\n");
    }
    s.push('\n');
    // AppState struct
    s.push_str("pub struct AppState {\n");
    for f in fields {
        s.push_str(&format!("    pub {}: {},\n", f.name, f.ty));
    }
    s.push_str("}\n\n");

    // Default impl
    s.push_str("impl Default for AppState {\n    fn default() -> Self {\n        Self {\n");
    for f in fields {
        s.push_str(&format!("            {}: {},\n", f.name, f.default_expr));
    }
    s.push_str("        }\n    }\n}\n\n");

    // ExportedApp
    let channel_pairs =
        crate::codegen::rust_wiring::channel_field_pairs(&tree.app_props.rust_wiring);
    s.push_str("pub struct ExportedApp {\n    pub state: AppState,\n");
    if has_images {
        s.push_str("    svg_textures: HashMap<&'static str, egui::TextureHandle>,\n");
    }
    for (decl, _) in &channel_pairs {
        s.push_str(decl);
        s.push('\n');
    }
    for (h, result) in &async_handlers {
        for decl in crate::codegen::rust_wiring::async_struct_fields(h, result) {
            s.push_str(&decl);
            s.push('\n');
        }
    }
    s.push_str("}\n\n");
    // ExportedApp::default — channels need explicit construction (not Default-able),
    // so when channels (or async tasks) exist we build them in a `fn default()` body.
    let needs_default_body = !channel_pairs.is_empty() || !async_handlers.is_empty();
    if !needs_default_body {
        s.push_str("impl Default for ExportedApp {\n    fn default() -> Self {\n        Self {\n            state: AppState::default(),\n");
        if has_images {
            s.push_str("            svg_textures: HashMap::new(),\n");
        }
        s.push_str("        }\n    }\n}\n\n");
    } else {
        s.push_str("impl Default for ExportedApp {\n    fn default() -> Self {\n");
        for (_, init) in &channel_pairs {
            if !init.is_empty() {
                s.push_str(init);
                s.push('\n');
            }
        }
        s.push_str("        Self {\n            state: AppState::default(),\n");
        if has_images {
            s.push_str("            svg_textures: HashMap::new(),\n");
        }
        for ch in &tree.app_props.rust_wiring.channels {
            let name = ch
                .name
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect::<String>()
                .to_lowercase();
            s.push_str(&format!("            {name}_tx,\n            {name}_rx,\n"));
        }
        for (h, result) in &async_handlers {
            for init in crate::codegen::rust_wiring::async_default_fields(h, result) {
                s.push_str(&init);
                s.push('\n');
            }
        }
        s.push_str("        }\n    }\n}\n\n");
    }

    // eframe::App impl
    s.push_str("impl eframe::App for ExportedApp {\n    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {\n");
    // Drain completed async tasks first so UI reflects fresh status this frame.
    for (h, result) in &async_handlers {
        s.push_str(&crate::codegen::rust_wiring::async_drain_block(h, result));
    }
    // Schedule a repaint while any task is still in flight so the UI stays
    // responsive without requiring user interaction.
    if !async_handlers.is_empty() {
        s.push_str(&crate::codegen::rust_wiring::async_repaint_block(
            &async_handlers,
            "        ",
        ));
    }
    s.push_str(&gen_theme_setup(&tree.app_props.theme));
    s.push_str("        egui::CentralPanel::default().show(ctx, |_ui| {});\n");

    let child_ids: HashSet<Uuid> = tree
        .widgets
        .iter()
        .flat_map(|w| w.children.iter().copied())
        .collect();

    for w in &tree.widgets {
        if child_ids.contains(&w.id) {
            continue;
        }
        let label = string_literal(&w.props.label);
        let label_expr = if let Some(ref lb) = w.label_binding {
            field_binding(Some(lb.as_str()))
                .map(|b| format!("&self.state.{b}"))
                .unwrap_or_else(|| label.clone())
        } else {
            label.clone()
        };
        let binding = field_binding(w.state_binding.as_deref());
        let area_id = string_literal(&format!("widget_{}", w.id));
        s.push_str(&format!(
            "        egui::Area::new(egui::Id::new({area_id}))\n            .fixed_pos(egui::pos2({:.1}, {:.1}))\n            .show(ctx, |ui| {{\n                ui.set_min_size(egui::vec2({:.1}, {:.1}));\n",
            w.rect.x, w.rect.y, w.rect.w, w.rect.h
        ));

        // enabled
        if w.enabled == Some(false) {
            s.push_str("                ui.set_enabled(false);\n");
        }

        // widget line
        let tip = w.tooltip.as_deref().map(string_literal);
        let line = match &w.kind {
            WidgetKind::Button => {
                let rounding_chain = w
                    .corner_radius
                    .filter(|&r| r > 0.0)
                    .map(|r| format!(".rounding(egui::Rounding::same({r:.1}))"))
                    .unwrap_or_default();
                let fill_chain = w
                    .bg_color
                    .map(|c| {
                        format!(
                            ".fill(egui::Color32::from_rgb({}, {}, {}))",
                            c[0], c[1], c[2]
                        )
                    })
                    .unwrap_or_default();
                let btn_label = if w.label_binding.is_some() {
                    label_expr.clone()
                } else {
                    rich_text_export_expr(&label, w.font_size, w.fg_color)
                };
                let base = format!(
                    "ui.add_sized([{:.1}, {:.1}], egui::Button::new({btn_label}){rounding_chain}{fill_chain})",
                    w.rect.w, w.rect.h
                );
                let with_tip = export_tip(base, tip.as_deref());
                event_dispatch_block(w, &with_tip, &handler_registry)
            }
            WidgetKind::Label => {
                let expr = match binding {
                    Some(b) => format!("ui.label(&self.state.{b})"),
                    None => {
                        let text = if w.label_binding.is_some() {
                            label_expr.clone()
                        } else {
                            rich_text_export_expr(&label, w.font_size, w.fg_color)
                        };
                        let mut lbl = format!("egui::Label::new({text})");
                        if let Some(wrap) = w.props.text_wrap {
                            if wrap {
                                lbl.push_str(".wrap()");
                            } else {
                                lbl.push_str(".extend()");
                            }
                        }
                        format!("ui.add({lbl})")
                    }
                };
                format!("                {};\n", export_tip(expr, tip.as_deref()))
            }
            WidgetKind::TextInput => match binding {
                Some(b) => {
                    let mut te = format!("egui::TextEdit::singleline(&mut self.state.{b})");
                    if !w.props.placeholder.is_empty() {
                        te.push_str(&format!(
                            ".hint_text({})",
                            string_literal(&w.props.placeholder)
                        ));
                    }
                    if w.props.password_mode {
                        te.push_str(".password(true)");
                    }
                    let base = format!("ui.add_sized([{:.1}, {:.1}], {te})", w.rect.w, w.rect.h);
                    let with_tip = export_tip(base, tip.as_deref());
                    event_dispatch_block(w, &with_tip, &handler_registry)
                }
                None => format!("                // TextInput {label}: set a valid Binding\n"),
            },
            WidgetKind::Slider => match binding {
                Some(b) => {
                    let mut slider = format!(
                        "egui::Slider::new(&mut self.state.{b}, {:.1}..={:.1}).text({label})",
                        w.props.min, w.props.max
                    );
                    if let Some(step) = w.props.step {
                        slider.push_str(&format!(".step_by({step} as f64)"));
                    }
                    if !w.props.show_value {
                        slider.push_str(".show_value(false)");
                    }
                    if w.props.orientation == crate::project::schema::Orientation::Vertical {
                        slider.push_str(".vertical()");
                    }
                    let base =
                        format!("ui.add_sized([{:.1}, {:.1}], {slider})", w.rect.w, w.rect.h);
                    let with_tip = export_tip(base, tip.as_deref());
                    event_dispatch_block(w, &with_tip, &handler_registry)
                }
                None => format!("                // Slider {label}: set a valid Binding\n"),
            },
            WidgetKind::Checkbox => match binding {
                Some(b) => {
                    let base = format!(
                        "ui.add_sized([{:.1}, {:.1}], egui::Checkbox::new(&mut self.state.{b}, {label}))",
                        w.rect.w, w.rect.h
                    );
                    let with_tip = export_tip(base, tip.as_deref());
                    event_dispatch_block(w, &with_tip, &handler_registry)
                }
                None => format!("                // Checkbox {label}: set a valid Binding\n"),
            },
            WidgetKind::Frame => {
                let inner_m = w.props.inner_margin;
                let stroke_w = w.props.stroke_width;
                let stroke_col = w
                    .props
                    .stroke_color
                    .map(|c| format!("egui::Color32::from_rgb({}, {}, {})", c[0], c[1], c[2]))
                    .unwrap_or_else(|| "egui::Color32::from_gray(100)".to_owned());
                let mut frame_expr = format!(
                    "egui::Frame::none()\n                    .inner_margin({inner_m:.1})\n                    .stroke(egui::Stroke::new({stroke_w:.1}, {stroke_col}))"
                );
                if let Some(c) = w.bg_color {
                    frame_expr.push_str(&format!(
                        "\n                    .fill(egui::Color32::from_rgb({}, {}, {}))",
                        c[0], c[1], c[2]
                    ));
                }
                if let Some(r) = w.corner_radius.filter(|&r| r > 0.0) {
                    frame_expr.push_str(&format!(
                        "\n                    .rounding(egui::Rounding::same({r:.1}))"
                    ));
                }
                let mut code = format!(
                    "                {frame_expr}\n                    .show(ui, |ui| {{\n                        ui.set_min_size(egui::vec2({:.1}, {:.1}));\n",
                    w.rect.w, w.rect.h
                );
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        let rel_x = (child.rect.x - w.rect.x).max(0.0);
                        let rel_y = (child.rect.y - w.rect.y).max(0.0);
                        let rect_expr = format!(
                            "egui::Rect::from_min_size(ui.min_rect().min + egui::vec2({rel_x:.1}, {rel_y:.1}), egui::vec2({:.1}, {:.1}))",
                            child.rect.w, child.rect.h
                        );
                        let child_label = string_literal(&child.props.label);
                        let child_binding = field_binding(child.state_binding.as_deref());
                        code.push_str(&export_child_line(
                            child,
                            &rect_expr,
                            &child_label,
                            child_binding,
                            &handler_registry,
                        ));
                    }
                }
                code.push_str("                    });\n");
                code
            }
            WidgetKind::ComboBox => match binding {
                Some(b) => {
                    let options = combo_option_values(w);
                    let selected_expr =
                        combo_selected_text_expr(&format!("self.state.{b}"), &options);
                    let mut code = format!(
                        "                let combo_resp = egui::ComboBox::from_label({label})\n                    .selected_text({selected_expr})\n                    .width({:.1})\n                    .show_ui(ui, |ui| {{\n",
                        w.rect.w
                    );
                    for option in options {
                        let option_lit = string_literal(&option);
                        code.push_str(&format!(
                            "                        ui.selectable_value(&mut self.state.{b}, {option_lit}.to_owned(), {option_lit});\n"
                        ));
                    }
                    code.push_str("                    });\n");
                    let combo_handler = resolve_export_handler_change(w);
                    let uses_response = tip.is_some() || combo_handler.is_some();
                    if uses_response {
                        code.push_str(
                            "                let combo_response = combo_resp.response;\n",
                        );
                    }
                    if combo_handler.is_some() {
                        code.push_str(
                            "                let combo_changed = combo_response.changed();\n",
                        );
                    }
                    if let Some(tip) = tip.as_deref() {
                        code.push_str(&format!(
                            "                combo_response.on_hover_text({tip});\n"
                        ));
                    }
                    if let Some(h) = combo_handler {
                        let (reg_result, reg_async) = handler_registry
                            .get(h)
                            .cloned()
                            .unwrap_or((w.handler_result.clone(), w.async_handler));
                        let call = crate::codegen::rust_wiring::handler_call(
                            h,
                            reg_async,
                            &reg_result,
                            "                    ",
                        );
                        code.push_str(&format!(
                            "                if combo_changed {{\n{call}\n                }}\n"
                        ));
                    } else if !uses_response {
                        code.push_str("                let _ = combo_resp;\n");
                    }
                    code
                }
                None => format!("                // ComboBox {label}: set a valid Binding\n"),
            },
            WidgetKind::RadioButton => match binding {
                Some(b) => {
                    let value_lit = if w.props.radio_value.is_empty() {
                        label.clone()
                    } else {
                        string_literal(&w.props.radio_value)
                    };
                    let base = format!(
                        "ui.radio_value(&mut self.state.{b}, {value_lit}.to_owned(), {label})"
                    );
                    let with_tip = export_tip(base, tip.as_deref());
                    // radio_value marks the response changed on selection, so the
                    // shared dispatch's `.changed()` gate is correct here.
                    event_dispatch_block(w, &with_tip, &handler_registry)
                }
                None => format!("                // RadioButton {label}: set a valid Binding\n"),
            },
            WidgetKind::ProgressBar => match binding {
                Some(b) => {
                    let mut pb = format!("egui::ProgressBar::new(self.state.{b})");
                    if w.props.show_percentage {
                        pb.push_str(".show_percentage()");
                    }
                    if w.props.animated {
                        pb.push_str(".animate(true)");
                    }
                    if let Some(c) = w.fg_color {
                        pb.push_str(&format!(
                            ".fill(egui::Color32::from_rgb({}, {}, {}))",
                            c[0], c[1], c[2]
                        ));
                    }
                    let sized = format!("ui.add_sized([{:.1}, {:.1}], {pb})", w.rect.w, w.rect.h);
                    let with_tip = export_tip(sized, tip.as_deref());
                    format!("                {with_tip};\n")
                }
                None => format!("                // ProgressBar {label}: set a valid Binding\n"),
            },
            WidgetKind::TextArea => match binding {
                Some(b) => {
                    let mut te = format!("egui::TextEdit::multiline(&mut self.state.{b})");
                    if !w.props.placeholder.is_empty() {
                        te.push_str(&format!(
                            ".hint_text({})",
                            string_literal(&w.props.placeholder)
                        ));
                    }
                    let sized =
                        format!("ui.add_sized([{:.1}, {:.1}], {te})", w.rect.w, w.rect.h);
                    let with_tip = export_tip(sized, tip.as_deref());
                    event_dispatch_block(w, &with_tip, &handler_registry)
                }
                None => format!("                // TextArea {label}: set a valid Binding\n"),
            },
            WidgetKind::SpinBox => match binding {
                Some(b) => {
                    let dv = format!(
                        "egui::DragValue::new(&mut self.state.{b}).range({:.1}..={:.1})",
                        w.props.min, w.props.max
                    );
                    let with_tip = export_tip(format!("ui.add({dv})"), tip.as_deref());
                    event_dispatch_block(w, &with_tip, &handler_registry)
                }
                None => format!("                // SpinBox {label}: set a valid Binding\n"),
            },
            WidgetKind::FontComboBox => match binding {
                Some(b) => {
                    let font_handler = resolve_export_handler_change(w);
                    // When a handler is wired, bind the InnerResponse so we can read
                    // whether a selection changed this frame.
                    let assign = if font_handler.is_some() {
                        "let font_combo = "
                    } else {
                        ""
                    };
                    let mut code = format!(
                        "                {assign}egui::ComboBox::from_id_salt(\"{b}\")\n                    \
                        .selected_text(&self.state.{b})\n                    \
                        .show_ui(ui, |ui| {{\n                        \
                        let mut changed = false;\n                        \
                        for font in [\"Proportional\", \"Monospace\"] {{\n                            \
                        if ui.selectable_value(&mut self.state.{b}, font.to_owned(), font).changed() {{\n                                \
                        changed = true;\n                            \
                        }}\n                        \
                        }}\n                        \
                        changed\n                    \
                        }});\n"
                    );
                    if let Some(h) = font_handler {
                        let (reg_result, reg_async) = handler_registry
                            .get(h)
                            .cloned()
                            .unwrap_or((w.handler_result.clone(), w.async_handler));
                        let call = crate::codegen::rust_wiring::handler_call(
                            h,
                            reg_async,
                            &reg_result,
                            "                    ",
                        );
                        code.push_str(&format!(
                            "                if font_combo.inner == Some(true) {{\n{call}\n                }}\n"
                        ));
                    }
                    code
                }
                None => {
                    format!("                // FontComboBox {label}: set a valid Binding\n")
                }
            },
            WidgetKind::HorizontalSpacer => {
                format!("                ui.add_space({:.1});\n", w.rect.w)
            }
            WidgetKind::VerticalSpacer => {
                format!("                ui.add_space({:.1});\n", w.rect.h)
            }
            WidgetKind::GroupBox => {
                let lbl = string_literal(&w.props.label);
                format!(
                    "                egui::Frame::group(ui.style()).show(ui, |ui| {{\n                    ui.label({lbl});\n                }});\n"
                )
            }
            WidgetKind::VLayout => {
                use crate::project::schema::LayoutCrossAlign;
                let open = match w.props.layout_cross_align {
                    LayoutCrossAlign::Start => "                ui.vertical(|ui| {\n".to_owned(),
                    LayoutCrossAlign::Center => "                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {\n".to_owned(),
                    LayoutCrossAlign::End => "                ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {\n".to_owned(),
                };
                let mut code = open;
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        code.push_str(&export_layout_child_line(child, &handler_registry));
                    }
                }
                code.push_str("                });\n");
                code
            }
            WidgetKind::HLayout => {
                use crate::project::schema::LayoutCrossAlign;
                let open = match w.props.layout_cross_align {
                    LayoutCrossAlign::Start => "                ui.horizontal(|ui| {\n".to_owned(),
                    LayoutCrossAlign::Center => "                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {\n".to_owned(),
                    LayoutCrossAlign::End => "                ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {\n".to_owned(),
                };
                let mut code = open;
                for &child_id in &w.children {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        code.push_str(&export_layout_child_line(child, &handler_registry));
                    }
                }
                code.push_str("                });\n");
                code
            }
            WidgetKind::ScrollArea => {
                "                egui::ScrollArea::vertical().show(ui, |_ui| {});\n".to_string()
            }
            WidgetKind::GridLayout => {
                let columns = w.props.grid_columns.clamp(1, MAX_GRID_COLUMNS);
                let row_height_chain = w
                    .props
                    .grid_row_height
                    .map(|h| format!(".min_row_height({h:.1})"))
                    .unwrap_or_default();
                let mut code = format!(
                    "                egui::Grid::new(\"{}\"){row_height_chain}.show(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for (idx, &child_id) in w.children.iter().enumerate() {
                    if let Some(child) = tree.widgets.iter().find(|cw| cw.id == child_id) {
                        code.push_str(&export_layout_child_line(child, &handler_registry));
                        if (idx + 1) % columns == 0 {
                            code.push_str("                    ui.end_row();\n");
                        }
                    }
                }
                if !w.children.is_empty() && !w.children.len().is_multiple_of(columns) {
                    code.push_str("                    ui.end_row();\n");
                }
                code.push_str("                });\n");
                code
            }
            WidgetKind::TabWidget => {
                let mut s = format!(
                    "                egui::TopBottomPanel::top(\"{}_tabs\").show_inside(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for tab in &w.props.options {
                    s.push_str(&format!(
                        "                    ui.selectable_label(false, {});\n",
                        string_literal(tab)
                    ));
                }
                s.push_str("                });\n");
                s
            }
            WidgetKind::ToolButton => {
                format!(
                    "                if ui.small_button({}).clicked() {{}}\n",
                    string_literal(&w.props.label)
                )
            }
            WidgetKind::CommandLinkButton => {
                format!(
                    "                if ui.add_sized([{:.1}, {:.1}], egui::Button::new(format!(\"{{}}\\n{{}}\", {}, {}))).clicked() {{}}\n",
                    w.rect.w,
                    w.rect.h,
                    string_literal(&w.props.label),
                    string_literal(&w.props.placeholder)
                )
            }
            WidgetKind::DialogButtonBox => {
                let mut s = String::from("                ui.horizontal(|ui| {\n");
                for opt in &w.props.options {
                    s.push_str(&format!(
                        "                    if ui.button({}).clicked() {{}}\n",
                        string_literal(opt)
                    ));
                }
                s.push_str("                });\n");
                s
            }
            WidgetKind::MathLabel => {
                let label_lit = string_literal(&w.props.label);
                let decimals = w.props.formula_decimals;
                if !w.props.formula_expr.is_empty() {
                    match parse_formula(&w.props.formula_expr) {
                        Ok(node) => {
                            let vars = collect_variables(&node);
                            let rust_expr = emit_formula_rust(&node);
                            let binds: String = vars
                                .iter()
                                .map(|v| format!("                    let {v} = self.state.{v} as f64;\n"))
                                .collect();
                            format!(
                                "                ui.label(format!(\"{{}} = {{:.{decimals}}}\", {label_lit}, {{\n{binds}                    {rust_expr}\n                }}));\n"
                            )
                        }
                        Err(e) => format!("                // Formula parse error: {e}\n"),
                    }
                } else {
                    match binding {
                        Some(b) => format!(
                            "                ui.label(format!(\"{{}} = {{:.{decimals}}}\", {label_lit}, self.state.{b}));\n"
                        ),
                        None => format!("                // MathLabel {label}: set a valid Binding\n"),
                    }
                }
            }
            WidgetKind::FilePicker => match binding {
                Some(b) => format!(
                    "                if ui.button(\"Browse…\").clicked() {{\n                    \
                    if let Some(p) = rfd::FileDialog::new().pick_file() {{\n                        \
                    self.state.{b} = p.display().to_string();\n                    }}\n                \
                    }}\n                ui.label(&self.state.{b});\n"
                ),
                None => format!("                // FilePicker {label}: set a valid Binding\n"),
            },
            WidgetKind::Chart => match binding {
                Some(b) => chart_export_block(&format!("self.state.{b}"), w.rect.w, w.rect.h, 16),
                None => format!(
                    "                // Chart {label}: set a Vec<f32> Binding for painter output\n"
                ),
            },
            WidgetKind::Table => {
                let mut s = format!(
                    "                egui::Grid::new(\"{}\").striped(true).show(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for col in &w.props.options {
                    s.push_str(&format!(
                        "                    ui.label({});\n",
                        string_literal(col)
                    ));
                }
                s.push_str("                    ui.end_row();\n                });\n");
                s
            }
            WidgetKind::ListView => {
                let mut s = format!(
                    "                egui::ScrollArea::vertical().id_salt(\"{}\").show(ui, |ui| {{\n",
                    w.id.as_simple()
                );
                for item in &w.props.options {
                    s.push_str(&format!(
                        "                    ui.label({});\n",
                        string_literal(item)
                    ));
                }
                s.push_str("                });\n");
                s
            }
            WidgetKind::TreeView => {
                let root = w.props.options.first().cloned().unwrap_or_else(|| "Root".into());
                let mut s = format!(
                    "                egui::CollapsingHeader::new({}).default_open(true).show(ui, |ui| {{\n",
                    string_literal(&root)
                );
                for child in w.props.options.iter().skip(1) {
                    s.push_str(&format!(
                        "                    ui.label({});\n",
                        string_literal(child)
                    ));
                }
                s.push_str("                });\n");
                s
            }
            WidgetKind::StackedWidget => {
                "                ui.group(|_ui| {}); // StackedWidget\n".to_string()
            }
            WidgetKind::ToolBox => {
                let mut s = String::new();
                for sec in &w.props.options {
                    s.push_str(&format!(
                        "                egui::CollapsingHeader::new({}).show(ui, |_ui| {{}});\n",
                        string_literal(sec)
                    ));
                }
                s
            }
            WidgetKind::Image => image_export_line(w, tip.as_deref(), 16),
            WidgetKind::Custom(_) => {
                if let Some(ref tpl) = w.descriptor_export_tpl {
                    let rendered = crate::codegen::widget_descriptor::apply_template(
                        tpl,
                        w,
                        w.descriptor_name.as_deref().unwrap_or("Custom"),
                    );
                    format!("                {rendered}\n")
                } else {
                    format!(
                        "                // Custom widget {:?}: descriptor not loaded\n",
                        w.kind
                    )
                }
            }
        };
        s.push_str(&line);
        s.push_str("            });\n");
    }
    s.push_str("    }\n}\n");

    // Iterator-pipeline methods + handler stubs + Image helpers
    let iter_methods =
        crate::codegen::rust_wiring::iterator_methods(&tree.app_props.rust_wiring, "    ");
    if !handler_names.is_empty() || has_images || !iter_methods.is_empty() {
        s.push_str("\nimpl ExportedApp {\n");
        for (h, result, is_async, has_conflict) in &handler_names {
            if *has_conflict {
                s.push_str(&format!(
                    "    // CODEGEN CONFLICT: handler '{h}' is shared across widgets with \
                     different async/result modes; first definition wins — verify call sites.\n"
                ));
            }
            if *is_async {
                // Async: emit the launcher (spawns worker, stores receiver).
                s.push_str(&crate::codegen::rust_wiring::async_launcher_method(
                    h, result,
                ));
            } else {
                let sig = crate::codegen::rust_wiring::handler_signature(h, result);
                let body = crate::codegen::rust_wiring::handler_stub_body(result, h);
                s.push_str(&format!("    {sig} {{\n{body}\n    }}\n"));
            }
        }
        if !iter_methods.is_empty() {
            s.push_str(&iter_methods);
        }
        if has_images {
            s.push_str(
                "    fn show_svg_image(\n        &mut self,\n        ui: &mut egui::Ui,\n        ctx: &egui::Context,\n        key: &'static str,\n        svg_source: &'static str,\n        size: egui::Vec2,\n    ) -> egui::Response {\n        let size = egui::vec2(size.x.max(1.0), size.y.max(1.0));\n        let tex = self.svg_textures.entry(key).or_insert_with(|| {\n            let image = rohkai_svg::rasterize_or_fallback(svg_source, size.x.ceil() as u32, size.y.ceil() as u32);\n            ctx.load_texture(key, image, egui::TextureOptions::LINEAR)\n        });\n        ui.add(egui::Image::new((tex.id(), size)))\n    }\n",
            );
        }
        s.push_str("}\n");
    }

    // Module-level worker functions for async handlers (run off the UI thread).
    for (h, result) in &async_handlers {
        s.push('\n');
        s.push_str(&crate::codegen::rust_wiring::async_worker_fn(h, result));
    }

    if has_images {
        s.push_str("\n#[allow(dead_code)]\nmod svg_core {\n");
        s.push_str(SVG_CORE_SOURCE);
        if !SVG_CORE_SOURCE.ends_with('\n') {
            s.push('\n');
        }
        s.push_str("}\n");

        s.push_str("\n#[allow(dead_code)]\nmod rohkai_svg {\n");
        let embedded_rasterizer = SVG_RASTERIZER_SOURCE.replace(
            "use crate::svg_core::{self, Rgba};",
            "use super::svg_core::{self, Rgba};",
        );
        s.push_str(&embedded_rasterizer);
        if !embedded_rasterizer.ends_with('\n') {
            s.push('\n');
        }
        s.push_str("}\n");
    }

    // Stage 11 — user-authored trait impls.
    s.push_str(&crate::codegen::rust_wiring::trait_impl_blocks(
        &tree.app_props.rust_wiring,
    ));

    s
}

/// Chain `.on_hover_text(tip)` in export context.
fn export_tip(expr: String, tip: Option<&str>) -> String {
    match tip {
        Some(t) if !t.is_empty() => format!("{expr}.on_hover_text({t})"),
        _ => expr,
    }
}

fn combo_option_values(widget: &crate::project::schema::WidgetInstance) -> Vec<String> {
    let options: Vec<String> = widget
        .props
        .options
        .iter()
        .filter_map(|option| {
            let trimmed = option.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
        .collect();

    if options.is_empty() {
        vec![widget.props.label.clone()]
    } else {
        options
    }
}

fn combo_selected_text_expr(state_expr: &str, options: &[String]) -> String {
    let fallback = options.first().map(String::as_str).unwrap_or("Option A");
    let fallback_lit = string_literal(fallback);
    format!("if {state_expr}.is_empty() {{ {fallback_lit} }} else {{ {state_expr}.as_str() }}")
}

fn resolve_export_handler_click(w: &crate::project::schema::WidgetInstance) -> Option<&str> {
    crate::codegen::handlers::resolve_click_handler(w)
}

fn resolve_export_handler_change(w: &crate::project::schema::WidgetInstance) -> Option<&str> {
    crate::codegen::handlers::resolve_change_handler(w)
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// The handler name bound to a specific event on `w`, if any.  Maps each schema
/// [`WidgetEvent`] to its dedicated `WidgetInstance` field; Click/Change keep the
/// legacy `event_handler` fallback.  Single source used by both handler
/// collection and the per-widget dispatch block, so they cannot disagree.
fn event_field_handler(
    w: &crate::project::schema::WidgetInstance,
    ev: crate::project::schema::WidgetEvent,
) -> Option<&str> {
    use crate::project::schema::WidgetEvent;
    match ev {
        WidgetEvent::Click => resolve_export_handler_click(w),
        WidgetEvent::Change => resolve_export_handler_change(w),
        WidgetEvent::DoubleClick => non_empty(&w.on_double_click),
        WidgetEvent::LostFocus => non_empty(&w.on_lost_focus),
        WidgetEvent::DragStopped => non_empty(&w.on_drag_stopped),
    }
}

/// The egui `Response` predicate method that fires a given event.
fn event_egui_method(ev: crate::project::schema::WidgetEvent) -> &'static str {
    use crate::project::schema::WidgetEvent;
    match ev {
        WidgetEvent::Click => "clicked",
        WidgetEvent::DoubleClick => "double_clicked",
        WidgetEvent::Change => "changed",
        WidgetEvent::LostFocus => "lost_focus",
        WidgetEvent::DragStopped => "drag_stopped",
    }
}

/// Emit handler dispatch for a single-`Response` widget, covering **every**
/// supported event (primary + secondary).  Binds the response once, then emits
/// one `if evt_response.<method>() { <handler_call> }` per event that has a
/// handler set — every call routed through [`rust_wiring::handler_call`] using
/// the central registry mode.  When no handler is set, emits the widget as a
/// plain statement (no dangling binding).
fn event_dispatch_block(
    w: &crate::project::schema::WidgetInstance,
    resp_expr: &str,
    registry: &HashMap<String, (crate::project::schema::HandlerResult, bool)>,
) -> String {
    let mut arms: Vec<(&'static str, &str)> = Vec::new();
    for &ev in w.kind.supported_events() {
        if let Some(h) = event_field_handler(w, ev) {
            arms.push((event_egui_method(ev), h));
        }
    }
    if arms.is_empty() {
        return format!("                {resp_expr};\n");
    }
    let mut code = format!("                let evt_response = {resp_expr};\n");
    for (method, h) in arms {
        let (result, is_async) = registry
            .get(h)
            .cloned()
            .unwrap_or((w.handler_result.clone(), w.async_handler));
        let call =
            crate::codegen::rust_wiring::handler_call(h, is_async, &result, "                    ");
        code.push_str(&format!(
            "                if evt_response.{method}() {{\n{call}\n                }}\n"
        ));
    }
    code
}

fn rich_text_export_expr(
    label_lit: &str,
    font_size: Option<f32>,
    fg_color: Option<[u8; 3]>,
) -> String {
    let color = fg_color.map(|c| format!("egui::Color32::from_rgb({}, {}, {})", c[0], c[1], c[2]));
    match (font_size, color.as_deref()) {
        (Some(size), Some(col)) => {
            format!("egui::RichText::new({label_lit}).size({size:.1}).color({col})")
        }
        (Some(size), None) => format!("egui::RichText::new({label_lit}).size({size:.1})"),
        (None, Some(col)) => format!("egui::RichText::new({label_lit}).color({col})"),
        (None, None) => label_lit.to_owned(),
    }
}

fn chart_export_block(binding_expr: &str, width: f32, height: f32, indent: usize) -> String {
    let pad = " ".repeat(indent);
    format!(
        "{pad}let chart_size = egui::vec2({width:.1}, {height:.1});\n\
{pad}let (chart_rect, _) = ui.allocate_exact_size(chart_size, egui::Sense::hover());\n\
{pad}let chart_painter = ui.painter_at(chart_rect);\n\
{pad}chart_painter.rect_stroke(chart_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_gray(120)));\n\
{pad}let chart_values = &{binding_expr};\n\
{pad}if !chart_values.is_empty() {{\n\
{pad}    let chart_max = chart_values.iter().copied().fold(0.0_f32, f32::max).max(1.0);\n\
{pad}    let bar_w = chart_rect.width() / chart_values.len() as f32;\n\
{pad}    for (i, value) in chart_values.iter().enumerate() {{\n\
{pad}        let v = (*value).max(0.0) / chart_max;\n\
{pad}        let x0 = chart_rect.left() + i as f32 * bar_w + 2.0;\n\
{pad}        let x1 = (x0 + bar_w - 4.0).max(x0 + 1.0);\n\
{pad}        let y1 = chart_rect.bottom() - 2.0;\n\
{pad}        let y0 = y1 - (chart_rect.height() - 4.0) * v;\n\
{pad}        chart_painter.rect_filled(\n\
{pad}            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),\n\
{pad}            1.0,\n\
{pad}            egui::Color32::from_rgb(52, 211, 153),\n\
{pad}        );\n\
{pad}    }}\n\
{pad}}}\n"
    )
}

/// Emit handler dispatch for a single-`Response` nested child widget.  Mirrors
/// the top-level [`event_dispatch_block`] but at child indentation: binds the
/// `ui.put(...)`/`ui.radio_value(...)` response once, then one
/// `if child_response.<method>() { <handler_call> }` per wired event.  Every call
/// routes through [`rust_wiring::handler_call`] + the central registry.
fn export_child_event_dispatch(
    child: &crate::project::schema::WidgetInstance,
    resp_expr: &str,
    registry: &HashMap<String, (crate::project::schema::HandlerResult, bool)>,
) -> String {
    let mut arms: Vec<(&'static str, &str)> = Vec::new();
    for &ev in child.kind.supported_events() {
        if let Some(h) = event_field_handler(child, ev) {
            arms.push((event_egui_method(ev), h));
        }
    }
    if arms.is_empty() {
        return format!("                        {resp_expr};\n");
    }
    let mut code = format!("                        let child_response = {resp_expr};\n");
    for (method, h) in arms {
        let (result, is_async) = registry
            .get(h)
            .cloned()
            .unwrap_or((child.handler_result.clone(), child.async_handler));
        let call = crate::codegen::rust_wiring::handler_call(
            h,
            is_async,
            &result,
            "                            ",
        );
        code.push_str(&format!(
            "                        if child_response.{method}() {{\n{call}\n                        }}\n"
        ));
    }
    code
}

/// Emit an interactive nested-child combo (ComboBox / FontComboBox) at `rect`.
/// Renders a real `egui::ComboBox` (not a static label) so its `On Change`
/// handler can fire, gated on whether a selection changed this frame and routed
/// through [`rust_wiring::handler_call`].
fn export_child_combo(
    child: &crate::project::schema::WidgetInstance,
    rect_expr: &str,
    binding: &str,
    options: &[String],
    registry: &HashMap<String, (crate::project::schema::HandlerResult, bool)>,
) -> String {
    use crate::project::schema::WidgetEvent;
    let id = child.id.as_simple();
    let selected_expr = combo_selected_text_expr(&format!("self.state.{binding}"), options);
    let handler = event_field_handler(child, WidgetEvent::Change);
    let combo_assign = if handler.is_some() {
        "let child_combo = "
    } else {
        ""
    };
    let mut code = format!(
        "                        ui.allocate_ui_at_rect({rect_expr}, |ui| {{\n\
         \x20                           {combo_assign}egui::ComboBox::from_id_salt(\"child_combo_{id}\")\n\
         \x20                               .selected_text({selected_expr})\n\
         \x20                               .show_ui(ui, |ui| {{\n\
         \x20                                   let mut changed = false;\n"
    );
    for option in options {
        let option_lit = string_literal(option);
        code.push_str(&format!(
            "                                    if ui.selectable_value(&mut self.state.{binding}, {option_lit}.to_owned(), {option_lit}).changed() {{ changed = true; }}\n"
        ));
    }
    code.push_str("                                    changed\n");
    code.push_str("                                });\n");
    if let Some(h) = handler {
        let (result, is_async) = registry
            .get(h)
            .cloned()
            .unwrap_or((child.handler_result.clone(), child.async_handler));
        let call = crate::codegen::rust_wiring::handler_call(
            h,
            is_async,
            &result,
            "                                ",
        );
        code.push_str(&format!(
            "                            if child_combo.inner == Some(true) {{\n{call}\n                            }}\n"
        ));
    }
    code.push_str("                        });\n");
    code
}

fn export_child_line(
    child: &crate::project::schema::WidgetInstance,
    rect_expr: &str,
    child_label: &str,
    child_binding: Option<&str>,
    registry: &HashMap<String, (crate::project::schema::HandlerResult, bool)>,
) -> String {
    match &child.kind {
        WidgetKind::Button => {
            let resp = format!("ui.put({rect_expr}, egui::Button::new({child_label}))");
            export_child_event_dispatch(child, &resp, registry)
        }
        WidgetKind::Label => match child_binding {
            Some(b) => format!("                        ui.put({rect_expr}, egui::Label::new(&self.state.{b}));\n"),
            None => format!("                        ui.put({rect_expr}, egui::Label::new({child_label}));\n"),
        },
        WidgetKind::TextInput => match child_binding {
            Some(b) => {
                let resp = format!(
                    "ui.put({rect_expr}, egui::TextEdit::singleline(&mut self.state.{b}))"
                );
                export_child_event_dispatch(child, &resp, registry)
            }
            None => format!("                        // TextInput {child_label}: set a valid Binding\n"),
        },
        WidgetKind::Slider => match child_binding {
            Some(b) => {
                let resp = format!(
                    "ui.put({rect_expr}, egui::Slider::new(&mut self.state.{b}, {:.1}..={:.1}).text({child_label}))",
                    child.props.min, child.props.max
                );
                export_child_event_dispatch(child, &resp, registry)
            }
            None => format!("                        // Slider {child_label}: set a valid Binding\n"),
        },
        WidgetKind::Checkbox => match child_binding {
            Some(b) => {
                let resp = format!(
                    "ui.put({rect_expr}, egui::Checkbox::new(&mut self.state.{b}, {child_label}))"
                );
                export_child_event_dispatch(child, &resp, registry)
            }
            None => format!("                        // Checkbox {child_label}: set a valid Binding\n"),
        },
        WidgetKind::ComboBox => match child_binding {
            Some(b) => export_child_combo(child, rect_expr, b, &combo_option_values(child), registry),
            None => format!("                        // ComboBox {child_label}: set a valid Binding\n"),
        },
        WidgetKind::RadioButton => match child_binding {
            Some(b) => {
                let value_lit = if child.props.radio_value.is_empty() {
                    child_label.to_owned()
                } else {
                    string_literal(&child.props.radio_value)
                };
                let resp = format!(
                    "ui.radio_value(&mut self.state.{b}, {value_lit}.to_owned(), {child_label})"
                );
                export_child_event_dispatch(child, &resp, registry)
            }
            None => format!("                        // RadioButton {child_label}: set a valid Binding\n"),
        },
        WidgetKind::ProgressBar => match child_binding {
            Some(b) => {
                let mut pb = format!("egui::ProgressBar::new(self.state.{b})");
                if child.props.show_percentage {
                    pb.push_str(".show_percentage()");
                }
                if child.props.animated {
                    pb.push_str(".animate(true)");
                }
                format!("                        ui.put({rect_expr}, {pb});\n")
            }
            None => format!("                        // ProgressBar {child_label}: set a valid Binding\n"),
        },
        WidgetKind::Frame
        | WidgetKind::GroupBox
        | WidgetKind::VLayout
        | WidgetKind::HLayout
        | WidgetKind::ScrollArea
        | WidgetKind::GridLayout
        | WidgetKind::TabWidget
        | WidgetKind::StackedWidget
        | WidgetKind::ToolBox
        | WidgetKind::Table
        | WidgetKind::ListView
        | WidgetKind::TreeView
        | WidgetKind::Chart => format!(
            "                        // Nested container {:?} - not recursive in export\n",
            child.kind
        ),
        WidgetKind::ToolButton => format!(
            "                        if ui.put({rect_expr}, egui::Button::new({child_label}).small()).clicked() {{}}\n"
        ),
        WidgetKind::CommandLinkButton => format!(
            "                        if ui.put({rect_expr}, egui::Button::new({child_label})).clicked() {{}}\n"
        ),
        WidgetKind::DialogButtonBox => format!(
            "                        ui.put({rect_expr}, egui::Label::new({child_label})); // DialogButtonBox\n"
        ),
        WidgetKind::MathLabel => {
            let label_lit = string_literal(&child.props.label);
            let decimals = child.props.formula_decimals;
            if !child.props.formula_expr.is_empty() {
                match parse_formula(&child.props.formula_expr) {
                    Ok(node) => {
                        let vars = collect_variables(&node);
                        let rust_expr = emit_formula_rust(&node);
                        let binds: String = vars
                            .iter()
                            .map(|v| format!("                                let {v} = self.state.{v} as f64;\n"))
                            .collect();
                        format!(
                            "                        ui.put({rect_expr}, egui::Label::new(format!(\"{{}} = {{:.{decimals}}}\", {label_lit}, {{\n{binds}                                {rust_expr}\n                        }})));\n"
                        )
                    }
                    Err(e) => format!("                        // Formula parse error: {e}\n"),
                }
            } else {
                match child_binding {
                    Some(b) => format!(
                        "                        ui.put({rect_expr}, egui::Label::new(format!(\"{{}} = {{:.{decimals}}}\", {label_lit}, self.state.{b})));\n"
                    ),
                    None => format!("                        // MathLabel {child_label}: set a valid Binding\n"),
                }
            }
        }
        WidgetKind::FilePicker => match child_binding {
            Some(b) => format!(
                "                        ui.put({rect_expr}, egui::Label::new(&self.state.{b})); // FilePicker\n"
            ),
            None => format!("                        // FilePicker {child_label}: set a valid Binding\n"),
        },
        WidgetKind::TextArea => match child_binding {
            Some(b) => {
                let resp = format!(
                    "ui.put({rect_expr}, egui::TextEdit::multiline(&mut self.state.{b}))"
                );
                export_child_event_dispatch(child, &resp, registry)
            }
            None => format!("                        // TextArea {child_label}: set a valid Binding\n"),
        },
        WidgetKind::SpinBox => match child_binding {
            Some(b) => {
                let resp = format!(
                    "ui.put({rect_expr}, egui::DragValue::new(&mut self.state.{b}).range({:.1}..={:.1}))",
                    child.props.min, child.props.max
                );
                export_child_event_dispatch(child, &resp, registry)
            }
            None => format!("                        // SpinBox {child_label}: set a valid Binding\n"),
        },
        WidgetKind::FontComboBox => match child_binding {
            Some(b) => export_child_combo(
                child,
                rect_expr,
                b,
                &["Proportional".to_owned(), "Monospace".to_owned()],
                registry,
            ),
            None => format!("                        // FontComboBox {child_label}: set a valid Binding\n"),
        },
        WidgetKind::HorizontalSpacer => {
            format!("                        ui.add_space({:.1}); // HorizontalSpacer\n", child.rect.w)
        }
        WidgetKind::VerticalSpacer => {
            format!("                        ui.add_space({:.1}); // VerticalSpacer\n", child.rect.h)
        }
        WidgetKind::Image => image_export_child_line(child, rect_expr),
        WidgetKind::Custom(_) => {
            if let Some(ref tpl) = child.descriptor_export_tpl {
                let rendered = crate::codegen::widget_descriptor::apply_template(
                    tpl,
                    child,
                    child.descriptor_name.as_deref().unwrap_or("Custom"),
                );
                format!("                        {rendered}\n")
            } else {
                format!(
                    "                        // Custom child {:?}: descriptor not loaded\n",
                    child.kind
                )
            }
        }
    }
}

fn export_child_size_str(child: &WidgetInstance) -> String {
    match child.props.size_policy {
        SizePolicy::Fixed => format!("[{:.1}, {:.1}]", child.rect.w, child.rect.h),
        SizePolicy::FillWidth => format!("[ui.available_width(), {:.1}]", child.rect.h),
        SizePolicy::Fill => "ui.available_size()".to_owned(),
    }
}

/// Emit a child owned by an egui layout container.
///
/// Layout children are emitted sequentially inside the layout closure instead of
/// absolute-positioned with `ui.put`.
fn export_layout_child_line(
    child: &WidgetInstance,
    registry: &HashMap<String, (crate::project::schema::HandlerResult, bool)>,
) -> String {
    let child_label = string_literal(&child.props.label);
    let child_binding = field_binding(child.state_binding.as_deref());
    let mut code = format!("                    // widget_{}\n", child.id);
    match &child.kind {
        WidgetKind::Button => {
            let sz = export_child_size_str(child);
            let resp = format!("ui.add_sized({sz}, egui::Button::new({child_label}))");
            code.push_str(&export_child_event_dispatch(child, &resp, registry));
        }
        WidgetKind::Label => match child_binding {
            Some(b) => code.push_str(&format!("                    ui.label(&self.state.{b});\n")),
            None => code.push_str(&format!("                    ui.label({child_label});\n")),
        },
        WidgetKind::TextInput => match child_binding {
            Some(b) => {
                let sz = export_child_size_str(child);
                let resp =
                    format!("ui.add_sized({sz}, egui::TextEdit::singleline(&mut self.state.{b}))");
                code.push_str(&export_child_event_dispatch(child, &resp, registry));
            }
            None => code.push_str(&format!(
                "                    // TextInput {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::TextArea => match child_binding {
            Some(b) => {
                let sz = export_child_size_str(child);
                let resp =
                    format!("ui.add_sized({sz}, egui::TextEdit::multiline(&mut self.state.{b}))");
                code.push_str(&export_child_event_dispatch(child, &resp, registry));
            }
            None => code.push_str(&format!(
                "                    // TextArea {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::Slider => match child_binding {
            Some(b) => {
                let sz = export_child_size_str(child);
                let resp = format!(
                    "ui.add_sized({sz}, egui::Slider::new(&mut self.state.{b}, {:.1}..={:.1}).text({child_label}))",
                    child.props.min, child.props.max
                );
                code.push_str(&export_child_event_dispatch(child, &resp, registry));
            }
            None => code.push_str(&format!(
                "                    // Slider {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::SpinBox => match child_binding {
            Some(b) => {
                let resp = format!("ui.add(egui::DragValue::new(&mut self.state.{b}))");
                code.push_str(&export_child_event_dispatch(child, &resp, registry));
            }
            None => code.push_str(&format!(
                "                    // SpinBox {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::Checkbox => match child_binding {
            Some(b) => {
                let resp = format!("ui.checkbox(&mut self.state.{b}, {child_label})");
                code.push_str(&export_child_event_dispatch(child, &resp, registry));
            }
            None => code.push_str(&format!(
                "                    // Checkbox {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::RadioButton => match child_binding {
            Some(b) => {
                let value_lit = if child.props.radio_value.is_empty() {
                    child_label.clone()
                } else {
                    string_literal(&child.props.radio_value)
                };
                let resp = format!(
                    "ui.radio_value(&mut self.state.{b}, {value_lit}.to_owned(), {child_label})"
                );
                code.push_str(&export_child_event_dispatch(child, &resp, registry));
            }
            None => code.push_str(&format!(
                "                    // RadioButton {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::ComboBox => match child_binding {
            Some(b) => code.push_str(&export_layout_combo(child, b, registry)),
            None => code.push_str(&format!(
                "                    // ComboBox {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::FontComboBox => match child_binding {
            Some(b) => code.push_str(&export_layout_combo(child, b, registry)),
            None => code.push_str(&format!(
                "                    // FontComboBox {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::ProgressBar => match child_binding {
            Some(b) => {
                let percent = if child.props.show_percentage {
                    ".show_percentage()"
                } else {
                    ""
                };
                let sz = export_child_size_str(child);
                code.push_str(&format!(
                    "                    ui.add_sized({sz}, egui::ProgressBar::new(self.state.{b}){percent});\n"
                ));
            }
            None => code.push_str(&format!(
                "                    // ProgressBar {child_label}: set a valid Binding\n"
            )),
        },
        WidgetKind::HorizontalSpacer => code.push_str(&format!(
            "                    ui.add_space({:.1}); // HorizontalSpacer\n",
            child.rect.w
        )),
        WidgetKind::VerticalSpacer => code.push_str(&format!(
            "                    ui.add_space({:.1}); // VerticalSpacer\n",
            child.rect.h
        )),
        WidgetKind::Image => {
            let key = string_literal(&format!("svg_{}", child.id));
            let svg_source = raw_string_literal(child.svg_source.as_deref().unwrap_or(""));
            code.push_str(&format!(
                "                    self.show_svg_image(ui, ctx, {key}, {svg_source}, egui::vec2({:.1}, {:.1}));\n",
                child.rect.w, child.rect.h
            ));
        }
        WidgetKind::Custom(_) => {
            if let Some(ref tpl) = child.descriptor_export_tpl {
                code.push_str(&crate::codegen::widget_descriptor::apply_template(
                    tpl,
                    child,
                    child.descriptor_name.as_deref().unwrap_or("Custom"),
                ));
                code.push('\n');
            } else {
                code.push_str(&format!(
                    "                    // Custom child {:?}: descriptor not loaded\n",
                    child.kind
                ));
            }
        }
        _ => code.push_str(&format!(
            "                    // Layout child {:?}: sequential export not implemented yet\n",
            child.kind
        )),
    }
    code
}

fn export_layout_combo(
    child: &crate::project::schema::WidgetInstance,
    binding: &str,
    registry: &HashMap<String, (crate::project::schema::HandlerResult, bool)>,
) -> String {
    let options = combo_option_values(child);
    let selected_expr = combo_selected_text_expr(&format!("self.state.{binding}"), &options);
    let id = child.id.as_simple();
    let mut code = format!(
        "                    let child_combo = egui::ComboBox::from_id_salt(\"layout_combo_{id}\")\n                        .selected_text({selected_expr})\n                        .show_ui(ui, |ui| {{\n"
    );
    for option in options {
        let option_lit = string_literal(&option);
        code.push_str(&format!(
            "                            ui.selectable_value(&mut self.state.{binding}, {option_lit}.to_owned(), {option_lit});\n"
        ));
    }
    code.push_str("                        });\n");
    if let Some(h) = event_field_handler(child, WidgetEvent::Change) {
        let (result_mode, is_async) = registry
            .get(h)
            .cloned()
            .unwrap_or((child.handler_result.clone(), child.async_handler));
        let call = crate::codegen::rust_wiring::handler_call(
            h,
            is_async,
            &result_mode,
            "                        ",
        );
        code.push_str(&format!(
            "                    if child_combo.inner == Some(true) {{\n{call}\n                    }}\n"
        ));
    }
    code
}

fn image_export_line(
    widget: &crate::project::schema::WidgetInstance,
    tip: Option<&str>,
    indent: usize,
) -> String {
    let pad = " ".repeat(indent);
    let key = string_literal(&format!("svg_{}", widget.id));
    let svg_source = raw_string_literal(widget.svg_source.as_deref().unwrap_or(""));
    let base = format!(
        "self.show_svg_image(ui, ctx, {key}, {svg_source}, egui::vec2({:.1}, {:.1}))",
        widget.rect.w, widget.rect.h
    );
    let expr = export_tip(base, tip);
    format!("{pad}{expr};\n")
}

fn image_export_child_line(
    child: &crate::project::schema::WidgetInstance,
    rect_expr: &str,
) -> String {
    let key = string_literal(&format!("svg_{}", child.id));
    let svg_source = raw_string_literal(child.svg_source.as_deref().unwrap_or(""));
    format!(
        "                        ui.allocate_ui_at_rect({rect_expr}, |ui| {{\n                            self.show_svg_image(ui, ctx, {key}, {svg_source}, {rect_expr}.size());\n                        }});\n"
    )
}

fn gen_theme_setup(theme: &crate::project::schema::ThemeSettings) -> String {
    let mode = if theme.dark_mode {
        "egui::Visuals::dark()"
    } else {
        "egui::Visuals::light()"
    };
    let [r, g, b] = theme.accent_color;
    let is_default_accent = (r, g, b) == (52, 211, 153);
    let rounding_code = theme
        .global_corner_radius
        .map(|cr| {
            format!(
                "        let r = egui::Rounding::same({cr:.1});\n\
                 visuals.widgets.noninteractive.rounding = r;\n\
                 visuals.widgets.inactive.rounding = r;\n\
                 visuals.widgets.hovered.rounding = r;\n\
                 visuals.widgets.active.rounding = r;\n\
                 visuals.widgets.open.rounding = r;\n"
            )
        })
        .unwrap_or_default();
    let font_code = theme
        .base_font_size
        .map(|fs| {
            format!(
                "        let mut style = (*ctx.style()).clone();\n\
                 for font_id in style.text_styles.values_mut() {{ font_id.size = {fs:.1}; }}\n\
                 ctx.set_style(style);\n"
            )
        })
        .unwrap_or_default();
    // Skip emitting theme code if it's the default dark mode with default accent
    if theme.dark_mode
        && is_default_accent
        && theme.global_corner_radius.is_none()
        && font_code.is_empty()
    {
        return String::new();
    }
    format!(
        "        let mut visuals = {mode};\n\
         visuals.hyperlink_color = egui::Color32::from_rgb({r}, {g}, {b});\n\
         visuals.selection.bg_fill = egui::Color32::from_rgba_premultiplied({r}, {g}, {b}, 90);\n\
         {rounding_code}\
         ctx.set_visuals(visuals);\n\
         {font_code}"
    )
}

fn raw_string_literal(value: &str) -> String {
    let mut hashes = 0usize;
    loop {
        let fence = format!("\"{}", "#".repeat(hashes));
        if !value.contains(&fence) {
            return format!("r{}\"{}\"{}", "#".repeat(hashes), value, "#".repeat(hashes));
        }
        hashes += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Rect, WidgetInstance};

    #[test]
    fn image_widget_export_embeds_svg_renderer() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::Image,
                rect: Rect {
                    x: 10.0,
                    y: 20.0,
                    w: 120.0,
                    h: 80.0,
                },
                svg_source: Some("<svg/>".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);

        assert!(generated.contains("mod svg_core"));
        assert!(generated.contains("mod rohkai_svg"));
        assert!(generated.contains("fn show_svg_image"));
        assert!(generated.contains("use std::collections::HashMap;"));
        assert!(generated.contains("svg_textures: HashMap"));
        assert!(generated.contains("rohkai_svg::rasterize_or_fallback"));
        assert!(generated.contains("use super::svg_core::{self, Rgba};"));
        assert!(generated.contains("ui.add(egui::Image::new((tex.id(), size)))"));
        assert!(generated.contains("pub fn rasterize"));
        assert!(generated.contains("r\"<svg/>\""));
        assert!(!generated.contains("egui::Frame::none()"));
        assert!(!generated.contains("image_export_frame_placeholder_line"));
    }

    #[test]
    fn vlayout_exports_owned_children_sequentially_with_events() {
        let parent_id = Uuid::from_u128(0x91);
        let child_id = Uuid::from_u128(0x92);
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::VLayout,
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 90.0,
                        h: 28.0,
                    },
                    on_click: "layout_child_clicked".to_owned(),
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);

        assert_eq!(generated.matches("egui::Area::new").count(), 1);
        assert!(generated.contains("ui.vertical(|ui| {"));
        assert!(generated.contains(&format!("// widget_{child_id}")));
        assert!(generated.contains("let child_response = ui.add_sized"));
        assert!(generated.contains("child_response.clicked()"));
        assert!(generated.contains("if let Err(e) = self.layout_child_clicked()"));
        assert!(generated.contains("fn layout_child_clicked(&mut self) -> Result<(), String>"));
    }

    #[test]
    fn hlayout_exports_owned_children_sequentially_with_events() {
        let parent_id = Uuid::from_u128(0x93);
        let child_id = Uuid::from_u128(0x94);
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 90.0,
                        h: 28.0,
                    },
                    on_click: "horizontal_child_clicked".to_owned(),
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);

        assert_eq!(generated.matches("egui::Area::new").count(), 1);
        assert!(generated.contains("ui.horizontal(|ui| {"));
        assert!(generated.contains(&format!("// widget_{child_id}")));
        assert!(generated.contains("let child_response = ui.add_sized"));
        assert!(generated.contains("child_response.clicked()"));
        assert!(generated.contains("if let Err(e) = self.horizontal_child_clicked()"));
        assert!(generated.contains("fn horizontal_child_clicked(&mut self) -> Result<(), String>"));
    }

    #[test]
    fn gridlayout_exports_owned_children_row_major_with_events() {
        let parent_id = Uuid::from_u128(0x95);
        let child_ids = [
            Uuid::from_u128(0x96),
            Uuid::from_u128(0x97),
            Uuid::from_u128(0x98),
        ];
        let mut widgets = vec![WidgetInstance {
            id: parent_id,
            kind: WidgetKind::GridLayout,
            children: child_ids.to_vec(),
            props: crate::project::schema::WidgetProps {
                grid_columns: 2,
                ..Default::default()
            },
            ..Default::default()
        }];
        widgets.extend(
            child_ids
                .iter()
                .enumerate()
                .map(|(idx, id)| WidgetInstance {
                    id: *id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 90.0,
                        h: 28.0,
                    },
                    on_click: format!("grid_child_{idx}_clicked"),
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                }),
        );
        let tree = UiTree {
            widgets,
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);

        assert_eq!(generated.matches("egui::Area::new").count(), 1);
        assert!(generated.contains("egui::Grid::new"));
        assert_eq!(generated.matches("ui.end_row();").count(), 2);
        assert!(generated.contains("let child_response = ui.add_sized"));
        assert!(generated.contains("if let Err(e) = self.grid_child_0_clicked()"));
        assert!(generated.contains("fn grid_child_0_clicked(&mut self) -> Result<(), String>"));
        for child_id in child_ids {
            assert!(generated.contains(&format!("// widget_{child_id}")));
        }
    }

    #[test]
    fn hlayout_export_emits_reflowed_horizontal_spacer_space() {
        let parent_id = Uuid::from_u128(0x99);
        let left_id = Uuid::from_u128(0x9A);
        let spacer_id = Uuid::from_u128(0x9B);
        let right_id = Uuid::from_u128(0x9C);
        let mut tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: parent_id,
                    kind: WidgetKind::HLayout,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 300.0,
                        h: 60.0,
                    },
                    children: vec![left_id, spacer_id, right_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: left_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 50.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
                WidgetInstance {
                    id: spacer_id,
                    kind: WidgetKind::HorizontalSpacer,
                    ..Default::default()
                },
                WidgetInstance {
                    id: right_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 70.0,
                        h: 30.0,
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        tree.reflow_layouts();

        let generated = gen_app_rs(&tree);

        assert!(generated.contains("ui.horizontal(|ui| {"));
        assert!(generated.contains("ui.add_space(152.0); // HorizontalSpacer"));
    }

    fn async_button(handler: &str, result: crate::project::schema::HandlerResult) -> UiTree {
        UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::Button,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 80.0,
                    h: 30.0,
                },
                on_click: handler.to_owned(),
                async_handler: true,
                handler_result: result,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// A single value/selection widget with an `on_change` handler set.
    fn change_widget(
        kind: WidgetKind,
        binding: &str,
        handler: &str,
        is_async: bool,
        result: crate::project::schema::HandlerResult,
    ) -> UiTree {
        UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 140.0,
                    h: 28.0,
                },
                state_binding: Some(binding.to_owned()),
                on_change: handler.to_owned(),
                async_handler: is_async,
                handler_result: result,
                props: crate::project::schema::WidgetProps {
                    label: "L".to_owned(),
                    options: vec!["Proportional".to_owned(), "Monospace".to_owned()],
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// Build a one-widget tree whose `ev` handler field is set to `handler`.
    fn event_widget(
        kind: WidgetKind,
        binding: Option<&str>,
        ev: crate::project::schema::WidgetEvent,
        handler: &str,
        is_async: bool,
        result: crate::project::schema::HandlerResult,
    ) -> UiTree {
        use crate::project::schema::WidgetEvent;
        let mut w = WidgetInstance {
            id: Uuid::nil(),
            kind,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 28.0,
            },
            state_binding: binding.map(|s| s.to_owned()),
            async_handler: is_async,
            handler_result: result,
            props: crate::project::schema::WidgetProps {
                label: "L".to_owned(),
                options: vec!["A".to_owned(), "B".to_owned()],
                ..Default::default()
            },
            ..Default::default()
        };
        match ev {
            WidgetEvent::Click => w.on_click = handler.to_owned(),
            WidgetEvent::DoubleClick => w.on_double_click = handler.to_owned(),
            WidgetEvent::Change => w.on_change = handler.to_owned(),
            WidgetEvent::LostFocus => w.on_lost_focus = handler.to_owned(),
            WidgetEvent::DragStopped => w.on_drag_stopped = handler.to_owned(),
        }
        UiTree {
            widgets: vec![w],
            ..Default::default()
        }
    }

    /// The egui `Response` token export must emit to gate `ev` on `kind`.
    fn expected_gate(kind: &WidgetKind, ev: crate::project::schema::WidgetEvent) -> &'static str {
        use crate::project::schema::WidgetEvent;
        match ev {
            WidgetEvent::Click => ".clicked()",
            WidgetEvent::DoubleClick => ".double_clicked()",
            WidgetEvent::LostFocus => ".lost_focus()",
            WidgetEvent::DragStopped => ".drag_stopped()",
            // Change has kind-specific gates for the combo widgets.
            WidgetEvent::Change => match kind {
                WidgetKind::ComboBox => "combo_changed",
                WidgetKind::FontComboBox => "font_combo.inner == Some(true)",
                _ => ".changed()",
            },
        }
    }

    // INVARIANT: EVERY (kind, event) pair that `supported_events()` exposes must,
    // in export, route a handler through `rust_wiring::handler_call()` AND gate it
    // on the correct egui `Response` predicate.  Proven by Result mode (the
    // `if let Err(...)` wrapper that ONLY handler_call() emits — a bare
    // `self.h();` bypass cannot produce it).  Fails if any supported event lacks
    // export routing, including secondary events (DoubleClick/LostFocus/DragStopped).
    #[test]
    fn every_supported_event_is_exported_through_handler_call() {
        use crate::project::schema::{HandlerResult, EVENT_CAPABLE_KINDS};
        for kind in EVENT_CAPABLE_KINDS {
            for &ev in kind.supported_events() {
                let handler = "h_evt";
                let tree = event_widget(
                    kind.clone(),
                    Some("val"),
                    ev,
                    handler,
                    false,
                    HandlerResult::Result,
                );
                let g = gen_app_rs(&tree);
                assert!(
                    g.contains("if let Err(e) = self.h_evt()"),
                    "({kind:?}, {ev:?}) must route through handler_call() \
                     (Result `if let Err` wrapper missing). Generated:\n{g}"
                );
                let gate = expected_gate(&kind, ev);
                assert!(
                    g.contains(gate),
                    "({kind:?}, {ev:?}) must gate its handler on `{gate}`. Generated:\n{g}"
                );
            }
        }
    }

    /// Build a Frame parent containing a single child whose `ev` field is set.
    /// Both parent and child live in `tree.widgets`; the child is rendered only
    /// via `export_child_line` (skipped at top level).
    fn event_child_in_frame(
        kind: WidgetKind,
        binding: Option<&str>,
        ev: crate::project::schema::WidgetEvent,
        handler: &str,
        is_async: bool,
        result: crate::project::schema::HandlerResult,
    ) -> UiTree {
        use crate::project::schema::WidgetEvent;
        let frame_id = Uuid::from_u128(0x0F);
        let child_id = Uuid::from_u128(0xC1);
        let mut child = WidgetInstance {
            id: child_id,
            kind,
            rect: Rect {
                x: 10.0,
                y: 10.0,
                w: 120.0,
                h: 24.0,
            },
            state_binding: binding.map(|s| s.to_owned()),
            async_handler: is_async,
            handler_result: result,
            props: crate::project::schema::WidgetProps {
                label: "L".to_owned(),
                options: vec!["A".to_owned(), "B".to_owned()],
                ..Default::default()
            },
            ..Default::default()
        };
        match ev {
            WidgetEvent::Click => child.on_click = handler.to_owned(),
            WidgetEvent::DoubleClick => child.on_double_click = handler.to_owned(),
            WidgetEvent::Change => child.on_change = handler.to_owned(),
            WidgetEvent::LostFocus => child.on_lost_focus = handler.to_owned(),
            WidgetEvent::DragStopped => child.on_drag_stopped = handler.to_owned(),
        }
        let frame = WidgetInstance {
            id: frame_id,
            kind: WidgetKind::Frame,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 120.0,
            },
            children: vec![child_id],
            ..Default::default()
        };
        UiTree {
            widgets: vec![frame, child],
            ..Default::default()
        }
    }

    /// Gate token export must emit for a nested child's `ev`.  Child dispatch
    /// binds `child_response` (combos bind `child_combo`).
    fn expected_child_gate(
        kind: &WidgetKind,
        ev: crate::project::schema::WidgetEvent,
    ) -> &'static str {
        use crate::project::schema::WidgetEvent;
        match ev {
            WidgetEvent::Click => ".clicked()",
            WidgetEvent::DoubleClick => ".double_clicked()",
            WidgetEvent::LostFocus => ".lost_focus()",
            WidgetEvent::DragStopped => ".drag_stopped()",
            WidgetEvent::Change => match kind {
                WidgetKind::ComboBox | WidgetKind::FontComboBox => {
                    "child_combo.inner == Some(true)"
                }
                _ => ".changed()",
            },
        }
    }

    // INVARIANT (nested): EVERY (kind, event) pair must also route through
    // `handler_call()` in the nested/frame-child export path (`export_child_line`).
    // Mirrors the top-level invariant; fails if a child kind drops any event.
    #[test]
    fn every_supported_event_is_exported_in_nested_child() {
        use crate::project::schema::{HandlerResult, EVENT_CAPABLE_KINDS};
        for kind in EVENT_CAPABLE_KINDS {
            for &ev in kind.supported_events() {
                let tree = event_child_in_frame(
                    kind.clone(),
                    Some("val"),
                    ev,
                    "h_evt",
                    false,
                    HandlerResult::Result,
                );
                let g = gen_app_rs(&tree);
                assert!(
                    g.contains("if let Err(e) = self.h_evt()"),
                    "nested ({kind:?}, {ev:?}) must route through handler_call(). Generated:\n{g}"
                );
                let gate = expected_child_gate(&kind, ev);
                assert!(
                    g.contains(gate),
                    "nested ({kind:?}, {ev:?}) must gate on `{gate}`. Generated:\n{g}"
                );
                // Confirms the routing came from the child dispatch path.
                assert!(
                    g.contains("child_response") || g.contains("child_combo"),
                    "nested ({kind:?}, {ev:?}) must use the child dispatch path. Generated:\n{g}"
                );
            }
        }
    }

    #[test]
    fn nested_button_click_routes_through_handler_call() {
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::Button,
            None,
            crate::project::schema::WidgetEvent::Click,
            "on_c",
            false,
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("child_response.clicked()"));
        assert!(g.contains("if let Err(e) = self.on_c()"));
    }

    #[test]
    fn nested_button_double_click_routes_through_handler_call() {
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::Button,
            None,
            crate::project::schema::WidgetEvent::DoubleClick,
            "on_dc",
            false,
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("child_response.double_clicked()"));
        assert!(g.contains("if let Err(e) = self.on_dc()"));
    }

    #[test]
    fn nested_text_input_lost_focus_routes_through_handler_call() {
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::TextInput,
            Some("name"),
            crate::project::schema::WidgetEvent::LostFocus,
            "on_blur",
            false,
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("child_response.lost_focus()"));
        assert!(g.contains("if let Err(e) = self.on_blur()"));
    }

    #[test]
    fn nested_slider_drag_stopped_routes_through_handler_call_async() {
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::Slider,
            Some("vol"),
            crate::project::schema::WidgetEvent::DragStopped,
            "on_ds",
            true,
            crate::project::schema::HandlerResult::Plain,
        ));
        assert!(g.contains("child_response.drag_stopped()"));
        assert!(g.contains("self.on_ds();"));
        assert!(g.contains("fn on_ds_worker()"));
    }

    #[test]
    fn nested_spinbox_drag_stopped_routes_through_handler_call() {
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::SpinBox,
            Some("qty"),
            crate::project::schema::WidgetEvent::DragStopped,
            "on_sd",
            false,
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("child_response.drag_stopped()"));
        assert!(g.contains("if let Err(e) = self.on_sd()"));
    }

    #[test]
    fn nested_checkbox_change_routes_through_handler_call() {
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::Checkbox,
            Some("flag"),
            crate::project::schema::WidgetEvent::Change,
            "on_ch",
            false,
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("child_response.changed()"));
        assert!(g.contains("if let Err(e) = self.on_ch()"));
    }

    #[test]
    fn nested_combo_change_routes_through_interactive_combo() {
        // ComboBox child must render a REAL interactive combo (not a dead Label)
        // so On Change can fire.
        let g = gen_app_rs(&event_child_in_frame(
            WidgetKind::ComboBox,
            Some("sel"),
            crate::project::schema::WidgetEvent::Change,
            "on_sel",
            false,
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("egui::ComboBox::from_id_salt(\"child_combo_"));
        assert!(g.contains("child_combo.inner == Some(true)"));
        assert!(g.contains("if let Err(e) = self.on_sel()"));
        // No leftover dead-label placeholder.
        assert!(!g.contains("egui::Label::new(self.state.sel.as_str())); // ComboBox"));
    }

    #[test]
    fn conflict_between_top_level_and_nested_child_is_detected_and_normalized() {
        // Top-level Button (Click, async/Plain) registers first; nested child
        // Button (DoubleClick, sync/Result) reuses the name → conflict. Both call
        // sites normalize to the first definition (async Plain → launcher).
        let child_id = Uuid::from_u128(0xC1);
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: Uuid::from_u128(0x01),
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    on_click: "dup".to_owned(),
                    async_handler: true,
                    handler_result: crate::project::schema::HandlerResult::Plain,
                    ..Default::default()
                },
                WidgetInstance {
                    id: Uuid::from_u128(0x0F),
                    kind: WidgetKind::Frame,
                    rect: Rect {
                        x: 0.0,
                        y: 50.0,
                        w: 200.0,
                        h: 120.0,
                    },
                    children: vec![child_id],
                    ..Default::default()
                },
                WidgetInstance {
                    id: child_id,
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 10.0,
                        y: 60.0,
                        w: 80.0,
                        h: 24.0,
                    },
                    on_double_click: "dup".to_owned(),
                    async_handler: false,
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let g = gen_app_rs(&tree);
        // Conflict surfaced (covers nested children, not just top-level).
        assert!(g.contains("HANDLER CONFLICTS DETECTED"));
        assert!(g.contains("`dup`"));
        // Normalized to first def (async Plain): no Result `if let Err` wrapper,
        // both call sites invoke the launcher.
        assert!(!g.contains("if let Err(e) = self.dup()"));
        assert!(g.matches("self.dup();").count() >= 2);
        assert!(g.contains("fn dup_worker()"));
        // Top-level gate + nested gate both present.
        assert!(g.contains("evt_response.clicked()"));
        assert!(g.contains("child_response.double_clicked()"));
    }

    #[test]
    fn textarea_on_change_routes_through_handler_call_async() {
        let tree = change_widget(
            WidgetKind::TextArea,
            "note",
            "on_note",
            true,
            crate::project::schema::HandlerResult::Plain,
        );
        let g = gen_app_rs(&tree);
        // Gated change check present.
        assert!(g.contains(".changed()"));
        // Async path: launcher call + generated worker + receiver field.
        assert!(g.contains("self.on_note();"));
        assert!(g.contains("fn on_note_worker()"));
        assert!(g.contains("on_note_rx: Option<std::sync::mpsc::Receiver<()>>"));
        // Not the old hollow arm that emitted the widget with no handler call.
        assert!(!g.contains("// TextArea L: set a valid Binding"));
    }

    #[test]
    fn spinbox_on_change_routes_through_handler_call_result() {
        let tree = change_widget(
            WidgetKind::SpinBox,
            "qty",
            "on_qty",
            false,
            crate::project::schema::HandlerResult::Result,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains(".changed()"));
        // Result routing proves it is not a raw `self.on_qty();` bypass.
        assert!(g.contains("if let Err(e) = self.on_qty()"));
        assert!(g.contains("fn on_qty(&mut self) -> Result<(), String>"));
    }

    #[test]
    fn fontcombobox_on_change_routes_through_handler_call_result() {
        let tree = change_widget(
            WidgetKind::FontComboBox,
            "font",
            "on_font",
            false,
            crate::project::schema::HandlerResult::Result,
        );
        let g = gen_app_rs(&tree);
        // FontComboBox gates on its inner `changed` flag.
        assert!(g.contains("font_combo.inner == Some(true)"));
        assert!(g.contains("if let Err(e) = self.on_font()"));
    }

    #[test]
    fn fontcombobox_without_handler_has_no_dangling_binding() {
        // No handler: the combo must still emit and must NOT bind `font_combo`
        // (which would be an unused-variable warning in the exported crate).
        let tree = change_widget(
            WidgetKind::FontComboBox,
            "font",
            "",
            false,
            crate::project::schema::HandlerResult::Plain,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains("egui::ComboBox::from_id_salt(\"font\")"));
        assert!(!g.contains("let font_combo ="));
    }

    // --- Focused secondary-event tests (each uses Result or async to prove the
    //     call routes through handler_call(), not a raw `self.h();`). ---

    #[test]
    fn button_double_click_routes_through_handler_call_result() {
        let tree = event_widget(
            WidgetKind::Button,
            None,
            crate::project::schema::WidgetEvent::DoubleClick,
            "on_dbl",
            false,
            crate::project::schema::HandlerResult::Result,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains("evt_response.double_clicked()"));
        assert!(g.contains("if let Err(e) = self.on_dbl()"));
    }

    #[test]
    fn text_input_lost_focus_routes_through_handler_call_result() {
        let tree = event_widget(
            WidgetKind::TextInput,
            Some("name"),
            crate::project::schema::WidgetEvent::LostFocus,
            "on_blur",
            false,
            crate::project::schema::HandlerResult::Result,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains("evt_response.lost_focus()"));
        assert!(g.contains("if let Err(e) = self.on_blur()"));
    }

    #[test]
    fn text_area_lost_focus_routes_through_handler_call_async() {
        let tree = event_widget(
            WidgetKind::TextArea,
            Some("note"),
            crate::project::schema::WidgetEvent::LostFocus,
            "on_ta_blur",
            true,
            crate::project::schema::HandlerResult::Plain,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains("evt_response.lost_focus()"));
        // Async: launcher call + generated worker.
        assert!(g.contains("self.on_ta_blur();"));
        assert!(g.contains("fn on_ta_blur_worker()"));
    }

    #[test]
    fn slider_drag_stopped_routes_through_handler_call_result() {
        let tree = event_widget(
            WidgetKind::Slider,
            Some("vol"),
            crate::project::schema::WidgetEvent::DragStopped,
            "on_drag",
            false,
            crate::project::schema::HandlerResult::Result,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains("evt_response.drag_stopped()"));
        assert!(g.contains("if let Err(e) = self.on_drag()"));
    }

    #[test]
    fn spinbox_drag_stopped_routes_through_handler_call_async() {
        let tree = event_widget(
            WidgetKind::SpinBox,
            Some("qty"),
            crate::project::schema::WidgetEvent::DragStopped,
            "on_sd",
            true,
            crate::project::schema::HandlerResult::Plain,
        );
        let g = gen_app_rs(&tree);
        assert!(g.contains("evt_response.drag_stopped()"));
        assert!(g.contains("self.on_sd();"));
        assert!(g.contains("fn on_sd_worker()"));
    }

    #[test]
    fn primary_and_secondary_on_same_widget_both_export() {
        // One Slider with both On Change and On Drag Stopped handlers (distinct
        // names) — both must be wired off the single shared response.
        let mut tree = event_widget(
            WidgetKind::Slider,
            Some("vol"),
            crate::project::schema::WidgetEvent::Change,
            "on_change",
            false,
            crate::project::schema::HandlerResult::Result,
        );
        tree.widgets[0].on_drag_stopped = "on_stop".to_owned();
        let g = gen_app_rs(&tree);
        assert!(g.contains("let evt_response ="));
        assert!(g.contains("evt_response.changed()"));
        assert!(g.contains("evt_response.drag_stopped()"));
        assert!(g.contains("if let Err(e) = self.on_change()"));
        assert!(g.contains("if let Err(e) = self.on_stop()"));
    }

    #[test]
    fn conflict_across_event_fields_is_detected_and_normalized() {
        // Same handler name on two different widgets via different event fields
        // (Button Click async/Plain vs Button DoubleClick sync/Result). First
        // definition wins; both call sites normalize to it.
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    on_click: "dup".to_owned(),
                    async_handler: true,
                    handler_result: crate::project::schema::HandlerResult::Plain,
                    ..Default::default()
                },
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 40.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    on_double_click: "dup".to_owned(),
                    async_handler: false,
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let g = gen_app_rs(&tree);
        // Conflict surfaced in the top-of-file summary.
        assert!(g.contains("HANDLER CONFLICTS DETECTED"));
        assert!(g.contains("`dup`"));
        // Normalized to first definition (async Plain → launcher `self.dup();`),
        // so the second (Result) call site must NOT emit an `if let Err` wrapper.
        assert!(g.contains("evt_response.clicked()"));
        assert!(g.contains("evt_response.double_clicked()"));
        assert!(!g.contains("if let Err(e) = self.dup()"));
        assert!(g.matches("self.dup();").count() >= 2);
        // Async contract generated once for the winning definition.
        assert!(g.contains("fn dup_worker()"));
    }

    #[test]
    fn async_export_contains_receiver_field_and_status() {
        let g = gen_app_rs(&async_button(
            "on_save",
            crate::project::schema::HandlerResult::Result,
        ));
        // Per-handler receiver, running, and error fields.
        assert!(g.contains("on_save_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,"));
        assert!(g.contains("on_save_running: bool,"));
        assert!(g.contains("on_save_error: Option<String>,"));
        // Default initializes them.
        assert!(g.contains("on_save_rx: None,"));
        assert!(g.contains("on_save_running: false,"));
        assert!(g.contains("on_save_error: None,"));
    }

    #[test]
    fn async_export_contains_spawn_send_and_try_recv() {
        let g = gen_app_rs(&async_button(
            "on_load",
            crate::project::schema::HandlerResult::Plain,
        ));
        // Launcher spawns and sends worker result through the channel.
        assert!(g.contains("std::thread::spawn(move ||"));
        assert!(g.contains("let _ = tx.send(on_load_worker());"));
        assert!(g.contains("std::sync::mpsc::channel::<()>()"));
        // Worker is a free fn (no &mut self) that runs off the UI thread.
        assert!(g.contains("fn on_load_worker()"));
        // Drain in update() uses try_recv.
        assert!(g.contains("self.on_load_rx.as_ref().and_then(|rx| rx.try_recv().ok())"));
        assert!(g.contains("self.on_load_running = false;"));
        // Launcher guards double-launch.
        assert!(g.contains("if self.on_load_running {"));
        // Call site invokes the launcher.
        assert!(g.contains("self.on_load();"));
    }

    #[test]
    fn async_result_path_stores_and_surfaces_error() {
        let g = gen_app_rs(&async_button(
            "on_save",
            crate::project::schema::HandlerResult::Result,
        ));
        assert!(g.contains("fn on_save_worker() -> Result<(), String>"));
        assert!(g.contains("Ok(())"));
        // Drain records the error into the status field.
        assert!(g.contains("if let Some(Err(e)) = &on_save_msg {"));
        assert!(g.contains("self.on_save_error = Some(e.clone());"));
    }

    #[test]
    fn async_export_has_no_todo_only_placeholder() {
        let g = gen_app_rs(&async_button(
            "on_save",
            crate::project::schema::HandlerResult::Result,
        ));
        // The old hollow spawn-with-TODO-only body must be gone.
        assert!(!g.contains("// TODO: background work for on_save; report via an mpsc Sender."));
        // The spawn body actually calls the worker, not just a comment.
        assert!(g.contains("tx.send(on_save_worker())"));
    }

    #[test]
    fn non_async_handler_modes_still_work() {
        // Plain sync
        let mut tree = async_button("h_plain", crate::project::schema::HandlerResult::Plain);
        tree.widgets[0].async_handler = false;
        let g = gen_app_rs(&tree);
        assert!(g.contains("fn h_plain(&mut self) {"));
        assert!(g.contains("self.h_plain();"));
        assert!(!g.contains("h_plain_worker"));
        assert!(!g.contains("h_plain_rx"));

        // Result sync — stub returns Ok(()), call site logs Err
        let mut tree = async_button("h_res", crate::project::schema::HandlerResult::Result);
        tree.widgets[0].async_handler = false;
        let g = gen_app_rs(&tree);
        assert!(g.contains("fn h_res(&mut self) -> Result<(), String>"));
        assert!(g.contains("if let Err(e) = self.h_res()"));
        assert!(!g.contains("h_res_worker"));

        // Option sync
        let mut tree = async_button("h_opt", crate::project::schema::HandlerResult::Option);
        tree.widgets[0].async_handler = false;
        let g = gen_app_rs(&tree);
        assert!(g.contains("fn h_opt(&mut self) -> Option<()>"));
        assert!(g.contains("let _ = self.h_opt();"));
        assert!(!g.contains("h_opt_worker"));
    }

    #[test]
    fn rust_wiring_emits_channel_iterator_trait() {
        use crate::project::schema::{ChannelDef, IteratorPipeline, RustWiring, TraitImpl};
        let mut tree = UiTree::default();
        tree.app_props.rust_wiring = RustWiring {
            channels: vec![ChannelDef {
                id: Uuid::nil(),
                name: "progress".to_owned(),
                ty: "f32".to_owned(),
            }],
            iterators: vec![IteratorPipeline {
                id: Uuid::nil(),
                name: "evens".to_owned(),
                source: "self.state.nums".to_owned(),
                ops: vec![crate::project::schema::IterOp::Filter(
                    "x % 2 == 0".to_owned(),
                )],
            }],
            trait_impls: vec![TraitImpl {
                id: Uuid::nil(),
                trait_name: "Tick".to_owned(),
                method: "fn tick(&mut self)".to_owned(),
                body: "// tick".to_owned(),
            }],
        };
        let g = gen_app_rs(&tree);
        assert!(g.contains("progress_tx: std::sync::mpsc::Sender<f32>"));
        assert!(g.contains("std::sync::mpsc::channel::<f32>()"));
        assert!(g.contains("fn evens(&self) -> impl IntoIterator + '_"));
        assert!(g.contains(".collect::<Vec<_>>()"));
        assert!(g.contains("trait Tick"));
        assert!(g.contains("impl Tick for ExportedApp"));
    }

    #[test]
    fn file_picker_export_includes_rfd_dependency() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::FilePicker,
                state_binding: Some("picked_path".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let files = project_files(&tree);
        let cargo_toml = files
            .iter()
            .find(|(path, _)| path == "Cargo.toml")
            .map(|(_, contents)| contents.as_str())
            .unwrap();
        let app_rs = files
            .iter()
            .find(|(path, _)| path == "src/app.rs")
            .map(|(_, contents)| contents.as_str())
            .unwrap();

        assert!(cargo_toml.contains("rfd = \"0.14\""));
        assert!(app_rs.contains("rfd::FileDialog"));
        assert!(app_rs.contains("self.state.picked_path"));
    }

    #[test]
    fn math_label_export_escapes_label_as_value() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::MathLabel,
                state_binding: Some("total".to_owned()),
                props: crate::project::schema::WidgetProps {
                    label: "A {quoted} \"Total\"".to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);
        assert!(generated.contains("format!(\"{} = {:.2}\""));
        assert!(generated.contains("\"A {quoted} \\\"Total\\\"\""));
        assert!(generated.contains("self.state.total"));
    }

    #[test]
    fn async_repaint_block_emitted_for_async_handler() {
        let g = gen_app_rs(&async_button(
            "on_work",
            crate::project::schema::HandlerResult::Plain,
        ));
        assert!(g.contains("if self.on_work_running {"));
        assert!(g.contains("ctx.request_repaint_after(std::time::Duration::from_millis(16))"));
    }

    #[test]
    fn non_button_async_handler_routes_through_launcher() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::TextInput,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 160.0,
                    h: 28.0,
                },
                state_binding: Some("name".to_owned()),
                on_change: "on_name_change".to_owned(),
                async_handler: true,
                handler_result: crate::project::schema::HandlerResult::Plain,
                ..Default::default()
            }],
            ..Default::default()
        };
        let g = gen_app_rs(&tree);
        // Async contract emitted.
        assert!(g.contains("on_name_change_rx: Option<std::sync::mpsc::Receiver<()>>"));
        assert!(g.contains("fn on_name_change_worker()"));
        assert!(g.contains("std::thread::spawn(move ||"));
        // Changed call site uses launcher — no bare `if let Err` wrapping.
        assert!(g.contains("self.on_name_change();"));
        assert!(!g.contains("if let Err(e) = self.on_name_change()"));
        // Repaint scheduled while task is running.
        assert!(g.contains("self.on_name_change_running"));
        assert!(g.contains("request_repaint_after"));
    }

    #[test]
    fn handler_conflict_emits_warning_and_normalizes_call_sites() {
        // Button: async+Plain registers first. Slider uses same name: sync+Result.
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    on_click: "shared_handler".to_owned(),
                    async_handler: true,
                    handler_result: crate::project::schema::HandlerResult::Plain,
                    ..Default::default()
                },
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::Slider,
                    rect: Rect {
                        x: 0.0,
                        y: 50.0,
                        w: 160.0,
                        h: 28.0,
                    },
                    state_binding: Some("val".to_owned()),
                    on_change: "shared_handler".to_owned(),
                    async_handler: false,
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let g = gen_app_rs(&tree);
        // Conflict warning present.
        assert!(g.contains("CODEGEN CONFLICT"));
        assert!(g.contains("shared_handler"));
        // First definition wins: async launcher emitted, not a sync Result stub.
        assert!(g.contains("std::thread::spawn(move ||"));
        assert!(g.contains("shared_handler_worker()"));
        // Both call sites normalized to registered (async Plain) mode.
        let launcher_calls = g.matches("self.shared_handler();").count();
        assert!(
            launcher_calls >= 2,
            "both call sites must normalize to launcher call, got {launcher_calls}"
        );
        // Result wrapping absent — would be a type error against the void launcher.
        assert!(!g.contains("if let Err(e) = self.shared_handler()"));
    }

    #[test]
    fn combined_async_export_fixture_coherence() {
        // Two async buttons (Plain + Result) + one async TextInput.
        // Proves the combined generated structure is internally consistent.
        let tree = UiTree {
            widgets: vec![
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    on_click: "plain_task".to_owned(),
                    async_handler: true,
                    handler_result: crate::project::schema::HandlerResult::Plain,
                    ..Default::default()
                },
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::Button,
                    rect: Rect {
                        x: 0.0,
                        y: 40.0,
                        w: 80.0,
                        h: 30.0,
                    },
                    on_click: "result_task".to_owned(),
                    async_handler: true,
                    handler_result: crate::project::schema::HandlerResult::Result,
                    ..Default::default()
                },
                WidgetInstance {
                    id: Uuid::nil(),
                    kind: WidgetKind::TextInput,
                    rect: Rect {
                        x: 0.0,
                        y: 80.0,
                        w: 160.0,
                        h: 28.0,
                    },
                    state_binding: Some("name".to_owned()),
                    on_change: "on_name_change".to_owned(),
                    async_handler: true,
                    handler_result: crate::project::schema::HandlerResult::Plain,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let g = gen_app_rs(&tree);
        // All three receiver fields.
        assert!(g.contains("plain_task_rx: Option<std::sync::mpsc::Receiver<()>>"));
        assert!(g.contains("result_task_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>"));
        assert!(g.contains("on_name_change_rx: Option<std::sync::mpsc::Receiver<()>>"));
        // Result error field.
        assert!(g.contains("result_task_error: Option<String>"));
        // All three workers.
        assert!(g.contains("fn plain_task_worker()"));
        assert!(g.contains("fn result_task_worker() -> Result<(), String>"));
        assert!(g.contains("fn on_name_change_worker()"));
        // Repaint covers all three tasks.
        assert!(g.contains(
            "self.plain_task_running || self.result_task_running || self.on_name_change_running"
        ));
        assert!(g.contains("request_repaint_after"));
        // No spurious conflict (all handlers are distinct names).
        assert!(!g.contains("CODEGEN CONFLICT"));
    }

    #[test]
    fn chart_export_emits_bound_painter_code() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::Chart,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 180.0,
                    h: 90.0,
                },
                state_binding: Some("values".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);
        assert!(generated.contains("pub values: Vec<f32>"));
        assert!(generated.contains("values: vec![0.2, 0.5, 0.8, 0.4]"));
        assert!(generated.contains("let chart_values = &self.state.values"));
        assert!(generated.contains("chart_painter.rect_filled"));
        assert!(!generated.contains("bind a Vec<f32> and paint"));
    }

    // ORDERING DECISION: a Button with BOTH Click and DoubleClick handlers wires
    // both independently. egui native semantics: `clicked()` fires on the first
    // release, `double_clicked()` on the second click — both may run across a
    // double-click gesture. Click is intentionally NOT suppressed.
    #[test]
    fn button_click_and_double_click_both_emitted_no_suppression() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::Button,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 80.0,
                    h: 30.0,
                },
                on_click: "on_click".to_owned(),
                on_double_click: "on_dbl".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let g = gen_app_rs(&tree);
        // Both gates emitted off the one shared response — neither suppressed.
        assert!(g.contains("evt_response.clicked()"));
        assert!(g.contains("evt_response.double_clicked()"));
        assert!(g.contains("self.on_click();"));
        assert!(g.contains("self.on_dbl();"));
    }

    // -------------------------------------------------------------------------
    // Generated-export compile proof
    //
    // One fixture tree exercising the full event+async export surface. The smoke
    // test (always run) proves the project is generatable and the matrix is
    // present; the `#[ignore]`d test runs a real `cargo check` on the generated
    // crate (compiles eframe/egui — minutes on first run, so opt-in).
    // -------------------------------------------------------------------------

    /// A unique temp directory path (std-only; no tempfile crate).
    fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "rohkai_export_fixture_{tag}_{}_{nanos}",
            std::process::id()
        ));
        dir
    }

    /// The compile-proof fixture: top-level Button Click + DoubleClick, two async
    /// buttons (Plain + Result), a Frame with a TextInput (LostFocus) and a Slider
    /// (DragStopped) child, VLayout/HLayout/GridLayout owned children, FilePicker/rfd
    /// dependency, Rust wiring, and three state bindings (`name: String`,
    /// `vol: f32`, `picked_path: String`).
    fn compile_fixture_tree() -> UiTree {
        use crate::project::schema::{
            ChannelDef, HandlerResult, IterOp, IteratorPipeline, RustWiring, TraitImpl,
        };
        let frame_id = Uuid::from_u128(0xF0);
        let ti_id = Uuid::from_u128(0x71);
        let sl_id = Uuid::from_u128(0x51);
        let layout_id = Uuid::from_u128(0xD0);
        let layout_child_id = Uuid::from_u128(0xD1);
        let horizontal_layout_id = Uuid::from_u128(0xD2);
        let horizontal_layout_child_id = Uuid::from_u128(0xD3);
        let grid_layout_id = Uuid::from_u128(0xD4);
        let grid_layout_child_id = Uuid::from_u128(0xD5);

        let btn_events = WidgetInstance {
            id: Uuid::from_u128(0x01),
            kind: WidgetKind::Button,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 30.0,
            },
            on_click: "on_primary".to_owned(),
            on_double_click: "on_secondary".to_owned(),
            props: crate::project::schema::WidgetProps {
                label: "Go".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let btn_async_plain = WidgetInstance {
            id: Uuid::from_u128(0x02),
            kind: WidgetKind::Button,
            rect: Rect {
                x: 0.0,
                y: 40.0,
                w: 100.0,
                h: 30.0,
            },
            on_click: "load_async".to_owned(),
            async_handler: true,
            handler_result: HandlerResult::Plain,
            props: crate::project::schema::WidgetProps {
                label: "Load".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let btn_async_result = WidgetInstance {
            id: Uuid::from_u128(0x03),
            kind: WidgetKind::Button,
            rect: Rect {
                x: 0.0,
                y: 80.0,
                w: 100.0,
                h: 30.0,
            },
            on_click: "save_async".to_owned(),
            async_handler: true,
            handler_result: HandlerResult::Result,
            props: crate::project::schema::WidgetProps {
                label: "Save".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let file_picker = WidgetInstance {
            id: Uuid::from_u128(0x04),
            kind: WidgetKind::FilePicker,
            rect: Rect {
                x: 0.0,
                y: 220.0,
                w: 180.0,
                h: 28.0,
            },
            state_binding: Some("picked_path".to_owned()),
            props: crate::project::schema::WidgetProps {
                label: "Pick file".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let ti_child = WidgetInstance {
            id: ti_id,
            kind: WidgetKind::TextInput,
            rect: Rect {
                x: 10.0,
                y: 130.0,
                w: 160.0,
                h: 26.0,
            },
            state_binding: Some("name".to_owned()),
            on_lost_focus: "on_blur".to_owned(),
            props: crate::project::schema::WidgetProps {
                label: "Name".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let sl_child = WidgetInstance {
            id: sl_id,
            kind: WidgetKind::Slider,
            rect: Rect {
                x: 10.0,
                y: 160.0,
                w: 160.0,
                h: 26.0,
            },
            state_binding: Some("vol".to_owned()),
            on_drag_stopped: "on_drag".to_owned(),
            props: crate::project::schema::WidgetProps {
                label: "Vol".to_owned(),
                min: 0.0,
                max: 100.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let frame = WidgetInstance {
            id: frame_id,
            kind: WidgetKind::Frame,
            rect: Rect {
                x: 0.0,
                y: 120.0,
                w: 200.0,
                h: 90.0,
            },
            children: vec![ti_id, sl_id],
            ..Default::default()
        };
        let layout_child = WidgetInstance {
            id: layout_child_id,
            kind: WidgetKind::Button,
            rect: Rect {
                x: 220.0,
                y: 128.0,
                w: 140.0,
                h: 30.0,
            },
            on_click: "layout_child_clicked".to_owned(),
            handler_result: HandlerResult::Result,
            props: crate::project::schema::WidgetProps {
                label: "Layout Child".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let vlayout = WidgetInstance {
            id: layout_id,
            kind: WidgetKind::VLayout,
            rect: Rect {
                x: 220.0,
                y: 120.0,
                w: 180.0,
                h: 100.0,
            },
            children: vec![layout_child_id],
            ..Default::default()
        };
        let horizontal_layout_child = WidgetInstance {
            id: horizontal_layout_child_id,
            kind: WidgetKind::Button,
            rect: Rect {
                x: 420.0,
                y: 128.0,
                w: 150.0,
                h: 30.0,
            },
            on_click: "horizontal_layout_child_clicked".to_owned(),
            handler_result: HandlerResult::Result,
            props: crate::project::schema::WidgetProps {
                label: "HLayout Child".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let hlayout = WidgetInstance {
            id: horizontal_layout_id,
            kind: WidgetKind::HLayout,
            rect: Rect {
                x: 420.0,
                y: 120.0,
                w: 220.0,
                h: 70.0,
            },
            children: vec![horizontal_layout_child_id],
            ..Default::default()
        };
        let grid_layout_child = WidgetInstance {
            id: grid_layout_child_id,
            kind: WidgetKind::Button,
            rect: Rect {
                x: 660.0,
                y: 128.0,
                w: 120.0,
                h: 30.0,
            },
            on_click: "grid_layout_child_clicked".to_owned(),
            handler_result: HandlerResult::Result,
            props: crate::project::schema::WidgetProps {
                label: "Grid Child".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let grid_layout = WidgetInstance {
            id: grid_layout_id,
            kind: WidgetKind::GridLayout,
            rect: Rect {
                x: 660.0,
                y: 120.0,
                w: 240.0,
                h: 120.0,
            },
            children: vec![grid_layout_child_id],
            ..Default::default()
        };

        let mut tree = UiTree {
            widgets: vec![
                btn_events,
                btn_async_plain,
                btn_async_result,
                frame,
                ti_child,
                sl_child,
                vlayout,
                layout_child,
                hlayout,
                horizontal_layout_child,
                grid_layout,
                grid_layout_child,
                file_picker,
            ],
            ..Default::default()
        };
        tree.app_props.rust_wiring = RustWiring {
            channels: vec![ChannelDef {
                id: Uuid::from_u128(0xA1),
                name: "progress".to_owned(),
                ty: "f32".to_owned(),
            }],
            iterators: vec![IteratorPipeline {
                id: Uuid::from_u128(0xA2),
                name: "positive_values".to_owned(),
                source: "vec![1_i32, 2_i32, 3_i32]".to_owned(),
                ops: vec![
                    IterOp::Filter("**x > 1".to_owned()),
                    IterOp::Map("*x".to_owned()),
                ],
            }],
            trait_impls: vec![TraitImpl {
                id: Uuid::from_u128(0xA3),
                trait_name: "CompileProofBehavior".to_owned(),
                method: "fn touch(&mut self)".to_owned(),
                body: "self.state.vol = 42.0;".to_owned(),
            }],
        };
        tree
    }

    /// Breadth fixture: every built-in palette kind plus Image/SVG in one
    /// generated project. This guards against adding a visible widget kind whose
    /// export path compiles only in string-level tests.
    fn all_builtin_widgets_tree() -> UiTree {
        let mut widgets: Vec<WidgetInstance> = crate::widgets::ALL_KINDS
            .iter()
            .cloned()
            .enumerate()
            .map(|(idx, kind)| {
                let mut w = crate::widgets::default_for(&kind);
                w.id = Uuid::from_u128(0x1000 + idx as u128);
                w.rect.x = 20.0 + (idx % 4) as f32 * 320.0;
                w.rect.y = 20.0 + (idx / 4) as f32 * 190.0;
                w
            })
            .collect();

        widgets.push(WidgetInstance {
            id: Uuid::from_u128(0x1FFF),
            kind: WidgetKind::Image,
            rect: Rect {
                x: 20.0,
                y: 1600.0,
                w: 180.0,
                h: 120.0,
            },
            svg_source: Some(
                r##"<svg viewBox="0 0 120 80" xmlns="http://www.w3.org/2000/svg">
  <rect x="8" y="8" width="104" height="64" fill="#244" stroke="#34d399"/>
  <circle cx="60" cy="40" r="18" fill="#34d399"/>
</svg>"##
                    .to_owned(),
            ),
            ..Default::default()
        });

        UiTree {
            widgets,
            ..Default::default()
        }
    }

    #[test]
    fn embedded_svg_sources_keep_single_import_rewrite_contract() {
        const IMPORT: &str = "use crate::svg_core::{self, Rgba};";
        let crate_refs: Vec<_> = SVG_RASTERIZER_SOURCE.match_indices("crate::").collect();

        assert_eq!(
            crate_refs.len(),
            1,
            "embedded rasterizer gained a new crate-relative path"
        );
        assert!(
            SVG_RASTERIZER_SOURCE.contains(IMPORT),
            "known svg_core import changed; update export embedding deliberately"
        );
        assert!(
            !SVG_CORE_SOURCE.contains("crate::"),
            "embedded svg_core must remain standalone and crate-path free"
        );

        let embedded = SVG_RASTERIZER_SOURCE.replace(IMPORT, "use super::svg_core::{self, Rgba};");
        assert!(!embedded.contains("crate::"));
        assert!(embedded.contains("use super::svg_core::{self, Rgba};"));
    }

    /// Invariant: every SVG R4 feature that *renders* in the in-app rasterizer
    /// also renders in the export-embedded copy.  Because export embeds
    /// `svg_rasterizer.rs` verbatim, this checks that the render-path symbols
    /// (not merely diagnostics) are present in the embedded source, so there can
    /// be no in-app-only clip/compositing support.
    #[test]
    fn embedded_rasterizer_includes_r4_render_paths() {
        for marker in [
            "fn resolve_clip",              // clipPath reference resolution
            "fn build_mask",                // clip coverage -> alpha mask
            "DrawCommand::BeginLayer",      // group/viewport layer scoping
            "fn composite_offscreen",       // isolated group opacity compositing
            "fn blend_pixel_premultiplied", // premultiplied internal buffer
            "fn overflow_clip_shape",       // nested <svg> overflow clipping
        ] {
            assert!(
                SVG_RASTERIZER_SOURCE.contains(marker),
                "embedded rasterizer missing R4 render path: {marker}"
            );
        }
    }

    /// R9 invariant: markers, non-scaling-stroke, and pattern tiling all render
    /// in the export-embedded rasterizer (same verbatim source as in-app), not
    /// merely diagnostics — so exported apps render them identically.
    #[test]
    fn embedded_rasterizer_includes_r9_render_paths() {
        for marker in [
            "fn build_markers",           // marker resolution + placement
            "fn marker_placement",        // marker viewport/orient transform
            "fn build_pattern_sampler",   // pattern tile rendering
            "fn build_pattern_def",       // pattern href inheritance
            "fn effective_device_stroke", // vector-effect non-scaling-stroke
            "PaintSampler::Pattern",      // pattern paint sampling
        ] {
            assert!(
                SVG_RASTERIZER_SOURCE.contains(marker),
                "embedded rasterizer missing R9 render path: {marker}"
            );
        }
    }

    /// R10 invariant: tier-2 filter primitives render in the export-embedded
    /// rasterizer (same verbatim source as in-app), not merely diagnostics.
    #[test]
    fn embedded_rasterizer_includes_r10_render_paths() {
        for marker in [
            "fn composite_filter",            // feComposite (Porter-Duff + arithmetic)
            "fn blend_filter",                // feBlend
            "fn component_transfer",          // feComponentTransfer
            "fn morphology",                  // feMorphology
            "enum BlendMode",                 // shared blend (feBlend / mix-blend-mode)
            "fn composite_offscreen_blended", // mix-blend-mode group compositing
            "fn srgb_to_linear_premul",       // color-interpolation-filters: linearRGB
            "fn linear_to_srgb_premul",       // linearRGB -> sRGB at the boundary
            "fn clip_to_filter_region",       // precise filter-region clipping
        ] {
            assert!(
                SVG_RASTERIZER_SOURCE.contains(marker),
                "embedded rasterizer missing R10 render path: {marker}"
            );
        }
    }

    /// R11 invariant: raster text (bundled vector font + textPath) renders in
    /// the export-embedded rasterizer, not merely diagnostics.
    #[test]
    fn embedded_rasterizer_includes_r11_render_paths() {
        for marker in [
            "HERSHEY_SIMPLEX",         // bundled public-domain glyph data
            "fn lower_text_command",   // text -> stroked glyph Shape lowering
            "fn scan_text_runs",       // tspan/textPath content scanner
            "struct ArcLengthPath",    // textPath arc-length placement
            "fn append_glyph_strokes", // glyph polyline emission
        ] {
            assert!(
                SVG_RASTERIZER_SOURCE.contains(marker),
                "embedded rasterizer missing R11 render path: {marker}"
            );
        }
    }

    /// R12 invariant: namespace model, malformed recovery, and a11y metadata
    /// live in the export-embedded rasterizer (same verbatim source as in-app).
    #[test]
    fn embedded_rasterizer_includes_r12_paths() {
        for marker in [
            "fn apply_xmlns",       // xmlns scope resolution
            "enum Namespace",       // svg/xlink/foreign classification
            "fn consume_close_tag", // malformed-recovery close-tag counting
            "fn bounded_a11y_text", // <title>/<desc> extraction (bounded)
            "namespace.foreign_element",
            "recovery.malformed_markup",
        ] {
            assert!(
                SVG_RASTERIZER_SOURCE.contains(marker),
                "embedded rasterizer missing R12 path: {marker}"
            );
        }
    }

    #[test]
    fn wasm_export_generates_required_files_and_lib_entry() {
        use crate::project::schema::{Rect, WidgetInstance, WidgetKind};
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: uuid::Uuid::from_u128(0xBEEF),
                kind: WidgetKind::Button,
                rect: Rect {
                    x: 10.0,
                    y: 10.0,
                    w: 80.0,
                    h: 30.0,
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = project_files_wasm(&tree, true);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"Cargo.toml"), "WASM Cargo.toml missing");
        assert!(names.contains(&"src/lib.rs"), "WASM lib.rs missing");
        assert!(names.contains(&"src/app.rs"), "WASM app.rs missing");
        assert!(names.contains(&"index.html"), "WASM index.html missing");
        assert!(names.contains(&"Trunk.toml"), "WASM Trunk.toml missing");
        assert!(
            !names.contains(&"src/main.rs"),
            "WASM must use lib.rs not main.rs"
        );

        let cargo = files
            .iter()
            .find(|(n, _)| n == "Cargo.toml")
            .map(|(_, c)| c.as_str())
            .unwrap();
        assert!(
            cargo.contains("cdylib"),
            "WASM Cargo.toml must declare cdylib"
        );
        assert!(
            cargo.contains("wasm-bindgen"),
            "WASM Cargo.toml must include wasm-bindgen feature"
        );
        assert!(
            !cargo.contains("rfd"),
            "WASM Cargo.toml must not include rfd"
        );

        let lib_rs = files
            .iter()
            .find(|(n, _)| n == "src/lib.rs")
            .map(|(_, c)| c.as_str())
            .unwrap();
        assert!(
            lib_rs.contains("wasm_bindgen(start)"),
            "lib.rs must export wasm start fn"
        );
        assert!(lib_rs.contains("WebRunner"), "lib.rs must use WebRunner");
        assert!(
            lib_rs.contains("the_canvas_id"),
            "lib.rs must reference canvas element id"
        );

        let html = files
            .iter()
            .find(|(n, _)| n == "index.html")
            .map(|(_, c)| c.as_str())
            .unwrap();
        assert!(
            html.contains("the_canvas_id"),
            "index.html must have canvas element"
        );
        assert!(html.contains("<canvas"), "index.html must have canvas tag");
    }

    #[test]
    fn wasm_export_file_picker_generates_notes_file() {
        use crate::project::schema::{Rect, WidgetInstance, WidgetKind};
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: uuid::Uuid::from_u128(0xF11E),
                kind: WidgetKind::FilePicker,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 30.0,
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let files = project_files_wasm(&tree, false);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            names.contains(&"WASM_NOTES.txt"),
            "must warn about FilePicker in WASM"
        );
    }

    /// Always-run smoke: the fixture generates the required files and its source
    /// contains every feature-matrix marker.  Fast (no compilation).
    #[test]
    fn export_compile_fixture_generates_required_files_and_matrix() {
        let tree = compile_fixture_tree();
        let dir = unique_temp_dir("smoke");
        write_project(&tree, &dir).expect("write_project");

        // Required files for `cargo check`.
        assert!(dir.join("Cargo.toml").exists(), "Cargo.toml missing");
        assert!(dir.join("src/main.rs").exists(), "main.rs missing");
        assert!(dir.join("src/app.rs").exists(), "app.rs missing");

        let app = std::fs::read_to_string(dir.join("src/app.rs"))
            .expect("exported src/app.rs must be readable");
        // Feature matrix markers.
        assert!(app.contains("evt_response.clicked()"), "top Button Click");
        assert!(
            app.contains("evt_response.double_clicked()"),
            "top Button DoubleClick"
        );
        assert!(
            app.contains("child_response.lost_focus()"),
            "nested TextInput LostFocus"
        );
        assert!(
            app.contains("child_response.drag_stopped()"),
            "nested Slider DragStopped"
        );
        assert!(
            app.contains("layout_child_clicked"),
            "VLayout child handler"
        );
        assert!(app.contains("ui.vertical(|ui| {"), "VLayout child nesting");
        assert!(
            app.contains("horizontal_layout_child_clicked"),
            "HLayout child handler"
        );
        assert!(
            app.contains("ui.horizontal(|ui| {"),
            "HLayout child nesting"
        );
        assert!(
            app.contains("grid_layout_child_clicked"),
            "GridLayout child handler"
        );
        assert!(app.contains("egui::Grid::new"), "GridLayout child nesting");
        assert!(app.contains("ui.end_row();"), "GridLayout row boundary");
        assert!(app.contains("fn load_async_worker()"), "async Plain worker");
        assert!(
            app.contains("fn save_async_worker() -> Result<(), String>"),
            "async Result worker"
        );
        assert!(app.contains("self.state.name"), "String binding");
        assert!(app.contains("self.state.vol"), "f32 binding");
        assert!(app.contains("self.state.picked_path"), "FilePicker binding");
        assert!(app.contains("rfd::FileDialog"), "FilePicker runtime call");
        assert!(
            app.contains("progress_tx: std::sync::mpsc::Sender<f32>"),
            "channel sender field"
        );
        assert!(
            app.contains("progress_rx: std::sync::mpsc::Receiver<f32>"),
            "channel receiver field"
        );
        assert!(app.contains("fn positive_values(&self) -> impl IntoIterator + '_"));
        assert!(app.contains(".filter(|x| **x > 1).map(|x| *x)"));
        assert!(app.contains("trait CompileProofBehavior"));
        assert!(app.contains("impl CompileProofBehavior for ExportedApp"));

        let cargo = std::fs::read_to_string(dir.join("Cargo.toml"))
            .expect("exported Cargo.toml must be readable");
        assert!(cargo.contains("eframe = \"0.29\""));
        assert!(cargo.contains("egui   = \"0.29\""));
        assert!(cargo.contains("rfd = \"0.14\""));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Always-run breadth smoke: every built-in palette widget kind plus Image
    /// generates a project with no obvious placeholder/fallback comments.
    #[test]
    fn all_builtin_widgets_export_generates_required_files_and_matrix() {
        let tree = all_builtin_widgets_tree();
        let dir = unique_temp_dir("all_widgets_smoke");
        write_project(&tree, &dir).expect("write_project");

        assert!(dir.join("Cargo.toml").exists(), "Cargo.toml missing");
        assert!(dir.join("src/main.rs").exists(), "main.rs missing");
        assert!(dir.join("src/app.rs").exists(), "app.rs missing");

        let app = std::fs::read_to_string(dir.join("src/app.rs"))
            .expect("exported src/app.rs must be readable");
        let cargo = std::fs::read_to_string(dir.join("Cargo.toml"))
            .expect("exported Cargo.toml must be readable");

        assert!(cargo.contains("eframe = \"0.29\""));
        assert!(cargo.contains("egui   = \"0.29\""));
        assert!(
            cargo.contains("rfd = \"0.14\""),
            "FilePicker in all-widget fixture must pull rfd"
        );
        assert!(
            app.matches("egui::Area::new").count() >= tree.widgets.len(),
            "each top-level widget should have an Area wrapper"
        );
        assert!(
            app.contains("egui::Button::new"),
            "Button-like export missing"
        );
        assert!(
            app.contains("egui::TextEdit::singleline"),
            "TextInput export missing"
        );
        assert!(
            app.contains("egui::TextEdit::multiline"),
            "TextArea export missing"
        );
        assert!(app.contains("egui::Slider::new"), "Slider export missing");
        assert!(
            app.contains("egui::DragValue::new"),
            "SpinBox export missing"
        );
        assert!(
            app.contains("egui::ComboBox::from_label"),
            "ComboBox export missing"
        );
        assert!(
            app.contains("egui::ComboBox::from_id_salt"),
            "FontComboBox export missing"
        );
        assert!(
            app.contains("egui::ProgressBar::new"),
            "ProgressBar export missing"
        );
        assert!(app.contains("egui::Grid::new"), "Grid/Table export missing");
        assert!(
            app.contains("egui::ScrollArea::vertical"),
            "Scroll/List export missing"
        );
        assert!(
            app.contains("egui::CollapsingHeader::new"),
            "Tree/ToolBox export missing"
        );
        assert!(
            app.contains("chart_painter.rect_filled"),
            "Chart export missing"
        );
        assert!(
            app.contains("rfd::FileDialog"),
            "FilePicker runtime export missing"
        );
        assert!(
            app.contains("mod rohkai_svg"),
            "Image/SVG renderer export missing"
        );
        assert!(
            !app.contains("set a valid Binding"),
            "all-widget defaults should provide required bindings"
        );
        assert!(
            !app.contains("descriptor not loaded"),
            "all-widget fixture should not rely on missing custom descriptors"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Opt-in compile proof: run a real `cargo check` on the generated crate.
    /// Ignored by default because it compiles eframe/egui (minutes on first run).
    /// Run with: `cargo test -p rohkai export_compile_fixture_cargo_check -- --ignored`
    /// A shared `CARGO_TARGET_DIR` caches deps across runs.
    #[test]
    #[ignore = "compiles a real eframe/egui crate; slow. Run with --ignored."]
    fn export_compile_fixture_cargo_check() {
        let tree = compile_fixture_tree();
        let dir = unique_temp_dir("check");
        write_project(&tree, &dir).expect("write_project");

        let mut target = std::env::temp_dir();
        target.push("rohkai_export_fixture_target");

        let output = std::process::Command::new("cargo")
            .args(["check", "--quiet"])
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .expect("failed to spawn cargo check");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Leave `dir` in place for debugging on failure.
            panic!(
                "generated export project failed `cargo check` (dir: {})\n{stderr}",
                dir.display()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Opt-in compile proof for the full built-in widget catalog.
    /// Run with: `cargo test -p rohkai all_builtin_widgets_export_cargo_check -- --ignored`
    #[test]
    #[ignore = "compiles a real eframe/egui crate with every built-in widget; slow. Run with --ignored."]
    fn all_builtin_widgets_export_cargo_check() {
        let tree = all_builtin_widgets_tree();
        let dir = unique_temp_dir("all_widgets_check");
        write_project(&tree, &dir).expect("write_project");

        let mut target = std::env::temp_dir();
        target.push("rohkai_export_fixture_target");

        let output = std::process::Command::new("cargo")
            .args(["check", "--quiet"])
            .current_dir(&dir)
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .expect("failed to spawn cargo check");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "all-widget export project failed `cargo check` (dir: {})\n{stderr}",
                dir.display()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asset_manifest_is_generated_with_correct_entries() {
        use crate::project::schema::{AppProps, AssetEntry, AssetKind};
        let tree = UiTree {
            app_props: AppProps {
                assets: vec![
                    AssetEntry {
                        id: Uuid::nil(),
                        name: "logo.png".to_owned(),
                        source_path: "/home/user/images/logo.png".to_owned(),
                        kind: AssetKind::Image,
                    },
                    AssetEntry {
                        id: Uuid::from_u128(1),
                        name: "data.json".to_owned(),
                        source_path: "/home/user/data/data.json".to_owned(),
                        kind: AssetKind::Data,
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let dir = unique_temp_dir("asset_manifest");
        write_project(&tree, &dir).expect("write_project");

        let manifest_path = dir.join("assets/MANIFEST.txt");
        assert!(manifest_path.exists(), "assets/MANIFEST.txt not generated");
        let manifest = std::fs::read_to_string(&manifest_path).expect("read manifest");
        assert!(manifest.contains("logo.png"), "manifest missing logo.png");
        assert!(
            manifest.contains("/home/user/images/logo.png"),
            "manifest missing source path for logo.png"
        );
        assert!(manifest.contains("data.json"), "manifest missing data.json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_descriptor_export_renders_template_not_placeholder() {
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id: Uuid::nil(),
                kind: WidgetKind::Custom("ply.button".to_owned()),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 80.0,
                    h: 30.0,
                },
                descriptor_name: Some("ply.button".to_owned()),
                descriptor_export_tpl: Some(
                    "ui.add(ply::Button::new({{label}}).size({{width}}, {{height}}));".to_owned(),
                ),
                descriptor_cargo_deps: vec!["ply = \"0.1\"".to_owned()],
                props: crate::project::schema::WidgetProps {
                    label: "Click me".to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        };

        let generated = gen_app_rs(&tree);

        assert!(
            generated.contains("ply::Button::new"),
            "descriptor template not rendered: got\n{generated}"
        );
        assert!(
            generated.contains("\"Click me\""),
            "{{{{label}}}} token not substituted"
        );
        assert!(
            generated.contains("80.0"),
            "{{{{width}}}} token not substituted"
        );
        assert!(
            !generated.contains("descriptor not loaded"),
            "descriptor template must not fall through to placeholder"
        );

        let dir = unique_temp_dir("custom_descriptor");
        write_project(&tree, &dir).expect("write_project");
        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).expect("Cargo.toml");
        assert!(
            cargo.contains("ply = \"0.1\""),
            "descriptor cargo dep not injected into exported Cargo.toml"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
