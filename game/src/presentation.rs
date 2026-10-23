//! Tomb Raider II's front end and HUD: the title, starting and loading
//! games, game over, the pause and the health bar. The rings themselves are
//! [`crate::inventory`]; the level is drawn by the engine
//! (`engine::render`).
//!
//! As the original's front end (`0x8006D534`) and game loop (`0x8001D7FC`,
//! `0x8006DB80`) do:
//! - the title is the inventory ring in its title mode over the title
//!   picture, in the title level (`levels/title`, the ring's models). The
//!   passport starts a new game at the Great Wall or loads; Lara's Home's
//!   photograph starts Lara's Home.
//! - in play, the passport's Save Game saves ([`crate::save`]), Load Game
//!   loads, Exit to Title leaves.
//! - Lara dead for 300 ticks, or 60 and a button pressed, brings the game
//!   over ring (a tick later), whose passport loads, restarts the level or
//!   leaves; at Lara's Home it is straight back to the title.
//! - Start pauses, showing "Paused" (`0x80068F20`); Start again plays on.
//!   Select there asks "Exit to title?", then "Are you sure?".
//!
//! Levels are numbered as the gameflow script numbers them, Lara's Home 0.
//! Evidence: `docs/save-evidence.md`.
use bevy::{camera::visibility::RenderLayers, prelude::*};
pub use engine::render::LevelRequest;
use engine::{
    controller::{Button, ControllerState},
    psx::PsxView,
    render::{
        ActiveLevel, LevelLoaded, LevelSystems, ViewSettings,
        overlay::{OverlayAnchor, overlay_projection},
    },
    schedule::WorldSystems,
    shell::GameSystems,
};

use crate::inventory::{
    GameStrings, Inventory, InventoryRing, InventorySystems, ItemTable, RingClosed, RingMode, item, ring::Passport,
};
use crate::player::*;
use crate::requester::{Answer, Requester};
use crate::save::{LoadChoice, PendingRestore, SaveGame, SavePath};
use crate::text::{Align, OriginalFont, ScreenLines, ScreenText};

/// The level holding the title's ring (`TITLE.PSX`).
pub const TITLE_LEVEL: &str = "title";
/// The title's music.
const TITLE_TRACK: usize = 2;

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct Session {
    /// At the title.
    pub title: bool,
    pub paused: bool,
    /// The level being played, by its number.
    pub level: usize,
    pub pickups: u32,
    pub kills: u32,
    pub secrets: u32,
    pub message: String,
}

/// The front end's own state between ticks.
#[derive(Resource, Default)]
struct FrontEnd {
    /// Ticks since Lara died (`0x8008C502`, counted by LaraControl at
    /// `0x80042B30`).
    death_ticks: u32,
    /// The game over ring is due next tick (`0x80087F9C` = 2).
    game_over: bool,
    /// The pause's question, if one is asked, and which.
    question: Option<(Question, Requester)>,
    /// The pause's held input, for its repeat stop.
    latch: u32,
    /// The controls item's page was opened for this choice of it.
    controls_shown: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Question {
    ExitToTitle,
    AreYouSure,
}

/// The HUD's words before the original font is loaded.
#[derive(Component)]
struct Hud;
/// The HUD's words in the original font.
#[derive(Component)]
struct HudLines;
#[derive(Component)]
struct HealthBar;
/// The health bar's frame, which hides it with the bar.
#[derive(Component)]
struct HealthFrame;
#[derive(Component)]
struct TitleBackdrop;
#[derive(Component)]
struct TitleCamera;

/// The title picture's render layer.
const TITLE_LAYER: usize = 24;

pub struct PresentationPlugin;
impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::player::arms::ArmsPlugin);
        app.init_resource::<Session>()
            .init_resource::<FrontEnd>()
            .init_resource::<SaveGame>()
            .init_resource::<SavePath>()
            .insert_resource(SimulationEnabled(true))
            .add_systems(Startup, setup)
            .add_systems(Update, show_title.run_if(crate::disc::data_ready).run_if(no_title))
            .add_systems(Update, level_loaded.after(LevelSystems::Load))
            .add_systems(
                FixedUpdate,
                crate::save::restore.in_set(engine::schedule::LifecycleSystems::Initialize).in_set(GameSystems::Game),
            )
            .add_systems(
                FixedUpdate,
                (ring_closed, front_end)
                    .chain()
                    .in_set(WorldSystems::Prepare)
                    .after(InventorySystems::Ring)
                    .before(PlayerSystems::Control)
                    .in_set(GameSystems::Game),
            )
            .add_systems(
                FixedUpdate,
                crate::braid::simulate.in_set(WorldSystems::SecondaryMotion).in_set(GameSystems::Game),
            )
            .add_systems(Update, (crate::braid::pose_segments.in_set(LevelSystems::Pose), update_hud));
    }
}

