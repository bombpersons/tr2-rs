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
    ANIMATION_FPS as ORIGINAL_FPS, AnimationLibrary, advance, animation_frames as original_frames,
};
use engine::camera::CameraSystems;
use engine::controller::{Button, ControllerState};
pub use engine::schedule::SimulationEnabled;
use engine::schedule::{Avatar, InteractionSystems, SchedulePlugin, WorldSystems};
pub use engine::world::units;
use engine::world::{FloorHit, WorldGeometry};

mod traversal;
pub use traversal::Traversal;
mod backward;
mod collision;
mod forward;
mod rolling;
mod surface;
mod swimming;
pub use swimming::{SubmergedMotion, SurfaceTuning, SwimmingTuning};

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
#[require(Motion, AnimationState, WeaponState, Transform, Avatar, Traversal, SubmergedMotion)]
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
    Run,
    /// State16: authored backward walk, including its start and stop clips.
    Back,
    /// State5: a complete backward hop; releasing Down does not cancel it.
    HopBack,
    /// Hop-back over an edge: authored clip93, then its landing dispatch.
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
    if clips.state(animation.animation)? != 3 {
        return None;
    }
    clips.dispatch(animation, 2)
}

#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct WeaponState {
    pub drawn: bool,
    pub firing: bool,
    /// Monotonic shot counter for hit detection, sound and muzzle flashes.
    pub shot_serial: u64,
    pub cooldown_seconds: f32,
}

/// Lara's systems, inside the engine's tick order.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum PlayerSystems {
    Control,
    Motion,
    Animation,
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
        app.register_type::<Traversal>();
        app.insert_resource(crate::interactions::swimming_tuning());
        app.configure_sets(FixedUpdate, PlayerSystems::Control.in_set(WorldSystems::Control))
            .configure_sets(FixedUpdate, PlayerSystems::Motion.in_set(WorldSystems::Motion))
            .configure_sets(FixedUpdate, PlayerSystems::Animation.in_set(WorldSystems::Animation))
            .add_systems(FixedUpdate, control.in_set(PlayerSystems::Control).run_if(simulation_enabled))
            .add_systems(
                FixedUpdate,
                (integrate, apply_turn_rate).chain().in_set(PlayerSystems::Motion).run_if(simulation_enabled),
            )
            .add_systems(FixedUpdate, animate.in_set(PlayerSystems::Animation).run_if(simulation_enabled))
            .add_systems(FixedUpdate, sync_transform.in_set(InteractionSystems::Publish).run_if(simulation_enabled));
    }
}

/// Lara's object type id: her placement is where the level starts.
pub const LARA: u16 = 0;
/// Lara's standing clip (state 2).
pub const STANDING_CLIP: usize = 11;

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
    if !grounded {
        player.mode = PlayerMode::Fall;
    }
    let animation = if grounded { AnimationState::default() } else { AnimationState::at_frame(34, 0.0) };
    entity.insert((player, Motion { grounded, ..default() }, animation, Name::new("Player")));
    crate::braid::spawn(entity, ctx);
}

