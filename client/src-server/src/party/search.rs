//! Online song search for the "not in my library" flow.
//!
//! Two steps, gated on lyric availability so we only ever offer downloadable
//! songs that will end up with good karaoke lyrics:
//!   1. `party_search_lrclib {query}` finds tracks in LRCLIB (canonical
//!      artist/title, confirms lyrics exist).
//!   2. `party_youtube_candidates {query}` lists YouTube videos to pick from.
//! The guest then enqueues a chosen video via `party_queue_add` carrying the
//! canonical artist/title, which the ingest writes onto the song so the
//! analyzer's own LRCLIB match is guaranteed.

use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::{ApiError, CmdResult};
use crate::party::ingest;

fn bad(e: impl std::fmt::Display) -> ApiError {
    ApiError(axum::http::StatusCode::BAD_REQUEST, format!("invalid args: {e}"))
}

fn serialise_err(e: serde_json::Error) -> ApiError {
    ApiError(
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("serialise: {e}"),
    )
}

#[derive(Debug, Deserialize)]
struct QueryArgs {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

/// `POST /api/cmd/party_search_lrclib {"query": "..."}` -> lyric-available tracks.
pub async fn party_search_lrclib(payload: Value) -> CmdResult {
    let args: QueryArgs = serde_json::from_value(payload).map_err(bad)?;
    let query = args.query.clone();
    // ureq is blocking; keep it off the async runtime.
    let results = tokio::task::spawn_blocking(move || app_core::search_lrclib_query(&query))
        .await
        .map_err(|e| {
            ApiError(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("lrclib task failed: {e}"),
            )
        })?;
    serde_json::to_value(results).map_err(serialise_err)
}

/// A YouTube search hit shown to the guest.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct YoutubeCandidate {
    video_id: String,
    url: String,
    title: String,
    channel: String,
    duration_secs: Option<f64>,
    thumbnail: String,
}

/// `POST /api/cmd/party_youtube_candidates {"query": "...", "limit": 8}`.
pub async fn party_youtube_candidates(payload: Value) -> CmdResult {
    let args: QueryArgs = serde_json::from_value(payload).map_err(bad)?;
    let query = args.query.trim().to_string();
    if query.is_empty() {
        return Err(bad("query must not be empty"));
    }
    let limit = args.limit.unwrap_or(8).clamp(1, 20);

    let candidates = tokio::task::spawn_blocking(move || youtube_candidates(&query, limit))
        .await
        .map_err(|e| {
            ApiError(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("youtube task failed: {e}"),
            )
        })?
        .map_err(|e| ApiError(axum::http::StatusCode::BAD_GATEWAY, e))?;

    serde_json::to_value(candidates).map_err(serialise_err)
}

/// Run a flat yt-dlp search (metadata only, no download) and parse the results.
/// Flat mode is fast; the thumbnail is derived from the video id rather than
/// fetched, so no per-video extraction is needed.
fn youtube_candidates(query: &str, limit: usize) -> Result<Vec<YoutubeCandidate>, String> {
    let ytdlp = ingest::resolve_ytdlp().ok_or_else(|| {
        "yt-dlp not found (set NIGHTINGALE_YTDLP, or install it on PATH)".to_string()
    })?;

    let search = format!("ytsearch{limit}:{query}");
    let output = Command::new(&ytdlp)
        .args([
            // Force UTF-8 stdout: without this yt-dlp emits the Windows ANSI
            // codepage and non-ASCII title characters (en dashes, accents)
            // arrive mangled.
            "--encoding",
            "utf-8",
            "--flat-playlist",
            "--no-warnings",
            "--no-playlist",
            "--print",
            // Tab-separated so titles with odd characters survive parsing.
            "%(id)s\t%(title)s\t%(channel)s\t%(duration)s",
            &search,
        ])
        .output()
        .map_err(|e| format!("failed to launch yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown error");
        return Err(reason.to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let candidates = stdout
        .lines()
        .filter_map(|line| {
            let mut cols = line.splitn(4, '\t');
            let id = cols.next()?.trim();
            if id.is_empty() || id == "NA" {
                return None;
            }
            let title = cols.next().unwrap_or("").trim().to_string();
            let channel = cols.next().unwrap_or("").trim().to_string();
            let duration_secs = cols
                .next()
                .map(str::trim)
                .filter(|d| !d.is_empty() && *d != "NA")
                .and_then(|d| d.parse::<f64>().ok());
            Some(YoutubeCandidate {
                url: format!("https://www.youtube.com/watch?v={id}"),
                thumbnail: format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"),
                video_id: id.to_string(),
                title,
                channel,
                duration_secs,
            })
        })
        .collect();

    Ok(candidates)
}
