use crate::canvas::interaction::{CanvasSettings, InteractionState};
use crate::panels::code_preview::{CodePreviewArgs, CodeStatus};
use crate::project::schema::{WidgetInstance, WidgetKind};
use crate::project::ui_tree::UiTree;
use crate::settings::UserSettings;
use egui::Key;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy)]
enum PendingCommand {
    New,
    Open,
}

pub struct RohKaiApp {
    pub ui_tree: UiTree,
    pub interaction: InteractionState,
    pub selected: Vec<Uuid>,
    pub current_file: Option<PathBuf>,
    pub saved_json: Option<String>,
    pub last_error: Option<String>,
    pub canvas_settings: CanvasSettings,
    /// (true = success) message after export
    pub export_message: Option<(bool, String)>,
    pub export_message_until: Option<f64>,
    /// Widget id highlighted in the code panel (Lazare double-click)
    pub highlighted_code_id: Option<Uuid>,
    /// When true, code panel scrolls to the highlighted line once
    pub scroll_to_code: bool,
    /// Tracé: if Some(name), code panel scrolls to fn {name} and inserts a stub if absent
    pub scroll_to_handler: Option<String>,
    /// Status message from template operations
    pub template_message: Option<(bool, String)>,
    pending_command: Option<PendingCommand>,
    /// Lazare edit buffer (used when code_status != Live)
    pub code_buffer: String,
    pub code_status: CodeStatus,
    /// Last generated code string — used to detect canvas changes and reset the code panel
    pub last_generated: String,
    pub code_split_ratio: f32,
    pub user_settings: UserSettings,
    pub preferences_draft: UserSettings,
    pub settings_path: PathBuf,
    pub settings_message: Option<(bool, String)>,
    pub preferences_open: bool,
    base_pixels_per_point: f32,
    dirty_cache: bool,
    dirty_cache_checked_at: f64,
}

