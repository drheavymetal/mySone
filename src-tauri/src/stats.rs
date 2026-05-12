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
    /// MusicBrainz work MBID resolved post-track-start when the play
    /// is classical (Phase 1). NULL for pop/rock plays. Lets the
    /// classical Hub aggregate plays per Work later (Phase 6).
    pub work_mbid: Option<&'a str>,
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

// ---------------------------------------------------------------------------
// Phase 6 — classical-aware aggregations
// ---------------------------------------------------------------------------

/// A single row of the "Top classical works" leaderboard. Ranks plays
/// grouped by `work_mbid` (only rows with a resolved parent Work survive
/// the filter). Display fields come from `MAX(...)` so any non-null
/// value across the merged rows wins.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopClassicalWork {
    pub work_mbid: String,
    pub plays: u64,
    pub listened_secs: u64,
    /// First non-null title from the underlying plays — used when the
    /// catalog's `Work.title` hasn't been hydrated client-side yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_album: Option<String>,
    /// How many distinct recordings of the work the user has touched.
    /// Lets the UI show "5 versions" badges on a Top Works card.
    pub distinct_recordings: u64,
}

/// A single row of the "Top classical composers" leaderboard. The
/// composer mbid comes from `artist_mbid` (the recording's artist). For
/// classical plays this is reliably the composer because the scrobbler
/// resolves MB artist credits, not free-text "Berlin Philharmonic".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopClassicalComposer {
    pub composer_mbid: String,
    pub plays: u64,
    pub listened_secs: u64,
    /// Distinct works played by this composer in the window.
    pub distinct_works: u64,
    /// First non-null artist name surfaced as display fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_name: Option<String>,
}

/// A single recently-played classical "session": a run of plays that
/// shares the same `work_mbid`. Lets the UI present "You played
/// Beethoven 5 (4 movements)" instead of four separate rows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentClassicalSession {
    pub work_mbid: String,
    /// Latest `started_at` across the session.
    pub last_started_at: i64,
    /// Earliest `started_at` (so the UI can show "started 14m ago").
    pub first_started_at: i64,
    pub plays: u64,
    pub listened_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_album: Option<String>,
    /// Distinct movements / tracks heard in the session — proxy for
    /// "did the user listen to the whole work?".
    pub distinct_recordings: u64,
}

/// One row of the "Recording comparison" view: same work, different
/// performances. The catalog hydrates these rows with conductor /
/// orchestra labels in a follow-up call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingComparisonRow {
    pub recording_mbid: String,
    pub plays: u64,
    pub listened_secs: u64,
    pub completed_count: u64,
    /// First non-null artist seen on the underlying rows (typically
    /// "Conductor · Orchestra"). The UI replaces it with a hydrated
    /// label when the Work catalog entry is warm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_album: Option<String>,
    pub last_started_at: i64,
}

/// Aggregate footprint of classical listening — surfaced in the Hub /
/// Stats overview as quick badges ("14 distinct composers · 248 plays").
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClassicalOverview {
    pub total_plays: u64,
    pub total_listened_secs: u64,
    pub distinct_works: u64,
    pub distinct_composers: u64,
    pub distinct_recordings: u64,
    pub since_unix: i64,
}

