//! Optional camera-dependent room meshes. Games supply a tessellator; the
//! engine owns mesh assets, visibility, scheduling and the coplanar underlay.
//! Without a policy, authored room meshes stay static (including the editor).
use bevy::{mesh::VertexAttributeValues, prelude::*};

use super::{GameCamera, InRoom, MeshBuilder, ViewSettings, view};
use crate::world::{Level, Room, position};

/// An authored corner in level-file units (y down), with atlas UV and the
/// encoded grey shade byte. Keeping source units permits exact midpoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomVertex {
    pub position: IVec3,
    pub uv: Vec2,
    pub shade: u8,
}

/// Corners follow the level's cyclic drawing order; texture UVs are paired
/// with their corners here so a policy need not know the file's UV ordering.
pub struct RoomFace {
    pub vertices: Vec<RoomVertex>,
    pub texture_attribute: u16,
}

/// The current camera in this room's local coordinates. View coordinates
/// use Bevy's convention: metres, y up, forward along -z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomView {
    pub view_from_room: Mat4,
    pub clip_from_view: Mat4,
    pub atlas_size: UVec2,
    pub full_size: UVec2,
}

impl RoomView {
    pub fn view_position(&self, vertex: RoomVertex) -> Vec3 {
        self.view_from_room.transform_point3(position(vertex.position.to_array()))
    }
}

/// Register a game's room-rendering rule before the level is loaded. A
/// changed policy invalidates visible meshes even if the camera is still.
#[derive(Resource)]
pub struct RoomMeshPolicy(pub Box<dyn RoomTessellator>);

pub trait RoomTessellator: Send + Sync + 'static {
    fn tessellate(&self, face: &RoomFace, view: &RoomView, output: &mut RoomMeshOutput);
}

/// Reusable CPU buffers; the underlay uses `PsxDepthBias::BEHIND` and inherits
/// its room's visibility. It is optional geometry, not a second world scene.
#[derive(Default)]
pub struct RoomMeshOutput {
    pub surface: RoomMeshBuilder,
    pub underlay: RoomMeshBuilder,
}

#[derive(Default)]
pub struct RoomMeshBuilder {
    positions: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

impl RoomMeshBuilder {
    pub fn triangle(&mut self, vertices: [RoomVertex; 3]) {
        for vertex in vertices {
            self.positions.push(position(vertex.position.to_array()).to_array());
            self.uv.push(vertex.uv.to_array());
            let grey = vertex.shade as f32 / crate::lighting::NEUTRAL_SHADE;
            self.colors.push(Color::srgb(grey, grey, grey).to_linear().to_f32_array());
        }
    }

