//! Riding the zip-line (type 102, `game::traps::zipline`).
//!
//! The line's collision routine starts the ride: Lara standing with free
//! hands and Action held, behind the line's handle and facing along it,
//! is lined up under it and asked for state 70, which standing's range
//! enters through the grab (state 19). The grab plays in place and ends
//! by stepping her onto the line, into the ride loop (state 70). From
//! then the line's control carries her: it moves first in the tick and
//! puts her where it is. She lets go when Action is released (state 70's
//! handler 0x80039E64: speed 100, fall speed 40) or when the line stops
//! (its control: speed and a quarter of it falling, the line's speed).
//! Either way she asks for a forward jump, which the ride's range enters
//! through the drop clip; its end lowers her 240 and frees her hands into
//! ordinary forward flight. Evidence: `docs/trap-evidence.md`.

use bevy::prelude::*;
use engine::animation::{AnimationState, advance};
use engine::camera::chase::{cos, sin};
use engine::controller::{Button, ControllerState};
use engine::world::WorldGeometry;

use super::{
    Motion, Player, PlayerAnimations, PlayerMode, Traversal, binary_angle, body_room,
    contract::{clip, state},
    original_units,
    weapons::{GunStatus, WeaponState},
};

/// Where Lara is in a ride.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStage {
    /// State 19: the grab clip plays in place.
    Grabbing,
    /// State 70: the line carries her.
    Riding,
    /// The line stopped under her this tick: her control lets go at these
    /// speeds (world units a tick). `flight`: the speeds she was already
    /// falling at, if she had let go but still hung on (below).
    LetGo { speed: i32, fall_speed: i32, flight: Option<Flight> },
    /// Letting go: falling at these speeds, through the drop clip (state
    /// 3). Until the ride clip reaches its range for the drop she is
    /// still in state 70 (`on_line`): the line keeps putting her back on
    /// it, and her ride handler runs again.
    Dropping { speed: i32, fall_speed: i32, on_line: bool },
}

/// Lara's speed along her heading and her fall speed, world units a tick.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flight {
    pub speed: i32,
    pub fall_speed: i32,
}

impl Flight {
    /// AnimateLara's flight for one tick (0x80043210..0x80043248): gravity,
    /// six a tick below 128 and one above, then the move along her heading.
    fn fly(&mut self, player: &mut Player) {
        self.fall_speed += if self.fall_speed < 128 { 6 } else { 1 };
        let yaw = i32::from(binary_angle(player.yaw));
        let mut at = original_units(player.position);
        at.x += (sin(yaw) * self.speed) >> 14;
        at.y += self.fall_speed;
        at.z += (cos(yaw) * self.speed) >> 14;
        player.position = engine::world::position(at.to_array());
    }
}

/// The flight a stage is falling with, if any.
fn flight(stage: ZipStage) -> Option<Flight> {
    match stage {
        ZipStage::Dropping { speed, fall_speed, .. } => Some(Flight { speed, fall_speed }),
        _ => None,
    }
}

/// What the ride state's handler asks the chase camera for: an angle
/// from Lara's facing (0x80039E78, `0x8008B29C`).
const RIDE_CAMERA_ANGLE: i16 = 12740;
/// Letting go of Action (0x80039ED4..0x80039EFC).
const RELEASE_SPEED: i32 = 100;
const RELEASE_FALL_SPEED: i32 = 40;

/// Lara is on the line, which moves her (state 70).
pub fn riding(player: &Player) -> bool {
    matches!(player.mode, PlayerMode::ZipLine { stage: ZipStage::Riding | ZipStage::Dropping { on_line: true, .. } })
}

/// Lara can take hold: standing (state 2), on the ground, hands free.
pub fn can_grab(player: &Player, motion: &Motion, weapon: &WeaponState, state: Option<usize>) -> bool {
    player.health > 0 && motion.grounded && weapon.hands_free() && state == Some(state::STAND)
}

/// The grab (0x8004B794..0x8004B7C0): with goal 70, AnimateItem steps her
/// clip until it is in state 19, all in this tick, and her hands are busy.
/// False, with nothing changed, when her table has no way into it.
pub fn grab(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    weapon: &mut WeaponState,
    traversal: &mut Traversal,
    clips: &PlayerAnimations,
) -> bool {
    let mut a = animation.clone();
    // Bounded only against a table with no way in; a walk's foot stop
    // (clip 3) takes a few dozen frames.
    for _ in 0..256 {
        match clips.dispatch(&a, state::ZIP_LINE) {
            Some((clip, frame)) => a.enter(clip, frame, clip),
            None => advance(&mut a, Some(clips), 1.0),
        }
        if clips.state(a.animation) == Some(state::PULL_UP) {
            break;
        }
    }
    if clips.state(a.animation) != Some(state::PULL_UP) {
        return false;
    }
    *animation = AnimationState::at_frame(a.animation, a.frame);
    player.mode = PlayerMode::ZipLine { stage: ZipStage::Grabbing };
    motion.velocity = Vec3::ZERO;
    motion.turn_rate = 0;
    traversal.goal = state::ZIP_LINE;
    weapon.status = GunStatus::HandsBusy;
    true
}

/// The line stopped with Lara on it (0x8004BB00): she lets go on her own
/// control this tick, as fast as it was going.
pub fn line_stopped(player: &mut Player, speed: i32) {
    if let PlayerMode::ZipLine { stage } = player.mode
        && riding(player)
    {
        let flight = flight(stage);
        player.mode = PlayerMode::ZipLine { stage: ZipStage::LetGo { speed, fall_speed: speed >> 2, flight } };
    }
}

