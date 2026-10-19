//! The Rooms window and the top-down Map, for finding your way around a
//! level's rooms; and the current room's outline in the 3D view.
//!
//! The current room is the one mesh mode edits (`MeshEdit::room`), so
//! choosing a room here, on the map, in the toolbar or by clicking its faces
//! in mesh mode all agree. See docs/editor-design.md for how rooms join.
use crate::{
    Editor,
    gizmo::OnTop,
    viewport::{file_offset, world},
};
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2 as EVec2};
use engine::{
    editor::{Link, neighbours},
    render::GameCamera,
    world::{Level, Room, WorldGeometry, format::SOLID},
};
use serde_json::Value;

const SECTOR: f32 = 1024.0;

/// The map's view and height slice.
pub struct MapView {
    /// Level x and z at the centre of the canvas.
    pub center: [f32; 2],
    /// Screen points per level unit.
    pub scale: f32,
    /// Fit every shown room on the next draw.
    pub fit: bool,
    /// Show only rooms overlapping this height range (level y, down).
    pub slice: Option<[i32; 2]>,
}
impl Default for MapView {
    fn default() -> Self {
        Self { center: [0.0; 2], scale: 0.01, fit: true, slice: None }
    }
}

pub struct RoomsPlugin;
impl Plugin for RoomsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, outline.after(bevy::transform::TransformSystems::Propagate));
    }
}

impl Editor {
    /// Make `room` the current room, as mesh mode edits it.
    pub fn select_room(&mut self, room: usize) {
        self.mesh_room(room);
        self.status = format!("Room {room}");
    }

    /// Close every doorway and hole between rooms `a` and `b`.
    pub fn disconnect(&mut self, a: usize, b: usize) {
        if self.level_edit("Disconnected", |level| engine::editor::disconnect(level, a, b)).is_some() {
            self.status = format!("Disconnected rooms {a} and {b} - Ctrl+Z to undo");
        }
    }

    /// Frame the current room in the 3D view.
    pub fn frame_room(&mut self) {
        self.frame = true;
        self.frame_room = true;
    }
}

fn link_arrow(link: Link) -> &'static str {
    match link {
        Link::Side => "->",
        Link::Below => "v",
        Link::Above => "^",
    }
}

fn in_slice(room: &Room, slice: Option<[i32; 2]>) -> bool {
    slice.is_none_or(|[high, low]| room.top <= low && room.bottom >= high)
}

/// The Rooms window: every room, and the current room's flags, neighbours
/// and objects. Sets `show_data` to open the Data window on the room.
pub fn window(ui: &mut egui::Ui, editor: &mut Editor, level: Option<&Level>, show_data: &mut bool) {
    let Some(level) = level else {
        ui.label("Loading...");
        return;
    };
    let current = editor.mesh.room.min(level.rooms.len() - 1);
    let counts = level.placements.iter().fold(vec![0; level.rooms.len()], |mut c, p| {
        if let Some(n) = c.get_mut(p.room) {
            *n += 1;
        }
        c
    });
    ui.label(egui::RichText::new("Click to choose; double-click to frame.").weak());
    egui::ScrollArea::vertical().id_salt("room list").max_height(220.0).auto_shrink([false, true]).show_rows(
        ui,
        18.0,
        level.rooms.len(),
        |ui, range| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            for index in range {
                let r = &level.rooms[index];
                let mut flags = String::new();
                if r.water {
                    flags.push_str(" water");
                }
                if r.wind {
                    flags.push_str(" wind");
                }
                if r.alternate.is_some() {
                    flags.push_str(" flip");
                }
                let size = format!("{}x{}", r.x_sectors, r.z_sectors);
                let text =
                    format!("{index:>3} {size:>5}  y {:>6}..{:<6} {:>2} obj{flags}", r.top, r.bottom, counts[index]);
                let response = ui.selectable_label(index == current, egui::RichText::new(text).monospace());
                if response.clicked() {
                    editor.select_room(index);
                }
                if response.double_clicked() {
                    editor.frame_room();
                }
            }
        },
    );
    ui.separator();
    details(ui, editor, level, current, show_data);
}

