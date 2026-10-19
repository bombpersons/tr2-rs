//! The editor's windows, menus and shortcuts, drawn with egui over the 3D view.
use crate::{
    Editor, EditorTarget, Leave,
    animation::AnimationPreview,
    edit::{Change, TURN},
    files::{self, Ask},
};
use bevy::{camera::RenderTarget, prelude::*, window::WindowCloseRequested};
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle, PrimaryEguiContext,
    egui::{self, Key, KeyboardShortcut, Modifiers, RichText},
};
use engine::{editor::child_pointer, objects::ObjectTypes, world::WorldGeometry};
use serde_json::Value;
use std::collections::BTreeMap;

/// Which windows are open, and how the Objects window lists placements.
pub struct Windows {
    pub objects: bool,
    pub properties: bool,
    pub data: bool,
    pub animation: bool,
    pub textures: bool,
    pub rooms: bool,
    pub map: bool,
    pub diagnostics: bool,
    pub map_view: crate::rooms::MapView,
    pub group: Group,
    pub filter: String,
}
impl Default for Windows {
    fn default() -> Self {
        Self {
            objects: true,
            properties: true,
            data: false,
            animation: false,
            textures: false,
            rooms: false,
            map: false,
            diagnostics: false,
            map_view: default(),
            group: Group::Room,
            filter: String::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Room,
    Type,
}

/// Drafts larger than this are edited field by field instead of as one JSON text.
const LARGEST_DRAFT: usize = 256 * 1024;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_systems(Startup, setup)
            .add_systems(Update, quit)
            .add_systems(EguiPrimaryContextPass, ui);
        app.world_mut().resource_mut::<EguiGlobalSettings>().auto_create_primary_context = false;
    }
}

fn setup(mut commands: Commands, target: Res<EditorTarget>) {
    let camera = commands
        .spawn((
            Camera2d,
            PrimaryEguiContext,
            // The 3D camera alone draws the level and its selection lines.
            bevy::camera::visibility::RenderLayers::layer(1),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
            Name::new("Editor UI"),
        ))
        .id();
    if target.headless {
        commands.entity(camera).insert(RenderTarget::Image(target.image.clone().into()));
    }
}

fn quit(mut editor: ResMut<Editor>, mut close: MessageReader<WindowCloseRequested>, mut exit: MessageWriter<AppExit>) {
    if close.read().next().is_some() {
        editor.request_leave(Leave::Quit);
    }
    if editor.exit_requested {
        exit.write(AppExit::Success);
    }
}

