//! A real room face and its fourteen PSX leaf packets, retained independently
//! of the port. This checks texel/shade rounding and projected depth scale.
use bevy::{camera::CameraProjection, mesh::VertexAttributeValues, prelude::*};
use engine::{
    render::{
        rooms::{RoomFace, RoomMeshOutput, RoomTessellator, RoomVertex, RoomView},
        view,
    },
    world::position,
};
use game::subdivision::RoomSubdivision;
use serde::Deserialize;

#[derive(Deserialize)]
struct Reference {
    origin: [i32; 3],
    vertices: Vec<[i32; 4]>,
    uv: Vec<[i32; 2]>,
    eye: [i32; 3],
    target: [i32; 3],
    leaves: Vec<Leaf>,
}
#[derive(Deserialize)]
struct Leaf {
    vertices: Vec<Corner>,
}
#[derive(Deserialize)]
struct Corner {
    uv: [i32; 2],
    color: [u8; 3],
    view: [i32; 3],
}

#[test]
fn subdivided_room_face_matches_recorded_psx_leaf_texels_shades_and_depths() {
    let reference: Reference = serde_json::from_str(include_str!("data/original/subdivision.json")).unwrap();
    let camera = view::camera_transform(position(reference.eye), position(reference.target));
    let projection = PerspectiveProjection { fov: game::display::view_settings().fov, ..default() };
    let view = RoomView {
        room: 0,
        view_from_room: camera.to_matrix().inverse()
            * Mat4::from_scale(view::LEVEL_TO_RENDER)
            * Mat4::from_translation(position(reference.origin)),
        clip_from_view: projection.get_clip_from_view(),
        full_size: UVec2::new(384, 240),
        // Use the original texture page as a simple test atlas. Translation
        // into the baked atlas is integer texels and does not affect rounding.
        atlas_size: UVec2::splat(256),
    };
    let face = RoomFace {
        source_indices: (0..reference.vertices.len()).collect(),
        vertices: reference
            .vertices
            .iter()
            .zip(&reference.uv)
            .map(|(v, uv)| RoomVertex {
                position: IVec3::new(v[0], v[1], v[2]),
                shade: v[3] as u8,
                uv: (IVec2::from_array(*uv).as_vec2() + Vec2::splat(0.5)) / 256.0,
            })
            .collect(),
        texture_attribute: 0,
    };
    let mut output = RoomMeshOutput::default();
    RoomSubdivision { enabled: true, fillers: true }.tessellate(&face, &view, &mut output);
    assert_eq!(reference.leaves.len(), 14);
    assert_eq!(output.surface.triangle_count(), 28);
    assert_eq!(output.underlay.triangle_count(), 2);
    let mesh = output.surface.into_mesh();
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!("UVs") };
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else {
        panic!("colours")
    };
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        panic!("positions")
    };
    let mut actual = Vec::new();
    for ((uv, c), p) in uvs.iter().zip(colors).zip(positions) {
        let uv = (Vec2::from_array(*uv) * 256.0 - Vec2::splat(0.5)).round().as_ivec2().to_array();
        let shade = (Color::linear_rgba(c[0], c[1], c[2], c[3]).to_srgba().red * 128.0).round() as u8;
        actual.push((uv, shade));
        let nearest = reference
            .leaves
            .iter()
            .flat_map(|l| &l.vertices)
            .find(|v| v.uv == uv && v.color[0] == shade)
            .expect("each emitted corner must occur in a PSX leaf");
        let v = view.view_from_room.transform_point3(Vec3::from_array(*p)) * 1024.0;
        let v = Vec3::new(v.x, -v.y, -v.z);
        assert!(
            (v - IVec3::from_array(nearest.view).as_vec3()).abs().max_element() < 16.0,
            "float camera vs original quantized view: {uv:?}, {v:?}, {:?}",
            nearest.view
        );
    }
    let mut expected: Vec<_> = reference
        .leaves
        .iter()
        .flat_map(|leaf| [0, 1, 2, 1, 3, 2].map(|i| (leaf.vertices[i].uv, leaf.vertices[i].color[0])))
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "all native leaf UVs and shade bytes, including repeated triangle corners");
}
