//! The T-Rex (type 214): TRexControl `0x800239C4` on the shared creature
//! routines in [`crate::creature`]. It is violent and large (its path is
//! barred by box flag 0x8000). It walks while bored or while Lara, out of
//! its sight, faces it, and otherwise runs at her, stopping now and then to
//! roar; close and ahead it bites. Touching Lara hurts her every tick, 10 a
//! tick while it runs and 1 otherwise. A bite that lands takes her in its
//! jaws: CreatureKill puts them both into its kill clips.
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, Mood, head, turn};

pub const KIND: u16 = 214;

/// T-Rex states (its clips' state numbers).
pub const STOP: usize = 1;
pub const WALK: usize = 2;
pub const RUN: usize = 3;
pub const DEATH: usize = 5;
pub const ROAR: usize = 6;
pub const ATTACK: usize = 7;
pub const KILL: usize = 8;

/// Its kill clip, counted from its model's first.
const KILL_CLIP: usize = 11;
/// The extra-animation object whose clips are Lara in its jaws, and the
/// state that takes her into them.
const LARA_KILLED: u16 = 12;
const LARA_KILLED_STATE: usize = 6;

/// Turn limits a tick (`0x80023BFC`, `0x80023C48`).
const WALK_TURN: i16 = 364;
const RUN_TURN: i16 = 728;
/// Squared distances: it bites within 1500 and stops running at a target
/// within 5120; a target between 1500 and 4096 that it could bite makes it
/// walk.
const BITE_RANGE: i32 = 0x22_5510;
const WALK_RANGE: i32 = 0x100_0000;
const STOP_RANGE: i32 = 0x190_0000;
/// Chance out of 32768 to stop and roar while moving at Lara ahead.
const ROAR_CHANCE: i32 = 512;
/// Contact damage a tick, running and otherwise.
const RUN_CONTACT: i32 = 10;
const CONTACT: i32 = 1;
/// Meshes whose touch closes its jaws on Lara (`0x80023CE4`).
const JAW_MESHES: u32 = 0x3000;
const BITE: i32 = 10000;

pub struct TRexRulesPlugin;

impl Plugin for TRexRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
    }
}

/// TRexControl `0x800239C4`.
pub fn control(c: &mut CreatureContext) {
    let (mut head_angle, mut angle) = (0, 0);
    if *c.health <= 0 {
        c.goal.state = if c.body.current_state == STOP { DEATH } else { STOP };
    } else {
        let info = c.ai_info();
        if info.ahead {
            head_angle = info.angle;
        }
        c.mood(&info, true);
        angle = turn(c.creature, c.body, c.creature.max_turn);
        if c.body.touch_bits != 0 {
            c.player.health -= if c.body.current_state == RUN { RUN_CONTACT } else { CONTACT };
            c.enemy.health = c.player.health;
        }
        let (creature, body, goal) = (&mut *c.creature, &mut *c.body, &mut *c.goal);
        // Lara out of sight but facing it, or within reach of a bite but
        // not yet close, makes it walk (`0x80023ABC`).
        let facing = (info.enemy_facing as u16).wrapping_add(0x3FFF) < 0x7FFF;
        creature.flags = i16::from(creature.mood != Mood::Escape && !info.ahead && facing);
        if creature.flags == 0 && info.distance > BITE_RANGE && info.distance < WALK_RANGE && info.bite {
            creature.flags = 1;
        }
        match body.current_state {
            STOP => {
                goal.state = if body.required_state != 0 {
                    body.required_state
                } else if info.distance < BITE_RANGE && info.bite {
                    ATTACK
                } else if creature.mood == Mood::Bored || creature.flags != 0 {
                    WALK
                } else {
                    RUN
                };
            }
            WALK => {
                creature.max_turn = WALK_TURN;
                if creature.mood != Mood::Bored && creature.flags == 0 {
                    goal.state = STOP;
                } else if info.ahead && c.random.draw() < ROAR_CHANCE {
                    body.required_state = ROAR;
                    goal.state = STOP;
                }
            }
            RUN => {
                creature.max_turn = RUN_TURN;
                if (info.distance < STOP_RANGE && info.bite) || creature.flags != 0 {
                    goal.state = STOP;
                } else if creature.mood != Mood::Escape && info.ahead && c.random.draw() < ROAR_CHANCE {
                    body.required_state = ROAR;
                    goal.state = STOP;
                } else if creature.mood == Mood::Bored {
                    goal.state = STOP;
                }
            }
            ATTACK if body.touch_bits & JAW_MESHES != 0 => {
                c.player.health -= BITE;
                c.enemy.health = c.player.health;
                c.kill(KILL_CLIP, KILL, LARA_KILLED, LARA_KILLED_STATE);
                return;
            }
            ATTACK => body.required_state = WALK,
            _ => {}
        }
    }
    head(c.creature, head_angle >> 1);
    c.creature.neck = c.creature.head;
    c.animate(angle, 0);
}
