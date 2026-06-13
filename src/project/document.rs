//! Canonical multi-surface RohKai project document.
//!
//! A [`ProjectDocument`] owns project-global resources and one or more
//! independently editable [`UiSurface`] values. Each surface owns exactly one
//! [`UiTree`], which remains the source of truth for that surface's widgets.

use crate::project::{
    schema::{AppProps, AssetEntry, Behavior, DesignComponent, RustWiring, ThemeSettings},
    ui_tree::UiTree,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};
use uuid::Uuid;

/// Project-global properties shared by every surface.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectProps {
    #[serde(default)]
    pub theme: ThemeSettings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<DesignComponent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<AssetEntry>,
    #[serde(default)]
    pub rust_wiring: RustWiring,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behaviors: Vec<Behavior>,
}

/// Window/canvas properties owned by one design surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceProps {
    pub title: String,
    pub size: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
    #[serde(default = "default_true")]
    pub resizable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_size: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<crate::project::schema::GuideRule>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_bezel: bool,
}

impl Default for SurfaceProps {
    fn default() -> Self {
        Self {
            title: "My App".to_owned(),
            size: [800.0, 600.0],
            icon_path: None,
            resizable: true,
            min_size: None,
            max_size: None,
            guides: Vec::new(),
            show_bezel: false,
        }
    }
}

/// Modal-dialog-specific behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalDialogProps {
    /// Backdrop clicks are intentionally ignored by default.
    #[serde(default)]
    pub close_on_backdrop: bool,
    /// Escape rejects the topmost modal.
    #[serde(default = "default_true")]
    pub reject_on_escape: bool,
    /// Widget activated by Enter when it is present and enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_button: Option<Uuid>,
    /// Widget activated by Escape before the dialog-level reject fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reject_button: Option<Uuid>,
}

impl Default for ModalDialogProps {
    fn default() -> Self {
        Self {
            close_on_backdrop: false,
            reject_on_escape: true,
            default_button: None,
            reject_button: None,
        }
    }
}

/// Role of one design surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    MainWindow,
    ModalDialog(ModalDialogProps),
}

/// One independently editable form.
#[derive(Debug, Clone)]
pub struct UiSurface {
    pub id: Uuid,
    pub name: String,
    pub kind: SurfaceKind,
    pub props: SurfaceProps,
    pub tree: UiTree,
}

#[derive(Serialize)]
struct UiSurfaceRef<'a> {
    id: Uuid,
    name: &'a str,
    kind: &'a SurfaceKind,
    props: &'a SurfaceProps,
    tree: SurfaceTreeRef<'a>,
}

#[derive(Serialize)]
struct SurfaceTreeRef<'a> {
    widgets: &'a [crate::project::schema::WidgetInstance],
}

#[derive(Deserialize)]
struct UiSurfaceOwned {
    id: Uuid,
    name: String,
    kind: SurfaceKind,
    #[serde(default)]
    props: SurfaceProps,
    #[serde(default)]
    tree: SurfaceTreeOwned,
}

#[derive(Default, Deserialize)]
struct SurfaceTreeOwned {
    #[serde(default)]
    widgets: Vec<crate::project::schema::WidgetInstance>,
}

