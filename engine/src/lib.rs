//! A reusable engine for 3D action games built from rooms on a sector grid,
//! with characters driven by authored animation and floor triggers, drawn
//! with a PlayStation look.
//!
//! The engine knows nothing about any one game. A game adds its own crate on
//! top: it registers object types and trigger actions with the engine, adds
//! controllers for its player and creatures, and supplies levels in the
//! engine's own format ([`world::format`]). `docs/engine-model.md` at the
//! repository root explains where the line between engine and game is drawn.
//!
//! Modules:
//! - [`world`]: the level asset and collision queries over rooms and sectors.
//! - [`schedule`]: the order of work in a tick, the pause gate and the
//!   [`schedule::Avatar`] the engine follows.
//! - [`animation`]: authored clips, state dispatch, model posing.
//! - [`objects`]: placed objects and the object type registry.
//! - [`triggers`]: floor triggers and the trigger action registry.
//! - [`camera`]: chase camera and fixed shots.
//! - [`lighting`], [`render`], [`audio`]: drawing and sound.
//! - [`config`]: RON tuning files for per-type design numbers.
//! - [`editor`]: authoring transactions, exact no-op saves and level validation.
//! - [`psx`], [`shell`], [`controller`], [`controls`], [`display`],
//!   [`pointer`], [`settings`], [`launcher`], [`viewer`], [`model_viewer`],
//!   [`testing`] and, with the `dev-mcp` feature, `dev_mcp`: the PS1 look,
//!   logic ticks, pad and mouse input, rebinding, the display menu, player
//!   settings files, the executable's command line, capture viewers, headless tests and the
//!   developer MCP. They live in the template's crate, [`psx_bevy`], which
//!   `psx template update` replaces; they are re-exported here so games
//!   depend on `engine` only (see `docs/template.md`).
pub mod animation;
pub mod audio;
pub mod camera;
pub mod config;
pub mod editor;
pub mod lighting;
pub mod objects;
pub mod render;
pub mod schedule;
pub mod triggers;
pub mod world;

pub use psx_bevy;
#[cfg(feature = "dev-mcp")]
pub use psx_bevy::dev_mcp;
pub use psx_bevy::{
    controller, controls, display, launcher, model_viewer, pointer, psx, settings, shell, testing, viewer,
};

use bevy::prelude::*;

/// The engine's rules: schedule, animation, objects, triggers and camera.
/// Renderer-free, so headless tests can add it. Safe to add more than once.
#[derive(Default)]
pub struct EngineRulesPlugin;

impl Plugin for EngineRulesPlugin {
    fn build(&self, app: &mut App) {
        add_once(app, schedule::SchedulePlugin);
        add_once(app, animation::AnimationPlugin);
        add_once(app, objects::ObjectsPlugin);
        add_once(app, triggers::TriggersPlugin);
        add_once(app, camera::CameraRulesPlugin);
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// The whole engine: rules plus drawing and sound.
pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EngineRulesPlugin, render::RenderPlugin, audio::AudioPlugin));
    }
}

/// Add `plugin` unless it is already there, so feature plugins can each
/// pull in the engine parts they need.
pub fn add_once<P: Plugin>(app: &mut App, plugin: P) {
    if !app.is_plugin_added::<P>() {
        app.add_plugins(plugin);
    }
}

/// What game code usually needs from the engine.
pub mod prelude {
    pub use crate::config::{TuningPlugin, load_ron};
    pub use crate::schedule::{Avatar, InteractionSystems, SimulationEnabled, WorldSystems, simulation_enabled};
    pub use psx_bevy::prelude::*;
}
