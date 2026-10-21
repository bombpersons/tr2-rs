//! Using an object: picking up, switches, keys and puzzle pieces.
//!
//! The object's collision routine starts the use (`game::interactions`):
//! it lines Lara up, puts her into one of the use clips and marks her hands
//! busy. The clip then plays in place. Its end runs the EmptyHands command
//! and links to standing, which hands control back. While it plays, the
//! state handler asks the chase camera for a set view
//! (`docs/interaction-evidence.md`).

use bevy::prelude::*;
use engine::animation::AnimationState;

use super::{
    Motion, Player, PlayerAnimations, PlayerMode,
    contract::{clip, state},
    weapons::{GunStatus, WeaponState},
};

/// What a use state asks the chase camera for: an angle from Lara's facing
/// and an elevation (65536 per turn), at 1024 units, and for the switch
/// states a faster approach (divisor 6). Handlers 0x80039930 (pickup),
/// 0x80039A20 / 0x80039A8C (switches), 0x80039AF8 (key), 0x80039B58
/// (puzzle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseCamera {
    pub angle: i16,
    pub elevation: i16,
    pub distance: i32,
    pub speed: Option<i32>,
}

pub fn camera(state: usize) -> Option<UseCamera> {
    let (angle, elevation, speed) = match state {
        state::PICKUP => (-23660, -2730, None),
        state::SWITCH_OFF | state::SWITCH_ON => (14560, -4550, Some(6)),
        state::USE_KEY | state::USE_PUZZLE => (-14560, -4550, None),
        _ => return None,
    };
    Some(UseCamera { angle, elevation, distance: 1024, speed })
}

/// Put Lara into a use clip at its first frame, with her hands busy.
pub fn start(
    player: &mut Player,
    motion: &mut Motion,
    animation: &mut AnimationState,
    weapon: &mut WeaponState,
    clip: usize,
) {
    player.mode = PlayerMode::Using { animation: clip };
    motion.velocity = Vec3::ZERO;
    animation.enter(clip, 0.0, clip);
    weapon.status = GunStatus::HandsBusy;
}

/// The state handler's camera request for this tick.
pub(super) fn control(motion: &mut Motion, clips: Option<&PlayerAnimations>, animation: &AnimationState) {
    let Some(request) = clips.and_then(|c| c.state(animation.animation)).and_then(camera) else { return };
    motion.camera_angle = request.angle;
    motion.camera_elevation = Some(super::swimming::radians(i32::from(request.elevation)));
    motion.camera_distance = Some(request.distance);
    motion.camera_speed = request.speed;
}

/// Once the use clip has ended into standing, its EmptyHands command has
/// freed the hands (AnimateLara's end commands, 0x80042F00) and the stand
/// handler takes over next tick. Underwater, Lara stays swimming: the
/// switch clip's EmptyHands frees them as it links back to treading
/// (0x80042F90).
pub(super) fn finish(mut players: Query<(&mut Player, &AnimationState, &mut WeaponState)>) {
    for (mut player, animation, mut weapon) in &mut players {
        if player.mode == PlayerMode::Swimming
            && weapon.status == GunStatus::HandsBusy
            && animation.animation != clip::UNDERWATER_SWITCH
        {
            weapon.status = GunStatus::HandsFree;
        }
        if let PlayerMode::Using { animation: clip } = player.mode
            && animation.animation != clip
        {
            if weapon.status == GunStatus::HandsBusy {
                weapon.status = GunStatus::HandsFree;
            }
            player.mode = PlayerMode::Idle;
        }
    }
}
