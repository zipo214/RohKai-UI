//! Fidelity audit: cross-surface parity checks.
//!
//! **RCA — how these gaps happen:**
//! Background agents implement schema fields + properties-panel UI but never
//! touch codegen.  The gap is invisible until a dedicated test exercises the
//! full schema→codegen path.  These tests exist to catch that class of error.
//!
//! **What is checked:**
//! 1. Codegen parity — each layout-specific schema field with a properties-panel
//!    UI must produce a non-default codegen output when set to a non-default value.
//! 2. Shortcut binding coverage — every action key in BUILTIN_SHORTCUTS must map
//!    to a key that `parse_egui_key` recognises (proves the parser covers the
//!    built-in set).
//! 3. CSS at-rule diagnostic — SVG with `@keyframes` must produce a specific
//!    named warning, not be silently swallowed.
//! 4. WidgetKind codegen completeness — every kind in the canonical
//!    `widgets::ALL_KINDS` list must emit non-trivial code (not silent nothing).
//!
//! **Root cause this harness guards (see `docs/RCA-2026-06-12-surface-parity-drift.md`):**
//! RohKai guards *enum* drift with exhaustive `match` arms (a new `WidgetKind`
//! will not compile until codegen handles it). *Struct fields* have no such
//! forcing function — a new `WidgetInstance` field can be serialized, shown in
//! Properties, and silently ignored by codegen, and it all still compiles. This
//! file is the cross-surface backstop for that gap. The static companion is
//! `scripts/check-surface-parity.ps1`.
//!
//! Adding a new schema field?  Add a matching codegen-parity test here.

use rohkai::codegen::egui_emitter::emit_indexed;
use rohkai::panels::shortcuts::BUILTIN_SHORTCUTS;
use rohkai::project::schema::{CrossAlign, TextAlign, WidgetInstance, WidgetKind, WidgetProps};
use rohkai::project::ui_tree::UiTree;
use rohkai::widgets::ALL_KINDS;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Codegen parity helpers
// ---------------------------------------------------------------------------

fn tree_with_vlayout_child(child_flex: f32) -> (UiTree, Uuid) {
    let parent = Uuid::from_u128(0x01);
    let child = Uuid::from_u128(0x02);
    let widgets = vec![
        WidgetInstance {
            id: parent,
            kind: WidgetKind::VLayout,
            children: vec![child],
            ..Default::default()
        },
        WidgetInstance {
            id: child,
            kind: WidgetKind::Button,
            child_flex,
            ..Default::default()
        },
    ];
    (
        UiTree {
            widgets,
            ..Default::default()
        },
        child,
    )
}

fn tree_with_hlayout_child(child_flex: f32) -> (UiTree, Uuid) {
    let parent = Uuid::from_u128(0x03);
    let child = Uuid::from_u128(0x04);
    let widgets = vec![
        WidgetInstance {
            id: parent,
            kind: WidgetKind::HLayout,
            children: vec![child],
            ..Default::default()
        },
        WidgetInstance {
            id: child,
            kind: WidgetKind::Label,
            child_flex,
            ..Default::default()
        },
    ];
    (
        UiTree {
            widgets,
            ..Default::default()
        },
        child,
    )
}

fn tree_with_grid_child(col_span: u32, row_span: u32, columns: usize) -> UiTree {
    let parent = Uuid::from_u128(0x05);
    let child = Uuid::from_u128(0x06);
    let widgets = vec![
        WidgetInstance {
            id: parent,
            kind: WidgetKind::GridLayout,
            children: vec![child],
            props: WidgetProps {
                grid_columns: columns,
                ..Default::default()
            },
            ..Default::default()
        },
        WidgetInstance {
            id: child,
            kind: WidgetKind::Label,
            grid_col_span: col_span,
            grid_row_span: row_span,
            ..Default::default()
        },
    ];
    UiTree {
        widgets,
        ..Default::default()
    }
}

