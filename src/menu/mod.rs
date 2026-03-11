pub mod folder;
pub mod profile;
pub mod settings;
pub mod sidebar;
pub mod song_card;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageType};
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::hover::HoverMap;
use bevy::prelude::*;

use crate::analyzer::cache::CacheDir;
use crate::analyzer::{AnalysisQueue, PlayTarget};
use crate::scanner::metadata::{AnalysisStatus, SongLibrary, TranscriptSource};
use crate::profile::ProfileStore;
use crate::states::AppState;
use crate::ui::UiTheme;
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
}

impl Default for MenuFocus {
    fn default() -> Self {
        Self {
            panel: FocusPanel::SongList,
            song_index: 0,
            sidebar_index: 0,
            nav_lock: 0,
        }
    }
}

const NAV_INITIAL_DELAY: f32 = 0.4;
const NAV_REPEAT_RATE: f32 = 0.06;

#[derive(Resource)]
struct NavRepeat {
    timer: Timer,
    started: bool,
}

impl Default for NavRepeat {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(NAV_INITIAL_DELAY, TimerMode::Once),
            started: false,
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

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuState>()
            .init_resource::<MenuFocus>()
            .init_resource::<NavRepeat>()
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
                    update_analysis_hint,
                    sidebar::handle_sidebar_click,
                    sidebar::handle_exit_input,
                    settings::handle_settings_click,
                    profile::handle_profile_click,
                    folder::poll_folder_result,
                    folder::poll_rescan,
                )
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(
                Update,
                (
                    handle_menu_nav
                        .after(handle_song_click)
                        .after(sidebar::handle_sidebar_click),
                    apply_menu_focus_styling
                        .after(handle_menu_nav)
                        .after(handle_song_click)
                        .after(sidebar::handle_sidebar_click),
                    scroll_to_focused.after(handle_menu_nav),
                )
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(
                Update,
                profile::handle_profile_input
                    .run_if(in_state(AppState::Menu))
                    .run_if(resource_exists::<profile::ProfileInputState>),
            )
            .add_systems(Update, send_scroll_events)
            .add_observer(on_scroll_handler)
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
pub const FA_SPINNER: &str = "\u{f1ce}";
pub const FA_USER: &str = "\u{f007}";

#[derive(Component)]
struct MenuRoot;

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

fn build_menu(
    mut commands: Commands,
    library: Res<SongLibrary>,
    menu_state: Res<MenuState>,
    art_cache: Res<AlbumArtCache>,
    theme: Res<UiTheme>,
    config: Res<crate::config::AppConfig>,
    asset_server: Res<AssetServer>,
    profiles: Res<ProfileStore>,
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
            sidebar::build_sidebar(root, &theme, has_folder, logo_handle, &icon_font, &profiles);
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
            .filter(|s| matches!(s.analysis_status, AnalysisStatus::Ready(_)))
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
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    exit_overlay_query: Query<(), With<sidebar::ExitOverlay>>,
    mut focus: ResMut<MenuFocus>,
    nav: Res<crate::input::NavInput>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
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
                }
            }
            Interaction::None => {}
        }
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
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    exit_overlay_query: Query<(), With<sidebar::ExitOverlay>>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
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

fn handle_search_input(
    mut key_events: MessageReader<KeyboardInput>,
    mut menu_state: ResMut<MenuState>,
    mut search_text_query: Query<&mut Text, With<SearchText>>,
    library: Res<SongLibrary>,
    mut card_query: Query<(&SongCard, &mut Node)>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    exit_overlay_query: Query<(), With<sidebar::ExitOverlay>>,
    mut focus: ResMut<MenuFocus>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
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
            AnalysisStatus::Ready(TranscriptSource::Generated) => "AI".into(),
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
            AnalysisStatus::Ready(_) | AnalysisStatus::Failed(_)
        ) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

const SCROLL_LINE_HEIGHT: f32 = 21.0;

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct ScrollEvent {
    entity: Entity,
    delta: Vec2,
}

