//! Free camera: the player orbits the chase camera with the mouse or the
//! right analogue stick. Not in the original, so every option is off by
//! default and the chase camera then runs exactly as ported.
//!
//! The orbit is an offset ([`CameraOrbit`], binary angle units) that the chase
//! camera adds to the avatar's facing and requested elevation, so walls, the
//! smart shift and the floor/ceiling clamp treat it like any chase view.
//! Fixed shots, the look camera and the aiming camera ignore it, and it eases
//! back to zero while they run. See docs/free-camera.md.
use std::path::PathBuf;

use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use serde::{Deserialize, Serialize};

use super::{CameraMode, CameraState, CameraSystems};
use crate::{
    controller::ControllerState,
    controls::{ControlOption, ControlsOptions},
    schedule::{Avatar, WorldSystems, simulation_enabled},
    settings,
    shell::{GameSystems, InputFocus},
    world::WorldGeometry,
};

/// The player's choices, saved in `camera.json`. All off by default.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[reflect(Resource)]
#[serde(default, deny_unknown_fields)]
pub struct FreeCameraSettings {
    /// Mouse movement orbits the camera (and the cursor is hidden in play).
    pub mouse: bool,
    /// The right stick orbits the camera.
    pub stick: bool,
    /// Moving the mouse or stick up looks down.
    pub invert_y: bool,
    /// 1..=10; scales [`FreeCameraTuning::mouse_units`].
    pub mouse_speed: u8,
    /// 1..=10; scales [`FreeCameraTuning::stick_units`].
    pub stick_speed: u8,
    /// Ease the camera back behind the avatar once it moves and the player
    /// has left the camera alone for [`FreeCameraTuning::recenter_delay`].
    pub recenter: bool,
}

impl Default for FreeCameraSettings {
    fn default() -> Self {
        Self { mouse: false, stick: false, invert_y: false, mouse_speed: 5, stick_speed: 5, recenter: true }
    }
}

impl FreeCameraSettings {
    pub const MAX_SPEED: u8 = 10;
}

/// Port choices for how the free camera feels. A mod can replace them.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct FreeCameraTuning {
    /// Binary angle units (65536 per turn) per mouse count, per speed step:
    /// speed 5 turns about 0.055 degrees per count.
    pub mouse_units: f32,
    /// Units per tick at full stick, per speed step: speed 5 is about 130
    /// degrees a second at 30 ticks a second.
    pub stick_units: f32,
    /// Limit of the orbit's elevation either way (12288 = 67.5 degrees). The
    /// chase camera's own limit (15470) still applies to the total.
    pub max_pitch: i32,
    /// Ticks without camera input before the orbit eases back.
    pub recenter_delay: u32,
    /// The orbit closes 1/divisor of its offset per easing tick.
    pub recenter_divisor: i32,
    /// Eye approach divisor while the player steers (the original's is 10),
    /// so the view keeps up with the mouse. A state's own request wins.
    pub steer_divisor: i32,
}

impl Default for FreeCameraTuning {
    fn default() -> Self {
        Self {
            mouse_units: 2.0,
            stick_units: 160.0,
            max_pitch: 12288,
            recenter_delay: 45,
            recenter_divisor: 8,
            steer_divisor: 3,
        }
    }
}

/// The player's orbit of the chase camera this tick, in binary angle units
/// (65536 per turn). Yaw adds to the facing (positive turns right, as the
/// Right button does); positive pitch lowers the eye, looking up.
#[derive(Resource, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Resource)]
pub struct CameraOrbit {
    pub yaw: i32,
    pub pitch: i32,
    /// Eye approach divisor for this tick's chase view; `None` is the
    /// original's.
    pub divisor: Option<i32>,
    /// Ticks since the player last steered the camera.
    pub idle: u32,
    /// The avatar's pose last tick, to tell when it moves.
    #[reflect(ignore)]
    last: Option<(Vec3, f32)>,
}

impl CameraOrbit {
    pub fn is_centred(&self) -> bool {
        self.yaw == 0 && self.pitch == 0
    }
}

