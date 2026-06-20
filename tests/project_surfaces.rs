use rohkai::panels::surfaces::{DialogTemplate, dialog_template_tree};
use rohkai::project::{
    document::{ActiveDocument, ProjectDiagnosticCode, ProjectDocument, SurfaceKind},
    io,
    schema::{
        Behavior, BehaviorTrigger, DialogButtonRole, DialogButtonSpec, SurfaceEvent,
        SurfaceEventRef, ValueExpr, VisualAction, WidgetEvent, WidgetEventRef, WidgetInstance,
        WidgetKind, WidgetProps,
    },
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
    assert_eq!(
        document.surface(first).map(|s| s.name.as_str()),
        Some("Settings")
    );
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
    document
        .surface_mut(source)
        .unwrap()
        .tree
        .add(WidgetInstance {
            id: button_id,
            kind: WidgetKind::Button,
            ..Default::default()
        });
    document.props.behaviors.push(Behavior {
        id: Uuid::new_v4(),
        trigger: BehaviorTrigger::Widget(WidgetEventRef {
            source_widget: button_id,
            event: WidgetEvent::Click,
        }),
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
        behavior.source_widget() == Some(duplicate_widget) && behavior.id != Uuid::nil()
    }));
}

#[test]
fn legacy_widget_behavior_json_migrates_to_typed_trigger() {
    let source = Uuid::new_v4();
    let json = serde_json::json!({
        "id": Uuid::new_v4(),
        "source_widget": source,
        "event": "Click",
        "action": {"Toggle": {"field": "open"}}
    });

    let behavior: Behavior = serde_json::from_value(json).expect("legacy behavior");

    assert_eq!(
        behavior.trigger,
        BehaviorTrigger::Widget(WidgetEventRef {
            source_widget: source,
            event: WidgetEvent::Click,
        })
    );
}

