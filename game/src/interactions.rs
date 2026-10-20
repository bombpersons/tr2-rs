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
use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
pub use engine::objects::{CODE_BITS, WorldObject};
use engine::{
    animation::{AnimationGoal, ObjectAnimation, select_state},
    controller::{Button, ControllerState},
    objects::{AppObjectTypes, ObjectType},
    schedule::{InteractionSystems, ItemSystems, LifecycleSystems, simulation_enabled},
    triggers::{AppTriggerActions, TriggerAction, TriggerState, TriggerSystems},
    world::WorldGeometry,
};

use crate::player::{Motion, PLAYER_HEIGHT, PLAYER_RADIUS, Player, PlayerMode, WeaponState};
use crate::presentation::{LEVELS, LevelRequest, Session};

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
    spikes: Option<Spikes>,
    #[serde(default)]
    block: Option<MovableBlock>,
    #[serde(default)]
    creature: Option<CreatureData>,
    #[serde(default)]
    attendant: Option<crate::winston::WinstonTuning>,
    #[serde(default)]
    swimming: Option<crate::player::SwimmingTuning>,
    #[serde(default)]
    pistols: Option<crate::player::GunTuning>,
}

#[derive(serde::Deserialize)]
struct ObjectTypeTable {
    types: Vec<ObjectTypeData>,
}

/// Collected by walking over it and holding Action.
#[derive(Component, serde::Deserialize, Debug, Clone)]
pub struct Pickup {
    pub message: String,
    /// Secret number (0..3), for the three secrets of each level.
    #[serde(default)]
    pub secret: Option<u8>,
}

/// Used with Action from in front of it; toggles its object.
#[derive(Component, serde::Deserialize, Debug, Clone, Default)]
pub struct Switch {
    /// Reached from a larger box around it, while swimming.
    #[serde(default)]
    pub underwater: bool,
}

/// Consumes the inventory item `key` when used.
#[derive(Component, serde::Deserialize, Debug, Clone)]
pub struct Keyhole {
    pub key: u16,
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

/// Kills on a fall, hurts while running over it.
#[derive(Component, serde::Deserialize, Debug, Clone, Default)]
pub struct Spikes {}

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
    /// The joint whose bone flags turn it by the head angle.
    #[serde(default)]
    head_joint: Option<usize>,
}

