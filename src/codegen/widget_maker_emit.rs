//! Code generation from Visual Widget Maker compositions.
//!
//! Converts `WidgetMakerDoc` primitives to egui Rust template strings.
//! Lives in `codegen/` to satisfy the invariant that Rust syntax strings
//! are produced only within this module tree.

use crate::canvas::widget_maker::{
    group_children, is_group_kind, MakerPrimKind, MakerPrimitive, WidgetMakerDoc,
};
use crate::codegen::rust::string_literal;

/// Generate the `live_preview` template string from the maker doc.
pub fn gen_live_preview(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["    {".to_owned()];
    lines.push("        let _painter = ui.painter();".to_owned());
    lines.push("        let _outer = ui.max_rect();".to_owned());
    lines.extend(emit_primitives(&doc.primitives, "        "));
    for slot in &doc.slots {
        let safe_name = slot.name.replace(|c: char| !c.is_alphanumeric(), "_");
        let slot_rect = format!(
            "egui::Rect::from_min_size(_outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
            slot.x, slot.y, slot.w, slot.h
        );
        lines.push(format!(
            "        let _slot_{safe_name} = {slot_rect}; // slot: {}",
            slot.name
        ));
        lines.push(format!(
            "        // TODO: drop widget into slot '{}'",
            slot.name
        ));
    }
    lines.push("    }".to_owned());
    lines.join("\n")
}

/// Generate the `export` template string from the maker doc.
pub fn gen_export_template(doc: &WidgetMakerDoc) -> String {
    let mut lines = vec!["                {".to_owned()];
    lines.push("                    let _painter = ui.painter();".to_owned());
    lines.push("                    let _outer = ui.max_rect();".to_owned());
    lines.extend(emit_primitives(&doc.primitives, "                    "));
    for slot in &doc.slots {
        let safe_name = slot.name.replace(|c: char| !c.is_alphanumeric(), "_");
        let slot_rect = format!(
            "egui::Rect::from_min_size(_outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
            slot.x, slot.y, slot.w, slot.h
        );
        lines.push(format!(
            "                    let _slot_{safe_name} = {slot_rect}; // slot: {}",
            slot.name
        ));
        lines.push(format!(
            "                    // TODO: drop widget into slot '{}'",
            slot.name
        ));
    }
    lines.push("                }".to_owned());
    lines.join("\n")
}

/// Two-pass primitive emission: groups wrap their children; claimed children are skipped.
fn emit_primitives(primitives: &[MakerPrimitive], indent: &str) -> Vec<String> {
    use std::collections::HashSet;

    // Collect all indices that are children of any group (claimed).
    let claimed: HashSet<usize> = (0..primitives.len())
        .filter(|&i| is_group_kind(&primitives[i].kind))
        .flat_map(|i| group_children(primitives, i))
        .collect();

    let mut lines = Vec::new();
    for (i, prim) in primitives.iter().enumerate() {
        if claimed.contains(&i) {
            // Already emitted inside a group wrapper — skip.
            continue;
        }
        if is_group_kind(&prim.kind) {
            let children_idx = group_children(primitives, i);
            let mut children: Vec<&MakerPrimitive> =
                children_idx.iter().map(|&j| &primitives[j]).collect();
            // Sort children by position for deterministic output.
            match prim.kind {
                MakerPrimKind::HGroup => {
                    children
                        .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
                }
                MakerPrimKind::VGroup => {
                    children
                        .sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
                }
                _ => {}
            }
            lines.extend(emit_group(prim, &children, indent));
        } else {
            lines.extend(prim_to_egui_lines(prim, indent));
        }
    }
    lines
}

