use bevy::prelude::*;

use crate::analyzer::transcript::{Segment, Transcript};

#[derive(Resource)]
pub struct LyricsState {
    pub transcript: Transcript,
    pub current_segment: usize,
}

#[derive(Component)]
pub struct LyricsRoot;

#[derive(Component)]
pub struct CurrentLine;

#[derive(Component)]
pub struct NextLine;

#[derive(Component)]
pub struct LyricWord {
    pub segment_idx: usize,
    pub word_idx: usize,
}

const SUNG_COLOR: Color = Color::srgb(0.4, 0.75, 1.0);
const UNSUNG_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.95);
const NEXT_LINE_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);
const BACKDROP_CURRENT: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);
const BACKDROP_NEXT: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);

pub fn setup_lyrics(commands: &mut Commands, transcript: &Transcript) {
    let state = LyricsState {
        transcript: transcript.clone(),
        current_segment: usize::MAX,
    };

    commands
        .spawn((
            LyricsRoot,
            Node {
                width: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                bottom: Val::Px(60.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(40.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                CurrentLine,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(8.0),
                    padding: UiRect::new(
                        Val::Px(20.0),
                        Val::Px(20.0),
                        Val::Px(10.0),
                        Val::Px(10.0),
                    ),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));

            root.spawn((
                NextLine,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::new(
                        Val::Px(16.0),
                        Val::Px(16.0),
                        Val::Px(6.0),
                        Val::Px(6.0),
                    ),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
        });

    commands.insert_resource(state);
}

pub fn update_lyrics(
    mut lyrics: ResMut<LyricsState>,
    current_time: f64,
    current_line_query: Query<(Entity, &mut BackgroundColor), (With<CurrentLine>, Without<NextLine>)>,
    next_line_query: Query<(Entity, &mut BackgroundColor), (With<NextLine>, Without<CurrentLine>)>,
    mut word_query: Query<(&LyricWord, &mut TextColor)>,
    commands: &mut Commands,
) {
    if lyrics.transcript.segments.is_empty() {
        return;
    }

    let new_segment = find_current_segment(&lyrics.transcript.segments, current_time);

    if new_segment != lyrics.current_segment {
        lyrics.current_segment = new_segment;
        let segments = &lyrics.transcript.segments;
        rebuild_lines(
            new_segment,
            segments,
            current_line_query,
            next_line_query,
            commands,
        );
        return;
    }

    let segments = &lyrics.transcript.segments;
    for (lw, mut color) in &mut word_query {
        if lw.segment_idx < segments.len() && lw.word_idx < segments[lw.segment_idx].words.len() {
            let word = &segments[lw.segment_idx].words[lw.word_idx];
            if current_time >= word.end {
                *color = TextColor(SUNG_COLOR);
            } else if current_time >= word.start {
                let progress = (current_time - word.start) / (word.end - word.start);
                let r = 1.0 - (1.0 - 0.4) * progress as f32;
                let g = 1.0 - (1.0 - 0.75) * progress as f32;
                let b = 1.0;
                *color = TextColor(Color::srgb(r, g, b));
            } else {
                *color = TextColor(UNSUNG_COLOR);
            }
        }
    }
}

fn find_current_segment(segments: &[Segment], time: f64) -> usize {
    for (i, seg) in segments.iter().enumerate() {
        if time < seg.end + 0.5 {
            return i;
        }
    }
    segments.len().saturating_sub(1)
}

fn rebuild_lines(
    idx: usize,
    segments: &[Segment],
    mut current_line_query: Query<(Entity, &mut BackgroundColor), (With<CurrentLine>, Without<NextLine>)>,
    mut next_line_query: Query<(Entity, &mut BackgroundColor), (With<NextLine>, Without<CurrentLine>)>,
    commands: &mut Commands,
) {
    if let Ok((entity, mut bg)) = current_line_query.single_mut() {
        commands.entity(entity).despawn_children();
        if idx < segments.len() {
            *bg = BackgroundColor(BACKDROP_CURRENT);
            commands.entity(entity).with_children(|parent| {
                for (wi, word) in segments[idx].words.iter().enumerate() {
                    parent.spawn((
                        LyricWord {
                            segment_idx: idx,
                            word_idx: wi,
                        },
                        Text::new(&word.word),
                        TextFont {
                            font_size: 42.0,
                            ..default()
                        },
                        TextColor(UNSUNG_COLOR),
                    ));
                }
            });
        } else {
            *bg = BackgroundColor(Color::NONE);
        }
    }

    if let Ok((entity, mut bg)) = next_line_query.single_mut() {
        commands.entity(entity).despawn_children();
        let next_idx = idx + 1;
        if next_idx < segments.len() {
            *bg = BackgroundColor(BACKDROP_NEXT);
            commands.entity(entity).with_children(|parent| {
                for word in &segments[next_idx].words {
                    parent.spawn((
                        Text::new(&word.word),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(NEXT_LINE_COLOR),
                    ));
                }
            });
        } else {
            *bg = BackgroundColor(Color::NONE);
        }
    }
}
