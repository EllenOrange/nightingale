use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::seq::IndexedRandom;
use rand::seq::SliceRandom;

const VIDEO_WIDTH: u32 = 1920;
const VIDEO_HEIGHT: u32 = 1080;
const FRAME_BYTES: usize = (VIDEO_WIDTH * VIDEO_HEIGHT * 4) as usize;
const TARGET_FPS: f64 = 30.0;
const MAX_CACHED_VIDEOS: usize = 12;
const PIXABAY_PER_PAGE: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VideoFlavor {
    Nature,
    Underwater,
    Space,
    City,
    Countryside,
}

impl VideoFlavor {
    pub const ALL: &[Self] = &[
        Self::Nature,
        Self::Underwater,
        Self::Space,
        Self::City,
        Self::Countryside,
    ];

    pub fn name(&self) -> &str {
        match self {
            Self::Nature => "Nature",
            Self::Underwater => "Underwater",
            Self::Space => "Space",
            Self::City => "City",
            Self::Countryside => "Countryside",
        }
    }

    fn keywords(&self) -> &[&str] {
        match self {
            Self::Nature => &[
                "nature landscape aerial",
                "forest trees cinematic",
                "mountain scenery drone",
                "sunset clouds timelapse",
                "waterfall tropical scenic",
                "autumn leaves forest",
                "river valley aerial",
            ],
            Self::Underwater => &[
                "underwater coral reef",
                "deep sea fish",
                "ocean jellyfish",
                "scuba diving reef",
                "tropical fish underwater",
                "sea turtle underwater",
            ],
            Self::Space => &[
                "galaxy stars universe",
                "nebula deep space",
                "aurora borealis sky",
                "earth orbit space",
                "milky way night sky",
                "starfield cosmos",
            ],
            Self::City => &[
                "city skyline night",
                "city traffic timelapse",
                "neon lights city",
                "urban aerial night",
                "downtown skyscrapers dusk",
                "highway traffic night",
            ],
            Self::Countryside => &[
                "countryside meadow aerial",
                "farm fields drone",
                "rolling hills green",
                "village landscape scenic",
                "vineyard countryside",
                "pastoral landscape sunset",
            ],
        }
    }

    fn category(&self) -> &str {
        match self {
            Self::Nature => "nature",
            Self::Underwater => "animals",
            Self::Space => "science",
            Self::City => "buildings",
            Self::Countryside => "places",
        }
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL[i % Self::ALL.len()]
    }
}

#[derive(Resource)]
pub struct ActiveVideoFlavor {
    pub index: usize,
}

impl Default for ActiveVideoFlavor {
    fn default() -> Self {
        Self { index: 0 }
    }
}

impl ActiveVideoFlavor {
    pub fn flavor(&self) -> VideoFlavor {
        VideoFlavor::from_index(self.index)
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % VideoFlavor::ALL.len();
    }
}

#[derive(Component)]
pub struct VideoSprite;

const POOL_SIZE: usize = 4;

#[derive(Resource)]
pub struct VideoBackground {
    pub image_handle: Handle<Image>,
    #[allow(dead_code)]
    pub flavor: VideoFlavor,
    frame_rx: Mutex<mpsc::Receiver<Vec<u8>>>,
    free_tx: Mutex<mpsc::SyncSender<Vec<u8>>>,
    cmd_tx: Mutex<mpsc::Sender<DecoderCommand>>,
    stop_downloads: Arc<AtomicBool>,
    elapsed: f64,
    frame_interval: f64,
}

enum DecoderCommand {
    Stop,
}

fn cache_dir(flavor: VideoFlavor) -> PathBuf {
    let base = dirs::home_dir()
        .expect("could not find home directory")
        .join(".nightingale")
        .join("videos")
        .join(flavor.name().to_lowercase());
    std::fs::create_dir_all(&base).ok();
    base
}

fn cached_videos(flavor: VideoFlavor) -> Vec<PathBuf> {
    let dir = cache_dir(flavor);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mp4"))
        .collect();
    files.sort();
    files
}

struct PendingDownload {
    video_id: u64,
    url: String,
    dest: PathBuf,
}