fn ui(
    mut contexts: EguiContexts,
    mut editor: ResMut<Editor>,
    types: Option<Res<ObjectTypes>>,
    mut previews: Query<&mut AnimationPreview>,
    world: Option<Res<WorldGeometry>>,
    server: Res<AssetServer>,
    cameras: Query<&GlobalTransform, With<engine::render::GameCamera>>,
) -> Result {
    // The Textures window and the toolbar's brush swatch draw from the atlas.
    let atlas = (editor.windows.textures || editor.mesh.on)
        .then(|| contexts.add_image(EguiTextureHandle::Strong(server.load::<Image>(editor.atlas()))));
    let ctx = contexts.ctx_mut()?;
    let editor = &mut *editor;
    let empty = ObjectTypes::default();
    let types = types.as_deref().unwrap_or(&empty);
    shortcuts(ctx, editor);
    if let Some(world) = world.as_deref() {
        editor.diagnostics.refresh(&world.data);
    }
    let top = menu_bar(ctx, editor);
    let top = top + crate::toolbar::toolbar(ctx, editor, top, world.as_deref().map(|w| &*w.data), atlas);
    status_bar(ctx, editor);
    let area = ctx.viewport_rect();
    let mut windows = std::mem::take(&mut editor.windows);
    egui::Window::new("Objects")
        .open(&mut windows.objects)
        .default_pos([8.0, top + 8.0])
        .default_size([270.0, 460.0])
        .show(ctx, |ui| objects(ui, editor, types, &mut windows.group, &mut windows.filter));
    let mut show_data = false;
    egui::Window::new("Properties")
        .open(&mut windows.properties)
        .default_pos([area.right() - 318.0, top + 8.0])
        .default_width(310.0)
        .show(ctx, |ui| {
            if editor.mesh.on {
                crate::mesh::panel(ui, editor, world.as_deref());
            } else {
                show_data = properties(ui, editor, types);
            }
        });
    egui::Window::new("Rooms")
        .open(&mut windows.rooms)
        .default_pos([8.0, top + 300.0])
        .default_size([330.0, 460.0])
        .show(ctx, |ui| crate::rooms::window(ui, editor, world.as_deref().map(|w| &*w.data), &mut show_data));
    egui::Window::new("Map")
        .open(&mut windows.map)
        .default_pos([area.right() - 448.0, area.bottom() - 470.0])
        .default_size([440.0, 420.0])
        .show(ctx, |ui| {
            let level = world.as_deref().map(|w| &*w.data);
            crate::rooms::map(ui, editor, &mut windows.map_view, level, cameras.single().ok())
        });
    egui::Window::new("Diagnostics")
        .open(&mut windows.diagnostics)
        .default_pos([290.0, area.bottom() - 330.0])
        .default_size([560.0, 290.0])
        .show(ctx, |ui| crate::diagnostics::window(ui, editor, world.as_deref().map(|w| &*w.data)));
    windows.data |= show_data;
    egui::Window::new("Data")
        .open(&mut windows.data)
        .default_pos([290.0, top + 8.0])
        .default_size([480.0, 420.0])
        .show(ctx, |ui| data(ui, editor));
    if let Ok(mut preview) = previews.single_mut() {
        egui::Window::new("Animation")
            .open(&mut windows.animation)
            .default_pos([290.0, area.bottom() - 190.0])
            .default_width(560.0)
            .show(ctx, |ui| crate::animation::window(ui, editor, &mut preview, world.as_deref()));
    }
    egui::Window::new("Textures")
        .open(&mut windows.textures)
        .default_pos([area.right() - 318.0, top + 300.0])
        .default_size([310.0, 360.0])
        .show(ctx, |ui| crate::mesh::textures(ui, editor, world.as_deref().map(|w| &*w.data), atlas));
    editor.windows = windows;
    leave_question(ctx, editor);
    Ok(())
}

const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const REDO_SHIFT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const SELECT_ALL: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
const DISCARD_QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Q);

fn shortcuts(ctx: &egui::Context, editor: &mut Editor) {
    if editor.leaving.is_some() {
        return;
    }
    // Text fields keep their own undo and select-all while typing.
    let typing = ctx.egui_wants_keyboard_input();
    ctx.input_mut(|input| {
        // Shifted shortcuts first: plain ones also match with Shift held.
        if input.consume_shortcut(&DISCARD_QUIT) {
            editor.exit_requested = true;
        } else if input.consume_shortcut(&QUIT) {
            editor.request_leave(Leave::Quit);
        } else if input.consume_shortcut(&SAVE_AS) {
            editor.ask = Some(Ask::SaveAs);
        } else if input.consume_shortcut(&SAVE) {
            editor.save();
        } else if input.consume_shortcut(&OPEN) {
            editor.request_leave(Leave::Open(None));
        } else if typing {
        } else if input.consume_shortcut(&REDO_SHIFT) || input.consume_shortcut(&REDO) {
            editor.redo();
        } else if input.consume_shortcut(&UNDO) {
            editor.undo();
        } else if input.consume_shortcut(&SELECT_ALL) {
            select_all(editor);
        }
    });
}

pub fn select_all(editor: &mut Editor) {
    let count = editor.doc.value()["placements"].as_array().map_or(0, Vec::len);
    editor.selection = (0..count).collect();
}

fn shortcut(ui: &mut egui::Ui, text: &str, shortcut: &KeyboardShortcut, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(text).shortcut_text(ui.ctx().format_shortcut(shortcut))).clicked()
}

