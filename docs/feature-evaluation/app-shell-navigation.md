# App Shell And Navigation Evaluation

## Scope

This covers the menu bar, left rail, status bar, windows/dialogs, shortcuts,
preview mode access, project windows, and general workspace navigation.

## Top-Class Expectation

A top-class GUI builder lets users move between design, structure, properties,
assets, generated code, preview, and export without hunting. The shell should
feel predictable under small and large windows, and every major mode should make
clear what is editable, preview-only, generated, or project-level.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Menu bar | 3 | File/Edit/View/Widgets-style entry points, export, preferences, descriptors, project files. | Needs command search and clearer mode grouping. |
| Left rail | 3 | Tabbed, width-capped Palette/Props/Layers/Components/Templates. | Needs keyboard focus model, pinned/custom tab layout, and better section search/filter. |
| Status bar/ribbon | 3 | Canvas size, snap, zoom, dirty/error/status affordances. | Needs less crowding and richer persistent notification center. |
| Shortcuts | 3 | F1/help window with categories. | Needs searchable commands and conflict detection. |
| Floating windows | 2-3 | Preferences, theme, project tree, Rust wiring, macro palette, descriptors. | Needs window management consistency and docking strategy. |
| Preview mode | 3 | F5 live egui preview with runtime values; code panel hidden. | Needs side-by-side design/preview and interactive state reset tools. |

## Utility

- Authoring utility: high. The shell determines how quickly users can reach the
  right panel.
- Inspection utility: high. Layers, project tree, and code panel are core
  confidence tools.
- Safety utility: medium. Menus and dirty indicators prevent accidental loss.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Command palette | Ctrl+P/Ctrl+Shift+P style launcher for all commands, recent files, widgets, templates, and settings. |
| Dockable panels | Left/right/bottom panels can be shown, hidden, resized, and reset to defaults. |
| Mode clarity | Design, Preview, Export Review, and Code Sync modes have visible state and escape paths. |
| Search/filter | Palette, properties, layers, templates, assets, and commands are filterable. |
| Accessibility | Keyboard navigation, focus rings, readable scale, predictable tab order. |
| Workspace profiles | Beginner, advanced, SVG-heavy, code-heavy, and compact profiles. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Discoverability | New user can create, preview, save, and export within 5 minutes without docs. |
| Panel stability | Left/right panels never cover the canvas unless explicitly floated. |
| Keyboard coverage | 90% of commands reachable without mouse. |
| Error visibility | Persistent errors are dismissible, searchable, and linked to affected feature. |
| Small-window behavior | App remains usable at narrow widths with intentional collapsed navigation. |

## Recommended Next Work

1. Add command palette with fuzzy command/widget/template search.
2. Add panel reset and workspace profile presets.
3. Add notification drawer for warnings/errors instead of cramped top-row text.
4. Add left-panel filter fields for Palette, Layers, Templates, and Components.

