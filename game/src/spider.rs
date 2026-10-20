//! The small spider (type 36): SpiderControl `0x8005C1EC` on the shared
//! creature routines in [`crate::creature`]. It is not violent: hit or out of
//! reach it may flee. Bored, it now and then wanders; otherwise it scuttles
//! at its enemy, bites anything it touches for 25, and leaps from close
//! range. Its movement (`0x8005C0D4`) refuses a climb of more than 384
//! units, replaying its leap clip instead. It dies by bursting into pieces.
use bevy::prelude::*;

use crate::creature::{AppCreatures, CreatureContext, ItemStatus, Mood, turn};

pub const KIND: u16 = 36;

/// Spider states (its clips' state numbers).
pub const STOP: usize = 1;
pub const WALK: usize = 2;
pub const RUN: usize = 3;
pub const BITE: usize = 4;
pub const LEAP: usize = 5;
pub const LUNGE: usize = 6;
/// The leap clip, counted from the model's first (`0x8005C198`).
const LEAP_CLIP: usize = 2;

/// Fixed turn limit, binary angle units a tick (`0x8005C2CC`).
const TURN: i16 = 1456;
/// Squared distances: lunge within 204, leap within 512 (`0x8005C404`).
const LUNGE_RANGE: i32 = 0xA28F;
const LEAP_RANGE: i32 = 0x3_FFFF;
/// Chance out of 32768 to start or stop wandering while bored.
const WANDER_CHANCE: i32 = 256;
const BITE_DAMAGE: i32 = 25;
/// A step up of more than this is refused (`0x8005C14C`).
const MAX_CLIMB: i32 = 384;
/// The spider model's meshes: ExplodingDeath throws one piece per mesh.
const PIECES: usize = 30;

pub struct SpiderRulesPlugin;

impl Plugin for SpiderRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control);
    }
}

/// SpiderControl `0x8005C1EC`.
pub fn control(c: &mut CreatureContext) {
    if *c.health <= 0 {
        // ExplodingDeath (0x800498D8) throws every mesh as an effect piece
        // with three random draws each, finishing in one call; then
        // DisableBaddieAI, KillItem and sound 349. The pieces' own flight is
        // presentation and not ported. Assumes free effect slots.
        for _ in 0..PIECES * 3 {
            c.random.draw();
        }
        c.creature.search = None;
        c.body.status = ItemStatus::Deactivated;
        c.body.collidable = false;
        c.body.removed = true;
        return;
    }
    let info = c.ai_info();
    c.mood(&info, false);
    let angle = turn(c.creature, c.body, TURN);
    let (creature, body, goal) = (&mut *c.creature, &mut *c.body, &mut *c.goal);
    let attacking = matches!(creature.mood, Mood::Attack | Mood::Escape);
    match body.current_state {
        STOP => {
            creature.flags = 0;
            if creature.mood == Mood::Bored {
                if c.random.draw() < WANDER_CHANCE {
                    goal.state = WALK;
                }
            } else if info.ahead && body.touch_bits != 0 {
                goal.state = BITE;
            } else if creature.mood == Mood::Stalk {
                goal.state = WALK;
            } else if attacking {
                goal.state = RUN;
            }
        }
        WALK => {
            if creature.mood == Mood::Bored {
                if c.random.draw() < WANDER_CHANCE {
                    goal.state = STOP;
                }
            } else if attacking {
                goal.state = RUN;
            }
        }
        RUN => {
            creature.flags = 0;
            if matches!(creature.mood, Mood::Bored | Mood::Stalk) {
                goal.state = WALK;
            } else if info.ahead {
                if body.touch_bits != 0 {
                    goal.state = STOP;
                } else if info.distance <= LUNGE_RANGE {
                    goal.state = LUNGE;
                } else if info.distance <= LEAP_RANGE {
                    goal.state = LEAP;
                }
            }
        }
        BITE | LEAP | LUNGE if creature.flags == 0 && body.touch_bits != 0 => {
            // CreatureEffect with blood at the bite point: presentation only.
            c.player.damage(BITE_DAMAGE);
            c.enemy.health = c.player.health;
            creature.flags = 1;
        }
        _ => {}
    }
    leap(c, angle);
}

/// SpiderLeap `0x8005C0D4`: move as any creature, but a step that would lift
/// it more than 384 is undone and its leap clip played from there instead.
fn leap(c: &mut CreatureContext, angle: i16) {
    let (old, room) = (c.body.position, c.body.room);
    c.animate(angle, 0);
    if c.body.position[1] > old[1] - MAX_CLIMB {
        return;
    }
    c.body.position = old;
    c.room_order.enter(c.body, room);
    if let Some(model) = c.world.data.model(KIND) {
        c.animation.animation = model.animations[0] + LEAP_CLIP;
        c.animation.frame = 0.0;
    }
    c.body.current_state = LEAP;
    c.animate(angle, 0);
}