/// The File / Edit / View bar; returns its height.
fn menu_bar(ctx: &egui::Context, editor: &mut Editor) -> f32 {
    let width = ctx.viewport_rect().width();
    egui::Area::new("menu bar".into())
        .fixed_pos([0.0, 0.0])
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            egui::Frame::side_top_panel(ui.style())
                .show(ui, |ui| {
                    ui.set_width(width - 16.0);
                    egui::MenuBar::new().ui(ui, |ui| {
                        ui.menu_button("File", |ui| file_menu(ui, editor));
                        ui.menu_button("Edit", |ui| {
                            if shortcut(ui, "Undo", &UNDO, editor.doc.can_undo()) {
                                editor.undo();
                            }
                            if shortcut(ui, "Redo", &REDO, editor.doc.can_redo()) {
                                editor.redo();
                            }
                            ui.separator();
                            if shortcut(ui, "Select all objects", &SELECT_ALL, true) {
                                select_all(editor);
                            }
                            if ui.add_enabled(!editor.selection.is_empty(), egui::Button::new("Deselect all")).clicked()
                            {
                                editor.click(None, false);
                            }
                            ui.separator();
                            let mut mesh = editor.mesh.on;
                            if ui
                                .add(egui::Checkbox::new(&mut mesh, "Mesh mode"))
                                .on_hover_text("Tab in the 3D view")
                                .changed()
                            {
                                editor.set_mesh_mode(mesh);
                            }
                        });
                        ui.menu_button("View", |ui| {
                            let windows = &mut editor.windows;
                            ui.checkbox(&mut windows.objects, "Objects");
                            ui.checkbox(&mut windows.properties, "Properties");
                            ui.checkbox(&mut windows.data, "Data");
                            ui.checkbox(&mut windows.animation, "Animation");
                            ui.checkbox(&mut windows.textures, "Textures");
                            ui.checkbox(&mut windows.rooms, "Rooms");
                            ui.checkbox(&mut windows.map, "Map");
                            ui.checkbox(&mut windows.diagnostics, "Diagnostics");
                            ui.separator();
                            if ui.add(egui::Button::new("Frame selection").shortcut_text("F")).clicked() {
                                editor.frame = true;
                            }
                            if ui.button("Overview").clicked() {
                                editor.frame = true;
                                editor.overview = true;
                            }
                        });
                        let name = editor.doc.value()["name"].as_str().unwrap_or("Untitled level").to_owned();
                        let dirty = if editor.doc.is_dirty() { "  (unsaved)" } else { "" };
                        ui.add_space(16.0);
                        ui.label(RichText::new(format!("{name}{dirty}")).weak());
                    });
                })
                .response
                .rect
                .height()
        })
        .inner
}

fn file_menu(ui: &mut egui::Ui, editor: &mut Editor) {
    if shortcut(ui, "Open...", &OPEN, true) {
        editor.request_leave(Leave::Open(None));
    }
    ui.menu_button("Open project level", |ui| {
        let levels = files::libraries(editor.doc.path());
        if levels.is_empty() {
            ui.label("No assets/levels folders found");
        }
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (name, path) in levels {
                if ui.button(name).clicked() {
                    editor.request_leave(Leave::Open(Some(path)));
                }
            }
        });
    });
    ui.add_enabled_ui(!editor.recent.is_empty(), |ui| {
        ui.menu_button("Open recent", |ui| {
            for path in editor.recent.clone() {
                if ui.button(path.display().to_string()).clicked() {
                    editor.request_leave(Leave::Open(Some(path)));
                }
            }
        });
    });
    ui.separator();
    if shortcut(ui, "Save", &SAVE, true) {
        editor.save();
    }
    if shortcut(ui, "Save As...", &SAVE_AS, true) {
        editor.ask = Some(Ask::SaveAs);
    }
    ui.separator();
    if shortcut(ui, "Quit", &QUIT, true) {
        editor.request_leave(Leave::Quit);
    }
}

fn status_bar(ctx: &egui::Context, editor: &mut Editor) {
    let rect = ctx.viewport_rect();
    egui::Area::new("status bar".into())
        .fixed_pos([0.0, rect.bottom() - 26.0])
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            egui::Frame::side_top_panel(ui.style()).show(ui, |ui| {
                ui.set_width(rect.width() - 16.0);
                ui.horizontal(|ui| {
                    let mode =
                        if editor.mesh.on { format!("Mesh: room {}", editor.mesh.room) } else { "Objects".into() };
                    ui.label(RichText::new(mode).strong()).on_hover_text("Tab switches between object and mesh mode");
                    ui.separator();
                    let new = editor.diagnostics.new_count();
                    if new > 0 {
                        let text = RichText::new(format!("{new} new problem{}", if new == 1 { "" } else { "s" }))
                            .color(ui.visuals().warn_fg_color);
                        if ui.link(text).on_hover_text("Open Diagnostics").clicked() {
                            editor.windows.diagnostics = true;
                        }
                        ui.separator();
                    }
                    ui.label(&editor.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(editor.doc.path().display().to_string()).weak());
                        if !editor.selection.is_empty() {
                            ui.label(format!("{} selected  |", editor.selection.len()));
                        }
                    });
                });
            });
        });
}

