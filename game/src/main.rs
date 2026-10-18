//! The executable. Window, renderer and command line come from the engine's
//! launcher (`engine::launcher`, see its docs for the flags); the game itself
//! is the library in `src/lib.rs` (crate `game`).
//!
//! Extra flag: `--level NAME` starts in `assets/levels/NAME/` without the
//! title screen (default: the title screen, then `wall`).
use engine::launcher::{self, Args, Game};

fn main() {
    launcher::run(Game {
        name: env!("CARGO_PKG_NAME"),
        title: game::TITLE,
        region: game::REGION,
        fields_per_tick: game::FIELDS_PER_TICK,
        screen: game::SCREEN,
        add: |app| {
            let level = Args::from_env().value("--level");
            app.insert_resource(game::display::view_settings())
                .insert_resource(game::presentation::Session { title: level.is_none(), ..Default::default() })
                .insert_resource(game::presentation::LevelRequest { name: level.unwrap_or_else(|| "wall".into()) })
                .add_plugins(game::GamePlugin);
        },
    });
}
