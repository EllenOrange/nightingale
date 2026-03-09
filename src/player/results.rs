use bevy::prelude::*;
use bevy_kira_audio::{Audio, AudioControl};

use crate::profile::ProfileStore;
use crate::states::AppState;
use crate::ui::UiTheme;

const FA_STAR: &str = "\u{f005}";
const FA_STAR_HALF: &str = "\u{f5c0}";
const FA_EXCLAMATION_TRIANGLE: &str = "\u{f071}";

const OVERLAY_DIM: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);
const CARD_RADIUS: f32 = 12.0;
const CARD_PADDING: f32 = 28.0;
const BTN_RADIUS: f32 = 6.0;

#[derive(Resource, Clone)]
pub struct SongResult {
    pub song_title: String,
    pub song_artist: String,
    pub song_hash: String,
    pub score: u32,
}

#[derive(Component)]
pub struct ResultsOverlay;

#[derive(Component)]
pub struct PauseOverlay;

#[derive(Component)]
pub struct BackToMenuButton;

#[derive(Component)]
pub struct ContinueButton;

#[derive(Component)]
pub struct ExitToMenuButton;

#[derive(Resource)]
pub struct PauseFocus(pub usize);

fn half_stars(score: u32) -> u32 {
    (score as f64 / 100.0).round().min(10.0) as u32
}

fn ensure_celebration_sound() -> std::path::PathBuf {
    let cache = dirs::home_dir()
        .expect("no home dir")
        .join(".cache")
        .join("nightingale")
        .join("sounds");
    let _ = std::fs::create_dir_all(&cache);
    let path = cache.join("celebration.wav");
    if path.is_file() {
        return path;
    }

    let sample_rate = 44100u32;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("create wav");
    let notes = [523.25_f64, 659.25, 783.99];
    let note_samples = (sample_rate as f64 * 0.12) as usize;
    let fade_samples = (sample_rate as f64 * 0.01) as usize;
    for freq in notes {
        for i in 0..note_samples {
            let t = i as f64 / sample_rate as f64;
            let mut sample = (t * freq * 2.0 * std::f64::consts::PI).sin();
            if i < fade_samples {
                sample *= i as f64 / fade_samples as f64;
            }
            let tail = note_samples - i;
            if tail < fade_samples {
                sample *= tail as f64 / fade_samples as f64;
            }
            sample *= 0.4;
            writer
                .write_sample((sample * i16::MAX as f64) as i16)
                .ok();
        }
    }
    writer.finalize().ok();
    path
}

fn spawn_overlay_root(commands: &mut Commands, marker: impl Component) -> Entity {
    commands
        .spawn((
            marker,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(OVERLAY_DIM),
            GlobalZIndex(10),
        ))
        .id()
}

fn spawn_primary_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    marker: impl Component,
    theme: &UiTheme,
) {
    parent
        .spawn((
            marker,
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(10.0),
                    Val::Px(10.0),
                ),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.accent),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_secondary_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    marker: impl Component,
    theme: &UiTheme,
) {
    parent
        .spawn((
            marker,
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(10.0),
                    Val::Px(10.0),
                ),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.popup_btn),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.text_primary),
            ));
        });
}

pub fn spawn_results_overlay(
    commands: &mut Commands,
    result: &SongResult,
    profiles: &mut ProfileStore,
    theme: &UiTheme,
    asset_server: &AssetServer,
    audio: &Audio,
) {
    profiles.add_score(&result.song_hash, result.score);

    let celebration_path = ensure_celebration_sound();
    if let Ok(canon) = celebration_path.canonicalize() {
        let handle = asset_server.load(canon.to_string_lossy().to_string());
        audio.play(handle);
    }

    let hs = half_stars(result.score);
    let top = profiles.top_scores_for_song(&result.song_hash, 5);
    let active_profile = profiles.active.clone().unwrap_or_default();
    let icon_font: Handle<Font> = asset_server.load("fonts/fa-solid-900.ttf");

    let star_gold = Color::srgb(1.0, 0.84, 0.0);
    let star_empty = theme.accent.with_alpha(0.15);

    let root = spawn_overlay_root(commands, ResultsOverlay);
    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: Val::Px(420.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(CARD_PADDING)),
                    row_gap: Val::Px(4.0),
                    border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)),
                    ..default()
                },
                BackgroundColor(theme.surface),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(&result.song_title),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(theme.text_primary),
                ));
                card.spawn((
                    Text::new(&result.song_artist),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                ));

                card.spawn((
                    Text::new(format!("{}", result.score)),
                    TextFont {
                        font_size: 36.0,
                        ..default()
                    },
                    TextColor(theme.accent),
                    Node {
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                ));

                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                })
                .with_children(|row| {
                    let filled = hs / 2;
                    let has_half = hs % 2 == 1;
                    let empty = 5 - filled - if has_half { 1 } else { 0 };

                    for _ in 0..filled {
                        row.spawn((
                            Text::new(FA_STAR),
                            TextFont {
                                font: icon_font.clone(),
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(star_gold),
                        ));
                    }
                    if has_half {
                        row.spawn((
                            Text::new(FA_STAR_HALF),
                            TextFont {
                                font: icon_font.clone(),
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(star_gold),
                        ));
                    }
                    for _ in 0..empty {
                        row.spawn((
                            Text::new(FA_STAR),
                            TextFont {
                                font: icon_font.clone(),
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(star_empty),
                        ));
                    }
                });

                if !top.is_empty() {
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(1.0),
                            margin: UiRect::bottom(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(theme.text_dim.with_alpha(0.2)),
                    ));

                    card.spawn((
                        Text::new("BEST SCORES"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.text_dim),
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    card.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    })
                    .with_children(|board| {
                        for (i, (name, score)) in top.iter().enumerate() {
                            let is_current = *name == active_profile && *score == result.score;
                            let text_col = if is_current {
                                theme.accent
                            } else {
                                theme.text_primary
                            };
                            let bg_col = if is_current {
                                theme.accent.with_alpha(0.1)
                            } else {
                                Color::NONE
                            };

                            board
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        padding: UiRect::new(
                                            Val::Px(12.0),
                                            Val::Px(12.0),
                                            Val::Px(6.0),
                                            Val::Px(6.0),
                                        ),
                                        border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                                        ..default()
                                    },
                                    BackgroundColor(bg_col),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new(format!("{}. {}", i + 1, name)),
                                        TextFont {
                                            font_size: 13.0,
                                            ..default()
                                        },
                                        TextColor(text_col),
                                    ));
                                    row.spawn((
                                        Text::new(format!("{}", score)),
                                        TextFont {
                                            font_size: 13.0,
                                            ..default()
                                        },
                                        TextColor(text_col),
                                    ));
                                });
                        }
                    });
                }

                spawn_primary_btn(card, "Back to Menu", BackToMenuButton, theme);
            });
    });
}

