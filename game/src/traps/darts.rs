//! Dart shooters and their darts. The emitter (DartEmitterControl
//! `0x8005FE80`) fires one dart each time its firing clip starts while
//! it is triggered; the dart (DartsControl `0x80060078`) flies on its
//! clip's speed, spinning, stings Lara for 50 when it touches her, and
//! breaks on the first floor or wall.
use bevy::prelude::*;

use super::{AppTraps, NewItem, TrapCollision, TrapContext, TrapType};

pub const DART: u16 = 61;
pub const DART_EMITTER: u16 = 62;

/// Emitter states (its clips').
const IDLE: usize = 0;
const FIRING: usize = 1;

/// How far behind the emitter's face a dart starts, and how high above.
const MUZZLE: i32 = 412;
const MUZZLE_HEIGHT: i32 = 512;
const STING: i32 = 50;
/// The dart's spin about its length each tick.
const SPIN: i16 = 4096;

pub(super) fn register(app: &mut App) {
    app.register_trap(DART_EMITTER, TrapType::new("dart_emitter").with_control(emitter))
        .register_trap(DART, TrapType::new("dart").with_control(dart).with_collision(dart_collision));
}

/// DartEmitterControl `0x8005FE80`.
fn emitter(c: &mut TrapContext) {
    if c.trigger_active() {
        if c.body.current_state == IDLE {
            c.goal.state = FIRING;
        }
    } else if c.body.current_state == FIRING {
        c.goal.state = IDLE;
    } else {
        c.animate();
        return;
    }
    if c.body.current_state == FIRING && c.animation.frame == 0.0 {
        // The offset is only applied for the four facings, and the one
        // facing +x tests 0x3FEC rather than 0x4000, so it never moves.
        let (dx, dz) = match c.body.yaw {
            -16384 => (MUZZLE, 0),
            i16::MIN => (0, MUZZLE),
            0 => (0, -MUZZLE),
            0x3FEC => (-MUZZLE, 0),
            _ => (0, 0),
        };
        let [x, y, z] = c.body.position;
        c.created.push(NewItem {
            kind: DART,
            position: [x + dx, y - MUZZLE_HEIGHT, z + dz],
            yaw: c.body.yaw,
            room: c.body.room,
        });
    }
    c.animate();
}

/// DartsControl `0x80060078`.
fn dart(c: &mut TrapContext) {
    if c.body.touch_bits != 0 {
        c.hurt_lara(STING);
    }
    c.animate();
    let floor = c.settle_room();
    c.body.floor = floor;
    c.body.pitch = c.body.pitch.wrapping_add(SPIN);
    if c.body.position[1] >= floor {
        c.kill();
        // The ricochet's spark draws once from the control generator.
        c.random.draw();
    }
}

/// TrapCollision `0x8001C65C`.
fn dart_collision(c: &mut TrapCollision) {
    c.generic();
}
