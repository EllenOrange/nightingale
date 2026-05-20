use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use rusqlite::{Connection, OptionalExtension, params};

use crate::cache::{analysis_queue_path, nightingale_dir, songs_path};
use crate::library_menu::{LibraryMenuItem, LibraryMenuItems};
use crate::library_model::{LibraryMenuFilters, LoadSongsParams, SongsMeta, SongsStore};
use crate::song::{Song, TranscriptSource};

const SCHEMA_VERSION: i32 = 1;

static LIBRARY_DB: OnceLock<Mutex<Connection>> = OnceLock::new();

static MIGRATING: AtomicBool = AtomicBool::new(false);
static MIGRATION_TOTAL: AtomicUsize = AtomicUsize::new(0);
static MIGRATION_DONE: AtomicUsize = AtomicUsize::new(0);

/// Incremented at the start of each `start_scan` so in-flight scan threads stop writing
/// after the library is cleared or replaced (folder change / new scan).
static SCAN_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn bump_scan_generation() -> u64 {
    SCAN_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

pub fn scan_generation_is_current(generation: u64) -> bool {
    SCAN_GENERATION.load(Ordering::SeqCst) == generation
}

pub fn library_db_path() -> PathBuf {
    nightingale_dir().join("songs.db")
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA cache_size = -64000;
        PRAGMA mmap_size = 268435456;
    ",
    )
}

fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let v: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v >= SCHEMA_VERSION {
        return Ok(());
    }
    if v == 0 {
        conn.execute_batch(
            "
            CREATE TABLE library_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                folder TEXT NOT NULL DEFAULT '',
                scan_count INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO library_meta (id, folder, scan_count) VALUES (1, '', 0);

            CREATE TABLE songs (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                file_hash TEXT NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                duration_secs REAL NOT NULL,
                album_art_path TEXT,
                is_analyzed INTEGER NOT NULL,
                language TEXT,
                transcript_source TEXT,
                is_video INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE INDEX idx_songs_file_hash ON songs(file_hash);
            CREATE INDEX idx_songs_artist_title ON songs(artist COLLATE NOCASE, title COLLATE NOCASE);
            CREATE INDEX idx_songs_album ON songs(album COLLATE NOCASE);

            CREATE VIRTUAL TABLE songs_fts USING fts5(
                title,
                artist,
                album,
                content = 'songs',
                content_rowid = 'id'
            );

            CREATE TABLE analysis_queue (
                file_hash TEXT PRIMARY KEY,
                status TEXT NOT NULL CHECK (status IN ('queued', 'analyzing', 'failed')),
                analyzing_pct INTEGER,
                failed_message TEXT
            );
        ",
        )?;
    }
    conn.execute(&format!("PRAGMA user_version = {SCHEMA_VERSION}"), [])?;
    Ok(())
}

fn open_connection_at(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    configure(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn open_connection() -> rusqlite::Result<Connection> {
    let path = library_db_path();
    open_connection_at(&path)
}

pub fn init_library() -> rusqlite::Result<()> {
    if LIBRARY_DB.get().is_some() {
        return Ok(());
    }
    let conn = open_connection()?;
    LIBRARY_DB.set(Mutex::new(conn)).map_err(|_| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
            Some("library db already initialized".into()),
        )
    })?;
    import_legacy_analysis_queue_json()?;
    maybe_start_songs_json_migration();
    Ok(())
}

pub fn reconnect_library_at_root(root: &Path) -> Result<(), String> {
    let db_path = root.join("songs.db");
    let conn =
        open_connection_at(&db_path).map_err(|e| format!("failed opening migrated songs db: {e}"))?;

    if let Some(existing) = LIBRARY_DB.get() {
        let mut guard = existing.lock().unwrap();
        *guard = conn;
        return Ok(());
    }

    LIBRARY_DB
        .set(Mutex::new(conn))
        .map_err(|_| "failed initializing library db connection".to_string())
}

fn with_conn<T, F: FnOnce(&Connection) -> rusqlite::Result<T>>(f: F) -> rusqlite::Result<T> {
    let g = LIBRARY_DB
        .get()
        .ok_or_else(|| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("init_library not called".into()),
            )
        })?
        .lock()
        .unwrap();
    f(&g)
}

fn with_conn_mut<T, F: FnOnce(&mut Connection) -> rusqlite::Result<T>>(
    f: F,
) -> rusqlite::Result<T> {
    let mut g = LIBRARY_DB
        .get()
        .ok_or_else(|| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                Some("init_library not called".into()),
            )
        })?
        .lock()
        .unwrap();
    f(&mut g)
}

