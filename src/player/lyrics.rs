use bevy::prelude::*;

use crate::analyzer::transcript::{Segment, Transcript};
use crate::ui::UiTheme;

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
pub struct CountdownNode;

#[derive(Component)]
pub struct LyricWord {
    pub segment_idx: usize,
    pub word_idx: usize,
}

const COUNTDOWN_DURATION: f64 = 3.0;
const COUNTDOWN_GAP_THRESHOLD: f64 = 5.0;
const LYRICS_LEAD: f64 = 0.15;

pub fn setup_lyrics(commands: &mut Commands, transcript: &Transcript, theme: &UiTheme) {
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
            root.spawn(Node::default())
            .with_children(|wrapper| {
                wrapper.spawn((
                    CountdownNode,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-20.0),
                        left: Val::Px(-20.0),
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Visibility::Hidden,
                    ZIndex(1),
                ))
                .with_children(|cd| {
                    cd.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(theme.countdown_color),
                    ));
                });

                wrapper.spawn((
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
                    Visibility::Hidden,
                ));
            });

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
                Visibility::Hidden,
            ));
        });

    commands.insert_resource(state);
}

pub fn update_lyrics(
    mut lyrics: ResMut<LyricsState>,
    current_time: f64,
    mut current_line_query: Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<CurrentLine>, Without<NextLine>, Without<CountdownNode>),
    >,
    mut next_line_query: Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<NextLine>, Without<CurrentLine>, Without<CountdownNode>),
    >,
    mut countdown_query: Query<
        (&mut Visibility, &mut BackgroundColor, &Children),
        (With<CountdownNode>, Without<CurrentLine>, Without<NextLine>),
    >,
    mut countdown_text_query: Query<&mut Text, Without<LyricWord>>,
    mut word_query: Query<(&LyricWord, &mut TextColor)>,
    commands: &mut Commands,
    theme: &UiTheme,
) {
    if lyrics.transcript.segments.is_empty() {
        return;
    }

    let seg_idx = find_current_segment(&lyrics.transcript.segments, current_time);

    if seg_idx != lyrics.current_segment {
        lyrics.current_segment = seg_idx;
        let segments = &lyrics.transcript.segments;
        rebuild_lines(seg_idx, segments, &current_line_query, &next_line_query, commands, theme);
    }

    let segments = &lyrics.transcript.segments;
    let seg = &segments[seg_idx];
    let active = current_time >= seg.start - LYRICS_LEAD && current_time <= seg.end + 0.5;

    let gap_before = if seg_idx == 0 {
        seg.start
    } else {
        seg.start - segments[seg_idx - 1].end
    };
    let time_until = seg.start - current_time;
    let show_countdown =
        gap_before >= COUNTDOWN_GAP_THRESHOLD && time_until > 0.0 && time_until <= COUNTDOWN_DURATION;

    let show_current = active || show_countdown;

    let next_exists = seg_idx + 1 < segments.len();
    let show_next = show_current && next_exists;

    if let Ok((_, mut bg, mut vis)) = current_line_query.single_mut() {
        if show_current {
            *vis = Visibility::Inherited;
            *bg = BackgroundColor(theme.lyric_backdrop);
        } else {
            *vis = Visibility::Hidden;
            *bg = BackgroundColor(Color::NONE);
        }
    }

    if let Ok((_, mut bg, mut vis)) = next_line_query.single_mut() {
        if show_next {
            *vis = Visibility::Inherited;
            *bg = BackgroundColor(theme.lyric_backdrop_next);
        } else {
            *vis = Visibility::Hidden;
            *bg = BackgroundColor(Color::NONE);
        }
    }

    if let Ok((mut vis, mut bg, children)) = countdown_query.single_mut() {
        if show_countdown {
            let n = time_until.ceil() as i32;
            *vis = Visibility::Inherited;
            *bg = BackgroundColor(theme.countdown_bg);
            for child in children.iter() {
                if let Ok(mut text) = countdown_text_query.get_mut(child) {
                    **text = format!("{n}");
                }
            }
        } else {
            *vis = Visibility::Hidden;
            *bg = BackgroundColor(Color::NONE);
        }
    }

    if !active {
        return;
    }

    let sung = theme.sung_color;
    let unsung = theme.unsung_color;

    let (sung_r, sung_g, sung_b) = extract_rgb(sung);

    for (lw, mut color) in &mut word_query {
        if lw.segment_idx < segments.len() && lw.word_idx < segments[lw.segment_idx].words.len() {
            let word = &segments[lw.segment_idx].words[lw.word_idx];
            if current_time >= word.end {
                *color = TextColor(sung);
            } else if current_time >= word.start {
                let progress = (current_time - word.start) / (word.end - word.start);
                let (ur, ug, ub) = extract_rgb(unsung);
                let r = ur + (sung_r - ur) * progress as f32;
                let g = ug + (sung_g - ug) * progress as f32;
                let b = ub + (sung_b - ub) * progress as f32;
                *color = TextColor(Color::srgb(r, g, b));
            } else {
                *color = TextColor(unsung);
            }
        }
    }
}

fn extract_rgb(color: Color) -> (f32, f32, f32) {
    let srgba = color.to_srgba();
    (srgba.red, srgba.green, srgba.blue)
}

pub fn last_segment_end(lyrics: &LyricsState) -> f64 {
    lyrics
        .transcript
        .segments
        .last()
        .map(|s| s.end)
        .unwrap_or(0.0)
}

pub fn first_segment_start(lyrics: &LyricsState) -> f64 {
    lyrics
        .transcript
        .segments
        .first()
        .map(|s| s.start)
        .unwrap_or(0.0)
}

fn find_current_segment(segments: &[Segment], time: f64) -> usize {
    for (i, seg) in segments.iter().enumerate() {
        if time < seg.end + 0.5 {
            if i + 1 < segments.len() && time >= segments[i + 1].start - LYRICS_LEAD {
                return i + 1;
            }
            return i;
        }
    }
    segments.len().saturating_sub(1)
}

fn rebuild_lines(
    idx: usize,
    segments: &[Segment],
    current_line_query: &Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<CurrentLine>, Without<NextLine>, Without<CountdownNode>),
    >,
    next_line_query: &Query<
        (Entity, &mut BackgroundColor, &mut Visibility),
        (With<NextLine>, Without<CurrentLine>, Without<CountdownNode>),
    >,
    commands: &mut Commands,
    theme: &UiTheme,
) {
    if let Ok((entity, _, _)) = current_line_query.single() {
        commands.entity(entity).despawn_children();
        if idx < segments.len() {
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
                        TextColor(theme.unsung_color),
                    ));
                }
            });
        }
    }

    if let Ok((entity, _, _)) = next_line_query.single() {
        commands.entity(entity).despawn_children();
        let next_idx = idx + 1;
        if next_idx < segments.len() {
            commands.entity(entity).with_children(|parent| {
                for word in &segments[next_idx].words {
                    parent.spawn((
                        Text::new(&word.word),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(theme.next_line_color),
                    ));
                }
            });
        }
    }
}
