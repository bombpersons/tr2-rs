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
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2 as EVec2};
use engine::{
    editor::{Link, STEP, flip_bases, live_rooms, neighbours},
    objects::WorldObject,
    render::{GameCamera, InRoom, LevelEntity},
    world::{Level, Room, WorldGeometry, format::SOLID},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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
    /// The New room tool: dragging draws a room's open floor.
    pub drawing: bool,
    /// Where a drag began and where it is now (level x, z).
    drag: Option<([f32; 2], [f32; 2])>,
    /// A drawn room waiting for its heights.
    pub draft: Option<Draft>,
}
impl Default for MapView {
    fn default() -> Self {
        Self { center: [0.0; 2], scale: 0.01, fit: true, slice: None, drawing: false, drag: None, draft: None }
    }
}

/// The Rooms window's split and merge choices; the planned cut is drawn in
/// the 3D view while the window is open.
pub struct RoomTools {
    /// 0 splits across x, 2 across z, 1 at a height.
    pub axis: usize,
    /// The sector line to split along, in the room's grid.
    pub line: usize,
    pub height: i32,
    pub merge_with: Option<usize>,
}
impl Default for RoomTools {
    fn default() -> Self {
        Self { axis: 0, line: 2, height: -1024, merge_with: None }
    }
}

/// A room drawn on the map: its open floor in world sector numbers, and heights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draft {
    pub x: [i32; 2],
    pub z: [i32; 2],
    pub floor: i32,
    pub ceiling: i32,
}

impl Draft {
    /// The sectors between two level points, inclusive.
    fn between(a: [f32; 2], b: [f32; 2], (floor, ceiling): (i32, i32)) -> Self {
        let cells = |i: usize| {
            let (lo, hi) = (a[i].min(b[i]), a[i].max(b[i]));
            [(lo / SECTOR).floor() as i32, (hi / SECTOR).floor() as i32 + 1]
        };
        Draft { x: cells(0), z: cells(1), floor, ceiling }
    }
}

/// The floor and ceiling most of a room's open sectors have, for a new room
/// beside it.
fn usual_heights(level: &Level, room: usize) -> (i32, i32) {
    let mut counts = BTreeMap::<(i32, i32), usize>::new();
    for s in level.rooms[room].sectors.iter().filter(|s| s.floor > SOLID) {
        *counts.entry((s.floor, s.ceiling)).or_default() += 1;
    }
    counts.into_iter().max_by_key(|&(_, n)| n).map_or((0, -2048), |(h, _)| h)
}

/// What a drawn room would overlap, and which rooms it could be connected to.
fn draft_notes(level: &Level, d: &Draft) -> Vec<String> {
    let (mut overlaps, mut beside) = (BTreeSet::new(), BTreeSet::new());
    let meets = |s: &engine::world::Sector| s.floor > SOLID && s.ceiling < d.floor && s.floor > d.ceiling;
    for (i, r) in level.rooms.iter().enumerate() {
        for sx in 1..r.x_sectors.saturating_sub(1) {
            for sz in 1..r.z_sectors.saturating_sub(1) {
                if !meets(&r.sectors[sx * r.z_sectors + sz]) {
                    continue;
                }
                let (x, z) = (r.origin[0].div_euclid(1024) + sx as i32, r.origin[2].div_euclid(1024) + sz as i32);
                let inside = (d.x[0]..d.x[1]).contains(&x) && (d.z[0]..d.z[1]).contains(&z);
                let ring = (d.x[0] - 1..=d.x[1]).contains(&x) && (d.z[0] - 1..=d.z[1]).contains(&z);
                let corner = (x == d.x[0] - 1 || x == d.x[1]) && (z == d.z[0] - 1 || z == d.z[1]);
                if inside {
                    overlaps.insert(i);
                } else if ring && !corner {
                    beside.insert(i);
                }
            }
        }
    }
    let list = |set: &BTreeSet<usize>| set.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ");
    let mut notes = vec![];
    if !beside.is_empty() {
        notes.push(format!(
            "Shares an edge with room {}: select walls there in mesh mode and press Connect",
            list(&beside)
        ));
    }
    if !overlaps.is_empty() {
        notes.push(format!(
            "Overlaps room {}'s space: allowed (rooms can overlap unseen), but they cannot be connected",
            list(&overlaps)
        ));
    }
    notes
}

