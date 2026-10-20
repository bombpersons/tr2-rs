//! The editor's own preferences, the theme and interface size, kept between
//! runs in `<config dir>/level-editor/preferences.json`. Command-line flags
//! override them for a run; changing either in the View menu saves both.
//! Headless runs (tests, the smoke script) neither read nor write the file.
use crate::theme::Theme;
use bevy::prelude::*;
use bevy_egui::EguiContexts;
use engine::psx_bevy::settings;
use serde_json::{Value, json};
use std::path::PathBuf;

/// What the file holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Saved {
    pub theme: Theme,
    pub ui_scale: f32,
}

impl Default for Saved {
    fn default() -> Self {
        Self { theme: Theme::default(), ui_scale: 1.0 }
    }
}

/// The interface sizes the flag and the file accept.
pub fn valid_scale(scale: f32) -> bool {
    (0.5..=2.0).contains(&scale)
}

impl Saved {
    /// Reads what it can: a missing, unknown or out-of-range field keeps its
    /// default, and text that isn't a JSON object gives the defaults.
    pub fn parse(bytes: &[u8]) -> Self {
        let value = serde_json::from_slice::<Value>(bytes).unwrap_or(Value::Null);
        let defaults = Self::default();
        Self {
            theme: value["theme"].as_str().and_then(Theme::parse).unwrap_or(defaults.theme),
            ui_scale: value["ui_scale"]
                .as_f64()
                .map(|s| s as f32)
                .filter(|&s| valid_scale(s))
                .unwrap_or(defaults.ui_scale),
        }
    }

    pub fn to_json(self) -> Vec<u8> {
        let text = serde_json::to_string_pretty(&json!({
            "theme": self.theme.name(),
            // 0.9, not the f32's 0.8999999761581543.
            "ui_scale": (f64::from(self.ui_scale) * 100.0).round() / 100.0,
        }));
        text.unwrap_or_default().into_bytes()
    }
}

/// Where the preferences live and what was last saved there.
#[derive(Resource)]
pub struct Preferences {
    /// `None` for headless runs, or when there is no config folder.
    path: Option<PathBuf>,
    saved: Saved,
}

impl Preferences {
    /// The user's saved preferences, or the defaults (with a warning on
    /// stderr when the file can't be read).
    pub fn load(headless: bool) -> Self {
        let path = (!headless).then(|| settings::config_path("level-editor", "preferences.json")).flatten();
        let saved = match path.as_deref().map(settings::read) {
            Some(Ok(Some(bytes))) => Saved::parse(&bytes),
            Some(Err(error)) => {
                eprintln!("editor preferences: {error}");
                Saved::default()
            }
            _ => Saved::default(),
        };
        Self { path, saved }
    }

    pub fn saved(&self) -> Saved {
        self.saved
    }

    /// Records the run's starting state, so it is written only once the user changes it.
    pub fn start_with(&mut self, now: Saved) {
        self.saved = now;
    }
}

/// Writes the preferences when the theme or interface size changes.
pub fn save(
    mut contexts: EguiContexts,
    theme: Res<Theme>,
    mut preferences: ResMut<Preferences>,
    mut started: Local<bool>,
) -> Result {
    // The starting size reaches egui's zoom factor only on the next pass.
    if !std::mem::replace(&mut *started, true) {
        return Ok(());
    }
    let now = Saved { theme: *theme, ui_scale: contexts.ctx_mut()?.zoom_factor() };
    if now == preferences.saved {
        return Ok(());
    }
    preferences.saved = now;
    if let Some(path) = &preferences.path
        && let Err(error) = settings::write(path, &now.to_json())
    {
        warn!("Could not save editor preferences to {}: {error}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let saved = Saved { theme: Theme::Orange, ui_scale: 0.9 };
        assert_eq!(Saved::parse(&saved.to_json()), saved);
    }

    #[test]
    fn bad_fields_fall_back_one_at_a_time() {
        let defaults = Saved::default();
        assert_eq!(Saved::parse(b"not json"), defaults);
        assert_eq!(Saved::parse(b"[1, 2]"), defaults);
        assert_eq!(Saved::parse(br#"{"theme": "purple", "ui_scale": 0.8}"#), Saved { ui_scale: 0.8, ..defaults });
        assert_eq!(
            Saved::parse(br#"{"theme": "tomb raider", "ui_scale": 9}"#),
            Saved { theme: Theme::TombRaider, ..defaults }
        );
        assert_eq!(Saved::parse(br#"{"ui_scale": "big"}"#), defaults);
    }
}
