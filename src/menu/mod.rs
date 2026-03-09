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
use crate::scanner::metadata::{AnalysisStatus, SongLibrary};
use crate::profile::ProfileStore;
use crate::states::AppState;
use crate::ui::UiTheme;
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
                    sidebar::handle_sidebar_click,
                    settings::handle_settings_click,
                    profile::handle_profile_click,
                    folder::poll_folder_result,
                    folder::poll_rescan,
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
struct MenuState {
    search_query: String,
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
    mut interaction_query: Query<
        (&Interaction, &SongCard, &mut BackgroundColor, &mut BorderColor),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    mut next_state: ResMut<NextState<AppState>>,
    mut queue: ResMut<AnalysisQueue>,
    theme: Res<UiTheme>,
    overlay_query: Query<(), With<SettingsOverlay>>,
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() {
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
    profile_overlay_query: Query<(), With<ProfileOverlay>>,
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() {
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
) {
    if !overlay_query.is_empty() || !profile_overlay_query.is_empty() {
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

fn cleanup_menu(
    mut commands: Commands,
    query: Query<Entity, With<MenuRoot>>,
    settings_query: Query<Entity, With<SettingsOverlay>>,
    profile_query: Query<Entity, With<ProfileOverlay>>,
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
    commands.remove_resource::<AlbumArtCache>();
    commands.remove_resource::<IconFont>();
    commands.remove_resource::<profile::ProfileInputState>();
    commands.remove_resource::<profile::PendingDeleteProfile>();
}
