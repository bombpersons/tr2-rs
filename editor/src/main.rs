//! A full-resolution authoring app over engine formats, with no game dependency.
mod animation;
mod edit;
mod files;
mod gizmo;
mod mesh;
mod ui;
mod viewport;
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        settings::WgpuSettings,
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
};
use engine::{
    camera::CameraState,
    editor::LevelDocument,
    psx::{PsxMaterial, PsxScreen, PsxView},
    render::{GameCamera, LevelSystems, PreviewLevel},
};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Resource)]
struct Editor {
    doc: LevelDocument,
    /// Selected placement indices; the last one is active and shown in Properties.
    selection: Vec<usize>,
    /// Values shown while a drag is in progress, committed as one undo step.
    overlay: BTreeMap<String, Value>,
    /// The Data window's current table or field.
    pointer: String,
    /// The Data window's draft: the field or table being edited, and its text.
    draft: Option<(String, String)>,
    status: String,
    revision: u64,
    preview: bool,
    frame: bool,
    recent: Vec<PathBuf>,
    exit_requested: bool,
    overview: bool,
    /// What to do once unsaved changes are saved or discarded.
    leaving: Option<Leave>,
    windows: ui::Windows,
    /// A native file dialog to show next.
    ask: Option<files::Ask>,
    /// Mesh mode's room, selection and tools.
    mesh: mesh::MeshEdit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Leave {
    Quit,
    /// Open this level, or ask for one with a native dialog.
    Open(Option<PathBuf>),
}

impl Editor {
    fn new(doc: LevelDocument) -> Self {
        Self {
            doc,
            selection: vec![],
            overlay: BTreeMap::new(),
            pointer: String::new(),
            draft: None,
            status: "Ready".into(),
            revision: 0,
            preview: true,
            frame: true,
            recent: vec![],
            exit_requested: false,
            overview: true,
            leaving: None,
            windows: default(),
            ask: None,
            mesh: default(),
        }
    }
}

/// Automation uses the same transactional commands as the UI. The template's
/// MCP can set these reflected fields on the entity named `Editor`.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct EditorCommand {
    pointer: String,
    json: String,
    apply: bool,
    navigate: bool,
    undo: bool,
    redo: bool,
    save: bool,
    frame: bool,
    /// Open this level file (asking about unsaved changes first).
    open: String,
    /// Save As this path; an existing file is replaced.
    save_as: String,
    /// Placement indices, such as "1,2"; "none" clears the selection.
    select: String,
    /// Dismiss the unsaved-changes question, keeping the document.
    cancel: bool,
    /// "object" or "mesh".
    mode: String,
    /// Mesh mode's room; negative leaves it.
    mesh_room: i64,
    /// Mesh elements such as "q12,v4" ("none" clears); Shift-click semantics
    /// are not needed: the list replaces the selection.
    mesh_select: String,
    /// Move the mesh selection by "x,y,z" level units (snapped to the grid).
    mesh_move: String,
    /// Raise (negative) or lower the selected floor or ceiling faces.
    mesh_extrude: i32,
    /// Push the selected wall faces out by a sector.
    mesh_push: bool,
    /// Paint the selected faces with this texture-table entry.
    mesh_paint: i64,
    /// Open the Textures window.
    show_textures: bool,
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
struct EditorStatus {
    file: String,
    dirty: bool,
    pointer: String,
    message: String,
    revision: u64,
    rooms: usize,
    placements: usize,
    selection: Vec<usize>,
    asking_to_save: bool,
    mode: String,
    mesh_room: usize,
    mesh_selection: String,
}

fn main() -> AppExit {
    let args = engine::launcher::Args::from_env();
    if args.flag("--help") {
        println!(
            "cargo run -p editor -- --level PATH/level.json [--types PATH/types.json] [--software] [--headless --mcp]\nSee docs/editor.md for controls and save semantics."
        );
        return AppExit::Success;
    }
    let path = args.value("--level").unwrap_or_else(|| "engine/assets/levels/sandbox/level.json".into());
    let doc = match LevelDocument::open(&path) {
        Ok(doc) => doc,
        Err(error) => {
            eprintln!("{}: {error}", path);
            return AppExit::error();
        }
    };
    #[cfg(not(feature = "dev-mcp"))]
    if args.flag("--mcp") {
        eprintln!("--mcp requires --features dev-mcp");
        return AppExit::error();
    }
    let headless = args.flag("--headless");
    let mut app = App::new();
    let mut plugins = DefaultPlugins
        .set(AssetPlugin { file_path: std::path::MAIN_SEPARATOR_STR.into(), ..default() })
        .set(WindowPlugin {
            primary_window: (!headless).then(|| Window {
                title: "Level editor".into(),
                resolution: (1280, 800).into(),
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: 800.0,
                    min_height: 600.0,
                    ..default()
                },
                ..default()
            }),
            close_when_requested: false,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        })
        .set(bevy::render::RenderPlugin {
            render_creation: WgpuSettings { force_fallback_adapter: args.flag("--software"), ..default() }.into(),
            synchronous_pipeline_compilation: true,
            ..default()
        });
    if headless {
        plugins = plugins.disable::<bevy::winit::WinitPlugin>();
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 60.0)));
    }
    // Audio and controller discovery aren't needed to author assets.
    plugins = plugins.disable::<bevy::audio::AudioPlugin>().disable::<bevy::gilrs::GilrsPlugin>();
    app.add_plugins(plugins)
        .add_plugins(engine::shell::ShellPlugin {
            tick: engine::shell::TickRate::every_nth_field(engine::psx::Region::Ntsc, 2),
        })
        .init_asset::<PsxMaterial>()
        .init_resource::<CameraState>()
        // Selection casts only on click.
        .insert_resource(bevy::picking::mesh_picking::MeshPickingSettings { require_markers: true, ..default() })
        .insert_resource(engine::render::ViewSettings { far: 10000.0, ..default() })
        .add_plugins(engine::render::RenderPlugin)
        .insert_resource(Editor::new(doc))
        .add_systems(Startup, setup)
        .add_systems(Update, (automation, files::show, files::receive, preview).chain().before(LevelSystems::Load))
        .add_systems(Update, (standard_preview, status).chain().after(LevelSystems::Visibility))
        .add_plugins((
            ui::UiPlugin,
            viewport::ViewportPlugin,
            gizmo::GizmoPlugin,
            mesh::MeshPlugin,
            animation::AnimationPreviewPlugin,
        ))
        .init_resource::<files::FileDialogs>();
    // The MCP's `screen` is an ordinary full-resolution render target here;
    // no PS1 plugin, screen pass, quantization or affine shader is installed.
    let image = app.world_mut().resource_mut::<Assets<Image>>().add({
        let mut image =
            Image::new_target_texture(1280, 800, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
        image
    });
    app.insert_resource(PsxScreen {
        width: 1280,
        height: 800,
        image: image.clone(),
        canvas: image.clone(),
        ..default()
    });
    app.insert_resource(EditorTarget { headless, image });
    #[cfg(feature = "dev-mcp")]
    if args.flag("--mcp") {
        app.add_plugins(engine::dev_mcp::GameMcpPlugin);
    }
    if let Some(path) = args.value("--screenshot") {
        app.insert_resource(Capture { path, frames: 0, requested: false }).add_systems(Update, capture);
    }
    if let Some(path) = args.value("--types") {
        let types = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| engine::objects::ObjectTypes::names_from_json(&text).map_err(|e| e.to_string()));
        match types {
            Ok(types) => {
                app.insert_resource(types);
            }
            Err(error) => {
                eprintln!("{path}: {error}");
                return AppExit::error();
            }
        }
    }
    app.run()
}

