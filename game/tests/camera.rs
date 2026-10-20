//! Camera checks use original psx captures, plus explicit lifecycle fixtures.
#![cfg(feature = "baked-assets")]
use std::sync::Arc;

use bevy::prelude::*;
use engine::{
    camera::{CameraRulesPlugin, CameraState, approach_eye},
    controller::Button,
    testing::{TickApp, headless_app},
    world::{Level, WorldGeometry, format::CameraShot, position},
};
use game::{
    interactions::InteractionsPlugin,
    player::{
        AnimationState, Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin,
        SimulationEnabled,
    },
};

fn wall() -> (App, Entity) {
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((CameraRulesPlugin, InteractionsPlugin));
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/wall/level.json")).unwrap();
    app.insert_resource(WorldGeometry::from(Arc::new(data)));
    let player = app.world_mut().spawn(Player::at(position([62976, 1542, 23040]), 0.0, 35)).id();
    (app, player)
}

#[test]
fn wall_once_camera_continues_and_matches_independent_left_input_capture() {
    let (mut app, _) = wall();
    let recorded = include_str!("data/original/camera_heldout_left.csv");
    // Original checkpoint field16695, LEFT held16 fields.
    // This checks selection/eye, not locomotion.
    for row in recorded.lines().skip(2).step_by(2) {
        app.run_tick(&[Button::Left]);
        let values: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        let state = app.world().resource::<CameraState>();
        assert_eq!(state.fixed_camera, Some(2));
        assert_eq!(state.room, 35);
        assert_eq!(state.position, position([values[2], values[3], values[4]]));
    }
}

#[test]
fn fixed_eye_matches_held_out_right_capture_recorded_after_implementation() {
    let (mut app, _) = wall();
    for row in include_str!("data/original/camera_heldout_right.csv").lines().skip(2).step_by(2) {
        app.run_tick(&[Button::Right]);
        let values: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        let state = app.world().resource::<CameraState>();
        assert_eq!(state.fixed_camera, Some(2));
        assert_eq!(state.position, position([values[2], values[3], values[4]]));
    }
}

#[test]
fn authored_one_second_timer_matches_patched_original_operand() {
    let (mut app, _) = wall();
    let mut data: Level = serde_json::from_slice(include_bytes!("../assets/levels/wall/level.json")).unwrap();
    // Same controlled source mutation as psx capture: both entrance camera
    // settings words (floor data 3396 and 3402, in triggers 3393 and 3399)
    // become 0x8001: one second, reusable, default speed.
    let mut patched = 0;
    for trigger in data.floor_infos.iter_mut().flat_map(|f| &mut f.triggers).filter(|t| [3393, 3399].contains(&t.id)) {
        trigger.actions[0].camera = Some(CameraShot { timer: 1, once: false, speed: 0 });
        patched += 1;
    }
    assert_eq!(patched, 2);
    app.insert_resource(WorldGeometry::from(Arc::new(data)));
    for row in include_str!("data/original/camera_authored_timer_probe.csv").lines().skip(19).step_by(2) {
        app.run_tick(&[]);
        let values: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        let state = app.world().resource::<CameraState>();
        assert_eq!(state.ticks_left.unwrap() as i32, values[11].max(0));
        if values[11] >= 0 || values[0] == 76 {
            assert_eq!(state.fixed_camera, Some(2));
        } else {
            assert_eq!(state.fixed_camera, None);
        }
    }
}

#[test]
fn eye_easing_matches_psx_memory_perturbation_for_every_tick() {
    let (mut app, _) = wall();
    app.run_tick(&[]);
    app.world_mut().resource_mut::<CameraState>().position = position([62052, 384, 23040]);
    // Only the source camera X was poked (+100) at field16695; all subsequent
    // observations are original execution, not generated reference values.
    for row in include_str!("data/original/camera_smoothing_probe.csv").lines().skip(2).step_by(2) {
        app.run_tick(&[]);
        let values: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(app.world().resource::<CameraState>().position, position([values[2], values[3], values[4]]));
    }
    // The opposite sign also truncates towards zero, not towards -infinity.
    assert_eq!(approach_eye(position([61951, 384, 23040]), position([61952, 384, 23040]), 10).x, 61951.0 / 1024.0);
}

#[test]
fn timed_camera_expires_without_resetting_on_each_trigger_tick() {
    let (mut app, _) = wall();
    app.run_tick(&[]);
    app.world_mut().resource_mut::<CameraState>().ticks_left = Some(3);
    let rows: Vec<_> = include_str!("data/original/camera_timer_probe.csv").lines().skip(2).step_by(2).collect();
    for (i, row) in rows.iter().enumerate() {
        app.run_tick(&[]);
        let values: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        let state = app.world().resource::<CameraState>();
        if i < 3 {
            assert_eq!(state.position, position([values[2], values[3], values[4]]));
            assert_eq!(state.fixed_camera, Some(2));
            assert_eq!(state.ticks_left.unwrap() as i32, values[11].max(0));
        } else {
            assert_eq!(state.fixed_camera, None);
        }
    }
}

#[test]
fn leaving_once_shot_releases_camera_and_reentry_does_not_restart_it() {
    let (mut app, player) = wall();
    app.run_tick(&[]);
    assert_eq!(app.world().resource::<CameraState>().fixed_camera, Some(2));
    // Actual settled cave position has left the camera2 trigger sectors.
    app.world_mut().get_mut::<Player>(player).unwrap().position = position([64176, 5376, 26090]);
    app.run_tick(&[]);
    assert_eq!(app.world().resource::<CameraState>().fixed_camera, None);
    app.world_mut().get_mut::<Player>(player).unwrap().position = position([62976, 1542, 23040]);
    app.run_tick(&[]);
    assert_eq!(app.world().resource::<CameraState>().fixed_camera, None);
}

