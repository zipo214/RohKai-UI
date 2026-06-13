//! Code generation from Visual Widget Maker compositions.
//!
//! Converts `WidgetMakerDoc` primitives to egui Rust template strings.
//! Lives in `codegen/` to satisfy the invariant that Rust syntax strings
//! are produced only within this module tree.

use crate::canvas::widget_maker::{
    MakerPrimKind, MakerPrimitive, PrimState, StyleTokens, WidgetMakerDoc, group_children,
    is_group_kind,
};
use crate::codegen::rust::string_literal;

/// Generate the `live_preview` template string from the maker doc.
pub fn gen_live_preview(doc: &WidgetMakerDoc) -> String {
    let indent = "        ";
    let mut lines = vec!["    {".to_owned()];
    lines.push(format!("{indent}let _painter = ui.painter();"));
    lines.push(format!("{indent}let _outer = ui.max_rect();"));
    // Emit style token const declarations
    let t = &doc.style_tokens;
    let [ar, ag, ab] = t.accent;
    let [br, bg, bb] = t.border;
    let [tr, tg, tb] = t.text_color;
    lines.push(format!(
        "{indent}let _tok_accent = egui::Color32::from_rgb({ar}, {ag}, {ab});"
    ));
    lines.push(format!(
        "{indent}let _tok_border = egui::Color32::from_rgb({br}, {bg}, {bb});"
    ));
    lines.push(format!(
        "{indent}let _tok_text   = egui::Color32::from_rgb({tr}, {tg}, {tb});"
    ));
    lines.extend(emit_primitives(&doc.primitives, &doc.style_tokens, indent));
    for slot in &doc.slots {
        let safe_name = slot.name.replace(|c: char| !c.is_alphanumeric(), "_");
        let slot_rect = format!(
            "egui::Rect::from_min_size(_outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
            slot.x, slot.y, slot.w, slot.h
        );
        lines.push(format!(
            "{indent}let _slot_{safe_name} = {slot_rect}; // slot: {}",
            slot.name
        ));
        lines.push(format!(
            "{indent}// TODO: drop widget into slot '{}'",
            slot.name
        ));
    }
    lines.push("    }".to_owned());
    lines.join("\n")
}

/// Generate the `export` template string from the maker doc.
pub fn gen_export_template(doc: &WidgetMakerDoc) -> String {
    let indent = "                    ";
    let mut lines = vec!["                {".to_owned()];
    lines.push(format!("{indent}let _painter = ui.painter();"));
    lines.push(format!("{indent}let _outer = ui.max_rect();"));
    // Emit style token const declarations
    let t = &doc.style_tokens;
    let [ar, ag, ab] = t.accent;
    let [br, bg, bb] = t.border;
    let [tr, tg, tb] = t.text_color;
    lines.push(format!(
        "{indent}let _tok_accent = egui::Color32::from_rgb({ar}, {ag}, {ab});"
    ));
    lines.push(format!(
        "{indent}let _tok_border = egui::Color32::from_rgb({br}, {bg}, {bb});"
    ));
    lines.push(format!(
        "{indent}let _tok_text   = egui::Color32::from_rgb({tr}, {tg}, {tb});"
    ));
    lines.extend(emit_primitives(&doc.primitives, &doc.style_tokens, indent));
    for slot in &doc.slots {
        let safe_name = slot.name.replace(|c: char| !c.is_alphanumeric(), "_");
        let slot_rect = format!(
            "egui::Rect::from_min_size(_outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
            slot.x, slot.y, slot.w, slot.h
        );
        lines.push(format!(
            "{indent}let _slot_{safe_name} = {slot_rect}; // slot: {}",
            slot.name
        ));
        lines.push(format!(
            "{indent}// TODO: drop widget into slot '{}'",
            slot.name
        ));
    }
    lines.push("                }".to_owned());
    lines.join("\n")
}

