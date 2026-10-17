//! A full-resolution authoring app over engine formats, with no game dependency.
mod ui;
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
use std::sync::Arc;

#[derive(Resource)]
struct Editor {
    doc: LevelDocument,
    pointer: String,
    selected: usize,
    page: usize,
    status: String,
    input: Option<ui::Input>,
    revision: u64,
    preview: bool,
    frame: bool,
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
                    min_width: 1000.0,
                    min_height: 800.0,
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
        .insert_resource(engine::render::ViewSettings { far: 10000.0, ..default() })
        .add_plugins(engine::render::RenderPlugin)
        .insert_resource(Editor {
            doc,
            pointer: String::new(),
            selected: 0,
            page: 0,
            status: "Ready".into(),
            input: None,
            revision: 0,
            preview: true,
            frame: true,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (ui::input, automation, preview).chain().before(LevelSystems::Load))
        .add_systems(Update, (standard_preview, ui::camera, status).chain().after(LevelSystems::Visibility))
        .add_plugins(ui::UiPlugin);
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
    commands.spawn((Name::new("Editor"), EditorCommand::default(), EditorStatus::default()));
}

fn automation(mut editor: ResMut<Editor>, mut command: Query<&mut EditorCommand>) {
    let Ok(mut c) = command.single_mut() else { return };
    if c.navigate {
        c.navigate = false;
        if editor.doc.value().pointer(&c.pointer).is_some() {
            editor.pointer = c.pointer.clone();
            editor.page = 0;
            editor.selected = 0;
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
        let result = editor.doc.save();
        editor.result(result, "Saved");
    }
    if c.frame {
        c.frame = false;
        editor.frame = true;
    }
}

impl Editor {
    fn result(&mut self, result: Result<(), String>, success: &str) {
        match result {
            Ok(()) => {
                self.status = success.into();
                self.revision += 1;
                self.preview = true;
            }
            Err(error) => self.status = error,
        }
    }
    fn undo(&mut self) {
        if self.doc.undo() {
            self.changed("Undone");
        }
    }
    fn redo(&mut self) {
        if self.doc.redo() {
            self.changed("Redone");
        }
    }
    fn changed(&mut self, text: &str) {
        self.status = text.into();
        self.revision += 1;
        self.preview = true;
    }
}

fn preview(mut editor: ResMut<Editor>, mut previews: MessageWriter<PreviewLevel>) {
    if !editor.preview {
        return;
    }
    editor.preview = false;
    match editor.doc.level() {
        Ok(level) => {
            let atlas = editor.doc.path().parent().unwrap().join("atlas.png");
            previews.write(PreviewLevel {
                data: Arc::new(level),
                atlas: atlas.to_string_lossy().trim_start_matches('/').into(),
            });
        }
        Err(error) => editor.status = error,
    }
}

// Reuse the engine's mesh construction and posing, then dress the preview
// with ordinary Bevy materials. This adapter never changes the game renderer.
#[allow(clippy::type_complexity)]
fn standard_preview(
    mut commands: Commands,
    psx: Res<Assets<PsxMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    meshes: Query<(Entity, &MeshMaterial3d<PsxMaterial>)>,
    mut cameras: Query<(Entity, &mut Camera), (With<GameCamera>, Added<GameCamera>)>,
    target: Res<EditorTarget>,
) {
    let mut cache = std::collections::HashMap::new();
    for (entity, material) in &meshes {
        let id = material.0.id();
        let Some(material) = psx.get(id) else { continue };
        let handle = cache
            .entry(id)
            .or_insert_with(|| {
                standard.add(StandardMaterial {
                    base_color_texture: material.texture.clone(),
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                })
            })
            .clone();
        commands.entity(entity).remove::<MeshMaterial3d<PsxMaterial>>().insert(MeshMaterial3d(handle));
    }
    for (entity, mut camera) in &mut cameras {
        camera.sub_camera_view = None;
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
    }
}
