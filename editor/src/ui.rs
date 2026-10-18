use crate::{Editor, EditorTarget};
use bevy::{
    camera::RenderTarget,
    input::{
        ButtonState,
        keyboard::KeyboardInput,
        mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    },
    prelude::*,
    window::WindowCloseRequested,
};
use engine::{
    editor::{LevelDocument, child_pointer},
    objects::ObjectTypes,
    render::{GameCamera, view},
    world::position,
};
use serde_json::Value;

const ROWS: usize = 6;
pub(super) const PANEL: f32 = 500.0;
const INK: Color = Color::srgb(0.86, 0.90, 0.97);
const MUTED: Color = Color::srgb(0.50, 0.62, 0.74);
const BACK: Color = Color::srgb(0.055, 0.07, 0.10);

#[derive(Clone, PartialEq, Eq)]
pub struct Input {
    pub(super) mode: Mode,
    pub(super) text: String,
    pub(super) select_all: bool,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum Mode {
    Field(String),
    Value(String),
    Angle(String),
    File(crate::files::Picker),
    ConfirmLeave { quit: bool },
    Find(Vec<(String, String)>),
}
#[derive(Component)]
struct Header;
#[derive(Component)]
struct Footer;
#[derive(Component)]
struct Prompt;
#[derive(Component)]
struct Breadcrumb;
#[derive(Component)]
struct FieldRow(usize);
#[derive(Component, Clone)]
pub(super) enum Action {
    Up,
    Previous,
    Next,
    Edit,
    Save,
    SaveAs,
    Undo,
    Redo,
    Open,
    Frame,
    Overview,
    Find,
    Pick(String),
    Apply,
    Cancel,
    Row(usize),
    Browse(std::path::PathBuf),
    Files(bool),
    Location,
    Navigate(String),
    EditValue(String),
    EditAngle(String),
    Nudge(String, i64),
    Toggle(String),
    SaveContinue,
    DiscardContinue,
}

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (refresh.after(camera), hover, tabs.after(hover)));
    }
}

