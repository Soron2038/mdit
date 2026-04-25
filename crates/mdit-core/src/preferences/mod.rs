//! Plattformneutrale Persistenz für Theme- und Schriftgrößen-Einstellungen.
//!
//! Ersetzt die NSUserDefaults-basierte Variante. Schreibt eine kleine
//! `preferences.json` ins per-User Config-Directory (via `directories`):
//!
//! - macOS:   `~/Library/Application Support/mdit/preferences.json`
//! - Windows: `%APPDATA%\mdit\config\preferences.json`
//! - Linux:   `$XDG_CONFIG_HOME/mdit/preferences.json`

use std::fs;
use std::io;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::theme::ThemePreference;

pub const DEFAULT_FONT_SIZE: f64 = 16.0;
pub const MIN_FONT_SIZE: f64 = 12.0;
pub const MAX_FONT_SIZE: f64 = 24.0;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "Soron2038";
const APPLICATION: &str = "mdit";
const FILENAME: &str = "preferences.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Preferences {
    #[serde(default)]
    pub theme: ThemePreference,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
}

fn default_font_size() -> f64 {
    DEFAULT_FONT_SIZE
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::default(),
            font_size: DEFAULT_FONT_SIZE,
        }
    }
}

impl Preferences {
    pub fn clamp_font_size(size: f64) -> f64 {
        size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
    }

    /// Liest die Preferences aus dem Standard-Pfad. Bei jedem Fehler (kein
    /// Pfad ermittelbar, Datei fehlt, JSON ungültig) werden Defaults
    /// zurückgegeben — Preferences sind nie blocking.
    pub fn load() -> Self {
        match preferences_path() {
            Some(path) => Self::load_from(&path).unwrap_or_default(),
            None => Self::default(),
        }
    }

    /// Schreibt die Preferences in den Standard-Pfad. Schluckt Fehler
    /// (loggen wäre Sache der UI-Schicht — der Core soll seitenfrei bleiben).
    pub fn save(&self) {
        if let Some(path) = preferences_path() {
            let _ = self.save_to(&path);
        }
    }

    pub fn load_from(path: &std::path::Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save_to(&self, path: &std::path::Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, json)
    }
}

/// Liefert den Pfad zur Preferences-Datei, oder `None` wenn das Betriebssystem
/// kein Config-Directory bereitstellt (extrem selten — sandboxed/headless).
pub fn preferences_path() -> Option<PathBuf> {
    let dirs = ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)?;
    Some(dirs.config_dir().join(FILENAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_system_theme_and_default_font_size() {
        let p = Preferences::default();
        assert_eq!(p.theme, ThemePreference::System);
        assert_eq!(p.font_size, DEFAULT_FONT_SIZE);
    }

    #[test]
    fn font_size_clamps_to_range() {
        assert_eq!(Preferences::clamp_font_size(2.0), MIN_FONT_SIZE);
        assert_eq!(Preferences::clamp_font_size(99.0), MAX_FONT_SIZE);
        assert_eq!(Preferences::clamp_font_size(16.0), 16.0);
    }

    #[test]
    fn roundtrip_through_disk() {
        let dir = std::env::temp_dir().join(format!("mdit-prefs-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("preferences.json");

        let p = Preferences {
            theme: ThemePreference::Dark,
            font_size: 18.0,
        };
        p.save_to(&path).expect("save");

        let loaded = Preferences::load_from(&path).expect("load");
        assert_eq!(loaded, p);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_returns_error() {
        let path = std::env::temp_dir().join(format!("mdit-missing-{}.json", uuid::Uuid::new_v4()));
        assert!(Preferences::load_from(&path).is_err());
    }

    #[test]
    fn corrupt_json_returns_invalid_data_error() {
        let dir = std::env::temp_dir().join(format!("mdit-prefs-corrupt-{}", uuid::Uuid::new_v4()));
        let path = dir.join("bad.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"{ not json").unwrap();
        let err = Preferences::load_from(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
