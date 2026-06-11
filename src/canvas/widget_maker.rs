//! Visual Widget Maker — data model and codegen for the primitive mini-canvas.
//!
//! `WidgetMakerDoc` is the document for one widget under construction.
//! `MakerPrimitive` is a normalised [0, 1] shape or text element.
//! `doc_to_descriptor` converts a finished document to a `WidgetDescriptor`.
//! `sanitize_widget_id_to_filename` converts a widget ID to a safe filename stem.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

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
}
