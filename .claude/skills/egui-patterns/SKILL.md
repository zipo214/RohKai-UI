---
name: egui-patterns
description: Use when writing or reviewing egui UI code outside src/canvas/. For
canvas hit-testing, drag state, or resize handles use canvas-patterns instead.
Contains idiomatic egui API patterns for this project.
---

# egui Patterns

## Basic panel
```rust
egui::CentralPanel::default().show(ctx, |ui| { /* ... */ });
egui::SidePanel::left("id").show(ctx, |ui| { /* ... */ });
egui::TopBottomPanel::bottom("id").min_height(180.0).show(ctx, |ui| { /* ... */ });
```

## Button with action
```rust
if ui.button("Label").clicked() { self.count += 1; }
```

## Text input
```rust
ui.text_edit_singleline(&mut self.name);
```

## Slider
```rust
ui.add(egui::Slider::new(&mut self.value, 0.0..=100.0).text("Speed"));
```

## Checkbox
```rust
ui.checkbox(&mut self.enabled, "Enable feature");
```

## DragValue (numeric property editor)
```rust
ui.add(egui::DragValue::new(&mut self.x).speed(1.0));
```

## Layout
```rust
ui.horizontal(|ui| { /* ... */ });
ui.vertical(|ui| { /* ... */ });
ui.columns(2, |cols| { cols[0].label("left"); cols[1].label("right"); });
```

## Allocate a painter (custom drawing)
```rust
let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
painter.rect_filled(resp.rect, 0.0, egui::Color32::from_gray(40));
painter.text(pos, egui::Align2::CENTER_CENTER, "text", egui::FontId::proportional(13.0), egui::Color32::WHITE);
```

## App structure (eframe)
```rust
struct MyApp { /* state fields */ }

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // all UI code goes here, runs every frame
    }
}
```

## Patterns to avoid

> **DO NOT USE — analysis only, not templates.**

- `SidePanel::left("my_panel")` — string literal IDs collide across frames; use a stable `egui::Id` or a constant.
- Calling `ui.allocate_painter(...)` unconditionally every frame without checking `ui.is_rect_visible(...)` first — allocates painter budget even when off-screen.
- `ctx.request_repaint()` in a tight loop without a condition — burns CPU; gate on actual state change.

## Entry point
```rust
// API shape for eframe 0.29 — verify Cargo.toml if NativeOptions/viewport errors appear
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native("App Name", options, Box::new(|_cc| Ok(Box::new(MyApp::default()))))
}
```
