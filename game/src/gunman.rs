//! Venice's gunmen (types 16, 17 and 18, three looks of one man): their
//! control `0x8004DFE4` on the shared creature routines in
//! [`crate::creature`]. Not violent. They walk or run toward Lara and, once
//! she is ahead within 8192 units and in clear sight, stop to aim and fire
//! (standing or kneeling) or fire on the run. Each shot rolls to hit, worse
//! at range and against a Lara running across the line of fire, and a hit
//! takes 50.
use bevy::prelude::*;

use crate::creature::{AiInfo, AppCreatures, CreatureContext, Mood, head, tilt, turn};
use engine::camera::chase::{Point, Space, line_of_sight, sin};

/// The three types, all run by [`control`].
pub const KINDS: [u16; 3] = [16, 17, 18];

/// Gunman states (its clips' state numbers).
pub const WALK: usize = 1;
pub const RUN: usize = 2;
pub const STOP: usize = 3;
pub const WAIT: usize = 4;
pub const LOOK: usize = 5;
pub const AIM: usize = 6;
pub const SHOOT: usize = 7;
pub const RUN_SHOOT: usize = 9;
pub const KNEEL_AIM: usize = 10;
pub const KNEEL_SHOOT: usize = 11;
pub const DEATH: usize = 12;
/// The first of its two death clips (one chosen at random), counted from
/// its model's first (`0x8004E088`).
const DEATH_CLIP: usize = 20;

/// Turn limit a tick while walking or running; stopping clears it
/// (`0x8004E11C`).
const TURN: i16 = 910;
/// Squared distances: beyond 2048 it runs rather than walks (and fires on
/// the run), and it can fire at Lara within 8192.
const RUN_RANGE: i32 = 0x3F_FFFF;
const SHOT_RANGE: i32 = 0x400_0000;
/// Chances out of 32768: to stand or kneel to aim, to look about or walk
/// on while bored, and to leave a bored look or walk.
const STAND_AIM: i32 = 0x4000;
const WAIT_TO_LOOK: i32 = 1280;
const WAIT_TO_WALK: i32 = 2560;
const LEAVE_LOOK: i32 = 256;
const WALK_TO_STOP: i32 = 768;
/// Its gun: damage a hit, and the height its shots leave from and aim at.
const SHOT_DAMAGE: i32 = 50;
const GUN_HEIGHT: i32 = 768;

pub struct GunmanRulesPlugin;

impl Plugin for GunmanRulesPlugin {
    fn build(&self, app: &mut App) {
        for kind in KINDS {
            app.register_creature(kind, control);
        }
    }
}

