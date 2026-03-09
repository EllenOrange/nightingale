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
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                spawn_sidebar_button(row, "Settings", SidebarAction::Settings, theme, true);

                let theme_glyph = if theme.mode == crate::ui::ThemeMode::Dark {
                    FA_SUN
                } else {
                    FA_MOON
                };
                spawn_icon_btn(row, theme_glyph, SidebarAction::ToggleTheme, theme, icon_font, ThemeToggleIcon);

                row.spawn((
                    SidebarButton {
                        action: SidebarAction::Profile,
                    },
                    Button,
                    Node {
                        width: Val::Px(40.0),
                        height: Val::Px(40.0),
                        flex_shrink: 0.0,
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.sidebar_btn),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(FA_USER),
                        TextFont {
                            font: icon_font.0.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(profile_icon_color),
                    ));
                });
            });

        spawn_sidebar_button(sidebar, "Exit", SidebarAction::Exit, theme, true);
    });
}

fn spawn_icon_btn(
    parent: &mut ChildSpawnerCommands,
    glyph: &str,
    action: SidebarAction,
    theme: &UiTheme,
    icon_font: &IconFont,
    marker: impl Component,
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
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
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
                TextColor(theme.text_primary),
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
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.sidebar_btn),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, 13.0, text_color);
        });
}

pub fn handle_sidebar_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SidebarButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut exit: MessageWriter<AppExit>,
    mut config: ResMut<crate::config::AppConfig>,
    pending: Option<Res<PendingFolderPick>>,
    pending_rescan: Option<Res<PendingRescan>>,
    mut theme: ResMut<UiTheme>,
    cache: Res<CacheDir>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    profiles: Res<ProfileStore>,
    mut next_state: ResMut<NextState<AppState>>,
    asset_server: Res<AssetServer>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() {
        return;
    }
    for (interaction, sidebar_btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => match sidebar_btn.action {
                SidebarAction::RescanFolder => {
                    if pending_rescan.is_some() {
                        return;
                    }
                    if let Some(folder) = config.last_folder.clone() {
                        let cache_path = cache.path.clone();
                        let result: Arc<Mutex<Option<Vec<Song>>>> =
                            Arc::new(Mutex::new(None));
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
                SidebarAction::Settings => {
                    spawn_settings_popup(&mut commands, &theme, &config);
                }
                SidebarAction::Profile => {
                    super::profile::spawn_profile_popup(
                        &mut commands,
                        &theme,
                        &profiles,
                        &asset_server,
                    );
                }
                SidebarAction::ToggleTheme => {
                    theme.toggle();
                    config.dark_mode = Some(theme.mode == crate::ui::ThemeMode::Dark);
                    config.save();
                    next_state.set(AppState::Menu);
                    return;
                }
                SidebarAction::Exit => {
                    exit.write(AppExit::Success);
                }
            },
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.sidebar_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.sidebar_btn);
            }
        }
    }
}
