//! The creature routines every Tomb Raider II enemy shares, ported once:
//! AI slots, what a creature knows about its enemy, its mood and path,
//! turning, tilt, head and movement with box collision. Each enemy type's
//! own controller (`crate::tiger`) calls them in the original's order and
//! keeps only its state machine and attack rules.
//!
//! Evidence (docs/tiger-evidence.md): CreatureActive `0x80014C74`,
//! CreatureAIInfo `0x80014D00`, CreatureMood `0x80015800`, CreatureTurn
//! `0x800172E0`, CreatureTilt `0x8001745C`, CreatureHead `0x800174A8`,
//! CreatureAnimation `0x800166F8` with AnimateItem `0x8001DD98`, BadFloor
//! `0x800164A8` and the item-overlap check `0x80016354`; slot set-up
//! EnableBaddieAI `0x80048C2C`, InitialiseSlot `0x80048EC8`. The path search
//! itself is the engine's [`BoxSearch`].
//!
//! Positions, angles and speeds are the original's integers (world units,
//! y down, 65536 to a turn) in [`CreatureBody`]; `Transform` follows them.
use bevy::prelude::*;
use engine::{
    animation::{
        AnimationCue, AnimationGoal, ObjectAnimation, ObjectAnimationCue, frame_bounds, step_animation_frame_from,
    },
    camera::chase::{cos, heading, sin},
    objects::WorldObject,
    random::GameRandom,
    schedule::{InteractionSystems, ItemSystems, LifecycleSystems, WorldSystems, simulation_enabled},
    triggers::TriggerSystems,
    world::{BoxFlags, BoxSearch, Command, Level, NO_BOX, TargetKind, WorldGeometry},
};

use crate::interactions::Enemy;
use crate::player::{Motion, Player};

/// Creature AI slots (`0x8008C1C0`, five of 68 bytes).
pub const SLOTS: usize = 5;

/// What a creature wants from its enemy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum Mood {
    #[default]
    Bored,
    Attack,
    Escape,
    Stalk,
}

/// A creature type's set-up values: the object table's pivot (+0x28) and
/// radius (+0x2A), and the search limits InitialiseSlot writes per type.
#[derive(serde::Deserialize, Debug, Clone, Copy, Reflect)]
pub struct CreatureSetup {
    /// Distance ahead of the origin the creature measures its enemy from.
    #[serde(default)]
    pub pivot: i32,
    pub radius: i32,
    /// Largest rise and drop (negative) its path may take between boxes.
    #[serde(default = "default_step")]
    pub step: i32,
    #[serde(default = "default_drop")]
    pub drop: i32,
    /// The joint its bone flags turn by its head angle (Y).
    #[serde(default)]
    pub head_joint: Option<usize>,
}

fn default_step() -> i32 {
    256
}

fn default_drop() -> i32 {
    -512
}

/// Item status bits 1-2 (`item+0x48`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub enum ItemStatus {
    /// Placed, not yet triggered.
    #[default]
    Inactive,
    Active,
    /// Its animation ran a kill command; the AI slot is released.
    Deactivated,
}

/// The original item fields a creature's code reads and writes.
#[derive(Component, Reflect, Debug, Clone, Default)]
#[reflect(Component)]
pub struct CreatureBody {
    /// World units, y down.
    pub position: [i32; 3],
    pub pitch: i16,
    pub yaw: i16,
    pub roll: i16,
    pub room: usize,
    /// Navigation box under the creature, refreshed by [`ai_info`].
    pub box_number: i16,
    pub speed: i16,
    pub fall_speed: i16,
    /// Floor height under it, from the last movement.
    pub floor: i32,
    /// The state it is in (`item+0x0E`). Usually its clip's, but a
    /// controller may set it apart (a death it is about to play).
    pub current_state: usize,
    /// A state the controller wants played once (`item+0x12`), or 0.
    pub required_state: usize,
    pub gravity: bool,
    /// Shot since its last animation step (status bit 0x10).
    pub hit: bool,
    /// Which of its meshes touched Lara in her last collision pass.
    pub touch_bits: u32,
    pub status: ItemStatus,
    /// Lara collides with it (status bit 0x20).
    pub collidable: bool,
    /// Taken out of the level (KillItem `0x8003749C`): no longer drawn.
    pub removed: bool,
    /// When it last joined its room's item list (see [`RoomOrder`]).
    pub room_entry: u64,
}

/// The order items joined their rooms' item lists. The original keeps a
/// list per room; an item is pushed onto its head when placed (in placement
/// order) and when it changes room (ItemNewRoom). CreatureCollided walks a
/// room's list from the head and stops at itself, so a creature only walks
/// into those that joined the room after it did.
#[derive(Resource, Debug, Default)]
pub struct RoomOrder {
    next: u64,
}

impl RoomOrder {
    /// Move `body` into `room`, at the head of its list if it changed room.
    pub fn enter(&mut self, body: &mut CreatureBody, room: usize) {
        if body.room != room {
            body.room = room;
            body.room_entry = self.next;
            self.next += 1;
        }
    }

    /// Placements take entries up to their index; later moves come after.
    fn after_placements(&mut self, count: usize) {
        self.next = self.next.max(count as u64);
    }
}

/// A creature's AI slot: its mood, steering target and path search.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct Creature {
    pub setup: CreatureSetup,
    /// Head turn toward its enemy, binary angle.
    pub head: i16,
    pub neck: i16,
    pub max_turn: i16,
    /// Type-specific flags; the tiger's records a landed bite.
    pub flags: i16,
    pub mood: Mood,
    /// Where it steers this tick.
    pub target: [i32; 3],
    /// The path search, present while it holds an AI slot.
    pub search: Option<BoxSearch>,
}

