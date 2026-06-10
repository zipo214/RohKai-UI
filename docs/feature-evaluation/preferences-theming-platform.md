# Preferences, Theming, And Platform Evaluation

## Scope

This covers user preferences, global UI scale, code/canvas font controls,
generated app theming, document/window presets, platform behavior, scripts, and
future WASM/platform targets.

## Top-Class Expectation

RohKai should be comfortable to use for different eyes, monitors, operating
systems, and target apps. Preferences should affect the designer without dirtying
projects. Project theming should affect generated apps deterministically.

## Current State

| Feature | Current Depth | What It Does Now | Gap |
|---|---:|---|---|
| Preferences | 3 | User-level settings, UI scale, font size, snap step. | Needs search, reset sections, live preview, import/export. |
| UI scale | 3 | Applies globally on Apply/OK to avoid self-resizing slider issue. | Needs per-monitor and accessibility preset testing. |
| Code/canvas text scale | 3 | Settings-backed code font and canvas label/tag scale. | Needs broader font audit and text overflow testing. |
| Theming | 3 | Generated app theme fields, `.rktheme`, apply to designer. | Needs theme preview, contrast checks, component-state coverage. |
| Document presets | 3 | Desktop/mobile/custom sizes, window chrome visualization. | Needs preset management and target platform profiles. |
| PowerShell 7 scripts | 3 | UTF-8 checks, preflight, SVG validation. | Needs cross-platform Rust `xtask` eventually. |
| WASM target | 0 | Planned. | Needs platform abstraction before implementation. |

## Utility

- Authoring utility: medium-high.
- Safety utility: medium-high for settings/project separation.
- Runtime utility: high for generated app theme/window behavior.
- Accessibility utility: high.

## Ideal State

| Capability | Ideal Behavior |
|---|---|
| Preferences search | Users can find any setting by name. |
| Accessibility presets | Large text, high contrast, reduced motion, dense/comfortable spacing. |
| Theme diagnostics | Contrast warnings and generated-app preview before export. |
| Platform profiles | Desktop, WASM, mobile-size preview, native dialogs policy, file-picker fallback. |
| Cross-platform automation | Script checks available as Rust `xtask`, with PowerShell wrappers optional. |

## Depth Measurements

| Measure | Target For Top-Class |
|---|---|
| Settings persistence | User settings survive restart and never dirty project files. |
| Accessibility | App remains usable at 75%-200% scale and with large code fonts. |
| Theme correctness | Generated app applies theme before first frame and all supported widgets respect it. |
| Platform clarity | Unsupported platform features are flagged before export. |
| Encoding safety | Repo text checks catch mojibake and replacement characters. |

## Recommended Next Work

1. Add preferences search and section reset buttons.
2. Add contrast checker for generated themes.
3. Add platform capability matrix before WASM work.
4. Move critical checks into Rust `xtask` while keeping `pwsh` wrappers.
5. Add UI-scale screenshot tests for key windows.

