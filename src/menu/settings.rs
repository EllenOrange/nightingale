use bevy::prelude::*;
use bevy::window::WindowMode;

use super::song_card::*;
use crate::ui::{self, UiTheme};

const MODELS: &[&str] = &["large-v3-turbo", "large-v3"];

pub fn spawn_settings_popup(
    commands: &mut Commands,
    theme: &UiTheme,
    config: &crate::config::AppConfig,
) {
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
                        "Toggle between fullscreen and windowed mode");

                    spawn_settings_section(card, theme, "Analyzer");
                    spawn_settings_row(card, theme, "Model", config.whisper_model(),
                        SettingsValueText(SettingsField::Model),
                        &[("<", SettingsAction::ModelPrev), (">", SettingsAction::ModelNext)],
                        "turbo is fastest, v3 is most accurate");
                    spawn_settings_row(card, theme, "Beam size", &config.beam_size().to_string(),
                        SettingsValueText(SettingsField::Beam),
                        &[("-", SettingsAction::BeamDown), ("+", SettingsAction::BeamUp)],
                        "Higher values improve accuracy at the cost of speed");
                    spawn_settings_row(card, theme, "Batch size", &config.batch_size().to_string(),
                        SettingsValueText(SettingsField::Batch),
                        &[("-", SettingsAction::BatchDown), ("+", SettingsAction::BatchUp)],
                        "Higher values use more memory but process faster");

                    card.spawn((
                        Text::new("Changes apply to future analyses. Use the re-analyze button on song cards to apply."),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(theme.text_dim),
                        Node {
                            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(4.0), Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    spawn_settings_btn(card, "Restore Defaults", SettingsAction::RestoreDefaults, theme, true);
                    spawn_settings_btn(card, "Close", SettingsAction::Close, theme, true);
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
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|wrapper| {
            wrapper
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(8.0), Val::Px(8.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
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

pub fn handle_settings_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SettingsButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut config: ResMut<crate::config::AppConfig>,
    overlay_query: Query<Entity, With<SettingsOverlay>>,
    mut value_texts: Query<(&SettingsValueText, &mut Text)>,
    theme: Res<UiTheme>,
    mut windows: Query<&mut Window>,
) {
    for (interaction, settings_btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                match settings_btn.action {
                    SettingsAction::ToggleFullscreen => {
                        if let Ok(mut window) = windows.single_mut() {
                            let is_fs = matches!(window.mode, WindowMode::BorderlessFullscreen(_));
                            window.mode = if is_fs {
                                WindowMode::Windowed
                            } else {
                                WindowMode::BorderlessFullscreen(
                                    bevy::window::MonitorSelection::Current,
                                )
                            };
                            config.fullscreen = Some(!is_fs);
                            config.save();
                            let new_label = if is_fs { "Windowed" } else { "Fullscreen" };
                            set_settings_text(&mut value_texts, SettingsField::Fullscreen, new_label);
                        }
                    }
                    SettingsAction::ModelPrev | SettingsAction::ModelNext => {
                        let current = config.whisper_model();
                        let idx = MODELS.iter().position(|&m| m == current).unwrap_or(0);
                        let next_idx = if matches!(settings_btn.action, SettingsAction::ModelNext) {
                            (idx + 1) % MODELS.len()
                        } else {
                            (idx + MODELS.len() - 1) % MODELS.len()
                        };
                        let new_model = MODELS[next_idx];
                        config.whisper_model = Some(new_model.to_string());
                        config.save();
                        set_settings_text(&mut value_texts, SettingsField::Model, new_model);
                    }
                    SettingsAction::BeamUp => {
                        let new_val = (config.beam_size() + 1).min(15);
                        config.beam_size = Some(new_val);
                        config.save();
                        set_settings_text(&mut value_texts, SettingsField::Beam, &new_val.to_string());
                    }
                    SettingsAction::BeamDown => {
                        let new_val = config.beam_size().saturating_sub(1).max(1);
                        config.beam_size = Some(new_val);
                        config.save();
                        set_settings_text(&mut value_texts, SettingsField::Beam, &new_val.to_string());
                    }
                    SettingsAction::BatchUp => {
                        let new_val = (config.batch_size() + 1).min(16);
                        config.batch_size = Some(new_val);
                        config.save();
                        set_settings_text(&mut value_texts, SettingsField::Batch, &new_val.to_string());
                    }
                    SettingsAction::BatchDown => {
                        let new_val = config.batch_size().saturating_sub(1).max(1);
                        config.batch_size = Some(new_val);
                        config.save();
                        set_settings_text(&mut value_texts, SettingsField::Batch, &new_val.to_string());
                    }
                    SettingsAction::RestoreDefaults => {
                        config.whisper_model = None;
                        config.beam_size = None;
                        config.batch_size = None;
                        config.fullscreen = None;
                        config.save();

                        set_settings_text(&mut value_texts, SettingsField::Model, config.whisper_model());
                        set_settings_text(&mut value_texts, SettingsField::Beam, &config.beam_size().to_string());
                        set_settings_text(&mut value_texts, SettingsField::Batch, &config.batch_size().to_string());
                        let fs_label = if config.is_fullscreen() { "Fullscreen" } else { "Windowed" };
                        set_settings_text(&mut value_texts, SettingsField::Fullscreen, fs_label);

                        if let Ok(mut window) = windows.single_mut() {
                            window.mode = if config.is_fullscreen() {
                                WindowMode::BorderlessFullscreen(
                                    bevy::window::MonitorSelection::Current,
                                )
                            } else {
                                WindowMode::Windowed
                            };
                        }
                    }
                    SettingsAction::Close => {
                        for entity in &overlay_query {
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
            Interaction::Hovered => {
                *bg = match settings_btn.action {
                    SettingsAction::RestoreDefaults | SettingsAction::Close => {
                        BackgroundColor(theme.popup_btn_hover)
                    }
                    _ => BackgroundColor(theme.accent),
                };
            }
            Interaction::None => {
                *bg = match settings_btn.action {
                    SettingsAction::RestoreDefaults | SettingsAction::Close => {
                        BackgroundColor(theme.popup_btn)
                    }
                    _ => BackgroundColor(theme.popup_btn_hover),
                };
            }
        }
    }
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
