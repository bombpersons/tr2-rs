//! Camera rules backed by the supplied PSX executable and runtime captures.
//!
//! See docs/camera-evidence.md and docs/chase-camera-evidence.md. Fixed shots
//! and the chase camera (ideal eye, smart shift, line of sight, movement) are
//! ported from the executable; the L1 look camera remains provisional.
use std::collections::BTreeSet;

pub mod chase;

use bevy::prelude::*;

use crate::{
    animation::{AnimationState, vertical_bounds},
    controller::{Button, ControllerState},
    schedule::{Avatar, WorldSystems, simulation_enabled},
    shell::GameSystems,
    world::{Level, WorldGeometry, position},
};

/// A fixed camera shot requested by a floor trigger this tick.
#[derive(Clone, Copy, Debug)]
pub struct CameraCue {
    /// Index into the level's fixed cameras.
    pub index: usize,
    /// Shot length in seconds; 0 = until the avatar leaves the trigger.
    pub timer: u8,
    /// Play this shot only once per level.
    pub once: bool,
    /// Approach speed: the eye closes 1/(8*speed+1) of the gap per tick.
    pub speed: u8,
    pub activate: bool,
    pub restart: bool,
}

/// Written by floor-trigger dispatch, consumed after player animation.
#[derive(Resource, Default)]
pub struct CameraCues(pub Vec<CameraCue>);

#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct CameraState {
    pub position: Vec3,
    pub target: Vec3,
    pub room: usize,
    /// Render-only height offset keeping the view inside floor/ceiling
    /// (0x8008b27c); `position` is the stored eye the original samples.
    pub view_offset: f32,
    pub initialized: bool,
    pub fixed_camera: Option<usize>,
    /// None means untimed; zero means expired until this trigger is left.
    pub ticks_left: Option<u32>,
    last_camera: Option<usize>,
    #[reflect(ignore)]
    used: BTreeSet<usize>,
}

/// How the camera frames an avatar that starts the level falling: it first
/// frames the avatar's standing clip (none = the clip it starts in).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct CameraStartup {
    pub standing_clip: Option<usize>,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CameraSystems {
    Initialize,
    Update,
}

pub struct CameraRulesPlugin;
impl Plugin for CameraRulesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            initialize_camera.in_set(CameraSystems::Initialize).in_set(WorldSystems::Prepare).in_set(GameSystems::Game),
        );
        app.init_resource::<CameraCues>().init_resource::<CameraState>().init_resource::<CameraStartup>().add_systems(
            FixedUpdate,
            update_camera.in_set(CameraSystems::Update).in_set(WorldSystems::Camera).in_set(GameSystems::Game).run_if(simulation_enabled),
        );
    }
}

fn initialize_camera(
    world: Option<Res<WorldGeometry>>,
    players: Query<(&Avatar, Option<&AnimationState>)>,
    mut camera: ResMut<CameraState>,
    startup: Res<CameraStartup>,
) {
    let Some(world) = world.filter(|w| w.is_changed()) else { return };
    let Ok((player, animation)) = players.single() else { return };
    *camera = default();
    camera.used.extend(world.data.cameras.iter().enumerate().filter_map(|(i, c)| c.once.then_some(i)));
    if !player.falling {
        return;
    }
    // 0x80017da8 seeds the look target 1024 units above the spawn.
    // Initialization calls the camera with the avatar's standing clip first
    // (0x80017e68), then the first level draw frames the fall pose (the first
    // game's falling start records target y 512 -> 622 -> 709).
    let seed = player.position + Vec3::Y;
    let standing = startup.standing_clip.map(|clip| AnimationState::at_frame(clip, 0.0));
    let startup = actor_target(&world.data, player, standing.as_ref().or(animation));
    let target = approach_eye(seed, startup, 4);
    camera.target = actor_target(&world.data, player, animation)
        .with_y(approach_eye(target, actor_target(&world.data, player, animation), 4).y);
    camera.position = seed - Vec3::Z * (100.0 / 1024.0);
    camera.room = player.room;
    camera.initialized = true;
}

/// PSX 0x80017e9c..0x80017f7c: move 1/divisor of the remaining distance,
/// truncating each axis towards zero in original world units. This affects
/// the last few pixels of a stationary shot and is kept at the unit boundary.
pub fn approach_eye(current: Vec3, desired: Vec3, divisor: u32) -> Vec3 {
    let current = (current * 1024.0).round().as_ivec3();
    let desired = (desired * 1024.0).round().as_ivec3();
    let step = (desired - current) / divisor.max(1) as i32;
    (current + step).as_vec3() / 1024.0
}

/// PSX 0x80019d64..0x80019d88 frames the upper quarter of the animated bounds.
fn actor_target(data: &Level, player: &Avatar, animation: Option<&AnimationState>) -> Vec3 {
    let Some((min_y, max_y)) = animation.and_then(|a| vertical_bounds(data, a)) else {
        return player.position + Vec3::Y * 0.65;
    };
    let offset = max_y + (((min_y - max_y) * 3) >> 2);
    player.position - Vec3::Y * offset as f32 / 1024.0
}

