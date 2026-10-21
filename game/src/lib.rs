//! Tomb Raider II on the `engine` crate, as a library: `src/main.rs` opens
//! the window and adds [`GamePlugins`]; tests in `tests/` drive the same
//! plugins headless (`engine::testing`).
//!
//! The engine draws levels, runs floor triggers, the camera and animation.
//! This crate adds what is Tomb Raider II's own: Lara's controller
//! ([`player`], [`braid`]), the object types and trigger actions
//! ([`interactions`], `data/object_types.json`), sounds ([`audio`]), menus
//! and HUD ([`presentation`]) and saves ([`save`]). Rules plugins stay free of
//! rendering so they run in tests.
//!
//! Mods are crates built on this library (see `mods/example`): keep what a
//! mod would reuse `pub`, and each feature plugin usable on its own.
pub mod audio;
pub mod braid;
pub mod caustics;
mod contact;
pub mod creature;
pub mod crow;
pub mod items;
pub mod display;
pub mod interactions;
pub mod player;
pub mod presentation;
pub mod save;
pub mod sky;
pub mod spider;
pub mod subdivision;
pub mod tiger;
pub mod traps;
pub mod trex;
pub mod water_palette;
pub mod winston;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use engine::psx::Region;
use engine::shell::TickRate;

/// Window title.
pub const TITLE: &str = "Tomb Raider II";

/// The original game's video region. It sets the display rate and, with
/// [`FIELDS_PER_TICK`], the logic tick rate. Mega Man 8 and Croc (PAL) use
/// `Region::Pal`. `--region ntsc|pal` overrides it at run time.
pub const REGION: Region = Region::Ntsc;

/// Video fields per logic tick: 1 when the original's main loop runs every
/// vblank, 2 when it waits for two (Croc PAL: 25 Hz logic on a 50 Hz display).
pub const FIELDS_PER_TICK: u32 = 2;

/// Display window: the first 368 columns of the 384x240 projection.
/// Evidence: display setup at 0x80065c48/0x80065c90; docs/original-evidence.md.
pub const SCREEN: UVec2 = UVec2::new(368, 240);

/// The logic tick rate for a region.
pub fn tick_rate(region: Region) -> TickRate {
    TickRate::every_nth_field(region, FIELDS_PER_TICK)
}

/// The whole game: the engine, then each feature's plugin. A group, so a
/// crate built on the game can drop or swap one feature:
/// `GamePlugins.build().disable::<presentation::PresentationPlugin>()`.
///
/// The creature types (`tiger`, `spider`, `winston`) come with
/// [`interactions::InteractionsPlugin`]; a mod changes one with
/// `replace_creature` or `replace_object_type` rather than disabling it.
pub struct GamePlugins;

impl PluginGroup for GamePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(engine::EnginePlugin)
            .add(player::PlayerRulesPlugin)
            .add(interactions::InteractionsPlugin)
            .add(presentation::PresentationPlugin)
            .add(audio::GameAudioPlugin)
            .add(subdivision::SubdivisionPlugin)
            .add(sky::SkyPlugin)
            .add(water_palette::WaterPalettePlugin)
    }
}

/// What game code usually needs from the engine.
pub use engine::prelude;
