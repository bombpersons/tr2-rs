//! The mod's executable: the game's launcher with the game's feature plugins
//! minus its title screen, HUD, sound, sky and water palette, plus the mod's
//! own (`src/lib.rs`), starting in the mod's level.
use bevy::app::PluginGroup;
use engine::launcher::{self, Game};

fn main() {
    // The game's assets, layered under this crate's own assets/ (which wins).
    engine::assets::add_root(example_mod::GAME_ASSETS);
    // And what the game imported from the player's disc, if anything.
    engine::assets::add_root(format!("{}/imported", example_mod::GAME_ASSETS));
    launcher::run(Game {
        name: env!("CARGO_PKG_NAME"),
        title: "Example mod: arrows to walk",
        region: game::REGION,
        fields_per_tick: game::FIELDS_PER_TICK,
        screen: game::SCREEN,
        add: |app| {
            app.insert_resource(game::display::view_settings())
                .insert_resource(engine::render::LevelRequest { name: example_mod::LEVEL.into() })
                .add_plugins(
                    game::GamePlugins
                        .build()
                        .disable::<game::presentation::PresentationPlugin>()
                        .disable::<game::audio::GameAudioPlugin>()
                        .disable::<game::sky::SkyPlugin>()
                        .disable::<game::water_palette::WaterPalettePlugin>(),
                )
                .add_plugins(example_mod::ModPlugin);
        },
    });
}
