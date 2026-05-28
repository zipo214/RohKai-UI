# RohKai Roadmap — Comparative Analysis & Adjusted Plan

**Date:** 2026-05-27  
**Purpose:** Research-based analysis of mature UI designers and adjusted roadmap for RohKai

---

## Executive Summary

This document provides:
1. In-depth UX mechanics research of mature UI designers (Lazarus/Delphi, Qt Designer, Glade, Interface Builder, LabVIEW, C++Builder)
2. Feature-by-feature comparison with RohKai's current roadmap
3. Adjusted roadmap with new stages and enhancements (no deletions)
4. Implementation recommendations

**Key Finding:** RohKai's roadmap is well-aligned with mature designers but needs enhancement in UX ergonomics (Stage 8.5), layout system maturity (Stage 9.5), and visual data binding (Stage 10-11).

---

## Part 1: UX Mechanics of Mature Designers

### 1.1 Lazarus IDE / Delphi (LCL/VCL)

**Maturity:** 25+ years of development

**Workflow Pattern:**
1. **Component Palette** — Categorized, searchable, with custom component installation
2. **Form Designer** — Click to place, drag to resize, visual alignment guides
3. **Object Inspector** — Two tabs: Properties (categorized) and Events (alphabetical)
4. **Form Structure** — Tree view showing component hierarchy
5. **Event Wiring** — Double-click component to create event handler, or select from Events tab
6. **Menu Designer** — Visual menu/bar editor with item editor
7. **Dialog Templates** — Standard dialogs accessible from palette

**Key UX Insights:**
- **Property Grouping** — Properties organized by category (Data, Visual, Behavior, etc.)
- **Default Properties** — Right-click to reset any property to default
- **Component Naming** — Auto-suggest naming convention (Button1, Edit1, etc.)
- **Live Preview** — Run form in isolation (Shift+F12)
- **Inheritance** — Visual inheritance from base forms

**Infrastructure Required:**
- LCL (Lazarus Component Library) — 500+ components
- Form streaming (.lfm format) — text-based component tree serialization
- IDE designer framework — property editors, component editors
- Package manager — compile-time and design-time registration

---

### 1.2 Qt Designer

**Maturity:** 20+ years

**Workflow Pattern:**
1. **Widget Box** — Left panel with categorized widgets (Containers, Input, Display, etc.)
2. **Canvas** — Central design area with layout preview
3. **Property Editor** — Right panel with grouped properties (layout, sizePolicy, font, etc.)
4. **Signal/Slot Editor** — F4 to enter editing mode, drag from signal to slot
5. **Buddy Editing** — Ctrl+2 to set label buddies for keyboard navigation
6. **Tab Order** — Ctrl+1 to set keyboard navigation order
7. **Form Preview** — Preview form in isolation

**Key UX Insights:**
- **Promotion System** — Promote basic widgets to custom classes (e.g., QWidget → MyCustomWidget)
- **Size Policies** — Visual editor for horizontal/vertical sizing behavior (Fixed, Minimum, Maximum, Preferred, Expanding)
- **Layout Preview** — See how layouts behave when window is resized
- **Resource Editor** — Integrated image/translation management
- ** Buddy System** — Automatic Alt+Letter keyboard shortcuts for labels

**Infrastructure Required:**
- Qt Meta-Object System (MOC) — runtime type info, signals/slots
- uic compiler — .ui XML → C++ code
- Qt Designer plugin API — custom widget integration
- Property editor framework — factory pattern for editors

---

### 1.3 Glade (GTK Interface Builder)

**Maturity:** 20+ years

**Workflow Pattern:**
1. **Palette** — Widgets organized by category (Controls, Display, Containers, Toplevel)
2. **Canvas** — Click to place at center, or drag to position
3. **Properties Panel** — Three tabs:
   - **Common** — ID, tooltip, events, accessibility
   - **General** — Widget-specific properties
   - **Accessibility** — ARIA-like attributes
4. **Signals Tab** — Separate panel for event connections
5. **Hierarchy View** — Tree panel showing widget nesting
6. **Preview** — Test UI in isolation

**Key UX Insights:**
- **Signal Separation** — Events in separate tab (cleaner than mixing with properties)
- **Mandatory Naming** — Every widget requires an ID (for code reference)
- **GtkBuilder Format** — Human-readable XML (.glade files)
- **Clipboard Operations** — Copy/paste with full property preservation
- **Packing Options** — Visual editor for container packing (expand, fill, padding)

