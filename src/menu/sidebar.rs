use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::app::AppExit;
use bevy::prelude::*;

use super::folder::{PendingFolderPick, PendingRescan};
use super::settings::spawn_settings_popup;
use super::song_card::*;
use super::{IconFont, FA_MOON, FA_SUN, FA_USER};
use crate::analyzer::cache::{dir_size, CacheDir};
use crate::profile::ProfileStore;
use crate::scanner::metadata::Song;
use crate::states::AppState;
use crate::ui::{self, UiTheme};

const FA_GEAR: &str = "\u{f013}";
const FA_RIGHT_FROM_BRACKET: &str = "\u{f2f5}";

#[derive(Resource, Default, Clone)]
pub struct CacheStats {
    pub songs_bytes: u64,
    pub videos_bytes: u64,
    pub models_bytes: u64,
    pub other_bytes: u64,
    pub clearable_videos_bytes: u64,
}

impl CacheStats {
    pub fn total(&self) -> u64 {
        self.songs_bytes + self.videos_bytes + self.models_bytes + self.other_bytes
    }
}

#[derive(Resource)]
pub struct PendingCacheStats {
    result: Arc<Mutex<Option<CacheStats>>>,
}

pub fn start_cache_stats_computation(commands: &mut Commands) {
    let result: Arc<Mutex<Option<CacheStats>>> = Arc::new(Mutex::new(None));
    let r = Arc::clone(&result);
    std::thread::spawn(move || {
        let base = dirs::home_dir()
            .expect("could not find home directory")
            .join(".nightingale");
        let songs_bytes = dir_size(&base.join("cache"));
        let videos_bytes = dir_size(&base.join("videos"));
        let models_bytes = dir_size(&base.join("models"));
        let other_bytes = dir_size(&base.join("vendor"))
            + dir_size(&base.join("sounds"))
            + base
                .join("nightingale.log")
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0)
            + base
                .join("config.json")
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0)
            + base
                .join("profiles.json")
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);
        let clearable_videos_bytes = crate::vendor::clearable_video_bytes();
        *r.lock().unwrap() = Some(CacheStats {
            songs_bytes,
            videos_bytes,
            models_bytes,
            other_bytes,
            clearable_videos_bytes,
        });
    });
    commands.insert_resource(PendingCacheStats { result });
}

#[derive(Resource)]
pub struct CacheStatsTimer(pub Timer);

impl Default for CacheStatsTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(5.0, TimerMode::Repeating))
    }
}

pub fn poll_cache_stats(
    mut commands: Commands,
    pending: Option<Res<PendingCacheStats>>,
    mut stats: ResMut<CacheStats>,
    time: Res<Time>,
    mut timer: ResMut<CacheStatsTimer>,
) {
    if let Some(pending) = pending {
        if let Some(computed) = pending.result.lock().unwrap().take() {
            *stats = computed;
            commands.remove_resource::<PendingCacheStats>();
        }
        return;
    }

    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        start_cache_stats_computation(&mut commands);
    }
}

#[derive(Component)]
pub(crate) struct DiskUsageTotalLabel;

#[derive(Component)]
pub(crate) struct DiskUsageBarSegment {
    pub category: CacheCategory,
}

#[derive(Component)]
pub(crate) struct DiskUsageCategorySize {
    pub category: CacheCategory,
}