pub struct RoomsPlugin;
impl Plugin for RoomsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, outline.after(bevy::transform::TransformSystems::Propagate))
            .add_systems(Update, flip_visibility.after(crate::animation::preview));
    }
}

impl Editor {
    /// Make `room` the current room, as mesh mode edits it. Choosing a room
    /// or its alternate shows the level in the state that room is in.
    pub fn select_room(&mut self, room: usize) {
        if room != self.mesh.room {
            self.room_tools.merge_with = None;
        }
        self.mesh_room(room);
        self.status = format!("Room {room}");
        if let Some((_, alternate)) = self.flip_pair(room)
            && alternate != self.flipped
        {
            self.flipped = alternate;
            self.status = format!(
                "Room {room} - showing the level {}",
                if alternate { "flipped (it is an alternate)" } else { "as it starts" }
            );
        }
    }

    /// The room a flip swaps with `room`, and whether `room` is the alternate.
    fn flip_pair(&self, room: usize) -> Option<(usize, bool)> {
        let rooms = self.doc.value()["rooms"].as_array()?;
        let alternate = |r: &Value| r.get("alternate").and_then(Value::as_u64).map(|a| a as usize);
        if let Some(a) = rooms.get(room).and_then(alternate) {
            return Some((a, false));
        }
        rooms.iter().position(|r| alternate(r) == Some(room)).map(|b| (b, true))
    }

    /// Show the level flipped or as it starts. The current room follows to
    /// the version shown.
    pub fn set_flipped(&mut self, flipped: bool) {
        self.flipped = flipped;
        if let Some((partner, alternate)) = self.flip_pair(self.mesh.room)
            && alternate != flipped
        {
            self.room_tools.merge_with = None;
            self.mesh_room(partner);
        }
        self.status = if flipped {
            "Showing the level flipped: rooms with an alternate are replaced by it".into()
        } else {
            "Showing the level as it starts".into()
        };
    }

    /// Split `room` as the Rooms window's tools say.
    pub fn split_room(&mut self, room: usize) {
        let RoomTools { axis, line, height, .. } = self.room_tools;
        let part = self.level_edit("Split", |level| match axis {
            1 => engine::editor::split_height(level, room, height),
            _ => engine::editor::split_along(level, room, axis, line),
        });
        if let Some(part) = part {
            self.status = format!("Split room {room} into rooms {room} and {part} - Ctrl+Z to undo");
        }
    }

    /// Merge room `b` into room `a` and make the result current.
    pub fn merge_room(&mut self, a: usize, b: usize) {
        if let Some(room) = self.level_edit("Merged", |level| engine::editor::merge_rooms(level, a, b)) {
            self.mesh.room = usize::MAX;
            self.select_room(room);
            self.room_tools.merge_with = None;
            self.status = format!("Merged room {b} into room {room} - Ctrl+Z to undo");
        }
    }

    /// Close every doorway and hole between rooms `a` and `b`.
    pub fn disconnect(&mut self, a: usize, b: usize) {
        if self.level_edit("Disconnected", |level| engine::editor::disconnect(level, a, b)).is_some() {
            self.status = format!("Disconnected rooms {a} and {b} - Ctrl+Z to undo");
        }
    }

