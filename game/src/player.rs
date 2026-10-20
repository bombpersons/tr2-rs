//! Lara's gameplay rules, independent of rendering and disc access.
//!
//! Distances use Bevy's y-up coordinates, with one unit = 1024 original units.
//! Speeds are per second; extracted animation velocities and jump commands use
//! 30 game frames per second at their conversion boundary. The locomotion state
//! machine is a reconstruction, not a cycle-exact copy of the original controller.
//! Clip numbers (6 run, 20 walk, 103 stand...) index the levels' shared Lara
//! animation table.
use bevy::prelude::*;
pub use engine::animation::AnimationState;
use engine::animation::{
    ANIMATION_FPS as ORIGINAL_FPS, AnimationGoal, AnimationLibrary, ObjectAnimation, advance,
    advance_authored_animation, animation_frames as original_frames,
};
use engine::camera::CameraSystems;
use engine::controller::{Button, ControllerState};
pub use engine::schedule::SimulationEnabled;
use engine::schedule::{Avatar, InteractionSystems, SchedulePlugin, WorldSystems};
pub use engine::world::units;
use engine::world::{FloorHit, WorldGeometry};

mod traversal;
pub mod using;
pub use traversal::Traversal;
mod backward;
mod collision;
pub(crate) use collision::{CollisionFrame, item_push_is_clear};
pub mod contract;
use contract::{clip, state};
mod forward;
mod rolling;
mod surface;
mod swimming;
mod wading;
pub use swimming::{SubmergedMotion, SurfaceTuning, SwimmingTuning};
pub use wading::{Wading, WadingTuning};
pub mod arms;
pub mod weapons;
pub use weapons::{Arm, Gun, GunHit, GunSound, GunStatus, GunTuning, WeaponState};

/// The level animation table, as Lara's rules read it.
pub type PlayerAnimations = AnimationLibrary;
/// WALL.PSX animation 0: signed 16.16 speed 3080192 = 47 units/frame.
pub const RUN_SPEED: f32 = units(47.0 * ORIGINAL_FPS);
/// WALL.PSX animation 1: signed 16.16 speed 983040 = 15 units/frame.
pub const WALK_SPEED: f32 = units(15.0 * ORIGINAL_FPS);
/// WALL.PSX animation 40 (backward walk): 10 units/frame.
pub const BACK_SPEED: f32 = units(10.0 * ORIGINAL_FPS);
/// WALL.PSX animations 65/67: 9 units/frame.
pub const STEP_SPEED: f32 = units(9.0 * ORIGINAL_FPS);
/// Standing jump command 2 in WALL.PSX animation 76: vertical -100, forward 50.
pub const JUMP_SPEED: f32 = units(100.0 * ORIGINAL_FPS);
pub const JUMP_FORWARD_SPEED: f32 = units(50.0 * ORIGINAL_FPS);
/// Running jump command 2 in WALL.PSX animation 16: vertical -100, forward 75.
pub const RUN_JUMP_SPEED: f32 = units(75.0 * ORIGINAL_FPS);
/// AnimateLara0x80043210..0x80043230 adds6 units/tick below128 fall speed,
/// otherwise1. Fresh airborne recordings independently verify the6 branch.
pub const GRAVITY: f32 = units(6.0 * ORIGINAL_FPS * ORIGINAL_FPS);
/// PSX object-push world validation0x8001CC00 passes a762-unit body height.
/// Lara's forward slide state.
const SLIDE_STATE: usize = state::SLIDE;
pub const PLAYER_HEIGHT: f32 = units(762.0);
/// PSX Lara collision context0x800381E8..0x800381EC starts with radius100.
pub const PLAYER_RADIUS: f32 = units(100.0);
/// Inherited Character::stepHeight value; unchanged by contact verification.
pub const STEP_HEIGHT: f32 = units(256.0);
pub const MAX_HEALTH: i32 = 1000;
/// Original Lara control 0x80038194: state handlers push one turn rate by
/// 409 yaw units/tick, then it decays 364 toward zero and is added to yaw.
const TURN_PUSH: i32 = 409;
const TURN_DECAY: i32 = 364;

#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Motion, AnimationState, WeaponState, Transform, Avatar, Traversal, SubmergedMotion, Wading, CollisionFrame)]
pub struct Player {
    /// Feet position; rendering synchronizes this into Transform.
    pub position: Vec3,
    /// Radians; zero faces +Z, positive turns toward +X.
    pub yaw: f32,
    pub room: usize,
    pub health: i32,
    pub mode: PlayerMode,
}

impl Player {
    pub fn at(position: Vec3, yaw: f32, room: usize) -> Self {
        Self { position, yaw, room, health: MAX_HEALTH, mode: PlayerMode::Idle }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    pub fn damage(&mut self, amount: i32) {
        self.health = self.health.saturating_sub(amount.max(0)).max(0);
        if self.health == 0 {
            self.mode = PlayerMode::Dead;
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::at(Vec3::ZERO, 0.0, 0)
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Default)]
pub enum PlayerMode {
    #[default]
    Idle,
    Walk,
    /// Authored shallow-water forward gait (state65); contact lives in Wading.
    Wade,
    Run,
    /// State16: authored backward walk, including its start and stop clips.
    Back,
    /// State5: a complete backward hop; releasing Down does not cancel it.
    HopBack,
    /// Back and side flight: the hop-back fall (state29), back jump (25) and
    /// right/left jumps (26/27), through their landing dispatch.
    FallingBack,
    /// States22/21: finish the current stride and stop pose after release.
    StepLeft {
        stopping: bool,
    },
    StepRight {
        stopping: bool,
    },
    Sliding {
        direction: Vec3,
        backwards: bool,
    },
    SlideStopping {
        direction: Vec3,
        seconds_left: f32,
    },
    PreparingJump {
        launch_velocity: Vec3,
        frames_left: f32,
        standing: bool,
        running: bool,
    },
    Jump,
    Fall,
    Landing {
        seconds_left: f32,
        animation: usize,
        start_frame: f32,
    },
    Hanging {
        ledge: Vec3,
    },
    /// A vertical ledge jump, including its authored preparation and drop.
    JumpingUp {
        launch_speed: f32,
    },
    /// Forward flight, including running off a ledge, and authored reach/grab.
    JumpingForward,
    /// State9: retain the collision heading, brake in air and use the heavy landing.
    FastFalling,
    /// A fatal fast-fall contact dispatches into the authored ground death pose.
    FatalLanding,
    /// State14: a low airborne impact locks clip31 through its authored end.
    JumpImpact,
    /// Authored pull-up clips apply their own end offsets, then settle.
    PullingUp,
    /// A landing whose authored ranges determine when control can resume.
    Recovering,
    /// A front-wall/ledge stop retains standing control through walking stop2/3.
    WalkStopping,
    /// Position is frozen while the clip plays; `offset` (world units) is
    /// applied when it ends, as the clip's end-of-animation command.
    Climbing {
        animation: usize,
        frames_left: f32,
        offset: Vec3,
    },
    /// State12: wall-hit53/54, or a standing stop outside their foot ranges.
    HitWall {
        animation: usize,
        frames_left: f32,
    },
    /// A committed authored roll; entry starts clip146 at its third frame.
    Rolling {
        entering: bool,
    },
    /// Using an object: a pickup, switch, key or puzzle clip plays in
    /// place until it ends into standing ([`using`]).
    Using {
        animation: usize,
    },
    /// Submerged: stroke, glide, tread and the dive (water mode 1).
    Swimming,
    /// On the water surface: tread and the four strokes (water mode 2).
    Surface,
    Dead,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Motion {
    /// World units/second, upward positive.
    pub velocity: Vec3,
    pub grounded: bool,
    /// Underwater pitch, positive upward. Separate from the tank heading.
    pub pitch: f32,
    pub oxygen_seconds: f32,
    /// Original animation-time frames since entering this locomotion mode.
    pub locomotion_frame: f32,
    /// Original turn rate (global 0x8008c570), yaw units/tick, after decay.
    pub turn_rate: i32,
    /// Standing turn state direction: -1 left (state7), +1 right (state6/20).
    pub turning: i32,
    pub turn_frames: f32,
    /// Original airborne animation family, retained through the jump's descent.
    pub air_animation: usize,
    /// Heading written by the preceding collision pass (0x8008C572).
    /// Backward entry retains it after a forced stop; fast fall retains
    /// the rebound heading independently of the player's facing yaw.
    pub movement_heading: Option<f32>,
    /// Guns out (gun status 4) when this tick's state handler ran. The turn
    /// handlers then ask for the fast turn at once (0x80038DC4/0x80038E94).
    pub guns_ready: bool,
    /// The slide handler 0x80039598 (state 24) sets camera flags 2 when it
    /// runs, so the camera reads slopes exactly this tick.
    pub camera_exact_slopes: bool,
    /// State camera elevation request, radians, cleared before each control tick.
    pub camera_elevation: Option<f32>,
    /// State camera angle, distance and speed requests, cleared with it.
    pub camera_angle: i16,
    pub camera_distance: Option<i32>,
    pub camera_speed: Option<i32>,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            grounded: true,
            pitch: 0.0,
            oxygen_seconds: 60.0,
            locomotion_frame: 0.0,
            turn_rate: 0,
            turning: 0,
            turn_frames: 0.0,
            air_animation: 0,
            movement_heading: None,
            guns_ready: false,
            camera_exact_slopes: false,
            camera_elevation: None,
            camera_angle: 0,
            camera_distance: None,
            camera_speed: None,
        }
    }
}

/// Landing damage0x8003E044: no damage through140 units/tick, quadratic
/// through154 and lethal above154. PSX controlled impacts are retained in
/// docs/fall-damage-evidence.md; >154 writes the original negative sentinel.
fn landing_damage(player: &mut Player, velocity: Vec3) {
    let impact = (-velocity.y * 1024.0 / ORIGINAL_FPS).round() as i32;
    if impact > 140 {
        if impact > 154 {
            player.health = -1; // Lethal sentinel at0x8003E0CC..0x8003E0D4.
            player.mode = PlayerMode::Dead;
        } else {
            player.damage((impact - 140).pow(2) * MAX_HEALTH / 196);
        }
    }
}

/// Lara's freefall clips belong to state 3; landing asks for goal state 2
/// (standing) through their authored ranges.
fn freefall_landing(clips: &PlayerAnimations, animation: &AnimationState) -> Option<(usize, f32)> {
    if clips.state(animation.animation)? != state::FORWARD_JUMP {
        return None;
    }
    clips.dispatch(animation, state::STAND)
}

/// Lara's systems, inside the engine's tick order.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum PlayerSystems {
    Control,
    Motion,
    Animation,
    /// LaraGun, after object collision and room update and before the floor
    /// triggers (0x8003844C, docs/tick-order-evidence.md).
    Weapons,
}