fn details(ui: &mut egui::Ui, editor: &mut Editor, level: &Level, index: usize, show_data: &mut bool) {
    let r = &level.rooms[index];
    ui.horizontal(|ui| {
        ui.heading(format!("Room {index}"));
        if ui.button("Frame").clicked() {
            editor.frame_room();
        }
        if ui.add_enabled(!editor.mesh.on, egui::Button::new("Edit mesh")).clicked() {
            editor.set_mesh_mode(true);
            editor.select_room(index);
        }
        if ui.button("Show in Data").clicked() {
            editor.navigate(format!("/rooms/{index}"));
            *show_data = true;
        }
    });
    let base = format!("/rooms/{index}");
    egui::Grid::new("room details").num_columns(2).striped(true).show(ui, |ui| {
        ui.label("Grid");
        ui.label(format!("{} x {} sectors at x {}, z {}", r.x_sectors, r.z_sectors, r.origin[0], r.origin[2]));
        ui.end_row();
        ui.label("Height");
        ui.label(format!("{} (top) to {} (bottom)", r.top, r.bottom));
        ui.end_row();
        ui.label("Flags");
        ui.horizontal(|ui| {
            for (field, label, hint) in [
                ("water", "Water", "The room is under water: swimming, the underwater palette and caustics"),
                ("wind", "Wind", "Hair and cloth blow in this room's wind"),
            ] {
                let pointer = format!("{base}/{field}");
                let mut value = editor.value(&pointer).and_then(Value::as_bool).unwrap_or(false);
                if ui.checkbox(&mut value, label).on_hover_text(hint).changed() {
                    editor.edit(vec![(pointer, Value::Bool(value))], false);
                }
            }
        });
        ui.end_row();
        ui.label("Ambient");
        let pointer = format!("{base}/ambient/shade");
        let mut shade = editor.value(&pointer).and_then(Value::as_i64).unwrap_or(0);
        let response = ui
            .add(egui::DragValue::new(&mut shade).range(0..=8191).speed(16.0).update_while_editing(false))
            .on_hover_text("Light every lit object starts from: 0 bright, 8191 dark");
        if response.changed() {
            editor.edit(vec![(pointer, Value::from(shade))], response.dragged());
        }
        if response.drag_stopped() {
            editor.commit();
        }
        ui.end_row();
        ui.label("Lights");
        ui.label(r.lights.len().to_string());
        ui.end_row();
        if let Some(alternate) = r.alternate {
            ui.label("Flips to");
            if ui.link(format!("Room {alternate}")).clicked() {
                editor.select_room(alternate);
            }
            ui.end_row();
        }
    });
    ui.label("Opens into");
    ui.horizontal_wrapped(|ui| {
        let links = neighbours(level, index);
        if links.is_empty() {
            ui.label(egui::RichText::new("no portals").weak());
        }
        for (room, link) in links {
            let hint = match link {
                Link::Side => "through a doorway",
                Link::Below => "through a hole in the floor",
                Link::Above => "through an opening in the ceiling",
            };
            if ui.button(format!("{} {room}", link_arrow(link))).on_hover_text(hint).clicked() {
                editor.select_room(room);
            }
            if ui
                .small_button("x")
                .on_hover_text(format!("Disconnect room {room}: close every doorway and hole between the two rooms"))
                .clicked()
            {
                editor.disconnect(index, room);
            }
        }
    });
    ui.label("Objects");
    ui.horizontal_wrapped(|ui| {
        let objects: Vec<_> = (0..level.placements.len()).filter(|&i| level.placements[i].room == index).collect();
        if objects.is_empty() {
            ui.label(egui::RichText::new("none").weak());
        }
        for i in objects {
            if ui.selectable_label(editor.selection.contains(&i), i.to_string()).clicked() {
                editor.click(Some(i), false);
            }
        }
    });
}

/// The map's level (x, z) to screen and back. Screen right is +x and screen
/// up is +z, as seen looking down in the 3D view.
struct Transform2 {
    rect: Rect,
    center: [f32; 2],
    scale: f32,
}
impl Transform2 {
    fn screen(&self, x: f32, z: f32) -> Pos2 {
        self.rect.center() + EVec2::new(x - self.center[0], -(z - self.center[1])) * self.scale
    }
    fn level(&self, p: Pos2) -> [f32; 2] {
        let d = (p - self.rect.center()) / self.scale;
        [self.center[0] + d.x, self.center[1] - d.y]
    }
}

fn open(room: &Room, sx: usize, sz: usize) -> bool {
    room.sectors.get(sx * room.z_sectors + sz).is_some_and(|s| s.floor > SOLID)
}