/// Emit the egui closure for a group primitive and its children.
fn emit_group(prim: &MakerPrimitive, children: &[&MakerPrimitive], indent: &str) -> Vec<String> {
    let sub_rect = format!(
        "egui::Rect::from_min_size(\
            _outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), \
            egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
        prim.x, prim.y, prim.w, prim.h
    );
    let gap = prim.group_gap;
    let mut lines = Vec::new();
    let child_indent = format!("{indent}        ");

    match prim.kind {
        MakerPrimKind::HGroup => {
            lines.push(format!(
                "{indent}ui.allocate_ui_at_rect({sub_rect}, |ui| {{"
            ));
            lines.push(format!("{indent}    ui.set_min_size({sub_rect}.size());"));
            lines.push(format!(
                "{indent}    ui.spacing_mut().item_spacing.x = {gap:.1};"
            ));
            lines.push(format!("{indent}    ui.horizontal(|ui| {{"));
            for child in children {
                lines.extend(prim_to_egui_lines(child, &child_indent));
            }
            lines.push(format!("{indent}    }});"));
            lines.push(format!("{indent}}});"));
        }
        MakerPrimKind::VGroup => {
            lines.push(format!(
                "{indent}ui.allocate_ui_at_rect({sub_rect}, |ui| {{"
            ));
            lines.push(format!("{indent}    ui.set_min_size({sub_rect}.size());"));
            lines.push(format!(
                "{indent}    ui.spacing_mut().item_spacing.y = {gap:.1};"
            ));
            lines.push(format!("{indent}    ui.vertical(|ui| {{"));
            for child in children {
                lines.extend(prim_to_egui_lines(child, &child_indent));
            }
            lines.push(format!("{indent}    }});"));
            lines.push(format!("{indent}}});"));
        }
        MakerPrimKind::Grid => {
            let cols = prim.grid_cols.max(1) as usize;
            // Use a stable grid id derived from position.
            let grid_id = format!("\"wm_grid_{:.0}_{:.0}\"", prim.x * 1000.0, prim.y * 1000.0);
            lines.push(format!(
                "{indent}ui.allocate_ui_at_rect({sub_rect}, |ui| {{"
            ));
            lines.push(format!(
                "{indent}    egui::Grid::new({grid_id}).spacing([{gap:.1}, {gap:.1}]).show(ui, |ui| {{"
            ));
            for (i, child) in children.iter().enumerate() {
                lines.extend(prim_to_egui_lines(child, &child_indent));
                if (i + 1) % cols == 0 {
                    lines.push(format!("{child_indent}ui.end_row();"));
                }
            }
            lines.push(format!("{indent}    }});"));
            lines.push(format!("{indent}}});"));
        }
        MakerPrimKind::Stack => {
            lines.push(format!("{indent}// stack group"));
            for child in children {
                lines.extend(prim_to_egui_lines(child, indent));
            }
        }
        _ => {}
    }
    lines
}

fn prim_to_egui_lines(prim: &MakerPrimitive, indent: &str) -> Vec<String> {
    let [r, g, b] = prim.fill;
    let color = format!("egui::Color32::from_rgb({r}, {g}, {b})");
    let sub_rect = format!(
        "egui::Rect::from_min_size(\
            _outer.min + egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}), \
            egui::vec2(_outer.width() * {:.3}, _outer.height() * {:.3}))",
        prim.x, prim.y, prim.w, prim.h
    );
    match prim.kind {
        MakerPrimKind::Rect => vec![format!(
            "{indent}_painter.rect_filled({sub_rect}, {:.1}, {color});",
            prim.corner_radius
        )],
        MakerPrimKind::Outline => vec![format!(
            "{indent}_painter.rect_stroke({sub_rect}, {:.1}, egui::Stroke::new(1.0, {color}));",
            prim.corner_radius
        )],
        MakerPrimKind::Ellipse => vec![format!(
            "{indent}_painter.circle_filled({sub_rect}.center(), \
                {sub_rect}.width().min({sub_rect}.height()) * 0.5, {color});"
        )],
        MakerPrimKind::Text => {
            let text_lit = if prim.use_label_token {
                // Template token — double-braces are intentional in the output
                "\"{{label}}\"".to_owned()
            } else {
                // string_literal() uses Debug formatting: handles \, \n, \t, " correctly
                string_literal(&prim.text_content)
            };
            let tc = format!("egui::Color32::from_rgb({r}, {g}, {b})");
            vec![format!(
                "{indent}ui.put({sub_rect}, egui::Label::new(egui::RichText::new({text_lit}).size({:.1}).color({tc})).wrap(false));",
                prim.font_size
            )]
        }
        // Group kinds are handled by emit_group; this arm is unreachable in normal flow
        // but must be exhaustive.
        MakerPrimKind::HGroup
        | MakerPrimKind::VGroup
        | MakerPrimKind::Grid
        | MakerPrimKind::Stack => vec![],
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
}