**Infrastructure Required:**
- GtkBuilder — XML → widget tree deserialization
- Property editor framework — type-specific editors
- Signal connection system — runtime signal/slot binding

---

### 1.4 Interface Builder (macOS/iOS/Xcode)

**Maturity:** 15+ years (modern version)

**Workflow Pattern:**
1. **Object Library** — Searchable palette with categories (Views, Controls, Data Views, etc.)
2. **Canvas** — Storyboard or XIB file with scene-based layout
3. **Inspectors** (right panel):
   - **Attributes Inspector** — Widget properties grouped by type
   - **Size Inspector** — Frame, autoresizing, constraints
   - **Connections Inspector** — Outlets (IBOutlets) and actions (IBActions)
   - **Identity Inspector** — Custom class, module, restoration ID
4. **Document Outline** — Hierarchical view with drag-reorder
5. **Assistant Editor** — Split view with corresponding code file
6. **Control Drag** — Drag from UI element to code to create outlet/action

**Key UX Insights:**
- **Auto Layout** — Visual constraint editor with alignment guides and constraint icons
- **Size Classes** — Design for different device sizes simultaneously (Regular/Compact)
- **Segues** — Visual connection between view controllers
- **Live Preview** — Real-time preview on device simulators
- **Control Drag** — Most intuitive way to create connections (drag from button to code)
- **Stack Views** — Automatic layout management (like CSS Flexbox)

**Infrastructure Required:**
- Auto Layout engine — constraint-based layout solver
- IBDesignables — live rendering of custom views in designer
- Storyboard runtime — load and instantiate scenes
- Size class system — adaptive layout engine

---

### 1.5 LabVIEW (National Instruments)

**Maturity:** 35+ years

**Workflow Pattern:**
1. **Front Panel** — UI design area with controls (inputs) and indicators (outputs)
2. **Block Diagram** — Graphical code (G language) showing dataflow
3. **Controls Palette** — Numeric, Boolean, String, Array, Cluster, etc.
4. **Functions Palette** — Programming structures, math, I/O, analysis
5. **Tools Palette** — Wiring tool, positioning tool, coloring tool, etc.
6. **Wiring** — Connect outputs to inputs with type-safe wires

**Key UX Insights:**
- **Dual View** — Front panel (UI) and Block Diagram (logic) are separate but linked
- **Dataflow Programming** — Wires connect outputs to inputs visually
- **Type-Safe Wiring** — Only compatible types can be connected (color-coded)
- **Execution Highlighting** — Animated data flow during debugging
- **SubVIs** — Create reusable components with custom icons
- **Error Clustering** — Automatic error propagation through wires

**Infrastructure Required:**
- G language compiler — graphical dataflow language
- Front panel runtime — render controls and indicators
- Dataflow scheduler — execute nodes when all inputs ready
- Type system — enforce type-safe connections

**Relevance to RohKai:** LabVIEW's visual programming model directly informs RohKai's Stage 10-11 vision (visual data binding, channel connections, iterator pipelines).

---

### 1.6 C++Builder / wxFormBuilder

**C++Builder (Embarcadero):**

**Workflow Pattern:**
1. **Tool Palette** — Components organized by tabs (Standard, Additional, Win32, etc.)
2. **Form Designer** — Visual layout with snap-to-grid and smart guides
3. **Object Inspector** — Properties and Events tabs
4. **Structure Pane** — Tree view of components
5. **Code Editor** — Integrated with form designer (split view)

**Key UX Insights:**
- **Rapid Application Development** — Double-click component to create event handler
- **LiveBindings** — Visual data binding editor with bidirectional arrows
- **Multi-Device Preview** — See UI on different screen sizes
- **Style Book** — Centralized styling for components
- **Action Lists** — Centralized action management (like Qt Actions)

---

**wxFormBuilder:**

**Workflow Pattern:**
1. **Palette** — Standard wxWidgets controls
2. **Object Tree** — Hierarchical view
3. **Properties** — Context-sensitive based on selection
4. **Events** — Separate tab for event handlers
5. **Generate Code** — Outputs C++, Python, or PHP

**Key UX Insights:**
- **Code Generation** — Multiple language output
- **Sizer System** — Visual sizer (layout) editor
- **Inheritance** — Generate base class for extension
- **Plugin Architecture** — Custom control support

---

## Part 2: Feature Comparison Matrix

