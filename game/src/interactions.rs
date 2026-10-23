//! Tomb Raider II's objects and trigger actions, independent of rendering.
//!
//! Each object type id is registered with the engine from
//! `data/object_types.json`, which says what the type is (pickup, switch,
//! keyhole, door, trap, creature) and holds its tuning values. The engine
//! runs the floor triggers; this module adds the game's trigger actions
//! (`end_level`, `music`, `secret`).
//!
//! IDs: TR2Main global/types.h GAME_OBJECT_ID. Floor commands and interaction
//! bounds: OpenLara format.h. Tiger health/radius/damage/state IDs: enemy.h.
//! Navigation and attack hitboxes remain a bounded approximation: no nav-box
//! path search, mesh-sphere attack masks, bosses, vehicles, or humanoid gun AI.
use std::collections::BTreeSet;

use bevy::prelude::*;
pub use engine::objects::{CODE_BITS, WorldObject};
use engine::{
    animation::{AnimationGoal, ObjectAnimation, select_state},
    objects::{AppObjectTypes, ObjectType},
    schedule::{InteractionSystems, ItemSystems, LifecycleSystems, simulation_enabled},
    triggers::{AppTriggerActions, TriggerAction, TriggerSystems},
    world::WorldGeometry,
};

use crate::player::{PLAYER_HEIGHT, PLAYER_RADIUS, Player};

use crate::presentation::{LEVELS, LevelRequest, Session};

mod usable;
pub use usable::{KEYHOLE_BOUNDS, PICKUP_BOUNDS, SWITCH_BOUNDS, UsableItem, UseBounds, align, in_bounds};

/// The behaviours an object type can have, as `data/object_types.json`
/// lists them.
#[derive(serde::Deserialize, Debug, Clone)]
struct ObjectTypeData {
    id: u16,
    name: String,
    #[serde(default)]
    pickup: Option<Pickup>,
    #[serde(default)]
    switch: Option<Switch>,
    #[serde(default)]
    keyhole: Option<Keyhole>,
    #[serde(default)]
    door: Option<Door>,
    #[serde(default)]
    block: Option<MovableBlock>,
    #[serde(default)]
    creature: Option<CreatureData>,
    #[serde(default)]
    swimming: Option<crate::player::SwimmingTuning>,
    #[serde(default)]
    pistols: Option<crate::player::GunTuning>,
    /// Another type whose clips this one plays (its setup shares them).
    #[serde(default)]
    animations_from: Option<u16>,
}

#[derive(serde::Deserialize)]
struct ObjectTypeTable {
    types: Vec<ObjectTypeData>,
}

/// Picked up by standing at it and holding Action ([`usable`]).
#[derive(Component, serde::Deserialize, Debug, Clone)]
pub struct Pickup {
    pub message: String,
    /// Secret number (0..3), for the three secrets of each level.
    #[serde(default)]
    pub secret: Option<u8>,
}

/// Used with Action from in front of it ([`usable`]); its trigger toggles
/// the objects it names once its own clip has played.
#[derive(Component, serde::Deserialize, Debug, Clone)]
pub struct Switch {
    /// Reached from a larger box around it, while swimming.
    #[serde(default)]
    pub underwater: bool,
    /// Lara is moved to stand this far in front of it (units); without it
    /// she stays where she pressed Action.
    #[serde(default)]
    pub align: Option<i32>,
    /// Lara's clip for a switch in state 1, then in state 0.
    #[serde(default = "default_switch_clips")]
    pub clips: [usize; 2],
    /// Only usable in state 0 (type 92).
    #[serde(default)]
    pub one_way: bool,
}

fn default_switch_clips() -> [usize; 2] {
    [crate::player::contract::clip::SWITCH_OFF, crate::player::contract::clip::SWITCH_ON]
}

/// Takes the inventory item the placed object `key` becomes, chosen on the
/// keys ring ([`usable`]).
#[derive(Component, serde::Deserialize, Debug, Clone)]
pub struct Keyhole {
    pub key: u16,
    /// A puzzle hole: Lara places the piece (clip 134) and the hole turns
    /// into its filled type.
    #[serde(default)]
    pub puzzle: bool,
}