#[derive(Resource)]
struct EditorTarget {
    headless: bool,
    image: Handle<Image>,
}
#[derive(Resource)]
struct Capture {
    path: String,
    frames: u32,
    requested: bool,
}
fn capture(mut commands: Commands, mut capture: ResMut<Capture>, target: Res<EditorTarget>) {
    capture.frames += 1;
    if capture.frames > 100 && !capture.requested {
        capture.requested = true;
        let screenshot =
            if target.headless { Screenshot::image(target.image.clone()) } else { Screenshot::primary_window() };
        commands.spawn(screenshot).observe(save_to_disk(capture.path.clone())).observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Editor"),
        EditorCommand { mesh_room: -1, mesh_paint: -1, ..default() },
        EditorStatus::default(),
        animation::AnimationPreview::default(),
    ));
}

fn automation(mut editor: ResMut<Editor>, mut command: Query<&mut EditorCommand>) {
    let Ok(mut c) = command.single_mut() else { return };
    if !c.mode.is_empty() {
        let mode = std::mem::take(&mut c.mode);
        editor.set_mesh_mode(mode == "mesh");
    }
    if c.mesh_room >= 0 {
        let room = std::mem::replace(&mut c.mesh_room, -1);
        editor.mesh_room(room as usize);
    }
    if !c.mesh_select.is_empty() {
        let text = std::mem::take(&mut c.mesh_select);
        editor.mesh_clear();
        let room = editor.mesh.room;
        for element in text.split(',').filter_map(mesh::Element::parse) {
            editor.mesh_click(room, Some(element), true);
        }
    }
    if !c.mesh_move.is_empty() {
        let text = std::mem::take(&mut c.mesh_move);
        match text.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<Vec<_>, _>>().as_deref() {
            Ok(&[x, y, z]) => editor.mesh_move([x, y, z]),
            _ => editor.status = "mesh_move takes x,y,z".into(),
        }
    }
    if c.mesh_extrude != 0 {
        let dy = std::mem::take(&mut c.mesh_extrude);
        editor.mesh_extrude(dy);
    }
    if c.mesh_push {
        c.mesh_push = false;
        editor.mesh_push();
    }
    if c.show_textures {
        c.show_textures = false;
        editor.windows.textures = true;
    }
    if c.mesh_paint >= 0 {
        let texture = std::mem::replace(&mut c.mesh_paint, -1);
        editor.mesh.brush = Some(texture as usize);
        editor.mesh_paint(None);
    }
    if !c.open.is_empty() {
        let path = std::mem::take(&mut c.open);
        editor.request_leave(Leave::Open(Some(path.into())));
    }
    if c.cancel {
        c.cancel = false;
        editor.leaving = None;
    }
    if !c.save_as.is_empty() {
        let path = std::mem::take(&mut c.save_as);
        editor.save_as(path.into());
    }
    if !c.select.is_empty() {
        let text = std::mem::take(&mut c.select);
        editor.selection = text.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        editor.status = format!("Selected {} objects", editor.selection.len());
    }
    if c.navigate {
        c.navigate = false;
        if editor.doc.value().pointer(&c.pointer).is_some() {
            editor.navigate(c.pointer.clone());
        } else {
            editor.status = "No such field".into();
        }
    }
    if c.apply {
        c.apply = false;
        let result = editor.doc.replace(&c.pointer, &c.json);
        if result.is_ok() {
            editor.pointer = c.pointer.rsplit_once('/').map_or("", |(p, _)| p).into();
        }
        editor.result(result, "Applied edit");
    }
    if c.undo {
        c.undo = false;
        editor.undo();
    }
    if c.redo {
        c.redo = false;
        editor.redo();
    }
    if c.save {
        c.save = false;
        editor.save();
    }
    if c.frame {
        c.frame = false;
        editor.frame = true;
    }
}

