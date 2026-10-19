//! Behavioral regression tests, not claims of measured PS1 trajectory parity.
use std::sync::Arc;

use bevy::prelude::*;
use engine::controller::Button;
use engine::testing::{TickApp, headless_app};
use engine::world::{Level, Room, Sector, WorldGeometry};
use game::player::{
    AnimationState, Motion, Player, PlayerMode, PlayerRulesPlugin, RUN_SPEED, SimulationEnabled, WALK_SPEED,
    WeaponState,
};

fn app() -> (App, Entity) {
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(PlayerRulesPlugin);
    let player = app.world_mut().spawn(Player::default()).id();
    (app, player)
}

fn player(app: &App, entity: Entity) -> &Player {
    app.world().get::<Player>(entity).unwrap()
}
fn seconds(ticks: u32) -> f32 {
    ticks as f32 / game::tick_rate(game::REGION).hz as f32
}

/// Authored 4x4 collision fixture, independent of extracted WALL geometry.
fn room(raised: i32, water: bool) -> WorldGeometry {
    let sectors = (0..16)
        .map(|i| Sector { floor: if i % 4 >= 2 { -raised } else { 0 }, ceiling: -4096, ..Default::default() })
        .collect();
    let room = Room { top: -4096, bottom: 0, x_sectors: 4, z_sectors: 4, sectors, water, ..Default::default() };
    WorldGeometry { data: Arc::new(Level { name: "test".into(), rooms: vec![room], ..Default::default() }) }
}

#[test]
fn forward_run_and_walk_use_decoded_animation_velocities() {
    let (mut app, entity) = app();
    app.run_ticks(40, &[Button::Up]);
    let start = player(&app, entity).position.z;
    app.run_ticks(15, &[Button::Up]);
    assert!((player(&app, entity).position.z - start - RUN_SPEED * seconds(15)).abs() < 1e-5);
    app.run_ticks(40, &[Button::Up, Button::R1]);
    let start = player(&app, entity).position.z;
    app.run_ticks(15, &[Button::Up, Button::R1]);
    assert!((player(&app, entity).position.z - start - WALK_SPEED * seconds(15)).abs() < 1e-5);
    assert_eq!(player(&app, entity).mode, PlayerMode::Walk);
}

#[test]
fn left_and_right_turn_without_strafing() {
    let (mut app, entity) = app();
    app.run_ticks(10, &[Button::Right]);
    assert_eq!(player(&app, entity).position, Vec3::ZERO);
    assert!(player(&app, entity).yaw > 0.0);
    app.run_ticks(10, &[Button::Up]);
    assert!(player(&app, entity).position.x > 0.0);
    assert!(player(&app, entity).position.z > 0.0);
}

#[test]
fn backwards_and_sidestep_preserve_heading() {
    let (mut app, entity) = app();
    app.run_ticks(8, &[Button::Down]);
    assert!(player(&app, entity).position.z < 0.0);
    app.run_ticks(8, &[Button::R2, Button::Right]);
    assert!(player(&app, entity).position.x > 0.0);
    assert_eq!(player(&app, entity).yaw, 0.0);
    assert_eq!(player(&app, entity).mode, PlayerMode::StepRight { stopping: false });
}

#[test]
fn square_jumps_and_a_held_button_does_not_repeat_on_landing() {
    let (mut app, entity) = app();
    app.run_tick(&[Button::Square]);
    assert!(matches!(player(&app, entity).mode, PlayerMode::PreparingJump { .. }));
    app.run_ticks(14, &[Button::Square]);
    assert!(player(&app, entity).position.y > 0.0);
    assert!(!app.world().get::<Motion>(entity).unwrap().grounded);
    app.run_ticks(90, &[Button::Square]);
    assert_eq!(player(&app, entity).position.y, 0.0);
    assert_eq!(player(&app, entity).health, 1000);
    assert_eq!(player(&app, entity).mode, PlayerMode::Idle);
    app.run_tick(&[]);
    app.run_tick(&[Button::Square]);
    app.run_ticks(14, &[Button::Square]);
    assert!(player(&app, entity).position.y > 0.0);
}

#[test]
fn collision_stops_running_at_a_high_wall() {
    let (mut app, entity) = app();
    app.insert_resource(room(1024, false));
    app.world_mut().get_mut::<Player>(entity).unwrap().position = Vec3::new(1.5, 0.0, 1.5);
    app.run_ticks(40, &[Button::Up]);
    let p = player(&app, entity);
    assert!(p.position.z < 2.0, "walked through wall: {p:?}");
    assert_eq!(p.position.y, 0.0);
}