fn leave_question(ctx: &egui::Context, editor: &mut Editor) {
    let Some(leave) = editor.leaving.clone() else { return };
    let file = editor.doc.path().file_name().unwrap_or_default().to_string_lossy().into_owned();
    egui::Modal::new("unsaved changes".into()).show(ctx, |ui| {
        ui.heading("Save your changes?");
        ui.label(format!("{file} has unsaved changes."));
        if editor.status.contains("changed on disk") {
            ui.colored_label(ui.visuals().warn_fg_color, &editor.status);
        }
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                editor.save();
                if !editor.doc.is_dirty() {
                    editor.leave(leave.clone());
                }
            }
            if ui.button("Discard changes").clicked() {
                editor.leave(leave.clone());
            }
            if ui.button("Cancel").clicked() {
                editor.leaving = None;
            }
        });
    });
}

fn type_name(types: &ObjectTypes, kind: u64) -> String {
    types.get(kind as u16).map_or_else(|| format!("Type {kind}"), |t| t.name.clone())
}

fn objects(ui: &mut egui::Ui, editor: &mut Editor, types: &ObjectTypes, group: &mut Group, filter: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Group by");
        ui.selectable_value(group, Group::Room, "Room");
        ui.selectable_value(group, Group::Type, "Type");
    });
    ui.add(egui::TextEdit::singleline(filter).hint_text("Filter by name or number"));
    ui.separator();
    let query = filter.trim().to_lowercase();
    let mut groups = BTreeMap::<(u64, String), Vec<(usize, String)>>::new();
    for (index, object) in editor.doc.value()["placements"].as_array().into_iter().flatten().enumerate() {
        let kind = object["kind"].as_u64().unwrap_or(0);
        let room = object["room"].as_u64().unwrap_or(0);
        let name = type_name(types, kind);
        if !query.is_empty() && index.to_string() != query && !name.to_lowercase().contains(&query) {
            continue;
        }
        let (key, label) = match group {
            Group::Room => ((room, String::new()), format!("{index}  {name}")),
            Group::Type => ((0, name.clone()), format!("{index}  room {room}")),
        };
        groups.entry(key).or_default().push((index, label));
    }
    let mut clicked = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for ((room, name), items) in groups {
            let title = if *group == Group::Room { format!("Room {room}") } else { name };
            egui::CollapsingHeader::new(format!("{title}  ({})", items.len()))
                .id_salt((*group == Group::Room, room, &title))
                .default_open(true)
                .show(ui, |ui| {
                    for (index, label) in items {
                        let selected = editor.selection.contains(&index);
                        let text = if editor.active() == Some(index) { RichText::new(label).strong() } else { label.into() };
                        let response = ui.selectable_label(selected, text);
                        if response.clicked() {
                            let modifiers = ui.input(|i| i.modifiers);
                            clicked = Some((index, modifiers.shift || modifiers.command));
                        }
                        if response.double_clicked() {
                            editor.frame = true;
                        }
                    }
                });
        }
    });
    if let Some((index, extend)) = clicked {
        editor.click(Some(index), extend);
    }
}

