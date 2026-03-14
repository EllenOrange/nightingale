pub mod about;
pub mod components;
pub mod folder;
mod nav;
pub mod profile;
mod scroll;
mod search;
pub mod settings;
pub mod sidebar;
pub mod song_card;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageType};
use bevy::prelude::*;

use crate::analyzer::cache::CacheDir;
use crate::analyzer::{AnalysisQueue, PlayTarget};
use crate::config::AppConfig;
use crate::scanner::metadata::{AnalysisStatus, SongLibrary, TranscriptSource};
use crate::profile::ProfileStore;
use crate::states::AppState;
use crate::ui::UiTheme;
use components::*;
use song_card::*;

#[derive(Default, PartialEq, Clone, Copy)]
pub enum FocusPanel {
    #[default]
    SongList,
    Sidebar,
}

#[derive(Resource)]
pub struct MenuFocus {
    pub panel: FocusPanel,
    pub song_index: usize,
    pub sidebar_index: usize,
    pub nav_lock: u8,
    pub analyze_all_focused: bool,
    pub active: bool,
}

impl Default for MenuFocus {
    fn default() -> Self {
        Self {
            panel: FocusPanel::SongList,
            song_index: 0,
            sidebar_index: 0,
            nav_lock: 0,
            analyze_all_focused: false,
            active: false,
        }
    }
}

pub const SIDEBAR_ACTIONS: &[SidebarAction] = &[
    SidebarAction::ChangeFolder,
    SidebarAction::RescanFolder,
    SidebarAction::Profile,
    SidebarAction::Settings,
    SidebarAction::ToggleTheme,
    SidebarAction::Exit,
];

#[derive(Resource, Default)]
pub struct AnyOverlayOpen(pub bool);

fn update_overlay_state(
    mut overlay_open: ResMut<AnyOverlayOpen>,
    settings: Query<(), With<SettingsOverlay>>,
    profile: Query<(), With<ProfileOverlay>>,
    exit: Query<(), With<sidebar::ExitOverlay>>,
    lang_picker: Query<(), With<LanguagePickerOverlay>>,
    about: Query<(), With<AboutOverlay>>,
) {
    overlay_open.0 = !settings.is_empty()
        || !profile.is_empty()
        || !exit.is_empty()
        || !lang_picker.is_empty()
        || !about.is_empty();
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuState>()
            .init_resource::<MenuFocus>()
            .init_resource::<nav::NavRepeat>()
            .init_resource::<AnyOverlayOpen>()
            .init_resource::<sidebar::CacheStats>()
            .init_resource::<sidebar::CacheStatsTimer>()
            .add_systems(
                OnEnter(AppState::Menu),
                (load_album_art, build_menu, kick_off_cache_stats).chain(),
            )
            .add_systems(
                Update,
                update_overlay_state.run_if(in_state(AppState::Menu)),
            )
            .add_systems(
                Update,
                (
                    handle_song_click,
                    handle_reanalyze_click,
                    handle_delete_cache_click,
                    handle_analyze_all_click,
                    handle_language_button_click,
                    handle_language_picker_interaction,
                    search::handle_search_input,
                    update_status_badges,
                    animate_spinners,
                    update_analysis_hint,
                    sidebar::handle_sidebar_click,
                    sidebar::handle_exit_input,
                    sidebar::poll_cache_stats,
                    sidebar::update_disk_usage_display,
                    sidebar::handle_clear_cache_click,
                    settings::handle_settings_click,
                    profile::handle_profile_click,
                    about::handle_about_click,
                    folder::poll_folder_result,
                    folder::poll_rescan,
                )
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(
                Update,
                sidebar::update_about_link_hover
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(
                Update,
                (
                    nav::clear_focus_on_empty_hover
                        .before(nav::handle_menu_nav),
                    nav::handle_menu_nav
                        .after(handle_song_click)
                        .after(sidebar::handle_sidebar_click),
                    nav::apply_menu_focus_styling
                        .after(nav::handle_menu_nav)
                        .after(handle_song_click)
                        .after(sidebar::handle_sidebar_click)
                        .after(nav::clear_focus_on_empty_hover),
                    nav::scroll_to_focused.after(nav::handle_menu_nav),
                )
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(
                Update,
                profile::handle_profile_input
                    .run_if(in_state(AppState::Menu))
                    .run_if(resource_exists::<profile::ProfileInputState>),
            )
            .add_systems(Update, scroll::send_scroll_events)
            .add_observer(scroll::on_scroll_handler)
            .add_systems(OnExit(AppState::Menu), cleanup_menu);
    }
}

#[derive(Resource, Default)]
pub(crate) struct MenuState {
    pub(crate) search_query: String,
}

#[derive(Resource)]
struct AlbumArtCache {
    handles: Vec<Option<Handle<Image>>>,
}

#[derive(Resource, Clone)]
pub struct IconFont(pub Handle<Font>);

pub const FA_REFRESH: &str = "\u{f021}";
pub const FA_SUN: &str = "\u{f0eb}";
pub const FA_MOON: &str = "\u{f186}";
pub const FA_USER: &str = "\u{f007}";

#[derive(Component)]
pub(crate) struct MenuRoot;

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

fn kick_off_cache_stats(mut commands: Commands) {
    sidebar::start_cache_stats_computation(&mut commands);
}

fn build_menu(
    mut commands: Commands,
    library: Res<SongLibrary>,
    menu_state: Res<MenuState>,
    art_cache: Res<AlbumArtCache>,
    theme: Res<UiTheme>,
    config: Res<crate::config::AppConfig>,
    asset_server: Res<AssetServer>,
    profiles: Res<ProfileStore>,
    cache_stats: Res<sidebar::CacheStats>,
    mut focus: ResMut<MenuFocus>,
) {
    *focus = MenuFocus::default();
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
            sidebar::build_sidebar(root, &theme, has_folder, logo_handle, &icon_font, &profiles, &cache_stats);
            build_main_area(root, &library, &menu_state, &art_cache, &theme, &icon_font, &profiles);
        });
}