fn fetch_video_listing(flavor: VideoFlavor) -> Vec<PendingDownload> {
    let api_key = option_env!("PIXABAY_API_KEY")
        .map(|s| s.to_string())
        .or_else(|| std::env::var("PIXABAY_API_KEY").ok())
        .unwrap_or_default();
    if api_key.is_empty() {
        warn!("PIXABAY_API_KEY not set, cannot fetch videos");
        return vec![];
    }

    let mut rng = rand::rng();

    let keyword = {
        let kws = flavor.keywords();
        kws.choose(&mut rng).unwrap_or(&kws[0])
    };

    let order = if rand::random::<bool>() { "popular" } else { "latest" };

    let category = flavor.category();

    let url = format!(
        "https://pixabay.com/api/videos/?key={}&q={}&video_type=film&category={}&per_page={}&safesearch=true&order={}",
        api_key,
        urlencodeq(keyword),
        category,
        PIXABAY_PER_PAGE,
        order,
    );

    info!(
        "Pixabay: fetching videos for '{}' (keyword='{}', order={})",
        flavor.name(),
        keyword,
        order,
    );

    let body: serde_json::Value = match ureq::get(&url).call() {
        Ok(resp) => match resp.into_body().read_json() {
            Ok(v) => v,
            Err(e) => {
                warn!("Pixabay: failed to parse response: {e}");
                return vec![];
            }
        },
        Err(e) => {
            warn!("Pixabay: request failed: {e}");
            return vec![];
        }
    };

    let hits = match body["hits"].as_array() {
        Some(h) => h,
        None => {
            warn!("Pixabay: no hits in response");
            return vec![];
        }
    };

    let dir = cache_dir(flavor);

    let mut results: Vec<PendingDownload> = hits
        .iter()
        .filter_map(|hit| {
            let video_id = hit["id"].as_u64().unwrap_or(0);
            let video_url = hit["videos"]["large"]["url"]
                .as_str()
                .or_else(|| hit["videos"]["medium"]["url"].as_str())?;
            Some(PendingDownload {
                video_id,
                url: video_url.to_string(),
                dest: dir.join(format!("{video_id}.mp4")),
            })
        })
        .collect();

    results.shuffle(&mut rng);
    results
}

fn download_file(url: &str, dest: &PathBuf) -> Result<(), String> {
    let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let mut body = resp.into_body();
    let mut reader = body.as_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    Ok(())
}