/// The HUD and menus are laid out in the original's 368x240 display pixels
/// (`ViewSettings::size`) and scale with the screen's height; a wider screen
/// adds room at the sides, where edge elements follow through
/// `OverlayAnchor`. At the original's size this is one unit per pixel.
fn setup(mut commands: Commands, view: Res<ViewSettings>, session: Res<Session>, mut request: ResMut<LevelRequest>) {
    if session.title {
        request.name = TITLE_LEVEL.into();
    }
    commands.spawn((
        Camera2d,
        // After the level's and the inventory ring's cameras.
        Camera { order: 4, clear_color: ClearColorConfig::None, ..default() },
        overlay_projection(&view),
        PsxView,
    ));
    // The title picture, behind the title's ring (`0x80032FD4`).
    commands.spawn((
        Camera2d,
        Camera {
            order: crate::inventory::view::BACKDROP_ORDER,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            is_active: false,
            ..default()
        },
        overlay_projection(&view),
        PsxView,
        RenderLayers::layer(TITLE_LAYER),
        TitleCamera,
    ));
    commands.spawn((
        Text2d::new("Loading..."),
        TextFont { font_size: FontSize::Px(9.0), ..default() },
        TextColor(Color::WHITE),
        TextLayout::justify(Justify::Center),
        Transform::from_xyz(0.0, 90.0, 1.0),
        Hud,
    ));
    commands.spawn((ScreenLines::default(), Transform::from_xyz(0.0, 0.0, 1.0), HudLines));
    // Health bar frame 89 units from the left edge and 14 from the top; the
    // bar is its child, so the HUD system only sets the bar's local x.
    commands.spawn((
        Sprite::from_color(Color::srgb(0.05, 0.05, 0.05), Vec2::new(106.0, 8.0)),
        OverlayAnchor::new(Vec2::new(-1.0, 1.0), Vec2::new(89.0, -14.0)),
        HealthFrame,
        children![(
            Sprite::from_color(Color::srgb(0.8, 0.18, 0.12), Vec2::new(100.0, 4.0)),
            Transform::from_xyz(0.0, 0.0, 1.0),
            HealthBar
        )],
    ));
}

fn no_title(title: Query<(), With<TitleBackdrop>>) -> bool {
    title.is_empty()
}

/// The title picture and music, once the game's data is imported.
fn show_title(
    mut commands: Commands,
    server: Res<AssetServer>,
    view: Res<ViewSettings>,
    session: Res<Session>,
    mut music: MessageWriter<crate::audio::TrackRequest>,
) {
    if session.title {
        music.write(crate::audio::TrackRequest(TITLE_TRACK));
    }
    // The picture is 384 by 256; the original shows it unscaled, its rows
    // 8..248 and first 368 columns (matched against `.psx/card_title`).
    // Wider screens pillarbox it in the title camera's black.
    let size = view.size.as_vec2();
    commands.spawn((
        Sprite {
            custom_size: Some(size),
            rect: Some(Rect::new(0.0, 8.0, size.x, 8.0 + size.y)),
            ..Sprite::from_image(server.load("screens/pixus_titleus.png"))
        },
        TitleBackdrop,
        RenderLayers::layer(TITLE_LAYER),
    ));
}

