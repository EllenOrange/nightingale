pub mod song_card;

use bevy::app::AppExit;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageType};
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::analyzer::{AnalysisQueue, PlayTarget};
use crate::scanner::metadata::{AnalysisStatus, SongLibrary};
use crate::states::AppState;
use crate::ui;
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
                    handle_search_input,
                    update_status_badges,
                    handle_sidebar_click,
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

#[derive(Component)]
struct MenuRoot;

fn build_menu(
    mut commands: Commands,
    library: Res<SongLibrary>,
    menu_state: Res<MenuState>,
    art_cache: Res<AlbumArtCache>,
) {
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(ui::BG_COLOR),
        ))
        .with_children(|root| {
            build_sidebar(root);
            build_main_area(root, &library, &menu_state, &art_cache);
        });
}

fn build_sidebar(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Px(200.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(16.0)),
            row_gap: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(SIDEBAR_BG),
    ))
    .with_children(|sidebar| {
        sidebar.spawn((
            Text::new("KARASAD"),
            TextFont {
                font_size: 28.0,
                ..default()
            },
            TextColor(ui::ACCENT),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ));

        sidebar.spawn((
            Text::new("Your own Karaoke"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(ui::TEXT_SECONDARY),
            Node {
                margin: UiRect::bottom(Val::Px(24.0)),
                ..default()
            },
        ));

        spawn_sidebar_button(sidebar, "Rescan Folder", SidebarAction::RescanFolder);
        spawn_sidebar_button(sidebar, "Change Folder", SidebarAction::ChangeFolder);
        spawn_sidebar_button(sidebar, "Exit", SidebarAction::Exit);
    });
}

fn spawn_sidebar_button(parent: &mut ChildSpawnerCommands, label: &str, action: SidebarAction) {
    parent
        .spawn((
            SidebarButton { action },
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(12.0), Val::Px(12.0), Val::Px(10.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(SIDEBAR_BTN),
        ))
        .with_children(|btn| {
            ui::spawn_label(btn, label, 14.0, ui::TEXT_PRIMARY);
        });
}

fn build_main_area(
    root: &mut ChildSpawnerCommands,
    library: &SongLibrary,
    menu_state: &MenuState,
    art_cache: &AlbumArtCache,
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
        main.spawn((
            Node {
                width: Val::Px(600.0),
                height: Val::Px(44.0),
                padding: UiRect::horizontal(Val::Px(16.0)),
                margin: UiRect::bottom(Val::Px(20.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(CARD_COLOR),
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
                TextColor(ui::TEXT_SECONDARY),
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
            TextColor(ui::TEXT_SECONDARY),
            Node {
                margin: UiRect::bottom(Val::Px(16.0)),
                ..default()
            },
        ));

        main.spawn((
            SongListRoot,
            Node {
                width: Val::Px(700.0),
                flex_grow: 1.0,
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
                build_song_card(list, song, i, art);
            }
        });
    });
}

fn handle_song_click(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SongCard, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut library: ResMut<SongLibrary>,
    mut next_state: ResMut<NextState<AppState>>,
    mut queue: ResMut<AnalysisQueue>,
) {
    for (interaction, song_card, mut bg) in &mut interaction_query {
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
                *bg = BackgroundColor(CARD_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(CARD_COLOR);
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
    mut next_state: ResMut<NextState<AppState>>,
    mut queue: ResMut<AnalysisQueue>,
    mut exit: MessageWriter<AppExit>,
    config: Res<crate::config::AppConfig>,
) {
    for (interaction, sidebar_btn, mut bg) in &mut interaction_query {
        match interaction {
            Interaction::Pressed => match sidebar_btn.action {
                SidebarAction::RescanFolder => {
                    if let Some(folder) = config.last_folder.clone() {
                        commands.remove_resource::<SongLibrary>();
                        queue.queue.clear();
                        queue.active = None;
                        commands.insert_resource(crate::scanner::ScanRequest { folder });
                        next_state.set(AppState::Scanning);
                    }
                }
                SidebarAction::ChangeFolder => {
                    commands.remove_resource::<SongLibrary>();
                    queue.queue.clear();
                    queue.active = None;
                    next_state.set(AppState::FolderSelect);
                }
                SidebarAction::Exit => {
                    exit.write(AppExit::Success);
                }
            },
            Interaction::Hovered => {
                *bg = BackgroundColor(SIDEBAR_BTN_HOVER);
            }
            Interaction::None => {
                *bg = BackgroundColor(SIDEBAR_BTN);
            }
        }
    }
}

fn handle_search_input(
    mut key_events: MessageReader<KeyboardInput>,
    mut menu_state: ResMut<MenuState>,
    mut search_text_query: Query<&mut Text, With<SearchText>>,
    library: Res<SongLibrary>,
    song_list_query: Query<Entity, With<SongListRoot>>,
    mut commands: Commands,
    art_cache: Res<AlbumArtCache>,
) {
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
        );
    }
}

fn update_status_badges(
    library: Res<SongLibrary>,
    queue: Res<AnalysisQueue>,
    time: Res<Time>,
    mut badge_query: Query<(&StatusBadge, &mut BackgroundColor)>,
    mut badge_text_query: Query<
        (&BadgeText, &mut Text),
        (Without<StatsText>, Without<SpinnerDotText>),
    >,
    mut stats_query: Query<&mut Text, (With<StatsText>, Without<SpinnerDotText>)>,
    mut spinner_query: Query<(&SpinnerOverlay, &mut Visibility)>,
    mut dot_text_query: Query<&mut Text, With<SpinnerDotText>>,
) {
    for (badge, mut bg) in &mut badge_query {
        if badge.song_index >= library.songs.len() {
            continue;
        }
        let color = match &library.songs[badge.song_index].analysis_status {
            AnalysisStatus::Ready => BADGE_READY,
            AnalysisStatus::NotAnalyzed => BADGE_NOT_ANALYZED,
            AnalysisStatus::Queued => BADGE_QUEUED,
            AnalysisStatus::Analyzing => BADGE_ANALYZING,
            AnalysisStatus::Failed(_) => BADGE_FAILED,
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

    let dot_phase = (time.elapsed_secs() * 2.5) as usize % 3;
    let dots = match dot_phase {
        0 => ".",
        1 => "..",
        _ => "...",
    };

    for (spinner, mut vis) in &mut spinner_query {
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
    }

    for mut dot_text in &mut dot_text_query {
        **dot_text = dots.into();
    }
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<AlbumArtCache>();
}
