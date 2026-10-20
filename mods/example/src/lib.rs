//! An example mod: a small game built on the Tomb Raider II crate by
//! recompiling it, using only `game`'s public API and this crate's hand-made
//! `assets/`.
//!
//! - It keeps the game's rules (object types, triggers, creatures) and drops
//!   its title screen, HUD, sound, sky and water palette
//!   (`GamePlugins.build().disable::<...>()`, see `src/main.rs`).
//! - It brings its own level (`assets/levels/example_mod/`), found through
//!   the layered asset folders ahead of the game's.
//! - It adds an object type, a wanderer the player steers ([`WANDERER`]),
//!   and its own trigger action (`chime`).
//! - It replaces one of the game's types: pickup [`PICKUP`] keeps the
//!   game's own hook and also spins ([`replace_object_type`]).
//!
//! Not yet: the game's player controller driving a hand-made character. That
//! needs Lara's asset contract written down first (her clip, state and
//! command numbers); see `TODO.md`.
//!
//! [`replace_object_type`]: engine::objects::AppObjectTypes::replace_object_type
use bevy::prelude::*;
use engine::{
    controller::ControllerState,
    objects::{AppObjectTypes, ObjectType, ObjectTypes, SpawnContext},
    prelude::*,
    schedule::ItemSystems,
    triggers::{AppTriggerActions, TriggerAction, TriggerSystems},
    world::{WorldGeometry, angle, position},
};

/// The game's asset folder, relative to this crate (see
/// `engine::assets::add_root`). A shipped build points at wherever the
/// game's assets are installed instead.
pub const GAME_ASSETS: &str = "../../game/assets";

/// The mod's level, `assets/levels/example_mod/level.json`.
pub const LEVEL: &str = "example_mod";

/// The mod's own type: a wanderer the player steers. Mod type ids start at
/// 300, clear of the original's 265.
pub const WANDERER: u16 = 300;

/// The game's pickup type the mod changes.
pub const PICKUP: u16 = 135;

/// Metres per second and radians per second.
const WALK_SPEED: f32 = 2.0;
const TURN_SPEED: f32 = 2.5;
/// Highest step the wanderer climbs, metres.
const STEP: f32 = 0.3;
/// Radians per second.
const SPIN_SPEED: f32 = 1.5;

/// The player's character: an avatar that walks on the level's floor, so the
/// engine's camera, triggers and room visibility follow it.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
#[require(Avatar, Transform)]
pub struct Wanderer;

/// Added to the replaced pickup: it turns in place.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Spin;

/// How often the mod's `chime` action fired.
#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct Chimes(pub u32);

/// The mod's rules: a new type, a new action and a replaced type. No
/// rendering, so it runs in headless tests (`tests/rules.rs`). Add it after
/// the game's plugins.
pub struct ModPlugin;

impl Plugin for ModPlugin {
    fn build(&self, app: &mut App) {
        // The game's own pickup hook, plus a spin.
        let pickup = app
            .world()
            .get_resource::<ObjectTypes>()
            .and_then(|types| types.get(PICKUP).cloned())
            .unwrap_or_else(|| panic!("pickup {PICKUP} is not registered: add the game's plugins first"));
        app.register_object_type(WANDERER, ObjectType::new("wanderer").on_spawn(spawn_wanderer))
            .replace_object_type(
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
                    wander.in_set(WorldSystems::Control),
                    spin.in_set(ItemSystems::Control),
                    chime.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                )
                    .run_if(simulation_enabled),
            );
    }
}

/// The wanderer's spawn hook.
pub fn spawn_wanderer(entity: &mut bevy::ecs::system::EntityCommands, ctx: &SpawnContext) {
    let p = ctx.placement;
    let avatar = Avatar {
        position: position(p.position),
        yaw: angle(p.rotation),
        room: p.room,
        grounded: true,
        alive: true,
        ..default()
    };
    entity.insert((Wanderer, avatar, Transform::from_translation(position(p.position)), Name::new("Wanderer")));
}

/// Turn with left/right, walk with up/down; stay on the floor, climb small
/// steps, stop at walls and solid sectors.
fn wander(
    time: Res<Time>,
    pad: Res<ControllerState>,
    world: Option<Res<WorldGeometry>>,
    mut wanderers: Query<(&mut Avatar, &mut Transform), With<Wanderer>>,
) {
    let dt = time.delta_secs();
    let direction = pad.direction();
    for (mut avatar, mut transform) in &mut wanderers {
        avatar.yaw = (avatar.yaw + direction.x * TURN_SPEED * dt).rem_euclid(std::f32::consts::TAU);
        let forward = Vec3::new(avatar.yaw.sin(), 0.0, avatar.yaw.cos());
        let proposed = avatar.position + forward * direction.y * WALK_SPEED * dt;
        if let Some(hit) = world.as_deref().and_then(|world| world.floor_at(proposed, avatar.room))
            && hit.height - avatar.position.y <= STEP
            && hit.ceiling - hit.height >= 0.5
        {
            avatar.position = proposed.with_y(hit.height);
            avatar.room = hit.room;
        }
        transform.translation = avatar.position;
        transform.rotation = Quat::from_rotation_y(avatar.yaw);
    }
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
