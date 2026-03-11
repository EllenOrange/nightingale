use bevy::prelude::*;

use super::{IconFont, FA_REFRESH, FA_SPINNER};
use crate::scanner::metadata::Song;
use crate::ui::UiTheme;

#[derive(Component)]
pub struct SongCard {
    pub song_index: usize,
}

#[derive(Component)]
pub struct SongListRoot;

#[derive(Component)]
pub struct SearchText;

#[derive(Component)]
pub struct StatusBadge {
    pub song_index: usize,
}

#[derive(Component)]
pub struct BadgeText {
    pub song_index: usize,
}

#[derive(Component)]
pub struct StatsText;

#[derive(Component)]
pub struct AnalysisHint;

#[derive(Component)]
pub struct AlbumArtSlot;

#[derive(Component)]
pub struct SpinnerOverlay {
    pub song_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    RescanFolder,
    ChangeFolder,
    Settings,
    ToggleTheme,
    Profile,
    Exit,
}

#[derive(Component)]
pub struct ThemeToggleIcon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    ToggleFullscreen,
    ModelPrev,
    ModelNext,
    BeamUp,
    BeamDown,
    BatchUp,
    BatchDown,
    RestoreDefaults,
    Close,
}

#[derive(Component)]
pub struct SettingsOverlay;

#[derive(Component)]
pub struct SettingsButton {
    pub action: SettingsAction,
}

#[derive(Component)]
pub struct SettingsRow(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    Model,
    Beam,
    Batch,
    Fullscreen,
}

#[derive(Component)]
pub struct SettingsValueText(pub SettingsField);

#[derive(Component)]
pub struct SidebarButton {
    pub action: SidebarAction,
}

#[derive(Component)]
pub struct ReanalyzeButton {
    pub song_index: usize,
}

#[derive(Component)]
pub struct EmptyStateRoot;

#[derive(Component)]
pub struct ProfileOverlay;

#[derive(Component)]
pub struct ProfileNameInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    Create,
    Switch(usize),
    Delete(usize),
    ConfirmDelete,
    CancelDelete,
    NewProfile,
    Close,
}

#[derive(Component)]
pub struct ProfileButton {
    pub action: ProfileAction,
}

#[derive(Component)]
pub struct ProfileLabelText;

#[derive(Component)]
pub struct ProfileNameLabel;

use crate::scanner::metadata::{AnalysisStatus, TranscriptSource};

const FA_STAR: &str = "\u{f005}";
const FA_STAR_HALF: &str = "\u{f5c0}";

