//! Code generation from Visual Widget Maker compositions.
//!
//! Converts `WidgetMakerDoc` primitives to egui Rust template strings.
//! Lives in `codegen/` to satisfy the invariant that Rust syntax strings
//! are produced only within this module tree.

use crate::canvas::widget_maker::{MakerPrimKind, MakerPrimitive, WidgetMakerDoc};
use crate::codegen::rust::string_literal;

/// Generate the `live_preview` template string from the maker doc.
pub fn gen_live_preview(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["    {".to_owned()];
    lines.push("        let _painter = ui.painter();".to_owned());
    lines.push("        let _outer = ui.max_rect();".to_owned());
    for (idx, prim) in doc.primitives.iter().enumerate() {
        lines.extend(prim_to_egui_lines(prim, idx));
    }
    lines.push("    }".to_owned());
    lines.join("\n")
}

/// Generate the `export` template string from the maker doc.
pub fn gen_export_template(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["                {".to_owned()];
    lines.push("                    let _painter = ui.painter();".to_owned());
    lines.push("                    let _outer = ui.max_rect();".to_owned());
    for (idx, prim) in doc.primitives.iter().enumerate() {
        lines.extend(prim_to_egui_lines(prim, idx));
    }
    lines.push("                }".to_owned());
    lines.join("\n")
}

fn prim_to_egui_lines(prim: &MakerPrimitive, idx: usize) -> Vec<String> {
    let [r, g, b] = prim.fill;
    let normal_color = format!("egui::Color32::from_rgb({r}, {g}, {b})");
    let sub_rect = format!(
        "egui::Rect::from_min_size(\
            _outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), \
            egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
        prim.x, prim.y, prim.w, prim.h
    );

    // If this primitive has any state variants, allocate an interactive response
    // so we can react to hover/pressed in the generated code.
    let needs_response = prim.variants.has_any();
    let resp_var = format!("_sr_{idx}");

    let mut lines = Vec::new();

    if needs_response {
        lines.push(format!(
            "        let {resp_var} = ui.allocate_rect({sub_rect}, egui::Sense::click_and_drag());"
        ));
    }

    match prim.kind {
        MakerPrimKind::Rect => {
            lines.push(format!(
                "        _painter.rect_filled({sub_rect}, {:.1}, {normal_color});",
                prim.corner_radius
            ));
            emit_rect_variant_overrides(&mut lines, prim, &sub_rect, &resp_var, needs_response);
        }
        MakerPrimKind::Outline => {
            lines.push(format!(
                "        _painter.rect_stroke({sub_rect}, {:.1}, egui::Stroke::new(1.0, {normal_color}));",
                prim.corner_radius
            ));
            // Variant overrides for outline: repaint the stroke in the override colour.
            if needs_response {
                emit_outline_variant_overrides(&mut lines, prim, &sub_rect, &resp_var);
            }
        }
        MakerPrimKind::Ellipse => {
            lines.push(format!(
                "        _painter.circle_filled({sub_rect}.center(), \
                    {sub_rect}.width().min({sub_rect}.height()) * 0.5, {normal_color});"
            ));
            emit_ellipse_variant_overrides(&mut lines, prim, &sub_rect, &resp_var, needs_response);
        }
        MakerPrimKind::Text => {
            let text_lit = if prim.use_label_token {
                // Template token — double-braces are intentional in the output
                "\"{{label}}\"".to_owned()
            } else {
                // string_literal() uses Debug formatting: handles \, \n, \t, " correctly
                string_literal(&prim.text_content)
            };
            // Text colour: check variant overrides at runtime for text_color.
            let tc_normal = format!("egui::Color32::from_rgb({r}, {g}, {b})");
            if needs_response {
                // Emit a runtime colour selection expression.
                emit_text_with_variants(
                    &mut lines, prim, &sub_rect, &resp_var, &text_lit, &tc_normal,
                );
            } else {
                lines.push(format!(
                    "        ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc_normal})).wrap(false));",
                    prim.font_size
                ));
            }
        }
    }

    lines
}

