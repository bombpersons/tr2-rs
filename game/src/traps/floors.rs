//! Floors that give way: the crumbling tile (FallingBlock `0x8005F97C`)
//! and the trapdoor (TrapDoorControl `0x8005F57C`). Both are solid only
//! through their floor and ceiling routines, which the level's height
//! queries call for sectors whose floor data names them; the port lays
//! the same slab with [`ObjectSurface`].
use bevy::prelude::*;
use engine::{
    objects::WorldObject,
    schedule::{ItemSystems, simulation_enabled},
    world::{ObjectSurface, WorldGeometry},
};

use super::{AppTraps, TrapContext, TrapType};
use crate::items::{ItemBody, ItemStatus};

/// The crumbling tile, and a taller one the same routine runs.
pub const FALLING_BLOCK: u16 = 55;
pub const TALL_FALLING_BLOCK: u16 = 57;
pub const TRAPDOOR: u16 = 114;

/// Tile states (its clips').
const STILL: usize = 0;
const SHAKING: usize = 1;
const FALLING: usize = 2;
const LANDED: usize = 3;

pub(super) fn register(app: &mut App) {
    app.register_trap(FALLING_BLOCK, TrapType::new("falling_block").with_control(falling_block))
        .register_trap(TALL_FALLING_BLOCK, TrapType::new("tall_falling_block").with_control(falling_block))
        .register_trap(TRAPDOOR, TrapType::new("trapdoor").with_control(trapdoor))
        .add_systems(
            FixedUpdate,
            lay_surfaces.in_set(ItemSystems::Control).after(super::control).run_if(simulation_enabled),
        );
}

/// How far the tile's walkable top is above its origin.
fn height(kind: u16) -> i32 {
    if kind == TALL_FALLING_BLOCK { 1024 } else { 512 }
}

/// The tile's slab while it has not started to fall: a floor at its top
/// and, under it, a ceiling 256 lower (FallingBlockFloor `0x8005FB5C`,
/// FallingBlockCeiling `0x8005FBAC`), wherever floor data names it.
pub fn falling_block_surface(kind: u16, y: i32, state: usize) -> Option<ObjectSurface> {
    let top = y - height(kind);
    (state < FALLING).then_some(ObjectSurface { sectors: None, top, underside: top + 256, nearest: false })
}

/// FallingBlock `0x8005F97C`. Triggered while Lara stands on it, it waits
/// for her feet to be exactly on its top (else it goes back to sleep until
/// the trigger adds it again), shakes, drops under gravity and lands.
fn falling_block(c: &mut TrapContext) {
    match c.body.current_state {
        STILL => {
            if c.lara.position.y != c.body.position[1] - height(c.object.kind) {
                c.body.status = ItemStatus::Inactive;
                c.remove();
                return;
            }
            c.goal.state = SHAKING;
        }
        SHAKING => c.goal.state = FALLING,
        FALLING if c.goal.state != LANDED => c.body.gravity = true,
        _ => {}
    }
    c.animate();
    if c.body.status == ItemStatus::Deactivated {
        c.remove();
    } else {
        let floor = c.settle_room();
        c.body.floor = floor;
        if c.body.current_state == FALLING && floor <= c.body.position[1] {
            c.goal.state = LANDED;
            c.body.fall_speed = 0;
            c.body.position[1] = floor;
            c.body.gravity = false;
        }
    }
}

/// The trapdoor's slab while shut: over its own sector and the next one
/// it faces, a floor at its origin and a ceiling 256 under it, each only
/// nearer than what is there (TrapDoorFloor `0x8005F608`, TrapDoorCeiling
/// `0x8005F690`, OnTrapDoor `0x8005F718`). Only the four facings count.
pub fn trapdoor_surface(position: [i32; 3], yaw: i16, state: usize) -> Option<ObjectSurface> {
    let [x, _, z] = position.map(|v| v >> 10);
    let (dx, dz) = match yaw {
        0 => (0, 1),
        i16::MIN => (0, -1),
        16384 => (1, 0),
        -16384 => (-1, 0),
        _ => return None,
    };
    (state == 0).then_some(ObjectSurface {
        sectors: Some([[x, z], [x + dx, z + dz]]),
        top: position[1],
        underside: position[1] + 256,
        nearest: true,
    })
}

/// TrapDoorControl `0x8005F57C`: opens while triggered, shuts after.
fn trapdoor(c: &mut TrapContext) {
    if c.trigger_active() {
        if c.body.current_state == 0 {
            c.goal.state = 1;
        }
    } else if c.body.current_state == 1 {
        c.goal.state = 0;
    }
    c.animate();
}

/// Keep every tile's and trapdoor's slab in the level as it now stands,
/// listed or not.
fn lay_surfaces(world: Option<ResMut<WorldGeometry>>, traps: Query<(&WorldObject, &ItemBody), With<super::Trap>>) {
    let Some(mut world) = world else { return };
    for (object, body) in &traps {
        let surface = match object.kind {
            FALLING_BLOCK | TALL_FALLING_BLOCK => {
                falling_block_surface(object.kind, body.position[1], body.current_state)
            }
            TRAPDOOR => trapdoor_surface(body.position, body.yaw, body.current_state),
            _ => continue,
        };
        if world.object_surface(object.index) != surface.as_ref() {
            world.set_object_surface(object.index, surface);
        }
    }
}