/// Two-pass primitive emission: groups wrap their children; claimed children are skipped at top level.
fn emit_primitives(
    primitives: &[MakerPrimitive],
    tokens: &StyleTokens,
    indent: &str,
) -> Vec<String> {
    use std::collections::HashSet;
    let claimed: HashSet<usize> = (0..primitives.len())
        .filter(|&i| is_group_kind(&primitives[i].kind))
        .flat_map(|i| group_children(primitives, i))
        .collect();
    let mut lines = Vec::new();
    for (i, prim) in primitives.iter().enumerate() {
        if claimed.contains(&i) {
            continue;
        }
        if is_group_kind(&prim.kind) {
            let children_idx = group_children(primitives, i);
            let mut children: Vec<(usize, &MakerPrimitive)> =
                children_idx.iter().map(|&j| (j, &primitives[j])).collect();
            match prim.kind {
                MakerPrimKind::HGroup => {
                    children.sort_by(|a, b| {
                        a.1.x
                            .partial_cmp(&b.1.x)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                MakerPrimKind::VGroup => {
                    children.sort_by(|a, b| {
                        a.1.y
                            .partial_cmp(&b.1.y)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                _ => {}
            }
            lines.extend(emit_group(prim, &children, tokens, indent));
        } else {
            lines.extend(prim_to_egui_lines(prim, tokens, i, indent));
        }
    }
    lines
}

/// Emit the egui closure for a group primitive and its sorted children.
fn emit_group(
    prim: &MakerPrimitive,
    children: &[(usize, &MakerPrimitive)],
    tokens: &StyleTokens,
    indent: &str,
) -> Vec<String> {
    let sub_rect = format!(
        "egui::Rect::from_min_size(\
            _outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), \
            egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
        prim.x, prim.y, prim.w, prim.h
    );
    let gap = prim.group_gap;
    let child_indent = format!("{indent}        ");
    let mut lines = Vec::new();
    match prim.kind {
        MakerPrimKind::HGroup => {
            lines.push(format!(
                "{indent}ui.scope_builder(egui::UiBuilder::new().max_rect({sub_rect}), |ui| {{"
            ));
            lines.push(format!("{indent}    ui.set_min_size({sub_rect}.size());"));
            lines.push(format!(
                "{indent}    ui.spacing_mut().item_spacing.x = {gap:.1};"
            ));
            lines.push(format!("{indent}    ui.horizontal(|ui| {{"));
            for (idx, child) in children {
                lines.extend(prim_to_egui_lines(child, tokens, *idx, &child_indent));
            }
            lines.push(format!("{indent}    }});"));
            lines.push(format!("{indent}}});"));
        }
        MakerPrimKind::VGroup => {
            lines.push(format!(
                "{indent}ui.scope_builder(egui::UiBuilder::new().max_rect({sub_rect}), |ui| {{"
            ));
            lines.push(format!("{indent}    ui.set_min_size({sub_rect}.size());"));
            lines.push(format!(
                "{indent}    ui.spacing_mut().item_spacing.y = {gap:.1};"
            ));
            lines.push(format!("{indent}    ui.vertical(|ui| {{"));
            for (idx, child) in children {
                lines.extend(prim_to_egui_lines(child, tokens, *idx, &child_indent));
            }
            lines.push(format!("{indent}    }});"));
            lines.push(format!("{indent}}});"));
        }
        MakerPrimKind::Grid => {
            let cols = prim.grid_cols.max(1) as usize;
            let grid_id = format!("\"wm_grid_{:.0}_{:.0}\"", prim.x * 1000.0, prim.y * 1000.0);
            lines.push(format!(
                "{indent}ui.scope_builder(egui::UiBuilder::new().max_rect({sub_rect}), |ui| {{"
            ));
            lines.push(format!("{indent}    egui::Grid::new({grid_id}).spacing([{gap:.1}, {gap:.1}]).show(ui, |ui| {{"));
            for (i, (idx, child)) in children.iter().enumerate() {
                lines.extend(prim_to_egui_lines(child, tokens, *idx, &child_indent));
                if (i + 1) % cols == 0 {
                    lines.push(format!("{child_indent}ui.end_row();"));
                }
            }
            lines.push(format!("{indent}    }});"));
            lines.push(format!("{indent}}});"));
        }
        MakerPrimKind::Stack => {
            lines.push(format!("{indent}// stack group"));
            for (idx, child) in children {
                lines.extend(prim_to_egui_lines(child, tokens, *idx, indent));
            }
        }
        _ => {}
    }
    lines
}

fn prim_to_egui_lines(
    prim: &MakerPrimitive,
    _tokens: &StyleTokens,
    idx: usize,
    indent: &str,
) -> Vec<String> {
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
            "{indent}let {resp_var} = ui.allocate_rect({sub_rect}, egui::Sense::click_and_drag());"
        ));
    }

    match prim.kind {
        MakerPrimKind::Rect => {
            lines.push(format!(
                "{indent}_painter.rect_filled({sub_rect}, {:.1}, {color});",
                prim.corner_radius
            ));
            if needs_response {
                let cr = prim.corner_radius;
                if let Some(ov) = prim.variants.get(PrimState::Hover)
                    && let Some([hr, hg, hb]) = ov.fill
                {
                    lines.push(format!("{indent}if {resp_var}.hovered() {{ _painter.rect_filled({sub_rect}, {cr:.1}, egui::Color32::from_rgb({hr}, {hg}, {hb})); }}"));
                }
                if let Some(ov) = prim.variants.get(PrimState::Pressed)
                    && let Some([pr, pg, pb]) = ov.fill
                {
                    lines.push(format!("{indent}if {resp_var}.is_pointer_button_down_on() {{ _painter.rect_filled({sub_rect}, {cr:.1}, egui::Color32::from_rgb({pr}, {pg}, {pb})); }}"));
                }
            }
            lines
        }
        MakerPrimKind::Outline => {
            lines.push(format!(
                "{indent}_painter.rect_stroke({sub_rect}, {:.1}, egui::Stroke::new(1.0, {color}), egui::StrokeKind::Inside);",
                prim.corner_radius
            ));
            lines
        }
        MakerPrimKind::Ellipse => {
            lines.push(format!(
                "{indent}_painter.circle_filled({sub_rect}.center(), \
                {sub_rect}.width().min({sub_rect}.height()) * 0.5, {color});"
            ));
            lines
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
                "{indent}ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc})).wrap(false));",
                prim.font_size
            ));
            lines
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
                "{indent}let {varname} = ui.allocate_rect({sub_rect}, egui::Sense {{ click: {click}, drag: {drag}, focusable: {focus} }}); // hit region",
            ));
            if prim.sense_click {
                lines.push(format!(
                    "{indent}if {varname}.clicked() {{ /* on_{} */ }}",
                    if prim.prim_name.is_empty() {
                        idx.to_string()
                    } else {
                        prim.prim_name.clone()
                    }
                ));
            }
            if prim.sense_hover {
                lines.push(format!(
                    "{indent}let _{varname}_hovered = {varname}.hovered();"
                ));
            }
            lines
        }
        // Group kinds are handled by emit_group; exhaustive match arm.
        MakerPrimKind::HGroup
        | MakerPrimKind::VGroup
        | MakerPrimKind::Grid
        | MakerPrimKind::Stack => lines,
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
        assert!(
            !out.contains("allocate_rect"),
            "no variants → no allocate_rect: {out}"
        );
    }

    #[test]
    fn state_variant_hover_emits_hovered_check() {
        use crate::canvas::widget_maker::{PrimStyleOverride, PrimVariants};
        let mut prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        prim.variants = PrimVariants {
            hover: Some(PrimStyleOverride {
                fill: Some([255, 0, 0]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("hovered()"),
            "hover variant must emit hovered(): {out}"
        );
    }

    #[test]
    fn state_variant_pressed_emits_pointer_down() {
        use crate::canvas::widget_maker::{PrimStyleOverride, PrimVariants};
        let mut prim = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            ..Default::default()
        };
        prim.variants = PrimVariants {
            pressed: Some(PrimStyleOverride {
                fill: Some([0, 0, 255]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = gen_live_preview(&doc_with(prim));
        assert!(
            out.contains("is_pointer_button_down_on()"),
            "pressed variant must emit pointer_down: {out}"
        );
    }

    // --- Group / slot codegen tests ---

    #[test]
    fn hgroup_emits_horizontal_closure() {
        use crate::canvas::widget_maker::SlotDef;
        let _ = SlotDef::default(); // ensure type is accessible
        let group = MakerPrimitive {
            kind: MakerPrimKind::HGroup,
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            group_gap: 4.0,
            ..Default::default()
        };
        let child = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            x: 0.1,
            y: 0.1,
            w: 0.4,
            h: 0.8,
            fill: [200, 100, 50],
            ..Default::default()
        };
        let mut doc = WidgetMakerDoc::new_with_defaults();
        doc.primitives = vec![group, child];
        let code = gen_live_preview(&doc);
        assert!(
            code.contains("ui.horizontal"),
            "HGroup must emit ui.horizontal: {code}"
        );
    }

    #[test]
    fn group_child_not_emitted_twice() {
        let group = MakerPrimitive {
            kind: MakerPrimKind::HGroup,
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            group_gap: 0.0,
            ..Default::default()
        };
        let child = MakerPrimitive {
            kind: MakerPrimKind::Rect,
            x: 0.1,
            y: 0.1,
            w: 0.4,
            h: 0.8,
            fill: [200, 100, 50],
            ..Default::default()
        };
        let mut doc = WidgetMakerDoc::new_with_defaults();
        doc.primitives = vec![group, child];
        let code = gen_live_preview(&doc);
        let count = code.matches("rect_filled").count();
        assert_eq!(
            count, 1,
            "child rect_filled should appear exactly once: {code}"
        );
    }

    #[test]
    fn slot_emits_slot_comment() {
        use crate::canvas::widget_maker::SlotDef;
        let mut doc = WidgetMakerDoc::new_with_defaults();
        doc.primitives = vec![];
        doc.slots.push(SlotDef {
            name: "content".to_owned(),
            x: 0.1,
            y: 0.1,
            w: 0.8,
            h: 0.8,
        });
        let code = gen_live_preview(&doc);
        assert!(
            code.contains("slot: content"),
            "slot comment expected: {code}"
        );
    }
}