impl Serialize for UiSurface {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        UiSurfaceRef {
            id: self.id,
            name: &self.name,
            kind: &self.kind,
            props: &self.props,
            tree: SurfaceTreeRef {
                widgets: &self.tree.widgets,
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UiSurface {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let surface = UiSurfaceOwned::deserialize(deserializer)?;
        Ok(Self {
            id: surface.id,
            name: surface.name,
            kind: surface.kind,
            props: surface.props,
            tree: UiTree {
                widgets: surface.tree.widgets,
                app_props: AppProps::default(),
            },
        })
    }
}

/// Canonical source of truth for a RohKai project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocument {
    #[serde(default)]
    pub props: ProjectProps,
    pub root_surface: Uuid,
    pub surfaces: Vec<UiSurface>,
}

impl Default for ProjectDocument {
    fn default() -> Self {
        let root_surface = Uuid::new_v4();
        Self {
            props: ProjectProps::default(),
            root_surface,
            surfaces: vec![UiSurface {
                id: root_surface,
                name: "Main Window".to_owned(),
                kind: SurfaceKind::MainWindow,
                props: SurfaceProps::default(),
                tree: UiTree::default(),
            }],
        }
    }
}

impl ProjectDocument {
    /// Convert the former single-tree project model without losing properties.
    pub fn from_legacy_tree(mut tree: UiTree) -> Self {
        let legacy = std::mem::take(&mut tree.app_props);
        let root_surface = Uuid::new_v4();
        let mut document = Self {
            props: ProjectProps {
                theme: legacy.theme,
                components: legacy.components,
                assets: legacy.assets,
                rust_wiring: legacy.rust_wiring,
                behaviors: legacy.behaviors,
            },
            root_surface,
            surfaces: vec![UiSurface {
                id: root_surface,
                name: "Main Window".to_owned(),
                kind: SurfaceKind::MainWindow,
                props: SurfaceProps {
                    title: legacy.title,
                    size: [legacy.win_w, legacy.win_h],
                    icon_path: legacy.icon_path,
                    resizable: legacy.resizable,
                    min_size: legacy.min_size,
                    max_size: legacy.max_size,
                    guides: legacy.guides,
                    show_bezel: legacy.show_bezel,
                },
                tree,
            }],
        };
        document.validate_and_repair();
        document
    }

    #[must_use]
    pub fn root_surface(&self) -> &UiSurface {
        self.surface(self.root_surface)
            .expect("validated ProjectDocument always has a root surface")
    }

    pub fn root_surface_mut(&mut self) -> &mut UiSurface {
        let root = self.root_surface;
        self.surface_mut(root)
            .expect("validated ProjectDocument always has a root surface")
    }

    #[must_use]
    pub fn surface(&self, id: Uuid) -> Option<&UiSurface> {
        self.surfaces.iter().find(|surface| surface.id == id)
    }

    pub fn surface_mut(&mut self, id: Uuid) -> Option<&mut UiSurface> {
        self.surfaces.iter_mut().find(|surface| surface.id == id)
    }

    pub fn add_modal_surface(&mut self, preferred_name: impl AsRef<str>) -> Uuid {
        let name = self.unique_surface_name(preferred_name.as_ref(), None);
        let id = Uuid::new_v4();
        self.surfaces.push(UiSurface {
            id,
            name: name.clone(),
            kind: SurfaceKind::ModalDialog(ModalDialogProps::default()),
            props: SurfaceProps {
                title: name,
                size: [480.0, 320.0],
                resizable: false,
                ..Default::default()
            },
            tree: UiTree::default(),
        });
        id
    }

    pub fn rename_surface(&mut self, id: Uuid, preferred_name: impl AsRef<str>) -> bool {
        let name = self.unique_surface_name(preferred_name.as_ref(), Some(id));
        let Some(surface) = self.surface_mut(id) else {
            return false;
        };
        surface.name = name;
        true
    }

    pub fn remove_surface(&mut self, id: Uuid) -> bool {
        if id == self.root_surface {
            return false;
        }
        let Some(index) = self.surfaces.iter().position(|surface| surface.id == id) else {
            return false;
        };
        let removed_ids: HashSet<Uuid> = self.surfaces[index]
            .tree
            .widgets
            .iter()
            .map(|widget| widget.id)
            .collect();
        self.surfaces.remove(index);
        self.props
            .behaviors
            .retain(|behavior| !removed_ids.contains(&behavior.source_widget));
        for behavior in &mut self.props.behaviors {
            if behavior
                .target_widget
                .is_some_and(|target| removed_ids.contains(&target))
            {
                behavior.target_widget = None;
            }
        }
        true
    }

    pub fn move_surface(&mut self, id: Uuid, target_index: usize) -> bool {
        if id == self.root_surface {
            return false;
        }
        let Some(source_index) = self.surfaces.iter().position(|surface| surface.id == id) else {
            return false;
        };
        let surface = self.surfaces.remove(source_index);
        let target = target_index.clamp(1, self.surfaces.len());
        self.surfaces.insert(target, surface);
        true
    }