impl Creature {
    pub fn new(setup: CreatureSetup) -> Self {
        Self { setup, head: 0, neck: 0, max_turn: 0, flags: 0, mood: Mood::Bored, target: [0; 3], search: None }
    }

    /// The head joint turned by the creature's head angle, as its model's
    /// bone flags (set per type at load) turn it for drawing and collision.
    pub fn joint_turns(&self) -> Vec<engine::animation::JointPose> {
        let Some(joint) = self.setup.head_joint else { return vec![] };
        let mut turns = vec![engine::animation::JointPose::default(); joint + 1];
        turns[joint].after = [0, self.head, 0];
        turns
    }
}

/// What CreatureAIInfo learns about the enemy this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct AiInfo {
    pub zone: i16,
    /// With bit 0x4000 when the enemy's box is blocked or unreachable.
    pub enemy_zone: i16,
    /// Squared horizontal distance from the pivot point.
    pub distance: i32,
    /// The enemy is within a quarter turn either side.
    pub ahead: bool,
    /// Ahead, alive and within 384 units of height.
    pub bite: bool,
    pub angle: i16,
    pub enemy_facing: i16,
}

/// Lara as the creatures see her: the item fields they read.
#[derive(Clone, Copy, Debug)]
pub struct EnemyView {
    pub position: [i32; 3],
    pub yaw: i16,
    pub room: usize,
    pub box_number: i16,
    pub health: i32,
    pub speed: i32,
}

