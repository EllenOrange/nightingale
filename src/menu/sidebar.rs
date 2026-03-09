use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::app::AppExit;
use bevy::prelude::*;

use super::folder::{PendingFolderPick, PendingRescan};
use super::settings::spawn_settings_popup;
use super::song_card::*;
use super::{IconFont, FA_MOON, FA_SUN, FA_USER};
use crate::analyzer::cache::CacheDir;
use crate::profile::ProfileStore;
use crate::scanner::metadata::Song;
use crate::states::AppState;
use crate::ui::{self, UiTheme};

const FA_GEAR: &str = "\u{f013}";
const FA_RIGHT_FROM_BRACKET: &str = "\u{f2f5}";

#[derive(Component)]
pub struct ExitOverlay;

#[derive(Component)]
pub(crate) struct ExitCancelButton;

#[derive(Component)]
pub(crate) struct ExitConfirmButton;

#[derive(Resource)]
pub struct ExitFocus(pub usize);

pub fn build_sidebar(
    root: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    has_folder: bool,
    logo: Handle<Image>,
    icon_font: &IconFont,
    profiles: &ProfileStore,
) {
    root.spawn((
        Node {
            width: Val::Px(220.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(16.0), Val::Px(16.0)),
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(theme.sidebar_bg),
    ))
    .with_children(|sidebar| {
        sidebar.spawn((
            ImageNode::new(logo),
            Node {
                width: Val::Px(196.0),
                margin: UiRect::bottom(Val::Px(20.0)),
                ..default()
            },
        ));

        let folder_label = if has_folder {
            "Change Folder"
        } else {
            "Select Folder"
        };
        spawn_sidebar_button(sidebar, folder_label, SidebarAction::ChangeFolder, theme, true);

        spawn_sidebar_button(
            sidebar,
            "Rescan Folder",
            SidebarAction::RescanFolder,
            theme,
            has_folder,
        );

        sidebar.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });

        let profile_icon_color = if profiles.active.is_some() {
            theme.accent
        } else {
            theme.text_primary
        };

        if let Some(ref name) = profiles.active {
            sidebar.spawn((
                ProfileNameLabel,
                Text::new(name.as_str()),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(theme.accent),
                Node {
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
            ));
        }

        sidebar
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                spawn_icon_btn(
                    row,
                    FA_USER,
                    SidebarAction::Profile,
                    theme,
                    icon_font,
                    ProfileIconMarker,
                    profile_icon_color,
                );

                spawn_icon_btn(
                    row,
                    FA_GEAR,
                    SidebarAction::Settings,
                    theme,
                    icon_font,
                    SettingsIconMarker,
                    theme.text_primary,
                );

                let theme_glyph = if theme.mode == crate::ui::ThemeMode::Dark {
                    FA_SUN
                } else {
                    FA_MOON
                };
                spawn_icon_btn(
                    row,
                    theme_glyph,
                    SidebarAction::ToggleTheme,
                    theme,
                    icon_font,
                    ThemeToggleIcon,
                    theme.text_primary,
                );

                spawn_icon_btn(
                    row,
                    FA_RIGHT_FROM_BRACKET,
                    SidebarAction::Exit,
                    theme,
                    icon_font,
                    ExitIconMarker,
                    theme.text_primary,
                );
            });
    });
}

#[derive(Component)]
struct ProfileIconMarker;

#[derive(Component)]
struct SettingsIconMarker;

#[derive(Component)]
struct ExitIconMarker;

fn spawn_icon_btn(
    parent: &mut ChildSpawnerCommands,
    glyph: &str,
    action: SidebarAction,
    theme: &UiTheme,
    icon_font: &IconFont,
    marker: impl Component,
    text_color: Color,
) {
    parent
        .spawn((
            SidebarButton { action },
            marker,
            Button,
            Node {
                width: Val::Px(40.0),
                height: Val::Px(40.0),
                flex_shrink: 0.0,
                flex_grow: 1.0,
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.sidebar_btn),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(glyph),
                TextFont {
                    font: icon_font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(text_color),
            ));
        });
}

fn spawn_sidebar_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: SidebarAction,
    theme: &UiTheme,
    enabled: bool,
) {
    let text_color = if enabled {
        theme.text_primary
    } else {
        theme.text_dim
    };

    parent
        .spawn((
            SidebarButton { action },
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(10.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(theme.sidebar_btn),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, 13.0, text_color);
        });
}