impl CreatureData {
    fn setup(self) -> crate::creature::CreatureSetup {
        crate::creature::CreatureSetup {
            pivot: self.pivot,
            radius: self.radius as i32,
            step: self.step.unwrap_or(256),
            drop: self.drop.unwrap_or(-512),
            head_joint: self.head_joint,
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

/// The local attendant thresholds live with the existing per-type tuning data.
pub(crate) fn winston_tuning() -> crate::winston::WinstonTuning {
    object_types().into_iter().find_map(|entry| entry.attendant).expect("attendant tuning is registered")
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
        if data.id == crate::player::LARA {
            object_type = object_type.on_spawn(crate::player::spawn_lara);
        }
        let hook = data.clone();
        object_type = object_type.on_init(move |entity, object, world| {
            if hook.id == crate::winston::KIND {
                entity.insert((crate::winston::Winston::default(), AnimationGoal { state: 2 }));
            }
            if let Some(pickup) = &hook.pickup {
                entity.insert(pickup.clone());
            }
            if let Some(switch) = &hook.switch {
                entity.insert(switch.clone());
            }
            if let Some(keyhole) = &hook.keyhole {
                entity.insert(keyhole.clone());
            }
            if let Some(spikes) = &hook.spikes {
                entity.insert(spikes.clone());
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
                    entity.insert(DoorBoxes(door_boxes(world, placement)));
                }
                let state = world
                    .and_then(|w| w.data.model(hook.id).and_then(|m| w.data.animations.get(m.animations[0])))
                    .map_or(0, |clip| clip.state);
                entity.insert((door.clone(), AnimationGoal { state }));
                if let Some((min, max)) = world.and_then(|w| w.model_bounds(hook.id)) {
                    entity.insert(ObjectCollider { min, max });
                }
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

/// Counts of original pickup objects; ammo is stored as pickup packs, not pellets.
#[derive(Resource, Reflect, Default, Debug, serde::Serialize, serde::Deserialize)]
#[reflect(Resource)]
pub struct Inventory {
    pub items: BTreeMap<u16, u32>,
    pub small_medipacks: u32,
    pub large_medipacks: u32,
    pub secret_mask: u8,
}

impl Inventory {
    pub fn add(&mut self, kind: u16) {
        *self.items.entry(kind).or_default() += 1;
        match kind {
            149 => self.small_medipacks += 1,
            150 => self.large_medipacks += 1,
            190..=192 => self.secret_mask |= 1 << (kind - 190),
            _ => {}
        }
    }
    pub fn consume(&mut self, kind: u16) -> bool {
        let Some(n) = self.items.get_mut(&kind) else {
            return false;
        };
        if *n == 0 {
            return false;
        }
        *n -= 1;
        true
    }
    pub fn use_medipack(&mut self, large: bool, player: &mut Player) -> bool {
        if player.health <= 0 || player.health >= 1000 {
            return false;
        }
        let kind = if large { 150 } else { 149 };
        if !self.consume(kind) {
            return false;
        }
        if large {
            self.large_medipacks -= 1;
        } else {
            self.small_medipacks -= 1;
        }
        player.health = (player.health + if large { 1000 } else { 500 }).min(1000);
        true
    }
}

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
        register_object_types(app);
        app.register_trigger_action("end_level").register_trigger_action("music").register_trigger_action("secret");
        app.add_message::<crate::player::GunHit>();
        app.init_resource::<Inventory>()
            .init_resource::<InteractionProgress>()
            .init_resource::<InteractionRuntime>()
            .add_systems(
                FixedUpdate,
                (
                    reset_level.in_set(LifecycleSystems::Reset),
                    block_level_boxes.in_set(LifecycleSystems::Initialize),
                    collect_and_use.in_set(InteractionSystems::Use),
                    trigger_actions.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                    resolve_objects.in_set(InteractionSystems::Resolve),
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
        // Keys/puzzles/secrets belong to a level; weapons, ammo and healing carry.
        inv.items.retain(|&kind, _| !(174..=177).contains(&kind) && !(190..=206).contains(&kind));
        inv.secret_mask = 0;
    }
}

#[allow(clippy::type_complexity)]
fn collect_and_use(
    pad: Res<ControllerState>,
    mut inv: ResMut<Inventory>,
    mut progress: ResMut<InteractionProgress>,
    mut triggers: ResMut<TriggerState>,
    players: Query<(&Player, &WeaponState)>,
    mut objects: Query<(&mut WorldObject, &Transform, Option<&Pickup>, Option<&Switch>, Option<&Keyhole>)>,
) {
    let Ok((p, weapon)) = players.single() else {
        return;
    };
    if p.health <= 0 || !weapon.hands_free() || !pad.held(Button::Cross) {
        return;
    }
    for (mut o, t, pickup, switch, keyhole) in &mut objects {
        if o.consumed || o.room != p.room {
            continue;
        }
        let local = t.rotation.inverse() * (p.position - t.translation) * 1024.0;
        if let Some(pickup) = pickup {
            let underwater = p.mode == PlayerMode::Swimming;
            let in_range = if underwater {
                local.abs().max_element() <= 512.0
            } else {
                local.x.abs() <= 256.0 && local.y.abs() <= 100.0 && (-256.0..=100.0).contains(&local.z)
            };
            if !in_range {
                continue;
            }
            o.consumed = true;
            triggers.mark_collected(o.index);
            inv.add(o.kind);
            progress.pickups += 1;
            if pickup.secret.is_some() {
                progress.secret_mask |= u32::from(inv.secret_mask);
                progress.secrets = progress.secret_mask.count_ones();
            }
            progress.message = pickup.message.clone();
        } else if pad.pressed(Button::Cross) && (switch.is_some() || keyhole.is_some()) {
            let underwater = switch.is_some_and(|s| s.underwater);
            let near = if underwater {
                local.x.abs() <= 1024.0 && local.y.abs() <= 1024.0 && (-1024.0..=512.0).contains(&local.z)
            } else {
                local.x.abs() <= 200.0 && local.y.abs() <= 100.0 && (250.0..=512.0).contains(&local.z)
            };
            if !near {
                continue;
            }
            if switch.is_some() {
                triggers.mark_used(o.index);
                o.active = !o.active;
            } else if let Some(keyhole) = keyhole {
                if inv.consume(keyhole.key) {
                    triggers.mark_used(o.index);
                    o.active = true;
                    progress.message = "Lock opened".into();
                } else {
                    progress.message = "A matching item is required".into();
                }
            }
        }
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
    mut enemies: Query<(&mut Enemy, &mut WorldObject, Option<&mut crate::creature::CreatureBody>)>,
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
    time: Res<Time>,
    objects: Query<(&WorldObject, &Transform, Option<&ObjectCollider>, Has<Spikes>)>,
    mut players: Query<(&mut Player, &Motion)>,
) {
    for (mut p, motion) in &mut players {
        for (o, t, collider, spikes) in &objects {
            if o.consumed {
                continue;
            }
            if spikes {
                let d = p.position - t.translation;
                if d.with_y(0.0).length() < 0.5 && d.y.abs() < 0.3 {
                    if !motion.grounded && motion.velocity.y < 0.0 {
                        p.damage(1000);
                    } else if motion.velocity.with_y(0.0).length() * 1024.0 / 30.0 >= 30.0 {
                        p.damage((15.0 * 30.0 * time.delta_secs()).ceil() as i32);
                    }
                }
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
// inactive, it shuts them again unless fully open (0x8004C9FC..0x8004CA80).
// Only the navigation boxes are ported here; the sectors still read as
// open floor, and Lara is kept out by the door's collider instead.
fn control_doors(
    world: Option<Res<WorldGeometry>>,
    mut flags: ResMut<engine::world::BoxFlags>,
    mut doors: Query<(&WorldObject, &ObjectAnimation, &mut AnimationGoal, Option<&DoorBoxes>), With<Door>>,
) {
    let Some(world) = world else { return };
    for (object, animation, mut goal, boxes) in &mut doors {
        let Some(clip) = world.data.animations.get(animation.animation) else { continue };
        let shut = match (object.active, clip.state) {
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
        if let (Some(shut), Some(boxes)) = (shut, boxes) {
            for &b in &boxes.0 {
                flags.set_blocked(b, shut);
            }
        }
    }
}

fn sync_session(progress: Res<InteractionProgress>, inv: Res<Inventory>, mut session: Option<ResMut<Session>>) {
    if let Some(session) = session.as_deref_mut() {
        session.pickups = progress.pickups;
        session.kills = progress.kills;
        session.secrets = progress.secrets;
        if !progress.message.is_empty() {
            session.message = progress.message.clone();
        }
        session.medipacks = inv.small_medipacks + inv.large_medipacks;
    }
}

/// The level's object type ids that are registered, for tests and tools.
pub fn registered_types() -> BTreeSet<u16> {
    object_types().into_iter().map(|t| t.id).collect()
}