#[derive(Component)]
pub(crate) struct ClearCacheButton {
    pub category: CacheClearAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheCategory {
    Songs,
    Videos,
    Models,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheClearAction {
    All,
    Videos,
    Models,
}

impl CacheCategory {
    fn label(&self) -> &'static str {
        match self {
            Self::Songs => "Songs",
            Self::Videos => "Videos",
            Self::Models => "Models",
            Self::Other => "Other",
        }
    }

    fn color(&self, theme: &UiTheme) -> Color {
        match self {
            Self::Songs => theme.accent,
            Self::Videos => Color::srgb(0.28, 0.72, 0.42),
            Self::Models => Color::srgb(0.88, 0.62, 0.18),
            Self::Other => theme.text_dim,
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn build_disk_usage_widget(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    stats: &CacheStats,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(8.0), Val::Px(0.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                DiskUsageTotalLabel,
                Text::new(format!("{} used", format_bytes(stats.total()))),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(theme.text_primary),
            ));

            let total = stats.total().max(1) as f32;
            col.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|bar| {
                for cat in [
                    CacheCategory::Songs,
                    CacheCategory::Videos,
                    CacheCategory::Models,
                    CacheCategory::Other,
                ] {
                    let bytes = match cat {
                        CacheCategory::Songs => stats.songs_bytes,
                        CacheCategory::Videos => stats.videos_bytes,
                        CacheCategory::Models => stats.models_bytes,
                        CacheCategory::Other => stats.other_bytes,
                    };
                    let pct = (bytes as f32 / total * 100.0).max(0.0);
                    bar.spawn((
                        DiskUsageBarSegment { category: cat },
                        Node {
                            width: Val::Percent(pct),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(cat.color(theme)),
                    ));
                }
            });

            for cat in [
                CacheCategory::Songs,
                CacheCategory::Videos,
                CacheCategory::Models,
                CacheCategory::Other,
            ] {
                let bytes = match cat {
                    CacheCategory::Songs => stats.songs_bytes,
                    CacheCategory::Videos => stats.videos_bytes,
                    CacheCategory::Models => stats.models_bytes,
                    CacheCategory::Other => stats.other_bytes,
                };

                col.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: Val::Px(8.0),
                            height: Val::Px(8.0),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(cat.color(theme)),
                    ));
                    row.spawn((
                        Text::new(cat.label()),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.text_secondary),
                        Node {
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));
                    row.spawn((
                        DiskUsageCategorySize { category: cat },
                        Text::new(format_bytes(bytes)),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(theme.text_dim),
                    ));
                });
            }

            col.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            })
            .with_children(|row| {
                let clear_actions: &[(CacheClearAction, &str)] = &[
                    (CacheClearAction::All, "Clear All"),
                    (CacheClearAction::Videos, "Videos"),
                    (CacheClearAction::Models, "Models"),
                ];

                for &(action, label) in clear_actions {
                    let visible = match action {
                        CacheClearAction::All => stats.total() > 0,
                        CacheClearAction::Videos => stats.clearable_videos_bytes > 0,
                        CacheClearAction::Models => stats.models_bytes > 0,
                    };
                    let display = if visible { Display::Flex } else { Display::None };

                    row.spawn((
                        ClearCacheButton { category: action },
                        Button,
                        Node {
                            display,
                            flex_grow: 1.0,
                            padding: UiRect::new(
                                Val::Px(8.0),
                                Val::Px(8.0),
                                Val::Px(4.0),
                                Val::Px(4.0),
                            ),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme.sidebar_btn),
                        BorderColor::all(theme.text_dim.with_alpha(0.2)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 10.0,
                                ..default()
                            },
                            TextColor(theme.text_secondary),
                        ));
                    });
                }
            });
        });
}

