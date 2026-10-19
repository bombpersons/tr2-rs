//! Structural animation fixtures distinguish contact keys from render/clearance
//! interpolation; no title-specific geometry is needed by the engine.
use bevy::prelude::*;
use engine::{
    animation::{collision_bounds, frame_bounds, model_spheres},
    world::{Animation, Level, Model, ObjectMesh, Pose, format::Joint},
};

#[test]
fn contact_uses_nearest_key_with_earlier_key_at_halfway() {
    let mut data = Level {
        animations: vec![Animation {
            frame_rate: 4,
            frame_start: 0,
            frame_end: 6,
            bounds: vec![[0; 6], [40; 6], [60; 6]],
            ..default()
        }],
        ..default()
    };
    assert_eq!(collision_bounds(&data, 0, 2.0), Some([0; 6]));
    assert_eq!(frame_bounds(&data, 0, 2.0), Some([20; 6]));
    assert_eq!(collision_bounds(&data, 0, 3.0), Some([40; 6]));
    // Last interval is shortened to two frames; its halfway still uses key1.
    assert_eq!(collision_bounds(&data, 0, 5.0), Some([40; 6]));
    assert_eq!(collision_bounds(&data, 0, 6.0), Some([60; 6]));
    // The source compares relative key time against the absolute end frame.
    data.animations[0].frame_start = 100;
    data.animations[0].frame_end = 106;
    assert_eq!(collision_bounds(&data, 0, 6.0), Some([40; 6]));
}

#[test]
fn sphere_joint_ids_survive_empty_meshes_and_root_transforms() {
    let data = Level {
        meshes: vec![ObjectMesh::default(), ObjectMesh { center: [1024, 0, 0], radius: 128, ..default() }],
        animations: vec![Animation { frame_rate: 1, frame_end: 1, poses: vec![Pose::default()], ..default() }],
        ..default()
    };
    let model = Model {
        meshes: vec![0, 1],
        joints: vec![Joint { parent: None, offset: [0; 3] }, Joint { parent: Some(0), offset: [0, -1024, 0] }],
        animations: [0, 1],
        ..default()
    };
    let root = Transform::from_xyz(3.0, 2.0, 5.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    let spheres = model_spheres(&data, &model, 0, 0.0, &root);
    assert_eq!(spheres.len(), 1);
    assert_eq!(spheres[0].joint, 1);
    assert!((spheres[0].center - Vec3::new(3.0, 3.0, 4.0)).length() < 0.00001);
    assert_eq!(spheres[0].radius, 0.125);
}
