//! Preview mode — renders the canvas as live interactive egui widgets
//! at 1:1 zoom, giving a faithful preview of the exported app layout.
//!
//! `PreviewState` holds mutable runtime values keyed by `state_binding`.
//! It is initialised from widget defaults when entering preview mode and
//! discarded on exit; it never touches `UiTree` or generated code.

use crate::{
    canvas::{rulers::canvas_origin, widget_instance::canvas_rect},
    project::{
        document::{ProjectDocument, SurfaceKind},
        schema::{
            BehaviorTrigger, DialogButtonRole, DialogButtonTarget, SurfaceEvent, VisualAction,
            WidgetEvent, WidgetEventRef, WidgetKind, resolve_dialog_button_target,
            resolve_dialog_initial_focus_target,
        },
        ui_tree::UiTree,
    },
};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum PreviewValue {
    Str(String),
    Float(f32),
    Bool(bool),
    StringList(Vec<String>),
    FloatList(Vec<f32>),
}

#[derive(Debug, Clone)]
pub struct PreviewModal {
    pub surface_id: Uuid,
    pub draft: HashMap<String, PreviewValue>,
}

#[derive(Clone, Default)]
pub struct PreviewState {
    pub values: HashMap<String, PreviewValue>,
    pub modal_stack: Vec<PreviewModal>,
    pub diagnostics: Vec<String>,
    project: Option<ProjectDocument>,
    preview_surface: Option<Uuid>,
    dispatch_depth: usize,
    focus_return: Vec<Option<egui::Id>>,
    initial_focus: Vec<Uuid>,
    pending_focus_restore: Option<egui::Id>,
    next_modal_focus: Option<Option<egui::Id>>,
}

#[derive(Debug, Clone, Copy)]
enum PreviewInteraction {
    Widget(WidgetEvent),
    DialogButton(DialogButtonRole),
}

impl PreviewState {
    /// Build initial values from widget defaults.
    pub fn init_from_tree(tree: &UiTree) -> Self {
        let mut values = HashMap::new();
        for w in &tree.widgets {
            let key = match &w.state_binding {
                Some(b) if !b.is_empty() => b.clone(),
                _ => continue,
            };
            let val = match w.kind {
                WidgetKind::Slider | WidgetKind::ProgressBar => {
                    PreviewValue::Float(w.props.default_value)
                }
                WidgetKind::Checkbox => PreviewValue::Bool(false),
                // Radios sharing a binding form a group: the binding holds the
                // selected option's label, so mutual exclusion works correctly.
                WidgetKind::RadioButton => PreviewValue::Str(w.props.label.clone()),
                _ => PreviewValue::Str(w.props.label.clone()),
            };
            values.insert(key, val);
        }
        Self {
            values,
            ..Default::default()
        }
    }

    /// Build one shared runtime store from every surface and retain a frozen
    /// project snapshot for deterministic behavior dispatch during F5 preview.
    pub fn init_from_document(document: &ProjectDocument) -> Self {
        let mut values = HashMap::new();
        for surface in &document.surfaces {
            extend_defaults(&mut values, &surface.tree);
        }
        Self {
            values,
            project: Some(document.clone()),
            ..Default::default()
        }
    }

    pub fn init_for_surface(document: &ProjectDocument, surface: Uuid) -> Self {
        let mut state = Self::init_from_document(document);
        if document.surface(surface).is_some() {
            state.preview_surface = Some(surface);
        } else {
            state
                .diagnostics
                .push(format!("Missing isolated preview surface {surface}."));
        }
        state
    }

    #[must_use]
    pub fn project(&self) -> Option<&ProjectDocument> {
        self.project.as_ref()
    }

    #[must_use]
    pub fn preview_surface(&self) -> Option<Uuid> {
        self.preview_surface
    }

    pub fn dispatch_widget_event(&mut self, widget: Uuid, event: WidgetEvent) {
        self.dispatch_widget_event_on_surface(Uuid::nil(), widget, event);
    }

    pub fn dispatch_widget_event_on_surface(
        &mut self,
        surface: Uuid,
        widget: Uuid,
        event: WidgetEvent,
    ) {
        self.dispatch_trigger(
            BehaviorTrigger::Widget(WidgetEventRef {
                source_widget: widget,
                event,
            }),
            (surface != Uuid::nil()).then_some(surface),
        );
    }

    pub fn dispatch_surface_event(&mut self, surface: Uuid, event: SurfaceEvent) {
        self.dispatch_trigger(
            BehaviorTrigger::Surface(crate::project::schema::SurfaceEventRef {
                source_surface: surface,
                event,
            }),
            None,
        );
    }

    pub fn open_modal(&mut self, surface_id: Uuid) -> bool {
        let opener = self.next_modal_focus.take().unwrap_or(None);
        self.open_modal_with_focus(surface_id, opener)
    }

