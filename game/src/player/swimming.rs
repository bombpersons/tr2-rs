//! Submerged stroke, glide and tread, from PSX0x80045384..0x800456DC, and
//! their shared collision response 0x80045CB8.
use super::collision::{self, Contact};
use super::{AnimationState, Motion, ORIGINAL_FPS, Player, PlayerAnimations, Traversal, original_frames, units};
use bevy::prelude::*;
use engine::{
    animation::advance,
    camera::chase::{cos, sin},
    controller::{Button, ControllerState},
    world::WorldGeometry,
};

#[derive(Resource, Reflect, serde::Deserialize, Debug, Clone)]
#[reflect(Resource)]
pub struct SwimmingTuning {
    /// Source units per authored tick; acceleration/drag are their per-tick changes.
    pub max_speed: f32,
    pub max_air_ticks: i32,
    pub drowning_damage: i32,
    /// Underwater death0x8004570C /0x80045810: source units per tick.
    pub death_drag: f32,
    pub death_rise: f32,
    pub death_surface_margin: f32,
    pub acceleration: f32,
    pub drag: f32,
    pub tread_threshold: f32,
    /// Binary angle units (65536 per turn), retained for exact authored ticks.
    pub pitch_step: i32,
    pub pitch_limit: i32,
    pub turn_push: i32,
    pub turn_limit: i32,
    pub turn_drag: i32,
    pub bank_push: i32,
    pub bank_drag: i32,
    pub bank_limit: i32,
    pub surface: SurfaceTuning,
}

/// Surface swimming (0x80044264..0x80044FC4); speeds in the source's
/// quarter-units per tick, turns in binary angle units per tick.
#[derive(Reflect, serde::Deserialize, Debug, Clone)]
pub struct SurfaceTuning {
    pub max_speed: i32,
    pub acceleration: i32,
    pub drag: i32,
    pub turn: i32,
    pub side_turn: i32,
    pub dive_hold_ticks: i32,
    pub air_refill: i32,
}

