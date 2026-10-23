//! Shallow-water PSX replays; only documented fixture placement uses pokes.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    camera::{CameraRulesPlugin, CameraState},
    testing::{Alignment, Replay, TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use game::player::{
    AnimationState, Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin, SimulationEnabled,
    SubmergedMotion, Traversal, Wading,
};

/// Replay native controller ticks without correcting state after row0.
/// Compare complete gameplay samples; metadata declares whole-route camera coverage.
fn compare_case(csv: &str, metadata: &str, last_field: i32) {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/imported/levels/assault/level.json")).unwrap();
    let headers: Vec<_> = csv.lines().next().unwrap().split(',').collect();
    let rows: Vec<Vec<i32>> = csv.lines().skip(1).map(|l| l.split(',').map(|v| v.parse().unwrap()).collect()).collect();
    let at = |row: &[i32], key: &str| row[headers.iter().position(|&k| k == key).unwrap()];
    let start = &rows[0];
    let angle = |v: i32| v as f32 * std::f32::consts::TAU / 65536.0;
    let binary = |v: f32| (v * 65536.0 / std::f32::consts::TAU).round() as i32 as i16 as i32;
    let clip = at(start, "animation") as usize;
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((PlayerRulesPlugin, CameraRulesPlugin)).insert_resource(PlayerAnimations::from(&data));
    let mut player = Player::at(
        position([at(start, "x"), at(start, "y"), at(start, "z")]),
        angle(at(start, "yaw")),
        at(start, "room") as usize,
    );
    player.mode = match at(start, "water_mode") {
        0 => PlayerMode::Idle,
        1 => PlayerMode::Swimming,
        2 => PlayerMode::Surface,
        _ => match at(start, "state") {
            65 => PlayerMode::Wade,
            55 => PlayerMode::PullingUp,
            _ => PlayerMode::Idle,
        },
    };
    player.health = at(start, "health");
    let entity = app
        .world_mut()
        .spawn((
            player,
            Wading { active: at(start, "water_mode") == 4 },
            AnimationState::at_frame(
                clip,
                at(start, "animation_frame") as f32 - data.animations[clip].frame_start as f32,
            ),
            Motion {
                grounded: matches!(at(start, "water_mode"), 0 | 4),
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
    app.insert_resource(SimulationEnabled(false));
    app.run_tick(&[]);
    app.insert_resource(SimulationEnabled(true));
    {
        let v = |prefix: &str| {
            position([
                at(start, &format!("{prefix}_x")),
                at(start, &format!("{prefix}_y")),
                at(start, &format!("{prefix}_z")),
            ])
        };
        let eye = v("camera");
        let room = at(start, "camera_room") as usize;
        let mut camera = app.world_mut().resource_mut::<CameraState>();
        camera.position = eye;
        camera.target = v("target");
        camera.room = room;
        camera.initialized = true;
    }
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let ticks = metadata["logic_ticks"].as_array().unwrap();
    let events: Vec<_> =
        ticks.iter().enumerate().map(|(i, t)| serde_json::json!({"frame":i,"buttons":t["buttons"]})).collect();
    let inputs = Replay::parse(&serde_json::to_string(&events).unwrap())
        .unwrap()
        .tick_inputs(Alignment::new(1, 0), ticks.len())
        .unwrap();
    let mut completed = 0;
    for row in rows.iter().skip(2).step_by(2) {
        let field = at(row, "run_frame");
        if field > last_field {
            break;
        }
        while completed < ticks.len() && ticks[completed]["field"].as_i64().unwrap() < i64::from(field) {
            app.run_tick(&inputs[completed]);
            completed += 1;
        }
        let world = app.world();
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let s = world.get::<SubmergedMotion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        let c = &world.resource::<WorldGeometry>().data.animations[a.animation];
        let water_mode = if world.get::<Wading>(entity).unwrap().active {
            4
        } else {
            match p.mode {
                PlayerMode::Swimming => 1,
                PlayerMode::Surface => 2,
                _ => 0,
            }
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
        {
            checks.extend([
                ("goal", world.get::<Traversal>(entity).unwrap().goal as i32),
                ("pitch", binary(m.pitch)),
                ("roll", binary(s.bank)),
                ("turn_rate", m.turn_rate),
                ("swim_speed", (s.speed * 4096.0 / 30.0).round() as i32),
                ("air", s.air_ticks),
            ]);
        }
        if matches!(water_mode, 0 | 4) {
            checks.push(("speed", (m.velocity.with_y(0.0).length() * 1024.0 / 30.0).round() as i32));
        }
        for (name, actual) in &checks {
            assert_eq!(*actual, at(row, name), "field{field} {name}");
        }
        let camera = world.resource::<CameraState>();
        if metadata["verify_camera"] == true {
            for (prefix, v) in [("camera", camera.position), ("target", camera.target)] {
                for (key, actual) in [
                    ("x", (v.x * 1024.0).round() as i32),
                    ("y", (-v.y * 1024.0).round() as i32),
                    ("z", (v.z * 1024.0).round() as i32),
                ] {
                    assert_eq!(actual, at(row, &format!("{prefix}_{key}")), "field{field} {prefix}_{key}");
                }
            }
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
                include_str!(concat!("data/original/wading_", $name, ".csv")),
                include_str!(concat!("data/original/wading_", $name, ".json")),
                $last,
            );
        }
    };
}

case!(original_shallow_forward_stride_and_wall_stop, "forward_stop");
case!(original_shallow_turn_bank_and_authored_stop, "turn");
case!(original_shallow_back_and_both_side_steps, "back_side");

case!(original_submerged_shallow_entry_stands_up_and_recovers, "stand_up");
case!(heldout_walk_modifier_uses_the_same_wading_gait, "walk_modifier_heldout");
case!(heldout_reholding_forward_waits_for_the_authored_stop, "restart_heldout");
case!(calibration_turn_in_place_ignores_roll_and_starts_wading, "turn_in_place_heldout");

case!(heldout_shallow_water_leaves_wading_through_authored_running, "dry_exit_heldout");
case!(heldout_deeper_water_enters_forward_surface_stroke, "deep_entry_heldout");

case!(original_surface_stroke_stands_up_in_shallow_water, "surface_step_out");

case!(heldout_right_fast_turn_then_left_wading, "idle_right_validation_heldout");
case!(heldout_surface_step_out_with_turn_ignored_during_standup, "surface_validation_heldout");

case!(original_wading_action_cannot_vault_the_fountain_wall, "action_wall_stop");