/// A level loaded: closed menus, fresh counters, and its start noted for
/// the load list (with what Lara carries, less what stays in the last
/// level), unless a saved position is about to be put back.
fn level_loaded(
    mut loaded: MessageReader<LevelLoaded>,
    mut session: ResMut<Session>,
    strings: Res<GameStrings>,
    inventory: Res<Inventory>,
    pending: Option<Res<PendingRestore>>,
    mut save: ResMut<SaveGame>,
) {
    for level in loaded.read() {
        if level.name == TITLE_LEVEL {
            continue;
        }
        if !level.data.placements.iter().any(|p| p.kind == LARA) {
            session.message = "This scene has no playable Lara spawn".into();
        }
        session.level = strings.level_number(&level.name).unwrap_or(session.level);
        session.paused = false;
        session.kills = 0;
        session.pickups = 0;
        if pending.is_none() {
            let mut start = inventory.clone();
            start.leave_level();
            save.reach(session.level, &start);
        }
    }
}

/// Starts or restarts a level: loaded again even when it is the one shown.
fn start_level(commands: &mut Commands, request: &mut LevelRequest, strings: &GameStrings, level: usize) -> bool {
    let Some(folder) = strings.level_folder(level) else { return false };
    request.name = folder;
    commands.remove_resource::<ActiveLevel>();
    true
}

/// What the passport chose to load, from the save on disk.
#[allow(clippy::too_many_arguments)]
fn load(
    choice: LoadChoice,
    commands: &mut Commands,
    request: &mut LevelRequest,
    strings: &GameStrings,
    save_path: &SavePath,
    save: &mut SaveGame,
    inventory: &mut Inventory,
    table: &ItemTable,
) -> bool {
    let Some(file) = save_path.load() else { return false };
    let started = match choice {
        LoadChoice::Level(level) => {
            *inventory =
                file.levels.get(&level).map_or_else(|| Inventory::new_game(table), |start| start.inventory.clone());
            start_level(commands, request, strings, level)
        }
        LoadChoice::Position => {
            let Some(position) = file.position.clone() else { return false };
            let level = position.level;
            commands.insert_resource(PendingRestore(position));
            start_level(commands, request, strings, level)
        }
    };
    if started {
        *save = file;
    }
    started
}

/// Acts on what a ring closed on (the callers of Display_Inventory).
#[allow(clippy::too_many_arguments)]
fn ring_closed(
    mut commands: Commands,
    mut closed: MessageReader<RingClosed>,
    mut session: ResMut<Session>,
    mut request: ResMut<LevelRequest>,
    mut inventory: ResMut<Inventory>,
    mut save: ResMut<SaveGame>,
    save_path: Res<SavePath>,
    table: Res<ItemTable>,
    strings: Res<GameStrings>,
    players: Query<&Player>,
    objects: Query<(&crate::interactions::WorldObject, &Transform, Option<&crate::interactions::Enemy>)>,
    mut music: MessageWriter<crate::audio::TrackRequest>,
) {
    for closed in closed.read() {
        let mut to_title = false;
        if let Some(LoadChoice::Level(_) | LoadChoice::Position) = closed.load {
            let choice = closed.load.unwrap();
            if load(choice, &mut commands, &mut request, &strings, &save_path, &mut save, &mut inventory, &table) {
                session.title = false;
            }
            continue;
        }
        match (closed.mode, closed.exit.passport) {
            (RingMode::Title, Some(Passport::NewGame)) => {
                // A new game forgets the levels reached (`0x80051CD4`).
                *save = SaveGame::default();
                *inventory = Inventory::new_game(&table);
                session.title = !start_level(&mut commands, &mut request, &strings, 1);
            }
            (RingMode::Title, _) if closed.exit.chosen == Some(item::LARAS_HOME) => {
                *inventory = Inventory::start_at(&table, "assault");
                session.title = !start_level(&mut commands, &mut request, &strings, 0);
            }
            (RingMode::Game, Some(Passport::Save)) => {
                if let Ok(player) = players.single() {
                    save.position = Some(SaveGame::capture(session.level, player, &inventory, objects.iter()));
                    if let Some(path) = save_path.0.as_deref()
                        && let Err(e) = save.write(path)
                    {
                        warn!("saving to {}: {e}", path.display());
                    }
                }
            }
            (RingMode::Death, Some(Passport::RestartLevel)) => {
                if let Some(start) = save.levels.get(&session.level) {
                    *inventory = start.inventory.clone();
                }
                start_level(&mut commands, &mut request, &strings, session.level);
            }
            (RingMode::Game | RingMode::Death, Some(Passport::ExitToTitle)) => to_title = true,
            _ => {}
        }
        if to_title {
            session.title = true;
            session.paused = false;
            request.name = TITLE_LEVEL.into();
            music.write(crate::audio::TrackRequest(TITLE_TRACK));
        }
    }
}

