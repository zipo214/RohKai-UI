# Widgets And Components Evaluation

## Scope

This covers built-in visual widgets, layout widgets, data-display widgets,
technical widgets, and design-time non-visual components.

## Top-Class Expectation

Top-class widget depth means each widget has:

- realistic canvas representation,
- contextual properties,
- AppState binding where applicable,
- live code preview,
- export code that compiles,
- preview-mode behavior,
- persistence,
- tests,
- documented limitations.

Qt Designer and Lazarus set the baseline for catalog breadth and object
inspector depth. RohKai's differentiator should be truthful Rust/egui codegen.

## Current Built-In Widget Depth

| Family | Widgets | Current Depth | Current Behavior | Ideal State |
|---|---|---:|---|---|
| Basic | Button, Label | 3 | Canvas, properties, codegen/export, label editing. | Full event surface, style states, accessibility labels. |
| Text/input | TextInput, TextArea, SpinBox, Slider | 3 | Bindings, min/max/defaults, codegen/export. | Validation rules, formatting, input masks, preview state tools. |
| Choice | Checkbox, RadioButton, ComboBox, FontComboBox | 3 | Bindings/options, canvas visuals, codegen/export. | Option models, keyboard behavior, searchable combo, enum binding. |
| Display | ProgressBar, MathLabel | 2-3 | ProgressBar real; MathLabel computed f32 label MVP. | Rich formatting, formula expressions, diagnostics, units. |
| Layout | Frame, GroupBox, VLayout, HLayout, GridLayout, ScrollArea, TabWidget | 2-3 | Visual boxes and basic export forms. | True child layout semantics, constraints, slot editing, container-specific inspector. |
| Spacers | HorizontalSpacer, VerticalSpacer | 2 | Canvas markers and simple export space. | Layout-aware flexible/fixed spacer behavior. |
| Buttons | ToolButton, CommandLinkButton, DialogButtonBox | 2-3 | Basic egui output from labels/options. | Icons, roles, standard actions, keyboard/default/cancel semantics. |
| Containers | StackedWidget, ToolBox | 2 | Visual/container MVPs with option sections. | Active-page editing, child ownership, page management UI. |
| Data views | Table, ListView, TreeView | 2 | Static option-backed display. | Model-bound views, selection, editing, sorting, filtering, large data virtualization. |
| Technical | Chart, FilePicker | 2-3 | FilePicker real native picker; Chart minimal Vec<f32> bar painter. | Chart series/axes/legends/interactions; platform-aware file picking and filters. |
| Non-visual | Timer, DataSource, Lifecycle, StateMachine, HttpRequest | 1-2 | Tray/config, some state fields, generated comments. | Real runtime dispatch, lifecycle hooks, async/network policies, visual wiring. |

## Current MVPs That Must Not Be Overclaimed

| Feature | Why It Is MVP |
|---|---|
| MathLabel | It formats one bound `f32`; it does not parse formulas. |
| Chart | It renders bars from `Vec<f32>`; no axes, labels, multiple series, legends, or interactions. |
| Table/ListView/TreeView | Static options, not model/view or data-bound widgets. |
| Timer | Comment stub, not a scheduler. |
| StateMachine | State field and comment, not a transition engine. |
| HttpRequest | Response field and comment, not network execution. |

## Ideal Widget Catalog

| Category | Ideal Features |
|---|---|
| Forms | Validation, required flags, error text, input masks, focus order, submit grouping. |
| Data views | Data source binding, model separation, selection model, edit delegates, sorting/filtering. |
| Charts | Multiple series, axes, legend, scales, tooltips, zoom/pan, color palettes, data binding. |
| Formula | Parser, typed expressions, field dependencies, formatting, error diagnostics, preview values. |
| Components | Timers, lifecycle, HTTP, data sources, state machines as real design-time non-visual objects. |
| Accessibility | Labels, descriptions, tab order, keyboard interaction, contrast warnings. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Widget pipeline | Every widget has schema, default, palette, canvas, properties, preview, codegen, export, tests. |
| Export compile proof | Generated app with every widget compiles in CI or fixture test. |
| Property coverage | Each relevant egui API knob is exposed or intentionally documented as omitted. |
| State binding | Bound widgets have type-safe state defaults and collision handling. |
| UX match | Canvas preview resembles actual egui output at supported states. |

## Recommended Next Work

1. Add generated-project compile fixture containing every built-in widget.
2. Promote DataSource from state-only to real data provider model.
3. Build Formula Widget as a separate feature, not by stretching MathLabel.
4. Build Chart v2: axes, labels, series model, and generated painter helpers.
5. Add model-bound Table/List/Tree lane under Stage 13.

