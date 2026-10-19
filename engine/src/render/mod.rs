//! Drawing the level: loading it, building room and model meshes, posing
//! models, hiding rooms out of view, lighting items, and the 3D camera.
//!
//! All geometry comes from the level asset; no captured frame is used.
pub mod backdrop;
pub mod rooms;
pub mod textures;
pub mod view;

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::MeshAabb,
    mesh::PrimitiveTopology,
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
};

use crate::{
    animation::{AnimationLibrary, AnimationState, ObjectAnimation, model_pose},
    camera::{CameraState, chase_pose},
    controller::{Button, ControllerState},
    lighting,
    objects::{INVISIBLE, ObjectTypes, SpawnContext, WorldObject},
    psx::{PsxMaterial, PsxScreen, PsxView},
    schedule::Avatar,
    world::{Level, LevelLoader, Texture, WorldGeometry, angle, position},
};

/// The level to show: `assets/levels/<name>/level.json`. Empty = none.
#[derive(Resource, Default)]
pub struct LevelRequest {
    pub name: String,
}

/// Load the level named `name` from `path` (relative to the asset root)
/// rather than its `level.json`: an editor's play-test copy. Everything else
/// keyed by the name (atlas, sky, audio, the level order) stays the level's.
#[derive(Resource, Clone, Debug)]
pub struct LevelFile {
    pub name: String,
    pub path: String,
}

/// A level finished loading and its entities were spawned this frame.
#[derive(Message, Clone)]
pub struct LevelLoaded {
    pub name: String,
    pub data: Arc<Level>,
}

/// Show an authored level without saving it. The caller supplies a validated
/// level; the atlas path is relative to the configured asset root. An editor
/// uses a metadata-only object registry, so no game controllers are spawned.
#[derive(Message, Clone)]
pub struct PreviewLevel {
    pub data: Arc<Level>,
    pub atlas: String,
}

/// The level that is spawned now.
#[derive(Resource)]
pub struct ActiveLevel {
    pub name: String,
}

#[derive(Resource)]
struct Pending {
    handle: Handle<Level>,
    name: String,
}

/// Everything spawned for the current level; despawned when another loads.
#[derive(Component)]
pub struct LevelEntity;

/// The room an entity is drawn in; it is hidden when that room is out of view.
#[derive(Component)]
pub struct InRoom(pub usize);

#[derive(Component)]
struct Bone {
    root: Entity,
    model: usize,
    joint: usize,
}

/// The 3D camera that shows the level.
#[derive(Component)]
pub struct GameCamera;

/// Developer override of the 3D camera (`manual`), editable over the MCP.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct CameraRig {
    pub manual: bool,
    pub position: Vec3,
    pub target: Vec3,
}

/// Projection and fog of the level camera.
#[derive(Resource, Clone, Debug)]
pub struct ViewSettings {
    /// Vertical field of view, radians.
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    /// Linear black fog from `fog_start` to `fog_end` metres.
    pub fog_start: f32,
    pub fog_end: f32,
    /// Full projected width and the drawn width (a narrower display window
    /// crops the right edge of the projection).
    pub full_size: UVec2,
    pub size: UVec2,
}

impl Default for ViewSettings {
    /// A plain 60° view filling a 320x240 screen. Games insert their own
    /// settings and pass the matching size to `launcher::Game::screen`.
    fn default() -> Self {
        Self {
            fov: 60f32.to_radians(),
            near: 0.05,
            far: 80.0,
            fog_start: 12.0,
            fog_end: 25.0,
            full_size: UVec2::new(320, 240),
            size: UVec2::new(320, 240),
        }
    }
}

/// Use `settings` for the level camera and size the PS1 screen (and the
/// window, at 3x) to its drawn size. Call after `PsxPlugin` is added, before
/// the app runs. For a launcher app, prefer `launcher::Game::screen` and
/// insert the matching settings in its `add` closure; this helper also
/// supports hand-built apps that need to resize an existing screen.
pub fn configure_view(app: &mut App, settings: ViewSettings) {
    let size = settings.size;
    let world = app.world_mut();
    let target = || {
        let mut image = Image::new_target_texture(
            size.x,
            size.y,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        );
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
        image
    };
    let (image, canvas) = {
        let mut images = world.resource_mut::<Assets<Image>>();
        (images.add(target()), images.add(target()))
    };
    let mut screen = world.resource_mut::<PsxScreen>();
    (screen.width, screen.height, screen.image, screen.canvas) = (size.x, size.y, image, canvas);
    let mut windows = world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
    for mut window in windows.iter_mut(world) {
        window.resolution.set((size.x * 3) as f32, (size.y * 3) as f32);
    }
    world.insert_resource(settings);
}