impl EnemyView {
    pub fn of(player: &Player, motion: Option<&Motion>) -> Self {
        let p = (player.position * 1024.0).round();
        let yaw = (player.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16;
        let speed = motion.map_or(0.0, |m| m.velocity.with_y(0.0).length() * 1024.0 / 30.0).round() as i32;
        Self {
            position: [p.x as i32, -p.y as i32, p.z as i32],
            yaw,
            room: player.room,
            box_number: NO_BOX,
            health: player.health,
            speed,
        }
    }
}

/// Zone-table indices of the level file for a search: ground zones by step
/// (zones[0] for 256, zones[1] for 512), fly at 4; flipped rooms five on.
pub fn zone_tables(step: i32, fly: bool) -> [usize; 2] {
    let normal = if fly { 4 } else { ((step >> 8) - 1).max(0) as usize };
    [normal, normal + 5]
}

fn box_of(world: &WorldGeometry, room: usize, x: i32, z: i32) -> i16 {
    world.navigation_box(room, x, z).map_or(NO_BOX, |b| b as i16)
}

fn zone_at(zone: &[i16], box_number: i16) -> i16 {
    usize::try_from(box_number).ok().and_then(|b| zone.get(b).copied()).unwrap_or(i16::MIN)
}

/// InitialiseSlot `0x80048EC8` and CreateZone `0x800490EC`: give a creature
/// an AI slot. Turn limit 182, bored, a cleared search over its own zone.
pub fn enable(world: &WorldGeometry, creature: &mut Creature, body: &mut CreatureBody) {
    let setup = creature.setup;
    let mut search = BoxSearch::new(world.data.boxes.len(), setup.step, setup.drop, 0, zone_tables(setup.step, false));
    body.box_number = box_of(world, body.room, body.position[0], body.position[2]);
    search.create_zone(&world.data, body.box_number);
    creature.head = 0;
    creature.neck = 0;
    creature.max_turn = 182;
    creature.flags = 0;
    creature.mood = Mood::Bored;
    creature.search = Some(search);
}

/// CreatureAIInfo `0x80014D00`.
pub fn ai_info(
    world: &WorldGeometry,
    flags: &BoxFlags,
    creature: &Creature,
    body: &mut CreatureBody,
    enemy: &mut EnemyView,
    flipped: bool,
) -> AiInfo {
    let Some(search) = &creature.search else { return AiInfo::default() };
    let level = &world.data;
    let zone = search.zone(level, flipped);
    body.box_number = box_of(world, body.room, body.position[0], body.position[2]);
    enemy.box_number = box_of(world, enemy.room, enemy.position[0], enemy.position[2]);
    let mut info =
        AiInfo { zone: zone_at(zone, body.box_number), enemy_zone: zone_at(zone, enemy.box_number), ..default() };
    let blocked = usize::try_from(enemy.box_number)
        .ok()
        .filter(|&b| b < level.boxes.len())
        .is_some_and(|b| flags.overlap(level, b) & search.block_mask != 0);
    let unreachable = usize::try_from(body.box_number)
        .ok()
        .and_then(|b| search.nodes.get(b))
        .is_some_and(|n| n.search_number == search.search_number | 0x8000);
    if blocked || unreachable {
        info.enemy_zone |= 0x4000;
    }
    let yaw = i32::from(body.yaw);
    let pivot = creature.setup.pivot;
    let dz = enemy.position[2].wrapping_sub(body.position[2] + ((pivot * cos(yaw)) >> 14));
    let dx = enemy.position[0].wrapping_sub(body.position[0] + ((pivot * sin(yaw)) >> 14));
    let angle = heading(dx, dz);
    info.distance = dz.wrapping_mul(dz).wrapping_add(dx.wrapping_mul(dx));
    info.angle = (angle - yaw) as i16;
    info.enemy_facing = (angle - (i32::from(enemy.yaw) + 0x8000)) as i16;
    info.ahead = ((i32::from(info.angle) + 16383) as u16) < 32767;
    info.bite = info.ahead && enemy.health > 0 && (enemy.position[1] - body.position[1]).abs() <= 384;
    info
}

/// ValidBox `0x800156DC`: a box of the creature's zone, not blocked, that
/// it is not already standing in.
fn valid_box(
    level: &Level,
    flags: &BoxFlags,
    search: &BoxSearch,
    body: &CreatureBody,
    zone_number: i16,
    box_number: i16,
    flipped: bool,
) -> bool {
    if zone_at(search.zone(level, flipped), box_number) != zone_number {
        return false;
    }
    let b = level.boxes[box_number as usize];
    if flags.overlap(level, box_number as usize) & search.block_mask != 0 {
        return false;
    }
    let [x, _, z] = body.position;
    !((b.z_min << 10) < z && z < (b.z_max << 10) && (b.x_min << 10) < x && x < (b.x_max << 10))
}

/// Quadrant of a point around the enemy (0..3), as both box tests read it.
fn quadrant(z_beyond: bool, x_beyond: bool) -> i32 {
    match (z_beyond, x_beyond) {
        (true, true) => 2,
        (true, false) => 1,
        (false, true) => 3,
        (false, false) => 0,
    }
}

/// StalkBox `0x800154B8`: near the enemy, and not on the far side of it.
fn stalk_box(level: &Level, body: &CreatureBody, enemy: &EnemyView, box_number: i16) -> bool {
    let b = level.boxes[box_number as usize];
    let z = ((b.z_min + b.z_max) << 9) - enemy.position[2];
    let x = ((b.x_min + b.x_max) << 9) - enemy.position[0];
    let z_range = ((b.z_max - b.z_min) << 10) + 3072;
    let x_range = ((b.x_max - b.x_min) << 10) + 3072;
    if x > x_range || x < -x_range || z > z_range || z < -z_range {
        return false;
    }
    let enemy_quad = (i32::from(enemy.yaw) >> 14) + 2;
    let box_quad = quadrant(z > 0, x > 0);
    if enemy_quad == box_quad {
        return false;
    }
    let baddie_quad = quadrant(body.position[2] > enemy.position[2], body.position[0] > enemy.position[0]);
    !(enemy_quad == baddie_quad && (enemy_quad - box_quad).abs() == 2)
}

/// EscapeBox `0x80015608`: far from the enemy, on the creature's side.
fn escape_box(level: &Level, body: &CreatureBody, enemy: &EnemyView, box_number: i16) -> bool {
    let b = level.boxes[box_number as usize];
    let z = ((b.z_min + b.z_max) << 9) - enemy.position[2];
    let x = ((b.x_min + b.x_max) << 9) - enemy.position[0];
    if ((x + 5119) as u32) < 10239 && ((z + 5119) as u32) < 10239 {
        return false;
    }
    let same_z = if body.position[2] > enemy.position[2] { z > 0 } else { z <= 0 };
    let same_x = if body.position[0] > enemy.position[0] { x > 0 } else { x <= 0 };
    same_z || same_x
}

/// CreatureMood `0x80015800`: update the mood from the enemy's zone and
/// hits (`violent` creatures attack when they can, others may flee), pick a
/// box to head for, and work out this tick's steering target.
#[allow(clippy::too_many_arguments)]
pub fn mood(
    world: &WorldGeometry,
    flags: &BoxFlags,
    creature: &mut Creature,
    body: &CreatureBody,
    info: &AiInfo,
    enemy: Option<&EnemyView>,
    violent: bool,
    random: &mut GameRandom,
    flipped: bool,
) {
    let level = &world.data;
    let Some(mut search) = creature.search.take() else { return };
    let mut draw = || random.draw();
    let item_node = search.nodes.get(body.box_number.max(0) as usize).copied();
    if item_node.is_some_and(|n| n.search_number == search.search_number | 0x8000) {
        search.required_box = NO_BOX;
    }
    if creature.mood != Mood::Attack
        && search.required_box != NO_BOX
        && !valid_box(level, flags, &search, body, info.zone, search.required_box, flipped)
    {
        if info.zone == info.enemy_zone {
            creature.mood = Mood::Bored;
        }
        search.required_box = NO_BOX;
    }
    let old = creature.mood;
    let enemy = match enemy {
        Some(enemy) if enemy.health > 0 => {
            let same_zone = info.zone == info.enemy_zone;
            creature.mood = if violent {
                match creature.mood {
                    Mood::Attack if !same_zone => Mood::Bored,
                    Mood::Bored | Mood::Stalk if same_zone => Mood::Attack,
                    Mood::Bored | Mood::Stalk if body.hit => Mood::Escape,
                    Mood::Escape if same_zone => Mood::Attack,
                    mood => mood,
                }
            } else {
                match creature.mood {
                    Mood::Attack if body.hit && (draw() < 2048 || !same_zone) => Mood::Escape,
                    Mood::Attack if !body.hit && !same_zone => Mood::Bored,
                    Mood::Bored | Mood::Stalk if body.hit && (draw() < 2048 || !same_zone) => Mood::Escape,
                    Mood::Bored | Mood::Stalk if same_zone => {
                        if info.distance > 0x8F_FFFF && (creature.mood != Mood::Stalk || search.required_box != NO_BOX)
                        {
                            Mood::Stalk
                        } else {
                            Mood::Attack
                        }
                    }
                    Mood::Escape if same_zone && draw() < 256 => Mood::Stalk,
                    mood => mood,
                }
            };
            Some(enemy)
        }
        // A dead (or absent) enemy leaves the creature bored.
        _ => {
            creature.mood = Mood::Bored;
            None
        }
    };
    if creature.mood != old {
        if old == Mood::Attack {
            let target = search.target_box;
            search.target_box(level, target, &mut draw);
        }
        search.required_box = NO_BOX;
    }
    let random_box = |search: &BoxSearch, draw: &mut dyn FnMut() -> i32| {
        let index = (draw() * search.zone_count as i32) >> 15;
        search.nodes[index as usize].box_number
    };
    match (creature.mood, enemy) {
        (Mood::Attack, Some(enemy)) => {
            search.target = enemy.position;
            search.required_box = enemy.box_number;
            // Flying creatures aim at the enemy's bounds; none are ported.
        }
        (Mood::Bored, _) => {
            let box_number = random_box(&search, &mut draw);
            if valid_box(level, flags, &search, body, info.zone, box_number, flipped) {
                if let Some(enemy) = enemy.filter(|e| e.health > 0)
                    && stalk_box(level, body, enemy, box_number)
                {
                    search.target_box(level, box_number, &mut draw);
                    creature.mood = Mood::Stalk;
                } else if search.required_box == NO_BOX {
                    search.target_box(level, box_number, &mut draw);
                }
            }
        }
        (Mood::Stalk, Some(enemy)) => {
            let current = search.required_box;
            if current == NO_BOX || !stalk_box(level, body, enemy, current) {
                let box_number = random_box(&search, &mut draw);
                if valid_box(level, flags, &search, body, info.zone, box_number, flipped) {
                    if stalk_box(level, body, enemy, box_number) {
                        search.target_box(level, box_number, &mut draw);
                    } else if search.required_box == NO_BOX {
                        search.target_box(level, box_number, &mut draw);
                        if info.zone != info.enemy_zone {
                            creature.mood = Mood::Bored;
                        }
                    }
                }
            }
        }
        (Mood::Escape, Some(enemy)) => {
            let box_number = random_box(&search, &mut draw);
            if valid_box(level, flags, &search, body, info.zone, box_number, flipped) && search.required_box == NO_BOX {
                if escape_box(level, body, enemy, box_number) {
                    search.target_box(level, box_number, &mut draw);
                } else if info.zone == info.enemy_zone && stalk_box(level, body, enemy, box_number) {
                    search.target_box(level, box_number, &mut draw);
                    creature.mood = Mood::Stalk;
                }
            }
        }
        _ => {}
    }
    if search.target_box == NO_BOX {
        search.target_box(level, body.box_number, &mut draw);
    }
    let (target, _kind): ([i32; 3], TargetKind) =
        search.calculate_target(level, flags, flipped, body.position, body.box_number, &mut draw);
    creature.target = target;
    creature.search = Some(search);
}

/// CreatureTurn `0x800172E0`: turn toward the target by at most
/// `max_turn`, half that for a target behind inside the turning circle.
/// Returns the turn made.
pub fn turn(creature: &Creature, body: &mut CreatureBody, max_turn: i16) -> i16 {
    if creature.search.is_none() || body.speed == 0 || max_turn == 0 {
        return 0;
    }
    let dz = creature.target[2] - body.position[2];
    let dx = creature.target[0] - body.position[0];
    let mut angle = (heading(dx, dz) - i32::from(body.yaw)) as i16 as i32;
    let mut cap = i32::from(max_turn);
    let range = (i32::from(body.speed) << 14) / cap;
    let behind = (angle + 16384) as u16 > 0x8000;
    if behind && dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz)) < range.wrapping_mul(range) {
        cap = i32::from(max_turn) >> 1;
    }
    angle = angle.clamp(-cap, cap);
    body.yaw = body.yaw.wrapping_add(angle as i16);
    angle as i16
}