fn parse_legacy_queue_status(v: &serde_json::Value) -> (&'static str, Option<i64>, Option<String>) {
    let Some(o) = v.as_object() else {
        return ("queued", None, None);
    };
    if o.contains_key("Queued") {
        return ("queued", None, None);
    }
    if let Some(n) = o.get("Analyzing").and_then(|x| x.as_u64()) {
        return ("analyzing", Some(n as i64), None);
    }
    if let Some(s) = o.get("Failed").and_then(|x| x.as_str()) {
        return ("failed", None, Some(s.to_string()));
    }
    ("queued", None, None)
}

fn import_legacy_analysis_queue_json() -> rusqlite::Result<()> {
    let path = analysis_queue_path();
    if !path.is_file() {
        return Ok(());
    }
    let data = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let v: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let Some(entries) = v.get("entries").and_then(|e| e.as_object()) else {
        return Ok(());
    };
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        for (hash, val) in entries {
            let (st, pct, msg) = parse_legacy_queue_status(val);
            tx.execute(
                "INSERT INTO analysis_queue (file_hash, status, analyzing_pct, failed_message)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(file_hash) DO UPDATE SET
                   status = excluded.status,
                   analyzing_pct = excluded.analyzing_pct,
                   failed_message = excluded.failed_message",
                params![hash, st, pct, msg],
            )?;
        }
        tx.commit()?;
        Ok(())
    })?;
    let _ = std::fs::rename(&path, path.with_extension("json.bak"));
    Ok(())
}

fn upsert_queue_in_tx(
    tx: &rusqlite::Transaction<'_>,
    file_hash: &str,
    status: &str,
    analyzing_pct: Option<i64>,
    failed_message: Option<&str>,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO analysis_queue (file_hash, status, analyzing_pct, failed_message)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(file_hash) DO UPDATE SET
           status = excluded.status,
           analyzing_pct = excluded.analyzing_pct,
           failed_message = excluded.failed_message",
        params![file_hash, status, analyzing_pct, failed_message],
    )?;
    Ok(())
}

pub fn analysis_queue_upsert_row(
    file_hash: &str,
    status: &str,
    analyzing_pct: Option<i64>,
    failed_message: Option<&str>,
) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        upsert_queue_in_tx(&tx, file_hash, status, analyzing_pct, failed_message)?;
        tx.commit()?;
        Ok(())
    })
}

pub fn analysis_queue_delete(file_hash: &str) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        c.execute(
            "DELETE FROM analysis_queue WHERE file_hash = ?",
            [file_hash],
        )?;
        Ok(())
    })
}

pub fn analysis_queue_clear() -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        c.execute("DELETE FROM analysis_queue", [])?;
        Ok(())
    })
}

pub fn analysis_queue_load_rows()
-> rusqlite::Result<Vec<(String, String, Option<i64>, Option<String>)>> {
    with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT file_hash, status, analyzing_pct, failed_message FROM analysis_queue",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.collect()
    })
}

pub fn analysis_queue_save_rows(
    rows: &[(String, String, Option<i64>, Option<String>)],
) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        tx.execute("DELETE FROM analysis_queue", [])?;
        for (hash, st, pct, msg) in rows {
            upsert_queue_in_tx(&tx, hash, st.as_str(), *pct, msg.as_deref())?;
        }
        tx.commit()?;
        Ok(())
    })
}

fn song_to_payload(song: &Song) -> rusqlite::Result<String> {
    serde_json::to_string(song).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        )))
    })
}

fn transcript_source_to_db(t: Option<TranscriptSource>) -> Option<String> {
    t.map(|s| match s {
        TranscriptSource::Lyrics => "lyrics".to_string(),
        TranscriptSource::Generated => "generated".to_string(),
        TranscriptSource::Usdx => "usdx".to_string(),
    })
}

const INSERT_SONG_SQL: &str = "\
INSERT INTO songs (path, file_hash, title, artist, album, duration_secs, album_art_path,
    is_analyzed, language, transcript_source, is_video, payload)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)";

fn insert_song_row_prepared(
    stmt: &mut rusqlite::Statement<'_>,
    song: &Song,
) -> rusqlite::Result<()> {
    let payload = song_to_payload(song)?;
    let album_art = song
        .album_art_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    stmt.execute(params![
        song.path.to_string_lossy(),
        song.file_hash,
        song.title,
        song.artist,
        song.album,
        song.duration_secs,
        album_art,
        song.is_analyzed as i32,
        song.language,
        transcript_source_to_db(song.transcript_source),
        song.is_video as i32,
        payload,
    ])?;
    Ok(())
}