pub fn handle_results_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut btn_query: Query<
        (&Interaction, &mut BackgroundColor),
        (With<BackToMenuButton>, Changed<Interaction>),
    >,
    overlay_query: Query<Entity, With<ResultsOverlay>>,
    theme: Res<UiTheme>,
) {
    if overlay_query.is_empty() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) || keyboard.just_pressed(KeyCode::Enter) {
        for entity in &overlay_query {
            commands.entity(entity).despawn();
        }
        next_state.set(AppState::Menu);
        return;
    }

    for (interaction, mut bg) in &mut btn_query {
        match interaction {
            Interaction::Pressed => {
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                next_state.set(AppState::Menu);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.accent_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.accent);
            }
        }
    }
}

pub fn spawn_pause_overlay(
    commands: &mut Commands,
    theme: &UiTheme,
    asset_server: &AssetServer,
) {
    commands.insert_resource(PauseFocus(0));
    let icon_font: Handle<Font> = asset_server.load("fonts/fa-solid-900.ttf");

    let root = spawn_overlay_root(commands, PauseOverlay);
    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: Val::Px(340.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(CARD_PADDING)),
                    row_gap: Val::Px(6.0),
                    border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)),
                    ..default()
                },
                BackgroundColor(theme.surface),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("Paused"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(theme.text_primary),
                    Node {
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                ));

                {
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Text::new(FA_EXCLAMATION_TRIANGLE),
                            TextFont {
                                font: icon_font.clone(),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(theme.badge_failed),
                        ));
                        row.spawn((
                            Text::new("Exiting now won't save your score"),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(theme.text_secondary),
                        ));
                    });
                }

                spawn_primary_btn(card, "Continue", ContinueButton, theme);
                spawn_secondary_btn(card, "Exit to Menu", ExitToMenuButton, theme);
            });
    });
}

pub fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut continue_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<ContinueButton>, Without<ExitToMenuButton>),
    >,
    mut exit_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<ExitToMenuButton>, Without<ContinueButton>),
    >,
    overlay_query: Query<Entity, With<PauseOverlay>>,
    karaoke: Option<ResMut<super::audio::KaraokeAudio>>,
    mut audio_instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>,
    theme: Res<UiTheme>,
    mut pause_focus: Option<ResMut<PauseFocus>>,
) {
    if overlay_query.is_empty() {
        return;
    }

    if let Some(ref mut pf) = pause_focus {
        if keyboard.just_pressed(KeyCode::ArrowDown) || keyboard.just_pressed(KeyCode::ArrowUp) {
            pf.0 = if pf.0 == 0 { 1 } else { 0 };
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            if pf.0 == 0 {
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                commands.remove_resource::<PauseFocus>();
                if let Some(karaoke) = &karaoke {
                    super::audio::resume_audio(karaoke, &mut audio_instances);
                }
                return;
            } else {
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                commands.remove_resource::<PauseFocus>();
                next_state.set(AppState::Menu);
                return;
            }
        }
    }

    let continue_focused = pause_focus.as_ref().is_some_and(|pf| pf.0 == 0);
    let exit_focused = pause_focus.as_ref().is_some_and(|pf| pf.0 == 1);

    for (interaction, mut bg, mut border) in &mut continue_query {
        match interaction {
            Interaction::Pressed => {
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                commands.remove_resource::<PauseFocus>();
                if let Some(karaoke) = &karaoke {
                    super::audio::resume_audio(karaoke, &mut audio_instances);
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.accent_hover);
                *border = BorderColor::all(theme.accent);
            }
            Interaction::None => {
                if continue_focused {
                    *bg = BackgroundColor(theme.accent_hover);
                    *border = BorderColor::all(theme.accent);
                } else {
                    *bg = BackgroundColor(theme.accent);
                    *border = BorderColor::all(Color::NONE);
                }
            }
        }
    }

    for (interaction, mut bg, mut border) in &mut exit_query {
        match interaction {
            Interaction::Pressed => {
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                commands.remove_resource::<PauseFocus>();
                next_state.set(AppState::Menu);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.popup_btn_hover);
                *border = BorderColor::all(theme.accent);
            }
            Interaction::None => {
                if exit_focused {
                    *bg = BackgroundColor(theme.popup_btn_hover);
                    *border = BorderColor::all(theme.accent);
                } else {
                    *bg = BackgroundColor(theme.popup_btn);
                    *border = BorderColor::all(Color::NONE);
                }
            }
        }
    }
}
