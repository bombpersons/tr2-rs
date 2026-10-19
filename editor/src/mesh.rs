//! Mesh mode: select a room's vertices and faces, move them on the sector
//! grid, extrude floors and walls, and paint textures.
//!
//! Every operation is one of the engine's grid-aware edits
//! (`engine::editor::move_vertices`, `extrude`, `push_wall`, `paint`), which
//! re-derive the sectors they touch so collision stays equal to what is drawn,
//! applied to the document as one undo step. The view, overlays and picking
//! read the previewed level, which is the document as last applied.
use crate::{
    Editor,
    gizmo::{self, OnTop},
    viewport::{ViewportFocus, file_offset, world},
};
use bevy::prelude::*;
use bevy_egui::{egui, input::EguiWantsInput};
use engine::{
    editor::{self as authoring, Face, SECTOR, STEP, Surface, sector_face},
    render::GameCamera,
    world::{Level, Room, WorldGeometry},
};
use std::collections::BTreeSet;

/// What mesh mode has selected, and its tools.
#[derive(Default)]
pub struct MeshEdit {
    pub on: bool,
    pub room: usize,
    pub vertices: BTreeSet<usize>,
    pub faces: BTreeSet<Face>,
    /// The last element clicked, shown in the Mesh panel.
    pub active: Option<Element>,
    /// The texture-table entry that painting applies.
    pub brush: Option<usize>,
    /// Clicking a face paints it instead of selecting it.
    pub paint: bool,
    /// The snapped offset of a gizmo move in progress, applied on release.
    pub dragging: Option<[i32; 3]>,
    pub hover: Option<(usize, Element)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Vertex(usize),
    Face(Face),
}

impl Element {
    /// `v4`, `q12` or `t3`, as automation names them.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let index = text.get(1..)?.parse().ok()?;
        Some(match text.as_bytes()[0] {
            b'v' => Self::Vertex(index),
            b'q' => Self::Face(Face::Quad(index)),
            b't' => Self::Face(Face::Triangle(index)),
            _ => return None,
        })
    }
    pub fn name(self) -> String {
        match self {
            Self::Vertex(i) => format!("v{i}"),
            Self::Face(Face::Quad(i)) => format!("q{i}"),
            Self::Face(Face::Triangle(i)) => format!("t{i}"),
        }
    }
}

pub struct MeshPlugin;
impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, keys.after(crate::viewport::focus)).add_systems(
            PostUpdate,
            (pick, draw).chain().after(gizmo::interact).after(bevy::transform::TransformSystems::Propagate),
        );
    }
}

impl Editor {
    /// Enter or leave mesh mode. Entering edits the active object's room, or
    /// the room open in the Data window.
    pub fn set_mesh_mode(&mut self, on: bool) {
        self.commit();
        self.mesh.on = on;
        self.mesh.dragging = None;
        if on {
            let pointer_room = self.pointer.strip_prefix("/rooms/").and_then(|p| p.split('/').next()?.parse().ok());
            let object_room =
                self.active().and_then(|i| self.doc.value()["placements"][i]["room"].as_u64()).map(|r| r as usize);
            if let Some(room) = object_room.or(pointer_room) {
                self.mesh_room(room);
            }
            self.status = format!("Mesh mode: room {} (Tab returns to objects)", self.mesh.room);
        } else {
            self.status = "Object mode".into();
        }
    }

    /// Edit another room, clearing the mesh selection.
    pub fn mesh_room(&mut self, room: usize) {
        let rooms = self.doc.value()["rooms"].as_array().map_or(0, Vec::len);
        if room >= rooms || room == self.mesh.room {
            return;
        }
        self.mesh.room = room;
        self.mesh_clear();
    }

    pub fn mesh_clear(&mut self) {
        self.mesh.vertices.clear();
        self.mesh.faces.clear();
        self.mesh.active = None;
    }

    /// Click semantics as for objects: a plain click selects only `hit`;
    /// `extend` toggles it. A click in another room switches to it.
    pub fn mesh_click(&mut self, room: usize, hit: Option<Element>, extend: bool) {
        if room != self.mesh.room {
            self.mesh_room(room);
        }
        if !extend {
            self.mesh_clear();
        }
        let Some(hit) = hit else { return };
        let added = match hit {
            Element::Vertex(v) => self.mesh.vertices.insert(v) || !self.mesh.vertices.remove(&v),
            Element::Face(f) => self.mesh.faces.insert(f) || !self.mesh.faces.remove(&f),
        };
        self.mesh.active = added.then_some(hit);
    }

