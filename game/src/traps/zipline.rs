//! The zip-line (type 102; control `0x8004B824`, collision `0x8004B6CC`).
//! Lara takes hold of its handle from behind with Action, standing with
//! free hands; it waits while she grabs (its first clip, which ends by
//! moving it 1024 forward onto the line), then slides down its heading,
//! five units a tick faster each tick up to 100, dropping a quarter of
//! that, and carries her (`crate::player::zipline`). It stops where the
//! way ahead is blocked; she drops off, and it waits there until a
//! trigger sends it home.
use bevy::prelude::*;
use engine::camera::chase::{cos, sin};

use super::{AppTraps, NO_HEIGHT, TrapCollision, TrapContext, TrapType, first_clip};
use crate::interactions::{UseBounds, align, in_bounds};
use crate::items::ItemStatus;
use crate::player::{original_units, zipline};

pub const ZIP_LINE: u16 = 102;

/// Its states (its clips'): waiting for Lara, and sliding.
const WAITING: usize = 1;

/// Item flag the grab sets and the stop clears (0x100). A listed line
/// without it is one a trigger reactivated: it goes home.
const RIDDEN: u16 = 0x100;

/// Where Lara must stand to take hold (0x80089644): behind the handle,
/// facing along the line.
const GRAB_BOUNDS: UseBounds = UseBounds {
    min: IVec3::new(-256, -100, 256),
    max: IVec3::new(256, 100, 512),
    angle_min: [0, -4550, 0],
    angle_max: [0, 4550, 0],
};
/// Where she is lined up, in its frame (0x8008965C).
const GRAB_STAND: IVec3 = IVec3::new(0, 0, 371);

const ACCELERATION: i16 = 5;
const TOP_SPEED: i16 = 100;
/// How far ahead, and how far below the line, it looks for the way
/// ahead: a floor more than 320 below and a ceiling more than 192 above.
const LOOKAHEAD: i32 = 1024;
const PROBE_DROP: i32 = 64;
const FLOOR_CLEARANCE: i32 = 320;
const CEILING_CLEARANCE: i32 = 192;

pub(super) fn register(app: &mut App) {
    app.register_trap(ZIP_LINE, TrapType::new("zip-line").with_control(zip_line).with_collision(zip_line_collision));
}

/// ZipLineControl `0x8004B824`.
fn zip_line(c: &mut TrapContext) {
    if c.body.status != ItemStatus::Active {
        return;
    }
    if c.object.flags & RIDDEN == 0 {
        go_home(c);
        return;
    }
    if c.body.current_state == WAITING {
        c.animate();
        return;
    }
    c.animate();
    if c.body.fall_speed < TOP_SPEED {
        c.body.fall_speed += ACCELERATION;
    }
    let speed = i32::from(c.body.fall_speed);
    let yaw = i32::from(c.body.yaw);
    let (sin, cos) = (sin(yaw), cos(yaw));
    c.body.position[0] += (speed * sin) >> 14;
    c.body.position[1] += speed >> 2;
    c.body.position[2] += (speed * cos) >> 14;
    let [x, y, z] = c.body.position;
    if let Some(room) = c.world.floor_room(x, y, z, c.body.room) {
        c.body.room = room;
        c.object.room = room;
    }
    if zipline::riding(c.player) {
        c.player.position = engine::world::position(c.body.position);
    }
    let ahead = [x + ((sin * LOOKAHEAD) >> 14), y + PROBE_DROP, z + ((cos * LOOKAHEAD) >> 14)];
    let room = c.world.floor_room(ahead[0], ahead[1], ahead[2], c.body.room).unwrap_or(c.body.room);
    let floor = c.world.floor_height(ahead[0], ahead[1], ahead[2], room).unwrap_or(NO_HEIGHT);
    if y + FLOOR_CLEARANCE < floor {
        let ceiling = c.world.ceiling_height(ahead[0], ahead[1], ahead[2], room).unwrap_or(NO_HEIGHT);
        if ceiling < y - CEILING_CLEARANCE {
            // Its running sound (280) is presentation.
            return;
        }
    }
    // Blocked: Lara lets go, and it stops where it is (its stop sound,
    // 281, is presentation).
    zipline::line_stopped(c.player, speed);
    c.remove();
    c.body.status = ItemStatus::Inactive;
    c.object.flags &= !RIDDEN;
}

/// Back where it was placed, waiting, off the list.
fn go_home(c: &mut TrapContext) {
    c.body.position = c.trap.start;
    if c.body.room != c.trap.start_room {
        c.body.room = c.trap.start_room;
        c.object.room = c.trap.start_room;
    }
    c.body.current_state = WAITING;
    c.goal.state = WAITING;
    c.body.status = ItemStatus::Inactive;
    *c.animation = first_clip(c.world, c.object.kind);
    c.remove();
}

/// ZipLineCollision `0x8004B6CC`: Lara standing behind it with Action
/// held and free hands takes hold, and it goes on the active list.
fn zip_line_collision(c: &mut TrapCollision) {
    let Some(clips) = c.clips else { return };
    let lara_state = clips.state(c.pose.animation);
    if !c.action
        || !zipline::can_grab(c.player, c.motion, c.weapon, lara_state)
        || c.body.status != ItemStatus::Inactive
    {
        return;
    }
    let at = IVec3::from_array(c.body.position);
    if !in_bounds(&GRAB_BOUNDS, at, c.body.yaw, original_units(c.player.position), c.lara.yaw) {
        return;
    }
    align(Some(c.world), c.player, at, c.body.yaw, GRAB_STAND);
    if !zipline::grab(c.player, c.motion, c.pose, c.weapon, c.traversal, clips) {
        return;
    }
    c.add_active = true;
    c.body.status = ItemStatus::Active;
    c.object.flags |= RIDDEN;
}