fn urlencodeq(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b' ' => "+".to_string(),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Download one video per flavor that has no cached videos yet.
/// Intended to be called during first-launch setup so video backgrounds
/// are ready when the user first opens a flavor.
pub fn prefetch_one_per_flavor(mut on_progress: impl FnMut(&str)) {
    for flavor in VideoFlavor::ALL {
        let existing = cached_videos(*flavor);
        if !existing.is_empty() {
            on_progress(&format!("{}: already cached", flavor.name()));
            continue;
        }

        on_progress(&format!("{}: fetching listing...", flavor.name()));
        let listing = fetch_video_listing(*flavor);
        let first = listing.into_iter().find(|p| !p.dest.exists());
        let Some(dl) = first else {
            on_progress(&format!("{}: no videos available", flavor.name()));
            continue;
        };

        on_progress(&format!("{}: downloading...", flavor.name()));
        match download_file(&dl.url, &dl.dest) {
            Ok(_) => {
                on_progress(&format!("{}: ready", flavor.name()));
                info!("Prefetch: saved {} for {}", dl.dest.display(), flavor.name());
            }
            Err(e) => {
                on_progress(&format!("{}: download failed", flavor.name()));
                warn!("Prefetch: failed for {}: {e}", flavor.name());
            }
        }
    }
}

fn background_video_worker(
    flavor: VideoFlavor,
    frame_tx: mpsc::SyncSender<Vec<u8>>,
    free_rx: mpsc::Receiver<Vec<u8>>,
    recycle_tx: mpsc::SyncSender<Vec<u8>>,
    cmd_rx: mpsc::Receiver<DecoderCommand>,
    stop_downloads: Arc<AtomicBool>,
) {
    let mut existing = cached_videos(flavor);

    if !existing.is_empty() {
        let mut rng = rand::rng();
        existing.shuffle(&mut rng);

        let playlist = std::sync::Arc::new(std::sync::Mutex::new(existing));

        {
            let playlist_ref = playlist.clone();
            let stop = stop_downloads.clone();
            thread::Builder::new()
                .name("video-dl".into())
                .spawn(move || {
                    download_and_refresh(flavor, &playlist_ref, &stop);
                })
                .ok();
        }

        pipeline_decode_loop(playlist, frame_tx, free_rx, recycle_tx, cmd_rx);
        return;
    }

    let pending: Vec<PendingDownload> = fetch_video_listing(flavor)
        .into_iter()
        .filter(|p| !p.dest.exists())
        .take(MAX_CACHED_VIDEOS)
        .collect();

    let mut pending_iter = pending.into_iter();

    if let Some(first) = pending_iter.next() {
        info!("Pixabay: downloading first video {}...", first.video_id);
        match download_file(&first.url, &first.dest) {
            Ok(_) => {
                info!("Pixabay: saved {}", first.dest.display());
                let playlist =
                    std::sync::Arc::new(std::sync::Mutex::new(vec![first.dest]));

                let remaining: Vec<PendingDownload> = pending_iter.collect();
                if !remaining.is_empty() {
                    let playlist_ref = playlist.clone();
                    let stop = stop_downloads.clone();
                    thread::Builder::new()
                        .name("video-dl".into())
                        .spawn(move || {
                            for dl in remaining {
                                if stop.load(Ordering::Relaxed) {
                                    return;
                                }
                                info!(
                                    "Pixabay: downloading video {} in background...",
                                    dl.video_id
                                );
                                match download_file(&dl.url, &dl.dest) {
                                    Ok(_) => {
                                        info!("Pixabay: saved {}", dl.dest.display());
                                        if let Ok(mut pl) = playlist_ref.lock() {
                                            pl.push(dl.dest);
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Pixabay: download failed for {}: {e}",
                                            dl.video_id
                                        );
                                    }
                                }
                            }
                        })
                        .ok();
                }

                pipeline_decode_loop(playlist, frame_tx, free_rx, recycle_tx, cmd_rx);
                return;
            }
            Err(e) => {
                warn!("Pixabay: download failed for {}: {e}", first.video_id);
            }
        }
    }

    warn!("No videos available for flavor '{}'", flavor.name());
}

fn download_and_refresh(
    flavor: VideoFlavor,
    playlist: &std::sync::Mutex<Vec<PathBuf>>,
    stop: &AtomicBool,
) {
    let listing = fetch_video_listing(flavor);
    let flavor_name = flavor.name().to_string();

    let current_count = playlist.lock().unwrap().len();
    let needed = MAX_CACHED_VIDEOS.saturating_sub(current_count);

    if needed > 0 {
        for dl in listing.iter().filter(|p| !p.dest.exists()).take(needed) {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            info!(
                "Pixabay[{}]: downloading video {} in background...",
                flavor_name, dl.video_id
            );
            match download_file(&dl.url, &dl.dest) {
                Ok(_) => {
                    info!("Pixabay: saved {}", dl.dest.display());
                    if let Ok(mut pl) = playlist.lock() {
                        pl.push(dl.dest.clone());
                    }
                }
                Err(e) => {
                    warn!(
                        "Pixabay[{}]: download failed for {}: {e}",
                        flavor_name, dl.video_id
                    );
                }
            }
        }
    }

    const ROTATE_COUNT: usize = 2;
    let mut rotated = 0;
    for dl in listing.iter().filter(|p| !p.dest.exists()) {
        if stop.load(Ordering::Relaxed) || rotated >= ROTATE_COUNT {
            break;
        }
        info!(
            "Pixabay[{}]: rotating cache — downloading new video {}...",
            flavor_name, dl.video_id
        );
        match download_file(&dl.url, &dl.dest) {
            Ok(_) => {
                info!("Pixabay: saved {}", dl.dest.display());
                if let Ok(mut pl) = playlist.lock() {
                    pl.push(dl.dest.clone());
                    if pl.len() > MAX_CACHED_VIDEOS {
                        let evicted = pl.remove(0);
                        info!("Pixabay[{}]: evicting old video {}", flavor_name, evicted.display());
                        std::fs::remove_file(&evicted).ok();
                    }
                }
                rotated += 1;
            }
            Err(e) => {
                warn!(
                    "Pixabay[{}]: download failed for {}: {e}",
                    flavor_name, dl.video_id
                );
            }
        }
    }
}

fn decode_video(
    path: &PathBuf,
    frame_tx: &mpsc::SyncSender<Vec<u8>>,
    free_rx: &mpsc::Receiver<Vec<u8>>,
    recycle_tx: &mpsc::SyncSender<Vec<u8>>,
    cmd_rx: &mpsc::Receiver<DecoderCommand>,
) -> bool {
    decode_video_at(path, frame_tx, free_rx, recycle_tx, cmd_rx, 0.0)
}

fn decode_video_at(
    path: &PathBuf,
    frame_tx: &mpsc::SyncSender<Vec<u8>>,
    free_rx: &mpsc::Receiver<Vec<u8>>,
    recycle_tx: &mpsc::SyncSender<Vec<u8>>,
    cmd_rx: &mpsc::Receiver<DecoderCommand>,
    start_secs: f64,
) -> bool {
    info!("Video decoder: playing {} (seek={:.1}s)", path.display(), start_secs);

    let mut args: Vec<String> = Vec::new();
    if start_secs > 0.5 {
        args.extend(["-ss".into(), format!("{start_secs:.3}")]);
    }
    args.extend([
        "-i".into(),
        path.to_string_lossy().into_owned(),
        "-f".into(), "rawvideo".into(),
        "-pix_fmt".into(), "rgba".into(),
        "-s".into(), format!("{VIDEO_WIDTH}x{VIDEO_HEIGHT}"),
        "-r".into(), format!("{TARGET_FPS}"),
        "-v".into(), "error".into(),
        "-".into(),
    ]);

    let result = crate::vendor::silent_command(crate::vendor::ffmpeg_path())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match result {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to start ffmpeg: {e}. Is ffmpeg installed?");
            return false;
        }
    };

    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            error!("No stdout from ffmpeg");
            return false;
        }
    };

    loop {
        if should_stop(cmd_rx) {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }

        let mut buf = match free_rx.recv() {
            Ok(b) => b,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        };

        match stdout.read_exact(&mut buf) {
            Ok(_) => {
                if frame_tx.send(buf).is_err() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
            Err(_) => {
                let _ = recycle_tx.send(buf);
                break;
            }
        }
    }

    let _ = child.wait();
    true
}