/// Order of the level's per-frame work in `Update`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelSystems {
    /// Load the requested level and spawn it.
    Load,
    /// Pose models, then decide what is visible.
    Pose,
    Visibility,
    /// Rebuild opt-in camera-dependent room geometry after visibility.
    Meshes,
    /// Apply per-mesh texture overrides after lighting and mesh restoration.
    Textures,
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(backdrop::BackdropPlugin)
            .init_asset::<Level>()
            .init_asset_loader::<LevelLoader>()
            .init_resource::<LevelRequest>()
            .init_resource::<ViewSettings>()
            .init_resource::<ObjectTypes>()
            .init_resource::<textures::TextureVariants>()
            .add_message::<LevelLoaded>()
            .add_message::<PreviewLevel>()
            .configure_sets(
                Update,
                (
                    LevelSystems::Load,
                    LevelSystems::Pose,
                    LevelSystems::Visibility,
                    LevelSystems::Meshes,
                    LevelSystems::Textures,
                )
                    .chain(),
            )
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (request_level, finish_level).chain().in_set(LevelSystems::Load))
            .add_systems(Update, (pose_models, follow_camera).in_set(LevelSystems::Pose))
            .add_systems(Update, (room_visibility, light_items).chain().in_set(LevelSystems::Visibility))
            .add_systems(Update, rooms::rebuild.in_set(LevelSystems::Meshes))
            .add_systems(Update, textures::apply.in_set(LevelSystems::Textures));
    }
}

fn spawn_camera(mut commands: Commands, settings: Res<ViewSettings>) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            sub_camera_view: Some(bevy::camera::SubCameraView {
                full_size: settings.full_size,
                offset: Vec2::ZERO,
                size: settings.size,
            }),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: settings.fov,
            near: settings.near,
            far: settings.far,
            ..default()
        }),
        DistanceFog {
            color: Color::BLACK,
            falloff: FogFalloff::Linear { start: settings.fog_start, end: settings.fog_end },
            ..default()
        },
        PsxView,
        Transform::default(),
        GameCamera,
        CameraRig::default(),
        Name::new("ChaseCamera"),
    ));
}

fn request_level(
    mut commands: Commands,
    request: Res<LevelRequest>,
    active: Option<Res<ActiveLevel>>,
    pending: Option<Res<Pending>>,
    server: Res<AssetServer>,
    file: Option<Res<LevelFile>>,
) {
    if request.name.is_empty()
        || active.as_ref().is_some_and(|a| a.name == request.name)
        || pending.as_ref().is_some_and(|p| p.name == request.name)
    {
        return;
    }
    let path = match file {
        Some(file) if file.name == request.name => file.path.clone(),
        _ => format!("levels/{}/level.json", request.name),
    };
    let handle = server.load(path);
    commands.insert_resource(Pending { handle, name: request.name.clone() });
}

struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    normals: Vec<[f32; 3]>,
}

impl MeshBuilder {
    fn new() -> Self {
        Self { positions: vec![], uv: vec![], colors: vec![], normals: vec![] }
    }
    fn face_lit(&mut self, indices: &[usize], texture: &Texture, vertices: &[[f32; 3]], normals: &[[f32; 3]]) {
        let start = self.positions.len();
        self.face(indices, texture, vertices, &vec![1.0; vertices.len()]);
        let order: &[usize] = if indices.len() == 4 { &[0, 1, 2, 0, 2, 3] } else { &[0, 1, 2] };
        for &corner in order {
            self.normals.push(normals[indices[corner]]);
        }
        debug_assert_eq!(self.normals.len(), self.positions.len().max(start));
    }
    fn face(&mut self, indices: &[usize], texture: &Texture, vertices: &[[f32; 3]], light: &[f32]) {
        let order: &[usize] = if indices.len() == 4 { &[0, 1, 2, 0, 2, 3] } else { &[0, 1, 2] };
        let uv_order: &[usize] = if indices.len() == 4 { &[0, 1, 3, 2] } else { &[0, 1, 2] };
        for &corner in order {
            let i = indices[corner];
            self.positions.push(vertices[i]);
            self.uv.push(texture.uv[uv_order[corner]]);
            // `light` is the GPU's encoded multiplier (128 = 1.0); PsxMaterial
            // expects linear vertex colours and re-encodes them.
            let c = light[i];
            self.colors.push(Color::srgb(c, c, c).to_linear().to_f32_array());
        }
    }
    fn finish(self) -> Mesh {
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        if self.normals.is_empty() { mesh } else { mesh.with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals) }
    }
}

