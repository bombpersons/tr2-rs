//! Settings a game adds to the controls menu, shown on a page of their own
//! beside the bindings (Tab, or L1/R1 on a pad). Each option is a short list
//! of choices. The menu drafts them with the bindings: Save applies both,
//! Restore defaults resets both. The menu does not store them: the game owns
//! the setting and its file, and watches [`ControlsOptions`] for changes.
use bevy::prelude::*;

/// One setting: a label and the choices it steps through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlOption {
    /// Stable name the game looks the option up by.
    pub id: String,
    /// Shown in the action column; up to 10 characters leave a gap before the value.
    pub label: String,
    /// Shown in the value column; 9 characters fit.
    pub choices: Vec<String>,
    pub value: usize,
    /// What Restore defaults sets.
    pub default: usize,
}

impl ControlOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>, choices: &[&str], default: usize) -> Self {
        let choices: Vec<String> = choices.iter().map(|c| c.to_string()).collect();
        let default = default.min(choices.len().saturating_sub(1));
        Self { id: id.into(), label: label.into(), choices, value: default, default }
    }
    /// An OFF/ON switch.
    pub fn toggle(id: impl Into<String>, label: impl Into<String>, default: bool) -> Self {
        Self::new(id, label, &["OFF", "ON"], usize::from(default))
    }
    pub fn choice(&self) -> &str {
        self.choices.get(self.value).map_or("", |c| c.as_str())
    }
    /// Moves `delta` choices along, wrapping.
    pub fn step(&mut self, delta: i32) {
        let n = self.choices.len().max(1) as i32;
        self.value = (self.value as i32 + delta).rem_euclid(n) as usize;
    }
}

/// The options page. Empty (the default) hides the page entirely.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct ControlsOptions {
    /// The page's tab name.
    pub title: String,
    pub options: Vec<ControlOption>,
}

impl Default for ControlsOptions {
    fn default() -> Self {
        Self { title: "OPTIONS".into(), options: Vec::new() }
    }
}

impl ControlsOptions {
    /// The page has room for this many rows.
    pub const MAX: usize = 14;

    /// Adds `option`, or replaces the one with its id.
    pub fn add(&mut self, option: ControlOption) -> &mut Self {
        if let Some(slot) = self.options.iter_mut().find(|o| o.id == option.id) {
            *slot = option;
        } else if self.options.len() < Self::MAX {
            self.options.push(option);
        } else {
            warn!("Controls menu: no room for option {}", option.id);
        }
        self
    }
    pub fn get(&self, id: &str) -> Option<&ControlOption> {
        self.options.iter().find(|o| o.id == id)
    }
    /// The chosen index of `id`.
    pub fn value(&self, id: &str) -> Option<usize> {
        self.get(id).map(|o| o.value)
    }
    /// A switch made by [`ControlOption::toggle`].
    pub fn enabled(&self, id: &str) -> bool {
        self.value(id).is_some_and(|v| v != 0)
    }
    /// Sets `id` to choice `value` (clamped); false when there is no such option.
    pub fn set(&mut self, id: &str, value: usize) -> bool {
        let Some(option) = self.options.iter_mut().find(|o| o.id == id) else { return false };
        option.value = value.min(option.choices.len().saturating_sub(1));
        true
    }
    pub fn restore_defaults(&mut self) {
        for option in &mut self.options {
            option.value = option.default;
        }
    }
}

/// The controls menu's pages.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    #[default]
    Bindings,
    Options,
}