#[test]
fn walking_refuses_an_unprotected_drop_but_running_can_fall() {
    let (mut app, entity) = app();
    app.insert_resource(room(-1024, false));
    app.world_mut().get_mut::<Player>(entity).unwrap().position = Vec3::new(1.5, 0.0, 1.8);
    app.run_ticks(40, &[Button::Up, Button::R1]);
    assert!(player(&app, entity).position.z < 2.0);
    app.run_ticks(12, &[Button::Up]);
    assert!(player(&app, entity).position.z > 2.0);
    assert!(player(&app, entity).position.y < 0.0);
}

#[test]
fn action_climbs_a_two_click_ledge() {
    let (mut app, entity) = app();
    app.insert_resource(room(512, false));
    app.world_mut().get_mut::<Player>(entity).unwrap().position = Vec3::new(1.5, 0.0, 1.8);
    app.run_ticks(45, &[Button::Up, Button::Cross]);
    assert!(player(&app, entity).position.z >= 2.0);
    assert_eq!(player(&app, entity).position.y, 0.5);
}

#[test]
fn airborne_feet_can_clear_a_step_that_blocks_ground_movement() {
    let (mut app, entity) = app();
    app.insert_resource(room(512, false));
    {
        let mut p = app.world_mut().get_mut::<Player>(entity).unwrap();
        p.position = Vec3::new(1.5, 0.52, 1.9);
        p.mode = PlayerMode::Jump;
    }
    {
        let mut motion = app.world_mut().get_mut::<Motion>(entity).unwrap();
        motion.grounded = false;
        motion.velocity = Vec3::new(0.0, 1.0, RUN_SPEED);
    }
    app.run_ticks(4, &[]);
    assert!(player(&app, entity).position.z > 2.0);
}

#[test]
fn swim_uses_square_for_propulsion_and_up_for_downward_pitch() {
    let (mut app, entity) = app();
    // Hand-authored two-state graph keeps this non-disc regression on the
    // same authored-dispatch boundary as the measured submerged controller.
    use engine::world::{Animation, format::StateChange};
    use game::player::PlayerAnimations;
    let mut data = (*room(0, true).data).clone();
    data.animations = vec![
        Animation {
            state: 13,
            frame_rate: 1,
            frame_end: 100,
            state_changes: vec![StateChange { state: 17, first_frame: 0, last_frame: 101, animation: 1, frame: 101 }],
            ..default()
        },
        Animation {
            state: 17,
            frame_rate: 1,
            frame_start: 101,
            frame_end: 201,
            next_animation: 1,
            next_frame: 101,
            ..default()
        },
    ];
    app.insert_resource(PlayerAnimations::from(&data)).insert_resource(WorldGeometry::new(data));
    {
        let mut p = app.world_mut().get_mut::<Player>(entity).unwrap();
        p.position = Vec3::new(1.5, 1.0, 1.0);
        p.mode = PlayerMode::Swimming;
    }
    app.world_mut().get_mut::<game::player::Traversal>(entity).unwrap().goal = 13;
    app.run_tick(&[]);
    assert_eq!(player(&app, entity).mode, PlayerMode::Swimming);
    app.run_ticks(8, &[Button::Square, Button::Up]);
    assert!(player(&app, entity).position.z > 1.0);
    assert!(player(&app, entity).position.y < 1.0);
    assert!(app.world().get::<Motion>(entity).unwrap().oxygen_seconds < 60.0);
}

#[test]
fn lethal_falls_disable_movement_and_firing() {
    let (mut app, entity) = app();
    app.world_mut().get_mut::<Player>(entity).unwrap().position.y = 12.0;
    app.world_mut().get_mut::<Motion>(entity).unwrap().grounded = false;
    app.run_ticks(180, &[]);
    // A lethal impact above154 writes-1 (PSX0x8003E0D4), retained in
    // airborne_fall_damage_155_impact rather than clamped to zero.
    assert_eq!(player(&app, entity).health, -1);
    assert_eq!(player(&app, entity).mode, PlayerMode::Dead);
    let position = player(&app, entity).position;
    app.run_ticks(20, &[Button::Up, Button::Triangle, Button::Cross]);
    assert_eq!(player(&app, entity).position, position);
    assert_eq!(app.world().get::<WeaponState>(entity).unwrap().shot_serial, 0);
}

#[test]
fn draw_and_fire_uses_a_shot_counter_for_consumers() {
    let (mut app, entity) = app();
    app.run_tick(&[Button::Cross]);
    assert_eq!(app.world().get::<WeaponState>(entity).unwrap().shot_serial, 0);
    app.run_tick(&[Button::Triangle, Button::Cross]);
    assert_eq!(app.world().get::<WeaponState>(entity).unwrap().shot_serial, 1);
    app.run_ticks(30, &[Button::Cross]);
    assert!(app.world().get::<WeaponState>(entity).unwrap().shot_serial > 1);
}

