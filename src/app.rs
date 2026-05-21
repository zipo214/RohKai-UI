use crate::canvas::interaction::{CanvasSettings, InteractionState};
use crate::project::schema::WidgetInstance;
use crate::project::ui_tree::UiTree;
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
    /// Widget id highlighted in the code panel (Lazare double-click)
    pub highlighted_code_id: Option<Uuid>,
    /// When true, code panel scrolls to the highlighted line once
    pub scroll_to_code: bool,
    /// Status message from template operations
    pub template_message: Option<(bool, String)>,
    pending_command: Option<PendingCommand>,
}

impl RohKaiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        Self {
            ui_tree: UiTree::default(),
            interaction: InteractionState::default(),
            selected: Vec::new(),
            current_file: None,
            saved_json: None,
            last_error: None,
            canvas_settings: CanvasSettings::default(),
            export_message: None,
            highlighted_code_id: None,
            scroll_to_code: false,
            template_message: None,
            pending_command: None,
        }
    }

    fn is_dirty(&self) -> bool {
        let current = crate::project::io::serialize(&self.ui_tree).unwrap_or_default();
        match &self.saved_json {
            Some(snap) => current != *snap,
            None => !self.ui_tree.widgets.is_empty(),
        }
    }

    fn do_save(&mut self, path: PathBuf) {
        match crate::project::io::save(&path, &self.ui_tree) {
            Ok(json) => {
                self.saved_json = Some(json);
                self.current_file = Some(path);
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(e),
        }
    }

    fn request_destructive_command(&mut self, command: PendingCommand) {
        if self.is_dirty() {
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
                        if !self.is_dirty() {
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
        // ---------------------------------------------------------------
        // Global keyboard shortcuts
        // ---------------------------------------------------------------
        let ctrl_n = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::N));
        let ctrl_o = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::O));
        let ctrl_shift_s =
            ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::S));
        let ctrl_s = ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::S));

        if ctrl_n {
            self.request_destructive_command(PendingCommand::New);
        } else if ctrl_o {
            self.request_destructive_command(PendingCommand::Open);
        } else if ctrl_shift_s {
            self.cmd_save_as();
        } else if ctrl_s {
            self.cmd_save();
        }

        // ---------------------------------------------------------------
        // Window title
        // ---------------------------------------------------------------
        let dirty = self.is_dirty();
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

                // Error / export status
                if let Some(err) = &self.last_error {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(err.as_str())
                            .color(egui::Color32::RED)
                            .small(),
                    );
                }
                if let Some((ok, msg)) = &self.export_message {
                    ui.separator();
                    let color = if *ok {
                        egui::Color32::from_rgb(52, 211, 153)
                    } else {
                        egui::Color32::RED
                    };
                    ui.label(egui::RichText::new(msg.as_str()).color(color).small());
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
            });
        });

        // ---------------------------------------------------------------
        // Right panel: generated code
        // ---------------------------------------------------------------
        crate::panels::code_preview::show(
            ctx,
            &self.ui_tree,
            self.highlighted_code_id,
            &mut self.scroll_to_code,
        );

        // ---------------------------------------------------------------
        // Left panel — returns (palette_drag, template_action) to process outside
        // ---------------------------------------------------------------
        let window_w = ctx.screen_rect().width();
        let left_full = window_w >= 580.0;

        let (palette_drag, tmpl_action) = egui::SidePanel::left("left_panel")
            .min_width(if left_full { 160.0 } else { 28.0 })
            .max_width(if left_full { f32::INFINITY } else { 28.0 })
            .show(ctx, |ui| {
                if !left_full {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("«").weak());
                    });
                    return (None, crate::panels::templates::TemplateAction::None);
                }

                let palette_drag = crate::panels::palette::show_content(ui, &mut self.ui_tree);
                ui.add_space(4.0);

                egui::CollapsingHeader::new("Properties")
                    .default_open(true)
                    .show(ui, |ui| {
                        crate::panels::properties::show_content(
                            ui,
                            &mut self.ui_tree,
                            &mut self.selected,
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

                (palette_drag, tmpl_action)
            })
            .inner;

        // Process palette drag — set interaction.template_drag for canvas drop next frame
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
    }
}
