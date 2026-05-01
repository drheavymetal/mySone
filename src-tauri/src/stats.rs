//! Local-only listening statistics — privacy-first "continuous Wrapped".
//!
//! Every play (including skips) is recorded to a SQLite database in the
//! user's config directory. Aggregations power a stats page in the UI:
//! top tracks/artists/albums, listening heatmap (day-of-week × hour),
//! daily minutes-listened, totals.
//!
//! No telemetry, no upload — the database lives at
//! `~/.config/sone/stats.db` and never leaves the machine. Kept in plain
//! SQLite (no SQLCipher) to avoid a heavy bundled dep; rely on the user's
//! filesystem perms (the rest of the config dir is similarly trusted).

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS plays (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at      INTEGER NOT NULL,
    finished_at     INTEGER NOT NULL,
    track_id        INTEGER,
    title           TEXT NOT NULL,
    artist          TEXT NOT NULL,
    album           TEXT,
    album_artist    TEXT,
    duration_secs   INTEGER NOT NULL,
    listened_secs   INTEGER NOT NULL,
    completed       INTEGER NOT NULL,
    isrc            TEXT,
    chosen_by_user  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_plays_started_at ON plays(started_at);
CREATE INDEX IF NOT EXISTS idx_plays_track_id   ON plays(track_id);
CREATE INDEX IF NOT EXISTS idx_plays_artist     ON plays(artist);
";

/// Plays under this many seconds are noise (immediate skip, accidental
/// click, queue-shuffle preview). Don't pollute stats with them.
const MIN_RECORDABLE_SECS: u32 = 5;

#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatsWindow {
    Day,
    Week,
    Month,
    Year,
    All,
}

