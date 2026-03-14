use bevy::prelude::*;

use super::components::*;
use super::{IconFont, FA_REFRESH};
use crate::scanner::metadata::Song;
use crate::ui::layout::{ART_SIZE, CARD_MIN_HEIGHT, OVERLAY_WIDTH_SM};
use crate::ui::UiTheme;

#[derive(Component)]
pub struct SongCard {
    pub song_index: usize,
}

#[derive(Component)]
pub struct SongListRoot;

#[derive(Component)]
pub struct StatusBadge {
    pub song_index: usize,
}

#[derive(Component)]
pub struct BadgeText {
    pub song_index: usize,
}

#[derive(Component)]
pub struct AlbumArtSlot;

#[derive(Component)]
pub struct SpinnerOverlay {
    pub song_index: usize,
}

#[derive(Component)]
pub struct ReanalyzeButton {
    pub song_index: usize,
}

#[derive(Component)]
pub struct DeleteCacheButton {
    pub song_index: usize,
}

#[derive(Component)]
pub struct LanguageButton {
    pub song_index: usize,
}

#[derive(Component)]
pub struct LanguageText {
    pub song_index: usize,
}

#[derive(Component)]
pub struct LanguageBadgeInner {
    pub song_index: usize,
}

use crate::scanner::metadata::{AnalysisStatus, TranscriptSource};

const FA_STAR: &str = "\u{f005}";
const FA_STAR_HALF: &str = "\u{f5c0}";
const FA_GLOBE: &str = "\u{f0ac}";
const FA_TRASH: &str = "\u{f1f8}";
const FA_FILM: &str = "\u{f008}";

pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("ru", "Russian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh", "Chinese"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("sv", "Swedish"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("cs", "Czech"),
    ("ro", "Romanian"),
    ("hu", "Hungarian"),
];

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
    let display = if visible { Display::Flex } else { Display::None };

    parent
        .spawn((
            SongCard { song_index: index },
            Button,
            Node {
                display,
                width: Val::Percent(100.0),
                min_height: Val::Px(CARD_MIN_HEIGHT),
                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(10.0), Val::Px(10.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.card_bg),
        ))
        .with_children(|card| {
            spawn_album_art(card, index, art_handle, song.is_video, theme, icon_font);
            spawn_song_info(card, song, theme, best_score, icon_font, index);
            spawn_right_column(card, index, song, badge_text, badge_color, theme, icon_font);
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
            flex_shrink: 0.0,
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

fn spawn_video_badge(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    let text_color = theme.badge_video;

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            margin: UiRect::bottom(Val::Px(1.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(FA_FILM),
                TextFont {
                    font: icon_font.0.clone(),
                    font_size: 9.0,
                    ..default()
                },
                TextColor(text_color),
            ));
            row.spawn((
                Text::new("VIDEO"),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(text_color),
            ));
        });
}

fn badge_info<'a>(status: &AnalysisStatus, theme: &UiTheme) -> (&'a str, Color) {
    match status {
        AnalysisStatus::Ready(TranscriptSource::Lyrics) => ("LYRICS", theme.badge_lyrics),
        AnalysisStatus::Ready(TranscriptSource::Generated) => ("TRANSCRIPT", theme.badge_ready),
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
    is_video: bool,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    card.spawn((
        AlbumArtSlot,
        Node {
            width: Val::Px(ART_SIZE),
            height: Val::Px(ART_SIZE),
            flex_shrink: 0.0,
            ..default()
        },
    ))
    .with_children(|wrapper| {
        if let Some(handle) = art_handle {
            wrapper.spawn((
                ImageNode::new(handle),
                Node {
                    width: Val::Px(ART_SIZE),
                    height: Val::Px(ART_SIZE),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
            ));
        } else if is_video {
            wrapper
                .spawn((
                    Node {
                        width: Val::Px(ART_SIZE),
                        height: Val::Px(ART_SIZE),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface_hover),
                ))
                .with_children(|art| {
                    art.spawn((
                        Text::new(FA_FILM),
                        TextFont {
                            font: icon_font.0.clone(),
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(theme.accent),
                    ));
                });
        } else {
            wrapper
                .spawn((
                    Node {
                        width: Val::Px(ART_SIZE),
                        height: Val::Px(ART_SIZE),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(4.0)),
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
                    width: Val::Px(ART_SIZE),
                    height: Val::Px(ART_SIZE),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(theme.spinner_overlay),
                Visibility::Hidden,
            ));
    });
}

fn spawn_song_info(
    card: &mut ChildSpawnerCommands,
    song: &Song,
    theme: &UiTheme,
    best_score: Option<u32>,
    icon_font: &IconFont,
    index: usize,
) {
    card.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_shrink: 1.0,
        overflow: Overflow {
            x: OverflowAxis::Clip,
            y: OverflowAxis::Visible,
        },
        row_gap: Val::Px(3.0),
        ..default()
    })
    .with_children(|info| {
        if song.is_video {
            spawn_video_badge(info, theme, icon_font);
        }

        info.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|title_row| {
            title_row.spawn((
                Text::new(song.display_title()),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(theme.text_primary),
                Node {
                    flex_shrink: 1.0,
                    overflow: Overflow {
                        x: OverflowAxis::Clip,
                        ..default()
                    },
                    ..default()
                },
            ));
            if let Some(score) = best_score {
                spawn_mini_stars(title_row, score, theme, icon_font);
            }
        });

        info.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(0.0),
            ..default()
        })
        .with_children(|sub_row| {
            let duration_str = format_duration(song.duration_secs);
            let mut subtitle = format!("{} · {}", song.display_artist(), song.album);
            if !subtitle.is_empty() {
                subtitle.push_str(" · ");
            }
            subtitle.push_str(&duration_str);

            sub_row.spawn((
                Text::new(&subtitle),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme.text_secondary),
                Node {
                    flex_shrink: 1.0,
                    ..default()
                },
            ));

            let lang_display = song
                .language
                .as_deref()
                .map(|l| l.to_uppercase())
                .unwrap_or_default();
            let lang_vis = if song.language.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            sub_row
                .spawn((
                    LanguageButton { song_index: index },
                    Button,
                    Node {
                        flex_shrink: 0.0,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    lang_vis,
                ))
                .with_children(|lang_wrapper| {
                    lang_wrapper.spawn((
                        Text::new(" · "),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(theme.text_dim),
                    ));
                    lang_wrapper
                        .spawn((
                            LanguageBadgeInner { song_index: index },
                            Node {
                                padding: UiRect::new(
                                    Val::Px(5.0),
                                    Val::Px(5.0),
                                    Val::Px(1.0),
                                    Val::Px(1.0),
                                ),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(3.0),
                                ..default()
                            },
                            BorderColor::all(theme.accent.with_alpha(0.4)),
                            BackgroundColor(theme.accent.with_alpha(0.1)),
                        ))
                        .with_children(|badge| {
                            badge.spawn((
                                Text::new(FA_GLOBE),
                                TextFont {
                                    font: icon_font.0.clone(),
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(theme.accent),
                            ));
                            badge.spawn((
                                LanguageText { song_index: index },
                                Text::new(lang_display),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(theme.accent),
                            ));
                        });
                });
        });
    });
}

fn spawn_right_column(
    card: &mut ChildSpawnerCommands,
    index: usize,
    song: &Song,
    badge_text: &str,
    badge_color: Color,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    let action_vis = if matches!(
        song.analysis_status,
        AnalysisStatus::Ready(_) | AnalysisStatus::Failed(_)
    ) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    card.spawn(Node {
        flex_direction: FlexDirection::Row,
        flex_shrink: 0.0,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            DeleteCacheButton { song_index: index },
            Button,
            Node {
                flex_shrink: 0.0,
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.surface_hover),
            BorderColor::all(theme.text_dim.with_alpha(0.2)),
            action_vis,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(FA_TRASH),
                TextFont {
                    font: icon_font.0.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme.text_dim),
            ));
        });

        row.spawn((
            ReanalyzeButton { song_index: index },
            Button,
            Node {
                flex_shrink: 0.0,
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.surface_hover),
            BorderColor::all(theme.text_dim.with_alpha(0.2)),
            action_vis,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(FA_REFRESH),
                TextFont {
                    font: icon_font.0.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme.text_dim),
            ));
        });

        spawn_status_badge(row, index, badge_text, badge_color, theme);
    });
}