pub fn update_disk_usage_display(
    stats: Res<CacheStats>,
    theme: Res<UiTheme>,
    mut total_query: Query<&mut Text, With<DiskUsageTotalLabel>>,
    mut bar_query: Query<(&DiskUsageBarSegment, &mut Node), Without<DiskUsageTotalLabel>>,
    mut size_query: Query<
        (&DiskUsageCategorySize, &mut Text),
        (Without<DiskUsageTotalLabel>, Without<DiskUsageBarSegment>),
    >,
    mut clear_query: Query<
        (&ClearCacheButton, &Children, &mut Node),
        (Without<DiskUsageTotalLabel>, Without<DiskUsageBarSegment>, Without<DiskUsageCategorySize>),
    >,
    mut text_colors: Query<&mut TextColor>,
) {
    if !stats.is_changed() {
        return;
    }

    for mut text in &mut total_query {
        **text = format!("{} used", format_bytes(stats.total()));
    }

    let total = stats.total().max(1) as f32;
    for (seg, mut node) in &mut bar_query {
        let bytes = match seg.category {
            CacheCategory::Songs => stats.songs_bytes,
            CacheCategory::Videos => stats.videos_bytes,
            CacheCategory::Models => stats.models_bytes,
            CacheCategory::Other => stats.other_bytes,
        };
        node.width = Val::Percent((bytes as f32 / total * 100.0).max(0.0));
    }

    for (cat_size, mut text) in &mut size_query {
        let bytes = match cat_size.category {
            CacheCategory::Songs => stats.songs_bytes,
            CacheCategory::Videos => stats.videos_bytes,
            CacheCategory::Models => stats.models_bytes,
            CacheCategory::Other => stats.other_bytes,
        };
        **text = format_bytes(bytes);
    }

    for (btn, children, mut node) in &mut clear_query {
        let (enabled, visible) = match btn.category {
            CacheClearAction::All => (stats.total() > 0, true),
            CacheClearAction::Videos => (stats.clearable_videos_bytes > 0, stats.clearable_videos_bytes > 0),
            CacheClearAction::Models => (stats.models_bytes > 0, stats.models_bytes > 0),
        };
        node.display = if visible { Display::Flex } else { Display::None };
        let color = if enabled {
            theme.text_secondary
        } else {
            theme.text_dim
        };
        for child in children.iter() {
            if let Ok(mut tc) = text_colors.get_mut(child) {
                tc.0 = color;
            }
        }
    }
}