/// Build an authored object mesh with its atlas UVs and vertex lighting.
/// Model placement, posing and material choice belong to the caller.
pub fn object_mesh(m: &crate::world::ObjectMesh, textures: &[Texture]) -> Mesh {
    let vs: Vec<_> = m.vertices.iter().map(|v| position([v[0], v[1], v[2]]).to_array()).collect();
    let mut b = MeshBuilder::new();
    if m.normals.is_empty() {
        let light: Vec<_> = m.vertices.iter().map(|v| (1.0 - v[3] as f32 / 8192.0).clamp(0.05, 1.0)).collect();
        for f in &m.quads {
            b.face(&f[..4], &textures[f[4]], &vs, &light);
        }
        for f in &m.triangles {
            b.face(&f[..3], &textures[f[3]], &vs, &light);
        }
    } else {
        // Normal-lit meshes: ItemLighting supplies the shade per object.
        let ns: Vec<_> = m
            .normals
            .iter()
            .map(|n| Vec3::new(n[0] as f32, -n[1] as f32, n[2] as f32).normalize_or_zero().to_array())
            .collect();
        for f in &m.quads {
            b.face_lit(&f[..4], &textures[f[4]], &vs, &ns);
        }
        for f in &m.triangles {
            b.face_lit(&f[..3], &textures[f[3]], &vs, &ns);
        }
    }
    b.finish()
}