pub(super) fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: bevy::text::FontSize::Px(size), ..default() }, TextColor(color))
}
pub(super) fn button(parent: &mut ChildSpawnerCommands, text: &str, action: Action) {
    parent
        .spawn((
            action,
            bevy::picking::hover::Hovered::default(),
            ControlColor(Color::srgb(0.12, 0.17, 0.24)),
            Node { padding: UiRect::axes(px(9), px(6)), ..default() },
            BackgroundColor(Color::srgb(0.12, 0.17, 0.24)),
        ))
        .observe(clicked)
        .with_child(label(text, 14.0, INK));
}
fn setup(mut commands: Commands, target: Res<EditorTarget>) {
    let camera = commands
        .spawn((
            Camera2d,
            // UI uses UiTargetCamera; world selection lines belong to the 3D camera.
            bevy::camera::visibility::RenderLayers::layer(1),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
            Name::new("Editor UI"),
        ))
        .id();
    if target.headless {
        commands.entity(camera).insert(RenderTarget::Image(target.image.clone().into()));
    }
    commands
        .spawn((
            UiTargetCamera(camera),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                bottom: px(0),
                width: px(PANEL),
                padding: UiRect::all(px(14)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(BACK),
        ))
        .with_children(|root| {
            root.spawn(label("Level editor", 23.0, INK));
            root.spawn((
                Header,
                label("", 13.0, MUTED),
                Node { height: px(32), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
            ));
            root.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(5),
                row_gap: px(5),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|bar| {
                for (text, action) in [
                    ("Open", Action::Open),
                    ("Save", Action::Save),
                    ("Save copy", Action::SaveAs),
                    ("Undo", Action::Undo),
                    ("Redo", Action::Redo),
                    ("Frame", Action::Frame),
                    ("Overview", Action::Overview),
                ] {
                    button(bar, text, action);
                }
            });
            root.spawn(label("SCENE", 12.0, MUTED));
            root.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: px(5), row_gap: px(5), ..default() })
                .with_children(|bar| {
                    for (name, pointer) in [
                        ("Objects", "/placements"),
                        ("Rooms", "/rooms"),
                        ("Cameras", "/cameras"),
                        ("Animations", "/animations"),
                        ("All data", ""),
                    ] {
                        button(bar, name, Action::Navigate(pointer.into()));
                    }
                });
            root.spawn((
                Breadcrumb,
                label("/", 15.0, Color::srgb(0.4, 0.8, 1.0)),
                Node { height: px(28), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
            ));
            root.spawn(Node { column_gap: px(5), ..default() }).with_children(|bar| {
                for (text, action) in [
                    ("Find", Action::Find),
                    ("Parent", Action::Up),
                    ("Previous", Action::Previous),
                    ("Next", Action::Next),
                    ("Edit value", Action::Edit),
                ] {
                    button(bar, text, action);
                }
            });
            for row in 0..ROWS {
                root.spawn((
                    Action::Row(row),
                    FieldRow(row),
                    Node {
                        height: px(27),
                        width: percent(100),
                        flex_shrink: 0.0,
                        padding: UiRect::axes(px(7), px(3)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(BACK),
                ))
                .observe(clicked)
                .with_child(label("", 14.0, INK));
            }
            root.spawn((
                crate::inspector::Panel,
                Node { flex_direction: FlexDirection::Column, row_gap: px(4), flex_shrink: 0.0, ..default() },
            ));
            root.spawn((
                Prompt,
                label("", 14.0, INK),
                Node { max_height: px(46), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
            ));
            root.spawn((Footer, label("", 13.0, MUTED), Node { overflow: Overflow::clip(), ..default() }));
        });
    commands.spawn((UiTargetCamera(camera), Node { position_type: PositionType::Absolute, left: px(PANEL + 16.0), top: px(16), padding: UiRect::all(px(10)), ..default() }, BackgroundColor(BACK)))
        .with_child(label("Scene   |   Click an object to inspect it\nWheel: zoom   Middle drag: pan   Right drag: look\nWASD: fly   Q/E: down/up   F: focus", 14.0, INK));
}

#[derive(Component)]
pub(super) struct ControlColor(pub Color);

fn hover(
    mut buttons: Query<
        (&bevy::picking::hover::Hovered, &ControlColor, &mut BackgroundColor),
        Changed<bevy::picking::hover::Hovered>,
    >,
) {
    for (hovered, normal, mut background) in &mut buttons {
        background.0 = if hovered.get() { Color::srgb(0.20, 0.34, 0.46) } else { normal.0 };
    }
}

fn tabs(
    editor: Res<Editor>,
    mut controls: Query<(&Action, &bevy::picking::hover::Hovered, &mut ControlColor, &mut BackgroundColor)>,
) {
    for (action, hovered, mut normal, mut background) in &mut controls {
        let Action::Navigate(pointer) = action else { continue };
        let selected = if pointer.is_empty() {
            editor.pointer.is_empty()
        } else {
            editor.pointer == *pointer || editor.pointer.starts_with(&format!("{pointer}/"))
        };
        normal.0 = if selected { Color::srgb(0.12, 0.32, 0.46) } else { Color::srgb(0.12, 0.17, 0.24) };
        if !hovered.get() {
            background.0 = normal.0;
        }
    }
}

/// Child browsing is bounded even for very large vertex/animation tables.
fn entries(editor: &Editor) -> Vec<(String, String)> {
    let Some(value) = editor.doc.value().pointer(&editor.pointer) else { return vec![] };
    match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| {
                (child_pointer(&editor.pointer, key), format!("{}   {}", humanize(key), summary(value)))
            })
            .collect(),
        Value::Array(array) => array
            .iter()
            .enumerate()
            .map(|(i, value)| (child_pointer(&editor.pointer, &i.to_string()), format!("[{i}]   {}", summary(value))))
            .collect(),
        value => vec![(editor.pointer.clone(), summary(value))],
    }
}
pub(super) fn humanize(key: &str) -> String {
    let text = key.replace('_', " ");
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().collect::<String>() + chars.as_str())
}
fn summary(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            if let Some(kind) = map.get("kind") {
                format!("object type {kind}  ({} fields)", map.len())
            } else {
                format!("{{ {} fields }}", map.len())
            }
        }
        Value::Array(array) => {
            if array.len() <= 5 && array.iter().all(|v| !v.is_array() && !v.is_object()) {
                value.to_string()
            } else {
                format!("[ {} entries ]", array.len())
            }
        }
        _ => value.to_string().chars().take(100).collect(),
    }
}
pub(super) fn selected_pointer(editor: &Editor) -> String {
    entries(editor).get(editor.page + editor.selected).map_or_else(|| editor.pointer.clone(), |(p, _)| p.clone())
}
pub(super) fn begin_find(editor: &mut Editor, types: Option<&ObjectTypes>) {
    let table = editor
        .pointer
        .split('/')
        .nth(1)
        .filter(|table| matches!(*table, "placements" | "rooms" | "cameras" | "animations"))
        .unwrap_or("placements");
    let name = if table == "placements" { "Object" } else { table.trim_end_matches('s') };
    let candidates = editor
        .doc
        .value()
        .get(table)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let detail = item
                        .get("kind")
                        .and_then(Value::as_u64)
                        .map(|kind| {
                            types
                                .and_then(|types| types.get(kind as u16))
                                .map_or_else(|| format!("Type {kind}"), |kind| kind.name.clone())
                        })
                        .unwrap_or_default();
                    (format!("/{table}/{index}"), format!("{} {index}  {detail}", humanize(name)))
                })
                .collect()
        })
        .unwrap_or_default();
    editor.input = Some(Input { mode: Mode::Find(candidates), text: String::new(), select_all: true });
    editor.status.clear();
}
pub(super) fn find_matches<'a>(candidates: &'a [(String, String)], text: &str) -> Vec<&'a (String, String)> {
    let query = text.trim().to_lowercase();
    candidates
        .iter()
        .filter(|(pointer, name)| {
            if query.parse::<usize>().is_ok() {
                pointer.rsplit('/').next() == Some(query.as_str())
            } else {
                name.to_lowercase().contains(&query)
            }
        })
        .collect()
}
fn enter(editor: &mut Editor) {
    let pointer = selected_pointer(editor);
    match editor.doc.value().pointer(&pointer) {
        Some(Value::Array(_) | Value::Object(_)) => {
            editor.pointer = pointer;
            editor.page = 0;
            editor.selected = 0;
        }
        Some(_) => begin_edit(editor),
        None => (),
    }
}
fn begin_edit(editor: &mut Editor) {
    let pointer = selected_pointer(editor);
    edit_value(editor, pointer);
}
fn edit_value(editor: &mut Editor, pointer: String) {
    if let Some(value) = editor.doc.value().pointer(&pointer) {
        if let Some(value) = value.as_bool() {
            let result = editor.doc.replace(&pointer, &(!value).to_string());
            editor.result(result, "Updated checkbox");
        } else {
            let text = value.as_str().map_or_else(|| value.to_string(), str::to_owned);
            let mode = if value.is_array() || value.is_object() { Mode::Field(pointer) } else { Mode::Value(pointer) };
            editor.input = Some(Input { mode, text, select_all: true });
        }
    }
}
pub(super) fn begin_file(editor: &mut Editor, save: bool) {
    let (picker, text) = crate::files::Picker::new(editor.doc.path(), save);
    editor.status.clear();
    editor.input = Some(Input { mode: Mode::File(picker), text, select_all: true });
}
pub(super) fn request_leave(editor: &mut Editor, quit: bool) {
    if editor.doc.is_dirty() {
        editor.input = Some(Input { mode: Mode::ConfirmLeave { quit }, text: String::new(), select_all: false });
        editor.status.clear();
    } else if quit {
        editor.exit_requested = true;
    } else {
        begin_file(editor, false);
    }
}
pub(super) fn act(editor: &mut Editor, action: Action) {
    if editor.input.is_some()
        && !matches!(
            action,
            Action::Apply
                | Action::Cancel
                | Action::Browse(_)
                | Action::Files(_)
                | Action::Location
                | Action::SaveContinue
                | Action::DiscardContinue
                | Action::Pick(_)
        )
    {
        return;
    }
    match action {
        Action::Up => {
            editor.pointer = editor.pointer.rsplit_once('/').map_or("", |(p, _)| p).into();
            editor.page = 0;
            editor.selected = 0;
        }
        Action::Previous => {
            editor.page = editor.page.saturating_sub(ROWS);
            editor.selected = 0;
        }
        Action::Next => {
            if editor.page + ROWS < entries(editor).len() {
                editor.page += ROWS;
                editor.selected = 0;
            }
        }
        Action::Edit => begin_edit(editor),
        Action::Find => begin_find(editor, None),
        Action::Pick(pointer) => {
            if matches!(editor.input.as_ref().map(|input| &input.mode), Some(Mode::Find(_))) {
                editor.input = None;
                editor.pointer = pointer;
                editor.page = 0;
                editor.selected = 0;
                editor.frame = true;
            }
        }
        Action::Row(row) => {
            if editor.page + row < entries(editor).len() {
                editor.selected = row;
                enter(editor);
            }
        }
        Action::Save => {
            let result = editor.doc.save();
            editor.status = result.err().unwrap_or_else(|| "Saved".into());
        }
        Action::SaveAs => begin_file(editor, true),
        Action::Open => request_leave(editor, false),
        Action::Browse(path) => {
            let Some(Input { mode: Mode::File(picker), .. }) = &editor.input else { return };
            if path.is_dir() {
                let save = picker.save;
                let location = picker.focus == crate::files::Focus::Location;
                let input = editor.input.as_mut().unwrap();
                if !save || location {
                    input.text = if save { "level-copy.json".into() } else { String::new() };
                }
                input.mode = Mode::File(crate::files::Picker {
                    directory: path,
                    page: 0,
                    selected: 0,
                    save,
                    focus: if save { crate::files::Focus::Filename } else { crate::files::Focus::Search },
                });
                input.select_all = true;
                editor.status.clear();
            } else if !picker.save {
                open_file(editor, &path);
            } else {
                editor.status = "Choose a new filename. Existing files are never overwritten by Save copy.".into();
            }
        }
        Action::Files(next) => {
            if let Some(input) = &mut editor.input
                && let Mode::File(picker) = &mut input.mode
            {
                let count = picker.entries(&input.text).map_or(0, |paths| paths.len());
                picker.page = if next && (picker.page + 1) * crate::files::PAGE_SIZE < count {
                    picker.page + 1
                } else if !next {
                    picker.page.saturating_sub(1)
                } else {
                    picker.page
                };
                picker.selected = picker.page * crate::files::PAGE_SIZE;
            }
        }
        Action::Location => {
            if let Some(input) = &mut editor.input
                && let Mode::File(picker) = &mut input.mode
            {
                picker.focus = crate::files::Focus::Location;
                input.text = picker.directory.display().to_string();
                input.select_all = true;
            }
        }
        Action::Navigate(pointer) => {
            editor.pointer = pointer;
            editor.page = 0;
            editor.selected = 0;
        }
        Action::EditValue(pointer) => edit_value(editor, pointer),
        Action::EditAngle(pointer) => {
            if let Some(angle) = editor.doc.value().pointer(&pointer).and_then(Value::as_u64) {
                editor.input = Some(Input {
                    mode: Mode::Angle(pointer),
                    text: format!("{:.6}", angle as f64 * 360.0 / 65536.0),
                    select_all: true,
                });
            }
        }
        Action::Nudge(pointer, delta) => {
            if let Some(value) = editor.doc.value().pointer(&pointer).and_then(Value::as_i64) {
                let value = if pointer.ends_with("/rotation") {
                    (value + delta).rem_euclid(65536)
                } else {
                    value.saturating_add(delta)
                };
                let result = editor.doc.replace(&pointer, &value.to_string());
                editor.result(result, "Updated property - Ctrl+Z to undo");
            }
        }
        Action::Toggle(pointer) => {
            let result = if let Some(value) = editor.doc.value().pointer(&pointer).and_then(Value::as_bool) {
                editor.doc.replace(&pointer, &(!value).to_string())
            } else if let Some((parent, key)) = pointer.rsplit_once('/') {
                let mut value = editor.doc.value().pointer(parent).cloned().unwrap_or(Value::Null);
                if let Some(object) = value.as_object_mut() {
                    object.insert(key.into(), Value::Bool(true));
                    editor.doc.replace(parent, &value.to_string())
                } else {
                    Err("This property is not available.".into())
                }
            } else {
                Err("This property is not available.".into())
            };
            editor.result(result, "Updated checkbox");
        }
        Action::SaveContinue | Action::DiscardContinue => {
            let Some(Input { mode: Mode::ConfirmLeave { quit }, .. }) = &editor.input else { return };
            let quit = *quit;
            if matches!(action, Action::SaveContinue)
                && let Err(error) = editor.doc.save()
            {
                editor.status = error;
                return;
            }
            editor.input = None;
            if quit {
                editor.exit_requested = true;
            } else {
                begin_file(editor, false);
            }
        }
        Action::Undo => editor.undo(),
        Action::Redo => editor.redo(),
        Action::Frame => editor.frame = true,
        Action::Overview => {
            editor.frame = true;
            editor.overview = true;
        }
        Action::Apply => apply(editor),
        Action::Cancel => editor.input = None,
    }
}
fn open_file(editor: &mut Editor, path: &std::path::Path) {
    match LevelDocument::open(path) {
        Ok(doc) => {
            let old = editor.doc.path().to_path_buf();
            editor.recent.retain(|p| p != &old && p != doc.path());
            editor.recent.insert(0, old);
            editor.recent.truncate(5);
            editor.doc = doc;
            editor.pointer = "/placements".into();
            editor.page = 0;
            editor.selected = 0;
            editor.frame = true;
            editor.overview = true;
            editor.input = None;
            editor.changed("Opened level");
        }
        Err(error) => editor.status = format!("Could not open {}: {error}", path.display()),
    }
}
fn apply(editor: &mut Editor) {
    let Some(input) = editor.input.take() else { return };
    if let Mode::Find(candidates) = &input.mode {
        let target = find_matches(candidates, &input.text).first().map(|(pointer, _)| pointer.clone());
        editor.input = Some(input);
        if let Some(pointer) = target {
            act(editor, Action::Pick(pointer));
        } else {
            editor.status = "No matches. Try an item number or a name.".into();
        }
        return;
    }
    if let Mode::File(picker) = &input.mode {
        let target = if picker.focus == crate::files::Focus::Location {
            let path = std::path::PathBuf::from(&input.text);
            if path.is_absolute() { path } else { picker.directory.join(path) }
        } else if picker.save {
            // A filename is one component; use Location to change folders.
            let mut components = std::path::Path::new(&input.text).components();
            if !matches!(components.next(), Some(std::path::Component::Normal(_)))
                || components.next().is_some()
                || input.text.trim().is_empty()
            {
                editor.status = "Enter a filename, such as level-copy.json.".into();
                editor.input = Some(input);
                return;
            }
            let mut filename = std::path::PathBuf::from(&input.text);
            if filename.extension().is_none() {
                filename.set_extension("json");
            }
            picker.directory.join(filename)
        } else {
            match picker.entries(&input.text).ok().and_then(|paths| paths.get(picker.selected).cloned()) {
                Some(path) => path,
                None => {
                    editor.status = "Choose a file or folder from the list.".into();
                    editor.input = Some(input);
                    return;
                }
            }
        };
        let save = picker.save;
        let location = picker.focus == crate::files::Focus::Location;
        editor.input = Some(input);
        if target.is_dir() {
            act(editor, Action::Browse(target));
        } else if save && !location {
            let result = if target.exists() {
                Err("Choose a new filename for the copy; this file already exists.".into())
            } else {
                editor.doc.save_to(target)
            };
            if result.is_ok() {
                editor.input = None;
            }
            editor.result(result, "Saved copy");
        } else if !save {
            open_file(editor, &target);
        } else {
            editor.status = "Choose an existing folder for the location.".into();
        }
        return;
    }
    let result = match &input.mode {
        Mode::Field(pointer) => editor.doc.replace(pointer, &input.text),
        Mode::Value(pointer) => {
            if editor.doc.value().pointer(pointer).is_some_and(Value::is_string) {
                editor.doc.replace(pointer, &Value::String(input.text.clone()).to_string())
            } else if editor.doc.value().pointer(pointer).is_some_and(Value::is_number) {
                match serde_json::from_str::<Value>(&input.text) {
                    Ok(value @ Value::Number(_)) => editor.doc.replace(pointer, &value.to_string()),
                    _ => Err("Enter a number, such as 128 or -64.".into()),
                }
            } else {
                editor.doc.replace(pointer, &input.text)
            }
        }
        Mode::Angle(pointer) => {
            input.text.parse::<f64>().map_err(|_| "Enter an angle in degrees.".to_string()).and_then(|angle| {
                if !angle.is_finite() {
                    return Err("Enter a finite angle in degrees.".into());
                }
                editor.doc.replace(
                    pointer,
                    &(((angle.rem_euclid(360.0) * 65536.0 / 360.0).round() as u32) % 65536).to_string(),
                )
            })
        }
        Mode::File(_) | Mode::ConfirmLeave { .. } | Mode::Find(_) => {
            editor.input = Some(input);
            return;
        }
    };
    if result.is_err() {
        editor.input = Some(input);
    }
    editor.result(result, "Applied");
}
pub(super) fn clicked(
    mut click: On<bevy::picking::events::PointerClick>,
    mut editor: ResMut<Editor>,
    actions: Query<&Action>,
    types: Option<Res<ObjectTypes>>,
) {
    if click.button != bevy::picking::pointer::PointerButton::Primary {
        return;
    }
    if let Ok(action) = actions.get(click.entity) {
        if matches!(action, Action::Find) && editor.input.is_none() {
            begin_find(&mut editor, types.as_deref());
        } else {
            act(&mut editor, action.clone());
        }
        click.propagate(false);
    }
}

