mod analyzer;
mod config;
mod menu;
mod player;
mod scanner;
mod states;
pub mod ui;

use bevy::asset::{AssetPlugin, UnapprovedPathMode, load_internal_binary_asset};
use bevy::prelude::*;
use bevy::window::WindowMode;
use bevy_kira_audio::AudioPlugin;

use analyzer::cache::CacheDir;
use config::AppConfig;
use player::background::BackgroundPlugin;
use scanner::metadata::SongLibrary;
use states::AppState;
use ui::UiTheme;

fn main() {
    let mut app = App::new();

    let config = AppConfig::load();
    let has_saved_folder = config.last_folder.as_ref().is_some_and(|f| f.is_dir());

    let window_mode = if config.is_fullscreen() {
        WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Karasad — Own Your Karaoke".into(),
                    resolution: (1280, 720).into(),
                    mode: window_mode,
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

    let bg_theme = player::background::ActiveTheme {
        index: config.last_theme.unwrap_or(0),
    };
    let ui_theme = UiTheme::from_config(&config);

    app.add_plugins(AudioPlugin)
        .add_plugins(BackgroundPlugin)
        .init_state::<AppState>()
        .insert_resource(CacheDir::new())
        .insert_resource(SongLibrary { songs: vec![] })
        .insert_resource(config)
        .insert_resource(bg_theme)
        .insert_resource(ui_theme)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, toggle_fullscreen)
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

fn toggle_fullscreen(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
    mut config: ResMut<AppConfig>,
) {
    if keyboard.just_pressed(KeyCode::F11) {
        if let Ok(mut window) = windows.single_mut() {
            let is_fs = matches!(window.mode, WindowMode::BorderlessFullscreen(_));
            window.mode = if is_fs {
                WindowMode::Windowed
            } else {
                WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
            };
            config.fullscreen = Some(!is_fs);
            config.save();
        }
    }
}
