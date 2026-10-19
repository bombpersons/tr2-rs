//! TR2's room-only tessellation rule, verified from the supplied PSX disc.
//! See docs/subdivision-evidence.md: 0x800617b0 chooses depth bands and
//! 0x800620dc / 0x80062474 split positions before projection. Object meshes
//! use another renderer and are deliberately outside this plugin.
use bevy::prelude::*;
use engine::render::{
    LevelSystems,
    rooms::{RoomFace, RoomMeshBuilder, RoomMeshOutput, RoomMeshPolicy, RoomTessellator, RoomVertex, RoomView},
};

/// Developer switches for comparing the same paused view through MCP.
#[derive(Component, Reflect, Clone, Copy)]
#[reflect(Component)]
pub struct RoomSubdivision {
    pub enabled: bool,
    pub fillers: bool,
}

impl Default for RoomSubdivision {
    fn default() -> Self {
        Self { enabled: true, fillers: true }
    }
}

pub struct SubdivisionPlugin;

impl Plugin for SubdivisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::caustics::CausticsPlugin)
            .insert_resource(RoomMeshPolicy(Box::new(RoomSubdivision::default())))
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((Name::new("RoomSubdivision"), RoomSubdivision::default()));
            })
            .add_systems(Update, sync_controls.after(LevelSystems::Visibility).before(LevelSystems::Meshes));
    }
}

fn sync_controls(
    controls: Query<Ref<RoomSubdivision>>,
    effects: Query<Ref<crate::caustics::Caustics>>,
    tables: Res<crate::caustics::CausticTables>,
    world: Option<Res<engine::world::WorldGeometry>>,
    mut policy: ResMut<RoomMeshPolicy>,
) {
    let (Some(world), Ok(effects)) = (world, effects.single()) else { return };
    for controls in &controls {
        if controls.is_changed() || effects.is_changed() || world.is_changed() {
            *policy = crate::caustics::room_policy(*controls, *effects, &world.data, tables.clone());
        }
    }
}

/// Source-world units per Bevy world unit (engine::world::position).
/// 0x80062e58 divides Q14 matrix coefficients by four for the GTE's Q12
/// rotation, so SZ retains source-world scale, not a quarter-scale distance.
const DEPTH_UNITS: f32 = 1024.0;
/// Room max-SZ tests at 0x80061a14 and 0x80061a78 (triangle: 0x80061d40).
const SPLIT_FAR: f32 = 5000.0;
const SPLIT_NEAR: f32 = 2500.0;

#[derive(Clone, Copy, Default)]
struct Projected {
    screen: Vec2,
    depth: f32,
    error: bool,
}

fn project(vertex: RoomVertex, view: &RoomView) -> Projected {
    let v = view.view_position(vertex) * DEPTH_UNITS;
    let depth = (-v.z).floor().clamp(0.0, 65535.0);
    let focal = view.clip_from_view.y_axis.y * view.full_size.y as f32 * 0.5;
    // The geometric limits that set the RTPS error flag: divide overflow
    // at SZ <= H/2, signed view-coordinate overflow, or screen saturation.
    // Bevy keeps its float camera; fixed-point GTE rounding is not emulated.
    let screen = Vec2::new(v.x, -v.y) * (focal / depth.max(0.0001)).min(2.0) + view.full_size.as_vec2() * 0.5;
    let error = depth <= focal * 0.5
        || v.x < -32768.0
        || v.x > 32767.0
        || v.y < -32768.0
        || v.y > 32767.0
        || screen.min_element() < -1024.0
        || screen.max_element() > 1023.0;
    Projected { screen: screen.clamp(Vec2::splat(-1024.0), Vec2::splat(1023.0)), depth, error }
}

fn projections(vertices: &[RoomVertex], view: &RoomView) -> [Projected; 4] {
    let mut result = [Projected::default(); 4];
    for (out, &vertex) in result.iter_mut().zip(vertices) {
        *out = project(vertex, view);
    }
    result
}

fn front_facing(p: &[Projected]) -> bool {
    // NCLIP uses the last three quad corners in GPU strip order; these are
    // cyclic corners 1,3,2. The triangle loop accepts the opposite NCLIP
    // sign (0x80061cd4), which is the same cyclic winding in engine data.
    let p = if p.len() == 4 { &p[1..] } else { p };
    (p[1].screen - p[0].screen).perp_dot(p[2].screen - p[0].screen) > 0.0
}

fn outside(p: &[Projected], view: &RoomView) -> bool {
    // 0x80061fc4 rejects only when all corners lie outside the same edge.
    // The edges are the original's projection window, widened by the margin
    // a wider screen draws beyond it (zero at the original's size).
    let (min, max) = (-view.margin, view.full_size.as_vec2() + view.margin);
    p.iter().all(|p| p.screen.x < min.x)
        || p.iter().all(|p| p.screen.y < min.y)
        || p.iter().all(|p| p.screen.x >= max.x)
        || p.iter().all(|p| p.screen.y >= max.y)
}

