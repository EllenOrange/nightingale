pub mod metadata;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use walkdir::WalkDir;

use crate::analyzer::cache::CacheDir;
use crate::states::AppState;
use crate::ui::{self, UiTheme};
use metadata::{AnalysisStatus, Song, SongLibrary};

pub struct ScannerPlugin;

impl Plugin for ScannerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Scanning), start_scan)
            .add_systems(
                Update,
                poll_scan.run_if(in_state(AppState::Scanning)),
            );
    }
}

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "aac", "wma"];

#[derive(Resource)]
pub struct ScanRequest {
    pub folder: PathBuf,
}

#[derive(Resource)]
struct PendingScan {
    result: Arc<Mutex<Option<Vec<Song>>>>,
}

#[derive(Component)]
struct ScanningUi;

fn start_scan(
    mut commands: Commands,
    scan_request: Res<ScanRequest>,
    cache: Res<CacheDir>,
    theme: Res<UiTheme>,
) {
    let folder = scan_request.folder.clone();
    let cache_path = cache.path.clone();

    info!("Scanning folder: {}", folder.display());

    commands
        .spawn((
            ScanningUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(theme.bg),
        ))
        .with_children(|root| {
            ui::spawn_label(root, "Scanning music folder...", 28.0, theme.accent);
            ui::spawn_label(root, format!("{}", folder.display()), 14.0, theme.text_dim);
        });

    let result: Arc<Mutex<Option<Vec<Song>>>> = Arc::new(Mutex::new(None));
    let result_clone = Arc::clone(&result);

    std::thread::spawn(move || {
        let cache = CacheDir { path: cache_path };
        let songs = scan_folder(&folder, &cache);
        *result_clone.lock().unwrap() = Some(songs);
    });

    commands.insert_resource(PendingScan { result });
}

fn poll_scan(
    mut commands: Commands,
    pending: Option<Res<PendingScan>>,
    mut next_state: ResMut<NextState<AppState>>,
    ui_query: Query<Entity, With<ScanningUi>>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    if let Some(ref songs) = *lock {
        info!("Found {} songs", songs.len());
        commands.insert_resource(SongLibrary {
            songs: songs.clone(),
        });
        drop(lock);
        commands.remove_resource::<PendingScan>();

        for entity in &ui_query {
            commands.entity(entity).despawn();
        }

        next_state.set(AppState::Menu);
    }
}

fn scan_folder(folder: &Path, cache: &CacheDir) -> Vec<Song> {
    let mut songs = Vec::new();

    for entry in WalkDir::new(folder)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let is_audio = ext
            .as_deref()
            .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e));

        if !is_audio {
            continue;
        }

        match build_song(path, cache) {
            Ok(song) => songs.push(song),
            Err(e) => warn!("Failed to process {}: {}", path.display(), e),
        }
    }

    songs.sort_by(|a, b| {
        a.display_artist()
            .cmp(b.display_artist())
            .then(a.display_title().cmp(b.display_title()))
    });
    songs
}

fn build_song(path: &Path, cache: &CacheDir) -> Result<Song, Box<dyn std::error::Error>> {
    let file_hash = compute_file_hash(path)?;

    let analysis_status = if cache.transcript_exists(&file_hash) {
        AnalysisStatus::Ready
    } else {
        AnalysisStatus::NotAnalyzed
    };

    Ok(Song::from_path(path, file_hash, analysis_status))
}

fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex()[..32].to_string())
}
