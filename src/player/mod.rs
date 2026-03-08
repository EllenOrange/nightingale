pub mod audio;
pub mod background;
pub mod lyrics;

use bevy::prelude::*;
use bevy_kira_audio::AudioInstance;

use crate::analyzer::cache::CacheDir;
use crate::analyzer::transcript::Transcript;
use crate::analyzer::PlayTarget;
use crate::scanner::metadata::SongLibrary;
use crate::states::AppState;
use audio::{KaraokeAudio, cleanup_audio, setup_audio, start_playback, update_vocals_volume};
use background::{
    ActiveTheme, AuroraMaterial, BackgroundQuad, NebulaMaterial, PlasmaMaterial,
    StarfieldMaterial, WavesMaterial, despawn_background, spawn_background,
};
use lyrics::{CurrentLine, LyricWord, LyricsRoot, LyricsState, NextLine, setup_lyrics};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), enter_playing)
            .add_systems(
                Update,
                (player_update, handle_player_input).run_if(in_state(AppState::Playing)),
            )
            .add_systems(OnExit(AppState::Playing), exit_playing);
    }
}

#[derive(Component)]
struct PlayerHud;

#[derive(Component)]
struct GuideVolumeText;

#[derive(Component)]
struct ThemeText;

fn enter_playing(
    mut commands: Commands,
    target: Res<PlayTarget>,
    library: Res<SongLibrary>,
    cache: Res<CacheDir>,
    config: Res<crate::config::AppConfig>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plasma_materials: ResMut<Assets<PlasmaMaterial>>,
    mut aurora_materials: ResMut<Assets<AuroraMaterial>>,
    mut waves_materials: ResMut<Assets<WavesMaterial>>,
    mut nebula_materials: ResMut<Assets<NebulaMaterial>>,
    mut starfield_materials: ResMut<Assets<StarfieldMaterial>>,
    theme: Res<ActiveTheme>,
) {
    let song = &library.songs[target.song_index];
    let hash = &song.file_hash;

    let transcript_path = cache.transcript_path(hash);
    let mut transcript = match Transcript::load(&transcript_path) {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to load transcript: {e}");
            return;
        }
    };

    transcript.split_long_segments(8);

    let saved_guide = config.guide_volume.unwrap_or(0.0);
    setup_audio(&mut commands, &asset_server, &target, &library, &cache, saved_guide);
    setup_lyrics(&mut commands, &transcript);
    spawn_background(
        &mut commands,
        &mut meshes,
        &mut plasma_materials,
        &mut aurora_materials,
        &mut waves_materials,
        &mut nebula_materials,
        &mut starfield_materials,
        &theme,
    );

    let title = song.display_title().to_string();
    let artist = song.display_artist().to_string();

    let guide_vol = config.guide_volume.unwrap_or(0.0);
    let guide_text = format_guide_text(guide_vol);
    let theme_text = format!("Theme: {} [T]", theme.name());

    commands
        .spawn((
            PlayerHud,
            Node {
                width: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(24.0)),
                ..default()
            },
        ))
        .with_children(|hud| {
            hud.spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|info| {
                info.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                info.spawn((
                    Text::new(artist),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                ));
            });

            hud.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                ..default()
            })
            .with_children(|ctrl| {
                ctrl.spawn((
                    GuideVolumeText,
                    Text::new(guide_text),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                ));
                ctrl.spawn((
                    ThemeText,
                    Text::new(theme_text),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                ));
                ctrl.spawn((
                    Text::new("[ESC] Back"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                ));
            });
        });
}

fn format_guide_text(volume: f64) -> String {
    let vol_pct = (volume * 100.0) as i32;
    if vol_pct == 0 {
        "Guide: OFF [G +/-]".into()
    } else {
        format!("Guide: {vol_pct}% [G +/-]")
    }
}

fn player_update(
    mut karaoke: ResMut<KaraokeAudio>,
    audio: Res<bevy_kira_audio::Audio>,
    time: Res<Time>,
    lyrics_state: Option<ResMut<LyricsState>>,
    current_line_query: Query<(Entity, &mut BackgroundColor), (With<CurrentLine>, Without<NextLine>)>,
    next_line_query: Query<(Entity, &mut BackgroundColor), (With<NextLine>, Without<CurrentLine>)>,
    word_query: Query<(&LyricWord, &mut TextColor)>,
    mut commands: Commands,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    start_playback(&mut karaoke, &audio, &time);
    update_vocals_volume(&karaoke, &mut audio_instances);

    let current_time = audio::playback_time(&karaoke, &audio_instances);

    if let Some(lyrics) = lyrics_state {
        lyrics::update_lyrics(
            lyrics,
            current_time,
            current_line_query,
            next_line_query,
            word_query,
            &mut commands,
        );
    }
}

fn handle_player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut karaoke: Option<ResMut<KaraokeAudio>>,
    mut theme: ResMut<ActiveTheme>,
    mut config: ResMut<crate::config::AppConfig>,
    mut guide_text_query: Query<&mut Text, (With<GuideVolumeText>, Without<ThemeText>)>,
    mut theme_text_query: Query<&mut Text, (With<ThemeText>, Without<GuideVolumeText>)>,
    mut commands: Commands,
    bg_query: Query<Entity, With<BackgroundQuad>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plasma_materials: ResMut<Assets<PlasmaMaterial>>,
    mut aurora_materials: ResMut<Assets<AuroraMaterial>>,
    mut waves_materials: ResMut<Assets<WavesMaterial>>,
    mut nebula_materials: ResMut<Assets<NebulaMaterial>>,
    mut starfield_materials: ResMut<Assets<StarfieldMaterial>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
        return;
    }

    if let Some(ref mut karaoke) = karaoke {
        let mut guide_changed = false;

        if keyboard.just_pressed(KeyCode::KeyG) {
            karaoke.guide_volume = if karaoke.guide_volume > 0.0 {
                0.0
            } else {
                0.3
            };
            guide_changed = true;
        }

        if keyboard.just_pressed(KeyCode::Equal) {
            karaoke.guide_volume = (karaoke.guide_volume + 0.1).min(1.0);
            guide_changed = true;
        }
        if keyboard.just_pressed(KeyCode::Minus) {
            karaoke.guide_volume = (karaoke.guide_volume - 0.1).max(0.0);
            guide_changed = true;
        }

        if guide_changed {
            config.guide_volume = Some(karaoke.guide_volume);
            config.save();
            if let Ok(mut text) = guide_text_query.single_mut() {
                **text = format_guide_text(karaoke.guide_volume);
            }
        }
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        despawn_background(&mut commands, &bg_query);
        theme.next();
        spawn_background(
            &mut commands,
            &mut meshes,
            &mut plasma_materials,
            &mut aurora_materials,
            &mut waves_materials,
            &mut nebula_materials,
            &mut starfield_materials,
            &theme,
        );

        config.last_theme = Some(theme.index);
        config.save();

        if let Ok(mut text) = theme_text_query.single_mut() {
            **text = format!("Theme: {} [T]", theme.name());
        }
    }
}

fn exit_playing(
    mut commands: Commands,
    audio: Res<bevy_kira_audio::Audio>,
    hud_query: Query<Entity, With<PlayerHud>>,
    lyrics_query: Query<Entity, With<LyricsRoot>>,
    bg_query: Query<Entity, With<BackgroundQuad>>,
) {
    cleanup_audio(&mut commands, &audio);

    for entity in &lyrics_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<LyricsState>();

    for entity in &hud_query {
        commands.entity(entity).despawn();
    }

    for entity in &bg_query {
        commands.entity(entity).despawn();
    }
}