    fn open_modal_with_focus(&mut self, surface_id: Uuid, opener: Option<egui::Id>) -> bool {
        if self
            .modal_stack
            .iter()
            .any(|modal| modal.surface_id == surface_id)
        {
            self.diagnostics
                .push(format!("Dialog {surface_id} is already open."));
            return false;
        }
        if self.modal_stack.len() >= 16 {
            self.diagnostics
                .push("Modal stack limit (16) reached.".to_owned());
            return false;
        }
        let Some(surface) = self
            .project
            .as_ref()
            .and_then(|document| document.surface(surface_id))
        else {
            self.diagnostics
                .push(format!("Missing modal surface {surface_id}."));
            return false;
        };
        if !matches!(surface.kind, SurfaceKind::ModalDialog(_)) {
            self.diagnostics
                .push(format!("Surface {surface_id} is not a modal dialog."));
            return false;
        }

        let mut draft = HashMap::new();
        for widget in &surface.tree.widgets {
            if let Some(binding) = widget
                .state_binding
                .as_deref()
                .map(str::trim)
                .filter(|binding| !binding.is_empty())
            {
                let value = self
                    .values
                    .get(binding)
                    .cloned()
                    .unwrap_or_else(|| default_preview_value(widget));
                draft.insert(binding.to_owned(), value);
            }
            if let Some(data_binding) = widget
                .props
                .data_source_binding
                .as_deref()
                .map(str::trim)
                .filter(|binding| !binding.is_empty())
                && widget.kind != WidgetKind::Table
            {
                let value = self
                    .values
                    .get(data_binding)
                    .cloned()
                    .unwrap_or_else(|| default_data_preview_value(widget));
                draft.insert(data_binding.to_owned(), value);
            }
        }
        self.modal_stack.push(PreviewModal { surface_id, draft });
        self.focus_return.push(opener);
        self.initial_focus.push(surface_id);
        self.dispatch_surface_event(surface_id, SurfaceEvent::Opened);
        true
    }

    pub fn accept_dialog(&mut self, surface_id: Uuid) -> bool {
        if self.modal_stack.last().map(|modal| modal.surface_id) != Some(surface_id) {
            self.diagnostics.push(format!(
                "Only the topmost dialog can be accepted ({surface_id})."
            ));
            return false;
        }
        let modal = self.modal_stack.pop().expect("top modal checked above");
        self.values.extend(modal.draft);
        self.finish_modal_focus(surface_id);
        self.dispatch_surface_event(surface_id, SurfaceEvent::Accepted);
        self.dispatch_surface_event(surface_id, SurfaceEvent::Closed);
        true
    }

    pub fn reject_dialog(&mut self, surface_id: Uuid) -> bool {
        if self.modal_stack.last().map(|modal| modal.surface_id) != Some(surface_id) {
            self.diagnostics.push(format!(
                "Only the topmost dialog can be rejected ({surface_id})."
            ));
            return false;
        }
        self.modal_stack.pop();
        self.finish_modal_focus(surface_id);
        self.dispatch_surface_event(surface_id, SurfaceEvent::Rejected);
        self.dispatch_surface_event(surface_id, SurfaceEvent::Closed);
        true
    }

    fn finish_modal_focus(&mut self, surface_id: Uuid) {
        self.initial_focus.retain(|pending| *pending != surface_id);
        self.pending_focus_restore = self.focus_return.pop().flatten();
    }

    fn is_initial_focus_pending(&self, surface_id: Uuid) -> bool {
        self.initial_focus.contains(&surface_id)
    }

    fn clear_initial_focus(&mut self, surface_id: Uuid) {
        self.initial_focus.retain(|pending| *pending != surface_id);
    }

    fn capture_opener_focus(&mut self, ctx: &egui::Context) {
        self.next_modal_focus = Some(ctx.memory(|memory| memory.focused()));
    }

    fn clear_opener_focus_capture(&mut self) {
        self.next_modal_focus = None;
    }

    fn take_pending_focus_restore(&mut self) -> Option<egui::Id> {
        self.pending_focus_restore.take()
    }

