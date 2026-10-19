//! A move/turn handle around the selection's centre. Arrows move along one
//! level axis; the ring turns every selected object about the centre. Hold
//! Ctrl to snap to quarter-sector clicks (256 units) and 15 degrees.
use crate::{
    Editor,
    edit::TURN,
    viewport::{ViewportFocus, file_offset, placement_position, world},
};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use engine::render::GameCamera;
use serde_json::Value;
use std::f32::consts::{FRAC_PI_2, TAU};

pub const SNAP_UNITS: f64 = 256.0;
pub const SNAP_DEGREES: f32 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Level axis 0 (x), 1 (y, down) or 2 (z).
    Axis(usize),
    Ring,
}

struct Drag {
    handle: Handle,
    pivot: Vec3,
    /// Arrow parameter or ring angle where the drag began.
    from: f32,
    /// Each object's index, position and heading before the drag.
    start: Vec<(usize, [f64; 3], i64)>,
}

#[derive(Resource, Default)]
pub struct Handles {
    hovered: Option<Handle>,
    drag: Option<Drag>,
}
impl Handles {
    /// The pointer is on a handle, so clicks don't select through it.
    pub fn busy(&self) -> bool {
        self.hovered.is_some() || self.drag.is_some()
    }
}

/// Lines drawn over the scene: handles and selection boxes.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct OnTop;

pub struct GizmoPlugin;
impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Handles>()
            .init_gizmo_group::<OnTop>()
            .add_systems(Startup, |mut store: ResMut<GizmoConfigStore>| {
                let (config, _) = store.config_mut::<OnTop>();
                config.depth_bias = -1.0;
                config.line.width = 3.0;
            })
            .add_systems(PostUpdate, (interact, draw).chain().after(bevy::transform::TransformSystems::Propagate));
    }
}

/// An arrow's direction in world space: along positive level x and z, and
/// up (toward negative level y) for height.
fn direction(axis: usize) -> Vec3 {
    world([[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]][axis]).normalize()
}

/// Where the handles are and how big they look.
fn layout(editor: &Editor, camera: &GlobalTransform) -> Option<(Vec3, f32)> {
    let points: Vec<_> = editor.selection.iter().filter_map(|&i| placement_position(editor, i)).map(world).collect();
    if points.is_empty() {
        return None;
    }
    let pivot = points.iter().sum::<Vec3>() / points.len() as f32;
    Some((pivot, camera.translation().distance(pivot).max(0.5) * 0.15))
}

/// The point on the line `origin + t * direction` closest to `ray`.
fn along(origin: Vec3, direction: Vec3, ray: Ray3d) -> f32 {
    let w = origin - ray.origin;
    let (b, c) = (direction.dot(*ray.direction), ray.direction.length_squared());
    let denominator = direction.length_squared() * c - b * b;
    if denominator.abs() < 1e-6 {
        return 0.0;
    }
    (b * ray.direction.dot(w) - c * direction.dot(w)) / denominator
}

/// The heading (level angle about the vertical) of the cursor around `pivot`.
fn heading(pivot: Vec3, ray: Ray3d) -> Option<f32> {
    let distance = ray.intersect_plane(pivot, InfinitePlane3d::new(Vec3::Y))?;
    let offset = file_offset(ray.get_point(distance) - pivot);
    Some((offset[0] as f32).atan2(offset[2] as f32))
}

