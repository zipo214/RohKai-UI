//! Visual Widget Maker — data model and codegen for the primitive mini-canvas.
//!
//! `WidgetMakerDoc` is the document for one widget under construction.
//! `MakerPrimitive` is a normalised [0, 1] shape or text element.
//! `doc_to_descriptor` converts a finished document to a `WidgetDescriptor`.
//! `doc_from_descriptor` reconstructs a `WidgetMakerDoc` from a VWM-generated descriptor.
//! `sanitize_widget_id_to_filename` converts a widget ID to a safe filename stem.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Anchor point for a primitive inside the widget bounding box.
///
/// Controls which corner/edge the primitive is pinned to when the widget is
/// resized. Serialised with `rename_all = "snake_case"` for forward-compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrimAnchor {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl PrimAnchor {
    /// Human-readable label for UI display.
    pub fn label(self) -> &'static str {
        match self {
            PrimAnchor::TopLeft => "Top-Left",
            PrimAnchor::TopRight => "Top-Right",
            PrimAnchor::BottomLeft => "Bottom-Left",
            PrimAnchor::BottomRight => "Bottom-Right",
            PrimAnchor::Center => "Center",
        }
    }

    /// All variants in display order.
    pub const ALL: &'static [PrimAnchor] = &[
        PrimAnchor::TopLeft,
        PrimAnchor::TopRight,
        PrimAnchor::BottomLeft,
        PrimAnchor::BottomRight,
        PrimAnchor::Center,
    ];
}

/// A visual primitive in the Widget Maker mini-canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakerPrimitive {
    pub kind: MakerPrimKind,
    /// Normalised position/size in [0, 1] relative to the widget bounding box.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Fill colour RGB.
    pub fill: [u8; 3],
    /// Corner radius (for Rect/RoundedRect kinds).
    pub corner_radius: f32,
    /// Static text content (Text kind only; ignored for shape kinds).
    pub text_content: String,
    /// Font size for text (logical pixels).
    pub font_size: f32,
    /// If true, substitute `{{label}}` instead of `text_content`.
    pub use_label_token: bool,
    /// Anchor point: which corner/edge this primitive is pinned to.
    #[serde(default)]
    pub anchor: PrimAnchor,
    /// Minimum normalised width (clamped during resize). Default 0.0.
    #[serde(default)]
    pub min_w: f32,
    /// Minimum normalised height (clamped during resize). Default 0.0.
    #[serde(default)]
    pub min_h: f32,
}

