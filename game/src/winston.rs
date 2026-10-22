//! Home's attendant (type 260): WinstonControl `0x800500A0` on the shared
//! creature routines in [`crate::creature`]. A one-time floor trigger
//! activates him, and he then follows Lara through the house: he sets off
//! when she is more than 1536 units away or behind him, and stops once
//! she is closer than that and ahead. He is violent in the mood's sense
//! (he heads straight for her whenever she is in his zone), never a target
//! (hit points -16384 in the object table) and never attacks.
//!
//! His collision is the generic object push (`0x8001C65C`), not
//! CreatureCollision: [`contact`]. His voice sounds (0x158..0x15B, picked
//! with the draw-side random) are not ported.
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, turn};

mod contact;

pub const KIND: u16 = 260;

/// Winston's states (his clips' state numbers).
pub const STOP: usize = 1;
pub const MOVE: usize = 2;

/// Squared distance beyond which he follows (`0x80050128`: 1536 units).
const FOLLOW_RANGE: i32 = 0x24_0000;

pub struct WinstonRulesPlugin;

impl Plugin for WinstonRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
        contact::install(app);
    }
}

/// WinstonControl `0x800500A0`. The state decision follows the turn, and
/// both read where Lara stood before this tick.
pub fn control(c: &mut CreatureContext) {
    let info = c.ai_info();
    c.mood(&info, true);
    let angle = turn(c.creature, c.body, c.creature.max_turn);
    if c.body.current_state == STOP {
        if (info.distance > FOLLOW_RANGE || !info.ahead) && c.goal.state != MOVE {
            c.goal.state = MOVE;
        }
    } else if info.distance < FOLLOW_RANGE && info.ahead {
        c.goal.state = STOP;
    }
    c.animate(angle, 0);
}
