//! Sky mesh projection checked against the supplied disc's GPU capture.
#![cfg(feature = "baked-assets")]
use bevy::prelude::*;
use engine::{
    animation::model_pose,
    render::view,
    world::{Level, position},
};

fn compare_projection(reference: &str) {
    let level: Level = serde_json::from_slice(&std::fs::read("assets/levels/assault/level.json").unwrap()).unwrap();
    let reference: serde_json::Value = serde_json::from_str(reference).unwrap();
    let model = level.models.iter().find(|m| m.id == reference["model"].as_u64().unwrap() as u16).unwrap();
    let rotation = model_pose(&level, model, model.animations[0], 0.0)[0].rotation;
    let point = |key: &str| {
        let a = reference["camera"][key].as_array().unwrap();
        Vec3::new(a[0].as_f64().unwrap() as f32, -a[1].as_f64().unwrap() as f32, a[2].as_f64().unwrap() as f32) / 1024.0
    };
    let camera = view::camera_transform(point("eye"), point("target"));
    let vertices: Vec<_> = level.meshes[model.meshes[0]]
        .vertices
        .iter()
        .map(|v| {
            let p = view::render_position(rotation * position([v[0], v[1], v[2]]));
            camera.rotation.inverse() * p * 1024.0
        })
        .collect();
    // Each GPU corner must be an authored vertex after the rotation-only
    // transform. GTE matrix quantization accounts for at most a few units.
    for command in reference["textured_commands"].as_array().unwrap() {
        for corner in command["vertices"].as_array().unwrap() {
            let a = corner["view"].as_array().unwrap();
            let measured = Vec3::new(
                a[0].as_f64().unwrap() as f32,
                -a[1].as_f64().unwrap() as f32,
                -a[2].as_f64().unwrap() as f32,
            );
            let predicted = vertices
                .iter()
                .min_by(|a, b| a.distance_squared(measured).total_cmp(&b.distance_squared(measured)))
                .unwrap();
            assert!(predicted.distance(measured) < 6.0, "{predicted:?} vs {measured:?}");
            let screen = Vec2::new(predicted.x, -predicted.y) * (228.0 / -predicted.z) + Vec2::new(192.0, 120.0);
            let recorded =
                Vec2::new(corner["x"].as_f64().unwrap() as f32, corner["y"].as_f64().unwrap() as f32 - 256.0);
            assert!(screen.distance(recorded) < 2.0, "{screen:?} vs {recorded:?}");
        }
    }
}

#[test]
fn sky_rotation_and_geometry_match_the_original_projected_cloud_bands() {
    compare_projection(include_str!("data/original/home_sky.json"));
}

#[test]
fn heldout_sky_cloud_bands_follow_a_new_original_camera_orientation() {
    compare_projection(include_str!("data/original/home_sky_turn.json"));
}