fn pipeline_decode_loop(
    playlist: std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>,
    frame_tx: mpsc::SyncSender<Vec<u8>>,
    free_rx: mpsc::Receiver<Vec<u8>>,
    recycle_tx: mpsc::SyncSender<Vec<u8>>,
    cmd_rx: mpsc::Receiver<DecoderCommand>,
) {
    let mut rng = rand::rng();
    loop {
        let order: Vec<PathBuf> = {
            let pl = playlist.lock().unwrap();
            if pl.is_empty() {
                return;
            }
            let mut snapshot = pl.clone();
            snapshot.shuffle(&mut rng);
            snapshot
        };

        for path in &order {
            if should_stop(&cmd_rx) {
                return;
            }
            if !decode_video(path, &frame_tx, &free_rx, &recycle_tx, &cmd_rx) {
                return;
            }
        }
    }
}

fn should_stop(cmd_rx: &mpsc::Receiver<DecoderCommand>) -> bool {
    match cmd_rx.try_recv() {
        Ok(DecoderCommand::Stop) | Err(mpsc::TryRecvError::Disconnected) => true,
        Err(mpsc::TryRecvError::Empty) => false,
    }
}

pub fn spawn_video_background(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    flavor: VideoFlavor,
) {
    let size = Extent3d {
        width: VIDEO_WIDTH,
        height: VIDEO_HEIGHT,
        depth_or_array_layers: 1,
    };
    let image = Image {
        data: Some(vec![0u8; FRAME_BYTES]),
        texture_descriptor: bevy::render::render_resource::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
                | bevy::render::render_resource::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        sampler: ImageSampler::linear(),
        ..default()
    };
    let image_handle = images.add(image);

    let (frame_tx, frame_rx) = mpsc::sync_channel(POOL_SIZE);
    let (free_tx, free_rx) = mpsc::sync_channel(POOL_SIZE);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let stop_downloads = Arc::new(AtomicBool::new(false));

    for _ in 0..POOL_SIZE {
        let _ = free_tx.send(vec![0u8; FRAME_BYTES]);
    }

    let recycle_tx = free_tx.clone();
    let stop_clone = stop_downloads.clone();
    thread::Builder::new()
        .name("video-bg-worker".into())
        .spawn(move || {
            background_video_worker(flavor, frame_tx, free_rx, recycle_tx, cmd_rx, stop_clone);
        })
        .expect("failed to spawn video worker thread");

    commands.insert_resource(VideoBackground {
        image_handle: image_handle.clone(),
        flavor,
        frame_rx: Mutex::new(frame_rx),
        free_tx: Mutex::new(free_tx),
        cmd_tx: Mutex::new(cmd_tx),
        stop_downloads,
        elapsed: 0.0,
        frame_interval: 1.0 / TARGET_FPS,
    });

    commands.spawn((
        VideoSprite,
        Sprite::from_image(image_handle),
        Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)),
    ));
}