/// The title's ring, game over, and the pause, a tick at a time.
#[allow(clippy::too_many_arguments)]
fn front_end(
    pad: Res<ControllerState>,
    mut session: ResMut<Session>,
    mut front: ResMut<FrontEnd>,
    mut enabled: ResMut<SimulationEnabled>,
    mut request: ResMut<LevelRequest>,
    mut ring: ResMut<InventoryRing>,
    active: Option<Res<ActiveLevel>>,
    font: Option<Res<OriginalFont>>,
    players: Query<&Player>,
    mut controls: Option<ResMut<engine::controls::ControlsMenu>>,
    mut music: MessageWriter<crate::audio::TrackRequest>,
) {
    let menu_open = controls.as_ref().is_some_and(|m| m.is_open());
    // The options ring's controls stand in for the original's controls
    // page: the template's controls menu, while it is chosen.
    let controls_chosen = ring.ring.as_ref().is_some_and(|r| {
        use crate::inventory::ring::Status;
        r.motion.status == Status::Selected && r.current_item() == Some(item::CONTROLS)
    });
    if controls_chosen && !front.controls_shown {
        if let Some(menu) = controls.as_mut() {
            menu.open();
        }
        front.controls_shown = true;
    } else if !controls_chosen {
        front.controls_shown = false;
    }
    if session.title {
        front.death_ticks = 0;
        front.game_over = false;
        front.question = None;
        enabled.0 = false;
        let ready = active.is_some_and(|a| a.name == TITLE_LEVEL) && font.is_some_and(|f| f.level == TITLE_LEVEL);
        if request.name != TITLE_LEVEL {
            request.name = TITLE_LEVEL.into();
        }
        if ready && !ring.is_open() && ring.request.is_none() {
            ring.request = Some(RingMode::Title);
        }
        return;
    }
    if ring.is_open() {
        return;
    }
    let word = crate::inventory::input_word(&pad) | if pad.pressed(Button::Start) { 0x2000 } else { 0 };
    // Game over (`0x8001DA00`): due when Lara has been dead 300 ticks, or
    // 60 with a button pressed, and opened a tick later.
    let dead = players.single().is_ok_and(|p| p.health <= 0);
    if session.paused || !dead {
        front.death_ticks = if dead { front.death_ticks } else { 0 };
    } else {
        front.death_ticks += 1;
    }
    if front.death_ticks > 300 || (front.death_ticks > 60 && word != 0) || front.game_over {
        if session.level == 0 {
            // Lara's Home has no game over: back to the title.
            session.title = true;
            request.name = TITLE_LEVEL.into();
            music.write(crate::audio::TrackRequest(TITLE_TRACK));
            return;
        }
        if front.game_over {
            front.game_over = false;
            front.death_ticks = 0;
            ring.request = Some(RingMode::Death);
            return;
        }
        front.game_over = true;
    }
    // The pause (`0x80068F20`): its input word with the repeat stop.
    let db = if word == 0 {
        front.latch = 0;
        0
    } else if front.latch != 0 {
        0
    } else {
        front.latch = word;
        word
    };
    if session.paused {
        if let Some((question, requester)) = front.question.as_mut() {
            match (requester.input(db), *question) {
                (Some(Answer::Chosen(1)), Question::ExitToTitle) => {
                    front.question =
                        Some((Question::AreYouSure, Requester::question("Are you sure?", ["Yes", "No"], 1)));
                }
                (Some(Answer::Chosen(0)), Question::AreYouSure) => {
                    front.question = None;
                    session.paused = false;
                    session.title = true;
                    request.name = TITLE_LEVEL.into();
                    music.write(crate::audio::TrackRequest(TITLE_TRACK));
                    return;
                }
                (Some(_), _) => {
                    front.question = None;
                    session.paused = false;
                }
                _ => {}
            }
        } else if db & 0x2000 != 0 {
            session.paused = false;
        } else if db & crate::inventory::ring::input::OPTION != 0 {
            front.question =
                Some((Question::ExitToTitle, Requester::question("Exit to title?", ["Continue", "Quit"], 0)));
        }
    } else if db & 0x2000 != 0 && !dead {
        session.paused = true;
    }
    enabled.0 = !session.paused && !menu_open;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_hud(
    session: Res<Session>,
    front: Res<FrontEnd>,
    ring: Res<InventoryRing>,
    strings: Res<GameStrings>,
    players: Query<(&Player, &Motion)>,
    font: Option<Res<OriginalFont>>,
    mut text: Query<&mut Text2d, (With<Hud>, Without<HealthBar>)>,
    mut block: Query<&mut ScreenLines, With<HudLines>>,
    mut health: Query<(&mut Sprite, &mut Transform, &mut Visibility), (With<HealthBar>, Without<Hud>)>,
    mut title: Query<&mut Visibility, (With<TitleBackdrop>, Without<HealthBar>, Without<HealthFrame>)>,
    mut title_camera: Query<&mut Camera, With<TitleCamera>>,
    mut frame: Query<&mut Visibility, (With<HealthFrame>, Without<HealthBar>, Without<TitleBackdrop>)>,
) {
    // The ring hides the HUD; a medi pack in front shows Lara's health
    // (0x80033974).
    let medipack = ring.ring.as_ref().is_some_and(|r| {
        use crate::inventory::ring::Status;
        let resting = matches!(r.motion.status, Status::Open | Status::Selecting | Status::Selected);
        resting && !r.rotating && matches!(r.current_item(), Some(item::SMALL_MEDIPACK | item::LARGE_MEDIPACK))
    });
    for mut vis in &mut frame {
        *vis = if session.title || (ring.is_open() && !medipack) { Visibility::Hidden } else { Visibility::Inherited };
    }
    for mut vis in &mut title {
        *vis = if session.title { Visibility::Visible } else { Visibility::Hidden };
    }
    for mut camera in &mut title_camera {
        camera.is_active = session.title;
    }
    let mut lines = vec![];
    if session.paused && !ring.is_open() {
        // "Paused" (PlayStation string 22), 24 above the bottom.
        lines.push(ScreenText::new(strings.psx(22), IVec2::new(0, -24), Align::BOTTOM));
        if let Some((_, requester)) = &front.question {
            lines.extend(requester.lines());
        }
    }
    if let Ok(mut label) = text.single_mut() {
        // Until a level's font is loaded the template's text stands in.
        let waiting = if font.is_none() { "Loading..." } else { "" };
        if label.0 != waiting {
            label.0 = waiting.into();
        }
    }
    if let Ok(mut block) = block.single_mut() {
        block.set_if_neq(ScreenLines(lines));
    }
    if let Ok((p, _)) = players.single()
        && let Ok((mut sprite, mut t, mut vis)) = health.single_mut()
    {
        let width = (p.health as f32 / 10.0).clamp(0.0, 100.0);
        sprite.custom_size = Some(Vec2::new(width, 4.0));
        t.translation.x = -50.0 + width / 2.0;
        *vis = Visibility::Inherited;
    }
}