use engine::schedule::simulation_enabled;

/// Publishes Lara as the engine's [`Avatar`]: the engine reads her through
/// it, so it is refreshed before each read.
pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, SchedulePlugin);
        // Lara's standing clip, which a falling start first frames.
        app.insert_resource(engine::camera::CameraStartup { standing_clip: Some(STANDING_CLIP) });
        app.add_systems(
            FixedUpdate,
            (
                publish_avatar.in_set(WorldSystems::Prepare).before(CameraSystems::Initialize),
                publish_avatar.in_set(InteractionSystems::Publish),
                publish_avatar.in_set(WorldSystems::Camera).before(CameraSystems::Update),
            ),
        )
        // Drawing reads it every frame, including while ticks are paused.
        .add_systems(Update, publish_avatar.before(engine::render::LevelSystems::Pose));
    }
}

pub struct PlayerRulesPlugin;

impl Plugin for PlayerRulesPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, AvatarPlugin);
        app.register_type::<Traversal>().register_type::<Wading>();
        app.insert_resource(crate::interactions::swimming_tuning());
        app.insert_resource(crate::interactions::pistol_tuning());
        app.add_message::<GunSound>().add_message::<GunHit>().register_type::<WeaponState>();
        app.init_resource::<engine::random::GameRandom>();
        app.configure_sets(FixedUpdate, PlayerSystems::Control.in_set(WorldSystems::Control))
            .configure_sets(FixedUpdate, PlayerSystems::Motion.in_set(WorldSystems::Motion))
            .configure_sets(FixedUpdate, PlayerSystems::Animation.in_set(WorldSystems::Animation))
            .configure_sets(FixedUpdate, PlayerSystems::Weapons.in_set(InteractionSystems::Combat))
            .add_systems(
                FixedUpdate,
                collision::remember
                    .in_set(WorldSystems::Control)
                    .before(PlayerSystems::Control)
                    .run_if(simulation_enabled),
            )
            .add_systems(FixedUpdate, control.in_set(PlayerSystems::Control).run_if(simulation_enabled))
            .add_systems(
                FixedUpdate,
                rolling::collide
                    .in_set(WorldSystems::Interaction)
                    .after(InteractionSystems::Resolve)
                    .before(InteractionSystems::Publish)
                    .run_if(simulation_enabled),
            )
            .add_systems(
                FixedUpdate,
                (integrate, apply_turn_rate).chain().in_set(PlayerSystems::Motion).run_if(simulation_enabled),
            )
            .add_systems(
                FixedUpdate,
                (animate, using::finish).chain().in_set(PlayerSystems::Animation).run_if(simulation_enabled),
            )
            .add_systems(FixedUpdate, update_guns.in_set(PlayerSystems::Weapons).run_if(simulation_enabled))
            .add_systems(FixedUpdate, sync_transform.in_set(InteractionSystems::Publish).run_if(simulation_enabled));
    }
}

