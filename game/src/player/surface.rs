//! Swimming on the water surface: surfacing, tread/forward/back/side strokes,
//! diving and climbing out, from PSX 0x80044264..0x80044FC4 and the
//! water-mode transitions of LaraControl 0x800424F0.
//!
//! The surface reuses the submerged speed field (item+32, quarter-units per
//! tick), so it lives in [`SubmergedMotion::speed`]. State handlers write yaw
//! directly; the collision pass writes the heading the next tick moves along.
use super::{
    AnimationState, GroundControls, Motion, ORIGINAL_FPS, Player, PlayerAnimations, PlayerMode, SubmergedMotion,
    SwimmingTuning, Traversal, backward::grid_shift, original_frames, swimming, units,
};
use bevy::prelude::*;
use engine::{
    animation::advance,
    camera::chase::{cos, sin},
    controller::{Button, ControllerState},
    world::WorldGeometry,
};

/// Surface states (control table 0x80082C98).
const TREAD: usize = 33;
const FORWARD: usize = 34;
const BACK: usize = 47;
const LEFT: usize = 48;
const RIGHT: usize = 49;
/// Clips the transitions enter directly.
const SURFACING_CLIP: usize = 114;
const DIVE_CLIP: usize = 119;
const ENTRY_CLIP: usize = 112;
const FALL_CLIP: usize = 34;
/// GetHeight's sentinel for a solid or missing sector.
const NO_HEIGHT: i32 = -32512;

/// Speed in the source's quarter-units per tick.
fn quarters(speed: f32) -> i32 {
    (speed * 4096.0 / ORIGINAL_FPS).round() as i32
}
fn from_quarters(quarters: i32) -> f32 {
    units(quarters as f32 / 4.0 * ORIGINAL_FPS)
}
/// Original y-down integer height of a level-space y.
fn original_y(y: f32) -> i32 {
    (-y * 1024.0).round() as i32
}

/// LaraControl's transitions out of submerged swimming, run before the
/// tick's state handler (0x800429A4..0x80042B00). They act only once the
/// previous tick's room update left Lara in a dry room.
pub(super) fn leave_water(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
) {
    if world.data.rooms.get(player.room).is_none_or(|room| room.water) {
        return;
    }
    let below = world.water_surface(player.position, player.room).map(|surface| surface - player.position.y);
    match below {
        // Within 256 of the surface: snap one unit under it and tread.
        Some(depth) if (original_y(player.position.y) - original_y(player.position.y + depth)).abs() < 256 => {
            player.position.y += depth - units(1.0);
            player.mode = PlayerMode::Surface;
            traversal.goal = TREAD;
            animation.enter(SURFACING_CLIP, 0.0, SURFACING_CLIP);
            water.speed = 0.0;
            water.bank = 0.0;
            water.dive_hold = 11; // A held Jump must be released before diving again.
            motion.pitch = 0.0;
            player.room = super::body_room(world, player.position, player.room, 381.0);
        }
        // Otherwise fall out of the water with a quarter of the swim speed.
        _ => {
            let forward = player.forward() * from_quarters(quarters(water.speed) / 4 * 4);
            player.mode = PlayerMode::JumpingForward;
            traversal.goal = 3;
            animation.enter(FALL_CLIP, 0.0, FALL_CLIP);
            motion.velocity = forward;
            motion.grounded = false;
            motion.pitch = 0.0;
            *water = SubmergedMotion { air_ticks: water.air_ticks, ..default() };
        }
    }
}

