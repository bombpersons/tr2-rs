//! Animated item contact replays from the supplied PSX disc. Both actors run
//! their real rules; no positions or clip choices are injected after tick0.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    animation::{AnimationGoal, AnimationState, ObjectAnimation},
    objects::{Initialized, WorldObject},
    testing::{Alignment, Replay, TickApp, headless_app},
    world::{Level, WorldGeometry, position},
};
use game::{
    player::{Motion, Player, PlayerAnimations, PlayerMode, PlayerRulesPlugin, Traversal},
    winston::{Winston, WinstonRulesPlugin},
};

fn compare_case(csv: &str, metadata: &str) {
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/assault/level.json")).unwrap();
    let headers: Vec<_> = csv.lines().next().unwrap().split(',').collect();
    let rows: Vec<Vec<i32>> = csv.lines().skip(1).map(|l| l.split(',').map(|v| v.parse().unwrap()).collect()).collect();
    let at = |row: &[i32], key: &str| row[headers.iter().position(|&h| h == key).unwrap()];
    let pos = |row: &[i32], prefix: &str| {
        position([at(row, &format!("{prefix}_x")), at(row, &format!("{prefix}_y")), at(row, &format!("{prefix}_z"))])
    };
    let start = &rows[0];
    let angle = |v: i32| v as f32 * std::f32::consts::TAU / 65536.0;
    let mut app = headless_app(game::tick_rate(game::REGION));
    app.add_plugins((PlayerRulesPlugin, WinstonRulesPlugin));
    app.insert_resource(PlayerAnimations::from(&data));
    let clip = at(start, "lara_animation") as usize;
    let lara = app
        .world_mut()
        .spawn((
            Player::at(pos(start, "lara"), angle(at(start, "lara_yaw")), at(start, "lara_room") as usize),
            AnimationState::at_frame(
                clip,
                (at(start, "lara_animation_frame") - data.animations[clip].frame_start as i32) as f32,
            ),
        ))
        .id();
    if headers.contains(&"lara_movement_heading") {
        app.world_mut().get_mut::<Motion>(lara).unwrap().movement_heading =
            Some(angle(at(start, "lara_movement_heading")));
    }
    let clip = at(start, "actor_animation") as usize;
    let actor = app
        .world_mut()
        .spawn((
            Initialized,
            WorldObject::new(9, 260, at(start, "actor_room") as usize, 0),
            Transform::from_translation(pos(start, "actor"))
                .with_rotation(Quat::from_rotation_y(angle(at(start, "actor_yaw")))),
            Winston {
                speed: at(start, "actor_speed"),
                contact_mask: at(start, "actor_contact_mask") as u32,
                ..default()
            },
            ObjectAnimation {
                animation: clip,
                frame: (at(start, "actor_animation_frame") - data.animations[clip].frame_start as i32) as f32,
            },
            AnimationGoal { state: at(start, "actor_goal") as usize },
        ))
        .id();
    app.insert_resource(WorldGeometry::new(data));
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let replay = Replay::parse(&metadata["input"].to_string()).unwrap();
    let inputs = replay.tick_inputs(Alignment::new(2, 0), (rows.len() - 1) / 2).unwrap();
    for (row, input) in rows.iter().skip(2).step_by(2).zip(inputs) {
        app.run_tick(&input);
        let world = app.world();
        let player = world.get::<Player>(lara).unwrap();
        let pose = world.get::<AnimationState>(lara).unwrap();
        let motion = world.get::<Motion>(lara).unwrap();
        let pclip = &world.resource::<WorldGeometry>().data.animations[pose.animation];
        let actor_pose = world.get::<ObjectAnimation>(actor).unwrap();
        let transform = world.get::<Transform>(actor).unwrap();
        let actor_state = world.get::<Winston>(actor).unwrap();
        let aclip = &world.resource::<WorldGeometry>().data.animations[actor_pose.animation];
        for (name, actual) in [
            ("lara_x", player.position.x * 1024.0),
            ("lara_y", -player.position.y * 1024.0),
            ("lara_z", player.position.z * 1024.0),
            ("lara_yaw", (player.yaw * 65536.0 / std::f32::consts::TAU).round()),
            ("lara_animation", pose.animation as f32),
            ("lara_animation_frame", pclip.frame_start as f32 + pose.frame),
            ("lara_state", pclip.state as f32),
            ("lara_goal", world.get::<Traversal>(lara).unwrap().goal as f32),
            ("lara_room", player.room as f32),
            (
                "lara_speed",
                (if matches!(player.mode, PlayerMode::JumpingUp { .. }) {
                    motion.velocity.dot(player.forward())
                } else {
                    motion.velocity.with_y(0.0).length()
                } * 1024.0
                    / 30.0)
                    .round(),
            ),
            ("actor_x", transform.translation.x * 1024.0),
            ("actor_y", -transform.translation.y * 1024.0),
            ("actor_z", transform.translation.z * 1024.0),
            ("actor_animation", actor_pose.animation as f32),
            ("actor_animation_frame", aclip.frame_start as f32 + actor_pose.frame),
            ("actor_state", aclip.state as f32),
            ("actor_goal", world.get::<AnimationGoal>(actor).unwrap().state as f32),
            ("actor_contact_mask", actor_state.contact_mask as f32),
        ] {
            assert!(
                (actual - at(row, name) as f32).abs() <= if name == "lara_y" { 0.02 } else { 0.0 },
                "field{} {name}: port {actual}, PSX {}",
                at(row, "run_frame"),
                at(row, name)
            );
        }
    }
}

