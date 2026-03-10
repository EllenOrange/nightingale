mod analyzer;
mod config;
pub mod input;
mod menu;
mod player;
pub mod profile;
mod scanner;
mod setup;
mod states;
pub mod ui;
pub mod vendor;
pub mod vendor_scripts;

use bevy::asset::{AssetPlugin, UnapprovedPathMode, load_internal_binary_asset};
use bevy::prelude::*;
use bevy::window::WindowMode;
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};
use bevy_kira_audio::AudioPlugin;

use analyzer::cache::CacheDir;
use config::AppConfig;
use player::background::BackgroundPlugin;
use profile::ProfileStore;
use scanner::metadata::SongLibrary;
use states::AppState;
use ui::UiTheme;

fn main() {
    dotenvy::dotenv().ok();

    let force_setup = std::env::args().any(|a| a == "--setup");
    if force_setup {
        vendor::reset();
    }

    let vendor_ready = vendor::is_ready();

    let mut app = App::new();

    let config = AppConfig::load();
    let has_saved_folder = config.last_folder.as_ref().is_some_and(|f| f.is_dir());

    let window_mode = if config.is_fullscreen() {
        WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };

    app.add_plugins(EmbeddedAssetPlugin {
        mode: PluginMode::ReplaceAndFallback {
            path: "assets".into(),
        },
    });

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Nightingale — Your Karaoke".into(),
                    resolution: (1280, 720).into(),
                    mode: window_mode,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                unapproved_path_mode: UnapprovedPathMode::Deny,
                ..default()
            })
            .set(ImagePlugin::default_linear()),
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
    let video_flavor = player::video_bg::ActiveVideoFlavor {
        index: config.last_video_flavor.unwrap_or(0),
    };
    let ui_theme = UiTheme::from_config(&config);

    let profile_store = ProfileStore::load();

    app.add_plugins(AudioPlugin)
        .add_plugins(input::InputPlugin)
        .add_plugins(BackgroundPlugin)
        .init_state::<AppState>()
        .insert_resource(ClearColor(ui_theme.bg))
        .insert_resource(CacheDir::new())
        .insert_resource(SongLibrary { songs: vec![] })
        .insert_resource(config)
        .insert_resource(bg_theme)
        .insert_resource(video_flavor)
        .insert_resource(ui_theme)
        .insert_resource(profile_store)
        .add_systems(Startup, (setup_camera, update_ui_scale))
        .add_systems(Update, toggle_fullscreen)
        .add_systems(PreUpdate, update_ui_scale)
        .add_plugins(setup::SetupPlugin)
        .add_plugins(scanner::ScannerPlugin)
        .add_plugins(analyzer::AnalyzerPlugin)
        .add_plugins(menu::MenuPlugin)
        .add_plugins(player::PlayerPlugin);

    if vendor_ready {
        app.add_systems(Startup, skip_setup);
    }

    if has_saved_folder && vendor_ready {
        app.add_systems(Startup, auto_open_saved_folder.after(skip_setup));
    }

    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn skip_setup(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Menu);
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

const REFERENCE_WIDTH: f32 = 1280.0;
const REFERENCE_HEIGHT: f32 = 720.0;

fn update_ui_scale(
    windows: Query<&Window>,
    mut ui_scale: ResMut<bevy::ui::UiScale>,
    theme: Res<UiTheme>,
    mut clear: ResMut<ClearColor>,
    state: Res<State<AppState>>,
    bg_theme: Res<player::background::ActiveTheme>,
) {
    let Ok(window) = windows.single() else { return };
    let factor = (window.width() / REFERENCE_WIDTH)
        .min(window.height() / REFERENCE_HEIGHT)
        .max(1.0);
    if (ui_scale.0 - factor).abs() > 0.01 {
        ui_scale.0 = factor;
    }
    let target = if *state.get() == AppState::Playing && bg_theme.is_video() {
        Color::BLACK
    } else {
        theme.bg
    };
    if clear.0 != target {
        clear.0 = target;
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