/// LaraControl's entry test (0x80042624): once the feet are 384 below the
/// surface of water at least 475 deep, the actor starts the authored entry
/// dive (clip112) and its fall speed becomes 3/2 of a swim speed.
#[allow(clippy::too_many_arguments)]
pub(super) fn enter_water(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    tuning: &SwimmingTuning,
    wading: &mut super::Wading,
) {
    let (Some(surface), Some(floor)) =
        (world.water_surface(player.position, player.room), world.floor_at(player.position, player.room))
    else {
        return;
    };
    let below = original_y(player.position.y) - original_y(surface);
    let depth = original_y(floor.height) - original_y(surface);
    if below < 384 {
        return;
    }
    if depth < 475 {
        if below > 384 {
            wading.active = true;
            if motion.grounded {
                traversal.goal = 2;
            }
        }
        return;
    }
    if world.data.rooms.get(player.room).is_none_or(|room| !room.water) {
        return;
    }
    let fall_speed = (-motion.velocity.y * 1024.0 / ORIGINAL_FPS).round() as i32;
    player.mode = PlayerMode::Swimming;
    player.position.y -= units(100.0);
    animation.enter(ENTRY_CLIP, 0.0, ENTRY_CLIP);
    traversal.goal = 17;
    motion.pitch = swimming::radians(-8190);
    motion.grounded = false;
    motion.velocity = Vec3::ZERO;
    motion.air_animation = 0;
    *water = SubmergedMotion {
        speed: from_quarters(fall_speed * 3 / 2),
        bank: water.bank,
        air_ticks: tuning.max_air_ticks,
        dive_hold: 0,
    };
}

/// Surface state handlers (0x800444C0..0x800449B8), after the per-tick air
/// refill of LaraControl's surface branch (0x80042C58).
#[allow(clippy::too_many_arguments)]
pub(super) fn control(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &AnimationState,
    traversal: &mut Traversal,
    pad: &ControllerState,
    clips: Option<&PlayerAnimations>,
    tuning: &SwimmingTuning,
) {
    if player.health >= 0 {
        water.air_ticks = (water.air_ticks + tuning.surface.air_refill).min(tuning.max_air_ticks);
    }
    motion.oxygen_seconds = water.air_ticks.max(0) as f32 / ORIGINAL_FPS;
    motion.turn_rate = 0;
    let Some(state) = clips.and_then(|c| c.state(animation.animation)) else { return };
    if state == 44 {
        // Surface wrapper0x80044364 uses the same state44 controller as
        // submerged motion. Surface death is authored clip132, not124.
        swimming::death_control(motion, water, tuning);
        return;
    }
    let controls = GroundControls::read(pad);
    let held = |x: f32| {
        if x < -0.1 {
            -1
        } else if x > 0.1 {
            1
        } else {
            0
        }
    };
    let turn = held(controls.direction.x);
    let forward = held(controls.direction.y);
    let side = controls.side as i32;
    let surface = &tuning.surface;
    let mut speed = quarters(water.speed);
    let mut turn_by = |amount: i32| {
        player.yaw = swimming::radians((swimming::angle(player.yaw) + turn * amount) as i16 as i32);
    };
    if state == TREAD {
        speed = (speed - surface.drag).max(0);
        water.speed = from_quarters(speed);
        if player.health <= 0 {
            traversal.goal = 44;
            return;
        }
        turn_by(surface.turn);
        if forward > 0 {
            traversal.goal = FORWARD;
        } else if forward < 0 {
            traversal.goal = BACK;
        }
        if side < 0 {
            traversal.goal = LEFT;
        } else if side > 0 {
            traversal.goal = RIGHT;
        }
        if pad.held(Button::Square) {
            water.dive_hold += 1;
            if water.dive_hold == surface.dive_hold_ticks {
                traversal.goal = 17;
            }
        } else {
            water.dive_hold = 0;
        }
        return;
    }
    if player.health <= 0 {
        traversal.goal = 44;
        return;
    }
    let keep = match state {
        FORWARD => {
            water.dive_hold = 0;
            turn_by(surface.turn);
            forward > 0 && !pad.held(Button::Square)
        }
        BACK => {
            turn_by(surface.side_turn);
            forward < 0
        }
        LEFT => {
            turn_by(surface.side_turn);
            side < 0
        }
        RIGHT => {
            turn_by(surface.side_turn);
            side > 0
        }
        _ => return,
    };
    if !keep {
        traversal.goal = TREAD;
    }
    // Every stroke accelerates, even on the tick it asks to tread.
    water.speed = from_quarters((speed + surface.acceleration).min(surface.max_speed));
}