#[test]
fn original_backward_airborne_contact_preserves_the_authored_hop_and_landing() {
    compare_case(
        include_str!("data/original/winston_contact_back.csv"),
        include_str!("data/original/winston_contact_back.json"),
    );
}

#[test]
fn heldout_backward_walk_contacts_keep_walking_without_stagger() {
    compare_case(
        include_str!("data/original/winston_contact_walk_heldout.csv"),
        include_str!("data/original/winston_contact_walk_heldout.json"),
    );
}

#[test]
fn heldout_delayed_airborne_contact_uses_the_current_animation_box() {
    compare_case(
        include_str!("data/original/winston_contact_air_delayed_heldout.csv"),
        include_str!("data/original/winston_contact_air_delayed_heldout.json"),
    );
}

#[test]
fn heldout_walking_release_preserves_collision_correction_through_recovery() {
    compare_case(
        include_str!("data/original/winston_contact_walk_release_heldout.csv"),
        include_str!("data/original/winston_contact_walk_release_heldout.json"),
    );
}

#[test]
fn ordinary_skeletal_math_bounds_the_four_native_sphere_clouds() {
    use engine::animation::model_spheres;
    let data: Level = serde_json::from_slice(include_bytes!("../assets/levels/assault/level.json")).unwrap();
    let native: serde_json::Value = serde_json::from_str(include_str!("data/original/winston_spheres.json")).unwrap();
    for case in native["cases"].as_array().unwrap() {
        let mut clouds = Vec::new();
        for call in case["calls"].as_array().unwrap() {
            let kind = call["kind"].as_u64().unwrap() as u16;
            let model = data.model(kind).unwrap();
            let animation = call["animation"].as_u64().unwrap() as usize;
            let frame = call["absolute_frame"].as_u64().unwrap() as f32 - data.animations[animation].frame_start as f32;
            let coordinates = std::array::from_fn(|i| call["position"][i].as_i64().unwrap() as i32);
            let yaw = call["yaw"].as_i64().unwrap() as f32 * std::f32::consts::TAU / 65536.0;
            let root = Transform::from_translation(position(coordinates)).with_rotation(Quat::from_rotation_y(yaw));
            let spheres = model_spheres(&data, model, animation, frame, &root);
            let reference = call["spheres"].as_array().unwrap();
            assert_eq!(spheres.len(), reference.len());
            let mut maximum = 0.0_f32;
            for sphere in &spheres {
                let expected = &reference[sphere.joint];
                let actual = [sphere.center.x * 1024.0, -sphere.center.y * 1024.0, sphere.center.z * 1024.0];
                for i in 0..3 {
                    maximum = maximum.max((actual[i] - expected[i].as_i64().unwrap() as f32).abs());
                }
                assert_eq!(sphere.radius * 1024.0, expected[3].as_i64().unwrap() as f32);
            }
            println!("{} field{} max sphere-center error {maximum} source units", kind, call["run_field"]);
            // Modern bone transforms deliberately do not reproduce the GTE's
            // intermediate matrix truncation. Four units is1/256 world unit.
            assert!(maximum <= 4.0, "kind{kind}: error {maximum}");
            clouds.push(spheres);
        }
        let mask = clouds[0]
            .iter()
            .filter(|a| clouds[1].iter().any(|b| a.center.distance_squared(b.center) < (a.radius + b.radius).powi(2)))
            .fold(0_u32, |mask, a| mask | (1 << a.joint));
        let field = case["calls"][0]["run_field"].as_u64().unwrap();
        assert_eq!(mask, if field == 39 { 0x10000 } else { 0x110000 });
    }
}