    pub fn duplicate_surface(&mut self, source: Uuid) -> Option<Uuid> {
        let original = self.surface(source)?.clone();
        let widget_ids: HashSet<Uuid> = original
            .tree
            .widgets
            .iter()
            .map(|widget| widget.id)
            .collect();
        let name = self.unique_surface_name(&format!("{} Copy", original.name), None);
        let new_surface_id = Uuid::new_v4();
        let mut duplicate = original;
        duplicate.id = new_surface_id;
        duplicate.name = name.clone();
        duplicate.props.title = name;
        duplicate.kind = SurfaceKind::ModalDialog(ModalDialogProps::default());

        let mut id_map = HashMap::with_capacity(duplicate.tree.widgets.len());
        for widget in &mut duplicate.tree.widgets {
            let old = widget.id;
            widget.id = Uuid::new_v4();
            id_map.insert(old, widget.id);
        }
        for widget in &mut duplicate.tree.widgets {
            for child in &mut widget.children {
                if let Some(replacement) = id_map.get(child) {
                    *child = *replacement;
                }
            }
        }

        let duplicated_behaviors: Vec<Behavior> = self
            .props
            .behaviors
            .iter()
            .filter(|behavior| widget_ids.contains(&behavior.source_widget))
            .map(|behavior| {
                let mut duplicated = behavior.clone();
                duplicated.id = Uuid::new_v4();
                if let Some(replacement) = id_map.get(&duplicated.source_widget) {
                    duplicated.source_widget = *replacement;
                }
                if let Some(target) = duplicated.target_widget
                    && let Some(replacement) = id_map.get(&target)
                {
                    duplicated.target_widget = Some(*replacement);
                }
                duplicated
            })
            .collect();
        self.props.behaviors.extend(duplicated_behaviors);
        self.surfaces.push(duplicate);
        Some(new_surface_id)
    }

    /// Repair IDs, names, references, dimensions, and the single-main invariant.
    pub fn validate_and_repair(&mut self) {
        if self.surfaces.is_empty() {
            *self = Self::default();
            return;
        }

        let mut surface_ids = HashSet::new();
        for surface in &mut self.surfaces {
            if !surface_ids.insert(surface.id) {
                surface.id = Uuid::new_v4();
                surface_ids.insert(surface.id);
            }
            surface.tree.validate_and_repair();
            repair_surface_props(&mut surface.props);
        }

        if !surface_ids.contains(&self.root_surface) {
            self.root_surface = self.surfaces[0].id;
        }

        for surface in &mut self.surfaces {
            if surface.id == self.root_surface {
                surface.kind = SurfaceKind::MainWindow;
            } else if matches!(surface.kind, SurfaceKind::MainWindow) {
                surface.kind = SurfaceKind::ModalDialog(ModalDialogProps::default());
            }
        }

        let mut used_names = HashSet::new();
        for surface in &mut self.surfaces {
            let base = normalized_surface_name(&surface.name);
            let mut candidate = base.clone();
            let mut suffix = 2usize;
            while !used_names.insert(candidate.to_lowercase()) {
                candidate = format!("{base} {suffix}");
                suffix += 1;
            }
            surface.name = candidate;
        }

        let live_widgets: HashSet<Uuid> = self
            .surfaces
            .iter()
            .flat_map(|surface| surface.tree.widgets.iter().map(|widget| widget.id))
            .collect();
        self.props
            .behaviors
            .retain(|behavior| live_widgets.contains(&behavior.source_widget));
        for behavior in &mut self.props.behaviors {
            if behavior
                .target_widget
                .is_some_and(|target| !live_widgets.contains(&target))
            {
                behavior.target_widget = None;
            }
        }
    }