/// Integer UV-byte interpolation survives atlas translation: the importer
/// puts every original texel at an integer atlas position plus half a texel.
fn uv_midpoint(a: Vec2, b: Vec2, atlas: UVec2) -> Vec2 {
    let size = atlas.as_vec2();
    (((a + b) * size * 0.5 - Vec2::splat(0.5)).floor() + Vec2::splat(0.5)) / size
}

fn midpoint(a: RoomVertex, b: RoomVertex, atlas: UVec2) -> RoomVertex {
    RoomVertex {
        // Arithmetic shifts round down, including negative source Y.
        position: (a.position + b.position) >> 1,
        uv: uv_midpoint(a.uv, b.uv, atlas),
        // The carry-masked packed RGB average is per-channel floor average.
        shade: ((u16::from(a.shade) + u16::from(b.shade)) >> 1) as u8,
    }
}

fn emit(vertices: &[RoomVertex], mesh: &mut RoomMeshBuilder) {
    if vertices.len() == 4 {
        // GPU GT4 strip 0,1,3,2: split on cyclic diagonal 1--3, rather than
        // the static engine builder's 0--2 diagonal.
        mesh.triangle([vertices[0], vertices[1], vertices[3]]);
        mesh.triangle([vertices[1], vertices[2], vertices[3]]);
    } else {
        mesh.triangle([vertices[0], vertices[1], vertices[2]]);
    }
}

fn split(vertices: &[RoomVertex], levels: u8, near_parent: bool, view: &RoomView, mesh: &mut RoomMeshBuilder) {
    if levels == 0 {
        let projected = projections(vertices, view);
        let p = &projected[..vertices.len()];
        // Leaf rejection at 0x800623a8 / 0x8006261c. A near-plane parent
        // skips its own winding test, so each surviving leaf tests winding.
        if p.iter().any(|p| p.error) || (near_parent && !front_facing(p)) || outside(p, view) {
            return;
        }
        emit(vertices, mesh);
        return;
    }
    let [a, b, c] = [vertices[0], vertices[1], vertices[2]];
    let ab = midpoint(a, b, view.atlas_size);
    let bc = midpoint(b, c, view.atlas_size);
    if vertices.len() == 4 {
        let d = vertices[3];
        let cd = midpoint(c, d, view.atlas_size);
        let da = midpoint(d, a, view.atlas_size);
        let centre = RoomVertex {
            // The position centre is a direct four-corner average; the
            // colour centre averages opposite edge averages (0x8006223c).
            position: (a.position + b.position + c.position + d.position) >> 2,
            uv: uv_midpoint(a.uv, c.uv, view.atlas_size),
            shade: midpoint(da, bc, view.atlas_size).shade,
        };
        for child in [[a, ab, centre, da], [ab, b, bc, centre], [centre, bc, c, cd], [da, centre, cd, d]] {
            split(&child, levels - 1, near_parent, view, mesh);
        }
    } else {
        let ca = midpoint(c, a, view.atlas_size);
        for child in [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]] {
            split(&child, levels - 1, near_parent, view, mesh);
        }
    }
}

impl RoomTessellator for RoomSubdivision {
    fn tessellate(&self, face: &RoomFace, view: &RoomView, output: &mut RoomMeshOutput) {
        let vertices = &face.vertices;
        if !self.enabled {
            emit(vertices, &mut output.surface);
            return;
        }
        let projected = projections(vertices, view);
        let p = &projected[..vertices.len()];
        let max_depth = p.iter().map(|p| p.depth).fold(0.0, f32::max);
        let near_parent = p.iter().any(|p| p.error);
        // Quad guard at 0x80061974; the triangle loop tests only all-zero
        // SZ (0x80061cb8). Leaves get the stricter RTPS test in either case.
        let minimum = if vertices.len() == 4 { 128.0 } else { 1.0 };
        if (near_parent && max_depth < minimum) || (!near_parent && !front_facing(p)) || outside(p, view) {
            return;
        }
        if max_depth >= SPLIT_FAR {
            emit(vertices, &mut output.surface);
            return;
        }
        let levels = if near_parent {
            3
        } else if max_depth < SPLIT_NEAR {
            2
        } else {
            1
        };
        split(vertices, levels, near_parent, view, &mut output.surface);
        // 0x80061aa4..0x80061ab8: uv3's pad byte is the low byte of the
        // texture attribute already retained by the importer. The flag's
        // broader meaning is still unknown; only this zero test is ported.
        if self.fillers && !near_parent && max_depth >= 512.0 && face.texture_attribute & 0xff == 0 {
            emit(vertices, &mut output.underlay);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::CameraProjection;

    fn view() -> RoomView {
        let projection = PerspectiveProjection { fov: 2.0 * (120.0_f32 / 228.0).atan(), ..default() };
        RoomView {
            room: 0,
            view_from_room: Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0)),
            clip_from_view: projection.get_clip_from_view(),
            atlas_size: UVec2::splat(256),
            full_size: UVec2::new(384, 240),
            margin: Vec2::ZERO,
        }
    }

