pub mod audio;
pub mod background;
pub mod lyrics;
pub mod microphone;
pub mod results;
pub mod scoring;
pub mod video_bg;

use bevy::prelude::*;
use bevy_kira_audio::AudioInstance;

use crate::analyzer::cache::CacheDir;
use crate::analyzer::transcript::Transcript;
use crate::analyzer::PlayTarget;
use crate::profile::ProfileStore;
use crate::scanner::metadata::SongLibrary;
use crate::states::AppState;
use crate::ui::UiTheme;
use audio::{KaraokeAudio, cleanup_audio, setup_audio, start_playback, update_vocals_volume};
use background::{
    ActiveTheme, AuroraMaterial, BackgroundQuad, NebulaMaterial, PlasmaMaterial,
    StarfieldMaterial, WavesMaterial, despawn_background, spawn_background,
};
use video_bg::{ActiveVideoFlavor, VideoBackground, VideoSprite};
use lyrics::{
    CountdownNode, CurrentLine, LyricWord, LyricsRoot, LyricsState, NextLine, setup_lyrics,
};
use results::{PauseOverlay, ResultsOverlay};
use scoring::{MicStatusText, ScoreText};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, configure_gizmos)
            .add_systems(OnEnter(AppState::Playing), enter_playing)
            .add_systems(
                Update,
                (
                    check_song_finished,
                    player_update,
                    handle_escape,
                    handle_guide_volume,
                    update_time_display,
                )
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                handle_theme_switch
                    .run_if(in_state(AppState::Playing))
                    .run_if(no_player_overlay),
            )
            .add_systems(
                Update,
                handle_flavor_switch
                    .run_if(in_state(AppState::Playing))
                    .run_if(no_player_overlay),
            )
            .add_systems(
                Update,
                scoring::poll_vocals_load
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                microphone::poll_mic_load
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                handle_skip_buttons
                    .run_if(in_state(AppState::Playing))
                    .run_if(no_player_overlay),
            )
            .add_systems(
                Update,
                (
                    handle_mic_toggle,
                    check_mic_health,
                    scoring::update_pitch_scoring,
                    scoring::update_score_text,
                )
                    .run_if(in_state(AppState::Playing))
                    .run_if(no_player_overlay),
            )
            .add_systems(
                Update,
                (
                    video_bg::update_video_frame,
                    video_bg::fit_video_to_window,
                )
                    .run_if(in_state(AppState::Playing))
                    .run_if(no_player_overlay)
                    .run_if(resource_exists::<VideoBackground>),
            )
            .add_systems(
                Update,
                (
                    scoring::draw_pitch_waves,
                    update_pitch_backdrop,
                )
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(
                Update,
                (
                    results::handle_results_input,
                    results::handle_pause_input,
                )
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(OnExit(AppState::Playing), exit_playing);
    }
}

#[derive(Resource)]
pub struct SourceVideoPath(pub std::path::PathBuf);

#[derive(Component)]
struct PlayerHud;

#[derive(Component)]
struct GuideVolumeText;

#[derive(Component)]
struct ThemeText;

#[derive(Component)]
struct PixabayCreditText;

#[derive(Component)]
struct TimeDisplayText;

#[derive(Component)]
struct PitchBackdrop;

#[derive(Component)]
struct SkipIntroButton;

#[derive(Component)]
struct SkipOutroButton;

