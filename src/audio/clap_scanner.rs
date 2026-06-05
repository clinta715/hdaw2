use clack_host::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub path: PathBuf,
    pub features: Vec<String>,
    pub is_instrument: bool,
}

pub fn scan_plugins() -> Vec<PluginDescriptor> {
    let mut results = Vec::new();
    for dir in clap_search_dirs() {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if is_clap_file(&path) {
                    scan_clap_file(&path, &mut results);
                }
            }
        }
    }
    results
}

fn clap_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(target_os = "windows") {
        if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local_app).join("Programs").join("Common").join("CLAP"));
        }
        if let Ok(prog_files) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(prog_files).join("Common Files").join("CLAP"));
        }
        if let Ok(prog_files) = std::env::var("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(prog_files).join("Common Files").join("CLAP"));
        }
    } else if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/CLAP"));
        }
    } else {
        dirs.push(PathBuf::from("/usr/lib/clap"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join(".clap"));
        }
    }
    dirs
}

fn is_clap_file(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e == "clap")
}

fn scan_clap_file(path: &PathBuf, results: &mut Vec<PluginDescriptor>) {
    let entry = match unsafe { PluginEntry::load(path) } {
        Ok(e) => e,
        Err(_) => return,
    };
    let factory = match entry.get_plugin_factory() {
        Some(f) => f,
        None => return,
    };
    for desc in factory.plugin_descriptors() {
        let id = desc.id().map(|id| id.to_string_lossy().into_owned()).unwrap_or_default();
        let name = desc.name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| id.clone());
        let vendor = desc.vendor().map(|v| v.to_string_lossy().into_owned()).unwrap_or_default();
        let version = desc.version().map(|v| v.to_string_lossy().into_owned()).unwrap_or_default();
        let features: Vec<String> = desc.features().map(|f| f.to_string_lossy().into_owned()).collect();
        let is_instrument = features.iter().any(|f| f == "instrument");
        results.push(PluginDescriptor {
            id,
            name,
            vendor,
            version,
            path: path.clone(),
            features,
            is_instrument,
        });
    }
}