pub fn handle_sidebar_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SidebarButton, &mut BackgroundColor, &mut BorderColor),
        Changed<Interaction>,
    >,
    mut config: ResMut<crate::config::AppConfig>,
    pending: Option<Res<PendingFolderPick>>,
    pending_rescan: Option<Res<PendingRescan>>,
    mut theme: ResMut<UiTheme>,
    cache: Res<CacheDir>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    exit_overlay_query: Query<(), With<ExitOverlay>>,
    profiles: Res<ProfileStore>,
    mut next_state: ResMut<NextState<AppState>>,
    asset_server: Res<AssetServer>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<super::MenuFocus>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Enter)
        && focus.active
        && focus.panel == super::FocusPanel::Sidebar
        && focus.sidebar_index < super::SIDEBAR_ACTIONS.len()
    {
        let action = super::SIDEBAR_ACTIONS[focus.sidebar_index];
        execute_sidebar_action(
            action,
            &mut commands,
            &mut config,
            pending.as_deref(),
            pending_rescan.as_deref(),
            &mut theme,
            &cache,
            &profiles,
            &mut next_state,
            &asset_server,
        );
        return;
    }

    for (interaction, sidebar_btn, mut bg, mut border) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                execute_sidebar_action(
                    sidebar_btn.action,
                    &mut commands,
                    &mut config,
                    pending.as_deref(),
                    pending_rescan.as_deref(),
                    &mut theme,
                    &cache,
                    &profiles,
                    &mut next_state,
                    &asset_server,
                );
            }
            Interaction::Hovered => {
                focus.active = false;
                *bg = BackgroundColor(theme.sidebar_btn_hover);
                *border = BorderColor::all(theme.accent);
            }
            Interaction::None => {
                let idx = super::SIDEBAR_ACTIONS
                    .iter()
                    .position(|&a| a == sidebar_btn.action);
                let is_focused = focus.active
                    && focus.panel == super::FocusPanel::Sidebar
                    && idx == Some(focus.sidebar_index);
                if is_focused {
                    *bg = BackgroundColor(theme.sidebar_btn_hover);
                    *border = BorderColor::all(theme.accent);
                } else {
                    *bg = BackgroundColor(theme.sidebar_btn);
                    *border = BorderColor::all(Color::NONE);
                }
            }
        }
    }
}

fn execute_sidebar_action(
    action: SidebarAction,
    commands: &mut Commands,
    config: &mut crate::config::AppConfig,
    pending: Option<&PendingFolderPick>,
    pending_rescan: Option<&PendingRescan>,
    theme: &mut UiTheme,
    cache: &CacheDir,
    profiles: &ProfileStore,
    next_state: &mut NextState<AppState>,
    asset_server: &AssetServer,
) {
    match action {
        SidebarAction::Settings => {
            spawn_settings_popup(commands, theme, config);
        }
        SidebarAction::Profile => {
            super::profile::spawn_profile_popup(commands, theme, profiles, asset_server);
        }
        SidebarAction::ToggleTheme => {
            theme.toggle();
            config.dark_mode = Some(theme.mode == crate::ui::ThemeMode::Dark);
            config.save();
            next_state.set(AppState::Menu);
        }
        SidebarAction::Exit => {
            spawn_exit_popup(commands, theme);
        }
        SidebarAction::ChangeFolder => {
            if pending.is_some() {
                return;
            }
            let result: Arc<Mutex<Option<Option<PathBuf>>>> = Arc::new(Mutex::new(None));
            let result_clone = Arc::clone(&result);
            std::thread::spawn(move || {
                let folder = rfd::FileDialog::new()
                    .set_title("Select your music folder")
                    .pick_folder();
                *result_clone.lock().unwrap() = Some(folder);
            });
            commands.insert_resource(PendingFolderPick { result });
        }
        SidebarAction::RescanFolder => {
            if pending_rescan.is_some() {
                return;
            }
            if let Some(folder) = config.last_folder.clone() {
                let cache_path = cache.path.clone();
                let result: Arc<Mutex<Option<Vec<Song>>>> = Arc::new(Mutex::new(None));
                let result_clone = Arc::clone(&result);
                std::thread::spawn(move || {
                    let scan_result = std::panic::catch_unwind(
                        std::panic::AssertUnwindSafe(|| {
                            let cache = CacheDir { path: cache_path };
                            crate::scanner::scan_folder(&folder, &cache)
                        }),
                    );
                    match scan_result {
                        Ok(songs) => {
                            *result_clone.lock().unwrap() = Some(songs);
                        }
                        Err(_) => {
                            error!("Rescan thread panicked");
                            *result_clone.lock().unwrap() = Some(vec![]);
                        }
                    }
                });
                commands.insert_resource(PendingRescan { result });
            }
        }
    }
}