| Feature | RohKai | Lazarus | Qt Designer | Glade | Interface Builder | LabVIEW |
|---------|--------|---------|-------------|-------|-------------------|---------|
| **Palette** | ✅ Categorized | ✅ Categorized | ✅ Widget Box | ✅ Categorized | ✅ Object Library | ✅ Dual (Controls/Functions) |
| **Properties Panel** | ✅ Basic | ✅ Categorized | ✅ Grouped | ✅ Tabbed | ✅ Multiple inspectors | ✅ Property window |
| **Event Wiring** | ✅ Text field | ✅ Events tab | ✅ Signal/Slot editor | ✅ Signals tab | ✅ Connections inspector | ✅ Visual wires |
| **Smart Guides** | ✅ Stage 6 | ✅ Snap guides | ✅ Alignment guides | ✅ Basic | ✅ Auto Layout guides | ✅ Alignment tools |
| **Undo/Redo** | ❌ Stage 14 | ✅ 50+ levels | ✅ Full | ✅ Full | ✅ Full | ✅ Full |
| **Hierarchy Panel** | ❌ Stage 14 | ✅ Structure pane | ✅ Object tree | ✅ Hierarchy view | ✅ Document outline | ✅ Hierarchy window |
| **Layout System** | 🟡 Frame only | ✅ Alignments | ✅ Full layouts | ✅ Packing | ✅ Auto Layout + Stack Views | ✅ Sizers |
| **Preview Mode** | ❌ | ✅ Shift+F12 | ✅ Preview | ✅ Preview | ✅ Live preview | ✅ Run front panel |
| **Custom Widgets** | ✅ .rkwd | ✅ Packages | ✅ Promotion | ✅ Plugins | ✅ IBDesignables | ✅ SubVIs |
| **Data Binding** | 🟡 Basic | ✅ Data-aware controls | ✅ Model/View | ❌ | ✅ Bindings | ✅ Visual wires |
| **Templates** | ✅ .rktp | ✅ Frames | ✅ Forms | ✅ Templates | ✅ Storyboards | ✅ SubVIs |
| **Export** | ✅ Full project | ✅ Compile | ✅ uic codegen | ✅ XML | ✅ Storyboard compile | ✅ VI compile |
| **Theming** | ✅ .rktheme | ✅ Skins | ✅ Stylesheets | ✅ CSS | ✅ Appearance manager | ✅ System colors |
| **SVG Import** | ✅ | ❌ | 🟡 Limited | ❌ | ✅ Asset catalog | ❌ |
| **Bidirectional Sync** | ✅ Lazare | 🟡 Partial | 🟡 Partial | ❌ | ✅ Partial | ✅ Full |

**Legend:** ✅ Implemented | 🟡 Partial | ❌ Not implemented

---

## Part 3: Adjusted RohKai Roadmap

### **Stage 8.5 — UX Polish & Designer Ergonomics (NEW)**

*Insert between Stage 8 and Stage 9 — Focus on the designer's own usability*

- [ ] **Widget Naming Convention** — Auto-generate meaningful IDs (button_submit, label_title) based on kind + context
- [ ] **Document Outline Panel** — Tree view of widget hierarchy with drag-reorder, click-to-select
- [ ] **Search in Canvas** — Ctrl+F to find widgets by name, kind, or property value
- [ ] **Preview Mode** — F5 to test UI in isolation without exporting (sandboxed egui context)
- [ ] **Clipboard Enhancements** — Copy/paste with full property preservation, paste-at-cursor
- [ ] **Keyboard Shortcut Customization** — User-configurable shortcuts in preferences
- [ ] **Context Tooltips** — Hover over any UI element in designer to see its purpose
- [ ] **Error Highlighting** — Visual indicators for validation errors (missing bindings, invalid IDs)
- [ ] **Zoom to Selection** — F key to focus canvas on selected widget(s)
- [ ] **Minimap** — Small overview of entire canvas in corner for navigation
- [ ] **Property Reset** — Right-click property to reset to default value
- [ ] **Multi-Select Property Editing** — Edit same property across multiple selected widgets

---

### **Stage 9 — Widget Depth & Lazarus Completeness (ENHANCED)**

*Original items retained, additions marked with [+]*

- [x] Parallelism Foundation (rayon integration) ✅
- [ ] Lazarus Completeness
  - [ ] Full contextual properties per widget kind
  - [ ] Design-time non-visual components (component tray)
  - [ ] Full event list per widget kind
  - [ ] Object Inspector true bidirectionality
  - **[+] Property Categories** — Group properties: Geometry, Appearance, Behavior, Data, Events
  - **[+] Property Search** — Filter properties by name in inspector
  - **[+] Reset to Default** — Right-click property to reset to default value
  - **[+] Property Inheritance** — Show which properties are inherited from parent/container
