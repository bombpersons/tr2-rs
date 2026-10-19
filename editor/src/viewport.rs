//! The 3D view: camera flight, click selection over the rendered mesh
//! hierarchy, selection boxes, and placing objects as they are dragged.
use crate::{Editor, EditorTarget, gizmo};
use bevy::{
    camera::primitives::Aabb,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
    prelude::*,
};
use bevy_egui::input::EguiWantsInput;
use engine::{
    objects::WorldObject,
    render::{GameCamera, InRoom, view},
    world::{angle, position},
};
use serde_json::Value;

/// The viewport owns keyboard flight after a click in it, until a click on a window.
#[derive(Resource)]
pub(super) struct ViewportFocus {
    pub active: bool,
}
impl Default for ViewportFocus {
    fn default() -> Self {
        Self { active: true }
    }
}

pub struct ViewportPlugin;
impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewportFocus>()
            .init_resource::<EguiWantsInput>()
            .add_systems(
                Update,
                (focus, camera).chain().after(engine::render::LevelSystems::Visibility),
            )
            .add_systems(
                PostUpdate,
                (
                    place.before(bevy::transform::TransformSystems::Propagate),
                    (select, highlight)
                        .chain()
                        .after(gizmo::interact)
                        .after(bevy::transform::TransformSystems::Propagate),
                ),
            );
    }
}

pub(super) fn focus(
    mut focus: ResMut<ViewportFocus>,
    mouse: Res<ButtonInput<MouseButton>>,
    egui: Res<EguiWantsInput>,
) {
    if mouse.any_just_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]) {
        focus.active = !egui.wants_any_pointer_input();
    }
}

/// A level file position (y down) in the editor's world space.
pub fn world(p: [f64; 3]) -> Vec3 {
    view::render_position(Vec3::new(p[0] as f32, -p[1] as f32, p[2] as f32) / 1024.0)
}

/// A world-space offset as a level file offset.
pub fn file_offset(offset: Vec3) -> [f64; 3] {
    let level = view::render_position(offset) * 1024.0;
    [level.x as f64, -level.y as f64, level.z as f64]
}

/// A placement's position as shown now, including any drag in progress.
pub fn placement_position(editor: &Editor, index: usize) -> Option<[f64; 3]> {
    let axis = |a: usize| editor.value(&format!("/placements/{index}/position/{a}")).and_then(Value::as_f64);
    Some([axis(0)?, axis(1)?, axis(2)?])
}

type Hierarchy<'w, 's> =
    Query<'w, 's, (Option<&'static WorldObject>, Option<&'static InRoom>, Option<&'static ChildOf>)>;

/// The placement index (or else the room) a rendered entity belongs to.
fn owner(mut entity: Entity, hierarchy: &Hierarchy) -> (Option<usize>, Option<usize>) {
    let mut room = None;
    while let Ok((object, in_room, parent)) = hierarchy.get(entity) {
        if let Some(object) = object {
            return (Some(object.index), room);
        }
        if let Some(in_room) = in_room {
            room = Some(in_room.0);
        }
        let Some(parent) = parent else { break };
        entity = parent.parent();
    }
    (None, room)
}

/// Click an object to select it; Shift or Ctrl adds or removes it. A click on
/// a room or empty space clears the selection. Rooms are hit from the side
/// their faces are drawn for, so objects inside a room are clicked through
/// its outer shell.
#[allow(clippy::too_many_arguments)]
pub(super) fn select(
    mut editor: ResMut<Editor>,
    focus: Res<ViewportFocus>,
    handles: Res<gizmo::Handles>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    mut ray_cast: MeshRayCast,
    hierarchy: Hierarchy,
    level: Option<Res<engine::world::WorldGeometry>>,
) {
    if editor.mesh.on {
        return;
    }
    if !focus.active || handles.busy() || editor.leaving.is_some() || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(Window::cursor_position) else { return };
    let Ok((camera, transform)) = cameras.single() else { return };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else { return };
    let objects = |entity| owner(entity, &hierarchy).0.is_some();
    let settings = MeshRayCastSettings::default().with_filter(&objects);
    let object = ray_cast.cast_ray(ray, &settings).first().map(|(e, h)| (owner(*e, &hierarchy).0, h.distance));
    let room = level.and_then(|level| crate::mesh::room_distance(&level.data, ray));
    let hit = object.filter(|&(_, distance)| room.is_none_or(|room| distance <= room)).and_then(|(index, _)| index);
    let extend =
        keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::ControlRight]);
    editor.click(hit, extend);
}