impl StatsWindow {
    /// Lower bound (inclusive) for `started_at` queries, in unix seconds.
    /// `All` returns 0.
    fn since(self, now: i64) -> i64 {
        match self {
            Self::All => 0,
            Self::Day => now - 24 * 3600,
            Self::Week => now - 7 * 24 * 3600,
            Self::Month => now - 30 * 24 * 3600,
            Self::Year => now - 365 * 24 * 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRecord<'a> {
    pub started_at: i64,
    pub finished_at: i64,
    pub track_id: Option<u64>,
    pub title: &'a str,
    pub artist: &'a str,
    pub album: Option<&'a str>,
    pub album_artist: Option<&'a str>,
    pub duration_secs: u32,
    pub listened_secs: u32,
    pub completed: bool,
    pub isrc: Option<&'a str>,
    pub chosen_by_user: bool,
    /// Origin of the play. `"local"` for plays produced by SONE itself,
    /// `"listenbrainz"` for rows backfilled from a ListenBrainz import,
    /// `"lastfm"` if/when we wire that up. Stored so the UI can tell
    /// imported history from native plays.
    pub source: &'a str,
    /// MusicBrainz recording MBID resolved at play time, when available.
    /// Used by stats queries to dedupe "same recording, different
    /// release" splits caused by name-matching.
    pub recording_mbid: Option<&'a str>,
    /// MusicBrainz release-group MBID — the album as a concept, not a
    /// specific edition. Lets stats group "Original / Remaster /
    /// Anniversary" under the same album row.
    pub release_group_mbid: Option<&'a str>,
    /// MusicBrainz artist MBID. Lets stats collapse "The Beatles" and
    /// "Beatles" (or any other casing/punctuation drift) into one row.
    pub artist_mbid: Option<&'a str>,
}

/// Result of a bulk import: how many rows were inserted vs. skipped as
/// duplicates of existing local plays.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BulkImportResult {
    pub imported: u64,
    pub skipped: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    pub total_plays: u64,
    pub completed_plays: u64,
    pub total_listened_secs: u64,
    pub distinct_tracks: u64,
    pub distinct_artists: u64,
    pub distinct_albums: u64,
    pub since_unix: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopTrack {
    pub track_id: Option<u64>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub plays: u64,
    pub listened_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopArtist {
    pub artist: String,
    pub plays: u64,
    pub listened_secs: u64,
    pub distinct_tracks: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopAlbum {
    pub album: String,
    pub artist: String,
    pub plays: u64,
    pub listened_secs: u64,
}

/// One cell in the day-of-week × hour-of-day heatmap. `dow` follows
/// SQLite's `strftime('%w', …)` convention: 0 = Sunday, …, 6 = Saturday.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapCell {
    pub dow: u8,
    pub hour: u8,
    pub plays: u64,
    pub listened_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMinutes {
    /// `YYYY-MM-DD` in the local timezone (computed by SQLite via 'localtime').
    pub date: String,
    pub minutes: u64,
}

/// Listening minutes aggregated per hour-of-day across the entire window.
/// Powers the radial "hour clock" chart — answers "am I a morning or
/// late-night listener?" in one glance, distinct from the dow×hour
/// heatmap which shows the weekly pattern.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HourMinutes {
    pub hour: u8,
    pub minutes: u64,
}

/// One day in the discovery curve: how many *brand-new* artists the
/// user heard for the first time on that day (relative to the entire
/// local history, not just the window). Used by the frontend to draw a
/// cumulative "exploration rate" line.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryPoint {
    /// `YYYY-MM-DD` in local timezone.
    pub date: String,
    pub new_artists: u64,
    pub new_tracks: u64,
}

pub struct StatsDb {
    conn: Mutex<Connection>,
}

impl StatsDb {
    pub fn open(config_dir: &Path) -> rusqlite::Result<Self> {
        let path = config_dir.join("stats.db");
        let conn = Connection::open(&path)?;
        conn.execute_batch(SCHEMA)?;
        Self::migrate(&conn)?;
        // Pragmas that match a desktop-app workload: WAL for read concurrency,
        // synchronous=NORMAL is durable enough (we don't lose plays on crash
        // beyond the last few seconds, which are inherently lossy anyway).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        log::info!("[stats] db open at {}", path.display());
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Idempotent schema upgrades. Adds columns the original schema did
    /// not have, on installations that pre-date them.
    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        for (col, ddl) in [
            (
                "source",
                "ALTER TABLE plays ADD COLUMN source TEXT NOT NULL DEFAULT 'local'",
            ),
            (
                "recording_mbid",
                "ALTER TABLE plays ADD COLUMN recording_mbid TEXT",
            ),
            (
                "release_group_mbid",
                "ALTER TABLE plays ADD COLUMN release_group_mbid TEXT",
            ),
            ("artist_mbid", "ALTER TABLE plays ADD COLUMN artist_mbid TEXT"),
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM pragma_table_info('plays') WHERE name = ?1",
                    [col],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if !exists {
                conn.execute(ddl, [])?;
                log::info!("[stats] migrated: added {col} column");
            }
        }
        // Indexes that pay off once MBIDs land in the table.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_plays_recording_mbid ON plays(recording_mbid);
             CREATE INDEX IF NOT EXISTS idx_plays_artist_mbid    ON plays(artist_mbid);
             CREATE INDEX IF NOT EXISTS idx_plays_rg_mbid        ON plays(release_group_mbid);",
        )?;
        Ok(())
    }

    pub fn record_play(&self, p: &PlayRecord) -> rusqlite::Result<()> {
        if p.listened_secs < MIN_RECORDABLE_SECS {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO plays (
                started_at, finished_at, track_id, title, artist, album,
                album_artist, duration_secs, listened_secs, completed,
                isrc, chosen_by_user, source,
                recording_mbid, release_group_mbid, artist_mbid
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       ?13, ?14, ?15, ?16)",
            params![
                p.started_at,
                p.finished_at,
                p.track_id.map(|v| v as i64),
                p.title,
                p.artist,
                p.album,
                p.album_artist,
                p.duration_secs,
                p.listened_secs,
                p.completed as i32,
                p.isrc,
                p.chosen_by_user as i32,
                p.source,
                p.recording_mbid,
                p.release_group_mbid,
                p.artist_mbid,
            ],
        )?;
        Ok(())
    }

    /// Insert a batch of historical plays, skipping rows that already
    /// exist locally (matched by `(started_at, lower(title), lower(artist))`).
    /// Used by the ListenBrainz history importer to backfill stats with
    /// pre-SONE history without producing duplicates if the user re-runs
    /// the import or scrobbles the same track later.
    pub fn bulk_import_plays(&self, records: &[PlayRecord]) -> rusqlite::Result<BulkImportResult> {
        if records.is_empty() {
            return Ok(BulkImportResult::default());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let min_ts = records.iter().map(|r| r.started_at).min().unwrap();
        let max_ts = records.iter().map(|r| r.started_at).max().unwrap();

        // Pre-load existing keys in the timestamp range so dedup is one
        // query rather than one per row.
        let mut existing: HashSet<(i64, String, String)> = HashSet::new();
        {
            let mut stmt = tx.prepare(
                "SELECT started_at, lower(title), lower(artist)
                 FROM plays
                 WHERE started_at BETWEEN ?1 AND ?2",
            )?;
            let rows = stmt.query_map(params![min_ts, max_ts], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for r in rows.flatten() {
                existing.insert(r);
            }
        }

        let mut imported = 0u64;
        let mut skipped = 0u64;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO plays (
                    started_at, finished_at, track_id, title, artist, album,
                    album_artist, duration_secs, listened_secs, completed,
                    isrc, chosen_by_user, source,
                    recording_mbid, release_group_mbid, artist_mbid
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                           ?13, ?14, ?15, ?16)",
            )?;
            for r in records {
                let key = (
                    r.started_at,
                    r.title.to_lowercase(),
                    r.artist.to_lowercase(),
                );
                if existing.contains(&key) {
                    skipped += 1;
                    continue;
                }
                stmt.execute(params![
                    r.started_at,
                    r.finished_at,
                    r.track_id.map(|v| v as i64),
                    r.title,
                    r.artist,
                    r.album,
                    r.album_artist,
                    r.duration_secs,
                    r.listened_secs,
                    r.completed as i32,
                    r.isrc,
                    r.chosen_by_user as i32,
                    r.source,
                    r.recording_mbid,
                    r.release_group_mbid,
                    r.artist_mbid,
                ])?;
                existing.insert(key);
                imported += 1;
            }
        }
        tx.commit()?;
        Ok(BulkImportResult { imported, skipped })
    }

    /// Most recent `started_at` of any play in the DB, or `None` if empty.
    /// Used by the importer to pick a default `min_ts` for incremental runs.
    pub fn latest_started_at(&self) -> rusqlite::Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT MAX(started_at) FROM plays", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
    }

