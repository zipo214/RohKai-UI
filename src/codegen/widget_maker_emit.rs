//! Code generation from Visual Widget Maker compositions.
//!
//! Converts `WidgetMakerDoc` primitives to egui Rust template strings.
//! Lives in `codegen/` to satisfy the invariant that Rust syntax strings
//! are produced only within this module tree.

use crate::canvas::widget_maker::{
    MakerPrimKind, MakerPrimitive, PrimState, StyleTokens, WidgetMakerDoc,
};
use crate::codegen::rust::string_literal;

/// Generate the `live_preview` template string from the maker doc.
pub fn gen_live_preview(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["    {".to_owned()];
    lines.push("        let _painter = ui.painter();".to_owned());
    lines.push("        let _outer = ui.max_rect();".to_owned());
    // Emit style token const declarations
    let t = &doc.style_tokens;
    let [ar, ag, ab] = t.accent;
    let [br, bg, bb] = t.border;
    let [tr, tg, tb] = t.text_color;
    lines.push(format!(
        "        let _tok_accent = egui::Color32::from_rgb({ar}, {ag}, {ab});"
    ));
    lines.push(format!(
        "        let _tok_border = egui::Color32::from_rgb({br}, {bg}, {bb});"
    ));
    lines.push(format!(
        "        let _tok_text   = egui::Color32::from_rgb({tr}, {tg}, {tb});"
    ));
    for (idx, prim) in doc.primitives.iter().enumerate() {
        lines.extend(prim_to_egui_lines(prim, &doc.style_tokens, idx));
    }
    lines.push("    }".to_owned());
    lines.join("\n")
}

/// Generate the `export` template string from the maker doc.
pub fn gen_export_template(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["                {".to_owned()];
    lines.push("                    let _painter = ui.painter();".to_owned());
    lines.push("                    let _outer = ui.max_rect();".to_owned());
    // Emit style token const declarations
    let t = &doc.style_tokens;
    let [ar, ag, ab] = t.accent;
    let [br, bg, bb] = t.border;
    let [tr, tg, tb] = t.text_color;
    lines.push(format!(
        "        let _tok_accent = egui::Color32::from_rgb({ar}, {ag}, {ab});"
    ));
    lines.push(format!(
        "        let _tok_border = egui::Color32::from_rgb({br}, {bg}, {bb});"
    ));
    lines.push(format!(
        "        let _tok_text   = egui::Color32::from_rgb({tr}, {tg}, {tb});"
    ));
    for (idx, prim) in doc.primitives.iter().enumerate() {
        lines.extend(prim_to_egui_lines(prim, &doc.style_tokens, idx));
    }
    lines.push("                }".to_owned());
    lines.join("\n")
}