/// LaraGun for one tick: Draw and Action are read held (pistols_held_draw).
#[allow(clippy::too_many_arguments)]
fn update_guns(
    pad: Res<ControllerState>,
    tuning: Res<GunTuning>,
    world: Option<Res<WorldGeometry>>,
    mut random: ResMut<engine::random::GameRandom>,
    mut sounds: MessageWriter<GunSound>,
    mut hits: MessageWriter<GunHit>,
    mut players: Query<(&Player, &mut WeaponState)>,
    creatures: Query<(
        Entity,
        &crate::interactions::Enemy,
        &engine::objects::WorldObject,
        &Transform,
        &ObjectAnimation,
    )>,
) {
    let input = weapons::GunInput::read(&pad);
    let targets = world.as_deref().map_or_else(Vec::new, |world| gun_targets(world, &creatures));
    let line_of_sight = |from: IVec3, room: usize, to: IVec3, to_room: usize| {
        use engine::camera::chase::{Point, Space, line_of_sight};
        let Some(world) = world.as_deref() else { return true };
        let start = Point { x: from.x, y: from.y, z: from.z, room };
        let mut end = Point { x: to.x, y: to.y, z: to.z, room: to_room };
        line_of_sight(Space { world, camera: false }, start, &mut end)
    };
    for (player, mut weapon) in &mut players {
        let gun_world = weapons::GunWorld {
            position: original_units(player.position),
            room: player.room,
            yaw: (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16,
            targets: &targets,
            line_of_sight: &line_of_sight,
        };
        let alive = player.health > 0;
        let tick = weapons::update(
            &mut weapon,
            input,
            alive,
            weapons::can_draw(player),
            &tuning,
            &gun_world,
            &mut random,
            &mut |sound| {
                sounds.write(GunSound(sound));
            },
        );
        hits.write_batch(tick.hits);
    }
}

/// A level position in the original's integer units (y down).
pub(crate) fn original_units(position: Vec3) -> IVec3 {
    let v = (position * 1024.0).round();
    IVec3::new(v.x as i32, -v.y as i32, v.z as i32)
}

/// Active creatures as the guns see them (the creature slots at
/// 0x8008C1C0 hold active creatures; entity order stands in for slot order).
fn gun_targets(
    world: &WorldGeometry,
    creatures: &Query<(
        Entity,
        &crate::interactions::Enemy,
        &engine::objects::WorldObject,
        &Transform,
        &ObjectAnimation,
    )>,
) -> Vec<weapons::GunTarget> {
    use engine::animation::{collision_bounds, model_spheres};
    use engine::camera::chase::{cos, sin};
    creatures
        .iter()
        .filter(|(_, _, object, _, _)| object.active && !object.consumed)
        .filter_map(|(entity, enemy, object, transform, animation)| {
            let position = original_units(transform.translation);
            let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            let yaw = (yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
            // 0x80040C40: centre of the nearest key's box, a third down from its top.
            let b = collision_bounds(&world.data, animation.animation, animation.frame)?;
            let (cx, cz) = ((b[0] + b[1]) / 2, (b[4] + b[5]) / 2);
            let aim_point = IVec3::new(
                position.x + ((cos(yaw) * cx + sin(yaw) * cz) >> 14),
                position.y + b[2] + (b[3] - b[2]) / 3,
                position.z + ((cos(yaw) * cz - sin(yaw) * cx) >> 14),
            );
            let spheres = world.data.model(object.kind).map_or_else(Vec::new, |model| {
                model_spheres(&world.data, model, animation.animation, animation.frame, transform)
                    .into_iter()
                    .map(|s| (original_units(s.center), (s.radius * 1024.0).round() as i32))
                    .collect()
            });
            Some(weapons::GunTarget {
                entity,
                position,
                aim_point,
                room: object.room,
                alive: enemy.health > 0,
                spheres,
            })
        })
        .collect()
}

/// Lara's object type id: her placement is where the level starts.
pub const LARA: u16 = 0;
/// Lara's standing clip (state 2).
pub const STANDING_CLIP: usize = clip::STAND;

/// Build Lara from her placement (the object registry's spawn hook). A
/// spawn in mid-air starts falling: native Wall initializes clip34/frame492
/// before its first field, and the next tick advances to 493 rather than
/// reselecting frame 492.
pub fn spawn_lara(entity: &mut bevy::ecs::system::EntityCommands, ctx: &engine::objects::SpawnContext) {
    use engine::world::{angle, position};
    let e = ctx.placement;
    let mut player = Player::at(position(e.position), angle(e.rotation), e.room);
    let grounded = ctx
        .world
        .floor_at(player.position, player.room)
        .is_some_and(|hit| (hit.height - player.position.y).abs() < 0.05);
    // Mid-air, that clip is forward flight's state3 with goal3.
    if !grounded {
        player.mode = PlayerMode::JumpingForward;
    }
    let animation = if grounded { AnimationState::default() } else { AnimationState::at_frame(clip::FALL, 0.0) };
    let gun = if weapons::UNARMED_LEVELS.contains(&ctx.world.data.name.as_str()) { Gun::Unarmed } else { Gun::Pistols };
    entity.insert((
        player,
        Motion { grounded, ..default() },
        animation,
        WeaponState::holding(gun),
        Traversal { goal: if grounded { state::STAND } else { state::FORWARD_JUMP }, ..default() },
        Name::new("Player"),
    ));
    crate::braid::spawn(entity, ctx);
}

/// Copy Lara's state into the engine's [`Avatar`].
pub fn publish_avatar(mut players: Query<(&Player, &Motion, &WeaponState, &AnimationState, &mut Avatar)>) {
    for (player, motion, weapon, animation, mut avatar) in &mut players {
        avatar.set_if_neq(Avatar {
            position: player.position,
            yaw: player.yaw,
            room: player.room,
            grounded: motion.grounded,
            // A mid-air spawn plays forward flight's fall clip34.
            falling: player.mode == PlayerMode::Fall || (!motion.grounded && animation.animation == clip::FALL),
            armed: weapon.ready(),
            // LaraGun asks for the combat camera while drawing or ready
            // (0x8004001C, 0x800401B8), looking at the target or along the aim.
            aim: weapon.aiming_camera().then(|| weapon.camera_aim()),
            camera_exact_slopes: motion.camera_exact_slopes,
            camera_elevation: motion.camera_elevation,
            camera_angle: motion.camera_angle,
            camera_distance: motion.camera_distance,
            camera_speed: motion.camera_speed,
            alive: player.health > 0,
        });
    }
}

struct Vault {
    animation: usize,
    yaw: f32,
    /// Bevy y such that the clip's end command lands on the front floor.
    height: f32,
    jump_speed: Option<f32>,
}

/// Original vault test 0x8003d924, using GetCollisionInfo 0x8001b018 probes:
/// front = radius along the facing quadrant's axis plus the trigonometric
/// offset across it; left/right = the quadrant's two forward corners.
fn vault(world: Option<&WorldGeometry>, position: Vec3, yaw: f32, room: usize) -> Option<Vault> {
    let world = world?;
    let angle = (yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
    let quadrant = ((angle + 8192) >> 14).rem_euclid(4);
    let delta = (angle - quadrant * 16384 + 32768).rem_euclid(65536) - 32768;
    if delta.abs() > 5460 {
        return None;
    }
    let r = 100;
    let across = |trig: f32| ((trig * 16384.0).trunc() as i32 * r) >> 14;
    let (sin, cos) = yaw.sin_cos();
    let (front, left, right) = match quadrant {
        0 => ([across(sin), r], [-r, r], [r, r]),
        1 => ([r, across(cos)], [r, r], [r, -r]),
        2 => ([across(sin), -r], [r, -r], [-r, -r]),
        _ => ([-r, across(cos)], [-r, -r], [-r, r]),
    };
    let base = (position * 1024.0).round();
    let probe = |[dx, dz]: [i32; 2]| {
        let hit = world.floor_at(Vec3::new(base.x + dx as f32, base.y, base.z + dz as f32) / 1024.0, room)?;
        // Original y-down floor relative to Lara, and clearance under the ceiling.
        let relative = ((position.y - hit.height) * 1024.0).round() as i32;
        Some((relative, hit.ceiling - hit.height >= PLAYER_HEIGHT))
    };
    let (front, front_clear) = probe(front)?;
    let (left, left_clear) = probe(left)?;
    let (right, right_clear) = probe(right)?;
    if (left - right).abs() >= 60 || !(front_clear && left_clear && right_clear) {
        return None;
    }
    let (animation, climb, jump_speed) = match front {
        -640..=-384 => (clip::VAULT_TWO, 512, None),
        -896..=-641 => (clip::VAULT_THREE, 768, None),
        // Vault test 0x8003DBB0: a higher flat ledge requests the
        // authored vertical jump, overriding its end command's lift.
        -1920..=-897 => {
            (clip::JUMP_UP_PREPARE, 0, Some(units((((front + 800) * -12) as f32).sqrt().floor() + 3.0) * ORIGINAL_FPS))
        }
        _ => return None,
    };
    Some(Vault {
        animation,
        yaw: quadrant as f32 * std::f32::consts::FRAC_PI_2,
        height: if jump_speed.is_some() { position.y } else { position.y - (front + climb) as f32 / 1024.0 },
        jump_speed,
    })
}

/// Collision shift out of a wall along one axis: radius + 1 from the sector
/// edge. Positive X is recorded in climb_two_click.csv; negative Z in
/// traversal_three_click.csv (z92261, exactly 101 past the z92160 edge).
fn wall_contact(old: f32, proposed: f32) -> f32 {
    let (old, proposed) = ((old * 1024.0).round() as i32, (proposed * 1024.0).round() as i32);
    let contact = if proposed > old {
        (proposed + 100).div_euclid(1024) * 1024 - 101
    } else {
        ((proposed - 100).div_euclid(1024) + 1) * 1024 + 101
    };
    // The shift can push back beyond the old coordinate: forward_climb.csv
    // reaches z88164, then collision restores the101-unit margin at88165.
    contact as f32 / 1024.0
}

/// Wall test 0x8003c91c(256, 0, -640): solid, or no clearance, 256 ahead on
/// the facing quadrant's axis at 640 above the feet.
fn wall_ahead(world: Option<&WorldGeometry>, position: Vec3, yaw: f32, room: usize) -> bool {
    let Some(world) = world else { return false };
    let quadrant = ((yaw * 65536.0 / std::f32::consts::TAU).round() as i32 + 8192) >> 14;
    let step = units(256.0);
    let offset = match quadrant.rem_euclid(4) {
        0 => Vec3::Z * step,
        1 => Vec3::X * step,
        2 => -Vec3::Z * step,
        _ => -Vec3::X * step,
    };
    let test = position.y + units(640.0);
    world.floor_at(position + offset, room).is_none_or(|hit| hit.height >= test || hit.ceiling <= test)
}

/// UpdateLaraRoom 0x8001C0E8: the room containing a point `offset` original
/// units above the feet (381 for ordinary and transition updates, so the
/// body rather than the feet decides when Lara crosses a water surface).
fn body_room(world: &WorldGeometry, position: Vec3, room: usize, offset: f32) -> usize {
    world.floor_at(position + Vec3::Y * units(offset), room).map_or(room, |hit| hit.room)
}

fn floor(world: Option<&WorldGeometry>, position: Vec3, room: usize) -> Option<FloorHit> {
    match world {
        Some(world) => world.floor_at(position, room),
        None => Some(FloorHit { height: 0.0, ceiling: f32::INFINITY, room, water: false, death: false }),
    }
}

/// Original animation speed is truncated to whole world units before movement.
/// WALL.PSX run-start clip6 (14 frames), walk-start clips20 (10) and21 (17).
/// Confirmed by native RAM samples: .psx/home_run_reference and .psx/home_walk,
/// item0017c87c +0x1e speed, +0x14 animation, sampled every two NTSC fields.
fn ground_speed(mode: PlayerMode, frame: f32) -> f32 {
    let frame = frame.floor() as i32;
    let original = match mode {
        PlayerMode::Run if frame < 14 => ((1_572_863_i64 + i64::from(frame) * 115_948) / 65_536) as f32,
        PlayerMode::Run => 47.0,
        PlayerMode::Walk if frame < 10 => 2.0,
        PlayerMode::Walk if frame < 27 => ((524_288_i64 + i64::from(frame - 10) * 28_671) / 65_536) as f32,
        PlayerMode::Walk => 15.0,
        _ => return 0.0,
    };
    units(original * ORIGINAL_FPS)
}

fn jump_animation(forward: Vec3, velocity: Vec3) -> usize {
    let horizontal = velocity.with_y(0.0);
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    if horizontal.length_squared() < 0.01 {
        clip::JUMP_UP
    } else if horizontal.dot(right).abs() > horizontal.dot(forward).abs() {
        if horizontal.dot(right) > 0.0 { clip::JUMP_RIGHT } else { clip::JUMP_LEFT }
    } else if horizontal.dot(forward) < 0.0 {
        clip::JUMP_BACK
    } else {
        clip::JUMP_FORWARD
    }
}

/// Ground directions after the original's R2 modifier (0x80069634..0x800696F0).
/// R1 slows walking/turning; R2+Left/Right sidesteps. Up/Down while stepping
/// steer toward/away from the facing direction. R2 alone requests no movement.
struct GroundControls {
    direction: Vec2,
    side: f32,
    walk: bool,
}

impl GroundControls {
    fn read(pad: &ControllerState) -> Self {
        let direction = pad.direction();
        let modified = pad.held(Button::R2);
        let side = if modified && direction.x.abs() > 0.1 { direction.x.signum() } else { 0.0 };
        if side != 0.0 {
            Self { direction: Vec2::new(-side * direction.y, 0.0), side, walk: pad.held(Button::R1) }
        } else {
            Self { direction, side, walk: pad.held(Button::R1) || (modified && direction.y != 0.0) }
        }
    }
}

/// The state handler (keyed by the mode at the start of the tick) adjusts the
/// shared turn rate; the rate then decays and turns Lara in every state.
/// Handlers: stop 0x8003899c, walk 0x800386c4 (cap 728), run 0x800387b0
/// (1456), turn 0x80038d94 (+409 before reading input), fast turn 0x80039370
/// (sets 1456), walk back 0x80039270 (728), forward jump 0x80038b88 (546).
fn turn(
    player: &mut Player,
    motion: &mut Motion,
    pad: &ControllerState,
    ticks: f32,
    fast_turn: bool,
    animation_state: Option<usize>,
) {
    if matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface) {
        // Water control owns its turning: submerged inertia and bank, or
        // the surface handlers' direct yaw changes.
        return;
    }
    let controls = GroundControls::read(pad);
    let direction = controls.direction;
    let held = if direction.x < -0.1 {
        -1
    } else if direction.x > 0.1 {
        1
    } else {
        0
    };
    let walk = controls.walk;
    let side_step = controls.side != 0.0;
    let push = |rate: &mut i32, cap: i32| {
        if held != 0 {
            *rate = (*rate + held * TURN_PUSH).clamp(-cap, cap);
        }
    };
    let alive = player.health > 0;
    match player.mode {
        PlayerMode::Idle if alive && motion.turning != 0 => {
            if fast_turn {
                motion.turn_rate = if motion.turn_rate >= 0 { 1456 } else { -1456 };
            } else {
                motion.turn_rate += motion.turning * TURN_PUSH;
                // With guns ready the handler skips the walk cap and asks for
                // the fast turn directly (0x80038DC4).
                if walk && !motion.guns_ready && motion.turn_rate.abs() >= 729 {
                    motion.turn_rate = motion.turn_rate.signum() * 728;
                }
            }
            if held == motion.turning && direction.y.abs() <= 0.1 {
                motion.turn_frames += ticks;
            } else {
                motion.turning = 0;
                motion.turn_frames = 0.0;
            }
        }
        // Stop requests compression on Jump before reading Up, Left or Right
        // (0x80038B00); compression itself does not turn.
        PlayerMode::Idle if alive && animation_state == Some(state::STAND) && pad.held(Button::Square) => {}
        // Stop delegates to the walk/run handler on the tick UP is pressed.
        PlayerMode::Idle if alive && direction.y > 0.1 => push(&mut motion.turn_rate, if walk { 728 } else { 1456 }),
        // Stop delegates DOWN+walk to state16 (0x80038B44..0x80038B5C).
        PlayerMode::Idle if alive && direction.y < -0.1 && walk => push(&mut motion.turn_rate, 728),
        PlayerMode::Idle if alive && motion.grounded && held != 0 && !side_step && direction.y.abs() <= 0.1 => {
            motion.turning = held;
            motion.turn_frames = ticks;
        }
        // Side handlers 0x800393D8/0x800394B8 steer with the walk turn cap.
        PlayerMode::Walk | PlayerMode::Back | PlayerMode::StepLeft { .. } | PlayerMode::StepRight { .. } if alive => {
            push(&mut motion.turn_rate, 728);
        }
        PlayerMode::Run if alive => push(&mut motion.turn_rate, 1456),
        // Hop-back handler 0x80038D00: cap1092, or 728 after shared decay.
        PlayerMode::HopBack if alive => push(&mut motion.turn_rate, 1092),
        PlayerMode::Jump | PlayerMode::Fall if alive => push(&mut motion.turn_rate, 546),
        PlayerMode::JumpingForward if alive && animation_state == Some(state::FORWARD_JUMP) => {
            push(&mut motion.turn_rate, 546)
        }
        _ => {}
    }
    if motion.turn_rate.abs() <= TURN_DECAY {
        motion.turn_rate = 0;
    } else {
        motion.turn_rate -= motion.turn_rate.signum() * TURN_DECAY;
    }
}

/// Runs after integration: climb_two_click.csv shows each tick's displacement
/// uses the heading from before that tick's turn (x/z rounding at yaw 19166).
fn apply_turn_rate(time: Res<Time>, mut players: Query<(&mut Player, &Motion)>) {
    let ticks = original_frames(time.delta_secs());
    for (mut player, motion) in &mut players {
        if motion.turn_rate != 0 && !matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface) {
            let yaw = player.yaw + motion.turn_rate as f32 * ticks * std::f32::consts::TAU / 65536.0;
            player.yaw = yaw.rem_euclid(std::f32::consts::TAU);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn control(
    pad: Res<ControllerState>,
    time: Res<Time>,
    clips: Option<Res<PlayerAnimations>>,
    world: Option<Res<WorldGeometry>>,
    tuning: Res<SwimmingTuning>,
    mut players: Query<(
        &mut Player,
        &mut Motion,
        &WeaponState,
        &mut AnimationState,
        &mut Traversal,
        &mut SubmergedMotion,
        &mut Wading,
    )>,
) {
    let dt = time.delta_secs();
    let controls = GroundControls::read(&pad);
    for (mut player, mut motion, weapon, mut animation, mut traversal, mut submerged, mut wading) in &mut players {
        motion.camera_elevation = None;
        motion.camera_angle = 0;
        motion.camera_distance = None;
        motion.camera_speed = None;
        // LaraControl's water transitions precede the tick's state handler.
        if let Some(world) = world.as_deref() {
            wading::transitions(
                &mut player,
                &mut motion,
                &mut submerged,
                &mut animation,
                &mut traversal,
                &mut wading,
                world,
                clips.as_deref(),
                &tuning.wading,
            );
            match player.mode {
                PlayerMode::Swimming => surface::leave_water(
                    &mut player,
                    &mut motion,
                    &mut submerged,
                    &mut animation,
                    &mut traversal,
                    world,
                ),
                PlayerMode::Surface | PlayerMode::Dead => {}
                _ if !wading.active => surface::enter_water(
                    &mut player,
                    &mut motion,
                    &mut submerged,
                    &mut animation,
                    &mut traversal,
                    world,
                    &tuning,
                    &mut wading,
                ),
                _ => {}
            }
        }
        let animation = &*animation;
        let was_running = player.mode == PlayerMode::Run;
        let was_walking = player.mode == PlayerMode::Walk;
        let was_standing = player.mode == PlayerMode::Idle
            && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(state::STAND);
        if was_running {
            // Run handler0x800388D4..0x80038944 persists a jump-enable
            // latch. Clip6 clears it; clip0 frame4 or any other clip sets it.
            if animation.animation == clip::RUN_START {
                traversal.run_jump_ready = false;
            } else if animation.animation != clip::RUN || animation.frame == 4.0 {
                traversal.run_jump_ready = true;
            }
        }
        // Submerged input is read directly; its steering runs before movement.
        let direction = if player.mode == PlayerMode::Swimming { pad.direction() } else { controls.direction };
        // State20 sets the fast rate; slow states6/7 keep adding 409 even
        // after a long R1 hold is released (0x80038DA4..0x80038E04).
        let fast_turn = clips
            .as_deref()
            .and_then(|c| c.state(animation.animation))
            .map_or(motion.turn_frames >= 33.0, |s| s == state::FAST_TURN);
        motion.guns_ready = weapon.ready();
        motion.camera_exact_slopes = clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(SLIDE_STATE);
        if wading::control(
            &mut player,
            &mut motion,
            &mut submerged,
            animation,
            &mut traversal,
            &wading,
            &pad,
            clips.as_deref(),
            &tuning,
        ) {
            continue;
        }
        turn(
            &mut player,
            &mut motion,
            &pad,
            original_frames(dt),
            fast_turn,
            clips.as_deref().and_then(|c| c.state(animation.animation)),
        );
        if player.health <= 0 && matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface) {
            player.health = -1;
        }
        if player.health <= 0 && !matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface) {
            if player.mode == PlayerMode::FatalLanding {
                // LaraControl clamps a dead actor to-1 on its next tick
                // (0x80042B10), including impact154's initial health0.
                player.health = -1;
                continue;
            }
            player.mode = PlayerMode::Dead;
            motion.velocity.x = 0.0;
            motion.velocity.z = 0.0;
            continue;
        }
        if motion.grounded
            && pad.held(Button::Circle)
            && (player.mode == PlayerMode::WalkStopping
                || (player.mode == PlayerMode::Recovering
                    && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(state::STAND)))
        {
            player.mode = PlayerMode::Rolling { entering: true };
        }
        if matches!(
            player.mode,
            PlayerMode::Back | PlayerMode::HopBack | PlayerMode::StepLeft { .. } | PlayerMode::StepRight { .. }
        ) && motion.grounded
            && let Some(clips) = clips.as_deref()
        {
            // State5 always asks to stop (0x80038D0C); state16 asks to stop
            // unless DOWN+walk remains held (0x80039294..0x800392C0).
            // Authored ranges decide when a walking foot can stop. The hop
            // instead finishes clips88->89->90->11 regardless of release.
            // Side handlers only assign goal2 on release; re-holding before
            // the stop window does not cancel it (0x80039418/0x800394F8).
            match &mut player.mode {
                PlayerMode::StepLeft { stopping } => *stopping |= controls.side >= 0.0,
                PlayerMode::StepRight { stopping } => *stopping |= controls.side <= 0.0,
                _ => {}
            }
            let index = mode_clip(&player, &motion, &pad);
            let next = step_animation(animation, index, None, Some(clips), original_frames(dt));
            let forward = player.forward();
            let right = Vec3::new(forward.z, 0.0, -forward.x);
            // AnimateLara uses the preceding collision heading. The side
            // collision writes yaw +/-90 degrees at 0x8003B5FC.
            let heading = match clips.state(animation.animation) {
                Some(state::STEP_RIGHT) => right,
                Some(state::STEP_LEFT) => -right,
                _ => -forward,
            };
            motion.velocity = heading * clip_speed(clips, &next).unwrap_or(0.0);
            if clips.state(next.animation) == Some(state::STAND) {
                player.mode = PlayerMode::Idle;
            }
            continue;
        }
        match player.mode {
            PlayerMode::Sliding { direction, backwards } => {
                // WALL.PSX clip70: speed50. Native Wall entry confirms no-input
                // sliding at50 units/tick and cardinal downhill headings.
                motion.velocity = direction * units(50.0 * ORIGINAL_FPS);
                if clips.is_some() {
                    // Slide controls0x80039598/0x80039808 read held Jump
                    // without the opposite direction (input&0x12, &0x11):
                    // a forward slide requests jump3, a back slide jump25.
                    // The slide clip's authored range enters the takeoff.
                    let away = if backwards { pad.held(Button::Up) } else { pad.held(Button::Down) };
                    traversal.goal = match (pad.held(Button::Square) && !away, backwards) {
                        (true, false) => state::FORWARD_JUMP,
                        (true, true) => state::JUMP_BACK,
                        (false, false) => state::SLIDE,
                        (false, true) => state::BACK_SLIDE,
                    };
                } else if pad.pressed(Button::Square) {
                    player.mode = PlayerMode::Jump;
                    motion.grounded = false;
                    motion.velocity += Vec3::Y * JUMP_SPEED;
                    motion.air_animation = jump_animation(player.forward(), motion.velocity);
                }
                continue;
            }
            PlayerMode::SlideStopping { direction, seconds_left } if seconds_left > dt => {
                player.mode = PlayerMode::SlideStopping { direction, seconds_left: seconds_left - dt };
                // Clip71 then 72: 15 frames at 30, then 9 at 0 (wall_initial
                // settles at z26090). Speed follows the clip being shown.
                let speed = clips.as_deref().and_then(|clips| {
                    clip_speed(
                        clips,
                        &step_animation(animation, clip::SLIDE_STOP, None, Some(clips), original_frames(dt)),
                    )
                });
                let fallback = if seconds_left > 0.3 { units(30.0 * ORIGINAL_FPS) } else { 0.0 };
                motion.velocity = direction * speed.unwrap_or(fallback);
                continue;
            }
            PlayerMode::PreparingJump { launch_velocity, frames_left, standing, running } => {
                if frames_left <= dt * ORIGINAL_FPS + 0.001 {
                    player.mode = PlayerMode::Jump;
                    motion.velocity = launch_velocity;
                    motion.grounded = false;
                    motion.air_animation = jump_animation(player.forward(), launch_velocity);
                } else {
                    player.mode = PlayerMode::PreparingJump {
                        launch_velocity,
                        frames_left: frames_left - dt * ORIGINAL_FPS,
                        standing,
                        running,
                    };
                }
                continue;
            }
            PlayerMode::Using { .. } => {
                using::control(&mut motion, clips.as_deref(), animation);
                continue;
            }
            PlayerMode::Climbing { .. }
            | PlayerMode::HitWall { .. }
            | PlayerMode::JumpingUp { .. }
            | PlayerMode::JumpingForward
            | PlayerMode::FastFalling
            | PlayerMode::JumpImpact
            | PlayerMode::Hanging { .. }
            | PlayerMode::PullingUp
            | PlayerMode::Recovering
            | PlayerMode::WalkStopping => continue,
            PlayerMode::FallingBack => continue,
            PlayerMode::Landing { seconds_left, animation, start_frame } if seconds_left > dt => {
                player.mode = PlayerMode::Landing { seconds_left: seconds_left - dt, animation, start_frame };
                motion.velocity.x = 0.0;
                motion.velocity.z = 0.0;
                continue;
            }
            PlayerMode::Rolling { .. } => continue,
            _ => {}
        }
        let walk = controls.walk;
        let side = controls.side;
        let forward = player.forward();
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        if player.mode == PlayerMode::Surface {
            surface::control(
                &mut player,
                &mut motion,
                &mut submerged,
                animation,
                &mut traversal,
                &pad,
                clips.as_deref(),
                &tuning,
            );
            continue;
        }
        if player.mode == PlayerMode::Swimming {
            swimming::control(
                &mut player,
                &mut motion,
                &mut submerged,
                animation,
                &mut traversal,
                &pad,
                clips.as_deref(),
                &tuning,
            );
            continue;
        }
        *submerged = SubmergedMotion { air_ticks: tuning.max_air_ticks, ..default() };
        if !motion.grounded {
            continue;
        }
        // Standing and run controllers force roll146 at+2 on held Roll
        // (0x800389D4 and0x800387CC); walking control has no roll request.
        if pad.held(Button::Circle)
            && clips.as_deref().is_none_or(|c| matches!(c.state(animation.animation), Some(state::RUN | state::STAND)))
        {
            player.mode = PlayerMode::Rolling { entering: true };
            motion.velocity = forward * units(50.0 * ORIGINAL_FPS);
            continue;
        }
        let (mode, velocity) = if side.abs() > 0.1 {
            (
                if side < 0.0 {
                    PlayerMode::StepLeft { stopping: false }
                } else {
                    PlayerMode::StepRight { stopping: false }
                },
                right * side * STEP_SPEED,
            )
        } else if direction.y > 0.1 {
            (
                if walk { PlayerMode::Walk } else { PlayerMode::Run },
                forward * direction.y * if walk { WALK_SPEED } else { RUN_SPEED },
            )
        } else if direction.y < -0.1 {
            // Stop handler 0x80038B44: DOWN alone requests state5; with
            // walk it delegates to state16. Both moves use the level clips.
            (if walk { PlayerMode::Back } else { PlayerMode::HopBack }, forward * direction.y * BACK_SPEED)
        } else if was_running && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(state::RUN) {
            // Run control requests goal2 on release (0x8003898C), but the
            // running state and authored speed persist until its stop range.
            (PlayerMode::Run, forward * RUN_SPEED)
        } else if was_walking && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(state::WALK) {
            // Walk control requests goal2 on release (0x800387A0).
            // Finish the stride and its authored stop before standing.
            (PlayerMode::Walk, forward * WALK_SPEED)
        } else {
            (PlayerMode::Idle, Vec3::ZERO)
        };
        if player.mode == mode {
            motion.locomotion_frame += original_frames(dt);
        } else {
            motion.locomotion_frame = 0.0;
        }
        player.mode = mode;
        // Movement and the stop-state handoff use the same prospective
        // authored frame; the animation system commits it after collision.
        let next = clips.as_deref().map(|clips| {
            let index = mode_clip(&player, &motion, &pad);
            step_animation(animation, index, None, Some(clips), original_frames(dt))
        });
        motion.velocity = if matches!(
            mode,
            PlayerMode::Run
                | PlayerMode::Walk
                | PlayerMode::Back
                | PlayerMode::HopBack
                | PlayerMode::StepLeft { .. }
                | PlayerMode::StepRight { .. }
        ) {
            // The move uses the frame this tick's animation step will show.
            let speed = clips.as_deref().zip(next.as_ref()).and_then(|(clips, next)| clip_speed(clips, next));
            if matches!(
                mode,
                PlayerMode::Back | PlayerMode::HopBack | PlayerMode::StepLeft { .. } | PlayerMode::StepRight { .. }
            ) {
                // AnimateLara moves using the previous collision pass's
                // heading (0x80043248). Entering backward walk or a sidestep
                // takes one forward step before collision changes the heading
                // (0x8003B43C/0x8003B5FC); both PSX recordings confirm it.
                let heading = if matches!(mode, PlayerMode::Back | PlayerMode::HopBack) {
                    motion.movement_heading.map_or(forward, |yaw| Vec3::new(yaw.sin(), 0.0, yaw.cos()))
                } else {
                    forward
                };
                speed.map_or(velocity, |speed| heading * speed)
            } else {
                let direction = if matches!(mode, PlayerMode::Run | PlayerMode::Walk) && direction.y == 0.0 {
                    1.0
                } else {
                    direction.y
                };
                forward * direction * speed.unwrap_or_else(|| ground_speed(mode, motion.locomotion_frame))
            }
        } else {
            velocity
        };
        if matches!(mode, PlayerMode::Run | PlayerMode::Walk)
            && direction.y == 0.0
            && clips.as_deref().zip(next.as_ref()).and_then(|(clips, next)| clips.state(next.animation))
                == Some(state::STAND)
        {
            player.mode = PlayerMode::Idle;
        }
        // Stop handler0x80038B00 tests held Jump before delegating Up to
        // running. A run-start wall stop therefore retries a held request
        // on the following standing tick, without another button edge.
        let jump_requested = pad.pressed(Button::Square)
            || ((was_standing || (was_running && traversal.run_jump_ready)) && pad.held(Button::Square));
        if jump_requested && (!was_running || traversal.run_jump_ready) {
            if clips.is_some() {
                // Only standing and running take Jump, whatever else is
                // held. Stop+Jump enters compression73, whose control picks
                // the direction. Running asks for goal3 through its authored
                // foot-dependent ranges; these select16 or18
                // (0x8003899C/0x800387B0).
                if was_standing || was_running {
                    traversal.enter = if was_running { None } else { Some((clip::JUMP_PREPARE, 0.0)) };
                    traversal.goal = if was_running { state::FORWARD_JUMP } else { state::JUMP_PREPARE };
                    player.mode = PlayerMode::JumpingForward;
                    motion.velocity = Vec3::ZERO;
                }
                continue;
            }
            let jump_forward = if mode == PlayerMode::Run { RUN_JUMP_SPEED } else { JUMP_FORWARD_SPEED };
            let jump_direction = if side.abs() > 0.1 {
                right * side.signum()
            } else if direction.y.abs() > 0.1 {
                forward * direction.y.signum()
            } else {
                right * direction.x.signum()
            };
            let standing = direction.length_squared() <= 0.01 && side.abs() <= 0.1;
            let launch_velocity = if !standing {
                jump_direction * jump_forward + Vec3::Y * JUMP_SPEED
            } else {
                // WALL.PSX animation 91 command 2: vertical -110, forward 2.
                forward * units(2.0 * ORIGINAL_FPS) + Vec3::Y * units(110.0 * ORIGINAL_FPS)
            };
            // The standing compression and takeoff clips73/91 last11+3 frames.
            // Verified in native home_clean_jump: first airborne state at tick15.
            player.mode = PlayerMode::PreparingJump {
                launch_velocity,
                frames_left: if mode == PlayerMode::Run { 7.0 } else { 14.0 },
                standing,
                running: mode == PlayerMode::Run,
            };
            if mode != PlayerMode::Run {
                motion.velocity = Vec3::ZERO;
            }
        }
    }
}

#[allow(clippy::type_complexity, reason = "Locomotion updates the player's independent ECS components together.")]
fn integrate(
    world: Option<Res<WorldGeometry>>,
    pad: Res<ControllerState>,
    time: Res<Time>,
    clips: Option<Res<PlayerAnimations>>,
    tuning: Res<SwimmingTuning>,
    mut players: Query<(
        &mut Player,
        &mut Motion,
        &WeaponState,
        &mut AnimationState,
        &mut Traversal,
        &mut SubmergedMotion,
        &mut Wading,
        &mut CollisionFrame,
    )>,
) {
    let dt = time.delta_secs();
    let geometry = world.as_deref();
    for (mut player, mut motion, weapon, mut animation, mut traversal, mut submerged, mut wading, mut collision) in
        &mut players
    {
        if let PlayerMode::Using { .. } = player.mode {
            // The use clips have no speed: Lara stays where her object put her.
            motion.velocity = Vec3::ZERO;
            continue;
        }
        if player.mode == PlayerMode::Surface {
            surface::step(
                &mut player,
                &mut motion,
                &mut submerged,
                &mut animation,
                &mut traversal,
                geometry,
                clips.as_deref(),
                weapon.hands_free() && pad.held(Button::Cross),
                &tuning,
                &mut wading,
                dt,
            );
            continue;
        }
        if player.mode == PlayerMode::Swimming {
            swimming::step(
                &mut player,
                &mut motion,
                &mut submerged,
                &mut animation,
                &mut traversal,
                geometry,
                clips.as_deref(),
                &tuning,
                &mut wading,
                dt,
            );
            continue;
        }
        if wading::step(
            &mut player,
            &mut motion,
            &mut submerged,
            &mut animation,
            &mut traversal,
            &wading,
            geometry,
            clips.as_deref(),
            dt,
        ) {
            continue;
        }
        if rolling::step(
            &mut player,
            &mut motion,
            &mut animation,
            &mut traversal,
            geometry,
            clips.as_deref(),
            &mut collision,
            dt,
        ) {
            continue;
        }
        if let PlayerMode::Sliding { backwards, .. } = player.mode
            && let (Some(world), Some(clips)) = (geometry, clips.as_deref())
            && matches!(traversal.goal, state::FORWARD_JUMP | state::JUMP_BACK)
        {
            // A slide-jump request leaves through the slide clip's authored
            // range into takeoff76 (state3) or74 (state25), at the frame
            // the range names. That clip's handler owns this tick's move and collision.
            let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
            let mut goal = AnimationGoal { state: traversal.goal };
            advance_authored_animation(&world.data, &mut a, &mut goal, original_frames(dt));
            if clips.state(a.animation) == Some(traversal.goal) {
                player.mode = if backwards { PlayerMode::FallingBack } else { PlayerMode::JumpingForward };
                traversal.enter = Some((a.animation, a.frame));
                motion.velocity = Vec3::ZERO;
            }
        }
        if backward::step(
            &mut player,
            &mut motion,
            &mut animation,
            &mut traversal,
            geometry,
            clips.as_deref(),
            weapon,
            &pad,
            dt,
        ) {
            continue;
        }
        if forward::step(&mut player, &mut motion, &mut animation, &mut traversal, geometry, clips.as_deref(), &pad, dt)
        {
            continue;
        }
        if matches!(player.mode, PlayerMode::Jump | PlayerMode::Fall)
            && !motion.grounded
            && weapon.hands_free()
            && pad.held(Button::Cross)
            && let Some(state) = clips.as_deref().and_then(|c| c.state(animation.animation))
        {
            match state {
                state::JUMP_UP => {
                    player.mode = PlayerMode::JumpingUp { launch_speed: 0.0 };
                    traversal.goal = state::JUMP_UP;
                }
                state::FORWARD_JUMP => {
                    player.mode = PlayerMode::JumpingForward;
                    traversal.goal = state::REACH;
                }
                _ => {}
            }
        }
        if traversal::step(
            &mut player,
            &mut motion,
            &mut animation,
            &mut traversal,
            geometry,
            clips.as_deref(),
            weapon,
            &pad,
            dt,
        ) {
            continue;
        }
        if let PlayerMode::Climbing { animation, frames_left, offset } = player.mode {
            motion.velocity = Vec3::ZERO;
            let frames_left = frames_left - original_frames(dt);
            if frames_left > 0.001 {
                player.mode = PlayerMode::Climbing { animation, frames_left, offset };
                continue;
            }
            // climb_two_click.csv: +128 forward, -512 up on the tick clip50
            // hands over to clip51, then standing collision settles on the floor.
            player.position += offset;
            motion.grounded = true;
            motion.air_animation = 0;
            player.mode = PlayerMode::Idle;
            if let Some(hit) = floor(geometry, player.position, player.room) {
                player.room = hit.room;
                player.position.y = hit.height;
            }
            continue;
        }
        if let PlayerMode::HitWall { animation, frames_left } = player.mode {
            motion.velocity = Vec3::ZERO;
            let frames_left = frames_left - original_frames(dt);
            player.mode =
                if frames_left > 0.001 { PlayerMode::HitWall { animation, frames_left } } else { PlayerMode::Idle };
            if let Some(hit) = floor(geometry, player.position, player.room) {
                player.position.y = hit.height;
            }
            continue;
        }
        if matches!(player.mode, PlayerMode::Hanging { .. }) {
            continue;
        }
        let old = player.position;
        // Ordinary standing/forward collision refreshes the heading. The
        // backward pass above retains its own heading after a forced stop.
        motion.movement_heading = Some(player.yaw + motion.turn_rate as f32 * std::f32::consts::TAU / 65536.0);
        let Some(current_floor) = floor(geometry, old, player.room) else {
            continue;
        };
        if motion.grounded && !current_floor.water && player.health > 0 {
            let [sx, sz] = geometry.map_or([0, 0], |world| world.slope_at(old, player.room));
            let steep = sx.unsigned_abs() > 2 || sz.unsigned_abs() > 2;
            if steep {
                // Steeper cardinal axis (X on a tie). Rule predates the PSX-only
                // restriction; wall_entry.csv independently confirms +z, +x, +z
                // ramps, speed 50, the turn tick and clip71 at 30. Tie-break unverified.
                // FloorHit uses y-up, so downhill is opposite the height gradient.
                let direction = if sx.unsigned_abs() >= sz.unsigned_abs() {
                    Vec3::X * -f32::from(sx.signum())
                } else {
                    Vec3::Z * -f32::from(sz.signum())
                };
                let backwards = player.forward().dot(direction) < -0.001;
                let heading = if backwards { -direction } else { direction };
                player.yaw = heading.x.atan2(heading.z).rem_euclid(std::f32::consts::TAU);
                if !matches!(player.mode, PlayerMode::Sliding { .. }) {
                    player.mode = PlayerMode::Sliding { direction, backwards };
                    motion.velocity = Vec3::ZERO;
                } else {
                    player.mode = PlayerMode::Sliding { direction, backwards };
                    motion.velocity = direction * units(50.0 * ORIGINAL_FPS);
                    if let (Some(world), Some(clips)) = (geometry, clips.as_deref())
                        && matches!(clips.state(animation.animation), Some(state::SLIDE | state::BACK_SLIDE))
                    {
                        // Slide control keeps its own state; AnimateLara then
                        // moves along the downhill heading at the authored
                        // speed. Back-slide104 accelerates before looping105.
                        let mut a = ObjectAnimation { animation: animation.animation, frame: animation.frame };
                        let mut goal =
                            AnimationGoal { state: if backwards { state::BACK_SLIDE } else { state::SLIDE } };
                        advance_authored_animation(&world.data, &mut a, &mut goal, original_frames(dt));
                        *animation = AnimationState::at_frame(a.animation, a.frame);
                        traversal.animation_stepped = true;
                        motion.velocity = direction * clips.speed(&animation).unwrap_or(0.0);
                    }
                }
            } else if let PlayerMode::Sliding { direction, .. } = player.mode {
                player.mode = PlayerMode::SlideStopping { direction, seconds_left: 0.8 };
                motion.velocity = direction * units(30.0 * ORIGINAL_FPS);
            }
        }
        motion.oxygen_seconds = 60.0;
        if !motion.grounded {
            let gravity = if -motion.velocity.y >= units(128.0 * ORIGINAL_FPS) { GRAVITY / 6.0 } else { GRAVITY };
            motion.velocity.y -= gravity * dt;
            if motion.velocity.y < 0.0 && player.health > 0 {
                player.mode = PlayerMode::Fall;
            }
        }
        let mut displacement = motion.velocity * dt;
        if motion.grounded && matches!(player.mode, PlayerMode::Run | PlayerMode::Idle) {
            // AnimateLara0x80043248 uses the quantized sine table even on
            // ordinary ground/braking frames. Continuous trig can move an
            // extra unit before an oblique contact (fresh right/start routes).
            let angle = (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
            let speed = (motion.velocity.with_y(0.0).length() * 1024.0 / ORIGINAL_FPS).round() as i32;
            displacement.x = units(((engine::camera::chase::sin(angle) * speed) >> 14) as f32 * original_frames(dt));
            displacement.z = units(((engine::camera::chase::cos(angle) * speed) >> 14) as f32 * original_frames(dt));
        } else {
            // Native positions are integral units. Per-axis floor is visible:
            // at the recorded oblique heading, a2-unit jump speed moves(-2,0).
            // Suppress trigonometric roundoff at exact cardinal headings.
            for axis in [0, 2] {
                let original = displacement[axis] * 1024.0;
                let stable = if (original - original.round()).abs() < 0.0001 { original.round() } else { original };
                displacement[axis] = units(stable.floor());
            }
        }
        let proposed = old + displacement;
        let horizontal = Vec3::new(proposed.x, old.y, proposed.z);
        let ahead = horizontal + motion.velocity.with_y(0.0).normalize_or_zero() * PLAYER_RADIUS;
        let candidate_floor = floor(geometry, ahead, player.room);
        let walk = matches!(
            player.mode,
            PlayerMode::Walk | PlayerMode::Back | PlayerMode::StepLeft { .. } | PlayerMode::StepRight { .. }
        );
        let passable = |hit: &FloorHit| {
            hit.ceiling - hit.height >= PLAYER_HEIGHT
                // Original jump.csv includes the last horizontal displacement
                // on the landing tick. Test a descending sweep's upper endpoint
                // so crossing the floor does not become a horizontal wall.
                && (hit.height <= if motion.grounded { old.y + STEP_HEIGHT } else { old.y.max(proposed.y) + 0.02 })
                && (!walk || old.y - hit.height <= STEP_HEIGHT)
        };
        let can_move = candidate_floor.as_ref().is_some_and(passable);
        if can_move {
            player.position.x = proposed.x;
            player.position.z = proposed.z;
        } else if weapon.hands_free()
            && pad.held(Button::Cross)
            && pad.direction().y > 0.1
            && candidate_floor.is_some()
            && motion.grounded
            && let Some(vault) = vault(geometry, horizontal, player.yaw, player.room)
        {
            // The grab tick discards the move; collision first shifts
            // Lara out of the wall (climb_two_click.csv: x64411, z kept).
            let forward = Vec3::new(vault.yaw.sin(), 0.0, vault.yaw.cos());
            let axis = if forward.x.abs() > forward.z.abs() { 0 } else { 2 };
            player.position = old;
            player.position[axis] = wall_contact(old[axis], proposed[axis]);
            player.position.y = vault.height;
            player.yaw = vault.yaw;
            motion.turn_rate = 0;
            if let Some(launch_speed) = vault.jump_speed {
                player.mode = PlayerMode::JumpingUp { launch_speed };
                motion.velocity = Vec3::ZERO;
                traversal.goal = state::JUMP_UP;
                traversal.animation_stepped = true;
                animation.enter(vault.animation, 0.0, vault.animation);
                continue;
            }
            let (frames, offset) = clips.as_ref().and_then(|c| c.climb(vault.animation)).unwrap_or(
                if vault.animation == clip::VAULT_TWO { (28.0, [0, -512, 128]) } else { (66.0, [0, -768, 128]) },
            );
            let (sin, cos) = vault.yaw.sin_cos();
            let forward = offset[2] as f32;
            let offset = Vec3::new(sin * forward, -offset[1] as f32, cos * forward) / 1024.0;
            player.mode = PlayerMode::Climbing { animation: vault.animation, frames_left: frames, offset };
            // The grab tick retains its animation speed (31 in the
            // three-click recording). The frozen climb clears it on
            // its following tick, after the clip has advanced.
            continue;
        }
        if !can_move {
            // Resolve axes independently so a diagonal approach slides along a wall.
            // Test the collision radius in the direction of each attempted movement.
            for axis in [0, 2] {
                let delta = proposed[axis] - old[axis];
                if delta.abs() < f32::EPSILON {
                    continue;
                }
                let mut probe = player.position;
                probe[axis] = proposed[axis] + delta.signum() * PLAYER_RADIUS;
                if floor(geometry, probe, player.room).as_ref().is_some_and(passable) {
                    player.position[axis] = proposed[axis];
                } else if motion.grounded {
                    player.position[axis] = wall_contact(old[axis], proposed[axis]);
                }
            }
        }
        let mut blocked_run_floor = None;
        let forward = player.forward();
        let facing = if forward.x.abs() > forward.z.abs() { 0 } else { 2 };
        if !can_move && player.mode == PlayerMode::Run && motion.grounded && player.position[facing] != proposed[facing]
        {
            // A front impact restores the perpendicular coordinate from the
            // tick's start before applying the normal shift (GetCollisionInfo
            // and LaraDeflectEdge0x8003C544; oblique-wall trace field225).
            // Sliding along a side contact remains in the separate axis path.
            player.position[2 - facing] = old[2 - facing];
            // Run collision 0x8003a844: no hit-wall clip during run-start clip6
            // (holding UP against the wall keeps standing); otherwise a wall 256
            // ahead and 640 up (0x8003c91c) selects clip53 before frame10, else 54.
            let request = mode_clip(&player, &motion, &pad);
            let next =
                clips.as_deref().map(|c| (c, step_animation(&animation, request, None, Some(c), original_frames(dt))));
            let hit_wall = next.as_ref().is_some_and(|(_, next)| next.animation != clip::RUN_START)
                && wall_ahead(geometry, old, player.yaw, player.room);
            let hit_clip = next.filter(|(_, next)| next.animation == clip::RUN).map(|(c, next)| {
                let frame = c.clips[clip::RUN].frame_start + next.frame.floor() as usize;
                if frame < 10 { clip::HIT_WALL_EARLY } else { clip::HIT_WALL_LATE }
            });
            motion.velocity = Vec3::ZERO;
            match hit_clip.filter(|_| hit_wall) {
                Some(clip) => {
                    let frames = clips
                        .as_deref()
                        .and_then(|c| c.clips.get(clip))
                        .map_or(15.0, |c| (c.frame_end + 1).saturating_sub(c.frame_start) as f32);
                    player.mode = PlayerMode::HitWall { animation: clip, frames_left: frames };
                    // climb_two_click.csv field 187: height settles on the next tick.
                    player.position.y = old.y;
                    continue;
                }
                None => {
                    player.mode = PlayerMode::Idle;
                    if let Some(clips) = clips.as_deref() {
                        // LaraColStop0x8003A56C restores a preceding stand
                        // or turn pose and advances it toward the held turn
                        // goal. Otherwise it enters clip11 without advancing.
                        traversal.goal = state::STAND;
                        if matches!(
                            clips.state(animation.animation),
                            Some(state::STAND | state::TURN_RIGHT | state::TURN_LEFT | state::FAST_TURN)
                        ) {
                            traversal.goal = if pad.held(Button::Left) {
                                state::TURN_LEFT
                            } else if pad.held(Button::Right) {
                                state::TURN_RIGHT
                            } else {
                                state::STAND
                            };
                            let request = match traversal.goal {
                                state::TURN_LEFT => clip::TURN_LEFT,
                                state::TURN_RIGHT => clip::TURN_RIGHT,
                                _ => STANDING_CLIP,
                            };
                            *animation = step_animation(&animation, request, None, Some(clips), original_frames(dt));
                        } else {
                            animation.enter(STANDING_CLIP, 0.0, STANDING_CLIP);
                        }
                        if hit_wall {
                            // Run collision sets state12 before testing the
                            // foot ranges (0x8003A8F0..0x8003A944). If neither
                            // range accepts, ColStop enters11 but retains12
                            // until that clip's natural end on the next tick.
                            let clip = &clips.clips[animation.animation];
                            player.mode = PlayerMode::HitWall {
                                animation: animation.animation,
                                frames_left: (clip.frame_end + 1 - clip.frame_start) as f32 - animation.frame,
                            };
                        }
                        traversal.animation_stepped = true;
                    }
                    // Run collision samples the centre floor before shifting
                    // out of the wall, then keeps that height after stopping
                    // (0x8003C34C, 0x8003A944, 0x8003AA04). On a slope this
                    // differs from the floor under the resolved position.
                    // wall_stop_trace: proposed x65458 -> y4627, resolved
                    // x65435 -> y4633; releasing UP restores the latter.
                    blocked_run_floor = floor(geometry, horizontal, player.room);
                }
            }
        }
        let Some(hit) = blocked_run_floor.or_else(|| floor(geometry, player.position, player.room)) else {
            continue;
        };
        player.position.y = proposed.y;
        player.room = geometry.map_or(hit.room, |world| body_room(world, player.position, hit.room, 381.0));
        if player.position.y <= hit.height && motion.velocity.y <= 0.0 {
            if !motion.grounded {
                landing_damage(&mut player, motion.velocity);
            }
            if !motion.grounded && player.health > 0 {
                // Landing lock is a provisional readable substitute for animation dispatch windows.
                let (animation, start_frame) = clips
                    .as_deref()
                    .and_then(|clips| freefall_landing(clips, &animation))
                    .unwrap_or((clip::HEAVY_LANDING, 0.0));
                player.mode = PlayerMode::Landing { seconds_left: 0.15, animation, start_frame };
            }
            motion.grounded = true;
            motion.air_animation = 0;
            motion.velocity.y = 0.0;
            player.position.y = hit.height;
        } else if motion.grounded {
            if let PlayerMode::Sliding { backwards, .. } = player.mode
                && clips.is_some()
                && old.y - hit.height > units(200.0)
            {
                // Slide collision0x8003C364 leaves the slope once the centre
                // is over200 above the floor (0x8003C3B4..0x8003C41C): a
                // forward slide enters clip34/state3, a back slide clip93/
                // state29. Both keep the slide speed with zero fall speed.
                let (clip, mode, goal) = if backwards {
                    (clip::FALL_BACK, PlayerMode::FallingBack, state::FALL_BACK)
                } else {
                    (clip::FALL, PlayerMode::JumpingForward, state::FORWARD_JUMP)
                };
                motion.grounded = false;
                motion.velocity.y = 0.0;
                player.mode = mode;
                traversal.goal = goal;
                traversal.animation_stepped = true;
                animation.enter(clip, 0.0, clip);
            } else if old.y - hit.height <= if clips.is_some() { units(384.0) } else { STEP_HEIGHT } {
                // Ground collisions settle any drop the ledge-fall test
                // accepts (stop0x8003AAB8, walk0x8003A818).
                player.position.y = hit.height;
            } else {
                motion.grounded = false;
                if clips.is_some() {
                    // Stop, walk, run, step and wade collisions share the
                    // ledge-fall test0x8003A500: a centre drop >384 enters
                    // clip34/state3, retains horizontal speed and starts with
                    // zero fall speed. Forward collision owns the flight.
                    player.mode = PlayerMode::JumpingForward;
                    motion.velocity.y = 0.0;
                    traversal.goal = state::FORWARD_JUMP;
                    traversal.animation_stepped = true;
                    animation.enter(clip::FALL, 0.0, clip::FALL);
                } else {
                    player.mode = PlayerMode::Fall;
                }
            }
        }
        if player.position.y + PLAYER_HEIGHT > hit.ceiling && motion.velocity.y > 0.0 {
            player.position.y = (hit.ceiling - PLAYER_HEIGHT).max(hit.height);
            motion.velocity.y = 0.0;
        }
        if hit.death && motion.grounded {
            player.damage(MAX_HEALTH);
        }
    }
}

/// The clip each mode asks for; `step_animation` decides how it is entered.
fn mode_clip(player: &Player, motion: &Motion, pad: &ControllerState) -> usize {
    let controls = GroundControls::read(pad);
    match player.mode {
        PlayerMode::Idle => {
            // Slow-turn control0x80038DD0/0x80038EA0 requests fast turn
            // above728 before decay (above364 afterward). The clip's
            // authored dispatch window decides when it can take effect.
            // With guns ready the turn handler requests it on each tick it
            // runs; the stand tick that starts the turn (turn_frames 1) only
            // asks for state 6/7 (pistols_turn_ready).
            let ready_turn = motion.guns_ready && motion.turn_frames > 1.0;
            if motion.turning != 0 && (ready_turn || (motion.turn_rate.abs() > TURN_DECAY && !controls.walk)) {
                clip::FAST_TURN_RIGHT
            } else if motion.turning < 0 {
                clip::TURN_LEFT
            } else if motion.turning > 0 {
                clip::TURN_RIGHT
            } else {
                clip::STAND_IDLE
            }
        }
        PlayerMode::Wade => clip::WADE,
        PlayerMode::Walk => {
            if controls.direction.y == 0.0 {
                STANDING_CLIP
            } else {
                clip::WALK_START
            }
        }
        PlayerMode::Run => {
            if controls.direction.y == 0.0 {
                STANDING_CLIP
            } else {
                clip::RUN_START
            }
        }
        PlayerMode::Back => {
            if controls.direction.y < -0.1 && controls.walk {
                clip::WALK_BACK
            } else {
                clip::STAND_IDLE
            }
        }
        PlayerMode::HopBack => clip::HOP_BACK,
        PlayerMode::FallingBack => clip::FALL_BACK,
        PlayerMode::StepLeft { stopping } => {
            if stopping {
                clip::STAND_IDLE
            } else {
                clip::STEP_LEFT
            }
        }
        PlayerMode::StepRight { stopping } => {
            if stopping {
                clip::STAND_IDLE
            } else {
                clip::STEP_RIGHT
            }
        }
        PlayerMode::Sliding { backwards, .. } => {
            if backwards {
                clip::BACK_SLIDE_LOOP
            } else {
                clip::SLIDE
            }
        }
        PlayerMode::SlideStopping { .. } => clip::SLIDE_STOP,
        PlayerMode::PreparingJump { frames_left, standing, running, .. } => {
            if running {
                clip::RUN_JUMP
            } else if frames_left > 3.0 {
                clip::JUMP_PREPARE
            } else if standing {
                clip::STANDING_TAKEOFF
            } else {
                clip::JUMP_FORWARD_TAKEOFF
            }
        }
        PlayerMode::Jump => {
            if motion.air_animation != 0 {
                motion.air_animation
            } else {
                jump_animation(player.forward(), motion.velocity)
            }
        }
        PlayerMode::Fall => {
            if motion.air_animation != 0 {
                motion.air_animation
            } else {
                clip::FALL
            }
        }
        PlayerMode::Landing { animation, .. } => animation,
        PlayerMode::Hanging { .. } => clip::HANG,
        PlayerMode::JumpingUp { .. } => clip::JUMP_UP_PREPARE,
        PlayerMode::JumpingForward => clip::JUMP_FORWARD_TAKEOFF,
        PlayerMode::FastFalling => clip::FAST_FALL,
        PlayerMode::FatalLanding => clip::FATAL_LANDING,
        PlayerMode::JumpImpact => clip::JUMP_IMPACT,
        PlayerMode::PullingUp => clip::PULL_UP,
        PlayerMode::Recovering => clip::JUMP_IMPACT,
        PlayerMode::WalkStopping => STANDING_CLIP,
        PlayerMode::Climbing { animation, .. }
        | PlayerMode::HitWall { animation, .. }
        | PlayerMode::Using { animation } => animation,
        PlayerMode::Rolling { .. } => clip::ROLL,
        PlayerMode::Surface => clip::SURFACE_TREAD,
        PlayerMode::Swimming => {
            if pad.held(Button::Square) {
                clip::SWIM
            } else {
                clip::TREAD
            }
        }
        PlayerMode::Dead => clip::DEAD,
    }
}

/// One tick of the original animation order: advance, then (when the mode asks
/// for another state) enter it through the clip's authored range, then chain
/// finished clips. Without extracted clips, a request restarts at frame 0.
pub fn step_animation(
    current: &AnimationState,
    index: usize,
    start_frame: Option<f32>,
    clips: Option<&PlayerAnimations>,
    ticks: f32,
) -> AnimationState {
    let mut animation = current.clone();
    if let Some(clips) = clips
        && matches!(clips.state(current.animation), Some(state::TURN_RIGHT | state::TURN_LEFT))
        && clips.state(index) == Some(state::FAST_TURN)
    {
        // Slow-turn goal20 can precede its dispatch window by many ticks.
        // The authored range also chooses44 on the right and69 on the left.
        if let Some((clip, frame)) = clips.dispatch(current, state::FAST_TURN) {
            animation.enter(clip, frame, index);
        } else {
            animation.acknowledge(index);
            advance(&mut animation, Some(clips), ticks);
        }
        return animation;
    }
    if let Some(clips) = clips
        && let (Some(state), Some(target)) = (clips.state(current.animation), clips.state(index))
        && (matches!(state, state::HOP_BACK | state::WALK_BACK | state::STEP_RIGHT | state::STEP_LEFT)
            || (state == state::WALK && matches!(index, clip::STAND | clip::WALK_START | clip::STAND_IDLE))
            || (state == state::RUN && matches!(index, clip::RUN_START | clip::STAND | clip::STAND_IDLE))
            || (state == state::STAND && matches!(index, clip::RUN_START | clip::WALK_START))
            || (state == state::STAND
                && matches!(target, state::HOP_BACK | state::WALK_BACK | state::STEP_RIGHT | state::STEP_LEFT)))
    {
        // AnimateLara 0x80042E30..0x80042FD8 advances, tries the goal
        // every tick, then follows end links. A walk, run or sidestep
        // stop request must wait for a foot's authored dispatch window.
        // Run applies only to ordinary run/stand requests: collision-forced
        // clips (e.g. vault50, whose declared state is also2) enter directly.
        let goal = if state == state::HOP_BACK { state::STAND } else { target };
        if state != goal
            && let Some((clip, frame)) = clips.dispatch(current, goal)
        {
            animation.enter(clip, frame, index);
        } else {
            animation.acknowledge(index);
            advance(&mut animation, Some(clips), ticks);
        }
        return animation;
    }
    if animation.requested() != index {
        let target = clips.and_then(|c| c.state(index));
        let state = clips.and_then(|c| c.state(animation.animation));
        let ending = clips.and_then(|c| c.clips.get(animation.animation)).and_then(|clip| {
            let duration = (clip.frame_end + 1).saturating_sub(clip.frame_start) as f32;
            (animation.frame + ticks >= duration).then(|| clips.and_then(|c| c.state(clip.next_animation))).flatten()
        });
        // Authored entries verified in climb_two_click.csv: turn clips12/13/44
        // stop through clip11, and clip51 starts running through clip52.
        let entry = match (state, target) {
            (Some(from), Some(to))
                if matches!(from, state::TURN_RIGHT | state::TURN_LEFT | state::FAST_TURN) && to == state::STAND =>
            {
                clips.and_then(|c| c.dispatch(&animation, state::STAND))
            }
            (Some(state::STAND), Some(to)) if matches!(to, state::WALK | state::RUN) => {
                clips.and_then(|c| c.dispatch(&animation, to))
            }
            _ => None,
        };
        if let Some((clip, frame)) = entry {
            animation.enter(clip, frame, index);
            return animation;
        }
        if start_frame.is_none() && target == Some(state::STAND) && (state == target || ending == target) {
            // Already standing (or finishing into a standing clip): keep playing,
            // e.g. clip50's climb hands over to clip51, which chains to clip11.
            animation.acknowledge(index);
        } else {
            animation.select(index);
            animation.frame = start_frame.unwrap_or(0.0);
            return animation;
        }
    }
    advance(&mut animation, clips, ticks);
    animation
}

/// Original ground speed of a clip frame: (speed + accel * frame) >> 16.
fn clip_speed(clips: &PlayerAnimations, animation: &AnimationState) -> Option<f32> {
    clips.speed(animation)
}

fn animate(
    time: Res<Time>,
    clips: Option<Res<PlayerAnimations>>,
    pad: Res<ControllerState>,
    mut players: Query<(&Player, &Motion, &mut AnimationState, &mut Traversal)>,
) {
    for (player, motion, mut animation, mut traversal) in &mut players {
        if std::mem::take(&mut traversal.animation_stepped) {
            continue;
        }
        let index = mode_clip(player, motion, &pad);
        if let Some(goal) = clips.as_deref().and_then(|c| c.state(index)) {
            traversal.goal = goal;
        }
        let start = match player.mode {
            PlayerMode::Landing { start_frame, .. } => Some(start_frame),
            _ => None,
        };
        *animation = step_animation(&animation, index, start, clips.as_deref(), original_frames(time.delta_secs()));
    }
}

/// Commit visual placement after object pushes, without stepping the animation
/// a second time. The camera and the render hierarchy see the same final actor.
fn sync_transform(mut players: Query<(&Player, &Motion, &SubmergedMotion, &Wading, &mut Transform)>) {
    for (player, motion, submerged, wading, mut transform) in &mut players {
        transform.translation = player.position;
        transform.rotation = Quat::from_rotation_y(player.yaw);
        if matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface) {
            transform.rotation *= Quat::from_rotation_x(-motion.pitch) * Quat::from_rotation_z(-submerged.bank);
        } else if wading.active || player.mode == PlayerMode::Wade {
            transform.rotation *= Quat::from_rotation_z(-submerged.bank);
        }
    }
}
