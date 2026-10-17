//! Tomb Raider II's menus and HUD: title screen, pause and inventory
//! menus, health bar, and the level list. The level itself is drawn by the
//! engine (`engine::render`).
use bevy::prelude::*;
use engine::{
    controller::{Button, ControllerState},
    psx::{PsxScreen, PsxView},
    render::{ActiveLevel, LevelLoaded, LevelSystems},
    schedule::WorldSystems,
    shell::GameSystems,
};
pub use engine::render::LevelRequest;

use crate::player::*;

pub const LEVELS: &[(&str,&str)] = &[
    ("wall","The Great Wall"),("venice","Venice"),("boat","Bartoli's Hideout"),
    ("opera","Opera House"),("rig","Offshore Rig"),("platform","Diving Area"),
    ("unwater","40 Fathoms"),("keel","Wreck of the Maria Doria"),("living","Living Quarters"),
    ("deck","The Deck"),("skidoo","Tibetan Foothills"),("monastry","Barkhang Monastery"),
    ("catacomb","Catacombs of the Talion"),("icecave","Ice Palace"),("emprtomb","Temple of Xian"),
    ("floating","Floating Islands"),("xian","The Dragon's Lair"),("house","Home Sweet Home"),
    ("assault","Lara's Home"),
];

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct Session {
    pub title: bool,
    pub paused: bool,
    pub inventory: bool,
    pub level_index: usize,
    pub menu_selection: usize,
    pub medipacks: u32,
    pub pickups: u32,
    pub kills: u32,
    pub secrets: u32,
    pub message: String,
}
#[derive(Component)]
struct Hud;
#[derive(Component)]
struct HealthBar;
#[derive(Component)]
struct TitleBackdrop;

pub struct PresentationPlugin;
impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Session>()
            .insert_resource(SimulationEnabled(true))
            .add_systems(Startup, setup)
            .add_systems(Update, (level_loaded, crate::save::restore).chain().after(LevelSystems::Load))
            .add_systems(FixedUpdate, menus.in_set(WorldSystems::Prepare).before(PlayerSystems::Control).in_set(GameSystems::Game))
            .add_systems(FixedUpdate, crate::braid::simulate.in_set(WorldSystems::Effects).in_set(GameSystems::Game))
            .add_systems(Update, (crate::braid::pose_segments.in_set(LevelSystems::Pose), update_hud));
    }
}

fn setup(mut commands: Commands, server: Res<AssetServer>, screen: Res<PsxScreen>, session: Res<Session>, mut music: MessageWriter<crate::audio::TrackRequest>) {
    if session.title {
        music.write(crate::audio::TrackRequest(2));
    }
    commands.spawn((Camera2d, Camera { order: 1, clear_color: ClearColorConfig::None, ..default() }, PsxView));
    commands.spawn((
        Sprite { custom_size: Some(Vec2::new(screen.width as f32, screen.height as f32)), ..Sprite::from_image(server.load("screens/pixus_titleus.png")) },
        Transform::from_xyz(0.0, 0.0, 0.5),
        TitleBackdrop,
    ));
    commands.spawn((
        Text2d::new("Loading..."),
        TextFont { font_size: FontSize::Px(9.0), ..default() },
        TextColor(Color::WHITE),
        TextLayout::justify(Justify::Center),
        Transform::from_xyz(0.0, 90.0, 1.0),
        Hud,
    ));
    commands.spawn((Sprite::from_color(Color::srgb(0.05, 0.05, 0.05), Vec2::new(106.0, 8.0)), Transform::from_xyz(-95.0, 106.0, 0.0)));
    commands.spawn((Sprite::from_color(Color::srgb(0.8, 0.18, 0.12), Vec2::new(100.0, 4.0)), Transform::from_xyz(-95.0, 106.0, 1.0), HealthBar));
}

/// A new level starts with closed menus and fresh counters.
fn level_loaded(mut loaded: MessageReader<LevelLoaded>, mut session: ResMut<Session>) {
    for level in loaded.read() {
        if !level.data.placements.iter().any(|p| p.kind == LARA) {
            session.message = "This scene has no playable Lara spawn".into();
        }
        session.level_index = LEVELS.iter().position(|(name, _)| *name == level.name).unwrap_or(0);
        session.paused = false;
        session.inventory = false;
        session.medipacks = 2;
        session.kills = 0;
        session.pickups = 0;
    }
}