fn build_main_area(
    root: &mut ChildSpawnerCommands,
    library: &SongLibrary,
    menu_state: &MenuState,
    art_cache: &AlbumArtCache,
    theme: &UiTheme,
    icon_font: &IconFont,
    profiles: &ProfileStore,
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

        main.spawn(Node {
            width: Val::Px(crate::ui::layout::MAIN_CONTENT_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            margin: UiRect::bottom(Val::Px(20.0)),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|top_row| {
            top_row.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(44.0),
                    padding: UiRect::horizontal(Val::Px(16.0)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.card_bg),
            ))
            .with_children(|bar| {
                let (display_text, text_color) = if menu_state.search_query.is_empty() {
                    ("Type to search songs...", theme.text_dim)
                } else {
                    (menu_state.search_query.as_str(), theme.text_primary)
                };
                bar.spawn((
                    SearchText,
                    Text::new(display_text),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(text_color),
                ));
            });

            let has_unanalyzed = library.songs.iter().any(|s| {
                matches!(s.analysis_status, AnalysisStatus::NotAnalyzed | AnalysisStatus::Failed(_))
            });
            let (btn_bg, btn_text_color) = if has_unanalyzed {
                (theme.surface_hover, theme.text_primary)
            } else {
                (theme.surface, theme.text_dim)
            };

            top_row.spawn((
                AnalyzeAllButton,
                Button,
                Node {
                    height: Val::Px(44.0),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(Val::Px(16.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor::all(Color::NONE),
                BackgroundColor(btn_bg),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Analyze All"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(btn_text_color),
                ));
            });
        });

        let ready_count = library
            .songs
            .iter()
            .filter(|s| matches!(s.analysis_status, AnalysisStatus::Ready(_)))
            .count();
        let video_count = library.songs.iter().filter(|s| s.is_video).count();
        let audio_count = library.songs.len() - video_count;
        let found_text = if video_count > 0 {
            format!("{audio_count} songs, {video_count} videos found")
        } else {
            format!("{} songs found", library.songs.len())
        };
        main.spawn((
            StatsText,
            Text::new(format!(
                "{found_text} · {ready_count} ready for karaoke"
            )),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(theme.text_secondary),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
        ));

        main.spawn((
            AnalysisHint,
            Text::new("First analysis per language downloads additional models and may take longer."),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(theme.text_dim),
            Node {
                flex_shrink: 0.0,
                margin: UiRect::bottom(Val::Px(12.0)),
                ..default()
            },
            Visibility::Hidden,
        ));

        main.spawn((
            SongListRoot,
            Node {
                width: Val::Px(crate::ui::layout::MAIN_CONTENT_WIDTH),
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
            let active_profile = profiles.active.as_deref();
            for (i, song) in library.songs.iter().enumerate() {
                let visible = query.is_empty()
                    || song.display_title().to_lowercase().contains(&query)
                    || song.display_artist().to_lowercase().contains(&query);
                let art = art_cache.handles.get(i).and_then(|h| h.clone());
                let best = active_profile
                    .and_then(|p| profiles.best_score(&song.file_hash, p));
                build_song_card(list, song, i, art, theme, icon_font, visible, best);
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
    interaction_query: Query<
        (&Interaction, &SongCard),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    mut next_state: ResMut<NextState<AppState>>,
    mut queue: ResMut<AnalysisQueue>,
    overlay_open: Res<AnyOverlayOpen>,
    mut focus: ResMut<MenuFocus>,
    nav: Res<crate::input::NavInput>,
) {
    if overlay_open.0 {
        return;
    }
    for (interaction, song_card) in &interaction_query {
        match interaction {
            Interaction::Pressed => {
                activate_song(song_card.song_index, &mut commands, &mut library, &mut next_state, &mut queue);
            }
            Interaction::Hovered => {
                if focus.nav_lock == 0 {
                    focus.panel = FocusPanel::SongList;
                    focus.song_index = song_card.song_index;
                    focus.analyze_all_focused = false;
                    focus.active = true;
                }
            }
            Interaction::None => {}
        }
    }

    if nav.confirm && focus.panel == FocusPanel::SongList && focus.analyze_all_focused {
        for (i, song) in library.songs.iter_mut().enumerate() {
            if matches!(song.analysis_status, AnalysisStatus::NotAnalyzed | AnalysisStatus::Failed(_)) {
                song.analysis_status = AnalysisStatus::Queued;
                queue.enqueue(i);
            }
        }
        return;
    }

    if nav.confirm && focus.panel == FocusPanel::SongList {
        activate_song(focus.song_index, &mut commands, &mut library, &mut next_state, &mut queue);
    }
}

fn activate_song(
    idx: usize,
    commands: &mut Commands,
    library: &mut SongLibrary,
    next_state: &mut NextState<AppState>,
    queue: &mut AnalysisQueue,
) {
    if idx >= library.songs.len() {
        return;
    }
    match library.songs[idx].analysis_status {
        AnalysisStatus::Ready(_) => {
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

fn handle_reanalyze_click(
    mut interaction_query: Query<
        (&Interaction, &ReanalyzeButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
    cache: Res<CacheDir>,
    theme: Res<UiTheme>,
    overlay_open: Res<AnyOverlayOpen>,
) {
    if overlay_open.0 {
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
                *bg = BackgroundColor(theme.surface_hover);
            }
        }
    }
}

fn handle_delete_cache_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &DeleteCacheButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    cache: Res<CacheDir>,
    theme: Res<UiTheme>,
    overlay_open: Res<AnyOverlayOpen>,
) {
    if overlay_open.0 {
        return;
    }
    for (interaction, btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => {
                let idx = btn.song_index;
                if idx >= library.songs.len() {
                    continue;
                }
                let song = &mut library.songs[idx];
                if !matches!(
                    song.analysis_status,
                    AnalysisStatus::Ready(_) | AnalysisStatus::Failed(_)
                ) {
                    continue;
                }
                cache.delete_song_cache(&song.file_hash);
                song.analysis_status = AnalysisStatus::NotAnalyzed;
                song.language = None;
                sidebar::start_cache_stats_computation(&mut commands);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.sidebar_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.surface_hover);
            }
        }
    }
}

fn handle_analyze_all_click(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<AnalyzeAllButton>),
    >,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
    theme: Res<UiTheme>,
    overlay_open: Res<AnyOverlayOpen>,
) {
    if overlay_open.0 {
        return;
    }
    let has_unanalyzed = library.songs.iter().any(|s| {
        matches!(s.analysis_status, AnalysisStatus::NotAnalyzed | AnalysisStatus::Failed(_))
    });
    for (interaction, mut bg) in &mut interaction_query {
        if !has_unanalyzed {
            *bg = BackgroundColor(theme.surface);
            continue;
        }
        match interaction {
            Interaction::Pressed => {
                for (i, song) in library.songs.iter_mut().enumerate() {
                    if matches!(song.analysis_status, AnalysisStatus::NotAnalyzed | AnalysisStatus::Failed(_)) {
                        song.analysis_status = AnalysisStatus::Queued;
                        queue.enqueue(i);
                    }
                }
                *bg = BackgroundColor(theme.surface);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.card_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.surface_hover);
            }
        }
    }
}

fn handle_language_button_click(
    mut commands: Commands,
    interaction_query: Query<
        (&Interaction, &LanguageButton),
        Changed<Interaction>,
    >,
    mut badge_inner_query: Query<
        (&LanguageBadgeInner, &mut BackgroundColor, &mut BorderColor),
    >,
    theme: Res<UiTheme>,
    icon_font: Res<IconFont>,
    picker_query: Query<(), With<LanguagePickerOverlay>>,
    library: Res<SongLibrary>,
) {
    if !picker_query.is_empty() {
        return;
    }
    for (interaction, btn) in &interaction_query {
        let disabled = btn.song_index < library.songs.len()
            && matches!(
                library.songs[btn.song_index].analysis_status,
                AnalysisStatus::Analyzing | AnalysisStatus::Queued
            );

        match interaction {
            Interaction::Pressed => {
                if !disabled {
                    spawn_language_picker(&mut commands, btn.song_index, &theme, &icon_font);
                }
            }
            Interaction::Hovered => {
                if let Some((_, mut bg, mut border)) = badge_inner_query
                    .iter_mut()
                    .find(|(b, _, _)| b.song_index == btn.song_index)
                {
                    if disabled {
                        *bg = BackgroundColor(theme.accent.with_alpha(0.05));
                        *border = BorderColor::all(theme.accent.with_alpha(0.2));
                    } else {
                        *bg = BackgroundColor(theme.accent.with_alpha(0.2));
                        *border = BorderColor::all(theme.accent.with_alpha(0.6));
                    }
                }
            }
            Interaction::None => {
                if let Some((_, mut bg, mut border)) = badge_inner_query
                    .iter_mut()
                    .find(|(b, _, _)| b.song_index == btn.song_index)
                {
                    if disabled {
                        *bg = BackgroundColor(theme.accent.with_alpha(0.05));
                        *border = BorderColor::all(theme.accent.with_alpha(0.2));
                    } else {
                        *bg = BackgroundColor(theme.accent.with_alpha(0.1));
                        *border = BorderColor::all(theme.accent.with_alpha(0.4));
                    }
                }
            }
        }
    }
}

fn handle_language_picker_interaction(
    mut commands: Commands,
    mut item_query: Query<
        (&Interaction, &LanguagePickerItem, &mut BackgroundColor),
        (Changed<Interaction>, Without<LanguagePickerClose>),
    >,
    mut close_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<LanguagePickerClose>, Without<LanguagePickerItem>),
    >,
    overlay_query: Query<Entity, With<LanguagePickerOverlay>>,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
    cache: Res<CacheDir>,
    mut config: ResMut<AppConfig>,
    theme: Res<UiTheme>,
    keyboard: Res<ButtonInput<KeyCode>>,
    nav: Res<crate::input::NavInput>,
) {
    if overlay_query.is_empty() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) || nav.back {
        for entity in &overlay_query {
            commands.entity(entity).despawn();
        }
        commands.remove_resource::<LanguagePickerTarget>();
        return;
    }

    for (interaction, mut bg) in &mut close_query {
        match interaction {
            Interaction::Pressed => {
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                commands.remove_resource::<LanguagePickerTarget>();
                return;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.popup_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.popup_btn);
            }
        }
    }

    for (interaction, item, mut bg) in &mut item_query {
        match interaction {
            Interaction::Pressed => {
                let idx = item.song_index;
                let lang = item.lang_code.clone();
                if idx < library.songs.len() {
                    let hash = library.songs[idx].file_hash.clone();
                    config.set_language_override(hash.clone(), lang.clone());
                    config.save();

                    let transcript = cache.transcript_path(&hash);
                    if transcript.is_file() {
                        let _ = std::fs::remove_file(&transcript);
                    }

                    library.songs[idx].language = Some(lang);
                    library.songs[idx].analysis_status = AnalysisStatus::Queued;
                    queue.enqueue(idx);
                }
                for entity in &overlay_query {
                    commands.entity(entity).despawn();
                }
                commands.remove_resource::<LanguagePickerTarget>();
                return;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.surface_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::NONE);
            }
        }
    }
}

