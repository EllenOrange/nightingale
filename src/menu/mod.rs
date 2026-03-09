pub mod song_card;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageType};
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::WindowMode;

use crate::analyzer::cache::CacheDir;
use crate::analyzer::{AnalysisQueue, PlayTarget};
use crate::scanner::metadata::{AnalysisStatus, Song, SongLibrary};
use crate::states::AppState;
use crate::ui::{self, UiTheme};
use song_card::*;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuState>()
            .add_systems(
                OnEnter(AppState::Menu),
                (load_album_art, build_menu).chain(),
            )
            .add_systems(
                Update,
                (
                    handle_song_click,
                    handle_reanalyze_click,
                    handle_search_input,
                    update_status_badges,
                    handle_sidebar_click,
                    handle_settings_click,
                    poll_folder_result,
                    poll_rescan,
                )
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(OnExit(AppState::Menu), cleanup_menu);
    }
}

#[derive(Resource, Default)]
struct MenuState {
    search_query: String,
}

#[derive(Resource)]
struct AlbumArtCache {
    handles: Vec<Option<Handle<Image>>>,
}

#[derive(Resource)]
struct PendingFolderPick {
    result: Arc<Mutex<Option<Option<PathBuf>>>>,
}

#[derive(Resource)]
struct PendingRescan {
    result: Arc<Mutex<Option<Vec<Song>>>>,
}

fn load_album_art(
    mut commands: Commands,
    library: Res<SongLibrary>,
    mut images: ResMut<Assets<Image>>,
) {
    let handles: Vec<Option<Handle<Image>>> = library
        .songs
        .iter()
        .map(|song| {
            song.album_art.as_ref().and_then(|bytes| {
                Image::from_buffer(
                    bytes,
                    ImageType::MimeType("image/jpeg"),
                    default(),
                    true,
                    ImageSampler::default(),
                    RenderAssetUsages::RENDER_WORLD,
                )
                .ok()
                .or_else(|| {
                    Image::from_buffer(
                        bytes,
                        ImageType::MimeType("image/png"),
                        default(),
                        true,
                        ImageSampler::default(),
                        RenderAssetUsages::RENDER_WORLD,
                    )
                    .ok()
                })
                .map(|img| images.add(img))
            })
        })
        .collect();
    commands.insert_resource(AlbumArtCache { handles });
}

#[derive(Resource, Clone)]
pub struct IconFont(pub Handle<Font>);

pub const FA_REFRESH: &str = "\u{f021}";
pub const FA_SUN: &str = "\u{f185}";
pub const FA_MOON: &str = "\u{f186}";
pub const FA_SPINNER: &str = "\u{f1ce}";

#[derive(Component)]
struct MenuRoot;

fn build_menu(
    mut commands: Commands,
    library: Res<SongLibrary>,
    menu_state: Res<MenuState>,
    art_cache: Res<AlbumArtCache>,
    theme: Res<UiTheme>,
    config: Res<crate::config::AppConfig>,
    asset_server: Res<AssetServer>,
) {
    let has_folder = config.last_folder.as_ref().is_some_and(|f| f.is_dir());

    let logo_handle: Handle<Image> = asset_server.load("images/logo.png");
    let icon_font = IconFont(asset_server.load("fonts/fa-solid-900.ttf"));
    commands.insert_resource(icon_font.clone());

    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(theme.bg),
        ))
        .with_children(|root| {
            build_sidebar(root, &theme, has_folder, logo_handle, &icon_font);
            build_main_area(root, &library, &menu_state, &art_cache, &theme, &icon_font);
        });
}

fn build_sidebar(
    root: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    has_folder: bool,
    logo: Handle<Image>,
    icon_font: &IconFont,
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
                width: Val::Px(180.0),
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
                row.spawn((
                    SidebarButton {
                        action: SidebarAction::ToggleTheme,
                    },
                    ThemeToggleIcon,
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
                        Text::new(theme_glyph),
                        TextFont {
                            font: icon_font.0.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme.text_primary),
                    ));
                });
            });

        spawn_sidebar_button(sidebar, "Exit", SidebarAction::Exit, theme, true);
    });
}

fn spawn_sidebar_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: SidebarAction,
    theme: &UiTheme,
    enabled: bool,
) {
    let bg = if enabled {
        theme.sidebar_btn
    } else {
        theme.sidebar_btn
    };
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
            BackgroundColor(bg),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, 13.0, text_color);
        });
}

