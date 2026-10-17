//! Tomb Raider II's sounds: Lara's animation and weapon sounds, and the
//! soundtrack's logical track ids. Playback is the engine's (`engine::audio`).
use bevy::prelude::*;
use engine::{
    audio::{MusicRequest, SoundEnvironment, SoundRequest, animation_sounds},
    render::LevelRequest,
    schedule::{SimulationEnabled, WorldSystems},
    shell::GameSystems,
    world::WorldGeometry,
};

use crate::player::{AnimationState, Motion, Player, WeaponState};

/// Logical TR2 music id, before the game's CDDA track remapping.
#[derive(Message)]
pub struct TrackRequest(pub usize);

#[derive(Resource, Default)]
struct SoundState {
    animation: usize,
    frame: f32,
    shots: u64,
    drawn: bool,
    level: String,
}

pub struct GameAudioPlugin;
impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TrackRequest>()
            .init_resource::<SoundState>()
            .add_systems(FixedUpdate, lara_sounds.in_set(WorldSystems::Effects).in_set(GameSystems::Game))
            .add_systems(Update, tracks);
    }
}

/// Weapon sound ids: draw, holster, pistol shot.
const DRAW: usize = 6;
const HOLSTER: usize = 7;
const SHOT: usize = 8;

fn lara_sounds(
    world: Option<Res<WorldGeometry>>,
    request: Res<LevelRequest>,
    enabled: Res<SimulationEnabled>,
    player: Query<(&AnimationState, &WeaponState, &Motion), With<Player>>,
    mut state: ResMut<SoundState>,
    mut sounds: MessageWriter<SoundRequest>,
) {
    if !enabled.0 {
        return;
    }
    let (Some(world), Ok((animation, weapon, motion))) = (world, player.single()) else { return };
    let Some(clip) = world.data.animations.get(animation.animation) else { return };
    let previous = if state.animation == animation.animation && state.level == request.name { state.frame } else { -1.0 };
    for (sound, environment) in animation_sounds(clip, previous, animation.frame) {
        // Land and water sounds: the grounded flag stands in for "on land".
        let plays = match environment {
            SoundEnvironment::Anywhere => true,
            SoundEnvironment::OnLand => motion.grounded,
            SoundEnvironment::InWater => !motion.grounded,
            SoundEnvironment::Never => false,
        };
        if plays {
            sounds.write(SoundRequest(sound));
        }
    }
    if weapon.shot_serial != state.shots {
        sounds.write(SoundRequest(SHOT));
    }
    if weapon.drawn != state.drawn {
        sounds.write(SoundRequest(if weapon.drawn { DRAW } else { HOLSTER }));
    }
    state.animation = animation.animation;
    state.frame = animation.frame;
    state.shots = weapon.shot_serial;
    state.drawn = weapon.drawn;
    state.level = request.name.clone();
}

// OpenLara src/gameflow.h remapTrack TR2; original logical ids are not physical
// CDDA track numbers. Preserve the table at the resource boundary.
pub const CDDA_TRACKS: &[u8] = &[
    2, 2, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 18, 18, 19, 20, 21, 22, 23, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
    59, 60, 61,
];

fn tracks(mut requests: MessageReader<TrackRequest>, mut music: MessageWriter<MusicRequest>) {
    let Some(request) = requests.read().last() else { return };
    if let Some(track) = CDDA_TRACKS.get(request.0) {
        music.write(MusicRequest(format!("music/{track:02}.wav")));
    }
}
