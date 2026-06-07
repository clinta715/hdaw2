use crate::ui::preferences::PreferencesState;
use std::fs;
use std::path::PathBuf;

fn prefs_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")) {
        let mut path = PathBuf::from(dir);
        path.push("hdaw");
        path.push("preferences.ron");
        path
    } else {
        PathBuf::from("preferences.ron")
    }
}

pub fn save_preferences(prefs: &PreferencesState) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(data) = ron::ser::to_string(prefs) else {
        tracing::error!("failed to serialize preferences");
        return;
    };
    if let Err(e) = fs::write(&path, data) {
        tracing::error!("failed to write preferences: {e}");
    }
}

pub fn load_preferences() -> Option<PreferencesState> {
    let path = prefs_path();
    if !path.exists() {
        return None;
    }
    let data = fs::read_to_string(&path).ok()?;
    let mut prefs: PreferencesState = ron::from_str(&data).ok()?;
    prefs.clamp_panel_sizes();
    Some(prefs)
}