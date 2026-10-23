//! The inventory ring against the original, frame by frame: each recorded
//! frame of Display_Inventory (its input word and the vsyncs its motions
//! used, `tools/record_inventory.py`) is replayed through [`Ring::frame`],
//! and the ring, its motion and the main ring's items are compared with the
//! original's after that frame. All from the Great Wall's start inventory.
use std::collections::HashMap;

use game::inventory::{
    Inventory, ItemTable,
    ring::{Poses, Ring, RingMode, Status},
};

/// One recording: its samples by run frame and column, and its ring frames.
struct Recording {
    columns: HashMap<String, usize>,
    rows: Vec<Vec<i64>>,
    frames: Vec<serde_json::Value>,
}

impl Recording {
    fn load(name: &str) -> Self {
        let csv = std::fs::read_to_string(format!("tests/data/original/inventory_{name}.csv")).unwrap();
        let mut lines = csv.lines();
        let columns = lines.next().unwrap().split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let value = |v: &str| match v.strip_prefix("0x") {
            Some(hex) => i64::from_str_radix(hex, 16).unwrap(),
            None => v.parse().unwrap(),
        };
        let rows = lines.map(|l| l.split(',').map(value).collect()).collect();
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(format!("tests/data/original/inventory_{name}.json")).unwrap())
                .unwrap();
        Self { columns, rows, frames: meta["ring_frames"].as_array().unwrap().clone() }
    }
    fn get(&self, sample: usize, column: &str) -> i64 {
        self.rows[sample][self.columns[column]]
    }
}

/// The main ring's objects as the recording names them.
const ITEMS: [(&str, u16); 6] =
    [("stats", 121), ("pistols", 157), ("shotgun", 158), ("flare", 173), ("large", 172), ("small", 171)];

/// Replays a recording; returns how many frames were compared.
fn replay(name: &str) -> usize {
    let recording = Recording::load(name);
    let table = ItemTable::default();
    let inventory = Inventory::new_game(&table);
    let mut poses = Poses::default();
    let mut ring: Option<Ring> = None;
    let mut compared = 0;
    for (n, frame) in recording.frames.iter().enumerate() {
        let sample = frame["sample"].as_u64().unwrap() as usize;
        if frame["open"].as_bool().unwrap() {
            ring = Some(Ring::open(RingMode::Game, &inventory, &table, &mut poses, 0));
        }
        let ring_now = ring.as_mut().expect("a frame of an open ring");
        let input = frame["input"].as_u64().unwrap() as u32;
        let vsyncs = frame["vsyncs"].as_u64().unwrap() as u32;
        let exit = ring_now.frame(input, vsyncs, &table, &mut poses);
        let at = |column: &str| recording.get(sample, column);
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
        if ring_now.rotating {
            assert_eq!(i64::from(ring_now.target), at("target"), "{name}: target after frame {n}");
            assert_eq!(i64::from(ring_now.rot_adder), at("rot_adder"), "{name}: rot_adder after frame {n}");
        }
        for (item, object) in ITEMS {
            let pose = poses[&object];
            let fields: [(&str, i64); 6] = [
                ("frame", pose.frame.into()),
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
            let chosen = recording.get(sample, "item_chosen");
            assert_eq!(exit.chosen.map_or(-1, i64::from), chosen, "{name}: chosen after frame {n}");
            ring = None;
        }
    }
    assert!(ring.is_none(), "{name}: the ring closes");
    compared
}

/// Rotating both ways, the options ring and back, the pistols chosen (they
/// confirm themselves and the ring closes).
#[test]
fn browsing_the_ring_matches_the_original() {
    assert_eq!(replay("browse"), 253);
}

/// The statistics opened and put back, the ring turned and closed.
#[test]
fn choosing_and_putting_back_matches_the_original() {
    assert_eq!(replay("stats"), 196);
}

/// Both medi packs chosen at full health: each confirms after opening and
/// closes the ring, and the ring opens again at the statistics.
#[test]
fn medi_packs_at_full_health_match_the_original() {
    assert_eq!(replay("medipacks"), 246);
}

/// Held out (recorded after the ring was written): the options ring, the
/// passport chosen, put back with Triangle, and the ring closed from there.
#[test]
fn the_passport_matches_the_original_held_out() {
    assert_eq!(replay("passport_heldout"), 216);
}

/// The backdrop as the original drew it in one frame of the open ring
/// (`validation/inventory/gpu_ring`, the ripple's 109th pass since power-on):
/// every vertex's grey on screen, and four vertices' places in the camera's
/// view (the GPU capture's `view`), with the screen's columns along the
/// grid's rows.
#[cfg(feature = "baked-assets")]
#[test]
fn the_backdrop_ripples_as_the_original_draws_it() {
    use bevy::prelude::*;
    use engine::{
        render::view::LEVEL_TO_RENDER,
        world::{Level, position},
    };
    use game::inventory::view::{BackdropRipple, backdrop_camera, backdrop_placement};
    let data: Level =
        serde_json::from_slice(&std::fs::read("assets/imported/levels/wall/level.json").unwrap()).unwrap();
    let model = data.models.iter().find(|m| m.id == 223).unwrap();
    let mesh = &data.meshes[model.meshes[0]];
    let mut ripple = BackdropRipple::default();
    for _ in 0..109 {
        ripple.step();
    }
    // (row, column) of the grid: grey, and the view position where captured.
    let captured: [(usize, usize, u8, Option<[f32; 3]>); 6] = [
        (2, 3, 52, Some([-12384.0, -3376.0, 12820.0])),
        (3, 3, 62, Some([-8971.0, -3278.0, 12820.0])),
        (10, 6, 63, Some([12405.0, 5846.0, 12764.0])),
        (10, 7, 50, Some([12608.0, 9240.0, 12743.0])),
        (2, 1, 50, None),
        (2, 7, 21, None),
    ];
    let to_view = backdrop_camera().compute_affine().inverse()
        * Transform::from_scale(LEVEL_TO_RENDER).compute_affine()
        * backdrop_placement(&data, model).compute_affine();
    for (row, column, grey, view) in captured {
        let index = row * 9 + column;
        let ([dx, dz], ours) = ripple.vertex(index);
        assert_eq!(ours, grey, "grey of vertex {index}");
        let Some(view) = view else { continue };
        let v = mesh.vertices[index];
        let p = to_view.transform_point3(position([v[0] + dx, v[1], v[2] + dz])) * 1024.0;
        // Bevy's view looks down -z with y up; the original's down +z with y down.
        let p = [p.x, -p.y, -p.z];
        // Within 40 units of some 12800 away: the original's fixed-point
        // matrices (measured: at most 18).
        for (axis, (ours, original)) in p.into_iter().zip(view).enumerate() {
            assert!((ours - original).abs() < 40.0, "vertex {index} axis {axis}: {p:?} against {view:?}");
        }
    }
}
