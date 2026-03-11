use bevy::prelude::*;
use bevy::window::WindowMode;

use super::song_card::*;
use crate::ui::{self, UiTheme};

const SEPARATORS: &[(&str, &str)] = &[
    ("karaoke", "UVR Karaoke"),
    ("demucs", "Demucs"),
];
const MODELS: &[&str] = &["large-v3", "large-v3-turbo"];

#[derive(Resource)]
pub struct SettingsFocus(pub usize);

const SETTINGS_ROW_COUNT: usize = 7;

struct SettingsRowMapping {
    enter: SettingsAction,
    left: Option<SettingsAction>,
    right: Option<SettingsAction>,
}

const fn row_mapping(
    enter: SettingsAction,
    left: Option<SettingsAction>,
    right: Option<SettingsAction>,
) -> SettingsRowMapping {
    SettingsRowMapping { enter, left, right }
}

fn settings_rows() -> [SettingsRowMapping; SETTINGS_ROW_COUNT] {
    [
        row_mapping(SettingsAction::ToggleFullscreen, None, None),
        row_mapping(
            SettingsAction::SeparatorNext,
            Some(SettingsAction::SeparatorPrev),
            Some(SettingsAction::SeparatorNext),
        ),
        row_mapping(
            SettingsAction::ModelNext,
            Some(SettingsAction::ModelPrev),
            Some(SettingsAction::ModelNext),
        ),
        row_mapping(
            SettingsAction::BeamUp,
            Some(SettingsAction::BeamDown),
            Some(SettingsAction::BeamUp),
        ),
        row_mapping(
            SettingsAction::BatchUp,
            Some(SettingsAction::BatchDown),
            Some(SettingsAction::BatchUp),
        ),
        row_mapping(SettingsAction::RestoreDefaults, None, None),
        row_mapping(SettingsAction::Close, None, None),
    ]
}

pub fn spawn_settings_popup(
    commands: &mut Commands,
    theme: &UiTheme,
    config: &crate::config::AppConfig,
) {
    commands.insert_resource(SettingsFocus(0));
    commands
        .spawn((
            SettingsOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(460.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(8.0),
                        border_radius: BorderRadius::all(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("Settings"),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(theme.text_primary),
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    spawn_settings_section(card, theme, "General");
                    let fs_label = if config.is_fullscreen() { "Fullscreen" } else { "Windowed" };
                    spawn_settings_row(card, theme, "Window", fs_label,
                        SettingsValueText(SettingsField::Fullscreen),
                        &[("Switch", SettingsAction::ToggleFullscreen)],
                        "Toggle between fullscreen and windowed mode", 0);

                    spawn_settings_section(card, theme, "Analyzer");
                    let sep_label = separator_display(config.separator());
                    spawn_settings_row(card, theme, "Separator", sep_label,
                        SettingsValueText(SettingsField::Separator),
                        &[("<", SettingsAction::SeparatorPrev), (">", SettingsAction::SeparatorNext)],
                        "Karaoke removes backing vocals for cleaner lyrics; Demucs is faster", 1);
                    spawn_settings_row(card, theme, "Model", config.whisper_model(),
                        SettingsValueText(SettingsField::Model),
                        &[("<", SettingsAction::ModelPrev), (">", SettingsAction::ModelNext)],
                        "turbo is fastest, v3 is most accurate", 2);
                    spawn_settings_row(card, theme, "Beam size", &config.beam_size().to_string(),
                        SettingsValueText(SettingsField::Beam),
                        &[("-", SettingsAction::BeamDown), ("+", SettingsAction::BeamUp)],
                        "Higher values improve accuracy at the cost of speed", 3);
                    spawn_settings_row(card, theme, "Batch size", &config.batch_size().to_string(),
                        SettingsValueText(SettingsField::Batch),
                        &[("-", SettingsAction::BatchDown), ("+", SettingsAction::BatchUp)],
                        "Higher values use more memory but process faster", 4);

                    card.spawn((
                        Text::new("Changes apply to future analyses. Use the re-analyze button on song cards to apply."),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(theme.text_dim),
                        Node {
                            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(4.0), Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    spawn_settings_wide_btn(card, "Restore Defaults", SettingsAction::RestoreDefaults, theme, 5);
                    spawn_settings_wide_btn(card, "Close", SettingsAction::Close, theme, 6);
                });
        });
}

fn spawn_settings_section(parent: &mut ChildSpawnerCommands, theme: &UiTheme, title: &str) {
    parent.spawn((
        Text::new(title.to_uppercase()),
        TextFont { font_size: 11.0, ..default() },
        TextColor(theme.text_dim),
        Node {
            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(12.0), Val::Px(2.0)),
            ..default()
        },
    ));
}

fn spawn_settings_row(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    label: &str,
    value: &str,
    marker: SettingsValueText,
    buttons: &[(&str, SettingsAction)],
    description: &str,
    row_idx: usize,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|wrapper| {
            wrapper
                .spawn((
                    SettingsRow(row_idx),
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(8.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::NONE),
                    BackgroundColor(theme.popup_btn),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(label),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(theme.text_secondary),
                        Node {
                            width: Val::Px(100.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));

                    row.spawn((
                        marker,
                        Text::new(value),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(theme.text_primary),
                        Node {
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));

                    for &(btn_label, action) in buttons {
                        spawn_settings_btn(row, btn_label, action, theme, false);
                    }
                });

            wrapper.spawn((
                Text::new(description),
                TextFont { font_size: 11.0, ..default() },
                TextColor(theme.text_dim),
                Node {
                    padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(2.0), Val::Px(0.0)),
                    ..default()
                },
            ));
        });
}

fn spawn_settings_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: SettingsAction,
    theme: &UiTheme,
    wide: bool,
) {
    let width = if wide { Val::Percent(100.0) } else { Val::Auto };
    let padding = if wide {
        UiRect::new(Val::Px(16.0), Val::Px(16.0), Val::Px(10.0), Val::Px(10.0))
    } else {
        UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(5.0), Val::Px(5.0))
    };
    let font_size = if wide { 14.0 } else { 13.0 };
    let bg = if wide {
        theme.popup_btn
    } else {
        theme.popup_btn_hover
    };
    parent
        .spawn((
            SettingsButton { action },
            Button,
            Node {
                width,
                padding,
                margin: UiRect::left(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, font_size, theme.text_primary);
        });
}

