//! Ingest: one call takes a YouTube URL or search string and produces an
//! analyzed, lyric-aligned song in the library. Automates the manual sequence
//! proven in Phase 0 (download -> scan -> analyze), relying on the analyzer's
//! own LRCLIB-first path (which fires automatically once `--embed-metadata`
//! gives the file real artist/title tags) rather than a separate lyrics step.
//!
//! The pipeline is blocking and slow (~4-5 min for a fresh song, analysis is
//! serial on 8 GB VRAM), so `party_ingest` kicks it off on a background thread
//! and reports progress over the event bus as `party.ingest` frames. Callers
//! (the Step 3 queue, or a test) observe those or poll `load_songs`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::commands::{ApiError, CmdResult};
use crate::events::EventBus;
use crate::state::AppState;

/// How long to wait for a fresh scan to surface the downloaded file, and for
/// analysis to finish. Analysis of a 5-minute song took ~4.5 min in Phase 0;
/// give generous headroom for a cold model load on the first song.
const SCAN_TIMEOUT: Duration = Duration::from_secs(120);
const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const POLL_INTERVAL: Duration = Duration::from_millis(750);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IngestArgs {
    query: String,
}

/// A terminal or intermediate progress frame, broadcast as `party.ingest`.
#[derive(Debug, Serialize)]
struct IngestProgress<'a> {
    query: &'a str,
    stage: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcript_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// `POST /api/cmd/party_ingest {"query": "..."}`
///
/// Returns immediately after spawning the pipeline. Watch `party.ingest`
/// events (or poll `load_songs`) for completion.
pub async fn party_ingest(state: &AppState, payload: Value) -> CmdResult {
    let args: IngestArgs = serde_json::from_value(payload)
        .map_err(|e| ApiError(axum::http::StatusCode::BAD_REQUEST, format!("invalid args: {e}")))?;

    let query = args.query.trim().to_string();
    if query.is_empty() {
        return Err(ApiError(
            axum::http::StatusCode::BAD_REQUEST,
            "query must not be empty".to_string(),
        ));
    }

    let events = state.events.clone();
    // The pipeline shells out and blocks on analysis, so keep it off the async
    // runtime entirely (a plain OS thread, as the shift_* commands do).
    std::thread::spawn(move || {
        run_ingest(&query, &events);
    });

    Ok(json!({ "started": true }))
}

/// Run the full download -> scan -> analyze pipeline, emitting progress. Errors
/// become a terminal `error` frame rather than a panic.
fn run_ingest(query: &str, events: &EventBus) {
    let emit = |stage: &str, hash: Option<String>, err: Option<String>| {
        events.emit(
            "party.ingest",
            &IngestProgress {
                query,
                stage,
                file_hash: hash,
                title: None,
                artist: None,
                transcript_source: None,
                error: err,
            },
        );
    };

    emit("downloading", None, None);

    let library_dir = match folder_library_dir() {
        Some(dir) => dir,
        None => {
            emit(
                "error",
                None,
                Some("party ingest requires a Folder library source".to_string()),
            );
            return;
        }
    };

    let downloaded = match download(query, &library_dir) {
        Ok(path) => path,
        Err(e) => {
            emit("error", None, Some(format!("download failed: {e}")));
            return;
        }
    };

    emit("scanning", None, None);
    let hash = match scan_for_path(&downloaded) {
        Ok(h) => h,
        Err(e) => {
            emit("error", None, Some(format!("scan failed: {e}")));
            return;
        }
    };

    // Idempotency (T2.3): if this exact file is already analyzed, skip the
    // ~5-minute analysis entirely and report ready. yt-dlp already no-ops the
    // re-download when the file is present, so a repeated ingest is cheap.
    let already_analyzed = app_core::song_by_hash(&hash)
        .map(|s| s.is_analyzed)
        .unwrap_or(false);

    if !already_analyzed {
        emit("analyzing", Some(hash.clone()), None);
        app_core::enqueue_one(&hash);
        if let Err(e) = wait_for_analysis(&hash) {
            emit("error", Some(hash.clone()), Some(e));
            return;
        }
    }

    // Report the final transcript source so callers can see whether LRCLIB
    // matched (source "lyrics"/Lrc) or it fell back to Whisper ("generated").
    let song = app_core::song_by_hash(&hash);
    events.emit(
        "party.ingest",
        &IngestProgress {
            query,
            stage: "ready",
            file_hash: Some(hash.clone()),
            title: song.as_ref().map(|s| s.title.clone()),
            artist: song.as_ref().map(|s| s.artist.clone()),
            transcript_source: song
                .as_ref()
                .and_then(|s| s.transcript_source.as_ref())
                .map(|ts| format!("{ts:?}")),
            error: None,
        },
    );
}

