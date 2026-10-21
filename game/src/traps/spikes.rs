//! Floor spikes (SpikeCollision `0x8005F378`). They have no control
//! routine: everything happens when Lara touches them. Standing still or
//! moving slowly is safe; running or walking fast through them costs 15
//! a tick, and landing on them from a fall kills her outright on the
//! impaled clip.
use bevy::prelude::*;

use super::{AppTraps, TrapCollision, TrapType};
use crate::player::{PlayerMode, contract::clip};

pub const SPIKES: u16 = 59;

/// Below this forward speed Lara on the ground is not hurt.
const SAFE_SPEED: i32 = 30;
/// A fall faster than this onto the spikes is fatal.
const SAFE_FALL: i32 = 6;
const STAB: i32 = 15;
/// Blood drops for a fatal landing; otherwise a random 0 or 1.
const FATAL_DROPS: i32 = 20;

pub(super) fn register(app: &mut App) {
    app.register_trap(SPIKES, TrapType::new("spikes").with_collision(spikes));
}

/// SpikeCollision `0x8005F378`.
fn spikes(c: &mut TrapCollision) {
    if c.player.health < 0 || c.touch(false) == 0 {
        return;
    }
    let mut drops = c.random.draw() / 0x6000;
    if !c.lara.airborne {
        if c.lara.speed < SAFE_SPEED {
            return;
        }
    } else if c.lara.fall_speed > SAFE_FALL {
        c.player.health = -1;
        drops = FATAL_DROPS;
    }
    c.player.health -= STAB;
    // Each blood drop's offset and heading draw four times.
    for _ in 0..drops * 4 {
        c.random.draw();
    }
    if c.player.health <= 0 {
        c.player.mode = PlayerMode::Killed { animation: clip::IMPALED, set_camera: false };
        c.pose.enter(clip::IMPALED, 0.0, clip::IMPALED);
        c.player.position.y = -(c.body.position[1] as f32) / 1024.0;
        c.motion.velocity = Vec3::ZERO;
        c.motion.grounded = true;
    }
}
