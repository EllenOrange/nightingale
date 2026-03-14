use bevy::prelude::*;

use super::components::*;
use crate::ui::{self, ButtonVariant, UiTheme};

const ATTRIBUTIONS: &[(&str, &str)] = &[
    ("Video backgrounds", "Pixabay (pixabay.com)"),
    ("Lyrics data", "LRCLIB (lrclib.net)"),
    ("Noto Sans CJK font", "Google — SIL Open Font License"),
    ("Font Awesome Free icons", "SIL OFL / CC BY 4.0"),
    ("Stem separation", "Demucs by Meta Research — MIT"),
    ("Speech recognition", "WhisperX / OpenAI Whisper — MIT"),
];

pub fn spawn_about_popup(commands: &mut Commands, theme: &UiTheme) {
    commands
        .spawn((
            AboutOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.overlay_dim),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(crate::ui::layout::OVERLAY_WIDTH_LG),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(32.0)),
                        row_gap: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("Nightingale"),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(theme.text_primary),
                    ));

                    card.spawn((
                        Text::new(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                        TextFont { font_size: 13.0, ..default() },
                        TextColor(theme.text_secondary),
                    ));

                    card.spawn((
                        Text::new("License: GPL-3.0-or-later"),
                        TextFont { font_size: 13.0, ..default() },
                        TextColor(theme.text_secondary),
                        Node {
                            margin: UiRect::bottom(Val::Px(8.0)),
                            ..default()
                        },
                    ));

                    card.spawn((
                        Text::new("ATTRIBUTIONS"),
                        TextFont { font_size: 11.0, ..default() },
                        TextColor(theme.text_dim),
                        Node {
                            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(4.0), Val::Px(4.0)),
                            ..default()
                        },
                    ));

                    for &(label, value) in ATTRIBUTIONS {
                        spawn_attribution_row(card, theme, label, value);
                    }

                    card.spawn(Node {
                        height: Val::Px(8.0),
                        ..default()
                    });

                    let close_id = ui::spawn_button(
                        card,
                        ButtonVariant::Secondary,
                        "Close",
                        theme,
                        AboutCloseButton,
                    );
                    card.commands().entity(close_id).insert((
                        BackgroundColor(theme.popup_btn_hover),
                        BorderColor::all(theme.accent),
                    ));
                });
        });
}

fn spawn_attribution_row(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    label: &str,
    value: &str,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(6.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.popup_btn),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont { font_size: 13.0, ..default() },
                TextColor(theme.text_secondary),
                Node {
                    width: Val::Px(170.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));

            row.spawn((
                Text::new(value),
                TextFont { font_size: 13.0, ..default() },
                TextColor(theme.text_primary),
            ));
        });
}

pub fn handle_about_click(
    mut commands: Commands,
    interactions: Query<&Interaction, (Changed<Interaction>, With<AboutCloseButton>)>,
    overlay_query: Query<Entity, With<AboutOverlay>>,
    nav: Res<crate::input::NavInput>,
) {
    if overlay_query.is_empty() {
        return;
    }

    if nav.back || nav.confirm {
        for entity in &overlay_query {
            commands.entity(entity).despawn();
        }
        return;
    }

    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            for entity in &overlay_query {
                commands.entity(entity).despawn();
            }
            return;
        }
    }
}
