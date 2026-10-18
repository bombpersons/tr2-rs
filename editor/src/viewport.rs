//! Editor-only viewport input and selection over the rendered mesh hierarchy.
use crate::{Editor, ui};
use bevy::{
    camera::primitives::Aabb,
    picking::{
        hover::HoverMap,
        mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
        pointer::PointerId,
    },
    prelude::*,
};
use engine::{
    objects::WorldObject,
    render::{GameCamera, InRoom},
};

#[derive(Resource)]
pub(super) struct ViewportFocus {
    pub active: bool,
}
impl Default for ViewportFocus {
    fn default() -> Self {
        Self { active: true }
    }
}

pub(super) fn focus(
    mut focus: ResMut<ViewportFocus>,
    editor: Res<Editor>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    hover: Res<HoverMap>,
    nodes: Query<(), With<Node>>,
) {
    if !mouse.any_just_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]) {
        return;
    }
    let over_ui = hover.get(&PointerId::Mouse).is_some_and(|hits| hits.keys().any(|&e| nodes.contains(e)));
    focus.active = editor.input.is_none()
        && !over_ui
        && windows.single().is_ok_and(|w| w.focused && w.cursor_position().is_some_and(|p| p.x >= ui::PANEL));
}

type Hierarchy<'w, 's> =
    Query<'w, 's, (Option<&'static WorldObject>, Option<&'static InRoom>, Option<&'static ChildOf>)>;

fn pointer_for(mut entity: Entity, hierarchy: &Hierarchy) -> Option<String> {
    let mut room = None;
    while let Ok((object, in_room, parent)) = hierarchy.get(entity) {
        if let Some(object) = object {
            return Some(format!("/placements/{}", object.index));
        }
        if let Some(in_room) = in_room {
            room = Some(in_room.0);
        }
        let Some(parent) = parent else { break };
        entity = parent.parent();
    }
    room.map(|r| format!("/rooms/{r}"))
}

pub(super) fn select(
    mut editor: ResMut<Editor>,
    focus: Res<ViewportFocus>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
    mut ray_cast: MeshRayCast,
    hierarchy: Hierarchy,
) {
    if !focus.active || editor.input.is_some() || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(Window::cursor_position) else { return };
    let Ok((camera, transform)) = cameras.single() else { return };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else { return };
    let Some((entity, _)) = ray_cast.cast_ray(ray, &MeshRayCastSettings::default()).first() else {
        editor.status = "Viewport focused. WASD moves; hold right mouse to look.".into();
        return;
    };
    if let Some(pointer) = pointer_for(*entity, &hierarchy) {
        editor.status = format!("Selected {pointer}. F frames selection; edit its fields on the left.");
        editor.pointer = pointer;
        editor.selected = 0;
        editor.page = 0;
    }
}

pub(super) fn highlight(
    editor: Res<Editor>,
    hierarchy: Hierarchy,
    meshes: Query<(Entity, &Aabb, &GlobalTransform), With<Mesh3d>>,
    mut gizmos: Gizmos,
) {
    let selected = ui::selected_pointer(&editor);
    let mut parts = selected.split('/').skip(1);
    let (Some(table @ ("placements" | "rooms")), Some(index)) = (parts.next(), parts.next()) else { return };
    let pointer = format!("/{table}/{index}");
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (entity, bounds, transform) in &meshes {
        if pointer_for(entity, &hierarchy).as_deref() != Some(&pointer) {
            continue;
        }
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let p = transform.transform_point(
                        Vec3::from(bounds.center) + Vec3::from(bounds.half_extents) * Vec3::new(x, y, z),
                    );
                    min = min.min(p);
                    max = max.max(p);
                }
            }
        }
    }
    if min.is_finite() {
        gizmos.cube(
            Transform::from_translation((min + max) * 0.5).with_scale((max - min).max(Vec3::splat(0.02))),
            Color::srgb(1.0, 0.75, 0.1),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo, Viewport, visibility::SetViewVisibility},
        ecs::system::RunSystemOnce,
    };

    #[test]
    fn viewport_click_picks_the_nearest_placement_through_its_mesh_children() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ViewportFocus>();
        app.insert_resource(Editor {
            doc: engine::editor::LevelDocument::open(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../engine/assets/levels/sandbox/level.json"
            ))
            .unwrap(),
            pointer: String::new(),
            selected: 0,
            page: 0,
            status: String::new(),
            input: None,
            revision: 0,
            preview: false,
            frame: false,
            recent: vec![],
            exit_requested: false,
            overview: false,
        });
        let mut window = Window { resolution: (1280, 800).into(), ..default() };
        window.set_cursor_position(Some(Vec2::new(890.0, 400.0)));
        app.world_mut().spawn(window);
        app.world_mut().spawn((
            GameCamera,
            GlobalTransform::IDENTITY,
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(500, 0),
                    physical_size: UVec2::new(780, 800),
                    ..default()
                }),
                computed: ComputedCameraValues {
                    clip_from_view: PerspectiveProjection {
                        fov: 1.0,
                        aspect_ratio: 780.0 / 800.0,
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
        let visibility = ViewVisibility::default();
        let room = app
            .world_mut()
            .spawn((
                Mesh3d(mesh.clone()),
                InRoom(0),
                GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -8.0)),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                InheritedVisibility::VISIBLE,
                visibility,
            ))
            .id();
        let root = app.world_mut().spawn((WorldObject::new(1, 1, 0, 0), InRoom(0))).id();
        let child = app
            .world_mut()
            .spawn((
                ChildOf(root),
                Mesh3d(mesh),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, -4.0).with_scale(Vec3::new(1.0, 1.0, -1.0))),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                InheritedVisibility::VISIBLE,
                visibility,
                bevy::picking::mesh_picking::ray_cast::RayCastBackfaces,
            ))
            .id();
        for entity in [room, child] {
            app.world_mut().get_mut::<ViewVisibility>(entity).unwrap().set_visible();
        }
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.world_mut().run_system_once(select).unwrap();
        let editor = app.world().resource::<Editor>();
        assert_eq!(editor.pointer, "/placements/1");
        assert!(!editor.doc.is_dirty());
        app.world_mut().resource_mut::<ViewportFocus>().active = false;
        *app.world_mut().get_mut::<ViewVisibility>(child).unwrap() = ViewVisibility::default();
        app.world_mut().run_system_once(select).unwrap();
        assert_eq!(app.world().resource::<Editor>().pointer, "/placements/1");
        app.world_mut().resource_mut::<ViewportFocus>().active = true;
        app.world_mut().run_system_once(select).unwrap();
        assert_eq!(app.world().resource::<Editor>().pointer, "/rooms/0");
        app.world_mut().despawn(room);
        app.world_mut().run_system_once(select).unwrap();
        assert!(app.world().resource::<Editor>().status.contains("Viewport focused"));
        app.init_resource::<HoverMap>();
        app.world_mut().run_system_once(focus).unwrap();
        assert!(app.world().resource::<ViewportFocus>().active);
        // Help panels, animation controls and modal backdrops are all UI nodes.
        let ui = app.world_mut().spawn(Node::default()).id();
        app.world_mut()
            .resource_mut::<HoverMap>()
            .entry(PointerId::Mouse)
            .or_default()
            .insert(ui, bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
        app.world_mut().run_system_once(focus).unwrap();
        assert!(!app.world().resource::<ViewportFocus>().active);
        app.world_mut().resource_mut::<HoverMap>().clear();
        app.world_mut()
            .query::<&mut Window>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(100.0, 400.0)));
        app.world_mut().run_system_once(focus).unwrap();
        assert!(!app.world().resource::<ViewportFocus>().active);
        app.world_mut()
            .query::<&mut Window>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(890.0, 400.0)));
        app.world_mut().run_system_once(focus).unwrap();
        assert!(app.world().resource::<ViewportFocus>().active);
    }
}