    /// Build a drawn room in the style of the current room, and make it current.
    pub fn create_room(&mut self, draft: Draft) {
        let template = self.mesh.room;
        let created = self.level_edit("Created a room", |level| {
            let template = template.min(level.rooms.len() - 1);
            let spec = engine::editor::NewRoom {
                x: draft.x,
                z: draft.z,
                floor: draft.floor,
                ceiling: draft.ceiling,
                textures: engine::editor::typical_textures(level, template),
                shade: engine::editor::typical_shade(level, template),
                ambient: level.rooms[template].ambient,
            };
            engine::editor::new_room(level, &spec)
        });
        if let Some(room) = created {
            self.select_room(room);
            self.frame_room();
            self.status = format!(
                "Created room {room} - connect it by selecting its walls beside another room in mesh mode (Ctrl+Z to undo)"
            );
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
    let (bases, live) = (flip_bases(level), live_rooms(level, editor.flipped));
    ui.label(egui::RichText::new("Click to choose; double-click to frame.").weak());
    let row = ui.spacing().interact_size.y.max(ui.text_style_height(&egui::TextStyle::Monospace))
        + ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical().id_salt("room list").max_height(200.0).auto_shrink([false, true]).show_rows(
        ui,
        row,
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
                if let Some(a) = r.alternate {
                    flags.push_str(&format!(" flips to {a}"));
                }
                if let Some(b) = bases[index] {
                    flags.push_str(&format!(" alternate of {b}"));
                }
                let size = format!("{}x{}", r.x_sectors, r.z_sectors);
                let text =
                    format!("{index:>3} {size:>5}  y {:>6}..{:<6} {:>2} obj{flags}", r.top, r.bottom, counts[index]);
                let mut text = egui::RichText::new(text).monospace();
                if !live[index] {
                    // Not in the level as shown: the other flip state's.
                    text = text.weak();
                }
                let response = ui.selectable_label(index == current, text);
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
            if ui
                .link(format!("Room {alternate}"))
                .on_hover_text("Its alternate, shown when the level is flipped")
                .clicked()
            {
                editor.select_room(alternate);
            }
            ui.end_row();
        }
        if let Some(base) = flip_bases(level)[index] {
            ui.label("Alternate of");
            if ui
                .link(format!("Room {base}"))
                .on_hover_text("The room this one replaces when the level flips")
                .clicked()
            {
                editor.select_room(base);
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
    split_and_merge(ui, editor, level, index);
}

fn split_and_merge(ui: &mut egui::Ui, editor: &mut Editor, level: &Level, index: usize) {
    let r = &level.rooms[index];
    egui::CollapsingHeader::new("Split and merge").default_open(true).show(ui, |ui| {
        let tools = &mut editor.room_tools;
        ui.horizontal(|ui| {
            ui.label("Split");
            ui.selectable_value(&mut tools.axis, 0, "across x");
            ui.selectable_value(&mut tools.axis, 2, "across z");
            ui.selectable_value(&mut tools.axis, 1, "at a height");
        });
        let mut can = true;
        ui.horizontal(|ui| {
            if tools.axis == 1 {
                ui.label("height");
                let (low, high) = (r.top.div_euclid(STEP) * STEP + STEP, r.bottom.div_euclid(STEP) * STEP - STEP);
                can = low <= high;
                tools.height = tools.height.clamp(low.min(high), high.max(low)).div_euclid(STEP) * STEP;
                ui.add(egui::DragValue::new(&mut tools.height).speed(32.0).range(low..=high))
                    .on_hover_text("y of the cut (up is negative), in whole 256-unit steps; drawn in magenta");
            } else {
                ui.label("sector line");
                let size = if tools.axis == 0 { r.x_sectors } else { r.z_sectors };
                can = size >= 4;
                tools.line = tools.line.clamp(2, size.saturating_sub(2).max(2));
                ui.add(egui::DragValue::new(&mut tools.line).range(2..=size.saturating_sub(2).max(2)))
                    .on_hover_text("Each part keeps at least one sector of open floor; drawn in magenta");
            }
        });
        let hint = if tools.axis == 1 {
            "Stack two rooms joined through holes wherever the space reaches across the height"
        } else {
            "Two rooms overlapping by two sectors, joined by doorways wherever their space meets"
        };
        if ui.add_enabled(can, egui::Button::new("Split")).on_hover_text(hint).clicked() {
            editor.split_room(index);
        }
        let joined: Vec<usize> =
            neighbours(level, index).into_iter().map(|(room, _)| room).collect::<BTreeSet<_>>().into_iter().collect();
        ui.horizontal(|ui| {
            ui.label("Merge with");
            let mut with = editor
                .room_tools
                .merge_with
                .filter(|w| *w != index && *w < level.rooms.len())
                .or(joined.first().copied());
            egui::ComboBox::from_id_salt("merge with")
                .selected_text(with.map_or("choose".into(), |w| format!("room {w}")))
                .show_ui(ui, |ui| {
                    for room in (0..level.rooms.len()).filter(|&r| r != index) {
                        let label = if joined.contains(&room) {
                            format!("room {room} (joined)")
                        } else {
                            format!("room {room}")
                        };
                        ui.selectable_value(&mut with, Some(room), label);
                    }
                });
            editor.room_tools.merge_with = with;
            let help =
                "Rooms joined only by doorways along one line (or side by side), or only by holes over the same grid";
            if ui.add_enabled(with.is_some(), egui::Button::new("Merge")).on_hover_text(help).clicked()
                && let Some(with) = with
            {
                editor.merge_room(index, with);
            }
        });
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
        if level.rooms.iter().any(|r| r.alternate.is_some()) {
            let hint = "Show the level as it is once flipped: each room with an alternate is replaced by it";
            if ui.selectable_label(editor.flipped, "Flipped").on_hover_text(hint).clicked() {
                editor.set_flipped(!editor.flipped);
            }
        }
        let hint =
            "Drag a rectangle of open floor. Draw it beside another room's floor, sharing an edge, to connect them";
        if ui.selectable_label(view.drawing, "New room").on_hover_text(hint).clicked() {
            view.drawing = !view.drawing;
            view.drag = None;
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
    if let Some(mut d) = view.draft {
        let mut create = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("New room: {} x {} sectors", d.x[1] - d.x[0], d.z[1] - d.z[0])).strong());
            ui.label("floor");
            ui.add(egui::DragValue::new(&mut d.floor).speed(32.0).range(d.ceiling + 256..=65536))
                .on_hover_text("Floor height (y down; whole 256-unit steps)");
            ui.label("ceiling");
            ui.add(egui::DragValue::new(&mut d.ceiling).speed(32.0).range(-65536..=d.floor - 256))
                .on_hover_text("Ceiling height (above the floor, so smaller)");
            (d.floor, d.ceiling) = (d.floor.div_euclid(256) * 256, d.ceiling.div_euclid(256) * 256);
            create = ui.button("Create").clicked();
            if ui.button("Cancel").clicked() {
                view.draft = None;
            }
        });
        for note in draft_notes(level, &d) {
            ui.label(RichText::new(note).weak());
        }
        if view.draft.is_some() {
            view.draft = Some(d);
        }
        if create {
            view.draft = None;
            view.drawing = false;
            editor.create_room(d);
        }
    }
    let (response, painter) =
        ui.allocate_painter(ui.available_size().max(EVec2::splat(120.0)), Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 26));
    let live = live_rooms(level, editor.flipped);
    let shown: Vec<usize> =
        (0..level.rooms.len()).filter(|&i| live[i] && in_slice(&level.rooms[i], view.slice)).collect();
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
    // Pan by dragging (the right or middle button while drawing); zoom about the cursor.
    let panning = if view.drawing {
        response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle)
    } else {
        response.dragged()
    };
    if panning {
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
    let bases = flip_bases(level);
    for (i, p) in level.placements.iter().enumerate() {
        // Objects stay when their room flips; those in an alternate appear with it.
        if !level.rooms.get(p.room).is_some_and(|r| in_slice(r, view.slice))
            || (!editor.flipped && bases[p.room].is_some())
        {
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
    if view.drawing {
        let at = response.interact_pointer_pos().map(|p| t.level(p));
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(at) = at
        {
            view.drag = Some((at, at));
        }
        if let (Some((start, _)), Some(at)) = (view.drag, at)
            && response.dragged_by(egui::PointerButton::Primary)
        {
            view.drag = Some((start, at));
        }
        let heights = view.draft.map_or_else(|| usual_heights(level, current), |d| (d.floor, d.ceiling));
        if response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some((start, end)) = view.drag.take()
        {
            view.draft = Some(Draft::between(start, end, heights));
        } else if response.clicked()
            && let Some(at) = at
        {
            view.draft = Some(Draft::between(at, at, heights));
        }
        let shown = view.drag.map(|(a, b)| Draft::between(a, b, heights)).or(view.draft);
        if let Some(d) = shown {
            let (x0, z0) = (d.x[0] as f32 * SECTOR, d.z[0] as f32 * SECTOR);
            let (x1, z1) = (d.x[1] as f32 * SECTOR, d.z[1] as f32 * SECTOR);
            let area = Rect::from_two_pos(t.screen(x0, z0), t.screen(x1, z1));
            painter.rect_filled(area, 0.0, Color32::from_rgba_unmultiplied(90, 230, 110, 60));
            painter.rect_stroke(area, 0.0, Stroke::new(2.0, Color32::from_rgb(90, 230, 110)), egui::StrokeKind::Inside);
            // The solid border ring the room gets around its floor.
            let ring = Rect::from_two_pos(t.screen(x0 - SECTOR, z0 - SECTOR), t.screen(x1 + SECTOR, z1 + SECTOR));
            painter.rect_stroke(ring, 0.0, Stroke::new(1.0, Color32::from_rgb(60, 140, 70)), egui::StrokeKind::Inside);
        }
    } else if let Some(cursor) = response.hover_pos() {
        let [x, z] = t.level(cursor);
        let found: Vec<_> = rooms_at(level, view.slice, x, z).into_iter().filter(|&r| live[r]).collect();
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
        if view.drawing {
            "New room: drag its floor  Right-drag: pan  Scroll: zoom"
        } else {
            "Drag: pan  Scroll: zoom  Click: choose (again: next in stack)  Double-click: frame"
        },
        egui::FontId::proportional(10.0),
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
    // The planned split, in magenta.
    if editor.windows.rooms {
        let cut = Color::srgb(0.95, 0.3, 0.85);
        let tools = &editor.room_tools;
        let corners = match tools.axis {
            1 => {
                let y = tools.height as f64;
                [[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]]
            }
            0 => {
                let x = x0 + tools.line as f64 * 1024.0;
                [[x, top, z0], [x, top, z1], [x, bottom, z1], [x, bottom, z0]]
            }
            _ => {
                let z = z0 + tools.line as f64 * 1024.0;
                [[x0, top, z], [x1, top, z], [x1, bottom, z], [x0, bottom, z]]
            }
        };
        let points = corners.map(world);
        gizmos.linestrip(points.iter().copied().chain([points[0]]), cut);
        gizmos.line(points[0], points[2], cut);
        gizmos.line(points[1], points[3], cut);
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

/// Hide the rooms the level does not have in the state shown, and objects
/// authored in an alternate until the level flips. Runs after the animation
/// preview, which shows everything it does not isolate.
#[allow(clippy::type_complexity)]
fn flip_visibility(
    editor: Res<Editor>,
    world: Option<Res<WorldGeometry>>,
    mut entities: Query<(&InRoom, &mut Visibility, Option<&WorldObject>), With<LevelEntity>>,
) {
    let Some(world) = world else { return };
    let level = &world.data;
    if level.rooms.iter().all(|r| r.alternate.is_none()) {
        return;
    }
    let (live, bases) = (live_rooms(level, editor.flipped), flip_bases(level));
    for (room, mut visibility, object) in &mut entities {
        let shown = match object {
            Some(o) => editor.flipped || bases.get(o.room).is_none_or(Option::is_none),
            None => live.get(room.0).copied().unwrap_or(true),
        };
        if !shown {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choosing_an_alternate_flips_the_view_and_hides_the_rooms_it_replaces() {
        let mut editor = crate::sandbox();
        let mut rooms = editor.doc.value()["rooms"].clone();
        let copy = rooms[0].clone();
        rooms.as_array_mut().unwrap().push(copy);
        rooms[0]["alternate"] = Value::from(1);
        editor.edit(vec![("/rooms".into(), rooms)], false);
        editor.select_room(1);
        assert!(editor.flipped, "room 1 is an alternate");
        editor.set_flipped(false);
        assert_eq!(editor.mesh.room, 0, "the current room follows to the version shown");
        editor.set_flipped(true);
        assert_eq!(editor.mesh.room, 1);
        editor.select_room(0);
        assert!(!editor.flipped);

        let level = editor.doc.level().unwrap();
        let mut app = App::new();
        app.insert_resource(WorldGeometry::new(level)).add_systems(Update, flip_visibility);
        let spawn =
            |app: &mut App, room| app.world_mut().spawn((LevelEntity, InRoom(room), Visibility::Inherited)).id();
        let (original, alternate) = (spawn(&mut app, 0), spawn(&mut app, 1));
        let object =
            app.world_mut().spawn((LevelEntity, InRoom(1), Visibility::Inherited, WorldObject::new(0, 1, 1, 0))).id();
        let shown = |app: &App| {
            [original, alternate, object].map(|e| app.world().get::<Visibility>(e) != Some(&Visibility::Hidden))
        };
        app.insert_resource(editor);
        app.update();
        assert_eq!(shown(&app), [true, false, false]);
        app.world_mut().resource_mut::<Editor>().set_flipped(true);
        for e in [original, alternate, object] {
            *app.world_mut().get_mut::<Visibility>(e).unwrap() = Visibility::Inherited;
        }
        app.update();
        assert_eq!(shown(&app), [false, true, true]);
    }

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

    #[test]
    fn a_drawn_rectangle_becomes_a_room_beside_the_one_it_was_drawn_next_to() {
        let mut editor = crate::sandbox();
        let level = editor.doc.level().unwrap();
        // Dragging from one point to another takes every sector they touch, either way round.
        let d = Draft::between([5600.0, 7000.0], [5200.0, 3100.0], (0, -2048));
        assert_eq!((d.x, d.z), ([5, 6], [3, 7]));
        assert_eq!(usual_heights(&level, 0), (0, -2048));
        // The sandbox's open floor is sectors 1..5 on both axes.
        let beside = Draft { x: [5, 7], z: [2, 4], floor: 0, ceiling: -2048 };
        assert!(draft_notes(&level, &beside)[0].contains("room 0"), "{:?}", draft_notes(&level, &beside));
        let over = Draft { x: [3, 7], ..beside };
        assert!(draft_notes(&level, &over).iter().any(|n| n.starts_with("Overlaps room 0")));
        let above = Draft { floor: -4096, ceiling: -6144, ..over };
        assert!(draft_notes(&level, &above).is_empty(), "above the sandbox room nothing meets it");
        editor.create_room(beside);
        assert_eq!(editor.doc.value()["rooms"].as_array().unwrap().len(), 2);
        assert_eq!(editor.mesh.room, 1, "the new room becomes current");
        assert!(editor.frame && editor.frame_room);
        assert!(editor.status.starts_with("Created room 1"), "{}", editor.status);
        editor.undo();
        assert!(!editor.doc.is_dirty());
        editor.create_room(Draft { floor: -4096, ceiling: -2048, ..beside });
        assert!(!editor.doc.is_dirty(), "a floor above its ceiling is refused: {}", editor.status);
    }

    #[test]
    fn splitting_and_merging_from_the_rooms_window_are_single_undo_steps() {
        let mut editor = crate::sandbox();
        let before = editor.doc.value().clone();
        let rooms = |e: &Editor| e.doc.value()["rooms"].as_array().unwrap().len();
        editor.room_tools = RoomTools { axis: 0, line: 3, ..Default::default() };
        editor.split_room(0);
        assert_eq!(rooms(&editor), 2, "{}", editor.status);
        assert!(editor.status.starts_with("Split room 0 into rooms 0 and 1"));
        editor.merge_room(0, 1);
        assert_eq!(rooms(&editor), 1, "{}", editor.status);
        assert_eq!(editor.mesh.room, 0);
        editor.room_tools = RoomTools { axis: 1, height: -1024, ..Default::default() };
        editor.split_room(0);
        assert_eq!(rooms(&editor), 2, "{}", editor.status);
        let level = editor.doc.level().unwrap();
        assert_eq!(level.rooms[1].sectors[2 * 6 + 2].below, Some(0), "the upper part opens down");
        editor.merge_room(1, 0);
        assert_eq!(rooms(&editor), 1, "{}", editor.status);
        for _ in 0..4 {
            editor.undo();
        }
        assert_eq!(editor.doc.value(), &before);
        editor.room_tools = RoomTools { axis: 0, line: 1, ..Default::default() };
        editor.split_room(0);
        assert_eq!(rooms(&editor), 1, "too near the edge is refused: {}", editor.status);
    }
}
