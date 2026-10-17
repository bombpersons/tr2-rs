//! A second, tiny game made only from the engine and hand-authored content
//! (`assets/levels/sandbox/`): no Tomb Raider code or data. It keeps the
//! engine honest: if a feature of the port cannot be used here without
//! reaching into the game crate, the engine boundary is in the wrong place.
//!
//! It shows the four things a new game does:
//! 1. registers object types: a walker the player controls (type 1) and a
//!    pillar with a trivial controller that rises once activated (type 2);
//! 2. registers a trigger action of its own (`chime`) and handles it;
//! 3. writes its player's [`Avatar`], so the camera, triggers and room
//!    visibility follow it;
//! 4. authors a level in the engine's format.
//!
//! cargo run -p engine --example sandbox [--features dev-mcp -- --mcp --headless --software]
use bevy::prelude::*;
use engine::{
    EnginePlugin,
    controller::ControllerState,
    objects::{AppObjectTypes, ObjectType, SpawnContext, WorldObject},
    prelude::*,
    launcher::{self, Game},
    render::LevelRequest,
    triggers::{AppTriggerActions, TriggerAction, TriggerSystems},
    world::{WorldGeometry, angle, position},
};

const WALKER: u16 = 1;
const PILLAR: u16 = 2;
/// Metres per second and radians per second.
const WALK_SPEED: f32 = 2.0;
const TURN_SPEED: f32 = 2.5;
/// Highest step the walker climbs without jumping, metres.
const STEP: f32 = 0.3;

/// The sandbox's rules: object types, the walker, the pillar, the chime.
pub struct SandboxPlugin;

impl Plugin for SandboxPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, engine::EngineRulesPlugin);
        app.register_object_type(WALKER, ObjectType::new("walker").on_spawn(spawn_walker))
            .register_object_type(PILLAR, ObjectType::new("pillar").on_init(|entity, _, _| {
                entity.insert(Pillar::default());
            }))
            .register_trigger_action("chime")
            .init_resource::<Chimes>()
            .add_systems(
                FixedUpdate,
                (
                    walk.in_set(WorldSystems::Control),
                    chime.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                    raise_pillars.in_set(WorldSystems::Animation),
                )
                    .run_if(simulation_enabled),
            );
    }
}

/// The player: an avatar that walks on the level's floor.
#[derive(Component, Default)]
#[require(Avatar, Transform)]
struct Walker;

/// Rises one metre over a second once its object is active.
#[derive(Component, Default)]
struct Pillar {
    raised: f32,
}

/// How often a `chime` action fired, and its last argument.
#[derive(Resource, Default, Debug)]
struct Chimes {
    count: u32,
    last: u16,
}

fn spawn_walker(entity: &mut bevy::ecs::system::EntityCommands, ctx: &SpawnContext) {
    let p = ctx.placement;
    let avatar = Avatar {
        position: position(p.position),
        yaw: angle(p.rotation),
        room: p.room,
        grounded: true,
        alive: true,
        ..default()
    };
    entity.insert((Walker, avatar, Name::new("Walker")));
}