/// Blocks Lara with its model's bounds while closed.
#[derive(Component, serde::Deserialize, Debug, Clone, Default)]
pub struct Door {}

/// A block placed in the level, whose sector it raises and whose navigation
/// box it blocks (pushing it is not ported).
#[derive(Component, serde::Deserialize, Debug, Clone, Default)]
pub struct MovableBlock {}

/// The navigation boxes a door blocks while shut.
#[derive(Component, Debug, Clone, Default)]
pub struct DoorBoxes(pub Vec<usize>);

/// The sectors a door makes solid while shut, as (room, x, z) in world
/// units ([`door_sectors`]).
#[derive(Component, Debug, Clone, Default)]
pub struct DoorSectors(pub Vec<(usize, i32, i32)>);

#[derive(serde::Deserialize, Debug, Clone, Copy)]
struct CreatureData {
    health: i32,
    /// World units.
    radius: f32,
    damage: i32,
    /// Object table +0x28: how far ahead of its origin it measures from.
    #[serde(default)]
    pivot: i32,
    /// The largest rise and drop its path may take (InitialiseSlot).
    #[serde(default)]
    step: Option<i32>,
    #[serde(default)]
    drop: Option<i32>,
    /// A flier's climb a tick (InitialiseSlot); 0 walks.
    #[serde(default)]
    fly: i32,
    /// Box flags that bar its path (InitialiseSlot), when not doors'.
    #[serde(default)]
    block_mask: Option<u16>,
    /// The joint whose bone flags turn it by the head angle.
    #[serde(default)]
    head_joint: Option<usize>,
    /// A second flagged joint, turned by the neck angle.
    #[serde(default)]
    neck_joint: Option<usize>,
}

impl CreatureData {
    fn setup(self) -> crate::creature::CreatureSetup {
        crate::creature::CreatureSetup {
            pivot: self.pivot,
            radius: self.radius as i32,
            step: self.step.unwrap_or(256),
            drop: self.drop.unwrap_or(-512),
            fly: self.fly,
            block_mask: self.block_mask.unwrap_or(engine::world::BLOCKED),
            head_joint: self.head_joint,
            neck_joint: self.neck_joint,
        }
    }
}

#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct Enemy {
    pub health: i32,
    pub radius: f32,
    pub attack_damage: i32,
    pub attack_latched: bool,
    pub attack_frame: f32,
    pub killed: bool,
}

impl Enemy {
    fn new(data: CreatureData) -> Self {
        Self {
            health: data.health,
            radius: data.radius / 1024.0,
            attack_damage: data.damage,
            attack_latched: false,
            attack_frame: 0.0,
            killed: false,
        }
    }

    /// The creature components of object type `kind`, if it is one.
    pub fn for_kind(kind: u16) -> Option<Self> {
        object_types().into_iter().find(|t| t.id == kind)?.creature.map(Self::new)
    }

    /// The shared creature routines' set-up values for type `kind`.
    pub fn setup_for_kind(kind: u16) -> Option<crate::creature::CreatureSetup> {
        object_types().into_iter().find(|t| t.id == kind)?.creature.map(CreatureData::setup)
    }
}

#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct ObjectCollider {
    pub min: Vec3,
    pub max: Vec3,
}

fn object_types() -> Vec<ObjectTypeData> {
    let table: ObjectTypeTable =
        serde_json::from_str(include_str!("../data/object_types.json")).expect("data/object_types.json is valid");
    table.types
}

pub(crate) fn swimming_tuning() -> crate::player::SwimmingTuning {
    object_types().into_iter().find_map(|entry| entry.swimming).expect("swimming tuning is registered")
}

/// Lara's pistols: the gun table entry at 0x80089464.
pub fn pistol_tuning() -> crate::player::GunTuning {
    object_types().into_iter().find_map(|entry| entry.pistols).expect("pistol tuning is registered")
}