/// A number field of every selected object: drag it to move them all by the
/// same amount, or type a value to set them all to it.
fn number(ui: &mut egui::Ui, editor: &mut Editor, field: &str, speed: f64, range: Option<(f64, f64)>) {
    let Some(index) = editor.active() else { return };
    let pointer = format!("/placements/{index}/{field}");
    let degrees = field == "rotation";
    let scale = if degrees { 360.0 / TURN as f64 } else { 1.0 };
    let saved = editor.doc.value().pointer(&pointer).and_then(Value::as_f64).unwrap_or(0.0);
    let mut shown = editor.value(&pointer).and_then(Value::as_f64).unwrap_or(0.0) * scale;
    let mut drag = egui::DragValue::new(&mut shown).speed(speed).update_while_editing(false);
    drag = if degrees { drag.suffix(" deg").fixed_decimals(1) } else { drag.fixed_decimals(0) };
    if let Some((low, high)) = range {
        drag = drag.range(low..=high);
    }
    let response = ui.add(drag);
    if response.changed() {
        let units = (shown / scale).round() as i64;
        if response.dragged() {
            editor.edit_selected(field, Change::Add(units - saved as i64), true);
        } else {
            let units = if degrees { units.rem_euclid(TURN) } else { units };
            editor.edit_selected(field, Change::Set(&Value::from(units)), false);
        }
    }
    if response.drag_stopped() {
        editor.commit();
    }
}

fn flag(ui: &mut egui::Ui, editor: &mut Editor, field: &str, label: &str) {
    let Some(index) = editor.active() else { return };
    let mut value = editor.value(&format!("/placements/{index}/{field}")).and_then(Value::as_bool).unwrap_or(false);
    if ui.checkbox(&mut value, label).changed() {
        editor.edit_selected(field, Change::Set(&Value::Bool(value)), false);
    }
}

const PLACEMENT_FIELDS: [&str; 8] = ["kind", "room", "position", "rotation", "code", "inverted", "invisible", "other_flags"];

/// The active object's fields; returns whether to show it in the Data window.
fn properties(ui: &mut egui::Ui, editor: &mut Editor, types: &ObjectTypes) -> bool {
    let Some(index) = editor.active() else {
        ui.label("Click an object in the 3D view or the Objects window. Shift-click selects more.");
        return false;
    };
    let base = format!("/placements/{index}");
    let Some(object) = editor.doc.value().pointer(&base).cloned() else { return false };
    let kind = object["kind"].as_u64().unwrap_or(0);
    ui.heading(format!("Object {index}: {}", type_name(types, kind)));
    if editor.selection.len() > 1 {
        ui.label(
            RichText::new(format!("{} selected. Edits apply to all; drags move them together.", editor.selection.len()))
                .weak(),
        );
    }
    ui.separator();
    egui::Grid::new("properties").num_columns(2).spacing([12.0, 6.0]).striped(true).show(ui, |ui| {
        ui.label("Type");
        ui.horizontal(|ui| {
            if types.iter().next().is_some() {
                egui::ComboBox::from_id_salt("kind").selected_text(type_name(types, kind)).height(400.0).show_ui(ui, |ui| {
                    for (id, info) in types.iter() {
                        if ui.selectable_label(u64::from(id) == kind, format!("{id}  {}", info.name)).clicked() {
                            editor.edit_selected("kind", Change::Set(&Value::from(id)), false);
                        }
                    }
                });
            }
            number(ui, editor, "kind", 0.05, Some((0.0, u16::MAX as f64)));
        });
        ui.end_row();
        ui.label("Room");
        number(ui, editor, "room", 0.05, Some((0.0, f64::MAX)));
        ui.end_row();
        ui.label("Position").on_hover_text("Level units: 1024 per sector, Y increases downward. Drag to move.");
        ui.horizontal(|ui| {
            for (axis, name) in ["X", "Y", "Z"].iter().enumerate() {
                ui.label(*name);
                number(ui, editor, &format!("position/{axis}"), 8.0, None);
            }
        });
        ui.end_row();
        ui.label("Heading");
        number(ui, editor, "rotation", 1.0, None);
        ui.end_row();
        ui.label("Activation code").on_hover_text("Code bits the object starts with; all five start it active.");
        number(ui, editor, "code", 0.05, Some((0.0, 31.0)));
        ui.end_row();
        ui.label("Starts");
        ui.horizontal(|ui| {
            flag(ui, editor, "inverted", "Inverted");
            flag(ui, editor, "invisible", "Hidden");
        });
        ui.end_row();
        ui.label("Other flags");
        number(ui, editor, "other_flags", 0.05, Some((0.0, u16::MAX as f64)));
        ui.end_row();
        for (key, value) in object.as_object().into_iter().flatten() {
            if !PLACEMENT_FIELDS.contains(&key.as_str()) {
                ui.label(humanize(key));
                ui.label(summary(value));
                ui.end_row();
            }
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Frame (F)").clicked() {
            editor.frame = true;
        }
        let show = ui.button("Show in Data").clicked();
        if show {
            editor.navigate(base);
        }
        show
    })
    .inner
}

