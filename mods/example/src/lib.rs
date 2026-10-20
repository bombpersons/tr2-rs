//! An example mod: a small game built on the Tomb Raider II crate by
//! recompiling it, using only `game`'s public API and this crate's hand-made
//! `assets/`.
//!
//! - It keeps the game's rules (object types, triggers, creatures) and drops
//!   its title screen, HUD, sound, sky and water palette
//!   (`GamePlugins.build().disable::<...>()`, see `src/main.rs`).
//! - It brings its own level (`assets/levels/example_mod/`), found through
//!   the layered asset folders ahead of the game's.
//! - Its character is a placement of the game's player type (0), so Lara's
//!   ported controller drives it. The model and its clips are hand-made
//!   (`tools/make_character.py`) and meet the ground-locomotion part of her
//!   asset contract (`docs/lara-contract.md`): stand, walk, run, turn and
//!   stop. [`character_contract`] checks it.
//! - It adds its own trigger action (`chime`).
//! - It replaces one of the game's types: pickup [`PICKUP`] keeps the
//!   game's own hook and also spins ([`replace_object_type`]).
//!
//! [`replace_object_type`]: engine::objects::AppObjectTypes::replace_object_type
use bevy::prelude::*;
use engine::{
    objects::{AppObjectTypes, ObjectTypes},
    prelude::*,
    schedule::ItemSystems,
    triggers::{AppTriggerActions, TriggerAction, TriggerSystems},
    world::Level,
};
use game::player::contract::{self, ContractReport, Feature};

/// The game's asset folder, relative to this crate (see
/// `engine::assets::add_root`). A shipped build points at wherever the
/// game's assets are installed instead.
pub const GAME_ASSETS: &str = "../../game/assets";

/// The mod's level, `assets/levels/example_mod/level.json`.
pub const LEVEL: &str = "example_mod";

/// The game's pickup type the mod changes.
pub const PICKUP: u16 = 135;

/// The parts of Lara's contract the mod's character provides.
pub const CHARACTER_FEATURES: &[Feature] = &[Feature::Ground, Feature::Skeleton];

/// Radians per second.
const SPIN_SPEED: f32 = 1.5;

/// Added to the replaced pickup: it turns in place.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Spin;

/// How often the mod's `chime` action fired.
#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct Chimes(pub u32);

/// The mod's rules: a new action and a replaced type. No rendering, so it
/// runs in headless tests (`tests/rules.rs`). Add it after the game's
/// plugins; the character is the game's own player type.
pub struct ModPlugin;

impl Plugin for ModPlugin {
    fn build(&self, app: &mut App) {
        // The game's own pickup hook, plus a spin.
        let pickup = app
            .world()
            .get_resource::<ObjectTypes>()
            .and_then(|types| types.get(PICKUP).cloned())
            .unwrap_or_else(|| panic!("pickup {PICKUP} is not registered: add the game's plugins first"));
        app.replace_object_type(
            PICKUP,
            pickup.on_init(|entity, _, _| {
                entity.insert(Spin);
            }),
        )
        .register_trigger_action("chime")
        .init_resource::<Chimes>()
        .add_systems(
            FixedUpdate,
            (
                spin.in_set(ItemSystems::Control),
                chime.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
            )
                .run_if(simulation_enabled),
        );
    }
}

/// How the mod's level meets Lara's asset contract. The character provides
/// [`CHARACTER_FEATURES`]; the controller's other moves (jumps, climbing,
/// water...) show the standing pose instead, their clip numbers being filled
/// with standing clips.
pub fn character_contract(level: &Level) -> ContractReport {
    contract::check(level)
}

fn spin(time: Res<Time>, mut spinners: Query<&mut Transform, With<Spin>>) {
    for mut transform in &mut spinners {
        transform.rotate_y(SPIN_SPEED * time.delta_secs());
    }
}

fn chime(mut actions: MessageReader<TriggerAction>, mut chimes: ResMut<Chimes>) {
    for action in actions.read().filter(|a| a.name == "chime") {
        chimes.0 += 1;
        info!("chime {}", action.argument);
    }
}