/// Register every object type of `data/object_types.json` with the engine.
fn register_object_types(app: &mut App) {
    for data in object_types() {
        let mut object_type = ObjectType::new(data.name.clone());
        if let Some(source) = data.animations_from {
            object_type = object_type.with_animations_from(source);
        }
        if data.id == crate::player::LARA {
            object_type = object_type.on_spawn(crate::player::spawn_lara);
        }
        let hook = data.clone();
        object_type = object_type.on_init(move |entity, object, world| {
            if hook.id == crate::winston::KIND {
                entity.insert(crate::creature::OwnCollision);
            }
            if let Some(pickup) = &hook.pickup {
                entity.insert((pickup.clone(), UsableItem::default()));
            }
            if let Some(switch) = &hook.switch {
                // The switch animates itself in its control and collision
                // (AnimateItem calls), starting in its first clip's state.
                let state = world
                    .and_then(|w| w.data.model(hook.id).and_then(|m| w.data.animations.get(m.animations[0])))
                    .map_or(0, |clip| clip.state);
                entity.insert((
                    switch.clone(),
                    UsableItem { state, ..default() },
                    AnimationGoal { state },
                    engine::animation::ControllerAnimated,
                ));
            }
            if let Some(keyhole) = &hook.keyhole {
                entity.insert((keyhole.clone(), UsableItem::default()));
            }
            if let Some(creature) = hook.creature {
                entity.insert(Enemy::new(creature));
                // The shared creature state; types with a ported controller
                // take it over (crate::creature).
                if let Some(world) = world {
                    entity.insert(crate::creature::spawn_components(world, object, creature.setup()));
                }
            }
            if let Some(block) = &hook.block {
                entity.insert(block.clone());
            }
            if let Some(door) = &hook.door {
                if let Some(world) = world
                    && let Some(placement) = world.data.placements.get(object.index)
                {
                    entity
                        .insert((DoorBoxes(door_boxes(world, placement)), DoorSectors(door_sectors(world, placement))));
                }
                let state = world
                    .and_then(|w| w.data.model(hook.id).and_then(|m| w.data.animations.get(m.animations[0])))
                    .map_or(0, |clip| clip.state);
                entity.insert((door.clone(), AnimationGoal { state }));
            }
        });
        app.register_object_type(data.id, object_type);
    }
}

/// The navigation boxes a door shuts (InitialiseDoor `0x8004C358`, shut by
/// `0x8004C284`): the sector behind it (one step against its facing) and
/// its own sector in the room through that sector's portal, each resolved
/// through a further portal (GetDoor `0x8001FACC`) and in the flipped rooms
/// too. Only blockable boxes count.
pub fn door_boxes(world: &WorldGeometry, placement: &engine::world::Placement) -> Vec<usize> {
    let level = &world.data;
    let [x, _, z] = placement.position;
    let (dx, dz) = match placement.rotation {
        0 => (0, -1),
        0x8000 => (0, 1),
        0x4000 => (-1, 0),
        _ => (1, 0),
    };
    let portal = |room: usize, x: i32, z: i32| {
        world.sector_at(room, x, z).and_then(|s| level.floor_info(s).portal).filter(|&r| r < level.rooms.len())
    };
    // The box of a sector, through its portal when it has one.
    let resolved_box = |room: usize, x: i32, z: i32| {
        let room = portal(room, x, z).unwrap_or(room);
        world.sector_at(room, x, z).and_then(|s| s.nav_box)
    };
    let (bx, bz) = (x + dx * 1024, z + dz * 1024);
    let mut rooms = vec![placement.room];
    rooms.extend(level.rooms.get(placement.room).and_then(|r| r.alternate));
    let mut boxes = vec![];
    for &room in &rooms {
        boxes.extend(resolved_box(room, bx, bz));
        if let Some(other) = portal(room, bx, bz) {
            boxes.extend(resolved_box(other, x, z));
            if let Some(flipped) = level.rooms.get(other).and_then(|r| r.alternate) {
                boxes.extend(resolved_box(flipped, x, z));
            }
        }
    }
    boxes.retain(|&b| level.boxes.get(b).is_some_and(|b| b.overlap as u16 & engine::world::BLOCKABLE != 0));
    boxes.dedup();
    boxes
}