fn spawn_exit_popup(commands: &mut Commands, theme: &UiTheme) {
    commands.insert_resource(ExitFocus(0));

    commands
        .spawn((
            ExitOverlay,
            GlobalZIndex(100),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(340.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(28.0)),
                        row_gap: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("Exit"),
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
                        Text::new("Are you sure you want to quit?"),
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

                    card.spawn((
                        ExitCancelButton,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::new(
                                Val::Px(14.0),
                                Val::Px(14.0),
                                Val::Px(10.0),
                                Val::Px(10.0),
                            ),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme.accent),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Cancel"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                    card.spawn((
                        ExitConfirmButton,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::new(
                                Val::Px(14.0),
                                Val::Px(14.0),
                                Val::Px(10.0),
                                Val::Px(10.0),
                            ),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme.popup_btn),
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Exit"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(theme.text_primary),
                        ));
                    });
                });
        });
}

pub fn handle_exit_input(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    overlay_query: Query<Entity, With<ExitOverlay>>,
    mut cancel_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<ExitCancelButton>, Without<ExitConfirmButton>),
    >,
    mut confirm_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (With<ExitConfirmButton>, Without<ExitCancelButton>),
    >,
    theme: Res<UiTheme>,
    mut exit_focus: Option<ResMut<ExitFocus>>,
    menu_state: Res<super::MenuState>,
    settings_query: Query<(), With<SettingsOverlay>>,
    profile_query: Query<(), With<ProfileOverlay>>,
) {
    let overlay_entity = overlay_query.single();

    if overlay_entity.is_err() {
        if keyboard.just_pressed(KeyCode::Escape)
            && menu_state.search_query.is_empty()
            && settings_query.is_empty()
            && profile_query.is_empty()
        {
            spawn_exit_popup(&mut commands, &theme);
        }
        return;
    }

    let overlay_entity = overlay_entity.unwrap();

    if keyboard.just_pressed(KeyCode::Escape) {
        commands.entity(overlay_entity).despawn();
        commands.remove_resource::<ExitFocus>();
        return;
    }

    if let Some(ref mut ef) = exit_focus {
        if keyboard.just_pressed(KeyCode::ArrowUp) || keyboard.just_pressed(KeyCode::ArrowDown)
            || keyboard.just_pressed(KeyCode::ArrowLeft) || keyboard.just_pressed(KeyCode::ArrowRight)
            || keyboard.just_pressed(KeyCode::Tab)
        {
            ef.0 = 1 - ef.0;
        }

        if keyboard.just_pressed(KeyCode::Enter) {
            if ef.0 == 0 {
                commands.entity(overlay_entity).despawn();
                commands.remove_resource::<ExitFocus>();
                return;
            } else {
                exit.write(AppExit::Success);
                return;
            }
        }
    }

    let focus_idx = exit_focus.map(|f| f.0);

    for (interaction, mut bg, mut border) in &mut cancel_query {
        match interaction {
            Interaction::Pressed => {
                commands.entity(overlay_entity).despawn();
                commands.remove_resource::<ExitFocus>();
                return;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.accent_hover);
                *border = BorderColor::all(theme.accent);
            }
            Interaction::None => {
                let focused = focus_idx == Some(0);
                if focused {
                    *bg = BackgroundColor(theme.accent_hover);
                    *border = BorderColor::all(theme.accent);
                } else {
                    *bg = BackgroundColor(theme.accent);
                    *border = BorderColor::all(Color::NONE);
                }
            }
        }
    }

    for (interaction, mut bg, mut border) in &mut confirm_query {
        match interaction {
            Interaction::Pressed => {
                exit.write(AppExit::Success);
                return;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.popup_btn_hover);
                *border = BorderColor::all(theme.accent);
            }
            Interaction::None => {
                let focused = focus_idx == Some(1);
                if focused {
                    *bg = BackgroundColor(theme.popup_btn_hover);
                    *border = BorderColor::all(theme.accent);
                } else {
                    *bg = BackgroundColor(theme.popup_btn);
                    *border = BorderColor::all(Color::NONE);
                }
            }
        }
    }
}