/// Existing chase framing/collision, now evaluated once per logic tick.
/// These distances are provisional port choices, not newly verified PSX rules.
pub fn chase_pose(player: &Avatar, world: Option<&WorldGeometry>, look: bool) -> (Vec3, Vec3, usize) {
    let target = player.position + Vec3::Y * 0.65;
    let mut desired =
        target - Vec3::new(player.yaw.sin(), 0.0, player.yaw.cos()) * if look { 1.0 } else { 2.25 } + Vec3::Y * if look { 0.6 } else { 0.55 };
    let mut room = player.room;
    if let Some(world) = world {
        let mut safe = target;
        for i in 1..=24 {
            let point = target.lerp(desired, i as f32 / 24.0);
            let Some(hit) = world.floor_at(point, room) else { break };
            if point.y < hit.height + 0.08 || point.y > hit.ceiling - 0.08 {
                break;
            }
            safe = point;
            room = hit.room;
        }
        desired = safe;
    }
    (desired, target, room)
}

/// Chase mode (0x8001a02c..0x8001a1cc then 0x80019214): the look target is
/// the actor's x/z with the bounds height approached by a quarter per tick.
fn follow(camera: &mut CameraState, world: &WorldGeometry, player: &Avatar, animation: Option<&AnimationState>) {
    use chase::Point;
    let wanted = actor_target(&world.data, player, animation);
    let target_y = if camera.initialized { approach_eye(camera.target, wanted, 4).y } else { wanted.y };
    let mut target = Point::from_world(wanted.with_y(target_y), player.room);
    // 0x8001a154..0x8001a198: camera geometry stays on unless the target is
    // below its floor.
    let hit = world.floor_at_for(target.world(), player.room, true);
    target.room = hit.map_or(player.room, |hit| hit.room);
    let space = chase::Space { world, camera: hit.is_some_and(|hit| hit.height <= target.world().y) };
    let mut eye = Point::from_world(camera.position, camera.room);
    let yaw = (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32;
    let (mut ideal, distance2) = chase::ideal_eye(target, eye.room, yaw, 0, chase::CHASE_DISTANCE);
    if !camera.initialized {
        eye = ideal;
    }
    chase::shift_ideal(space, target, eye, &mut ideal, distance2);
    // 0x8001a22c: divisor 10 for chase.
    let offset = chase::move_eye(space, target, &mut eye, ideal, 10);
    camera.position = eye.world();
    camera.target = target.world();
    camera.room = eye.room;
    camera.view_offset = -offset as f32 / 1024.0;
}

fn update_camera(
    world: Option<Res<WorldGeometry>>,
    players: Query<(&Avatar, Option<&AnimationState>)>,
    cues: Res<CameraCues>,
    mut camera: ResMut<CameraState>,
    pad: Res<ControllerState>,
    time: Res<Time>,
) {
    let Ok((player, animation)) = players.single() else { return };
    let Some(world) = world else { return };
    let previous = camera.last_camera;
    let cue = cues.0.iter().rev().find(|cue| {
        world.data.cameras.get(cue.index).and_then(|c| c.room).is_some_and(|room| room < world.data.rooms.len())
            && (previous == Some(cue.index) || (cue.activate && !camera.used.contains(&cue.index)))
    });
    let restarting = cue.is_some_and(|cue| cue.activate && cue.restart && !camera.used.contains(&cue.index));
    if let Some(cue) = cue.filter(|cue| previous != Some(cue.index) || restarting) {
        // Dispatcher 0x8001f3b0: camera timer bytes are always seconds * 30.
        camera.ticks_left = match cue.timer {
            0 => None,
            seconds => Some(u32::from(seconds) * 30),
        };
        if cue.once {
            camera.used.insert(cue.index);
        }
    }
    camera.last_camera = cue.map(|cue| cue.index);
    let shot = cue.filter(|_| camera.ticks_left != Some(0));
    camera.fixed_camera = shot.map(|cue| cue.index);
    if let Some(cue) = shot {
        let record = world.data.cameras[cue.index];
        let eye = position(record.position);
        // 0x8001f3ec: authored speed is 8*n+1 on activation. The end-of-tick
        // reset at 0x8001a22c restores divisor 10 for continuing fixed shots.
        let divisor =
            if previous == Some(cue.index) && !restarting { 10 } else { u32::from(cue.speed) * 8 + 1 };
        if !camera.initialized {
            let (eye, target, _) = chase_pose(player, Some(&world), false);
            camera.position = eye;
            camera.target = target;
        }
        camera.position = approach_eye(camera.position, eye, divisor);
        // 0x8001a100..0x8001a128: target Y closes one quarter of the gap.
        let target = actor_target(&world.data, player, animation);
        let y = approach_eye(camera.target, target, 4).y;
        camera.target = target.with_y(y);
        camera.room = record.room.unwrap_or(0);
        camera.view_offset = 0.0;
        if let Some(ticks) = camera.ticks_left.as_mut() {
            *ticks = ticks.saturating_sub(1);
        }
    } else if pad.held(Button::L1) {
        let (eye, target, room) = chase_pose(player, Some(&world), true);
        camera.position =
            if camera.initialized { camera.position.lerp(eye, 1.0 - (-10.0 * time.delta_secs()).exp()) } else { eye };
        camera.target = target;
        camera.room = room;
        camera.view_offset = 0.0;
    } else {
        follow(&mut camera, &world, player, animation);
    }
    camera.initialized = true;
}
