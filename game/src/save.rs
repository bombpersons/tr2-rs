//! The game's save: the port's memory card.
//!
//! The original keeps one save (`0x800DE250`, 0x1CD0 bytes) and writes it
//! whole to the card: for each level Lara has reached, what she carried as
//! she started it (0x2C bytes a level), and, once saved in play, where she
//! is (`0x800DE684` its level, `0x800DE69C` bit 0 set). The passport's load
//! list (`0x8006E60C`) offers each level reached, to play again from its
//! start, and "Current Position". [`SaveGame`] is that save, kept as the
//! game goes and written to `savegame.json` in the player's config folder
//! when saved; there is no card, so its screens are not ported. Evidence:
//! `docs/save-evidence.md`.
use std::{collections::BTreeMap, path::PathBuf};

use bevy::prelude::*;
use engine::{settings, world::WorldGeometry};
use serde::{Deserialize, Serialize};

use crate::{
    interactions::{Enemy, WorldObject},
    inventory::Inventory,
    player::{AnimationState, Motion, Player, PlayerMode},
};

const VERSION: u32 = 2;
/// Saves are larger than settings files; larger still are refused.
const MAX_BYTES: u64 = 4 << 20;

/// The game so far: each level reached by number, with what Lara carried
/// at its start, and the position saved in play.
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SaveGame {
    version: u32,
    pub levels: BTreeMap<usize, LevelStart>,
    pub position: Option<Position>,
}

/// A level as Lara started it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LevelStart {
    pub inventory: Inventory,
}

/// Where Lara was when the game was saved in play, and the level's objects.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Position {
    pub level: usize,
    position: [f32; 3],
    yaw: f32,
    room: usize,
    health: i32,
    inventory: Inventory,
    objects: Vec<SavedObject>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

/// One line of the load list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadChoice {
    /// A level again from its start.
    Level(usize),
    /// The position saved in play.
    Position,
}

impl SaveGame {
    /// Notes a level started with `inventory`.
    pub fn reach(&mut self, level: usize, inventory: &Inventory) {
        self.levels.insert(level, LevelStart { inventory: inventory.clone() });
    }

    /// The load list (`0x8006E60C`): each level reached after Lara's Home,
    /// in order, with "Current Position" after the level it was saved in,
    /// which is the line chosen at first. `levels` is the game's count of
    /// levels. Empty when nothing is saved.
    pub fn load_list(&self, levels: usize) -> (Vec<LoadChoice>, usize) {
        let mut list = vec![];
        let mut selected = 0;
        for level in 1..levels {
            if self.levels.contains_key(&level) {
                list.push(LoadChoice::Level(level));
            }
            if self.position.as_ref().is_some_and(|p| p.level == level) {
                selected = list.len();
                list.push(LoadChoice::Position);
            }
        }
        (list, selected)
    }

    /// Where Lara is now, with the level's objects.
    pub fn capture<'a>(
        level: usize,
        player: &Player,
        inventory: &Inventory,
        objects: impl IntoIterator<Item = (&'a WorldObject, &'a Transform, Option<&'a Enemy>)>,
    ) -> Position {
        let objects = objects
            .into_iter()
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
        Position {
            level,
            position: player.position.to_array(),
            yaw: player.yaw,
            room: player.room,
            health: player.health,
            inventory: inventory.clone(),
            objects,
        }
    }

    /// Reads a save; `None` when there is none.
    pub fn read(path: &std::path::Path) -> Result<Option<Self>, String> {
        use std::io::Read;
        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = vec![];
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("save too large".into());
        }
        let save: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if save.version != VERSION {
            return Err(format!("save version {}, not {VERSION}", save.version));
        }
        if let Some(p) = &save.position
            && (!p.position.iter().all(|x| x.is_finite()) || !p.yaw.is_finite() || !(0..=1000).contains(&p.health))
        {
            return Err("invalid saved position".into());
        }
        Ok(Some(save))
    }

    /// Writes the save, replacing the last.
    pub fn write(&self, path: &std::path::Path) -> Result<(), String> {
        let save = Self { version: VERSION, ..self.clone() };
        let bytes = serde_json::to_vec(&save).map_err(|e| e.to_string())?;
        settings::write(path, &bytes)
    }
}

/// Where the save is kept: `savegame.json` in the player's config folder,
/// or nowhere (tests).
#[derive(Resource, Clone, Debug)]
pub struct SavePath(pub Option<PathBuf>);

impl Default for SavePath {
    fn default() -> Self {
        Self(settings::config_path(env!("CARGO_PKG_NAME"), "savegame.json"))
    }
}

impl SavePath {
    /// The save on disk, or none (missing, unreadable or no path).
    pub fn load(&self) -> Option<SaveGame> {
        let path = self.0.as_deref()?;
        SaveGame::read(path).unwrap_or_else(|e| {
            warn!("ignoring the save at {}: {e}", path.display());
            None
        })
    }
}

/// A saved position to put back once its level has loaded.
#[derive(Resource)]
pub struct PendingRestore(pub Position);

/// Puts Lara and the level's objects back where they were saved, on the
/// first update with the saved level loaded.
pub(crate) fn restore(
    mut commands: Commands,
    pending: Option<Res<PendingRestore>>,
    world: Option<Res<WorldGeometry>>,
    strings: Res<crate::inventory::GameStrings>,
    mut player: Query<(&mut Player, &mut Motion, &mut AnimationState, &mut Transform)>,
    mut objects: Query<(Entity, &mut WorldObject, &mut Transform, Option<&mut Enemy>), Without<Player>>,
    mut inventory: ResMut<Inventory>,
) {
    let (Some(pending), Some(world)) = (pending, world) else {
        return;
    };
    let data = &pending.0;
    if strings.level_number(&world.data.name) != Some(data.level) {
        return;
    }
    let Ok((mut p, mut motion, mut animation, mut t)) = player.single_mut() else {
        return;
    };
    commands.remove_resource::<PendingRestore>();
    if data.room >= world.data.rooms.len() {
        warn!("saved room {} is not in the level", data.room);
        return;
    }
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
    *inventory = data.inventory.clone();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// After a new game saved in the Great Wall, the list holds the Wall
    /// and "Current Position" after it, chosen (`validation/save/wall_load`:
    /// 0x80089D10 holds 1 then -1000).
    #[test]
    fn the_load_list_follows_the_original() {
        let mut save = SaveGame::default();
        assert_eq!(save.load_list(19), (vec![], 0));
        save.reach(1, &Inventory::default());
        save.position = Some(SaveGame::capture(1, &Player::default(), &Inventory::default(), []));
        assert_eq!(save.load_list(19), (vec![LoadChoice::Level(1), LoadChoice::Position], 1));
        // Lara's Home is never offered.
        save.reach(0, &Inventory::default());
        save.reach(3, &Inventory::default());
        assert_eq!(save.load_list(19).0, [LoadChoice::Level(1), LoadChoice::Position, LoadChoice::Level(3)]);
    }

    #[test]
    fn a_save_reads_back_as_written() {
        let dir = std::env::temp_dir().join(format!("tr2-save-test-{}", std::process::id()));
        let path = dir.join("savegame.json");
        let mut save = SaveGame::default();
        save.reach(1, &Inventory::default());
        save.position = Some(SaveGame::capture(1, &Player::default(), &Inventory::default(), []));
        save.write(&path).unwrap();
        let read = SaveGame::read(&path).unwrap().unwrap();
        assert_eq!(read.levels, save.levels);
        assert_eq!(read.position, save.position);
        std::fs::remove_dir_all(dir).ok();
    }
}
