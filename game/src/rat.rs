//! The rat (type 21): RatControl `0x80051990` on the shared creature
//! routines in [`crate::creature`]. It is not violent. Every change of
//! gait goes through its stop: it sets the state it wants next, and stops.
//! Bored or stalking, it wanders, pausing now and then; attacking, it runs
//! at Lara and bites once from within 341 units, for 20.
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, Mood, head, turn};

pub const KIND: u16 = 21;

/// Rat states (its clips' state numbers).
pub const RUN: usize = 1;
pub const STOP: usize = 2;
pub const PAUSE: usize = 3;
pub const WALK: usize = 4;
pub const BITE: usize = 5;
pub const DEATH: usize = 6;
/// Its death clip, counted from its model's first (`0x80051A1C`).
const DEATH_CLIP: usize = 9;

/// Its turn limit a tick, whatever the state (`0x80051A74`).
const TURN: i16 = 1092;
/// Squared distance within which it bites (341 units).
const BITE_RANGE: i32 = 0x1_C638;
/// Chances out of 32768 (one draw a tick): to pause, to stop running or
/// walking, and to set off again from a pause.
const TO_PAUSE: i32 = 1280;
const RUN_TO_STOP: i32 = 2560;
const WALK_ON: i32 = 2561;
const LEAVE_PAUSE: i32 = 1280;
/// Meshes whose contact with Lara counts as a bite (`0x80051BC4`).
const BITE_MESHES: u32 = 0x7F;
const BITE_DAMAGE: i32 = 20;

pub struct RatRulesPlugin;

impl Plugin for RatRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
    }
}

/// RatControl `0x80051990`.
pub fn control(c: &mut CreatureContext) {
    let (mut head_angle, mut angle) = (0, 0);
    if *c.health <= 0 {
        if c.body.current_state != DEATH
            && let Some(model) = c.world.data.model(KIND)
        {
            c.animation.animation = model.animations[0] + DEATH_CLIP;
            c.animation.frame = 0.0;
            c.body.current_state = DEATH;
        }
    } else {
        let info = c.ai_info();
        if info.ahead {
            head_angle = info.angle;
        }
        c.mood(&info, false);
        angle = turn(c.creature, c.body, TURN);
        let wanders = matches!(c.creature.mood, Mood::Bored | Mood::Stalk);
        match c.body.current_state {
            RUN => {
                c.creature.max_turn = TURN;
                if wanders {
                    let roll = c.random.draw();
                    if roll < TO_PAUSE {
                        c.body.required_state = PAUSE;
                        c.goal.state = STOP;
                    } else if roll < RUN_TO_STOP {
                        c.goal.state = STOP;
                    }
                } else if info.ahead && info.distance <= BITE_RANGE {
                    c.goal.state = STOP;
                }
            }
            STOP => {
                c.creature.max_turn = 0;
                if c.body.required_state != 0 {
                    c.goal.state = c.body.required_state;
                }
            }
            PAUSE => {
                if c.random.draw() < LEAVE_PAUSE {
                    c.goal.state = STOP;
                }
            }
            WALK => {
                if wanders {
                    let roll = c.random.draw();
                    if roll < TO_PAUSE {
                        c.body.required_state = PAUSE;
                    } else if roll >= WALK_ON {
                        c.body.required_state = RUN;
                    }
                } else {
                    c.body.required_state = if info.distance > BITE_RANGE { RUN } else { BITE };
                }
                if c.body.required_state != 0 {
                    c.goal.state = STOP;
                }
            }
            BITE if c.body.required_state == 0 && c.body.touch_bits & BITE_MESHES != 0 => {
                // CreatureEffect's blood at the bite point is presentation only.
                c.player.health -= BITE_DAMAGE;
                c.enemy.health = c.player.health;
                c.body.required_state = STOP;
            }
            _ => {}
        }
    }
    head(c.creature, head_angle);
    c.animate(angle, 0);
}
