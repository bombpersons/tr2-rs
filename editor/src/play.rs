//! Play from view: run the game on the level as it is in the editor, unsaved
//! edits included, with Lara standing where the centre of the 3D view meets
//! the level and facing the way the view looks.
//!
//! The document is written as `levels/<name>/editor-play.json` under the
//! game's assets (beside the level's own files, which it never touches) and
//! the game starts with `--level <name> --level-file` that copy, so the
//! level keeps its atlas, sky, audio and place in the level order.
use crate::Editor;
use bevy::prelude::*;
use engine::{
    editor::{live_rooms, rooms_containing},
    render::GameCamera,
    world::{Level, Placement, WorldGeometry, position},
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
};

/// The game's object type for Lara (`game::player::LARA`): her placement is
/// where a level starts.
const LARA: u16 = 0;
/// How far along the view to look for somewhere to stand, in level units.
const REACH: f64 = 65536.0;

/// What a Play request does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Play {
    /// Write the play copy and start the game on it.
    Run,
    /// Only write the play copy, under this asset folder (for tests).
    Export(PathBuf),
}

/// Where Lara starts: on a floor, in the room that floor belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub position: [i32; 3],
    pub room: usize,
    pub rotation: u16,
}

pub struct PlayPlugin;
impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, play);
    }
}

/// The game started last, so a new Play replaces it.
#[derive(Default)]
struct Running(Option<Child>);