fn build_main_area(
    root: &mut ChildSpawnerCommands,
    library: &SongLibrary,
    menu_state: &MenuState,
    art_cache: &AlbumArtCache,
    theme: &UiTheme,
    icon_font: &IconFont,
) {
    root.spawn(Node {
        flex_grow: 1.0,
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::all(Val::Px(20.0)),
        ..default()
    })
    .with_children(|main| {
        if library.songs.is_empty() {
            build_empty_state(main, theme);
            return;
        }

        main.spawn((
            Node {
                width: Val::Px(600.0),
                height: Val::Px(44.0),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(Val::Px(16.0)),
                margin: UiRect::bottom(Val::Px(20.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(theme.card_bg),
        ))
        .with_children(|bar| {
            let display_text = if menu_state.search_query.is_empty() {
                "Type to search songs..."
            } else {
                &menu_state.search_query
            };
            bar.spawn((
                SearchText,
                Text::new(display_text),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(theme.text_secondary),
            ));
        });

        let ready_count = library
            .songs
            .iter()
            .filter(|s| s.analysis_status == AnalysisStatus::Ready)
            .count();
        main.spawn((
            StatsText,
            Text::new(format!(
                "{} songs found · {} ready for karaoke",
                library.songs.len(),
                ready_count
            )),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(theme.text_secondary),
            Node {
                flex_shrink: 0.0,
                margin: UiRect::bottom(Val::Px(16.0)),
                ..default()
            },
        ));

        main.spawn((
            SongListRoot,
            Node {
                width: Val::Px(700.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|list| {
            let query = menu_state.search_query.to_lowercase();
            for (i, song) in library.songs.iter().enumerate() {
                if !query.is_empty() {
                    let matches = song.display_title().to_lowercase().contains(&query)
                        || song.display_artist().to_lowercase().contains(&query);
                    if !matches {
                        continue;
                    }
                }
                let art = art_cache.handles.get(i).and_then(|h| h.clone());
                build_song_card(list, song, i, art, theme, icon_font);
            }
        });
    });
}

fn build_empty_state(parent: &mut ChildSpawnerCommands, theme: &UiTheme) {
    parent
        .spawn((
            EmptyStateRoot,
            Node {
                flex_grow: 1.0,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
        ))
        .with_children(|empty| {
            empty.spawn((
                Text::new("♪"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(theme.text_dim),
            ));
            empty.spawn((
                Text::new("No songs loaded"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(theme.text_secondary),
            ));
            empty.spawn((
                Text::new("Select a music folder to get started"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(theme.text_dim),
            ));
        });
}

fn handle_song_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SongCard, &mut BackgroundColor, &mut BorderColor),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    mut next_state: ResMut<NextState<AppState>>,
    mut queue: ResMut<AnalysisQueue>,
    theme: Res<UiTheme>,
    overlay_query: Query<(), With<SettingsOverlay>>,
) {
    if !overlay_query.is_empty() {
        return;
    }
    for (interaction, song_card, mut bg, mut border) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                let idx = song_card.song_index;
                match library.songs[idx].analysis_status {
                    AnalysisStatus::Ready => {
                        commands.insert_resource(PlayTarget { song_index: idx });
                        next_state.set(AppState::Playing);
                    }
                    AnalysisStatus::NotAnalyzed | AnalysisStatus::Failed(_) => {
                        queue.enqueue(idx);
                        library.songs[idx].analysis_status = AnalysisStatus::Queued;
                    }
                    AnalysisStatus::Queued | AnalysisStatus::Analyzing => {}
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.card_hover);
                *border = BorderColor::all(theme.accent);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.card_bg);
                *border = BorderColor::all(Color::NONE);
            }
        }
    }
}

fn handle_reanalyze_click(
    mut interaction_query: Query<
        (&Interaction, &ReanalyzeButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
    cache: Res<CacheDir>,
    theme: Res<UiTheme>,
    overlay_query: Query<(), With<SettingsOverlay>>,
) {
    if !overlay_query.is_empty() {
        return;
    }
    for (interaction, btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                let idx = btn.song_index;
                if idx >= library.songs.len() {
                    continue;
                }
                let hash = &library.songs[idx].file_hash;
                let transcript = cache.transcript_path(hash);
                if transcript.is_file() {
                    let _ = std::fs::remove_file(&transcript);
                }
                library.songs[idx].analysis_status = AnalysisStatus::Queued;
                queue.enqueue(idx);
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

fn handle_sidebar_click(
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
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !overlay_query.is_empty() {
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
                            let cache = CacheDir { path: cache_path };
                            let songs = crate::scanner::scan_folder(&folder, &cache);
                            *result_clone.lock().unwrap() = Some(songs);
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
                SidebarAction::ToggleTheme => {
                    theme.toggle();
                    config.dark_mode = Some(theme.mode == crate::ui::ThemeMode::Dark);
                    config.save();
                    rebuild_menu(&mut commands, &mut next_state);
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

fn rebuild_menu(_commands: &mut Commands, next_state: &mut ResMut<NextState<AppState>>) {
    next_state.set(AppState::Menu);
}

fn poll_folder_result(
    mut commands: Commands,
    pending: Option<Res<PendingFolderPick>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut config: ResMut<crate::config::AppConfig>,
    mut queue: ResMut<AnalysisQueue>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    if let Some(ref maybe_folder) = *lock {
        if let Some(folder) = maybe_folder {
            info!("Selected folder: {}", folder.display());
            commands.insert_resource(SongLibrary { songs: vec![] });
            queue.queue.clear();
            queue.active = None;
            commands.insert_resource(crate::scanner::ScanRequest {
                folder: folder.clone(),
            });
            config.last_folder = Some(folder.clone());
            config.save();
            next_state.set(AppState::Scanning);
        }
        drop(lock);
        commands.remove_resource::<PendingFolderPick>();
    }
}

fn poll_rescan(
    mut commands: Commands,
    pending: Option<Res<PendingRescan>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    let Some(ref new_songs) = *lock else { return };

    let mut status_by_hash: std::collections::HashMap<String, AnalysisStatus> =
        std::collections::HashMap::new();
    for song in &library.songs {
        match &song.analysis_status {
            AnalysisStatus::Queued | AnalysisStatus::Analyzing => {
                status_by_hash.insert(song.file_hash.clone(), song.analysis_status.clone());
            }
            _ => {}
        }
    }

    let old_active_hash = queue
        .active
        .as_ref()
        .and_then(|a| library.songs.get(a.song_index))
        .map(|s| s.file_hash.clone());

    let old_queued_hashes: Vec<String> = queue
        .queue
        .iter()
        .filter_map(|&idx| library.songs.get(idx))
        .map(|s| s.file_hash.clone())
        .collect();

    let mut merged = new_songs.clone();
    for song in &mut merged {
        if let Some(status) = status_by_hash.get(&song.file_hash) {
            song.analysis_status = status.clone();
        }
    }

    let hash_to_new_idx: std::collections::HashMap<&str, usize> = merged
        .iter()
        .enumerate()
        .map(|(i, s)| (s.file_hash.as_str(), i))
        .collect();

    if let Some(ref mut active) = queue.active {
        if let Some(ref old_hash) = old_active_hash {
            if let Some(&new_idx) = hash_to_new_idx.get(old_hash.as_str()) {
                active.song_index = new_idx;
            }
        }
    }

    let mut new_queue = std::collections::VecDeque::new();
    for hash in &old_queued_hashes {
        if let Some(&new_idx) = hash_to_new_idx.get(hash.as_str()) {
            new_queue.push_back(new_idx);
        }
    }
    queue.queue = new_queue;

    library.songs = merged;

    drop(lock);
    commands.remove_resource::<PendingRescan>();

    rebuild_menu(&mut commands, &mut next_state);
}

fn handle_search_input(
    mut key_events: MessageReader<KeyboardInput>,
    mut menu_state: ResMut<MenuState>,
    mut search_text_query: Query<&mut Text, With<SearchText>>,
    library: Res<SongLibrary>,
    song_list_query: Query<Entity, With<SongListRoot>>,
    mut commands: Commands,
    art_cache: Res<AlbumArtCache>,
    theme: Res<UiTheme>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    icon_font: Res<IconFont>,
) {
    if !overlay_query.is_empty() {
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

    if let Ok(mut text) = search_text_query.single_mut() {
        if menu_state.search_query.is_empty() {
            **text = "Type to search songs...".into();
        } else {
            **text = menu_state.search_query.clone();
        }
    }

    if let Ok(list_entity) = song_list_query.single() {
        populate_song_list(
            &mut commands,
            list_entity,
            &library.songs,
            &menu_state.search_query,
            &art_cache.handles,
            &theme,
            &icon_font,
        );
    }
}

fn update_status_badges(
    library: Res<SongLibrary>,
    queue: Res<AnalysisQueue>,
    time: Res<Time>,
    theme: Res<UiTheme>,
    mut badge_query: Query<(&StatusBadge, &mut BackgroundColor), Without<SpinnerOverlay>>,
    mut badge_text_query: Query<(&BadgeText, &mut Text), Without<StatsText>>,
    mut stats_query: Query<&mut Text, With<StatsText>>,
    mut spinner_query: Query<
        (&SpinnerOverlay, &mut Visibility, &mut BackgroundColor),
        (Without<ReanalyzeButton>, Without<StatusBadge>),
    >,
    mut reanalyze_query: Query<(&ReanalyzeButton, &mut Visibility), Without<SpinnerOverlay>>,
) {
    for (badge, mut bg) in &mut badge_query {
        if badge.song_index >= library.songs.len() {
            continue;
        }
        let color = match &library.songs[badge.song_index].analysis_status {
            AnalysisStatus::Ready => theme.badge_ready,
            AnalysisStatus::NotAnalyzed => theme.badge_not_analyzed,
            AnalysisStatus::Queued => theme.badge_queued,
            AnalysisStatus::Analyzing => theme.badge_analyzing,
            AnalysisStatus::Failed(_) => theme.badge_failed,
        };
        *bg = BackgroundColor(color);
    }

    for (bt, mut text) in &mut badge_text_query {
        if bt.song_index >= library.songs.len() {
            continue;
        }
        let new_text = match &library.songs[bt.song_index].analysis_status {
            AnalysisStatus::Ready => "READY".into(),
            AnalysisStatus::NotAnalyzed => "NOT ANALYZED".into(),
            AnalysisStatus::Queued => "QUEUED".into(),
            AnalysisStatus::Analyzing => {
                if let Some(info) = queue.active_progress(bt.song_index) {
                    format!("{}%", info.percent)
                } else {
                    "ANALYZING...".into()
                }
            }
            AnalysisStatus::Failed(_) => "FAILED".into(),
        };
        **text = new_text;
    }

    if let Ok(mut stats) = stats_query.single_mut() {
        let ready_count = library
            .songs
            .iter()
            .filter(|s| s.analysis_status == AnalysisStatus::Ready)
            .count();
        **stats = format!(
            "{} songs found · {} ready for karaoke",
            library.songs.len(),
            ready_count
        );
    }

    let spinner_alpha = (time.elapsed_secs() * 3.0).sin() * 0.25 + 0.75;

    for (spinner, mut vis, mut bg) in &mut spinner_query {
        if spinner.song_index >= library.songs.len() {
            continue;
        }
        let analyzing =
            library.songs[spinner.song_index].analysis_status == AnalysisStatus::Analyzing;
        *vis = if analyzing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if analyzing {
            *bg = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, spinner_alpha));
        }
    }

    for (btn, mut vis) in &mut reanalyze_query {
        if btn.song_index >= library.songs.len() {
            continue;
        }
        *vis = if matches!(
            library.songs[btn.song_index].analysis_status,
            AnalysisStatus::Ready | AnalysisStatus::Failed(_)
        ) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn spawn_settings_popup(
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
                            margin: UiRect::bottom(Val::Px(12.0)),
                            ..default()
                        },
                    ));

                    spawn_settings_section(card, theme, "General");
                    let fs_label = if config.is_fullscreen() { "Fullscreen" } else { "Windowed" };
                    spawn_settings_row(card, theme, "Window", fs_label, SettingsFullscreenText,
                        &[("Switch", SettingsAction::ToggleFullscreen)],
                        "Toggle between fullscreen and windowed mode");

                    spawn_settings_section(card, theme, "Analyzer");
                    spawn_settings_row(card, theme, "Model", config.whisper_model(), SettingsModelText,
                        &[("Switch", SettingsAction::ToggleModel)],
                        "v3 is more accurate but slower, turbo is faster");
                    spawn_settings_row(card, theme, "Beam size", &config.beam_size().to_string(), SettingsBeamText,
                        &[("-", SettingsAction::BeamDown), ("+", SettingsAction::BeamUp)],
                        "Higher values improve accuracy at the cost of speed");
                    spawn_settings_row(card, theme, "Batch size", &config.batch_size().to_string(), SettingsBatchText,
                        &[("-", SettingsAction::BatchDown), ("+", SettingsAction::BatchUp)],
                        "Higher values use more memory but process faster");

                    card.spawn((
                        Text::new("Changes apply to future analyses. Use the re-analyze button on song cards to apply."),
                        TextFont { font_size: 12.0, ..default() },
                        TextColor(theme.text_dim),
                        Node {
                            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(8.0), Val::Px(4.0)),
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
    marker: impl Component,
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
                    BackgroundColor(theme.card_bg),
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
            BackgroundColor(theme.sidebar_btn),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, font_size, theme.text_primary);
        });
}

fn handle_settings_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SettingsButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut config: ResMut<crate::config::AppConfig>,
    overlay_query: Query<Entity, With<SettingsOverlay>>,
    mut model_text: Query<&mut Text, (With<SettingsModelText>, Without<SettingsBeamText>, Without<SettingsBatchText>, Without<SettingsFullscreenText>)>,
    mut beam_text: Query<&mut Text, (With<SettingsBeamText>, Without<SettingsModelText>, Without<SettingsBatchText>, Without<SettingsFullscreenText>)>,
    mut batch_text: Query<&mut Text, (With<SettingsBatchText>, Without<SettingsModelText>, Without<SettingsBeamText>, Without<SettingsFullscreenText>)>,
    mut fs_text: Query<&mut Text, (With<SettingsFullscreenText>, Without<SettingsModelText>, Without<SettingsBeamText>, Without<SettingsBatchText>)>,
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
                            if let Ok(mut text) = fs_text.single_mut() {
                                **text = new_label.to_string();
                            }
                        }
                    }
                    SettingsAction::ToggleModel => {
                        let new_model = if config.whisper_model() == "large-v3-turbo" {
                            "large-v3"
                        } else {
                            "large-v3-turbo"
                        };
                        config.whisper_model = Some(new_model.to_string());
                        config.save();
                        if let Ok(mut text) = model_text.single_mut() {
                            **text = new_model.to_string();
                        }
                    }
                    SettingsAction::BeamUp => {
                        let new_val = (config.beam_size() + 1).min(15);
                        config.beam_size = Some(new_val);
                        config.save();
                        if let Ok(mut text) = beam_text.single_mut() {
                            **text = new_val.to_string();
                        }
                    }
                    SettingsAction::BeamDown => {
                        let new_val = config.beam_size().saturating_sub(1).max(1);
                        config.beam_size = Some(new_val);
                        config.save();
                        if let Ok(mut text) = beam_text.single_mut() {
                            **text = new_val.to_string();
                        }
                    }
                    SettingsAction::BatchUp => {
                        let new_val = (config.batch_size() + 1).min(16);
                        config.batch_size = Some(new_val);
                        config.save();
                        if let Ok(mut text) = batch_text.single_mut() {
                            **text = new_val.to_string();
                        }
                    }
                    SettingsAction::BatchDown => {
                        let new_val = config.batch_size().saturating_sub(1).max(1);
                        config.batch_size = Some(new_val);
                        config.save();
                        if let Ok(mut text) = batch_text.single_mut() {
                            **text = new_val.to_string();
                        }
                    }
                    SettingsAction::RestoreDefaults => {
                        config.whisper_model = None;
                        config.beam_size = None;
                        config.batch_size = None;
                        config.fullscreen = None;
                        config.save();

                        if let Ok(mut text) = model_text.single_mut() {
                            **text = config.whisper_model().to_string();
                        }
                        if let Ok(mut text) = beam_text.single_mut() {
                            **text = config.beam_size().to_string();
                        }
                        if let Ok(mut text) = batch_text.single_mut() {
                            **text = config.batch_size().to_string();
                        }
                        if let Ok(mut window) = windows.single_mut() {
                            window.mode = if config.is_fullscreen() {
                                WindowMode::BorderlessFullscreen(
                                    bevy::window::MonitorSelection::Current,
                                )
                            } else {
                                WindowMode::Windowed
                            };
                        }
                        if let Ok(mut text) = fs_text.single_mut() {
                            **text = if config.is_fullscreen() {
                                "Fullscreen"
                            } else {
                                "Windowed"
                            }
                            .to_string();
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
                *bg = BackgroundColor(theme.sidebar_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.sidebar_btn);
            }
        }
    }
}

fn cleanup_menu(
    mut commands: Commands,
    query: Query<Entity, With<MenuRoot>>,
    settings_query: Query<Entity, With<SettingsOverlay>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    for entity in &settings_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<AlbumArtCache>();
    commands.remove_resource::<IconFont>();
}
