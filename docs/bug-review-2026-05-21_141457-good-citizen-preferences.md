# Bug Review — Good Citizen Preferences Pass

Date: 2026-05-21 14:14:57

Scope: Preferences/settings implementation, UI scaling behavior, adjacent persistence hooks.

## Fixed During Review

### P1 — UI scale slider resized its own Preferences window while dragging

Before: `Preferences` sliders mutated `user_settings` directly, and `update()` applied `set_pixels_per_point()` every frame. Dragging UI scale changed the window under the cursor.

After: `Preferences` edits `preferences_draft`. UI scale applies only on `Apply` or `OK`; `Cancel` and closing the window discard the draft.

## Remaining Findings

### P2 — Settings fallback path is not durable when APPDATA/LOCALAPPDATA are unavailable

Before: `settings_path()` falls back to `std::env::temp_dir()`, which can be cleaned by the OS.

Proposed after: fall back to a stable binary-adjacent directory such as `<current_exe_parent>/user-settings/settings.json`, still outside `.rohkai.json`.

### P3 — Save failure still applies settings for the current session

Before: `Apply` copies draft settings into live state before saving. If save fails, the UI changes remain active but will not persist after restart.

Proposed after: keep this behavior, but change the error message to explicitly say "Applied for this session only; save failed: ...".

## Good Citizen Options

- Clear stale "Preferences saved" text as soon as the draft differs from live settings.
- Add one unit test for invalid settings JSON loading to lock the default-reset behavior.
- Keep Preferences user-level; do not serialize these values into `.rohkai.json`.
