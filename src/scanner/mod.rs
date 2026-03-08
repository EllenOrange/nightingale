pub mod metadata;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lofty::prelude::*;
use walkdir::WalkDir;

use crate::analyzer::cache::CacheDir;
use crate::states::AppState;
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

fn start_scan(mut commands: Commands, scan_request: Res<ScanRequest>, cache: Res<CacheDir>) {
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
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Scanning music folder..."),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(Color::srgb(0.4, 0.6, 1.0)),
            ));
            root.spawn((
                Text::new(format!("{}", folder.display())),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
            ));
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
    scan_request: Res<ScanRequest>,
    mut next_state: ResMut<NextState<AppState>>,
    ui_query: Query<Entity, With<ScanningUi>>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    if let Some(ref songs) = *lock {
        info!("Found {} songs", songs.len());
        commands.insert_resource(SongLibrary {
            songs: songs.clone(),
            root_folder: scan_request.folder.clone(),
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

    let (title, artist, album, duration_secs, album_art) = read_metadata(path);

    Ok(Song {
        path: path.to_path_buf(),
        file_hash,
        title,
        artist,
        album,
        duration_secs,
        album_art,
        analysis_status,
    })
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

fn read_metadata(path: &Path) -> (String, String, String, f64, Option<Vec<u8>>) {
    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return (String::new(), String::new(), String::new(), 0.0, None),
    };

    let properties = tagged.properties();
    let duration_secs = properties.duration().as_secs_f64();

    let tag = match tagged.primary_tag().or_else(|| tagged.first_tag()) {
        Some(t) => t,
        None => {
            return (
                String::new(),
                String::new(),
                String::new(),
                duration_secs,
                None,
            )
        }
    };

    let title = tag.title().map(|s| s.to_string()).unwrap_or_default();
    let artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
    let album = tag.album().map(|s| s.to_string()).unwrap_or_default();

    let album_art = tag.pictures().first().map(|pic| pic.data().to_vec());

    (title, artist, album, duration_secs, album_art)
}