/// The sectors InitialiseDoor `0x8004C358` saves and ShutThatDoor
/// `0x8004C284` makes solid: the sector beside the door, one step against
/// its facing, in its room; the door's own sector in the room through that
/// sector's portal (GetDoor `0x8001FACC`); and both in the flipped rooms.
pub fn door_sectors(world: &WorldGeometry, placement: &engine::world::Placement) -> Vec<(usize, i32, i32)> {
    let level = &world.data;
    let [x, _, z] = placement.position;
    let (dx, dz) = match placement.rotation {
        0 => (0, -1),
        0x8000 => (0, 1),
        0x4000 => (-1, 0),
        _ => (1, 0),
    };
    let (bx, bz) = (x + dx * 1024, z + dz * 1024);
    let alternate = |room: usize| level.rooms.get(room).and_then(|r| r.alternate);
    let mut sectors = vec![(placement.room, bx, bz)];
    sectors.extend(alternate(placement.room).map(|r| (r, bx, bz)));
    if let Some(other) = world
        .sector_at(placement.room, bx, bz)
        .and_then(|s| level.floor_info(s).portal)
        .filter(|&r| r < level.rooms.len())
    {
        sectors.push((other, x, z));
        sectors.extend(alternate(other).map(|r| (r, x, z)));
    }
    sectors
}

/// The box a placed block raises and blocks (InitialiseMovableBlock
/// `0x8004A05C` calls AlterFloorHeight `0x8004AA4C` with -1024).
pub fn block_box(world: &WorldGeometry, placement: &engine::world::Placement) -> Option<usize> {
    let [x, y, z] = placement.position;
    world.floor_box(x, y, z, placement.room).1
}

/// The box flags a level starts with: every door shut, every visible block
/// in place. The original's level setup does the same before any tick.
pub fn initial_box_flags(world: &WorldGeometry) -> engine::world::BoxFlags {
    let mut flags = engine::world::BoxFlags::new(&world.data);
    let types = object_types();
    let kind = |id: u16| types.iter().find(|t| t.id == id);
    for placement in &world.data.placements {
        let Some(data) = kind(placement.kind) else { continue };
        if data.door.is_some() {
            for b in door_boxes(world, placement) {
                flags.set_blocked(b, true);
            }
        }
        if data.block.is_some()
            && !placement.invisible
            && let Some(b) = block_box(world, placement)
        {
            flags.set_blocked(b, true);
        }
    }
    flags
}

/// A newly loaded level's doors and blocks block their boxes.
fn block_level_boxes(world: Option<Res<WorldGeometry>>, mut flags: ResMut<engine::world::BoxFlags>) {
    if let Some(world) = world.filter(|w| w.is_changed()) {
        *flags = initial_box_flags(&world);
    }
}

pub use crate::inventory::Inventory;

#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct InteractionProgress {
    pub pickups: u32,
    pub kills: u32,
    pub secrets: u32,
    pub secret_mask: u32,
    pub level_complete: bool,
    /// Original logical soundtrack ID; audio maps it through TR2_TRACK_MAPPING.
    pub requested_track: Option<u16>,
    pub message: String,
}

#[derive(Resource, Default)]
struct InteractionRuntime {
    level: String,
}

pub struct InteractionsPlugin;

