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

struct PendingSvgImport {
    stem: String,
    svg_text: String,
}

// ---------------------------------------------------------------------------
// RohKaiApp state sub-structs
// ---------------------------------------------------------------------------

/// The persistent project document: tree, file path, last-saved snapshot.
pub struct ProjectState {
    pub ui_tree: UiTree,
    pub current_file: Option<PathBuf>,
    pub saved_json: Option<String>,
}

/// Per-session interaction and canvas view state (not persisted).
pub struct SessionState {
    pub interaction: InteractionState,
    pub selected: Vec<Uuid>,
    pub canvas_settings: CanvasSettings,
    /// Widget id highlighted in the code panel (Lazare double-click).
    pub highlighted_code_id: Option<Uuid>,
    /// When true, code panel scrolls to the highlighted line once.
    pub scroll_to_code: bool,
    /// Tracé: if Some(name), code panel scrolls to fn {name} and inserts a stub if absent.
    pub scroll_to_handler: Option<String>,
    /// Last known screen rect of the canvas panel — used for visible-centre placement.
    pub last_canvas_rect: egui::Rect,
    /// SVG source viewer: widget id whose source is currently displayed, if any.
    pub svg_viewer_id: Option<Uuid>,
    /// Guide currently under the pointer (for highlight + drag/delete).
    pub hovered_guide: Option<Uuid>,
    /// Guide currently being dragged.
    pub dragging_guide: Option<Uuid>,
    /// Whether the theme settings window is open.
    pub theme_open: bool,
    /// Lock canvas W:H ratio when editing canvas size in the status bar.
    pub lock_aspect_ratio: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            interaction: InteractionState::default(),
            selected: Vec::new(),
            canvas_settings: CanvasSettings::default(),
            highlighted_code_id: None,
            scroll_to_code: false,
            scroll_to_handler: None,
            last_canvas_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            svg_viewer_id: None,
            hovered_guide: None,
            dragging_guide: None,
            theme_open: false,
            lock_aspect_ratio: false,
        }
    }
}

/// Ephemeral one-frame status messages shown in the status bar.
#[derive(Default)]
pub struct MessageState {
    pub last_error: Option<String>,
    /// (true = success) message after export.
    pub export_message: Option<(bool, String)>,
    pub export_message_until: Option<f64>,
    /// Status message from template operations.
    pub template_message: Option<(bool, String)>,
}

/// User preferences — live settings, draft copy, persistence path.
pub struct PreferencesState {
    pub user_settings: UserSettings,
    pub draft: UserSettings,
    pub settings_path: PathBuf,
    pub settings_message: Option<(bool, String)>,
    pub open: bool,
}

/// Lazare code-panel state.
pub struct CodePanelState {
    pub buffer: String,
    pub status: CodeStatus,
    pub last_generated: String,
    pub split_ratio: f32,
}

impl Default for CodePanelState {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            status: CodeStatus::Live,
            last_generated: String::new(),
            split_ratio: 0.6,
        }
    }
}

/// Loaded `.rkwd` widget descriptors and any non-fatal load errors.
pub struct DescriptorState {
    pub widgets: Vec<crate::codegen::widget_descriptor::WidgetDescriptor>,
    pub errors: Vec<String>,
    /// In-app descriptor editor — Some while window is open.
    pub editor: Option<crate::panels::descriptor_editor::DescriptorEditorState>,
    /// Beginner widget builder — Some while window is open.
    pub builder: Option<crate::panels::widget_builder::WidgetBuilderState>,
}

// ---------------------------------------------------------------------------

