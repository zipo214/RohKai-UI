use rohkai::project::{
    document::{ActiveDocument, ProjectDocument, SurfaceKind},
    io,
    schema::{Behavior, ValueExpr, VisualAction, WidgetEvent, WidgetInstance, WidgetKind},
    ui_tree::UiTree,
};
use uuid::Uuid;

#[test]
fn default_document_has_exactly_one_main_surface() {
    let document = ProjectDocument::default();

    assert_eq!(document.surfaces.len(), 1);
    assert_eq!(document.root_surface().kind, SurfaceKind::MainWindow);
}

#[test]
fn modal_surface_crud_preserves_root_and_unique_names() {
    let mut document = ProjectDocument::default();
    let first = document.add_modal_surface("Settings");
    let second = document.add_modal_surface("Settings");

    assert_ne!(first, second);
    assert_eq!(document.surface(first).map(|s| s.name.as_str()), Some("Settings"));
    assert_eq!(
        document.surface(second).map(|s| s.name.as_str()),
        Some("Settings 2")
    );
    assert!(!document.remove_surface(document.root_surface));
    assert!(document.remove_surface(first));
    assert!(document.surface(first).is_none());
}

#[test]
fn duplicating_surface_rewrites_widget_and_behavior_ids() {
    let mut document = ProjectDocument::default();
    let source = document.add_modal_surface("Editor");
    let button_id = Uuid::new_v4();
    document.surface_mut(source).unwrap().tree.add(WidgetInstance {
        id: button_id,
        kind: WidgetKind::Button,
        ..Default::default()
    });
    document.props.behaviors.push(Behavior {
        id: Uuid::new_v4(),
        source_widget: button_id,
        event: WidgetEvent::Click,
        target_widget: None,
        action: VisualAction::Set {
            field: "accepted".to_owned(),
            value: ValueExpr::Flag(true),
        },
    });

    let duplicate = document.duplicate_surface(source).expect("duplicate");
    let duplicate_widget = document.surface(duplicate).unwrap().tree.widgets[0].id;

    assert_ne!(duplicate_widget, button_id);
    assert!(document.props.behaviors.iter().any(|behavior| {
        behavior.source_widget == duplicate_widget && behavior.id != Uuid::nil()
    }));
}

#[test]
fn schema_v1_tree_migrates_without_losing_project_or_surface_state() {
    let widget_id = Uuid::new_v4();
    let mut tree = UiTree::default();
    tree.app_props.title = "Legacy App".to_owned();
    tree.app_props.win_w = 1024.0;
    tree.app_props.win_h = 768.0;
    tree.app_props.theme.dark_mode = false;
    tree.widgets.push(WidgetInstance {
        id: widget_id,
        kind: WidgetKind::Label,
        ..Default::default()
    });
    let legacy_json = serde_json::json!({
        "schema_version": 1,
        "tree": tree,
    })
    .to_string();

    let migrated = io::deserialize(&legacy_json).expect("v1 migration");

    assert_eq!(migrated.root_surface().props.title, "Legacy App");
    assert_eq!(migrated.root_surface().props.size, [1024.0, 768.0]);
    assert!(!migrated.props.theme.dark_mode);
    assert_eq!(migrated.root_surface().tree.widgets[0].id, widget_id);
}

#[test]
fn schema_v2_round_trip_preserves_multiple_surfaces() {
    let mut document = ProjectDocument::default();
    document.add_modal_surface("Settings");
    document.add_modal_surface("About");

    let json = io::serialize(&document).expect("serialize");
    let round_trip = io::deserialize(&json).expect("deserialize");

    assert_eq!(round_trip.surfaces.len(), 3);
    assert_eq!(round_trip.root_surface, document.root_surface);
}

#[test]
fn active_document_switch_flushes_surface_and_global_edits() {
    let mut active = ActiveDocument::default();
    let root = active.document().root_surface;
    let dialog = active.add_modal_surface("Settings");
    active.app_props.title = "Renamed Main".to_owned();
    active.app_props.theme.dark_mode = false;

    assert!(active.set_active_surface(dialog));
    active.app_props.title = "Settings Dialog".to_owned();
    assert!(active.set_active_surface(root));

    assert_eq!(active.app_props.title, "Renamed Main");
    assert!(!active.app_props.theme.dark_mode);
    let snapshot = active.snapshot();
    assert_eq!(snapshot.surface(dialog).unwrap().props.title, "Settings Dialog");
    assert!(!snapshot.props.theme.dark_mode);
}
