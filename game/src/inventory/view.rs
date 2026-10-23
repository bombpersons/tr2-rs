//! Drawing the inventory ring: the backdrop, the items and the ring's text.
//!
//! The original draws the ring in its own views (Display_Inventory
//! `0x80030F30`): the backdrop, object 223's mesh 12288 below a camera at
//! y -512 looking straight down (`0x80032FF4`), rippling as it is drawn
//! ([`BackdropRipple`]), then the items around a
//! camera on the ring's axis looking at (0, -96, radius) (`0x80034B64`),
//! lit from (-1536, 256, 1024) (`0x80034C00`). Each item hangs at the
//! ring's radius, tilts towards the camera by its `pt_xrot`, then takes its
//! own offset and turns (`0x800328E8`). Both views use the play camera's
//! projection. Positions here are the original's (y down) until
//! [`engine::world::position`] makes them level space; the scene's root
//! makes level space render space, as the level's does.
//!
//! The text is the original's ([`crate::text`]) in its places
//! (`0x800331D4`, `0x800334DC`).
use bevy::{camera::visibility::RenderLayers, prelude::*};
use engine::{
    animation::{joint_turn, model_pose},
    camera::chase::{cos, sin},
    psx::{PsxMaterial, PsxView},
    render::{
        ActiveLevel, GameCamera, LevelSystems, ObjectMeshes, shaded_object_mesh,
        view::{LEVEL_TO_RENDER, render_position},
    },
    world::{Level, Model, WorldGeometry, position},
};

use super::{GameStrings, Inventory, InventoryRing, InventorySystems, ItemTable, RingMode, item, ring::RingKind};
use crate::text::{Align, ScreenLines, ScreenText};

/// Render layers of the ring's backdrop and items.
pub const BACKDROP_LAYER: usize = 25;
pub const RING_LAYER: usize = 26;
/// Camera orders: after the level's cameras, before the HUD (`presentation`).
pub const BACKDROP_ORDER: isize = 2;
pub const RING_ORDER: isize = 3;

/// The ring's backdrop model (`0x800960C4`: object 223's clip and mesh).
const BACKDROP_OBJECT: u16 = 223;
/// How far below its camera the backdrop hangs (`0x800890FC`).
const BACKDROP_DEPTH: i32 = 12288;

pub struct InventoryViewPlugin;

impl Plugin for InventoryViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<GameflowAsset>()
            .init_asset_loader::<GameflowLoader>()
            .init_resource::<BackdropRipple>()
            .add_systems(Update, load_strings.run_if(crate::disc::data_ready))
            .add_systems(FixedUpdate, step_ripple.after(InventorySystems::Ring))
            .add_systems(
                Update,
                (build_scene, pose_scene, ripple_backdrop, ring_text)
                    .chain()
                    .after(LevelSystems::Pose)
                    .before(LevelSystems::Visibility),
            );
    }
}

/// `imported/gameflow.json` as an asset.
#[derive(Asset, TypePath, Debug)]
pub struct GameflowAsset(pub import::gameflow::Gameflow);

#[derive(Default, TypePath)]
struct GameflowLoader;

impl bevy::asset::AssetLoader for GameflowLoader {
    type Asset = GameflowAsset;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &(),
        _context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<GameflowAsset, std::io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(GameflowAsset(serde_json::from_slice(&bytes)?))
    }
    fn extensions(&self) -> &[&str] {
        &["gameflow.json"]
    }
}

fn load_strings(
    server: Res<AssetServer>,
    assets: Res<Assets<GameflowAsset>>,
    mut strings: ResMut<GameStrings>,
    mut handle: Local<Option<Handle<GameflowAsset>>>,
) {
    if !strings.0.game_strings.is_empty() {
        return;
    }
    let handle = handle.get_or_insert_with(|| server.load("gameflow.json"));
    if let Some(flow) = assets.get(&*handle) {
        strings.0 = flow.0.clone();
    }
}

