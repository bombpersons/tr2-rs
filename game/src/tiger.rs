//! The tiger (type 39): TigerControl `0x80021FCC` on the shared creature
//! routines in [`crate::creature`]. It is violent: it attacks whenever its
//! enemy shares its zone. Stopped, it lunges at an enemy ahead within 341
//! units, leaps within 1024 and otherwise runs or roars; running, it stops
//! next to an enemy that has stopped and pounces on one still moving. A
//! landed attack bites once for 100.
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, Mood, head, tilt, turn};

pub const KIND: u16 = 39;

/// Tiger states (its clips' state numbers).
pub const STOP: usize = 1;
pub const WALK: usize = 2;
pub const RUN: usize = 3;
pub const ROAR: usize = 5;
pub const LUNGE: usize = 6;
pub const POUNCE: usize = 7;
pub const LEAP: usize = 8;
pub const DEATH: usize = 9;
/// The death clip, counted from the model's first (`0x80022064`).
const DEATH_CLIP: usize = 11;

/// Turn limits in binary angle units a tick (`0x800221E8`, `0x80022214`).
const WALK_TURN: i16 = 546;
const RUN_TURN: i16 = 1092;
/// Squared distances: lunge within 341, leap within 1024, pounce within 1536.
const LUNGE_RANGE: i32 = 0x1_C638;
const LEAP_RANGE: i32 = 0xF_FFFF;
const POUNCE_RANGE: i32 = 0x23_FFFF;
/// Chance out of 32768 to roar, or to stop and roar while walking.
const ROAR_CHANCE: i32 = 96;
/// Meshes whose contact with Lara counts as a bite (`0x800222D8`).
const BITE_MESHES: u32 = 0x07FD_C000;
const BITE_DAMAGE: i32 = 100;

pub struct TigerRulesPlugin;

impl Plugin for TigerRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
    }
}

/// TigerControl `0x80021FCC`.
pub fn control(c: &mut CreatureContext) {
    let level = &c.world.data;
    let state = c.body.current_state;
    let (mut head_angle, mut angle) = (0, 0);
    if *c.health <= 0 {
        if state != DEATH
            && let Some(model) = level.model(KIND)
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
        c.mood(&info, true);
        angle = turn(c.creature, c.body, c.creature.max_turn);
        let (creature, body, goal) = (&mut *c.creature, &mut *c.body, &mut *c.goal);
        match state {
            STOP => {
                creature.max_turn = 0;
                creature.flags = 0;
                goal.state = if creature.mood == Mood::Escape {
                    RUN
                } else if creature.mood == Mood::Bored {
                    // Its roar request is overwritten (0x80022138).
                    c.random.draw();
                    WALK
                } else if info.ahead && info.distance <= LUNGE_RANGE {
                    LUNGE
                } else if info.ahead && info.distance <= LEAP_RANGE {
                    creature.max_turn = WALK_TURN;
                    LEAP
                } else if body.required_state != 0 {
                    body.required_state
                } else if creature.mood != Mood::Attack && c.random.draw() < ROAR_CHANCE {
                    ROAR
                } else {
                    RUN
                };
            }
            WALK => {
                creature.max_turn = WALK_TURN;
                if c.random.draw() < ROAR_CHANCE {
                    goal.state = STOP;
                    body.required_state = ROAR;
                }
            }
            RUN => {
                creature.max_turn = RUN_TURN;
                if creature.mood == Mood::Bored || (creature.flags != 0 && info.ahead) {
                    goal.state = STOP;
                } else if creature.flags == 0 && info.ahead && info.distance <= POUNCE_RANGE {
                    goal.state = if c.enemy.speed == 0 { STOP } else { POUNCE };
                } else if creature.mood != Mood::Attack && c.random.draw() < ROAR_CHANCE {
                    body.required_state = ROAR;
                    goal.state = STOP;
                }
                creature.flags = 0;
            }
            LUNGE | POUNCE | LEAP if creature.flags == 0 && body.touch_bits & BITE_MESHES != 0 => {
                // CreatureEffect with blood at the bite point: presentation only.
                c.player.damage(BITE_DAMAGE);
                c.enemy.health = c.player.health;
                creature.flags = 1;
            }
            _ => {}
        }
    }
    tilt(c.body, 0);
    head(c.creature, head_angle);
    c.animate(angle, 0);
}