/// The configured library folder, or `None` if the source is remote/unset.
pub(crate) fn folder_library_dir() -> Option<PathBuf> {
    match app_core::AppConfig::load().library_source {
        Some(app_core::LibrarySource::Folder { path }) => Some(path),
        _ => None,
    }
}

/// Path to the vendored ffmpeg, which yt-dlp needs for muxing.
fn ffmpeg_path() -> PathBuf {
    app_core::nightingale_dir()
        .join("vendor")
        .join("ffmpeg.exe")
}

/// Resolve the yt-dlp executable. Order: `NIGHTINGALE_YTDLP` override, then a
/// bare `yt-dlp` on PATH, then the per-user WinGet package location (where it
/// installs without a PATH shim on this machine). Returns the string to pass to
/// `Command::new`.
pub(crate) fn resolve_ytdlp() -> Option<String> {
    if let Ok(p) = std::env::var("NIGHTINGALE_YTDLP") {
        let p = p.trim();
        if !p.is_empty() {
            return Some(p.to_string());
        }
    }

    // Trust PATH if a resolvable yt-dlp exists there.
    if Command::new("yt-dlp")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some("yt-dlp".to_string());
    }

    // WinGet installs per-user under LOCALAPPDATA with a stable package folder
    // name but no Links shim on this box.
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let candidate = Path::new(&local)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages")
            .join("yt-dlp.yt-dlp_Microsoft.Winget.Source_8wekyb3d8bbwe")
            .join("yt-dlp.exe");
        if candidate.exists() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

/// Build the yt-dlp argument vector. Pure so it can be unit-tested (T2.1): the
/// one flag whose absence silently breaks the LRCLIB path is `--embed-metadata`.
///
/// A bare query is wrapped as `ytsearch1:<query>`; an `http(s)` URL or an
/// already-`ytsearch`-prefixed target is passed through untouched.
fn build_ytdlp_args(query: &str, library_dir: &Path, ffmpeg: &Path) -> Vec<String> {
    let target = resolve_target(query);
    let output_template = library_dir
        .join("%(artist)s - %(title)s.%(ext)s")
        .to_string_lossy()
        .into_owned();

    vec![
        // UTF-8 output so unicode in the printed final path survives on Windows.
        "--encoding".to_string(),
        "utf-8".to_string(),
        // Best video + best audio, preferring <=1080p, muxed to mp4.
        "-f".to_string(),
        "bv*+ba/b".to_string(),
        "-S".to_string(),
        "res:1080".to_string(),
        "--merge-output-format".to_string(),
        "mp4".to_string(),
        // Mandatory: embeds artist/title tags so the scanner indexes real
        // metadata and the analyzer's LRCLIB search can match.
        "--embed-metadata".to_string(),
        "--windows-filenames".to_string(),
        "--no-playlist".to_string(),
        "--ffmpeg-location".to_string(),
        ffmpeg.to_string_lossy().into_owned(),
        "-o".to_string(),
        output_template,
        // Download AND print the final on-disk path so we can locate the file
        // to scan without diffing the directory.
        "--no-simulate".to_string(),
        "--print".to_string(),
        "after_move:filepath".to_string(),
        target,
    ]
}

/// Wrap a bare search string; pass URLs and existing search prefixes through.
fn resolve_target(query: &str) -> String {
    let q = query.trim();
    let lower = q.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ytsearch")
    {
        q.to_string()
    } else {
        format!("ytsearch1:{q}")
    }
}