/// Boxes around selected objects, drawn through walls; the active one is brighter.
pub(super) fn highlight(
    editor: Res<Editor>,
    hierarchy: Hierarchy,
    meshes: Query<(Entity, &Aabb, &GlobalTransform), With<Mesh3d>>,
    mut gizmos: Gizmos<gizmo::OnTop>,
) {
    if editor.selection.is_empty() || editor.mesh.on {
        return;
    }
    let mut bounds = std::collections::HashMap::<usize, (Vec3, Vec3)>::new();
    for (entity, aabb, transform) in &meshes {
        let Some(index) = owner(entity, &hierarchy).0.filter(|i| editor.selection.contains(i)) else { continue };
        let (min, max) = bounds.entry(index).or_insert((Vec3::INFINITY, Vec3::NEG_INFINITY));
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let p = transform.transform_point(Vec3::from(aabb.center) + Vec3::from(aabb.half_extents) * sign);
            *min = min.min(p);
            *max = max.max(p);
        }
    }
    for &index in &editor.selection {
        let active = editor.active() == Some(index);
        let color = if active { Color::srgb(1.0, 0.75, 0.1) } else { Color::srgb(0.85, 0.45, 0.1) };
        let (center, size) = match bounds.get(&index) {
            Some(&(min, max)) => ((min + max) * 0.5, (max - min).max(Vec3::splat(0.02))),
            // Objects without a model still show where they are.
            None => match placement_position(&editor, index) {
                Some(p) => (world(p), Vec3::splat(0.25)),
                None => continue,
            },
        };
        gizmos.cube(Transform::from_translation(center).with_scale(size), color);
    }
}

/// Show dragged and typed positions and headings on the objects at once.
pub(super) fn place(editor: Res<Editor>, mut objects: Query<(&WorldObject, &mut Transform)>) {
    for (object, mut transform) in &mut objects {
        let Some(p) = placement_position(&editor, object.index) else { continue };
        let heading = editor.value(&format!("/placements/{}/rotation", object.index)).and_then(Value::as_u64);
        let target = Transform::from_translation(position([p[0] as i32, p[1] as i32, p[2] as i32]))
            .with_rotation(Quat::from_rotation_y(angle(heading.unwrap_or(0) as u16)));
        if transform.translation != target.translation || transform.rotation != target.rotation {
            transform.translation = target.translation;
            transform.rotation = target.rotation;
        }
    }
}

/// The selection's centre and size, for framing.
fn selection_bounds(editor: &Editor, level: Option<&engine::world::WorldGeometry>) -> Option<(Vec3, f32)> {
    let points: Vec<_> = if editor.mesh.on {
        crate::mesh::selection_points(editor, level)
    } else {
        editor.selection.iter().filter_map(|&i| placement_position(editor, i)).map(world).collect()
    };
    if points.is_empty() {
        return None;
    }
    let center = points.iter().sum::<Vec3>() / points.len() as f32;
    let spread = points.iter().map(|p| p.distance(center)).fold(0.0, f32::max);
    Some((center, spread * 1.5 + 4.0))
}

#[allow(clippy::too_many_arguments)]
pub fn camera(
    mut editor: ResMut<Editor>,
    mut cameras: Query<&mut Transform, With<GameCamera>>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    focus: Res<ViewportFocus>,
    egui: Res<EguiWantsInput>,
    target: Res<EditorTarget>,
    level: Option<Res<engine::world::WorldGeometry>>,
) {
    let Ok(mut transform) = cameras.single_mut() else { return };
    let typing = egui.wants_any_keyboard_input();
    if focus.active && !typing && keys.just_pressed(KeyCode::KeyF) && !ctrl(&keys) {
        editor.frame = true;
    }
    if editor.frame {
        editor.frame = false;
        let room = editor.overview || editor.frame_room;
        let selected = if room { None } else { selection_bounds(&editor, level.as_deref()) };
        let (center, distance) = selected.unwrap_or_else(|| room_bounds(&editor));
        editor.overview = false;
        editor.frame_room = false;
        // camera_transform takes level space; this is its own inverse.
        let center = view::render_position(center);
        *transform =
            view::camera_transform(center + Vec3::new(distance * 0.7, distance * 0.8, -distance * 0.7), center);
    }
    if target.headless || !focus.active || editor.leaving.is_some() || ctrl(&keys) {
        return;
    }
    if mouse.pressed(MouseButton::Right) {
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        yaw -= motion.delta.x * 0.003;
        pitch = (pitch - motion.delta.y * 0.003).clamp(-1.54, 1.54);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }
    if mouse.pressed(MouseButton::Middle) {
        let pan = (*transform.right() * -motion.delta.x + *transform.up() * motion.delta.y) * 0.015;
        transform.translation += pan;
    }
    if !egui.wants_any_pointer_input() {
        let scroll_scale = if scroll.unit == MouseScrollUnit::Pixel { 0.01 } else { 0.75 };
        let zoom = *transform.forward() * (scroll.delta.y * scroll_scale).clamp(-20.0, 20.0);
        transform.translation += zoom;
    }
    if typing {
        return;
    }
    let forward = *transform.forward();
    let right = *transform.right();
    let mut delta = Vec3::ZERO;
    for (key, vector) in [
        (KeyCode::KeyW, forward),
        (KeyCode::KeyS, -forward),
        (KeyCode::KeyA, -right),
        (KeyCode::KeyD, right),
        (KeyCode::KeyQ, -Vec3::Y),
        (KeyCode::KeyE, Vec3::Y),
    ] {
        if keys.pressed(key) {
            delta += vector;
        }
    }
    let speed = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { 30.0 } else { 6.0 };
    transform.translation += delta.normalize_or_zero() * time.delta_secs() * speed;
}