    /// Draw an authored polygon with the engine's static-mesh diagonal.
    pub fn face(&mut self, vertices: &[RoomVertex]) {
        self.triangle([vertices[0], vertices[1], vertices[2]]);
        if vertices.len() == 4 {
            self.triangle([vertices[0], vertices[2], vertices[3]]);
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn into_mesh(self) -> Mesh {
        MeshBuilder { positions: self.positions, uv: self.uv, colors: self.colors, normals: vec![] }.finish()
    }

    fn clear(&mut self) {
        self.positions.clear();
        self.uv.clear();
        self.colors.clear();
    }

    fn write(&self, mesh: &mut Mesh) {
        // Keep both the scratch and mesh-attribute allocations between frames.
        if let Some(VertexAttributeValues::Float32x3(values)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            values.clone_from(&self.positions);
        }
        if let Some(VertexAttributeValues::Float32x2(values)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
            values.clone_from(&self.uv);
        }
        if let Some(VertexAttributeValues::Float32x4(values)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
            values.clone_from(&self.colors);
        }
    }
}

/// Observable rendering counts; no simulation state depends on them.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct RoomMeshStats {
    pub triangles: usize,
    pub underlay_triangles: usize,
    pub rebuilds: usize,
}

#[derive(Component)]
pub(super) struct RoomMeshSource {
    faces: Vec<RoomFace>,
    atlas: Handle<Image>,
    surface: Handle<Mesh>,
    underlay: Option<Handle<Mesh>>,
    underlay_entity: Entity,
    last_view: Option<RoomView>,
    last_policy: Option<u32>,
    output: RoomMeshOutput,
}

impl RoomMeshSource {
    pub(super) fn new(
        room: &Room,
        level: &Level,
        atlas: Handle<Image>,
        surface: Handle<Mesh>,
        underlay_entity: Entity,
    ) -> Self {
        let face = |indices: &[usize], texture: usize| {
            let texture = &level.textures[texture];
            let order = if indices.len() == 4 { &[0, 1, 3, 2][..] } else { &[0, 1, 2][..] };
            RoomFace {
                vertices: indices
                    .iter()
                    .zip(order)
                    .map(|(&index, &uv)| {
                        let [x, y, z, shade] = room.vertices[index];
                        RoomVertex {
                            position: IVec3::new(x, y, z),
                            uv: Vec2::from_array(texture.uv[uv]),
                            shade: shade as u8,
                        }
                    })
                    .collect(),
                texture_attribute: texture.attribute,
            }
        };
        Self {
            faces: room
                .quads
                .iter()
                .map(|f| face(&f[..4], f[4]))
                .chain(room.triangles.iter().map(|f| face(&f[..3], f[3])))
                .collect(),
            atlas,
            surface,
            underlay: None,
            underlay_entity,
            last_view: None,
            last_policy: None,
            output: RoomMeshOutput::default(),
        }
    }
}

/// Runs after the camera and room visibility in Update, before extraction.
/// Use the freshly written local camera Transform, not last frame's propagated
/// GlobalTransform. Rooms are children of the engine's reflected level root.
pub(super) fn rebuild(
    mut commands: Commands,
    policy: Option<Res<RoomMeshPolicy>>,
    settings: Res<ViewSettings>,
    camera: Query<(&Transform, &Projection, &Camera), With<GameCamera>>,
    images: Res<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rooms: Query<(Entity, &Transform, &Visibility, &mut RoomMeshSource, &mut RoomMeshStats), With<InRoom>>,
) {
    let Some(policy) = policy else { return };
    let revision = policy.last_changed().get();
    let Ok((camera, projection, camera_view)) = camera.single() else { return };
    let clip_from_view = if let Some(sub) = &camera_view.sub_camera_view {
        projection.get_clip_from_view_for_sub(sub)
    } else {
        projection.get_clip_from_view()
    };
    let view_from_level = camera.to_matrix().inverse() * Mat4::from_scale(view::LEVEL_TO_RENDER);
    for (entity, transform, visibility, mut source, mut stats) in &mut rooms {
        if *visibility == Visibility::Hidden {
            continue;
        }
        let Some(atlas) = images.get(&source.atlas) else { continue };
        let context = RoomView {
            view_from_room: view_from_level * transform.to_matrix(),
            clip_from_view,
            atlas_size: atlas.size(),
            full_size: settings.full_size,
        };
        if source.last_view == Some(context) && source.last_policy == Some(revision) {
            continue;
        }
        let RoomMeshSource { faces, output, .. } = &mut *source;
        output.surface.clear();
        output.underlay.clear();
        for face in faces {
            policy.0.tessellate(face, &context, output);
        }
        stats.triangles = output.surface.triangle_count();
        stats.underlay_triangles = output.underlay.triangle_count();
        if stats.triangles == 0 {
            commands.entity(entity).remove::<Mesh3d>();
        } else {
            if let Some(mut mesh) = meshes.get_mut(&source.surface) {
                source.output.surface.write(&mut mesh);
            }
            commands.entity(entity).insert(Mesh3d(source.surface.clone()));
        }
        if stats.underlay_triangles == 0 {
            commands.entity(source.underlay_entity).remove::<Mesh3d>();
        } else {
            let handle = if let Some(handle) = source.underlay.clone() {
                if let Some(mut mesh) = meshes.get_mut(&handle) {
                    source.output.underlay.write(&mut mesh);
                }
                handle
            } else {
                // Allocate an asset only when there is something to draw.
                let handle = meshes.add(copy_mesh(&source.output.underlay));
                source.underlay = Some(handle.clone());
                handle
            };
            commands.entity(source.underlay_entity).insert(Mesh3d(handle));
        }
        stats.rebuilds += 1;
        source.last_view = Some(context);
        source.last_policy = Some(revision);
    }
}

fn copy_mesh(data: &RoomMeshBuilder) -> Mesh {
    let mut mesh = MeshBuilder::new().finish();
    data.write(&mut mesh);
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPolicy(bool);
    impl RoomTessellator for TestPolicy {
        fn tessellate(&self, face: &RoomFace, _: &RoomView, output: &mut RoomMeshOutput) {
            output.surface.face(&face.vertices);
            if self.0 {
                output.underlay.face(&face.vertices);
            }
        }
    }

    #[test]
    fn room_meshes_follow_camera_visibility_and_policy_changes_without_reallocating_handles() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<ViewSettings>()
            .insert_resource(RoomMeshPolicy(Box::new(TestPolicy(true))))
            .add_systems(Update, rebuild);
        let atlas = app.world_mut().resource_mut::<Assets<Image>>().add(Image::default());
        let surface = app.world_mut().resource_mut::<Assets<Mesh>>().add(MeshBuilder::new().finish());
        let underlay = app.world_mut().spawn_empty().id();
        let vertices =
            [IVec3::ZERO, IVec3::X, IVec3::Z].map(|position| RoomVertex { position, uv: Vec2::ZERO, shade: 128 });
        let room = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Inherited,
                InRoom(0),
                Mesh3d(surface.clone()),
                RoomMeshStats::default(),
                RoomMeshSource {
                    faces: vec![RoomFace { vertices: vertices.to_vec(), texture_attribute: 0 }],
                    atlas,
                    surface: surface.clone(),
                    underlay: None,
                    underlay_entity: underlay,
                    last_view: None,
                    last_policy: None,
                    output: RoomMeshOutput::default(),
                },
            ))
            .id();
        let camera =
            app.world_mut().spawn((GameCamera, Transform::default(), Projection::default(), Camera::default())).id();
        app.update();
        let underlay_handle = app.world().get::<Mesh3d>(underlay).unwrap().0.clone();
        assert_eq!(app.world().get::<RoomMeshStats>(room).unwrap().rebuilds, 1);
        app.update();
        assert_eq!(
            app.world().get::<RoomMeshStats>(room).unwrap().rebuilds,
            1,
            "a paused still camera reuses the mesh"
        );
        app.world_mut().get_mut::<Transform>(camera).unwrap().translation.x = 1.0;
        app.world_mut().get_mut::<Visibility>(room).unwrap().set_if_neq(Visibility::Hidden);
        app.update();
        assert_eq!(app.world().get::<RoomMeshStats>(room).unwrap().rebuilds, 1, "hidden rooms wait until needed");
        app.world_mut().get_mut::<Visibility>(room).unwrap().set_if_neq(Visibility::Inherited);
        app.update();
        assert_eq!(app.world().get::<RoomMeshStats>(room).unwrap().rebuilds, 2);
        assert_eq!(app.world().get::<Mesh3d>(room).unwrap().0, surface);
        assert_eq!(app.world().get::<Mesh3d>(underlay).unwrap().0, underlay_handle);
        app.world_mut().insert_resource(RoomMeshPolicy(Box::new(TestPolicy(false))));
        app.update();
        assert_eq!(app.world().get::<RoomMeshStats>(room).unwrap().rebuilds, 3);
        assert!(app.world().get::<Mesh3d>(underlay).is_none(), "unused underlay is not submitted as an empty mesh");
        app.world_mut().insert_resource(RoomMeshPolicy(Box::new(TestPolicy(true))));
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(underlay).unwrap().0, underlay_handle);
        app.world_mut().remove_resource::<RoomMeshPolicy>();
        app.world_mut().get_mut::<Transform>(camera).unwrap().translation.x = 2.0;
        app.update();
        assert_eq!(
            app.world().get::<RoomMeshStats>(room).unwrap().rebuilds,
            4,
            "without a policy meshes remain static"
        );
        // A policy revision must also reach a room that was hidden when
        // the resource changed, even when its cached camera is unchanged.
        app.world_mut().get_mut::<Transform>(camera).unwrap().translation.x = 1.0;
        *app.world_mut().get_mut::<Visibility>(room).unwrap() = Visibility::Hidden;
        app.world_mut().insert_resource(RoomMeshPolicy(Box::new(TestPolicy(false))));
        app.update();
        *app.world_mut().get_mut::<Visibility>(room).unwrap() = Visibility::Inherited;
        app.update();
        assert_eq!(app.world().get::<RoomMeshStats>(room).unwrap().rebuilds, 5);
        assert!(app.world().get::<Mesh3d>(underlay).is_none());
    }
}