/// CreatureTilt `0x8001745C`: lean toward `angle * 4`, 546 a tick at most.
pub fn tilt(body: &mut CreatureBody, angle: i16) {
    let change = ((i32::from(angle) << 2) - i32::from(body.roll)) as i16;
    body.roll = body.roll.wrapping_add(change.clamp(-546, 546));
}

/// CreatureHead `0x800174A8`: turn the head 910 a tick toward `required`,
/// within a third of a turn each side.
pub fn head(creature: &mut Creature, required: i16) {
    if creature.search.is_none() {
        return;
    }
    let change = required.wrapping_sub(creature.head).clamp(-910, 910);
    creature.head = creature.head.wrapping_add(change).clamp(-12288, 12288);
}

/// Another creature it would walk into (`0x80016354`): an active, moving
/// creature in the same room within its radius.
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub entity: Entity,
    pub room: usize,
    pub position: [i32; 3],
    pub moving: bool,
    /// Its [`CreatureBody::room_entry`].
    pub entry: u64,
}

/// What [`animate`] reports back to the type's controller.
pub struct Animated {
    pub cues: Vec<AnimationCue>,
}

/// BadFloor `0x800164A8`.
fn bad_floor(
    world: &WorldGeometry,
    flags: &BoxFlags,
    search: &BoxSearch,
    at: [i32; 3],
    height: i32,
    next_height: i32,
    room: usize,
) -> bool {
    let (_, floor_box) = world.floor_box(at[0], at[1], at[2], room);
    let Some((index, b)) = floor_box.and_then(|b| world.data.boxes.get(b).map(|bx| (b, bx))) else { return true };
    if flags.overlap(&world.data, index) & search.block_mask != 0 {
        return true;
    }
    let change = height - b.floor;
    if change > search.step || change < search.drop {
        return true;
    }
    if change < -search.step && b.floor > next_height {
        return true;
    }
    search.fly != 0 && at[1] > b.floor + search.fly
}

/// Floor height of a box and of the next box on its path.
fn heights(level: &Level, search: &BoxSearch, floor_box: Option<usize>) -> (i32, i32) {
    let Some(b) = floor_box else { return (0, 0) };
    let height = level.boxes[b].floor;
    let exit = search.nodes[b].exit_box;
    let next = if exit != NO_BOX { level.boxes[exit as usize].floor } else { height };
    (height, next)
}

