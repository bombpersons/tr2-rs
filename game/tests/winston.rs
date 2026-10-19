//! Local follow trajectories from ordinary PSX Home replays. Lara follows
//! recorded positions so this isolates the item controller from avatar rules;
//! each item tick reads the preceding avatar pose, as ControlPhase does.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, ObjectAnimation},
    objects::{Initialized, WorldObject},
    testing::{TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use game::{
    player::Player,
    winston::{Winston, WinstonRulesPlugin},
};

fn compare_case(csv: &str) {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/assault/level.json")).unwrap();
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins(WinstonRulesPlugin);
    let rows: Vec<Vec<i32>> = csv.lines().skip(1).map(|l| l.split(',').map(|v| v.parse().unwrap()).collect()).collect();
    let headers: Vec<_> = csv.lines().next().unwrap().split(',').collect();
    let at = |row: &[i32], key: &str| row[headers.iter().position(|&s| s == key).unwrap()];
    let pos = |row: &[i32], prefix: &str| {
        position([at(row, &format!("{prefix}_x")), at(row, &format!("{prefix}_y")), at(row, &format!("{prefix}_z"))])
    };
    let first = &rows[0];
    let clip = at(first, "actor_animation") as usize;
    let rotation = |v: i32| Quat::from_rotation_y(v as f32 * std::f32::consts::TAU / 65536.0);
    let actor = app
        .world_mut()
        .spawn((
            Initialized,
            WorldObject::new(9, 260, at(first, "actor_room") as usize, 0),
            Transform::from_translation(pos(first, "actor")).with_rotation(rotation(at(first, "actor_yaw"))),
            Winston { speed: at(first, "actor_speed") },
            ObjectAnimation {
                animation: clip,
                frame: (at(first, "actor_animation_frame") - data.animations[clip].frame_start as i32) as f32,
            },
            AnimationGoal { state: at(first, "actor_goal") as usize },
        ))
        .id();
    let player = app.world_mut().spawn(Player::at(pos(first, "lara"), 0.0, at(first, "lara_room") as usize)).id();
    app.insert_resource(WorldGeometry::new(data));
    let mut previous = first;
    for row in rows.iter().skip(2).step_by(2) {
        app.world_mut().get_mut::<Player>(player).unwrap().position = pos(previous, "lara");
        app.run_tick(&[]);
        let transform = app.world().get::<Transform>(actor).unwrap();
        let a = app.world().get::<ObjectAnimation>(actor).unwrap();
        let goal = app.world().get::<AnimationGoal>(actor).unwrap();
        let w = app.world().get::<Winston>(actor).unwrap();
        let clip = &app.world().resource::<WorldGeometry>().data.animations[a.animation];
        let yaw = (transform.rotation.to_euler(EulerRot::YXZ).0 * 65536.0 / std::f32::consts::TAU).round() as i32 as i16
            as i32;
        let actual = [
            (transform.translation.x * 1024.0).round() as i32,
            (-transform.translation.y * 1024.0).round() as i32,
            (transform.translation.z * 1024.0).round() as i32,
            yaw,
            a.animation as i32,
            clip.frame_start as i32 + a.frame as i32,
            clip.state as i32,
            goal.state as i32,
            w.speed,
            app.world().get::<WorldObject>(actor).unwrap().room as i32,
        ];
        let expected = ["x", "y", "z", "yaw", "animation", "animation_frame", "state", "goal", "speed", "room"]
            .map(|k| at(row, &format!("actor_{k}")));
        assert_eq!(actual, expected, "native field {}", at(row, "run_frame"));
        previous = row;
    }
}

#[test]
fn original_local_run_follow_start_and_complete_stop() {
    compare_case(include_str!("data/original/winston_local_run.csv"));
}

#[test]
fn heldout_short_run_settles_after_authored_deceleration() {
    compare_case(include_str!("data/original/winston_heldout_short.csv"));
}

#[test]
fn heldout_diagonal_follow_uses_previous_avatar_pose() {
    compare_case(include_str!("data/original/winston_heldout_turn.csv"));
}

#[test]
fn heldout_walking_departure_starts_and_stops_the_attendant() {
    compare_case(include_str!("data/original/winston_heldout_walk.csv"));
}