    pub fn overview(&self, window: StatsWindow) -> rusqlite::Result<StatsOverview> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(completed), 0),
                    COALESCE(SUM(listened_secs), 0),
                    COUNT(DISTINCT COALESCE(
                        recording_mbid,
                        lower(title) || '|' || lower(artist)
                    )),
                    COUNT(DISTINCT COALESCE(artist_mbid, lower(artist))),
                    COUNT(DISTINCT COALESCE(
                        release_group_mbid,
                        lower(COALESCE(album, '')) || '|' || lower(artist)
                    ))
                 FROM plays WHERE started_at >= ?1",
                params![since],
                |row| {
                    Ok(StatsOverview {
                        total_plays: row.get::<_, i64>(0)? as u64,
                        completed_plays: row.get::<_, i64>(1)? as u64,
                        total_listened_secs: row.get::<_, i64>(2)? as u64,
                        distinct_tracks: row.get::<_, i64>(3)? as u64,
                        distinct_artists: row.get::<_, i64>(4)? as u64,
                        distinct_albums: row.get::<_, i64>(5)? as u64,
                        since_unix: since,
                    })
                },
            )
            .optional()?;
        Ok(row.unwrap_or_default())
    }

    pub fn top_tracks(&self, window: StatsWindow, limit: u32) -> rusqlite::Result<Vec<TopTrack>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        // Group by recording_mbid when available, otherwise fall back to a
        // case-insensitive title|artist key (avoids splitting "The Beatles"
        // and "Beatles" into separate rows). Use MAX(...) for display-level
        // fields so any non-null value bubbles up across the merged rows.
        let mut stmt = conn.prepare(
            "SELECT
                MAX(track_id), MAX(title), MAX(artist), MAX(album),
                COUNT(*) AS plays, SUM(listened_secs)
             FROM plays
             WHERE started_at >= ?1 AND completed = 1
             GROUP BY COALESCE(
                recording_mbid,
                lower(title) || '|' || lower(artist)
             )
             ORDER BY plays DESC, listened_secs DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit], |row| {
                Ok(TopTrack {
                    track_id: row.get::<_, Option<i64>>(0)?.map(|v| v as u64),
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    plays: row.get::<_, i64>(4)? as u64,
                    listened_secs: row.get::<_, i64>(5)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn top_artists(&self, window: StatsWindow, limit: u32) -> rusqlite::Result<Vec<TopArtist>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        // Group by artist_mbid when present, fall back to lowercased name.
        // The display name comes from MAX(artist) so the canonical
        // capitalisation seen in any row wins.
        let mut stmt = conn.prepare(
            "SELECT MAX(artist),
                    COUNT(*) AS plays,
                    SUM(listened_secs),
                    COUNT(DISTINCT COALESCE(
                        recording_mbid,
                        lower(title) || '|' || lower(artist)
                    ))
             FROM plays
             WHERE started_at >= ?1 AND completed = 1
             GROUP BY COALESCE(artist_mbid, lower(artist))
             ORDER BY plays DESC, listened_secs DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit], |row| {
                Ok(TopArtist {
                    artist: row.get(0)?,
                    plays: row.get::<_, i64>(1)? as u64,
                    listened_secs: row.get::<_, i64>(2)? as u64,
                    distinct_tracks: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn top_albums(&self, window: StatsWindow, limit: u32) -> rusqlite::Result<Vec<TopAlbum>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        // Group by release_group_mbid when present so reissues / remasters
        // collapse onto the canonical album row.
        let mut stmt = conn.prepare(
            "SELECT MAX(album), MAX(artist),
                    COUNT(*) AS plays,
                    SUM(listened_secs)
             FROM plays
             WHERE started_at >= ?1 AND completed = 1
                   AND album IS NOT NULL AND album <> ''
             GROUP BY COALESCE(
                 release_group_mbid,
                 lower(album) || '|' || lower(artist)
             )
             ORDER BY plays DESC, listened_secs DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit], |row| {
                Ok(TopAlbum {
                    album: row.get(0)?,
                    artist: row.get(1)?,
                    plays: row.get::<_, i64>(2)? as u64,
                    listened_secs: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn heatmap(&self, window: StatsWindow) -> rusqlite::Result<Vec<HeatmapCell>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                CAST(strftime('%w', started_at, 'unixepoch', 'localtime') AS INTEGER) AS dow,
                CAST(strftime('%H', started_at, 'unixepoch', 'localtime') AS INTEGER) AS hour,
                COUNT(*),
                SUM(listened_secs)
             FROM plays
             WHERE started_at >= ?1
             GROUP BY dow, hour
             ORDER BY dow, hour",
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok(HeatmapCell {
                    dow: row.get::<_, i64>(0)? as u8,
                    hour: row.get::<_, i64>(1)? as u8,
                    plays: row.get::<_, i64>(2)? as u64,
                    listened_secs: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn daily_minutes(&self, window: StatsWindow) -> rusqlite::Result<Vec<DailyMinutes>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                date(started_at, 'unixepoch', 'localtime') AS day,
                SUM(listened_secs) / 60
             FROM plays
             WHERE started_at >= ?1
             GROUP BY day
             ORDER BY day",
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok(DailyMinutes {
                    date: row.get(0)?,
                    minutes: row.get::<_, i64>(1)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Listening minutes per hour-of-day across all days in the window.
    /// Returns up to 24 rows; hours with no listening are simply absent
    /// (the frontend fills in zeros so the radial chart has 24 slices).
    pub fn hour_minutes(&self, window: StatsWindow) -> rusqlite::Result<Vec<HourMinutes>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                CAST(strftime('%H', started_at, 'unixepoch', 'localtime') AS INTEGER) AS hour,
                SUM(listened_secs) / 60
             FROM plays
             WHERE started_at >= ?1
             GROUP BY hour
             ORDER BY hour",
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok(HourMinutes {
                    hour: row.get::<_, i64>(0)? as u8,
                    minutes: row.get::<_, i64>(1)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Per-day count of artists/tracks heard for the *first time ever*
    /// (across the entire local DB), restricted to days inside the
    /// window. Tracks the user's exploration vs. comfort-zone tendency.
    /// "New" = the very first appearance of that artist/recording in
    /// the user's history; later replays in the same window don't count.
    /// The frontend turns this into a cumulative line chart.
    pub fn discovery_curve(
        &self,
        window: StatsWindow,
    ) -> rusqlite::Result<Vec<DiscoveryPoint>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "WITH artist_first AS (
                SELECT
                    COALESCE(artist_mbid, lower(artist)) AS akey,
                    MIN(started_at) AS first_at
                FROM plays
                GROUP BY akey
             ),
             track_first AS (
                SELECT
                    COALESCE(recording_mbid, lower(title) || '|' || lower(artist)) AS tkey,
                    MIN(started_at) AS first_at
                FROM plays
                GROUP BY tkey
             ),
             new_artists AS (
                SELECT date(first_at, 'unixepoch', 'localtime') AS day, COUNT(*) AS n
                FROM artist_first
                WHERE first_at >= ?1
                GROUP BY day
             ),
             new_tracks AS (
                SELECT date(first_at, 'unixepoch', 'localtime') AS day, COUNT(*) AS n
                FROM track_first
                WHERE first_at >= ?1
                GROUP BY day
             ),
             days AS (
                SELECT day FROM new_artists
                UNION
                SELECT day FROM new_tracks
             )
             SELECT
                days.day,
                COALESCE(new_artists.n, 0),
                COALESCE(new_tracks.n, 0)
             FROM days
             LEFT JOIN new_artists ON new_artists.day = days.day
             LEFT JOIN new_tracks  ON new_tracks.day  = days.day
             ORDER BY days.day",
        )?;
        let rows = stmt
            .query_map(params![since], |row| {
                Ok(DiscoveryPoint {
                    date: row.get(0)?,
                    new_artists: row.get::<_, i64>(1)? as u64,
                    new_tracks: row.get::<_, i64>(2)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