/// AnimateItem `0x8001DD98` for a creature: clear contacts and hits, step
/// one authored frame, run its end commands, then move by its authored
/// speed (or fall).
fn animate_item(
    level: &Level,
    body: &mut CreatureBody,
    animation: &mut ObjectAnimation,
    goal: &mut AnimationGoal,
) -> Vec<AnimationCue> {
    body.touch_bits = 0;
    body.hit = false;
    let step = step_animation_frame_from(level, animation, goal, &mut body.current_state);
    if (step.changed || step.ended) && body.required_state == body.current_state {
        body.required_state = 0;
    }
    for cue in step.cues.iter().filter(|c| c.at_end) {
        match cue.command {
            Command::SetPosition([dx, dy, dz]) => {
                let yaw = i32::from(body.yaw);
                body.position[0] += (cos(yaw) * dx + sin(yaw) * dz) >> 14;
                body.position[1] += dy;
                body.position[2] += (cos(yaw) * dz - sin(yaw) * dx) >> 14;
            }
            Command::JumpVelocity([vertical, horizontal]) => {
                body.fall_speed = vertical as i16;
                body.speed = horizontal as i16;
                body.gravity = true;
            }
            Command::Kill => body.status = ItemStatus::Deactivated,
            _ => {}
        }
    }
    if let Some(clip) = level.animations.get(animation.animation) {
        if body.gravity {
            body.fall_speed += if body.fall_speed < 128 { 6 } else { 1 };
            body.position[1] += i32::from(body.fall_speed);
        } else {
            let frame = animation.frame.floor() as i32;
            body.speed = (clip.speed.wrapping_add(clip.accel.wrapping_mul(frame)) >> 16) as i16;
        }
    }
    let yaw = i32::from(body.yaw);
    body.position[0] += (sin(yaw) * i32::from(body.speed)) >> 14;
    body.position[2] += (cos(yaw) * i32::from(body.speed)) >> 14;
    step.cues
}

/// CreatureAnimation `0x800166F8`: animate and move the creature, keeping
/// it out of boxes its path may not enter by pushing it back from sector
/// edges, then settle it onto the floor. `angle` is the turn made this
/// tick (added again when it is pushed); returns false once its slot is
/// released.
#[allow(clippy::too_many_arguments)]
pub fn animate(
    world: &WorldGeometry,
    flags: &BoxFlags,
    creature: &mut Creature,
    body: &mut CreatureBody,
    animation: &mut ObjectAnimation,
    goal: &mut AnimationGoal,
    angle: i16,
    tilt_angle: i16,
    obstacles: &[Obstacle],
    order: &mut RoomOrder,
    this: Entity,
    health: &mut i32,
    cues: &mut Vec<AnimationCue>,
) -> bool {
    let level = &world.data;
    let Some(search) = creature.search.as_ref() else { return false };
    let old = body.position;
    let box_height = usize::try_from(body.box_number).ok().map_or(0, |b| level.boxes[b].floor);
    let zone = search.zone(level, false);
    // Room before the step (object flag bit 23 clear for walking creatures).
    if let Some(room) = world.floor_room(body.position[0], body.position[1], body.position[2], body.room) {
        order.enter(body, room);
    }
    cues.extend(animate_item(level, body, animation, goal));
    if body.status == ItemStatus::Deactivated {
        // CreatureDie 0x8001659C: no longer a target (hit points -16384),
        // no longer solid, off the active list, its AI slot released.
        // Items it carried are not ported (none of the ported types carry).
        *health = -16384;
        body.collidable = false;
        creature.search = None;
        return false;
    }
    let search = creature.search.as_ref().unwrap();
    let top = frame_bounds(level, animation.animation, animation.frame).map_or(0, |b| b[2]);
    let y = body.position[1] + top;
    let mut room = world.floor_room(old[0], y, old[2], body.room).unwrap_or(body.room);
    let (found, mut floor_box) = world.floor_box(body.position[0], y, body.position[2], room);
    room = found;
    let (mut height, mut next_height) = heights(level, search, floor_box);
    let floor_zone = floor_box.map_or(i16::MIN, |b| zone_at(zone, b as i16));
    let rise = box_height - height;
    if floor_box.is_none() || zone_at(zone, body.box_number) != floor_zone || rise > search.step || rise < search.drop {
        // Step back to the old sector's edge. Both axes test the new x
        // sector (0x800169C0..0x80016A40): z compares it with the old z
        // sector, so a creature in a far column snaps to a z edge too.
        let now = body.position[0] >> 10;
        for axis in [0, 2] {
            let was = old[axis] >> 10;
            if now < was {
                body.position[axis] = old[axis] & !1023;
            } else if now > was {
                body.position[axis] = old[axis] | 1023;
            }
        }
        let (found, again) = world.floor_box(body.position[0], y, body.position[2], room);
        room = found;
        floor_box = again;
        (height, next_height) = heights(level, search, floor_box);
    }
    let [x, _, z] = body.position;
    let (pos_x, pos_z) = (x & 1023, z & 1023);
    let radius = creature.setup.radius;
    let bad = |px: i32, pz: i32| bad_floor(world, flags, search, [px, y, pz], height, next_height, room);
    let yaw_in = |offset: i32| ((i32::from(body.yaw) + offset) as u16) < 32767;
    let (mut shift_x, mut shift_z) = (0, 0);
    let far = 1024 - radius;
    if pos_z < radius {
        if bad(x, z - radius) {
            shift_z = radius - pos_z;
        }
        if pos_x < radius {
            if bad(x - radius, z) {
                shift_x = radius - pos_x;
            } else if shift_z == 0 && bad(x - radius, z - radius) {
                if yaw_in(24575) {
                    shift_z = radius - pos_z;
                } else {
                    shift_x = radius - pos_x;
                }
            }
        } else if pos_x > far {
            if bad(x + radius, z) {
                shift_x = far - pos_x;
            } else if shift_z == 0 && bad(x + radius, z - radius) {
                if yaw_in(8191) {
                    shift_z = radius - pos_z;
                } else {
                    shift_x = far - pos_x;
                }
            }
        }
    } else if pos_z > far {
        if bad(x, z + radius) {
            shift_z = far - pos_z;
        }
        if pos_x < radius {
            if bad(x - radius, z) {
                shift_x = radius - pos_x;
            } else if shift_z == 0 && bad(x - radius, z + radius) {
                if yaw_in(8191) {
                    shift_x = radius - pos_x;
                } else {
                    shift_z = far - pos_z;
                }
            }
        } else if pos_x > far {
            if bad(x + radius, z) {
                shift_x = far - pos_x;
            } else if shift_z == 0 && bad(x + radius, z + radius) {
                if yaw_in(24575) {
                    shift_x = far - pos_x;
                } else {
                    shift_z = far - pos_z;
                }
            }
        }
    } else if pos_x < radius {
        if bad(x - radius, z) {
            shift_x = radius - pos_x;
        }
    } else if pos_x > far && bad(x + radius, z) {
        shift_x = far - pos_x;
    }
    body.position[0] += shift_x;
    body.position[2] += shift_z;
    if shift_x != 0 || shift_z != 0 {
        room = world.floor_room(body.position[0], y, body.position[2], room).unwrap_or(room);
        body.yaw = body.yaw.wrapping_add(angle);
        if tilt_angle != 0 {
            tilt(body, tilt_angle.wrapping_mul(2));
        }
    }
    let radius_sq = radius.wrapping_mul(radius);
    let collided = obstacles.iter().any(|o| {
        o.entity != this && o.moving && o.room == body.room && o.entry > body.room_entry && {
            let d = [0, 1, 2].map(|i| o.position[i] - body.position[i]);
            d[0].wrapping_mul(d[0]).wrapping_add(d[1].wrapping_mul(d[1])).wrapping_add(d[2].wrapping_mul(d[2]))
                < radius_sq
        }
    });
    if collided {
        body.position = old;
        return true;
    }
    // Walking creatures settle onto the floor, rising at once and dropping
    // 64 a tick (0x80017198..0x800171FC).
    room = world.floor_room(body.position[0], body.position[1], body.position[2], room).unwrap_or(room);
    let floor = world.floor_height(body.position[0], body.position[1], body.position[2], room).unwrap_or(-32512);
    body.floor = floor;
    let y = body.position[1];
    if floor < y {
        body.position[1] = floor;
    } else if floor - y > 64 {
        body.position[1] += 64;
    } else if y < floor {
        body.position[1] = floor;
    }
    body.pitch = 0;
    // Walking creatures drown in water rooms.
    let room_above = world.floor_room(body.position[0], body.position[1] - 512, body.position[2], room).unwrap_or(room);
    room = room_above;
    if level.rooms.get(room).is_some_and(|r| r.water) {
        *health = 0;
    }
    order.enter(body, room);
    true
}