impl Plugin for InteractionsPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, engine::EngineRulesPlugin);
        engine::add_once(app, crate::player::AvatarPlugin);
        engine::add_once(app, crate::winston::WinstonRulesPlugin);
        engine::add_once(app, crate::tiger::TigerRulesPlugin);
        engine::add_once(app, crate::spider::SpiderRulesPlugin);
        engine::add_once(app, crate::crow::CrowRulesPlugin);
        engine::add_once(app, crate::trex::TRexRulesPlugin);
        engine::add_once(app, crate::dog::DogRulesPlugin);
        engine::add_once(app, crate::rat::RatRulesPlugin);
        engine::add_once(app, crate::gunman::GunmanRulesPlugin);
        engine::add_once(app, crate::club_man::ClubManRulesPlugin);
        engine::add_once(app, crate::traps::TrapsPlugin);
        engine::add_once(app, crate::inventory::InventoryPlugin);
        register_object_types(app);
        app.register_trigger_action("end_level").register_trigger_action("music").register_trigger_action("secret");
        app.add_message::<crate::player::GunHit>();
        app.init_resource::<InteractionProgress>().init_resource::<InteractionRuntime>().add_systems(
            FixedUpdate,
            (
                reset_level.in_set(LifecycleSystems::Reset),
                block_level_boxes.in_set(LifecycleSystems::Initialize),
                (usable::collide, usable::use_keys).chain().in_set(InteractionSystems::Use),
                usable::control.in_set(ItemSystems::Control),
                usable::ready_sources.in_set(InteractionSystems::Triggers).before(TriggerSystems::Dispatch),
                usable::sources_fired.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                trigger_actions.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                resolve_objects.in_set(InteractionSystems::Resolve),
                collide_doors.in_set(InteractionSystems::Resolve),
                creatures.in_set(ItemSystems::Control),
                control_doors.in_set(ItemSystems::Control),
                combat.in_set(InteractionSystems::Combat).after(crate::player::PlayerSystems::Weapons),
                sync_session.in_set(InteractionSystems::Report),
            )
                .run_if(simulation_enabled),
        );
    }
}

fn reset_level(
    world: Option<Res<WorldGeometry>>,
    mut state: ResMut<InteractionRuntime>,
    mut progress: ResMut<InteractionProgress>,
    mut inv: ResMut<Inventory>,
) {
    let Some(w) = world else {
        return;
    };
    if state.level != w.data.name {
        *state = InteractionRuntime { level: w.data.name.clone() };
        *progress = default();
        inv.leave_level();
    }
}

/// The game's trigger actions: finish the level, request music, find a secret.
fn trigger_actions(
    mut actions: MessageReader<TriggerAction>,
    world: Option<Res<WorldGeometry>>,
    mut progress: ResMut<InteractionProgress>,
    mut request: Option<ResMut<LevelRequest>>,
) {
    for action in actions.read() {
        match action.name.as_str() {
            "end_level" => {
                progress.level_complete = true;
                progress.message = "Level complete".into();
                if let (Some(request), Some(w)) = (request.as_deref_mut(), world.as_deref())
                    && let Some(i) = LEVELS.iter().position(|v| v.0.eq_ignore_ascii_case(&w.data.name))
                    && i < 17
                {
                    request.name = LEVELS[i + 1].0.into();
                }
            }
            "music" => progress.requested_track = Some(action.argument),
            "secret" => {
                let bit = 1u8.checked_shl(u32::from(action.argument)).unwrap_or(0);
                if bit != 0 {
                    progress.secret_mask |= u32::from(bit);
                    progress.secrets = progress.secret_mask.count_ones();
                }
            }
            _ => {}
        }
    }
}

/// HitTarget 0x8004132C: each gun hit takes its damage from the creature's
/// hit points (they may go below zero); the hit that takes the last ones
/// counts a kill (0x8004133C..0x80041360).
fn combat(
    mut hits: MessageReader<crate::player::GunHit>,
    mut progress: ResMut<InteractionProgress>,
    mut enemies: Query<(&mut Enemy, &mut WorldObject, Option<&mut crate::creature::ItemBody>)>,
) {
    for hit in hits.read() {
        let Ok((mut enemy, mut object, body)) = enemies.get_mut(hit.target) else { continue };
        // HitTarget sets the item's hit flag, which the creature's next
        // mood update reads (status bit 0x10).
        if let Some(mut body) = body {
            body.hit = true;
        }
        if enemy.health > 0 && enemy.health <= hit.damage {
            enemy.killed = true;
            progress.kills += 1;
        }
        enemy.health -= hit.damage;
        object.active = true;
    }
}

