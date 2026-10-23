//! The title, game over and in-play passport rings against the original,
//! frame by frame (`tools/record_passport.py`): each recorded frame of
//! Display_Inventory is replayed through [`Ring::frame`], and the ring, its
//! motion and the options ring's items are compared with the original's
//! after that frame, and what the ring closes on with the original's
//! chosen item (`0x80089032`) and passport page (`0x80089034`).
use std::collections::HashMap;

use game::inventory::{
    Inventory, ItemTable,
    ring::{Passport, Poses, Ring, RingExit, RingMode, Status},
};

struct Recording {
    columns: HashMap<String, usize>,
    rows: Vec<Vec<i64>>,
    frames: Vec<serde_json::Value>,
}

impl Recording {
    fn load(name: &str) -> Self {
        let csv = std::fs::read_to_string(format!("tests/data/original/passport_{name}.csv")).unwrap();
        let mut lines = csv.lines();
        let columns = lines.next().unwrap().split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let value = |v: &str| match v.strip_prefix("0x") {
            Some(hex) => i64::from_str_radix(hex, 16).unwrap(),
            None => v.parse().unwrap(),
        };
        let rows = lines.map(|l| l.split(',').map(value).collect()).collect();
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(format!("tests/data/original/passport_{name}.json")).unwrap())
                .unwrap();
        Self { columns, rows, frames: meta["ring_frames"].as_array().unwrap().clone() }
    }
    fn get(&self, sample: usize, column: &str) -> i64 {
        self.rows[sample][self.columns[column]]
    }
}

/// The options ring's items as the recordings name them.
const ITEMS: [(&str, u16); 5] = [("passport", 133), ("screen", 153), ("sound", 154), ("controls", 155), ("home", 122)];

/// Replays a recording of a ring opened in `mode` with `options_current`
/// in front; returns how many frames were compared and what it closed on.
fn replay(name: &str, mode: RingMode, options_current: i16) -> (usize, Option<RingExit>) {
    let recording = Recording::load(name);
    let table = ItemTable::default();
    let inventory = Inventory::new_game(&table);
    let mut poses = Poses::default();
    let mut ring: Option<Ring> = None;
    let mut compared = 0;
    let mut closed = None;
    for (n, frame) in recording.frames.iter().enumerate() {
        let sample = frame["sample"].as_u64().unwrap() as usize;
        let at = |column: &str| recording.get(sample, column);
        if frame["open"].as_bool().unwrap() {
            let opened_in = if at("mode") == 1 { RingMode::Title } else { mode };
            ring = Some(Ring::open(opened_in, &inventory, &table, &mut poses, 0, options_current));
        }
        let ring_now = ring.as_mut().expect("a frame of an open ring");
        let input = frame["input"].as_u64().unwrap() as u32;
        let vsyncs = frame["vsyncs"].as_u64().unwrap() as u32;
        let exit = ring_now.frame(input, vsyncs, &table, &mut poses);
        let ours: Vec<(&str, i64)> = vec![
            ("ring_type", ring_now.kind as i64),
            ("radius", ring_now.radius.into()),
            ("camera_pitch", ring_now.camera_pitch.into()),
            ("rotating", ring_now.rotating.into()),
            ("current", ring_now.current.into()),
            ("count", ring_now.count.into()),
            ("ring_yaw", ring_now.yaw.into()),
            ("camera_y", ring_now.camera_y.into()),
            ("camera_z", ring_now.camera_z.into()),
            ("motion_count", ring_now.motion.count.into()),
            ("status", ring_now.motion.status as i64),
            ("status_target", ring_now.motion.status_target as i64),
        ];
        for (column, value) in ours {
            assert_eq!(value, at(column), "{name}: {column} after frame {n} (sample {sample})");
        }
        for (item, object) in ITEMS {
            let Some(pose) = poses.get(&object).copied() else { continue };
            if !ring_now.list().contains(&object) {
                continue;
            }
            let fields: [(&str, i64); 8] = [
                ("object", pose.object.into()),
                ("frame", pose.frame.into()),
                ("goal", pose.goal.into()),
                ("pt_xrot", pose.pt_xrot.into()),
                ("x_rot", pose.x_rot.into()),
                ("y_rot", pose.y_rot.into()),
                ("ytrans", pose.y.into()),
                ("ztrans", pose.z.into()),
            ];
            for (field, value) in fields {
                let column = format!("{item}_{field}");
                assert_eq!(value, at(&column), "{name}: {column} after frame {n} (sample {sample})");
            }
        }
        compared += 1;
        if let Some(exit) = exit {
            assert_eq!(ring_now.motion.status, Status::Done);
            assert_eq!(exit.chosen.map_or(-1, i64::from), at("item_chosen"), "{name}: chosen after frame {n}");
            if let Some(passport) = exit.passport {
                let page = match passport {
                    Passport::Load(_) => 0,
                    Passport::NewGame | Passport::Save | Passport::RestartLevel => 1,
                    Passport::ExitToTitle => 2,
                };
                assert_eq!(page, at("page"), "{name}: the passport's page");
            }
            closed = Some(exit);
            ring = None;
        }
    }
    (compared, closed)
}

/// From power-on: the first title's ring opens on Lara's Home, tilted;
/// the passport opened, turned to New Game but not past it, back to Load
/// Game and closed with Triangle; the ring turned back to Lara's Home.
#[test]
fn the_title_ring_matches_the_original() {
    let (frames, closed) = replay("title", RingMode::Title, 4);
    assert_eq!(frames, 530);
    assert_eq!(closed, None, "the title's ring stays open");
}

/// Game over: the passport opens by itself, turns through Restart Level to
/// Exit to Title and back, ignores Triangle, and closes on Restart Level.
#[test]
fn the_game_over_ring_matches_the_original() {
    let (frames, closed) = replay("death", RingMode::Death, 0);
    assert_eq!(frames, 337);
    assert_eq!(closed, Some(RingExit { chosen: Some(120), passport: Some(Passport::RestartLevel) }));
}

/// Held out: in play, the passport turned to Save Game and Exit to Title,
/// not past it, closed from the last page onwards, and the ring closed.
#[test]
fn the_passport_in_play_matches_the_original_held_out() {
    let (frames, closed) = replay("game_pages_heldout", RingMode::Game, 0);
    assert_eq!(frames, 289);
    assert_eq!(closed, Some(RingExit { chosen: None, passport: None }));
}