    fn unique_surface_name(&self, preferred: &str, except: Option<Uuid>) -> String {
        let base = normalized_surface_name(preferred);
        let used: HashSet<String> = self
            .surfaces
            .iter()
            .filter(|surface| Some(surface.id) != except)
            .map(|surface| surface.name.to_lowercase())
            .collect();
        if !used.contains(&base.to_lowercase()) {
            return base;
        }
        let mut suffix = 2usize;
        loop {
            let candidate = format!("{base} {suffix}");
            if !used.contains(&candidate.to_lowercase()) {
                return candidate;
            }
            suffix += 1;
        }
    }
}

/// Session adapter exposing the active surface's tree to legacy single-tree
/// canvas/codegen paths while keeping [`ProjectDocument`] as persisted truth.
///
/// The active tree's `app_props` is a derived compatibility view. Switching or
/// snapshotting flushes that view back into the document before hydrating the
/// next surface.
#[derive(Debug, Clone)]
pub struct ActiveDocument {
    document: ProjectDocument,
    active_surface: Uuid,
}

impl Default for ActiveDocument {
    fn default() -> Self {
        Self::new(ProjectDocument::default())
    }
}

impl ActiveDocument {
    #[must_use]
    pub fn new(mut document: ProjectDocument) -> Self {
        document.validate_and_repair();
        let active_surface = document.root_surface;
        let mut active = Self {
            document,
            active_surface,
        };
        active.hydrate_active_cache();
        active
    }

    #[must_use]
    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }

    #[must_use]
    pub fn active_surface_id(&self) -> Uuid {
        self.active_surface
    }

    #[must_use]
    pub fn active_surface(&self) -> &UiSurface {
        self.document
            .surface(self.active_surface)
            .expect("validated ActiveDocument always has an active surface")
    }

    pub fn set_active_surface(&mut self, id: Uuid) -> bool {
        if id == self.active_surface {
            return true;
        }
        if self.document.surface(id).is_none() {
            return false;
        }
        self.flush_active_cache();
        self.active_surface = id;
        self.hydrate_active_cache();
        true
    }

    pub fn add_modal_surface(&mut self, name: impl AsRef<str>) -> Uuid {
        self.flush_active_cache();
        self.document.add_modal_surface(name)
    }

    pub fn rename_surface(&mut self, id: Uuid, name: impl AsRef<str>) -> bool {
        self.flush_active_cache();
        let renamed = self.document.rename_surface(id, name);
        self.hydrate_active_cache();
        renamed
    }

    pub fn duplicate_surface(&mut self, id: Uuid) -> Option<Uuid> {
        self.flush_active_cache();
        self.document.duplicate_surface(id)
    }

    pub fn remove_surface(&mut self, id: Uuid) -> bool {
        self.flush_active_cache();
        let removed = self.document.remove_surface(id);
        if removed && self.active_surface == id {
            self.active_surface = self.document.root_surface;
        }
        self.hydrate_active_cache();
        removed
    }

    pub fn move_surface(&mut self, id: Uuid, target_index: usize) -> bool {
        self.flush_active_cache();
        self.document.move_surface(id, target_index)
    }

    pub fn replace_active_tree(&mut self, tree: UiTree) {
        if let Some(surface) = self.document.surface_mut(self.active_surface) {
            surface.tree = tree;
        }
        self.hydrate_active_cache();
    }

    pub fn replace_surface_tree(&mut self, id: Uuid, tree: UiTree) -> bool {
        self.flush_active_cache();
        let Some(surface) = self.document.surface_mut(id) else {
            return false;
        };
        surface.tree = tree;
        self.hydrate_active_cache();
        true
    }

    pub fn set_modal_dialog_props(&mut self, props: ModalDialogProps) -> bool {
        let Some(surface) = self.document.surface_mut(self.active_surface) else {
            return false;
        };
        if !matches!(surface.kind, SurfaceKind::ModalDialog(_)) {
            return false;
        }
        surface.kind = SurfaceKind::ModalDialog(props);
        true
    }

    #[must_use]
    pub fn snapshot(&self) -> ProjectDocument {
        let mut active = self.clone();
        active.flush_active_cache();
        active.document
    }

    pub fn replace(&mut self, document: ProjectDocument) {
        *self = Self::new(document);
    }