#[test]
fn camera_lifetime_and_pose_freeze_during_pause() {
    let (mut app, _) = wall();
    app.run_tick(&[]);
    app.world_mut().resource_mut::<CameraState>().ticks_left = Some(3);
    let before = app.world().resource::<CameraState>().position;
    app.insert_resource(SimulationEnabled(false));
    app.run_ticks(20, &[]);
    let camera = app.world().resource::<CameraState>();
    assert_eq!(camera.position, before);
    assert_eq!(camera.ticks_left, Some(3));
}

#[test]
fn original_animation_bounds_drive_the_recorded_falling_camera_target() {
    let (mut app, player) = wall();
    app.run_tick(&[]);
    app.world_mut().resource_mut::<CameraState>().target = position([62976, 709, 23040]);
    // Feed observed actor poses to isolate camera rules from locomotion error.
    // Golden source: neutral Wall entry, first initialized field16693.
    for row in include_str!("data/original/wall_entry.csv").lines().skip(2).step_by(2).take(48) {
        let v: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        app.world_mut().get_mut::<Player>(player).unwrap().position = position([v[2], v[3], v[4]]);
        let start = app.world().resource::<WorldGeometry>().data.animations[v[6] as usize].frame_start;
        let mut a = app.world_mut().get_mut::<AnimationState>(player).unwrap();
        a.animation = v[6] as usize;
        a.frame = v[7] as f32 - start as f32;
        app.run_tick(&[]);
        let state = app.world().resource::<CameraState>();
        assert_eq!(state.fixed_camera, Some(2));
        assert_eq!(state.target, position([v[14], v[15], v[16]]), "original field {}", v[1]);
    }
}

#[test]
fn wall_spawn_camera_matches_original_without_seeding_a_recorded_target() {
    let (mut app, player) = wall();
    app.add_plugins(PlayerRulesPlugin);
    let clips = PlayerAnimations::from(app.world().resource::<WorldGeometry>().data.as_ref());
    app.insert_resource(clips);
    {
        let mut p = app.world_mut().get_mut::<Player>(player).unwrap();
        p.position = position([62976, 1536, 23040]);
        p.mode = PlayerMode::Fall;
    }
    app.world_mut().get_mut::<Motion>(player).unwrap().grounded = false;
    *app.world_mut().get_mut::<AnimationState>(player).unwrap() = AnimationState::at_frame(34, 0.0);
    for row in include_str!("data/original/wall_entry.csv").lines().skip(3).step_by(2).take(48) {
        app.run_tick(&[]);
        let v: Vec<i32> = row.split(',').map(|s| s.parse().unwrap()).collect();
        let camera = app.world().resource::<CameraState>();
        assert_eq!(camera.position, position([v[11], v[12], v[13]]));
        assert_eq!(camera.target, position([v[14], v[15], v[16]]), "original field {}", v[1]);
    }
}

#[test]
fn chase_camera_matches_original_through_turn_climb_and_wall_stop() {
    // climb_two_click route (tests/player.rs) from the settled Wall start;
    // original eye/target sampled from 0x8008b258/0x8008b268 every field.
    let (mut app, player) = wall();
    app.add_plugins(PlayerRulesPlugin);
    let clips = PlayerAnimations::from(app.world().resource::<WorldGeometry>().data.as_ref());
    app.insert_resource(clips);
    {
        let mut p = app.world_mut().get_mut::<Player>(player).unwrap();
        p.position = position([64176, 5376, 26090]);
        p.room = 35;
        p.mode = PlayerMode::Idle;
    }
    *app.world_mut().get_mut::<AnimationState>(player).unwrap() = AnimationState::at_frame(103, 45.0);
    // Let level-load camera initialization run once before seeding the
    // original's settled camera.
    app.insert_resource(SimulationEnabled(false));
    app.run_tick(&[]);
    app.insert_resource(SimulationEnabled(true));
    {
        let mut camera = app.world_mut().resource_mut::<CameraState>();
        camera.position = position([63752, 4787, 25846]);
        camera.target = position([64176, 4795, 26090]);
        camera.room = 35;
        camera.initialized = true;
    }
    let rows: Vec<Vec<i32>> = include_str!("data/original/climb_camera.csv")
        .lines()
        .skip(1)
        .map(|l| l.split(',').map(|v| v.parse().unwrap()).collect())
        .collect();
    let limit: usize = std::env::var("CHASE_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(108);
    let mut failures = vec![];
    for t in 0..limit {
        let buttons = match 2 * t {
            f if f < 56 => vec![Button::Right],
            f if f >= 60 => vec![Button::Up, Button::Cross],
            _ => vec![],
        };
        app.run_tick(&buttons);
        let v = &rows[2 * t + 3];
        let camera = app.world().resource::<CameraState>();
        let eye = (camera.position * 1024.0).round();
        let target = (camera.target * 1024.0).round();
        let port = [eye.x as i32, -eye.y as i32, eye.z as i32, target.x as i32, -target.y as i32, target.z as i32];
        if port[..] != v[2..8] {
            failures.push(format!("field {}: original {:?} port {:?}", v[0], &v[2..8], port));
        }
    }
    assert!(failures.is_empty(), "{} ticks differ:\n{}", failures.len(), failures.join("\n"));
}
