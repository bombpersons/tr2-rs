//! The crow (type 38): BirdControl `0x800128A4`, which the eagle (type 47)
//! shares, on the shared creature routines with a flying search. It sits
//! eating until it is roused, flies at Lara, glides when she is not ahead
//! and close, and pecks once per dive for 20. Shot down, it falls under
//! gravity and lies where it lands.
use bevy::prelude::*;
use engine::{animation::ObjectAnimation, objects::WorldObject, random::GameRandom};

use crate::creature::{AppCreatures, CreatureContext, Mood, turn};
use crate::items::ItemBody;

pub const KIND: u16 = 38;
pub const EAGLE: u16 = 47;

/// Bird states (its clips' state numbers).
const FLY: usize = 1;
const STOP: usize = 2;
const GLIDE: usize = 3;
const FALL: usize = 4;
const DEAD: usize = 5;
const ATTACK: usize = 6;
const EAT: usize = 7;

/// Its turn a tick in flight.
const TURN: i16 = 546;
/// Squared distance it dives at Lara within: 512.
const ATTACK_RANGE: i32 = 0x4_0000;
const PECK: i32 = 20;

pub struct CrowRulesPlugin;

impl Plugin for CrowRulesPlugin {
    fn build(&self, app: &mut App) {
        app.register_creature(KIND, control).add_systems(
            FixedUpdate,
            initialise
                .in_set(engine::schedule::LifecycleSystems::Initialize)
                .after(engine::objects::initialize_objects)
                .run_if(engine::schedule::simulation_enabled),
        );
    }
}

/// The bird's clips: where its death fall and its resting pose sit among
/// its model's, and the state it rests in.
fn clips(kind: u16) -> (usize, usize, usize) {
    if kind == KIND { (1, 14, EAT) } else { (8, 5, STOP) }
}

/// Added once a bird's own InitialiseItem routine has run.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BirdInitialised;

/// InitialiseBird `0x800127DC`: InitialiseCreature turns it by a random
/// amount (`0x80014C04`), then it rests on its eating clip.
fn initialise(
    mut commands: Commands,
    world: Option<Res<engine::world::WorldGeometry>>,
    mut random: ResMut<GameRandom>,
    mut birds: Query<
        (Entity, &WorldObject, &mut ItemBody, &mut ObjectAnimation, &mut engine::animation::AnimationGoal),
        Without<BirdInitialised>,
    >,
) {
    let Some(world) = world else { return };
    let mut added: Vec<_> = birds.iter_mut().filter(|(_, o, ..)| o.kind == KIND).collect();
    // Items initialise in placement order.
    added.sort_by_key(|(_, o, ..)| o.index);
    for (entity, object, mut body, mut animation, mut goal) in added {
        commands.entity(entity).insert(BirdInitialised);
        let turned = (random.draw() - 0x4000) >> 1;
        body.yaw = body.yaw.wrapping_add(turned as i16);
        body.collidable = true;
        let Some(model) = world.data.model(object.kind) else { continue };
        let (_, rest, state) = clips(object.kind);
        animation.animation = model.animations[0] + rest;
        animation.frame = 0.0;
        body.current_state = state;
        goal.state = state;
    }
}

/// BirdControl `0x800128A4`.
pub fn control(c: &mut CreatureContext) {
    let kind = c.kind;
    if *c.health <= 0 {
        match c.body.current_state {
            FALL => {
                if c.body.floor < c.body.position[1] {
                    c.goal.state = DEAD;
                    c.body.position[1] = c.body.floor;
                    c.body.fall_speed = 0;
                    c.body.gravity = false;
                }
            }
            DEAD => c.body.position[1] = c.body.floor,
            _ => {
                if let Some(model) = c.world.data.model(kind) {
                    c.animation.animation = model.animations[0] + clips(kind).0;
                    c.animation.frame = 0.0;
                }
                c.body.current_state = FALL;
                c.body.speed = 0;
                c.body.gravity = true;
            }
        }
        c.body.pitch = 0;
        // The original passes a register it never set here as the turn; it
        // only matters if the body is pushed off a sector edge.
        c.animate(0, 0);
        return;
    }
    let info = c.ai_info();
    c.mood(&info, false);
    let angle = turn(c.creature, c.body, TURN);
    let close = info.ahead && info.distance < ATTACK_RANGE;
    let (creature, body, goal) = (&mut *c.creature, &mut *c.body, &mut *c.goal);
    match body.current_state {
        EAT | STOP => {
            body.position[1] = body.floor;
            if creature.mood != Mood::Bored {
                goal.state = FLY;
            }
        }
        FLY => {
            creature.flags = 0;
            if body.required_state != 0 {
                goal.state = body.required_state;
            }
            goal.state = if creature.mood == Mood::Bored {
                STOP
            } else if close {
                ATTACK
            } else {
                GLIDE
            };
        }
        GLIDE => {
            if creature.mood == Mood::Bored {
                body.required_state = STOP;
                goal.state = FLY;
            } else if close {
                goal.state = ATTACK;
            }
        }
        ATTACK if creature.flags == 0 && body.touch_bits != 0 => {
            // CreatureEffect puts blood at its beak: presentation only.
            c.player.damage(PECK);
            c.enemy.health = c.player.health;
            creature.flags = 1;
        }
        _ => {}
    }
    c.animate(angle, 0);
}