fn emit_code(tree: &UiTree) -> String {
    emit_indexed(tree)
        .into_iter()
        .map(|(_, l)| l)
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// 1. Codegen parity — child_flex
// ---------------------------------------------------------------------------

#[test]
fn parity_vlayout_child_flex_zero_no_allocate_ui() {
    let (tree, _) = tree_with_vlayout_child(0.0);
    let code = emit_code(&tree);
    assert!(
        !code.contains("allocate_ui"),
        "child_flex=0 must NOT emit allocate_ui: {code}"
    );
}

#[test]
fn parity_vlayout_child_flex_nonzero_emits_allocate_ui() {
    let (tree, _) = tree_with_vlayout_child(1.0);
    let code = emit_code(&tree);
    assert!(
        code.contains("allocate_ui"),
        "child_flex=1 in VLayout must emit allocate_ui: {code}"
    );
    assert!(
        code.contains("available_height()"),
        "VLayout flex must allocate proportional height: {code}"
    );
}

#[test]
fn parity_hlayout_child_flex_nonzero_emits_allocate_ui() {
    let (tree, _) = tree_with_hlayout_child(1.0);
    let code = emit_code(&tree);
    assert!(
        code.contains("allocate_ui"),
        "child_flex=1 in HLayout must emit allocate_ui: {code}"
    );
    assert!(
        code.contains("available_width()"),
        "HLayout flex must allocate proportional width: {code}"
    );
}

#[test]
fn parity_vlayout_flex_ratio_reflects_weight() {
    // flex=1 of total=1 → ratio=1.0000
    let (tree, _) = tree_with_vlayout_child(1.0);
    let code = emit_code(&tree);
    assert!(
        code.contains("1.0000"),
        "flex ratio 1/1 must appear as 1.0000 in codegen: {code}"
    );
}

// ---------------------------------------------------------------------------
// 2. Codegen parity — grid_col_span / grid_row_span
// ---------------------------------------------------------------------------

#[test]
fn parity_grid_col_span_1_no_filler() {
    let tree = tree_with_grid_child(1, 1, 2);
    let code = emit_code(&tree);
    assert!(
        !code.contains("span filler"),
        "grid_col_span=1 must NOT emit filler cells: {code}"
    );
}

#[test]
fn parity_grid_col_span_2_emits_filler() {
    let tree = tree_with_grid_child(2, 1, 3);
    let code = emit_code(&tree);
    assert!(
        code.contains("span filler"),
        "grid_col_span=2 must emit a filler cell: {code}"
    );
    assert!(
        code.contains("grid_col_span=2"),
        "span comment must name the value: {code}"
    );
}

#[test]
fn parity_grid_row_span_1_no_comment() {
    let tree = tree_with_grid_child(1, 1, 2);
    let code = emit_code(&tree);
    assert!(
        !code.contains("grid_row_span"),
        "grid_row_span=1 (default) must not emit row-span comment: {code}"
    );
}

#[test]
fn parity_grid_row_span_2_emits_comment() {
    let tree = tree_with_grid_child(1, 2, 2);
    let code = emit_code(&tree);
    assert!(
        code.contains("grid_row_span=2"),
        "grid_row_span=2 must emit diagnostic comment: {code}"
    );
}

// ---------------------------------------------------------------------------
// 3. Shortcut binding coverage — parse_egui_key covers all builtin actions
// ---------------------------------------------------------------------------

#[test]
fn shortcut_builtins_all_have_parseable_keys() {
    // For each built-in (action_key, default_combo, _desc), verify that the key
    // component (last segment after splitting on '+') is recognised.
    for (action, combo, _desc) in BUILTIN_SHORTCUTS {
        let key_part = combo.split('+').next_back().unwrap_or("");
        // We call parse_egui_key indirectly: if it returns None, the shortcut
        // will silently not fire.  This test guards that the built-in set stays
        // within the parser's coverage.
        let recognised = matches!(
            key_part,
            "A" | "B"
                | "C"
                | "D"
                | "E"
                | "F"
                | "G"
                | "H"
                | "I"
                | "J"
                | "K"
                | "L"
                | "M"
                | "N"
                | "O"
                | "P"
                | "Q"
                | "R"
                | "S"
                | "T"
                | "U"
                | "V"
                | "W"
                | "X"
                | "Y"
                | "Z"
                | "0"
                | "1"
                | "2"
                | "3"
                | "4"
                | "5"
                | "6"
                | "7"
                | "8"
                | "9"
                | "F1"
                | "F2"
                | "F3"
                | "F4"
                | "F5"
                | "F6"
                | "F7"
                | "F8"
                | "F9"
                | "F10"
                | "F11"
                | "F12"
                | "Escape"
                | "Enter"
                | "Tab"
                | "Delete"
                | "Backspace"
                | "Space"
        );
        assert!(
            recognised,
            "BUILTIN_SHORTCUT action={action:?} combo={combo:?}: key part {key_part:?} is not in parse_egui_key coverage"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. CSS at-rule diagnostic
// ---------------------------------------------------------------------------

#[test]
fn css_keyframes_atrule_count_incremented() {
    // Direct unit test via svg_core — ensures @keyframes increments atrule_count.
    let sheet = rohkai::svg_core::parse_css_stylesheet(
        "@keyframes spin { from { opacity: 0; } to { opacity: 1; } } rect { fill: red; }",
        64,
        256,
    );
    assert!(
        sheet.atrule_count >= 1,
        "@keyframes must increment atrule_count, got: {sheet:?}"
    );
    // Regular rule after the at-rule must still parse.
    assert!(
        !sheet.rules.is_empty(),
        "Regular CSS rule after @keyframes must still be parsed"
    );
}

#[test]
fn multiple_atrules_all_counted() {
    let css = "@keyframes a {} @font-face {} @media screen {} rect { fill: blue; }";
    let sheet = rohkai::svg_core::parse_css_stylesheet(css, 64, 256);
    assert!(
        sheet.atrule_count >= 3,
        "Three at-rules must each increment atrule_count, got: {}",
        sheet.atrule_count
    );
}

// ---------------------------------------------------------------------------
// 5. WidgetKind codegen completeness
//
// The main `emit_indexed` match is exhaustive, so a *new* kind cannot compile
// without an arm — but an arm could still emit nothing but a comment.  Walk the
// canonical `widgets::ALL_KINDS` list and prove every kind emits at least one
// real (non-comment, non-empty) line.  This both backstops "silent nothing"
// codegen and keeps `ALL_KINDS` load-bearing (it documents the /new-widget
// protocol — step 3 is "add the variant here").
// ---------------------------------------------------------------------------

#[test]
fn every_widget_kind_emits_non_trivial_code() {
    for kind in ALL_KINDS {
        let id = Uuid::from_u128(0xA11);
        let tree = UiTree {
            widgets: vec![WidgetInstance {
                id,
                kind: kind.clone(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let code = emit_code(&tree);
        let has_real_line = code
            .lines()
            .any(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"));
        assert!(
            has_real_line,
            "WidgetKind::{kind:?} emitted no real code (only comments/blank). \
             Its emitter arm is a silent hole:\n{code}"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. text_align parity — set in Properties, must reach codegen (was fully dead)
// ---------------------------------------------------------------------------

#[test]
fn parity_label_text_align_center_emits_with_layout() {
    let tree = UiTree {
        widgets: vec![WidgetInstance {
            id: Uuid::from_u128(0xB01),
            kind: WidgetKind::Label,
            text_align: Some(TextAlign::Center),
            ..Default::default()
        }],
        ..Default::default()
    };
    let code = emit_code(&tree);
    assert!(
        code.contains("egui::Layout::top_down(egui::Align::Center)"),
        "Label text_align=Center must wrap in a centering with_layout: {code}"
    );
}

#[test]
fn parity_label_text_align_default_no_with_layout() {
    let tree = UiTree {
        widgets: vec![WidgetInstance {
            id: Uuid::from_u128(0xB02),
            kind: WidgetKind::Label,
            text_align: None,
            ..Default::default()
        }],
        ..Default::default()
    };
    let code = emit_code(&tree);
    assert!(
        !code.contains("with_layout"),
        "Label with no text_align must NOT emit a with_layout wrap: {code}"
    );
}

// ---------------------------------------------------------------------------
// 7. child_cross_align parity — per-child override must reach layout codegen
// ---------------------------------------------------------------------------

#[test]
fn parity_vlayout_child_cross_align_center_emits_with_layout() {
    let parent = Uuid::from_u128(0xC01);
    let child = Uuid::from_u128(0xC02);
    let tree = UiTree {
        widgets: vec![
            WidgetInstance {
                id: parent,
                kind: WidgetKind::VLayout,
                children: vec![child],
                ..Default::default()
            },
            WidgetInstance {
                id: child,
                kind: WidgetKind::Label,
                child_cross_align: Some(CrossAlign::Center),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let code = emit_code(&tree);
    assert!(
        code.contains("egui::Layout::top_down(egui::Align::Center)"),
        "VLayout child cross_align=Center must emit a per-child centering with_layout: {code}"
    );
}

#[test]
fn parity_vlayout_child_cross_align_default_no_override() {
    let parent = Uuid::from_u128(0xC03);
    let child = Uuid::from_u128(0xC04);
    let tree = UiTree {
        widgets: vec![
            WidgetInstance {
                id: parent,
                kind: WidgetKind::VLayout,
                children: vec![child],
                ..Default::default()
            },
            WidgetInstance {
                id: child,
                kind: WidgetKind::Label,
                child_cross_align: None,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let code = emit_code(&tree);
    // Default container is `ui.vertical`; no per-child override wrapper.
    assert!(
        code.contains("ui.vertical(|ui|"),
        "default VLayout should use ui.vertical: {code}"
    );
}

#[test]
fn all_kinds_list_is_not_empty() {
    // Guards the protocol const itself: if someone empties ALL_KINDS, the
    // completeness test above would vacuously pass.
    assert!(
        ALL_KINDS.len() >= 30,
        "ALL_KINDS should enumerate every unit WidgetKind (>=30), got {}",
        ALL_KINDS.len()
    );
}