#[test]
fn surface_behavior_round_trip_preserves_trigger_and_modal_action() {
    let source_surface = Uuid::new_v4();
    let target_surface = Uuid::new_v4();
    let behavior = Behavior {
        id: Uuid::new_v4(),
        trigger: BehaviorTrigger::Surface(SurfaceEventRef {
            source_surface,
            event: SurfaceEvent::Opened,
        }),
        target_widget: None,
        action: VisualAction::OpenModal {
            surface: target_surface,
        },
    };

    let json = serde_json::to_string(&behavior).expect("serialize");
    let round_trip: Behavior = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(round_trip, behavior);
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
fn schema_v2_serializes_surface_trees_without_compatibility_app_props() {
    let mut document = ProjectDocument::default();
    document.root_surface_mut().tree.app_props.title = "stale cache".to_owned();

    let json = io::serialize(&document).expect("serialize");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let tree = &value["document"]["surfaces"][0]["tree"];

    assert!(tree.get("widgets").is_some());
    assert!(
        tree.get("app_props").is_none(),
        "schema v2 must persist canonical project/surface properties only"
    );
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
    assert_eq!(
        snapshot.surface(dialog).unwrap().props.title,
        "Settings Dialog"
    );
    assert!(!snapshot.props.theme.dark_mode);
}

#[test]
fn dialog_templates_are_real_editable_trees() {
    let blank = dialog_template_tree(DialogTemplate::Blank);
    let ok_cancel = dialog_template_tree(DialogTemplate::OkCancel);
    let settings = dialog_template_tree(DialogTemplate::Settings);

    assert!(blank.widgets.is_empty());
    assert!(
        ok_cancel
            .widgets
            .iter()
            .any(|widget| widget.kind == WidgetKind::DialogButtonBox)
    );
    assert!(
        settings
            .widgets
            .iter()
            .any(|widget| widget.kind == WidgetKind::TextInput)
    );
}

#[test]
fn surface_reorder_never_moves_root_from_first_position() {
    let mut document = ProjectDocument::default();
    let a = document.add_modal_surface("A");
    let b = document.add_modal_surface("B");

    assert!(document.move_surface(b, 1));
    assert!(!document.move_surface(document.root_surface, 2));
    assert_eq!(document.surfaces[0].id, document.root_surface);
    assert_eq!(document.surfaces[1].id, b);
    assert_eq!(document.surfaces[2].id, a);
}

fn exported_modal_document() -> ProjectDocument {
    let mut document = ProjectDocument::default();
    let opener = Uuid::from_u128(0xD01);
    document.root_surface_mut().tree.add(WidgetInstance {
        id: opener,
        kind: WidgetKind::Button,
        ..Default::default()
    });
    let dialog = document.add_modal_surface("Settings");
    let accept = Uuid::from_u128(0xD02);
    let reject = Uuid::from_u128(0xD03);
    let surface = document.surface_mut(dialog).expect("dialog");
    surface.tree.add(WidgetInstance {
        id: Uuid::from_u128(0xD04),
        kind: WidgetKind::TextInput,
        state_binding: Some("name".to_owned()),
        ..Default::default()
    });
    surface.tree.add(WidgetInstance {
        id: accept,
        kind: WidgetKind::Button,
        props: rohkai::project::schema::WidgetProps {
            label: "OK".to_owned(),
            ..Default::default()
        },
        ..Default::default()
    });
    surface.tree.add(WidgetInstance {
        id: reject,
        kind: WidgetKind::Button,
        props: rohkai::project::schema::WidgetProps {
            label: "Cancel".to_owned(),
            ..Default::default()
        },
        ..Default::default()
    });
    if let SurfaceKind::ModalDialog(props) = &mut surface.kind {
        props.default_button = Some(accept);
        props.reject_button = Some(reject);
    }
    document.props.behaviors.extend([
        Behavior::widget(
            Uuid::from_u128(0xD05),
            opener,
            WidgetEvent::Click,
            None,
            VisualAction::OpenModal { surface: dialog },
        ),
        Behavior::widget(
            Uuid::from_u128(0xD06),
            accept,
            WidgetEvent::Click,
            None,
            VisualAction::AcceptDialog { surface: dialog },
        ),
        Behavior::widget(
            Uuid::from_u128(0xD07),
            reject,
            WidgetEvent::Click,
            None,
            VisualAction::RejectDialog { surface: dialog },
        ),
    ]);
    document
}

#[test]
fn project_export_emits_real_modal_runtime_and_surface_modules() {
    let document = exported_modal_document();
    let files = rohkai::codegen::export::project_files_document(&document);
    let app = files
        .iter()
        .find(|(path, _)| path == "src/app.rs")
        .map(|(_, source)| source)
        .expect("app source");

    assert!(files.iter().any(|(path, _)| path == "src/surfaces/mod.rs"));
    assert!(files.iter().any(|(path, _)| {
        path.starts_with("src/surfaces/surface_settings_") && path.ends_with(".rs")
    }));
    assert!(app.contains("egui::Modal::new"));
    assert!(app.contains("pending_dialog_actions"));
    assert!(app.contains("fn commit(self, state: &mut AppState)"));
    assert!(app.contains("DialogAction::Accept"));
    assert!(app.contains("DialogAction::Reject"));
    assert!(app.contains("dialog_focus_return"));
    assert!(app.contains("if request_initial_focus { evt_response.request_focus(); }"));
    assert!(app.contains("apply_pending_dialog_actions(ctx)"));
    assert!(!app.contains("ROHKAI_DIALOG_FOCUS"));
    assert!(!app.contains("ROHKAI_WIDGETS_BEGIN\n        egui::Area::new(egui::Id::new(\"widget_00000000-0000-0000-0000-000000000d02"));
}

#[test]
fn wasm_project_export_keeps_modal_runtime() {
    let files =
        rohkai::codegen::export::project_files_document_wasm(&exported_modal_document(), true);
    let app = files
        .iter()
        .find(|(path, _)| path == "src/app.rs")
        .map(|(_, source)| source)
        .expect("app source");

    assert!(app.contains("egui::Modal::new"));
    assert!(files.iter().any(|(path, _)| path == "src/surfaces/mod.rs"));
}

#[test]
fn modal_export_rewrites_state_paths_without_mutating_literal_text() {
    let mut document = ProjectDocument::default();
    let dialog = document.add_modal_surface("Literal Safety");
    let default_button = Uuid::from_u128(0xD22);
    let surface = document.surface_mut(dialog).expect("dialog");
    surface.tree.add(WidgetInstance {
        id: Uuid::from_u128(0xD20),
        kind: WidgetKind::Label,
        props: WidgetProps {
            label: "show self.state.visible exactly".to_owned(),
            ..Default::default()
        },
        ..Default::default()
    });
    surface.tree.add(WidgetInstance {
        id: Uuid::from_u128(0xD21),
        kind: WidgetKind::TextInput,
        state_binding: Some("name".to_owned()),
        ..Default::default()
    });
    surface.tree.add(WidgetInstance {
        id: default_button,
        kind: WidgetKind::Button,
        ..Default::default()
    });
    if let SurfaceKind::ModalDialog(policy) = &mut surface.kind {
        policy.default_button = Some(default_button);
    }
    document.props.behaviors.push(Behavior::widget(
        Uuid::from_u128(0xD23),
        default_button,
        WidgetEvent::Click,
        None,
        VisualAction::Set {
            field: "name".to_owned(),
            value: ValueExpr::Text("self.state.payload".to_owned()),
        },
    ));

    let files = rohkai::codegen::export::project_files_document(&document);
    let app = files
        .iter()
        .find(|(path, _)| path == "src/app.rs")
        .map(|(_, source)| source)
        .expect("app source");

    assert!(app.contains("\"show self.state.visible exactly\""));
    assert!(!app.contains("\"show draft.visible exactly\""));
    assert!(app.contains("&mut draft.name"));
    assert!(app.contains("draft.name = \"self.state.payload\".to_owned();"));
    let enter_dispatch = app
        .rfind("consume_key(egui::Modifiers::NONE, egui::Key::Enter)")
        .expect("Enter dispatch");
    let draft_store = app.rfind(" = Some(draft);").expect("draft storage");
    assert!(
        enter_dispatch < draft_store,
        "keyboard actions must run before the local draft is moved back into self"
    );
}

#[test]
fn modal_export_routes_semantic_dialog_button_roles() {
    let mut document = ProjectDocument::default();
    let dialog = document.add_modal_surface("Confirm");
    document
        .surface_mut(dialog)
        .expect("dialog")
        .tree
        .add(WidgetInstance {
            id: Uuid::from_u128(0xDB02),
            kind: WidgetKind::DialogButtonBox,
            props: WidgetProps {
                dialog_buttons: vec![
                    DialogButtonSpec {
                        label: "OK".to_owned(),
                        role: DialogButtonRole::Accept,
                    },
                    DialogButtonSpec {
                        label: "Cancel".to_owned(),
                        role: DialogButtonRole::Reject,
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        });

    let files = rohkai::codegen::export::project_files_document(&document);
    let app = files
        .iter()
        .find(|(path, _)| path == "src/app.rs")
        .map(|(_, source)| source)
        .expect("app source");

    assert!(
        app.contains("DialogAction::Accept(surfaces::surface_confirm_"),
        "accept role was not emitted:\n{app}"
    );
    assert!(
        app.contains("DialogAction::Reject(surfaces::surface_confirm_"),
        "reject role was not emitted:\n{app}"
    );
    assert!(!app.contains("if ui.button(\"OK\").clicked() {}"));
    assert!(!app.contains("if ui.button(\"Cancel\").clicked() {}"));
}

#[test]
fn project_diagnostics_report_missing_and_recursive_modal_targets() {
    let mut document = ProjectDocument::default();
    let first = document.add_modal_surface("First");
    let second = document.add_modal_surface("Second");
    let first_button = Uuid::from_u128(0xDA01);
    let second_button = Uuid::from_u128(0xDA02);
    document
        .surface_mut(first)
        .expect("first")
        .tree
        .add(WidgetInstance {
            id: first_button,
            kind: WidgetKind::Button,
            ..Default::default()
        });
    document
        .surface_mut(second)
        .expect("second")
        .tree
        .add(WidgetInstance {
            id: second_button,
            kind: WidgetKind::Button,
            ..Default::default()
        });
    document.props.behaviors.extend([
        Behavior::widget(
            Uuid::from_u128(0xDA03),
            first_button,
            WidgetEvent::Click,
            None,
            VisualAction::OpenModal { surface: second },
        ),
        Behavior::widget(
            Uuid::from_u128(0xDA04),
            second_button,
            WidgetEvent::Click,
            None,
            VisualAction::OpenModal { surface: first },
        ),
        Behavior::widget(
            Uuid::from_u128(0xDA05),
            first_button,
            WidgetEvent::DoubleClick,
            None,
            VisualAction::OpenModal {
                surface: Uuid::from_u128(0xDEAD),
            },
        ),
    ]);

    let diagnostics = document.diagnostics();

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::RecursiveModalOpen })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::MissingModalTarget })
    );
}