// ---------------------------------------------------------------------------
// Variant override emitters — all live in codegen/ per invariant
// ---------------------------------------------------------------------------

fn emit_rect_variant_overrides(
    lines: &mut Vec<String>,
    prim: &MakerPrimitive,
    sub_rect: &str,
    resp_var: &str,
    has_response: bool,
) {
    if !has_response {
        return;
    }
    let cr = prim.corner_radius;
    // Hover
    if let Some(ov) = &prim.variants.hover {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        if {resp_var}.hovered() {{ _painter.rect_filled({sub_rect}, {cr:.1}, egui::Color32::from_rgb({r}, {g}, {b})); }}"
            ));
        }
    }
    // Pressed (takes priority over hover, rendered last)
    if let Some(ov) = &prim.variants.pressed {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        if {resp_var}.is_pointer_button_down_on() {{ _painter.rect_filled({sub_rect}, {cr:.1}, egui::Color32::from_rgb({r}, {g}, {b})); }}"
            ));
        }
    }
    // Disabled (no runtime egui sense for arbitrary rects; emit as comment)
    if let Some(ov) = &prim.variants.disabled {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        // disabled state: use Color32::from_rgb({r}, {g}, {b}) when widget is disabled"
            ));
        }
    }
    // Checked
    if let Some(ov) = &prim.variants.checked {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        // checked state: use Color32::from_rgb({r}, {g}, {b}) when widget is checked"
            ));
        }
    }
}

fn emit_outline_variant_overrides(
    lines: &mut Vec<String>,
    prim: &MakerPrimitive,
    sub_rect: &str,
    resp_var: &str,
) {
    let cr = prim.corner_radius;
    if let Some(ov) = &prim.variants.hover {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        if {resp_var}.hovered() {{ _painter.rect_stroke({sub_rect}, {cr:.1}, egui::Stroke::new(1.0, egui::Color32::from_rgb({r}, {g}, {b}))); }}"
            ));
        }
    }
    if let Some(ov) = &prim.variants.pressed {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        if {resp_var}.is_pointer_button_down_on() {{ _painter.rect_stroke({sub_rect}, {cr:.1}, egui::Stroke::new(1.0, egui::Color32::from_rgb({r}, {g}, {b}))); }}"
            ));
        }
    }
    if let Some(ov) = &prim.variants.disabled {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        // disabled state (outline): use Color32::from_rgb({r}, {g}, {b}) when widget is disabled"
            ));
        }
    }
    if let Some(ov) = &prim.variants.checked {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        // checked state (outline): use Color32::from_rgb({r}, {g}, {b}) when widget is checked"
            ));
        }
    }
}

fn emit_ellipse_variant_overrides(
    lines: &mut Vec<String>,
    prim: &MakerPrimitive,
    sub_rect: &str,
    resp_var: &str,
    has_response: bool,
) {
    if !has_response {
        return;
    }
    if let Some(ov) = &prim.variants.hover {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        if {resp_var}.hovered() {{ _painter.circle_filled({sub_rect}.center(), {sub_rect}.width().min({sub_rect}.height()) * 0.5, egui::Color32::from_rgb({r}, {g}, {b})); }}"
            ));
        }
    }
    if let Some(ov) = &prim.variants.pressed {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        if {resp_var}.is_pointer_button_down_on() {{ _painter.circle_filled({sub_rect}.center(), {sub_rect}.width().min({sub_rect}.height()) * 0.5, egui::Color32::from_rgb({r}, {g}, {b})); }}"
            ));
        }
    }
    if let Some(ov) = &prim.variants.disabled {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        // disabled state (ellipse): use Color32::from_rgb({r}, {g}, {b}) when widget is disabled"
            ));
        }
    }
    if let Some(ov) = &prim.variants.checked {
        if let Some([r, g, b]) = ov.fill {
            lines.push(format!(
                "        // checked state (ellipse): use Color32::from_rgb({r}, {g}, {b}) when widget is checked"
            ));
        }
    }
}

