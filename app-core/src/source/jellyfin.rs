//! Jellyfin media source. Talks to a Jellyfin server's REST API (see
//! [api.jellyfin.org](https://api.jellyfin.org/)).
//!
//! Scan flow:
//!  1. `GET /Items?Recursive=true&IncludeItemTypes=Audio,MusicVideo,Movie,Video,Episode`
//!     paginated by `StartIndex`/`Limit` and ordered by `SortName` for stable
//!     pagination.
//!  2. For each item we synthesize a `Song` row with `origin = Jellyfin { item_id }`,
//!     a stable placeholder `file_hash` (blake3 of the item id) and the album-art
//!     downloaded to `cache/<cover_hash>_cover.jpg` so the rest of the UI keeps
//!     treating `album_art_path` as a local path.
//!  3. Songs are flushed to the library DB in batches and stale rows (item ids
//!     no longer present upstream) are pruned.
//!
//! Audio is materialised lazily by `ensure_local_audio`: the analyzer calls it
//! when it needs to read bytes, we download `GET /Items/{Id}/Download` once to
//! `cache/sources/<file_hash>.<container>`, and the row is rekeyed to the real
//! blake3 hash so the rest of the cache layout (`<hash>_instrumental.mp3` etc.)
//! stays consistent.

use std::collections::HashSet;
use std::io::Read;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::cache::CacheDir;
use crate::error::NightingaleError;
use crate::library_db;
use crate::song::{Song, SongOrigin};

use super::{MediaSource, ScanContext, SourceKind};