/// The level's integer pose as a `Transform`.
pub fn sync_transform(body: &CreatureBody, transform: &mut Transform) {
    let [x, y, z] = body.position;
    transform.translation = Vec3::new(x as f32, -y as f32, z as f32) / 1024.0;
    let angle = |a: i16| f32::from(a) * std::f32::consts::TAU / 65536.0;
    transform.rotation = Quat::from_rotation_y(angle(body.yaw))
        * Quat::from_rotation_x(-angle(body.pitch))
        * Quat::from_rotation_z(-angle(body.roll));
}

/// Creatures on the original's active-item list, in update order: each
/// newly activated one goes to the front (AddActiveItem), and one is taken
/// off when it dies.
#[derive(Resource, Reflect, Default, Debug, Clone)]
#[reflect(Resource)]
pub struct ActiveCreatures(pub Vec<Entity>);

/// What a creature type's control function works on for one creature.
pub struct CreatureContext<'a> {
    pub world: &'a WorldGeometry,
    /// Navigation boxes as doors and blocks have left them.
    pub boxes: &'a BoxFlags,
    pub random: &'a mut GameRandom,
    pub player: &'a mut Player,
    /// Lara as this creature sees her this tick.
    pub enemy: &'a mut EnemyView,
    pub entity: Entity,
    pub creature: &'a mut Creature,
    pub body: &'a mut CreatureBody,
    pub health: &'a mut i32,
    pub animation: &'a mut ObjectAnimation,
    pub goal: &'a mut AnimationGoal,
    /// The other creatures, as they stand after those before this one moved.
    pub obstacles: &'a [Obstacle],
    pub room_order: &'a mut RoomOrder,
    pub cues: &'a mut Vec<AnimationCue>,
}

impl CreatureContext<'_> {
    /// CreatureAnimation for this creature (see [`animate`]).
    pub fn animate(&mut self, angle: i16, tilt_angle: i16) -> bool {
        animate(
            self.world,
            self.boxes,
            self.creature,
            self.body,
            self.animation,
            self.goal,
            angle,
            tilt_angle,
            self.obstacles,
            self.room_order,
            self.entity,
            self.health,
            self.cues,
        )
    }

    /// CreatureAIInfo for this creature (see [`ai_info`]).
    pub fn ai_info(&mut self) -> AiInfo {
        ai_info(self.world, self.boxes, self.creature, self.body, self.enemy, false)
    }

    /// CreatureMood for this creature (see [`mood`]).
    pub fn mood(&mut self, info: &AiInfo, violent: bool) {
        mood(self.world, self.boxes, self.creature, self.body, info, Some(self.enemy), violent, self.random, false);
    }
}

/// A creature type's control function: the original's per-type control
/// routine (the object table's +0x10).
pub type CreatureControlFn = fn(&mut CreatureContext);

/// Creature control functions by object type id.
#[derive(Resource, Default)]
pub struct CreatureControls(std::collections::BTreeMap<u16, CreatureControlFn>);

