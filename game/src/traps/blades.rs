//! Cutting traps: the spinning blade that slides along the floor
//! (`0x8005DC50`), the blade that swings out of a wall (`0x8005E6B8`) and
//! the spiked roller that runs back and forth between walls
//! (`0x8005E25C`). Lara's blood from them comes from the draw generator,
//! not the control one, so only the hit points are ported.
use bevy::prelude::*;
use engine::camera::chase::{cos, sin};

use super::{AppTraps, NO_HEIGHT, TrapCollision, TrapContext, TrapType, set_clip};
use crate::items::ItemStatus;

pub const ROLLING_SPINDLE: u16 = 80;
pub const WALL_BLADE: u16 = 81;
pub const SLIDING_BLADE: u16 = 85;

/// Blade and spindle states (their clips').
const READY: usize = 1;
const MOVING: usize = 2;

const SLIDING_CUT: i32 = 20;
const WALL_CUT: i32 = 100;
const SPINDLE_CUT: i32 = 100;
/// The wall blade cuts with its second mesh only.
const WALL_BLADE_EDGE: u32 = 0b10;
/// How far ahead the spindle looks for a wall.
const SPINDLE_LOOKAHEAD: i32 = 1536;

pub(super) fn register(app: &mut App) {
    app.register_trap(
        ROLLING_SPINDLE,
        TrapType::new("rolling_spindle")
            .with_initialise(|w, k, a, b| set_clip(w, k, 3, a, b))
            .with_control(rolling_spindle)
            .with_collision(|c| c.generic()),
    )
    .register_trap(
        WALL_BLADE,
        TrapType::new("wall_blade")
            .with_initialise(|w, k, a, b| set_clip(w, k, 2, a, b))
            .with_control(wall_blade)
            .with_collision(wall_blade_collision),
    )
    .register_trap(
        SLIDING_BLADE,
        TrapType::new("sliding_blade").with_control(sliding_blade).with_collision(|c| c.generic()),
    );
}

/// `0x8005DC50`. While triggered it slides 16 units a tick (less when
/// turned off the axes) as long as the floor ahead is level with it; the
/// first step that is not stops it for good. It does not animate.
fn sliding_blade(c: &mut TrapContext) {
    if c.trigger_active() && c.body.status != ItemStatus::Deactivated {
        let yaw = i32::from(c.body.yaw);
        let [x, y, z] = c.body.position;
        let (nx, nz) = (x + (sin(yaw) >> 10), z + (cos(yaw) >> 10));
        let room = c.world.floor_room(nx, y, nz, c.body.room).unwrap_or(c.body.room);
        let floor = c.world.floor_height(nx, y, nz, room).unwrap_or(NO_HEIGHT);
        if floor == y {
            c.body.position[0] = nx;
            c.body.position[2] = nz;
            c.body.room = room;
            c.object.room = room;
        } else {
            c.body.status = ItemStatus::Deactivated;
        }
    }
    if c.body.touch_bits != 0 {
        c.hurt_lara(SLIDING_CUT);
        c.body.touch_bits = 0;
    }
}

/// `0x8005E6B8`: swings while triggered, and its edge cuts Lara for 100
/// on every tick of the swing it touches her.
fn wall_blade(c: &mut TrapContext) {
    let active = c.trigger_active();
    c.goal.state = if active && c.body.current_state == READY { MOVING } else { READY };
    if c.body.touch_bits & WALL_BLADE_EDGE != 0 && c.body.current_state == MOVING {
        c.hurt_lara(WALL_CUT);
    }
    c.animate();
}

/// `0x8001C7F0`: a swinging blade only marks what it touches; at rest it
/// pushes Lara like any trap.
fn wall_blade_collision(c: &mut TrapCollision) {
    match c.body.status {
        ItemStatus::Active => {
            c.touch(false);
        }
        ItemStatus::Invisible => {}
        _ => c.generic(),
    }
}

/// `0x8005E25C`. Triggered, it rolls on its clip until it sees no floor
/// 1536 ahead; then it slows to a stop, turns about and waits to be
/// triggered again. It cuts Lara for 100 every tick it touches her while
/// rolling, and always sits on the floor.
fn rolling_spindle(c: &mut TrapContext) {
    let rolling = c.body.current_state == MOVING;
    if rolling {
        if c.goal.state != READY {
            let yaw = i32::from(c.body.yaw);
            let [x, y, z] = c.body.position;
            let ahead = (x + (sin(yaw) * SPINDLE_LOOKAHEAD >> 14), z + (cos(yaw) * SPINDLE_LOOKAHEAD >> 14));
            if c.height_at(ahead.0, y, ahead.1) == NO_HEIGHT {
                c.goal.state = READY;
            }
        }
        if c.body.touch_bits != 0 {
            c.hurt_lara(SPINDLE_CUT);
        }
    } else if c.trigger_active() {
        c.goal.state = MOVING;
    }
    c.animate();
    let floor = c.settle_room();
    c.body.position[1] = floor;
    c.body.floor = floor;
    if rolling && c.body.current_state == READY {
        c.body.yaw = c.body.yaw.wrapping_add(i16::MIN);
    }
}
