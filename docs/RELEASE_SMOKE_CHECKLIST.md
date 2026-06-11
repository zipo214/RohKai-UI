# RohKai Release Smoke Checklist

Run this manually before tagging a release. Each step verifies a distinct
surface. Mark each checkbox as the tester works through the list.

## Setup

```
cargo build --release
```

Run the release binary for all steps below.

---

## 1. Canvas Basics

- [ ] App opens with an empty canvas and default layout (palette left, canvas
      centre, properties right, code panel bottom).
- [ ] Drag a Button from the palette onto the canvas — widget appears.
- [ ] Select the widget — properties panel populates.
- [ ] Resize the widget by dragging a corner handle — size updates.
- [ ] Multi-select: rubber-band two widgets — both highlight; move the group.
- [ ] Z-order: right-click → "Bring to Front" / "Send to Back" — works.
- [ ] Smart guides appear when dragging near another widget edge.

## 2. Save / Load

- [ ] File → Save As → save to a `.rohkai.json` file.
- [ ] File → New (discard if prompted).
- [ ] File → Open → reopen the saved file — widgets restored identically.
- [ ] Modify a widget after load and verify the dirty indicator (title bar or
      window) reflects unsaved changes.
- [ ] File → Save (Ctrl+S) — dirty indicator clears.

## 3. Export

- [ ] File → Export → choose a destination directory.
- [ ] Verify that `Cargo.toml`, `src/main.rs`, and `src/app.rs` are written.
- [ ] In the export directory: `cargo check` (or `cargo build`) succeeds.
- [ ] Add a FilePicker widget, re-export — `Cargo.toml` contains `rfd`.
- [ ] Add an Image widget with SVG source, re-export — `src/app.rs` contains
      `mod rohkai_svg`.

## 4. Code Panel (Lazare Bidirectional Sync)

- [ ] Select a Button on the canvas — code panel scrolls to it and highlights
      the widget's generated block.
- [ ] In the code panel, change the label string literal — canvas widget label
      updates.
- [ ] Type a syntactically invalid Rust expression — error indicator appears,
      canvas is unchanged.
- [ ] Delete all code in the panel — canvas clears.
- [ ] Paste a valid widget block back in — widget re-appears.

## 5. Preview Mode

- [ ] Press F5 (or View → Preview) — canvas enters preview mode.
- [ ] Widgets render in their final state; drag/resize handles disappear.
- [ ] Press F5 again — returns to design mode.

## 6. Templates

- [ ] Templates panel shows "Built-in" section with "Form Layout" and
      "Login Dialog".
- [ ] Click "Form Layout" — a GridLayout with label/TextInput pairs is added
      to the canvas at centre.
- [ ] Drag "Login Dialog" onto the canvas — VLayout with 2 fields + button
      is placed at drop position.
- [ ] Select two widgets, File → Save as Template (or context menu) — template
      appears in the "Saved" section.
- [ ] Click the saved template — widgets are added to the canvas.

## 7. Preferences & Theme

- [ ] File → Preferences (or the gear icon) — panel opens.
- [ ] Change UI scale — canvas/panel sizes update immediately.
- [ ] Change accent colour — buttons and highlights update.
- [ ] Close and reopen the app — preferences are persisted.
- [ ] File → Theme → Dark / Light — theme switches.
- [ ] Export a `.rktheme` file, import it back — theme applies correctly.

## 8. SVG Import

- [ ] File → Import SVG (or drag an `.svg` file onto the canvas).
- [ ] SVG appears as a template in the templates panel (with preview thumbnail).
- [ ] Drag the SVG template onto the canvas — an Image widget appears.
- [ ] Select the Image widget → Properties panel shows the SVG report panel
      (Fidelity, Rendered count, Warnings, Unsupported features).
- [ ] Toggle "Expand SVG inline in code panel" — code panel switches between
      compact `[SVG: N bytes]` and the full raw string literal.
- [ ] Import an SVG > 10 KB with inline toggle enabled — warning label
      appears below the checkbox.

## 9. Custom Widgets

- [ ] Open the Widget Builder (Widgets → New Custom Widget).
- [ ] Fill in a name, add a label property, set a simple codegen template.
- [ ] Save the descriptor — widget appears in the palette.
- [ ] Place it on the canvas — properties panel shows the custom property.
- [ ] Code panel reflects the rendered template.

## 10. Multi-Select & Group Operations

- [ ] Select multiple widgets — Properties panel shows "Group/Ungroup"
      controls.
- [ ] Group them — Frame/container wraps the selection.
- [ ] Ungroup — widgets return to root level.
- [ ] Select and delete a group — canvas clears correctly.

## 11. Undo / Redo

- [ ] Add a widget, press Ctrl+Z — widget removed.
- [ ] Press Ctrl+Y — widget restored.
- [ ] Chain 5+ operations, undo all the way to empty — no crash.

## 12. Project Tree

- [ ] View → Project Files — project tree opens.
- [ ] Generated files are listed (main.rs, app.rs, Cargo.toml, …).
- [ ] Click a file in the tree — content viewer shows the generated source.
- [ ] Add an asset via the asset panel — MANIFEST.txt appears in the tree.

---

## Notes

- Mark items `[ x ]` (space-x-space) as each is tested.
- Record the RohKai version / git hash at the top of a copy of this file when
  doing a release run.
- Any regression found during smoke gets its own bug report before the release
  tag is applied.