pub fn switch_flavor(video_bg: &mut VideoBackground, new_flavor: VideoFlavor) {
    if let Ok(tx) = video_bg.cmd_tx.lock() {
        let _ = tx.send(DecoderCommand::Stop);
    }
    video_bg.stop_downloads.store(true, Ordering::Relaxed);

    let (frame_tx, frame_rx) = mpsc::sync_channel(POOL_SIZE);
    let (free_tx, free_rx) = mpsc::sync_channel(POOL_SIZE);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let stop_downloads = Arc::new(AtomicBool::new(false));

    for _ in 0..POOL_SIZE {
        let _ = free_tx.send(vec![0u8; FRAME_BYTES]);
    }

    let flavor = new_flavor;
    let recycle_tx = free_tx.clone();
    let stop_clone = stop_downloads.clone();
    thread::Builder::new()
        .name("video-bg-worker".into())
        .spawn(move || {
            background_video_worker(flavor, frame_tx, free_rx, recycle_tx, cmd_rx, stop_clone);
        })
        .expect("failed to spawn video worker thread");

    if let Ok(mut rx) = video_bg.frame_rx.lock() {
        *rx = frame_rx;
    }
    if let Ok(mut tx) = video_bg.free_tx.lock() {
        *tx = free_tx;
    }
    if let Ok(mut tx) = video_bg.cmd_tx.lock() {
        *tx = cmd_tx;
    }
    video_bg.stop_downloads = stop_downloads;
    video_bg.flavor = new_flavor;
    video_bg.elapsed = 0.0;
}

pub fn seek_source_video(video_bg: &mut VideoBackground, source_path: PathBuf, start_secs: f64) {
    if let Ok(tx) = video_bg.cmd_tx.lock() {
        let _ = tx.send(DecoderCommand::Stop);
    }

    let (frame_tx, frame_rx) = mpsc::sync_channel(POOL_SIZE);
    let (free_tx, free_rx) = mpsc::sync_channel(POOL_SIZE);
    let (cmd_tx, cmd_rx) = mpsc::channel();

    for _ in 0..POOL_SIZE {
        let _ = free_tx.send(vec![0u8; FRAME_BYTES]);
    }

    let recycle_tx = free_tx.clone();
    thread::Builder::new()
        .name("source-video-worker".into())
        .spawn(move || {
            source_video_decode_loop(source_path, frame_tx, free_rx, recycle_tx, cmd_rx, start_secs);
        })
        .expect("failed to spawn source video worker");

    if let Ok(mut rx) = video_bg.frame_rx.lock() { *rx = frame_rx; }
    if let Ok(mut tx) = video_bg.free_tx.lock() { *tx = free_tx; }
    if let Ok(mut tx) = video_bg.cmd_tx.lock() { *tx = cmd_tx; }
    video_bg.elapsed = 0.0;
}