/// The backdrop's ripple. The original draws the backdrop's 9 by 13 grid
/// (`0x8006759C` with flag 1) moving each vertex across by up to 0x140 on
/// a circle and shading it `0x20 + edge + sin(a) / 1024 + sin(b) / 1024`
/// (128 neutral), where `edge` darkens the outer two rings of vertices
/// (the bytes at `0x80089A88`: 0, 0x10, then 0x20). The three angles
/// step along each row and down the rows from phases (`0x80089AFE`,
/// `0x80089B00`, `0x80089B02`; 0, 0, 0x4000 at power-on) that move on
/// once per pass of the ring's loop and carry over between openings.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackdropRipple {
    pub phases: [i16; 3],
}

impl Default for BackdropRipple {
    fn default() -> Self {
        Self { phases: [0, 0, 0x4000] }
    }
}

impl BackdropRipple {
    const COLUMNS: usize = 9;
    const ROWS: usize = 13;
    /// Each pass's change of the phases.
    const STEP: [i16; 3] = [-0x200, -0x267, -0x267];
    /// The angles' change from one vertex to the next along a row, and
    /// from one row to the next.
    const ALONG: [i16; 3] = [0x1422, 0x33E7, 0x33E7];
    const DOWN: [i16; 3] = [0x1822, 0x3000, 0x3000];
    /// How far a vertex moves.
    const REACH: i32 = 0x140;

    pub fn step(&mut self) {
        for (phase, step) in self.phases.iter_mut().zip(Self::STEP) {
            *phase = phase.wrapping_add(step);
        }
    }

    /// The `index`th vertex's move across (x, z) and its grey.
    pub fn vertex(&self, index: usize) -> ([i32; 2], u8) {
        let (row, column) = (index / Self::COLUMNS, index % Self::COLUMNS);
        let [a, b, c]: [i32; 3] = std::array::from_fn(|i| {
            let angle = self.phases[i]
                .wrapping_add(Self::DOWN[i].wrapping_mul(row as i16))
                .wrapping_add(Self::ALONG[i].wrapping_mul(column as i16));
            i32::from(angle)
        });
        let edge = row.min(Self::ROWS - 1 - row).min(column).min(Self::COLUMNS - 1 - column).min(2) as i32 * 0x10;
        let grey = ((sin(a) >> 10) + (sin(b) >> 10) + 0x20 + edge).clamp(0, 0xFF) as u8;
        ([(sin(c) * Self::REACH) >> 14, (cos(c) * Self::REACH) >> 14], grey)
    }
}

/// The backdrop's camera, in render space: at (0, -512, 0) looking
/// straight down (GetVectorAngles(0, 4096, 0)), screen up along +z.
pub fn backdrop_camera() -> Transform {
    Transform::from_translation(render_position(position([0, -512, 0])))
        .looking_to(Vec3::NEG_Y, render_position(Vec3::Z))
}

/// The backdrop model's place under the ring scene's root: 12288 below,
/// turned to face the camera, posed by its first frame, then a half turn
/// (`0x80032FF4`).
pub fn backdrop_placement(data: &Level, model: &Model) -> Transform {
    let frame = model_pose(data, model, model.animations[0], 0.0).first().copied().unwrap_or_default();
    Transform::from_translation(position([0, BACKDROP_DEPTH, 0])).with_rotation(joint_turn([0x4000, 0, -0x8000]))
        * frame
        * Transform::from_rotation(joint_turn([0, i16::MIN, 0]))
}

/// The ripple moves on once per frame of the open ring.
fn step_ripple(state: Res<InventoryRing>, mut ripple: ResMut<BackdropRipple>) {
    if state.ring.is_some() {
        ripple.step();
    }
}

/// The backdrop's mesh, rebuilt from its model mesh as it ripples.
#[derive(Component)]
struct Backdrop {
    mesh: usize,
}

/// The scene drawn while the ring is open.
#[derive(Component)]
struct RingScene {
    kind: RingKind,
    level: String,
    /// The title's, without the backdrop.
    title: bool,
}

#[derive(Component)]
struct RingCamera;