    /// Every selected vertex, including the corners of selected faces.
    pub fn mesh_vertices(&self, room: &Room) -> BTreeSet<usize> {
        let faces: Vec<_> = self.mesh.faces.iter().copied().collect();
        let mut set = authoring::face_vertices(room, &faces);
        set.extend(self.mesh.vertices.iter().filter(|&&v| v < room.vertices.len()));
        set
    }

    /// Run a grid-aware edit on a copy of the level and keep it as one undo step.
    fn mesh_edit<T>(&mut self, done: &str, op: impl FnOnce(&mut Level, usize) -> Result<T, String>) -> Option<T> {
        self.commit();
        let steps = self.doc.undo_steps();
        let result = self.doc.level().and_then(|mut level| {
            let value = op(&mut level, self.mesh.room)?;
            self.doc.apply_level(&level)?;
            Ok(value)
        });
        match result {
            Ok(value) if self.doc.undo_steps() == steps => {
                self.status = "Nothing changed".into();
                Some(value)
            }
            Ok(value) => {
                self.status = format!("{done} - Ctrl+Z to undo");
                self.revision += 1;
                self.preview = true;
                Some(value)
            }
            Err(error) => {
                self.status = error;
                None
            }
        }
    }

    /// Move the selection by a snapped offset (whole sectors across, 256-unit steps up and down).
    pub fn mesh_move(&mut self, delta: [i32; 3]) {
        if delta == [0; 3] {
            return;
        }
        let selection = self.mesh.clone_selection();
        self.mesh_edit("Moved", |level, room| {
            let vertices = selection.vertices(&level.rooms[room]);
            if vertices.is_empty() {
                return Err("Select vertices or faces to move".into());
            }
            authoring::move_vertices(level, room, &vertices, delta)
        });
    }

    /// Raise (negative `dy`) or lower the selected floor or ceiling faces,
    /// adding step faces around them.
    pub fn mesh_extrude(&mut self, dy: i32) {
        let quads: Vec<_> =
            self.mesh.faces.iter().filter_map(|f| if let Face::Quad(i) = f { Some(*i) } else { None }).collect();
        let what = if dy < 0 { "Raised" } else { "Lowered" };
        if let Some(region) = self.mesh_edit(what, |level, room| authoring::extrude(level, room, &quads, dy)) {
            self.mesh.vertices.clear();
            self.mesh.faces = region.into_iter().map(Face::Quad).collect();
            self.mesh.active = self.mesh.faces.last().copied().map(Element::Face);
        }
    }

    /// Push each selected wall face out by one sector, opening the space behind it.
    pub fn mesh_push(&mut self) {
        let quads: Vec<_> =
            self.mesh.faces.iter().filter_map(|f| if let Face::Quad(i) = f { Some(*i) } else { None }).collect();
        let pushed = self.mesh_edit("Pushed wall out", |level, room| {
            if quads.is_empty() {
                return Err("Select a wall face to push out".into());
            }
            quads.iter().try_for_each(|&quad| authoring::push_wall(level, room, quad).map(drop))
        });
        if pushed.is_some() {
            self.mesh.vertices.clear();
        }
    }

    /// Paint `faces` (or the selection) with the brush texture.
    pub fn mesh_paint(&mut self, faces: Option<Vec<Face>>) {
        let Some(texture) = self.mesh.brush else {
            self.status = "Choose a texture in the Textures window first".into();
            return;
        };
        let faces = faces.unwrap_or_else(|| self.mesh.faces.iter().copied().collect());
        if faces.is_empty() {
            self.status = "Select faces to paint".into();
            return;
        }
        self.mesh_edit(&format!("Painted texture {texture}"), |level, room| {
            authoring::paint(level, room, &faces, texture)
        });
    }

    /// Drop selected elements that no longer exist (after undo, for example).
    pub fn mesh_retain(&mut self, room: &Room) {
        let (v, q, t) = (room.vertices.len(), room.quads.len(), room.triangles.len());
        self.mesh.vertices.retain(|&i| i < v);
        self.mesh.faces.retain(|f| match *f {
            Face::Quad(i) => i < q,
            Face::Triangle(i) => i < t,
        });
    }