/// Collision probes of GetCollisionInfo 0x8001B018 for the surface: floors
/// relative to y+700, ceilings relative to 800 above that.
struct Probe {
    floor: i32,
    ceiling: i32,
}

fn probe(world: &WorldGeometry, at: Vec3, room: usize, base: i32) -> Probe {
    // Rooms are resolved 160 above the ceiling line (0x8001B080).
    let resolve = at.with_y(-((base - 800 - 160) as f32) / 1024.0);
    match world.floor_at(resolve, room) {
        Some(hit) => Probe { floor: original_y(hit.height) - base, ceiling: original_y(hit.ceiling) - (base - 800) },
        None => Probe { floor: NO_HEIGHT, ceiling: NO_HEIGHT },
    }
}

#[derive(PartialEq)]
enum Contact {
    None,
    /// Front, clamp or top: the move is undone.
    Front,
    Left,
    Right,
}

/// One tick of the surface wrapper after control: authored animation,
/// movement along the previous collision heading, then state collision.
#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    action: bool,
    tuning: &SwimmingTuning,
    wading: &mut super::Wading,
    dt: f32,
) {
    let Some(clips) = clips else { return };
    // Roll recovers 364 per tick; the surface has no turn-rate inertia.
    water.bank = swimming::radians(swimming::decay(swimming::angle(water.bank), 364));
    if clips.state(animation.animation) != Some(traversal.goal)
        && let Some((clip, frame)) = clips.dispatch(animation, traversal.goal)
    {
        animation.enter(clip, frame, clip);
    } else {
        advance(animation, Some(clips), original_frames(dt));
    }
    traversal.animation_stepped = true;
    let heading = swimming::angle(motion.movement_heading.unwrap_or(player.yaw));
    let speed = quarters(water.speed);
    let old = player.position;
    player.position +=
        Vec3::new(units(((sin(heading) * speed) >> 16) as f32), 0.0, units(((cos(heading) * speed) >> 16) as f32))
            * original_frames(dt);
    motion.velocity = (player.position - old) / dt;
    motion.grounded = false;
    let Some(world) = world else { return };
    let state = clips.state(animation.animation).unwrap_or(TREAD);
    if state == 44 {
        swimming::death_collision(player, motion, water, Some(world), old, tuning, dt);
        player.room = super::body_room(world, player.position, player.room, -100.0);
        return;
    }
    if state == TREAD && traversal.goal == 17 {
        dive(player, motion, water, animation);
    }
    let yaw = swimming::angle(player.yaw);
    let facing = match state {
        BACK => yaw + 0x8000,
        LEFT => yaw - 0x4000,
        RIGHT => yaw + 0x4000,
        _ => yaw,
    } as i16 as i32;
    motion.movement_heading = Some(swimming::radians(facing));
    let base = original_y(player.position.y) + 700;
    let bad_neg = if state == FORWARD { -384 } else { -128 };
    let blocked = |p: &Probe| p.floor < bad_neg || p.ceiling > 100;
    let mid = probe(world, player.position, player.room, base);
    let [front_at, left_at, right_at] = super::backward::probes(player.position, swimming::radians(facing));
    let front = probe(world, front_at, player.room, base);
    let left = probe(world, left_at, player.room, base);
    let right = probe(world, right_at, player.room, base);
    let quadrant = ((facing + 8192) >> 14).rem_euclid(4);
    let contact = if mid.floor == NO_HEIGHT || mid.floor - mid.ceiling <= 0 || mid.ceiling >= 0 || blocked(&front) {
        Contact::Front
    } else if blocked(&left) {
        Contact::Left
    } else if blocked(&right) {
        Contact::Right
    } else {
        Contact::None
    };
    // ShiftItem pushes a side contact back out of the probe's sector.
    let shift = |probe: Vec3, position: &mut Vec3| {
        if quadrant % 2 == 0 {
            position.x += grid_shift(probe.x, position.x);
        } else {
            position.z += grid_shift(probe.z, position.z);
        }
    };
    match contact {
        Contact::Front => {
            water.speed = 0.0;
            player.position = old;
        }
        Contact::Left => {
            shift(left_at, &mut player.position);
            player.yaw = swimming::radians((yaw + 910) as i16 as i32);
        }
        Contact::Right => {
            shift(right_at, &mut player.position);
            player.yaw = swimming::radians((yaw - 910) as i16 as i32);
        }
        Contact::None => {}
    }
    if contact == Contact::Front
        && state == FORWARD
        && climb_out(player, motion, water, animation, traversal, world, action, &front, &left, &right, &mid)
    {
        return;
    }
    // LaraTestWaterStepOut0x80044BB4. A floor less than128 above the
    // probe line keeps a side-step goal, otherwise it uses the authored190.
    // Front contact cannot step out; the Action climb was tested above.
    if contact != Contact::Front && mid.floor < 0 {
        let clip = if mid.floor < -128 { 190 } else { 177 };
        if clip == 190 {
            traversal.goal = 2;
        } else {
            traversal.goal = match traversal.goal {
                48 => 22,
                49 => 21,
                _ => 65,
            };
        }
        animation.enter(clip, 0.0, clip);
        player.position.y -= units((695 + front.floor) as f32);
        player.room = super::body_room(world, player.position, player.room, 381.0);
        player.mode = if clip == 190 { PlayerMode::PullingUp } else { PlayerMode::Wade };
        motion.grounded = true;
        motion.velocity = Vec3::ZERO;
        motion.pitch = 0.0;
        water.speed = 0.0;
        water.bank = 0.0;
        wading.active = true;
        return;
    }
    // The surface wrapper updates the room 100 below the feet.
    player.room = super::body_room(world, player.position, player.room, -100.0);
}