#[derive(Component)]
struct BackdropCamera;

/// One item on the ring, by its place in the list.
#[derive(Component)]
struct RingItem {
    index: usize,
    model: usize,
}

/// One joint of a ring item's model.
#[derive(Component)]
struct RingBone {
    joint: usize,
}

#[derive(Component)]
struct RingLight(Handle<PsxMaterial>);

/// Spawns the ring's scene when the ring opens or changes ring, removes it
/// when the ring closes, and switches the play camera off meanwhile.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_scene(
    mut commands: Commands,
    state: Res<InventoryRing>,
    world: Option<Res<WorldGeometry>>,
    level: Option<Res<ActiveLevel>>,
    object_meshes: Option<Res<ObjectMeshes>>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PsxMaterial>>,
    scenes: Query<(Entity, &RingScene)>,
    mut game_camera: Query<&mut Camera, (With<GameCamera>, Without<RingCamera>, Without<BackdropCamera>)>,
) {
    let wanted = state.ring.as_ref().zip(level.as_deref());
    let current = scenes.iter().next();
    let stale = match (wanted, current) {
        (Some((ring, level)), Some((_, scene))) => {
            scene.kind != ring.kind || scene.level != level.name || scene.title != (ring.mode == RingMode::Title)
        }
        (None, Some(_)) => true,
        _ => false,
    };
    if stale {
        for (entity, _) in &scenes {
            commands.entity(entity).despawn();
        }
    }
    for mut camera in &mut game_camera {
        camera.is_active = wanted.is_none();
    }
    let (Some((ring, level)), Some(world), Some(object_meshes)) = (wanted, world, object_meshes) else { return };
    if current.is_some() && !stale {
        return;
    }
    let atlas: Handle<Image> = server.load(format!("levels/{}/atlas.png", level.name));
    let root = commands
        .spawn((
            Transform::from_scale(LEVEL_TO_RENDER),
            Visibility::default(),
            RingScene { kind: ring.kind, level: level.name.clone(), title: ring.mode == RingMode::Title },
            Name::new("InventoryRing"),
        ))
        .id();
    // Two cameras with the play camera's projection: the backdrop's view
    // straight down, then the ring's.
    // The title draws its picture behind the ring instead of the backdrop
    // (`0x80032FD4`): `presentation`'s title camera.
    let title = ring.mode == RingMode::Title;
    for (order, layer, name) in [(BACKDROP_ORDER, BACKDROP_LAYER, "Backdrop"), (RING_ORDER, RING_LAYER, "Ring")] {
        if title && order == BACKDROP_ORDER {
            continue;
        }
        let mut camera = commands.spawn((
            Camera3d::default(),
            Camera {
                order,
                clear_color: if order == BACKDROP_ORDER {
                    ClearColorConfig::Custom(Color::BLACK)
                } else {
                    ClearColorConfig::None
                },
                ..default()
            },
            PsxView,
            RenderLayers::layer(layer),
            Transform::default(),
            // Not under the root: a camera keeps an unmirrored transform.
            RingScene { kind: ring.kind, level: level.name.clone(), title: ring.mode == RingMode::Title },
            Name::new(format!("Inventory{name}Camera")),
        ));
        if order == BACKDROP_ORDER {
            camera.insert((BackdropCamera, backdrop_camera()));
        } else {
            camera.insert(RingCamera);
        }
    }
    let data = &world.data;
    if !title
        && let Some(model) = data.models.iter().find(|m| m.id == BACKDROP_OBJECT)
        && let Some(&mesh) = model.meshes.first()
    {
        commands.spawn((
            Mesh3d(meshes.add(Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::default(),
            ))),
            MeshMaterial3d(materials.add(PsxMaterial {
                texture: Some(atlas.clone()),
                fog: false,
                double_sided: true,
                ..default()
            })),
            backdrop_placement(data, model),
            Backdrop { mesh },
            RenderLayers::layer(BACKDROP_LAYER),
            ChildOf(root),
            Name::new("InventoryBackdrop"),
        ));
    }
    let lit = materials.add(PsxMaterial {
        texture: Some(atlas),
        lit: true,
        use_scene_lighting: false,
        fog: false,
        double_sided: true,
        ..default()
    });
    commands.entity(root).insert(RingLight(lit.clone()));
    for (index, &object) in ring.list().iter().enumerate() {
        // The passport also has its open model, shown while it is chosen.
        let models: &[u16] = if object == item::PASSPORT { &[item::PASSPORT, item::PASSPORT_OPEN] } else { &[object] };
        for &model_object in models {
            let Some(model) = data.models.iter().position(|m| m.id == model_object) else { continue };
            let item = commands
                .spawn((
                    RingItem { index, model },
                    Transform::default(),
                    Visibility::default(),
                    RenderLayers::layer(RING_LAYER),
                    ChildOf(root),
                    Name::new(format!("RingItem{model_object}")),
                ))
                .id();
            for (joint, &mesh) in data.models[model].meshes.iter().enumerate() {
                commands.spawn((
                    Mesh3d(object_meshes.0[mesh].clone()),
                    MeshMaterial3d(lit.clone()),
                    Transform::default(),
                    Visibility::default(),
                    RenderLayers::layer(RING_LAYER),
                    RingBone { joint },
                    ChildOf(item),
                ));
            }
        }
    }
}

