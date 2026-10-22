//! Breakable windows (type 72). Their collision is TrapCollision
//! `0x8001C65C`: Lara touching the glass is pushed back out of it
//! (ItemPushLara `0x8001C890`). Their control `0x8004C034`, which smashes
//! the glass (`0x8004BEE0`) when an active window is touched at speed 50
//! along its facing, runs only once a trigger lists the window; it is not
//! ported, nor is the initialise routine's box blocking (`0x8004BE0C`).
//! Evidence: `docs/assault-course-evidence.md`.
use bevy::prelude::*;

use super::{AppTraps, TrapCollision, TrapType};

pub const WINDOW: u16 = 72;

pub(super) fn register(app: &mut App) {
    app.register_trap(WINDOW, TrapType::new("window").with_collision(window));
}

/// TrapCollision `0x8001C65C`.
fn window(c: &mut TrapCollision) {
    c.generic();
}