pub fn humanize(key: &str) -> String {
    let text = key.replace('_', " ");
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().collect::<String>() + chars.as_str())
}

fn summary(value: &Value) -> String {
    match value {
        Value::Object(map) => match map.get("kind") {
            Some(kind) => format!("object type {kind}  ({} fields)", map.len()),
            None => format!("{{ {} fields }}", map.len()),
        },
        Value::Array(array) if array.len() <= 5 && array.iter().all(|v| !v.is_array() && !v.is_object()) => {
            value.to_string()
        }
        Value::Array(array) => format!("[ {} entries ]", array.len()),
        _ => value.to_string().chars().take(100).collect(),
    }
}

/// Every field of the level, browsed table by table.
fn data(ui: &mut egui::Ui, editor: &mut Editor) {
    let mut go = None;
    ui.horizontal_wrapped(|ui| {
        if ui.link("Level").clicked() {
            go = Some(String::new());
        }
        let mut pointer = String::new();
        for part in editor.pointer.split('/').skip(1) {
            pointer = format!("{pointer}/{part}");
            ui.label("/");
            let name = if part == "placements" { "Objects".into() } else { humanize(part) };
            if ui.link(name).clicked() {
                go = Some(pointer.clone());
            }
        }
    });
    ui.separator();
    let mut draft = None;
    let mut select = None;
    {
        let pointer = editor.pointer.as_str();
        let current = editor.doc.value().pointer(pointer);
        let count = match current {
            Some(Value::Object(map)) => map.len(),
            Some(Value::Array(array)) => array.len(),
            _ => 0,
        };
        let child = |i: usize| match current {
            Some(Value::Object(map)) => map.iter().nth(i).map(|(k, v)| (k.clone(), humanize(k), v)),
            Some(Value::Array(array)) => array.get(i).map(|v| (i.to_string(), format!("[{i}]"), v)),
            _ => None,
        };
        if count > 0 {
            let height = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
            egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show_rows(ui, height, count, |ui, rows| {
                egui::Grid::new("data rows").num_columns(2).striped(true).show(ui, |ui| {
                    for i in rows {
                        let Some((key, name, value)) = child(i) else { continue };
                        if ui.selectable_label(false, name).clicked() {
                            let pointer = child_pointer(pointer, &key);
                            if value.is_array() || value.is_object() {
                                go = Some(pointer);
                            } else {
                                draft = Some((pointer, value.as_str().map_or_else(|| value.to_string(), str::to_owned)));
                            }
                        }
                        ui.label(summary(value));
                        ui.end_row();
                    }
                });
            });
        } else if let Some(value) = current {
            ui.label(summary(value));
        }
        ui.horizontal(|ui| {
            let text = current.map(Value::to_string).unwrap_or_default();
            let fits = text.len() <= LARGEST_DRAFT;
            let edit = ui.add_enabled(fits, egui::Button::new("Edit as JSON"));
            let edit = edit.on_disabled_hover_text("This table is too large to edit as one text; open its entries.");
            if edit.clicked() {
                let text = if count > 0 { serde_json::to_string_pretty(current.unwrap()).unwrap_or(text) } else { text };
                draft = Some((pointer.to_string(), text));
            }
            if pointer.starts_with("/placements/")
                && let Some(index) = pointer.split('/').nth(2).and_then(|i| i.parse().ok())
                && ui.button("Select object").clicked()
            {
                select = Some(index);
            }
        });
    }
    if let Some(pointer) = go {
        editor.navigate(pointer);
    }
    if let Some(index) = select {
        editor.click(Some(index), false);
    }
    if draft.is_some() {
        editor.draft = draft;
    }
    let Some((pointer, mut text)) = editor.draft.take() else { return };
    ui.separator();
    ui.label(RichText::new(format!("Editing {}", if pointer.is_empty() { "/" } else { &pointer })).strong());
    let multiline = text.contains('\n');
    if multiline {
        egui::ScrollArea::vertical().id_salt("draft").max_height(240.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut text).code_editor().desired_width(f32::INFINITY));
        });
    } else {
        ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
    }
    let enter = !multiline && ui.input(|i| i.key_pressed(Key::Enter));
    let mut keep = true;
    ui.horizontal(|ui| {
        if ui.button("Apply").clicked() || enter {
            let result = apply_text(editor, &pointer, &text);
            keep = result.is_err();
            editor.result(result, "Applied");
        }
        if ui.button("Cancel").clicked() {
            keep = false;
        }
    });
    if keep {
        editor.draft = Some((pointer, text));
    }
}