impl Default for MakerPrimitive {
    fn default() -> Self {
        Self {
            kind: MakerPrimKind::Rect,
            x: 0.1,
            y: 0.1,
            w: 0.8,
            h: 0.8,
            fill: [100, 120, 200],
            corner_radius: 4.0,
            text_content: "Label".to_owned(),
            font_size: 14.0,
            use_label_token: false,
            anchor: PrimAnchor::TopLeft,
            min_w: 0.0,
            min_h: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MakerPrimKind {
    /// Filled rectangle (optionally rounded).
    Rect,
    /// Outlined rectangle (stroke only).
    Outline,
    /// Filled ellipse/circle.
    Ellipse,
    /// Text label.
    Text,
}

/// Complete visual composition document for the Widget Maker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WidgetMakerDoc {
    pub primitives: Vec<MakerPrimitive>,
    /// Index of the selected primitive (None = nothing selected).
    #[serde(skip)]
    pub selected: Option<usize>,
    /// Corner being dragged for resize (0=TL,1=TR,2=BL,3=BR); not persisted.
    #[serde(skip)]
    pub resize_corner: Option<u8>,
    /// Name for the generated WidgetDescriptor.
    pub widget_name: String,
    /// ID prefix (e.g. "mylib.button").
    pub widget_id: String,
    /// Category in the palette.
    pub category: String,
    /// Default canvas size [w, h].
    pub default_size: [f32; 2],
    /// Accent colour RGB.
    pub accent_color: [u8; 3],
}

impl WidgetMakerDoc {
    pub fn new_with_defaults() -> Self {
        Self {
            resize_corner: None,
            primitives: vec![
                MakerPrimitive {
                    kind: MakerPrimKind::Rect,
                    x: 0.0,
                    y: 0.0,
                    w: 1.0,
                    h: 1.0,
                    fill: [60, 80, 160],
                    corner_radius: 4.0,
                    ..Default::default()
                },
                MakerPrimitive {
                    kind: MakerPrimKind::Text,
                    x: 0.05,
                    y: 0.25,
                    w: 0.9,
                    h: 0.5,
                    fill: [240, 240, 240],
                    use_label_token: true,
                    font_size: 14.0,
                    ..Default::default()
                },
            ],
            selected: None,
            widget_name: "My Widget".to_owned(),
            widget_id: "custom.my_widget".to_owned(),
            category: "Custom".to_owned(),
            default_size: [120.0, 40.0],
            accent_color: [60, 80, 160],
        }
    }
}

// ---------------------------------------------------------------------------
// Code generation from the composition
// ---------------------------------------------------------------------------

/// Generate the `live_preview` template string from the maker doc.
pub fn gen_live_preview(doc: &WidgetMakerDoc) -> String {
    crate::codegen::widget_maker_emit::gen_live_preview(doc)
}

/// Generate the `export` template string from the maker doc.
pub fn gen_export_template(doc: &WidgetMakerDoc) -> String {
    crate::codegen::widget_maker_emit::gen_export_template(doc)
}

// ---------------------------------------------------------------------------
// Convert doc to WidgetDescriptor
// ---------------------------------------------------------------------------

pub fn doc_to_descriptor(
    doc: &WidgetMakerDoc,
) -> crate::codegen::widget_descriptor::WidgetDescriptor {
    use crate::codegen::widget_descriptor::{
        CanvasPreviewMode, DescriptorCanvasPreview, DescriptorCodegen, WidgetDescriptor,
    };
    WidgetDescriptor {
        schema_version: 1,
        id: doc.widget_id.clone(),
        name: doc.widget_name.clone(),
        category: doc.category.clone(),
        default_size: doc.default_size,
        accent_color: doc.accent_color,
        properties: vec![],
        state_fields: vec![],
        codegen: DescriptorCodegen {
            live_preview: gen_live_preview(doc),
            export: gen_export_template(doc),
            on_click_stub: String::new(),
        },
        canvas_preview: DescriptorCanvasPreview {
            mode: CanvasPreviewMode::LabelBox,
            label_template: "{{label}}".to_owned(),
        },
        cargo_deps: vec![],
        events: vec![],
    }
}

// ---------------------------------------------------------------------------
// Round-trip: WidgetDescriptor → WidgetMakerDoc (Item 4)
// ---------------------------------------------------------------------------

/// Attempt to reconstruct a `WidgetMakerDoc` from a `WidgetDescriptor`.
///
/// Succeeds only when the descriptor was generated by the Visual Widget Maker —
/// identified by `desc.codegen.live_preview` starting with `"    {"` (the
/// sentinel produced by [`gen_live_preview`]).
///
/// On success, restores all descriptor metadata (`widget_id`, `widget_name`,
/// `category`, `default_size`, `accent_color`) so the user can re-open and
/// re-edit the document. The primitive list is not reconstructed from the
/// template body (template body parsing is deferred); `primitives` is `vec![]`.
///
/// Returns `None` for descriptors not generated by the VWM.
// The function is a public API exercised by tests; the main binary does not
// call it yet, so suppress the dead_code lint rather than forcing a caller.
#[allow(dead_code)]
pub fn doc_from_descriptor(
    desc: &crate::codegen::widget_descriptor::WidgetDescriptor,
) -> Option<WidgetMakerDoc> {
    // The VWM marker: gen_live_preview always starts its output with "    {"
    if !desc.codegen.live_preview.starts_with("    {") {
        return None;
    }
    Some(WidgetMakerDoc {
        widget_id: desc.id.clone(),
        widget_name: desc.name.clone(),
        category: desc.category.clone(),
        default_size: desc.default_size,
        accent_color: desc.accent_color,
        primitives: vec![],
        selected: None,
        resize_corner: None,
    })
}

// ---------------------------------------------------------------------------
// Filename sanitizer (Invariant 7)
// ---------------------------------------------------------------------------

/// Convert a widget ID (e.g. `"mylib.button"`) to a safe filename stem.
///
/// Whitelists `[A-Za-z0-9_-]`. Every other character (including Windows-reserved
/// `<>:"\|?*\`, control bytes, and path separators) becomes `_`.
/// Falls back to `"widget"` when the result is empty or all underscores.
pub fn sanitize_widget_id_to_filename(id: &str) -> String {
    let s: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() || s.chars().all(|c| c == '_') {
        "widget".to_owned()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_live_preview_contains_painter() {
        let doc = WidgetMakerDoc::new_with_defaults();
        let code = gen_live_preview(&doc);
        assert!(code.contains("_painter"), "should use painter: {code}");
        assert!(code.contains("rect_filled"), "background rect: {code}");
    }

    #[test]
    fn text_prim_with_label_token_uses_template_syntax() {
        let doc = WidgetMakerDoc {
            primitives: vec![MakerPrimitive {
                kind: MakerPrimKind::Text,
                use_label_token: true,
                ..Default::default()
            }],
            ..WidgetMakerDoc::new_with_defaults()
        };
        let code = gen_live_preview(&doc);
        assert!(code.contains("{{label}}"), "label token expected: {code}");
    }

    #[test]
    fn doc_to_descriptor_round_trips_name_and_id() {
        let doc = WidgetMakerDoc {
            widget_name: "Test Widget".to_owned(),
            widget_id: "test.widget".to_owned(),
            ..WidgetMakerDoc::new_with_defaults()
        };
        let d = doc_to_descriptor(&doc);
        assert_eq!(d.name, "Test Widget");
        assert_eq!(d.id, "test.widget");
    }

    #[test]
    fn outline_prim_emits_rect_stroke() {
        let doc = WidgetMakerDoc {
            primitives: vec![MakerPrimitive {
                kind: MakerPrimKind::Outline,
                ..Default::default()
            }],
            ..WidgetMakerDoc::new_with_defaults()
        };
        let code = gen_live_preview(&doc);
        assert!(
            code.contains("rect_stroke"),
            "outline must use rect_stroke: {code}"
        );
    }

    // --- sanitize_widget_id_to_filename (Invariant 7) ---

    #[test]
    fn sanitize_dots_and_slashes_to_underscores() {
        assert_eq!(
            sanitize_widget_id_to_filename("mylib.button"),
            "mylib_button"
        );
        assert_eq!(
            sanitize_widget_id_to_filename("custom/widget"),
            "custom_widget"
        );
    }

    #[test]
    fn sanitize_strips_windows_reserved_chars() {
        // <, >, :, ", \, |, ?, * must all become _
        let id = r#"bad:<>:"\|?*"#;
        let result = sanitize_widget_id_to_filename(id);
        assert!(
            result
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "reserved chars survived: {result}"
        );
    }

    #[test]
    fn sanitize_control_bytes_to_fallback() {
        assert_eq!(sanitize_widget_id_to_filename("\x01\x02"), "widget");
    }

    #[test]
    fn sanitize_empty_gives_fallback() {
        assert_eq!(sanitize_widget_id_to_filename(""), "widget");
    }

    #[test]
    fn sanitize_all_underscores_gives_fallback() {
        assert_eq!(sanitize_widget_id_to_filename("..."), "widget");
    }

    #[test]
    fn sanitize_valid_id_preserved() {
        assert_eq!(
            sanitize_widget_id_to_filename("my_widget-v2"),
            "my_widget-v2"
        );
    }

    #[test]
    fn ellipse_prim_emits_circle_filled() {
        let doc = WidgetMakerDoc {
            primitives: vec![MakerPrimitive {
                kind: MakerPrimKind::Ellipse,
                ..Default::default()
            }],
            ..WidgetMakerDoc::new_with_defaults()
        };
        let code = gen_live_preview(&doc);
        assert!(
            code.contains("circle_filled"),
            "ellipse must use circle_filled: {code}"
        );
    }

    // --- Item 2: Z-order reorder ---

    #[test]
    fn swap_first_and_last_primitive_changes_order() {
        let mut doc = WidgetMakerDoc::new_with_defaults();
        // Add a third primitive so we have indices 0, 1, 2
        doc.primitives.push(MakerPrimitive {
            kind: MakerPrimKind::Ellipse,
            ..Default::default()
        });
        assert_eq!(doc.primitives.len(), 3);
        let first_kind = doc.primitives[0].kind.clone();
        let last_kind = doc.primitives[2].kind.clone();
        // Swap first and last
        doc.primitives.swap(0, 2);
        assert_eq!(
            doc.primitives[0].kind, last_kind,
            "first should now be old last"
        );
        assert_eq!(
            doc.primitives[2].kind, first_kind,
            "last should now be old first"
        );
    }

    // --- Item 3: min_w / min_h constraints ---

    #[test]
    fn resize_below_min_w_is_clamped() {
        let mut prim = MakerPrimitive {
            x: 0.2,
            y: 0.2,
            w: 0.5,
            h: 0.5,
            min_w: 0.3,
            min_h: 0.0,
            ..Default::default()
        };
        // Drag the bottom-right corner strongly to the left (dx = -0.4 normalised)
        // so that w would become 0.5 - 0.4 = 0.1, below min_w = 0.3
        crate::panels::widget_maker_panel::apply_corner_resize(&mut prim, 3, -0.4, 0.0);
        assert!(
            prim.w >= prim.min_w,
            "width must be >= min_w after resize: w={} min_w={}",
            prim.w,
            prim.min_w
        );
    }

    // --- Item 4: doc_from_descriptor round-trip ---

    #[test]
    fn doc_from_descriptor_round_trips_metadata() {
        let doc = WidgetMakerDoc {
            widget_name: "RoundTrip".to_owned(),
            widget_id: "rt.widget".to_owned(),
            category: "Test".to_owned(),
            default_size: [80.0, 30.0],
            accent_color: [1, 2, 3],
            ..WidgetMakerDoc::new_with_defaults()
        };
        let descriptor = doc_to_descriptor(&doc);
        let restored =
            doc_from_descriptor(&descriptor).expect("VWM-generated descriptor must round-trip");
        assert_eq!(restored.widget_name, doc.widget_name);
        assert_eq!(restored.widget_id, doc.widget_id);
        assert_eq!(restored.category, doc.category);
    }

    #[test]
    fn doc_from_descriptor_returns_none_for_non_vwm_descriptor() {
        use crate::codegen::widget_descriptor::{
            CanvasPreviewMode, DescriptorCanvasPreview, DescriptorCodegen, WidgetDescriptor,
        };
        let desc = WidgetDescriptor {
            schema_version: 1,
            id: "hand.written".to_owned(),
            name: "Hand Written".to_owned(),
            category: "Custom".to_owned(),
            default_size: [100.0, 40.0],
            accent_color: [0, 0, 0],
            properties: vec![],
            state_fields: vec![],
            codegen: DescriptorCodegen {
                // Does NOT start with "    {" — hand-written template
                live_preview: "ui.label(\"hello\");".to_owned(),
                export: String::new(),
                on_click_stub: String::new(),
            },
            canvas_preview: DescriptorCanvasPreview {
                mode: CanvasPreviewMode::LabelBox,
                label_template: String::new(),
            },
            cargo_deps: vec![],
            events: vec![],
        };
        assert!(
            doc_from_descriptor(&desc).is_none(),
            "non-VWM descriptor must return None"
        );
    }

    // --- PrimAnchor serde round-trip ---

    #[test]
    fn prim_anchor_serde_default_roundtrip() {
        // Ensure existing .rkwd files (missing anchor/min_w/min_h) still deserialise
        let json = r#"{"kind":"Rect","x":0.1,"y":0.1,"w":0.8,"h":0.8,"fill":[100,120,200],"corner_radius":4.0,"text_content":"Label","font_size":14.0,"use_label_token":false}"#;
        let prim: MakerPrimitive = serde_json::from_str(json).expect("must deserialise");
        assert_eq!(
            prim.anchor,
            PrimAnchor::TopLeft,
            "default anchor is TopLeft"
        );
        assert_eq!(prim.min_w, 0.0);
        assert_eq!(prim.min_h, 0.0);
    }
}