fn prim_to_egui_lines(prim: &MakerPrimitive, _tokens: &StyleTokens, idx: usize) -> Vec<String> {
    let [r, g, b] = prim.fill;
    // Fill color: either token reference or literal RGB
    let color = if prim.use_token_fill {
        "_tok_accent".to_owned()
    } else {
        format!("egui::Color32::from_rgb({r}, {g}, {b})")
    };
    let sub_rect = format!(
        "egui::Rect::from_min_size(\
            _outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), \
            egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
        prim.x, prim.y, prim.w, prim.h
    );
    // If this prim has state variants, prepend an interactive response allocation.
    let needs_response = prim.variants.has_any();
    let resp_var = format!("_sr_{idx}");
    let mut lines: Vec<String> = Vec::new();
    if needs_response {
        lines.push(format!(
            "        let {resp_var} = ui.allocate_rect({sub_rect}, egui::Sense::click_and_drag());"
        ));
    }

    match prim.kind {
        MakerPrimKind::Rect => {
            lines.push(format!(
                "        _painter.rect_filled({sub_rect}, {:.1}, {color});",
                prim.corner_radius
            ));
            if needs_response {
                let cr = prim.corner_radius;
                if let Some(ov) = prim.variants.get(PrimState::Hover) {
                    if let Some([hr, hg, hb]) = ov.fill {
                        lines.push(format!("        if {resp_var}.hovered() {{ _painter.rect_filled({sub_rect}, {cr:.1}, egui::Color32::from_rgb({hr}, {hg}, {hb})); }}"));
                    }
                }
                if let Some(ov) = prim.variants.get(PrimState::Pressed) {
                    if let Some([pr, pg, pb]) = ov.fill {
                        lines.push(format!("        if {resp_var}.is_pointer_button_down_on() {{ _painter.rect_filled({sub_rect}, {cr:.1}, egui::Color32::from_rgb({pr}, {pg}, {pb})); }}"));
                    }
                }
            }
            return lines;
        }
        MakerPrimKind::Outline => {
            lines.push(format!(
                "        _painter.rect_stroke({sub_rect}, {:.1}, egui::Stroke::new(1.0, {color}));",
                prim.corner_radius
            ));
            return lines;
        }
        MakerPrimKind::Ellipse => {
            lines.push(format!(
                "        _painter.circle_filled({sub_rect}.center(), \
                {sub_rect}.width().min({sub_rect}.height()) * 0.5, {color});"
            ));
            return lines;
        }
        MakerPrimKind::Text => {
            let text_lit = if prim.use_label_token {
                "\"{{label}}\"".to_owned()
            } else {
                string_literal(&prim.text_content)
            };
            let tc = if prim.use_token_text_color {
                "_tok_text".to_owned()
            } else {
                format!("egui::Color32::from_rgb({r}, {g}, {b})")
            };
            lines.push(format!(
                "        ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc})).wrap(false));",
                prim.font_size
            ));
            return lines;
        }
        MakerPrimKind::HitRegion => {
            let varname = if prim.prim_name.is_empty() {
                format!("_hr_{idx}")
            } else {
                format!("_hr_{}", prim.prim_name)
            };
            let click = prim.sense_click;
            let drag = prim.sense_drag;
            let focus = false;
            lines.push(format!(
                "        let {varname} = ui.allocate_rect({sub_rect}, egui::Sense {{ click: {click}, drag: {drag}, focusable: {focus} }}); // hit region",
            ));
            if prim.sense_click {
                lines.push(format!(
                    "        if {varname}.clicked() {{ /* on_{} */ }}",
                    if prim.prim_name.is_empty() { idx.to_string() } else { prim.prim_name.clone() }
                ));
            }
            if prim.sense_hover {
                lines.push(format!("        let _{varname}_hovered = {varname}.hovered();"));
            }
            lines
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::widget_maker::{MakerPrimKind, MakerPrimitive, WidgetMakerDoc};

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

    // --- Style Token tests ---

    #[test]
    fn token_declarations_appear_in_live_preview() {
        let doc = WidgetMakerDoc::new_with_defaults();
        let out = gen_live_preview(&doc);
        assert!(
            out.contains("_tok_accent"),
            "must declare _tok_accent: {out}"
        );
        assert!(
            out.contains("_tok_border"),
            "must declare _tok_border: {out}"
        );
        assert!(out.contains("_tok_text"), "must declare _tok_text: {out}");
    }

    #[test]
    fn token_fill_uses_tok_accent_variable() {
        let prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            use_token_fill: true,
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            fill: [100, 100, 100],
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("_tok_accent"),
            "token fill must reference _tok_accent: {out}"
        );
        // Must not use a literal RGB for the fill when token is active
        assert!(
            !out.contains("from_rgb(100, 100, 100)"),
            "literal fill RGB must not appear when use_token_fill is true: {out}"
        );
    }

    // --- Hit Region tests ---

    #[test]
    fn hit_region_emits_allocate_rect() {
        let prim = MakerPrimitive {
            kind: MakerPrimKind::HitRegion,
            sense_click: true,
            x: 0.1,
            y: 0.1,
            w: 0.8,
            h: 0.8,
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("allocate_rect"),
            "hit region must emit allocate_rect: {out}"
        );
        assert!(
            out.contains("clicked()"),
            "sense_click=true must emit clicked(): {out}"
        );
    }

    #[test]
    fn named_hit_region_uses_prim_name() {
        let prim = MakerPrimitive {
            kind: MakerPrimKind::HitRegion,
            prim_name: "close".to_owned(),
            sense_click: false,
            x: 0.0,
            y: 0.0,
            w: 0.2,
            h: 0.2,
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("_hr_close"),
            "named hit region must use prim_name: {out}"
        );
    }

    #[test]
    fn prim_without_variants_has_no_allocate_rect() {
        let prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(!out.contains("allocate_rect"), "no variants → no allocate_rect: {out}");
    }

    #[test]
    fn state_variant_hover_emits_hovered_check() {
        use crate::canvas::widget_maker::{PrimStyleOverride, PrimVariants};
        let mut prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        prim.variants = PrimVariants {
            hover: Some(PrimStyleOverride { fill: Some([255, 0, 0]), ..Default::default() }),
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(out.contains("hovered()"), "hover variant must emit hovered(): {out}");
    }

    #[test]
    fn state_variant_pressed_emits_pointer_down() {
        use crate::canvas::widget_maker::{PrimStyleOverride, PrimVariants};
        let mut prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        prim.variants = PrimVariants {
            pressed: Some(PrimStyleOverride { fill: Some([0, 0, 255]), ..Default::default() }),
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(out.contains("is_pointer_button_down_on()"), "pressed variant must emit pointer_down: {out}");
    }
}