fn emit_text_with_variants(
    lines: &mut Vec<String>,
    prim: &MakerPrimitive,
    sub_rect: &str,
    resp_var: &str,
    text_lit: &str,
    tc_normal: &str,
) {
    // Build a runtime colour expression that falls back through states.
    // We emit the normal put first, then re-put on hover/pressed (painter-layer approach).
    lines.push(format!(
        "        ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc_normal})).wrap(false));",
        prim.font_size
    ));
    if let Some(ov) = &prim.variants.hover {
        let tc = ov
            .text_color
            .or(ov.fill)
            .map(|[r, g, b]| format!("egui::Color32::from_rgb({r}, {g}, {b})"))
            .unwrap_or_else(|| tc_normal.to_owned());
        lines.push(format!(
            "        if {resp_var}.hovered() {{ ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc})).wrap(false)); }}",
            prim.font_size
        ));
    }
    if let Some(ov) = &prim.variants.pressed {
        let tc = ov
            .text_color
            .or(ov.fill)
            .map(|[r, g, b]| format!("egui::Color32::from_rgb({r}, {g}, {b})"))
            .unwrap_or_else(|| tc_normal.to_owned());
        lines.push(format!(
            "        if {resp_var}.is_pointer_button_down_on() {{ ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc})).wrap(false)); }}",
            prim.font_size
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::widget_maker::{
        MakerPrimKind, MakerPrimitive, PrimStyleOverride, PrimVariants, WidgetMakerDoc,
    };

    fn text_prim(content: &str) -> MakerPrimitive {
        MakerPrimitive {
            kind: MakerPrimKind::Text,
            text_content: content.to_owned(),
            use_label_token: false,
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            fill: [255, 255, 255],
            font_size: 14.0,
            ..Default::default()
        }
    }

    fn doc_with(prim: MakerPrimitive) -> WidgetMakerDoc {
        let mut doc = WidgetMakerDoc::new_with_defaults();
        doc.primitives = vec![prim];
        doc
    }

    #[test]
    fn text_prim_escapes_backslash() {
        let out = gen_export_template(&doc_with(text_prim("path\\to\\file")));
        assert!(
            out.contains("path\\\\to\\\\file"),
            "backslashes must be escaped: {out}"
        );
        assert!(
            !out.contains("path\\to\\file") || out.contains("path\\\\"),
            "raw backslash in output"
        );
    }

    #[test]
    fn text_prim_escapes_double_quote() {
        let out = gen_export_template(&doc_with(text_prim("say \"hello\"")));
        assert!(
            out.contains("\\\"hello\\\""),
            "quotes must be escaped: {out}"
        );
    }

    #[test]
    fn label_token_uses_double_braces() {
        let mut prim = text_prim("ignored");
        prim.use_label_token = true;
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("{{label}}"),
            "label token must use double braces: {out}"
        );
    }

    // --- State variant tests ---

    #[test]
    fn prim_without_variants_has_no_allocate_rect() {
        let prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            !out.contains("allocate_rect"),
            "no variants => no allocate_rect: {out}"
        );
    }

    #[test]
    fn state_variant_hover_emits_hovered_check() {
        let mut prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            fill: [60, 80, 160],
            ..Default::default()
        };
        prim.variants.hover = Some(PrimStyleOverride {
            fill: Some([255, 0, 0]),
            ..Default::default()
        });
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("hovered()"),
            "hover variant must emit hovered() check: {out}"
        );
        assert!(
            out.contains("allocate_rect"),
            "hover variant requires allocate_rect: {out}"
        );
    }

    #[test]
    fn state_variant_pressed_emits_pointer_down() {
        let mut prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        prim.variants.pressed = Some(PrimStyleOverride {
            fill: Some([0, 200, 100]),
            ..Default::default()
        });
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("is_pointer_button_down_on()"),
            "pressed variant must emit is_pointer_button_down_on(): {out}"
        );
    }

    #[test]
    fn prim_variants_default_is_empty() {
        assert!(!PrimVariants::default().has_any());
    }
}
