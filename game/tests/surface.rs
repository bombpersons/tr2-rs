//! Water-surface PSX replays: surfacing, surface strokes, dive and climb-out.
//! The only pokes created the documented submerged checkpoint
//! (tools/record_swimming.py --setup); pad replays receive no corrections.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    testing::{Alignment, Replay, TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use game::player::{
    AnimationState, Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin, SubmergedMotion, Traversal,
};

/// Replays `csv`, comparing every column through `last_field` (inclusive).
fn compare_case(csv: &str, metadata: &str, last_field: i32) {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/assault/level.json")).unwrap();
    let headers: Vec<_> = csv.lines().next().unwrap().split(',').collect();
    let rows: Vec<Vec<i32>> = csv.lines().skip(1).map(|l| l.split(',').map(|v| v.parse().unwrap()).collect()).collect();
    let at = |row: &[i32], key: &str| row[headers.iter().position(|&k| k == key).unwrap()];
    let start = &rows[0];
    let angle = |v: i32| v as f32 * std::f32::consts::TAU / 65536.0;
    let binary = |v: f32| (v * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32;
    let clip = at(start, "animation") as usize;
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(PlayerRulesPlugin).insert_resource(PlayerAnimations::from(&data));
    let mut player = Player::at(
        position([at(start, "x"), at(start, "y"), at(start, "z")]),
        angle(at(start, "yaw")),
        at(start, "room") as usize,
    );
    player.mode = match at(start, "water_mode") {
        0 => PlayerMode::Idle,
        1 => PlayerMode::Swimming,
        _ => PlayerMode::Surface,
    };
    player.health = at(start, "health");
    let entity = app
        .world_mut()
        .spawn((
            player,
            AnimationState::at_frame(
                clip,
                at(start, "animation_frame") as f32 - data.animations[clip].frame_start as f32,
            ),
            Motion {
                grounded: at(start, "water_mode") == 0,
                pitch: angle(at(start, "pitch")),
                turn_rate: at(start, "turn_rate"),
                oxygen_seconds: at(start, "air") as f32 / 30.0,
                movement_heading: Some(angle(at(start, "yaw"))),
                ..default()
            },
            SubmergedMotion {
                speed: at(start, "swim_speed") as f32 * 30.0 / 4096.0,
                bank: angle(at(start, "roll")),
                air_ticks: at(start, "air"),
                dive_hold: 0,
            },
        ))
        .id();
    app.world_mut().get_mut::<Traversal>(entity).unwrap().goal = at(start, "goal") as usize;
    app.insert_resource(WorldGeometry::new(data));
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let inputs = Replay::parse(&metadata["input"].to_string())
        .unwrap()
        .tick_inputs(Alignment::new(2, 0), (rows.len() - 1) / 2)
        .unwrap();
    for (row, input) in rows.iter().skip(2).step_by(2).zip(inputs) {
        if at(row, "run_frame") > last_field {
            break;
        }
        app.run_tick(&input);
        let world = app.world();
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let s = world.get::<SubmergedMotion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        let c = &world.resource::<WorldGeometry>().data.animations[a.animation];
        let water_mode = match p.mode {
            PlayerMode::Swimming => 1,
            PlayerMode::Surface => 2,
            _ => 0,
        };
        let mut checks = vec![
            ("water_mode", water_mode),
            ("x", (p.position.x * 1024.0).round() as i32),
            ("y", (-p.position.y * 1024.0).round() as i32),
            ("z", (p.position.z * 1024.0).round() as i32),
            ("yaw", binary(p.yaw)),
            ("animation", a.animation as i32),
            ("animation_frame", c.frame_start as i32 + a.frame as i32),
            ("state", c.state as i32),
            ("room", p.room as i32),
            ("health", p.health),
        ];
        // Standing jump preparation keeps its own provisional goal labels
        // (state15 requests25 at once in the original); water rows check goals.
        if water_mode != 0 || at(row, "water_mode") != 0 {
            checks.extend([
                ("goal", world.get::<Traversal>(entity).unwrap().goal as i32),
                ("pitch", binary(m.pitch)),
                ("roll", binary(s.bank)),
                ("turn_rate", m.turn_rate),
                ("swim_speed", (s.speed * 4096.0 / 30.0).round() as i32),
                ("air", s.air_ticks),
            ]);
        }
        for (name, actual) in checks {
            assert_eq!(actual, at(row, name), "field{} {name}", at(row, "run_frame"));
        }
    }
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        case!($test, $name, i32::MAX);
    };
    ($test:ident, $name:literal, $last:expr) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/surface_", $name, ".csv")),
                include_str!(concat!("data/original/surface_", $name, ".json")),
                $last,
            );
        }
    };
}

case!(original_submerged_stroke_surfaces_into_tread_and_refills_air, "rise");
case!(original_surface_forward_stroke_turn_and_release, "forward_turn");
case!(original_surface_back_and_side_strokes, "back_side");
case!(original_held_jump_dives_after_ten_tread_ticks, "dive");
case!(original_surface_wall_stops_and_rejects_low_ceiling_climb, "wall_climb_blocked");
case!(original_surface_climb_out_onto_ledge, "climb_out");

// Independent routes, first recorded after the rules above were written.
case!(heldout_angled_east_climb_snaps_to_the_axis, "climb_east_heldout");
case!(heldout_right_stroke_into_wall_stops_and_deflects, "strafe_wall_heldout");
case!(heldout_dive_with_up_and_turn_pitches_down_and_settles, "dive_steer_heldout");
// Both hops end at the pool's east wall (fields64/84), where the
// submerged wall collision begins; that collision is not ported yet.
case!(heldout_back_hop_off_the_edge_enters_the_entry_dive, "entry_hop_heldout", 62);
case!(heldout_turned_back_hop_enters_at_an_angle, "entry_angled_hop_heldout", 82);