#[allow(clippy::too_many_arguments)]
fn finish_level(
    mut commands: Commands,
    pending: Option<Res<Pending>>,
    mut assets: ResMut<Assets<Level>>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PsxMaterial>>,
    types: Res<ObjectTypes>,
    old: Query<Entity, (With<LevelEntity>, Without<ChildOf>)>,
    mut loaded: MessageWriter<LevelLoaded>,
    mut previews: MessageReader<PreviewLevel>,
    room_policy: Option<Res<rooms::RoomMeshPolicy>>,
) {
    let (name, data, atlas_path) = if let Some(preview) = previews.read().last() {
        (preview.data.name.clone(), preview.data.clone(), preview.atlas.clone())
    } else {
        let Some(p) = pending else { return };
        let Some(data) = assets.remove(&p.handle) else { return };
        (p.name.clone(), Arc::new(data), format!("levels/{}/atlas.png", p.name))
    };
    for e in &old {
        commands.entity(e).despawn();
    }
    // Keep rule-space positions local. Only the render hierarchy changes
    // handedness; camera conversion applies the same boundary separately.
    let render_root = commands
        .spawn((Transform::from_scale(view::LEVEL_TO_RENDER), Visibility::default(), LevelEntity, Name::new("LevelWorld")))
        .id();
    let world = WorldGeometry { data: data.clone() };
    let atlas = server.load(atlas_path);
    let material = materials.add(PsxMaterial { texture: Some(atlas.clone()), lit: false, double_sided: true, ..default() });
    let underlay_material = room_policy.as_ref().map(|_| {
        materials.add(PsxMaterial {
            texture: Some(atlas.clone()),
            lit: false,
            double_sided: true,
            depth_bias: crate::psx::PsxDepthBias::BEHIND,
            ..default()
        })
    });
    for (room_index, room) in data.rooms.iter().enumerate() {
        let vs: Vec<_> = room.vertices.iter().map(|v| position([v[0], v[1], v[2]]).to_array()).collect();
        let light: Vec<_> = room.vertices.iter().map(|v| lighting::room_vertex_shade(v[3])).collect();
        let mut b = MeshBuilder::new();
        for f in &room.quads {
            b.face(&f[..4], &data.textures[f[4]], &vs, &light);
        }
        for f in &room.triangles {
            b.face(&f[..3], &data.textures[f[3]], &vs, &light);
        }
        let built = b.finish();
        let bounds = built.get_aabb();
        let mesh = meshes.add(built);
        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(position(room.origin)),
                InRoom(room_index),
                LevelEntity,
                ChildOf(render_root),
                Name::new(format!("Room{room_index}")),
            ))
            .id();
        if let Some(material) = &underlay_material {
            let underlay = commands
                .spawn((
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Inherited,
                    ChildOf(entity),
                    Name::new("RoomUnderlay"),
                ))
                .id();
            commands.entity(entity).insert((
                rooms::RoomMeshSource::new(room, &data, atlas.clone(), mesh, underlay),
                rooms::RoomMeshStats::default(),
            ));
            // All future pieces/fillers remain inside the authored room.
            // Preserve conservative bounds instead of keeping the first
            // camera-dependent subset's AABB when the camera moves.
            if let Some(bounds) = bounds {
                commands.entity(entity).insert(bounds);
                commands.entity(underlay).insert(bounds);
            }
        }
    }
    let object_meshes: Vec<_> = data
        .meshes
        .iter()
        .map(|m| {
            meshes.add(object_mesh(m, &data.textures))
        })
        .collect();
    for (room_index, room) in data.rooms.iter().enumerate() {
        for s in &room.static_placements {
            let Some(def) = data.static_meshes.iter().find(|m| m.id == s.mesh_id) else { continue };
            let Some(mesh) = object_meshes.get(def.mesh) else { continue };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(position(s.position)).with_rotation(Quat::from_rotation_y(angle(s.rotation))),
                InRoom(room_index),
                LevelEntity,
                ChildOf(render_root),
                Name::new(format!("Static{}", s.mesh_id)),
            ));
        }
    }
    for (index, placement) in data.placements.iter().enumerate() {
        let model = data.models.iter().enumerate().find(|(_, m)| m.id == placement.kind);
        let t = Transform::from_translation(position(placement.position))
            .with_rotation(Quat::from_rotation_y(angle(placement.rotation)));
        let lit = model.map(|_| materials.add(PsxMaterial {
            texture: Some(atlas.clone()),
            lit: true,
            use_scene_lighting: false,
            double_sided: true,
            ..default()
        }));
        let mut entity = commands.spawn((
            t,
            Visibility::default(),
            InRoom(placement.room),
            LevelEntity,
            ChildOf(render_root),
            Name::new(format!("Object{}_{}", placement.kind, index)),
        ));
        match types.get(placement.kind).and_then(|t| t.spawn_hook()) {
            Some(hook) => hook(
                &mut entity,
                &SpawnContext {
                    index,
                    placement,
                    world: &world,
                    render_root,
                    meshes: &object_meshes,
                    lit_material: lit.clone(),
                },
            ),
            // Visibility on the root lets room_visibility hide dormant,
            // collected and out-of-view objects together with their meshes.
            None => {
                entity.insert((
                    WorldObject::from_placement(index, placement),
                    ObjectAnimation { animation: model.map_or(usize::MAX, |(_, m)| m.animations[0]), frame: 0.0 },
                ));
            }
        }
        let root = entity.id();
        let (Some((model_index, model)), Some(lit)) = (model, lit) else { continue };
        commands.entity(root).insert(ItemLighting(lit.clone()));
        for (joint, &mi) in model.meshes.iter().enumerate() {
            let part = commands
                .spawn((Mesh3d(object_meshes[mi].clone()), MeshMaterial3d(lit.clone()), Transform::default(), Bone { root, model: model_index, joint }))
                .id();
            commands.entity(root).add_child(part);
        }
    }
    commands.insert_resource(AnimationLibrary::from(data.as_ref()));
    commands.insert_resource(CameraState::default());
    commands.insert_resource(world);
    commands.insert_resource(ActiveLevel { name: name.clone() });
    commands.remove_resource::<Pending>();
    loaded.write(LevelLoaded { name, data });
}

/// Per-object material whose lighting follows the item light of
/// [`lighting::item_light`].
#[derive(Component)]
pub struct ItemLighting(pub Handle<PsxMaterial>);