/// Rooms with an open sector under level (x, z), highest first.
fn rooms_at(level: &Level, slice: Option<[i32; 2]>, x: f32, z: f32) -> Vec<usize> {
    let mut found: Vec<_> = (0..level.rooms.len())
        .filter(|&i| {
            let r = &level.rooms[i];
            let (sx, sz) = (((x - r.origin[0] as f32) / SECTOR).floor(), ((z - r.origin[2] as f32) / SECTOR).floor());
            in_slice(r, slice)
                && sx >= 0.0
                && sz >= 0.0
                && (sx as usize) < r.x_sectors
                && (sz as usize) < r.z_sectors
                && open(r, sx as usize, sz as usize)
        })
        .collect();
    found.sort_by_key(|&i| level.rooms[i].top);
    found
}

/// The next room to choose at a point: the highest, or the one after the
/// current room when it is already there, so clicking again steps down a stack.
fn next_room(found: &[usize], current: usize) -> Option<usize> {
    match found.iter().position(|&r| r == current) {
        Some(i) => Some(found[(i + 1) % found.len()]),
        None => found.first().copied(),
    }
}

/// The Map window: rooms from above, their open sectors, doorways, holes,
/// objects and the camera.
pub fn map(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    view: &mut MapView,
    level: Option<&Level>,
    camera: Option<&GlobalTransform>,
) {
    let Some(level) = level else {
        ui.label("Loading...");
        return;
    };
    let current = editor.mesh.room.min(level.rooms.len() - 1);
    ui.horizontal_wrapped(|ui| {
        if ui.button("Fit").on_hover_text("Show every room in the slice").clicked() {
            view.fit = true;
        }
        let mut slicing = view.slice.is_some();
        if ui.checkbox(&mut slicing, "Slice").on_hover_text("Show only rooms within a height range").changed() {
            let r = &level.rooms[current];
            view.slice = slicing.then_some([r.top, r.bottom]);
        }
        if let Some([high, low]) = view.slice.as_mut() {
            ui.label("from");
            ui.add(egui::DragValue::new(high).speed(64.0).range(-65536..=*low))
                .on_hover_text("Highest y (up is negative)");
            ui.label("to");
            ui.add(egui::DragValue::new(low).speed(64.0).range(*high..=65536)).on_hover_text("Lowest y");
            if ui.button("Current room").on_hover_text("Slice to the current room's height").clicked() {
                let r = &level.rooms[current];
                *high = r.top;
                *low = r.bottom;
            }
        }
    });
    let (response, painter) =
        ui.allocate_painter(ui.available_size().max(EVec2::splat(120.0)), Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 26));
    let shown: Vec<usize> = (0..level.rooms.len()).filter(|&i| in_slice(&level.rooms[i], view.slice)).collect();
    if view.fit && !shown.is_empty() {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for &i in &shown {
            let r = &level.rooms[i];
            let (x, z) = (r.origin[0] as f32, r.origin[2] as f32);
            lo = [lo[0].min(x), lo[1].min(z)];
            hi = [hi[0].max(x + r.x_sectors as f32 * SECTOR), hi[1].max(z + r.z_sectors as f32 * SECTOR)];
        }
        view.center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        view.scale =
            (rect.width() / (hi[0] - lo[0]).max(SECTOR)).min(rect.height() / (hi[1] - lo[1]).max(SECTOR)) * 0.92;
        view.fit = false;
    }
    // Pan by dragging; zoom about the cursor.
    if response.dragged() {
        let d = response.drag_delta() / view.scale;
        view.center = [view.center[0] - d.x, view.center[1] + d.y];
    }
    if let Some(cursor) = response.hover_pos() {
        let zoom = ui.input(|i| i.zoom_delta() * (i.smooth_scroll_delta.y / 400.0).exp());
        if zoom != 1.0 {
            let before = Transform2 { rect, center: view.center, scale: view.scale }.level(cursor);
            view.scale = (view.scale * zoom).clamp(0.0005, 1.0);
            let after = Transform2 { rect, center: view.center, scale: view.scale }.level(cursor);
            view.center = [view.center[0] + before[0] - after[0], view.center[1] + before[1] - after[1]];
        }
    }
    let t = Transform2 { rect, center: view.center, scale: view.scale };
    let painter = painter.with_clip_rect(rect);
    let near: Vec<usize> = neighbours(level, current).into_iter().map(|(r, _)| r).collect();
    // Larger rooms first, so smaller ones overlapping them stay visible.
    let mut order = shown.clone();
    order.sort_by_key(|&i| std::cmp::Reverse(level.rooms[i].x_sectors * level.rooms[i].z_sectors));
    for &i in &order {
        let r = &level.rooms[i];
        let fill = if r.water {
            Color32::from_rgba_unmultiplied(60, 120, 200, 70)
        } else {
            Color32::from_rgba_unmultiplied(150, 150, 160, 45)
        };
        for sx in 0..r.x_sectors {
            for sz in 0..r.z_sectors {
                if open(r, sx, sz) {
                    let (x, z) = (r.origin[0] as f32 + sx as f32 * SECTOR, r.origin[2] as f32 + sz as f32 * SECTOR);
                    painter.rect_filled(
                        Rect::from_two_pos(t.screen(x, z), t.screen(x + SECTOR, z + SECTOR)),
                        0.0,
                        fill,
                    );
                }
            }
        }
        let corner = |dx: usize, dz: usize| {
            t.screen(r.origin[0] as f32 + (dx as f32) * SECTOR, r.origin[2] as f32 + (dz as f32) * SECTOR)
        };
        let outline = Rect::from_two_pos(corner(0, 0), corner(r.x_sectors, r.z_sectors));
        let stroke = if i == current {
            Stroke::new(2.5, Color32::from_rgb(255, 215, 40))
        } else if near.contains(&i) {
            Stroke::new(1.5, Color32::from_rgb(80, 210, 230))
        } else {
            Stroke::new(1.0, Color32::from_gray(90))
        };
        painter.rect_stroke(outline, 0.0, stroke, egui::StrokeKind::Inside);
        if outline.width() > 28.0 && outline.height() > 14.0 {
            painter.text(
                outline.center(),
                egui::Align2::CENTER_CENTER,
                i.to_string(),
                egui::FontId::proportional(11.0),
                Color32::from_gray(200),
            );
        }
    }
    // The current room's portals: doorways as green lines, holes as orange boxes.
    let r = &level.rooms[current];
    for p in &r.portals {
        let xs = p.vertices.iter().map(|v| (r.origin[0] + v[0]) as f32);
        let zs = p.vertices.iter().map(|v| (r.origin[2] + v[2]) as f32);
        let (x0, x1) = xs.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        let (z0, z1) = zs.fold((f32::MAX, f32::MIN), |(a, b), z| (a.min(z), b.max(z)));
        if p.normal[1] == 0 {
            painter
                .line_segment([t.screen(x0, z0), t.screen(x1, z1)], Stroke::new(3.0, Color32::from_rgb(90, 230, 110)));
        } else {
            let color =
                if p.normal[1] < 0 { Color32::from_rgb(240, 150, 50) } else { Color32::from_rgb(200, 120, 240) };
            painter.rect_stroke(
                Rect::from_two_pos(t.screen(x0, z0), t.screen(x1, z1)),
                0.0,
                Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
        }
    }
    for (i, p) in level.placements.iter().enumerate() {
        if !level.rooms.get(p.room).is_some_and(|r| in_slice(r, view.slice)) {
            continue;
        }
        let selected = editor.selection.contains(&i);
        let color = if selected { Color32::from_rgb(255, 215, 40) } else { Color32::from_rgb(230, 90, 90) };
        painter.circle_filled(
            t.screen(p.position[0] as f32, p.position[2] as f32),
            if selected { 3.5 } else { 2.0 },
            color,
        );
    }
    if let Some(camera) = camera {
        let eye = file_offset(camera.translation());
        let forward = file_offset(*camera.forward());
        let at = t.screen(eye[0] as f32, eye[2] as f32);
        let dir = EVec2::new(forward[0] as f32, -forward[2] as f32).normalized();
        if dir.is_finite() {
            let side = EVec2::new(-dir.y, dir.x);
            let points = vec![at + dir * 10.0, at - dir * 5.0 + side * 6.0, at - dir * 5.0 - side * 6.0];
            painter.add(egui::Shape::convex_polygon(points, Color32::WHITE, Stroke::NONE));
        }
    }
    if let Some(cursor) = response.hover_pos() {
        let [x, z] = t.level(cursor);
        let found = rooms_at(level, view.slice, x, z);
        if !found.is_empty() {
            let list = found.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ");
            response.clone().on_hover_text_at_pointer(format!("Room {list}  (x {x:.0}, z {z:.0})"));
        }
        // The first click of a double-click has already chosen the room.
        if response.double_clicked() {
            editor.frame_room();
        } else if response.clicked()
            && let Some(room) = next_room(&found, current)
        {
            editor.select_room(room);
        }
    }
    painter.text(
        rect.left_bottom() + EVec2::new(6.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        "Drag: pan   Scroll: zoom   Click: choose (again: next in a stack)   Double-click: frame",
        egui::FontId::proportional(11.0),
        Color32::from_gray(140),
    );
}

/// The current room's box and portals in the 3D view, while the Rooms or Map
/// window is open.
fn outline(
    editor: Res<Editor>,
    world_geometry: Option<Res<WorldGeometry>>,
    cameras: Query<&GlobalTransform, With<GameCamera>>,
    mut gizmos: Gizmos<OnTop>,
) {
    if !(editor.windows.rooms || editor.windows.map) || cameras.single().is_err() {
        return;
    }
    let Some(w) = world_geometry else { return };
    let Some(r) = w.data.rooms.get(editor.mesh.room) else { return };
    let (x0, z0) = (r.origin[0] as f64, r.origin[2] as f64);
    let (x1, z1) = (x0 + r.x_sectors as f64 * 1024.0, z0 + r.z_sectors as f64 * 1024.0);
    let (top, bottom) = (r.top as f64, r.bottom as f64);
    let color = Color::srgba(1.0, 0.85, 0.15, 0.8);
    for y in [top, bottom] {
        let ring = [[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1], [x0, y, z0]].map(world);
        gizmos.linestrip(ring, color);
    }
    for (x, z) in [(x0, z0), (x1, z0), (x1, z1), (x0, z1)] {
        gizmos.line(world([x, top, z]), world([x, bottom, z]), color);
    }
    for p in &r.portals {
        let color = match p.normal[1] {
            0 => Color::srgb(0.35, 0.9, 0.45),
            y if y < 0 => Color::srgb(0.95, 0.6, 0.2),
            _ => Color::srgb(0.8, 0.5, 0.95),
        };
        let points: Vec<_> =
            p.vertices.iter().map(|v| world([x0 + v[0] as f64, v[1] as f64, z0 + v[2] as f64])).collect();
        if let Some(&first) = points.first() {
            gizmos.linestrip(points.iter().copied().chain([first]), color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_a_stack_steps_down_it_and_the_map_round_trips() {
        assert_eq!(next_room(&[4, 9, 2], 0), Some(4));
        assert_eq!(next_room(&[4, 9, 2], 4), Some(9));
        assert_eq!(next_room(&[4, 9, 2], 2), Some(4));
        assert_eq!(next_room(&[], 2), None);
        let t = Transform2 {
            rect: Rect::from_min_size(Pos2::ZERO, EVec2::splat(200.0)),
            center: [5000.0, 3000.0],
            scale: 0.01,
        };
        let p = t.screen(6024.0, 4024.0);
        assert!(p.x > 100.0 && p.y < 100.0, "+x is right and +z is up");
        let back = t.level(p);
        assert!((back[0] - 6024.0).abs() < 0.01 && (back[1] - 4024.0).abs() < 0.01);
    }

    #[test]
    fn choosing_a_room_and_editing_its_flags_is_undoable() {
        let mut editor = crate::sandbox();
        let level = editor.doc.level().unwrap();
        editor.select_room(0);
        assert_eq!(editor.mesh.room, 0);
        assert_eq!(rooms_at(&level, None, 2560.0, 2560.0), [0]);
        assert!(rooms_at(&level, None, 512.0, 512.0).is_empty(), "solid border");
        assert!(rooms_at(&level, Some([1024, 4096]), 2560.0, 2560.0).is_empty(), "sliced away");
        editor.edit(vec![("/rooms/0/water".into(), Value::Bool(true))], false);
        assert_eq!(editor.doc.value()["rooms"][0]["water"], true);
        assert!(editor.preview);
        editor.undo();
        assert!(!editor.doc.is_dirty());
        editor.frame_room();
        assert!(editor.frame && editor.frame_room);
    }
}
