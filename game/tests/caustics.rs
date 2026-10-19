//! Native room shading observations, including a fresh controlled odd phase.
use bevy::{camera::CameraProjection, mesh::VertexAttributeValues, prelude::*};
use engine::{
    render::rooms::{RoomFace, RoomMeshOutput, RoomVertex, RoomView},
    testing::{TickApp, headless_app},
    world::{Level, Room},
};
use game::{
    caustics::{CausticTables, Caustics, CausticsPlugin, room_policy},
    subdivision::RoomSubdivision,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Observation {
    phase: u8,
    vertex: usize,
    base: u8,
    shaded: u8,
}
#[derive(Deserialize)]
struct Reference {
    native_observations: usize,
    independent: bool,
    observations: Vec<Observation>,
}

#[test]
fn vertex_shades_match_original_calibration_and_fresh_odd_phase() {
    let tables = CausticTables::default();
    for (text, independent) in [
        (include_str!("data/original/caustics_calibration.json"), false),
        (include_str!("data/original/caustics_heldout.json"), true),
    ] {
        let reference: Reference = serde_json::from_str(text).unwrap();
        assert_eq!(reference.independent, independent);
        assert_eq!(reference.native_observations, 628);
        assert!(reference.observations.len() > 200);
        for observation in reference.observations {
            assert_eq!(
                tables.shade(observation.vertex, observation.base, observation.phase),
                observation.shaded,
                "vertex {} phase {} base {}",
                observation.vertex,
                observation.phase,
                observation.base
            );
        }
    }
}

#[test]
fn wave_clock_matches_140_native_ticks_across_wraps() {
    for (text, initial) in [
        (include_str!("data/original/caustics_phase_calibration.csv"), 0),
        (include_str!("data/original/caustics_phase_heldout.csv"), 17),
    ] {
        let values: Vec<u8> =
            text.lines().skip(1).map(|line| line.rsplit(',').next().unwrap().parse().unwrap()).collect();
        let mut app = headless_app(game::tick_rate(game::REGION));
        app.add_plugins(CausticsPlugin);
        app.run_tick(&[]);
        let entity = app.world_mut().query_filtered::<Entity, With<Caustics>>().single(app.world()).unwrap();
        app.world_mut().get_mut::<Caustics>(entity).unwrap().phase = initial;
        for tick in 1..=70 {
            app.run_tick(&[]);
            assert_eq!(app.world().get::<Caustics>(entity).unwrap().phase, values[2 * tick], "tick {tick}");
        }
    }
}

#[test]
fn room_policy_shades_wet_corners_and_preserves_dry_corners() {
    let reference: Reference = serde_json::from_str(include_str!("data/original/caustics_calibration.json")).unwrap();
    let corners = &reference.observations[..3];
    let face = RoomFace {
        vertices: corners
            .iter()
            .map(|corner| RoomVertex { position: IVec3::ZERO, uv: Vec2::ZERO, shade: corner.base })
            .collect(),
        source_indices: corners.iter().map(|corner| corner.vertex).collect(),
        texture_attribute: 0,
    };
    let level = Level { rooms: vec![Room::default(), Room { water: true, ..default() }], ..default() };
    let policy = room_policy(
        RoomSubdivision { enabled: false, fillers: true },
        Caustics { enabled: true, phase: corners[0].phase },
        &level,
        CausticTables::default(),
    );
    for room in 0..2 {
        let view = RoomView {
            room,
            view_from_room: Mat4::IDENTITY,
            clip_from_view: PerspectiveProjection::default().get_clip_from_view(),
            atlas_size: UVec2::splat(256),
            full_size: UVec2::new(384, 240),
            margin: Vec2::ZERO,
        };
        let mut output = RoomMeshOutput::default();
        policy.0.tessellate(&face, &view, &mut output);
        let mesh = output.surface.into_mesh();
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else {
            panic!("room colors")
        };
        for (color, corner) in colors.iter().zip(corners) {
            let encoded = Color::linear_rgba(color[0], color[1], color[2], color[3]).to_srgba().red;
            assert_eq!((encoded * 128.0).round() as u8, if room == 1 { corner.shaded } else { corner.base });
        }
    }
}