fn ring_points(pivot: Vec3, radius: f32) -> impl Iterator<Item = Vec3> {
    (0..=48).map(move |i| {
        let a = i as f32 / 48.0 * TAU;
        pivot + Vec3::new(a.cos(), 0.0, a.sin()) * radius
    })
}

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    mut editor: ResMut<Editor>,
    mut handles: ResMut<Handles>,
    focus: Res<ViewportFocus>,
    egui: Res<EguiWantsInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
) {
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let (Ok((camera, eye)), Some(cursor)) = (cameras.single(), cursor) else {
        handles.hovered = None;
        return;
    };
    let Ok(ray) = camera.viewport_to_world(eye, cursor) else { return };
    let snap = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if let Some(drag) = &handles.drag {
        if mouse.pressed(MouseButton::Left) {
            let edits = drag_edits(drag, ray, snap);
            editor.edit(edits, true);
        } else {
            editor.commit();
            handles.drag = None;
        }
        return;
    }
    handles.hovered = None;
    let Some((pivot, size)) = layout(&editor, eye) else { return };
    if egui.wants_any_pointer_input() || editor.leaving.is_some() {
        return;
    }
    let screen = |p: Vec3| camera.world_to_viewport(eye, p).ok();
    let mut best = (12.0, None);
    for axis in 0..3 {
        if let (Some(a), Some(b)) = (screen(pivot), screen(pivot + direction(axis) * size)) {
            let distance = segment_distance(cursor, a, b);
            if distance < best.0 {
                best = (distance, Some(Handle::Axis(axis)));
            }
        }
    }
    let ring: Vec<_> = ring_points(pivot, size * 1.3).filter_map(screen).collect();
    for pair in ring.windows(2) {
        let distance = segment_distance(cursor, pair[0], pair[1]);
        if distance < best.0 {
            best = (distance, Some(Handle::Ring));
        }
    }
    handles.hovered = best.1;
    let Some(handle) = best.1 else { return };
    if !mouse.just_pressed(MouseButton::Left) || !focus.active {
        return;
    }
    let from = match handle {
        Handle::Axis(axis) => along(pivot, direction(axis), ray),
        Handle::Ring => {
            let Some(angle) = heading(pivot, ray) else { return };
            angle
        }
    };
    let start = editor
        .selection
        .iter()
        .filter_map(|&index| {
            let p = placement_position(&editor, index)?;
            let rotation = editor.value(&format!("/placements/{index}/rotation")).and_then(Value::as_i64)?;
            Some((index, p, rotation))
        })
        .collect();
    handles.drag = Some(Drag { handle, pivot, from, start });
}

/// The fields a drag sets so far, measured from where it began.
fn drag_edits(drag: &Drag, ray: Ray3d, snap: bool) -> Vec<(String, Value)> {
    let mut edits = vec![];
    match drag.handle {
        Handle::Axis(axis) => {
            let moved = along(drag.pivot, direction(axis), ray) - drag.from;
            let mut offset = file_offset(direction(axis) * moved)[axis];
            if snap {
                offset = (offset / SNAP_UNITS).round() * SNAP_UNITS;
            }
            for &(index, p, _) in &drag.start {
                edits.push((format!("/placements/{index}/position/{axis}"), Value::from((p[axis] + offset).round() as i64)));
            }
        }
        Handle::Ring => {
            let Some(angle) = heading(drag.pivot, ray) else { return edits };
            let mut turn = (angle - drag.from).rem_euclid(TAU);
            if snap {
                let step = SNAP_DEGREES.to_radians();
                turn = (turn / step).round() * step;
            }
            let (sin, cos) = (turn.sin() as f64, turn.cos() as f64);
            let pivot = file_offset(drag.pivot);
            for &(index, p, rotation) in &drag.start {
                let (dx, dz) = (p[0] - pivot[0], p[2] - pivot[2]);
                let x = pivot[0] + dx * cos + dz * sin;
                let z = pivot[2] - dx * sin + dz * cos;
                let rotation = (rotation + (turn / TAU * TURN as f32).round() as i64).rem_euclid(TURN);
                edits.push((format!("/placements/{index}/position/0"), Value::from(x.round() as i64)));
                edits.push((format!("/placements/{index}/position/2"), Value::from(z.round() as i64)));
                edits.push((format!("/placements/{index}/rotation"), Value::from(rotation)));
            }
        }
    }
    edits
}