fn creatures(
    time: Res<Time>,
    world: Option<Res<WorldGeometry>>,
    ported: Res<crate::creature::CreatureControls>,
    mut players: Query<&mut Player>,
    mut enemies: Query<(&mut Enemy, &mut WorldObject, &mut Transform, Option<&mut ObjectAnimation>), Without<Player>>,
) {
    let Ok(mut p) = players.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    for (mut enemy, mut o, mut t, mut animation) in &mut enemies {
        // Ported creature types run their own controllers (crate::creature).
        if crate::creature::is_ported(&ported, o.kind) {
            continue;
        }
        let Some(w) = world.as_deref() else {
            continue;
        };
        if enemy.health <= 0 {
            if let Some(a) = animation.as_deref_mut() {
                select_state(w, o.kind, 0, a);
            }
            continue;
        }
        if !o.active || p.health <= 0 {
            continue;
        }
        let delta = p.position - t.translation;
        let flat = delta.with_y(0.0);
        let distance = flat.length();
        // Tiger states: death0,stop1,walk2,run3,attacks6/7/8.
        // Spider clips use the same decoded state lookup; navigation is provisional.
        let attack = distance < enemy.radius + PLAYER_RADIUS + 0.2 && delta.y.abs() < 0.5;
        let desired = if attack { 6 } else { 3 };
        if let Some(a) = animation.as_deref_mut() {
            select_state(w, o.kind, desired, a);
        }
        if attack {
            let frame = animation.as_ref().map(|a| a.frame).unwrap_or(0.0);
            if frame < enemy.attack_frame {
                enemy.attack_latched = false;
            }
            if !enemy.attack_latched {
                p.damage(enemy.attack_damage);
                enemy.attack_latched = true;
            }
            enemy.attack_frame = frame;
            continue;
        }
        enemy.attack_latched = false;
        let forward = t.rotation * Vec3::Z;
        let desired = flat.normalize_or_zero();
        let yaw = forward.x.atan2(forward.z);
        let target = desired.x.atan2(desired.z);
        let turn = (target - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let heading = yaw + turn.clamp(-std::f32::consts::PI * dt, std::f32::consts::PI * dt);
        t.rotation = Quat::from_rotation_y(heading);
        let speed = animation
            .as_ref()
            .and_then(|a| w.data.animations.get(a.animation))
            .map(|a| a.speed as f32 / 65536.0)
            .unwrap_or(0.0)
            .max(0.0);
        let proposed = t.translation + t.rotation * Vec3::Z * (speed * 30.0 / 1024.0 * dt);
        let probe = proposed + (t.rotation * Vec3::Z) * enemy.radius;
        if let Some(hit) = w.floor_at(probe, o.room)
            && hit.height - t.translation.y <= 0.25
            && t.translation.y - hit.height <= 1.0
            && hit.ceiling - hit.height >= 0.5
        {
            t.translation = proposed.with_y(hit.height);
            o.room = hit.room;
        }
    }
}

#[allow(clippy::type_complexity)]
fn resolve_objects(
    objects: Query<(&WorldObject, &Transform, Option<&ObjectCollider>)>,
    mut players: Query<&mut Player>,
) {
    for mut p in &mut players {
        for (o, t, collider) in &objects {
            if o.consumed {
                continue;
            }
            let Some(c) = collider else {
                continue;
            };
            if o.active {
                continue;
            }
            let mut local = t.rotation.inverse() * (p.position - t.translation);
            if local.y + PLAYER_HEIGHT < c.min.y || local.y > c.max.y {
                continue;
            }
            let min = c.min - Vec3::new(PLAYER_RADIUS, 0.0, PLAYER_RADIUS);
            let max = c.max + Vec3::new(PLAYER_RADIUS, 0.0, PLAYER_RADIUS);
            if local.x > min.x && local.x < max.x && local.z > min.z && local.z < max.z {
                let sides = [
                    (local.x - min.x, 0, min.x),
                    (max.x - local.x, 0, max.x),
                    (local.z - min.z, 2, min.z),
                    (max.z - local.z, 2, max.z),
                ];
                let &(_, axis, value) = sides.iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
                local[axis] = value;
                p.position = t.translation + t.rotation * local;
            }
        }
    }
}

// PSX DoorControl 0x8004CA04..0x8004CA60 only changes the goal when
// closed+active or open+inactive. Reopening during a close cannot cancel it:
// the close completes before the next open request (docs/door-animation-evidence.md).
// Active, the door opens its sectors once it has left its closed state;
// inactive, it shuts them again unless fully open (0x8004C9FC..0x8004CA80):
// shut sectors read as solid ([`door_sectors`]) and their boxes blocked.
// Venice keeps a door open while Lara is in one area (0x8004C964..0x8004C9C4,
// when type 14 is loaded and 0x80087A48 is 0); that is not ported.
#[allow(clippy::type_complexity)]
fn control_doors(
    world: Option<ResMut<WorldGeometry>>,
    mut flags: ResMut<engine::world::BoxFlags>,
    mut doors: Query<
        (&mut WorldObject, &ObjectAnimation, &mut AnimationGoal, Option<&DoorBoxes>, Option<&DoorSectors>),
        With<Door>,
    >,
) {
    let Some(mut world) = world else { return };
    for (mut object, animation, mut goal, boxes, sectors) in &mut doors {
        let Some(clip) = world.data.animations.get(animation.animation) else { continue };
        // DoorControl asks TriggerActive, which counts a timed trigger's
        // hold down (0x8001F718).
        let shut = match (object.trigger_active(), clip.state) {
            (true, 0) => {
                goal.state = 1;
                None
            }
            (true, _) => Some(false),
            (false, 1) => {
                goal.state = 0;
                None
            }
            (false, _) => Some(true),
        };
        let Some(shut) = shut else { continue };
        for &b in boxes.map_or(&[][..], |b| &b.0) {
            flags.set_blocked(b, shut);
        }
        for &(room, x, z) in sectors.map_or(&[][..], |s| &s.0) {
            if world.is_shut(room, x, z) != shut {
                world.shut_sector(room, x, z, shut);
            }
        }
    }
}

/// DoorCollision `0x8001C70C`: Lara is pushed out of a door whose meshes
/// touch hers, through the shared contact pass, unless her state has turned
/// pushes off (the use states clear the flag, `0x80039950`).
#[allow(clippy::type_complexity)]
fn collide_doors(
    world: Option<Res<WorldGeometry>>,
    mut players: Query<(
        &mut Player,
        &engine::animation::AnimationState,
        &mut crate::player::CollisionFrame,
        Option<&engine::animation::PoseOverrides>,
    )>,
    doors: Query<(&WorldObject, &Transform, &ObjectAnimation), (With<Door>, Without<Player>)>,
) {
    let (Some(world), Ok((mut player, pose, mut collision, aim))) = (world, players.single_mut()) else { return };
    if matches!(player.mode, crate::player::PlayerMode::Using { .. }) {
        return;
    }
    let aim = aim.map_or(&[][..], |a| a.0.as_slice());
    let Some(mut pass) = crate::contact::ContactPass::begin(&world, &player, pose, &collision, aim) else { return };
    for (object, transform, animation) in &doors {
        pass.touch(&world, &mut player, pose, &mut collision, object.kind, transform, animation, &[], true);
    }
}

fn sync_session(progress: Res<InteractionProgress>, mut session: Option<ResMut<Session>>) {
    if let Some(session) = session.as_deref_mut() {
        session.pickups = progress.pickups;
        session.kills = progress.kills;
        session.secrets = progress.secrets;
        if !progress.message.is_empty() {
            session.message = progress.message.clone();
        }
    }
}

/// The level's object type ids that are registered, for tests and tools.
pub fn registered_types() -> BTreeSet<u16> {
    object_types().into_iter().map(|t| t.id).collect()
}