pub fn build_song_card(
    parent: &mut ChildSpawnerCommands,
    song: &Song,
    index: usize,
    art_handle: Option<Handle<Image>>,
    theme: &UiTheme,
    icon_font: &IconFont,
    visible: bool,
    best_score: Option<u32>,
) {
    let (badge_text, badge_color) = badge_info(&song.analysis_status, theme);
    let duration_str = format_duration(song.duration_secs);
    let display = if visible { Display::Flex } else { Display::None };

    parent
        .spawn((
            SongCard { song_index: index },
            Button,
            Node {
                display,
                width: Val::Percent(100.0),
                min_height: Val::Px(72.0),
                padding: UiRect::all(Val::Px(16.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                border: UiRect::left(Val::Px(3.0)),
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.card_bg),
        ))
        .with_children(|card| {
            spawn_album_art(card, index, art_handle, theme, icon_font);
            spawn_song_info(card, song, theme, best_score, icon_font);

            card.spawn((
                Text::new(duration_str),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.text_secondary),
                Node {
                    flex_shrink: 0.0,
                    margin: UiRect::right(Val::Px(12.0)),
                    ..default()
                },
            ));

            spawn_status_badge(card, index, badge_text, badge_color, theme);

            let reanalyze_vis = if matches!(
                song.analysis_status,
                AnalysisStatus::Ready(_) | AnalysisStatus::Failed(_)
            ) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            card.spawn((
                ReanalyzeButton { song_index: index },
                Button,
                Node {
                    flex_shrink: 0.0,
                    padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(6.0), Val::Px(6.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(theme.sidebar_btn),
                reanalyze_vis,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new(FA_REFRESH),
                    TextFont {
                        font: icon_font.0.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.text_primary),
                ));
            });
        });
}

fn spawn_mini_stars(
    parent: &mut ChildSpawnerCommands,
    score: u32,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    let half_stars = (score as f64 / 100.0).round().min(10.0) as u32;
    let filled = half_stars / 2;
    let has_half = half_stars % 2 == 1;
    let empty = 5 - filled - if has_half { 1 } else { 0 };

    let star_filled = theme.accent;
    let star_empty = theme.text_dim.with_alpha(0.2);

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|row| {
            for _ in 0..filled {
                row.spawn((
                    Text::new(FA_STAR),
                    TextFont {
                        font: icon_font.0.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(star_filled),
                ));
            }
            if has_half {
                row.spawn((
                    Text::new(FA_STAR_HALF),
                    TextFont {
                        font: icon_font.0.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(star_filled),
                ));
            }
            for _ in 0..empty {
                row.spawn((
                    Text::new(FA_STAR),
                    TextFont {
                        font: icon_font.0.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(star_empty),
                ));
            }
        });
}

fn badge_info<'a>(status: &AnalysisStatus, theme: &UiTheme) -> (&'a str, Color) {
    match status {
        AnalysisStatus::Ready(TranscriptSource::Lyrics) => ("LYRICS", theme.badge_lyrics),
        AnalysisStatus::Ready(TranscriptSource::Generated) => ("AI", theme.badge_ready),
        AnalysisStatus::NotAnalyzed => ("NOT ANALYZED", theme.badge_not_analyzed),
        AnalysisStatus::Queued => ("QUEUED", theme.badge_queued),
        AnalysisStatus::Analyzing => ("ANALYZING...", theme.badge_analyzing),
        AnalysisStatus::Failed(_) => ("FAILED", theme.badge_failed),
    }
}

fn spawn_album_art(
    card: &mut ChildSpawnerCommands,
    index: usize,
    art_handle: Option<Handle<Image>>,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    card.spawn((
        AlbumArtSlot,
        Node {
            width: Val::Px(48.0),
            height: Val::Px(48.0),
            ..default()
        },
    ))
    .with_children(|wrapper| {
        if let Some(handle) = art_handle {
            wrapper.spawn((
                ImageNode::new(handle),
                Node {
                    width: Val::Px(48.0),
                    height: Val::Px(48.0),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
            ));
        } else {
            wrapper
                .spawn((
                    Node {
                        width: Val::Px(48.0),
                        height: Val::Px(48.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface_hover),
                ))
                .with_children(|art| {
                    art.spawn((
                        Text::new("♪"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(theme.accent),
                    ));
                });
        }

        wrapper
            .spawn((
                SpinnerOverlay { song_index: index },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(48.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                Visibility::Hidden,
            ))
            .with_children(|overlay| {
                overlay.spawn((
                    Text::new(FA_SPINNER),
                    TextFont {
                        font: icon_font.0.clone(),
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(theme.accent),
                ));
            });
    });
}

fn spawn_song_info(
    card: &mut ChildSpawnerCommands,
    song: &Song,
    theme: &UiTheme,
    best_score: Option<u32>,
    icon_font: &IconFont,
) {
    card.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_shrink: 1.0,
        overflow: Overflow {
            x: OverflowAxis::Clip,
            y: OverflowAxis::Visible,
        },
        row_gap: Val::Px(4.0),
        ..default()
    })
    .with_children(|info| {
        info.spawn((
            Text::new(song.display_title()),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(theme.text_primary),
        ));
        info.spawn((
            Text::new(format!("{} · {}", song.display_artist(), song.album)),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(theme.text_secondary),
        ));
        if let Some(score) = best_score {
            spawn_mini_stars(info, score, theme, icon_font);
        }
    });
}

fn spawn_status_badge(
    card: &mut ChildSpawnerCommands,
    index: usize,
    text: &str,
    color: Color,
    theme: &UiTheme,
) {
    card.spawn((
        StatusBadge { song_index: index },
        Node {
            flex_shrink: 0.0,
            padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(4.0), Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(color),
    ))
    .with_children(|badge| {
        badge.spawn((
            BadgeText { song_index: index },
            Text::new(text),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(theme.text_primary),
        ));
    });
}

pub fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}