- [ ] SVG Renderer Progression
- [ ] New Widget Kinds — Layouts & Spacers
  - [ ] Vertical Layout
  - [ ] Horizontal Layout
  - [ ] Grid Layout
  - [ ] Form Layout
  - [ ] Horizontal Spacer
  - [ ] Vertical Spacer
  - **[+] Size Policies** — Minimum/Maximum/Preferred size with expand/shrink behavior
  - **[+] Alignment Options** — Per-widget alignment within layout cells
- [ ] New Widget Kinds — Containers
- [ ] New Widget Kinds — Input Additions

---

### **Stage 9.5 — Layout System Maturity (NEW)**

*New stage focused on professional layout capabilities*

- [ ] **Constraint-Based Layout** — Visual constraint editor (like Auto Layout)
  - [ ] Horizontal/Vertical constraints
  - [ ] Center/align constraints
  - [ ] Equal size constraints
  - [ ] Aspect ratio constraints
- [ ] **Layout Preview** — See how layout responds to size changes
- [ ] **Anchor System** — Enhanced anchoring with visual handles
- [ ] **Margin & Padding Editor** — Visual spacing controls
- [ ] **Layout Validation** — Detect conflicting constraints
- [ ] **Responsive Design** — Size class support for different target resolutions
- [ ] **Nested Layouts** — Support for complex nested layout structures
- [ ] **Layout Templates** — Save and reuse layout patterns

---

### **Stage 10 — Technical & Computational Widgets (ENHANCED)**

*Original items retained, additions marked with [+]*

- [ ] Computational & Non-Visual Components
- [ ] Data Display Widgets
- [ ] New Widget Kinds — Data Views
- [ ] New Widget Kinds — Additional Containers & Buttons
- **[+] Visual Data Binding Editor** — Drag wires between widgets and state fields (LabVIEW-inspired)
- **[+] Type-Safe Connections** — Only compatible types can be connected
- **[+] Binding Validation** — Real-time validation of data bindings
- **[+] Collection Binding** — Bind lists/tables to Vec<T> with automatic iteration
- **[+] Two-Way Binding** — Automatic sync between UI and state

---

### **Stage 11 — Rust-Centric Visual Features (ENHANCED)**

*Original items retained, additions marked with [+]*

- [ ] Ownership visualization
- [ ] Async task wiring
- [ ] Channel connections
- [ ] Error propagation
- [ ] Iterator pipeline builder
- [ ] Trait binding
- [ ] Macro palette
- **[+] Visual State Machine Editor** — Define states and transitions graphically
- **[+] Dataflow Visualization** — Show data flow between widgets during preview
- **[+] Debug Mode** — Step through UI interactions with visual feedback
- **[+] Performance Profiler** — Visual heatmap of widget render times
- **[+] Memory Visualization** — Show ownership and borrowing relationships

---

### **Stage 12 — Platform Targets (ENHANCED)**

- [ ] WASM export panel
- [ ] Configure output path, bundler
- [ ] Generate cargo build compatible project
- [ ] Web-specific widget considerations
- [ ] Preview in browser button
- **[+] Multi-Platform Preview** — See how UI looks on different platforms
- **[+] Platform-Specific Adaptations** — Auto-adjust for platform conventions
- **[+] Native Widget Mapping** — Use native widgets where available (WASM → HTML)

---

### **Stage 13 — Data & Integration (ENHANCED)**

- [ ] DB connection configurator
- [ ] Uses sqlx or rusqlite crate
- [ ] Visual query builder
- [ ] Bind widget to query result field
- [ ] Generated code uses correct Rust DB crate
- [ ] Schema viewer
- [ ] Generates AppState with db connection pool field
- **[+] Live Data Preview** — Test database bindings with sample data
- **[+] Data Validation Rules** — Define validation in designer
- **[+] CRUD Generator** — Auto-generate create/read/update/delete UIs from schema

---

### **Stage 14 — Project Infrastructure (ENHANCED)**

- [ ] Project tree panel
- [ ] Click file to view/edit content
- [ ] Add non-generated files
- [ ] Assets folder management
- [ ] Help system
- [ ] Interactive sandbox mode
- [ ] Full undo/redo stack (50 steps, Ctrl+Z/Ctrl+Y)
- [ ] Widget hierarchy/layers panel
- **[+] Undo/Redo with Visual Feedback** — Highlight affected widgets
- **[+] Action History** — Scrollable list of recent actions with search
- **[+] Collaborative Editing** — Multi-user support (future consideration)
- **[+] Version Control Integration** — Git diff visualization for UI changes
- **[+] Project Templates** — Start from predefined project structures

