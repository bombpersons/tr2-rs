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
    let clip_from_view = projection.get_clip_from_view_for_sub(&SubCameraView {
        full_size: view.full_size,
        offset: Vec2::ZERO,
        size: view.size,
    });
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

/// Other screen sizes (the display menu, `--aspect window`) keep the
/// original's picture: the same vertical view scaled by the screen's height,
/// and Hor+ widening split equally between the two sides. Every landmark
/// lands where the original's 368x240 frame puts it, scaled, and shifted by
/// the new left margin.
#[test]
fn resized_screens_keep_the_original_view_scaled_and_centred() {
    let references: References = serde_json::from_str(include_str!("data/original/view_projection.json")).unwrap();
    let view = game::display::view_settings();
    let native = view.sub_view(game::SCREEN);
    assert_eq!(native, SubCameraView { full_size: view.full_size, offset: Vec2::ZERO, size: view.size });
    assert_eq!(view.margin(&native), Vec2::ZERO);
    let projection = PerspectiveProjection { fov: view.fov, near: view.near, far: view.far, ..default() };
    for (screen, scale, margin) in [
        (UVec2::new(426, 240), 1.0, 29.0),
        (UVec2::new(736, 480), 2.0, 0.0),
        (UVec2::new(854, 480), 2.0, 29.5),
        (UVec2::new(320, 240), 1.0, -24.0),
    ] {
        let sub = view.sub_view(screen);
        assert_eq!(view.margin(&sub), Vec2::new(margin, 0.0), "{screen}");
        let clip_from_view = projection.get_clip_from_view_for_sub(&sub);
        let native_clip = projection.get_clip_from_view_for_sub(&native);
        for case in &references.cases {
            let camera = camera_transform(position(case.eye), position(case.target));
            for point in &case.landmarks {
                let at = |clip_from_view: Mat4, size: Vec2| {
                    let clip = clip_from_view
                        * camera.to_matrix().inverse()
                        * render_position(position(point.world)).extend(1.0);
                    let ndc = clip.truncate() / clip.w;
                    Vec2::new((ndc.x + 1.0) * size.x / 2.0, (1.0 - ndc.y) * size.y / 2.0)
                };
                let expected = (at(native_clip, view.size.as_vec2()) + Vec2::new(margin, 0.0)) * scale;
                let actual = at(clip_from_view, screen.as_vec2());
                assert!((actual - expected).abs().max_element() < 0.01, "{screen}: {actual} expected {expected}");
            }
        }
    }
}