/// This tick's camera input from the enabled devices, in units: x turns,
/// y raises the view (positive looks up).
pub fn steering(pad: &ControllerState, settings: &FreeCameraSettings, tuning: &FreeCameraTuning) -> Vec2 {
    let speed = |s: u8| f32::from(s.clamp(1, FreeCameraSettings::MAX_SPEED));
    let mut input = Vec2::ZERO;
    if settings.mouse {
        // Mouse motion is y down: moving it away from the player looks up.
        let motion = pad.mouse.motion;
        input += Vec2::new(motion.x, -motion.y) * tuning.mouse_units * speed(settings.mouse_speed);
    }
    if settings.stick {
        input += pad.right_stick * tuning.stick_units * speed(settings.stick_speed);
    }
    if settings.invert_y {
        input.y = -input.y;
    }
    input
}

/// One tick of the orbit: steer with `input` while the chase camera runs;
/// otherwise, or once the avatar `moved` after the player let go, ease back.
pub fn step_orbit(
    orbit: &mut CameraOrbit,
    input: Vec2,
    mode: CameraMode,
    moved: bool,
    settings: &FreeCameraSettings,
    tuning: &FreeCameraTuning,
) {
    let chase = mode == CameraMode::Chase;
    let turn = input.round().as_ivec2();
    if chase && turn != IVec2::ZERO {
        orbit.yaw = (orbit.yaw + turn.x + 32768).rem_euclid(65536) - 32768;
        orbit.pitch = (orbit.pitch + turn.y).clamp(-tuning.max_pitch, tuning.max_pitch);
        orbit.idle = 0;
        orbit.divisor = Some(tuning.steer_divisor);
        return;
    }
    orbit.idle = orbit.idle.saturating_add(1);
    orbit.divisor = None;
    if !chase || (settings.recenter && moved && orbit.idle > tuning.recenter_delay) {
        let divisor = tuning.recenter_divisor.max(1);
        let ease = |v: i32| if v.abs() < divisor { 0 } else { v - v / divisor };
        orbit.yaw = ease(orbit.yaw);
        orbit.pitch = ease(orbit.pitch);
    }
}

/// Before the camera update: reads this tick's input. The mode is the one
/// the camera used last tick.
fn steer_orbit(
    pad: Res<ControllerState>,
    settings: Res<FreeCameraSettings>,
    tuning: Res<FreeCameraTuning>,
    camera: Res<CameraState>,
    players: Query<&Avatar>,
    mut orbit: ResMut<CameraOrbit>,
) {
    let Ok(player) = players.single() else { return };
    let pose = (player.position, player.yaw);
    let moved = orbit.last.is_some_and(|last| last != pose);
    orbit.last = Some(pose);
    let input = steering(&pad, &settings, &tuning);
    // Off and centred is the original camera: leave the resource untouched.
    if input == Vec2::ZERO && orbit.is_centred() && orbit.divisor.is_none() {
        return;
    }
    step_orbit(&mut orbit, input, camera.mode, moved, &settings, &tuning);
}

/// The orbit rules, added by [`super::CameraRulesPlugin`]. Renderer-free.
pub(super) fn rules(app: &mut App) {
    app.init_resource::<FreeCameraSettings>()
        .init_resource::<FreeCameraTuning>()
        .init_resource::<CameraOrbit>()
        .register_type::<FreeCameraSettings>()
        .register_type::<FreeCameraTuning>()
        .register_type::<CameraOrbit>()
        .add_systems(
            FixedUpdate,
            steer_orbit
                .before(CameraSystems::Update)
                .in_set(WorldSystems::Camera)
                .in_set(GameSystems::Game)
                .run_if(simulation_enabled),
        );
}

/// The player-facing side: the options on the controls menu's CAMERA page,
/// `camera.json`, and the hidden cursor while mouse look is on. Added by the
/// executable, not by the game's plugins, so tests never read a player's file.
#[derive(Default)]
pub struct FreeCameraMenuPlugin {
    /// Settings file; `None` keeps the settings in memory only.
    pub path: Option<PathBuf>,
}

impl FreeCameraMenuPlugin {
    /// `camera.json` beside the controls file for game `app`.
    pub fn default_path(app: &str) -> Option<PathBuf> {
        settings::config_path(app, "camera.json")
    }
}

#[derive(Resource)]
struct SettingsFile(Option<PathBuf>);

impl Plugin for FreeCameraMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FreeCameraSettings>()
            .init_resource::<ControlsOptions>()
            .init_resource::<InputFocus>()
            .insert_resource(SettingsFile(self.path.clone()))
            .add_systems(Startup, load)
            .add_systems(
                Update,
                (
                    apply_options.run_if(resource_changed::<ControlsOptions>),
                    show_settings.run_if(resource_changed::<FreeCameraSettings>),
                    grab_cursor,
                )
                    .chain(),
            );
    }
}