/// Turn with left/right, walk with up/down; stay on the floor, climb small
/// steps, stop at walls and solid sectors.
fn walk(
    time: Res<Time>,
    pad: Res<ControllerState>,
    world: Option<Res<WorldGeometry>>,
    mut walkers: Query<(&mut Avatar, &mut Transform), With<Walker>>,
) {
    let dt = time.delta_secs();
    let direction = pad.direction();
    for (mut avatar, mut transform) in &mut walkers {
        avatar.yaw = (avatar.yaw + direction.x * TURN_SPEED * dt).rem_euclid(std::f32::consts::TAU);
        let forward = Vec3::new(avatar.yaw.sin(), 0.0, avatar.yaw.cos());
        let proposed = avatar.position + forward * direction.y * WALK_SPEED * dt;
        let hit = match world.as_deref() {
            Some(world) => world.floor_at(proposed, avatar.room),
            None => None,
        };
        if let Some(hit) = hit
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

fn chime(mut actions: MessageReader<TriggerAction>, mut chimes: ResMut<Chimes>) {
    for action in actions.read().filter(|a| a.name == "chime") {
        chimes.count += 1;
        chimes.last = action.argument;
        info!("chime {}", action.argument);
    }
}

fn raise_pillars(time: Res<Time>, mut pillars: Query<(&WorldObject, &mut Pillar, &mut Transform)>) {
    for (object, mut pillar, mut transform) in &mut pillars {
        if object.active && pillar.raised < 1.0 {
            let step = time.delta_secs().min(1.0 - pillar.raised);
            pillar.raised += step;
            transform.translation.y += step;
        }
    }
}

fn main() {
    launcher::run(Game {
        name: "engine-sandbox",
        title: "Engine sandbox",
        region: Region::Ntsc,
        fields_per_tick: 2,
        add: |app| {
            app.add_plugins((EnginePlugin, SandboxPlugin)).insert_resource(LevelRequest { name: "sandbox".into() });
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{
        camera::CameraState,
        controller::Button,
        objects::WorldObject,
        testing::{TickApp, headless_app},
        world::Level,
    };

    fn sandbox() -> (App, Entity, Entity) {
        let level: Level = serde_json::from_str(include_str!("../assets/levels/sandbox/level.json")).unwrap();
        let mut app = headless_app(TickRate::every_nth_field(Region::Ntsc, 2));
        app.add_plugins(SandboxPlugin).insert_resource(WorldGeometry::new(level));
        // What the renderer's level spawn does for these two placements.
        let walker = app
            .world_mut()
            .spawn((Walker, Avatar { position: position([2560, 0, 2560]), grounded: true, alive: true, ..default() }))
            .id();
        let pillar = app.world_mut().spawn((WorldObject::new(1, PILLAR, 0, 0), Transform::from_translation(position([4608, 0, 4608])))).id();
        (app, walker, pillar)
    }

    fn avatar(app: &App, walker: Entity) -> Avatar {
        *app.world().get::<Avatar>(walker).unwrap()
    }

    #[test]
    fn walker_climbs_the_step_and_stops_at_the_wall() {
        let (mut app, walker, _) = sandbox();
        // Facing +z from sector (2, 2); turn to face +x and walk onto the
        // raised sector (4, 2), then on into the wall at x = 5 m.
        app.run_ticks(19, &[Button::Right]);
        let yaw = avatar(&app, walker).yaw;
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.1, "{yaw}");
        app.run_ticks(60, &[Button::Up]);
        let a = avatar(&app, walker);
        assert!(a.position.x < 5.0 && a.position.x > 4.0, "{a:?}");
        assert_eq!(a.position.y, 0.25, "standing on the step");
    }

    #[test]
    fn trigger_raises_the_pillar_and_chimes_once() {
        let (mut app, walker, pillar) = sandbox();
        app.world_mut().get_mut::<Avatar>(walker).unwrap().position = position([3584, 0, 3584]);
        app.run_ticks(40, &[]);
        assert!(app.world().get::<WorldObject>(pillar).unwrap().active);
        assert!(app.world().get::<Pillar>(pillar).unwrap().raised > 0.99);
        let chimes = app.world().resource::<Chimes>();
        assert_eq!((chimes.count, chimes.last), (1, 3), "a one-shot trigger");
    }

    #[test]
    fn camera_cuts_to_the_fixed_shot_while_on_its_sector() {
        let (mut app, walker, _) = sandbox();
        app.run_tick(&[]);
        assert_eq!(app.world().resource::<CameraState>().fixed_camera, None);
        app.world_mut().get_mut::<Avatar>(walker).unwrap().position = position([1536, 0, 4608]);
        app.run_tick(&[]);
        assert_eq!(app.world().resource::<CameraState>().fixed_camera, Some(0));
    }
}