impl RohKaiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        let loaded_settings = crate::settings::load();
        let user_settings = loaded_settings.settings;
        let canvas_settings = CanvasSettings {
            snap_step: user_settings.default_snap_step,
            ..Default::default()
        };
        let base_pixels_per_point = cc.egui_ctx.pixels_per_point();
        cc.egui_ctx
            .set_pixels_per_point(base_pixels_per_point * user_settings.ui_scale);
        Self {
            ui_tree: UiTree::default(),
            interaction: InteractionState::default(),
            selected: Vec::new(),
            current_file: None,
            saved_json: None,
            last_error: None,
            canvas_settings,
            export_message: None,
            export_message_until: None,
            highlighted_code_id: None,
            scroll_to_code: false,
            scroll_to_handler: None,
            template_message: None,
            pending_command: None,
            code_buffer: String::new(),
            code_status: CodeStatus::Live,
            last_generated: String::new(),
            code_split_ratio: 0.6,
            user_settings: user_settings.clone(),
            preferences_draft: user_settings,
            settings_path: loaded_settings.path,
            settings_message: loaded_settings.error.map(|e| (false, e)),
            preferences_open: false,
            base_pixels_per_point,
            dirty_cache: false,
            dirty_cache_checked_at: 0.0,
        }
    }

    fn compute_dirty_exact(&self) -> bool {
        let current = crate::project::io::serialize(&self.ui_tree).unwrap_or_default();
        match &self.saved_json {
            Some(snap) => current != *snap,
            None => !self.ui_tree.widgets.is_empty(),
        }
    }

    fn cached_dirty(&mut self, ctx: &egui::Context) -> bool {
        let now = ctx.input(|i| i.time);
        if now - self.dirty_cache_checked_at > 0.25 {
            self.dirty_cache = self.compute_dirty_exact();
            self.dirty_cache_checked_at = now;
        }
        self.dirty_cache
    }

    fn do_save(&mut self, path: PathBuf) {
        match crate::project::io::save(&path, &self.ui_tree) {
            Ok(json) => {
                self.saved_json = Some(json);
                self.current_file = Some(path);
                self.last_error = None;
                self.dirty_cache = false;
                self.dirty_cache_checked_at = 0.0;
            }
            Err(e) => self.last_error = Some(e),
        }
    }

    fn request_destructive_command(&mut self, command: PendingCommand) {
        if self.compute_dirty_exact() {
            self.pending_command = Some(command);
        } else {
            self.run_destructive_command(command);
        }
    }

    fn run_destructive_command(&mut self, command: PendingCommand) {
        match command {
            PendingCommand::New => self.cmd_new(),
            PendingCommand::Open => self.cmd_open(),
        }
    }

    fn cmd_new(&mut self) {
        self.ui_tree = UiTree::default();
        self.current_file = None;
        self.saved_json = None;
        self.selected.clear();
        self.last_error = None;
        self.highlighted_code_id = None;
        self.dirty_cache = false;
        self.dirty_cache_checked_at = 0.0;
    }

    fn cmd_open(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("RohKai project", &["rohkai.json", "json"])
            .pick_file();
        if let Some(path) = path {
            match crate::project::io::load(&path) {
                Ok(tree) => {
                    let snap = crate::project::io::serialize(&tree).unwrap_or_default();
                    self.ui_tree = tree;
                    self.saved_json = Some(snap);
                    self.current_file = Some(path);
                    self.selected.clear();
                    self.last_error = None;
                    self.highlighted_code_id = None;
                    self.dirty_cache = false;
                    self.dirty_cache_checked_at = 0.0;
                }
                Err(e) => self.last_error = Some(e),
            }
        }
    }

    fn cmd_save(&mut self) {
        if let Some(path) = self.current_file.clone() {
            self.do_save(path);
        } else {
            self.cmd_save_as();
        }
    }

    fn cmd_save_as(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("RohKai project", &["rohkai.json", "json"])
            .set_file_name("untitled.rohkai.json")
            .save_file();
        if let Some(path) = path {
            self.do_save(path);
        }
    }

    fn cmd_export(&mut self) {
        self.export_message = None;
        self.export_message_until = None;
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Export project — choose destination folder")
            .pick_folder()
        else {
            return;
        };
        match crate::codegen::export::write_project(&self.ui_tree, &folder) {
            Ok(()) => {
                self.export_message = Some((true, format!("Exported → {}", folder.display())));
            }
            Err(e) => {
                self.export_message = Some((false, format!("Export failed: {e}")));
            }
        }
    }

    fn cmd_save_template(&mut self) {
        self.template_message = None;
        if self.selected.is_empty() {
            return;
        }

        let widgets: Vec<WidgetInstance> = self
            .selected
            .iter()
            .filter_map(|&id| self.ui_tree.widgets.iter().find(|w| w.id == id).cloned())
            .collect();

        let name = rfd::FileDialog::new()
            .set_title("Save template")
            .set_directory(
                crate::panels::templates::templates_dir().unwrap_or_else(|| PathBuf::from(".")),
            )
            .set_file_name("my_template.rktp")
            .add_filter("RohKai Template", &["rktp"])
            .save_file();

        if let Some(path) = name {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("template")
                .to_owned();
            match crate::panels::templates::save_template(&stem, &widgets) {
                Ok(_) => {
                    self.template_message = Some((true, format!("Saved \"{stem}\"")));
                }
                Err(e) => {
                    self.template_message = Some((false, format!("Save failed: {e}")));
                }
            }
        }
    }

    fn cmd_import_svg_template(&mut self) {
        self.template_message = None;
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import SVG as template")
            .add_filter("SVG", &["svg"])
            .pick_file()
        else {
            return;
        };

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("svg_template")
            .to_owned();

        match std::fs::read_to_string(&path)
            .map_err(|e| format!("read svg: {e}"))
            .and_then(|svg| {
                let output = crate::svg_import::import_svg_template(
                    &svg,
                    crate::svg_import::SvgImportOptions::default(),
                )
                .map_err(|e| e.to_string())?;
                let count = output.widgets.len();
                let report = output.report;
                let _ = report.diagnostics_digest();
                crate::panels::templates::save_imported_svg_template(&stem, &output.widgets, &svg)
                    .map(|_| {
                        (
                            count,
                            report.skipped_element_count,
                            report.unsupported_feature_count,
                            report.fidelity,
                        )
                    })
            }) {
            Ok((count, skipped, unsupported, fidelity)) => {
                self.template_message = Some((
                    true,
                    format!(
                        "Imported SVG \"{stem}\" ({count} placeholders, skipped {skipped}, unsupported {unsupported}, fidelity {fidelity})"
                    ),
                ));
            }
            Err(e) => {
                self.template_message = Some((false, format!("SVG import failed: {e}")));
            }
        }
    }

    fn apply_ui_scale(&self, ctx: &egui::Context) {
        ctx.set_pixels_per_point(self.base_pixels_per_point * self.user_settings.ui_scale);
    }

    fn save_user_settings(&mut self) {
        self.user_settings.sanitize();
        match crate::settings::save(&self.settings_path, &self.user_settings) {
            Ok(()) => {
                self.settings_message = Some((true, "Preferences saved".to_owned()));
            }
            Err(e) => {
                self.settings_message = Some((
                    false,
                    format!("Applied this session only — save failed: {e}"),
                ));
            }
        }
    }

    fn show_preferences_window(&mut self, ctx: &egui::Context) {
        if !self.preferences_open {
            return;
        }

        let mut open = self.preferences_open;
        let mut apply_requested = false;
        let mut ok_requested = false;
        let mut cancel_requested = false;

        egui::Window::new("Preferences")
            .open(&mut open)
            .default_width(420.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Appearance");
                ui.add(
                    egui::Slider::new(&mut self.preferences_draft.ui_scale, 0.75..=1.75)
                        .text("UI scale")
                        .suffix("x"),
                )
                .on_hover_text("Scales the whole RohKai interface, similar to app zoom.");
                if ui.small_button("Reset UI scale").clicked() {
                    self.preferences_draft.ui_scale = UserSettings::default().ui_scale;
                }

                ui.separator();
                ui.heading("Editor");
                ui.add(
                    egui::Slider::new(&mut self.preferences_draft.code_font_size, 9.0..=22.0)
                        .text("Code font")
                        .suffix(" pt"),
                )
                .on_hover_text("Controls egui output and AppState preview text size.");

                ui.separator();
                ui.heading("Canvas");
                ui.add(
                    egui::Slider::new(&mut self.preferences_draft.canvas_label_scale, 0.75..=2.0)
                        .text("Widget label scale")
                        .suffix("x"),
                );
                ui.add(
                    egui::Slider::new(&mut self.preferences_draft.canvas_tag_scale, 0.75..=2.0)
                        .text("Widget tag scale")
                        .suffix("x"),
                );
                ui.add(
                    egui::Slider::new(&mut self.preferences_draft.default_snap_step, 1.0..=256.0)
                        .text("Default snap")
                        .suffix(" px"),
                )
                .on_hover_text("Applies to the current canvas and future sessions.");

                ui.separator();
                ui.heading("General");
                ui.label(egui::RichText::new("Settings file").small().weak());
                ui.label(
                    egui::RichText::new(self.settings_path.display().to_string())
                        .monospace()
                        .small(),
                );
                if ui.button("Restore Defaults").clicked() {
                    self.preferences_draft = UserSettings::default();
                }

                if let Some((ok, msg)) = &self.settings_message {
                    let color = if *ok {
                        egui::Color32::from_rgb(52, 211, 153)
                    } else {
                        egui::Color32::RED
                    };
                    ui.label(egui::RichText::new(msg.as_str()).small().color(color));
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let dirty = self.preferences_draft != self.user_settings;
                    if ui.add_enabled(dirty, egui::Button::new("Apply")).clicked() {
                        apply_requested = true;
                    }
                    if ui.button("OK").clicked() {
                        ok_requested = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_requested = true;
                    }
                });
            });

        if apply_requested || ok_requested {
            self.preferences_draft.sanitize();
            self.user_settings = self.preferences_draft.clone();
            self.apply_ui_scale(ctx);
            self.canvas_settings.snap_step = self.user_settings.default_snap_step;
            self.save_user_settings();
            if ok_requested {
                open = false;
            }
        }

        if cancel_requested || (!open && !apply_requested && !ok_requested) {
            self.preferences_draft = self.user_settings.clone();
            open = false;
        }

        self.preferences_open = open;
    }

    fn show_pending_command_dialog(&mut self, ctx: &egui::Context) {
        let Some(command) = self.pending_command else {
            return;
        };

        let command_label = match command {
            PendingCommand::New => "start a new project",
            PendingCommand::Open => "open another project",
        };

        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Save your changes before you {command_label}?"));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.cmd_save();
                        if !self.compute_dirty_exact() {
                            self.pending_command = None;
                            self.run_destructive_command(command);
                        }
                    }
                    if ui.button("Discard").clicked() {
                        self.pending_command = None;
                        self.run_destructive_command(command);
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending_command = None;
                    }
                });
            });
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "NotoSans".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Regular.ttf")),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("NotoSans".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("NotoSans".to_owned());
    ctx.set_fonts(fonts);
}