fn play(
    mut editor: ResMut<Editor>,
    world: Option<Res<WorldGeometry>>,
    cameras: Query<&GlobalTransform, With<GameCamera>>,
    mut running: Local<Running>,
) {
    let Some(request) = editor.play.take() else { return };
    let (Some(world), Ok(camera)) = (world, cameras.single()) else {
        editor.status = "The level is still loading".into();
        return;
    };
    let eye = crate::viewport::file_offset(camera.translation());
    let forward = crate::viewport::file_offset(*camera.forward());
    let Some(spot) = spot(&world, eye, forward) else {
        editor.status = "Play: point the centre of the view at a floor to start Lara there".into();
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let assets = match &request {
        Play::Run => root.join("game/assets"),
        Play::Export(folder) => folder.clone(),
    };
    let name = editor.doc.path().parent().and_then(Path::file_name).map(|n| n.to_string_lossy().to_string());
    let Some(name) = name else { return };
    let atlas = editor.doc.path().with_file_name("atlas.png");
    let file = match export(editor.doc.value(), &world.data, &spot, &assets, &name, &atlas) {
        Ok(file) => file,
        Err(error) => {
            editor.status = format!("Play: {error}");
            return;
        }
    };
    let at = format!("room {} at {:?}", spot.room, spot.position);
    if request != Play::Run {
        editor.status = format!("Wrote {file} with Lara in {at}");
        return;
    }
    if let Some(mut old) = running.0.take() {
        // One play-test at a time.
        let _ = old.kill();
        let _ = old.wait();
    }
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.current_dir(&root).args(["run", "-q", "-p", "tomb-raider-ii"]);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    command.args(["--", "--level", &name, "--level-file", &file]);
    match command.spawn() {
        Ok(child) => {
            running.0 = Some(child);
            editor.status = format!("Starting the game with Lara in {at} (building it first if needed)");
        }
        Err(error) => editor.status = format!("Play: could not run cargo: {error}"),
    }
}

/// The first place along the view, from `eye` (level units, y down), that is
/// inside a room before the level flips, and the floor under it. Lara faces
/// along the view.
pub fn spot(world: &WorldGeometry, eye: [f64; 3], forward: [f64; 3]) -> Option<Spot> {
    let level = &world.data;
    let length = forward.iter().map(|f| f * f).sum::<f64>().sqrt();
    if !length.is_normal() {
        return None;
    }
    let dir = forward.map(|f| f / length);
    let live = live_rooms(level, false);
    let yaw = dir[0].atan2(dir[2]);
    let rotation = (yaw / std::f64::consts::TAU * 65536.0).round().rem_euclid(65536.0) as u16;
    for step in 0..(REACH / 64.0) as usize {
        let t = step as f64 * 64.0;
        let p = [0, 1, 2].map(|i| (eye[i] + dir[i] * t).round() as i32);
        for room in rooms_containing(level, p).into_iter().filter(|&r| live[r]) {
            let Some(hit) = world.floor_at(position(p), room) else { continue };
            let (floor, ceiling) = ((-hit.height * 1024.0).round() as i32, (-hit.ceiling * 1024.0).round() as i32);
            if ceiling <= p[1] && p[1] <= floor {
                return Some(Spot { position: [p[0], floor, p[2]], room: hit.room, rotation });
            }
        }
    }
    None
}

/// Write `document` with Lara at `spot` as `levels/<name>/editor-play.json`
/// under `assets`, copying the level's atlas there if the folder has none.
/// Returns the copy's path relative to `assets`.
pub fn export(
    document: &Value,
    level: &Level,
    spot: &Spot,
    assets: &Path,
    name: &str,
    atlas: &Path,
) -> Result<String, String> {
    if level.model(LARA).is_none() {
        return Err("this level has no Lara model, so the game cannot play it".into());
    }
    let mut document = document.clone();
    let placements = document["placements"].as_array_mut().ok_or("the level has no placements table")?;
    let lara = match level.placements.iter().position(|p| p.kind == LARA) {
        Some(index) => &mut placements[index],
        None => {
            let placement = Placement { kind: LARA, ..Default::default() };
            placements.push(serde_json::to_value(placement).map_err(|e| e.to_string())?);
            placements.last_mut().unwrap()
        }
    };
    lara["room"] = spot.room.into();
    lara["position"] = spot.position.to_vec().into();
    lara["rotation"] = spot.rotation.into();
    let folder = assets.join("levels").join(name);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    if !folder.join("atlas.png").exists() {
        std::fs::copy(atlas, folder.join("atlas.png")).map_err(|e| format!("{}: {e}", atlas.display()))?;
    }
    let file = folder.join("editor-play.json");
    let text = serde_json::to_vec(&document).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(format!("levels/{name}/editor-play.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lara_starts_on_the_floor_the_view_looks_at() {
        let doc = engine::editor::LevelDocument::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../engine/assets/levels/sandbox/level.json"
        ))
        .unwrap();
        let level = doc.level().unwrap();
        let world = WorldGeometry::new(level.clone());
        // The sandbox room: open sectors 1..5 on both axes, floor 0, ceiling -2048.
        let from_above = spot(&world, [2560.0, -6000.0, -3000.0], [0.0, 6000.0, 5560.0]).unwrap();
        assert_eq!(from_above.room, 0);
        assert_eq!(from_above.position[1], 0, "on the floor");
        assert!((1024..5120).contains(&from_above.position[0]) && (1024..5120).contains(&from_above.position[2]));
        assert_eq!(from_above.rotation, 0, "facing +z");
        let inside = spot(&world, [2000.0, -1000.0, 2000.0], [1.0, 0.0, 0.0]).unwrap();
        assert_eq!((inside.position, inside.rotation), ([2000, 0, 2000], 16384), "under the eye, facing +x");
        assert_eq!(spot(&world, [2000.0, -1000.0, 2000.0], [0.0, -1.0, 0.0]).map(|s| s.room), Some(0));
        assert_eq!(spot(&world, [90000.0, 0.0, 0.0], [1.0, 0.0, 0.0]), None, "nothing out there");

        let folder = std::env::temp_dir().join(format!("editor-play-{}", std::process::id()));
        let atlas = doc.path().with_file_name("atlas.png");
        let refused = export(doc.value(), &level, &inside, &folder, "sandbox", &atlas);
        assert!(refused.unwrap_err().contains("no Lara model"), "the sandbox has none");
        let mut with_lara = level.clone();
        with_lara.models.push(engine::world::Model { id: LARA, ..Default::default() });
        let file = export(doc.value(), &with_lara, &inside, &folder, "sandbox", &atlas).unwrap();
        assert_eq!(file, "levels/sandbox/editor-play.json");
        let written: Value = serde_json::from_slice(&std::fs::read(folder.join(&file)).unwrap()).unwrap();
        let placements = written["placements"].as_array().unwrap();
        assert_eq!(placements.len(), level.placements.len() + 1, "a Lara placement is added");
        let lara = placements.last().unwrap();
        assert_eq!(
            (&lara["kind"], &lara["room"], &lara["rotation"]),
            (&Value::from(0), &Value::from(0), &Value::from(16384))
        );
        assert_eq!(lara["position"], serde_json::json!([2000, 0, 2000]));
        assert!(folder.join("levels/sandbox/atlas.png").exists());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