pub fn handle_clear_cache_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &ClearCacheButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    cache: Res<CacheDir>,
    mut library: ResMut<crate::scanner::metadata::SongLibrary>,
    stats: Res<CacheStats>,
    theme: Res<UiTheme>,
) {
    for (interaction, btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                match btn.category {
                    CacheClearAction::All => {
                        if stats.total() == 0 {
                            continue;
                        }
                        cache.clear_all();
                        crate::vendor::clear_videos();
                        crate::vendor::clear_models();
                        for song in &mut library.songs {
                            song.analysis_status =
                                crate::scanner::metadata::AnalysisStatus::NotAnalyzed;
                            song.language = None;
                        }
                    }
                    CacheClearAction::Videos => {
                        if stats.clearable_videos_bytes == 0 {
                            continue;
                        }
                        crate::vendor::clear_videos();
                    }
                    CacheClearAction::Models => {
                        if stats.models_bytes == 0 {
                            continue;
                        }
                        crate::vendor::clear_models();
                    }
                }
                start_cache_stats_computation(&mut commands);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.sidebar_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.sidebar_btn);
            }
        }
    }
}

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
    cache_stats: &CacheStats,
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
                width: Val::Percent(80.0),
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

        build_disk_usage_widget(sidebar, theme, cache_stats);
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
                border_radius: BorderRadius::all(Val::Px(5.0)),
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
                border_radius: BorderRadius::all(Val::Px(5.0)),
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
    interaction_query: Query<
        (&Interaction, &SidebarButton),
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
    nav: Res<crate::input::NavInput>,
    mut focus: ResMut<super::MenuFocus>,
    menu_root_query: Query<Entity, With<super::MenuRoot>>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
        return;
    }

    let mut triggered_action: Option<SidebarAction> = None;

    if nav.confirm
        && focus.panel == super::FocusPanel::Sidebar
        && focus.sidebar_index < super::SIDEBAR_ACTIONS.len()
    {
        triggered_action = Some(super::SIDEBAR_ACTIONS[focus.sidebar_index]);
    }

    if triggered_action.is_none() {
        for (interaction, sidebar_btn) in &interaction_query {
            match interaction {
                Interaction::Pressed => {
                    triggered_action = Some(sidebar_btn.action);
                }
                Interaction::Hovered => {
                    if let Some(idx) = super::SIDEBAR_ACTIONS
                        .iter()
                        .position(|&a| a == sidebar_btn.action)
                    {
                        focus.panel = super::FocusPanel::Sidebar;
                        focus.sidebar_index = idx;
                        focus.active = true;
                    }
                }
                Interaction::None => {}
            }
        }
    }

    if let Some(action) = triggered_action {
        if action == SidebarAction::ToggleTheme {
            for entity in &menu_root_query {
                commands.entity(entity).despawn();
            }
        }
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
                        border_radius: BorderRadius::all(Val::Px(8.0)),
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
                            border_radius: BorderRadius::all(Val::Px(5.0)),
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
                            border_radius: BorderRadius::all(Val::Px(5.0)),
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
    nav: Res<crate::input::NavInput>,
    mut exit: MessageWriter<AppExit>,
    overlay_query: Query<Entity, With<ExitOverlay>>,
    cancel_events: Query<&Interaction, (With<ExitCancelButton>, Changed<Interaction>)>,
    confirm_events: Query<&Interaction, (With<ExitConfirmButton>, Changed<Interaction>)>,
    mut cancel_style: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<ExitCancelButton>, Without<ExitConfirmButton>),
    >,
    mut confirm_style: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<ExitConfirmButton>, Without<ExitCancelButton>),
    >,
    theme: Res<UiTheme>,
    mut exit_focus: Option<ResMut<ExitFocus>>,
    menu_state: Res<super::MenuState>,
    settings_query: Query<(), With<SettingsOverlay>>,
    profile_query: Query<(), With<ProfileOverlay>>,
    lang_picker_query: Query<(), With<super::song_card::LanguagePickerOverlay>>,
) {
    let overlay_entity = overlay_query.single();

    if overlay_entity.is_err() {
        if nav.back
            && menu_state.search_query.is_empty()
            && settings_query.is_empty()
            && profile_query.is_empty()
            && lang_picker_query.is_empty()
        {
            spawn_exit_popup(&mut commands, &theme);
        }
        return;
    }

    let overlay_entity = overlay_entity.unwrap();

    if nav.back {
        commands.entity(overlay_entity).despawn();
        commands.remove_resource::<ExitFocus>();
        return;
    }

    if let Some(ref mut ef) = exit_focus {
        if nav.up || nav.down || nav.left || nav.right
            || keyboard.just_pressed(KeyCode::Tab)
        {
            ef.0 = 1 - ef.0;
        }

        if nav.confirm {
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

    for interaction in &cancel_events {
        match interaction {
            Interaction::Pressed => {
                commands.entity(overlay_entity).despawn();
                commands.remove_resource::<ExitFocus>();
                return;
            }
            Interaction::Hovered => {
                if let Some(ref mut ef) = exit_focus {
                    ef.0 = 0;
                }
            }
            Interaction::None => {}
        }
    }

    for interaction in &confirm_events {
        match interaction {
            Interaction::Pressed => {
                exit.write(AppExit::Success);
                return;
            }
            Interaction::Hovered => {
                if let Some(ref mut ef) = exit_focus {
                    ef.0 = 1;
                }
            }
            Interaction::None => {}
        }
    }

    if let Ok((mut bg, mut border)) = cancel_style.single_mut() {
        let focused = exit_focus.as_ref().map(|f| f.0) == Some(0);
        bg.set_if_neq(if focused {
            BackgroundColor(theme.accent_hover)
        } else {
            BackgroundColor(theme.accent)
        });
        border.set_if_neq(if focused {
            BorderColor::all(theme.accent)
        } else {
            BorderColor::all(Color::NONE)
        });
    }

    if let Ok((mut bg, mut border)) = confirm_style.single_mut() {
        let focused = exit_focus.as_ref().map(|f| f.0) == Some(1);
        bg.set_if_neq(if focused {
            BackgroundColor(theme.popup_btn_hover)
        } else {
            BackgroundColor(theme.popup_btn)
        });
        border.set_if_neq(if focused {
            BorderColor::all(theme.accent)
        } else {
            BorderColor::all(Color::NONE)
        });
    }
}