/// The gunmen's control `0x8004DFE4`.
pub fn control(c: &mut CreatureContext) {
    let (mut head_angle, mut angle, mut tilt_angle) = (0, 0, 0);
    if *c.health <= 0 {
        if c.body.current_state != DEATH
            && let Some(model) = c.world.data.model(KINDS[0])
        {
            // All three take type 16's clips (`0x8004E084`).
            let clip = DEATH_CLIP + (c.random.draw() >> 14) as usize;
            c.animation.animation = model.animations[0] + clip;
            c.animation.frame = 0.0;
            c.body.current_state = DEATH;
        }
    } else {
        let info = c.ai_info();
        c.mood(&info, false);
        angle = turn(c.creature, c.body, c.creature.max_turn);
        let mood = c.creature.mood;
        let state = c.body.current_state;
        match state {
            STOP => {
                c.creature.max_turn = 0;
                if c.body.required_state != 0 {
                    c.goal.state = c.body.required_state;
                }
            }
            WAIT => {
                let next = if mood == Mood::Escape {
                    Some(RUN)
                } else if targetable(c, &info) {
                    Some(aim_state(c))
                } else if mood == Mood::Bored && info.ahead {
                    // Otherwise it waits on (`0x8004E1AC`).
                    let roll = c.random.draw();
                    if roll < WAIT_TO_LOOK {
                        Some(LOOK)
                    } else if roll < WAIT_TO_WALK {
                        Some(WALK)
                    } else {
                        None
                    }
                } else if mood == Mood::Bored || info.distance <= RUN_RANGE {
                    Some(WALK)
                } else {
                    Some(RUN)
                };
                if let Some(next) = next {
                    c.body.required_state = next;
                    c.goal.state = STOP;
                }
            }
            LOOK => {
                if targetable(c, &info) {
                    c.goal.state = STOP;
                    c.body.required_state = AIM;
                } else if mood != Mood::Bored || c.random.draw() < LEAVE_LOOK || !info.ahead {
                    c.goal.state = STOP;
                }
            }
            WALK => {
                c.creature.max_turn = TURN;
                if mood == Mood::Escape {
                    c.goal.state = RUN;
                } else if targetable(c, &info) {
                    c.body.required_state = aim_state(c);
                    c.goal.state = STOP;
                } else if info.distance > RUN_RANGE + 1 || !info.ahead {
                    c.goal.state = RUN;
                } else if mood == Mood::Bored && c.random.draw() < WALK_TO_STOP {
                    c.goal.state = STOP;
                }
            }
            RUN => {
                c.creature.max_turn = TURN;
                tilt_angle = angle / 4;
                c.creature.flags = 0;
                if mood == Mood::Escape {
                    if targetable(c, &info) {
                        c.goal.state = RUN_SHOOT;
                    }
                } else if targetable(c, &info) {
                    c.goal.state =
                        if info.distance > RUN_RANGE && info.zone == info.enemy_zone { RUN_SHOOT } else { STOP };
                } else if mood == Mood::Bored {
                    c.goal.state = STOP;
                }
            }
            AIM | KNEEL_AIM => {
                c.creature.flags = 0;
                if info.ahead {
                    head_angle = info.angle;
                }
                c.goal.state = if mood != Mood::Escape && targetable(c, &info) {
                    if state == AIM { SHOOT } else { KNEEL_SHOOT }
                } else {
                    STOP
                };
            }
            SHOOT | KNEEL_SHOOT => {
                if info.ahead {
                    head_angle = info.angle;
                }
                if c.creature.flags == 0 {
                    shoot(c, &info, SHOT_DAMAGE);
                    c.creature.flags = 1;
                }
            }
            RUN_SHOOT => {
                if info.ahead {
                    head_angle = info.angle;
                }
                if c.body.required_state == 0 {
                    if !shoot(c, &info, SHOT_DAMAGE) {
                        c.goal.state = RUN;
                    }
                    c.body.required_state = RUN_SHOOT;
                }
            }
            _ => {}
        }
    }
    tilt(c.body, tilt_angle);
    head(c.creature, head_angle);
    c.animate(angle, 0);
}

/// Aim standing or kneeling, at random (`0x8004E270`).
fn aim_state(c: &mut CreatureContext) -> usize {
    if c.random.draw() < STAND_AIM { AIM } else { KNEEL_AIM }
}

/// Targetable `0x8004D648`: Lara alive, ahead, within 8192 and in clear
/// sight from gun height to her gun height.
fn targetable(c: &CreatureContext, info: &AiInfo) -> bool {
    if c.enemy.health <= 0 || !info.ahead || info.distance >= SHOT_RANGE {
        return false;
    }
    let [x, y, z] = c.body.position;
    let start = Point { x, y: y - GUN_HEIGHT, z, room: c.body.room };
    let [ex, ey, ez] = c.enemy.position;
    let mut end = Point { x: ex, y: ey - GUN_HEIGHT, z: ez, room: c.enemy.room };
    line_of_sight(Space { world: c.world, camera: false }, start, &mut end)
}

/// ShotLara `0x8004DC90`: fire at Lara if she can be targeted, and roll to
/// hit. The chance falls with distance and with her speed across the line
/// of fire; a hit costs her `damage` and draws once more for its blood
/// (GunHit `0x8004DA24`). Returns whether it could fire at all. The muzzle
/// flash and a miss's ricochet are presentation only.
fn shoot(c: &mut CreatureContext, info: &AiInfo, damage: i32) -> bool {
    if info.distance > SHOT_RANGE || !targetable(c, info) {
        return false;
    }
    // The original's 32-bit arithmetic, overflow included.
    let across = ((sin(i32::from(info.enemy_facing)) * c.enemy.speed) >> 14).wrapping_shl(26);
    let across = ((i64::from(across) * 0x1B4E_81B5 >> 32) as i32 >> 5) - (across >> 31);
    let reach = info.distance.wrapping_add(across.wrapping_mul(across));
    let hit = reach <= SHOT_RANGE && {
        let left = SHOT_RANGE - info.distance;
        let chance = (((i64::from(left) * i64::from(0xA00A_00A1_u32 as i32)) >> 32) as i32 + left) >> 11;
        c.random.draw() < chance - (left >> 31) + 8192
    };
    if hit {
        c.random.draw();
        c.player.health -= damage;
        c.enemy.health = c.player.health;
    }
    true
}
