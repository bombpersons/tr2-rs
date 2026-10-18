//! Airborne wall collision routes recorded from the supplied PSX disc without memory pokes.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    testing::{Alignment, Replay, Trajectory, compare, headless_app},
    world::{Level, WorldGeometry},
};
use game::player::{AnimationState, Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin, Traversal};

fn compare_case(csv: &str, metadata: &str) {
    let alignment = Alignment::new(2, 0);
    let original = Trajectory::parse_psx_samples(csv, alignment).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let replay = Replay::parse(&metadata["input"].to_string()).unwrap();
    let data: Level = serde_json::from_slice(
        &std::fs::read(format!("assets/levels/{}/level.json", metadata["level"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    let clips = PlayerAnimations::from(&data);
    let starts: Vec<_> = clips.clips.iter().map(|c| c.frame_start).collect();
    let states: Vec<_> = clips.clips.iter().map(|c| c.state).collect();
    let position = Vec3::new(
        original.get(0, "x").unwrap() as f32,
        -original.get(0, "y").unwrap() as f32,
        original.get(0, "z").unwrap() as f32,
    ) / 1024.0;
    let yaw = original.get(0, "yaw").unwrap() as f32 * std::f32::consts::TAU / 65536.0;
    let clip = original.get(0, "animation").unwrap() as usize;
    let frame = original.get(0, "animation_frame").unwrap() as f32 - starts[clip] as f32;
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(PlayerRulesPlugin).insert_resource(clips).insert_resource(WorldGeometry::new(data));
    let entity = app
        .world_mut()
        .spawn((
            Player::at(position, yaw, original.get(0, "room").unwrap() as usize),
            AnimationState::at_frame(clip, frame),
        ))
        .id();
    let inputs = replay.tick_inputs(alignment, original.ticks() - 1).unwrap();
    let columns = [
        "x",
        "y",
        "z",
        "yaw",
        "animation",
        "animation_frame",
        "state",
        "room",
        "speed",
        "fall_speed",
        "health",
        "turn_rate",
        "goal",
    ];
    let rebuild = Trajectory::record(&mut app, &inputs, &columns, |world| {
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        // Vault collision temporarily overrides the clip's state 2 with 19.
        let state = if matches!(p.mode, PlayerMode::Climbing { .. }) { 19 } else { states[a.animation] };
        vec![
            f64::from(p.position.x * 1024.0),
            f64::from(-p.position.y * 1024.0),
            f64::from(p.position.z * 1024.0),
            f64::from((p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16),
            a.animation as f64,
            f64::from(starts[a.animation] as f32 + a.frame),
            state as f64,
            p.room as f64,
            f64::from({
                // Ground collision can refresh the backward probe heading
                // after the entry move. Only state9 has signed rebound speed.
                let speed = if p.mode == PlayerMode::FastFalling {
                    let heading = m.movement_heading.unwrap_or(p.yaw);
                    m.velocity.dot(Vec3::new(heading.sin(), 0.0, heading.cos()))
                } else {
                    m.velocity.with_y(0.0).length()
                };
                (speed * 1024.0 / 30.0).round()
            }),
            f64::from(-m.velocity.y * 1024.0 / 30.0),
            f64::from(p.health),
            f64::from(m.turn_rate),
            world.get::<Traversal>(entity).unwrap().goal as f64,
        ]
    });
    let tolerances: Vec<_> = columns.iter().map(|&column| (column, if column == "y" { 0.02 } else { 0.0 })).collect();
    compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}

macro_rules! recorded_case {
    ($test:ident, $case:literal) => {
        #[test]
        fn $test() {
            compare_case(
                include_str!(concat!("data/original/airborne_", $case, ".csv")),
                include_str!(concat!("data/original/airborne_", $case, ".json")),
            );
        }
    };
}
recorded_case!(original_forward_jump_rebounds_into_fast_fall_and_heavy_landing, "forward_wall");
recorded_case!(original_reach_keeps_speed_against_the_wall_and_uses_ordinary_landing, "reach_wall");
recorded_case!(original_turning_jump_uses_the_previous_collision_heading_before_rebounding, "forward_steer");
recorded_case!(heldout_delayed_rebound_ignores_jump_during_heavy_landing, "forward_delay_heldout");
recorded_case!(heldout_released_reach_lands_before_resuming_backward_walk, "reach_release_heldout");