    /// The selection as automation names it: `q12,v4`.
    pub fn mesh_selection_text(&self) -> String {
        let faces = self.mesh.faces.iter().map(|&f| Element::Face(f));
        let vertices = self.mesh.vertices.iter().map(|&v| Element::Vertex(v));
        faces.chain(vertices).map(Element::name).collect::<Vec<_>>().join(",")
    }
}

/// The selection, detached from the editor for use inside an edit.
struct Selection {
    vertices: BTreeSet<usize>,
    faces: Vec<Face>,
}
impl Selection {
    fn vertices(&self, room: &Room) -> BTreeSet<usize> {
        let mut set = authoring::face_vertices(room, &self.faces);
        set.extend(self.vertices.iter().filter(|&&v| v < room.vertices.len()));
        set
    }
}
impl MeshEdit {
    fn clone_selection(&self) -> Selection {
        Selection { vertices: self.vertices.clone(), faces: self.faces.iter().copied().collect() }
    }
}

/// A room vertex's level-file position.
pub fn vertex_position(room: &Room, v: usize) -> [f64; 3] {
    let p = room.vertices[v];
    [(room.origin[0] + p[0]) as f64, p[1] as f64, (room.origin[2] + p[2]) as f64]
}

/// Corner vertices of a face.
pub fn corners(room: &Room, face: Face) -> &[usize] {
    match face {
        Face::Quad(i) => room.quads.get(i).map_or(&[][..], |q| &q[..4]),
        Face::Triangle(i) => room.triangles.get(i).map_or(&[][..], |t| &t[..3]),
    }
}

/// The world-space points the gizmo centres on in mesh mode.
pub fn selection_points(editor: &Editor, world_geometry: Option<&WorldGeometry>) -> Vec<Vec3> {
    let Some(room) = world_geometry.and_then(|w| w.data.rooms.get(editor.mesh.room)) else { return vec![] };
    editor.mesh_vertices(room).into_iter().map(|v| world(vertex_position(room, v))).collect()
}

/// Where a ray first meets a face of `room` that is seen from the ray's side,
/// as a distance along it in level units.
fn ray_face(room: &Room, origin: [f64; 3], direction: [f64; 3], face: Face) -> Option<f64> {
    let points: Vec<_> = corners(room, face).iter().map(|&v| vertex_position(room, v)).collect();
    if points.len() < 3 {
        return None;
    }
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross =
        |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // Faces are wound clockwise from the side they are drawn for: their
    // right-handed normal points away from a viewer.
    let normal = cross(sub(points[1], points[0]), sub(points[2], points[0]));
    if dot(normal, direction) <= 0.0 {
        return None;
    }
    (1..points.len() - 1).find_map(|k| {
        let (a, b, c) = (points[0], points[k], points[k + 1]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let p = cross(direction, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-9 {
            return None;
        }
        let s = sub(origin, a);
        let u = dot(s, p) / det;
        let q = cross(s, e1);
        let v = dot(direction, q) / det;
        let t = dot(e2, q) / det;
        (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
    })
}

/// The nearest face a ray meets in `room`.
fn first_face(room: &Room, origin: [f64; 3], direction: [f64; 3]) -> Option<(f64, Face)> {
    let faces = (0..room.quads.len()).map(Face::Quad).chain((0..room.triangles.len()).map(Face::Triangle));
    faces.filter_map(|f| ray_face(room, origin, direction, f).map(|t| (t, f))).min_by(|a, b| a.0.total_cmp(&b.0))
}

/// How far along `ray` (world units) it first meets a room face, from the side
/// the face is drawn for.
pub fn room_distance(level: &Level, ray: Ray3d) -> Option<f32> {
    let (origin, direction) = (file_offset(ray.origin), file_offset(*ray.direction));
    level
        .rooms
        .iter()
        .filter_map(|room| first_face(room, origin, direction))
        .map(|(t, _)| t as f32)
        .min_by(f32::total_cmp)
}

/// What the cursor is over: a vertex of the edited room near it on screen,
/// or else the nearest face of any room along the ray.
fn hit(
    editor: &Editor,
    level: &Level,
    camera: &Camera,
    eye: &GlobalTransform,
    cursor: Vec2,
    any_room: bool,
) -> Option<(usize, Element)> {
    let ray = camera.viewport_to_world(eye, cursor).ok()?;
    let origin = file_offset(ray.origin);
    let direction = file_offset(*ray.direction);
    if let Some(room) = level.rooms.get(editor.mesh.room) {
        let near = (0..room.vertices.len())
            .filter_map(|v| {
                let p = camera.world_to_viewport(eye, world(vertex_position(room, v))).ok()?;
                Some((p.distance(cursor), v))
            })
            .filter(|(d, _)| *d < 8.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, v)) = near {
            return Some((editor.mesh.room, Element::Vertex(v)));
        }
    }
    let rooms: Vec<usize> = if any_room { (0..level.rooms.len()).collect() } else { vec![editor.mesh.room] };
    rooms
        .into_iter()
        .filter_map(|r| first_face(level.rooms.get(r)?, origin, direction).map(|(t, f)| (t, r, f)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, r, f)| (r, Element::Face(f)))
}

fn keys(
    mut editor: ResMut<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    focus: Res<ViewportFocus>,
) {
    if egui.wants_any_keyboard_input() || !focus.active || editor.leaving.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let on = !editor.mesh.on;
        editor.set_mesh_mode(on);
    }
    if !editor.mesh.on || keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        return;
    }
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        editor.mesh_extrude(-STEP);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        editor.mesh_extrude(STEP);
    }
    if keys.just_pressed(KeyCode::KeyP) {
        editor.mesh_push();
    }
    if keys.just_pressed(KeyCode::Escape) {
        editor.mesh_clear();
    }
}