fn spawn_skip_button(parent: &mut ChildSpawnerCommands, label: &str, component: impl Component, theme: &UiTheme) {
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
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BorderColor::all(theme.skip_btn_border),
            BackgroundColor(theme.skip_btn_bg),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(format!("{label}  ⏎")),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme.skip_btn_text),
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
    mut images: ResMut<Assets<Image>>,
    mut plasma_materials: ResMut<Assets<PlasmaMaterial>>,
    mut aurora_materials: ResMut<Assets<AuroraMaterial>>,
    mut waves_materials: ResMut<Assets<WavesMaterial>>,
    mut nebula_materials: ResMut<Assets<NebulaMaterial>>,
    mut starfield_materials: ResMut<Assets<StarfieldMaterial>>,
    mut bg_theme: ResMut<ActiveTheme>,
    video_flavor: Res<ActiveVideoFlavor>,
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

    let total_words: usize = transcript.segments.iter().map(|s| s.words.len()).sum();
    let transcript_source = transcript.source.clone();
    info!(
        "Loaded transcript: {} segments, {} words, lang={}, source={}",
        transcript.segments.len(),
        total_words,
        transcript.language,
        transcript_source,
    );

    transcript.split_long_segments(8);

    let saved_guide = config.guide_volume();
    setup_audio(
        &mut commands,
        &asset_server,
        &target,
        &library,
        &cache,
        saved_guide,
    );
    setup_lyrics(&mut commands, &transcript, &ui_theme);

    if song.is_video {
        commands.insert_resource(SourceVideoPath(song.path.clone()));
        bg_theme.set_source_video();
    } else if bg_theme.is_source_video() {
        bg_theme.index = config.last_theme.unwrap_or(0);
    }

    if bg_theme.is_source_video() {
        video_bg::spawn_source_video_background(
            &mut commands,
            &mut images,
            song.path.clone(),
            0.0,
        );
    } else if bg_theme.is_pixabay_video() {
        video_bg::spawn_video_background(&mut commands, &mut images, video_flavor.flavor());
    } else {
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
    }

    let vocals_path = cache.vocals_path(hash);
    scoring::spawn_vocals_load(&mut commands, vocals_path);

    microphone::spawn_mic_load(&mut commands, config.preferred_mic.clone());

    commands.insert_resource(scoring::PitchState::default());
    commands.insert_resource(scoring::ScoringState::new(0.0));

    let mic_active = config.mic_active.unwrap_or(true);
    let mic_device_name = "(loading…)".to_string();

    let title = song.display_title().to_string();
    let artist = song.display_artist().to_string();

    let guide_vol = config.guide_volume();
    let guide_text = format_guide_text(guide_vol);
    let theme_text = format_theme_text(&bg_theme, &video_flavor);
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
                max_width: Val::Percent(40.0),
                overflow: Overflow::clip(),
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
                    TextLayout { linebreak: LineBreak::WordBoundary, ..default() },
                ));
                info.spawn((
                    Text::new(artist),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(ui_theme.hud_secondary),
                    TextLayout { linebreak: LineBreak::WordBoundary, ..default() },
                ));
                let duration_text = crate::menu::song_card::format_duration(song.duration_secs);
                info.spawn((
                    TimeDisplayText,
                    Text::new(format!("0:00 / {duration_text}")),
                    TextFont {
                        font_size: 14.0,
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
                    spawn_skip_button(row, "Skip Intro", SkipIntroButton, &ui_theme);
                    spawn_skip_button(row, "Skip Outro", SkipOutroButton, &ui_theme);
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
            let disclaimer = if transcript_source == "lyrics" {
                "Timing is AI-generated and may not be perfectly accurate"
            } else {
                "Lyrics and timing are AI-generated and may not be perfectly accurate"
            };
            bar.spawn((
                Text::new(disclaimer),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.25)),
            ));
        });

    commands.spawn((
        PlayerHud,
        PixabayCreditText,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(16.0),
            display: if bg_theme.is_pixabay_video() {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        },
        Text::new("Videos by Pixabay"),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
    ));

    commands.spawn((
        PlayerHud,
        PitchBackdrop,
        Sprite {
            color: ui_theme.lyric_backdrop,
            custom_size: Some(Vec2::new(1.0, 1.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
        Visibility::Hidden,
    ));
}

fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();

    config.line.width = 3.0;
}

pub fn no_player_overlay(
    results_q: Query<(), With<ResultsOverlay>>,
    pause_q: Query<(), With<PauseOverlay>>,
) -> bool {
    results_q.is_empty() && pause_q.is_empty()
}

fn update_pitch_backdrop(
    mic: Option<Res<microphone::MicrophoneCapture>>,
    windows: Query<&Window>,
    mut query: Query<(&mut Transform, &mut Visibility, &mut Sprite), With<PitchBackdrop>>,
) {
    let Ok((mut transform, mut vis, mut sprite)) = query.single_mut() else {
        return;
    };

    let mic_active = mic.as_ref().is_some_and(|m| m.active);
    *vis = if mic_active {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    if mic_active {
        if let Ok(window) = windows.single() {
            let wh = window.height();
            let half_h = wh / 2.0;
            let center_y = half_h - scoring::display_top_offset(wh);
            transform.translation.x = 0.0;
            transform.translation.y = center_y;
            let backdrop_pad = 12.0 * scoring::display_scale(wh);
            sprite.custom_size = Some(Vec2::new(
                scoring::display_width(wh) + backdrop_pad * 2.0,
                scoring::display_height(wh) + backdrop_pad * 2.0,
            ));
        }
    }
}

fn format_guide_text(volume: f64) -> String {
    let vol_pct = (volume * 100.0) as i32;
    if vol_pct == 0 {
        "Guide: OFF [G +/-]".into()
    } else {
        format!("Guide: {vol_pct}% [G +/-]")
    }
}

pub fn format_mic_text(active: bool, device_name: &str) -> String {
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

fn transition_on_song_end(
    commands: &mut Commands,
    next_state: &mut ResMut<NextState<AppState>>,
    profiles: &mut ProfileStore,
    target: &PlayTarget,
    library: &SongLibrary,
    scoring_state: &Option<Res<scoring::ScoringState>>,
    theme: &UiTheme,
    asset_server: &AssetServer,
    audio: &bevy_kira_audio::Audio,
) {
    if profiles.active.is_some() {
        let song = &library.songs[target.song_index];
        let score = scoring_state.as_ref().map(|s| s.score()).unwrap_or(0);
        let result = results::SongResult {
            song_title: song.display_title().to_string(),
            song_artist: song.display_artist().to_string(),
            song_hash: song.file_hash.clone(),
            score,
        };
        results::spawn_results_overlay(commands, &result, profiles, theme, asset_server, audio);
        commands.insert_resource(result);
    } else {
        next_state.set(AppState::Menu);
    }
}

fn format_theme_text(theme: &ActiveTheme, flavor: &ActiveVideoFlavor) -> String {
    if theme.is_source_video() {
        "Theme: Source Video [T]".into()
    } else if theme.is_pixabay_video() {
        format!("Theme: Video — {} [T/F]", flavor.flavor().name())
    } else {
        format!("Theme: {} [T]", theme.name())
    }
}

fn check_song_finished(
    mut commands: Commands,
    karaoke: Option<Res<KaraokeAudio>>,
    audio_instances: Res<Assets<AudioInstance>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut profiles: ResMut<ProfileStore>,
    target: Res<PlayTarget>,
    library: Res<SongLibrary>,
    scoring_state: Option<Res<scoring::ScoringState>>,
    results_q: Query<(), With<ResultsOverlay>>,
    pause_q: Query<(), With<PauseOverlay>>,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
) {
    if !results_q.is_empty() || !pause_q.is_empty() {
        return;
    }
    let Some(karaoke) = karaoke else { return };
    if audio::is_finished(&karaoke, &audio_instances) {
        transition_on_song_end(
            &mut commands,
            &mut next_state,
            &mut profiles,
            &target,
            &library,
            &scoring_state,
            &theme,
            &asset_server,
            &audio,
        );
    }
}

fn update_time_display(
    karaoke: Option<Res<KaraokeAudio>>,
    audio_instances: Res<Assets<AudioInstance>>,
    target: Res<PlayTarget>,
    library: Res<SongLibrary>,
    mut query: Query<&mut Text, With<TimeDisplayText>>,
) {
    let Some(karaoke) = karaoke else { return };
    let current_time = audio::playback_time(&karaoke, &audio_instances);
    if let Ok(mut text) = query.single_mut() {
        let duration = library.songs[target.song_index].duration_secs;
        let cur = crate::menu::song_card::format_duration(current_time);
        let tot = crate::menu::song_card::format_duration(duration);
        **text = format!("{cur} / {tot}");
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
    mut intro_node: Query<&mut Node, (With<SkipIntroButton>, Without<SkipOutroButton>)>,
    mut outro_node: Query<&mut Node, (With<SkipOutroButton>, Without<SkipIntroButton>)>,
    ui_theme: Res<UiTheme>,
    results_q: Query<(), With<ResultsOverlay>>,
    pause_q: Query<(), With<PauseOverlay>>,
) {
    if !results_q.is_empty() || !pause_q.is_empty() {
        return;
    }

    start_playback(&mut karaoke, &audio, &time);
    update_vocals_volume(&karaoke, &mut audio_instances);

    let current_time = audio::playback_time(&karaoke, &audio_instances);

    if audio::is_finished(&karaoke, &audio_instances) {
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

fn seek_video_if_source(
    bg_theme: &ActiveTheme,
    source_video: &Option<Res<SourceVideoPath>>,
    video_bg: &mut Option<ResMut<VideoBackground>>,
    seek_target: f64,
) {
    if bg_theme.is_source_video() {
        if let (Some(sv), Some(vbg)) = (source_video, video_bg) {
            video_bg::seek_source_video(vbg, sv.0.clone(), seek_target);
        }
    }
}

fn handle_skip_buttons(
    mut commands: Commands,
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
    nav: Res<crate::input::NavInput>,
    lyrics_state: Option<Res<LyricsState>>,
    karaoke: Option<ResMut<KaraokeAudio>>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    song_ctx: (
        ResMut<NextState<AppState>>,
        ResMut<ProfileStore>,
        Res<PlayTarget>,
        Res<SongLibrary>,
        Option<Res<scoring::ScoringState>>,
    ),
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
    bg_theme: Res<ActiveTheme>,
    source_video: Option<Res<SourceVideoPath>>,
    mut video_bg: Option<ResMut<VideoBackground>>,
) {
    let (mut next_state, mut profiles, target, library, scoring_state) = song_ctx;

    for (interaction, mut bg) in &mut intro_query {
        match interaction {
            Interaction::Pressed => {
                if let (Some(lyrics), Some(karaoke)) = (&lyrics_state, &karaoke) {
                    let seek_target = (lyrics::first_segment_start(lyrics) - 3.0).max(0.0);
                    audio::seek_to(karaoke, &mut audio_instances, seek_target);
                    seek_video_if_source(&bg_theme, &source_video, &mut video_bg, seek_target);
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.skip_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.skip_btn_bg);
            }
        }
    }

    for (interaction, mut bg) in &mut outro_query {
        match interaction {
            Interaction::Pressed => {
                transition_on_song_end(
                    &mut commands,
                    &mut next_state,
                    &mut profiles,
                    &target,
                    &library,
                    &scoring_state,
                    &theme,
                    &asset_server,
                    &audio,
                );
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.skip_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.skip_btn_bg);
            }
        }
    }

    if nav.confirm {
        if let (Some(lyrics), Some(karaoke)) = (&lyrics_state, &karaoke) {
            let current_time = audio::playback_time(karaoke, &audio_instances);
            let first_start = lyrics::first_segment_start(lyrics);
            let last_end = lyrics::last_segment_end(lyrics);

            if current_time < first_start - 3.0 {
                let seek_target = (first_start - 3.0).max(0.0);
                audio::seek_to(karaoke, &mut audio_instances, seek_target);
                seek_video_if_source(&bg_theme, &source_video, &mut video_bg, seek_target);
            } else if current_time > last_end + 1.0 {
                transition_on_song_end(
                    &mut commands,
                    &mut next_state,
                    &mut profiles,
                    &target,
                    &library,
                    &scoring_state,
                    &theme,
                    &asset_server,
                    &audio,
                );
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
    results_q: Query<(), With<ResultsOverlay>>,
    pause_q: Query<(), With<PauseOverlay>>,
) {
    if !results_q.is_empty() || !pause_q.is_empty() {
        return;
    }
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
    nav: Res<crate::input::NavInput>,
    mut commands: Commands,
    results_q: Query<(), With<ResultsOverlay>>,
    pause_q: Query<Entity, With<PauseOverlay>>,
    karaoke: Option<Res<KaraokeAudio>>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
) {
    if !nav.back {
        return;
    }
    if !results_q.is_empty() {
        return;
    }
    if !pause_q.is_empty() {
        for entity in &pause_q {
            commands.entity(entity).despawn();
        }
        commands.remove_resource::<results::PauseFocus>();
        if let Some(karaoke) = &karaoke {
            audio::resume_audio(karaoke, &mut audio_instances);
        }
        return;
    }
    if let Some(karaoke) = &karaoke {
        audio::pause_audio(karaoke, &mut audio_instances);
    }
    results::spawn_pause_overlay(&mut commands, &theme, &asset_server);
}

fn handle_guide_volume(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut karaoke: Option<ResMut<KaraokeAudio>>,
    mut config: ResMut<crate::config::AppConfig>,
    mut query: Query<&mut Text, With<GuideVolumeText>>,
    results_q: Query<(), With<ResultsOverlay>>,
    pause_q: Query<(), With<PauseOverlay>>,
) {
    if !results_q.is_empty() || !pause_q.is_empty() {
        return;
    }
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
    video_flavor: Res<ActiveVideoFlavor>,
    source_video: Option<Res<SourceVideoPath>>,
    mut config: ResMut<crate::config::AppConfig>,
    mut query: Query<&mut Text, (With<ThemeText>, Without<PixabayCreditText>)>,
    mut credit_query: Query<&mut Node, With<PixabayCreditText>>,
    mut commands: Commands,
    bg_query: Query<Entity, With<BackgroundQuad>>,
    video_query: Query<Entity, With<VideoSprite>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut shader_mats: (
        ResMut<Assets<PlasmaMaterial>>,
        ResMut<Assets<AuroraMaterial>>,
        ResMut<Assets<WavesMaterial>>,
        ResMut<Assets<NebulaMaterial>>,
        ResMut<Assets<StarfieldMaterial>>,
    ),
    karaoke: Option<Res<KaraokeAudio>>,
    audio_instances: Res<Assets<AudioInstance>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyT) {
        return;
    }

    despawn_background(&mut commands, &bg_query);
    video_bg::despawn_video_background(&mut commands, &video_query);

    let has_source_video = source_video.is_some();
    if has_source_video {
        theme.next();
    } else {
        theme.next_skip_source_video();
    }

    if theme.is_source_video() {
        if let Some(ref sv) = source_video {
            let t = karaoke.as_ref()
                .map(|k| audio::playback_time(k, &audio_instances))
                .unwrap_or(0.0);
            video_bg::spawn_source_video_background(
                &mut commands,
                &mut images,
                sv.0.clone(),
                t,
            );
        }
    } else if theme.is_pixabay_video() {
        video_bg::spawn_video_background(&mut commands, &mut images, video_flavor.flavor());
    } else {
        spawn_background(
            &mut commands,
            &mut meshes,
            &mut shader_mats.0,
            &mut shader_mats.1,
            &mut shader_mats.2,
            &mut shader_mats.3,
            &mut shader_mats.4,
            &theme,
        );
    }

    if !theme.is_source_video() {
        config.last_theme = Some(theme.index);
        config.save();
    }

    if let Ok(mut text) = query.single_mut() {
        **text = format_theme_text(&theme, &video_flavor);
    }

    if let Ok(mut node) = credit_query.single_mut() {
        node.display = if theme.is_pixabay_video() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn handle_flavor_switch(
    keyboard: Res<ButtonInput<KeyCode>>,
    theme: Res<ActiveTheme>,
    mut video_flavor: ResMut<ActiveVideoFlavor>,
    mut config: ResMut<crate::config::AppConfig>,
    mut query: Query<&mut Text, (With<ThemeText>, Without<PixabayCreditText>)>,
    mut video_bg: Option<ResMut<VideoBackground>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyF) || !theme.is_pixabay_video() {
        return;
    }

    video_flavor.next();

    if let Some(ref mut bg) = video_bg {
        video_bg::switch_flavor(bg, video_flavor.flavor());
    }

    config.last_video_flavor = Some(video_flavor.index);
    config.save();

    if let Ok(mut text) = query.single_mut() {
        **text = format_theme_text(&theme, &video_flavor);
    }
}

fn exit_playing(
    mut commands: Commands,
    audio: Res<bevy_kira_audio::Audio>,
    hud_query: Query<Entity, With<PlayerHud>>,
    lyrics_query: Query<Entity, With<LyricsRoot>>,
    bg_query: Query<Entity, With<BackgroundQuad>>,
    video_query: Query<Entity, With<VideoSprite>>,
    results_query: Query<Entity, With<ResultsOverlay>>,
    pause_query: Query<Entity, With<PauseOverlay>>,
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

    for entity in &results_query {
        commands.entity(entity).despawn();
    }
    for entity in &pause_query {
        commands.entity(entity).despawn();
    }

    video_bg::despawn_video_background(&mut commands, &video_query);

    commands.remove_resource::<SourceVideoPath>();
    commands.remove_resource::<microphone::MicrophoneCapture>();
    commands.remove_resource::<microphone::MicLoadTask>();
    commands.remove_resource::<scoring::VocalsBuffer>();
    commands.remove_resource::<scoring::VocalsLoadTask>();
    commands.remove_resource::<scoring::PitchState>();
    commands.remove_resource::<scoring::ScoringState>();
    commands.remove_resource::<results::SongResult>();
    commands.remove_resource::<results::PauseFocus>();
}