pub fn input(
    mut editor: ResMut<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut events: MessageReader<KeyboardInput>,
    mut close: MessageReader<WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
    focus: Res<crate::viewport::ViewportFocus>,
    types: Option<Res<ObjectTypes>>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let quit = close.read().next().is_some() || (ctrl && keys.just_pressed(KeyCode::KeyQ));
    if quit {
        if ctrl && shift {
            editor.exit_requested = true;
        } else {
            request_leave(&mut editor, true);
        }
    }
    if editor.exit_requested {
        exit.write(AppExit::Success);
        return;
    }
    if editor.input.is_some() {
        for event in events.read().filter(|e| e.state == ButtonState::Pressed) {
            if ctrl && event.key_code == KeyCode::KeyL {
                act(&mut editor, Action::Location);
                continue;
            }
            if let Some(Input { mode: Mode::File(picker), text, .. }) = &mut editor.input {
                if matches!(event.key_code, KeyCode::ArrowDown | KeyCode::ArrowUp) {
                    let count = picker.entries(text).map_or(0, |paths| paths.len());
                    picker.selected = if event.key_code == KeyCode::ArrowDown {
                        (picker.selected + 1).min(count.saturating_sub(1))
                    } else {
                        picker.selected.saturating_sub(1)
                    };
                    picker.page = picker.selected / crate::files::PAGE_SIZE;
                    continue;
                }
                if event.key_code == KeyCode::Backspace && text.is_empty() {
                    if let Some(parent) = picker.directory.parent() {
                        let parent = parent.to_path_buf();
                        act(&mut editor, Action::Browse(parent));
                    }
                    continue;
                }
            }
            if matches!(editor.input.as_ref().map(|i| &i.mode), Some(Mode::ConfirmLeave { .. }))
                && event.key_code != KeyCode::Escape
            {
                continue;
            }
            match event.key_code {
                KeyCode::Escape => {
                    editor.input = None;
                    break;
                }
                KeyCode::Enter if !shift => {
                    apply(&mut editor);
                    break;
                }
                KeyCode::KeyA if ctrl => {
                    if let Some(input) = &mut editor.input {
                        input.select_all = true;
                    }
                }
                KeyCode::Backspace | KeyCode::Delete => {
                    if let Some(input) = &mut editor.input {
                        if input.select_all {
                            input.text.clear();
                        } else {
                            input.text.pop();
                        }
                        input.select_all = false;
                    }
                }
                _ if !ctrl => {
                    if let (Some(input), Some(text)) = (&mut editor.input, &event.text) {
                        if input.select_all {
                            input.text.clear();
                            input.select_all = false;
                        }
                        input.text.extend(text.chars().filter(|c| !c.is_control()));
                    }
                }
                _ => (),
            }
            if let Some(Input { mode: Mode::File(picker), .. }) = &mut editor.input
                && !matches!(event.key_code, KeyCode::ArrowDown | KeyCode::ArrowUp)
            {
                picker.page = 0;
                picker.selected = 0;
            }
        }
        return;
    }
    events.clear();
    if ctrl {
        if keys.just_pressed(KeyCode::KeyF) {
            begin_find(&mut editor, types.as_deref());
        }
        for (key, action) in [
            (KeyCode::KeyS, if shift { Action::SaveAs } else { Action::Save }),
            (KeyCode::KeyO, Action::Open),
            (KeyCode::KeyZ, if shift { Action::Redo } else { Action::Undo }),
            (KeyCode::KeyY, Action::Redo),
        ] {
            if keys.just_pressed(key) {
                act(&mut editor, action);
            }
        }
        return;
    }
    if mouse.pressed(MouseButton::Right) || focus.active {
        if keys.just_pressed(KeyCode::KeyF) {
            editor.frame = true;
        }
        return;
    }
    if keys.just_pressed(KeyCode::ArrowDown) && editor.page + editor.selected + 1 < entries(&editor).len() {
        editor.selected += 1;
        if editor.selected == ROWS {
            editor.page += ROWS;
            editor.selected = 0;
        }
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        if editor.selected > 0 {
            editor.selected -= 1;
        } else if editor.page > 0 {
            editor.page = editor.page.saturating_sub(ROWS);
            editor.selected = ROWS - 1;
        }
    }
    for (key, action) in [
        (KeyCode::ArrowLeft, Action::Up),
        (KeyCode::Backspace, Action::Up),
        (KeyCode::PageDown, Action::Next),
        (KeyCode::PageUp, Action::Previous),
        (KeyCode::KeyE, Action::Edit),
        (KeyCode::KeyF, Action::Frame),
    ] {
        if keys.just_pressed(key) {
            act(&mut editor, action);
        }
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::ArrowRight) {
        enter(&mut editor);
    }
}