/// Run yt-dlp and return the final on-disk path of the downloaded file.
pub(crate) fn download(query: &str, library_dir: &Path) -> Result<PathBuf, String> {
    let ytdlp = resolve_ytdlp().ok_or_else(|| {
        "yt-dlp not found (set NIGHTINGALE_YTDLP, or install it on PATH)".to_string()
    })?;
    let ffmpeg = ffmpeg_path();
    let args = build_ytdlp_args(query, library_dir, &ffmpeg);

    let output = Command::new(&ytdlp)
        .args(&args)
        .output()
        .map_err(|e| format!("failed to launch yt-dlp ({ytdlp}): {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Surface the last non-empty stderr line, which is usually the real
        // cause (age gate, region lock, no results).
        let reason = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown error");
        return Err(reason.to_string());
    }

    // `after_move:filepath` prints the final path as the last stdout line.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "yt-dlp produced no output path".to_string())?;

    if !path.exists() {
        return Err(format!("yt-dlp reported {} but it does not exist", path.display()));
    }
    Ok(path)
}

/// Trigger a scan and wait for the given file path to surface as an indexed
/// song, returning its blake3 file hash.
pub(crate) fn scan_for_path(path: &Path) -> Result<String, String> {
    app_core::start_scan();
    let deadline = Instant::now() + SCAN_TIMEOUT;
    loop {
        if let Some(hash) = hash_for_path(path) {
            return Ok(hash);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "file {} not indexed within {}s",
                path.display(),
                SCAN_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Find the indexed song whose path matches `path` (case-insensitive, since
/// Windows paths are), returning its hash.
fn hash_for_path(path: &Path) -> Option<String> {
    let want = normalize_path(path);
    app_core::SongsStore::load_all()
        .processed
        .into_iter()
        .find(|s| normalize_path(Path::new(&s.path)) == want)
        .map(|s| s.file_hash)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase().replace('/', "\\")
}

/// Poll the analysis queue until the hash leaves it analyzed, or it fails.
pub(crate) fn wait_for_analysis(hash: &str) -> Result<(), String> {
    use app_core::AnalysisQueue;

    let deadline = Instant::now() + ANALYSIS_TIMEOUT;
    loop {
        let queue = AnalysisQueue::load();
        match queue.entries.get(hash) {
            // A failed entry stays in the queue carrying its message.
            Some(app_core::QueuedStatus::Failed(msg)) => {
                return Err(format!("analysis failed: {msg}"));
            }
            // Still queued or analyzing: keep waiting.
            Some(_) => {}
            // Gone from the queue: done iff the song is now marked analyzed.
            None => {
                if app_core::song_by_hash(hash)
                    .map(|s| s.is_analyzed)
                    .unwrap_or(false)
                {
                    return Ok(());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "analysis of {hash} did not finish within {}s",
                ANALYSIS_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_for(query: &str) -> Vec<String> {
        build_ytdlp_args(
            query,
            Path::new("C:\\Music\\Karaoke"),
            Path::new("C:\\data\\vendor\\ffmpeg.exe"),
        )
    }

    // T2.1: --embed-metadata must always be present. Its absence is the proven
    // failure that silently drops every song to slow Whisper transcription.
    #[test]
    fn always_embeds_metadata() {
        assert!(args_for("never gonna give you up").contains(&"--embed-metadata".to_string()));
        assert!(args_for("https://youtu.be/dQw4w9WgXcQ").contains(&"--embed-metadata".to_string()));
    }

    #[test]
    fn forces_utf8_encoding() {
        let args = args_for("some song");
        let i = args.iter().position(|a| a == "--encoding").expect("--encoding present");
        assert_eq!(args[i + 1], "utf-8");
    }

    // T2.1: the output template must live under the library directory.
    #[test]
    fn output_template_under_library() {
        let args = args_for("some song");
        let o = args.iter().position(|a| a == "-o").expect("-o present");
        let template = &args[o + 1];
        assert!(
            template.starts_with("C:\\Music\\Karaoke"),
            "template {template} not under library dir"
        );
        assert!(template.ends_with("%(artist)s - %(title)s.%(ext)s"));
    }

    // T2.1: a bare query is wrapped once; a URL is never wrapped.
    #[test]
    fn wraps_query_but_not_url() {
        let target = args_for("bad romance").last().unwrap().clone();
        assert_eq!(target, "ytsearch1:bad romance");

        let url = "https://www.youtube.com/watch?v=abc";
        let target = args_for(url).last().unwrap().clone();
        assert_eq!(target, url, "a URL must not be wrapped in ytsearch1:");
    }

    // T2.1: an already-prefixed search target is not double-wrapped.
    #[test]
    fn does_not_double_wrap_search_prefix() {
        let target = args_for("ytsearch5:live version").last().unwrap().clone();
        assert_eq!(target, "ytsearch5:live version");
    }

    #[test]
    fn resolve_target_trims_and_lowercases_scheme() {
        assert_eq!(resolve_target("  HTTPS://x/y  "), "HTTPS://x/y");
        assert_eq!(resolve_target("hello world"), "ytsearch1:hello world");
    }
}
