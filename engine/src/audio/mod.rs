//! Level sound effects and music through ordinary Bevy audio assets.
//!
//! Sounds are addressed by the level's sound ids: the level maps each id to a
//! decoded sample (`assets/levels/<level>/audio/NNN.wav`) and a volume.
//! Animation clips cue sounds on authored frames ([`animation_sounds`]).
mod pcm;

use bevy::{
    audio::{AddAudioSource, Volume},
    prelude::*,
};
pub use pcm::{PcmAudio, PcmLoader};

use crate::render::LevelRequest;
use crate::animation::ObjectAnimationCue;
use crate::objects::WorldObject;
use crate::schedule::{WorldSystems, simulation_enabled};
use crate::shell::GameSystems;
use crate::world::{Animation, Command, WorldGeometry};
pub use crate::world::format::SoundEnvironment;

/// Play the level's sound `0`.
#[derive(Message)]
pub struct SoundRequest(pub usize);

/// Play the music file at this asset path, replacing what is playing.
#[derive(Message)]
pub struct MusicRequest(pub String);

#[derive(Component)]
struct Music;

/// Sounds a clip cues on frames crossed between `previous` and `current`
/// (clip-relative; `previous` is -1 when the clip just started). A frame is
/// crossed when previous < frame <= current, or across a wrap.
pub fn animation_sounds(clip: &Animation, previous: f32, current: f32) -> Vec<(usize, SoundEnvironment)> {
    clip.commands
        .iter()
        .filter_map(|command| match *command {
            Command::Sound { frame, sound, environment } => {
                let frame = frame as f32 - clip.frame_start as f32;
                let crossed =
                    if current >= previous { frame > previous && frame <= current } else { frame > previous || frame <= current };
                crossed.then_some((sound, environment))
            }
            _ => None,
        })
        .collect()
}

/// Authored object cues are produced during the item step. Playback is
/// deferred to the sound phase; no game-specific sound ids live here.
fn object_animation_sounds(
    mut cues: MessageReader<ObjectAnimationCue>,
    world: Option<Res<WorldGeometry>>,
    objects: Query<&WorldObject>,
    mut sounds: MessageWriter<SoundRequest>,
) {
    for event in cues.read() {
        let Command::Sound { sound, environment, .. } = event.cue.command else { continue };
        let water = world
            .as_ref()
            .and_then(|w| objects.get(event.entity).ok().and_then(|o| w.data.rooms.get(o.room)))
            .is_some_and(|r| r.water);
        let plays = match environment {
            SoundEnvironment::Anywhere => true,
            SoundEnvironment::OnLand => !water,
            SoundEnvironment::InWater => water,
            SoundEnvironment::Never => false,
        };
        if plays {
            sounds.write(SoundRequest(sound));
        }
    }
}

fn sound_requests(
    mut commands: Commands,
    mut requests: MessageReader<SoundRequest>,
    world: Option<Res<WorldGeometry>>,
    level: Res<LevelRequest>,
    server: Res<AssetServer>,
) {
    let Some(world) = world else {
        requests.clear();
        return;
    };
    for request in requests.read() {
        let Some(&Some(mapped)) = world.data.sound_map.get(request.0) else { continue };
        let Some(info) = world.data.sounds.get(mapped) else { continue };
        let handle: Handle<PcmAudio> = server.load(format!("levels/{}/audio/{:03}.wav", level.name, info.sample));
        commands.spawn((
            AudioPlayer::<PcmAudio>(handle),
            PlaybackSettings {
                volume: Volume::Linear((f32::from(info.volume) / 32767.0).clamp(0.0, 1.0)),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}

fn music_requests(
    mut commands: Commands,
    mut requests: MessageReader<MusicRequest>,
    old: Query<Entity, With<Music>>,
    server: Res<AssetServer>,
) {
    let Some(request) = requests.read().last() else { return };
    for e in &old {
        commands.entity(e).despawn();
    }
    commands.spawn((AudioPlayer::<PcmAudio>(server.load(request.0.clone())), PlaybackSettings::DESPAWN, Music));
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<PcmAudio>()
            .init_asset_loader::<PcmLoader>()
            .init_resource::<LevelRequest>()
            .add_message::<SoundRequest>()
            .add_message::<MusicRequest>()
            .add_message::<ObjectAnimationCue>()
            .add_systems(FixedUpdate, object_animation_sounds.in_set(WorldSystems::Effects).in_set(GameSystems::Game).run_if(simulation_enabled))
            .add_systems(Update, (sound_requests, music_requests));
    }
}