/// One row in the saved-favorites table — surfaced to the UI as cards
/// in the Library tab of the Hub.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassicalFavorite {
    pub id: i64,
    /// `'work' | 'recording' | 'composer' | 'performer'`
    pub kind: String,
    pub mbid: String,
    pub display_name: String,
    pub added_at: i64,
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
            // Phase 1 (Classical Hub): parent work MBID resolved from
            // recording_mbid via MB performance-rels. Always nullable —
            // non-classical plays leave it NULL.
            ("work_mbid", "ALTER TABLE plays ADD COLUMN work_mbid TEXT"),
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
             CREATE INDEX IF NOT EXISTS idx_plays_rg_mbid        ON plays(release_group_mbid);
             CREATE INDEX IF NOT EXISTS idx_plays_work_mbid      ON plays(work_mbid);",
        )?;

        // Phase 1: classical_favorites table. `kind` ∈ {'work',
        // 'recording', 'composer', 'performer'}. Aditive, never deleted.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS classical_favorites (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 kind TEXT NOT NULL,
                 mbid TEXT NOT NULL,
                 display_name TEXT NOT NULL,
                 added_at INTEGER NOT NULL,
                 UNIQUE(kind, mbid)
             );
             CREATE INDEX IF NOT EXISTS idx_classical_favorites_kind
                 ON classical_favorites(kind);",
        )?;
        // Phase 5 (D-021): classical_editorial table — user override of
        // the embedded Editor's Choice snapshot. `source` ∈
        // {'embedded', 'user'}. The catalog reads this table first and
        // falls back to the snapshot when no row exists.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS classical_editorial (
                 work_mbid TEXT PRIMARY KEY,
                 recording_mbid TEXT NOT NULL,
                 source TEXT NOT NULL,
                 note TEXT,
                 set_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_classical_editorial_source
                 ON classical_editorial(source);",
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
                recording_mbid, release_group_mbid, artist_mbid, work_mbid
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       ?13, ?14, ?15, ?16, ?17)",
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
                p.work_mbid,
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
                    recording_mbid, release_group_mbid, artist_mbid, work_mbid
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                           ?13, ?14, ?15, ?16, ?17)",
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
                    r.work_mbid,
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

    // -----------------------------------------------------------------
    // Phase 5 (D-021) — classical editorial overrides
    // -----------------------------------------------------------------

    /// Persist (or replace) a user-chosen Editor's Choice for a work.
    /// `source` is always `'user'` from this entry point — the embedded
    /// snapshot is read from the JSON file, never written back here.
    pub fn set_classical_editorial_choice(
        &self,
        work_mbid: &str,
        recording_mbid: &str,
        note: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = crate::now_secs() as i64;
        conn.execute(
            "INSERT INTO classical_editorial
                (work_mbid, recording_mbid, source, note, set_at)
             VALUES (?1, ?2, 'user', ?3, ?4)
             ON CONFLICT(work_mbid) DO UPDATE SET
                recording_mbid = excluded.recording_mbid,
                source = 'user',
                note = excluded.note,
                set_at = excluded.set_at",
            params![work_mbid, recording_mbid, note, now],
        )?;
        Ok(())
    }

    /// Look up a user override for a work. Returns `None` when no row
    /// exists — the caller falls back to the embedded snapshot.
    pub fn get_classical_editorial_choice(
        &self,
        work_mbid: &str,
    ) -> rusqlite::Result<Option<EditorialOverride>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT recording_mbid, source, note, set_at
             FROM classical_editorial
             WHERE work_mbid = ?1",
            params![work_mbid],
            |row| {
                Ok(EditorialOverride {
                    work_mbid: work_mbid.to_string(),
                    recording_mbid: row.get(0)?,
                    source: row.get(1)?,
                    note: row.get::<_, Option<String>>(2)?,
                    set_at: row.get(3)?,
                })
            },
        )
        .optional()
    }

    /// Clear a user override. Subsequent reads fall through to the
    /// embedded snapshot.
    pub fn clear_classical_editorial_choice(
        &self,
        work_mbid: &str,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM classical_editorial WHERE work_mbid = ?1",
            params![work_mbid],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Phase 6 — classical aggregations (read-only over `plays`).
    // All queries filter by `work_mbid IS NOT NULL` so non-classical
    // history never leaks into the Hub view.
    // -----------------------------------------------------------------

    /// Top-N works the user has played in the window. Aggregates by
    /// `work_mbid` so all movements of "Beethoven 9" collapse into one
    /// row. Returned ordered by play count desc, listened_secs as tiebreak.
    pub fn top_classical_works(
        &self,
        window: StatsWindow,
        limit: u32,
    ) -> rusqlite::Result<Vec<TopClassicalWork>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                work_mbid,
                COUNT(*) AS plays,
                SUM(listened_secs) AS secs,
                MAX(title), MAX(artist), MAX(album),
                COUNT(DISTINCT recording_mbid) AS distinct_recordings
             FROM plays
             WHERE started_at >= ?1
                   AND work_mbid IS NOT NULL
                   AND work_mbid <> ''
             GROUP BY work_mbid
             ORDER BY plays DESC, secs DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit], |row| {
                Ok(TopClassicalWork {
                    work_mbid: row.get(0)?,
                    plays: row.get::<_, i64>(1)? as u64,
                    listened_secs: row.get::<_, i64>(2)? as u64,
                    sample_title: row.get(3)?,
                    sample_artist: row.get(4)?,
                    sample_album: row.get(5)?,
                    distinct_recordings: row.get::<_, i64>(6)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Top-N composers the user has played in the window. Groups by
    /// `artist_mbid` (the play-time MB resolution of the artist credit;
    /// for classical recordings this is the composer when the scrobbler
    /// has resolved it). Plays without `artist_mbid` are skipped — the
    /// rest of the row would not be useful as a navigation target.
    pub fn top_classical_composers(
        &self,
        window: StatsWindow,
        limit: u32,
    ) -> rusqlite::Result<Vec<TopClassicalComposer>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                artist_mbid,
                COUNT(*) AS plays,
                SUM(listened_secs) AS secs,
                COUNT(DISTINCT work_mbid) AS distinct_works,
                MAX(album_artist), MAX(artist)
             FROM plays
             WHERE started_at >= ?1
                   AND work_mbid IS NOT NULL
                   AND work_mbid <> ''
                   AND artist_mbid IS NOT NULL
                   AND artist_mbid <> ''
             GROUP BY artist_mbid
             ORDER BY plays DESC, secs DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit], |row| {
                let album_artist: Option<String> = row.get(4)?;
                let artist: Option<String> = row.get(5)?;
                Ok(TopClassicalComposer {
                    composer_mbid: row.get(0)?,
                    plays: row.get::<_, i64>(1)? as u64,
                    listened_secs: row.get::<_, i64>(2)? as u64,
                    distinct_works: row.get::<_, i64>(3)? as u64,
                    sample_name: album_artist.or(artist),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Recently played classical sessions — runs of consecutive plays
    /// that share a `work_mbid`. We aggregate by `work_mbid` ordered by
    /// the most recent `started_at` so the UI can render "You were
    /// listening to Beethoven 5 · 14m ago".
    ///
    /// "Session" here is loose: simply the latest contiguous group per
    /// work. SQLite makes the gap-detection awkward without window
    /// functions in older builds, so we approximate by grouping all
    /// recent plays of the same work since `since_window_secs` ago.
    pub fn classical_recently_played_works(
        &self,
        since_window_secs: i64,
        limit: u32,
    ) -> rusqlite::Result<Vec<RecentClassicalSession>> {
        let now = crate::now_secs() as i64;
        let since = now - since_window_secs.max(0);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                work_mbid,
                MAX(started_at) AS last_at,
                MIN(started_at) AS first_at,
                COUNT(*) AS plays,
                SUM(listened_secs) AS secs,
                MAX(title), MAX(artist), MAX(album),
                COUNT(DISTINCT recording_mbid) AS distinct_recordings
             FROM plays
             WHERE started_at >= ?1
                   AND work_mbid IS NOT NULL
                   AND work_mbid <> ''
             GROUP BY work_mbid
             ORDER BY last_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since, limit], |row| {
                Ok(RecentClassicalSession {
                    work_mbid: row.get(0)?,
                    last_started_at: row.get(1)?,
                    first_started_at: row.get(2)?,
                    plays: row.get::<_, i64>(3)? as u64,
                    listened_secs: row.get::<_, i64>(4)? as u64,
                    sample_title: row.get(5)?,
                    sample_artist: row.get(6)?,
                    sample_album: row.get(7)?,
                    distinct_recordings: row.get::<_, i64>(8)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Recording comparison rows for a single work — every distinct
    /// recording the user has played, with their per-recording counts.
    /// Returned ordered by play count desc, then most recent.
    pub fn classical_recording_comparison(
        &self,
        work_mbid: &str,
    ) -> rusqlite::Result<Vec<RecordingComparisonRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT
                recording_mbid,
                COUNT(*) AS plays,
                SUM(listened_secs) AS secs,
                SUM(completed) AS completed_count,
                MAX(artist), MAX(album),
                MAX(started_at) AS last_at
             FROM plays
             WHERE work_mbid = ?1
                   AND recording_mbid IS NOT NULL
                   AND recording_mbid <> ''
             GROUP BY recording_mbid
             ORDER BY plays DESC, last_at DESC",
        )?;
        let rows = stmt
            .query_map(params![work_mbid], |row| {
                Ok(RecordingComparisonRow {
                    recording_mbid: row.get(0)?,
                    plays: row.get::<_, i64>(1)? as u64,
                    listened_secs: row.get::<_, i64>(2)? as u64,
                    completed_count: row.get::<_, i64>(3)? as u64,
                    sample_artist: row.get(4)?,
                    sample_album: row.get(5)?,
                    last_started_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Aggregate classical-only counters for the window. Mirrors
    /// `overview()` but filtered to `work_mbid IS NOT NULL`.
    pub fn classical_overview(
        &self,
        window: StatsWindow,
    ) -> rusqlite::Result<ClassicalOverview> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(listened_secs), 0),
                    COUNT(DISTINCT work_mbid),
                    COUNT(DISTINCT artist_mbid),
                    COUNT(DISTINCT recording_mbid)
                 FROM plays
                 WHERE started_at >= ?1
                       AND work_mbid IS NOT NULL
                       AND work_mbid <> ''",
                params![since],
                |row| {
                    Ok(ClassicalOverview {
                        total_plays: row.get::<_, i64>(0)? as u64,
                        total_listened_secs: row.get::<_, i64>(1)? as u64,
                        distinct_works: row.get::<_, i64>(2)? as u64,
                        distinct_composers: row.get::<_, i64>(3)? as u64,
                        distinct_recordings: row.get::<_, i64>(4)? as u64,
                        since_unix: since,
                    })
                },
            )
            .optional()?;
        Ok(row.unwrap_or_default())
    }

    /// Discovery curve filtered to classical plays only. Same shape as
    /// `discovery_curve` so the UI re-uses the existing chart, but
    /// counts only "first time hearing this composer / this recording"
    /// for plays whose `work_mbid` is set.
    pub fn classical_discovery_curve(
        &self,
        window: StatsWindow,
    ) -> rusqlite::Result<Vec<DiscoveryPoint>> {
        let now = crate::now_secs() as i64;
        let since = window.since(now);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "WITH classical AS (
                SELECT artist_mbid, recording_mbid, started_at
                FROM plays
                WHERE work_mbid IS NOT NULL AND work_mbid <> ''
             ),
             artist_first AS (
                SELECT
                    COALESCE(artist_mbid, '?') AS akey,
                    MIN(started_at) AS first_at
                FROM classical
                WHERE artist_mbid IS NOT NULL AND artist_mbid <> ''
                GROUP BY akey
             ),
             track_first AS (
                SELECT
                    COALESCE(recording_mbid, '?') AS tkey,
                    MIN(started_at) AS first_at
                FROM classical
                WHERE recording_mbid IS NOT NULL AND recording_mbid <> ''
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

    // -----------------------------------------------------------------
    // Phase 6 — favorites CRUD over `classical_favorites`
    // -----------------------------------------------------------------

    /// Persist a user-saved entity. `kind` ∈ {"work", "recording",
    /// "composer", "performer"}. Idempotent: a second add for the same
    /// `(kind, mbid)` is a no-op (the unique index swallows it).
    pub fn add_classical_favorite(
        &self,
        kind: &str,
        mbid: &str,
        display_name: &str,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = crate::now_secs() as i64;
        conn.execute(
            "INSERT OR IGNORE INTO classical_favorites
                (kind, mbid, display_name, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![kind, mbid, display_name, now],
        )?;
        Ok(())
    }

    /// Remove a saved favorite by `(kind, mbid)`.
    pub fn remove_classical_favorite(
        &self,
        kind: &str,
        mbid: &str,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM classical_favorites
             WHERE kind = ?1 AND mbid = ?2",
            params![kind, mbid],
        )?;
        Ok(())
    }

    /// Whether a `(kind, mbid)` is currently saved.
    pub fn is_classical_favorite(
        &self,
        kind: &str,
        mbid: &str,
    ) -> rusqlite::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM classical_favorites
                 WHERE kind = ?1 AND mbid = ?2",
                params![kind, mbid],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(n > 0)
    }

    /// List saved favorites of a given kind, newest first.
    pub fn list_classical_favorites(
        &self,
        kind: &str,
        limit: u32,
    ) -> rusqlite::Result<Vec<ClassicalFavorite>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, mbid, display_name, added_at
             FROM classical_favorites
             WHERE kind = ?1
             ORDER BY added_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![kind, limit], |row| {
                Ok(ClassicalFavorite {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    mbid: row.get(2)?,
                    display_name: row.get(3)?,
                    added_at: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Phase 6 — tests for classical aggregations
// ---------------------------------------------------------------------------

#[cfg(test)]
mod classical_tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a temp StatsDb in a fresh directory. The directory leaks
    /// after the test ends — acceptable for unit tests, the OS will
    /// reclaim it on next reboot.
    fn temp_db() -> StatsDb {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "sone-stats-test-{}-{}",
            std::process::id(),
            // nanos-since-epoch suffix for uniqueness across parallel tests
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        StatsDb::open(&dir).expect("open stats db")
    }

    /// Insert a play row with sensible defaults for tests. `t` is
    /// `started_at`; the play occupies a 60s window with all 60s
    /// listened.
    #[allow(clippy::too_many_arguments)]
    fn insert_play(
        db: &StatsDb,
        t: i64,
        title: &str,
        artist: &str,
        album: Option<&str>,
        recording_mbid: Option<&str>,
        artist_mbid: Option<&str>,
        work_mbid: Option<&str>,
    ) {
        let p = PlayRecord {
            started_at: t,
            finished_at: t + 60,
            track_id: None,
            title,
            artist,
            album,
            album_artist: None,
            duration_secs: 60,
            listened_secs: 60,
            completed: true,
            isrc: None,
            chosen_by_user: true,
            source: "local",
            recording_mbid,
            release_group_mbid: None,
            artist_mbid,
            work_mbid,
        };
        db.record_play(&p).expect("record play");
    }

    fn ago(secs: i64) -> i64 {
        crate::now_secs() as i64 - secs
    }

    // Suppress unused-variable warning when the dirs are intentionally not
    // cleaned up (tests run in parallel and may share temp roots).
    #[allow(dead_code)]
    fn _silence_unused_path() -> PathBuf {
        PathBuf::new()
    }

    #[test]
    fn top_classical_works_groups_by_work_mbid() {
        let db = temp_db();
        // 3 plays of work A across 2 recordings, 1 play of work B,
        // 1 play of non-classical (no work_mbid → must be ignored).
        insert_play(&db, ago(3600), "I. Allegro", "Karajan / BPO",
                    Some("Beethoven 9 - Karajan"), Some("rec-A1"),
                    Some("art-LvB"), Some("work-A"));
        insert_play(&db, ago(3500), "II. Molto", "Karajan / BPO",
                    Some("Beethoven 9 - Karajan"), Some("rec-A1"),
                    Some("art-LvB"), Some("work-A"));
        insert_play(&db, ago(3400), "I. Allegro", "Furtwangler / BPO",
                    Some("Beethoven 9 - Furtwangler"), Some("rec-A2"),
                    Some("art-LvB"), Some("work-A"));
        insert_play(&db, ago(3300), "Mass in B Minor", "Bach / Gardiner",
                    Some("Bach BMM"), Some("rec-B1"),
                    Some("art-JSB"), Some("work-B"));
        insert_play(&db, ago(3200), "Pop song", "Some Artist",
                    Some("Pop"), Some("pop-rec"), Some("pop-artist"),
                    None);

        let rows = db.top_classical_works(StatsWindow::All, 10).unwrap();
        // Only work-A and work-B appear. Pop is filtered.
        assert_eq!(rows.len(), 2);
        // work-A first (3 plays vs 1).
        assert_eq!(rows[0].work_mbid, "work-A");
        assert_eq!(rows[0].plays, 3);
        assert_eq!(rows[0].distinct_recordings, 2);
        assert_eq!(rows[1].work_mbid, "work-B");
        assert_eq!(rows[1].plays, 1);
    }

    #[test]
    fn top_classical_composers_groups_by_artist_mbid_and_skips_unknown() {
        let db = temp_db();
        insert_play(&db, ago(2000), "Sym 9 mvt I", "Karajan",
                    Some("B9"), Some("rec-1"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(1900), "Sym 9 mvt II", "Karajan",
                    Some("B9"), Some("rec-1"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(1800), "BMM Kyrie", "Gardiner",
                    Some("BMM"), Some("rec-2"), Some("art-JSB"),
                    Some("work-B"));
        // No artist_mbid → must NOT appear (no MBID to group by).
        insert_play(&db, ago(1700), "Sym X", "Anon",
                    Some("X"), Some("rec-3"), None, Some("work-C"));

        let rows = db.top_classical_composers(StatsWindow::All, 5).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].composer_mbid, "art-LvB");
        assert_eq!(rows[0].plays, 2);
        assert_eq!(rows[0].distinct_works, 1);
        assert_eq!(rows[1].composer_mbid, "art-JSB");
    }

    #[test]
    fn classical_recently_played_groups_by_work_and_orders_by_recency() {
        let db = temp_db();
        // Older session of work-X.
        insert_play(&db, ago(7200), "I", "Conductor",
                    Some("Album"), Some("rec-X"), Some("art-LvB"),
                    Some("work-X"));
        // Recent session of work-Y (3 movements, latest is most recent).
        insert_play(&db, ago(900), "I", "Conductor",
                    Some("Album-Y"), Some("rec-Y"), Some("art-LvB"),
                    Some("work-Y"));
        insert_play(&db, ago(600), "II", "Conductor",
                    Some("Album-Y"), Some("rec-Y"), Some("art-LvB"),
                    Some("work-Y"));
        insert_play(&db, ago(300), "III", "Conductor",
                    Some("Album-Y"), Some("rec-Y"), Some("art-LvB"),
                    Some("work-Y"));

        let rows = db
            .classical_recently_played_works(7 * 24 * 3600, 10)
            .unwrap();
        assert_eq!(rows.len(), 2);
        // work-Y first (most recent last_at).
        assert_eq!(rows[0].work_mbid, "work-Y");
        assert_eq!(rows[0].plays, 3);
        assert!(rows[0].last_started_at >= rows[0].first_started_at);
        assert_eq!(rows[1].work_mbid, "work-X");
    }

    #[test]
    fn classical_recording_comparison_buckets_per_recording() {
        let db = temp_db();
        // Same work-A, two recordings, different play counts.
        insert_play(&db, ago(800), "B9 I", "Karajan",
                    Some("DG"), Some("rec-K"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(700), "B9 II", "Karajan",
                    Some("DG"), Some("rec-K"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(600), "B9 IV", "Karajan",
                    Some("DG"), Some("rec-K"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(400), "B9 I", "Furtwangler",
                    Some("EMI"), Some("rec-F"), Some("art-LvB"),
                    Some("work-A"));

        let rows = db.classical_recording_comparison("work-A").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].recording_mbid, "rec-K");
        assert_eq!(rows[0].plays, 3);
        assert_eq!(rows[0].completed_count, 3);
        assert_eq!(rows[1].recording_mbid, "rec-F");
        assert_eq!(rows[1].plays, 1);
    }

    #[test]
    fn classical_overview_counts_only_classical() {
        let db = temp_db();
        insert_play(&db, ago(2500), "B9 I", "Karajan",
                    Some("DG"), Some("rec-K"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(2400), "BMM K", "Gardiner",
                    Some("Erato"), Some("rec-G"), Some("art-JSB"),
                    Some("work-B"));
        insert_play(&db, ago(2300), "Pop", "Pop A",
                    Some("Pop"), Some("pop-rec"), Some("pop-art"),
                    None);
        let ov = db.classical_overview(StatsWindow::All).unwrap();
        assert_eq!(ov.total_plays, 2);
        assert_eq!(ov.distinct_works, 2);
        assert_eq!(ov.distinct_composers, 2);
        assert_eq!(ov.distinct_recordings, 2);
    }

    #[test]
    fn favorites_round_trip_idempotent() {
        let db = temp_db();
        db.add_classical_favorite("work", "work-A", "Beethoven 9").unwrap();
        db.add_classical_favorite("work", "work-A", "Beethoven 9").unwrap(); // dup
        db.add_classical_favorite("composer", "art-LvB", "Beethoven").unwrap();

        assert!(db.is_classical_favorite("work", "work-A").unwrap());
        assert!(db.is_classical_favorite("composer", "art-LvB").unwrap());
        assert!(!db.is_classical_favorite("work", "work-Z").unwrap());

        let works = db.list_classical_favorites("work", 10).unwrap();
        assert_eq!(works.len(), 1, "duplicate add must be no-op");
        assert_eq!(works[0].mbid, "work-A");
        assert_eq!(works[0].display_name, "Beethoven 9");

        db.remove_classical_favorite("work", "work-A").unwrap();
        assert!(!db.is_classical_favorite("work", "work-A").unwrap());
    }

    #[test]
    fn classical_discovery_curve_filters_to_classical_only() {
        let db = temp_db();
        insert_play(&db, ago(50_000), "B9 I", "Karajan",
                    Some("DG"), Some("rec-K"), Some("art-LvB"),
                    Some("work-A"));
        insert_play(&db, ago(40_000), "BMM", "Gardiner",
                    Some("Erato"), Some("rec-G"), Some("art-JSB"),
                    Some("work-B"));
        insert_play(&db, ago(30_000), "Pop", "Pop A",
                    Some("Pop"), Some("pop-rec"), Some("pop-art"),
                    None);
        let curve = db.classical_discovery_curve(StatsWindow::All).unwrap();
        // Two distinct first-time classical artists across two days.
        // (Same day if the test runs fast — but we still expect <= 2
        // points and the cumulative new_artists must equal 2.)
        let total_new_artists: u64 = curve.iter().map(|p| p.new_artists).sum();
        assert_eq!(total_new_artists, 2);
        let total_new_tracks: u64 = curve.iter().map(|p| p.new_tracks).sum();
        assert_eq!(total_new_tracks, 2);
    }
}

/// Phase 5 — the user-set Editor's Choice for a work, persisted in the
/// `classical_editorial` table. Surfaced to the catalog so the UI can
/// honour the override when rendering recordings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorialOverride {
    pub work_mbid: String,
    pub recording_mbid: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub set_at: i64,
}