/// Hover and click in mesh mode: select elements, or paint faces.
#[allow(clippy::too_many_arguments)]
fn pick(
    mut editor: ResMut<Editor>,
    world_geometry: Option<Res<WorldGeometry>>,
    focus: Res<ViewportFocus>,
    handles: Res<gizmo::Handles>,
    egui: Res<EguiWantsInput>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
) {
    if !editor.mesh.on {
        return;
    }
    let Some(level) = world_geometry.as_ref().map(|w| w.data.clone()) else { return };
    if let Some(room) = level.rooms.get(editor.mesh.room)
        && (editor.mesh.vertices.iter().any(|&v| v >= room.vertices.len())
            || editor.mesh.faces.iter().any(|&f| corners(room, f).is_empty()))
    {
        editor.mesh_retain(room);
    }
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let (Ok((camera, eye)), Some(cursor)) = (cameras.single(), cursor) else { return };
    let blocked = egui.wants_any_pointer_input() || handles.busy() || editor.leaving.is_some();
    let hover = if blocked { None } else { hit(&editor, &level, camera, eye, cursor, false) };
    if editor.mesh.hover != hover {
        editor.mesh.hover = hover;
    }
    if blocked || !focus.active || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let found = hit(&editor, &level, camera, eye, cursor, true);
    if editor.mesh.paint
        && let Some((room, Element::Face(face))) = found
    {
        editor.mesh_room(room);
        editor.mesh_paint(Some(vec![face]));
        return;
    }
    let extend =
        keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::ControlRight]);
    let room = found.map_or(editor.mesh.room, |(r, _)| r);
    editor.mesh_click(room, found.map(|(_, e)| e), extend);
}

fn outline(gizmos: &mut Gizmos<OnTop>, room: &Room, face: Face, offset: [f64; 3], color: Color) {
    let points: Vec<_> = corners(room, face)
        .iter()
        .map(|&v| {
            let p = vertex_position(room, v);
            world([p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]])
        })
        .collect();
    if !points.is_empty() {
        gizmos.linestrip(points.iter().copied().chain([points[0]]), color);
    }
}

fn mark(gizmos: &mut Gizmos<OnTop>, p: Vec3, size: f32, color: Color) {
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        gizmos.line(p - axis * size, p + axis * size, color);
    }
}

