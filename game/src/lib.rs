//! Tomb Raider II on the `engine` crate, as a library: `src/main.rs` opens
//! the window and adds [`GamePlugin`]; tests in `tests/` drive the same
//! plugins headless (`engine::testing`).
//!
//! The engine draws levels, runs floor triggers, the camera and animation.
//! This crate adds what is Tomb Raider II's own: Lara's controller
//! ([`player`], [`braid`]), the object types and trigger actions
//! ([`interactions`], `data/object_types.json`), sounds ([`audio`]), menus
//! and HUD ([`presentation`]) and saves ([`save`]). Rules plugins stay free of
//! rendering so they run in tests.
pub mod audio;
pub mod braid;
pub mod caustics;
pub mod display;
pub mod interactions;
pub mod player;
pub mod presentation;
pub mod save;
pub mod sky;
pub mod subdivision;
pub mod water_palette;
pub mod winston;

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

/// The whole game: add each feature's plugin here.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            engine::EnginePlugin,
            player::PlayerRulesPlugin,
            interactions::InteractionsPlugin,
            presentation::PresentationPlugin,
            audio::GameAudioPlugin,
            subdivision::SubdivisionPlugin,
            sky::SkyPlugin,
            water_palette::WaterPalettePlugin,
        ));
    }
}

/// What game code usually needs from the engine.
pub use engine::prelude;
