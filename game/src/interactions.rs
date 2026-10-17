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
use engine::{
    animation::{AnimationGoal, ObjectAnimation, select_state},
    controller::{Button, ControllerState},
    objects::{AppObjectTypes, ObjectType},
    schedule::{InteractionSystems, ItemSystems, LifecycleSystems, simulation_enabled},
    triggers::{AppTriggerActions, TriggerAction, TriggerState, TriggerSystems},
    world::{WorldGeometry, clear_line},
};
pub use engine::objects::{CODE_BITS, WorldObject};

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
    creature: Option<CreatureData>,
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

/// Kills on a fall, hurts while running over it.
#[derive(Component, serde::Deserialize, Debug, Clone, Default)]
pub struct Spikes {}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
struct CreatureData {
    health: i32,
    /// World units.
    radius: f32,
    damage: i32,
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

/// Register every object type of `data/object_types.json` with the engine.
fn register_object_types(app: &mut App) {
    for data in object_types() {
        let mut object_type = ObjectType::new(data.name.clone());
        if data.id == crate::player::LARA {
            object_type = object_type.on_spawn(crate::player::spawn_lara);
        }
        let hook = data.clone();
        object_type = object_type.on_init(move |entity, _, world| {
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
            }
            if let Some(door) = &hook.door {
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
    shots: BTreeMap<Entity, u64>,
}

pub struct InteractionsPlugin;

impl Plugin for InteractionsPlugin {
    fn build(&self, app: &mut App) {
        engine::add_once(app, engine::EngineRulesPlugin);
        engine::add_once(app, crate::player::AvatarPlugin);
        register_object_types(app);
        app.register_trigger_action("end_level").register_trigger_action("music").register_trigger_action("secret");
        app.init_resource::<Inventory>()
            .init_resource::<InteractionProgress>()
            .init_resource::<InteractionRuntime>()
            .add_systems(
                FixedUpdate,
                (
                    reset_level.in_set(LifecycleSystems::Reset),
                    collect_and_use.in_set(InteractionSystems::Use),
                    trigger_actions.in_set(InteractionSystems::Triggers).after(TriggerSystems::Dispatch),
                    resolve_objects.in_set(InteractionSystems::Resolve),
                    creatures.in_set(ItemSystems::Control),
                    control_doors.in_set(ItemSystems::Control),
                    combat.in_set(InteractionSystems::Combat),
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
        *state = InteractionRuntime { level: w.data.name.clone(), ..default() };
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
    if p.health <= 0 || weapon.drawn || !pad.held(Button::Cross) {
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

fn combat(
    world: Option<Res<WorldGeometry>>,
    mut state: ResMut<InteractionRuntime>,
    mut progress: ResMut<InteractionProgress>,
    players: Query<(Entity, &Player, &WeaponState)>,
    mut enemies: Query<(&mut Enemy, &mut WorldObject, &Transform)>,
) {
    for (e, p, weapon) in &players {
        let previous = state.shots.insert(e, weapon.shot_serial).unwrap_or(0);
        let shots = weapon.shot_serial.saturating_sub(previous);
        if shots == 0 || p.health <= 0 {
            continue;
        }
        let eye = p.position + Vec3::Y * 0.65;
        let target = enemies
            .iter()
            .filter(|(enemy, o, t)| {
                enemy.health > 0 && o.active && !o.consumed && {
                    let d = t.translation - p.position;
                    let flat = d.with_y(0.0);
                    flat.length() < 8.0
                        && flat.normalize_or_zero().dot(p.forward()) > 0.85
                        && d.y.abs() < 2.0
                        && clear_line(world.as_deref(), eye, t.translation + Vec3::Y * 0.5, p.room)
                }
            })
            .min_by(|a, b| {
                a.2.translation.distance_squared(p.position).total_cmp(&b.2.translation.distance_squared(p.position))
            })
            .map(|(_, o, _)| o.index);
        if let Some(index) = target {
            for (mut enemy, mut o, _) in &mut enemies {
                if o.index == index {
                    // Each pistol projectile does one hit point in original creature health units.
                    enemy.health = (enemy.health - shots.min(i32::MAX as u64) as i32).max(0);
                    o.active = true;
                    if enemy.health == 0 && !enemy.killed {
                        enemy.killed = true;
                        progress.kills += 1;
                    }
                }
            }
        }
    }
}

fn creatures(
    time: Res<Time>,
    world: Option<Res<WorldGeometry>>,
    mut players: Query<&mut Player>,
    mut enemies: Query<(&mut Enemy, &mut WorldObject, &mut Transform, Option<&mut ObjectAnimation>), Without<Player>>,
) {
    let Ok(mut p) = players.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    for (mut enemy, mut o, mut t, mut animation) in &mut enemies {
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
fn control_doors(
    world: Option<Res<WorldGeometry>>,
    mut doors: Query<(&WorldObject, &ObjectAnimation, &mut AnimationGoal), With<Door>>,
) {
    let Some(world) = world else { return };
    for (object, animation, mut goal) in &mut doors {
        let Some(clip) = world.data.animations.get(animation.animation) else { continue };
        match (object.active, clip.state) {
            (true, 0) => goal.state = 1,
            (false, 1) => goal.state = 0,
            _ => (),
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
