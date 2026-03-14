use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use super::components::*;
use super::song_card::SongCard;
use super::{AnyOverlayOpen, FocusPanel, MenuFocus, MenuState};
use crate::scanner::metadata::SongLibrary;
use crate::ui::UiTheme;

pub(super) fn handle_search_input(
    mut key_events: MessageReader<KeyboardInput>,
    mut menu_state: ResMut<MenuState>,
    mut search_text_query: Query<(&mut Text, &mut TextColor), With<SearchText>>,
    theme: Res<UiTheme>,
    library: Res<SongLibrary>,
    mut card_query: Query<(&SongCard, &mut Node)>,
    overlay_open: Res<AnyOverlayOpen>,
    mut focus: ResMut<MenuFocus>,
) {
    if overlay_open.0 {
        return;
    }
    let mut changed = false;

    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }

        if ev.key_code == KeyCode::Backspace {
            if !menu_state.search_query.is_empty() {
                menu_state.search_query.pop();
                changed = true;
            }
            continue;
        }

        if ev.key_code == KeyCode::Escape {
            if !menu_state.search_query.is_empty() {
                menu_state.search_query.clear();
                changed = true;
            }
            continue;
        }

        if let Some(ref text) = ev.text {
            for c in text.chars() {
                if !c.is_control() {
                    menu_state.search_query.push(c);
                    changed = true;
                }
            }
        }
    }

    if !changed {
        return;
    }

    if let Ok((mut text, mut color)) = search_text_query.single_mut() {
        if menu_state.search_query.is_empty() {
            **text = "Type to search songs...".into();
            *color = TextColor(theme.text_dim);
        } else {
            **text = menu_state.search_query.clone();
            *color = TextColor(theme.text_primary);
        }
    }

    let query = menu_state.search_query.to_lowercase();
    let mut first_visible: Option<usize> = None;
    let mut current_still_visible = false;

    for (card, mut node) in &mut card_query {
        let visible = if query.is_empty() {
            true
        } else if card.song_index < library.songs.len() {
            let song = &library.songs[card.song_index];
            song.display_title().to_lowercase().contains(&query)
                || song.display_artist().to_lowercase().contains(&query)
        } else {
            false
        };
        node.display = if visible { Display::Flex } else { Display::None };
        if visible {
            if first_visible.map_or(true, |f| card.song_index < f) {
                first_visible = Some(card.song_index);
            }
            if card.song_index == focus.song_index {
                current_still_visible = true;
            }
        }
    }

    if focus.panel == FocusPanel::SongList && !current_still_visible {
        if let Some(idx) = first_visible {
            focus.song_index = idx;
        }
    }
}