    fn restore_pending_focus(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.take_pending_focus_restore() {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
    }

    pub fn apply_dialog(&mut self, surface_id: Uuid) -> bool {
        let Some(modal) = self
            .modal_stack
            .iter()
            .find(|modal| modal.surface_id == surface_id)
        else {
            return false;
        };
        self.values.extend(modal.draft.clone());
        true
    }

    pub fn reset_dialog(&mut self, surface_id: Uuid) -> bool {
        let Some(modal) = self
            .modal_stack
            .iter_mut()
            .find(|modal| modal.surface_id == surface_id)
        else {
            return false;
        };
        for (field, value) in &self.values {
            if modal.draft.contains_key(field) {
                modal.draft.insert(field.clone(), value.clone());
            }
        }
        true
    }

    pub fn activate_dialog_role(&mut self, surface_id: Uuid, widget: Uuid, role: DialogButtonRole) {
        match role {
            DialogButtonRole::Accept => {
                self.accept_dialog(surface_id);
            }
            DialogButtonRole::Reject => {
                self.reject_dialog(surface_id);
            }
            DialogButtonRole::Apply => {
                self.apply_dialog(surface_id);
            }
            DialogButtonRole::Reset => {
                self.reset_dialog(surface_id);
            }
            DialogButtonRole::Help | DialogButtonRole::Action => {
                self.dispatch_widget_event_on_surface(surface_id, widget, WidgetEvent::Click);
            }
        }
    }

    fn activate_dialog_target(&mut self, surface_id: Uuid, target: DialogButtonTarget) {
        match target {
            DialogButtonTarget::Widget(widget) => {
                self.dispatch_widget_event_on_surface(surface_id, widget, WidgetEvent::Click);
            }
            DialogButtonTarget::Role { widget, role } => {
                self.activate_dialog_role(surface_id, widget, role);
            }
        }
    }

    fn dispatch_trigger(&mut self, trigger: BehaviorTrigger, value_scope: Option<Uuid>) {
        if self.dispatch_depth >= 64 {
            self.diagnostics
                .push("Behavior dispatch limit (64) reached.".to_owned());
            return;
        }
        let actions: Vec<VisualAction> = self
            .project
            .as_ref()
            .map(|document| {
                document
                    .props
                    .behaviors
                    .iter()
                    .filter(|behavior| behavior.trigger == trigger)
                    .map(|behavior| behavior.action.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.dispatch_depth += 1;
        for action in actions {
            self.apply_action(action, value_scope);
        }
        self.dispatch_depth -= 1;
    }

    fn apply_action(&mut self, action: VisualAction, value_scope: Option<Uuid>) {
        match action {
            VisualAction::Set { field, value } => {
                self.values_for_scope_mut(value_scope).insert(
                    field,
                    match value {
                        crate::project::schema::ValueExpr::Number(value) => {
                            PreviewValue::Float(value)
                        }
                        crate::project::schema::ValueExpr::Text(value) => PreviewValue::Str(value),
                        crate::project::schema::ValueExpr::Flag(value) => PreviewValue::Bool(value),
                    },
                );
            }
            VisualAction::Add {
                field,
                amount,
                min,
                max,
            } => mutate_number(
                self.values_for_scope_mut(value_scope),
                &field,
                amount,
                min,
                max,
            ),
            VisualAction::Subtract {
                field,
                amount,
                min,
                max,
            } => mutate_number(
                self.values_for_scope_mut(value_scope),
                &field,
                -amount,
                min,
                max,
            ),
            VisualAction::Toggle { field } => {
                if let Some(PreviewValue::Bool(value)) =
                    self.values_for_scope_mut(value_scope).get_mut(&field)
                {
                    *value = !*value;
                } else {
                    self.diagnostics
                        .push(format!("Behavior field {field:?} is not a bool."));
                }
            }
            VisualAction::CallHandler { handler } => self.diagnostics.push(format!(
                "Preview cannot execute Rust handler {handler:?}; export will call it."
            )),
            VisualAction::OpenModal { surface } => {
                self.open_modal(surface);
            }
            VisualAction::AcceptDialog { surface } => {
                self.accept_dialog(surface);
            }
            VisualAction::RejectDialog { surface } => {
                self.reject_dialog(surface);
            }
        }
    }

    fn values_for_scope_mut(
        &mut self,
        surface: Option<Uuid>,
    ) -> &mut HashMap<String, PreviewValue> {
        if let Some(surface) = surface
            && let Some(modal) = self
                .modal_stack
                .iter_mut()
                .find(|modal| modal.surface_id == surface)
        {
            return &mut modal.draft;
        }
        &mut self.values
    }
}

fn extend_defaults(values: &mut HashMap<String, PreviewValue>, tree: &UiTree) {
    for widget in &tree.widgets {
        if let Some(binding) = widget
            .state_binding
            .as_deref()
            .map(str::trim)
            .filter(|binding| !binding.is_empty())
        {
            values
                .entry(binding.to_owned())
                .or_insert_with(|| default_preview_value(widget));
        }
        if let Some(data_binding) = widget
            .props
            .data_source_binding
            .as_deref()
            .map(str::trim)
            .filter(|binding| !binding.is_empty())
            && widget.kind != WidgetKind::Table
        {
            values
                .entry(data_binding.to_owned())
                .or_insert_with(|| default_data_preview_value(widget));
        }
    }
}

fn default_preview_value(widget: &crate::project::schema::WidgetInstance) -> PreviewValue {
    match widget.kind {
        WidgetKind::Chart => PreviewValue::FloatList(vec![0.2, 0.5, 0.8, 0.4]),
        WidgetKind::Slider
        | WidgetKind::SpinBox
        | WidgetKind::ProgressBar
        | WidgetKind::MathLabel => PreviewValue::Float(widget.props.default_value),
        WidgetKind::Checkbox => PreviewValue::Bool(false),
        WidgetKind::RadioButton => PreviewValue::Str(widget.props.label.clone()),
        _ => PreviewValue::Str(widget.props.label.clone()),
    }
}

fn default_data_preview_value(widget: &crate::project::schema::WidgetInstance) -> PreviewValue {
    PreviewValue::StringList(widget.props.options.clone())
}

fn mutate_number(
    values: &mut HashMap<String, PreviewValue>,
    field: &str,
    amount: f32,
    min: Option<f32>,
    max: Option<f32>,
) {
    let Some(PreviewValue::Float(value)) = values.get_mut(field) else {
        return;
    };
    *value += amount;
    if let Some(min) = min {
        *value = value.max(min);
    }
    if let Some(max) = max {
        *value = value.min(max);
    }
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

/// Render the canvas in preview mode inside `ui` (egui `CentralPanel`).
///
/// Forces 1:1 zoom with the canvas centred in the panel.
/// Returns `true` when the user clicks "Exit Preview".
pub fn render(
    ui: &mut egui::Ui,
    tree: &UiTree,
    state: &mut PreviewState,
    panel_rect: egui::Rect,
    svg_texture_cache: &mut crate::canvas::interaction::SvgTextureCache,
) -> bool {
    let (exit, events) = render_surface(ui, tree, state, panel_rect, svg_texture_cache, true, None);
    for (widget, interaction) in events {
        if let PreviewInteraction::Widget(event) = interaction {
            state.dispatch_widget_event(widget, event);
        }
    }
    exit
}

/// Render the root surface plus the live modal stack from the frozen preview
/// project snapshot.
pub fn render_project(
    ui: &mut egui::Ui,
    state: &mut PreviewState,
    panel_rect: egui::Rect,
    svg_texture_cache: &mut crate::canvas::interaction::SvgTextureCache,
) -> bool {
    if let Some(surface_id) = state.preview_surface
        && let Some(tree) = state
            .project()
            .and_then(|document| document.materialized_tree(surface_id))
    {
        let (exit, events) =
            render_surface(ui, &tree, state, panel_rect, svg_texture_cache, true, None);
        for (widget, interaction) in events {
            state.capture_opener_focus(ui.ctx());
            match interaction {
                PreviewInteraction::Widget(event) => {
                    state.dispatch_widget_event_on_surface(surface_id, widget, event);
                }
                PreviewInteraction::DialogButton(
                    DialogButtonRole::Help | DialogButtonRole::Action,
                ) => {
                    state.dispatch_widget_event_on_surface(surface_id, widget, WidgetEvent::Click);
                }
                PreviewInteraction::DialogButton(_) => {
                    state.diagnostics.push(
                        "Accept/Reject is disabled in isolated surface preview; use F5 project preview."
                            .to_owned(),
                    );
                }
            }
            state.clear_opener_focus_capture();
        }
        return exit;
    }

    let Some((root_id, root_tree)) = state.project().and_then(|document| {
        let root_id = document.root_surface;
        document
            .materialized_tree(root_id)
            .map(|tree| (root_id, tree))
    }) else {
        return false;
    };
    let (exit, root_events) = render_surface(
        ui,
        &root_tree,
        state,
        panel_rect,
        svg_texture_cache,
        true,
        None,
    );
    for (widget, interaction) in root_events {
        state.capture_opener_focus(ui.ctx());
        if let PreviewInteraction::Widget(event) = interaction {
            state.dispatch_widget_event_on_surface(root_id, widget, event);
        }
        state.clear_opener_focus_capture();
    }

    let modal_ids: Vec<Uuid> = state
        .modal_stack
        .iter()
        .map(|modal| modal.surface_id)
        .collect();
    for surface_id in modal_ids {
        let Some((surface, tree)) = state.project().and_then(|document| {
            let surface = document.surface(surface_id)?.clone();
            let tree = document.materialized_tree(surface_id)?;
            Some((surface, tree))
        }) else {
            continue;
        };
        let SurfaceKind::ModalDialog(policy) = surface.kind else {
            continue;
        };
        let Some(index) = state
            .modal_stack
            .iter()
            .position(|modal| modal.surface_id == surface_id)
        else {
            continue;
        };
        let draft = std::mem::take(&mut state.modal_stack[index].draft);
        let mut modal_state = PreviewState {
            values: draft,
            ..Default::default()
        };
        let request_initial_focus = state.is_initial_focus_pending(surface_id);
        let focus_target = request_initial_focus
            .then(|| resolve_dialog_initial_focus_target(&tree.widgets, policy.default_button));
        let focus_target = focus_target.flatten();
        let mut modal_events = Vec::new();
        let response = egui::Modal::new(egui::Id::new(("project_preview_modal", surface_id))).show(
            ui.ctx(),
            |ui| {
                ui.set_min_size(egui::vec2(surface.props.size[0], surface.props.size[1]));
                ui.heading(&surface.props.title);
                ui.separator();
                let body_rect = ui.available_rect_before_wrap();
                let (_, events) = render_surface(
                    ui,
                    &tree,
                    &mut modal_state,
                    body_rect,
                    svg_texture_cache,
                    false,
                    focus_target,
                );
                modal_events = events;
            },
        );
        if request_initial_focus && focus_target.is_none() {
            response.response.request_focus();
        }
        state.clear_initial_focus(surface_id);
        if let Some(modal) = state
            .modal_stack
            .iter_mut()
            .find(|modal| modal.surface_id == surface_id)
        {
            modal.draft = modal_state.values;
        }

        for (widget, interaction) in modal_events {
            state.capture_opener_focus(ui.ctx());
            match interaction {
                PreviewInteraction::Widget(event) => {
                    state.dispatch_widget_event_on_surface(surface_id, widget, event);
                }
                PreviewInteraction::DialogButton(role) => {
                    state.activate_dialog_role(surface_id, widget, role);
                }
            }
            state.clear_opener_focus_capture();
        }

        let is_top = state.modal_stack.last().map(|modal| modal.surface_id) == Some(surface_id);
        if is_top {
            let enter = ui
                .ctx()
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if enter
                && let Some(target) = resolve_dialog_button_target(
                    &tree.widgets,
                    policy.default_button,
                    DialogButtonRole::Accept,
                )
            {
                state.activate_dialog_target(surface_id, target);
            }
            let escape = policy.reject_on_escape
                && ui
                    .ctx()
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if escape {
                if let Some(target) = resolve_dialog_button_target(
                    &tree.widgets,
                    policy.reject_button,
                    DialogButtonRole::Reject,
                ) {
                    state.activate_dialog_target(surface_id, target);
                } else {
                    state.reject_dialog(surface_id);
                }
            } else if policy.close_on_backdrop && response.backdrop_response.clicked() {
                state.reject_dialog(surface_id);
            }
        }
    }

    state.restore_pending_focus(ui.ctx());
    exit
}

fn render_surface(
    ui: &mut egui::Ui,
    tree: &UiTree,
    state: &mut PreviewState,
    panel_rect: egui::Rect,
    svg_texture_cache: &mut crate::canvas::interaction::SvgTextureCache,
    show_preview_controls: bool,
    focus_target: Option<DialogButtonTarget>,
) -> (bool, Vec<(Uuid, PreviewInteraction)>) {
    let canvas_size = [tree.app_props.win_w, tree.app_props.win_h];
    let zoom = 1.0_f32;
    let pan = egui::Vec2::ZERO;
    let origin = canvas_origin(canvas_size, zoom, pan, panel_rect);

    // Canvas boundary.
    let boundary = egui::Rect::from_min_size(origin, egui::vec2(canvas_size[0], canvas_size[1]));
    ui.painter()
        .rect_filled(boundary, 0.0, egui::Color32::from_gray(22));
    ui.painter().rect_stroke(
        boundary,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
        egui::StrokeKind::Inside,
    );

    // Render each widget in draw order.
    let mut events = Vec::new();
    for widget in &tree.widgets {
        let w_rect = canvas_rect(widget, origin, zoom);
        if !panel_rect.intersects(w_rect) {
            continue;
        }
        let binding = widget.state_binding.clone().unwrap_or_default();
        events.extend(
            render_widget(
                ui,
                widget,
                w_rect,
                &binding,
                state,
                svg_texture_cache,
                focus_target,
            )
            .into_iter()
            .map(|interaction| (widget.id, interaction)),
        );
    }

    let exit = if show_preview_controls {
        // "PREVIEW" badge — top-left of panel.
        let badge_rect = egui::Rect::from_min_size(
            panel_rect.min + egui::vec2(8.0, 8.0),
            egui::vec2(70.0, 20.0),
        );
        ui.painter()
            .rect_filled(badge_rect, 4.0, egui::Color32::from_rgb(251, 191, 36));
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            "PREVIEW",
            egui::FontId::proportional(10.0),
            egui::Color32::BLACK,
        );

        let exit_max = panel_rect.max - egui::vec2(8.0, 8.0);
        let exit_rect = egui::Rect::from_min_max(exit_max - egui::vec2(148.0, 26.0), exit_max);
        ui.put(exit_rect, egui::Button::new("Exit Preview  [F5]").small())
            .clicked()
    } else {
        false
    };
    (exit, events)
}

// ---------------------------------------------------------------------------
// Per-widget rendering
// ---------------------------------------------------------------------------

fn render_widget(
    ui: &mut egui::Ui,
    widget: &crate::project::schema::WidgetInstance,
    w_rect: egui::Rect,
    binding: &str,
    state: &mut PreviewState,
    svg_texture_cache: &mut crate::canvas::interaction::SvgTextureCache,
    focus_target: Option<DialogButtonTarget>,
) -> Vec<PreviewInteraction> {
    let mut events = Vec::new();
    let size = w_rect.size();
    match &widget.kind {
        WidgetKind::Button => {
            let label = widget.props.label.clone();
            let response = ui
                .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, egui::Button::new(&label))
                })
                .inner;
            request_preview_widget_focus(&response, widget.id, focus_target);
            push_response_events(&response, &mut events);
        }
        WidgetKind::Label => {
            let label = widget.props.label.clone();
            // Mirror the codegen text alignment (egui_emitter / export) so the
            // preview surface does not diverge from generated output.
            let align = match widget.text_align {
                Some(crate::project::schema::TextAlign::Center) => Some(egui::Align::Center),
                Some(crate::project::schema::TextAlign::Right) => Some(egui::Align::RIGHT),
                _ => None,
            };
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                if let Some(a) = align {
                    ui.with_layout(egui::Layout::top_down(a), |ui| {
                        ui.add_sized(size, egui::Label::new(&label));
                    });
                } else {
                    ui.add_sized(size, egui::Label::new(&label));
                }
            });
        }
        WidgetKind::TextInput => {
            let hint = widget.props.placeholder.clone();
            let password = widget.props.password_mode;
            if let Some(PreviewValue::Str(s)) = state.values.get_mut(binding) {
                let te = egui::TextEdit::singleline(s)
                    .hint_text(&hint)
                    .password(password);
                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                        ui.add_sized(size, te)
                    })
                    .inner;
                request_preview_widget_focus(&response, widget.id, focus_target);
                push_response_events(&response, &mut events);
            } else {
                placeholder_box(ui, w_rect, "txt");
            }
        }
        WidgetKind::Slider => {
            let min = widget.props.min;
            let max = widget.props.max;
            if let Some(PreviewValue::Float(f)) = state.values.get_mut(binding) {
                let sl = egui::Slider::new(f, min..=max);
                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                        ui.add_sized(size, sl)
                    })
                    .inner;
                request_preview_widget_focus(&response, widget.id, focus_target);
                push_response_events(&response, &mut events);
            } else {
                placeholder_box(ui, w_rect, "sldr");
            }
        }
        WidgetKind::Checkbox => {
            let label = widget.props.label.clone();
            if let Some(PreviewValue::Bool(b)) = state.values.get_mut(binding) {
                let cb = egui::Checkbox::new(b, &label);
                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                        ui.add_sized(size, cb)
                    })
                    .inner;
                request_preview_widget_focus(&response, widget.id, focus_target);
                push_response_events(&response, &mut events);
            } else {
                placeholder_box(ui, w_rect, "chk");
            }
        }
        WidgetKind::RadioButton => {
            let label = widget.props.label.clone();
            if let Some(PreviewValue::Str(selected)) = state.values.get_mut(binding) {
                let checked = *selected == label;
                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                        ui.add_sized(size, egui::RadioButton::new(checked, &label))
                    })
                    .inner;
                request_preview_widget_focus(&response, widget.id, focus_target);
                if response.clicked() {
                    *selected = label;
                    events.push(PreviewInteraction::Widget(WidgetEvent::Change));
                }
            } else {
                placeholder_box(ui, w_rect, "radio");
            }
        }
        WidgetKind::ProgressBar => {
            let progress = state
                .values
                .get(binding)
                .and_then(|v| {
                    if let PreviewValue::Float(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .unwrap_or(widget.props.default_value);
            let animated = widget.props.animated;
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_sized(size, egui::ProgressBar::new(progress).animate(animated));
            });
        }
        WidgetKind::ComboBox => {
            // Simplified: show selected text as a read-only-looking button.
            let label = state
                .values
                .get(binding)
                .and_then(|v| {
                    if let PreviewValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| widget.props.label.clone());
            let cid = egui::Id::new(("preview_combo", widget.id));
            let mut dummy = label.clone();
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::ComboBox::from_id_salt(cid)
                    .selected_text(&dummy)
                    .width(size.x)
                    .show_ui(ui, |ui| {
                        for opt in &widget.props.options {
                            ui.selectable_value(&mut dummy, opt.clone(), opt);
                        }
                    });
            });
            // Write back if selection changed.
            if dummy != label {
                state
                    .values
                    .insert(binding.to_string(), PreviewValue::Str(dummy));
                events.push(PreviewInteraction::Widget(WidgetEvent::Change));
            }
        }
        WidgetKind::Frame => {
            let margin = widget.props.inner_margin;
            let stroke_w = widget.props.stroke_width;
            let stroke_col = widget
                .props
                .stroke_color
                .map_or(egui::Color32::from_gray(100), |[r, g, b]| {
                    egui::Color32::from_rgb(r, g, b)
                });
            let style = ui.style().clone();
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::Frame::group(&style)
                    .inner_margin(egui::Margin::from(margin))
                    .stroke(egui::Stroke::new(stroke_w, stroke_col))
                    .show(ui, |_ui| {});
            });
        }
        WidgetKind::TextArea => {
            let hint = widget.props.placeholder.clone();
            if let Some(PreviewValue::Str(s)) = state.values.get_mut(binding) {
                let te = egui::TextEdit::multiline(s).hint_text(&hint);
                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                        ui.add_sized(size, te)
                    })
                    .inner;
                request_preview_widget_focus(&response, widget.id, focus_target);
                push_response_events(&response, &mut events);
            } else {
                placeholder_box(ui, w_rect, "area");
            }
        }
        WidgetKind::SpinBox => {
            let min = widget.props.min;
            let max = widget.props.max;
            if let Some(PreviewValue::Float(f)) = state.values.get_mut(binding) {
                let dv = egui::DragValue::new(f).range(min..=max);
                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                        ui.add_sized(size, dv)
                    })
                    .inner;
                request_preview_widget_focus(&response, widget.id, focus_target);
                push_response_events(&response, &mut events);
            } else {
                placeholder_box(ui, w_rect, "spin");
            }
        }
        WidgetKind::FontComboBox => {
            const FONTS: &[&str] = &["Proportional", "Monospace"];
            let selected = state
                .values
                .get(binding)
                .and_then(|v| {
                    if let PreviewValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "Proportional".to_owned());
            let mut sel = selected.clone();
            let cid = egui::Id::new(("preview_font_combo", widget.id));
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::ComboBox::from_id_salt(cid)
                    .selected_text(&sel)
                    .width(size.x)
                    .show_ui(ui, |ui| {
                        for f in FONTS {
                            ui.selectable_value(&mut sel, f.to_string(), *f);
                        }
                    });
            });
            if sel != selected {
                state
                    .values
                    .insert(binding.to_string(), PreviewValue::Str(sel));
                events.push(PreviewInteraction::Widget(WidgetEvent::Change));
            }
        }
        WidgetKind::HorizontalSpacer => {
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_space(w_rect.width());
            });
        }
        WidgetKind::VerticalSpacer => {
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.add_space(w_rect.height());
            });
        }
        WidgetKind::GroupBox => {
            let label = widget.props.label.clone();
            let style = ui.style().clone();
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::Frame::group(&style).show(ui, |ui| {
                    ui.label(&label);
                });
            });
        }
        WidgetKind::VLayout
        | WidgetKind::HLayout
        | WidgetKind::ScrollArea
        | WidgetKind::GridLayout
        | WidgetKind::TabWidget => {
            let tag = match &widget.kind {
                WidgetKind::VLayout => "↕",
                WidgetKind::HLayout => "↔",
                WidgetKind::GridLayout => "⊞",
                WidgetKind::TabWidget => "⊡",
                _ => "⊡",
            };
            placeholder_box(ui, w_rect, tag);
        }
        WidgetKind::ToolButton => {
            let label = widget.props.label.clone();
            let response = ui
                .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, egui::Button::new(&label))
                })
                .inner;
            request_preview_widget_focus(&response, widget.id, focus_target);
            push_response_events(&response, &mut events);
        }
        WidgetKind::CommandLinkButton => {
            let title = widget.props.label.clone();
            let desc = widget.props.placeholder.clone();
            let response = ui
                .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.add_sized(size, egui::Button::new(format!("{title}\n{desc}")))
                })
                .inner;
            request_preview_widget_focus(&response, widget.id, focus_target);
            push_response_events(&response, &mut events);
        }
        WidgetKind::DialogButtonBox => {
            let buttons = crate::project::schema::effective_dialog_buttons(&widget.props);
            let mut focus_assigned = false;
            let clicked = ui
                .scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                    ui.horizontal(|ui| {
                        for button in &buttons {
                            let response = ui.button(&button.label);
                            if !focus_assigned
                                && matches!(
                                    focus_target,
                                    Some(DialogButtonTarget::Role {
                                        widget: target,
                                        role,
                                    }) if target == widget.id && role == button.role
                                )
                            {
                                response.request_focus();
                                focus_assigned = true;
                            }
                            if response.clicked() {
                                return Some(button.role);
                            }
                        }
                        None
                    })
                })
                .inner
                .inner;
            if let Some(role) = clicked {
                events.push(PreviewInteraction::DialogButton(role));
            }
        }
        WidgetKind::MathLabel => {
            let val = state.values.get(binding).and_then(|v| {
                if let PreviewValue::Float(f) = v {
                    Some(*f)
                } else {
                    None
                }
            });
            let label = widget.props.label.clone();
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.label(format!("{label} = {:.2}", val.unwrap_or(0.0)));
            });
        }
        WidgetKind::FilePicker => {
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                ui.horizontal(|ui| {
                    let _ = ui.button("Browse…");
                    let path = state.values.get(binding).and_then(|v| {
                        if let PreviewValue::Str(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                    ui.label(path.unwrap_or_else(|| "(no file)".to_owned()));
                });
            });
        }
        WidgetKind::ListView => {
            let opts = widget.props.options.clone();
            ui.scope_builder(egui::UiBuilder::new().max_rect(w_rect), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("preview_list", widget.id))
                    .show(ui, |ui| {
                        for o in &opts {
                            ui.label(o);
                        }
                    });
            });
        }
        WidgetKind::Chart
        | WidgetKind::Table
        | WidgetKind::TreeView
        | WidgetKind::StackedWidget
        | WidgetKind::ToolBox => {
            let tag = match &widget.kind {
                WidgetKind::Chart => "chart",
                WidgetKind::Table => "table",
                WidgetKind::TreeView => "tree",
                WidgetKind::StackedWidget => "stack",
                _ => "toolbox",
            };
            placeholder_box(ui, w_rect, tag);
        }
        WidgetKind::Image => {
            // Render the real rasterized SVG (matches the design canvas + export),
            // reusing the shared texture cache so preview layout/visual matches.
            if let Some(svg) = widget.svg_source.as_deref() {
                let ppp = ui.ctx().pixels_per_point();
                let tw = ((w_rect.width() * ppp).round() as u32).clamp(1, 4096);
                let th = ((w_rect.height() * ppp).round() as u32).clamp(1, 4096);
                let needs = svg_texture_cache
                    .get(&widget.id)
                    .map(|(_, _, ctw, cth)| *ctw != tw || *cth != th)
                    .unwrap_or(true);
                if needs {
                    let image = crate::canvas::svg_rasterizer::rasterize_or_fallback(svg, tw, th);
                    let tex = ui.ctx().load_texture(
                        format!("svg_{}", widget.id),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    svg_texture_cache.insert(widget.id, (tex, ppp, tw, th));
                }
                if let Some((tex, _, _, _)) = svg_texture_cache.get(&widget.id) {
                    ui.painter().image(
                        tex.id(),
                        w_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    placeholder_box(ui, w_rect, "img");
                }
            } else {
                placeholder_box(ui, w_rect, "img");
            }
        }
        WidgetKind::Custom(_) => {
            placeholder_box(ui, w_rect, "cst");
        }
    }
    events
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn push_response_events(response: &egui::Response, events: &mut Vec<PreviewInteraction>) {
    if response.clicked() {
        events.push(PreviewInteraction::Widget(WidgetEvent::Click));
    }
    if response.double_clicked() {
        events.push(PreviewInteraction::Widget(WidgetEvent::DoubleClick));
    }
    if response.changed() {
        events.push(PreviewInteraction::Widget(WidgetEvent::Change));
    }
    if response.lost_focus() {
        events.push(PreviewInteraction::Widget(WidgetEvent::LostFocus));
    }
    if response.drag_stopped() {
        events.push(PreviewInteraction::Widget(WidgetEvent::DragStopped));
    }
}

fn request_preview_widget_focus(
    response: &egui::Response,
    widget: Uuid,
    focus_target: Option<DialogButtonTarget>,
) {
    if matches!(
        focus_target,
        Some(DialogButtonTarget::Widget(target)) if target == widget
    ) {
        response.request_focus();
    }
}

fn placeholder_box(ui: &mut egui::Ui, rect: egui::Rect, tag: &str) {
    ui.painter()
        .rect_filled(rect, 2.0, egui::Color32::from_gray(35));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::monospace(9.0),
        egui::Color32::from_gray(100),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{Behavior, WidgetEvent, WidgetInstance, WidgetProps};

    fn transactional_document() -> (ProjectDocument, Uuid, Uuid) {
        let mut document = ProjectDocument::default();
        let opener = Uuid::from_u128(0xA01);
        document.root_surface_mut().tree.add(WidgetInstance {
            id: opener,
            kind: WidgetKind::Button,
            ..Default::default()
        });
        document.root_surface_mut().tree.add(WidgetInstance {
            id: Uuid::from_u128(0xA02),
            kind: WidgetKind::TextInput,
            state_binding: Some("name".to_owned()),
            props: WidgetProps {
                label: "Original".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        });
        let dialog = document.add_modal_surface("Editor");
        document
            .surface_mut(dialog)
            .expect("dialog")
            .tree
            .add(WidgetInstance {
                id: Uuid::from_u128(0xA03),
                kind: WidgetKind::TextInput,
                state_binding: Some("name".to_owned()),
                ..Default::default()
            });
        document.props.behaviors.push(Behavior::widget(
            Uuid::from_u128(0xA04),
            opener,
            WidgetEvent::Click,
            None,
            VisualAction::OpenModal { surface: dialog },
        ));
        (document, opener, dialog)
    }

    #[test]
    fn open_is_idempotent_and_reject_discards_draft() {
        let (document, opener, dialog) = transactional_document();
        let mut state = PreviewState::init_from_document(&document);

        state.dispatch_widget_event(opener, WidgetEvent::Click);
        state.dispatch_widget_event(opener, WidgetEvent::Click);
        assert_eq!(state.modal_stack.len(), 1);
        state.modal_stack[0]
            .draft
            .insert("name".to_owned(), PreviewValue::Str("Changed".to_owned()));

        assert!(state.reject_dialog(dialog));
        assert_eq!(
            state.values.get("name"),
            Some(&PreviewValue::Str("Original".to_owned()))
        );
    }

    #[test]
    fn accept_commits_draft_and_emits_result_then_closed() {
        let (mut document, opener, dialog) = transactional_document();
        document.props.behaviors.push(Behavior::surface(
            Uuid::from_u128(0xA05),
            dialog,
            SurfaceEvent::Accepted,
            VisualAction::Set {
                field: "accepted".to_owned(),
                value: crate::project::schema::ValueExpr::Flag(true),
            },
        ));
        document.props.behaviors.push(Behavior::surface(
            Uuid::from_u128(0xA06),
            dialog,
            SurfaceEvent::Closed,
            VisualAction::Set {
                field: "closed".to_owned(),
                value: crate::project::schema::ValueExpr::Flag(true),
            },
        ));
        let mut state = PreviewState::init_from_document(&document);
        state.dispatch_widget_event(opener, WidgetEvent::Click);
        state.modal_stack[0]
            .draft
            .insert("name".to_owned(), PreviewValue::Str("Committed".to_owned()));

        assert!(state.accept_dialog(dialog));
        assert_eq!(
            state.values.get("name"),
            Some(&PreviewValue::Str("Committed".to_owned()))
        );
        assert_eq!(
            state.values.get("accepted"),
            Some(&PreviewValue::Bool(true))
        );
        assert_eq!(state.values.get("closed"), Some(&PreviewValue::Bool(true)));
    }

    #[test]
    fn nested_dialogs_close_from_the_top_only() {
        let (mut document, _, first) = transactional_document();
        let second = document.add_modal_surface("Confirm");
        let mut state = PreviewState::init_from_document(&document);

        assert!(state.open_modal(first));
        assert!(state.open_modal(second));
        assert!(!state.reject_dialog(first));
        assert!(state.reject_dialog(second));
        assert!(state.reject_dialog(first));
        assert!(state.modal_stack.is_empty());
    }

    #[test]
    fn modal_focus_lifecycle_records_and_restores_the_opener() {
        let (document, _, dialog) = transactional_document();
        let mut state = PreviewState::init_from_document(&document);
        let opener = egui::Id::new("preview_opener");

        assert!(state.open_modal_with_focus(dialog, Some(opener)));
        assert!(state.is_initial_focus_pending(dialog));
        assert!(state.reject_dialog(dialog));
        assert_eq!(state.take_pending_focus_restore(), Some(opener));
    }

    #[test]
    fn modal_drafts_preserve_supported_vector_state() {
        let mut document = ProjectDocument::default();
        let dialog = document.add_modal_surface("Data");
        let surface = document.surface_mut(dialog).expect("dialog");
        surface.tree.add(WidgetInstance {
            kind: WidgetKind::Chart,
            state_binding: Some("series".to_owned()),
            ..Default::default()
        });
        surface.tree.add(WidgetInstance {
            kind: WidgetKind::ListView,
            props: WidgetProps {
                options: vec!["Alpha".to_owned(), "Beta".to_owned()],
                data_source_binding: Some("items".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut state = PreviewState::init_from_document(&document);

        assert!(state.open_modal(dialog));
        assert_eq!(
            state.modal_stack[0].draft.get("series"),
            Some(&PreviewValue::FloatList(vec![0.2, 0.5, 0.8, 0.4]))
        );
        assert_eq!(
            state.modal_stack[0].draft.get("items"),
            Some(&PreviewValue::StringList(vec![
                "Alpha".to_owned(),
                "Beta".to_owned()
            ]))
        );
    }

    #[test]
    fn isolated_preview_targets_one_saved_surface() {
        let (document, _, dialog) = transactional_document();

        let state = PreviewState::init_for_surface(&document, dialog);

        assert_eq!(state.preview_surface(), Some(dialog));
        assert!(state.project().is_some());
    }
}