fn send_scroll_events(
    mut mouse_wheel_reader: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut commands: Commands,
) {
    for mouse_wheel in mouse_wheel_reader.read() {
        let mut delta = -Vec2::new(mouse_wheel.x, mouse_wheel.y);

        if mouse_wheel.unit == MouseScrollUnit::Line {
            delta *= SCROLL_LINE_HEIGHT;
        }

        for pointer_map in hover_map.values() {
            for entity in pointer_map.keys().copied() {
                commands.trigger(ScrollEvent { entity, delta });
            }
        }
    }
}

fn on_scroll_handler(
    mut scroll: On<ScrollEvent>,
    mut query: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    let Ok((mut scroll_position, node, computed)) = query.get_mut(scroll.entity) else {
        return;
    };

    let max_offset = (computed.content_size() - computed.size()) * computed.inverse_scale_factor();
    let delta = &mut scroll.delta;

    if node.overflow.y == OverflowAxis::Scroll && delta.y != 0. {
        let at_limit = if delta.y > 0. {
            scroll_position.y >= max_offset.y
        } else {
            scroll_position.y <= 0.
        };

        if !at_limit {
            scroll_position.y = (scroll_position.y + delta.y).clamp(0., max_offset.y.max(0.));
            delta.y = 0.;
        }
    }

    if node.overflow.x == OverflowAxis::Scroll && delta.x != 0. {
        let at_limit = if delta.x > 0. {
            scroll_position.x >= max_offset.x
        } else {
            scroll_position.x <= 0.
        };

        if !at_limit {
            scroll_position.x = (scroll_position.x + delta.x).clamp(0., max_offset.x.max(0.));
            delta.x = 0.;
        }
    }

    if *delta == Vec2::ZERO {
        scroll.propagate(false);
    }
}

fn handle_menu_nav(
    keyboard: Res<ButtonInput<KeyCode>>,
    nav: Res<crate::input::NavInput>,
    mut focus: ResMut<MenuFocus>,
    card_query: Query<(&SongCard, &Node), Without<SidebarButton>>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    exit_overlay_query: Query<(), With<sidebar::ExitOverlay>>,
    time: Res<Time>,
    mut nav_repeat: ResMut<NavRepeat>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
        return;
    }

    let ud_just = nav.up || nav.down;
    let ud_held = nav.up_held || nav.down_held;

    let mut ud_step = false;
    if ud_just {
        nav_repeat.timer = Timer::from_seconds(NAV_INITIAL_DELAY, TimerMode::Once);
        nav_repeat.started = true;
        ud_step = true;
    } else if ud_held && nav_repeat.started {
        nav_repeat.timer.tick(time.delta());
        if nav_repeat.timer.just_finished() {
            nav_repeat.timer = Timer::from_seconds(NAV_REPEAT_RATE, TimerMode::Repeating);
            ud_step = true;
        }
    } else {
        nav_repeat.started = false;
    }

    if focus.nav_lock > 0 {
        focus.nav_lock -= 1;
    }

    let any_nav = ud_step
        || nav.left
        || nav.right
        || keyboard.just_pressed(KeyCode::Tab);
    if !any_nav {
        return;
    }

    if nav.left {
        focus.panel = FocusPanel::Sidebar;
    } else if nav.right {
        focus.panel = FocusPanel::SongList;
    } else if keyboard.just_pressed(KeyCode::Tab) {
        focus.panel = if focus.panel == FocusPanel::SongList {
            FocusPanel::Sidebar
        } else {
            FocusPanel::SongList
        };
    }

    let step_down = ud_step && nav.down_held;
    let step_up = ud_step && nav.up_held;

    if step_down || step_up {
        match focus.panel {
            FocusPanel::SongList => {
                let mut visible: Vec<usize> = card_query
                    .iter()
                    .filter(|(_, node)| node.display != Display::None)
                    .map(|(card, _)| card.song_index)
                    .collect();
                visible.sort();

                if !visible.is_empty() {
                    let pos = visible.iter().position(|&i| i == focus.song_index);
                    if step_down {
                        focus.song_index = match pos {
                            Some(p) if p + 1 < visible.len() => visible[p + 1],
                            None => visible[0],
                            _ => focus.song_index,
                        };
                    }
                    if step_up {
                        focus.song_index = match pos {
                            Some(p) if p > 0 => visible[p - 1],
                            None => visible[0],
                            _ => focus.song_index,
                        };
                    }
                    focus.nav_lock = 2;
                }
            }
            FocusPanel::Sidebar => {
                if step_down {
                    focus.sidebar_index =
                        (focus.sidebar_index + 1).min(SIDEBAR_ACTIONS.len() - 1);
                }
                if step_up {
                    focus.sidebar_index = focus.sidebar_index.saturating_sub(1);
                }
            }
        }
    }

}