fn menus(pad:Res<ControllerState>,mut session:ResMut<Session>,mut enabled:ResMut<SimulationEnabled>,mut request:ResMut<LevelRequest>,
    mut players:Query<&mut Player>,mut controls:Option<ResMut<engine::controls::ControlsMenu>>,mut inventory:ResMut<crate::interactions::Inventory>,
    mut commands:Commands,objects:Query<(&crate::interactions::WorldObject,&Transform,Option<&crate::interactions::Enemy>)>) {
    if session.title {
        if pad.pressed(Button::Down) {session.menu_selection=(session.menu_selection+1)%3;}
        if pad.pressed(Button::Up) {session.menu_selection=(session.menu_selection+2)%3;}
        if pad.pressed(Button::Cross)||pad.pressed(Button::Start) {match session.menu_selection {
            0=>{session.title=false;request.name="wall".into();},
            1=>{session.title=false;request.name="assault".into();},
            2=>{if let Some(menu)=controls.as_mut(){menu.open();}},_=>{}
        }}
        enabled.0=!session.title;
        return;
    }
    if pad.pressed(Button::Start) {session.paused=!session.paused;session.inventory=false;session.menu_selection=0;}
    if pad.pressed(Button::Select) {session.inventory=!session.inventory;session.paused=false;session.menu_selection=0;}
    if session.paused || session.inventory {
        if pad.pressed(Button::Triangle) {session.paused=false;session.inventory=false;}
        if pad.pressed(Button::Right) {session.level_index=(session.level_index+1)%LEVELS.len();}
        if pad.pressed(Button::Left) {session.level_index=(session.level_index+LEVELS.len()-1)%LEVELS.len();}
        if pad.pressed(Button::Down) {session.menu_selection=(session.menu_selection+1)%4;}
        if pad.pressed(Button::Up) {session.menu_selection=(session.menu_selection+3)%4;}
        if pad.pressed(Button::Cross) {
            match session.menu_selection {
                0=>{session.paused=false;session.inventory=false;}
                1=>{if session.inventory {if let Ok(mut p)=players.single_mut(){if !inventory.use_medipack(false,&mut p){inventory.use_medipack(true,&mut p);}}}
                    else {request.name=LEVELS[session.level_index].0.into();}}
                2=>{if let Ok(p)=players.single(){session.message=match crate::save::write(&request.name,p,&inventory,&objects){Ok(())=>"Game saved".into(),Err(e)=>format!("Save failed: {e}")};}}
                3=>{match crate::save::read(){Ok(save)=>{request.name=save.level.clone();commands.insert_resource(crate::save::PendingRestore(save));},Err(e)=>{session.message=format!("Load failed: {e}");}}}
                _=>{}
            }
        }
    }
    enabled.0=!session.paused&&!session.inventory&&!controls.as_ref().is_some_and(|m|m.is_open());
}

fn update_hud(session:Res<Session>,active:Option<Res<ActiveLevel>>,players:Query<(&Player,&Motion)>,
    mut text:Query<(&mut Text2d,&mut Transform),(With<Hud>,Without<HealthBar>)>,
    mut health:Query<(&mut Sprite,&mut Transform,&mut Visibility),(With<HealthBar>,Without<Hud>)>,
    mut title:Query<&mut Visibility,(With<TitleBackdrop>,Without<HealthBar>)>) {
    let Ok((mut label,mut label_transform))=text.single_mut()else{return;};
    label_transform.translation.y=if session.title {-65.0}else{0.0};
    for mut vis in &mut title {*vis=if session.title {Visibility::Visible}else{Visibility::Hidden};}
    if session.title {
        label.0=["New Game","Lara's Home","Controls"].iter().enumerate().map(|(i,s)|format!("{} {s}",if session.menu_selection==i {">"}else{" "})).collect::<Vec<_>>().join("\n\n");
    } else if session.paused || session.inventory {
        let items=if session.inventory {["Resume".to_owned(),format!("Small medipack ({})",session.medipacks),"Save game".into(),"Load game".into()]}
            else {["Resume".to_owned(),format!("Level: {}",LEVELS[session.level_index].1),"Save game".into(),"Load game".into()]};
        let menu=items.iter().enumerate().map(|(i,s)|format!("{} {}",if session.menu_selection==i {">"}else{" "},s)).collect::<Vec<_>>().join("\n\n");
        label.0=format!("{}\n\n{menu}\n\n{}",if session.inventory {"INVENTORY"}else{"PAUSED"},session.message);
    } else if let Some(active)=active {
        let title=LEVELS.iter().find(|(n,_)|*n==active.name).map(|(_,t)|*t).unwrap_or(&active.name);
        label.0=if players.single().is_ok_and(|(p,_)|p.health==0) {format!("{title}\n\nLara has died\nSTART for menu")}else{String::new()};
    }
    if let Ok((p,_))=players.single(){if let Ok((mut sprite,mut t,mut vis))=health.single_mut(){let width=(p.health as f32/10.0).clamp(0.0,100.0);sprite.custom_size=Some(Vec2::new(width,4.0));t.translation.x=-145.0+width/2.0;*vis=if session.title {Visibility::Hidden}else{Visibility::Inherited};}}
}
