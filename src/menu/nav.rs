use bevy::prelude::*;

use super::components::*;
use super::song_card::*;
use super::{AnyOverlayOpen, FocusPanel, MenuFocus, SIDEBAR_ACTIONS};
use crate::ui::UiTheme;

const NAV_INITIAL_DELAY: f32 = 0.4;
const NAV_REPEAT_RATE: f32 = 0.06;

#[derive(Resource)]
pub(super) struct NavRepeat {
    timer: Timer,
    started: bool,
}

impl Default for NavRepeat {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(NAV_INITIAL_DELAY, TimerMode::Once),
            started: false,
        }
    }
}

pub(super) fn clear_focus_on_empty_hover(
    mut cursor_events: MessageReader<bevy::window::CursorMoved>,
    mut focus: ResMut<MenuFocus>,
    card_query: Query<&Interaction, With<SongCard>>,
    sidebar_query: Query<&Interaction, With<SidebarButton>>,
    analyze_all_query: Query<&Interaction, With<AnalyzeAllButton>>,
) {
    if cursor_events.read().next().is_none() {
        return;
    }

    let any_hovered = card_query
        .iter()
        .chain(sidebar_query.iter())
        .chain(analyze_all_query.iter())
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));

    if !any_hovered {
        focus.active = false;
    }
}

pub(super) fn handle_menu_nav(
    keyboard: Res<ButtonInput<KeyCode>>,
    nav: Res<crate::input::NavInput>,
    mut focus: ResMut<MenuFocus>,
    card_query: Query<(&SongCard, &Node), Without<SidebarButton>>,
    overlay_open: Res<AnyOverlayOpen>,
    time: Res<Time>,
    mut nav_repeat: ResMut<NavRepeat>,
) {
    if overlay_open.0 {
        return;
    }

    let ud_just = nav.up || nav.down;
    let ud_held = nav.up_held || nav.down_held;

    let mut ud_step = false;
    if ud_just {
        nav_repeat.timer = Timer::from_seconds(NAV_INITIAL_DELAY, TimerMode::Once);
        nav_repeat.started = true;
        ud_step = true;
    } else if ud_held && nav_repeat.started {
        nav_repeat.timer.tick(time.delta());
        if nav_repeat.timer.just_finished() {
            nav_repeat.timer = Timer::from_seconds(NAV_REPEAT_RATE, TimerMode::Repeating);
            ud_step = true;
        }
    } else {
        nav_repeat.started = false;
    }

    if focus.nav_lock > 0 {
        focus.nav_lock -= 1;
    }

    let any_nav = ud_step
        || nav.left
        || nav.right
        || keyboard.just_pressed(KeyCode::Tab);
    if !any_nav {
        return;
    }

    focus.active = true;

    if nav.left {
        focus.panel = FocusPanel::Sidebar;
        focus.analyze_all_focused = false;
    } else if nav.right {
        focus.panel = FocusPanel::SongList;
    } else if keyboard.just_pressed(KeyCode::Tab) {
        focus.panel = if focus.panel == FocusPanel::SongList {
            focus.analyze_all_focused = false;
            FocusPanel::Sidebar
        } else {
            FocusPanel::SongList
        };
    }

    let step_down = ud_step && nav.down_held;
    let step_up = ud_step && nav.up_held;

    if step_down || step_up {
        match focus.panel {
            FocusPanel::SongList => {
                if focus.analyze_all_focused {
                    if step_down {
                        focus.analyze_all_focused = false;
                    }
                } else {
                    let mut visible: Vec<usize> = card_query
                        .iter()
                        .filter(|(_, node)| node.display != Display::None)
                        .map(|(card, _)| card.song_index)
                        .collect();
                    visible.sort();

                    if !visible.is_empty() {
                        let pos = visible.iter().position(|&i| i == focus.song_index);
                        if step_down {
                            focus.song_index = match pos {
                                Some(p) if p + 1 < visible.len() => visible[p + 1],
                                None => visible[0],
                                _ => focus.song_index,
                            };
                        }
                        if step_up {
                            match pos {
                                Some(0) | None => {
                                    focus.analyze_all_focused = true;
                                }
                                Some(p) => {
                                    focus.song_index = visible[p - 1];
                                }
                            }
                        }
                        focus.nav_lock = 2;
                    }
                }
            }
            FocusPanel::Sidebar => {
                if step_down {
                    focus.sidebar_index =
                        (focus.sidebar_index + 1).min(SIDEBAR_ACTIONS.len() - 1);
                }
                if step_up {
                    focus.sidebar_index = focus.sidebar_index.saturating_sub(1);
                }
            }
        }
    }
}

