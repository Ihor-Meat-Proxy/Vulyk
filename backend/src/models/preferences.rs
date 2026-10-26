//! User appearance preferences (theme, typography, motion).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const THEME_SYSTEM: &str = "system";
pub const THEME_LIGHT: &str = "light";
pub const THEME_DARK: &str = "dark";
pub const THEME_HIGH_CONTRAST: &str = "high_contrast";
pub const THEME_SEPIA: &str = "sepia";

pub const THEMES: &[&str] = &[
    THEME_SYSTEM,
    THEME_LIGHT,
    THEME_DARK,
    THEME_HIGH_CONTRAST,
    THEME_SEPIA,
];

pub const DENSITY_COMPACT: &str = "compact";
pub const DENSITY_COMFORTABLE: &str = "comfortable";

pub const DENSITIES: &[&str] = &[DENSITY_COMPACT, DENSITY_COMFORTABLE];

/// Font size as a percentage of the base (87–138). Steps: 87, 100, 112, 125, 138.
pub const FONT_SIZE_MIN: i32 = 87;
pub const FONT_SIZE_MAX: i32 = 138;
pub const FONT_SIZE_DEFAULT: i32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct UserPreferences {
    /// `system` | `light` | `dark` | `high_contrast` | `sepia`
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Base font scale percent (87–138).
    #[serde(default = "default_font_size")]
    pub font_size: i32,
    /// `compact` | `comfortable`
    #[serde(default = "default_density")]
    pub density: String,
    /// `None` = follow `prefers-reduced-motion`; `Some(true/false)` overrides.
    #[serde(default)]
    pub reduced_motion: Option<bool>,
    /// Email notification preference.
    #[serde(default = "default_true")]
    pub notify_email: bool,
    /// In-app notification preference.
    #[serde(default = "default_true")]
    pub notify_in_app: bool,
    /// Notify on test-run status updates.
    #[serde(default = "default_true")]
    pub notify_run_updates: bool,
}

fn default_theme() -> String {
    THEME_SYSTEM.to_string()
}

fn default_font_size() -> i32 {
    FONT_SIZE_DEFAULT
}

fn default_density() -> String {
    DENSITY_COMFORTABLE.to_string()
}

fn default_true() -> bool {
    true
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            font_size: default_font_size(),
            density: default_density(),
            reduced_motion: None,
            notify_email: true,
            notify_in_app: true,
            notify_run_updates: true,
        }
    }
}

impl UserPreferences {
    pub fn from_value(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }

    /// Merge partial updates; validate and clamp.
    pub fn merge_update(&mut self, patch: &UpdatePreferences) -> Result<(), String> {
        if let Some(theme) = patch.theme.as_deref() {
            let t = theme.trim().to_lowercase().replace('-', "_");
            if !THEMES.contains(&t.as_str()) {
                return Err(format!(
                    "invalid theme '{theme}'; expected one of {}",
                    THEMES.join(", ")
                ));
            }
            self.theme = t;
        }
        if let Some(font_size) = patch.font_size {
            if !(FONT_SIZE_MIN..=FONT_SIZE_MAX).contains(&font_size) {
                return Err(format!(
                    "font_size must be between {FONT_SIZE_MIN} and {FONT_SIZE_MAX}"
                ));
            }
            self.font_size = font_size;
        }
        if let Some(density) = patch.density.as_deref() {
            let d = density.trim().to_lowercase();
            if !DENSITIES.contains(&d.as_str()) {
                return Err(format!(
                    "invalid density '{density}'; expected one of {}",
                    DENSITIES.join(", ")
                ));
            }
            self.density = d;
        }
        // `Some(None)` clears override; `None` leaves unchanged; `Some(Some(v))` sets.
        if let Some(rm) = &patch.reduced_motion {
            self.reduced_motion = *rm;
        }
        if let Some(v) = patch.notify_email {
            self.notify_email = v;
        }
        if let Some(v) = patch.notify_in_app {
            self.notify_in_app = v;
        }
        if let Some(v) = patch.notify_run_updates {
            self.notify_run_updates = v;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdatePreferences {
    pub theme: Option<String>,
    pub font_size: Option<i32>,
    pub density: Option<String>,
    /// Pass `null` explicitly to clear override and follow system again.
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    pub reduced_motion: Option<Option<bool>>,
    pub notify_email: Option<bool>,
    pub notify_in_app: Option<bool>,
    pub notify_run_updates: Option<bool>,
}

fn deserialize_optional_bool<'de, D>(deserializer: D) -> Result<Option<Option<bool>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<bool>::deserialize(deserializer)?))
}

pub fn validate_theme(theme: &str) -> Result<String, String> {
    let t = theme.trim().to_lowercase().replace('-', "_");
    if THEMES.contains(&t.as_str()) {
        Ok(t)
    } else {
        Err(format!(
            "invalid theme '{theme}'; expected one of {}",
            THEMES.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_system_comfortable() {
        let p = UserPreferences::default();
        assert_eq!(p.theme, THEME_SYSTEM);
        assert_eq!(p.density, DENSITY_COMFORTABLE);
        assert_eq!(p.font_size, 100);
        assert_eq!(p.reduced_motion, None);
    }

    #[test]
    fn merge_rejects_bad_theme() {
        let mut p = UserPreferences::default();
        let err = p
            .merge_update(&UpdatePreferences {
                theme: Some("neon".into()),
                ..Default::default()
            })
            .unwrap_err();
        assert!(err.contains("invalid theme"));
    }

    #[test]
    fn merge_accepts_high_contrast_alias() {
        let mut p = UserPreferences::default();
        p.merge_update(&UpdatePreferences {
            theme: Some("high-contrast".into()),
            font_size: Some(112),
            density: Some("compact".into()),
            reduced_motion: Some(Some(true)),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(p.theme, THEME_HIGH_CONTRAST);
        assert_eq!(p.font_size, 112);
        assert_eq!(p.density, DENSITY_COMPACT);
        assert_eq!(p.reduced_motion, Some(true));
    }
}