/// The object lighter samples the light at the item position plus its
/// rotated bounds centre. Its shade is linear in n.L; two opposite material
/// lights reproduce it exactly.
#[allow(clippy::type_complexity)]
fn light_items(
    world: Option<Res<WorldGeometry>>,
    mut materials: ResMut<Assets<PsxMaterial>>,
    items: Query<(
        &ItemLighting,
        &Transform,
        Option<&Avatar>,
        Option<&AnimationState>,
        Option<&WorldObject>,
        Option<&ObjectAnimation>,
    )>,
) {
    let Some(world) = world else { return };
    for (lighting, t, avatar, animation, object, object_animation) in &items {
        let room = avatar.map(|p| p.room).or(object.map(|o| o.room)).unwrap_or(0);
        let (anim, frame) =
            animation.map(|a| (a.animation, a.frame)).or(object_animation.map(|a| (a.animation, a.frame))).unwrap_or((usize::MAX, 0.0));
        let centre = world
            .data
            .animations
            .get(anim)
            .and_then(|c| {
                let key = (frame.max(0.0) as usize) / c.frame_rate.max(1);
                // Bounds of the key being shown (the closing key is not shown).
                let key = if key < c.poses.len() { key } else { 0 };
                (!c.poses.is_empty()).then(|| c.bounds.get(key)).flatten()
            })
            .map_or(Vec3::ZERO, |&b| {
                Vec3::new((b[0] + b[1]) as f32 / 2.0, -((b[2] + b[3]) as f32) / 2.0, (b[4] + b[5]) as f32 / 2.0) / 1024.0
            });
        let point = t.translation + t.rotation * centre;
        let point = IVec3::new((point.x * 1024.0).round() as i32, (-point.y * 1024.0).round() as i32, (point.z * 1024.0).round() as i32);
        let light = lighting::item_light(&world.data, room, point);
        let Some(mut material) = materials.get_mut(&lighting.0) else { continue };
        let ambient = (255.0 - light.ambient.clamp(0, 8191) as f32 / 32.0) / lighting::NEUTRAL_SHADE;
        // Normal, light-vector and rotation scales are all 16384 in the
        // original lighter, so the directional shade is strength * n.L.
        let k = light.strength as f32 / (32.0 * lighting::NEUTRAL_SHADE);
        let dir = view::render_position(Vec3::new(light.direction.x, -light.direction.y, light.direction.z)).normalize_or_zero();
        let mut l = material.lighting;
        l.ambient = Vec3::splat(ambient);
        l.directions[0] = -dir;
        l.colors[0] = Vec3::splat(k);
        l.directions[1] = dir;
        l.colors[1] = Vec3::splat(-k);
        l.count = 2;
        if material.lighting != l {
            material.lighting = l;
        }
    }
}

fn pose_models(
    world: Option<Res<WorldGeometry>>,
    roots: Query<(Entity, Option<&AnimationState>, Option<&ObjectAnimation>)>,
    mut bones: Query<(&Bone, &mut Transform)>,
) {
    let Some(world) = world else { return };
    let mut cache = std::collections::HashMap::<(Entity, usize), Vec<Transform>>::new();
    for (bone, mut t) in &mut bones {
        let values = cache.entry((bone.root, bone.model)).or_insert_with(|| {
            let model = &world.data.models[bone.model];
            let Ok((_, player, object)) = roots.get(bone.root) else { return vec![] };
            let (anim, frame) =
                player.map(|a| (a.animation, a.frame)).or_else(|| object.map(|a| (a.animation, a.frame))).unwrap_or((model.animations[0], 0.0));
            model_pose(&world.data, model, anim, frame)
        });
        if let Some(v) = values.get(bone.joint) {
            *t = *v;
        }
    }
}

fn follow_camera(
    avatars: Query<&Avatar>,
    mut cams: Query<(&mut Transform, &CameraRig), With<GameCamera>>,
    world: Option<Res<WorldGeometry>>,
    state: Res<CameraState>,
    pad: Res<ControllerState>,
) {
    let Ok(p) = avatars.single() else { return };
    let Ok((mut c, rig)) = cams.single_mut() else { return };
    if rig.manual {
        *c = view::camera_transform(rig.position, rig.target);
        return;
    }
    let (eye, target) = if state.initialized {
        (state.position + Vec3::Y * state.view_offset, state.target)
    } else {
        let (eye, target, _) = chase_pose(p, world.as_deref(), pad.held(Button::L1));
        (eye, target)
    };
    if eye.distance_squared(target) > 0.001 {
        *c = view::camera_transform(eye, target);
    }
}

#[allow(clippy::type_complexity)]
fn room_visibility(
    world: Option<Res<WorldGeometry>>,
    avatars: Query<&Avatar>,
    camera: Res<CameraState>,
    mut objects: Query<(&mut InRoom, &mut Visibility, Option<&Avatar>, Option<&WorldObject>)>,
) {
    let (Some(world), Ok(p)) = (world, avatars.single()) else { return };
    let mut visible = world.visible_rooms(p.room, p.position);
    if camera.initialized {
        for (visible, from_eye) in visible.iter_mut().zip(world.visible_rooms(camera.room, camera.position)) {
            *visible |= from_eye;
        }
    }
    for (mut room, mut vis, avatar, object) in &mut objects {
        if let Some(avatar) = avatar {
            room.0 = avatar.room;
        }
        if let Some(object) = object {
            room.0 = object.room;
        }
        let hidden = object.is_some_and(|o| o.consumed || (!o.active && o.flags & INVISIBLE != 0));
        *vis = if !hidden && visible.get(room.0).copied().unwrap_or(false) { Visibility::Inherited } else { Visibility::Hidden };
    }
}