impl Editor {
    /// The level's atlas as an asset path (the asset root is the filesystem root).
    fn atlas(&self) -> String {
        let atlas = self.doc.path().parent().unwrap().join("atlas.png");
        atlas.to_string_lossy().trim_start_matches('/').into()
    }
    fn result(&mut self, result: Result<(), String>, success: &str) {
        match result {
            Ok(()) => self.changed(success),
            Err(error) => self.status = error,
        }
    }
    fn undo(&mut self) {
        self.overlay.clear();
        if self.doc.undo() {
            self.changed("Undone");
        }
    }
    fn redo(&mut self) {
        self.overlay.clear();
        if self.doc.redo() {
            self.changed("Redone");
        }
    }
    fn changed(&mut self, text: &str) {
        self.status = text.into();
        self.revision += 1;
        self.preview = true;
        let count = self.doc.value()["placements"].as_array().map_or(0, Vec::len);
        self.selection.retain(|&index| index < count);
    }
    fn navigate(&mut self, pointer: String) {
        self.pointer = pointer;
        self.draft = None;
    }
    fn save(&mut self) {
        let result = self.doc.save();
        self.status = result.err().unwrap_or_else(|| "Saved".into());
        self.revision += 1;
    }
    /// Save to another file; the native dialog has already confirmed replacing
    /// it. The level's atlas is copied alongside when the folder has none, so
    /// the copy still previews with its textures.
    fn save_as(&mut self, path: PathBuf) {
        let path = if path.extension().is_none() { path.with_extension("json") } else { path };
        if std::fs::canonicalize(&path).is_ok_and(|p| p == self.doc.path()) {
            return self.save();
        }
        let atlas = self.doc.path().with_file_name("atlas.png");
        let result = self.doc.save_replacing(&path);
        if result.is_ok() {
            let target = self.doc.path().with_file_name("atlas.png");
            if atlas.exists() && !target.exists() {
                let _ = std::fs::copy(&atlas, &target);
            }
        }
        self.status = result.err().unwrap_or_else(|| format!("Saved as {}", path.display()));
        self.revision += 1;
    }
    /// Ask about unsaved work before leaving this document.
    fn request_leave(&mut self, leave: Leave) {
        if self.doc.is_dirty() {
            self.leaving = Some(leave);
        } else {
            self.leave(leave);
        }
    }
    fn leave(&mut self, leave: Leave) {
        self.leaving = None;
        match leave {
            Leave::Quit => self.exit_requested = true,
            Leave::Open(Some(path)) => self.open(&path),
            Leave::Open(None) => self.ask = Some(files::Ask::Open),
        }
    }
    fn open(&mut self, path: &std::path::Path) {
        match LevelDocument::open(path) {
            Ok(doc) => {
                let old = self.doc.path().to_path_buf();
                self.recent.retain(|p| p != &old && p != doc.path());
                self.recent.insert(0, old);
                self.recent.truncate(8);
                self.doc = doc;
                self.selection.clear();
                self.mesh.room = 0;
                self.mesh_clear();
                self.overlay.clear();
                self.navigate(String::new());
                self.frame = true;
                self.overview = true;
                self.changed("Opened level");
            }
            Err(error) => self.status = format!("Could not open {}: {error}", path.display()),
        }
    }
}