const PAGE_SIZE: usize = 200;
const SCAN_SAVE_BATCH_SIZE: usize = 25;
const COVER_FILL_WIDTH: u32 = 300;
const CLIENT_NAME: &str = "Nightingale";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct JellyfinAuth {
    pub base_url: String,
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

pub struct JellyfinSource {
    auth: JellyfinAuth,
}

impl JellyfinSource {
    pub fn new(auth: JellyfinAuth) -> Self {
        Self { auth }
    }

    pub fn auth(&self) -> &JellyfinAuth {
        &self.auth
    }

    fn agent(&self) -> ureq::Agent {
        ureq::Agent::new_with_defaults()
    }

    /// Header value used by all Jellyfin clients. `Token` is optional during
    /// authentication; we include it for every subsequent call.
    fn auth_header(&self) -> String {
        format!(
            "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            CLIENT_NAME, CLIENT_NAME, self.auth.device_id, CLIENT_VERSION, self.auth.access_token,
        )
    }

    fn fetch_page(
        &self,
        agent: &ureq::Agent,
        start_index: usize,
    ) -> Result<ItemQueryResult, NightingaleError> {
        let url = format!(
            "{base}/Users/{uid}/Items?Recursive=true&IncludeItemTypes={types}&Fields={fields}&SortBy=SortName&SortOrder=Ascending&Limit={limit}&StartIndex={start}",
            base = trim_base_url(&self.auth.base_url),
            uid = urlencoding::encode(&self.auth.user_id),
            types = "Audio,MusicVideo,Movie,Video,Episode",
            fields = "MediaSources,RunTimeTicks,Path,Container,ProductionYear,Genres",
            limit = PAGE_SIZE,
            start = start_index,
        );

        let resp = agent
            .get(&url)
            .header("X-Emby-Authorization", &self.auth_header())
            .header("X-Emby-Token", &self.auth.access_token)
            .header("Accept", "application/json")
            .call()
            .map_err(|e| NightingaleError::Other(format!("Jellyfin list failed: {e}")))?;

        let parsed: ItemQueryResult = resp
            .into_body()
            .read_json()
            .map_err(|e| NightingaleError::Other(format!("Jellyfin list parse error: {e}")))?;
        Ok(parsed)
    }

    fn build_song(
        &self,
        item: &JellyfinItem,
        cache: &CacheDir,
    ) -> Option<Song> {
        let item_id = item.id.clone();
        if item_id.is_empty() {
            return None;
        }

        let stable_id = format!("jellyfin:{}", item_id);
        let file_hash = blake3::hash(stable_id.as_bytes()).to_hex()[..32].to_string();

        let title = item
            .name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown".to_string());
        let artist = item
            .album_artist
            .clone()
            .or_else(|| item.artists.as_ref().and_then(|v| v.first().cloned()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album = item
            .album
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown Album".to_string());

        let duration_secs = item
            .run_time_ticks
            .map(|t| (t as f64) / 10_000_000.0)
            .unwrap_or(0.0);

        let media_type = item.media_type.as_deref().unwrap_or("");
        let item_type = item.item_type.as_deref().unwrap_or("");
        let is_video = matches!(
            (media_type, item_type),
            ("Video", _) | (_, "MusicVideo") | (_, "Movie") | (_, "Episode")
        );

        let container = item
            .container
            .clone()
            .or_else(|| {
                item.media_sources
                    .as_ref()
                    .and_then(|sources| sources.first())
                    .and_then(|s| s.container.clone())
            });

        let placeholder_path = placeholder_path(cache, &file_hash, container.as_deref());

        let album_art_path =
            self.download_cover(cache, &item_id).map(|p| p.to_path_buf());

        let song = Song {
            path: placeholder_path,
            file_hash,
            title,
            artist,
            album,
            duration_secs,
            album_art_path,
            is_analyzed: false,
            language: None,
            transcript_source: None,
            key: None,
            override_key: None,
            tempo: 1.0,
            key_offset: 0,
            is_video,
            usdx: None,
            origin: SongOrigin::Jellyfin {
                item_id,
                base_url: trim_base_url(&self.auth.base_url),
                container,
            },
        };

        Some(song)
    }

    fn download_cover(&self, cache: &CacheDir, item_id: &str) -> Option<PathBuf> {
        let agent = self.agent();
        let url = format!(
            "{base}/Items/{id}/Images/Primary?fillWidth={w}&fillHeight={w}",
            base = trim_base_url(&self.auth.base_url),
            id = urlencoding::encode(item_id),
            w = COVER_FILL_WIDTH,
        );
        let resp = agent
            .get(&url)
            .header("X-Emby-Authorization", &self.auth_header())
            .header("X-Emby-Token", &self.auth.access_token)
            .call()
            .ok()?;

        let mut body = resp.into_body();
        let mut reader = body.as_reader();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).ok()?;
        if bytes.is_empty() {
            return None;
        }

        let cover_hash = blake3::hash(&bytes).to_hex()[..32].to_string();
        let cover_path = cache.cover_path(&cover_hash);
        if !cover_path.exists() {
            std::fs::write(&cover_path, &bytes).ok()?;
        }
        Some(cover_path)
    }

    fn download_source(
        &self,
        song: &Song,
        cache: &CacheDir,
    ) -> Result<PathBuf, NightingaleError> {
        let SongOrigin::Jellyfin {
            item_id, container, ..
        } = &song.origin
        else {
            return Err(NightingaleError::Other(
                "download_source called on non-Jellyfin song".into(),
            ));
        };

        let dest = placeholder_path(cache, &song.file_hash, container.as_deref());
        if dest.is_file() {
            return Ok(dest);
        }
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let agent = self.agent();
        let url = format!(
            "{base}/Items/{id}/Download",
            base = trim_base_url(&self.auth.base_url),
            id = urlencoding::encode(item_id),
        );

        info!("[jellyfin] Downloading source for {item_id}");
        let resp = agent
            .get(&url)
            .header("X-Emby-Authorization", &self.auth_header())
            .header("X-Emby-Token", &self.auth.access_token)
            .call()
            .map_err(|e| NightingaleError::Other(format!("Jellyfin download failed: {e}")))?;

        let tmp = dest.with_extension("part");
        {
            let mut body = resp.into_body();
            let mut reader = body.as_reader();
            let mut file = std::fs::File::create(&tmp)?;
            std::io::copy(&mut reader, &mut file)?;
        }
        std::fs::rename(&tmp, &dest)?;

        info!("[jellyfin] Saved source to {}", dest.display());
        Ok(dest)
    }

    /// Construct a stream URL the renderer can hit via the media-server's
    /// `/remote/` proxy to play back video items without ever materialising the
    /// full file. Kept as a `pub` helper so previews/lyrics flows can wire it
    /// up without re-deriving the same template.
    #[allow(dead_code)]
    pub fn video_stream_url(&self, item_id: &str) -> String {
        format!(
            "{base}/Videos/{id}/stream?static=true&api_key={token}",
            base = trim_base_url(&self.auth.base_url),
            id = urlencoding::encode(item_id),
            token = urlencoding::encode(&self.auth.access_token),
        )
    }
}

impl MediaSource for JellyfinSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Jellyfin
    }

    fn label(&self) -> String {
        format!("Jellyfin: {}", trim_base_url(&self.auth.base_url))
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<(), NightingaleError> {
        let agent = self.agent();

        let mut all_items: Vec<JellyfinItem> = Vec::new();
        let mut start_index = 0usize;
        let total_record_count: usize = loop {
            if !library_db::scan_generation_is_current(ctx.generation) {
                return Ok(());
            }
            let page = self.fetch_page(&agent, start_index)?;
            let count = page.total_record_count.max(0) as usize;
            let received = page.items.len();
            all_items.extend(page.items);
            start_index += received;
            if received == 0 || start_index >= count {
                break count;
            }
        };

        info!(
            "[jellyfin] Listed {} items from server",
            all_items.len()
        );

        let folder_label = self.label();
        let _ = library_db::update_library_meta(&folder_label, total_record_count.max(all_items.len()));

        // Track Jellyfin rows by `origin.item_id` rather than `path`: after the
        // first analysis the path is rewritten to the local cache file, so a
        // path-based diff would treat already-analysed rows as stale and wipe
        // their `is_analyzed`/transcript metadata on the next scan.
        let current_item_ids: Vec<String> = all_items
            .iter()
            .filter(|i| !i.id.is_empty())
            .map(|i| i.id.clone())
            .collect();
        let _ = library_db::delete_jellyfin_songs_not_in_item_ids(&current_item_ids);

        let known: HashSet<String> = library_db::load_jellyfin_item_ids().unwrap_or_default();

        let mut batch: Vec<Song> = Vec::new();
        for (i, item) in all_items.iter().enumerate() {
            if !library_db::scan_generation_is_current(ctx.generation) {
                return Ok(());
            }
            if item.id.is_empty() {
                continue;
            }

            if known.contains(&item.id) {
                continue;
            }

            match self.build_song(item, ctx.cache) {
                Some(mut song) => {
                    song.path = PathBuf::from(jellyfin_placeholder_string(&item.id));
                    batch.push(song);
                }
                None => continue,
            }

            if (i + 1) % SCAN_SAVE_BATCH_SIZE == 0 && !batch.is_empty() {
                flush_batch(&mut batch, ctx.generation);
            }
        }
        if !batch.is_empty() {
            flush_batch(&mut batch, ctx.generation);
        }

        Ok(())
    }

    fn ensure_local_audio(
        &self,
        song: &Song,
        cache: &CacheDir,
    ) -> Result<PathBuf, NightingaleError> {
        // If `path` is already a real on-disk file (post-rekey) just hand it
        // back to the analyzer; otherwise download.
        if song.path.is_file() {
            return Ok(song.path.clone());
        }
        self.download_source(song, cache)
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn trim_base_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// String we shove into `Song.path` for unmaterialised Jellyfin rows. Picked so
/// it's unambiguously NOT a local file path on any platform.
fn jellyfin_placeholder_string(item_id: &str) -> String {
    format!("jellyfin://item/{item_id}")
}

fn placeholder_path(
    cache: &CacheDir,
    file_hash: &str,
    container: Option<&str>,
) -> PathBuf {
    let dir = cache.path.join("sources");
    let _ = std::fs::create_dir_all(&dir);
    let ext = container.unwrap_or("bin");
    dir.join(format!("{file_hash}.{ext}"))
}

fn flush_batch(batch: &mut Vec<Song>, generation: u64) {
    let _ = library_db::append_songs_for_scan(batch, generation);
    batch.clear();
}

// ─── Authentication ──────────────────────────────────────────────────

/// Public auth response surfaced to the UI after a successful login.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JellyfinLoginResult {
    pub server_url: String,
    pub server_name: Option<String>,
    pub user_id: String,
    pub username: String,
    pub access_token: String,
    pub device_id: String,
}

/// Authenticate against a Jellyfin server and return the credentials we'll
/// persist in `AppConfig`. Generates a stable per-install `device_id` if not
/// supplied so the same Nightingale install shows up consistently in the
/// server's "Devices" UI.
pub fn login(
    base_url: &str,
    username: &str,
    password: &str,
    device_id: Option<String>,
) -> Result<JellyfinLoginResult, NightingaleError> {
    let server_url = trim_base_url(base_url);
    let device_id = device_id.unwrap_or_else(generate_device_id);

    let agent = ureq::Agent::new_with_defaults();
    let auth_header = format!(
        "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
        CLIENT_NAME, CLIENT_NAME, device_id, CLIENT_VERSION,
    );

    #[derive(Serialize)]
    struct Body<'a> {
        #[serde(rename = "Username")]
        username: &'a str,
        #[serde(rename = "Pw")]
        pw: &'a str,
    }

    let payload = Body {
        username,
        pw: password,
    };

    let resp = agent
        .post(&format!("{server_url}/Users/AuthenticateByName"))
        .header("X-Emby-Authorization", &auth_header)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send_json(payload)
        .map_err(|e| NightingaleError::Other(format!("Jellyfin login failed: {e}")))?;

    let auth: AuthByNameResponse = resp
        .into_body()
        .read_json()
        .map_err(|e| NightingaleError::Other(format!("Jellyfin login parse: {e}")))?;

    let user_id = auth
        .user
        .as_ref()
        .map(|u| u.id.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| NightingaleError::Other("Jellyfin login: missing user.Id".into()))?;
    let resolved_username = auth
        .user
        .as_ref()
        .map(|u| u.name.clone())
        .unwrap_or_else(|| username.to_string());

    // Best-effort: pull the server name. Failures don't abort login.
    let server_name = fetch_server_name(&server_url, &device_id, &auth.access_token).ok();

    Ok(JellyfinLoginResult {
        server_url,
        server_name,
        user_id,
        username: resolved_username,
        access_token: auth.access_token,
        device_id,
    })
}

fn fetch_server_name(
    server_url: &str,
    device_id: &str,
    token: &str,
) -> Result<String, NightingaleError> {
    let info = fetch_public_info(server_url, Some((device_id, token)))?;
    
    info.server_name
        .ok_or_else(|| NightingaleError::Other("missing ServerName".into()))
}

#[derive(Debug, Clone, Deserialize)]
struct PublicInfo {
    #[serde(rename = "ServerName", default)]
    server_name: Option<String>,
    #[serde(rename = "Version", default)]
    version: Option<String>,
    #[serde(rename = "Id", default)]
    id: Option<String>,
}

fn fetch_public_info(
    server_url: &str,
    creds: Option<(&str, &str)>,
) -> Result<PublicInfo, NightingaleError> {
    let agent = ureq::Agent::new_with_defaults();
    let mut req = agent
        .get(&format!("{server_url}/System/Info/Public"))
        .header("Accept", "application/json");
    if let Some((device_id, token)) = creds {
        let header = format!(
            "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\", Token=\"{}\"",
            CLIENT_NAME, CLIENT_NAME, device_id, CLIENT_VERSION, token,
        );
        req = req
            .header("X-Emby-Authorization", &header)
            .header("X-Emby-Token", token);
    }
    let resp = req
        .call()
        .map_err(|e| NightingaleError::Other(format!("server info failed: {e}")))?;
    resp.into_body()
        .read_json::<PublicInfo>()
        .map_err(|e| NightingaleError::Other(format!("server info parse: {e}")))
}

/// Public ping payload surfaced to the UI. Renders the small "online / offline"
/// pill next to the Jellyfin source in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct JellyfinHealth {
    pub reachable: bool,
    #[ts(optional)]
    pub server_name: Option<String>,
    #[ts(optional)]
    pub version: Option<String>,
    #[ts(optional)]
    pub server_id: Option<String>,
    #[ts(optional)]
    pub error: Option<String>,
}