#[test]
fn pause_freezes_rules_animation_and_combat() {
    let (mut app, entity) = app();
    app.insert_resource(SimulationEnabled(false));
    app.run_ticks(30, &[Button::Up, Button::Square, Button::Triangle, Button::Cross]);
    assert_eq!(player(&app, entity).position, Vec3::ZERO);
    assert_eq!(app.world().get::<AnimationState>(entity).unwrap().frame, 0.0);
    assert_eq!(app.world().get::<WeaponState>(entity).unwrap().shot_serial, 0);
    app.world_mut().resource_mut::<SimulationEnabled>().0 = true;
    app.run_tick(&[Button::Up]);
    assert!(player(&app, entity).position.z > 0.0);
}

fn recorded_case(
    csv: &str,
    buttons: &[Button],
    jump_release: bool,
) -> (engine::testing::Trajectory, engine::testing::Trajectory) {
    use engine::testing::{Alignment, Trajectory};
    let original = Trajectory::parse_psx_samples(csv, Alignment::new(2, 0)).unwrap();
    let (mut app, entity) = app();
    {
        let mut p = app.world_mut().get_mut::<Player>(entity).unwrap();
        p.position = Vec3::new(
            original.get(0, "x").unwrap() as f32 / 1024.0,
            0.0,
            original.get(0, "z").unwrap() as f32 / 1024.0,
        );
        p.yaw = original.get(0, "yaw").unwrap() as f32 * std::f32::consts::TAU / 65536.0;
    }
    let inputs: Vec<Vec<Button>> = (0..original.ticks() - 1)
        .map(|tick| if jump_release && tick >= 5 { vec![] } else { buttons.to_vec() })
        .collect();
    let rebuild = Trajectory::record(&mut app, &inputs, &["x", "y", "z", "yaw", "speed"], |world| {
        let p = world.get::<Player>(entity).unwrap();
        let m = world.get::<Motion>(entity).unwrap();
        let yaw = (p.yaw * 65536.0 / std::f32::consts::TAU).round() as i32 as i16;
        vec![
            f64::from(p.position.x * 1024.0),
            f64::from(2560.0 - p.position.y * 1024.0),
            f64::from(p.position.z * 1024.0),
            f64::from(yaw),
            f64::from(m.velocity.with_y(0.0).length() * 1024.0 / 30.0),
        ]
    });
    (original, rebuild)
}

#[test]
fn original_run_matches_all_sixty_ticks() {
    let (original, rebuild) = recorded_case(include_str!("data/original/run.csv"), &[Button::Up], false);
    engine::testing::compare(&original, &rebuild, &[("x", 0.0), ("y", 0.0), ("z", 0.0), ("speed", 0.0001)])
        .unwrap()
        .assert_passed();
}

#[test]
fn original_standing_turn_matches_yaw_through_acceleration_and_wrap() {
    let (original, rebuild) = recorded_case(include_str!("data/original/turn.csv"), &[Button::Right], false);
    engine::testing::compare(&original, &rebuild, &[("x", 0.0), ("z", 0.0), ("yaw", 0.0)]).unwrap().assert_passed();
}

#[test]
fn original_standing_jump_matches_compression_flight_and_landing() {
    let (original, rebuild) = recorded_case(include_str!("data/original/jump.csv"), &[Button::Square], true);
    engine::testing::compare(&original, &rebuild, &[("x", 0.0), ("y", 0.02), ("z", 0.0)]).unwrap().assert_passed();
}

#[test]
fn heldout_left_turn_matches_without_fitting_to_recording() {
    let (original, rebuild) = recorded_case(include_str!("data/original/heldout_left.csv"), &[Button::Left], false);
    engine::testing::compare(&original, &rebuild, &[("x", 0.0), ("z", 0.0), ("yaw", 0.0)]).unwrap().assert_passed();
}

#[test]
fn heldout_walk_matches_without_fitting_to_recording() {
    let (original, rebuild) =
        recorded_case(include_str!("data/original/heldout_walk.csv"), &[Button::Up, Button::R1], false);
    engine::testing::compare(&original, &rebuild, &[("x", 0.0), ("y", 0.0), ("z", 0.0), ("speed", 0.0001)])
        .unwrap()
        .assert_passed();
}

