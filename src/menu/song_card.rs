use bevy::prelude::*;

use crate::scanner::metadata::Song;
use crate::ui;

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
pub struct AlbumArtSlot;

#[derive(Component)]
pub struct SpinnerOverlay {
    pub song_index: usize,
}

#[derive(Component)]
pub struct SpinnerDotText;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    RescanFolder,
    ChangeFolder,
    Exit,
}

#[derive(Component)]
pub struct SidebarButton {
    pub action: SidebarAction,
}

pub const CARD_COLOR: Color = Color::srgb(0.14, 0.14, 0.20);
pub const CARD_HOVER: Color = Color::srgb(0.20, 0.20, 0.28);
pub const BADGE_READY: Color = Color::srgb(0.2, 0.7, 0.3);
pub const BADGE_NOT_ANALYZED: Color = Color::srgb(0.5, 0.5, 0.55);
pub const BADGE_QUEUED: Color = Color::srgb(0.7, 0.55, 0.1);
pub const BADGE_ANALYZING: Color = Color::srgb(0.9, 0.7, 0.1);
pub const BADGE_FAILED: Color = Color::srgb(0.8, 0.2, 0.2);
pub const SIDEBAR_BG: Color = Color::srgb(0.06, 0.06, 0.09);
pub const SIDEBAR_BTN: Color = Color::srgb(0.14, 0.14, 0.20);
pub const SIDEBAR_BTN_HOVER: Color = Color::srgb(0.22, 0.22, 0.30);

use crate::scanner::metadata::AnalysisStatus;

pub fn build_song_card(
    parent: &mut ChildSpawnerCommands,
    song: &Song,
    index: usize,
    art_handle: Option<Handle<Image>>,
) {
    let (badge_text, badge_color) = badge_info(&song.analysis_status);
    let duration_str = format_duration(song.duration_secs);

    parent
        .spawn((
            SongCard { song_index: index },
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(72.0),
                padding: UiRect::all(Val::Px(16.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(CARD_COLOR),
        ))
        .with_children(|card| {
            spawn_album_art(card, index, art_handle);
            spawn_song_info(card, song);

            card.spawn((
                Text::new(duration_str),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(ui::TEXT_SECONDARY),
                Node {
                    margin: UiRect::right(Val::Px(12.0)),
                    ..default()
                },
            ));

            spawn_status_badge(card, index, badge_text, badge_color);
        });
}

fn badge_info(status: &AnalysisStatus) -> (&'static str, Color) {
    match status {
        AnalysisStatus::Ready => ("READY", BADGE_READY),
        AnalysisStatus::NotAnalyzed => ("NOT ANALYZED", BADGE_NOT_ANALYZED),
        AnalysisStatus::Queued => ("QUEUED", BADGE_QUEUED),
        AnalysisStatus::Analyzing => ("ANALYZING...", BADGE_ANALYZING),
        AnalysisStatus::Failed(_) => ("FAILED", BADGE_FAILED),
    }
}

fn spawn_album_art(card: &mut ChildSpawnerCommands, index: usize, art_handle: Option<Handle<Image>>) {
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
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.28)),
                ))
                .with_children(|art| {
                    art.spawn((
                        Text::new("♪"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(ui::ACCENT),
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
                    SpinnerDotText,
                    Text::new("."),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(ui::ACCENT),
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                ));
            });
    });
}

fn spawn_song_info(card: &mut ChildSpawnerCommands, song: &Song) {
    card.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
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
            TextColor(ui::TEXT_PRIMARY),
        ));
        info.spawn((
            Text::new(format!("{} · {}", song.display_artist(), song.album)),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(ui::TEXT_SECONDARY),
        ));
    });
}

fn spawn_status_badge(card: &mut ChildSpawnerCommands, index: usize, text: &str, color: Color) {
    card.spawn((
        StatusBadge { song_index: index },
        Node {
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
            TextColor(ui::TEXT_PRIMARY),
        ));
    });
}

pub fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}

pub fn populate_song_list(
    commands: &mut Commands,
    list_entity: Entity,
    songs: &[Song],
    query: &str,
    art_handles: &[Option<Handle<Image>>],
) {
    commands.entity(list_entity).despawn_children();
    let lower = query.to_lowercase();
    commands.entity(list_entity).with_children(|list| {
        for (i, song) in songs.iter().enumerate() {
            if !lower.is_empty() {
                let matches = song.display_title().to_lowercase().contains(&lower)
                    || song.display_artist().to_lowercase().contains(&lower);
                if !matches {
                    continue;
                }
            }
            let art = art_handles.get(i).and_then(|h| h.clone());
            build_song_card(list, song, i, art);
        }
    });
}