pub fn read_library_meta() -> rusqlite::Result<(String, usize)> {
    with_conn(|c| {
        c.query_row(
            "SELECT folder, scan_count FROM library_meta WHERE id = 1",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)),
        )
    })
}

pub fn update_library_meta(folder: &str, scan_count: usize) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        c.execute(
            "UPDATE library_meta SET folder = ?1, scan_count = ?2 WHERE id = 1",
            params![folder, scan_count as i64],
        )?;
        Ok(())
    })
}

pub fn load_song_path_strings() -> rusqlite::Result<std::collections::HashSet<String>> {
    with_conn(|c| {
        let mut stmt = c.prepare("SELECT path FROM songs")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let v: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(v.into_iter().collect())
    })
}

pub fn append_songs(songs: &[Song]) -> rusqlite::Result<()> {
    if songs.is_empty() {
        return Ok(());
    }
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        {
            let mut stmt = tx.prepare(INSERT_SONG_SQL)?;
            for song in songs {
                insert_song_row_prepared(&mut stmt, song)?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn append_songs_for_scan(songs: &[Song], generation: u64) -> rusqlite::Result<()> {
    if songs.is_empty() || !scan_generation_is_current(generation) {
        return Ok(());
    }
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        {
            let mut stmt = tx.prepare(INSERT_SONG_SQL)?;
            for song in songs {
                if !scan_generation_is_current(generation) {
                    return Ok(());
                }
                insert_song_row_prepared(&mut stmt, song)?;
            }
        }
        if !scan_generation_is_current(generation) {
            return Ok(());
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn replace_all_songs_sorted(songs: &[Song]) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        tx.execute("DELETE FROM songs", [])?;
        {
            let mut stmt = tx.prepare(INSERT_SONG_SQL)?;
            for song in songs {
                insert_song_row_prepared(&mut stmt, song)?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn delete_songs_not_in_paths(paths: &[String]) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        if paths.is_empty() {
            c.execute("DELETE FROM songs", [])?;
            return Ok(());
        }
        let placeholders = (1..=paths.len()).map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM songs WHERE path NOT IN ({placeholders})");
        c.execute(
            &sql,
            rusqlite::params_from_iter(paths.iter().map(|s| s.as_str())),
        )?;
        Ok(())
    })
}

/// Item IDs of every Jellyfin-origin row currently in the DB. We track Jellyfin
/// songs by `origin.item_id` (extracted from the JSON payload) rather than by
/// `path` because `path` mutates after the first analysis (`rekey_song` rewrites
/// it to the local cache file) — keying on `item_id` keeps the row stable
/// across re-scans so analysed markers, language etc. survive.
pub fn load_jellyfin_item_ids() -> rusqlite::Result<std::collections::HashSet<String>> {
    with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT json_extract(payload, '$.origin.item_id')
             FROM songs
             WHERE json_extract(payload, '$.origin.kind') = 'jellyfin'",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, Option<String>>(0))?;
        let ids: Vec<Option<String>> = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(ids.into_iter().flatten().collect())
    })
}

/// `item_id -> ImageTags.Primary` for every Jellyfin row, used during scan to
/// decide whether a cover needs to be re-fetched. A `None` value means the
/// row exists but we never recorded a cover tag for it.
pub fn load_jellyfin_cover_tags()
-> rusqlite::Result<std::collections::HashMap<String, Option<String>>> {
    with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT json_extract(payload, '$.origin.item_id'),
                    json_extract(payload, '$.origin.cover_tag')
             FROM songs
             WHERE json_extract(payload, '$.origin.kind') = 'jellyfin'",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        let mut out = std::collections::HashMap::new();
        for row in rows {
            let (id, tag) = row?;
            if let Some(id) = id {
                out.insert(id, tag);
            }
        }
        Ok(out)
    })
}

/// Run `fetch` to obtain a fresh cover path for the given Jellyfin item id,
/// then update the row's `album_art_path` + payload accordingly. `fetch` is
/// invoked with the item id and may return `None` if the upstream cover
/// disappeared.
pub fn refresh_jellyfin_cover_for_item<F>(item_id: &str, fetch: F) -> rusqlite::Result<()>
where
    F: FnOnce(&str) -> Option<std::path::PathBuf>,
{
    let song_row = with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT file_hash, payload FROM songs
             WHERE json_extract(payload, '$.origin.kind') = 'jellyfin'
               AND json_extract(payload, '$.origin.item_id') = ?1
             LIMIT 1",
        )?;
        stmt.query_row([item_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .optional()
    })?;
    let Some((file_hash, payload)) = song_row else {
        return Ok(());
    };
    let mut song: Song = match serde_json::from_str(&payload) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let new_cover = fetch(item_id);
    if let crate::song::SongOrigin::Jellyfin { cover_tag, .. } = &mut song.origin
        && new_cover.is_none()
    {
        *cover_tag = None;
    }
    song.album_art_path = new_cover;
    update_song_fields(&file_hash, &song)
}

