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
const COUNTDOWN_GAP_THRESHOLD: f64 = 3.5;
const LYRICS_LEAD: f64 = 0.15;
const WORD_HIGHLIGHT_LEAD: f64 = 0.25;

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
            root.spawn((
                CurrentLine,
                Node {
                    flex_shrink: 0.0,
                    max_width: Val::Percent(100.0),
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
            ))
            .with_children(|cl| {
                cl.spawn((
                    CountdownNode,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-36.0),
                        left: Val::Px(-36.0),
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
            });

            root.spawn((
                NextLine,
                Node {
                    flex_shrink: 0.0,
                    max_width: Val::Percent(100.0),
                    padding: UiRect::new(Val::Px(16.0), Val::Px(16.0), Val::Px(6.0), Val::Px(6.0)),
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

    let seg_idx = find_current_segment(
        &lyrics.transcript.segments,
        current_time,
        lyrics.current_segment,
    );

    if seg_idx != lyrics.current_segment {
        lyrics.current_segment = seg_idx;
        let segments = &lyrics.transcript.segments;
        rebuild_lines(
            seg_idx,
            segments,
            &current_line_query,
            &next_line_query,
            commands,
            theme,
        );
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
    let show_countdown = gap_before >= COUNTDOWN_GAP_THRESHOLD
        && time_until > 0.0
        && time_until <= COUNTDOWN_DURATION;

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
    let (sung_r, sung_g, sung_b) = extract_rgb(sung);

    for (lw, mut color) in &mut word_query {
        if lw.segment_idx < segments.len() && lw.word_idx < segments[lw.segment_idx].words.len() {
            let word = &segments[lw.segment_idx].words[lw.word_idx];
            let unsung = if word.estimated {
                theme.unsung_estimated
            } else {
                theme.unsung_color
            };
            let w_start = word.start - WORD_HIGHLIGHT_LEAD;
            let w_end = word.end - WORD_HIGHLIGHT_LEAD;
            if current_time >= w_end {
                *color = TextColor(sung);
            } else if current_time >= w_start {
                let progress = (current_time - w_start) / (w_end - w_start);
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

fn find_current_segment(segments: &[Segment], time: f64, hint: usize) -> usize {
    let start = if hint < segments.len() && time >= segments[hint].start - LYRICS_LEAD {
        hint
    } else {
        0
    };
    for i in start..segments.len() {
        let seg = &segments[i];
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
        let has_words = idx < segments.len() && !segments[idx].words.is_empty();
        commands.entity(entity).with_children(|parent| {
            parent
                .spawn((
                    CountdownNode,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-36.0),
                        left: Val::Px(-36.0),
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
            if has_words {
                let words = &segments[idx].words;
                let first = &words[0];
                let first_color = if first.estimated {
                    theme.unsung_estimated
                } else {
                    theme.unsung_color
                };
                let first_text = spaced_word(&first.word, words.len() > 1);
                parent
                    .spawn((
                        LyricWord {
                            segment_idx: idx,
                            word_idx: 0,
                        },
                        Text::new(first_text),
                        TextFont {
                            font_size: 42.0,
                            ..default()
                        },
                        TextColor(first_color),
                        TextLayout {
                            justify: Justify::Center,
                            linebreak: LineBreak::WordBoundary,
                        },
                    ))
                    .with_children(|tp| {
                        for (wi, word) in words.iter().enumerate().skip(1) {
                            let unsung = if word.estimated {
                                theme.unsung_estimated
                            } else {
                                theme.unsung_color
                            };
                            tp.spawn((
                                LyricWord {
                                    segment_idx: idx,
                                    word_idx: wi,
                                },
                                TextSpan::new(spaced_word(
                                    &word.word,
                                    wi < words.len() - 1,
                                )),
                                TextFont {
                                    font_size: 42.0,
                                    ..default()
                                },
                                TextColor(unsung),
                            ));
                        }
                    });
            }
        });
    }

    if let Ok((entity, _, _)) = next_line_query.single() {
        commands.entity(entity).despawn_children();
        let next_idx = idx + 1;
        if next_idx < segments.len() && !segments[next_idx].words.is_empty() {
            let words = &segments[next_idx].words;
            commands.entity(entity).with_children(|parent| {
                let first = &words[0];
                let first_col = if first.estimated {
                    let est = theme.unsung_estimated.to_srgba();
                    Color::srgba(est.red, est.green, est.blue, 0.35)
                } else {
                    theme.next_line_color
                };
                let first_text = spaced_word(&first.word, words.len() > 1);
                parent
                    .spawn((
                        Text::new(first_text),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(first_col),
                        TextLayout {
                            justify: Justify::Center,
                            linebreak: LineBreak::WordBoundary,
                        },
                    ))
                    .with_children(|tp| {
                        for (wi, word) in words.iter().enumerate().skip(1) {
                            let col = if word.estimated {
                                let est = theme.unsung_estimated.to_srgba();
                                Color::srgba(est.red, est.green, est.blue, 0.35)
                            } else {
                                theme.next_line_color
                            };
                            tp.spawn((
                                TextSpan::new(spaced_word(
                                    &word.word,
                                    wi < words.len() - 1,
                                )),
                                TextFont {
                                    font_size: 28.0,
                                    ..default()
                                },
                                TextColor(col),
                            ));
                        }
                    });
            });
        }
    }
}

fn spaced_word(w: &str, trailing_space: bool) -> String {
    if trailing_space {
        format!("{w} ")
    } else {
        w.to_string()
    }
}
