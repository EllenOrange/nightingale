use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioInstance, prelude::*};

use crate::analyzer::cache::CacheDir;
use crate::analyzer::PlayTarget;
use crate::scanner::metadata::SongLibrary;

#[derive(Resource)]
pub struct KaraokeAudio {
    pub instrumental_handle: Handle<AudioSource>,
    pub vocals_handle: Handle<AudioSource>,
    pub instrumental_instance: Option<Handle<AudioInstance>>,
    pub vocals_instance: Option<Handle<AudioInstance>>,
    pub guide_volume: f64,
    pub playing: bool,
    pub start_time: f64,
}

fn amplitude_to_db(amp: f64) -> f32 {
    if amp <= 0.001 {
        -60.0
    } else {
        (20.0 * amp.log10()) as f32
    }
}

pub fn setup_audio(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    target: &Res<PlayTarget>,
    library: &Res<SongLibrary>,
    cache: &Res<CacheDir>,
    initial_guide_volume: f64,
) {
    let song = &library.songs[target.song_index];
    let hash = &song.file_hash;

    let instrumental_path = cache.instrumental_path(hash);
    let vocals_path = cache.vocals_path(hash);

    let instrumental_handle: Handle<AudioSource> =
        asset_server.load_override(instrumental_path.to_string_lossy().to_string());
    let vocals_handle: Handle<AudioSource> =
        asset_server.load_override(vocals_path.to_string_lossy().to_string());

    commands.insert_resource(KaraokeAudio {
        instrumental_handle,
        vocals_handle,
        instrumental_instance: None,
        vocals_instance: None,
        guide_volume: initial_guide_volume,
        playing: false,
        start_time: 0.0,
    });
}

pub fn start_playback(karaoke: &mut ResMut<KaraokeAudio>, audio: &Res<Audio>, time: &Res<Time>) {
    if karaoke.playing {
        return;
    }

    let inst = audio.play(karaoke.instrumental_handle.clone()).handle();

    let db = amplitude_to_db(karaoke.guide_volume);
    let voc = audio
        .play(karaoke.vocals_handle.clone())
        .with_volume(db)
        .handle();

    karaoke.instrumental_instance = Some(inst);
    karaoke.vocals_instance = Some(voc);
    karaoke.playing = true;
    karaoke.start_time = time.elapsed_secs_f64();
}

pub fn update_vocals_volume(
    karaoke: &KaraokeAudio,
    audio_instances: &mut ResMut<Assets<AudioInstance>>,
) {
    if let Some(ref handle) = karaoke.vocals_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            let db = amplitude_to_db(karaoke.guide_volume);
            instance.set_decibels(db, default());
        }
    }
}

pub fn playback_time(
    karaoke: &KaraokeAudio,
    audio_instances: &Assets<AudioInstance>,
) -> f64 {
    if !karaoke.playing {
        return 0.0;
    }
    if let Some(ref handle) = karaoke.instrumental_instance {
        if let Some(instance) = audio_instances.get(handle) {
            if let Some(position) = instance.state().position() {
                return position;
            }
        }
    }
    0.0
}

pub fn is_finished(karaoke: &KaraokeAudio, audio_instances: &Assets<AudioInstance>) -> bool {
    if !karaoke.playing {
        return false;
    }
    if let Some(ref handle) = karaoke.instrumental_instance {
        if let Some(instance) = audio_instances.get(handle) {
            return instance.state().position().is_none();
        }
    }
    false
}

pub fn seek_to(
    karaoke: &KaraokeAudio,
    audio_instances: &mut Assets<AudioInstance>,
    position: f64,
) {
    if let Some(ref handle) = karaoke.instrumental_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            instance.seek_to(position);
        }
    }
    if let Some(ref handle) = karaoke.vocals_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            instance.seek_to(position);
        }
    }
}

pub fn pause_audio(karaoke: &KaraokeAudio, audio_instances: &mut Assets<AudioInstance>) {
    if let Some(ref handle) = karaoke.instrumental_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            instance.pause(default());
        }
    }
    if let Some(ref handle) = karaoke.vocals_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            instance.pause(default());
        }
    }
}

pub fn resume_audio(karaoke: &KaraokeAudio, audio_instances: &mut Assets<AudioInstance>) {
    if let Some(ref handle) = karaoke.instrumental_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            instance.resume(default());
        }
    }
    if let Some(ref handle) = karaoke.vocals_instance {
        if let Some(instance) = audio_instances.get_mut(handle) {
            instance.resume(default());
        }
    }
}

pub fn cleanup_audio(commands: &mut Commands, audio: &Res<Audio>) {
    audio.stop();
    commands.remove_resource::<KaraokeAudio>();
}
