//! Submerged PSX replays. The only pokes created the documented initial
//! checkpoint; pad replays and these controllers receive no later corrections.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    testing::{Alignment, Replay, TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use game::player::{
    AnimationState, Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin, SubmergedMotion, Traversal,
};

fn compare_case(csv: &str, metadata: &str) {
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
    player.mode = PlayerMode::Swimming;
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
                grounded: false,
                pitch: angle(at(start, "pitch")),
                turn_rate: at(start, "turn_rate"),
                oxygen_seconds: at(start, "air") as f32 / 30.0,
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
        app.run_tick(&input);
        let world = app.world();
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let s = world.get::<SubmergedMotion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        let c = &world.resource::<WorldGeometry>().data.animations[a.animation];
        for (name, actual) in [
            ("x", (p.position.x * 1024.0).round() as i32),
            ("y", (-p.position.y * 1024.0).round() as i32),
            ("z", (p.position.z * 1024.0).round() as i32),
            ("yaw", binary(p.yaw)),
            ("pitch", binary(m.pitch)),
            ("roll", binary(s.bank)),
            ("turn_rate", m.turn_rate),
            ("swim_speed", (s.speed * 4096.0 / 30.0).round() as i32),
            ("speed", (c.speed >> 16)),
            ("animation", a.animation as i32),
            ("animation_frame", c.frame_start as i32 + a.frame as i32),
            ("state", c.state as i32),
            ("goal", world.get::<Traversal>(entity).unwrap().goal as i32),
            ("room", p.room as i32),
            ("air", s.air_ticks),
            ("health", p.health),
        ] {
            assert_eq!(actual, at(row, name), "field{} {name}", at(row, "run_frame"));
        }
        assert_eq!(p.mode, PlayerMode::Swimming);
    }
}

#[test]
fn original_submerged_stroke_acceleration_glide_and_complete_stop() {
    compare_case(
        include_str!("data/original/swimming_straight.csv"),
        include_str!("data/original/swimming_straight.json"),
    );
}
#[test]
fn original_submerged_turn_bank_pitch_and_release() {
    compare_case(include_str!("data/original/swimming_steer.csv"), include_str!("data/original/swimming_steer.json"));
}
#[test]
fn original_treading_turn_pitch_caps_and_bank_recovery() {
    compare_case(
        include_str!("data/original/swimming_idle_steer.csv"),
        include_str!("data/original/swimming_idle_steer.json"),
    );
}

#[test]
fn heldout_submerged_restart_waits_for_authored_glide_and_stroke_ranges() {
    compare_case(
        include_str!("data/original/swimming_restart_heldout.csv"),
        include_str!("data/original/swimming_restart_heldout.json"),
    );
}
#[test]
fn heldout_submerged_left_turn_and_upward_pitch_stop_completely() {
    compare_case(
        include_str!("data/original/swimming_left_up_heldout.csv"),
        include_str!("data/original/swimming_left_up_heldout.json"),
    );
}
#[test]
fn heldout_treading_direction_reversal_recovers_bank_and_turn_inertia() {
    compare_case(
        include_str!("data/original/swimming_idle_reverse_heldout.csv"),
        include_str!("data/original/swimming_idle_reverse_heldout.json"),
    );
}

// Collision 0x80045CB8, all recorded after the collision rules were written.
#[test]
fn heldout_head_on_stroke_stops_at_the_pool_wall() {
    compare_case(
        include_str!("data/original/swim_collision_wall_head_on.csv"),
        include_str!("data/original/swim_collision_wall_head_on.json"),
    );
}
#[test]
fn heldout_angled_stroke_along_the_wall_deflects_then_stops_in_the_corner() {
    compare_case(
        include_str!("data/original/swim_collision_wall_angled.csv"),
        include_str!("data/original/swim_collision_wall_angled.json"),
    );
}
#[test]
fn heldout_downward_stroke_is_pushed_off_the_pool_floor() {
    compare_case(
        include_str!("data/original/swim_collision_floor_dive.csv"),
        include_str!("data/original/swim_collision_floor_dive.json"),
    );
}
#[test]
fn heldout_rising_stroke_slides_under_the_tunnel_ceiling() {
    compare_case(
        include_str!("data/original/swim_collision_tunnel_ceiling.csv"),
        include_str!("data/original/swim_collision_tunnel_ceiling.json"),
    );
}