/// One-shot startup migration: pre-2026-05 builds stored an unmaterialised
/// Jellyfin row's `path` as `jellyfin://item/<id>`. The new code expects every
/// row to carry the future cache-file path so the `path.is_file()` check in
/// `ensure_local_media` works naturally. Rewrites legacy rows in-place.
pub fn rewrite_legacy_jellyfin_paths(cache_dir: &std::path::Path) -> rusqlite::Result<()> {
    let candidates = with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT file_hash, payload FROM songs
             WHERE json_extract(payload, '$.origin.kind') = 'jellyfin'
               AND path LIKE 'jellyfin://%'",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })?;

    if candidates.is_empty() {
        return Ok(());
    }

    let sources_dir = cache_dir.join("sources");

    with_conn_mut(|c| {
        let tx = c.transaction()?;
        {
            let mut stmt = tx.prepare(
                "UPDATE songs SET path = ?2, payload = ?3 WHERE file_hash = ?1",
            )?;
            for (file_hash, payload) in candidates {
                let Ok(mut song) = serde_json::from_str::<Song>(&payload) else {
                    continue;
                };
                let container = match &song.origin {
                    crate::song::SongOrigin::Jellyfin { container, .. } => container.clone(),
                    _ => None,
                };
                let ext = container.as_deref().unwrap_or("bin");
                let new_path = sources_dir.join(format!("{file_hash}.{ext}"));
                song.path = new_path.clone();
                let Ok(new_payload) = serde_json::to_string(&song) else {
                    continue;
                };
                stmt.execute(params![file_hash, new_path.to_string_lossy(), new_payload])?;
            }
        }
        tx.commit()
    })
}

/// Prune Jellyfin rows whose item id is no longer present upstream. Folder rows
/// are untouched.
pub fn delete_jellyfin_songs_not_in_item_ids(item_ids: &[String]) -> rusqlite::Result<()> {
    with_conn_mut(|c| {
        if item_ids.is_empty() {
            c.execute(
                "DELETE FROM songs WHERE json_extract(payload, '$.origin.kind') = 'jellyfin'",
                [],
            )?;
            return Ok(());
        }
        let placeholders = (1..=item_ids.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "DELETE FROM songs
             WHERE json_extract(payload, '$.origin.kind') = 'jellyfin'
               AND json_extract(payload, '$.origin.item_id') NOT IN ({placeholders})"
        );
        c.execute(
            &sql,
            rusqlite::params_from_iter(item_ids.iter().map(|s| s.as_str())),
        )?;
        Ok(())
    })
}

