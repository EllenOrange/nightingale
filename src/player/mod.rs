pub mod audio;
pub mod background;
pub mod lyrics;
pub mod microphone;
pub mod scoring;

use bevy::prelude::*;
use bevy_kira_audio::AudioInstance;

use crate::analyzer::cache::CacheDir;
use crate::analyzer::transcript::Transcript;
use crate::analyzer::PlayTarget;
use crate::scanner::metadata::SongLibrary;
use crate::states::AppState;
use crate::ui::UiTheme;
use audio::{KaraokeAudio, cleanup_audio, setup_audio, start_playback, update_vocals_volume};
use background::{
    ActiveTheme, AuroraMaterial, BackgroundQuad, NebulaMaterial, PlasmaMaterial,
    StarfieldMaterial, WavesMaterial, despawn_background, spawn_background,
};
use lyrics::{
    CountdownNode, CurrentLine, LyricWord, LyricsRoot, LyricsState, NextLine, setup_lyrics,
};
use scoring::{MicStatusText, ScoreText};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), enter_playing)
            .add_systems(
                Update,
                (
                    player_update,
                    handle_escape,
                    handle_guide_volume,
                    handle_theme_switch,
                )
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                (
                    handle_skip_buttons,
                    handle_mic_toggle,
                    check_mic_health,
                    scoring::update_pitch_scoring,
                    scoring::draw_pitch_waves,
                    scoring::update_score_text,
                )
                    .run_if(in_state(AppState::Playing)),
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

#[derive(Component)]
struct SkipIntroButton;

#[derive(Component)]
struct SkipOutroButton;

const SKIP_BTN_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const SKIP_BTN_HOVER: Color = Color::srgba(0.2, 0.2, 0.3, 0.7);

fn spawn_skip_button(parent: &mut ChildSpawnerCommands, label: &str, component: impl Component) {
    parent
        .spawn((
            component,
            Button,
            Node {
                display: Display::None,
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(6.0),
                    Val::Px(6.0),
                ),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(SKIP_BTN_BG),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            ));
        });
}

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
    bg_theme: Res<ActiveTheme>,
    ui_theme: Res<UiTheme>,
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
    setup_audio(
        &mut commands,
        &asset_server,
        &target,
        &library,
        &cache,
        saved_guide,
    );
    setup_lyrics(&mut commands, &transcript, &ui_theme);
    spawn_background(
        &mut commands,
        &mut meshes,
        &mut plasma_materials,
        &mut aurora_materials,
        &mut waves_materials,
        &mut nebula_materials,
        &mut starfield_materials,
        &bg_theme,
    );

    let vocals_path = cache.vocals_path(hash);
    let song_duration = song.duration_secs;
    if let Some(vocals_buf) = scoring::load_vocals_buffer(&vocals_path) {
        commands.insert_resource(vocals_buf);
    }

    let mut mic_capture =
        microphone::start_microphone(config.preferred_mic.as_deref());
    let mic_has_device = mic_capture.active;
    if mic_has_device {
        mic_capture.active = config.mic_active.unwrap_or(true);
    }
    let mic_active = mic_capture.active;
    let mic_device_name = mic_capture.device_name.clone();
    commands.insert_resource(mic_capture);
    commands.insert_resource(scoring::PitchState::default());
    commands.insert_resource(scoring::ScoringState::new(song_duration));

    let title = song.display_title().to_string();
    let artist = song.display_artist().to_string();

    let guide_vol = config.guide_volume.unwrap_or(0.0);
    let guide_text = format_guide_text(guide_vol);
    let theme_text = format!("Theme: {} [T]", bg_theme.name());
    let mic_text = format_mic_text(mic_active, &mic_device_name);

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
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|info| {
                info.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_primary),
                ));
                info.spawn((
                    Text::new(artist),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_secondary),
                ));

                info.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                })
                .with_children(|row| {
                    spawn_skip_button(row, "Skip Intro", SkipIntroButton);
                    spawn_skip_button(row, "Skip Outro", SkipOutroButton);
                });
            });

            hud.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                ..default()
            })
            .with_children(|ctrl| {
                ctrl.spawn((
                    ScoreText,
                    Text::new("Score: --"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_secondary),
                ));
                ctrl.spawn((
                    MicStatusText,
                    Text::new(mic_text),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_dim),
                ));
                ctrl.spawn((
                    GuideVolumeText,
                    Text::new(guide_text),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_dim),
                ));
                ctrl.spawn((
                    ThemeText,
                    Text::new(theme_text),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_dim),
                ));
                ctrl.spawn((
                    Text::new("[ESC] Back"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_dim),
                ));
            });
        });

    commands
        .spawn((
            PlayerHud,
            Node {
                width: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                bottom: Val::Px(8.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|bar| {
            bar.spawn((
                Text::new(
                    "Lyrics and timing are AI-generated and may not be perfectly accurate",
                ),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.25)),
            ));
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

fn format_mic_text(active: bool, device_name: &str) -> String {
    let short_name = if device_name.len() > 30 {
        format!("{}…", &device_name[..29])
    } else {
        device_name.to_string()
    };
    if active {
        format!("Mic: ON — {short_name} [M/N]")
    } else {
        format!("Mic: OFF — {short_name} [M/N]")
    }
}

fn player_update(
    mut karaoke: ResMut<KaraokeAudio>,
    audio: Res<bevy_kira_audio::Audio>,
    time: Res<Time>,
    lyrics_state: Option<ResMut<LyricsState>>,
    current_line_query: Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<CurrentLine>, Without<NextLine>, Without<CountdownNode>),
    >,
    next_line_query: Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<NextLine>, Without<CurrentLine>, Without<CountdownNode>),
    >,
    countdown_query: Query<
        (&mut Visibility, &mut BackgroundColor, &Children),
        (With<CountdownNode>, Without<CurrentLine>, Without<NextLine>),
    >,
    countdown_text_query: Query<&mut Text, Without<LyricWord>>,
    word_query: Query<(&LyricWord, &mut TextColor)>,
    mut commands: Commands,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut intro_node: Query<&mut Node, (With<SkipIntroButton>, Without<SkipOutroButton>)>,
    mut outro_node: Query<&mut Node, (With<SkipOutroButton>, Without<SkipIntroButton>)>,
    ui_theme: Res<UiTheme>,
) {
    start_playback(&mut karaoke, &audio, &time);
    update_vocals_volume(&karaoke, &mut audio_instances);

    let current_time = audio::playback_time(&karaoke, &audio_instances);

    if audio::is_finished(&karaoke, &audio_instances) {
        next_state.set(AppState::Menu);
        return;
    }

    if let Some(lyrics) = lyrics_state {
        let first_start = lyrics::first_segment_start(&lyrics);
        let last_end = lyrics::last_segment_end(&lyrics);

        if let Ok(mut node) = intro_node.single_mut() {
            node.display = if current_time < first_start - 3.0 {
                Display::Flex
            } else {
                Display::None
            };
        }
        if let Ok(mut node) = outro_node.single_mut() {
            node.display = if current_time > last_end + 1.0 {
                Display::Flex
            } else {
                Display::None
            };
        }

        lyrics::update_lyrics(
            lyrics,
            current_time,
            current_line_query,
            next_line_query,
            countdown_query,
            countdown_text_query,
            word_query,
            &mut commands,
            &ui_theme,
        );
    }
}

fn handle_skip_buttons(
    mut intro_query: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<SkipIntroButton>,
            Without<SkipOutroButton>,
            Changed<Interaction>,
        ),
    >,
    mut outro_query: Query<
        (&Interaction, &mut BackgroundColor),
        (
            With<SkipOutroButton>,
            Without<SkipIntroButton>,
            Changed<Interaction>,
        ),
    >,
    lyrics_state: Option<Res<LyricsState>>,
    karaoke: Option<ResMut<KaraokeAudio>>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, mut bg) in &mut intro_query {
        match interaction {
            Interaction::Pressed => {
                if let (Some(lyrics), Some(karaoke)) = (&lyrics_state, &karaoke) {
                    let target = (lyrics::first_segment_start(lyrics) - 3.0).max(0.0);
                    audio::seek_to(karaoke, &mut audio_instances, target);
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(SKIP_BTN_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(SKIP_BTN_BG);
            }
        }
    }

    for (interaction, mut bg) in &mut outro_query {
        match interaction {
            Interaction::Pressed => {
                next_state.set(AppState::Menu);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(SKIP_BTN_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(SKIP_BTN_BG);
            }
        }
    }
}

fn handle_mic_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mic: Option<ResMut<microphone::MicrophoneCapture>>,
    mut mic_text_query: Query<&mut Text, With<MicStatusText>>,
    mut config: ResMut<crate::config::AppConfig>,
) {
    let Some(mut mic) = mic else { return };

    if keyboard.just_pressed(KeyCode::KeyM) {
        mic.active = !mic.active;
        config.mic_active = Some(mic.active);
        config.save();
        if let Ok(mut text) = mic_text_query.single_mut() {
            **text = format_mic_text(mic.active, &mic.device_name);
        }
    }

    if keyboard.just_pressed(KeyCode::KeyN) {
        let devices = microphone::available_devices();
        if devices.len() <= 1 {
            return;
        }
        let current_idx = devices.iter().position(|n| *n == mic.device_name);
        let next_idx = match current_idx {
            Some(i) => (i + 1) % devices.len(),
            None => 0,
        };
        let next_name = &devices[next_idx];
        info!("Switching mic to: {next_name}");

        let was_active = mic.active;
        let mut new_capture = microphone::start_microphone(Some(next_name));
        new_capture.active = was_active && new_capture.active;
        let device_name = new_capture.device_name.clone();
        commands.insert_resource(new_capture);

        config.preferred_mic = Some(device_name.clone());
        config.save();

        if let Ok(mut text) = mic_text_query.single_mut() {
            **text = format_mic_text(was_active, &device_name);
        }
    }
}

fn check_mic_health(
    mut commands: Commands,
    mic: Option<ResMut<microphone::MicrophoneCapture>>,
    mut mic_text_query: Query<&mut Text, With<MicStatusText>>,
    config: Res<crate::config::AppConfig>,
) {
    let Some(mut mic) = mic else { return };
    if !mic.active {
        return;
    }

    if mic.check_health() {
        return;
    }

    warn!(
        "Mic '{}' disconnected, attempting auto-recovery",
        mic.device_name
    );

    let mut new_capture = microphone::start_microphone(config.preferred_mic.as_deref());
    if !new_capture.has_stream() {
        new_capture.active = false;
        new_capture.device_name = "(disconnected)".into();
    }

    let name = new_capture.device_name.clone();
    let active = new_capture.active;
    commands.insert_resource(new_capture);

    if active {
        info!("Mic auto-recovered to '{name}'");
    }

    if let Ok(mut text) = mic_text_query.single_mut() {
        **text = format_mic_text(active, &name);
    }
}

fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
    }
}

