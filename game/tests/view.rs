//! Project decoded room landmarks against the original's measured view matrices.
use bevy::{
    camera::{CameraProjection, SubCameraView},
    prelude::*,
};
use engine::{
    render::view::{camera_transform, render_position},
    world::position,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct References {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    checkpoint: String,
    eye: [i32; 3],
    target: [i32; 3],
    landmarks: Vec<Landmark>,
}
#[derive(Deserialize)]
struct Landmark {
    world: [i32; 3],
    screen: [f32; 2],
}

#[test]
fn rebuilt_view_projects_room_landmarks_like_four_original_camera_snapshots() {
    let references: References = serde_json::from_str(include_str!("data/original/view_projection.json")).unwrap();
    let view = game::display::view_settings();
    let projection = PerspectiveProjection { fov: view.fov, near: view.near, far: view.far, ..default() };
    let clip_from_view =
        projection.get_clip_from_view_for_sub(&SubCameraView { full_size: view.full_size, offset: Vec2::ZERO, size: view.size });
    let mut checked = 0;
    for case in references.cases {
        let camera = camera_transform(position(case.eye), position(case.target));
        let clip_from_world = clip_from_view * camera.to_matrix().inverse();
        for point in case.landmarks {
            let clip = clip_from_world * render_position(position(point.world)).extend(1.0);
            let ndc = clip.truncate() / clip.w;
            let screen = Vec2::new((ndc.x + 1.0) * 184.0, (1.0 - ndc.y) * 120.0);
            // The original quantizes view angles/matrix coefficients; Bevy uses
            // floating-point look_at. Keep that measured discrepancy bounded.
            let error = (screen - Vec2::from_array(point.screen)).abs().max_element();
            assert!(
                error < 1.0,
                "{} {:?}: {:?} expected {:?}, error {}px",
                case.checkpoint,
                point.world,
                screen,
                point.screen,
                error
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 64);
}
