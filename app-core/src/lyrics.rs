use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use crate::analyzer::{enqueue_one, is_usdx_song, update_song_analyzed};
use crate::cache::CacheDir;
use crate::library_db;
use crate::song::Song;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LrclibCandidate {
    #[serde(default, alias = "trackName")]
    pub track_name: String,
    #[serde(default, alias = "artistName")]
    pub artist_name: String,
    #[serde(default, alias = "albumName")]
    pub album_name: String,
    #[serde(default, alias = "duration")]
    pub duration_secs: f64,
    #[serde(skip_deserializing, default)]
    pub lines: Vec<String>,
    #[serde(default, rename = "plainLyrics", skip_serializing)]
    #[ts(skip)]
    plain_lyrics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LyricsFile {
    pub lines: Vec<String>,
}

pub fn lrclib_candidates(song: &Song) -> Vec<LrclibCandidate> {
    let title = &song.title;
    let artist = &song.artist;

    if title.is_empty() || artist == "Unknown Artist" {
        return Vec::new();
    }

    let agent = ureq::Agent::new_with_defaults();

    info!(
        "[lrclib] Searching: \"{title}\" by \"{artist}\" ({:.0}s, album=\"{}\")",
        song.duration_secs, song.album
    );

    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding::encode(title),
        urlencoding::encode(artist),
    );
    let resp = match agent
        .get(&url)
        .header("User-Agent", "Nightingale/1.0")
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            warn!("[lrclib] Search request failed: {e}");
            return Vec::new();
        }
    };
    let results: Vec<LrclibCandidate> = match resp.into_body().read_json() {
        Ok(r) => r,
        Err(e) => {
            warn!("[lrclib] Failed to parse search results: {e}");
            return Vec::new();
        }
    };

    let mut with_lyrics: Vec<_> = results
        .into_iter()
        .filter(|r| !r.plain_lyrics.is_empty())
        .collect();

    info!(
        "[lrclib] Search returned {} results with plain lyrics",
        with_lyrics.len()
    );

    let album_lower = song.album.to_lowercase();
    with_lyrics.sort_by_key(|r| {
        let album_bonus: i64 = if r.album_name.to_lowercase() == album_lower {
            0
        } else {
            5_000
        };
        let duration_penalty = ((r.duration_secs - song.duration_secs).abs() * 10.0) as i64;
        album_bonus + duration_penalty
    });

    with_lyrics
        .into_iter()
        .filter_map(|mut r| {
            r.lines = r
                .plain_lyrics
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if r.lines.is_empty() { None } else { Some(r) }
        })
        .collect()
}

pub fn search_lrclib_for_hash(file_hash: &str) -> Vec<LrclibCandidate> {
    let Some(song) = library_db::load_song_by_hash(file_hash).ok().flatten() else {
        return Vec::new();
    };
    lrclib_candidates(&song)
}

pub fn load_lyrics_file(file_hash: &str) -> Option<LyricsFile> {
    let cache = CacheDir::new();
    let path = cache.lyrics_path(file_hash);
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice::<LyricsFile>(&bytes).ok()
}

pub fn save_lyrics_and_realign(file_hash: &str, lines: Vec<String>) -> Result<(), String> {
    if is_usdx_song(file_hash) {
        return Err("Cannot edit lyrics for USDX songs".to_string());
    }

    let normalized: Vec<String> = lines
        .into_iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if normalized.is_empty() {
        return Err("Lyrics cannot be empty".to_string());
    }

    let cache = CacheDir::new();
    let previous_language = library_db::load_song_by_hash(file_hash)
        .ok()
        .flatten()
        .and_then(|song| song.language);
    write_lyrics_file(&cache, file_hash, &normalized)
        .map_err(|e| format!("Failed to write lyrics file: {e}"))?;

    let _ = std::fs::remove_file(cache.transcript_path(file_hash));
    cache.delete_transcript_variants(file_hash);

    update_song_analyzed(file_hash, false, previous_language, None, None, None);
    enqueue_one(file_hash);
    Ok(())
}

pub(crate) fn write_lyrics_file(
    cache: &CacheDir,
    file_hash: &str,
    lines: &[String],
) -> std::io::Result<PathBuf> {
    let out = cache.lyrics_path(file_hash);
    let lyrics_json = serde_json::json!({ "lines": lines });
    std::fs::write(&out, serde_json::to_string_pretty(&lyrics_json).unwrap())?;
    Ok(out)
}

pub(crate) fn fetch_lrclib_lyrics(song: &Song, cache: &CacheDir) -> Option<PathBuf> {
    let existing = cache.lyrics_path(&song.file_hash);
    if existing.is_file() {
        info!(
            "[lrclib] Using existing lyrics file at {}",
            existing.display()
        );
        return Some(existing);
    }

    let candidates = lrclib_candidates(song);
    let pick = candidates.into_iter().next()?;

    info!(
        "[lrclib] Picked \"{}\" from \"{}\" (duration {:.0}s, delta {:.1}s)",
        pick.track_name,
        pick.album_name,
        pick.duration_secs,
        (pick.duration_secs - song.duration_secs).abs()
    );
    info!("[lrclib] Extracted {} lines", pick.lines.len());

    match write_lyrics_file(cache, &song.file_hash, &pick.lines) {
        Ok(out) => {
            info!("[lrclib] Lyrics saved to {}", out.display());
            Some(out)
        }
        Err(e) => {
            warn!("[lrclib] Failed to write lyrics: {e}");
            None
        }
    }
}
