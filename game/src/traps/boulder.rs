//! The rolling boulder (RollingBallControl `0x8005EB84`, its collision
//! `0x8005EFD8`). Triggered, it rolls on its clip, dropping off ledges
//! under gravity, until the floor ahead rises above it; there it stops
//! for good. Once its trigger lets go it goes back to where it was placed.
//! It crushes Lara on the ground and hits her for 100 in the air.
use bevy::prelude::*;
use engine::camera::chase::{cos, sin};

use super::{AppTraps, TrapCollision, TrapContext, TrapType, first_clip};
use crate::items::ItemStatus;
use crate::player::{PlayerMode, contract::clip};

pub const BOULDER: u16 = 60;

/// Boulder states (its clips').
const RESTING: usize = 0;
const ROLLING: usize = 1;
/// The goal of the routine's two other boulder types as they stop.
const STOPPING: usize = 2;

/// How far ahead it looks for a rise that stops it.
const LOOKAHEAD: i32 = 384;
/// Fall speed it leaves a ledge with.
const DROP_SPEED: i16 = -10;
/// Within this of the floor it lands.
const LANDING: i32 = 256;
const HIT: i32 = 100;
/// Blood drops when it crushes Lara.
const CRUSH_DROPS: i32 = 15;

pub(super) fn register(app: &mut App) {
    app.register_trap(BOULDER, TrapType::new("boulder").with_control(boulder).with_collision(boulder_collision));
}

/// RollingBallControl `0x8005EB84`.
fn boulder(c: &mut TrapContext) {
    match c.body.status {
        ItemStatus::Active => {}
        ItemStatus::Deactivated => {
            if !c.trigger_active() {
                reset(c);
            }
            return;
        }
        _ => return,
    }
    if c.goal.state == STOPPING {
        c.animate();
        return;
    }
    if c.body.position[1] < c.body.floor {
        if !c.body.gravity {
            c.body.gravity = true;
            c.body.fall_speed = DROP_SPEED;
        }
    } else if c.body.current_state == RESTING {
        c.goal.state = ROLLING;
    }
    let [old_x, _, old_z] = c.body.position;
    c.animate();
    let floor = c.settle_room();
    c.body.floor = floor;
    // It also sets off heavy triggers under it (TestTriggers with heavy
    // set, 0x8005ED4C); the engine does not dispatch those yet.
    if floor - LANDING <= c.body.position[1] {
        c.body.fall_speed = 0;
        c.body.gravity = false;
        c.body.position[1] = floor;
        // Its landing sound and the camera shake near it are presentation.
    }
    let yaw = i32::from(c.body.yaw);
    let [x, y, z] = c.body.position;
    let ahead = c.height_at(x + (LOOKAHEAD * sin(yaw) >> 14), y, z + (LOOKAHEAD * cos(yaw) >> 14));
    if y <= ahead {
        return;
    }
    c.body.status = ItemStatus::Deactivated;
    c.body.position[0] = old_x;
    c.body.position[2] = old_z;
    c.body.fall_speed = 0;
    c.body.speed = 0;
    c.body.touch_bits = 0;
    c.body.position[1] = c.body.floor;
}

/// Back where it was placed, asleep, on its first clip, off the list.
fn reset(c: &mut TrapContext) {
    c.body.status = ItemStatus::Inactive;
    c.body.position = c.trap.start;
    c.body.room = c.trap.start_room;
    c.object.room = c.trap.start_room;
    *c.animation = first_clip(c.world, c.object.kind);
    let state = c.world.data.animations.get(c.animation.animation).map_or(0, |a| a.state);
    c.body.required_state = 0;
    c.body.current_state = state;
    c.goal.state = state;
    c.remove();
}

/// RollingBallCollision `0x8005EFD8`.
fn boulder_collision(c: &mut TrapCollision) {
    match c.body.status {
        ItemStatus::Active => {}
        ItemStatus::Invisible => return,
        _ => return c.generic(),
    }
    if c.touch(false) == 0 {
        return;
    }
    if c.lara.airborne {
        c.touch(true);
        c.player.health -= HIT;
        return;
    }
    if c.player.health <= 0 {
        return;
    }
    c.player.health = -1;
    c.player.yaw = engine::world::angle(c.body.yaw as u16);
    c.player.mode = PlayerMode::Killed { animation: clip::CRUSHED, set_camera: true };
    c.pose.enter(clip::CRUSHED, 0.0, clip::CRUSHED);
    c.motion.velocity = Vec3::ZERO;
    c.motion.pitch = 0.0;
    for _ in 0..CRUSH_DROPS * 4 {
        c.random.draw();
    }
}
