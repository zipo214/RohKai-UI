# Project Infrastructure Evaluation

## Scope

This covers save/load, project file format, templates, project tree, assets,
undo/redo, dirty tracking, settings separation, and generated project inspection.

## Top-Class Expectation

A top-class designer treats projects as durable documents. Users should trust
that they can experiment, undo, save, reopen, inspect generated files, manage
assets, and export without hidden state or path surprises.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Save/load | 4 | Versioned `.rohkai.json`, legacy load, validation/repair. | Needs migration framework and backup/recovery UX. |
| Dirty tracking | 3 | Snapshot-based exact dirty check and cache. | Needs large-project performance instrumentation. |
| Templates | 3 | `.rktp` save/load, folder panel, SVG import path. | Needs metadata, previews, search, versioning. |
| Project tree | 2-3 | Generated file viewer and asset registry. | Read-only viewer; not a full project IDE/file browser. |
| Assets | 2-3 | Asset entries and `assets/MANIFEST.txt`. | Needs copy-on-export policy, validation, previews, missing-file warnings. |
| Undo/redo | 3 | Snapshot stack, cap, Ctrl+Z/Y, drag coalescing. | Needs command-level semantic undo, labels, branch visibility. |
| Preferences separation | 3 | User settings outside project. | Needs import/export settings profile and per-project overrides where appropriate. |

## Utility

- Safety utility: very high.
- Inspection utility: medium-high.
- Authoring utility: medium-high.
- Runtime utility: high for export/project files.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Project migration | Every project version has tested migration and user-visible notes. |
| Recovery | Autosave/recovery, backup before destructive operations, crash restore. |
| Asset pipeline | Assets copied, hashed, validated, previewed, and referenced consistently in export. |
| Template library | Searchable templates with previews, tags, source provenance, and compatibility. |
| Project explorer | Generated and user files visible, editable when safe, and diffable before export. |
| Undo history | Named undo steps, grouping, command logs, and optional timeline view. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Data safety | No operation loses work without explicit confirmation. |
| Migration coverage | Every schema version has load tests. |
| Export reproducibility | Same project exports deterministic file list and contents. |
| Asset health | Missing/unsupported assets are caught before export. |
| Undo correctness | Common operations undo/redo exactly without dirty-state drift. |

## Recommended Next Work

1. Add schema migration table and tests.
2. Add asset health checker and copy-on-export implementation.
3. Add template metadata and preview thumbnails.
4. Add named undo commands.
5. Add generated-project diff before writing export destination.