pub(super) fn apply_menu_focus_styling(
    focus: Res<MenuFocus>,
    mut card_query: Query<
        (&SongCard, &mut BackgroundColor, &mut BorderColor),
        (Without<SidebarButton>, Without<AnalyzeAllButton>),
    >,
    mut sidebar_query: Query<
        (&SidebarButton, &mut BackgroundColor, &mut BorderColor),
        (Without<SongCard>, Without<AnalyzeAllButton>),
    >,
    mut analyze_all_query: Query<
        &mut BorderColor,
        (With<AnalyzeAllButton>, Without<SongCard>, Without<SidebarButton>),
    >,
    theme: Res<UiTheme>,
    overlay_open: Res<AnyOverlayOpen>,
) {
    if !focus.is_changed() && !theme.is_changed() {
        return;
    }
    if overlay_open.0 {
        return;
    }
    for (card, mut bg, mut border) in &mut card_query {
        let is_focused = focus.active
            && focus.panel == FocusPanel::SongList
            && !focus.analyze_all_focused
            && card.song_index == focus.song_index;
        if is_focused {
            bg.set_if_neq(BackgroundColor(theme.card_hover));
            border.set_if_neq(BorderColor::all(theme.accent));
        } else {
            bg.set_if_neq(BackgroundColor(theme.card_bg));
            border.set_if_neq(BorderColor::all(Color::NONE));
        }
    }
    for (btn, mut bg, mut border) in &mut sidebar_query {
        let idx = SIDEBAR_ACTIONS.iter().position(|&a| a == btn.action);
        let is_focused =
            focus.active && focus.panel == FocusPanel::Sidebar && idx == Some(focus.sidebar_index);
        if is_focused {
            bg.set_if_neq(BackgroundColor(theme.sidebar_btn_hover));
            border.set_if_neq(BorderColor::all(theme.accent));
        } else {
            bg.set_if_neq(BackgroundColor(theme.sidebar_btn));
            border.set_if_neq(BorderColor::all(Color::NONE));
        }
    }
    let aa_focused = focus.active && focus.panel == FocusPanel::SongList && focus.analyze_all_focused;
    for mut border in &mut analyze_all_query {
        if aa_focused {
            border.set_if_neq(BorderColor::all(theme.accent));
        } else {
            border.set_if_neq(BorderColor::all(Color::NONE));
        }
    }
}

pub(super) fn scroll_to_focused(
    focus: Res<MenuFocus>,
    mut scroll_query: Query<(&mut ScrollPosition, &ComputedNode), With<SongListRoot>>,
    card_query: Query<(&SongCard, &Node, &ComputedNode)>,
) {
    if !focus.is_changed() || focus.panel != FocusPanel::SongList {
        return;
    }

    let Ok((mut scroll_pos, list_computed)) = scroll_query.single_mut() else {
        return;
    };

    let list_height = list_computed.size().y * list_computed.inverse_scale_factor();
    if list_height < 1.0 {
        return;
    }

    let gap = 8.0;
    let mut cards: Vec<(usize, f32)> = card_query
        .iter()
        .filter(|(_, node, _)| node.display != Display::None)
        .map(|(card, _, computed)| {
            (
                card.song_index,
                computed.size().y * computed.inverse_scale_factor(),
            )
        })
        .collect();
    cards.sort_by_key(|(idx, _)| *idx);

    let mut y = 0.0;
    for (idx, height) in &cards {
        if *idx == focus.song_index {
            if y < scroll_pos.y {
                scroll_pos.y = y;
            } else if y + height > scroll_pos.y + list_height {
                scroll_pos.y = y + height - list_height;
            }
            return;
        }
        y += height + gap;
    }
}