fn preview(mut editor: ResMut<Editor>, mut previews: MessageWriter<PreviewLevel>) {
    if !editor.preview {
        return;
    }
    editor.preview = false;
    match editor.doc.level() {
        Ok(level) => {
            previews.write(PreviewLevel { data: Arc::new(level), atlas: editor.atlas() });
        }
        Err(error) => editor.status = error,
    }
}

// Reuse the engine's mesh construction and posing, then dress the preview
// with ordinary Bevy materials. This adapter never changes the game renderer.
// Room shells cull their outward-facing sides so the view and clicks reach
// inside a room from outside it; objects stay double-sided. Room faces are
// wound clockwise as seen from the side they are drawn for, which is Bevy's
// back face, so the culled side is Bevy's front.
#[allow(clippy::type_complexity)]
fn standard_preview(
    mut commands: Commands,
    psx: Res<Assets<PsxMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    meshes: Query<(Entity, &MeshMaterial3d<PsxMaterial>, Option<&Name>)>,
    mut cameras: Query<(Entity, &mut Camera), (With<GameCamera>, Added<GameCamera>)>,
    target: Res<EditorTarget>,
) {
    let mut cache = std::collections::HashMap::new();
    for (entity, material, name) in &meshes {
        let id = material.0.id();
        let Some(material) = psx.get(id) else { continue };
        let room = name.is_some_and(|name| name.starts_with("Room"));
        let handle = cache
            .entry((id, room))
            .or_insert_with(|| {
                standard.add(StandardMaterial {
                    base_color_texture: material.texture.clone(),
                    unlit: true,
                    cull_mode: room.then_some(bevy::render::render_resource::Face::Front),
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                })
            })
            .clone();
        let mut entity = commands.entity(entity);
        entity.remove::<MeshMaterial3d<PsxMaterial>>().insert(MeshMaterial3d(handle));
        if !room {
            entity.insert(bevy::picking::mesh_picking::ray_cast::RayCastBackfaces);
        }
    }
    for (entity, mut camera) in &mut cameras {
        camera.sub_camera_view = None;
        camera.viewport = None;
        camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.10, 0.12, 0.16));
        if target.headless {
            commands.entity(entity).insert(RenderTarget::Image(target.image.clone().into()));
        }
        commands.entity(entity).remove::<PsxView>().remove::<bevy::pbr::DistanceFog>().insert((
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        ));
    }
}

fn status(editor: Res<Editor>, mut status: Query<&mut EditorStatus>) {
    let Ok(mut status) = status.single_mut() else { return };
    if editor.is_changed() {
        status.file = editor.doc.path().display().to_string();
        status.dirty = editor.doc.is_dirty();
        status.pointer = editor.pointer.clone();
        status.message = editor.status.clone();
        status.revision = editor.revision;
        status.rooms = editor.doc.value()["rooms"].as_array().map_or(0, Vec::len);
        status.placements = editor.doc.value()["placements"].as_array().map_or(0, Vec::len);
        status.selection = editor.selection.clone();
        status.asking_to_save = editor.leaving.is_some();
        status.mode = if editor.mesh.on { "mesh" } else { "object" }.into();
        status.mesh_room = editor.mesh.room;
        status.mesh_selection = editor.mesh_selection_text();
    }
}

#[cfg(test)]
fn sandbox() -> Editor {
    let mut editor = Editor::new(
        LevelDocument::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../engine/assets/levels/sandbox/level.json"))
            .unwrap(),
    );
    editor.preview = false;
    editor.frame = false;
    editor.overview = false;
    editor
}