pub fn update_video_frame(
    time: Res<Time>,
    mut video_bg: ResMut<VideoBackground>,
    mut images: ResMut<Assets<Image>>,
) {
    video_bg.elapsed += time.delta_secs_f64();
    if video_bg.elapsed < video_bg.frame_interval {
        return;
    }
    video_bg.elapsed -= video_bg.frame_interval;

    let mut latest_frame = None;
    if let Ok(rx) = video_bg.frame_rx.lock() {
        if let Ok(frame) = rx.try_recv() {
            latest_frame = Some(frame);
        }
    }

    if let Some(frame_data) = latest_frame {
        let handle = video_bg.image_handle.clone();
        if let Some(image) = images.get_mut(&handle) {
            if let Some(old_data) = image.data.replace(frame_data) {
                if old_data.len() == FRAME_BYTES {
                    if let Ok(ftx) = video_bg.free_tx.lock() {
                        let _ = ftx.send(old_data);
                    }
                }
            }
        }
    }
}

pub fn fit_video_to_window(
    windows: Query<&Window>,
    mut sprite_query: Query<&mut Transform, With<VideoSprite>>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok(mut transform) = sprite_query.single_mut() else {
        return;
    };

    let scale_x = window.width() / VIDEO_WIDTH as f32;
    let scale_y = window.height() / VIDEO_HEIGHT as f32;
    let scale = scale_x.max(scale_y);

    transform.scale = Vec3::new(scale, scale, 1.0);
    transform.translation.z = -10.0;
}

pub fn despawn_video_background(
    commands: &mut Commands,
    sprite_query: &Query<Entity, With<VideoSprite>>,
) {
    for entity in sprite_query.iter() {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<VideoBackground>();
}

pub fn spawn_source_video_background(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    source_path: PathBuf,
    start_secs: f64,
) {
    let size = Extent3d {
        width: VIDEO_WIDTH,
        height: VIDEO_HEIGHT,
        depth_or_array_layers: 1,
    };
    let image = Image {
        data: Some(vec![0u8; FRAME_BYTES]),
        texture_descriptor: bevy::render::render_resource::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
                | bevy::render::render_resource::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        sampler: ImageSampler::linear(),
        ..default()
    };
    let image_handle = images.add(image);

    let (frame_tx, frame_rx) = mpsc::sync_channel(POOL_SIZE);
    let (free_tx, free_rx) = mpsc::sync_channel(POOL_SIZE);
    let (cmd_tx, cmd_rx) = mpsc::channel();

    for _ in 0..POOL_SIZE {
        let _ = free_tx.send(vec![0u8; FRAME_BYTES]);
    }

    let recycle_tx = free_tx.clone();
    thread::Builder::new()
        .name("source-video-worker".into())
        .spawn(move || {
            source_video_decode_loop(source_path, frame_tx, free_rx, recycle_tx, cmd_rx, start_secs);
        })
        .expect("failed to spawn source video worker thread");

    commands.insert_resource(VideoBackground {
        image_handle: image_handle.clone(),
        flavor: VideoFlavor::Nature,
        frame_rx: Mutex::new(frame_rx),
        free_tx: Mutex::new(free_tx),
        cmd_tx: Mutex::new(cmd_tx),
        stop_downloads: Arc::new(AtomicBool::new(false)),
        elapsed: 0.0,
        frame_interval: 1.0 / TARGET_FPS,
    });

    commands.spawn((
        VideoSprite,
        Sprite::from_image(image_handle),
        Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)),
    ));
}

fn source_video_decode_loop(
    path: PathBuf,
    frame_tx: mpsc::SyncSender<Vec<u8>>,
    free_rx: mpsc::Receiver<Vec<u8>>,
    recycle_tx: mpsc::SyncSender<Vec<u8>>,
    cmd_rx: mpsc::Receiver<DecoderCommand>,
    start_secs: f64,
) {
    let mut first = true;
    loop {
        if should_stop(&cmd_rx) {
            return;
        }
        let seek = if first { start_secs } else { 0.0 };
        first = false;
        if !decode_video_at(&path, &frame_tx, &free_rx, &recycle_tx, &cmd_rx, seek) {
            return;
        }
    }
}

impl Drop for VideoBackground {
    fn drop(&mut self) {
        self.stop_downloads.store(true, Ordering::Relaxed);
        if let Ok(tx) = self.cmd_tx.lock() {
            let _ = tx.send(DecoderCommand::Stop);
        }
    }
}