pub struct RohKaiApp {
    pub project: ProjectState,
    pub session: SessionState,
    pub messages: MessageState,
    pub prefs: PreferencesState,
    pub code: CodePanelState,
    pub descriptors: DescriptorState,
    pub svg_texture_cache: crate::canvas::interaction::SvgTextureCache,
    pending_command: Option<PendingCommand>,
    pending_svg_import: Option<PendingSvgImport>,
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
        let (widget_descriptors, descriptor_errors) =
            crate::codegen::widget_descriptor::load_from_widgets_dir();
        Self {
            project: ProjectState {
                ui_tree: UiTree::default(),
                current_file: None,
                saved_json: None,
            },
            session: SessionState {
                canvas_settings,
                ..Default::default()
            },
            messages: MessageState::default(),
            prefs: PreferencesState {
                draft: user_settings.clone(),
                settings_path: loaded_settings.path,
                settings_message: loaded_settings.error.map(|e| (false, e)),
                open: false,
                user_settings,
            },
            code: CodePanelState::default(),
            descriptors: DescriptorState {
                widgets: widget_descriptors,
                errors: descriptor_errors,
                editor: None,
                builder: None,
            },
            svg_texture_cache: crate::canvas::interaction::SvgTextureCache::default(),
            pending_command: None,
            pending_svg_import: None,
            base_pixels_per_point,
            dirty_cache: false,
            dirty_cache_checked_at: 0.0,
        }
    }

    fn compute_dirty_exact(&self) -> bool {
        let current = crate::project::io::serialize(&self.project.ui_tree).unwrap_or_default();
        match &self.project.saved_json {
            Some(snap) => current != *snap,
            None => !self.project.ui_tree.widgets.is_empty(),
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
        match crate::project::io::save(&path, &self.project.ui_tree) {
            Ok(json) => {
                self.project.saved_json = Some(json);
                self.project.current_file = Some(path);
                self.messages.last_error = None;
                self.dirty_cache = false;
                self.dirty_cache_checked_at = 0.0;
            }
            Err(e) => self.messages.last_error = Some(e),
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
        self.project.ui_tree = UiTree::default();
        self.project.current_file = None;
        self.project.saved_json = None;
        self.session.selected.clear();
        self.messages.last_error = None;
        self.session.highlighted_code_id = None;
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
                    self.project.ui_tree = tree;
                    self.project.saved_json = Some(snap);
                    self.project.current_file = Some(path);
                    self.session.selected.clear();
                    self.messages.last_error = None;
                    self.session.highlighted_code_id = None;
                    self.dirty_cache = false;
                    self.dirty_cache_checked_at = 0.0;
                }
                Err(e) => self.messages.last_error = Some(e),
            }
        }
    }

    fn cmd_save(&mut self) {
        if let Some(path) = self.project.current_file.clone() {
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
        self.messages.export_message = None;
        self.messages.export_message_until = None;
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Export project — choose destination folder")
            .pick_folder()
        else {
            return;
        };
        match crate::codegen::export::write_project(&self.project.ui_tree, &folder) {
            Ok(()) => {
                self.messages.export_message =
                    Some((true, format!("Exported → {}", folder.display())));
            }
            Err(e) => {
                self.messages.export_message = Some((false, format!("Export failed: {e}")));
            }
        }
    }

    fn cmd_save_template(&mut self) {
        self.messages.template_message = None;
        if self.session.selected.is_empty() {
            return;
        }

        let widgets: Vec<WidgetInstance> = self
            .session
            .selected
            .iter()
            .filter_map(|&id| {
                self.project
                    .ui_tree
                    .widgets
                    .iter()
                    .find(|w| w.id == id)
                    .cloned()
            })
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
                    self.messages.template_message = Some((true, format!("Saved \"{stem}\"")));
                }
                Err(e) => {
                    self.messages.template_message = Some((false, format!("Save failed: {e}")));
                }
            }
        }
    }

    fn cmd_import_svg_template(&mut self) {
        self.messages.template_message = None;
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import SVG")
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
        match std::fs::read_to_string(&path) {
            Ok(svg_text) => {
                self.pending_svg_import = Some(PendingSvgImport { stem, svg_text });
            }
            Err(e) => {
                self.messages.template_message = Some((false, format!("SVG read failed: {e}")));
            }
        }
    }

    fn cmd_reload_descriptors(&mut self) {
        let (widgets, errors) = crate::codegen::widget_descriptor::load_from_widgets_dir();
        self.descriptors.widgets = widgets;
        self.descriptors.errors = errors;
    }

    fn widgets_dir() -> Option<std::path::PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("widgets")))
    }

    fn cmd_new_descriptor(&mut self) {
        self.descriptors.editor =
            Some(crate::panels::descriptor_editor::DescriptorEditorState::new_blank());
    }

    fn cmd_new_widget_builder(&mut self) {
        self.descriptors.builder = Some(crate::panels::widget_builder::WidgetBuilderState::new());
    }

    fn cmd_edit_descriptor(&mut self, id: &str) {
        let state = if let Some(d) = self.descriptors.widgets.iter().find(|d| d.id == id) {
            crate::panels::descriptor_editor::DescriptorEditorState::from_descriptor(d)
        } else {
            crate::panels::descriptor_editor::DescriptorEditorState::new_blank()
        };
        self.descriptors.editor = Some(state);
    }

    fn cmd_import_widget_definition(&mut self) {
        let Some(src_path) = rfd::FileDialog::new()
            .set_title("Import Widget Definition")
            .add_filter("RohKai Widget Descriptor", &["rkwd"])
            .pick_file()
        else {
            return;
        };

        // Read and validate before copying.
        let raw = match std::fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(e) => {
                self.messages.template_message = Some((false, format!("Could not read file: {e}")));
                return;
            }
        };
        let descriptor: crate::codegen::widget_descriptor::WidgetDescriptor =
            match serde_json::from_str(&raw) {
                Ok(d) => d,
                Err(e) => {
                    self.messages.template_message =
                        Some((false, format!("Invalid .rkwd JSON: {e}")));
                    return;
                }
            };
        if descriptor.schema_version != 1 {
            self.messages.template_message = Some((
                false,
                format!(
                    "Unsupported schema_version {} (expected 1)",
                    descriptor.schema_version
                ),
            ));
            return;
        }

        // Determine destination: <binary_dir>/widgets/<filename>.rkwd
        let dest_dir = match std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("widgets")))
        {
            Some(d) => d,
            None => {
                self.messages.template_message =
                    Some((false, "Could not determine binary path".to_owned()));
                return;
            }
        };
        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
            self.messages.template_message =
                Some((false, format!("Could not create widgets/ dir: {e}")));
            return;
        }
        let file_name = src_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.rkwd", descriptor.id.replace('.', "-")));
        let dest_path = dest_dir.join(&file_name);
        if let Err(e) = std::fs::copy(&src_path, &dest_path) {
            self.messages.template_message = Some((false, format!("Copy failed: {e}")));
            return;
        }

        // Reload so the new descriptor is immediately available.
        let (widgets, errors) = crate::codegen::widget_descriptor::load_from_widgets_dir();
        self.descriptors.widgets = widgets;
        self.descriptors.errors = errors;
        self.messages.template_message = Some((
            true,
            format!("Imported \"{}\" successfully", descriptor.name),
        ));
    }

    fn cmd_export_widget_bundle(&mut self) {
        if self.descriptors.widgets.is_empty() {
            self.messages.template_message =
                Some((false, "No descriptors loaded to bundle.".to_owned()));
            return;
        }
        let Some(dest) = rfd::FileDialog::new()
            .set_title("Export Widget Bundle")
            .set_file_name("widgets.rkwb")
            .add_filter("RohKai Widget Bundle", &["rkwb"])
            .save_file()
        else {
            return;
        };
        let bundle = crate::codegen::widget_bundle::WidgetBundle::from_descriptors(
            &self.descriptors.widgets,
        );
        match bundle.to_json() {
            Ok(json) => match std::fs::write(&dest, json) {
                Ok(()) => {
                    let n = bundle.descriptors.len();
                    self.messages.template_message = Some((
                        true,
                        format!("Exported {n} descriptor(s) to {}", dest.display()),
                    ));
                }
                Err(e) => {
                    self.messages.template_message = Some((false, format!("Write failed: {e}")));
                }
            },
            Err(e) => {
                self.messages.template_message = Some((false, format!("Serialization error: {e}")));
            }
        }
    }

    fn cmd_import_widget_bundle(&mut self) {
        let Some(src) = rfd::FileDialog::new()
            .set_title("Import Widget Bundle")
            .add_filter("RohKai Widget Bundle", &["rkwb"])
            .pick_file()
        else {
            return;
        };
        let raw = match std::fs::read_to_string(&src) {
            Ok(s) => s,
            Err(e) => {
                self.messages.template_message = Some((false, format!("Could not read file: {e}")));
                return;
            }
        };
        let bundle = match crate::codegen::widget_bundle::WidgetBundle::from_json(&raw) {
            Ok(b) => b,
            Err(e) => {
                self.messages.template_message = Some((false, format!("Invalid bundle: {e}")));
                return;
            }
        };
        let dest_dir = match Self::widgets_dir() {
            Some(d) => d,
            None => {
                self.messages.template_message =
                    Some((false, "Could not determine binary path".to_owned()));
                return;
            }
        };
        match bundle.extract_to(&dest_dir) {
            Ok(ids) => {
                let n = ids.len();
                let (widgets, errors) = crate::codegen::widget_descriptor::load_from_widgets_dir();
                self.descriptors.widgets = widgets;
                self.descriptors.errors = errors;
                self.messages.template_message =
                    Some((true, format!("Imported {n} descriptor(s) from bundle")));
            }
            Err(e) => {
                self.messages.template_message = Some((false, format!("Extract failed: {e}")));
            }
        }
    }

    /// Translate and optionally scale `widgets` so their bounding-box center lands at the
    /// currently visible canvas centre, shrinking proportionally if larger than 80 % of the
    /// visible area. Mutates rect in-place; does **not** assign new IDs.
    fn place_at_visible_center(&self, widgets: &mut [WidgetInstance]) {
        if widgets.is_empty() {
            return;
        }
        let zoom = self.session.canvas_settings.zoom;
        let pan = self.session.canvas_settings.pan;
        let win_w = self.project.ui_tree.app_props.win_w;
        let win_h = self.project.ui_tree.app_props.win_h;

        // Visible canvas centre in canvas coordinates.
        // Formula mirrors the palette-click placement in the update loop.
        let cv_cx = -pan.x / zoom + win_w / 2.0;
        let cv_cy = -pan.y / zoom + win_h / 2.0;

        // Visible canvas dimensions in canvas coordinates.
        let vis_w = self.session.last_canvas_rect.width() / zoom;
        let vis_h = self.session.last_canvas_rect.height() / zoom;

        // Bounding box of all imported widgets.
        let min_x = widgets.iter().map(|w| w.rect.x).fold(f32::MAX, f32::min);
        let min_y = widgets.iter().map(|w| w.rect.y).fold(f32::MAX, f32::min);
        let max_x = widgets
            .iter()
            .map(|w| w.rect.x + w.rect.w)
            .fold(f32::MIN, f32::max);
        let max_y = widgets
            .iter()
            .map(|w| w.rect.y + w.rect.h)
            .fold(f32::MIN, f32::max);

        let group_w = (max_x - min_x).max(1.0);
        let group_h = (max_y - min_y).max(1.0);
        let group_cx = (min_x + max_x) / 2.0;
        let group_cy = (min_y + max_y) / 2.0;

        // Scale down proportionally if the group exceeds 80 % of the visible area.
        let max_w = vis_w * 0.8;
        let max_h = vis_h * 0.8;
        let scale = if group_w > max_w || group_h > max_h {
            (max_w / group_w).min(max_h / group_h)
        } else {
            1.0
        };

        for w in widgets.iter_mut() {
            let rel_x = w.rect.x - group_cx;
            let rel_y = w.rect.y - group_cy;
            w.rect.x = cv_cx + rel_x * scale;
            w.rect.y = cv_cy + rel_y * scale;
            w.rect.w *= scale;
            w.rect.h *= scale;
        }
    }

    fn do_svg_import(
        &mut self,
        stem: &str,
        svg_text: &str,
        mode: crate::svg_import::SvgImportMode,
    ) {
        let opts = crate::svg_import::SvgImportOptions {
            mode,
            limits: crate::svg_import::SvgImportLimits::default(),
        };

        // Parse SVG first; bail on error before touching canvas or disk.
        let output = match crate::svg_import::import_svg_template(svg_text, opts) {
            Ok(o) => o,
            Err(e) => {
                self.messages.template_message = Some((false, format!("SVG import failed: {e}")));
                return;
            }
        };

        let count = output.widgets.len();
        let skipped = output.report.skipped_element_count;
        let unsupported = output.report.unsupported_feature_count;
        let fidelity = output.report.fidelity;

        // Save template for later reuse from the Templates panel.
        let save_ok =
            crate::panels::templates::save_imported_svg_template(stem, &output.widgets, svg_text)
                .is_ok();

        // Place a copy on the canvas immediately, centred in the visible area.
        let mut to_place = output.widgets.clone();
        self.place_at_visible_center(&mut to_place);
        for mut w in to_place {
            w.id = Uuid::new_v4();
            self.project.ui_tree.add(w);
        }

        let suffix = if save_ok {
            ""
        } else {
            " (template save failed)"
        };
        self.messages.template_message = Some((
            save_ok,
            format!(
                "Imported \"{stem}\" ({count} item(s), skipped {skipped}, unsupported {unsupported}, fidelity {fidelity}){suffix}"
            ),
        ));
    }

    fn show_svg_import_modal(&mut self, ctx: &egui::Context) {
        let Some(ref pending) = self.pending_svg_import else {
            return;
        };
        let stem = pending.stem.clone();
        let svg_text = pending.svg_text.clone();

        egui::Window::new("Import SVG")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(320.0);
                ui.label(format!("How should \"{stem}\" be imported?"));
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        [300.0, 36.0],
                        egui::Button::new("Image  -  source-backed preview node"),
                    )
                    .clicked()
                {
                    self.pending_svg_import = None;
                    self.do_svg_import(&stem, &svg_text, crate::svg_import::SvgImportMode::Image);
                }
                ui.add_space(4.0);
                if ui
                    .add_sized(
                        [300.0, 36.0],
                        egui::Button::new("Components  -  editable frame per shape"),
                    )
                    .clicked()
                {
                    self.pending_svg_import = None;
                    self.do_svg_import(
                        &stem,
                        &svg_text,
                        crate::svg_import::SvgImportMode::Components,
                    );
                }
                ui.add_space(8.0);
                if ui.button("Cancel").clicked() {
                    self.pending_svg_import = None;
                }
            });
    }

    fn apply_ui_scale(&self, ctx: &egui::Context) {
        ctx.set_pixels_per_point(self.base_pixels_per_point * self.prefs.user_settings.ui_scale);
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let theme = &self.project.ui_tree.app_props.theme;
        let mut visuals = if theme.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        let [r, g, b] = theme.accent_color;
        let accent = egui::Color32::from_rgb(r, g, b);
        visuals.hyperlink_color = accent;
        visuals.selection.bg_fill = egui::Color32::from_rgba_premultiplied(r, g, b, 90);
        if let Some(rounding) = theme.global_corner_radius {
            let r = egui::Rounding::same(rounding);
            visuals.widgets.noninteractive.rounding = r;
            visuals.widgets.inactive.rounding = r;
            visuals.widgets.hovered.rounding = r;
            visuals.widgets.active.rounding = r;
            visuals.widgets.open.rounding = r;
        }
        ctx.set_visuals(visuals);
        if theme.base_font_size.is_some() || theme.spacing_scale.is_some() {
            let mut style = (*ctx.style()).clone();
            if let Some(fs) = theme.base_font_size {
                for font_id in style.text_styles.values_mut() {
                    font_id.size = fs;
                }
            }
            if let Some(scale) = theme.spacing_scale {
                let base = 4.0 * scale;
                style.spacing.item_spacing = egui::vec2(base * 2.0, base);
                style.spacing.button_padding = egui::vec2(base * 1.5, base * 0.5);
            }
            ctx.set_style(style);
        }
    }

    fn save_user_settings(&mut self) {
        self.prefs.user_settings.sanitize();
        match crate::settings::save(&self.prefs.settings_path, &self.prefs.user_settings) {
            Ok(()) => {
                self.prefs.settings_message = Some((true, "Preferences saved".to_owned()));
            }
            Err(e) => {
                self.prefs.settings_message = Some((
                    false,
                    format!("Applied this session only — save failed: {e}"),
                ));
            }
        }
    }

    fn show_preferences_window(&mut self, ctx: &egui::Context) {
        if !self.prefs.open {
            return;
        }

        let mut open = self.prefs.open;
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
                    egui::Slider::new(&mut self.prefs.draft.ui_scale, 0.75..=1.75)
                        .text("UI scale")
                        .suffix("x"),
                )
                .on_hover_text("Scales the whole RohKai interface, similar to app zoom.");
                if ui.small_button("Reset UI scale").clicked() {
                    self.prefs.draft.ui_scale = UserSettings::default().ui_scale;
                }

                ui.separator();
                ui.heading("Editor");
                ui.add(
                    egui::Slider::new(&mut self.prefs.draft.code_font_size, 9.0..=22.0)
                        .text("Code font")
                        .suffix(" pt"),
                )
                .on_hover_text("Controls egui output and AppState preview text size.");

                ui.separator();
                ui.heading("Canvas");
                ui.add(
                    egui::Slider::new(&mut self.prefs.draft.canvas_label_scale, 0.75..=2.0)
                        .text("Widget label scale")
                        .suffix("x"),
                );
                ui.add(
                    egui::Slider::new(&mut self.prefs.draft.canvas_tag_scale, 0.75..=2.0)
                        .text("Widget tag scale")
                        .suffix("x"),
                );
                ui.add(
                    egui::Slider::new(&mut self.prefs.draft.default_snap_step, 1.0..=256.0)
                        .text("Default snap")
                        .suffix(" px"),
                )
                .on_hover_text("Applies to the current canvas and future sessions.");

                ui.separator();
                ui.heading("General");
                ui.label(egui::RichText::new("Settings file").small().weak());
                ui.label(
                    egui::RichText::new(self.prefs.settings_path.display().to_string())
                        .monospace()
                        .small(),
                );
                if ui.button("Restore Defaults").clicked() {
                    self.prefs.draft = UserSettings::default();
                }

                if let Some((ok, msg)) = &self.prefs.settings_message {
                    let color = if *ok {
                        egui::Color32::from_rgb(52, 211, 153)
                    } else {
                        egui::Color32::RED
                    };
                    ui.label(egui::RichText::new(msg.as_str()).small().color(color));
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let dirty = self.prefs.draft != self.prefs.user_settings;
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
            self.prefs.draft.sanitize();
            self.prefs.user_settings = self.prefs.draft.clone();
            self.apply_ui_scale(ctx);
            self.session.canvas_settings.snap_step = self.prefs.user_settings.default_snap_step;
            self.save_user_settings();
            if ok_requested {
                open = false;
            }
        }

        if cancel_requested || (!open && !apply_requested && !ok_requested) {
            self.prefs.draft = self.prefs.user_settings.clone();
            open = false;
        }

        self.prefs.open = open;
    }

    fn show_svg_source_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.session.svg_viewer_id else {
            return;
        };
        let svg = self
            .project
            .ui_tree
            .widgets
            .iter()
            .find(|w| w.id == id)
            .and_then(|w| w.svg_source.as_deref())
            .unwrap_or("")
            .to_owned();

        let mut open = true;
        let byte_count = svg.len();
        egui::Window::new(format!("SVG Source — {byte_count} bytes"))
            .id(egui::Id::new("svg_source_viewer"))
            .open(&mut open)
            .resizable(true)
            .default_size([600.0, 400.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Read-only. Copy text to use elsewhere.")
                            .small()
                            .weak(),
                    );
                    if ui.small_button("Copy all").clicked() {
                        ui.output_mut(|o| o.copied_text = svg.clone());
                    }
                });
                ui.separator();
                egui::ScrollArea::both()
                    .id_salt("svg_src_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        // Read-only: pass a temporary clone so the source is never mutated.
                        let mut display = svg.clone();
                        ui.add(
                            egui::TextEdit::multiline(&mut display)
                                .font(egui::FontId::monospace(11.0))
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });
            });

        if !open {
            self.session.svg_viewer_id = None;
        }
    }

    fn show_descriptor_editor_window(&mut self, ctx: &egui::Context) {
        // Snapshot save_msg state BEFORE show() so we can detect a new save.
        let was_saved = self
            .descriptors
            .editor
            .as_ref()
            .and_then(|s| s.save_msg.as_ref())
            .map(|(ok, _)| *ok)
            .unwrap_or(false);

        let Some(state) = self.descriptors.editor.as_mut() else {
            return;
        };
        let widgets_dir = Self::widgets_dir();
        let still_open = crate::panels::descriptor_editor::show(ctx, state, widgets_dir);
        if !still_open {
            self.descriptors.editor = None;
            return;
        }
        // Reload palette only on a fresh successful save (save_msg transitioned to ok).
        let now_saved = self
            .descriptors
            .editor
            .as_ref()
            .and_then(|s| s.save_msg.as_ref())
            .map(|(ok, _)| *ok)
            .unwrap_or(false);
        if now_saved && !was_saved {
            self.cmd_reload_descriptors();
        }
    }

    fn show_widget_builder_window(&mut self, ctx: &egui::Context) {
        // Snapshot save_msg state BEFORE show() so we detect a new save.
        let was_saved = self
            .descriptors
            .builder
            .as_ref()
            .and_then(|s| s.save_msg.as_ref())
            .map(|(ok, _)| *ok)
            .unwrap_or(false);

        // Read one-shot signal before any mutable borrow to avoid split-borrow.
        let open_adv = self
            .descriptors
            .builder
            .as_ref()
            .is_some_and(|b| b.open_advanced_requested);

        let Some(state) = self.descriptors.builder.as_mut() else {
            return;
        };
        let widgets_dir = Self::widgets_dir();
        let still_open = crate::panels::widget_builder::show(ctx, state, widgets_dir);
        if !still_open {
            self.descriptors.builder = None;
            return;
        }

        // Reload palette on fresh save.
        let now_saved = self
            .descriptors
            .builder
            .as_ref()
            .and_then(|s| s.save_msg.as_ref())
            .map(|(ok, _)| *ok)
            .unwrap_or(false);
        if now_saved && !was_saved {
            self.cmd_reload_descriptors();
        }

        // Advanced handoff: copy draft into editor and close builder atomically.
        if open_adv {
            let draft = self.descriptors.builder.take().unwrap().draft;
            self.descriptors.editor = Some(
                crate::panels::descriptor_editor::DescriptorEditorState::from_descriptor(&draft),
            );
        }
    }

    fn show_theme_window(&mut self, ctx: &egui::Context) {
        if !self.session.theme_open {
            return;
        }
        let mut open = true;
        // Collect mutable state before the closure to avoid split-borrow issues.
        let dark = self.project.ui_tree.app_props.theme.dark_mode;
        let [r0, g0, b0] = self.project.ui_tree.app_props.theme.accent_color;
        let swatch = egui::Color32::from_rgb(r0, g0, b0);
        let has_font0 = self
            .project
            .ui_tree
            .app_props
            .theme
            .base_font_size
            .is_some();
        let has_rounding0 = self
            .project
            .ui_tree
            .app_props
            .theme
            .global_corner_radius
            .is_some();
        let has_spacing0 = self.project.ui_tree.app_props.theme.spacing_scale.is_some();

        let mut save_clicked = false;
        let mut load_clicked = false;
        let mut reset_clicked = false;

        egui::Window::new("Theme Settings")
            .id(egui::Id::new("theme_window"))
            .open(&mut open)
            .default_size([300.0, 400.0])
            .resizable(false)
            .constrain(false)
            .show(ctx, |ui| {
                let theme = &mut self.project.ui_tree.app_props.theme;

                ui.label(egui::RichText::new("Mode").small().weak());
                ui.horizontal(|ui| {
                    if ui.selectable_label(dark, "Dark").clicked() {
                        theme.dark_mode = true;
                    }
                    if ui.selectable_label(!dark, "Light").clicked() {
                        theme.dark_mode = false;
                    }
                });

                ui.add_space(6.0);
                ui.label(egui::RichText::new("Accent color").small().weak());
                let [r, g, b] = &mut theme.accent_color;
                ui.horizontal(|ui| {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(20.0, 20.0)),
                        3.0,
                        swatch,
                    );
                    ui.add_space(24.0);
                    ui.label(egui::RichText::new("R").small().weak());
                    ui.add(egui::DragValue::new(r).range(0u8..=255u8).speed(1.0));
                    ui.label(egui::RichText::new("G").small().weak());
                    ui.add(egui::DragValue::new(g).range(0u8..=255u8).speed(1.0));
                    ui.label(egui::RichText::new("B").small().weak());
                    ui.add(egui::DragValue::new(b).range(0u8..=255u8).speed(1.0));
                });

                ui.add_space(6.0);
                ui.separator();
                ui.label(egui::RichText::new("Typography").small().weak());
                let mut has_font = has_font0;
                ui.checkbox(&mut has_font, "Override base font size");
                if has_font {
                    let fs = theme.base_font_size.get_or_insert(14.0);
                    ui.add(
                        egui::DragValue::new(fs)
                            .range(8.0..=32.0)
                            .suffix("pt")
                            .speed(0.5),
                    );
                } else {
                    theme.base_font_size = None;
                }

                ui.add_space(4.0);
                ui.label(egui::RichText::new("Shape").small().weak());
                let mut has_rounding = has_rounding0;
                ui.checkbox(&mut has_rounding, "Override corner radius");
                if has_rounding {
                    let cr = theme.global_corner_radius.get_or_insert(4.0);
                    ui.add(
                        egui::DragValue::new(cr)
                            .range(0.0..=32.0)
                            .suffix("px")
                            .speed(0.5),
                    );
                } else {
                    theme.global_corner_radius = None;
                }

                let mut has_spacing = has_spacing0;
                ui.checkbox(&mut has_spacing, "Override spacing scale");
                if has_spacing {
                    let ss = theme.spacing_scale.get_or_insert(1.0);
                    ui.add(egui::DragValue::new(ss).range(0.5..=3.0).speed(0.05));
                } else {
                    theme.spacing_scale = None;
                }

                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    save_clicked = ui.button("Save .rktheme…").clicked();
                    load_clicked = ui.button("Load .rktheme…").clicked();
                    reset_clicked = ui.button("Reset defaults").clicked();
                });
            });
        if !open {
            self.session.theme_open = false;
        }
        if save_clicked {
            self.cmd_save_theme();
        }
        if load_clicked {
            self.cmd_load_theme();
        }
        if reset_clicked {
            self.project.ui_tree.app_props.theme = Default::default();
        }
    }

    fn cmd_save_theme(&mut self) {
        let Some(dest) = rfd::FileDialog::new()
            .set_title("Save Theme")
            .set_file_name("theme.rktheme")
            .add_filter("RohKai Theme", &["rktheme"])
            .save_file()
        else {
            return;
        };
        let theme = &self.project.ui_tree.app_props.theme;
        match serde_json::to_string_pretty(theme) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&dest, json) {
                    self.messages.template_message = Some((false, format!("Save failed: {e}")));
                } else {
                    self.messages.template_message = Some((true, "Theme saved".to_owned()));
                }
            }
            Err(e) => {
                self.messages.template_message = Some((false, format!("Serialize error: {e}")));
            }
        }
    }

    fn cmd_load_theme(&mut self) {
        let Some(src) = rfd::FileDialog::new()
            .set_title("Load Theme")
            .add_filter("RohKai Theme", &["rktheme"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&src) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(theme) => {
                    self.project.ui_tree.app_props.theme = theme;
                    self.messages.template_message = Some((true, "Theme loaded".to_owned()));
                }
                Err(e) => {
                    self.messages.template_message =
                        Some((false, format!("Invalid theme file: {e}")));
                }
            },
            Err(e) => {
                self.messages.template_message = Some((false, format!("Read error: {e}")));
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
        self.apply_theme(ctx);
        self.apply_ui_scale(ctx);
        let now = ctx.input(|i| i.time);
        if self.messages.export_message.is_some() {
            match self.messages.export_message_until {
                Some(until) if now >= until => {
                    self.messages.export_message = None;
                    self.messages.export_message_until = None;
                }
                None => self.messages.export_message_until = Some(now + 8.0),
                _ => {}
            }
        } else {
            self.messages.export_message_until = None;
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
        let ctrl_r = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::R));
        if ctrl_r {
            self.session.canvas_settings.show_rulers = !self.session.canvas_settings.show_rulers;
        }

        if ctrl_n {
            self.request_destructive_command(PendingCommand::New);
        } else if ctrl_o {
            self.request_destructive_command(PendingCommand::Open);
        } else if ctrl_shift_s {
            self.cmd_save_as();
        } else if ctrl_s {
            self.cmd_save();
        }

        if ctrl_g && self.session.selected.len() >= 2 {
            if let Some(new_id) = self.project.ui_tree.group(&self.session.selected) {
                self.session.selected.clear();
                self.session.selected.push(new_id);
            }
        } else if ctrl_shift_g {
            let frame_ids: Vec<Uuid> = self
                .session
                .selected
                .iter()
                .filter(|&&id| {
                    self.project
                        .ui_tree
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
                    let children = self.project.ui_tree.ungroup(frame_id);
                    new_sel.extend(children);
                }
                self.session.selected = new_sel;
            }
        }

        // ---------------------------------------------------------------
        // Window title
        // ---------------------------------------------------------------
        let dirty = self.cached_dirty(ctx);
        let title = match &self.project.current_file {
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
                            !self.session.selected.is_empty(),
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
                    if ui
                        .button("Import Widget Definition…")
                        .on_hover_text("Copy a .rkwd file into widgets/ and reload")
                        .clicked()
                    {
                        self.cmd_import_widget_definition();
                        ui.close_menu();
                    }
                    if ui
                        .button("Reload Widget Descriptors")
                        .on_hover_text("Rescan widgets/ folder for .rkwd files — no restart needed")
                        .clicked()
                    {
                        self.cmd_reload_descriptors();
                        ui.close_menu();
                    }
                    if ui
                        .button("Create Custom Widget…")
                        .on_hover_text("Guided builder for new custom widgets")
                        .clicked()
                    {
                        self.cmd_new_widget_builder();
                        ui.close_menu();
                    }
                    if ui
                        .button("New Widget Descriptor…")
                        .on_hover_text("Open the in-app .rkwd editor to create a new widget type")
                        .clicked()
                    {
                        self.cmd_new_descriptor();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Preferences…").clicked() {
                        self.prefs.draft = self.prefs.user_settings.clone();
                        self.prefs.open = true;
                        ui.close_menu();
                    }
                });

                // Widgets menu
                ui.menu_button("Widgets", |ui| {
                    if ui
                        .button("Create Custom Widget…")
                        .on_hover_text("Guided builder for new custom widgets")
                        .clicked()
                    {
                        self.cmd_new_widget_builder();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .button("New Descriptor…")
                        .on_hover_text("Open the in-app .rkwd editor to create a new widget type")
                        .clicked()
                    {
                        self.cmd_new_descriptor();
                        ui.close_menu();
                    }
                    if ui
                        .button("Import Definition…")
                        .on_hover_text("Copy a .rkwd file into widgets/ and reload")
                        .clicked()
                    {
                        self.cmd_import_widget_definition();
                        ui.close_menu();
                    }
                    if ui
                        .button("Reload Descriptors")
                        .on_hover_text("Rescan widgets/ folder for .rkwd files")
                        .clicked()
                    {
                        self.cmd_reload_descriptors();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .button("Export Bundle…")
                        .on_hover_text("Save all loaded descriptors as a .rkwb bundle")
                        .clicked()
                    {
                        self.cmd_export_widget_bundle();
                        ui.close_menu();
                    }
                    if ui
                        .button("Import Bundle…")
                        .on_hover_text("Load all descriptors from a .rkwb bundle")
                        .clicked()
                    {
                        self.cmd_import_widget_bundle();
                        ui.close_menu();
                    }
                    if !self.descriptors.widgets.is_empty() {
                        ui.separator();
                        let ids: Vec<(String, String)> = self
                            .descriptors
                            .widgets
                            .iter()
                            .map(|d| (d.id.clone(), d.name.clone()))
                            .collect();
                        for (id, name) in ids {
                            if ui.button(format!("Edit \"{name}\"")).clicked() {
                                self.cmd_edit_descriptor(&id);
                                ui.close_menu();
                            }
                        }
                    }
                });

                // View menu
                ui.menu_button("View", |ui| {
                    let ruler_label = if self.session.canvas_settings.show_rulers {
                        "Hide Rulers  [Ctrl+R]"
                    } else {
                        "Show Rulers  [Ctrl+R]"
                    };
                    if ui.button(ruler_label).clicked() {
                        self.session.canvas_settings.show_rulers =
                            !self.session.canvas_settings.show_rulers;
                        ui.close_menu();
                    }
                    if ui.button("Clear All Guides").clicked() {
                        self.project.ui_tree.app_props.guides.clear();
                        ui.close_menu();
                    }
                    let bezel_label = if self.project.ui_tree.app_props.show_bezel {
                        "Hide Canvas Bezel"
                    } else {
                        "Show Canvas Bezel"
                    };
                    if ui.button(bezel_label).clicked() {
                        self.project.ui_tree.app_props.show_bezel =
                            !self.project.ui_tree.app_props.show_bezel;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Theme…").clicked() {
                        self.session.theme_open = true;
                        ui.close_menu();
                    }
                });

                ui.separator();

                // Inline app title editor
                ui.label(egui::RichText::new("Title:").small().weak());
                ui.add(
                    egui::TextEdit::singleline(&mut self.project.ui_tree.app_props.title)
                        .desired_width(110.0)
                        .hint_text("App title"),
                );

                ui.separator();

                // Zoom indicator + reset
                let zoom_pct = (self.session.canvas_settings.zoom * 100.0).round() as u32;
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
                    self.session.canvas_settings.zoom = 1.0;
                    self.session.canvas_settings.pan = egui::Vec2::ZERO;
                }

                // File dirty indicator
                ui.separator();
                match &self.project.current_file {
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
        let prev_canvas_w = self.project.ui_tree.app_props.win_w;
        let prev_canvas_h = self.project.ui_tree.app_props.win_h;
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("W").small().weak());
                ui.add(
                    egui::DragValue::new(&mut self.project.ui_tree.app_props.win_w)
                        .speed(1.0)
                        .range(100.0..=4000.0),
                );
                ui.label(egui::RichText::new("H").small().weak());
                ui.add(
                    egui::DragValue::new(&mut self.project.ui_tree.app_props.win_h)
                        .speed(1.0)
                        .range(100.0..=4000.0),
                );
                let lock_icon = if self.session.lock_aspect_ratio {
                    "🔒"
                } else {
                    "🔓"
                };
                if ui
                    .small_button(lock_icon)
                    .on_hover_text("Lock aspect ratio")
                    .clicked()
                {
                    self.session.lock_aspect_ratio = !self.session.lock_aspect_ratio;
                }
                // Canvas preset picker
                ui.menu_button("▾ Preset", |ui| {
                    const PRESETS: &[(&str, f32, f32)] = &[
                        ("1920 × 1080 (FHD)", 1920.0, 1080.0),
                        ("2560 × 1440 (QHD)", 2560.0, 1440.0),
                        ("1366 × 768 (HD)", 1366.0, 768.0),
                        ("1280 × 720 (720p)", 1280.0, 720.0),
                        ("800 × 600 (default)", 800.0, 600.0),
                        ("375 × 812 (iPhone SE)", 375.0, 812.0),
                        ("390 × 844 (iPhone 14)", 390.0, 844.0),
                        ("414 × 896 (iPhone XS Max)", 414.0, 896.0),
                        ("768 × 1024 (iPad)", 768.0, 1024.0),
                    ];
                    for (label, w, h) in PRESETS {
                        if ui.button(*label).clicked() {
                            self.project.ui_tree.app_props.win_w = *w;
                            self.project.ui_tree.app_props.win_h = *h;
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                let (snap_text, snap_color) = if self.session.canvas_settings.snap_enabled {
                    ("Grid: ON", egui::Color32::from_rgb(52, 211, 153))
                } else {
                    ("Grid: OFF", egui::Color32::from_gray(120))
                };
                ui.label(egui::RichText::new(snap_text).small().color(snap_color));
                ui.add(
                    egui::DragValue::new(&mut self.session.canvas_settings.snap_step)
                        .speed(0.5)
                        .range(1.0..=256.0)
                        .suffix("px"),
                );
                ui.label(egui::RichText::new("[G]").small().weak());

                // Show descriptor load errors as a dismissable warning
                if !self.descriptors.errors.is_empty() {
                    ui.separator();
                    let count = self.descriptors.errors.len();
                    let tip = self.descriptors.errors.join("\n");
                    ui.label(
                        egui::RichText::new(format!("⚠ {count} widget descriptor error(s)"))
                            .color(egui::Color32::YELLOW)
                            .small(),
                    )
                    .on_hover_text(tip);
                }

                let mut clear_error = false;
                if let Some(err) = self.messages.last_error.as_deref() {
                    ui.separator();
                    ui.label(egui::RichText::new(err).color(egui::Color32::RED).small());
                    clear_error = ui
                        .small_button("x")
                        .on_hover_text("Dismiss error")
                        .clicked();
                }

                let mut clear_export = false;
                if let Some((ok, msg)) = self.messages.export_message.as_ref() {
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
                    self.messages.last_error = None;
                }
                if clear_export {
                    self.messages.export_message = None;
                    self.messages.export_message_until = None;
                }
            });
        });

        // Enforce aspect ratio after DragValue edits
        if self.session.lock_aspect_ratio && prev_canvas_w > 0.0 && prev_canvas_h > 0.0 {
            let cur_w = self.project.ui_tree.app_props.win_w;
            let cur_h = self.project.ui_tree.app_props.win_h;
            let ratio = prev_canvas_w / prev_canvas_h;
            if (cur_w - prev_canvas_w).abs() > 0.001 {
                self.project.ui_tree.app_props.win_h = (cur_w / ratio).clamp(100.0, 4000.0);
            } else if (cur_h - prev_canvas_h).abs() > 0.001 {
                self.project.ui_tree.app_props.win_w = (cur_h * ratio).clamp(100.0, 4000.0);
            }
        }

        // ---------------------------------------------------------------
        // Right panel: generated code
        // ---------------------------------------------------------------
        crate::panels::code_preview::show(
            ctx,
            &mut self.project.ui_tree,
            CodePreviewArgs {
                highlighted_id: self.session.highlighted_code_id,
                scroll_to: &mut self.session.scroll_to_code,
                scroll_to_handler: &mut self.session.scroll_to_handler,
                code_buffer: &mut self.code.buffer,
                code_status: &mut self.code.status,
                last_generated: &mut self.code.last_generated,
                split_ratio: &mut self.code.split_ratio,
                code_font_size: self.prefs.user_settings.code_font_size,
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

                    let (palette_click, palette_drag) =
                        crate::panels::palette::show_content(ui, &self.descriptors.widgets);
                    ui.add_space(4.0);

                    let shift_held = ui.input(|i| i.modifiers.shift);
                    let mut props_action = crate::panels::properties::PropertiesAction::None;
                    egui::CollapsingHeader::new("Properties")
                        .default_open(true)
                        .show(ui, |ui| {
                            props_action = crate::panels::properties::show_content(
                                ui,
                                &mut self.project.ui_tree,
                                &mut self.session.selected,
                                shift_held,
                                &self.descriptors.widgets,
                            );
                        });

                    ui.add_space(4.0);
                    ui.separator();

                    let mut tmpl_action = crate::panels::templates::TemplateAction::None;
                    egui::CollapsingHeader::new("Templates")
                        .default_open(true)
                        .show(ui, |ui| {
                            tmpl_action = crate::panels::templates::show(
                                ui,
                                &mut self.messages.template_message,
                            );
                        });

                    (palette_click, palette_drag, tmpl_action, props_action)
                })
                .inner;

        // Tracé — properties panel requested scroll-to-handler
        match props_action {
            crate::panels::properties::PropertiesAction::ScrollToHandler(name) => {
                self.session.scroll_to_handler = Some(name);
            }
            crate::panels::properties::PropertiesAction::ShowSvgSource(id) => {
                self.session.svg_viewer_id = Some(id);
            }
            crate::panels::properties::PropertiesAction::EditDescriptor(desc_id) => {
                self.cmd_edit_descriptor(&desc_id);
            }
            crate::panels::properties::PropertiesAction::None => {}
        }

        // Palette click → place at viewport center (accounting for zoom/pan)
        if let Some(mut instance) = palette_click {
            let zoom = self.session.canvas_settings.zoom;
            let pan = self.session.canvas_settings.pan;
            let win_w = self.project.ui_tree.app_props.win_w;
            let win_h = self.project.ui_tree.app_props.win_h;
            let cx = (-pan.x / zoom + win_w / 2.0).clamp(0.0, win_w);
            let cy = (-pan.y / zoom + win_h / 2.0).clamp(0.0, win_h);
            instance.rect.x = (cx - instance.rect.w / 2.0).max(0.0);
            instance.rect.y = (cy - instance.rect.h / 2.0).max(0.0);
            self.project.ui_tree.add(instance);
        }

        // Palette drag — set interaction.template_drag for canvas drop next frame
        if let Some(instance) = palette_drag {
            self.session.interaction.template_drag = Some(vec![instance]);
        }

        // Process template action BEFORE canvas handle() so AddAtCenter never enters
        // the drag-drop path (which runs in the same frame as primary_released)
        match tmpl_action {
            crate::panels::templates::TemplateAction::AddAtCenter(instances) => {
                let cx = self.project.ui_tree.app_props.win_w / 2.0;
                let cy = self.project.ui_tree.app_props.win_h / 2.0;
                let min_x = instances.iter().map(|w| w.rect.x).fold(f32::MAX, f32::min);
                let min_y = instances.iter().map(|w| w.rect.y).fold(f32::MAX, f32::min);
                for mut w in instances {
                    w.id = Uuid::new_v4();
                    w.rect.x = (w.rect.x - min_x + cx).max(0.0);
                    w.rect.y = (w.rect.y - min_y + cy).max(0.0);
                    self.project.ui_tree.add(w);
                }
            }
            crate::panels::templates::TemplateAction::BeginDrag(instances) => {
                self.session.interaction.template_drag = Some(instances);
            }
            crate::panels::templates::TemplateAction::None => {}
        }

        // ---------------------------------------------------------------
        // Canvas
        // ---------------------------------------------------------------
        let delete_pressed = ctx.input(|i| i.key_pressed(egui::Key::Delete));
        let has_hovered_guide = self.session.hovered_guide.is_some();
        egui::CentralPanel::default().show(ctx, |ui| {
            self.session.last_canvas_rect = ui.max_rect();
            let panel_rect = ui.max_rect();
            let zoom = self.session.canvas_settings.zoom;
            let pan = self.session.canvas_settings.pan;
            let canvas_size = [
                self.project.ui_tree.app_props.win_w,
                self.project.ui_tree.app_props.win_h,
            ];

            // Guide interaction runs before canvas (uses raw pointer input).
            let ruler_ctx = crate::canvas::rulers::RulerCtx {
                show_rulers: self.session.canvas_settings.show_rulers,
                zoom,
                pan,
                canvas_size,
                panel_rect,
            };
            crate::canvas::rulers::handle_interaction(
                ui,
                &mut self.project.ui_tree.app_props.guides,
                &mut self.session.hovered_guide,
                &mut self.session.dragging_guide,
                &ruler_ctx,
                delete_pressed && has_hovered_guide,
            );

            self.session.canvas_settings.guide_drag_active = self.session.dragging_guide.is_some();
            crate::canvas::interaction::handle(
                ui,
                &mut self.project.ui_tree,
                &mut self.session.interaction,
                &mut self.session.selected,
                &mut self.session.canvas_settings,
                crate::canvas::interaction::CanvasTextSettings {
                    label_scale: self.prefs.user_settings.canvas_label_scale,
                    tag_scale: self.prefs.user_settings.canvas_tag_scale,
                },
                &mut self.svg_texture_cache,
            );

            // Rulers and guide lines drawn on top of canvas content.
            crate::canvas::rulers::draw(
                ui,
                &self.project.ui_tree.app_props.guides,
                self.session.hovered_guide,
                &ruler_ctx,
            );

            // Canvas bezel — mock OS title bar chrome above the canvas rect.
            if self.project.ui_tree.app_props.show_bezel {
                crate::canvas::rulers::draw_bezel(
                    ui,
                    &ruler_ctx,
                    &self.project.ui_tree.app_props.title,
                );
            }
        });

        // Prune SVG texture cache for widgets removed from canvas.
        let live_ids: std::collections::HashSet<Uuid> =
            self.project.ui_tree.widgets.iter().map(|w| w.id).collect();
        crate::canvas::interaction::svg_texture_cache_retain_live(
            &mut self.svg_texture_cache,
            &live_ids,
        );

        // SVG import mode modal
        self.show_svg_import_modal(ctx);

        // ---------------------------------------------------------------
        // Post-canvas: read per-frame signals
        // ---------------------------------------------------------------

        // Lazare double-click → highlight code panel
        if let Some(id) = self.session.interaction.double_clicked_widget {
            self.session.highlighted_code_id = Some(id);
            self.session.scroll_to_code = true;
        }

        // ---------------------------------------------------------------
        // Delete key — widgets (when no guide was hovered at frame start)
        // ---------------------------------------------------------------
        if delete_pressed && !has_hovered_guide {
            let ids: Vec<Uuid> = self.session.selected.drain(..).collect();
            for id in ids {
                self.project.ui_tree.remove(id);
            }
        }

        self.show_pending_command_dialog(ctx);
        self.show_preferences_window(ctx);
        self.show_svg_source_window(ctx);
        self.show_descriptor_editor_window(ctx);
        self.show_widget_builder_window(ctx);
        self.show_theme_window(ctx);
    }
}
