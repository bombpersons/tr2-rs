//! The dog (type 15): DogControl `0x80021AC8` on the shared creature
//! routines in [`crate::creature`]. It is not violent. Bored, it wanders and
//! now and then sits or lies down; roused, it runs at Lara, leaps at her
//! from up to 768 units, and bites her standing or on the run. Each
//! attack bites once: 200 for the leap, 100 for the others.
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, Mood, head, tilt, turn};

pub const KIND: u16 = 15;

/// Dog states (its clips' state numbers).
pub const WALK: usize = 1;
pub const RUN: usize = 2;
pub const STOP: usize = 3;
/// Bored poses its stop and walk lead to, each left at random.
pub const IDLE_4: usize = 4;
pub const IDLE_5: usize = 5;
pub const IDLE_6: usize = 6;
pub const BITE: usize = 7;
pub const LEAP: usize = 8;
pub const RUN_BITE: usize = 9;
pub const DEATH: usize = 10;
/// Its death clip, counted from its model's first (`0x80021B5C`).
const DEATH_CLIP: usize = 13;

/// Turn limits a tick (`0x80021CA0`, `0x80021CFC`).
const WALK_TURN: i16 = 546;
const RUN_TURN: i16 = 1092;
/// Squared distances: it bites within 341, leaps within 768 and bites on
/// the run within 682 (`0x80021C74`, `0x80021D0C`, `0x80021DB0`).
const BITE_RANGE: i32 = 0x1_C638;
const LEAP_RANGE: i32 = 0x8_FFFF;
const RUN_BITE_RANGE: i32 = 0x7_18E3;
/// Chances out of 32768 (one draw a tick), bored: from stop, to the poses
/// 4 and 5 or to walk; from walk, by way of stop, to poses 4 and 5, or to
/// stop; and to leave each pose.
const STOP_TO_4: i32 = 768;
const STOP_TO_5: i32 = 1536;
const STOP_TO_WALK: i32 = 9728;
const WALK_TO_STOP: i32 = 2816;
const LEAVE_4: i32 = 1280;
const LEAVE_5: i32 = 256;
const LEAVE_6: i32 = 512;
/// Meshes whose contact with Lara counts as a bite (`0x80021D3C`).
const BITE_MESHES: u32 = 0x013F_7000;
const BITE_DAMAGE: i32 = 100;
const LEAP_DAMAGE: i32 = 200;

pub struct DogRulesPlugin;

impl Plugin for DogRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
    }
}

/// DogControl `0x80021AC8`.
pub fn control(c: &mut CreatureContext) {
    let (mut head_angle, mut angle, mut tilt_angle) = (0, 0, 0);
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
        angle = turn(c.creature, c.body, c.creature.max_turn);
        // One bite per attack: flags notes the state that landed it, and
        // CreatureEffect's blood at the bite point is presentation only.
        let bite = |c: &mut CreatureContext, flag: i16, damage: i32| {
            c.player.health -= damage;
            c.enemy.health = c.player.health;
            c.creature.flags = flag;
        };
        let bored = c.creature.mood == Mood::Bored;
        let state = c.body.current_state;
        match state {
            WALK => {
                c.creature.max_turn = WALK_TURN;
                if !bored {
                    c.goal.state = RUN;
                } else {
                    let roll = c.random.draw();
                    if roll < STOP_TO_4 {
                        c.body.required_state = IDLE_4;
                        c.goal.state = STOP;
                    } else if roll < STOP_TO_5 {
                        c.body.required_state = IDLE_5;
                        c.goal.state = STOP;
                    } else if roll < WALK_TO_STOP {
                        c.goal.state = STOP;
                    }
                }
            }
            RUN => {
                tilt_angle = angle;
                c.creature.max_turn = RUN_TURN;
                if bored {
                    c.goal.state = STOP;
                } else if info.distance <= LEAP_RANGE {
                    c.goal.state = LEAP;
                }
            }
            STOP => {
                c.creature.max_turn = 0;
                c.creature.flags = 0;
                if bored {
                    if c.body.required_state != 0 {
                        c.goal.state = c.body.required_state;
                    } else {
                        let roll = c.random.draw();
                        if roll < STOP_TO_4 {
                            c.goal.state = IDLE_4;
                        } else if roll < STOP_TO_5 {
                            c.goal.state = IDLE_5;
                        } else if roll < STOP_TO_WALK {
                            c.goal.state = WALK;
                        }
                    }
                } else if c.creature.mood == Mood::Escape || info.distance > BITE_RANGE || !info.ahead {
                    c.goal.state = RUN;
                } else {
                    c.goal.state = BITE;
                }
            }
            IDLE_4 | IDLE_5 | IDLE_6 => {
                let chance = match state {
                    IDLE_4 => LEAVE_4,
                    IDLE_5 => LEAVE_5,
                    _ => LEAVE_6,
                };
                if !bored || c.random.draw() < chance {
                    c.goal.state = STOP;
                }
            }
            BITE => {
                c.creature.max_turn = 0;
                if c.creature.flags != 1 && info.ahead && c.body.touch_bits & BITE_MESHES != 0 {
                    bite(c, 1, BITE_DAMAGE);
                }
                // Unsigned: outside 0x1C63A..=0x718E3 it stops.
                let near =
                    info.distance.wrapping_sub(BITE_RANGE + 2) as u32 <= (RUN_BITE_RANGE - BITE_RANGE - 2) as u32;
                c.goal.state = if near { RUN_BITE } else { STOP };
            }
            LEAP => {
                if c.creature.flags != 2 && c.body.touch_bits & BITE_MESHES != 0 {
                    bite(c, 2, LEAP_DAMAGE);
                }
                if info.distance <= BITE_RANGE {
                    c.goal.state = BITE;
                } else if info.distance <= RUN_BITE_RANGE {
                    c.goal.state = RUN_BITE;
                }
            }
            RUN_BITE => {
                c.creature.max_turn = RUN_TURN;
                if c.creature.flags != 3 && c.body.touch_bits & BITE_MESHES != 0 {
                    bite(c, 3, BITE_DAMAGE);
                }
                if info.distance <= BITE_RANGE {
                    c.goal.state = BITE;
                }
            }
            _ => {}
        }
    }
    tilt(c.body, tilt_angle);
    head(c.creature, head_angle);
    c.animate(angle, tilt_angle);
}
