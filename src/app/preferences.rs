use std::cell::Cell;

use objc2_foundation::{NSString, NSUserDefaults};

use mdit::ui::appearance::ThemePreference;

const THEME_PREF_KEY: &str = "mditThemePreference";
const FONT_SIZE_PREF_KEY: &str = "mditFontSize";
pub(super) const DEFAULT_FONT_SIZE: f64 = 16.0;
pub(super) const MIN_FONT_SIZE: f64 = 12.0;
pub(super) const MAX_FONT_SIZE: f64 = 24.0;

/// Owns the user's persisted theme and font size preferences.
pub(crate) struct Preferences {
    theme_pref: Cell<ThemePreference>,
    body_font_size: Cell<f64>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme_pref: Cell::new(ThemePreference::default()),
            body_font_size: Cell::new(DEFAULT_FONT_SIZE),
        }
    }
}

impl Preferences {
    /// Read preferences from NSUserDefaults and return a populated instance.
    pub(super) fn load() -> Self {
        Self {
            theme_pref: Cell::new(load_theme_pref()),
            body_font_size: Cell::new(load_font_size_pref()),
        }
    }

    pub(super) fn theme(&self) -> ThemePreference {
        self.theme_pref.get()
    }

    pub(super) fn set_theme(&self, pref: ThemePreference) {
        self.theme_pref.set(pref);
        save_theme_pref(pref);
    }

    /// Set the theme without persisting (used during init from loaded values).
    pub(super) fn set_theme_no_persist(&self, pref: ThemePreference) {
        self.theme_pref.set(pref);
    }

    pub(super) fn font_size(&self) -> f64 {
        self.body_font_size.get()
    }

    pub(super) fn set_font_size(&self, size: f64) {
        self.body_font_size.set(size);
        save_font_size_pref(size);
    }

    /// Set font size without persisting (used during init from loaded values).
    pub(super) fn set_font_size_no_persist(&self, size: f64) {
        self.body_font_size.set(size);
    }
}

/// Persist the user's theme choice to `NSUserDefaults`.
fn save_theme_pref(pref: ThemePreference) {
    let key = NSString::from_str(THEME_PREF_KEY);
    let val = NSString::from_str(pref.as_str());
    unsafe {
        let defaults = NSUserDefaults::standardUserDefaults();
        defaults.setObject_forKey(Some(&*val), &key);
    }
}

/// Load the user's theme choice from `NSUserDefaults`.
/// Falls back to `ThemePreference::System` when no value is stored.
fn load_theme_pref() -> ThemePreference {
    let key = NSString::from_str(THEME_PREF_KEY);
    let stored = NSUserDefaults::standardUserDefaults().stringForKey(&key);
    stored
        .as_deref()
        .map(|s| s.to_string().parse::<ThemePreference>().unwrap_or_default())
        .unwrap_or_default()
}

/// Persist the user's font size to `NSUserDefaults`.
fn save_font_size_pref(size: f64) {
    let key = NSString::from_str(FONT_SIZE_PREF_KEY);
    let val = NSString::from_str(&size.to_string());
    unsafe {
        let defaults = NSUserDefaults::standardUserDefaults();
        defaults.setObject_forKey(Some(&*val), &key);
    }
}

/// Load the user's font size from `NSUserDefaults`.
/// Falls back to `DEFAULT_FONT_SIZE` when no value is stored.
fn load_font_size_pref() -> f64 {
    let key = NSString::from_str(FONT_SIZE_PREF_KEY);
    let stored = NSUserDefaults::standardUserDefaults().stringForKey(&key);
    stored
        .as_deref()
        .and_then(|s| s.to_string().parse::<f64>().ok())
        .unwrap_or(DEFAULT_FONT_SIZE)
}
