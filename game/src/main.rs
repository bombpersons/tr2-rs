//! The executable. Window, renderer and command line come from the engine's
//! launcher (`engine::launcher`, see its docs for the flags); the game itself
//! is the library in `src/lib.rs` (crate `game`).
//!
//! Extra flags: `--level NAME` starts in `assets/levels/NAME/` without the
//! title (default: the title, `src/presentation.rs`). `--level-file PATH`
//! (relative to `assets/`) loads that level from another file, such as the
//! editor's play-test copy; its atlas, sky and audio stay the level's.
//! `--disc PATH` imports from that disc image, `--skip-import` starts
//! without importing (see `src/disc.rs`).
use bevy::app::PluginGroup;
use engine::launcher::{self, Args, Game};

fn main() {
    // The data imported from the player's disc, layered under assets/.
    engine::assets::add_root(game::disc::IMPORTED);
    launcher::run(Game {
        name: env!("CARGO_PKG_NAME"),
        title: game::TITLE,
        region: game::REGION,
        fields_per_tick: game::FIELDS_PER_TICK,
        screen: game::SCREEN,
        add: |app| {
            let args = Args::from_env();
            let level = args.value("--level");
            if let (Some(name), Some(path)) = (level.clone(), args.value("--level-file")) {
                app.insert_resource(engine::render::LevelFile { name, path });
            }
            if let Some(name) = &level {
                let table = game::inventory::ItemTable::default();
                app.insert_resource(game::inventory::Inventory::start_at(&table, name));
            }
            app.insert_resource(game::display::view_settings())
                .insert_resource(game::presentation::Session { title: level.is_none(), ..Default::default() })
                .insert_resource(game::presentation::LevelRequest { name: level.unwrap_or_default() })
                // Imports the game's data from the player's disc on first start.
                .add_plugins(game::GamePlugins.set(game::disc::GameDataPlugin::from_args()))
                // Mouse look and right-stick camera, on the controls menu's CAMERA page.
                .add_plugins(engine::camera::FreeCameraMenuPlugin {
                    path: engine::camera::FreeCameraMenuPlugin::default_path(env!("CARGO_PKG_NAME")),
                });
        },
    });
}
