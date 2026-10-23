//! Tomb Raider II's menus and HUD: title screen, pause menu, health bar,
//! and the level list. The inventory ring is [`crate::inventory`]. The level itself is drawn by the
//! engine (`engine::render`).
use bevy::prelude::*;
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

use crate::player::*;
use crate::text::{Align, OriginalFont, ScreenLines, ScreenText, symbol};

pub const LEVELS: &[(&str, &str)] = &[
    ("wall", "The Great Wall"),
    ("venice", "Venice"),
    ("boat", "Bartoli's Hideout"),
    ("opera", "Opera House"),
    ("rig", "Offshore Rig"),
    ("platform", "Diving Area"),
    ("unwater", "40 Fathoms"),
    ("keel", "Wreck of the Maria Doria"),
    ("living", "Living Quarters"),
    ("deck", "The Deck"),
    ("skidoo", "Tibetan Foothills"),
    ("monastry", "Barkhang Monastery"),
    ("catacomb", "Catacombs of the Talion"),
    ("icecave", "Ice Palace"),
    ("emprtomb", "Temple of Xian"),
    ("floating", "Floating Islands"),
    ("xian", "The Dragon's Lair"),
    ("house", "Home Sweet Home"),
    ("assault", "Lara's Home"),
];

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct Session {
    pub title: bool,
    pub paused: bool,
    pub level_index: usize,
    pub menu_selection: usize,
    pub pickups: u32,
    pub kills: u32,
    pub secrets: u32,
    pub message: String,
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

pub struct PresentationPlugin;
impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::player::arms::ArmsPlugin);
        app.init_resource::<Session>()
            .insert_resource(SimulationEnabled(true))
            .add_systems(Startup, setup)
            .add_systems(Update, show_title.run_if(crate::disc::data_ready).run_if(no_title))
            .add_systems(Update, (level_loaded, crate::save::restore).chain().after(LevelSystems::Load))
            .add_systems(
                FixedUpdate,
                menus
                    .in_set(WorldSystems::Prepare)
                    .after(crate::inventory::InventorySystems::Ring)
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
fn setup(mut commands: Commands, view: Res<ViewSettings>) {
    commands.spawn((
        Camera2d,
        // After the level's and the inventory ring's cameras.
        Camera { order: 4, clear_color: ClearColorConfig::None, ..default() },
        overlay_projection(&view),
        PsxView,
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
        music.write(crate::audio::TrackRequest(2));
    }
    // The title fills the original's display; wider screens pillarbox it in black.
    commands.spawn((
        Sprite {
            custom_size: Some(view.size.as_vec2()),
            ..Sprite::from_image(server.load("screens/pixus_titleus.png"))
        },
        Transform::from_xyz(0.0, 0.0, 0.5),
        TitleBackdrop,
        children![(Sprite::from_color(Color::BLACK, Vec2::splat(8192.0)), Transform::from_xyz(0.0, 0.0, -0.25))],
    ));
}

/// A new level starts with closed menus and fresh counters.
fn level_loaded(mut loaded: MessageReader<LevelLoaded>, mut session: ResMut<Session>) {
    for level in loaded.read() {
        if !level.data.placements.iter().any(|p| p.kind == LARA) {
            session.message = "This scene has no playable Lara spawn".into();
        }
        session.level_index = LEVELS.iter().position(|(name, _)| *name == level.name).unwrap_or(0);
        session.paused = false;
        session.kills = 0;
        session.pickups = 0;
    }
}

#[allow(clippy::too_many_arguments)]
fn menus(
    pad: Res<ControllerState>,
    mut session: ResMut<Session>,
    mut enabled: ResMut<SimulationEnabled>,
    mut request: ResMut<LevelRequest>,
    players: Query<&mut Player>,
    mut controls: Option<ResMut<engine::controls::ControlsMenu>>,
    mut inventory: ResMut<crate::inventory::Inventory>,
    table: Res<crate::inventory::ItemTable>,
    ring: Res<crate::inventory::InventoryRing>,
    mut commands: Commands,
    objects: Query<(&crate::interactions::WorldObject, &Transform, Option<&crate::interactions::Enemy>)>,
) {
    if session.title {
        if pad.pressed(Button::Down) {
            session.menu_selection = (session.menu_selection + 1) % 3;
        }
        if pad.pressed(Button::Up) {
            session.menu_selection = (session.menu_selection + 2) % 3;
        }
        if pad.pressed(Button::Cross) || pad.pressed(Button::Start) {
            match session.menu_selection {
                0 | 1 => {
                    session.title = false;
                    request.name = if session.menu_selection == 0 { "wall" } else { "assault" }.into();
                    *inventory = crate::inventory::Inventory::start_at(&table, &request.name);
                }
                2 => {
                    if let Some(menu) = controls.as_mut() {
                        menu.open();
                    }
                }
                _ => {}
            }
        }
        enabled.0 = !session.title;
        return;
    }
    if ring.is_open() {
        enabled.0 = false;
        return;
    }
    if pad.pressed(Button::Start) {
        session.paused = !session.paused;
        session.menu_selection = 0;
    }
    if session.paused {
        if pad.pressed(Button::Triangle) {
            session.paused = false;
        }
        if pad.pressed(Button::Right) {
            session.level_index = (session.level_index + 1) % LEVELS.len();
        }
        if pad.pressed(Button::Left) {
            session.level_index = (session.level_index + LEVELS.len() - 1) % LEVELS.len();
        }
        if pad.pressed(Button::Down) {
            session.menu_selection = (session.menu_selection + 1) % 4;
        }
        if pad.pressed(Button::Up) {
            session.menu_selection = (session.menu_selection + 3) % 4;
        }
        if pad.pressed(Button::Cross) {
            match session.menu_selection {
                0 => {
                    session.paused = false;
                }
                1 => {
                    request.name = LEVELS[session.level_index].0.into();
                }
                2 => {
                    if let Ok(p) = players.single() {
                        session.message = match crate::save::write(&request.name, p, &inventory, &objects) {
                            Ok(()) => "Game saved".into(),
                            Err(e) => format!("Save failed: {e}"),
                        };
                    }
                }
                3 => match crate::save::read() {
                    Ok(save) => {
                        request.name = save.level.clone();
                        commands.insert_resource(crate::save::PendingRestore(save));
                    }
                    Err(e) => {
                        session.message = format!("Load failed: {e}");
                    }
                },
                _ => {}
            }
        }
    }
    enabled.0 = !session.paused && !controls.as_ref().is_some_and(|m| m.is_open());
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_hud(
    session: Res<Session>,
    ring: Res<crate::inventory::InventoryRing>,
    active: Option<Res<ActiveLevel>>,
    players: Query<(&Player, &Motion)>,
    font: Option<Res<OriginalFont>>,
    mut text: Query<(&mut Text2d, &mut Transform), (With<Hud>, Without<HealthBar>)>,
    mut block: Query<&mut ScreenLines, With<HudLines>>,
    mut health: Query<(&mut Sprite, &mut Transform, &mut Visibility), (With<HealthBar>, Without<Hud>)>,
    mut title: Query<&mut Visibility, (With<TitleBackdrop>, Without<HealthBar>, Without<HealthFrame>)>,
    mut frame: Query<&mut Visibility, (With<HealthFrame>, Without<HealthBar>, Without<TitleBackdrop>)>,
) {
    // The ring hides the HUD; a medi pack in front shows Lara's health
    // (0x80033974).
    let medipack = ring.ring.as_ref().is_some_and(|r| {
        use crate::inventory::{item, ring::Status};
        let resting = matches!(r.motion.status, Status::Open | Status::Selecting | Status::Selected);
        resting && !r.rotating && matches!(r.current_item(), Some(item::SMALL_MEDIPACK | item::LARGE_MEDIPACK))
    });
    for mut vis in &mut frame {
        *vis = if session.title || (ring.is_open() && !medipack) { Visibility::Hidden } else { Visibility::Inherited };
    }
    let Ok((mut label, mut label_transform)) = text.single_mut() else {
        return;
    };
    for mut vis in &mut title {
        *vis = if session.title { Visibility::Visible } else { Visibility::Hidden };
    }
    // Lines centred across the screen, `y` pixels below its middle; the
    // selection is marked with the font's grey right arrow.
    let line = |y: i32, text: String| ScreenText::new(text, IVec2::new(0, y), Align::CENTRE);
    let selected = session.menu_selection;
    let menu = |items: &[String], top: i32| -> Vec<ScreenText> {
        let marker = |i| if i == selected { symbol::RIGHT } else { ' ' };
        items.iter().enumerate().map(|(i, s)| line(top + 20 * i as i32, format!("{} {s}", marker(i)))).collect()
    };
    let mut lines = vec![];
    if session.title {
        lines.extend(menu(&["New Game".into(), "Lara's Home".into(), "Controls".into()], 50));
    } else if ring.is_open() {
        // The ring writes its own text.
    } else if session.paused {
        let items = [
            "Resume".to_owned(),
            format!("Level: {}", LEVELS[session.level_index].1),
            "Save game".into(),
            "Load game".into(),
        ];
        lines.push(line(-60, "PAUSED".into()));
        lines.extend(menu(&items, -26));
        lines.push(line(68, session.message.clone()));
    } else if let Some(active) = active {
        let title = LEVELS.iter().find(|(n, _)| *n == active.name).map(|(_, t)| *t).unwrap_or(&active.name);
        if players.single().is_ok_and(|(p, _)| p.health <= 0) {
            lines.push(line(-20, title.to_owned()));
            lines.push(line(10, "Lara has died".into()));
            lines.push(line(26, "START for menu".into()));
        }
    }
    // Until a level's font is loaded the template's text stands in.
    if font.is_some() {
        label.0.clear();
        if let Ok(mut block) = block.single_mut() {
            block.set_if_neq(ScreenLines(lines));
        }
    } else if session.title || session.paused || !lines.is_empty() {
        let fallback = lines.iter().map(|l| l.text.replace(symbol::RIGHT, ">")).collect::<Vec<_>>().join("\n\n");
        label.0 = fallback;
        label_transform.translation.y = if session.title { -65.0 } else { 0.0 };
    }
    if let Ok((p, _)) = players.single() {
        if let Ok((mut sprite, mut t, mut vis)) = health.single_mut() {
            let width = (p.health as f32 / 10.0).clamp(0.0, 100.0);
            sprite.custom_size = Some(Vec2::new(width, 4.0));
            t.translation.x = -50.0 + width / 2.0;
            *vis = Visibility::Inherited;
        }
    }
}