fn spawn_status_badge(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    text: &str,
    color: Color,
    theme: &UiTheme,
) {
    parent
        .spawn((
            StatusBadge { song_index: index },
            Node {
                flex_shrink: 0.0,
                padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(3.0), Val::Px(3.0)),
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
                    font_size: 10.0,
                    ..default()
                },
                TextColor(theme.text_primary),
            ));
        });
}

pub fn spawn_language_picker(
    commands: &mut Commands,
    song_index: usize,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    commands.insert_resource(LanguagePickerTarget { song_index });

    commands
        .spawn((
            LanguagePickerOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(10),
            BackgroundColor(theme.overlay_dim),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(OVERLAY_WIDTH_SM),
                        max_height: Val::Percent(70.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn((
                            Text::new("Select Language"),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(theme.text_primary),
                        ));
                        header
                            .spawn((
                                LanguagePickerClose,
                                Button,
                                Node {
                                    padding: UiRect::new(
                                        Val::Px(8.0),
                                        Val::Px(8.0),
                                        Val::Px(4.0),
                                        Val::Px(4.0),
                                    ),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(theme.popup_btn),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("✕"),
                                    TextFont {
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(theme.text_secondary),
                                ));
                            });
                    });

                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        row_gap: Val::Px(2.0),
                        flex_grow: 1.0,
                        ..default()
                    })
                    .with_children(|list| {
                        for &(code, name) in LANGUAGES {
                            list.spawn((
                                LanguagePickerItem {
                                    lang_code: code.to_string(),
                                    song_index,
                                },
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::new(
                                        Val::Px(12.0),
                                        Val::Px(12.0),
                                        Val::Px(8.0),
                                        Val::Px(8.0),
                                    ),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(10.0),
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                            ))
                            .with_children(|row| {
                                row.spawn((
                                    Text::new(FA_GLOBE),
                                    TextFont {
                                        font: icon_font.0.clone(),
                                        font_size: 13.0,
                                        ..default()
                                    },
                                    TextColor(theme.text_dim),
                                ));
                                row.spawn((
                                    Text::new(code.to_uppercase()),
                                    TextFont {
                                        font_size: 13.0,
                                        ..default()
                                    },
                                    TextColor(theme.accent),
                                    Node {
                                        min_width: Val::Px(28.0),
                                        ..default()
                                    },
                                ));
                                row.spawn((
                                    Text::new(name),
                                    TextFont {
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(theme.text_primary),
                                ));
                            });
                        }
                    });
                });
        });
}

pub fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}

