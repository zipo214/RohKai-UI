# Visual Widget Maker Plan

RohKai currently has two descriptor-authoring layers:

- **Advanced Descriptor Editor**: the power-user `.rkwd` editor for every
  descriptor field, including templates, events, properties, state fields, and
  dependencies.
- **Guided Descriptor Builder**: a beginner-friendly form over the same
  `WidgetDescriptor` model. It creates simple valid descriptors without making
  the user edit raw Rust templates.

Those are useful, but they are not a true visual widget construction tool. The
future **Visual Widget Maker** is a third layer: a WYSIWYG component studio that
lets users build reusable widgets out of visible primitives and then emits a
`WidgetDescriptor`.

## Non-Confusion Rule

Do not call the current guided builder a full widget maker. It is a guided
descriptor builder. A true widget maker must let users visually compose the
widget body, not only set descriptor metadata.

## Desired User Model

The user opens a separate Widget Maker window and builds a reusable component
on a mini-canvas:

- draw primitive shapes, text, icons, and interaction zones;
- place child widgets or slots;
- group/layout primitive elements;
- expose selected primitive values as public widget properties;
- define internal state such as hover, pressed, checked, selected;
- wire events such as click, change, submit, or custom signals;
- preview how default properties and instance overrides change the result;
- save as a `.rkwd` descriptor for the normal palette/canvas/export pipeline.

## Relationship To Existing Layers

`WidgetDescriptor` remains the source of truth for reusable widget definitions.
The Visual Widget Maker should generate or update descriptors rather than
inventing a separate palette format.

Layer responsibilities:

- **Advanced Descriptor Editor** edits the descriptor directly.
- **Guided Descriptor Builder** creates simple descriptor presets safely.
- **Visual Widget Maker** edits an internal visual construction document and
  compiles that document into descriptor properties, preview/render metadata,
  and codegen templates.

## Future Data Model Sketch

The visual maker likely needs an intermediate document before it can emit a
descriptor:

```text
WidgetMakerDocument
  id
  name
  category
  default_size
  primitives[]
  exposed_properties[]
  internal_state[]
  events[]
  style_tokens[]
  generated_descriptor
```

Primitive examples:

- `Rect`, `RoundedRect`, `Line`, `Text`, `Icon`, `Image`, `Slot`
- `ChildWidget`, `LayoutGroup`, `HitRegion`
- future: vector path, state-driven variant, data-bound repeat

Each primitive should carry geometry, style, provenance, z-order, and optional
property bindings. The generated descriptor should be reproducible from the
document so saving/reopening does not drift.

## Minimum Viable Vertical Slice

1. Add `WidgetMakerDocument` as an internal, serde-ready model.
2. Add a separate Visual Widget Maker window, not embedded in the main canvas.
3. Support a small primitive set: rectangle, text, button-like hit region.
4. Let users expose a text primitive as a `label` property.
5. Generate a valid `WidgetDescriptor` with live/export templates.
6. Save the descriptor to `widgets/` and reload the palette.
7. Keep an "Advanced Descriptor" escape hatch for raw editing.

## Later Capabilities

- Layout groups: horizontal, vertical, grid, stack.
- Primitive constraints: anchor, padding, min/max size, proportional sizing.
- State variants: normal, hover, pressed, disabled, checked.
- Slots: named child content areas.
- Event zones distinct from visual shapes.
- Multi-property style tokens: accent, border, radius, text color, spacing.
- Codegen preview showing exactly what the visual document emits.
- Import existing simple descriptors into a visual document when possible.

## Risks

- A visual maker can accidentally become a second canvas system. Keep its model
  small and compile it into `WidgetDescriptor` output.
- Raw Rust templates are still needed for advanced widgets. Do not hide the
  Advanced Descriptor Editor.
- If generated descriptors cannot round-trip from the visual document, users
  will lose trust. Treat deterministic generation as a core requirement.