impl eframe::App for RohKaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_ui_scale(ctx);
        let now = ctx.input(|i| i.time);
        if self.export_message.is_some() {
            match self.export_message_until {
                Some(until) if now >= until => {
                    self.export_message = None;
                    self.export_message_until = None;
                }
                None => self.export_message_until = Some(now + 8.0),
                _ => {}
            }
        } else {
            self.export_message_until = None;
        }

        // ---------------------------------------------------------------
        // Global keyboard shortcuts
        // ---------------------------------------------------------------
        let ctrl_n = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::N));
        let ctrl_o = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::O));
        let ctrl_shift_s =
            ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::S));
        let ctrl_s = ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::S));
        let ctrl_shift_g =
            ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::G));
        let ctrl_g = ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::G));

        if ctrl_n {
            self.request_destructive_command(PendingCommand::New);
        } else if ctrl_o {
            self.request_destructive_command(PendingCommand::Open);
        } else if ctrl_shift_s {
            self.cmd_save_as();
        } else if ctrl_s {
            self.cmd_save();
        }

        if ctrl_g && self.selected.len() >= 2 {
            if let Some(new_id) = self.ui_tree.group(&self.selected) {
                self.selected.clear();
                self.selected.push(new_id);
            }
        } else if ctrl_shift_g {
            let frame_ids: Vec<Uuid> = self
                .selected
                .iter()
                .filter(|&&id| {
                    self.ui_tree
                        .widgets
                        .iter()
                        .find(|w| w.id == id)
                        .map(|w| w.kind == WidgetKind::Frame)
                        .unwrap_or(false)
                })
                .copied()
                .collect();
            if !frame_ids.is_empty() {
                let mut new_sel = Vec::new();
                for frame_id in frame_ids {
                    let children = self.ui_tree.ungroup(frame_id);
                    new_sel.extend(children);
                }
                self.selected = new_sel;
            }
        }

        // ---------------------------------------------------------------
        // Window title
        // ---------------------------------------------------------------
        let dirty = self.cached_dirty(ctx);
        let title = match &self.current_file {
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("untitled");
                if dirty {
                    format!("Rohkai — {name} *")
                } else {
                    format!("Rohkai — {name}")
                }
            }
            None => {
                if dirty {
                    "Rohkai — unsaved *".to_owned()
                } else {
                    "Rohkai".to_owned()
                }
            }
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // ---------------------------------------------------------------
        // Menu bar ribbon
        // ---------------------------------------------------------------
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // File menu
                ui.menu_button("File", |ui| {
                    if ui
                        .add(egui::Button::new("New").shortcut_text("Ctrl+N"))
                        .clicked()
                    {
                        self.request_destructive_command(PendingCommand::New);
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Open…").shortcut_text("Ctrl+O"))
                        .clicked()
                    {
                        self.request_destructive_command(PendingCommand::Open);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add(egui::Button::new("Save").shortcut_text("Ctrl+S"))
                        .clicked()
                    {
                        self.cmd_save();
                        ui.close_menu();
                    }
                    if ui
                        .add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S"))
                        .clicked()
                    {
                        self.cmd_save_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export Project…").clicked() {
                        self.cmd_export();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.selected.is_empty(),
                            egui::Button::new("Save as Template…"),
                        )
                        .on_hover_text("Save selected widgets as a reusable .rktp template")
                        .clicked()
                    {
                        self.cmd_save_template();
                        ui.close_menu();
                    }
                    if ui
                        .button("Import SVG as Template…")
                        .on_hover_text("Parse an SVG into draggable RohKai placeholder widgets")
                        .clicked()
                    {
                        self.cmd_import_svg_template();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Preferences…").clicked() {
                        self.preferences_draft = self.user_settings.clone();
                        self.preferences_open = true;
                        ui.close_menu();
                    }
                });

                ui.separator();

                // Inline app title editor
                ui.label(egui::RichText::new("Title:").small().weak());
                ui.add(
                    egui::TextEdit::singleline(&mut self.ui_tree.app_props.title)
                        .desired_width(110.0)
                        .hint_text("App title"),
                );

                ui.separator();

                // Zoom indicator + reset
                let zoom_pct = (self.canvas_settings.zoom * 100.0).round() as u32;
                ui.label(
                    egui::RichText::new(format!("{zoom_pct}%"))
                        .small()
                        .color(egui::Color32::from_gray(180)),
                );
                if ui
                    .small_button("⟲")
                    .on_hover_text("Reset zoom & pan (Ctrl+0)")
                    .clicked()
                {
                    self.canvas_settings.zoom = 1.0;
                    self.canvas_settings.pan = egui::Vec2::ZERO;
                }

                // File dirty indicator
                ui.separator();
                match &self.current_file {
                    Some(p) => {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("untitled");
                        let label = if dirty {
                            format!("{name} *")
                        } else {
                            name.to_owned()
                        };
                        ui.label(egui::RichText::new(label).weak());
                    }
                    None if dirty => {
                        ui.label(egui::RichText::new("unsaved *").weak());
                    }
                    None => {}
                }
            });
        });

        // ---------------------------------------------------------------
        // Bottom status bar — canvas size + grid snap
        // ---------------------------------------------------------------
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("W").small().weak());
                ui.add(
                    egui::DragValue::new(&mut self.ui_tree.app_props.win_w)
                        .speed(1.0)
                        .range(100.0..=4000.0),
                );
                ui.label(egui::RichText::new("H").small().weak());
                ui.add(
                    egui::DragValue::new(&mut self.ui_tree.app_props.win_h)
                        .speed(1.0)
                        .range(100.0..=4000.0),
                );
                ui.separator();
                let (snap_text, snap_color) = if self.canvas_settings.snap_enabled {
                    ("Grid: ON", egui::Color32::from_rgb(52, 211, 153))
                } else {
                    ("Grid: OFF", egui::Color32::from_gray(120))
                };
                ui.label(egui::RichText::new(snap_text).small().color(snap_color));
                ui.add(
                    egui::DragValue::new(&mut self.canvas_settings.snap_step)
                        .speed(0.5)
                        .range(1.0..=256.0)
                        .suffix("px"),
                );
                ui.label(egui::RichText::new("[G]").small().weak());

                let mut clear_error = false;
                if let Some(err) = self.last_error.as_deref() {
                    ui.separator();
                    ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
                    clear_error = ui
                        .small_button("x")
                        .on_hover_text("Dismiss error")
                        .clicked();
                }

                let mut clear_export = false;
                if let Some((ok, msg)) = self.export_message.as_ref() {
                    ui.separator();
                    let color = if *ok {
                        egui::Color32::from_rgb(52, 211, 153)
                    } else {
                        egui::Color32::RED
                    };
                    ui.label(egui::RichText::new(msg.as_str()).color(color).small());
                    clear_export = ui
                        .small_button("x")
                        .on_hover_text("Dismiss export message")
                        .clicked();
                }

                if clear_error {
                    self.last_error = None;
                }
                if clear_export {
                    self.export_message = None;
                    self.export_message_until = None;
                }
            });
        });

        // ---------------------------------------------------------------
        // Right panel: generated code
        // ---------------------------------------------------------------
        crate::panels::code_preview::show(
            ctx,
            &mut self.ui_tree,
            CodePreviewArgs {
                highlighted_id: self.highlighted_code_id,
                scroll_to: &mut self.scroll_to_code,
                scroll_to_handler: &mut self.scroll_to_handler,
                code_buffer: &mut self.code_buffer,
                code_status: &mut self.code_status,
                last_generated: &mut self.last_generated,
                split_ratio: &mut self.code_split_ratio,
                code_font_size: self.user_settings.code_font_size,
            },
        );

        // ---------------------------------------------------------------
        // Left panel — returns (palette_drag, template_action) to process outside
        // ---------------------------------------------------------------
        let window_w = ctx.screen_rect().width();
        let left_full = window_w >= 580.0;

        let (palette_click, palette_drag, tmpl_action, props_action) =
            egui::SidePanel::left("left_panel")
                .min_width(if left_full { 160.0 } else { 28.0 })
                .max_width(if left_full { f32::INFINITY } else { 28.0 })
                .show(ctx, |ui| {
                    if !left_full {
                        ui.centered_and_justified(|ui| {
                            ui.label(egui::RichText::new("«").weak());
                        });
                        return (
                            None,
                            None,
                            crate::panels::templates::TemplateAction::None,
                            crate::panels::properties::PropertiesAction::None,
                        );
                    }

                    let (palette_click, palette_drag) = crate::panels::palette::show_content(ui);
                    ui.add_space(4.0);

                    let shift_held = ui.input(|i| i.modifiers.shift);
                    let mut props_action = crate::panels::properties::PropertiesAction::None;
                    egui::CollapsingHeader::new("Properties")
                        .default_open(true)
                        .show(ui, |ui| {
                            props_action = crate::panels::properties::show_content(
                                ui,
                                &mut self.ui_tree,
                                &mut self.selected,
                                shift_held,
                            );
                        });

                    ui.add_space(4.0);
                    ui.separator();

                    let mut tmpl_action = crate::panels::templates::TemplateAction::None;
                    egui::CollapsingHeader::new("Templates")
                        .default_open(true)
                        .show(ui, |ui| {
                            tmpl_action =
                                crate::panels::templates::show(ui, &mut self.template_message);
                        });

                    (palette_click, palette_drag, tmpl_action, props_action)
                })
                .inner;

        // Tracé — properties panel requested scroll-to-handler
        if let crate::panels::properties::PropertiesAction::ScrollToHandler(name) = props_action {
            self.scroll_to_handler = Some(name);
        }

        // Palette click → place at viewport center (accounting for zoom/pan)
        if let Some(mut instance) = palette_click {
            let zoom = self.canvas_settings.zoom;
            let pan = self.canvas_settings.pan;
            let win_w = self.ui_tree.app_props.win_w;
            let win_h = self.ui_tree.app_props.win_h;
            let cx = (-pan.x / zoom + win_w / 2.0).clamp(0.0, win_w);
            let cy = (-pan.y / zoom + win_h / 2.0).clamp(0.0, win_h);
            instance.rect.x = (cx - instance.rect.w / 2.0).max(0.0);
            instance.rect.y = (cy - instance.rect.h / 2.0).max(0.0);
            self.ui_tree.add(instance);
        }

        // Palette drag — set interaction.template_drag for canvas drop next frame
        if let Some(instance) = palette_drag {
            self.interaction.template_drag = Some(vec![instance]);
        }

        // Process template action BEFORE canvas handle() so AddAtCenter never enters
        // the drag-drop path (which runs in the same frame as primary_released)
        match tmpl_action {
            crate::panels::templates::TemplateAction::AddAtCenter(instances) => {
                let cx = self.ui_tree.app_props.win_w / 2.0;
                let cy = self.ui_tree.app_props.win_h / 2.0;
                let min_x = instances.iter().map(|w| w.rect.x).fold(f32::MAX, f32::min);
                let min_y = instances.iter().map(|w| w.rect.y).fold(f32::MAX, f32::min);
                for mut w in instances {
                    w.id = Uuid::new_v4();
                    w.rect.x = (w.rect.x - min_x + cx).max(0.0);
                    w.rect.y = (w.rect.y - min_y + cy).max(0.0);
                    self.ui_tree.add(w);
                }
            }
            crate::panels::templates::TemplateAction::BeginDrag(instances) => {
                self.interaction.template_drag = Some(instances);
            }
            crate::panels::templates::TemplateAction::None => {}
        }

        // ---------------------------------------------------------------
        // Canvas
        // ---------------------------------------------------------------
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::canvas::interaction::handle(
                ui,
                &mut self.ui_tree,
                &mut self.interaction,
                &mut self.selected,
                &mut self.canvas_settings,
                crate::canvas::interaction::CanvasTextSettings {
                    label_scale: self.user_settings.canvas_label_scale,
                    tag_scale: self.user_settings.canvas_tag_scale,
                },
            );
        });

        // ---------------------------------------------------------------
        // Post-canvas: read per-frame signals
        // ---------------------------------------------------------------

        // Lazare double-click → highlight code panel
        if let Some(id) = self.interaction.double_clicked_widget {
            self.highlighted_code_id = Some(id);
            self.scroll_to_code = true;
        }

        // ---------------------------------------------------------------
        // Delete key
        // ---------------------------------------------------------------
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            let ids: Vec<Uuid> = self.selected.drain(..).collect();
            for id in ids {
                self.ui_tree.remove(id);
            }
        }

        self.show_pending_command_dialog(ctx);
        self.show_preferences_window(ctx);
    }
}
