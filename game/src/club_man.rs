//! Venice's club men (type 32): their control `0x8002BF54` on the shared
//! creature routines in [`crate::creature`]. Violent: they attack whenever
//! Lara shares their zone. They walk or run at her and swing their clubs
//! from three distances: a jab from close by (100), a swing from a step
//! away and a lunge from further (80 each). They climb up and drop down
//! whole blocks (CreatureVault).
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, Mood, head, neck, tilt, turn};

pub const KIND: u16 = 32;

/// Club man states (its clips' state numbers).
pub const STOP: usize = 1;
pub const WALK: usize = 2;
pub const JAB: usize = 3;
pub const READY_SWING: usize = 4;
pub const WAIT: usize = 5;
pub const READY_LUNGE: usize = 6;
pub const READY_JAB: usize = 7;
pub const LUNGE: usize = 8;
pub const SWING: usize = 9;
pub const RUN: usize = 10;
pub const DEATH: usize = 11;
/// Climbing up two, three and four clicks, and dropping four.
pub const CLIMB_4: usize = 12;
pub const CLIMB_2: usize = 13;
pub const CLIMB_3: usize = 14;
pub const DROP_4: usize = 15;
/// Clips counted from its model's first: death (`0x8002BFF0`) and the
/// climbs, in the order of [`CLIMB_4`] to [`DROP_4`] (`0x8002C5E4..`).
const DEATH_CLIP: usize = 26;
const CLIMB_CLIPS: [(i32, usize, usize); 4] = [(4, 27, CLIMB_4), (2, 28, CLIMB_2), (3, 29, CLIMB_3), (-4, 30, DROP_4)];
/// CreatureVault's offset from the block edge it climbs.
const VAULT_SHIFT: i32 = 260;

/// Turn limits a tick (`0x8002C19C`, `0x8002C248`).
const WALK_TURN: i16 = 910;
const RUN_TURN: i16 = 1092;
/// Squared distances: jab within 255, ready a swing within 1024, lunge
/// within 2048, and walk rather than run within 2048.
const JAB_RANGE: i32 = 0x3_FFFF;
const SWING_RANGE: i32 = 0xF_FFFF;
const LUNGE_RANGE: i32 = 0x23_FFFF;
const WALK_RANGE: i32 = 0x3F_FFFF;
/// Chances out of 32768, bored: to walk rather than wait, and to stop.
const BORED_WALK: i32 = 0x4000;
const BORED_STOP: i32 = 256;
/// Meshes whose contact with Lara counts as a hit (`0x8002C3A0`).
const CLUB_MESHES: u32 = 0x600;
const SWING_DAMAGE: i32 = 80;
const JAB_DAMAGE: i32 = 100;

pub struct ClubManRulesPlugin;

impl Plugin for ClubManRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
    }
}

/// The club men's control `0x8002BF54`.
pub fn control(c: &mut CreatureContext) {
    let (mut head_angle, mut neck_angle, mut angle, mut tilt_angle) = (0, 0, 0, 0);
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
        c.mood(&info, true);
        angle = turn(c.creature, c.body, c.creature.max_turn);
        let mood = c.creature.mood;
        // It looks at Lara with its head while moving and its torso while
        // attacking.
        let look = if info.ahead { info.angle } else { 0 };
        // Hits once per attack, flags noting the attack that landed.
        let hit = |c: &mut CreatureContext, flag: i16, damage: i32| {
            // CreatureEffect's blood and the swing sound are presentation.
            c.player.health -= damage;
            c.enemy.health = c.player.health;
            c.creature.flags = flag;
        };
        let touched = c.body.touch_bits & CLUB_MESHES != 0;
        match c.body.current_state {
            STOP => {
                c.creature.flags = 0;
                neck_angle = look;
                c.creature.max_turn = 0;
                c.goal.state = if mood == Mood::Escape {
                    RUN
                } else if mood == Mood::Bored {
                    if c.body.required_state != 0 {
                        c.body.required_state
                    } else if c.random.draw() < BORED_WALK {
                        WALK
                    } else {
                        WAIT
                    }
                } else if !info.bite {
                    RUN
                } else if info.distance <= JAB_RANGE {
                    READY_JAB
                } else if info.distance <= SWING_RANGE {
                    READY_LUNGE
                } else if info.distance <= WALK_RANGE {
                    WALK
                } else {
                    RUN
                };
            }
            WAIT => {
                neck_angle = look;
                if mood != Mood::Bored {
                    c.goal.state = STOP;
                } else if c.random.draw() < BORED_STOP {
                    c.body.required_state = WALK;
                    c.goal.state = STOP;
                }
            }
            WALK => {
                neck_angle = look;
                c.creature.max_turn = WALK_TURN;
                if mood == Mood::Escape || (mood != Mood::Bored && !info.bite) {
                    c.goal.state = RUN;
                } else if mood == Mood::Bored {
                    if c.random.draw() < BORED_STOP {
                        c.body.required_state = WAIT;
                        c.goal.state = STOP;
                    }
                } else if info.distance <= JAB_RANGE {
                    c.goal.state = STOP;
                } else if info.distance <= LUNGE_RANGE {
                    c.goal.state = READY_SWING;
                } else {
                    c.goal.state = RUN;
                }
            }
            RUN => {
                neck_angle = look;
                tilt_angle = angle / 2;
                c.creature.max_turn = RUN_TURN;
                if mood != Mood::Escape && (mood == Mood::Bored || (info.ahead && info.distance <= WALK_RANGE)) {
                    c.goal.state = WALK;
                }
            }
            READY_JAB => {
                head_angle = look;
                c.creature.flags = 0;
                c.goal.state = if info.bite && info.distance <= JAB_RANGE { SWING } else { STOP };
            }
            READY_LUNGE => {
                head_angle = look;
                c.creature.flags = 0;
                c.goal.state = if info.ahead && info.distance <= SWING_RANGE { LUNGE } else { STOP };
            }
            READY_SWING => {
                head_angle = look;
                c.creature.flags = 0;
                c.goal.state = if info.bite && info.distance <= LUNGE_RANGE { JAB } else { WALK };
            }
            SWING => {
                head_angle = look;
                if c.creature.flags == 0 && touched {
                    hit(c, 1, SWING_DAMAGE);
                }
            }
            LUNGE => {
                head_angle = look;
                if c.creature.flags == 0 && touched {
                    hit(c, 1, SWING_DAMAGE);
                }
                // An unsigned range test in the original (`0x8002C488`).
                if info.ahead && (SWING_RANGE + 2..=LUNGE_RANGE).contains(&info.distance) {
                    c.goal.state = JAB;
                }
            }
            JAB => {
                head_angle = look;
                if c.creature.flags != 2 && touched {
                    hit(c, 2, JAB_DAMAGE);
                }
            }
            _ => {}
        }
    }
    tilt(c.body, tilt_angle);
    head(c.creature, head_angle);
    neck(c.creature, neck_angle);
    if c.body.current_state < CLIMB_4 {
        let climb = c.vault(angle, VAULT_SHIFT);
        if let Some(&(_, clip, state)) = CLIMB_CLIPS.iter().find(|(c, ..)| *c == climb)
            && let Some(model) = c.world.data.model(KIND)
        {
            c.animation.animation = model.animations[0] + clip;
            c.animation.frame = 0.0;
            c.body.current_state = state;
        }
    } else {
        c.animate(angle, 0);
    }
}