/// Strings are typed without quotes and numbers must stay numbers; anything
/// else is JSON.
fn apply_text(editor: &mut Editor, pointer: &str, text: &str) -> Result<(), String> {
    match editor.doc.value().pointer(pointer) {
        Some(Value::String(_)) => editor.doc.replace(pointer, &Value::String(text.into()).to_string()),
        Some(Value::Number(_)) => match serde_json::from_str::<Value>(text) {
            Ok(value @ Value::Number(_)) => editor.doc.replace(pointer, &value.to_string()),
            _ => Err("Enter a number, such as 128 or -64.".into()),
        },
        _ => editor.doc.replace(pointer, text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values_keep_their_json_types_and_invalid_edits_are_rejected() {
        let mut editor = crate::sandbox();
        apply_text(&mut editor, "/name", "A level with \"quotes\"").unwrap();
        assert_eq!(editor.doc.value()["name"], "A level with \"quotes\"");
        assert!(apply_text(&mut editor, "/placements/1/position/0", "east").is_err());
        apply_text(&mut editor, "/placements/1/position/0", "4700").unwrap();
        assert_eq!(editor.doc.level().unwrap().placements[1].position[0], 4700);
        assert!(apply_text(&mut editor, "/placements/1/room", "9999").is_err());
        apply_text(&mut editor, "/placements/1/position", "[1, 2, 3]").unwrap();
        assert_eq!(editor.doc.level().unwrap().placements[1].position, [1, 2, 3]);
    }

    #[test]
    fn leaving_with_unsaved_work_asks_first_and_cancel_keeps_the_document() {
        let mut editor = crate::sandbox();
        editor.selection = vec![1];
        editor.edit_selected("position/0", Change::Add(128), false);
        let changed = editor.doc.value().clone();
        editor.request_leave(Leave::Quit);
        assert_eq!(editor.leaving, Some(Leave::Quit));
        assert!(!editor.exit_requested);
        editor.leaving = None;
        assert_eq!(editor.doc.value(), &changed);
        editor.request_leave(Leave::Open(None));
        editor.leave(Leave::Open(None));
        assert_eq!(editor.ask, Some(Ask::Open), "discarding asks for a level before replacing this one");
        assert_eq!(editor.doc.value(), &changed);
        editor.undo();
        editor.request_leave(Leave::Quit);
        assert!(editor.exit_requested, "a saved document leaves without asking");
    }

    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn save_as_replaces_a_confirmed_file_brings_the_atlas_and_reopens() {
        let folder = Temporary(std::env::temp_dir().join(format!(
            "editor-save-as-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        std::fs::create_dir(&folder.0).unwrap();
        let mut editor = crate::sandbox();
        let original = std::fs::read(editor.doc.path()).unwrap();
        let target = folder.0.join("copy.json");
        std::fs::write(&target, "not a level").unwrap();
        editor.save_as(folder.0.join("copy"));
        assert_eq!(editor.doc.path(), target);
        assert_eq!(std::fs::read(&target).unwrap(), original, "an untouched Save As keeps every byte");
        assert!(folder.0.join("atlas.png").exists());
        editor.selection = vec![1];
        editor.edit_selected("position/0", Change::Add(256), false);
        editor.save();
        assert!(!editor.doc.is_dirty());
        let other = folder.0.join("other.json");
        std::fs::write(&other, &original).unwrap();
        editor.open(&other);
        assert_eq!(editor.recent, std::slice::from_ref(&target));
        editor.open(&target);
        assert_eq!(
            editor.doc.level().unwrap().placements[1].position[0],
            serde_json::from_slice::<Value>(&original).unwrap()["placements"][1]["position"][0].as_i64().unwrap() as i32 + 256
        );
    }
}