#[cfg(feature = "baked-assets")]
#[test]
fn original_wall_entry_falls_slides_and_turns_across_every_recorded_tick() {
    use engine::testing::{Alignment, Trajectory};
    // Real decoded sectors and clips: three falls, three ramps (+z, +x, +z),
    // goal2 landing dispatch, slope-axis heading and the slide-stop clip71.
    let original =
        Trajectory::parse_psx_samples(include_str!("data/original/wall_entry.csv"), Alignment::new(2, 0)).unwrap();
    let data: Level = serde_json::from_slice(&std::fs::read("assets/levels/wall/level.json").unwrap()).unwrap();
    let clips = game::player::PlayerAnimations::from(&data);
    let starts: Vec<usize> = clips.clips.iter().map(|c| c.frame_start).collect();
    let (mut app, entity) = app();
    app.insert_resource(clips);
    app.insert_resource(WorldGeometry { data: Arc::new(data) });
    {
        let mut p = app.world_mut().get_mut::<Player>(entity).unwrap();
        p.position = Vec3::new(62976.0, -1536.0, 23040.0) / 1024.0;
        p.room = 35;
        p.mode = PlayerMode::Fall;
    }
    app.world_mut().get_mut::<Motion>(entity).unwrap().grounded = false;
    *app.world_mut().get_mut::<AnimationState>(entity).unwrap() = AnimationState::at_frame(34, 0.0);
    let inputs = vec![vec![]; original.ticks() - 1];
    let columns = ["x", "y", "z", "yaw", "animation", "animation_frame", "fall_speed"];
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
            (starts[a.animation] + a.frame.floor() as usize) as f64,
            f64::from(-m.velocity.y * 1024.0 / 30.0),
        ]
    });
    let tolerances = [
        ("x", 0.0),
        ("y", 0.02),
        ("z", 0.0),
        ("yaw", 0.0),
        ("animation", 0.0),
        ("animation_frame", 0.0),
        ("fall_speed", 0.001),
    ];
    engine::testing::compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}

#[cfg(feature = "baked-assets")]
#[test]
fn original_turn_run_grab_climb_and_wall_stop_match_every_tick() {
    // Wall, settled after the entry slide: RIGHT for fields [0,56), then
    // UP+CROSS from field 60. Covers the decaying turn rate after release,
    // running at an oblique heading, the two-click vault (clip50, end command
    // +128/-512), clip51->52 run start up a tilt-1 slope and the clip53 wall
    // stop, then repeated blocked run-start attempts. The sampled centre
    // floor precedes the wall shift, leaving Lara six units higher here.
    compare_wall_climb(include_str!("data/original/climb_two_click.csv"), &[(60, 260)]);
}

#[cfg(feature = "baked-assets")]
#[test]
fn heldout_wall_stop_release_and_retry_match_every_tick() {
    // Captured after fixing the floor sampling order. Releasing UP must
    // settle onto the resolved floor; each retry retains the proposed floor.
    compare_wall_climb(include_str!("data/original/wall_stop_heldout.csv"), &[(60, 240), (260, 300)]);
}

#[cfg(feature = "baked-assets")]
fn compare_wall_climb(csv: &str, forward_windows: &[(usize, usize)]) {
    use engine::testing::{Alignment, Trajectory};
    let original = Trajectory::parse_psx_samples(csv, Alignment::new(2, 1)).unwrap();
    let data: Level = serde_json::from_slice(&std::fs::read("assets/levels/wall/level.json").unwrap()).unwrap();
    let clips = game::player::PlayerAnimations::from(&data);
    let starts: Vec<usize> = clips.clips.iter().map(|c| c.frame_start).collect();
    let (mut app, entity) = app();
    app.insert_resource(clips);
    app.insert_resource(WorldGeometry { data: Arc::new(data) });
    {
        let mut p = app.world_mut().get_mut::<Player>(entity).unwrap();
        p.position = Vec3::new(64176.0, -5376.0, 26090.0) / 1024.0;
        p.room = 35;
    }
    *app.world_mut().get_mut::<AnimationState>(entity).unwrap() = AnimationState::at_frame(103, 45.0);
    // Tick t (0-based) reads the pad held at field 2t.
    let inputs: Vec<Vec<Button>> = (0..original.ticks() - 1)
        .map(|t| match 2 * t {
            f if f < 56 => vec![Button::Right],
            f if forward_windows.iter().any(|&(start, end)| (start..end).contains(&f)) => {
                vec![Button::Up, Button::Cross]
            }
            _ => vec![],
        })
        .collect();
    let columns = ["x", "y", "z", "yaw", "animation", "animation_frame", "speed"];
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
            (starts[a.animation] + a.frame.floor() as usize) as f64,
            f64::from((m.velocity.with_y(0.0).length() * 1024.0 / 30.0).round()),
        ]
    });
    let tolerances =
        [("x", 0.02), ("y", 0.02), ("z", 0.02), ("yaw", 0.0), ("animation", 0.0), ("animation_frame", 0.0)];
    engine::testing::compare(&original, &rebuild, &tolerances).unwrap().assert_passed();
}