fn load_song_from_payload_column(r: &rusqlite::Row<'_>) -> rusqlite::Result<Song> {
    let payload: String = r.get(0)?;
    serde_json::from_str(&payload).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub fn load_song_by_hash(file_hash: &str) -> rusqlite::Result<Option<Song>> {
    with_conn(|c| {
        let mut stmt = c.prepare("SELECT payload FROM songs WHERE file_hash = ?1 LIMIT 1")?;
        let song = stmt
            .query_row([file_hash], |r| {
                let payload: String = r.get(0)?;
                serde_json::from_str::<Song>(&payload).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .optional()?;
        Ok(song)
    })
}

/// Rewrite a song row keyed by `old_hash` so its `file_hash`, `path`, and
/// JSON payload reflect a freshly downloaded source whose true Blake3 differs
/// from the placeholder we initially stored. Also points any pending row in
/// `analysis_queue` at the new hash so the in-flight scan keeps working.
pub fn rekey_song(old_hash: &str, new_hash: &str, new_song: &Song) -> rusqlite::Result<()> {
    let payload = song_to_payload(new_song)?;
    let album_art = new_song
        .album_art_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    with_conn_mut(|c| {
        let tx = c.transaction()?;
        tx.execute(
            "UPDATE songs SET file_hash = ?2, path = ?3, payload = ?4, album_art_path = ?5,
                title = ?6, artist = ?7, album = ?8, duration_secs = ?9,
                is_analyzed = ?10, language = ?11, transcript_source = ?12, is_video = ?13
             WHERE file_hash = ?1",
            params![
                old_hash,
                new_hash,
                new_song.path.to_string_lossy(),
                payload,
                album_art,
                new_song.title,
                new_song.artist,
                new_song.album,
                new_song.duration_secs,
                new_song.is_analyzed as i32,
                new_song.language,
                transcript_source_to_db(new_song.transcript_source),
                new_song.is_video as i32,
            ],
        )?;
        // `analysis_queue.file_hash` is the PK; UPDATE-OR-IGNORE shape covers
        // the (extremely unlikely) case where a row already exists for the
        // new hash.
        tx.execute(
            "DELETE FROM analysis_queue WHERE file_hash = ?1",
            params![new_hash],
        )?;
        tx.execute(
            "UPDATE analysis_queue SET file_hash = ?2 WHERE file_hash = ?1",
            params![old_hash, new_hash],
        )?;
        tx.commit()?;
        Ok(())
    })
}

pub fn update_song_fields(file_hash: &str, song: &Song) -> rusqlite::Result<()> {
    let payload = song_to_payload(song)?;
    let album_art = song
        .album_art_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    with_conn_mut(|c| {
        c.execute(
            "UPDATE songs SET title = ?2, artist = ?3, album = ?4, duration_secs = ?5,
                album_art_path = ?6, is_analyzed = ?7, language = ?8, transcript_source = ?9,
                is_video = ?10, payload = ?11
             WHERE file_hash = ?1",
            params![
                file_hash,
                song.title,
                song.artist,
                song.album,
                song.duration_secs,
                album_art,
                song.is_analyzed as i32,
                song.language,
                transcript_source_to_db(song.transcript_source),
                song.is_video as i32,
                payload,
            ],
        )?;
        Ok(())
    })
}

pub fn load_meta_sql() -> rusqlite::Result<SongsMeta> {
    if MIGRATING.load(Ordering::Acquire) {
        return with_conn(|c| {
            let (folder, _scan_count): (String, i64) = c.query_row(
                "SELECT folder, scan_count FROM library_meta WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let songs_count: i64 =
                c.query_row("SELECT COUNT(*) FROM songs WHERE is_video = 0", [], |r| {
                    r.get(0)
                })?;
            let videos_count: i64 =
                c.query_row("SELECT COUNT(*) FROM songs WHERE is_video != 0", [], |r| {
                    r.get(0)
                })?;
            let analyzed_count: i64 = c.query_row(
                "SELECT COUNT(*) FROM songs WHERE is_analyzed != 0",
                [],
                |r| r.get(0),
            )?;
            Ok(SongsMeta {
                count: MIGRATION_TOTAL.load(Ordering::Acquire),
                folder,
                processed_count: MIGRATION_DONE.load(Ordering::Acquire),
                songs_count: songs_count as usize,
                videos_count: videos_count as usize,
                analyzed_count: analyzed_count as usize,
            })
        });
    }

    with_conn(|c| {
        let (folder, scan_count): (String, i64) = c.query_row(
            "SELECT folder, scan_count FROM library_meta WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let processed: i64 = c.query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))?;
        let songs_count: i64 =
            c.query_row("SELECT COUNT(*) FROM songs WHERE is_video = 0", [], |r| {
                r.get(0)
            })?;
        let videos_count: i64 =
            c.query_row("SELECT COUNT(*) FROM songs WHERE is_video != 0", [], |r| {
                r.get(0)
            })?;
        let analyzed_count: i64 = c.query_row(
            "SELECT COUNT(*) FROM songs WHERE is_analyzed != 0",
            [],
            |r| r.get(0),
        )?;
        Ok(SongsMeta {
            count: scan_count as usize,
            folder,
            processed_count: processed as usize,
            songs_count: songs_count as usize,
            videos_count: videos_count as usize,
            analyzed_count: analyzed_count as usize,
        })
    })
}

fn escape_like_pattern(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            c => out.push(c),
        }
    }
    out
}

fn search_words_from_query(q: &str) -> Option<Vec<String>> {
    let t = q.trim();
    if t.is_empty() {
        return None;
    }
    let words: Vec<String> = t
        .split_whitespace()
        .map(escape_like_pattern)
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() { None } else { Some(words) }
}

fn songs_where_like_words(words: &[String]) -> (String, Vec<String>) {
    let mut flat = Vec::new();
    let mut parts = Vec::new();
    for w in words {
        parts.push(
            "(s.title LIKE ('%' || ? || '%') ESCAPE '\\' OR \
             s.artist LIKE ('%' || ? || '%') ESCAPE '\\' OR \
             s.album LIKE ('%' || ? || '%') ESCAPE '\\' OR \
             s.path LIKE ('%' || ? || '%') ESCAPE '\\')",
        );
        for _ in 0..4 {
            flat.push(w.clone());
        }
    }
    (parts.join(" AND "), flat)
}

fn append_structural_filters(
    filters: &LibraryMenuFilters,
    where_parts: &mut Vec<String>,
    bind_strings: &mut Vec<String>,
) {
    let artist = filters.artist.as_deref();
    let album = filters.album.as_deref();
    let query = filters.query.as_deref();

    if let Some(a) = artist.filter(|s| !s.is_empty()) {
        if a == "unknown_artist" {
            where_parts.push("s.artist = ?".to_string());
            bind_strings.push("Unknown Artist".to_string());
        } else {
            where_parts.push("s.artist = ?".to_string());
            bind_strings.push(a.to_string());
        }
    }
    if let Some(al) = album.filter(|s| !s.is_empty()) {
        if al == "unknown_album" {
            where_parts.push("s.album = ?".to_string());
            bind_strings.push("Unknown Album".to_string());
        } else if let Some((a, b)) = al.split_once('\u{001f}') {
            where_parts.push("s.artist = ? AND s.album = ?".to_string());
            bind_strings.push(a.to_string());
            bind_strings.push(b.to_string());
        } else {
            where_parts.push("s.album = ?".to_string());
            bind_strings.push(al.to_string());
        }
    }
    if let Some(q) = query.filter(|s| !s.is_empty()) {
        match q {
            "analysed" => where_parts.push("s.is_analyzed = 1".to_string()),
            "videos" => where_parts.push("s.is_video = 1".to_string()),
            "usdx" => where_parts.push("s.transcript_source = 'usdx'".to_string()),
            _ => {}
        }
    }
}

fn build_song_where_clause(
    search_words: Option<&[String]>,
    filters: &LibraryMenuFilters,
    extra_where_parts: &[&str],
) -> (Option<String>, Vec<String>) {
    let mut where_parts: Vec<String> = Vec::new();
    let mut bind_strings: Vec<String> = Vec::new();

    if let Some(words) = search_words {
        let (w, mut b) = songs_where_like_words(words);
        where_parts.push(format!("({w})"));
        bind_strings.append(&mut b);
    }

    append_structural_filters(filters, &mut where_parts, &mut bind_strings);
    where_parts.extend(extra_where_parts.iter().map(|part| (*part).to_string()));

    if where_parts.is_empty() {
        (None, bind_strings)
    } else {
        (Some(where_parts.join(" AND ")), bind_strings)
    }
}

pub fn load_songs_page(params: &LoadSongsParams) -> rusqlite::Result<SongsStore> {
    let (folder, scan_count) = with_conn(|c| {
        c.query_row(
            "SELECT folder, scan_count FROM library_meta WHERE id = 1",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
    })?;

    let search_words = params.search.as_deref().and_then(search_words_from_query);
    let (where_sql, bind_strings) =
        build_song_where_clause(search_words.as_deref(), &params.filters, &[]);

    let processed = if let Some(ref where_sql) = where_sql {
        let sql = format!(
            "SELECT payload FROM songs s
             WHERE {where_sql}
             ORDER BY s.artist COLLATE NOCASE, s.title COLLATE NOCASE
             LIMIT {} OFFSET {}",
            params.take as i64, params.skip as i64
        );
        with_conn(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(bind_strings.iter().map(|s| s.as_str())),
                load_song_from_payload_column,
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        })?
    } else {
        with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT payload FROM songs
                 ORDER BY artist COLLATE NOCASE, title COLLATE NOCASE
                 LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(
                params![params.take as i64, params.skip as i64],
                load_song_from_payload_column,
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        })?
    };

    let processed_count = if let Some(ref where_sql) = where_sql {
        let sql = format!("SELECT COUNT(*) FROM songs s WHERE {where_sql}");
        with_conn(|c| {
            let n: i64 = c.query_row(
                &sql,
                rusqlite::params_from_iter(bind_strings.iter().map(|s| s.as_str())),
                |r| r.get(0),
            )?;
            Ok(n as usize)
        })?
    } else {
        with_conn(|c| {
            let n: i64 = c.query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))?;
            Ok(n as usize)
        })?
    };

    Ok(SongsStore {
        count: scan_count as usize,
        folder,
        processed,
        processed_count,
    })
}

