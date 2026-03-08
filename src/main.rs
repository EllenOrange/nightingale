mod analyzer;
mod config;
mod menu;
mod player;
mod scanner;
mod states;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::asset::{load_internal_binary_asset, AssetPlugin, UnapprovedPathMode};
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;

use analyzer::cache::CacheDir;
use config::AppConfig;
use player::background::BackgroundPlugin;
use states::AppState;

fn main() {
    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Karasad — AI Karaoke".into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                unapproved_path_mode: UnapprovedPathMode::Deny,
                ..default()
            }),
    );

    load_internal_binary_asset!(
        app,
        TextFont::default().font,
        "../assets/fonts/NotoSansCJKsc-Regular.otf",
        |bytes: &[u8], _path: String| { Font::try_from_bytes(bytes.to_vec()).unwrap() }
    );

    let config = AppConfig::load();
    let has_saved_folder = config
        .last_folder
        .as_ref()
        .is_some_and(|f| f.is_dir());

    let theme = player::background::ActiveTheme {
        index: config.last_theme.unwrap_or(0),
    };

    app.add_plugins(AudioPlugin)
        .add_plugins(BackgroundPlugin)
        .init_state::<AppState>()
        .insert_resource(CacheDir::new())
        .insert_resource(config)
        .insert_resource(theme)
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(AppState::FolderSelect), show_folder_select)
        .add_systems(
            Update,
            (handle_folder_button, poll_folder_result).run_if(in_state(AppState::FolderSelect)),
        )
        .add_systems(OnExit(AppState::FolderSelect), cleanup_folder_select)
        .add_plugins(scanner::ScannerPlugin)
        .add_plugins(analyzer::AnalyzerPlugin)
        .add_plugins(menu::MenuPlugin)
        .add_plugins(player::PlayerPlugin);

    if has_saved_folder {
        app.add_systems(Startup, auto_open_saved_folder);
    }

    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn auto_open_saved_folder(
    mut commands: Commands,
    config: Res<AppConfig>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if let Some(ref folder) = config.last_folder {
        if folder.is_dir() {
            info!("Auto-opening saved folder: {}", folder.display());
            commands.insert_resource(scanner::ScanRequest {
                folder: folder.clone(),
            });
            next_state.set(AppState::Scanning);
        }
    }
}

#[derive(Component)]
struct FolderSelectRoot;

#[derive(Component)]
struct FolderSelectButton;

#[derive(Resource)]
struct PendingFolderPick {
    result: Arc<Mutex<Option<Option<PathBuf>>>>,
}

fn show_folder_select(mut commands: Commands) {
    commands
        .spawn((
            FolderSelectRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("KARASAD"),
                TextFont {
                    font_size: 72.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.6, 1.0)),
            ));

            root.spawn((
                Text::new("AI-Powered Karaoke"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.65)),
            ));

            root.spawn((
                Text::new("Select a folder with your music to get started"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
                Node {
                    margin: UiRect::top(Val::Px(16.0)),
                    ..default()
                },
            ));

            root.spawn((
                FolderSelectButton,
                Button,
                Node {
                    padding: UiRect::new(
                        Val::Px(32.0),
                        Val::Px(32.0),
                        Val::Px(16.0),
                        Val::Px(16.0),
                    ),
                    margin: UiRect::top(Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.5, 0.9)),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Choose Music Folder"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

fn handle_folder_button(
    mut commands: Commands,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<FolderSelectButton>)>,
    pending: Option<Res<PendingFolderPick>>,
) {
    if pending.is_some() {
        return;
    }

    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
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
    }
}

fn poll_folder_result(
    mut commands: Commands,
    pending: Option<Res<PendingFolderPick>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut config: ResMut<AppConfig>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    if let Some(ref maybe_folder) = *lock {
        if let Some(folder) = maybe_folder {
            info!("Selected folder: {}", folder.display());
            commands.insert_resource(scanner::ScanRequest {
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

fn cleanup_folder_select(mut commands: Commands, query: Query<Entity, With<FolderSelectRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