/// Tread's dive (0x800449C0): authored clip119 heading down at 45 degrees.
fn dive(player: &mut Player, motion: &mut Motion, water: &mut SubmergedMotion, animation: &mut AnimationState) {
    player.mode = PlayerMode::Swimming;
    animation.enter(DIVE_CLIP, 0.0, DIVE_CLIP);
    motion.pitch = swimming::radians(-8190);
    water.speed = from_quarters(80);
}

/// Climb-out test 0x80044CE0: a front contact with Action held onto a flat
/// ledge 384..1211 above the probe line, facing within 35 degrees of an axis.
#[allow(clippy::too_many_arguments)]
fn climb_out(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: &WorldGeometry,
    action: bool,
    front: &Probe,
    left: &Probe,
    right: &Probe,
    mid: &Probe,
) -> bool {
    if !action
        || (left.floor - right.floor).abs() >= 60
        || front.ceiling > 0
        || mid.ceiling > -384
        || !(-1211..=-384).contains(&front.floor)
    {
        return false;
    }
    let yaw = swimming::angle(player.yaw);
    let cardinal = match yaw {
        -6370..=6370 => 0,
        10014..=22754 => 16384,
        26397..=32767 | -32768..=-32397 => -32768,
        -22754..=-10014 => -16384,
        _ => return false,
    };
    let lift = front.floor + 700;
    player.position.y -= units((lift - 5) as f32);
    let snap = |axis: f32, ahead: bool| {
        let base = ((axis * 1024.0).round() as i32) & !1023;
        units((base + if ahead { 1124 } else { -100 }) as f32)
    };
    match cardinal {
        0 => player.position.z = snap(player.position.z, true),
        16384 => player.position.x = snap(player.position.x, true),
        -32768 => player.position.z = snap(player.position.z, false),
        _ => player.position.x = snap(player.position.x, false),
    }
    player.room = super::body_room(world, player.position, player.room, 381.0);
    let clip = if lift < -128 {
        111
    } else if lift < 128 {
        191
    } else {
        193
    };
    animation.enter(clip, 0.0, clip);
    traversal.goal = 2;
    player.yaw = swimming::radians(cardinal);
    player.mode = PlayerMode::PullingUp;
    motion.pitch = 0.0;
    motion.velocity = Vec3::ZERO;
    motion.grounded = true;
    *water = SubmergedMotion { air_ticks: water.air_ticks, ..default() };
    true
}