fn spawn_settings_wide_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: SettingsAction,
    theme: &UiTheme,
    row_idx: usize,
) {
    parent
        .spawn((
            SettingsRow(row_idx),
            SettingsButton { action },
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(16.0), Val::Px(16.0), Val::Px(10.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.popup_btn),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, 14.0, theme.text_primary);
        });
}

fn dispatch_settings_action(
    action: SettingsAction,
    commands: &mut Commands,
    config: &mut crate::config::AppConfig,
    overlay_query: &Query<Entity, With<SettingsOverlay>>,
    value_texts: &mut Query<(&SettingsValueText, &mut Text)>,
    windows: &mut Query<&mut Window>,
) {
    match action {
        SettingsAction::ToggleFullscreen => {
            if let Ok(mut window) = windows.single_mut() {
                let is_fs = matches!(window.mode, WindowMode::BorderlessFullscreen(_));
                window.mode = if is_fs {
                    WindowMode::Windowed
                } else {
                    WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
                };
                config.fullscreen = Some(!is_fs);
                config.save();
                let new_label = if is_fs { "Windowed" } else { "Fullscreen" };
                set_settings_text(value_texts, SettingsField::Fullscreen, new_label);
            }
        }
        SettingsAction::SeparatorPrev | SettingsAction::SeparatorNext => {
            let current = config.separator();
            let idx = SEPARATORS.iter().position(|(k, _)| *k == current).unwrap_or(0);
            let next_idx = if matches!(action, SettingsAction::SeparatorNext) {
                (idx + 1) % SEPARATORS.len()
            } else {
                (idx + SEPARATORS.len() - 1) % SEPARATORS.len()
            };
            let (key, _) = SEPARATORS[next_idx];
            config.separator = Some(key.to_string());
            config.save();
            set_settings_text(value_texts, SettingsField::Separator, separator_display(key));
        }
        SettingsAction::ModelPrev | SettingsAction::ModelNext => {
            let current = config.whisper_model();
            let idx = MODELS.iter().position(|&m| m == current).unwrap_or(0);
            let next_idx = if matches!(action, SettingsAction::ModelNext) {
                (idx + 1) % MODELS.len()
            } else {
                (idx + MODELS.len() - 1) % MODELS.len()
            };
            let new_model = MODELS[next_idx];
            config.whisper_model = Some(new_model.to_string());
            config.save();
            set_settings_text(value_texts, SettingsField::Model, new_model);
        }
        SettingsAction::BeamUp => {
            let new_val = (config.beam_size() + 1).min(15);
            config.beam_size = Some(new_val);
            config.save();
            set_settings_text(value_texts, SettingsField::Beam, &new_val.to_string());
        }
        SettingsAction::BeamDown => {
            let new_val = config.beam_size().saturating_sub(1).max(1);
            config.beam_size = Some(new_val);
            config.save();
            set_settings_text(value_texts, SettingsField::Beam, &new_val.to_string());
        }
        SettingsAction::BatchUp => {
            let new_val = (config.batch_size() + 1).min(16);
            config.batch_size = Some(new_val);
            config.save();
            set_settings_text(value_texts, SettingsField::Batch, &new_val.to_string());
        }
        SettingsAction::BatchDown => {
            let new_val = config.batch_size().saturating_sub(1).max(1);
            config.batch_size = Some(new_val);
            config.save();
            set_settings_text(value_texts, SettingsField::Batch, &new_val.to_string());
        }
        SettingsAction::RestoreDefaults => {
            config.separator = None;
            config.whisper_model = None;
            config.beam_size = None;
            config.batch_size = None;
            config.fullscreen = None;
            config.save();
            set_settings_text(value_texts, SettingsField::Separator, separator_display(config.separator()));
            set_settings_text(value_texts, SettingsField::Model, config.whisper_model());
            set_settings_text(value_texts, SettingsField::Beam, &config.beam_size().to_string());
            set_settings_text(value_texts, SettingsField::Batch, &config.batch_size().to_string());
            let fs_label = if config.is_fullscreen() { "Fullscreen" } else { "Windowed" };
            set_settings_text(value_texts, SettingsField::Fullscreen, fs_label);
            if let Ok(mut window) = windows.single_mut() {
                window.mode = if config.is_fullscreen() {
                    WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
                } else {
                    WindowMode::Windowed
                };
            }
        }
        SettingsAction::Close => {
            for entity in overlay_query {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub fn handle_settings_click(
    mut commands: Commands,
    interaction_events: Query<
        (&Interaction, &SettingsButton),
        Changed<Interaction>,
    >,
    mut row_styles: Query<(&SettingsRow, &mut BackgroundColor, &mut BorderColor)>,
    mut btn_styles: Query<
        (&Interaction, &SettingsButton, &mut BackgroundColor),
        Without<SettingsRow>,
    >,
    mut config: ResMut<crate::config::AppConfig>,
    overlay_query: Query<Entity, With<SettingsOverlay>>,
    mut value_texts: Query<(&SettingsValueText, &mut Text)>,
    theme: Res<UiTheme>,
    mut windows: Query<&mut Window>,
    nav: Res<crate::input::NavInput>,
    mut settings_focus: Option<ResMut<SettingsFocus>>,
) {
    if overlay_query.is_empty() {
        return;
    }

    if nav.back {
        for entity in &overlay_query {
            commands.entity(entity).despawn();
        }
        commands.remove_resource::<SettingsFocus>();
        return;
    }

    if let Some(ref mut sf) = settings_focus {
        let rows = settings_rows();
        let mut action_to_dispatch: Option<SettingsAction> = None;

        if nav.down {
            sf.0 = (sf.0 + 1).min(SETTINGS_ROW_COUNT - 1);
        }
        if nav.up {
            sf.0 = sf.0.saturating_sub(1);
        }
        if nav.confirm {
            action_to_dispatch = Some(rows[sf.0].enter);
        }
        if nav.left {
            if let Some(a) = rows[sf.0].left {
                action_to_dispatch = Some(a);
            }
        }
        if nav.right {
            if let Some(a) = rows[sf.0].right {
                action_to_dispatch = Some(a);
            }
        }

        if let Some(action) = action_to_dispatch {
            dispatch_settings_action(
                action,
                &mut commands,
                &mut config,
                &overlay_query,
                &mut value_texts,
                &mut windows,
            );
            if action == SettingsAction::Close {
                commands.remove_resource::<SettingsFocus>();
                return;
            }
        }
    }

    for (interaction, settings_btn) in &interaction_events {
        match interaction {
            Interaction::Pressed => {
                dispatch_settings_action(
                    settings_btn.action,
                    &mut commands,
                    &mut config,
                    &overlay_query,
                    &mut value_texts,
                    &mut windows,
                );
                if settings_btn.action == SettingsAction::Close {
                    commands.remove_resource::<SettingsFocus>();
                    return;
                }
            }
            Interaction::Hovered => {
                let row_for_action = match settings_btn.action {
                    SettingsAction::ToggleFullscreen => Some(0),
                    SettingsAction::SeparatorPrev | SettingsAction::SeparatorNext => Some(1),
                    SettingsAction::ModelPrev | SettingsAction::ModelNext => Some(2),
                    SettingsAction::BeamDown | SettingsAction::BeamUp => Some(3),
                    SettingsAction::BatchDown | SettingsAction::BatchUp => Some(4),
                    SettingsAction::RestoreDefaults => Some(5),
                    SettingsAction::Close => Some(6),
                };
                if let (Some(sf), Some(row)) = (&mut settings_focus, row_for_action) {
                    sf.0 = row;
                }
            }
            Interaction::None => {}
        }
    }

    for (interaction, _btn, mut bg) in &mut btn_styles {
        let target = match interaction {
            Interaction::Hovered | Interaction::Pressed => BackgroundColor(theme.accent),
            Interaction::None => BackgroundColor(theme.popup_btn_hover),
        };
        bg.set_if_neq(target);
    }

    if let Some(ref sf) = settings_focus {
        let focus_idx = sf.0;
        for (row, mut bg, mut border) in &mut row_styles {
            let focused = row.0 == focus_idx;
            let target_bg = if focused {
                BackgroundColor(theme.popup_btn_hover)
            } else {
                BackgroundColor(theme.popup_btn)
            };
            let target_border = if focused {
                BorderColor::all(theme.accent)
            } else {
                BorderColor::all(Color::NONE)
            };
            bg.set_if_neq(target_bg);
            border.set_if_neq(target_border);
        }
    }
}

fn separator_display(key: &str) -> &str {
    SEPARATORS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, label)| *label)
        .unwrap_or(key)
}

fn set_settings_text(
    query: &mut Query<(&SettingsValueText, &mut Text)>,
    field: SettingsField,
    value: &str,
) {
    for (svt, mut text) in query.iter_mut() {
        if svt.0 == field {
            **text = value.to_string();
            return;
        }
    }
}