fn draw(editor: Res<Editor>, handles: Res<Handles>, cameras: Query<&GlobalTransform, With<GameCamera>>, mut gizmos: Gizmos<OnTop>) {
    let Ok(eye) = cameras.single() else { return };
    let Some((pivot, size)) = layout(&editor, eye) else { return };
    let current = handles.drag.as_ref().map(|d| d.handle).or(handles.hovered);
    let shade = |handle: Handle, color: Color| if current == Some(handle) { Color::srgb(1.0, 0.95, 0.3) } else { color };
    let colors = [Color::srgb(0.95, 0.25, 0.25), Color::srgb(0.3, 0.9, 0.3), Color::srgb(0.3, 0.5, 1.0)];
    for (axis, color) in colors.into_iter().enumerate() {
        gizmos.arrow(pivot, pivot + direction(axis) * size, shade(Handle::Axis(axis), color)).with_tip_length(size * 0.2);
    }
    gizmos.circle(
        Isometry3d::new(pivot, Quat::from_rotation_x(FRAC_PI_2)),
        size * 1.3,
        shade(Handle::Ring, Color::srgb(0.85, 0.85, 0.95)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ray(origin: Vec3, toward: Vec3) -> Ray3d {
        Ray3d::new(origin, Dir3::new(toward - origin).unwrap())
    }

    #[test]
    fn arrows_move_every_object_along_one_level_axis_and_snap() {
        let pivot = world([0.0, 0.0, 0.0]);
        let start = vec![(0, [1000.0, -200.0, 50.0], 0), (1, [-300.0, 0.0, 70.0], 0)];
        // Level z runs along world -Z; the cursor ray crosses the arrow 1.5 m along it.
        let drag = Drag { handle: Handle::Axis(2), pivot, from: 0.0, start };
        let eye = Vec3::new(5.0, 0.0, 0.0);
        let edits = drag_edits(&drag, ray(eye, pivot + direction(2) * 1.5), false);
        assert_eq!(edits[0], ("/placements/0/position/2".into(), Value::from(50 + 1536)));
        assert_eq!(edits[1], ("/placements/1/position/2".into(), Value::from(70 + 1536)));
        let edits = drag_edits(&drag, ray(eye, pivot + direction(2) * 1.4), true);
        assert_eq!(edits[0].1, Value::from(50 + 1536), "snaps the offset to 256 units");
        // Level y is down: the arrow points up the screen toward negative y.
        let drag = Drag { handle: Handle::Axis(1), ..drag };
        let edits = drag_edits(&drag, ray(eye, pivot + direction(1) * 0.5), false);
        assert_eq!(edits[0].1, Value::from(-200 - 512));
        assert!(direction(1).y > 0.0);
    }

    #[test]
    fn the_ring_turns_objects_about_the_centre_and_adds_to_their_headings() {
        let pivot = world([0.0, 0.0, 0.0]);
        let start = vec![(0, [0.0, 0.0, 1024.0], 0), (1, [0.0, 0.0, -1024.0], 60000)];
        let eye = Vec3::new(0.0, 10.0, 0.0);
        let from = heading(pivot, ray(eye, world([0.0, 0.0, 2048.0]))).unwrap();
        let drag = Drag { handle: Handle::Ring, pivot, from, start };
        // A quarter turn: forward (+z) becomes +x, as for a heading of 16384.
        let edits = drag_edits(&drag, ray(eye, world([2048.0, 0.0, 0.0])), false);
        let get = |name: &str| edits.iter().find(|(p, _)| p == name).unwrap().1.as_i64().unwrap();
        assert_eq!((get("/placements/0/position/0"), get("/placements/0/position/2")), (1024, 0));
        assert_eq!((get("/placements/1/position/0"), get("/placements/1/position/2")), (-1024, 0));
        assert_eq!(get("/placements/0/rotation"), 16384);
        assert_eq!(get("/placements/1/rotation"), (60000 + 16384) % 65536);
        let forward = Quat::from_rotation_y(engine::world::angle(16384)) * Vec3::Z;
        assert!((forward - Vec3::X).length() < 1e-5, "headings turn level +z toward +x");
    }
}
