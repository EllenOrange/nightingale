use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::thread;

use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::seq::IndexedRandom;

const VIDEO_WIDTH: u32 = 1920;
const VIDEO_HEIGHT: u32 = 1080;
const FRAME_BYTES: usize = (VIDEO_WIDTH * VIDEO_HEIGHT * 4) as usize;
const TARGET_FPS: f64 = 30.0;
const MAX_CACHED_VIDEOS: usize = 5;
const PIXABAY_PER_PAGE: u32 = 20;

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
            Self::Nature => &["nature", "forest", "mountains", "sunset", "ocean", "waterfall"],
            Self::Underwater => &["underwater", "ocean", "coral", "fish", "diving"],
            Self::Space => &["space", "galaxy", "stars", "nebula", "aurora borealis"],
            Self::City => &["city night", "traffic", "skyline", "urban", "neon"],
            Self::Countryside => &["countryside", "farm", "meadow", "fields", "village"],
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

#[derive(Resource)]
pub struct VideoBackground {
    pub image_handle: Handle<Image>,
    #[allow(dead_code)]
    pub flavor: VideoFlavor,
    frame_rx: Mutex<mpsc::Receiver<Vec<u8>>>,
    cmd_tx: Mutex<mpsc::Sender<DecoderCommand>>,
    elapsed: f64,
    frame_interval: f64,
}

enum DecoderCommand {
    Stop,
}

fn cache_dir(flavor: VideoFlavor) -> PathBuf {
    let base = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("nightingale")
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
    let api_key = match std::env::var("PIXABAY_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            warn!("PIXABAY_API_KEY not set, cannot fetch videos");
            return vec![];
        }
    };

    let keyword = {
        let kws = flavor.keywords();
        let mut rng = rand::rng();
        kws.choose(&mut rng).unwrap_or(&kws[0])
    };

    let url = format!(
        "https://pixabay.com/api/videos/?key={}&q={}&per_page={}&safesearch=true&order=popular",
        api_key,
        urlencodeq(keyword),
        PIXABAY_PER_PAGE,
    );

    info!(
        "Pixabay: fetching videos for '{}' (keyword='{}')",
        flavor.name(),
        keyword
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

    hits.iter()
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
        .collect()
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

fn background_video_worker(
    flavor: VideoFlavor,
    frame_tx: mpsc::SyncSender<Vec<u8>>,
    cmd_rx: mpsc::Receiver<DecoderCommand>,
) {
    let existing = cached_videos(flavor);

    if !existing.is_empty() {
        let playlist = std::sync::Arc::new(std::sync::Mutex::new(existing.clone()));
        let need_more = existing.len() < MAX_CACHED_VIDEOS;

        if need_more {
            let playlist_ref = playlist.clone();
            let flavor_name = flavor.name().to_string();
            thread::Builder::new()
                .name("video-dl".into())
                .spawn(move || {
                    download_missing(flavor, &flavor_name, &playlist_ref);
                })
                .ok();
        }

        pipeline_decode_loop(playlist, frame_tx, cmd_rx);
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
                    thread::Builder::new()
                        .name("video-dl".into())
                        .spawn(move || {
                            for dl in remaining {
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

                pipeline_decode_loop(playlist, frame_tx, cmd_rx);
                return;
            }
            Err(e) => {
                warn!("Pixabay: download failed for {}: {e}", first.video_id);
            }
        }
    }

    warn!("No videos available for flavor '{}'", flavor.name());
}

fn download_missing(
    flavor: VideoFlavor,
    flavor_name: &str,
    playlist: &std::sync::Mutex<Vec<PathBuf>>,
) {
    let listing = fetch_video_listing(flavor);
    let needed = {
        let pl = playlist.lock().unwrap();
        MAX_CACHED_VIDEOS.saturating_sub(pl.len())
    };
    for dl in listing.into_iter().filter(|p| !p.dest.exists()).take(needed) {
        info!(
            "Pixabay[{}]: downloading video {} in background...",
            flavor_name, dl.video_id
        );
        match download_file(&dl.url, &dl.dest) {
            Ok(_) => {
                info!("Pixabay: saved {}", dl.dest.display());
                if let Ok(mut pl) = playlist.lock() {
                    pl.push(dl.dest);
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

fn decode_video(
    path: &PathBuf,
    frame_tx: &mpsc::SyncSender<Vec<u8>>,
    cmd_rx: &mpsc::Receiver<DecoderCommand>,
) -> bool {
    info!("Video decoder: playing {}", path.display());

    let result = Command::new("ffmpeg")
        .args([
            "-i",
            path.to_str().unwrap_or(""),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{VIDEO_WIDTH}x{VIDEO_HEIGHT}"),
            "-r",
            &format!("{TARGET_FPS}"),
            "-v",
            "error",
            "-",
        ])
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

    let mut buf = vec![0u8; FRAME_BYTES];
    loop {
        if should_stop(cmd_rx) {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }

        match stdout.read_exact(&mut buf) {
            Ok(_) => {
                if frame_tx.send(buf.clone()).is_err() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
            Err(_) => break,
        }
    }

    let _ = child.wait();
    true
}


fn pipeline_decode_loop(
    playlist: std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>,
    frame_tx: mpsc::SyncSender<Vec<u8>>,
    cmd_rx: mpsc::Receiver<DecoderCommand>,
) {
    let mut idx = 0;
    loop {
        if should_stop(&cmd_rx) {
            return;
        }

        let path = {
            let pl = playlist.lock().unwrap();
            if pl.is_empty() {
                return;
            }
            pl[idx % pl.len()].clone()
        };

        if !decode_video(&path, &frame_tx, &cmd_rx) {
            return;
        }

        idx += 1;
        let len = playlist.lock().unwrap().len();
        if idx >= len && len > 0 {
            idx = 0;
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

    let (frame_tx, frame_rx) = mpsc::sync_channel(2);
    let (cmd_tx, cmd_rx) = mpsc::channel();

    thread::Builder::new()
        .name("video-bg-worker".into())
        .spawn(move || {
            background_video_worker(flavor, frame_tx, cmd_rx);
        })
        .expect("failed to spawn video worker thread");

    commands.insert_resource(VideoBackground {
        image_handle: image_handle.clone(),
        flavor,
        frame_rx: Mutex::new(frame_rx),
        cmd_tx: Mutex::new(cmd_tx),
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

    let (frame_tx, frame_rx) = mpsc::sync_channel(2);
    let (cmd_tx, cmd_rx) = mpsc::channel();

    let flavor = new_flavor;
    thread::Builder::new()
        .name("video-bg-worker".into())
        .spawn(move || {
            background_video_worker(flavor, frame_tx, cmd_rx);
        })
        .expect("failed to spawn video worker thread");

    if let Ok(mut rx) = video_bg.frame_rx.lock() {
        *rx = frame_rx;
    }
    if let Ok(mut tx) = video_bg.cmd_tx.lock() {
        *tx = cmd_tx;
    }
    video_bg.flavor = new_flavor;
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
        while let Ok(frame) = rx.try_recv() {
            latest_frame = Some(frame);
        }
    }

    if let Some(frame_data) = latest_frame {
        let handle = video_bg.image_handle.clone();
        if let Some(image) = images.get_mut(&handle) {
            image.data = Some(frame_data);
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

impl Drop for VideoBackground {
    fn drop(&mut self) {
        if let Ok(tx) = self.cmd_tx.lock() {
            let _ = tx.send(DecoderCommand::Stop);
        }
    }
}