#[allow(clippy::type_complexity)]
fn refresh(
    editor: Res<Editor>,
    types: Res<ObjectTypes>,
    mut texts: ParamSet<(
        Query<&mut Text, With<Header>>,
        Query<&mut Text, With<Breadcrumb>>,
        Query<&mut Text, With<Prompt>>,
        Query<&mut Text, With<Footer>>,
        Query<(&FieldRow, &Children, &mut BackgroundColor)>,
        Query<&mut Text>,
    )>,
) {
    if !editor.is_changed() {
        return;
    }
    for mut text in &mut texts.p0() {
        **text = format!(
            "{}{} ({})\n{}",
            if editor.doc.is_dirty() { "* UNSAVED  " } else { "" },
            editor.doc.value()["name"].as_str().unwrap_or("Untitled level"),
            editor.doc.path().file_name().unwrap().to_string_lossy(),
            editor
                .doc
                .path()
                .parent()
                .unwrap()
                .display()
                .to_string()
                .chars()
                .rev()
                .take(60)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        );
    }
    let entries = entries(&editor);
    for mut text in &mut texts.p1() {
        **text = format!(
            "{}  |  {} entries  |  page {}",
            if editor.pointer.is_empty() {
                "All data".into()
            } else {
                editor
                    .pointer
                    .trim_start_matches('/')
                    .split('/')
                    .map(|part| if part == "placements" { "Objects".into() } else { humanize(part) })
                    .collect::<Vec<_>>()
                    .join(" / ")
            },
            entries.len(),
            editor.page / ROWS + 1
        );
    }
    let prompt = match &editor.input {
        Some(_) => "Dialog open - Escape returns to the scene.".into(),
        None => {
            let pointer = selected_pointer(&editor);
            let mut owner = pointer.as_str();
            let kind = loop {
                if let Some(kind) =
                    editor.doc.value().pointer(owner).and_then(|v| v.get("kind")).and_then(Value::as_u64)
                {
                    break Some(kind);
                }
                match owner.rsplit_once('/') {
                    Some((parent, _)) => owner = parent,
                    None => break None,
                }
            };
            let metadata = kind
                .map(|id| types.get(id as u16).map_or_else(|| format!("Type {id} (unregistered)"), |t| t.name.clone()))
                .unwrap_or_default();
            if metadata.is_empty() {
                "Click to inspect. F focuses the selection.\nAll data contains every engine field.".into()
            } else {
                metadata
            }
        }
    };
    for mut text in &mut texts.p2() {
        **text = prompt.clone();
    }
    for mut text in &mut texts.p3() {
        **text =
            format!("{}\nCtrl+S save   Ctrl+Z undo   Ctrl+Y redo\nCtrl+O open   Ctrl+Shift+S save copy", editor.status);
    }
    let mut updates = vec![];
    for (row, children, mut background) in &mut texts.p4() {
        let value = entries.get(editor.page + row.0).map_or(String::new(), |(pointer, text)| {
            if editor.pointer == "/placements" {
                let object = editor.doc.value().pointer(pointer).unwrap();
                let kind = object["kind"].as_u64().unwrap_or(0) as u16;
                let name = types.get(kind).map_or_else(|| format!("Type {kind}"), |t| t.name.clone());
                format!("{}  {name}  |  Room {}", pointer.rsplit('/').next().unwrap_or(""), object["room"])
            } else if editor.pointer == "/rooms" {
                format!(
                    "Room {}  |  {} x {} sectors",
                    pointer.rsplit('/').next().unwrap_or(""),
                    editor.doc.value().pointer(pointer).unwrap()["x_sectors"],
                    editor.doc.value().pointer(pointer).unwrap()["z_sectors"]
                )
            } else {
                text.clone()
            }
        });
        *background = BackgroundColor(if row.0 == editor.selected && !value.is_empty() {
            Color::srgb(0.13, 0.25, 0.36)
        } else {
            BACK
        });
        if let Some(&child) = children.first() {
            updates.push((child, value));
        }
    }
    for (entity, value) in updates {
        if let Ok(mut text) = texts.p5().get_mut(entity) {
            **text = value;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn camera(
    mut editor: ResMut<Editor>,
    mut cameras: Query<(&mut Transform, &mut Camera), With<GameCamera>>,
    windows: Query<&Window>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    target: Res<EditorTarget>,
    focus: Res<crate::viewport::ViewportFocus>,
) {
    let Ok((mut transform, mut camera)) = cameras.single_mut() else { return };
    let (width, height, scale) =
        windows.single().map_or((1280, 800, 1.0), |w| (w.physical_width(), w.physical_height(), w.scale_factor()));
    let left = ((PANEL * scale) as u32).min(width.saturating_sub(1));
    camera.viewport = Some(bevy::camera::Viewport {
        physical_position: UVec2::new(left, 0),
        physical_size: UVec2::new(width - left, height.max(1)),
        ..default()
    });
    if target.headless {
        camera.viewport = Some(bevy::camera::Viewport {
            physical_position: UVec2::new(PANEL as u32, 0),
            physical_size: UVec2::new(1280 - PANEL as u32, 800),
            ..default()
        });
    }
    if editor.frame {
        editor.frame = false;
        let pointer = if editor.overview {
            editor.overview = false;
            "/rooms/0".into()
        } else {
            selected_pointer(&editor)
        };
        let mut parts = pointer.split('/').skip(1);
        let table = parts.next().unwrap_or("");
        let index: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let value = editor.doc.value();
        let (center, distance) = if table == "placements" || table == "cameras" {
            let p: [i32; 3] = serde_json::from_value(value[table][index]["position"].clone()).unwrap_or([0; 3]);
            (position(p), 4.0)
        } else {
            let room = &value["rooms"][if table == "rooms" { index } else { 0 }];
            let origin: [i32; 3] = serde_json::from_value(room["origin"].clone()).unwrap_or([0; 3]);
            let x = room["x_sectors"].as_u64().unwrap_or(4) as f32;
            let z = room["z_sectors"].as_u64().unwrap_or(4) as f32;
            let y = -(room["top"].as_i64().unwrap_or(0) + room["bottom"].as_i64().unwrap_or(0)) as f32 / 2048.0;
            (
                Vec3::new(origin[0] as f32 / 1024.0 + x / 2.0, y, origin[2] as f32 / 1024.0 + z / 2.0),
                x.max(z) * 0.9 + 2.0,
            )
        };
        *transform =
            view::camera_transform(center + Vec3::new(distance * 0.7, distance * 0.8, -distance * 0.7), center);
    }
    if editor.input.is_some()
        || !focus.active
        || keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight])
    {
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
    let scroll_scale = if scroll.unit == MouseScrollUnit::Pixel { 0.01 } else { 0.75 };
    let zoom = *transform.forward() * (scroll.delta.y * scroll_scale).clamp(-20.0, 20.0);
    transform.translation += zoom;
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::Key;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin))
            .add_message::<WindowCloseRequested>()
            .insert_resource(crate::viewport::ViewportFocus { active: false })
            .insert_resource(Editor {
                doc: LevelDocument::open(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../engine/assets/levels/sandbox/level.json"
                ))
                .unwrap(),
                pointer: "/placements/1/position".into(),
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
            })
            .add_systems(Update, input);
        app
    }

    fn tap(app: &mut App, code: KeyCode, text: Option<&str>) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut().write_message(KeyboardInput {
                key_code: code,
                logical_key: Key::Character(text.unwrap_or("").into()),
                state,
                text: text.map(Into::into),
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
        }
    }

    #[test]
    fn keyboard_field_edit_applies_to_the_document_and_escape_discards_the_draft() {
        let mut app = app();
        tap(&mut app, KeyCode::KeyE, Some("e"));
        assert!(app.world().resource::<Editor>().input.is_some());
        tap(&mut app, KeyCode::Digit4, Some("4700"));
        tap(&mut app, KeyCode::Enter, None);
        let editor = app.world().resource::<Editor>();
        assert_eq!(editor.doc.level().unwrap().placements[1].position[0], 4700);
        assert!(editor.doc.is_dirty());
        tap(&mut app, KeyCode::KeyE, Some("e"));
        tap(&mut app, KeyCode::Digit1, Some("123"));
        tap(&mut app, KeyCode::Escape, None);
        assert_eq!(app.world().resource::<Editor>().doc.level().unwrap().placements[1].position[0], 4700);
        app.world_mut().resource_mut::<Editor>().undo();
        assert!(!app.world().resource::<Editor>().doc.is_dirty());
    }

    #[test]
    fn viewport_keys_move_the_camera_without_starting_a_draft() {
        let mut app = app();
        app.insert_resource(EditorTarget { headless: true, image: Handle::default() })
            .insert_resource(crate::viewport::ViewportFocus { active: true })
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)))
            .add_systems(Update, camera.after(input));
        let entity = app.world_mut().spawn((GameCamera, Camera::default(), Transform::default())).id();
        app.update();
        tap(&mut app, KeyCode::KeyW, Some("w"));
        let transform = *app.world().get::<Transform>(entity).unwrap();
        assert!(transform.translation.z < -0.1, "W should fly without holding right mouse");
        tap(&mut app, KeyCode::KeyE, Some("e"));
        assert!(app.world().get::<Transform>(entity).unwrap().translation.y > 0.1);
        assert!(app.world().resource::<Editor>().input.is_none(), "E in the viewport must not edit JSON");
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
        app.world_mut().write_message(bevy::input::mouse::MouseMotion { delta: Vec2::new(50.0, -20.0) });
        app.update();
        assert_ne!(app.world().get::<Transform>(entity).unwrap().rotation, Quat::IDENTITY);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
        app.update();
        let before_pan = *app.world().get::<Transform>(entity).unwrap();
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Middle);
        app.world_mut().write_message(bevy::input::mouse::MouseMotion { delta: Vec2::new(20.0, 10.0) });
        app.update();
        assert_ne!(app.world().get::<Transform>(entity).unwrap().translation, before_pan.translation);
        assert_eq!(app.world().get::<Transform>(entity).unwrap().rotation, before_pan.rotation);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Middle);
        let before_zoom = app.world().get::<Transform>(entity).unwrap().translation;
        app.world_mut().write_message(bevy::input::mouse::MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert_ne!(app.world().get::<Transform>(entity).unwrap().translation, before_zoom);
        app.world_mut().resource_mut::<crate::viewport::ViewportFocus>().active = false;
        let before = *app.world().get::<Transform>(entity).unwrap();
        tap(&mut app, KeyCode::KeyW, Some("w"));
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), before);
        tap(&mut app, KeyCode::KeyE, Some("e"));
        assert!(app.world().resource::<Editor>().input.is_some());
        app.world_mut().resource_mut::<crate::viewport::ViewportFocus>().active = true;
        tap(&mut app, KeyCode::KeyW, Some("w"));
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), before, "draft must capture typing");
        tap(&mut app, KeyCode::Escape, None);
        tap(&mut app, KeyCode::KeyW, Some("w"));
        assert_ne!(*app.world().get::<Transform>(entity).unwrap(), before);
    }

    #[test]
    fn open_browses_directories_and_reports_errors_without_losing_the_document() {
        let mut app = app();
        let editor = &mut *app.world_mut().resource_mut::<Editor>();
        let original = editor.doc.path().to_path_buf();
        act(editor, Action::Open);
        let directory = original.parent().unwrap().to_path_buf();
        act(editor, Action::Browse(directory.clone()));
        assert!(matches!(&editor.input.as_ref().unwrap().mode, Mode::File(picker) if picker.directory == directory));
        act(editor, Action::Location);
        editor.input.as_mut().unwrap().text = directory.join("missing.json").display().to_string();
        act(editor, Action::Apply);
        assert!(editor.input.is_some());
        assert_eq!(editor.doc.path(), original);
        act(editor, Action::Browse(original.clone()));
        assert!(editor.input.is_none());
        assert_eq!(editor.doc.path(), original);
        assert!(!editor.doc.is_dirty());
        assert!(editor.frame);
        let temporary = std::env::temp_dir().join(format!(
            "editor-open-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&temporary).unwrap();
        let copy = temporary.join("another-level.json");
        std::fs::copy(&original, &copy).unwrap();
        act(editor, Action::Open);
        act(editor, Action::Browse(temporary.clone()));
        act(editor, Action::Browse(copy.clone()));
        assert_eq!(editor.doc.path(), copy);
        assert!(editor.input.is_none());
        std::fs::remove_file(copy).unwrap();
        std::fs::remove_dir(temporary).unwrap();
    }

    struct Temporary(std::path::PathBuf);
    impl Temporary {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "editor-usability-{}-{}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn file_search_keyboard_paging_and_save_copy_need_no_typed_paths() {
        let mut app = app();
        let temporary = Temporary::new();
        let original = std::fs::read(app.world().resource::<Editor>().doc.path()).unwrap();
        for index in 0..12 {
            std::fs::write(temporary.0.join(format!("level-{index:02}.json")), &original).unwrap();
        }
        std::fs::write(temporary.0.join("ignore.txt"), "not a level").unwrap();
        {
            let editor = &mut *app.world_mut().resource_mut::<Editor>();
            act(editor, Action::Open);
            act(editor, Action::Browse(temporary.0.clone()));
            act(editor, Action::Files(true));
            assert!(matches!(&editor.input.as_ref().unwrap().mode, Mode::File(p) if p.page == 1));
        }
        // Typing searches, resets paging, and Enter opens the matching file.
        tap(&mut app, KeyCode::KeyL, Some("level-09"));
        tap(&mut app, KeyCode::Enter, None);
        let editor = &mut *app.world_mut().resource_mut::<Editor>();
        assert_eq!(editor.doc.path(), temporary.0.join("level-09.json"));
        assert!(editor.input.is_none());
        assert_eq!(editor.recent.len(), 1);
        act(editor, Action::SaveAs);
        let copies = temporary.0.join("copies");
        std::fs::create_dir(&copies).unwrap();
        act(editor, Action::Browse(copies.clone()));
        act(editor, Action::Apply);
        let saved = copies.join("level-09-copy.json");
        assert_eq!(editor.doc.path(), saved);
        assert_eq!(std::fs::read(saved).unwrap(), original);
        act(editor, Action::SaveAs);
        editor.input.as_mut().unwrap().text = "level-09-copy.json".into();
        act(editor, Action::Apply);
        assert!(editor.input.is_some());
        assert!(editor.status.contains("new filename"));
        assert_eq!(std::fs::read(temporary.0.join("level-09.json")).unwrap(), original);
    }

    #[test]
    fn unsaved_choices_keep_the_document_until_a_successful_open_and_never_overwrite_conflicts() {
        let mut app = app();
        let temporary = Temporary::new();
        let editor = &mut *app.world_mut().resource_mut::<Editor>();
        let copy = temporary.0.join("level.json");
        let original = std::fs::read(editor.doc.path()).unwrap();
        std::fs::write(&copy, &original).unwrap();
        editor.doc = LevelDocument::open(&copy).unwrap();
        act(editor, Action::Nudge("/placements/1/position/0".into(), 128));
        let changed = editor.doc.value().clone();
        act(editor, Action::Open);
        assert!(matches!(editor.input.as_ref().unwrap().mode, Mode::ConfirmLeave { quit: false }));
        act(editor, Action::Cancel);
        assert_eq!(editor.doc.value(), &changed);
        act(editor, Action::Open);
        act(editor, Action::DiscardContinue);
        act(editor, Action::Browse(temporary.0.join("missing.json")));
        assert_eq!(editor.doc.value(), &changed);
        assert!(editor.doc.is_dirty());
        act(editor, Action::Cancel);
        act(editor, Action::Open);
        act(editor, Action::SaveContinue);
        assert!(!editor.doc.is_dirty());
        assert!(matches!(editor.input.as_ref().unwrap().mode, Mode::File(_)));
        act(editor, Action::Cancel);
        act(editor, Action::Nudge("/placements/1/position/0".into(), 128));
        std::fs::write(&copy, &original).unwrap(); // external modification
        act(editor, Action::Open);
        act(editor, Action::SaveContinue);
        assert!(matches!(editor.input.as_ref().unwrap().mode, Mode::ConfirmLeave { .. }));
        assert!(editor.doc.is_dirty());
        assert!(editor.status.contains("changed on disk"));
        assert_eq!(std::fs::read(copy).unwrap(), original);
        act(editor, Action::Cancel);
        request_leave(editor, true);
        act(editor, Action::Cancel);
        assert!(!editor.exit_requested);
        request_leave(editor, true);
        act(editor, Action::DiscardContinue);
        assert!(editor.exit_requested);
    }

    #[test]
    fn common_properties_use_numbers_degrees_and_plain_text_with_validation_and_undo() {
        let mut app = app();
        let editor = &mut *app.world_mut().resource_mut::<Editor>();
        let before = editor.doc.value().clone();
        act(editor, Action::Nudge("/placements/1/position/0".into(), 128));
        assert_eq!(
            editor.doc.value()["placements"][1]["position"][0].as_i64(),
            Some(before["placements"][1]["position"][0].as_i64().unwrap() + 128)
        );
        act(editor, Action::Undo);
        assert_eq!(editor.doc.value(), &before);
        editor.doc.replace("/placements/1/rotation", "9999").unwrap();
        act(editor, Action::EditAngle("/placements/1/rotation".into()));
        act(editor, Action::Apply);
        assert_eq!(editor.doc.value()["placements"][1]["rotation"], 9999);
        act(editor, Action::EditAngle("/placements/1/rotation".into()));
        editor.input.as_mut().unwrap().text = "90".into();
        act(editor, Action::Apply);
        assert_eq!(editor.doc.value()["placements"][1]["rotation"], 16384);
        act(editor, Action::Toggle("/placements/1/invisible".into()));
        assert_eq!(editor.doc.value()["placements"][1]["invisible"], true);
        act(editor, Action::Undo);
        assert!(editor.doc.value()["placements"][1].get("invisible").is_none());
        act(editor, Action::EditValue("/placements/1/room".into()));
        editor.input.as_mut().unwrap().text = "9999".into();
        act(editor, Action::Apply);
        assert!(editor.input.is_some());
        assert_eq!(editor.doc.value()["placements"][1]["room"], 0);
        act(editor, Action::Cancel);
        act(editor, Action::EditValue("/name".into()));
        editor.input.as_mut().unwrap().text = "A level with \"quotes\"".into();
        act(editor, Action::Apply);
        assert_eq!(editor.doc.value()["name"], "A level with \"quotes\"");
    }

    // Exercise layout, hit testing and event bubbling from the label to its
    // control. Triggering PointerClick directly would miss a disabled backend.
    #[test]
    fn mouse_clicks_browse_rows_and_apply_and_cancel_edits() {
        use bevy::{
            camera::{ComputedCameraValues, RenderTargetInfo},
            picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
            ui::UiGlobalTransform,
        };

        let mut app = app();
        app.add_plugins((
            bevy::asset::AssetPlugin::default(),
            bevy::transform::TransformPlugin,
            WindowPlugin::default(),
            bevy::image::ImagePlugin::default(),
            bevy::image::TextureAtlasPlugin,
            bevy::mesh::MeshPlugin,
            bevy::text::TextPlugin,
            bevy::camera::CameraPlugin,
            bevy::ui::UiPlugin,
            bevy::picking::DefaultPickingPlugins,
            UiPlugin,
        ))
        .add_systems(Update, (crate::dialogs::refresh, crate::inspector::refresh).after(input))
        .insert_resource(EditorTarget { headless: false, image: Handle::default() })
        .init_resource::<ObjectTypes>();
        app.update();
        let camera = app.world_mut().query_filtered::<Entity, With<Camera2d>>().single(app.world()).unwrap();
        app.world_mut().get_mut::<Camera>(camera).unwrap().computed = ComputedCameraValues {
            target_info: Some(RenderTargetInfo { physical_size: UVec2::new(1280, 800), scale_factor: 1.0 }),
            ..default()
        };
        // Font loading and layout settle without a renderer or OS window.
        for _ in 0..5 {
            app.update();
        }

        let click = |app: &mut App, matches: fn(&Action) -> bool| {
            let control = app
                .world_mut()
                .query::<(Entity, &Action)>()
                .iter(app.world())
                .find(|(_, action)| matches(action))
                .unwrap()
                .0;
            let label = app.world().get::<Children>(control).unwrap()[0];
            let position = app.world().get::<UiGlobalTransform>(label).unwrap().translation;
            assert!(app.world().get::<ComputedNode>(label).unwrap().size().min_element() > 0.0);
            let target = RenderTarget::default()
                .normalize(Some(
                    app.world_mut()
                        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
                        .single(app.world())
                        .unwrap(),
                ))
                .unwrap();
            let location = Location { target, position };
            for action in [
                PointerAction::Move { delta: Vec2::ZERO },
                PointerAction::Press(PointerButton::Primary),
                PointerAction::Release(PointerButton::Primary),
            ] {
                app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location.clone(), action));
                app.update();
            }
        };
        click(&mut app, |a| matches!(a, Action::Up));
        assert_eq!(app.world().resource::<Editor>().pointer, "/placements/1");
        click(&mut app, |a| matches!(a, Action::Row(0)));
        assert!(app.world().resource::<Editor>().input.is_some());
        click(&mut app, |a| matches!(a, Action::Cancel));
        assert!(app.world().resource::<Editor>().input.is_none());

        app.world_mut().resource_mut::<Editor>().pointer = "/placements/1/position".into();
        app.update();
        click(&mut app, |a| matches!(a, Action::Row(0)));
        assert!(app.world().resource::<Editor>().input.is_some());
        app.world_mut().resource_mut::<Editor>().input.as_mut().unwrap().text = "4700".into();
        click(&mut app, |a| matches!(a, Action::Apply));
        assert_eq!(app.world().resource::<Editor>().doc.level().unwrap().placements[1].position[0], 4700);
        click(&mut app, |a| matches!(a, Action::Undo));
        assert!(!app.world().resource::<Editor>().doc.is_dirty());
        click(&mut app, |a| matches!(a, Action::Open));
        assert!(app.world_mut().query::<&crate::dialogs::Dialog>().single(app.world()).is_ok());
        assert!(matches!(app.world().resource::<Editor>().input.as_ref().unwrap().mode, Mode::File(_)));
        click(&mut app, |a| matches!(a, Action::Browse(p) if p.file_name().is_some_and(|n| n == "level.json")));
        assert!(app.world().resource::<Editor>().input.is_none());
        click(&mut app, |a| matches!(a, Action::Navigate(p) if p == "/placements"));
        click(&mut app, |a| matches!(a, Action::Row(1)));
        let before = app.world().resource::<Editor>().doc.value()["placements"][1]["position"][0].as_i64().unwrap();
        click(&mut app, |a| matches!(a, Action::Nudge(p, 128) if p == "/placements/1/position/0"));
        assert_eq!(app.world().resource::<Editor>().doc.value()["placements"][1]["position"][0], before + 128);
        click(&mut app, |a| matches!(a, Action::Undo));
        assert!(!app.world().resource::<Editor>().doc.is_dirty());
    }
}