fn apply_menu_focus_styling(
    focus: Res<MenuFocus>,
    mut card_query: Query<
        (&SongCard, &mut BackgroundColor, &mut BorderColor),
        Without<SidebarButton>,
    >,
    mut sidebar_query: Query<
        (&SidebarButton, &mut BackgroundColor, &mut BorderColor),
        Without<SongCard>,
    >,
    theme: Res<UiTheme>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
    exit_overlay_query: Query<(), With<sidebar::ExitOverlay>>,
) {
    if !focus.is_changed() && !theme.is_changed() {
        return;
    }
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() || !exit_overlay_query.is_empty() {
        return;
    }
    for (card, mut bg, mut border) in &mut card_query {
        let is_focused =
            focus.panel == FocusPanel::SongList && card.song_index == focus.song_index;
        if is_focused {
            bg.set_if_neq(BackgroundColor(theme.card_hover));
            border.set_if_neq(BorderColor::all(theme.accent));
        } else {
            bg.set_if_neq(BackgroundColor(theme.card_bg));
            border.set_if_neq(BorderColor::all(Color::NONE));
        }
    }
    for (btn, mut bg, mut border) in &mut sidebar_query {
        let idx = SIDEBAR_ACTIONS.iter().position(|&a| a == btn.action);
        let is_focused = focus.panel == FocusPanel::Sidebar && idx == Some(focus.sidebar_index);
        if is_focused {
            bg.set_if_neq(BackgroundColor(theme.sidebar_btn_hover));
            border.set_if_neq(BorderColor::all(theme.accent));
        } else {
            bg.set_if_neq(BackgroundColor(theme.sidebar_btn));
            border.set_if_neq(BorderColor::all(Color::NONE));
        }
    }
}

fn scroll_to_focused(
    focus: Res<MenuFocus>,
    mut scroll_query: Query<(&mut ScrollPosition, &ComputedNode), With<SongListRoot>>,
    card_query: Query<(&SongCard, &Node, &ComputedNode)>,
) {
    if !focus.is_changed() || focus.panel != FocusPanel::SongList {
        return;
    }

    let Ok((mut scroll_pos, list_computed)) = scroll_query.single_mut() else {
        return;
    };

    let list_height = list_computed.size().y * list_computed.inverse_scale_factor();
    if list_height < 1.0 {
        return;
    }

    let gap = 8.0;
    let mut cards: Vec<(usize, f32)> = card_query
        .iter()
        .filter(|(_, node, _)| node.display != Display::None)
        .map(|(card, _, computed)| {
            (
                card.song_index,
                computed.size().y * computed.inverse_scale_factor(),
            )
        })
        .collect();
    cards.sort_by_key(|(idx, _)| *idx);

    let mut y = 0.0;
    for (idx, height) in &cards {
        if *idx == focus.song_index {
            if y < scroll_pos.y {
                scroll_pos.y = y;
            } else if y + height > scroll_pos.y + list_height {
                scroll_pos.y = y + height - list_height;
            }
            return;
        }
        y += height + gap;
    }
}

fn cleanup_menu(
    mut commands: Commands,
    query: Query<Entity, With<MenuRoot>>,
    settings_query: Query<Entity, With<SettingsOverlay>>,
    profile_query: Query<Entity, With<ProfileOverlay>>,
    exit_query: Query<Entity, With<sidebar::ExitOverlay>>,
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
    commands.remove_resource::<AlbumArtCache>();
    commands.remove_resource::<IconFont>();
    commands.remove_resource::<profile::ProfileInputState>();
    commands.remove_resource::<profile::PendingDeleteProfile>();
    commands.remove_resource::<profile::ProfileFocus>();
    commands.remove_resource::<settings::SettingsFocus>();
    commands.remove_resource::<sidebar::ExitFocus>();
}