fn ctrl(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight])
}

/// The room mesh mode edits, else the Data window's room (or the first room), for framing.
fn room_bounds(editor: &Editor) -> (Vec3, f32) {
    let mut parts = editor.pointer.split('/').skip(1);
    let index = match (parts.next(), parts.next().and_then(|s| s.parse::<usize>().ok())) {
        _ if editor.mesh.on || editor.frame_room => editor.mesh.room,
        (Some("rooms"), Some(index)) => index,
        _ => 0,
    };
    let room = &editor.doc.value()["rooms"][index];
    let origin: [i32; 3] = serde_json::from_value(room["origin"].clone()).unwrap_or([0; 3]);
    let x = room["x_sectors"].as_u64().unwrap_or(4) as f64 * 1024.0;
    let z = room["z_sectors"].as_u64().unwrap_or(4) as f64 * 1024.0;
    let y = (room["top"].as_i64().unwrap_or(0) + room["bottom"].as_i64().unwrap_or(0)) as f64 / 2.0;
    let center = world([origin[0] as f64 + x / 2.0, y, origin[2] as f64 + z / 2.0]);
    (center, (x.max(z) / 1024.0) as f32 * 0.9 + 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo, visibility::SetViewVisibility},
        ecs::system::RunSystemOnce,
    };

    #[test]
    fn file_and_world_offsets_round_trip() {
        let p = [2560.0, -512.0, 4608.0];
        let w = world(p);
        assert_eq!(w, view::render_position(position([2560, -512, 4608])));
        let back = file_offset(w);
        assert!(back.iter().zip(p).all(|(a, b)| (a - b).abs() < 1e-3), "{back:?}");
    }

    #[test]
    fn viewport_clicks_select_placements_and_clear_on_rooms_and_empty_space() {
        // Ray casts run on the compute pool.
        bevy::tasks::ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ViewportFocus>()
            .init_resource::<gizmo::Handles>()
            .insert_resource(crate::sandbox());
        let mut window = Window { resolution: (1280, 800).into(), ..default() };
        window.set_cursor_position(Some(Vec2::new(640.0, 400.0)));
        app.world_mut().spawn(window);
        app.world_mut().spawn((
            GameCamera,
            GlobalTransform::IDENTITY,
            Camera {
                computed: ComputedCameraValues {
                    clip_from_view: PerspectiveProjection {
                        fov: 1.0,
                        aspect_ratio: 1280.0 / 800.0,
                        near: 0.1,
                        ..default()
                    }
                    .get_clip_from_view(),
                    target_info: Some(RenderTargetInfo { physical_size: UVec2::new(1280, 800), scale_factor: 1.0 }),
                    ..default()
                },
                ..default()
            },
        ));
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(Cuboid::new(2.0, 2.0, 2.0));
        let room = app
            .world_mut()
            .spawn((
                Mesh3d(mesh.clone()),
                InRoom(0),
                GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -8.0)),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
            ))
            .id();
        let root = app.world_mut().spawn((WorldObject::new(1, 1, 0, 0), InRoom(0))).id();
        // Posed model parts are children; mirrored parts still hit through back faces.
        let child = app
            .world_mut()
            .spawn((
                ChildOf(root),
                Mesh3d(mesh),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, -4.0).with_scale(Vec3::new(1.0, 1.0, -1.0))),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
                bevy::picking::mesh_picking::ray_cast::RayCastBackfaces,
            ))
            .id();
        for entity in [room, child] {
            app.world_mut().get_mut::<ViewVisibility>(entity).unwrap().set_visible();
        }
        let click = |app: &mut App, shift: bool| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            if shift {
                keys.press(KeyCode::ShiftLeft);
            }
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.reset_all();
            mouse.press(MouseButton::Left);
            app.world_mut().run_system_once(select).unwrap();
        };
        app.world_mut().resource_mut::<Editor>().selection = vec![0];
        click(&mut app, false);
        assert_eq!(app.world().resource::<Editor>().selection, [1]);
        click(&mut app, true);
        assert!(app.world().resource::<Editor>().selection.is_empty(), "shift-click toggles");
        click(&mut app, true);
        app.world_mut().resource_mut::<ViewportFocus>().active = false;
        app.world_mut().despawn(child);
        click(&mut app, false);
        assert_eq!(app.world().resource::<Editor>().selection, [1], "clicks on windows don't reach the scene");
        app.world_mut().resource_mut::<ViewportFocus>().active = true;
        click(&mut app, true);
        assert_eq!(app.world().resource::<Editor>().selection, [1], "shift-clicking a room keeps the selection");
        click(&mut app, false);
        assert!(app.world().resource::<Editor>().selection.is_empty(), "clicking a room clears the selection");
        app.world_mut().resource_mut::<Editor>().selection = vec![1];
        app.world_mut().despawn(room);
        click(&mut app, false);
        assert!(app.world().resource::<Editor>().selection.is_empty(), "clicking empty space clears it");
        assert!(!app.world().resource::<Editor>().doc.is_dirty());
    }
}