#[test]
fn project_diagnostics_report_dialog_policy_and_unsupported_draft_fields() {
    let mut document = ProjectDocument::default();
    let dialog = document.add_modal_surface("Data");
    let surface = document.surface_mut(dialog).expect("dialog");
    surface.tree.add(WidgetInstance {
        id: Uuid::from_u128(0xDA10),
        kind: WidgetKind::Table,
        props: WidgetProps {
            data_source_binding: Some("rows".to_owned()),
            ..Default::default()
        },
        ..Default::default()
    });
    if let SurfaceKind::ModalDialog(policy) = &mut surface.kind {
        policy.default_button = Some(Uuid::from_u128(0xBAD0));
    }

    let diagnostics = document.diagnostics();

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::DanglingDialogButton })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == ProjectDiagnosticCode::UnsupportedDraftField })
    );
}

#[test]
fn invalid_dialog_control_policy_is_diagnosed_and_repaired() {
    let mut document = ProjectDocument::default();
    let dialog = document.add_modal_surface("Invalid Policy");
    let label = Uuid::from_u128(0xDA20);
    let surface = document.surface_mut(dialog).expect("dialog");
    surface.tree.add(WidgetInstance {
        id: label,
        kind: WidgetKind::Label,
        ..Default::default()
    });
    if let SurfaceKind::ModalDialog(policy) = &mut surface.kind {
        policy.default_button = Some(label);
        policy.reject_button = Some(Uuid::from_u128(0xDA21));
    }

    let policy_diagnostics = document
        .diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.code == ProjectDiagnosticCode::DanglingDialogButton)
        .count();
    assert_eq!(policy_diagnostics, 2);

    document.validate_and_repair();
    let SurfaceKind::ModalDialog(policy) = &document.surface(dialog).expect("dialog").kind else {
        panic!("dialog kind changed");
    };
    assert_eq!(policy.default_button, None);
    assert_eq!(policy.reject_button, None);
}