---

### **Stage 15 — Own Renderer (ENHANCED)**

- [ ] Replace egui rendering layer
- [ ] Widget descriptor format drives renderer
- [ ] Zero transient C dependencies
- [ ] All visual properties available
- **[+] Custom Shaders** — User-defined visual effects
- **[+] Animation System** — Built-in animation support
- **[+] Vector Graphics** — Native SVG rendering without rasterization
- **[+] GPU Acceleration** — Full GPU-accelerated rendering pipeline

---

## Part 4: Implementation Recommendations

### Priority Order

1. **Stage 8.5 (UX Polish)** — **Immediate priority**. These are foundational UX improvements that make the designer usable for complex projects.
   - Document Outline is essential for projects with 20+ widgets
   - Preview Mode allows testing without export cycle
   - Search is critical for navigation

2. **Stage 9 (Widget Depth)** — Continue as planned with property category additions.
   - Property grouping makes 50+ properties manageable
   - Size policies are essential for responsive layouts

3. **Stage 9.5 (Layout System)** — **High priority**. Professional UI design requires sophisticated layout capabilities.
   - Constraint-based layout is the modern standard (Interface Builder, Android Studio)
   - Size classes enable responsive design

4. **Stage 14 (Undo/Redo)** — **Move up in priority**. This is a basic expectation in any designer.
   - Command pattern design (Group 3 Rec 9) should be implemented now
   - Visual feedback for undo/redo improves UX significantly

5. **Remaining stages** — Keep current order.

### Effort Estimates

| Stage | New Items | Enhanced Items | Estimated Effort |
|-------|-----------|----------------|------------------|
| 8.5 | 12 | 0 | 2-3 weeks |
| 9 | 4 | 4 | 3-4 weeks |
| 9.5 | 8 | 0 | 4-6 weeks |
| 10 | 4 | 0 | 3-4 weeks |
| 11 | 4 | 0 | 3-4 weeks |
| 12 | 3 | 0 | 2-3 weeks |
| 13 | 3 | 0 | 3-4 weeks |
| 14 | 5 | 0 | 4-6 weeks |
| 15 | 4 | 0 | 6-8 weeks |

**Total Estimated Effort:** 34-48 weeks of focused development

---

## Part 5: Next Steps

### Questions for Review

1. **Create a detailed implementation plan for Stage 8.5?**
   - This would include specific file changes, API designs, and test plans
   - Recommended as the first implementation target

2. **Expand on any specific UX pattern research?**
   - Deep dive into constraint-based layout systems
   - Analysis of visual data binding patterns
   - Study of undo/redo implementations in designers

3. **Write the adjusted roadmap as a new document?**
   - Create a standalone `docs/ROADMAP_V2.md` with all changes integrated
   - Include migration notes for existing roadmap items

4. **Toggle to Act mode to begin implementing Stage 8.5 features?**
   - Start with Document Outline Panel (highest impact)
   - Then Preview Mode
   - Then Search functionality

---

## Appendix: Key Takeaways from Research

1. **Property Organization is Critical** — All mature designers group properties into categories. RohKai should implement this in Stage 9.

2. **Layout Systems Define Professional Capability** — Constraint-based layouts (Interface Builder) and size policies (Qt) are essential for complex UIs. Stage 9.5 addresses this.

3. **Visual Programming is the Future** — LabVIEW's wire-based dataflow and Interface Builder's control-drag are more intuitive than text-based event wiring. Stages 10-11 should embrace this.

4. **Preview Mode is Expected** — Every mature designer allows testing the UI in isolation. This should be Stage 8.5.

5. **Undo/Redo is Non-Negotiable** — This is a basic expectation since the 1990s. Should be prioritized within Stage 14.

6. **Document Outline is Essential** — For projects with 20+ widgets, a tree view is the only way to navigate. Should be Stage 8.5.

7. **Rust-Specific Features are Differentiators** — Ownership visualization, async wiring, and iterator pipelines are unique to RohKai. These should be emphasized in Stages 10-11.

---

*This document was created based on research into mature UI designers and comparison with RohKai's current roadmap. No features from the original roadmap have been deleted; only additions and reordering have been proposed.*