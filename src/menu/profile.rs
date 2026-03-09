use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use super::song_card::*;
use crate::profile::ProfileStore;
use crate::states::AppState;
use crate::ui::{self, UiTheme};

const OVERLAY_DIM: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);
const CARD_WIDTH: f32 = 360.0;
const CARD_RADIUS: f32 = 12.0;
const CARD_PADDING: f32 = 24.0;
const BTN_RADIUS: f32 = 6.0;
const ITEM_RADIUS: f32 = 6.0;
const FA_TRASH: &str = "\u{f2ed}";

#[derive(Resource, Default)]
pub struct ProfileInputState {
    pub text: String,
}

#[derive(Resource)]
pub struct PendingDeleteProfile {
    pub name: String,
}

pub fn spawn_profile_popup(
    commands: &mut Commands,
    theme: &UiTheme,
    profiles: &ProfileStore,
    asset_server: &AssetServer,
) {
    commands.insert_resource(ProfileInputState::default());
    let icon_font: Handle<Font> = asset_server.load("fonts/fa-solid-900.ttf");

    commands
        .spawn((
            ProfileOverlay,
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
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(CARD_WIDTH),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(CARD_PADDING)),
                        row_gap: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    if profiles.active.is_some() {
                        build_profile_list(card, theme, profiles, &icon_font);
                    } else {
                        build_create_form(card, theme, "Create Profile", false);
                    }
                });
        });
}

fn build_profile_list(
    card: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    profiles: &ProfileStore,
    icon_font: &Handle<Font>,
) {
    card.spawn((
        Text::new("Profile"),
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

    card.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        ..default()
    })
    .with_children(|list| {
        for (i, name) in profiles.profiles.iter().enumerate() {
            let is_active = profiles.active.as_deref() == Some(name.as_str());

            list.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ProfileButton {
                        action: ProfileAction::Switch(i),
                    },
                    Button,
                    Node {
                        flex_grow: 1.0,
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(8.0),
                            Val::Px(8.0),
                        ),
                        border_radius: BorderRadius::all(Val::Px(ITEM_RADIUS)),
                        border: UiRect::left(if is_active {
                            Val::Px(3.0)
                        } else {
                            Val::ZERO
                        }),
                        ..default()
                    },
                    BorderColor::all(if is_active { theme.accent } else { Color::NONE }),
                    BackgroundColor(theme.popup_btn),
                ))
                .with_children(|name_btn| {
                    let text_col = if is_active {
                        theme.accent
                    } else {
                        theme.text_primary
                    };
                    name_btn.spawn((
                        Text::new(name.as_str()),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(text_col),
                    ));
                });

                row.spawn((
                    ProfileButton {
                        action: ProfileAction::Delete(i),
                    },
                    Button,
                    Node {
                        width: Val::Px(34.0),
                        flex_shrink: 0.0,
                        border_radius: BorderRadius::all(Val::Px(ITEM_RADIUS)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.popup_btn),
                ))
                .with_children(|del_btn| {
                    del_btn.spawn((
                        Text::new(FA_TRASH),
                        TextFont {
                            font: icon_font.clone(),
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.text_dim),
                    ));
                });
            });
        }
    });

    spawn_secondary_btn(card, "New Profile", ProfileAction::NewProfile, theme);
    spawn_secondary_btn(card, "Close", ProfileAction::Close, theme);
}

fn build_create_form(
    card: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    title: &str,
    show_back: bool,
) {
    card.spawn((
        Text::new(title),
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

    card.spawn((
        ProfileNameInput,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(40.0),
            padding: UiRect::horizontal(Val::Px(12.0)),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
            ..default()
        },
        BackgroundColor(theme.popup_btn),
    ))
    .with_children(|input| {
        input.spawn((
            ProfileLabelText,
            Text::new("Type your name..."),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(theme.text_dim),
        ));
    });

    spawn_primary_btn(card, "Create", ProfileAction::Create, theme);

    if show_back {
        spawn_secondary_btn(card, "Back", ProfileAction::CancelDelete, theme);
    } else {
        spawn_secondary_btn(card, "Cancel", ProfileAction::Close, theme);
    }
}

fn build_delete_confirm(
    card: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    name: &str,
) {
    card.spawn((
        Text::new("Delete Profile?"),
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

    card.spawn((
        Text::new(format!(
            "\"{}\" and all their scores will be permanently deleted.",
            name
        )),
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

    spawn_danger_btn(card, "Delete", ProfileAction::ConfirmDelete, theme);
    spawn_secondary_btn(card, "Cancel", ProfileAction::CancelDelete, theme);
}

fn spawn_primary_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ProfileAction,
    theme: &UiTheme,
) {
    parent
        .spawn((
            ProfileButton { action },
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(10.0),
                    Val::Px(10.0),
                ),
                border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
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
    action: ProfileAction,
    theme: &UiTheme,
) {
    parent
        .spawn((
            ProfileButton { action },
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(10.0),
                    Val::Px(10.0),
                ),
                border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.popup_btn),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, 14.0, theme.text_primary);
        });
}

fn spawn_danger_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ProfileAction,
    theme: &UiTheme,
) {
    parent
        .spawn((
            ProfileButton { action },
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(10.0),
                    Val::Px(10.0),
                ),
                border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.badge_failed),
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

fn spawn_new_profile_input(commands: &mut Commands, theme: &UiTheme) {
    commands.insert_resource(ProfileInputState::default());

    commands
        .spawn((
            ProfileOverlay,
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
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(CARD_WIDTH),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(CARD_PADDING)),
                        row_gap: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    build_create_form(card, theme, "New Profile", true);
                });
        });
}