#[test]
fn document_app_state_aggregates_bindings_from_every_surface() {
    let mut document = ProjectDocument::default();
    document.root_surface_mut().tree.add(WidgetInstance {
        kind: WidgetKind::TextInput,
        state_binding: Some("main_name".to_owned()),
        ..Default::default()
    });
    let dialog = document.add_modal_surface("Editor");
    document
        .surface_mut(dialog)
        .expect("dialog")
        .tree
        .add(WidgetInstance {
            kind: WidgetKind::Checkbox,
            state_binding: Some("dialog_enabled".to_owned()),
            ..Default::default()
        });

    let aggregate = rohkai::codegen::export::project_state_tree(&document);
    let state = rohkai::codegen::state_emitter::emit(&aggregate);

    assert!(state.contains("main_name: String"));
    assert!(state.contains("dialog_enabled: bool"));
}

#[test]
fn modal_export_reports_and_excludes_unsupported_draft_fields() {
    let mut document = ProjectDocument::default();
    let dialog = document.add_modal_surface("Rows");
    document
        .surface_mut(dialog)
        .expect("dialog")
        .tree
        .add(WidgetInstance {
            kind: WidgetKind::Table,
            props: WidgetProps {
                data_source_binding: Some("rows".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        });

    let files = rohkai::codegen::export::project_files_document(&document);
    let app = files
        .iter()
        .find(|(path, _)| path == "src/app.rs")
        .map(|(_, source)| source)
        .expect("app source");
    let draft_start = app.find("struct Draft").expect("draft");
    let draft_end = app[draft_start..]
        .find("\n}\n")
        .map(|offset| draft_start + offset)
        .expect("draft end");
    let draft = &app[draft_start..draft_end];

    assert!(app.contains("DIALOG DIAGNOSTIC"));
    assert!(app.contains("rows"));
    assert!(!draft.contains("rows: Vec<Vec<String>>"));
}

#[test]
fn surface_export_fixture_is_available_to_external_compile_gate() {
    let Ok(destination) = std::env::var("ROHKAI_SURFACE_EXPORT_DEST") else {
        return;
    };
    let destination = std::path::PathBuf::from(destination);
    if destination.exists() {
        std::fs::remove_dir_all(&destination).expect("remove old fixture");
    }
    rohkai::codegen::export::write_project_document(&exported_modal_document(), &destination)
        .expect("write fixture");
}

#[test]
fn surface_wasm_export_fixture_is_available_to_external_compile_gate() {
    let Ok(destination) = std::env::var("ROHKAI_SURFACE_WASM_EXPORT_DEST") else {
        return;
    };
    let destination = std::path::PathBuf::from(destination);
    if destination.exists() {
        std::fs::remove_dir_all(&destination).expect("remove old fixture");
    }
    rohkai::codegen::export::write_project_document_wasm(
        &exported_modal_document(),
        &destination,
        true,
    )
    .expect("write WASM fixture");
}

#[test]
fn fifty_surfaces_and_ten_thousand_widgets_export_deterministically() {
    let started = std::time::Instant::now();
    let mut document = ProjectDocument::default();
    let mut surface_ids = vec![document.root_surface];
    for index in 1..50 {
        surface_ids.push(document.add_modal_surface(format!("Dialog {index:02}")));
    }

    let mut next_id = 1_u128;
    for surface_id in surface_ids {
        let surface = document.surface_mut(surface_id).expect("surface");
        for index in 0..200 {
            surface.tree.add(WidgetInstance {
                id: Uuid::from_u128(next_id),
                kind: WidgetKind::Label,
                props: WidgetProps {
                    label: format!("Widget {index:03}"),
                    ..Default::default()
                },
                ..Default::default()
            });
            next_id += 1;
        }
    }

    let first_json = rohkai::project::io::serialize(&document).expect("serialize");
    let second_json = rohkai::project::io::serialize(&document).expect("serialize again");
    assert_eq!(first_json, second_json);

    let first_export = rohkai::codegen::export::project_files_document(&document);
    let second_export = rohkai::codegen::export::project_files_document(&document);
    assert_eq!(first_export, second_export);
    assert_eq!(
        document
            .surfaces
            .iter()
            .map(|surface| surface.tree.widgets.len())
            .sum::<usize>(),
        10_000
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "10,000-widget project hardening fixture exceeded 15 seconds: {:?}",
        started.elapsed()
    );
}
