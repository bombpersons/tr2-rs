//! Portable game saves, separate from PS1 emulator checkpoints or memory cards.
use crate::{
    interactions::{Enemy, Inventory, WorldObject},
    player::{AnimationState, Motion, Player, PlayerMode},
    presentation::{LevelRequest, Session},
};
use bevy::prelude::*;
use engine::world::WorldGeometry;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SaveGame {
    version: u32,
    pub level: String,
    position: [f32; 3],
    yaw: f32,
    room: usize,
    health: i32,
    inventory: serde_json::Value,
    objects: Vec<SavedObject>,
}
#[derive(Serialize, Deserialize)]
struct SavedObject {
    index: usize,
    active: bool,
    consumed: bool,
    flags: u16,
    #[serde(default)]
    timer: i16,
    position: [f32; 3],
    rotation: [f32; 4],
    health: Option<i32>,
}
#[derive(Resource)]
pub struct PendingRestore(pub SaveGame);

pub fn write(
    level: &str,
    player: &Player,
    inventory: &Inventory,
    objects: &Query<(&WorldObject, &Transform, Option<&Enemy>)>,
) -> Result<(), String> {
    let objects = objects
        .iter()
        .map(|(o, t, e)| SavedObject {
            index: o.index,
            active: o.active,
            consumed: o.consumed,
            flags: o.flags,
            timer: o.timer,
            position: t.translation.to_array(),
            rotation: t.rotation.to_array(),
            health: e.map(|e| e.health),
        })
        .collect();
    let data = SaveGame {
        version: 1,
        level: level.into(),
        position: player.position.to_array(),
        yaw: player.yaw,
        room: player.room,
        health: player.health,
        inventory: serde_json::to_value(inventory).map_err(|e| e.to_string())?,
        objects,
    };
    let bytes = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write("savegame.json.tmp", bytes).map_err(|e| e.to_string())?;
    std::fs::rename("savegame.json.tmp", "savegame.json").map_err(|e| e.to_string())
}
pub fn read() -> Result<SaveGame, String> {
    let data: SaveGame = serde_json::from_slice(&std::fs::read("savegame.json").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if data.version != 1
        || !data.position.iter().all(|x| x.is_finite())
        || !data.yaw.is_finite()
        || !(0..=1000).contains(&data.health)
    {
        return Err("Unsupported or invalid save data".into());
    }
    if !crate::presentation::LEVELS.iter().any(|(name, _)| *name == data.level) {
        return Err("Unknown saved level".into());
    }
    Ok(data)
}

pub(crate) fn restore(
    mut commands: Commands,
    pending: Option<Res<PendingRestore>>,
    world: Option<Res<WorldGeometry>>,
    request: Res<LevelRequest>,
    mut player: Query<(&mut Player, &mut Motion, &mut AnimationState, &mut Transform)>,
    mut objects: Query<(Entity, &mut WorldObject, &mut Transform, Option<&mut Enemy>), Without<Player>>,
    mut inventory: ResMut<Inventory>,
    mut session: ResMut<Session>,
) {
    let (Some(pending), Some(world)) = (pending, world) else {
        return;
    };
    let data = &pending.0;
    if !world.data.name.eq_ignore_ascii_case(&data.level) || request.name != data.level {
        return;
    }
    if data.room >= world.data.rooms.len() {
        session.message = "Invalid saved room".into();
        commands.remove_resource::<PendingRestore>();
        return;
    }
    let Ok((mut p, mut motion, mut animation, mut t)) = player.single_mut() else {
        return;
    };
    p.position = Vec3::from_array(data.position);
    p.yaw = data.yaw;
    p.room = data.room;
    p.health = data.health;
    p.mode = if p.health == 0 { PlayerMode::Dead } else { PlayerMode::Idle };
    *motion = Motion::default();
    motion.grounded = world.floor_at(p.position, p.room).is_some_and(|hit| (hit.height - p.position.y).abs() < 0.05);
    if !motion.grounded && p.health > 0 {
        p.mode = PlayerMode::Fall;
    }
    *animation = AnimationState::default();
    t.translation = p.position;
    t.rotation = Quat::from_rotation_y(p.yaw);
    if let Ok(saved_inventory) = serde_json::from_value(data.inventory.clone()) {
        *inventory = saved_inventory;
    }
    for (entity, mut object, mut t, enemy) in &mut objects {
        let Some(saved) = data.objects.iter().find(|s| s.index == object.index) else {
            continue;
        };
        object.active = saved.active;
        object.consumed = saved.consumed;
        object.flags = saved.flags;
        object.timer = saved.timer;
        t.translation = Vec3::from_array(saved.position);
        t.rotation = Quat::from_array(saved.rotation).normalize();
        if let Some(health) = saved.health {
            if let Some(mut enemy) = enemy {
                enemy.health = health;
                enemy.killed = health == 0;
            } else if let Some(mut enemy) = Enemy::for_kind(object.kind) {
                enemy.health = health;
                enemy.killed = health == 0;
                commands.entity(entity).insert(enemy);
            }
        }
    }
    session.paused = false;
    session.message = "Game loaded".into();
    commands.remove_resource::<PendingRestore>();
}