/// Registering a creature type's control function.
pub trait AppCreatures {
    /// Panics if `kind` already has a control function.
    fn register_creature(&mut self, kind: u16, control: CreatureControlFn) -> &mut Self;
    /// Replaces a type's control function, for a mod that changes an existing
    /// creature. Panics if `kind` has none yet (add the game's plugins first).
    fn replace_creature(&mut self, kind: u16, control: CreatureControlFn) -> &mut Self;
}

impl AppCreatures for App {
    fn register_creature(&mut self, kind: u16, control: CreatureControlFn) -> &mut Self {
        engine::add_once(self, CreatureRulesPlugin);
        if self.world_mut().resource_mut::<CreatureControls>().0.insert(kind, control).is_some() {
            panic!("creature type {kind} registered twice");
        }
        self
    }

    fn replace_creature(&mut self, kind: u16, control: CreatureControlFn) -> &mut Self {
        engine::add_once(self, CreatureRulesPlugin);
        if self.world_mut().resource_mut::<CreatureControls>().0.insert(kind, control).is_none() {
            panic!("creature type {kind} replaced before it was registered");
        }
        self
    }
}

/// EnableBaddieAI `0x80048C2C` as a floor trigger calls it, in the order
/// the trigger's actions ran: a newly triggered creature joins the front of
/// the active list and takes a free slot, or that of the creature farthest
/// from the camera. The trigger runs in Lara's phase; the creature first
/// thinks on the next tick, so taking the slot just after is the same.
fn activate(
    world: Option<Res<WorldGeometry>>,
    controls: Res<CreatureControls>,
    camera: Option<Res<engine::camera::CameraState>>,
    mut activated: MessageReader<engine::triggers::ObjectActivated>,
    mut active: ResMut<ActiveCreatures>,
    mut creatures: Query<(Entity, &WorldObject, &mut Creature, &mut CreatureBody)>,
) {
    let Some(world) = world else { return };
    let camera = camera.map_or([0; 3], |c| {
        let p = (c.position * 1024.0).round();
        [p.x as i32, -p.y as i32, p.z as i32]
    });
    // Triggered this tick, in order; then any active one never seen (an
    // object that starts active, or one activated without a trigger).
    let mut waiting: Vec<Entity> = activated.read().map(|a| a.entity).collect();
    for (entity, object, _, body) in &creatures {
        if object.active && body.status == ItemStatus::Inactive && !waiting.contains(&entity) {
            waiting.push(entity);
        }
    }
    for entity in waiting {
        let Ok((_, object, _, body)) = creatures.get(entity) else { continue };
        if !object.active || body.status != ItemStatus::Inactive || !controls.0.contains_key(&object.kind) {
            continue;
        }
        let used = creatures.iter().filter(|(_, _, c, _)| c.search.is_some()).count();
        if used >= SLOTS {
            // Release the slot whose creature is farthest from the camera.
            let distance = |p: [i32; 3]| {
                let d = [0, 1, 2].map(|i| (p[i] - camera[i]) >> 8);
                d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
            };
            let farthest = creatures
                .iter()
                .filter(|(_, _, c, _)| c.search.is_some())
                .max_by_key(|(_, _, _, b)| distance(b.position))
                .map(|(e, ..)| e);
            if let Some(far) = farthest
                && let Ok((_, _, mut creature, _)) = creatures.get_mut(far)
            {
                creature.search = None;
            }
        }
        if let Ok((_, _, mut creature, mut body)) = creatures.get_mut(entity) {
            body.status = ItemStatus::Active;
            body.touch_bits = 0;
            enable(&world, &mut creature, &mut body);
            active.0.insert(0, entity);
        }
    }
}

/// Creatures of a type with a registered control function animate inside
/// it, as the original's do, and use authored state dispatch.
fn adopt(
    mut commands: Commands,
    controls: Res<CreatureControls>,
    world: Option<Res<WorldGeometry>>,
    added: Query<(Entity, &WorldObject, &CreatureBody), Added<Creature>>,
) {
    for (entity, object, body) in &added {
        if controls.0.contains_key(&object.kind) {
            let _ = &world;
            commands
                .entity(entity)
                .insert((engine::animation::ControllerAnimated, AnimationGoal { state: body.current_state }));
        }
    }
}

/// Whether a creature type runs a ported control function, rather than the
/// provisional rule in `crate::interactions`.
pub fn is_ported(controls: &CreatureControls, kind: u16) -> bool {
    controls.0.contains_key(&kind)
}

