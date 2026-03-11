#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

#[cfg(not(debug_assertions))]
use std::sync::Mutex;

use bevy::asset::{AssetPlugin, UnapprovedPathMode, load_internal_binary_asset};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode};
use bevy::winit::WinitWindows;
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};
use bevy_kira_audio::AudioPlugin;

use analyzer::cache::CacheDir;
use config::AppConfig;
use player::background::BackgroundPlugin;
use profile::ProfileStore;
use scanner::metadata::SongLibrary;
use states::AppState;
use ui::UiTheme;

#[cfg(not(debug_assertions))]
static LOG_FILE: Mutex<Option<std::sync::Arc<Mutex<std::fs::File>>>> = Mutex::new(None);

fn main() {
    dotenvy::dotenv().ok();

    #[cfg(not(debug_assertions))]
    setup_file_logging();

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

    #[allow(unused_mut)]
    let mut plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Nightingale — Your Karaoke".into(),
                resolution: (1280, 720).into(),
                mode: window_mode,
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        })
        .set(AssetPlugin {
            unapproved_path_mode: UnapprovedPathMode::Deny,
            ..default()
        })
        .set(ImagePlugin::default_linear());

    #[cfg(not(debug_assertions))]
    {
        plugins = plugins.set(bevy::log::LogPlugin {
            custom_layer: |_| {
                let guard = LOG_FILE.lock().unwrap();
                guard.as_ref().map(|file| {
                    let writer = file.clone();
                    Box::new(
                        tracing_subscriber::fmt::layer()
                            .with_ansi(false)
                            .with_writer(move || LogFileWriter(writer.clone()))
                    ) as Box<dyn bevy::log::tracing_subscriber::Layer<bevy::log::tracing_subscriber::Registry> + Send + Sync>
                })
            },
            ..default()
        });
    }

    app.add_plugins(plugins);

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
        .add_systems(Startup, (setup_camera, update_ui_scale, set_window_icon))
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

fn set_window_icon(
    winit_windows: Option<NonSend<WinitWindows>>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
) {
    let Some(winit_windows) = winit_windows else {
        return;
    };
    let Ok(entity) = windows.single() else { return };
    let Some(winit_window) = winit_windows.get_window(entity) else {
        return;
    };

    let icon_bytes = include_bytes!("../assets/images/logo_square.png");
    let img = image::load_from_memory(icon_bytes)
        .expect("Failed to decode icon PNG")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();
    let icon = winit::window::Icon::from_rgba(rgba, w, h).expect("Failed to create window icon");
    winit_window.set_window_icon(Some(icon));
}

#[cfg(not(debug_assertions))]
struct LogFileWriter(std::sync::Arc<Mutex<std::fs::File>>);

#[cfg(not(debug_assertions))]
impl std::io::Write for LogFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut f = self.0.lock().unwrap();
        f.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let mut f = self.0.lock().unwrap();
        f.flush()
    }
}

#[cfg(not(debug_assertions))]
fn setup_file_logging() {
    use std::io::Write;
    use std::sync::Arc;

    let log_dir = dirs::home_dir()
        .expect("could not find home directory")
        .join(".nightingale");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("nightingale.log");

    let file = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let shared = Arc::new(Mutex::new(file));
    *LOG_FILE.lock().unwrap() = Some(shared.clone());

    redirect_stderr(&log_path);

    let _ = writeln!(shared.lock().unwrap(), "--- Nightingale log started ---");
}

#[cfg(not(debug_assertions))]
fn redirect_stderr(log_path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::io::IntoRawFd;
        if let Ok(file) = std::fs::OpenOptions::new().append(true).open(log_path) {
            let fd = file.into_raw_fd();
            unsafe { libc::dup2(fd, 2); }
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::IntoRawHandle;
        if let Ok(file) = std::fs::OpenOptions::new().append(true).open(log_path) {
            let handle = file.into_raw_handle();
            unsafe {
                windows_sys::Win32::System::Console::SetStdHandle(
                    windows_sys::Win32::System::Console::STD_ERROR_HANDLE,
                    handle as _,
                );
            }
        }
    }
}
