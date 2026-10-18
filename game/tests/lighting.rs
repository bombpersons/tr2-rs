//! Room lights checked against fresh native CalculateLight traces.
#![cfg(feature = "baked-assets")]

use bevy::prelude::*;
use engine::{
    lighting::{ItemLight, item_light},
    world::Level,
};

#[test]
fn item_lighting_matches_original_home_and_wall_calculations() {
    let references: serde_json::Value = serde_json::from_str(include_str!("data/original/item_lighting.json")).unwrap();
    for case in references["cases"].as_array().unwrap() {
        let name = case["level"].as_str().unwrap();
        let level: Level =
            serde_json::from_slice(&std::fs::read(format!("assets/levels/{name}/level.json")).unwrap()).unwrap();
        let point = IVec3::new(
            case["point"][0].as_i64().unwrap() as i32,
            case["point"][1].as_i64().unwrap() as i32,
            case["point"][2].as_i64().unwrap() as i32,
        );
        let light = item_light(&level, case["room"].as_u64().unwrap() as usize, point);
        assert_eq!(light.ambient, case["ambient"].as_i64().unwrap() as i32, "{name}");
        assert_eq!(light.strength, case["strength"].as_i64().unwrap() as i32, "{name}");
    }
}

#[test]
fn directional_shade_matches_original_home_and_heldout_wall_vertices() {
    let references: serde_json::Value = serde_json::from_str(include_str!("data/original/item_lighting.json")).unwrap();
    let vector = |v: &serde_json::Value| {
        Vec3::new(v[0].as_i64().unwrap() as f32, v[1].as_i64().unwrap() as f32, v[2].as_i64().unwrap() as f32) / 16384.0
    };
    for case in references["cases"].as_array().unwrap() {
        let columns = &case["mesh_matrix_columns"];
        let rotation = Mat3::from_cols(vector(&columns[0]), vector(&columns[1]), vector(&columns[2]));
        let light = ItemLight {
            ambient: case["ambient"].as_i64().unwrap() as i32,
            strength: case["strength"].as_i64().unwrap() as i32,
            direction: vector(&case["view_light_direction"]),
        };
        let vertices = case["vertices"].as_array().unwrap();
        assert!(!vertices.is_empty());
        for (i, vertex) in vertices.iter().enumerate() {
            let shade = light.shade(rotation * vector(&vertex["normal"])) * 128.0;
            let original = vertex["shade"].as_f64().unwrap() as f32;
            // Float normals/lighting replace integer matrix division and
            // shade-byte truncation. Bound the difference to one shade byte.
            assert!((shade - original).abs() <= 1.0, "{} vertex {i}: {shade} vs {original}", case["level"]);
        }
    }
}