    fn quad(depth: i32) -> RoomFace {
        RoomFace {
            source_indices: vec![0, 1, 2, 3],
            vertices: [(-128, -128, 0, 0), (128, -128, 63, 0), (128, 128, 63, 63), (-128, 128, 0, 63)]
                .map(|(x, y, u, v)| RoomVertex {
                    position: IVec3::new(x, y, depth),
                    uv: (Vec2::new(u as f32, v as f32) + Vec2::splat(0.5)) / 256.0,
                    shade: 128,
                })
                .to_vec(),
            texture_attribute: 0,
        }
    }

    #[test]
    fn a_wider_screen_widens_the_off_screen_rejection_by_its_margin() {
        // Corners right of the original's 384-pixel projection window.
        let corners = [400.0, 410.0, 405.0].map(|x| Projected { screen: Vec2::new(x, 100.0), depth: 1000.0, error: false });
        assert!(outside(&corners, &view()), "rejected on the original's screen");
        assert!(!outside(&corners, &RoomView { margin: Vec2::new(29.0, 0.0), ..view() }), "drawn on a 426-wide screen");
        let left = corners.map(|p| Projected { screen: Vec2::new(-p.screen.x + 384.0 - 30.0, 100.0), ..p });
        assert!(outside(&left, &RoomView { margin: Vec2::new(29.0, 0.0), ..view() }), "beyond the widened left edge");
    }

    fn render(face: &RoomFace, view: &RoomView) -> RoomMeshOutput {
        let mut output = RoomMeshOutput::default();
        RoomSubdivision { enabled: true, fillers: true }.tessellate(face, view, &mut output);
        output
    }

    #[test]
    fn max_depth_bands_split_quads_and_triangles_at_the_original_boundaries() {
        for (depth, quads) in [(2499, 16), (2500, 4), (4999, 4), (5000, 1)] {
            let mut face = quad(depth);
            assert_eq!(render(&face, &view()).surface.triangle_count(), 2 * quads, "depth {depth}");
            face.vertices.pop();
            assert_eq!(render(&face, &view()).surface.triangle_count(), quads, "triangle depth {depth}");
        }
        let mut face = quad(1000);
        face.vertices[2].position.z = 5000;
        assert_eq!(render(&face, &view()).surface.triangle_count(), 2, "farthest corner chooses the parent band");
    }

    #[test]
    fn integer_midpoints_keep_uv_texels_and_round_negative_positions_down() {
        let face = quad(1000);
        let mut a = face.vertices[0];
        a.position = IVec3::new(-3, -5, 1000);
        a.shade = 24;
        let mut b = face.vertices[2];
        b.position = IVec3::new(0, 0, 1001);
        b.shade = 25;
        let half = midpoint(a, b, view().atlas_size);
        assert_eq!(half.position, IVec3::new(-2, -3, 1000));
        assert_eq!(half.uv * 256.0 - Vec2::splat(0.5), Vec2::splat(31.0));
        assert_eq!(half.shade, 24);
        let quarter = midpoint(a, half, view().atlas_size);
        assert_eq!(quarter.uv * 256.0 - Vec2::splat(0.5), Vec2::splat(15.0));
        let eighth = midpoint(a, quarter, view().atlas_size);
        assert_eq!(eighth.uv * 256.0 - Vec2::splat(0.5), Vec2::splat(7.0));
    }

    #[test]
    fn near_camera_leaves_are_subdivided_then_rejected_instead_of_bevy_clipped() {
        let mut face = quad(1024);
        face.vertices[0].position.z = 0;
        face.vertices[1].position.z = 0;
        let output = render(&face, &view());
        assert!(output.surface.triangle_count() > 32 && output.surface.triangle_count() < 128);
        assert_eq!(output.underlay.triangle_count(), 0, "near-plane parents never get filler");
        let mesh = output.surface.into_mesh();
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("positions");
        };
        assert!(
            positions.iter().all(|p| p[2] * 1024.0 > 114.0),
            "no emitted leaf crosses the divide-overflow boundary"
        );
        let behind = quad(-100);
        assert_eq!(render(&behind, &view()).surface.triangle_count(), 0);
    }

    #[test]
    fn filler_respects_the_depth_and_texture_flag_and_camera_motion_changes_the_band() {
        let mut face = quad(2000);
        assert_eq!(render(&face, &view()).underlay.triangle_count(), 2);
        face.texture_attribute = 1;
        assert_eq!(render(&face, &view()).underlay.triangle_count(), 0);
        face.texture_attribute = 0x100;
        assert_eq!(render(&face, &view()).underlay.triangle_count(), 2, "only the low attribute byte is tested");
        assert_eq!(render(&quad(511), &view()).underlay.triangle_count(), 0);
        let mut moved = view();
        moved.view_from_room = Mat4::from_translation(Vec3::new(0.0, 0.0, -4.0)) * moved.view_from_room;
        assert_eq!(render(&face, &moved).surface.triangle_count(), 2);
        let mut still = RoomMeshOutput::default();
        RoomSubdivision { enabled: false, fillers: false }.tessellate(&face, &view(), &mut still);
        assert_eq!(still.surface.triangle_count(), 2);
    }
}