/// Hit `/System/Info/Public` once. Cheap enough for the UI to poll on a slow
/// interval (every ~15s) and serves as a smoke test for "is the server up
/// and the credentials still valid".
pub fn ping(auth: &JellyfinAuth) -> JellyfinHealth {
    match fetch_public_info(
        &trim_base_url(&auth.base_url),
        Some((auth.device_id.as_str(), auth.access_token.as_str())),
    ) {
        Ok(info) => JellyfinHealth {
            reachable: true,
            server_name: info.server_name,
            version: info.version,
            server_id: info.id,
            error: None,
        },
        Err(e) => JellyfinHealth {
            reachable: false,
            server_name: None,
            version: None,
            server_id: None,
            error: Some(e.to_string()),
        },
    }
}

fn generate_device_id() -> String {
    // Cheap UUID-ish identifier — Blake3 over current time + os random is
    // plenty unique for a "this is the same Nightingale install" tag, and
    // saves us pulling in `uuid` for this one call site.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = blake3::Hasher::new();
    hasher.update(&nanos.to_le_bytes());
    hasher.update(&rand::random::<u128>().to_le_bytes());
    let hex = hasher.finalize().to_hex();
    hex[..24].to_string()
}

// ─── Jellyfin DTOs ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ItemQueryResult {
    #[serde(rename = "Items", default)]
    items: Vec<JellyfinItem>,
    #[serde(rename = "TotalRecordCount", default)]
    total_record_count: i64,
}

#[derive(Debug, Deserialize)]
struct JellyfinItem {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: Option<String>,
    #[serde(rename = "Album", default)]
    album: Option<String>,
    #[serde(rename = "AlbumArtist", default)]
    album_artist: Option<String>,
    #[serde(rename = "Artists", default)]
    artists: Option<Vec<String>>,
    #[serde(rename = "RunTimeTicks", default)]
    run_time_ticks: Option<u64>,
    #[serde(rename = "Container", default)]
    container: Option<String>,
    #[serde(rename = "MediaType", default)]
    media_type: Option<String>,
    #[serde(rename = "Type", default)]
    item_type: Option<String>,
    #[serde(rename = "MediaSources", default)]
    media_sources: Option<Vec<JellyfinMediaSource>>,
}

#[derive(Debug, Deserialize)]
struct JellyfinMediaSource {
    #[serde(rename = "Container", default)]
    container: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthByNameResponse {
    #[serde(rename = "AccessToken")]
    access_token: String,
    #[serde(rename = "User", default)]
    user: Option<AuthUser>,
}

#[derive(Debug, Deserialize)]
struct AuthUser {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
}