/// The edited room's vertices, the selection, what the cursor is over, and
/// where a move in progress will put the selection.
fn draw(
    editor: Res<Editor>,
    world_geometry: Option<Res<WorldGeometry>>,
    cameras: Query<&GlobalTransform, With<GameCamera>>,
    mut gizmos: Gizmos<OnTop>,
) {
    if !editor.mesh.on {
        return;
    }
    let (Some(w), Ok(eye)) = (world_geometry, cameras.single()) else { return };
    let Some(room) = w.data.rooms.get(editor.mesh.room) else { return };
    let scale = |p: Vec3| eye.translation().distance(p).max(0.5) * 0.006;
    // The room's sector grid, at its lowest floor.
    let (x, z) = ((room.x_sectors as i32 * SECTOR) as f64, (room.z_sectors as i32 * SECTOR) as f64);
    let (ox, oz, y) = (room.origin[0] as f64, room.origin[2] as f64, room.bottom as f64);
    let grid = Color::srgba(0.4, 0.6, 1.0, 0.35);
    for i in 0..=room.x_sectors {
        let at = ox + (i as i32 * SECTOR) as f64;
        gizmos.line(world([at, y, oz]), world([at, y, oz + z]), grid);
    }
    for i in 0..=room.z_sectors {
        let at = oz + (i as i32 * SECTOR) as f64;
        gizmos.line(world([ox, y, at]), world([ox + x, y, at]), grid);
    }
    let selected = editor.mesh_vertices(room);
    for v in 0..room.vertices.len() {
        let p = world(vertex_position(room, v));
        if !selected.contains(&v) {
            mark(&mut gizmos, p, scale(p), Color::srgba(0.8, 0.8, 0.9, 0.5));
        }
    }
    let yellow = Color::srgb(1.0, 0.85, 0.1);
    for &face in &editor.mesh.faces {
        outline(&mut gizmos, room, face, [0.0; 3], yellow);
    }
    for &v in &selected {
        let p = world(vertex_position(room, v));
        mark(&mut gizmos, p, scale(p) * 2.0, yellow);
    }
    if let Some((r, element)) = editor.mesh.hover
        && r == editor.mesh.room
    {
        let color = if editor.mesh.paint { Color::srgb(0.2, 0.9, 1.0) } else { Color::WHITE };
        match element {
            Element::Face(face) => outline(&mut gizmos, room, face, [0.0; 3], color),
            Element::Vertex(v) => {
                let p = world(vertex_position(room, v));
                mark(&mut gizmos, p, scale(p) * 2.5, color);
            }
        }
    }
    if let Some(offset) = editor.mesh.dragging {
        let offset = offset.map(f64::from);
        let orange = Color::srgb(1.0, 0.5, 0.1);
        for &face in &editor.mesh.faces {
            outline(&mut gizmos, room, face, offset, orange);
        }
        for &v in &selected {
            let p = vertex_position(room, v);
            let p = world([p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]]);
            mark(&mut gizmos, p, scale(p) * 2.0, orange);
        }
    }
}

/// What a face is to its room: a sector's floor or ceiling, or something else.
fn describe(level: &Level, room: usize, face: Face) -> String {
    let r = &level.rooms[room];
    let Face::Quad(i) = face else { return "Triangle".into() };
    match sector_face(r, &r.quads[i]) {
        Some(f) => {
            let s = &r.sectors[f.x * r.z_sectors + f.z];
            let info = level.floor_info(s);
            let (height, slope) = match f.surface {
                Surface::Floor => (s.floor, info.floor_slope),
                Surface::Ceiling => (s.ceiling, info.ceiling_slope),
            };
            format!("{:?} of sector ({}, {}): height {height}, slope {slope:?}", f.surface, f.x, f.z)
        }
        None => "Wall or detail face".into(),
    }
}

