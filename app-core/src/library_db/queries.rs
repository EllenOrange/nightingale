//! Read-side queries used by the menu / song-list / analyzer enqueue paths.
//!
//! `load_meta_sql` is also here because it returns the same `SongsMeta`
//! flavour both during a live `songs.json` → SQL migration and afterwards;
//! the two branches share the per-category COUNT() probes so they live in
//! one place.

use rusqlite::params;

use crate::library_menu::{LibraryMenuItem, LibraryMenuItems};
use crate::library_model::{LibraryMenuFilters, LoadSongsParams, SongsMeta, SongsStore};

use super::connection::with_conn;
use super::migrations::{is_song_migration_in_progress, song_migration_done, song_migration_total};
use super::songs::load_song_from_payload_column;

pub fn load_meta_sql() -> rusqlite::Result<SongsMeta> {
    if is_song_migration_in_progress() {
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
                count: song_migration_total(),
                folder,
                processed_count: song_migration_done(),
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