    fn hydrate_active_cache(&mut self) {
        let project = self.document.props.clone();
        let Some(surface) = self.document.surface_mut(self.active_surface) else {
            return;
        };
        let widget_ids: HashSet<Uuid> = surface
            .tree
            .widgets
            .iter()
            .map(|widget| widget.id)
            .collect();
        surface.tree.app_props = AppProps {
            title: surface.props.title.clone(),
            win_w: surface.props.size[0],
            win_h: surface.props.size[1],
            icon_path: surface.props.icon_path.clone(),
            resizable: surface.props.resizable,
            min_size: surface.props.min_size,
            max_size: surface.props.max_size,
            theme: project.theme,
            guides: surface.props.guides.clone(),
            show_bezel: surface.props.show_bezel,
            components: project.components,
            assets: project.assets,
            rust_wiring: project.rust_wiring,
            behaviors: project
                .behaviors
                .into_iter()
                .filter(|behavior| widget_ids.contains(&behavior.source_widget))
                .collect(),
        };
    }

    fn flush_active_cache(&mut self) {
        let Some(surface) = self.document.surface_mut(self.active_surface) else {
            return;
        };
        let cache = surface.tree.app_props.clone();
        surface.props = SurfaceProps {
            title: cache.title,
            size: [cache.win_w, cache.win_h],
            icon_path: cache.icon_path,
            resizable: cache.resizable,
            min_size: cache.min_size,
            max_size: cache.max_size,
            guides: cache.guides,
            show_bezel: cache.show_bezel,
        };
        let widget_ids: HashSet<Uuid> = surface
            .tree
            .widgets
            .iter()
            .map(|widget| widget.id)
            .collect();
        self.document
            .props
            .behaviors
            .retain(|behavior| !widget_ids.contains(&behavior.source_widget));
        self.document.props.behaviors.extend(cache.behaviors);
        self.document.props.theme = cache.theme;
        self.document.props.components = cache.components;
        self.document.props.assets = cache.assets;
        self.document.props.rust_wiring = cache.rust_wiring;
    }
}

impl Deref for ActiveDocument {
    type Target = UiTree;

    fn deref(&self) -> &Self::Target {
        &self.active_surface().tree
    }
}

impl DerefMut for ActiveDocument {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.document
            .surface_mut(self.active_surface)
            .map(|surface| &mut surface.tree)
            .expect("validated ActiveDocument always has an active surface")
    }
}

impl From<AppProps> for (ProjectProps, SurfaceProps) {
    fn from(value: AppProps) -> Self {
        (
            ProjectProps {
                theme: value.theme,
                components: value.components,
                assets: value.assets,
                rust_wiring: value.rust_wiring,
                behaviors: value.behaviors,
            },
            SurfaceProps {
                title: value.title,
                size: [value.win_w, value.win_h],
                icon_path: value.icon_path,
                resizable: value.resizable,
                min_size: value.min_size,
                max_size: value.max_size,
                guides: value.guides,
                show_bezel: value.show_bezel,
            },
        )
    }
}

fn repair_surface_props(props: &mut SurfaceProps) {
    props.size[0] = finite_clamped(props.size[0], 800.0, 100.0, 16_384.0);
    props.size[1] = finite_clamped(props.size[1], 600.0, 100.0, 16_384.0);
    if let Some([width, height]) = &mut props.min_size {
        *width = finite_clamped(*width, 100.0, 1.0, 16_384.0);
        *height = finite_clamped(*height, 100.0, 1.0, 16_384.0);
    }
    if let Some([width, height]) = &mut props.max_size {
        *width = finite_clamped(*width, 16_384.0, 1.0, 16_384.0);
        *height = finite_clamped(*height, 16_384.0, 1.0, 16_384.0);
    }
    if let (Some(min), Some(max)) = (&props.min_size, &mut props.max_size) {
        max[0] = max[0].max(min[0]);
        max[1] = max[1].max(min[1]);
    }
}

fn finite_clamped(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn normalized_surface_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Untitled Surface".to_owned()
    } else {
        trimmed.to_owned()
    }
}

const fn default_true() -> bool {
    true
}