/// The Mesh panel, shown in place of object properties in mesh mode.
pub fn panel(ui: &mut egui::Ui, editor: &mut Editor, world_geometry: Option<&WorldGeometry>) {
    let rooms = editor.doc.value()["rooms"].as_array().map_or(0, Vec::len);
    ui.horizontal(|ui| {
        ui.heading("Mesh");
        let mut room = editor.mesh.room;
        ui.label("Room");
        if ui.add(egui::DragValue::new(&mut room).range(0..=rooms.saturating_sub(1)).speed(0.05)).changed() {
            editor.mesh_room(room);
        }
    });
    ui.label(
        egui::RichText::new(
            "Click vertices or faces; Shift-click adds. Arrows move on the grid. Tab returns to objects.",
        )
        .weak(),
    );
    ui.separator();
    ui.label(format!("{} faces, {} vertices selected", editor.mesh.faces.len(), editor.mesh.vertices.len()));
    if let (Some(level), Some(Element::Face(face))) = (world_geometry.map(|w| &w.data), editor.mesh.active)
        && let Some(room) = level.rooms.get(editor.mesh.room)
        && !corners(room, face).is_empty()
    {
        ui.label(describe(level, editor.mesh.room, face));
        let texture = match face {
            Face::Quad(i) => room.quads[i][4],
            Face::Triangle(i) => room.triangles[i][3],
        };
        ui.label(format!("Texture {texture}"));
    }
    ui.separator();
    let faces = !editor.mesh.faces.is_empty();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(faces, egui::Button::new("Raise").shortcut_text("+"))
            .on_hover_text("Raise floor or ceiling faces 256 units")
            .clicked()
        {
            editor.mesh_extrude(-STEP);
        }
        if ui
            .add_enabled(faces, egui::Button::new("Lower").shortcut_text("-"))
            .on_hover_text("Lower floor or ceiling faces 256 units")
            .clicked()
        {
            editor.mesh_extrude(STEP);
        }
        if ui
            .add_enabled(faces, egui::Button::new("Push wall out").shortcut_text("P"))
            .on_hover_text("Open the sector behind a wall face")
            .clicked()
        {
            editor.mesh_push();
        }
        if ui
            .add_enabled(faces || !editor.mesh.vertices.is_empty(), egui::Button::new("Clear").shortcut_text("Esc"))
            .clicked()
        {
            editor.mesh_clear();
        }
    });
}