/// Lara's control on the line: the ride state's handler, or letting go
/// after the line stopped. True when she is on the line.
pub(super) fn control(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    clips: Option<&PlayerAnimations>,
    pad: &ControllerState,
) -> bool {
    let PlayerMode::ZipLine { stage } = player.mode else { return false };
    match stage {
        ZipStage::Riding | ZipStage::Dropping { on_line: true, .. } => {
            motion.camera_angle = RIDE_CAMERA_ANGLE;
            if !pad.held(Button::Cross) {
                let release = Flight { speed: RELEASE_SPEED, fall_speed: RELEASE_FALL_SPEED };
                let_go(player, motion, animation, traversal, clips, release, flight(stage));
            }
        }
        ZipStage::LetGo { speed, fall_speed, flight } => {
            let_go(player, motion, animation, traversal, clips, Flight { speed, fall_speed }, flight)
        }
        ZipStage::Grabbing | ZipStage::Dropping { .. } => {}
    }
    true
}

/// Ask for a forward jump and run AnimateLara once (0x80042E08): the
/// clip steps, into the drop if the ride's range for it is open, and if
/// she was already falling (`flight`) she flies on. Then she falls at
/// `speeds`. Until the range opens she still hangs in state 70 and comes
/// back here every tick, the line putting her back on it first: each tick
/// she flies twice at 100 and from a fall speed of 40, as the PSX
/// recording of a release shows.
fn let_go(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    clips: Option<&PlayerAnimations>,
    speeds: Flight,
    flight: Option<Flight>,
) {
    traversal.goal = state::FORWARD_JUMP;
    step_clip(animation, clips, state::FORWARD_JUMP);
    if let Some(mut flight) = flight {
        flight.fly(player);
    }
    let on_line = clips.and_then(|c| c.state(animation.animation)) == Some(state::ZIP_LINE);
    let Flight { speed, fall_speed } = speeds;
    player.mode = PlayerMode::ZipLine { stage: ZipStage::Dropping { speed, fall_speed, on_line } };
    motion.grounded = false;
    motion.movement_heading = Some(player.yaw);
}

/// One AnimateLara step towards `goal`: its range if the clip has one
/// open, else the next frame.
fn step_clip(animation: &mut AnimationState, clips: Option<&PlayerAnimations>, goal: usize) {
    let Some(clips) = clips else { return };
    if clips.state(animation.animation) != Some(goal)
        && let Some((clip, frame)) = clips.dispatch(animation, goal)
    {
        animation.enter(clip, frame, clip);
    } else {
        advance(animation, Some(clips), 1.0);
    }
}

/// Lara's own animation and movement on the line, in place of her
/// locomotion. The line moves her; she only animates, and her room
/// follows her body (UpdateLaraRoom 0x8001C0E8). True when she is on it.
#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    traversal: &mut Traversal,
    weapon: &mut WeaponState,
    world: Option<&WorldGeometry>,
    clips: Option<&PlayerAnimations>,
) -> bool {
    let PlayerMode::ZipLine { stage } = player.mode else { return false };
    let Some(clips) = clips else { return true };
    traversal.animation_stepped = true;
    let before = animation.animation;
    match stage {
        ZipStage::Grabbing | ZipStage::Riding | ZipStage::LetGo { .. } => {
            motion.velocity = Vec3::ZERO;
            advance(animation, Some(clips), 1.0);
            if before == clip::ZIP_GRAB && animation.animation != clip::ZIP_GRAB {
                end_offset(player, clips, clip::ZIP_GRAB);
                player.mode = PlayerMode::ZipLine { stage: ZipStage::Riding };
            }
        }
        ZipStage::Dropping { speed, fall_speed, .. } => {
            step_clip(animation, Some(clips), state::FORWARD_JUMP);
            if before == clip::ZIP_DROP && animation.animation != clip::ZIP_DROP {
                end_offset(player, clips, clip::ZIP_DROP);
                // The drop's EmptyHands.
                weapon.status = GunStatus::HandsFree;
            }
            let mut flight = Flight { speed, fall_speed };
            flight.fly(player);
            let Flight { speed, fall_speed } = flight;
            motion.velocity = player.forward() * super::units(speed as f32 * super::ORIGINAL_FPS)
                - Vec3::Y * super::units(fall_speed as f32 * super::ORIGINAL_FPS);
            let on_line = clips.state(animation.animation) == Some(state::ZIP_LINE);
            player.mode = if on_line || animation.animation == clip::ZIP_DROP {
                PlayerMode::ZipLine { stage: ZipStage::Dropping { speed, fall_speed, on_line } }
            } else {
                // Off the drop clip: ordinary forward flight from here.
                traversal.goal = state::FORWARD_JUMP;
                PlayerMode::JumpingForward
            };
        }
    }
    if let Some(world) = world {
        player.room = body_room(world, player.position, player.room, 381.0);
    }
    true
}

/// A clip's end-of-clip SetPosition, turned to Lara's heading.
fn end_offset(player: &mut Player, clips: &PlayerAnimations, clip: usize) {
    let Some((_, [dx, dy, dz])) = clips.climb(clip) else { return };
    let yaw = i32::from(binary_angle(player.yaw));
    let mut at = original_units(player.position);
    at.x += (cos(yaw) * dx + sin(yaw) * dz) >> 14;
    at.y += dy;
    at.z += (cos(yaw) * dz - sin(yaw) * dx) >> 14;
    player.position = engine::world::position(at.to_array());
}
