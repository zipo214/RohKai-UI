---
name: project-model
description: Use when reading or writing UiTree, WidgetInstance, or project schema
types. This is the source-of-truth data model — all agents must speak this language.
---

# Project Data Model

## UiTree  (`src/project/ui_tree.rs`)
The root. Holds `Vec<WidgetInstance>` plus canvas dimensions.

## WidgetInstance  (`src/project/schema.rs`)
```
id:            Uuid
kind:          WidgetKind        // Button | Label | TextInput | Slider | Checkbox
rect:          Rect              // x, y, w, h on the canvas (f32)
props:         WidgetProps       // label: String, min: f32, max: f32
state_binding: Option<String>   // name of the AppState field this widget binds to
```

## WidgetKind → generated Rust
```
Button    → if ui.button("{label}").clicked() { }
Label     → ui.label(&self.{binding});
TextInput → ui.text_edit_singleline(&mut self.{binding});
Slider    → ui.add(egui::Slider::new(&mut self.{binding}, {min}..={max}).text("{label}"));
Checkbox  → ui.checkbox(&mut self.{binding}, "{label}");
```

## AppState field types
```
Button    → (no state field)
Label     → String
TextInput → String
Slider    → f32
Checkbox  → bool
```

## Serialization
UiTree serializes to `.rohkai.json`. All types must have `#[derive(Serialize, Deserialize)]`.