/// The Textures window: the texture table drawn from the atlas. Click one to
/// choose it, then paint the selection or click faces with Paint on click.
pub fn textures(ui: &mut egui::Ui, editor: &mut Editor, level: Option<&Level>, atlas: Option<egui::TextureId>) {
    let Some(level) = level else {
        ui.label("Loading...");
        return;
    };
    ui.horizontal(|ui| {
        ui.checkbox(&mut editor.mesh.paint, "Paint on click").on_hover_text("In mesh mode, clicking a face paints it");
        let can = editor.mesh.brush.is_some() && !editor.mesh.faces.is_empty();
        if ui.add_enabled(can, egui::Button::new("Paint selection")).clicked() {
            editor.mesh_paint(None);
        }
    });
    ui.label(match editor.mesh.brush {
        Some(t) => format!("Brush: texture {t} of {}", level.textures.len()),
        None => format!("{} textures; click one to choose it", level.textures.len()),
    });
    ui.separator();
    let size = 56.0;
    let columns = ((ui.available_width() / (size + 8.0)).floor() as usize).max(1);
    let rows = level.textures.len().div_ceil(columns);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, size + 8.0, rows, |ui, range| {
        for row in range {
            ui.horizontal(|ui| {
                for index in row * columns..((row + 1) * columns).min(level.textures.len()) {
                    let uv = level.textures[index].uv;
                    let (min, max) = uv.iter().fold(([1.0f32; 2], [0.0f32; 2]), |(lo, hi), p| {
                        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
                    });
                    let chosen = editor.mesh.brush == Some(index);
                    let response = match atlas {
                        Some(id) => ui.add(
                            egui::Button::image(
                                egui::Image::new((id, egui::vec2(size, size)))
                                    .uv(egui::Rect::from_min_max(min.into(), max.into())),
                            )
                            .selected(chosen),
                        ),
                        None => ui.add_sized([size, size], egui::Button::selectable(chosen, index.to_string())),
                    };
                    if response.on_hover_text(format!("Texture {index}")).clicked() {
                        editor.mesh.brush = Some(index);
                    }
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sandbox floor face over sector (x, z).
    fn floor(editor: &Editor, x: usize, z: usize) -> Face {
        let level = editor.doc.level().unwrap();
        let room = &level.rooms[0];
        let i = (0..room.quads.len())
            .find(|&i| {
                sector_face(room, &room.quads[i]).is_some_and(|f| (f.x, f.z, f.surface) == (x, z, Surface::Floor))
            })
            .unwrap();
        Face::Quad(i)
    }

    fn sector(editor: &Editor, x: usize, z: usize) -> serde_json::Value {
        editor.doc.value()["rooms"][0]["sectors"][x * 6 + z].clone()
    }

    #[test]
    fn tab_enters_the_active_objects_room_and_clicks_select_and_toggle_elements() {
        let mut editor = crate::sandbox();
        editor.selection = vec![1];
        editor.set_mesh_mode(true);
        assert!(editor.mesh.on);
        assert_eq!(editor.mesh.room, 0);
        let face = Element::Face(floor(&editor, 2, 2));
        editor.mesh_click(0, Some(face), false);
        editor.mesh_click(0, Some(Element::Vertex(3)), true);
        assert_eq!(editor.mesh_selection_text(), format!("{},v3", face.name()));
        assert_eq!(editor.mesh.active, Some(Element::Vertex(3)));
        editor.mesh_click(0, Some(Element::Vertex(3)), true);
        assert!(editor.mesh.vertices.is_empty(), "shift-click toggles");
        editor.mesh_click(0, None, false);
        assert!(editor.mesh.faces.is_empty());
        assert_eq!(Element::parse("q12"), Some(Element::Face(Face::Quad(12))));
        assert_eq!(Element::parse("x1"), None);
        editor.set_mesh_mode(false);
        assert!(!editor.mesh.on);
    }

    #[test]
    fn raising_a_floor_is_one_undo_step_that_moves_the_sector_too() {
        let mut editor = crate::sandbox();
        let before = editor.doc.value().clone();
        editor.set_mesh_mode(true);
        editor.mesh_click(0, Some(Element::Face(floor(&editor, 2, 2))), false);
        editor.mesh_extrude(-STEP);
        assert_eq!(sector(&editor, 2, 2)["floor"], -256, "{}", editor.status);
        assert!(editor.preview, "geometry edits rebuild the preview");
        // The raised face stays selected, so raising again stacks.
        editor.mesh_extrude(-STEP);
        assert_eq!(sector(&editor, 2, 2)["floor"], -512);
        let level = editor.doc.level().unwrap();
        assert!(authoring::sector_mismatches(&level, 0).is_empty());
        editor.undo();
        editor.undo();
        assert_eq!(editor.doc.value(), &before);
        // Errors leave the document alone and say why.
        editor.mesh_clear();
        editor.mesh_extrude(-STEP);
        assert!(editor.status.contains("Select") || editor.status.contains("floor"), "{}", editor.status);
        assert!(!editor.doc.is_dirty());
    }

    #[test]
    fn walls_never_stand_in_open_floor_and_lowering_a_block_removes_its_sides() {
        let mut editor = crate::sandbox();
        editor.set_mesh_mode(true);
        let level = editor.doc.level().unwrap();
        let room = &level.rooms[0];
        let quads = room.quads.len();
        // The sandbox's raised block, 256 units high.
        let top =
            (0..quads).find(|&i| sector_face(room, &room.quads[i]).is_some_and(|f| f.corners == [-256; 4])).unwrap();
        let f = sector_face(room, &room.quads[top]).unwrap();
        editor.mesh_click(0, Some(Element::Face(Face::Quad(top))), false);
        // Moving only the top would leave its sides standing in the open.
        editor.mesh_move([0, 256, 0]);
        assert!(editor.status.contains("in the open"), "{}", editor.status);
        editor.mesh_move([0, 100, 0]);
        assert!(editor.status.contains("256"), "unsnapped moves are refused: {}", editor.status);
        assert!(!editor.doc.is_dirty());
        // Lowering it instead takes its sides away with it.
        editor.mesh_extrude(STEP);
        assert_eq!(sector(&editor, f.x, f.z)["floor"], 0, "{}", editor.status);
        let level = editor.doc.level().unwrap();
        assert_eq!(level.rooms[0].quads.len(), quads - 4);
        assert!(authoring::sector_mismatches(&level, 0).is_empty());
        assert!(authoring::wall_mismatches(&level, 0).is_empty());
        // Painting the selection is one more step.
        editor.mesh.brush = Some(1);
        editor.mesh_paint(None);
        let Some(Face::Quad(face)) = editor.mesh.faces.first().copied() else { panic!() };
        assert_eq!(editor.doc.value()["rooms"][0]["quads"][face][4], 1);
        // The sandbox's walls each span the whole room, so they cannot be pushed per sector.
        let wall =
            (0..level.rooms[0].quads.len()).find(|&i| sector_face(&level.rooms[0], &level.rooms[0].quads[i]).is_none());
        editor.mesh_click(0, wall.map(|i| Element::Face(Face::Quad(i))), false);
        editor.mesh_push();
        assert!(editor.status.contains("one sector"), "{}", editor.status);
        editor.undo();
        editor.undo();
        assert!(!editor.doc.is_dirty());
    }
}