const MOUSE: &str = "camera.mouse";
const MOUSE_SPEED: &str = "camera.mouse_speed";
const STICK: &str = "camera.stick";
const STICK_SPEED: &str = "camera.stick_speed";
const INVERT_Y: &str = "camera.invert_y";
const RECENTER: &str = "camera.recenter";
const SPEEDS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];

/// The menu rows for `settings`: their values, defaults from [`Default`].
pub fn menu_options(settings: &FreeCameraSettings) -> [ControlOption; 6] {
    let defaults = FreeCameraSettings::default();
    let speed = |id, label, value: u8, default: u8| {
        let mut option = ControlOption::new(id, label, &SPEEDS, usize::from(default.saturating_sub(1)));
        option.value = usize::from(value.clamp(1, FreeCameraSettings::MAX_SPEED) - 1);
        option
    };
    let toggle = |id, label, value: bool, default| ControlOption {
        value: usize::from(value),
        ..ControlOption::toggle(id, label, default)
    };
    [
        toggle(MOUSE, "Mouse look", settings.mouse, defaults.mouse),
        speed(MOUSE_SPEED, "Mouse sens", settings.mouse_speed, defaults.mouse_speed),
        toggle(STICK, "R stick", settings.stick, defaults.stick),
        speed(STICK_SPEED, "Stick sens", settings.stick_speed, defaults.stick_speed),
        toggle(INVERT_Y, "Invert Y", settings.invert_y, defaults.invert_y),
        toggle(RECENTER, "Recentre", settings.recenter, defaults.recenter),
    ]
}

/// The settings the menu rows show; `None` before they are registered.
pub fn settings_from_options(options: &ControlsOptions) -> Option<FreeCameraSettings> {
    let speed = |id| options.value(id).map(|v| v as u8 + 1);
    options.get(MOUSE)?;
    Some(FreeCameraSettings {
        mouse: options.enabled(MOUSE),
        stick: options.enabled(STICK),
        invert_y: options.enabled(INVERT_Y),
        mouse_speed: speed(MOUSE_SPEED)?,
        stick_speed: speed(STICK_SPEED)?,
        recenter: options.enabled(RECENTER),
    })
}

fn load(file: Res<SettingsFile>, mut settings: ResMut<FreeCameraSettings>, mut options: ResMut<ControlsOptions>) {
    if let Some(path) = &file.0 {
        match settings::read(path)
            .and_then(|b| b.map(|b| serde_json::from_slice(&b).map_err(|e| e.to_string())).transpose())
        {
            Ok(Some(loaded)) => *settings = loaded,
            Ok(None) => {}
            Err(error) => warn!("Camera settings were not loaded (defaults kept): {error}"),
        }
    }
    options.title = "CAMERA".into();
    for option in menu_options(&settings) {
        options.add(option);
    }
}

/// Save and close on the controls menu: take the new values and save them.
fn apply_options(options: Res<ControlsOptions>, file: Res<SettingsFile>, mut settings: ResMut<FreeCameraSettings>) {
    let Some(chosen) = settings_from_options(&options) else { return };
    if !settings.set_if_neq(chosen) {
        return;
    }
    let Some(path) = &file.0 else { return };
    let saved = serde_json::to_vec_pretty(&chosen).map_err(|e| e.to_string()).and_then(|b| settings::write(path, &b));
    if let Err(error) = saved {
        warn!("Could not save camera settings: {error}");
    }
}

/// Settings changed by code (a mod, the developer MCP) show in the menu.
fn show_settings(settings: Res<FreeCameraSettings>, mut options: ResMut<ControlsOptions>) {
    if settings_from_options(&options).is_some_and(|shown| shown != *settings) {
        for option in menu_options(&settings) {
            options.add(option);
        }
    }
}