fn handle_guide_volume(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut karaoke: Option<ResMut<KaraokeAudio>>,
    mut config: ResMut<crate::config::AppConfig>,
    mut query: Query<&mut Text, With<GuideVolumeText>>,
) {
    let Some(ref mut karaoke) = karaoke else {
        return;
    };

    let mut changed = false;

    if keyboard.just_pressed(KeyCode::KeyG) {
        karaoke.guide_volume = if karaoke.guide_volume > 0.0 {
            0.0
        } else {
            0.3
        };
        changed = true;
    }
    if keyboard.just_pressed(KeyCode::Equal) {
        karaoke.guide_volume = (karaoke.guide_volume + 0.1).min(1.0);
        changed = true;
    }
    if keyboard.just_pressed(KeyCode::Minus) {
        karaoke.guide_volume = (karaoke.guide_volume - 0.1).max(0.0);
        changed = true;
    }

    if changed {
        config.guide_volume = Some(karaoke.guide_volume);
        config.save();
        if let Ok(mut text) = query.single_mut() {
            **text = format_guide_text(karaoke.guide_volume);
        }
    }
}

fn handle_theme_switch(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut theme: ResMut<ActiveTheme>,
    mut config: ResMut<crate::config::AppConfig>,
    mut query: Query<&mut Text, With<ThemeText>>,
    mut commands: Commands,
    bg_query: Query<Entity, With<BackgroundQuad>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plasma_materials: ResMut<Assets<PlasmaMaterial>>,
    mut aurora_materials: ResMut<Assets<AuroraMaterial>>,
    mut waves_materials: ResMut<Assets<WavesMaterial>>,
    mut nebula_materials: ResMut<Assets<NebulaMaterial>>,
    mut starfield_materials: ResMut<Assets<StarfieldMaterial>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyT) {
        return;
    }

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

    if let Ok(mut text) = query.single_mut() {
        **text = format!("Theme: {} [T]", theme.name());
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

    commands.remove_resource::<microphone::MicrophoneCapture>();
    commands.remove_resource::<scoring::VocalsBuffer>();
    commands.remove_resource::<scoring::PitchState>();
    commands.remove_resource::<scoring::ScoringState>();
}