/// Places the ring's items, joints and camera from the ring's state.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn pose_scene(
    state: Res<InventoryRing>,
    world: Option<Res<WorldGeometry>>,
    mut materials: ResMut<Assets<PsxMaterial>>,
    lights: Query<&RingLight>,
    game_camera: Query<(&Projection, &Camera), (With<GameCamera>, Without<RingCamera>, Without<BackdropCamera>)>,
    mut cameras: Query<
        (&mut Transform, &mut Projection, &mut Camera, Has<RingCamera>),
        (Or<(With<RingCamera>, With<BackdropCamera>)>, Without<GameCamera>, Without<RingItem>, Without<RingBone>),
    >,
    mut items: Query<(&RingItem, &Children, &mut Transform, &mut Visibility), Without<RingBone>>,
    mut bones: Query<(&RingBone, &mut Transform, &mut Visibility), Without<RingItem>>,
) {
    let (Some(ring), Some(world)) = (state.ring.as_ref(), world) else { return };
    let data = &world.data;
    // The ring's view (0x80034B64): from the camera on its axis towards
    // (0, -96, radius), tilted by the ring's camera pitch.
    let eye = position([0, ring.camera_y, ring.camera_z]);
    let target = position([0, -96, i32::from(ring.radius)]);
    let look = (target - eye).normalize_or(Vec3::NEG_Z);
    let tilt = Quat::from_axis_angle(look.cross(Vec3::Y).normalize_or(Vec3::X), turn(ring.camera_pitch));
    let ring_view = Transform::from_translation(render_position(eye)).looking_to(render_position(tilt * look), Vec3::Y);
    if let Ok((projection, camera)) = game_camera.single() {
        for (mut transform, mut own_projection, mut own_camera, is_ring) in &mut cameras {
            *own_projection = projection.clone();
            own_camera.sub_camera_view = camera.sub_camera_view;
            if is_ring {
                *transform = ring_view;
            }
        }
    }
    // The ring's light, as an item light: ambient plus one direction.
    if let Ok(light) = lights.single()
        && let Some(mut material) = materials.get_mut(&light.0)
    {
        let towards = render_position(position([-0x600, 0x100, 0x400])).normalize();
        let mut lighting = material.lighting;
        lighting.ambient = Vec3::splat(0.6);
        lighting.directions[0] = towards;
        lighting.colors[0] = Vec3::splat(0.7);
        lighting.count = 1;
        if material.lighting != lighting {
            material.lighting = lighting;
        }
    }
    let ring_turn = joint_turn([0, ring.yaw, 0]);
    for (item, children, mut transform, mut visibility) in &mut items {
        let Some(&object) = ring.list().get(item.index) else { continue };
        let Some(pose) = state.poses.get(&object) else { continue };
        let model = &data.models[item.model];
        // Each item draws its current object: the passport's open model
        // replaces the closed one while it is chosen.
        let shown = model.id == pose.object;
        *visibility = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if !shown {
            continue;
        }
        let around = (item.index as i32 * i32::from(ring.angle_adder)) as i16;
        let place = Transform::from_rotation(ring_turn * joint_turn([0, around, 0]))
            * Transform::from_translation(position([i32::from(ring.radius), 0, 0]))
            * Transform::from_rotation(joint_turn([pose.pt_xrot, 0x4000, 0]))
            * Transform::from_translation(position([0, pose.y, pose.z]))
            * Transform::from_rotation(joint_turn([pose.x_rot, pose.y_rot, 0]));
        *transform = place;
        let frame = f32::from(pose.frame.max(0));
        let bones_pose = model_pose(data, model, model.animations[0], frame);
        for &child in children {
            let Ok((bone, mut t, mut visible)) = bones.get_mut(child) else { continue };
            if let Some(b) = bones_pose.get(bone.joint) {
                *t = *b;
            }
            let drawn = pose.meshes == -1 || pose.meshes & (1 << bone.joint) != 0;
            *visible = if drawn { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

/// Draws the backdrop as the ripple has it.
fn ripple_backdrop(
    ripple: Res<BackdropRipple>,
    world: Option<Res<WorldGeometry>>,
    mut meshes: ResMut<Assets<Mesh>>,
    backdrops: Query<(Ref<Backdrop>, &Mesh3d)>,
) {
    let Some(world) = world else { return };
    for (backdrop, handle) in &backdrops {
        if !ripple.is_changed() && !backdrop.is_added() {
            continue;
        }
        let Some(mesh) = world.data.meshes.get(backdrop.mesh) else { continue };
        let (vertices, grey): (Vec<_>, Vec<_>) = mesh
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let ([dx, dz], grey) = ripple.vertex(i);
                ([v[0] + dx, v[1], v[2] + dz], grey)
            })
            .unzip();
        meshes.insert(&handle.0, shaded_object_mesh(mesh, &world.data.textures, &vertices, &grey)).ok();
    }
}

/// A binary angle in radians.
fn turn(angle: i16) -> f32 {
    f32::from(angle) * std::f32::consts::TAU / 65536.0
}

/// The ring's text.
#[derive(Component)]
struct RingText;

/// The heading, the item's name and count, and the prompts and arrows, while
/// the ring rests or shows an item (`0x800331D4`, `0x800334DC`; the prompts'
/// places measured from the GPU capture of the open ring).
#[allow(clippy::too_many_arguments)]
fn ring_text(
    mut commands: Commands,
    state: Res<InventoryRing>,
    inventory: Res<Inventory>,
    table: Res<ItemTable>,
    strings: Res<GameStrings>,
    level: Option<Res<ActiveLevel>>,
    mut block: Query<&mut ScreenLines, With<RingText>>,
) {
    use super::ring::Status;
    let mut lines: Vec<ScreenText> = vec![];
    if let Some(ring) = state.ring.as_ref() {
        let status = ring.motion.status;
        let resting = matches!(
            status,
            Status::Open
                | Status::Selecting
                | Status::Selected
                | Status::Deselecting
                | Status::Deselect
                | Status::ClosingItem
        );
        let mode = ring.mode;
        if resting {
            // The title has no heading; game over's options ring is GAME OVER.
            let heading = match (ring.kind, mode) {
                (_, RingMode::Title) => None,
                (RingKind::Options, RingMode::Death) => Some(strings.game(3)),
                (RingKind::Main, _) => Some(strings.game(0)),
                (RingKind::Options, _) => Some(strings.game(1)),
                (RingKind::Keys, _) => Some(strings.game(2)),
            };
            if let Some(heading) = heading {
                lines.push(ScreenText::new(heading, IVec2::new(0, 26), Align::TOP));
            }
            if mode == RingMode::Game {
                // `[` and `]` are the font's up and down arrows.
                let up = ring.kind == RingKind::Options || (ring.kind == RingKind::Main && !inventory.keys.is_empty());
                let down = ring.kind == RingKind::Keys || ring.kind == RingKind::Main;
                if up {
                    lines.push(ScreenText::new("[", IVec2::new(20, 28), Align::TOP_LEFT));
                    lines.push(ScreenText::new("[", IVec2::new(-20, 28), Align::TOP_RIGHT));
                }
                if down {
                    lines.push(ScreenText::new("]", IVec2::new(20, -15), Align::BOTTOM_LEFT));
                    lines.push(ScreenText::new("]", IVec2::new(-20, -15), Align::BOTTOM_RIGHT));
                }
            }
            // "> Select" and "; Go Back": `>` and `;` are the Cross and
            // Triangle. Game over has no Go Back, nor the title's ring
            // until an item is chosen (`0x800317D0`, `0x800321D4`).
            lines.push(ScreenText::new(strings.psx(19), IVec2::new(25, -40), Align::BOTTOM_LEFT));
            let chosen = !matches!(status, Status::Open);
            if mode != RingMode::Death && (mode != RingMode::Title || chosen) {
                lines.push(ScreenText::new(strings.psx(20), IVec2::new(-25, -40), Align::BOTTOM_RIGHT));
            }
        }
        // The open passport shows its page instead of its name, or its
        // load list (`0x800706F8`, `0x800355C8`).
        let passport_open =
            ring.current_item().and_then(|o| state.poses.get(&o)).is_some_and(|p| p.object == item::PASSPORT_OPEN);
        if let Some(label) = ring.page_label {
            lines.push(ScreenText::new(label.text(strings.game(label.string)), IVec2::new(0, -24), Align::BOTTOM));
        }
        if let Some(requester) = &ring.requester {
            lines.extend(requester.lines());
        }
        if resting
            && !ring.rotating
            && !passport_open
            && let Some(object) = ring.current_item()
        {
            let level_index = level.as_deref().and_then(|l| strings.level_number(&l.name)).unwrap_or(0);
            if let Some(data) = table.get(object) {
                lines.push(ScreenText::new(strings.item(&data.name, level_index), IVec2::new(0, -16), Align::BOTTOM));
            }
            if let Some(count) = quantity(object, &inventory, &table) {
                lines.push(ScreenText::new(count, IVec2::new(64, -56), Align::BOTTOM));
            }
        }
    }
    if let Ok(mut block) = block.single_mut() {
        block.set_if_neq(ScreenLines(lines));
    } else {
        commands.spawn((RingText, ScreenLines(lines), Transform::from_xyz(0.0, 0.0, 5.0)));
    }
}

/// The count shown under an item (`0x80033708..0x80033A08`): rounds for a
/// gun (shotgun shells in boxes of six), twice the boxes of clips and shells,
/// grenades and flares as they are, medi packs and keys when more than one.
fn quantity(object: u16, inventory: &Inventory, table: &ItemTable) -> Option<String> {
    let count = i32::from(inventory.count(table, object));
    let ammo = &inventory.ammo;
    Some(match object {
        item::SHOTGUN => format!("{:5}", ammo.shotgun / 6),
        item::AUTOS => format!("{:5}", ammo.autos),
        item::UZIS => format!("{:5}", ammo.uzis),
        item::HARPOON => format!("{:5}", ammo.harpoon),
        item::M16 => format!("{:5}", ammo.m16),
        item::GRENADE_LAUNCHER => format!("{:5}", ammo.grenades),
        item::SHOTGUN_AMMO..=item::M16_AMMO => format!("{}", count * 2),
        item::GRENADES | item::FLARES => format!("{count}"),
        item::SMALL_MEDIPACK | item::LARGE_MEDIPACK | 178..=181 | 197..=200 | 207 | 208 if count >= 2 => {
            format!("{count}")
        }
        _ => return None,
    })
}