/// Mouse look hides and holds the cursor in play, and gives it back to the
/// menus (any that take input focus) and other windows.
fn grab_cursor(
    settings: Res<FreeCameraSettings>,
    focus: Res<InputFocus>,
    world: Option<Res<WorldGeometry>>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    for (window, mut cursor) in &mut windows {
        let grab = settings.mouse && world.is_some() && window.focused && !focus.captured;
        // X11 has no locked mode and falls back to confined; motion is raw either way.
        let wanted = if grab { CursorGrabMode::Locked } else { CursorGrabMode::None };
        if cursor.grab_mode != wanted || cursor.visible == grab {
            cursor.grab_mode = wanted;
            cursor.visible = !grab;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(orbit: &mut CameraOrbit, input: Vec2, mode: CameraMode, moved: bool) {
        step_orbit(orbit, input, mode, moved, &FreeCameraSettings::default(), &FreeCameraTuning::default());
    }

    #[test]
    fn devices_steer_only_when_enabled_and_invert_y_flips_pitch() {
        let tuning = FreeCameraTuning::default();
        let mut pad = ControllerState::default();
        pad.mouse.update(0, None, Vec2::new(3.0, -2.0), 0.0);
        pad.update(0, Vec2::ZERO, Vec2::new(0.5, 1.0));
        let mut settings = FreeCameraSettings::default();
        assert_eq!(steering(&pad, &settings, &tuning), Vec2::ZERO, "off by default");
        settings.mouse = true;
        assert_eq!(steering(&pad, &settings, &tuning), Vec2::new(30.0, 20.0), "mouse away looks up");
        settings.mouse = false;
        settings.stick = true;
        assert_eq!(steering(&pad, &settings, &tuning), Vec2::new(400.0, 800.0));
        settings.invert_y = true;
        assert_eq!(steering(&pad, &settings, &tuning), Vec2::new(400.0, -800.0));
    }

    #[test]
    fn orbit_wraps_yaw_clamps_pitch_and_eases_back_after_the_delay_once_moving() {
        let tuning = FreeCameraTuning::default();
        let mut orbit = CameraOrbit::default();
        tick(&mut orbit, Vec2::new(32000.0, 20000.0), CameraMode::Chase, false);
        tick(&mut orbit, Vec2::new(1000.0, 0.0), CameraMode::Chase, false);
        assert_eq!((orbit.yaw, orbit.pitch), (33000 - 65536, tuning.max_pitch));
        assert_eq!(orbit.divisor, Some(tuning.steer_divisor));
        // Standing still, the view stays where the player left it.
        for _ in 0..200 {
            tick(&mut orbit, Vec2::ZERO, CameraMode::Chase, false);
        }
        assert_eq!((orbit.yaw, orbit.pitch, orbit.divisor), (33000 - 65536, tuning.max_pitch, None));
        // Moving, it closes 1/8 per tick back to exactly zero.
        tick(&mut orbit, Vec2::ZERO, CameraMode::Chase, true);
        assert_eq!(orbit.yaw, -32536 + 4067);
        for _ in 0..100 {
            tick(&mut orbit, Vec2::ZERO, CameraMode::Chase, true);
        }
        assert!(orbit.is_centred());
    }

    #[test]
    fn other_camera_modes_ignore_input_and_recentre_at_once() {
        for mode in [CameraMode::Look, CameraMode::Aim, CameraMode::Fixed] {
            let mut orbit = CameraOrbit { yaw: 8000, pitch: -800, ..default() };
            tick(&mut orbit, Vec2::new(500.0, 500.0), mode, false);
            assert_eq!((orbit.yaw, orbit.pitch, orbit.idle), (7000, -700, 1), "{mode:?}");
        }
    }

    #[test]
    fn recentring_can_be_turned_off() {
        let settings = FreeCameraSettings { recenter: false, ..default() };
        let mut orbit = CameraOrbit { yaw: 8000, idle: 1000, ..default() };
        step_orbit(&mut orbit, Vec2::ZERO, CameraMode::Chase, true, &settings, &FreeCameraTuning::default());
        assert_eq!(orbit.yaw, 8000);
    }

    #[test]
    fn menu_rows_round_trip_the_settings() {
        let settings = FreeCameraSettings {
            mouse: true,
            stick: false,
            invert_y: true,
            mouse_speed: 10,
            stick_speed: 1,
            recenter: false,
        };
        let mut options = ControlsOptions::default();
        assert_eq!(settings_from_options(&options), None);
        for option in menu_options(&settings) {
            assert!(option.label.len() <= 10 && option.choices.iter().all(|c| c.len() <= 9));
            options.add(option);
        }
        assert_eq!(settings_from_options(&options), Some(settings));
        options.restore_defaults();
        assert_eq!(settings_from_options(&options), Some(FreeCameraSettings::default()));
    }

    #[test]
    fn saved_files_from_older_versions_fill_in_defaults() {
        let loaded: FreeCameraSettings = serde_json::from_str(r#"{"mouse": true}"#).unwrap();
        assert_eq!(loaded, FreeCameraSettings { mouse: true, ..default() });
    }
}