pub fn iter_file_hashes_filtered_not_analyzed(
    filters: &LibraryMenuFilters,
) -> rusqlite::Result<Vec<String>> {
    let (where_sql, bind_strings) = build_song_where_clause(None, filters, &["s.is_analyzed = 0"]);

    if let Some(where_sql) = where_sql {
        let sql = format!(
            "SELECT s.file_hash FROM songs s
             WHERE {where_sql}
             ORDER BY s.artist COLLATE NOCASE, s.title COLLATE NOCASE"
        );
        with_conn(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(bind_strings.iter().map(|s| s.as_str())),
                |r| r.get(0),
            )?;
            rows.collect()
        })
    } else {
        with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT file_hash FROM songs
                 WHERE is_analyzed = 0
                 ORDER BY artist COLLATE NOCASE, title COLLATE NOCASE",
            )?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })
    }
}

pub fn load_all_songs() -> rusqlite::Result<Vec<Song>> {
    with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT payload FROM songs ORDER BY artist COLLATE NOCASE, title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], load_song_from_payload_column)?;
        rows.collect()
    })
}

fn maybe_start_songs_json_migration() {
    let json_path = songs_path();
    if !json_path.is_file() {
        return;
    }
    let count: i64 =
        match with_conn(|c| c.query_row("SELECT COUNT(*) FROM songs", [], |r| r.get(0))) {
            Ok(n) => n,
            Err(_) => return,
        };
    if count > 0 {
        return;
    }

    let Ok(data) = std::fs::read_to_string(&json_path) else {
        return;
    };
    let Ok(store) = serde_json::from_str::<SongsStore>(&data) else {
        return;
    };
    let total = store.processed.len();
    if total == 0 {
        let _ = update_library_meta(&store.folder, store.count);
        let _ = std::fs::rename(&json_path, json_path.with_extension("json.bak"));
        return;
    }

    MIGRATING.store(true, Ordering::Release);
    MIGRATION_TOTAL.store(total, Ordering::Release);
    MIGRATION_DONE.store(0, Ordering::Release);

    let folder = store.folder.clone();
    let scan_count = store.count;
    let processed = store.processed;

    std::thread::spawn(move || {
        const BATCH: usize = 50;
        let _ = update_library_meta(&folder, scan_count);
        let success = migrate_song_batches(&processed, BATCH, |chunk| append_songs(chunk));
        MIGRATING.store(false, Ordering::Release);
        if success {
            let _ = std::fs::rename(&json_path, json_path.with_extension("json.bak"));
        }
    });
}