#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct SubmergedMotion {
    /// Forward water speed, level metres/second. The source stores quarters
    /// of an original unit/tick in item+32; conversion happens at movement.
    pub speed: f32,
    /// Body bank, radians. Positive source roll is represented by negative Z rotation.
    pub bank: f32,
    /// The source's oxygen counter, one per authored tick (0x8008C4FE).
    pub air_ticks: i32,
    /// Ticks Jump has been held while treading at the surface (0x8008C500).
    pub dive_hold: i32,
}
impl Default for SubmergedMotion {
    fn default() -> Self {
        Self { speed: 0.0, bank: 0.0, air_ticks: 1800, dive_hold: 0 }
    }
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    /// State35: the authored dive from the surface or a jump into water.
    Diving,
    Treading,
    Stroking,
    Gliding,
    Dying,
}
impl Phase {
    fn from_clip(state: usize) -> Option<Self> {
        match state {
            13 => Some(Self::Treading),
            17 => Some(Self::Stroking),
            18 => Some(Self::Gliding),
            35 => Some(Self::Diving),
            44 => Some(Self::Dying),
            _ => None,
        }
    }
}
pub(super) fn angle(radians: f32) -> i32 {
    (radians * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32
}
pub(super) fn radians(angle: i32) -> f32 {
    angle as f32 * std::f32::consts::TAU / 65536.0
}
pub(super) fn decay(value: i32, amount: i32) -> i32 {
    value - value.signum() * value.abs().min(amount)
}

/// The underwater wrapper applies angular recovery even when a state rejects
/// input (0x800450E4..0x800451CC). Death and entry skip active steering.
fn recover_angles(player: &mut Player, motion: &mut Motion, water: &mut SubmergedMotion, tuning: &SwimmingTuning) {
    motion.pitch = radians(angle(motion.pitch).clamp(-tuning.pitch_limit, tuning.pitch_limit));
    motion.turn_rate = decay(motion.turn_rate, tuning.turn_drag);
    water.bank = radians(decay(angle(water.bank), tuning.bank_drag));
    player.yaw = radians((angle(player.yaw) + motion.turn_rate) as i16 as i32);
}

/// State44's controller is shared by submerged and surface wrappers
/// (0x80045728..0x8004577C); each wrapper owns its own angular recovery.
pub(super) fn death_control(motion: &mut Motion, water: &mut SubmergedMotion, tuning: &SwimmingTuning) {
    water.speed = (water.speed - units(tuning.death_drag * ORIGINAL_FPS)).max(0.0);
    motion.pitch = radians(decay(angle(motion.pitch), tuning.pitch_step));
}

// Keep the independently owned ECS components explicit at the controller boundary.
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
    let Some(phase) = clips.and_then(|c| c.state(animation.animation)).and_then(Phase::from_clip) else { return };
    // LaraControl0x80042B10 normalizes a dead actor before oxygen accounting.
    //0x80042BF8 skips oxygen/damage for negative health; the lethal tick
    // itself may still reach zero or a negative value before state collision.
    if player.health <= 0 {
        player.health = -1;
    } else {
        water.air_ticks = (water.air_ticks - 1).max(-1);
        if water.air_ticks < 0 {
            // Do not switch to grounded death: authored water dispatch still
            // owns the animation, speed and collision on this tick.
            player.health -= tuning.drowning_damage;
        }
    }
    motion.oxygen_seconds = water.air_ticks.max(0) as f32 / ORIGINAL_FPS;
    if let Phase::Dying = phase {
        //0x80045728..0x8004577C brakes by8 quarter-units and levels pitch
        // by364 binary units; neither steering nor Jump changes the goal.
        death_control(motion, water, tuning);
        recover_angles(player, motion, water, tuning);
        return;
    }
    if player.health <= 0 && !matches!(phase, Phase::Diving) {
        traversal.goal = 44;
        recover_angles(player, motion, water, tuning);
        return;
    }
    if let Phase::Diving = phase {
        // Dive control0x800456DC only steers down; it keeps its entry speed.
        // The wrapper's turn and bank decay still apply.
        if pad.held(Button::Up) {
            motion.pitch = radians(angle(motion.pitch) - tuning.pitch_step / 2);
        }
        recover_angles(player, motion, water, tuning);
        return;
    }
    let pitch = angle(motion.pitch)
        + if pad.held(Button::Up) {
            -tuning.pitch_step
        } else if pad.held(Button::Down) {
            tuning.pitch_step
        } else {
            0
        };
    motion.pitch = radians(pitch.clamp(-tuning.pitch_limit, tuning.pitch_limit));
    let turn = if pad.held(Button::Left) {
        -1
    } else if pad.held(Button::Right) {
        1
    } else {
        0
    };
    motion.turn_rate = (motion.turn_rate + turn * tuning.turn_push).clamp(-tuning.turn_limit, tuning.turn_limit);
    motion.turn_rate = decay(motion.turn_rate, tuning.turn_drag);
    let bank = decay(angle(water.bank) + turn * tuning.bank_push, tuning.bank_drag)
        .clamp(-tuning.bank_limit, tuning.bank_limit);
    water.bank = radians(bank);
    // Underwater turning is applied before movement, unlike ordinary ground integration.
    player.yaw = radians((angle(player.yaw) + motion.turn_rate) as i16 as i32);
    match phase {
        Phase::Stroking => {
            water.speed =
                (water.speed + units(tuning.acceleration * ORIGINAL_FPS)).min(units(tuning.max_speed * ORIGINAL_FPS));
            if !pad.held(Button::Square) {
                traversal.goal = 18;
            }
        }
        Phase::Gliding => {
            if pad.held(Button::Square) {
                traversal.goal = 17;
            }
            water.speed = (water.speed - units(tuning.drag * ORIGINAL_FPS)).max(0.0);
            if water.speed < units(tuning.tread_threshold * ORIGINAL_FPS) {
                traversal.goal = 13;
            }
        }
        Phase::Diving | Phase::Dying => {}
        Phase::Treading => {
            if pad.held(Button::Square) {
                traversal.goal = 17;
            }
            water.speed = (water.speed - units(tuning.drag * ORIGINAL_FPS)).max(0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
    tuning: &SwimmingTuning,
    dt: f32,
) {
    let Some(clips) = clips else { return };
    if clips.state(animation.animation) != Some(traversal.goal)
        && let Some((clip, frame)) = clips.dispatch(animation, traversal.goal)
    {
        animation.enter(clip, frame, clip);
    } else {
        advance(animation, Some(clips), original_frames(dt));
    }
    // AnimateLara's default links change the current state but retain its goal.
    // This is observable when glide199 ends into tread108 while goal18 persists.
    traversal.animation_stepped = true;
    let (pitch, yaw) = (angle(motion.pitch), angle(player.yaw));
    let thrust = (water.speed * 4096.0 / ORIGINAL_FPS).round() as i32;
    //0x80045224..0x800452D4: round each horizontal thrust component first,
    // then apply pitch's cosine; vertical thrust uses a signed shift directly.
    let delta = Vec3::new(
        units(((((sin(yaw) * thrust) >> 16) * cos(pitch)) >> 14) as f32),
        units(((sin(pitch) * thrust) >> 16) as f32),
        units(((((cos(yaw) * thrust) >> 16) * cos(pitch)) >> 14) as f32),
    ) * original_frames(dt);
    let old = player.position;
    player.position += delta;
    motion.grounded = false;
    if clips.state(animation.animation) == Some(44) {
        death_collision(player, motion, water, world, old, tuning, dt);
    } else if let Some(world) = world {
        collide(player, motion, water, world, old);
    }
    if let Some(world) = world {
        // UpdateLaraRoom at the feet; a dry room hands over to LaraControl's
        // surfacing test next tick.
        player.room = super::body_room(world, player.position, player.room, 0.0);
    }
    motion.velocity = (player.position - old) / dt;
}

/// State44's shared collision0x80045810, selected by the surface wrapper at
///0x80044478 as well as the submerged wrapper. Float first, then body collision.
pub(super) fn death_collision(
    player: &mut Player,
    motion: &mut Motion,
    water: &mut SubmergedMotion,
    world: Option<&WorldGeometry>,
    old: Vec3,
    tuning: &SwimmingTuning,
    dt: f32,
) {
    player.health = -1;
    water.air_ticks = -1;
    motion.oxygen_seconds = 0.0;
    let Some(world) = world else { return };
    if world.water_surface(player.position, player.room).is_some_and(|surface| {
        (surface * 1024.0).round() > (player.position.y * 1024.0).round() + tuning.death_surface_margin
    }) {
        player.position.y += units(tuning.death_rise) * original_frames(dt);
    }
    collide(player, motion, water, world, old);
}

/// Submerged state collision 0x80045CB8 (states 13/17/18/35): a body whose
/// height follows pitch, GetCollisionInfo with radius 300, then pitch/yaw
/// deflection, the floor push and the water-depth test 0x80045BA4.
fn collide(player: &mut Player, motion: &mut Motion, water: &mut SubmergedMotion, world: &WorldGeometry, old: Vec3) {
    let mut pitch = angle(motion.pitch);
    let mut yaw = angle(player.yaw);
    // Beyond vertical the body faces backwards; this is also the heading a
    // surfacing tick moves along.
    let facing = if (pitch + 16384) & 0xffff > 0x8000 { yaw - 0x8000 } else { yaw } as i16 as i32;
    motion.movement_heading = Some(radians(facing));
    let height = ((sin(pitch) * 762) >> 14).abs().max(200);
    let mut at = collision::original(player.position);
    let limits = collision::Limits { radius: 300, bad_pos: 32512, bad_neg: -height, bad_ceiling: 400 };
    let info = collision::info(
        world,
        at + IVec3::Y * (height / 2),
        collision::original(old),
        player.room,
        height,
        facing,
        &limits,
    );
    at += info.shift;
    match info.contact {
        Contact::Front if pitch > 6370 => pitch += 364,
        Contact::Front if pitch < -6370 => pitch -= 364,
        Contact::Front | Contact::TopFront => water.speed = 0.0,
        Contact::Top if pitch >= -8190 => pitch -= 364,
        Contact::Left => yaw += 910,
        Contact::Right => yaw -= 910,
        Contact::Clamp => {
            player.position = old;
            water.speed = 0.0;
            return;
        }
        _ => {}
    }
    if info.mid.floor < 0 {
        at.y += info.mid.floor;
        pitch += 364;
    }
    player.position = collision::level(at);
    motion.pitch = radians(pitch as i16 as i32);
    player.yaw = radians(yaw as i16 as i32);
    // GetWaterDepth finds no water here: undo the move. Shallow water (depth
    // 512 or less) starts wading in the original, which is not ported.
    if world.water_surface(player.position, player.room).is_none() {
        player.position = old;
        water.speed = 0.0;
    }
}