fn spawn_delete_confirm(commands: &mut Commands, theme: &UiTheme, name: &str) {
    commands
        .spawn((
            ProfileOverlay,
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
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(CARD_WIDTH),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(CARD_PADDING)),
                        row_gap: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(CARD_RADIUS)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    build_delete_confirm(card, theme, name);
                });
        });
}

fn despawn_overlay(commands: &mut Commands, overlay_query: &Query<Entity, With<ProfileOverlay>>) {
    for entity in overlay_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<ProfileInputState>();
}

pub fn handle_profile_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &ProfileButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut profiles: ResMut<ProfileStore>,
    input_state: Option<Res<ProfileInputState>>,
    overlay_query: Query<Entity, With<ProfileOverlay>>,
    theme: Res<UiTheme>,
    pending_delete: Option<Res<PendingDeleteProfile>>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                match btn.action {
                    ProfileAction::Create => {
                        let name = input_state
                            .as_ref()
                            .map(|s| s.text.trim().to_string())
                            .unwrap_or_default();
                        if name.is_empty() {
                            return;
                        }
                        profiles.create_profile(name.clone());
                        despawn_overlay(&mut commands, &overlay_query);
                        next_state.set(AppState::Menu);
                        return;
                    }
                    ProfileAction::Switch(idx) => {
                        if let Some(name) = profiles.profiles.get(idx).cloned() {
                            profiles.switch_profile(&name);
                        }
                        despawn_overlay(&mut commands, &overlay_query);
                        next_state.set(AppState::Menu);
                        return;
                    }
                    ProfileAction::Delete(idx) => {
                        if let Some(name) = profiles.profiles.get(idx).cloned() {
                            despawn_overlay(&mut commands, &overlay_query);
                            commands.insert_resource(PendingDeleteProfile {
                                name: name.clone(),
                            });
                            spawn_delete_confirm(&mut commands, &theme, &name);
                        }
                    }
                    ProfileAction::ConfirmDelete => {
                        if let Some(ref pending) = pending_delete {
                            profiles.delete_profile(&pending.name);
                        }
                        despawn_overlay(&mut commands, &overlay_query);
                        commands.remove_resource::<PendingDeleteProfile>();
                        next_state.set(AppState::Menu);
                        return;
                    }
                    ProfileAction::CancelDelete => {
                        despawn_overlay(&mut commands, &overlay_query);
                        commands.remove_resource::<PendingDeleteProfile>();
                        spawn_profile_popup(&mut commands, &theme, &profiles, &asset_server);
                    }
                    ProfileAction::NewProfile => {
                        despawn_overlay(&mut commands, &overlay_query);
                        spawn_new_profile_input(&mut commands, &theme);
                    }
                    ProfileAction::Close => {
                        despawn_overlay(&mut commands, &overlay_query);
                        commands.remove_resource::<PendingDeleteProfile>();
                    }
                }
            }
            Interaction::Hovered => {
                *bg = match btn.action {
                    ProfileAction::Switch(_) | ProfileAction::Delete(_) => {
                        BackgroundColor(theme.popup_btn_hover)
                    }
                    ProfileAction::Create => BackgroundColor(theme.accent_hover),
                    ProfileAction::ConfirmDelete => {
                        BackgroundColor(Color::srgb(0.88, 0.25, 0.25))
                    }
                    _ => BackgroundColor(theme.popup_btn_hover),
                };
            }
            Interaction::None => {
                *bg = match btn.action {
                    ProfileAction::Switch(_) | ProfileAction::Delete(_) => {
                        BackgroundColor(theme.popup_btn)
                    }
                    ProfileAction::Create => BackgroundColor(theme.accent),
                    ProfileAction::ConfirmDelete => BackgroundColor(theme.badge_failed),
                    _ => BackgroundColor(theme.popup_btn),
                };
            }
        }
    }
}


pub fn handle_profile_input(
    mut key_events: MessageReader<KeyboardInput>,
    mut input_state: ResMut<ProfileInputState>,
    mut label_query: Query<(&mut Text, &mut TextColor), With<ProfileLabelText>>,
    theme: Res<UiTheme>,
) {
    let mut changed = false;

    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }

        if ev.key_code == KeyCode::Backspace {
            if !input_state.text.is_empty() {
                input_state.text.pop();
                changed = true;
            }
            continue;
        }

        if let Some(ref text) = ev.text {
            for c in text.chars() {
                if !c.is_control() && input_state.text.len() < 30 {
                    input_state.text.push(c);
                    changed = true;
                }
            }
        }
    }

    if !changed {
        return;
    }

    if let Ok((mut text, mut color)) = label_query.single_mut() {
        if input_state.text.is_empty() {
            **text = "Type your name...".into();
            *color = TextColor(theme.text_dim);
        } else {
            **text = format!("{}_", input_state.text);
            *color = TextColor(theme.text_primary);
        }
    }
}