/// The control phase for creatures: each active one, in active-list order,
/// runs its type's registered control function, then its `Transform`
/// follows its pose.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn control_creatures(
    world: Option<Res<WorldGeometry>>,
    boxes: Res<BoxFlags>,
    controls: Res<CreatureControls>,
    mut active: ResMut<ActiveCreatures>,
    mut random: ResMut<GameRandom>,
    mut room_order: ResMut<RoomOrder>,
    mut players: Query<(&mut Player, Option<&Motion>)>,
    mut creatures: Query<
        (
            &mut WorldObject,
            &mut Creature,
            &mut CreatureBody,
            &mut Enemy,
            &mut ObjectAnimation,
            &mut AnimationGoal,
            &mut Transform,
        ),
        Without<Player>,
    >,
    mut writer: MessageWriter<ObjectAnimationCue>,
) {
    let Some(world) = world else { return };
    let Ok((mut player, motion)) = players.single_mut() else { return };
    let mut enemy = EnemyView::of(&player, motion);
    room_order.after_placements(world.data.placements.len());
    let mut obstacles: Vec<Obstacle> = active
        .0
        .iter()
        .filter_map(|&entity| {
            let (_, _, body, ..) = creatures.get(entity).ok()?;
            Some(Obstacle {
                entity,
                room: body.room,
                position: body.position,
                moving: body.status == ItemStatus::Active && body.speed != 0,
                entry: body.room_entry,
            })
        })
        .collect();
    for entity in active.0.clone() {
        let Ok((mut object, mut creature, mut body, mut hit_points, mut animation, mut goal, mut transform)) =
            creatures.get_mut(entity)
        else {
            continue;
        };
        let Some(&control) = controls.0.get(&object.kind) else { continue };
        if !object.active || body.status != ItemStatus::Active {
            continue;
        }
        let mut cues = Vec::new();
        let mut context = CreatureContext {
            world: &world,
            boxes: &boxes,
            random: &mut random,
            player: &mut player,
            enemy: &mut enemy,
            entity,
            creature: &mut creature,
            body: &mut body,
            health: &mut hit_points.health,
            animation: &mut animation,
            goal: &mut goal,
            obstacles: &obstacles,
            room_order: &mut room_order,
            cues: &mut cues,
        };
        control(&mut context);
        sync_transform(&body, &mut transform);
        if body.removed {
            object.consumed = true;
        }
        if let Some(o) = obstacles.iter_mut().find(|o| o.entity == entity) {
            o.room = body.room;
            o.position = body.position;
            o.moving = body.status == ItemStatus::Active && body.speed != 0;
            o.entry = body.room_entry;
        }
        emit_cues(entity, cues, &mut writer);
    }
    // Killed creatures leave the active list (RemoveActiveItem).
    active.0.retain(|&entity| creatures.get(entity).is_ok_and(|(_, _, body, ..)| body.status == ItemStatus::Active));
}

/// The idle-time search (`0x800492B4`): while the original waits for the
/// next field it expands every slot's search one box at a time, round robin,
/// until the field arrives, so how far a search gets depends on how long the
/// frame took to draw (50 to 185 rounds measured in Wall's corridor). Most
/// searches finish within one wait and no fixed budget matches every
/// recording, so the port completes them; a whole-level search can take the
/// original two ticks (`docs/creature-evidence.md`).
fn finish_searches(world: Option<Res<WorldGeometry>>, boxes: Res<BoxFlags>, mut creatures: Query<&mut Creature>) {
    let Some(world) = world else { return };
    for mut creature in &mut creatures {
        if let Some(search) = creature.search.as_mut() {
            while search.update(&world.data, &boxes, false, 1) {}
        }
    }
}

/// CreatureCollision `0x8001C598`, from Lara's collision pass: a solid,
/// active creature's meshes that touch her become its contacts for its next
/// control (the tiger's bite reads them), and she is pushed out of it.
fn collide(
    world: Option<Res<WorldGeometry>>,
    mut players: Query<(
        &mut Player,
        &engine::animation::AnimationState,
        &mut crate::player::CollisionFrame,
        Option<&engine::animation::PoseOverrides>,
    )>,
    mut creatures: Query<(&WorldObject, &Creature, &mut CreatureBody, &Transform, &ObjectAnimation), Without<Player>>,
) {
    let (Some(world), Ok((mut player, pose, mut collision, aim))) = (world, players.single_mut()) else { return };
    let aim = aim.map_or(&[][..], |a| a.0.as_slice());
    let Some(mut pass) = crate::contact::ContactPass::begin(&world, &player, pose, &collision, aim) else { return };
    for (object, creature, mut body, transform, animation) in &mut creatures {
        if !body.collidable || body.status != ItemStatus::Active || object.consumed {
            continue;
        }
        let overrides = creature.joint_turns();
        let mask = pass.touch(&world, &mut player, pose, &mut collision, object.kind, transform, animation, &overrides);
        if mask != 0 {
            body.touch_bits = mask;
        }
    }
}

/// Send a controller's animation cues to the sound and effect systems.
pub fn emit_cues(entity: Entity, cues: Vec<AnimationCue>, writer: &mut MessageWriter<ObjectAnimationCue>) {
    for cue in cues {
        writer.write(ObjectAnimationCue { entity, cue });
    }
}

/// The shared creature systems; each type's controller adds its own.
pub struct CreatureRulesPlugin;

impl Plugin for CreatureRulesPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, engine::EngineRulesPlugin);
        app.init_resource::<GameRandom>()
            .init_resource::<CreatureControls>()
            .init_resource::<ActiveCreatures>()
            .init_resource::<RoomOrder>();
        app.register_type::<Creature>().register_type::<CreatureBody>().register_type::<ActiveCreatures>().add_systems(
            FixedUpdate,
            (
                adopt.in_set(LifecycleSystems::Initialize),
                (activate, control_creatures).chain().in_set(ItemSystems::Control),
                activate.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                collide.in_set(InteractionSystems::Resolve),
                finish_searches.in_set(WorldSystems::Effects),
            )
                .run_if(simulation_enabled),
        );
    }
}

/// The creature components for a placed object of a creature type.
pub fn spawn_components(world: &WorldGeometry, object: &WorldObject, setup: CreatureSetup) -> (Creature, CreatureBody) {
    let placement = world.data.placements.get(object.index);
    let position = placement.map_or([0; 3], |p| p.position);
    let yaw = placement.map_or(0, |p| p.rotation as i16);
    let state = world
        .data
        .model(object.kind)
        .and_then(|m| world.data.animations.get(m.animations[0]))
        .map_or(0, |clip| clip.state);
    let body = CreatureBody {
        position,
        yaw,
        room: object.room,
        room_entry: object.index as u64,
        box_number: NO_BOX,
        current_state: state,
        collidable: true,
        ..default()
    };
    (Creature::new(setup), body)
}

/// Lara's health is shared with [`Enemy`] bookkeeping elsewhere; this reads
/// a creature's hit points the way its controller sees them.
pub fn hit_points(enemy: &Enemy) -> i32 {
    enemy.health
}
