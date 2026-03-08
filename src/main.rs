mod analyzer;
mod config;
mod folder_select;
mod menu;
mod player;
mod scanner;
mod states;
pub mod ui;

use bevy::asset::{AssetPlugin, UnapprovedPathMode, load_internal_binary_asset};
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
                    title: "Karasad — Own Your Karaoke".into(),
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
    let has_saved_folder = config.last_folder.as_ref().is_some_and(|f| f.is_dir());

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
        .add_plugins(folder_select::FolderSelectPlugin)
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
