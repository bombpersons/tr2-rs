use crate::{Editor, EditorTarget};
use bevy::{
    camera::RenderTarget,
    input::{ButtonState, keyboard::KeyboardInput, mouse::AccumulatedMouseMotion},
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

const ROWS: usize = 12;
const PANEL: f32 = 500.0;
const INK: Color = Color::srgb(0.86, 0.90, 0.97);
const MUTED: Color = Color::srgb(0.50, 0.62, 0.74);
const BACK: Color = Color::srgb(0.055, 0.07, 0.10);

pub struct Input {
    mode: Mode,
    text: String,
    select_all: bool,
}
enum Mode {
    Field(String),
    Open,
    SaveAs,
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
#[derive(Component, Clone, Copy)]
enum Action {
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
    Apply,
    Cancel,
    Row(usize),
}

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, refresh.after(camera));
    }
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(text), TextFont { font_size: bevy::text::FontSize::Px(size), ..default() }, TextColor(color))
}
fn button(parent: &mut ChildSpawnerCommands, text: &str, action: Action) {
    parent
        .spawn((
            action,
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
            root.spawn(label("LEVEL EDITOR", 23.0, INK));
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
                    ("Save as", Action::SaveAs),
                    ("Undo", Action::Undo),
                    ("Redo", Action::Redo),
                    ("Frame", Action::Frame),
                ] {
                    button(bar, text, action);
                }
            });
            root.spawn((
                Breadcrumb,
                label("/", 15.0, Color::srgb(0.4, 0.8, 1.0)),
                Node { height: px(28), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
            ));
            root.spawn(Node { column_gap: px(5), ..default() }).with_children(|bar| {
                for (text, action) in [
                    ("Parent", Action::Up),
                    ("Previous", Action::Previous),
                    ("Next", Action::Next),
                    ("Edit JSON", Action::Edit),
                ] {
                    button(bar, text, action);
                }
            });
            for row in 0..ROWS {
                root.spawn((
                    Action::Row(row),
                    FieldRow(row),
                    Node {
                        height: px(23),
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
                Prompt,
                label("", 14.0, INK),
                Node { height: px(75), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
            ));
            root.spawn(Node { column_gap: px(5), ..default() }).with_children(|bar| {
                button(bar, "Apply / Enter", Action::Apply);
                button(bar, "Cancel / Esc", Action::Cancel);
            });
            root.spawn((Footer, label("", 13.0, MUTED), Node { overflow: Overflow::clip(), ..default() }));
        });
    commands.spawn((UiTargetCamera(camera), Node { position_type: PositionType::Absolute, left: px(PANEL + 16.0), top: px(16), padding: UiRect::all(px(10)), ..default() }, BackgroundColor(BACK)))
        .with_child(label("3D PREVIEW   |   Right drag: look   WASD: move   Q/E: down/up   Shift: faster\nF: frame room or placement   |   Full resolution / standard Bevy rendering", 14.0, INK));
}

/// Child browsing is bounded even for very large vertex/animation tables.
fn entries(editor: &Editor) -> Vec<(String, String)> {
    let Some(value) = editor.doc.value().pointer(&editor.pointer) else { return vec![] };
    match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| (child_pointer(&editor.pointer, key), format!("{key}   {}", summary(value))))
            .collect(),
        Value::Array(array) => array
            .iter()
            .enumerate()
            .map(|(i, value)| (child_pointer(&editor.pointer, &i.to_string()), format!("[{i}]   {}", summary(value))))
            .collect(),
        value => vec![(editor.pointer.clone(), summary(value))],
    }
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
fn selected_pointer(editor: &Editor) -> String {
    entries(editor).get(editor.page + editor.selected).map_or_else(|| editor.pointer.clone(), |(p, _)| p.clone())
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
    if let Some(value) = editor.doc.value().pointer(&pointer) {
        editor.input = Some(Input { mode: Mode::Field(pointer), text: value.to_string(), select_all: true });
    }
}
fn act(editor: &mut Editor, action: Action) {
    if editor.input.is_some() && !matches!(action, Action::Apply | Action::Cancel) {
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
        Action::SaveAs => {
            editor.input =
                Some(Input { mode: Mode::SaveAs, text: editor.doc.path().display().to_string(), select_all: true })
        }
        Action::Open => {
            if editor.doc.is_dirty() {
                editor.status = "Save or undo your changes before opening another file.".into();
            } else {
                editor.input =
                    Some(Input { mode: Mode::Open, text: editor.doc.path().display().to_string(), select_all: true });
            }
        }
        Action::Undo => editor.undo(),
        Action::Redo => editor.redo(),
        Action::Frame => editor.frame = true,
        Action::Apply => apply(editor),
        Action::Cancel => editor.input = None,
    }
}
fn apply(editor: &mut Editor) {
    let Some(input) = editor.input.take() else { return };
    let result = match &input.mode {
        Mode::Field(pointer) => editor.doc.replace(pointer, &input.text),
        Mode::SaveAs => editor.doc.save_to(&input.text),
        Mode::Open => LevelDocument::open(&input.text).map(|doc| {
            editor.doc = doc;
            editor.pointer.clear();
            editor.page = 0;
            editor.selected = 0;
            editor.frame = true;
        }),
    };
    if result.is_err() {
        editor.input = Some(input);
    }
    editor.result(result, "Applied");
}
fn clicked(mut click: On<bevy::picking::events::PointerClick>, mut editor: ResMut<Editor>, actions: Query<&Action>) {
    if let Ok(action) = actions.get(click.entity) {
        act(&mut editor, *action);
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
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let quit = close.read().next().is_some() || (ctrl && keys.just_pressed(KeyCode::KeyQ));
    if quit {
        if !editor.doc.is_dirty() || (ctrl && shift) {
            exit.write(AppExit::Success);
        } else {
            editor.status = "Unsaved changes. Save first, or Ctrl+Shift+Q to discard and quit.".into();
        }
    }
    if editor.input.is_some() {
        for event in events.read().filter(|e| e.state == ButtonState::Pressed) {
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
        }
        return;
    }
    events.clear();
    if ctrl {
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
    if mouse.pressed(MouseButton::Right) {
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
            "{}{}\n{}",
            if editor.doc.is_dirty() { "* UNSAVED  " } else { "" },
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
            if editor.pointer.is_empty() { "/" } else { &editor.pointer },
            entries.len(),
            editor.page / ROWS + 1
        );
    }
    let prompt = match &editor.input {
        Some(input) => {
            let title = match &input.mode {
                Mode::Field(p) => format!("Edit {p} (JSON)"),
                Mode::Open => "Open level file".into(),
                Mode::SaveAs => "Save as new file (atlas/audio stay in their folder)".into(),
            };
            let tail: String = input.text.chars().rev().take(280).collect::<String>().chars().rev().collect();
            format!("{title}\n{}{}", if input.select_all { "[Selected] " } else { "" }, tail)
        }
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
            format!(
                "{metadata}\nSelect a field. Enter opens a table; E edits JSON.\n{} registered object types",
                types.iter().count()
            )
        }
    };
    for mut text in &mut texts.p2() {
        **text = prompt.clone();
    }
    for mut text in &mut texts.p3() {
        **text = format!(
            "{}\nCtrl+S save   Ctrl+Z undo   Ctrl+Y redo\nCtrl+O open   Ctrl+Shift+S save as   Ctrl+Q quit",
            editor.status
        );
    }
    let mut updates = vec![];
    for (row, children, mut background) in &mut texts.p4() {
        let value = entries.get(editor.page + row.0).map_or("", |(_, text)| text.as_str());
        *background = BackgroundColor(if row.0 == editor.selected && !value.is_empty() {
            Color::srgb(0.13, 0.25, 0.36)
        } else {
            BACK
        });
        if let Some(&child) = children.first() {
            updates.push((child, value.to_string()));
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
    target: Res<EditorTarget>,
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
        let pointer = selected_pointer(&editor);
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
    if editor.input.is_some() || !mouse.pressed(MouseButton::Right) {
        return;
    }
    let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    yaw -= motion.delta.x * 0.003;
    pitch = (pitch - motion.delta.y * 0.003).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
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
}