fn migrate_song_batches<F>(processed: &[Song], batch: usize, mut append_fn: F) -> bool
where
    F: FnMut(&[Song]) -> rusqlite::Result<()>,
{
    for chunk in processed.chunks(batch) {
        if append_fn(chunk).is_err() {
            return false;
        }
        MIGRATION_DONE.fetch_add(chunk.len(), Ordering::AcqRel);
    }
    true
}

pub fn query_library_menu_items() -> rusqlite::Result<LibraryMenuItems> {
    with_conn(|c| {
        let (total, analysed_total, video_total, video_analysed, usdx_total, usdx_analysed): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = c.query_row(
            "SELECT
                (SELECT COUNT(*) FROM songs),
                (SELECT COUNT(*) FROM songs WHERE is_analyzed = 1),
                (SELECT COUNT(*) FROM songs WHERE is_video = 1),
                (SELECT COUNT(*) FROM songs WHERE is_video = 1 AND is_analyzed = 1),
                (SELECT COUNT(*) FROM songs WHERE transcript_source = 'usdx'),
                (SELECT COUNT(*) FROM songs WHERE transcript_source = 'usdx' AND is_analyzed = 1)",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;

        let hot = vec![
            LibraryMenuItem {
                value: "all".into(),
                label: "All".into(),
                analysed_count: analysed_total as u64,
                count: total as u64,
            },
            LibraryMenuItem {
                value: "analysed".into(),
                label: "Analysed".into(),
                analysed_count: analysed_total as u64,
                count: analysed_total as u64,
            },
            LibraryMenuItem {
                value: "videos".into(),
                label: "Videos".into(),
                analysed_count: video_analysed as u64,
                count: video_total as u64,
            },
            LibraryMenuItem {
                value: "usdx".into(),
                label: "USDX".into(),
                analysed_count: usdx_analysed as u64,
                count: usdx_total as u64,
            },
        ];

        let (unknown_artist_cnt, unknown_artist_an): (i64, i64) = c.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN is_analyzed = 1 THEN 1 ELSE 0 END), 0)
             FROM songs WHERE artist = 'Unknown Artist'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        let (unknown_album_cnt, unknown_album_an): (i64, i64) = c.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN is_analyzed = 1 THEN 1 ELSE 0 END), 0)
             FROM songs WHERE album = 'Unknown Album'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        let no_metadata = vec![
            LibraryMenuItem {
                value: "unknown_artist".into(),
                label: "Unknown Artist".into(),
                analysed_count: unknown_artist_an as u64,
                count: unknown_artist_cnt as u64,
            },
            LibraryMenuItem {
                value: "unknown_album".into(),
                label: "Unknown Album".into(),
                analysed_count: unknown_album_an as u64,
                count: unknown_album_cnt as u64,
            },
        ];

        let mut stmt = c.prepare(
            "SELECT artist, COUNT(*) AS cnt,
                    COALESCE(SUM(CASE WHEN is_analyzed = 1 THEN 1 ELSE 0 END), 0) AS analysed
             FROM songs
             GROUP BY artist
             ORDER BY artist COLLATE NOCASE",
        )?;
        let artists: Vec<LibraryMenuItem> = stmt
            .query_map([], |r| {
                let artist: String = r.get(0)?;
                let cnt: i64 = r.get(1)?;
                let analysed: i64 = r.get(2)?;
                Ok(LibraryMenuItem {
                    value: artist.clone(),
                    label: artist,
                    analysed_count: analysed as u64,
                    count: cnt as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut stmt = c.prepare(
            "SELECT artist, album, COUNT(*) AS cnt,
                    COALESCE(SUM(CASE WHEN is_analyzed = 1 THEN 1 ELSE 0 END), 0) AS analysed
             FROM songs
             GROUP BY artist, album
             ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE",
        )?;
        let albums: Vec<LibraryMenuItem> = stmt
            .query_map([], |r| {
                let artist: String = r.get(0)?;
                let album: String = r.get(1)?;
                let cnt: i64 = r.get(2)?;
                let analysed: i64 = r.get(3)?;
                Ok(LibraryMenuItem {
                    value: format!("{artist}\x1f{album}"),
                    label: format!("{album} — {artist}"),
                    analysed_count: analysed as u64,
                    count: cnt as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(LibraryMenuItems {
            hot,
            no_metadata,
            artists,
            albums,
        })
    })
}

fn maybe_rebase_string_path(path: &str, old_root: &Path, new_root: &Path) -> Option<String> {
    let rel = Path::new(path).strip_prefix(old_root).ok()?;
    Some(new_root.join(rel).to_string_lossy().into_owned())
}

pub fn rebase_song_album_art_paths(old_root: &Path, new_root: &Path) -> Result<(), String> {
    let db_path = new_root.join("songs.db");
    if !db_path.is_file() {
        return Ok(());
    }

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("failed opening songs db {:?}: {e}", db_path))?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("failed opening songs db transaction: {e}"))?;
    let mut stmt = tx
        .prepare("SELECT id, album_art_path, payload FROM songs")
        .map_err(|e| format!("failed preparing songs query: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("failed querying songs: {e}"))?;

    let mut updates: Vec<(i64, Option<String>, String)> = Vec::new();
    for row in rows {
        let (id, album_art_path, payload) =
            row.map_err(|e| format!("failed reading songs row: {e}"))?;

        let mut changed = false;
        let mut new_album_art = album_art_path.clone();
        if let Some(current) = album_art_path.as_deref() {
            if let Some(rebased) = maybe_rebase_string_path(current, old_root, new_root) {
                new_album_art = Some(rebased);
                changed = true;
            }
        }

        let mut new_payload = payload.clone();
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&payload) {
            if let Some(album_art_value) = value.get_mut("album_art_path") {
                if let Some(current) = album_art_value.as_str() {
                    if let Some(rebased) = maybe_rebase_string_path(current, old_root, new_root) {
                        *album_art_value = serde_json::Value::String(rebased);
                        if let Ok(serialized) = serde_json::to_string(&value) {
                            new_payload = serialized;
                            changed = true;
                        }
                    }
                }
            }
        }

        if changed {
            updates.push((id, new_album_art, new_payload));
        }
    }
    drop(stmt);

    for (id, album_art_path, payload) in updates {
        tx.execute(
            "UPDATE songs SET album_art_path = ?2, payload = ?3 WHERE id = ?1",
            params![id, album_art_path, payload],
        )
        .map_err(|e| format!("failed updating songs row {id}: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("failed committing songs path rewrite: {e}"))?;
    Ok(())
}

