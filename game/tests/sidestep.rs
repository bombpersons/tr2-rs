//! Sidestep controls and stops recorded from the supplied disc, without pokes.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    controller::Button,
    testing::{Alignment, Trajectory, compare, headless_app},
    world::{Level, WorldGeometry},
};
use game::player::{AnimationState, Motion, Player, PlayerAnimations, PlayerRulesPlugin};

fn compare_case(csv: &str, metadata: &str, position_tolerance: f64) {
    let original = Trajectory::parse_psx_samples(csv, Alignment::new(2, 0)).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let data: Level =
        serde_json::from_slice(&std::fs::read("assets/imported/levels/assault/level.json").unwrap()).unwrap();
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
    let entity = app.world_mut().spawn((Player::at(position, yaw, 16), AnimationState::at_frame(clip, frame))).id();
    let inputs: Vec<Vec<Button>> = (0..original.ticks() - 1)
        .map(|tick| {
            let field = (2 * tick) as u64;
            metadata["input"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["frame"].as_u64().unwrap() <= field && field < event["until"].as_u64().unwrap())
                .flat_map(|event| event["buttons"].as_array().unwrap())
                .map(|button| match button.as_str().unwrap() {
                    "DOWN" => Button::Down,
                    "UP" => Button::Up,
                    "L2" => Button::L2,
                    "R2" => Button::R2,
                    "R1" => Button::R1,
                    "LEFT" => Button::Left,
                    "RIGHT" => Button::Right,
                    other => panic!("unexpected recorded button {other}"),
                })
                .collect()
        })
        .collect();
    let columns = ["x", "y", "z", "yaw", "animation", "animation_frame", "state", "speed", "fall_speed", "turn_rate"];
    let rebuild = Trajectory::record(&mut app, &inputs, &columns, |world| {
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let a = world.get::<AnimationState>(entity).unwrap();
        vec![
            f64::from(p.position.x * 1024.0),
            f64::from(-p.position.y * 1024.0),
            f64::from(p.position.z * 1024.0),
            f64::from((p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16),
            a.animation as f64,
            f64::from(starts[a.animation] as f32 + a.frame),
            states[a.animation] as f64,
            f64::from((m.velocity.with_y(0.0).length() * 1024.0 / 30.0).round()),
            f64::from(-m.velocity.y * 1024.0 / 30.0),
            f64::from(m.turn_rate),
        ]
    });
    let tolerances: Vec<_> = columns
        .iter()
        .map(|&column| {
            (
                column,
                match column {
                    "x" | "z" => position_tolerance,
                    "y" => 0.02,
                    _ => 0.0,
                },
            )
        })
        .collect();
    compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}

#[test]
fn original_left_step_waits_for_stride_and_stop_pose() {
    compare_case(
        include_str!("data/original/sidestep_left_clear.csv"),
        include_str!("data/original/sidestep_left_clear.json"),
        0.0,
    );
}

#[test]
fn original_right_step_loops_then_stops() {
    compare_case(
        include_str!("data/original/sidestep_right_base.csv"),
        include_str!("data/original/sidestep_right_base.json"),
        0.0,
    );
}

#[test]
fn original_walk_left_turns_without_strafing() {
    compare_case(
        include_str!("data/original/sidestep_walk_left.csv"),
        include_str!("data/original/sidestep_walk_left.json"),
        0.0,
    );
}

#[test]
fn original_walk_right_stays_slow_after_thirty_three_ticks() {
    compare_case(
        include_str!("data/original/sidestep_walk_right.csv"),
        include_str!("data/original/sidestep_walk_right.json"),
        0.0,
    );
}

#[test]
fn original_l2_alone_does_not_sidestep() {
    compare_case(
        include_str!("data/original/sidestep_left.csv"),
        include_str!("data/original/sidestep_left.json"),
        0.0,
    );
}

#[test]
fn original_r2_alone_does_not_sidestep() {
    compare_case(
        include_str!("data/original/sidestep_right.csv"),
        include_str!("data/original/sidestep_right.json"),
        0.0,
    );
}

#[test]
fn heldout_sidestep_steers_then_finishes_after_release() {
    compare_case(
        include_str!("data/original/sidestep_steer_heldout.csv"),
        include_str!("data/original/sidestep_steer_heldout.json"),
        0.0,
    );
}

#[test]
fn heldout_reholding_does_not_cancel_stop_and_reversal_waits() {
    compare_case(
        include_str!("data/original/sidestep_rehold_heldout.csv"),
        include_str!("data/original/sidestep_rehold_heldout.json"),
        0.0,
    );
}
