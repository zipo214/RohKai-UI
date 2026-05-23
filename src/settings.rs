use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const APP_DIR: &str = "RohKai";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub ui_scale: f32,
    pub code_font_size: f32,
    pub canvas_label_scale: f32,
    pub canvas_tag_scale: f32,
    pub default_snap_step: f32,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            code_font_size: 11.0,
            canvas_label_scale: 1.0,
            canvas_tag_scale: 1.0,
            default_snap_step: 8.0,
        }
    }
}

impl UserSettings {
    pub fn sanitize(&mut self) {
        self.ui_scale = sane_range(self.ui_scale, 1.0, 0.75, 1.75);
        self.code_font_size = sane_range(self.code_font_size, 11.0, 9.0, 22.0);
        self.canvas_label_scale = sane_range(self.canvas_label_scale, 1.0, 0.75, 2.0);
        self.canvas_tag_scale = sane_range(self.canvas_tag_scale, 1.0, 0.75, 2.0);
        self.default_snap_step = sane_range(self.default_snap_step, 8.0, 1.0, 256.0);
    }
}

pub struct LoadedSettings {
    pub settings: UserSettings,
    pub path: PathBuf,
    pub error: Option<String>,
}

pub fn load() -> LoadedSettings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<UserSettings>(&json) {
            Ok(mut settings) => {
                settings.sanitize();
                LoadedSettings {
                    settings,
                    path,
                    error: None,
                }
            }
            Err(e) => LoadedSettings {
                settings: UserSettings::default(),
                path,
                error: Some(format!("Settings reset: {e}")),
            },
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => LoadedSettings {
            settings: UserSettings::default(),
            path,
            error: None,
        },
        Err(e) => LoadedSettings {
            settings: UserSettings::default(),
            path,
            error: Some(format!("Settings unavailable: {e}")),
        },
    }
}

pub fn save(path: &Path, settings: &UserSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create settings dir: {e}"))?;
    }
    let mut clean = settings.clone();
    clean.sanitize();
    let json = serde_json::to_string_pretty(&clean).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write settings: {e}"))
}

fn settings_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // Fall back to a directory adjacent to the binary so the OS
            // cannot silently purge settings the way it can temp_dir().
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("user-settings")))
                .unwrap_or_else(|| PathBuf::from("user-settings"))
        })
        .join(APP_DIR)
        .join(SETTINGS_FILE)
}

fn sane_range(value: f32, default: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_and_saves_user_settings() {
        let dir = std::env::temp_dir().join(format!("rohkai_settings_{}", uuid::Uuid::new_v4()));
        let path = dir.join("settings.json");
        let mut settings = UserSettings {
            ui_scale: f32::NAN,
            code_font_size: 99.0,
            canvas_label_scale: 0.1,
            canvas_tag_scale: 4.0,
            default_snap_step: -5.0,
        };

        settings.sanitize();
        assert_eq!(settings.ui_scale, 1.0);
        assert_eq!(settings.code_font_size, 22.0);
        assert_eq!(settings.canvas_label_scale, 0.75);
        assert_eq!(settings.canvas_tag_scale, 2.0);
        assert_eq!(settings.default_snap_step, 1.0);

        save(&path, &settings).unwrap();
        let saved: UserSettings = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .expect("saved settings should parse");
        assert_eq!(saved, settings);

        let _ = std::fs::remove_dir_all(dir);
    }
}