/// Copy Lara's state into the engine's [`Avatar`].
pub fn publish_avatar(mut players: Query<(&Player, &Motion, &WeaponState, &mut Avatar)>) {
    for (player, motion, weapon, mut avatar) in &mut players {
        avatar.set_if_neq(Avatar {
            position: player.position,
            yaw: player.yaw,
            room: player.room,
            grounded: motion.grounded,
            falling: player.mode == PlayerMode::Fall,
            armed: weapon.drawn,
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
        -640..=-384 => (50, 512, None),
        -896..=-641 => (42, 768, None),
        // Vault test 0x8003DBB0: a higher flat ledge requests the
        // authored vertical jump, overriding its end command's lift.
        -1920..=-897 => (26, 0, Some(units((((front + 800) * -12) as f32).sqrt().floor() + 3.0) * ORIGINAL_FPS)),
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
        28
    } else if horizontal.dot(right).abs() > horizontal.dot(forward).abs() {
        if horizontal.dot(right) > 0.0 { 81 } else { 79 }
    } else if horizontal.dot(forward) < 0.0 {
        75
    } else {
        77
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
                if walk && motion.turn_rate.abs() >= 729 {
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
        PlayerMode::JumpingForward if alive && animation_state == Some(3) => push(&mut motion.turn_rate, 546),
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
        &mut WeaponState,
        &mut AnimationState,
        &mut Traversal,
        &mut SubmergedMotion,
    )>,
) {
    let dt = time.delta_secs();
    let controls = GroundControls::read(&pad);
    for (mut player, mut motion, mut weapon, mut animation, mut traversal, mut submerged) in &mut players {
        // LaraControl's water transitions precede the tick's state handler.
        if let Some(world) = world.as_deref() {
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
                _ => surface::enter_water(
                    &mut player,
                    &mut motion,
                    &mut submerged,
                    &mut animation,
                    &mut traversal,
                    world,
                    &tuning,
                ),
            }
        }
        let animation = &*animation;
        let was_running = player.mode == PlayerMode::Run;
        let was_walking = player.mode == PlayerMode::Walk;
        let was_standing =
            player.mode == PlayerMode::Idle && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(2);
        if was_running {
            // Run handler0x800388D4..0x80038944 persists a jump-enable
            // latch. Clip6 clears it; clip0 frame4 or any other clip sets it.
            if animation.animation == 6 {
                traversal.run_jump_ready = false;
            } else if animation.animation != 0 || animation.frame == 4.0 {
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
            .map_or(motion.turn_frames >= 33.0, |state| state == 20);
        turn(
            &mut player,
            &mut motion,
            &pad,
            original_frames(dt),
            fast_turn,
            clips.as_deref().and_then(|c| c.state(animation.animation)),
        );
        weapon.cooldown_seconds = (weapon.cooldown_seconds - dt).max(0.0);
        weapon.firing = false;
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
        if player.health > 0 && pad.pressed(Button::Triangle) {
            weapon.drawn = !weapon.drawn;
        }
        if player.health > 0 && weapon.drawn && pad.held(Button::Cross) && weapon.cooldown_seconds == 0.0 {
            weapon.firing = true;
            weapon.shot_serial += 1;
            // Provisional pistol cadence; not yet measured from the PS1 weapon controller.
            weapon.cooldown_seconds = 0.2;
        }
        if motion.grounded
            && pad.held(Button::Circle)
            && (player.mode == PlayerMode::WalkStopping
                || (player.mode == PlayerMode::Recovering
                    && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(2)))
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
                Some(21) => right,
                Some(22) => -right,
                _ => -forward,
            };
            motion.velocity = heading * clip_speed(clips, &next).unwrap_or(0.0);
            if clips.state(next.animation) == Some(2) {
                player.mode = PlayerMode::Idle;
            }
            continue;
        }
        match player.mode {
            PlayerMode::Sliding { direction, .. } => {
                // WALL.PSX clip70: speed50. Native Wall entry confirms no-input
                // sliding at50 units/tick and cardinal downhill headings.
                motion.velocity = direction * units(50.0 * ORIGINAL_FPS);
                if pad.pressed(Button::Square) {
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
                    clip_speed(clips, &step_animation(animation, 71, None, Some(clips), original_frames(dt)))
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
            && clips.as_deref().is_none_or(|c| matches!(c.state(animation.animation), Some(1 | 2)))
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
        } else if was_running && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(1) {
            // Run control requests goal2 on release (0x8003898C), but the
            // running state and authored speed persist until its stop range.
            (PlayerMode::Run, forward * RUN_SPEED)
        } else if was_walking && clips.as_deref().and_then(|c| c.state(animation.animation)) == Some(0) {
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
            && clips.as_deref().zip(next.as_ref()).and_then(|(clips, next)| clips.state(next.animation)) == Some(2)
        {
            player.mode = PlayerMode::Idle;
        }
        // Stop handler0x80038B00 tests held Jump before delegating Up to
        // running. A run-start wall stop therefore retries a held request
        // on the following standing tick, without another button edge.
        let jump_requested = pad.pressed(Button::Square)
            || ((was_standing || (was_running && traversal.run_jump_ready)) && pad.held(Button::Square));
        if jump_requested && (!was_running || traversal.run_jump_ready) {
            if direction.y >= 0.0 && direction.x.abs() <= 0.1 && side == 0.0 && clips.is_some() {
                // Stop+Jump enters compression73 even with Up. Running asks
                // for goal3 through its authored foot-dependent ranges;
                // these select16 or18 (0x8003899C/0x800387B0).
                traversal.enter = if was_running { None } else { Some(73) };
                traversal.goal = if was_running { 3 } else { 15 };
                player.mode = PlayerMode::JumpingForward;
                motion.velocity = Vec3::ZERO;
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
    )>,
) {
    let dt = time.delta_secs();
    let geometry = world.as_deref();
    for (mut player, mut motion, weapon, mut animation, mut traversal, mut submerged) in &mut players {
        if player.mode == PlayerMode::Surface {
            surface::step(
                &mut player,
                &mut motion,
                &mut submerged,
                &mut animation,
                &mut traversal,
                geometry,
                clips.as_deref(),
                !weapon.drawn && pad.held(Button::Cross),
                &tuning,
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
                dt,
            );
            continue;
        }
        if rolling::step(&mut player, &mut motion, &mut animation, &mut traversal, geometry, clips.as_deref(), dt) {
            continue;
        }
        if backward::step(
            &mut player,
            &mut motion,
            &mut animation,
            &mut traversal,
            geometry,
            clips.as_deref(),
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
            && !weapon.drawn
            && pad.held(Button::Cross)
            && let Some(state) = clips.as_deref().and_then(|c| c.state(animation.animation))
        {
            match state {
                28 => {
                    player.mode = PlayerMode::JumpingUp { launch_speed: 0.0 };
                    traversal.goal = 28;
                }
                3 => {
                    player.mode = PlayerMode::JumpingForward;
                    traversal.goal = 11;
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
        } else if !weapon.drawn
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
                traversal.goal = 28;
                traversal.animation_stepped = true;
                animation.enter(vault.animation, 0.0, vault.animation);
                continue;
            }
            let (frames, offset) = clips
                .as_ref()
                .and_then(|c| c.climb(vault.animation))
                .unwrap_or(if vault.animation == 50 { (28.0, [0, -512, 128]) } else { (66.0, [0, -768, 128]) });
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
            let next = clips.as_deref().map(|c| (c, step_animation(&animation, 6, None, Some(c), original_frames(dt))));
            let hit_wall = next.as_ref().is_some_and(|(_, next)| next.animation != 6)
                && wall_ahead(geometry, old, player.yaw, player.room);
            let hit_clip = next.filter(|(_, next)| next.animation == 0).map(|(c, next)| {
                let frame = c.clips[0].frame_start + next.frame.floor() as usize;
                if frame < 10 { 53 } else { 54 }
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
                        traversal.goal = 2;
                        if matches!(clips.state(animation.animation), Some(2 | 6 | 7 | 20)) {
                            traversal.goal = if pad.held(Button::Left) {
                                7
                            } else if pad.held(Button::Right) {
                                6
                            } else {
                                2
                            };
                            let request = match traversal.goal {
                                7 => 13,
                                6 => 12,
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
                let (animation, start_frame) =
                    clips.as_deref().and_then(|clips| freefall_landing(clips, &animation)).unwrap_or((24, 0.0));
                player.mode = PlayerMode::Landing { seconds_left: 0.15, animation, start_frame };
            }
            motion.grounded = true;
            motion.air_animation = 0;
            motion.velocity.y = 0.0;
            player.position.y = hit.height;
        } else if motion.grounded {
            if old.y - hit.height <= STEP_HEIGHT {
                player.position.y = hit.height;
            } else {
                motion.grounded = false;
                if player.mode == PlayerMode::Run && clips.is_some() && old.y - hit.height > units(384.0) {
                    // Run collision calls the shared ledge-fall test
                    // 0x8003A500: a drop >384 enters clip34/state3, retains
                    // horizontal speed and starts with zero fall speed.
                    // Subsequent flight and landing use forward collision.
                    player.mode = PlayerMode::JumpingForward;
                    motion.velocity.y = 0.0;
                    traversal.goal = 3;
                    traversal.animation_stepped = true;
                    animation.enter(34, 0.0, 34);
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
            if motion.turning != 0 && motion.turn_rate.abs() > TURN_DECAY && !controls.walk {
                44
            } else if motion.turning < 0 {
                13
            } else if motion.turning > 0 {
                12
            } else {
                103
            }
        }
        PlayerMode::Walk => {
            if controls.direction.y == 0.0 {
                STANDING_CLIP
            } else {
                20
            }
        }
        PlayerMode::Run => {
            if controls.direction.y == 0.0 {
                STANDING_CLIP
            } else {
                6
            }
        }
        PlayerMode::Back => {
            if controls.direction.y < -0.1 && controls.walk {
                40
            } else {
                103
            }
        }
        PlayerMode::HopBack => 88,
        PlayerMode::FallingBack => 93,
        PlayerMode::StepLeft { stopping } => {
            if stopping {
                103
            } else {
                65
            }
        }
        PlayerMode::StepRight { stopping } => {
            if stopping {
                103
            } else {
                67
            }
        }
        PlayerMode::Sliding { backwards, .. } => {
            if backwards {
                105
            } else {
                70
            }
        }
        PlayerMode::SlideStopping { .. } => 71,
        PlayerMode::PreparingJump { frames_left, standing, running, .. } => {
            if running {
                16
            } else if frames_left > 3.0 {
                73
            } else if standing {
                91
            } else {
                76
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
                34
            }
        }
        PlayerMode::Landing { animation, .. } => animation,
        PlayerMode::Hanging { .. } => 96,
        PlayerMode::JumpingUp { .. } => 26,
        PlayerMode::JumpingForward => 76,
        PlayerMode::FastFalling => 23,
        PlayerMode::FatalLanding => 25,
        PlayerMode::JumpImpact => 31,
        PlayerMode::PullingUp => 97,
        PlayerMode::Recovering => 31,
        PlayerMode::WalkStopping => STANDING_CLIP,
        PlayerMode::Climbing { animation, .. } | PlayerMode::HitWall { animation, .. } => animation,
        PlayerMode::Rolling { .. } => 146,
        PlayerMode::Surface => 110,
        PlayerMode::Swimming => {
            if pad.held(Button::Square) {
                86
            } else {
                108
            }
        }
        PlayerMode::Dead => 87,
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
        && matches!(clips.state(current.animation), Some(6 | 7))
        && clips.state(index) == Some(20)
    {
        // Slow-turn goal20 can precede its dispatch window by many ticks.
        // The authored range also chooses44 on the right and69 on the left.
        if let Some((clip, frame)) = clips.dispatch(current, 20) {
            animation.enter(clip, frame, index);
        } else {
            animation.acknowledge(index);
            advance(&mut animation, Some(clips), ticks);
        }
        return animation;
    }
    if let Some(clips) = clips
        && let (Some(state), Some(target)) = (clips.state(current.animation), clips.state(index))
        && (matches!(state, 5 | 16 | 21 | 22)
            || (state == 0 && matches!(index, 11 | 20 | 103))
            || (state == 1 && matches!(index, 6 | 11 | 103))
            || (state == 2 && matches!(index, 6 | 20))
            || (state == 2 && matches!(target, 5 | 16 | 21 | 22)))
    {
        // AnimateLara 0x80042E30..0x80042FD8 advances, tries the goal
        // every tick, then follows end links. A walk, run or sidestep
        // stop request must wait for a foot's authored dispatch window.
        // Run applies only to ordinary run/stand requests: collision-forced
        // clips (e.g. vault50, whose declared state is also2) enter directly.
        let goal = if state == 5 { 2 } else { target };
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
            (Some(from), Some(to)) if matches!(from, 6 | 7 | 20) && to == 2 => {
                clips.and_then(|c| c.dispatch(&animation, 2))
            }
            (Some(2), Some(to)) if matches!(to, 0 | 1) => clips.and_then(|c| c.dispatch(&animation, to)),
            _ => None,
        };
        if let Some((clip, frame)) = entry {
            animation.enter(clip, frame, index);
            return animation;
        }
        if start_frame.is_none() && target == Some(2) && (state == target || ending == target) {
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
fn sync_transform(mut players: Query<(&Player, &Motion, &SubmergedMotion, &mut Transform)>) {
    for (player, motion, submerged, mut transform) in &mut players {
        transform.translation = player.position;
        transform.rotation = Quat::from_rotation_y(player.yaw);
        if matches!(player.mode, PlayerMode::Swimming | PlayerMode::Surface) {
            transform.rotation *= Quat::from_rotation_x(-motion.pitch) * Quat::from_rotation_z(-submerged.bank);
        }
    }
}