fn update_analysis_hint(
    library: Res<SongLibrary>,
    mut hint: Query<&mut Visibility, With<AnalysisHint>>,
) {
    let any_analyzing = library
        .songs
        .iter()
        .any(|s| matches!(s.analysis_status, AnalysisStatus::Analyzing | AnalysisStatus::Queued));
    if let Ok(mut vis) = hint.single_mut() {
        let target = if any_analyzing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        vis.set_if_neq(target);
    }
}

fn update_status_badges(
    library: Res<SongLibrary>,
    queue: Res<AnalysisQueue>,
    theme: Res<UiTheme>,
    mut badge_query: Query<(&StatusBadge, &mut BackgroundColor), Without<SpinnerOverlay>>,
    mut badge_text_query: Query<(&BadgeText, &mut Text), (Without<StatsText>, Without<LanguageText>)>,
    mut stats_query: Query<&mut Text, (With<StatsText>, Without<BadgeText>, Without<LanguageText>)>,
    mut spinner_vis_query: Query<
        (&SpinnerOverlay, &mut Visibility),
        (Without<ReanalyzeButton>, Without<StatusBadge>, Without<LanguageButton>),
    >,
    mut reanalyze_query: Query<(&ReanalyzeButton, &mut Visibility), (Without<SpinnerOverlay>, Without<LanguageButton>, Without<DeleteCacheButton>)>,
    mut delete_btn_query: Query<(&DeleteCacheButton, &mut Visibility), (Without<SpinnerOverlay>, Without<LanguageButton>, Without<ReanalyzeButton>)>,
    mut lang_text_query: Query<(&LanguageText, &mut Text), (Without<BadgeText>, Without<StatsText>)>,
    mut lang_btn_query: Query<(&LanguageButton, &mut Visibility), (Without<SpinnerOverlay>, Without<ReanalyzeButton>)>,
    mut lang_inner_query: Query<(&LanguageBadgeInner, &mut BackgroundColor, &mut BorderColor), (Without<StatusBadge>, Without<SpinnerOverlay>)>,
) {
    if !library.is_changed() && !queue.is_changed() && queue.active.is_none() {
        return;
    }

    for (badge, mut bg) in &mut badge_query {
        if badge.song_index >= library.songs.len() {
            continue;
        }
        let color = match &library.songs[badge.song_index].analysis_status {
            AnalysisStatus::Ready(TranscriptSource::Lyrics) => theme.badge_lyrics,
            AnalysisStatus::Ready(TranscriptSource::Generated) => theme.badge_ready,
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
            AnalysisStatus::Ready(TranscriptSource::Lyrics) => "LYRICS".into(),
            AnalysisStatus::Ready(TranscriptSource::Generated) => "TRANSCRIPT".into(),
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
            .filter(|s| matches!(s.analysis_status, AnalysisStatus::Ready(_)))
            .count();
        **stats = format!(
            "{} songs found · {} ready for karaoke",
            library.songs.len(),
            ready_count
        );
    }

    for (spinner, mut vis) in &mut spinner_vis_query {
        if spinner.song_index >= library.songs.len() {
            continue;
        }
        let analyzing =
            library.songs[spinner.song_index].analysis_status == AnalysisStatus::Analyzing;
        vis.set_if_neq(if analyzing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }

    for (btn, mut vis) in &mut reanalyze_query {
        if btn.song_index >= library.songs.len() {
            continue;
        }
        *vis = if matches!(
            library.songs[btn.song_index].analysis_status,
            AnalysisStatus::Ready(_) | AnalysisStatus::Failed(_)
        ) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for (btn, mut vis) in &mut delete_btn_query {
        if btn.song_index >= library.songs.len() {
            continue;
        }
        *vis = if matches!(
            library.songs[btn.song_index].analysis_status,
            AnalysisStatus::Ready(_) | AnalysisStatus::Failed(_)
        ) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for (lt, mut text) in &mut lang_text_query {
        if lt.song_index >= library.songs.len() {
            continue;
        }
        let new_lang = library.songs[lt.song_index]
            .language
            .as_deref()
            .map(|l| l.to_uppercase())
            .unwrap_or_default();
        if **text != new_lang {
            **text = new_lang;
        }
    }

    for (lb, mut vis) in &mut lang_btn_query {
        if lb.song_index >= library.songs.len() {
            continue;
        }
        let target = if library.songs[lb.song_index].language.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        vis.set_if_neq(target);
    }

    for (bi, mut bg, mut border) in &mut lang_inner_query {
        if bi.song_index >= library.songs.len() {
            continue;
        }
        let disabled = matches!(
            library.songs[bi.song_index].analysis_status,
            AnalysisStatus::Analyzing | AnalysisStatus::Queued
        );
        if disabled {
            *bg = BackgroundColor(theme.accent.with_alpha(0.05));
            *border = BorderColor::all(theme.accent.with_alpha(0.2));
        }
    }
}

fn animate_spinners(
    time: Res<Time>,
    theme: Res<UiTheme>,
    library: Res<SongLibrary>,
    mut spinner_query: Query<(&SpinnerOverlay, &mut BackgroundColor)>,
) {
    let spinner_alpha = (time.elapsed_secs() * 3.0).sin() * 0.25 + 0.75;
    for (spinner, mut bg) in &mut spinner_query {
        if spinner.song_index >= library.songs.len() {
            continue;
        }
        if library.songs[spinner.song_index].analysis_status == AnalysisStatus::Analyzing {
            *bg = BackgroundColor(theme.spinner_overlay.with_alpha(spinner_alpha));
        }
    }
}

fn cleanup_menu(
    mut commands: Commands,
    query: Query<Entity, With<MenuRoot>>,
    settings_query: Query<Entity, With<SettingsOverlay>>,
    profile_query: Query<Entity, With<ProfileOverlay>>,
    exit_query: Query<Entity, With<sidebar::ExitOverlay>>,
    lang_picker_query: Query<Entity, With<LanguagePickerOverlay>>,
    about_query: Query<Entity, With<AboutOverlay>>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    for entity in &settings_query {
        commands.entity(entity).despawn();
    }
    for entity in &profile_query {
        commands.entity(entity).despawn();
    }
    for entity in &exit_query {
        commands.entity(entity).despawn();
    }
    for entity in &lang_picker_query {
        commands.entity(entity).despawn();
    }
    for entity in &about_query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<AlbumArtCache>();
    commands.remove_resource::<IconFont>();
    commands.remove_resource::<profile::ProfileInputState>();
    commands.remove_resource::<profile::PendingDeleteProfile>();
    commands.remove_resource::<profile::ProfileFocus>();
    commands.remove_resource::<settings::SettingsFocus>();
    commands.remove_resource::<sidebar::ExitFocus>();
    commands.remove_resource::<LanguagePickerTarget>();
}
