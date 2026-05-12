//! `CatalogService` — entry point for any classical lookup. Holds the
//! shared providers + cache, and is the only thing Tauri commands call
//! directly. Does NOT touch audio routing, the writer, or any volume
//! state — it is read-only catalog logic.
//!
//! Strategy:
//!   * `get_work` — fetch from cache; on miss, run providers in order
//!     (MB → Wikipedia), then run the cascade matcher to bind each
//!     recording to a Tidal track. Result is cached for 30 d.
//!   * `get_recording` — small-grain enrichment for hover/click, not used
//!     by the Phase 1 list view directly (the list relies on the
//!     batched data from `get_work`).
//!   * `get_composer` — composer entity for the (Phase 2) composer page.
//!
//! Cache keys follow the §3.3 conventions:
//!   * `classical:work:v1:{mbid}` → CacheTier::StaticMeta (TTL 7d, SWR 30d)
//!   * `classical:recording:v1:{mbid}` → CacheTier::StaticMeta
//!   * `classical:composer:v1:{mbid}` → CacheTier::StaticMeta
//!
//! NOTE: §3.3 of the master doc lists 30d/24h for work entries but the
//! existing `DiskCache` only exposes the four built-in tiers. We use
//! `StaticMeta` (7d/30d SWR) — comparable, and we leave per-key custom
//! TTLs for Phase 5 if needed.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json;

use super::editorial::{EditorialPick, EditorialProvider};
use super::matching::{self, MatchOutcome, INFERRED_THRESHOLD};
use super::providers::{
    musicbrainz::MusicBrainzProvider,
    composers_extended::ExtendedComposersProvider,
    openopus::{genre_for_oo_label, OpenOpusProvider},
    tidal::{build_canonical_query, TidalProvider, TrackQualityMeta},
    wikidata::WikidataProvider,
    wikipedia::WikipediaProvider,
    ClassicalProvider, MbRateLimiter,
};
use super::quality;
use super::search::{self, SearchHit, SearchPlan, SearchResults};
use super::buckets;
use super::types::{
    CatalogueNumber, Composer, ComposerSummary, Era, Genre, MatchConfidence, Recording,
    RelatedComposer, Work, WorkBucket, WorkSummary, WorkType,
};
use crate::cache::{CacheResult, CacheTier, DiskCache};
use crate::stats::{
    ClassicalFavorite, ClassicalOverview, RecentClassicalSession, RecordingComparisonRow, StatsDb,
    StatsWindow, TopClassicalComposer, TopClassicalWork,
};
use crate::SoneError;

const WORK_CACHE_PREFIX: &str = "classical:work:v1:";
const RECORDING_CACHE_PREFIX: &str = "classical:recording:v1:";
const COMPOSER_CACHE_PREFIX: &str = "classical:composer:v1:";
// Phase 7 (D-029) — bumped from v1→v2 to invalidate non-paginated v1
// cache entries. Phase 7 entries embed `:offset` in the key suffix.
// D-047 (Phase 8.9 / A4): bumped v2 → v3 because `ComposerWorksPage`
// gained `next_offset`, so cached v2 payloads deserialise to a
// payload missing the field. Existing v2 entries are simply ignored
// at lookup time and overwritten on first refetch.
const COMPOSER_WORKS_CACHE_PREFIX: &str = "classical:composer-works:v3:";
const TRACK_QUALITY_CACHE_PREFIX: &str = "classical:track-quality:v1:";
/// Phase 6 — Wikidata composer enrichment. Cached aggressively because
/// the data (portraits, genres, birth years) is stable on the order of
/// years.
const WIKIDATA_COMPOSER_CACHE_PREFIX: &str = "classical:wd-composer:v1:";
/// Phase 6 — Wikidata related-composers list per QID.
const WIKIDATA_RELATED_CACHE_PREFIX: &str = "classical:wd-related:v1:";
/// Phase 6 — Browse-by-conductor / orchestra discography per artist
/// MBID. MB browse for a popular conductor (Karajan ≥ 100 recordings)
/// completes in one rate-limited call; we cache for the StaticMeta tier.
const ARTIST_DISCOGRAPHY_CACHE_PREFIX: &str = "classical:artist-disco:v1:";
const WORK_CACHE_TAG: &str = "classical-work";
const RECORDING_CACHE_TAG: &str = "classical-recording";
const COMPOSER_CACHE_TAG: &str = "classical-composer";
const COMPOSER_WORKS_CACHE_TAG: &str = "classical-composer-works";
const TRACK_QUALITY_CACHE_TAG: &str = "classical-track-quality";
const WIKIDATA_COMPOSER_CACHE_TAG: &str = "classical-wd-composer";
const WIKIDATA_RELATED_CACHE_TAG: &str = "classical-wd-related";
const ARTIST_DISCOGRAPHY_CACHE_TAG: &str = "classical-artist-disco";

const WORK_RECORDINGS_LIMIT: usize = 60;
/// MB browse endpoint caps `limit` at 100 per page. Phase 2 uses a single
/// page; Phase 5 will paginate when the user hits "view all".
const COMPOSER_WORKS_LIMIT: usize = 100;
/// Phase 4: how many top recordings receive the per-track quality probe
/// during a `build_work_fresh` pass. The remainder ship with their tier
/// tags only — sample-rate stays None until a manual refresh.
const QUALITY_REFINE_TOP_N: usize = 20;
/// Maximum concurrent in-flight `playbackinfopostpaywall` calls during
/// the quality-refinement step. Tidal does not publish a hard limit;
/// the spike sustained 6 req/s without 429s.
const QUALITY_REFINE_PARALLELISM: usize = 6;

pub struct CatalogService {
    cache: Arc<DiskCache>,
    mb: Arc<MusicBrainzProvider>,
    wikipedia: Arc<WikipediaProvider>,
    tidal: Arc<TidalProvider>,
    openopus: Arc<OpenOpusProvider>,
    /// Phase 7 (D-027 / D-031 / D-033) — extended composers universe
    /// (~6k composers harvested from Wikidata classical-genre filter).
    /// Used for BrowseComposers full universe + search tokenizer index.
    /// OpenOpusProvider stays the canonical "popular" source.
    composers_extended: Arc<ExtendedComposersProvider>,
    /// Phase 5 (D-020) — curated editorial seeds + per-composer notes.
    editorial: Arc<EditorialProvider>,
    /// Phase 6 (D-022) — Wikidata SPARQL provider for composer
    /// enrichment + related composers. Best-effort: failures degrade
    /// gracefully to empty enrichment / empty related list.
    wikidata: Arc<WikidataProvider>,
    /// Phase 5 (D-021) — user override storage. Read-only from this
    /// service; writes happen via `set_user_editors_choice`.
    stats: Arc<StatsDb>,
    /// All providers, in the order they're chained for `enrich_work`.
    /// Kept around so future fields (genre via Wikidata) can be plugged
    /// in without changing the call sites.
    #[allow(dead_code)]
    provider_chain: Vec<Arc<dyn ClassicalProvider>>,
    #[allow(dead_code)]
    mb_rate: Arc<MbRateLimiter>,
}

impl CatalogService {
    /// Wired by `build_catalog_service` — passing 9 `Arc<...>` is the
    /// natural shape for a service that aggregates this many providers.
    /// We deliberately spell out each parameter (rather than packing
    /// into a struct) so the call site is grep-friendly and the
    /// compiler enforces order at every change.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache: Arc<DiskCache>,
        mb_rate: Arc<MbRateLimiter>,
        mb: Arc<MusicBrainzProvider>,
        wikipedia: Arc<WikipediaProvider>,
        tidal: Arc<TidalProvider>,
        openopus: Arc<OpenOpusProvider>,
        composers_extended: Arc<ExtendedComposersProvider>,
        editorial: Arc<EditorialProvider>,
        wikidata: Arc<WikidataProvider>,
        stats: Arc<StatsDb>,
    ) -> Self {
        let provider_chain: Vec<Arc<dyn ClassicalProvider>> = vec![
            mb.clone() as Arc<dyn ClassicalProvider>,
            openopus.clone() as Arc<dyn ClassicalProvider>,
            wikipedia.clone() as Arc<dyn ClassicalProvider>,
        ];
        Self {
            cache,
            mb,
            wikipedia,
            tidal,
            openopus,
            composers_extended,
            editorial,
            wikidata,
            stats,
            provider_chain,
            mb_rate,
        }
    }

    // -----------------------------------------------------------------
    // Work
    // -----------------------------------------------------------------

    pub async fn get_work(&self, mbid: &str) -> Result<Work, SoneError> {
        if mbid.is_empty() {
            return Err(SoneError::Parse("empty work mbid".into()));
        }
        let key = format!("{WORK_CACHE_PREFIX}{mbid}");

        // Cache lookup — Fresh / Stale / Miss.
        match self.cache.get(&key, CacheTier::StaticMeta).await {
            CacheResult::Fresh(bytes) => {
                if let Ok(cached) = serde_json::from_slice::<Work>(&bytes) {
                    log::debug!("[catalog] work {mbid} cache HIT");
                    return Ok(cached);
                }
                log::warn!("[catalog] work {mbid} cache decode failed");
            }
            CacheResult::Stale(bytes) => {
                if let Ok(cached) = serde_json::from_slice::<Work>(&bytes) {
                    log::debug!("[catalog] work {mbid} cache STALE (returning, refresh later)");
                    // TODO Phase 4: spawn background refresh.
                    return Ok(cached);
                }
            }
            CacheResult::Miss => {}
        }

        // D-038 (bug 4) — transient errors must NOT be cached. Without
        // this guard, an MB connectivity blip would write
        // `tidal_unavailable=true` into the StaticMeta tier (TTL 7d),
        // poisoning the user's cache for a week even after MB recovers.
        let fresh = match self.build_work_fresh(mbid).await {
            Ok(w) => w,
            Err(e) if e.is_transient() => {
                log::warn!(
                    "[catalog] work {mbid} transient failure; not caching: {e}"
                );
                return Err(e);
            }
            Err(e) => {
                return Err(e);
            }
        };
        let bytes = serde_json::to_vec(&fresh).unwrap_or_default();
        if let Err(e) = self
            .cache
            .put(&key, &bytes, CacheTier::StaticMeta, &[WORK_CACHE_TAG])
            .await
        {
            log::warn!("[catalog] cache put for {mbid} failed: {e}");
        }
        Ok(fresh)
    }

    async fn build_work_fresh(&self, mbid: &str) -> Result<Work, SoneError> {
        // 1. Pull the MB skeleton.
        let mut work = self.mb.fetch_work(mbid).await?;

        // 1.b. Phase 9 (B9.1 / D-040) — compute the canonical
        //      `WorkBucket` from the data tier. Editorial override is
        //      consulted first; if absent, the heuristic in
        //      `classical::buckets::bucket_for(...)` runs over
        //      (work_type, genre, P136≈[], title). P136 keywords are
        //      not yet plumbed end-to-end (Wikidata enrichment for
        //      Work-level claims is Phase 10+); the editorial
        //      snapshot covers the canon explicitly meanwhile.
        let composer_for_bucket = work.composer_mbid.clone().unwrap_or_default();
        let editorial_bucket = self
            .editorial
            .lookup_bucket(&composer_for_bucket, &work.title);
        work.bucket = Some(editorial_bucket.unwrap_or_else(|| {
            buckets::bucket_for(work.work_type, work.genre, &[], &work.title)
        }));

        // 2. Wikipedia description (best-effort).
        if let Err(e) = self.wikipedia.enrich_work(&mut work).await {
            log::debug!("[catalog] wiki work {mbid}: {e}");
        }

        // 3. Recordings via single MB browse. Cap at WORK_RECORDINGS_LIMIT.
        //    D-038 (bug 4): we propagate transient failures so the caller
        //    (`get_work`) can skip the negative-cache path. Permanent
        //    errors (404 / parse) are swallowed to a `Vec::new()` —
        //    those represent a work MB genuinely lacks recordings for,
        //    and the negative cache flag (`tidal_unavailable=true`) is
        //    the correct signal for them.
        let recordings = match self
            .mb
            .fetch_recordings_for_work(mbid, WORK_RECORDINGS_LIMIT)
            .await
        {
            Ok(r) => r,
            Err(e) if e.is_transient() => {
                log::warn!(
                    "[catalog] recordings for {mbid} transient: {e} — propagating"
                );
                return Err(e);
            }
            Err(e) => {
                log::warn!("[catalog] recordings for {mbid} permanent: {e}");
                Vec::new()
            }
        };
        work.recording_count = recordings.len() as u32;

        // 4. Cascade matcher: for each recording, ISRC first, text-search
        //    second. We run these sequentially to respect Tidal request
        //    pacing — Tidal does not publish a hard rate limit, but
        //    ~6-8 req/s is what the spike sustained without 429s.
        work.recordings = self
            .resolve_recordings(
                &recordings,
                work.composer_name.as_deref(),
                &work.title,
                work.catalogue_number.as_ref(),
                work.bucket,
            )
            .await;

        // 5. Phase 4: quality refinement for the top-N playable
        //    recordings. Populates `sample_rate_hz` / `bit_depth` /
        //    `quality_score` on each refined row + the work-level
        //    `best_available_quality` summary.
        self.refine_work_quality(&mut work).await;

        // 6. Phase 5 (D-020 + D-021): editorial enrichment. Marks the
        //    Editor's Choice row + adds a work-level note. The user
        //    override (DB) wins over the embedded snapshot.
        self.apply_editorial(&mut work);

        // 7. Phase 8 / D-037 (bug 3) — work-level Tidal text-search
        //    fallback. When MB had zero recordings for the work, OR
        //    every recording came back NotFound from the cascade, try
        //    one more pass: a composer + work-title query against
        //    Tidal directly, no artist constraint. If the top hit
        //    crosses `WORK_LEVEL_THRESHOLD` (0.55), synthesize a single
        //    Recording marked `TidalDirectInferred` so the user has at
        //    least one playable row instead of an empty WorkPage with a
        //    false-negative "Tidal unavailable" banner.
        let cascade_failed = work.recordings.is_empty()
            || work
                .recordings
                .iter()
                .all(|r| r.tidal_track_id.is_none());
        if cascade_failed {
            self.try_work_level_fallback(&mut work).await;
        }

        // 8. Phase 7 (D-030) — Tidal availability flag. Negative cache
        //    when the cascade produced zero recordings AND zero playable
        //    matches. The flag travels with the cached Work and is
        //    surfaced by the WorkPage banner.
        work.tidal_unavailable = work.recordings.is_empty()
            || work
                .recordings
                .iter()
                .all(|r| r.tidal_track_id.is_none());

        Ok(work)
    }

    /// D-037 / D-041 (Phase 8.9) — work-level Tidal text-search
    /// fallback. Mutates `work.recordings` by appending up to
    /// `matching::MAX_WORK_LEVEL_SYNTH` synthetic `Recording`s, one
    /// per Tidal candidate that crosses `WORK_LEVEL_THRESHOLD` (0.62).
    /// No-op when nothing crosses.
    ///
    /// D-041 changes vs. the original D-037 implementation:
    ///   * `build_canonical_query` now receives the work's catalogue
    ///     number ("Op. 83", "BWV 244", "K. 466") so Tidal FTS
    ///     anchors on the discriminative token.
    ///   * `best_work_level_candidates_multiple` returns a sorted-desc
    ///     list (cap N=12), each row scored against the genre-bucket
    ///     penalty derived from the work's `WorkType`. This kills the
    ///     "Beethoven Op. 83 → Eroica I. Allegro" false-positive
    ///     while letting many legitimate lieder candidates through.
    ///
    /// Each synthetic recording uses a stable MBID
    /// `synthetic:tidal:{work_mbid}:{idx}` (idx ∈ 0..N) so cache keys
    /// and front-end de-duplication remain deterministic across
    /// re-fetches.
    async fn try_work_level_fallback(&self, work: &mut Work) {
        let query = build_canonical_query(
            work.composer_name.as_deref(),
            &work.title,
            work.catalogue_number.as_ref(),
            None,
            None,
        );
        if query.trim().is_empty() {
            return;
        }
        log::debug!(
            "[catalog] work-level fallback for {}: query='{}'",
            work.mbid,
            query
        );
        let results = match self.tidal.search_canonical(&query, 8).await {
            Ok(r) => r,
            Err(e) => {
                log::debug!(
                    "[catalog] work-level fallback search failed for {}: {e}",
                    work.mbid
                );
                return;
            }
        };
        if results.tracks.is_empty() {
            return;
        }
        let outcomes = matching::best_work_level_candidates_multiple(
            &results.tracks,
            &work.title,
            work.bucket,
            query,
        );
        if outcomes.is_empty() {
            log::debug!(
                "[catalog] work-level fallback for {} produced no synth (all candidates below threshold)",
                work.mbid
            );
            return;
        }
        log::info!(
            "[catalog] work-level fallback for {} synthesising {} recordings",
            work.mbid,
            outcomes.len()
        );
        for (idx, outcome) in outcomes.into_iter().enumerate() {
            // Stable MBID prefix `synthetic:tidal:{work}:{idx}` so
            // cache keys / dedup logic don't collide with real MB
            // recordings, and re-fetches keep deterministic ids.
            let synthetic_mbid = format!("synthetic:tidal:{}:{}", work.mbid, idx);
            let mut synth = Recording::shell(&synthetic_mbid, &work.mbid);
            synth.title = Some(work.title.clone());
            synth.tidal_track_id = outcome.track_id;
            synth.tidal_album_id = outcome.album_id;
            synth.audio_quality_tags = outcome.quality_tags;
            synth.audio_modes = outcome.audio_modes;
            synth.duration_secs = outcome.duration_secs;
            synth.cover_url = outcome.cover_url;
            synth.match_confidence = MatchConfidence::TidalDirectInferred;
            synth.match_query = outcome.query_used;
            synth.match_score = outcome.score;
            log::debug!(
                "[catalog] work-level synth #{} for {}: track_id={:?} score={:?}",
                idx,
                work.mbid,
                synth.tidal_track_id,
                synth.match_score
            );
            work.recordings.push(synth);
        }
        work.recording_count = work.recordings.len() as u32;
    }

    /// Phase 5 — fold the editorial seeds + user overrides into a fresh
    /// `Work`. Pure (mutates the passed work, no I/O after the DB read).
    /// Order of resolution:
    ///   1. User override (DB) — pins a specific recording_mbid.
    ///   2. Embedded snapshot (curated, defends choice with conductor +
    ///      year + label heuristics).
    ///   3. None.
    fn apply_editorial(&self, work: &mut Work) {
        // a) Apply work-level editor_note from the snapshot, if any.
        let composer_mbid = work.composer_mbid.clone().unwrap_or_default();
        let snapshot_entry = if !composer_mbid.is_empty() {
            self.editorial.lookup_work(&composer_mbid, &work.title)
        } else {
            None
        };
        if let Some(entry) = snapshot_entry.as_ref() {
            if work.editor_note.is_none() {
                work.editor_note = entry.editor_note.clone();
            }
        }

        // b) Resolve which recording is Editor's Choice.
        //    - DB override → exact recording_mbid.
        //    - Snapshot heuristic → match the first recording whose
        //      conductor / artist_credits string contains the seed
        //      conductor or performer (case-insensitive).
        let user_override = self
            .stats
            .get_classical_editorial_choice(&work.mbid)
            .unwrap_or(None);

        if let Some(over) = user_override.as_ref() {
            // Direct mbid match — surfaces the user's pick exactly.
            for rec in work.recordings.iter_mut() {
                if rec.mbid == over.recording_mbid {
                    rec.is_editors_choice = true;
                    rec.editor_note = over
                        .note
                        .clone()
                        .or_else(|| Some("User-marked Editor's Choice".to_string()));
                }
            }
            // If a snapshot pick existed, suppress it: user wins.
            return;
        }

        if let Some(entry) = snapshot_entry {
            if let Some(choice) = entry.editors_choice.as_ref() {
                let target_conductor = choice.conductor.as_deref().map(str::to_lowercase);
                let target_performer = choice.performer.to_lowercase();
                let target_year = choice.year;
                let mut hit_idx: Option<usize> = None;
                for (idx, rec) in work.recordings.iter().enumerate() {
                    if recording_matches_seed(
                        rec,
                        target_conductor.as_deref(),
                        &target_performer,
                        target_year,
                    ) {
                        hit_idx = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = hit_idx {
                    let rec = &mut work.recordings[idx];
                    rec.is_editors_choice = true;
                    rec.editor_note = choice.note.clone();
                }
            }
        }
    }

    /// Phase 4 (B4.1 + B4.2): batch-fetch quality metadata for the top
    /// playable recordings, populate per-row + aggregate fields.
    ///
    /// Cap top-N (currently 20) so a Beethoven 9 work with 60 matches
    /// doesn't fan out 60 manifest probes. The remainder still get their
    /// tier-only `quality_score` (no sample-rate refinement).
    async fn refine_work_quality(&self, work: &mut Work) {
        // Collect the track ids to probe in input order, capped.
        let probe_indices: Vec<usize> = work
            .recordings
            .iter()
            .enumerate()
            .filter(|(_, r)| r.tidal_track_id.is_some())
            .map(|(i, _)| i)
            .take(QUALITY_REFINE_TOP_N)
            .collect();

        // Fan out probes with bounded parallelism.
        let probes: Vec<(usize, u64)> = probe_indices
            .iter()
            .map(|i| (*i, work.recordings[*i].tidal_track_id.unwrap_or_default()))
            .collect();

        let metas = self.fetch_quality_metas_parallel(&probes).await;

        // Apply refinement to the rows we probed.
        for ((idx, _), meta_opt) in probes.iter().zip(metas.iter()) {
            if let Some(meta) = meta_opt {
                let rec = &mut work.recordings[*idx];
                rec.sample_rate_hz = meta.sample_rate_hz;
                rec.bit_depth = meta.bit_depth;
                // The Tidal tier from the probe is authoritative for
                // refining `audio_quality_tags`: if `mediaMetadata.tags`
                // missed a tier (rare but observed for some legacy
                // tracks), promote it.
                if !meta.tier.is_empty()
                    && !rec.audio_quality_tags.iter().any(|t| t == &meta.tier)
                {
                    rec.audio_quality_tags.push(meta.tier.clone());
                }
            }
        }

        // Compute `quality_score` for every recording (refined or not).
        for rec in work.recordings.iter_mut() {
            rec.quality_score = quality::score_recording(rec);
        }

        // Aggregate the work-level "best available" summary.
        work.best_available_quality = quality::best_available(&work.recordings);
    }

    /// Fetch quality meta for several track ids with bounded parallelism
    /// and per-id cache. Returns metas in the same order as input.
    async fn fetch_quality_metas_parallel(
        &self,
        probes: &[(usize, u64)],
    ) -> Vec<Option<TrackQualityMeta>> {
        use tokio::sync::Semaphore;
        let sem = Arc::new(Semaphore::new(QUALITY_REFINE_PARALLELISM));
        let mut handles = Vec::with_capacity(probes.len());

        for (idx, track_id) in probes.iter().copied() {
            let sem = Arc::clone(&sem);
            let cache = Arc::clone(&self.cache);
            let tidal = Arc::clone(&self.tidal);
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.ok();
                let meta = fetch_or_cache_track_quality(&cache, &tidal, track_id).await;
                (idx, meta)
            }));
        }

        let mut results: Vec<Option<TrackQualityMeta>> =
            (0..probes.len()).map(|_| None).collect();
        for (slot, h) in handles.into_iter().enumerate() {
            match h.await {
                Ok((_idx, meta)) => {
                    results[slot] = meta;
                }
                Err(e) => {
                    log::debug!("[catalog] quality probe task join error: {e}");
                }
            }
        }
        results
    }

    /// Run the cascade for a batch of recordings. Returns recordings in
    /// the same order as the input.
    async fn resolve_recordings(
        &self,
        input: &[Recording],
        composer_name: Option<&str>,
        work_title: &str,
        catalogue: Option<&CatalogueNumber>,
        bucket: Option<WorkBucket>,
    ) -> Vec<Recording> {
        let mut out: Vec<Recording> = Vec::with_capacity(input.len());
        for rec in input.iter() {
            let mut clone = rec.clone();
            let outcome = self
                .resolve_one_recording(
                    &clone,
                    composer_name,
                    work_title,
                    catalogue,
                    bucket,
                )
                .await;
            matching::apply_outcome(&mut clone, outcome);
            out.push(clone);
        }
        out
    }

    /// Single-recording cascade. Returns the best `MatchOutcome`.
    async fn resolve_one_recording(
        &self,
        rec: &Recording,
        composer_name: Option<&str>,
        work_title: &str,
        catalogue: Option<&CatalogueNumber>,
        bucket: Option<WorkBucket>,
    ) -> MatchOutcome {
        // 1) Try ISRC. First successful match wins.
        for isrc in rec.isrcs.iter() {
            match self.tidal.lookup_by_isrc(isrc).await {
                Ok(Some(hit)) => {
                    return MatchOutcome {
                        track_id: Some(hit.track_id),
                        album_id: hit.album_id,
                        quality_tags: hit.quality_tags,
                        audio_modes: hit.audio_modes,
                        duration_secs: Some(hit.duration_secs),
                        cover_url: hit.cover,
                        confidence: MatchConfidence::IsrcBound,
                        query_used: None,
                        score: Some(1.0),
                    };
                }
                Ok(None) => {}
                Err(e) => {
                    log::debug!("[catalog] tidal isrc {isrc}: {e}");
                }
            }
        }

        // 2) Fall back to text search using composer + work title +
        //    catalogue (D-041) + primary credited artist (typically the
        //    conductor).
        let primary_artist = rec.artist_credits.first().map(|s| s.as_str());
        let query = build_canonical_query(
            composer_name,
            work_title,
            catalogue,
            primary_artist,
            rec.recording_year,
        );

        match self.tidal.search_canonical(&query, 5).await {
            Ok(results) => {
                if results.tracks.is_empty() {
                    return MatchOutcome::not_found();
                }
                let mut outcome = matching::best_candidate(
                    &results.tracks,
                    primary_artist,
                    work_title,
                    rec.recording_year,
                    rec.duration_secs,
                    bucket,
                    query,
                );
                // If the best score didn't hit the threshold, fall to NotFound.
                if outcome.score.unwrap_or(0.0) < INFERRED_THRESHOLD {
                    outcome.confidence = MatchConfidence::NotFound;
                    outcome.track_id = None;
                    outcome.album_id = None;
                    outcome.quality_tags.clear();
                    outcome.audio_modes.clear();
                    outcome.duration_secs = None;
                    outcome.cover_url = None;
                }
                outcome
            }
            Err(e) => {
                log::debug!("[catalog] tidal search '{query}' failed: {e:?}");
                MatchOutcome::not_found()
            }
        }
    }

    // -----------------------------------------------------------------
    // Recording (lazy detail enrichment)
    // -----------------------------------------------------------------

    pub async fn get_recording(
        &self,
        mbid: &str,
        work_mbid: &str,
    ) -> Result<Recording, SoneError> {
        if mbid.is_empty() {
            return Err(SoneError::Parse("empty recording mbid".into()));
        }
        let key = format!("{RECORDING_CACHE_PREFIX}{mbid}");
        if let CacheResult::Fresh(bytes) =
            self.cache.get(&key, CacheTier::StaticMeta).await
        {
            if let Ok(cached) = serde_json::from_slice::<Recording>(&bytes) {
                return Ok(cached);
            }
        }

        let mut shell = Recording::shell(mbid, work_mbid);
        if let Err(e) = self.mb.enrich_recording(&mut shell).await {
            log::warn!("[catalog] recording detail {mbid}: {e}");
        }
        let bytes = serde_json::to_vec(&shell).unwrap_or_default();
        if let Err(e) = self
            .cache
            .put(&key, &bytes, CacheTier::StaticMeta, &[RECORDING_CACHE_TAG])
            .await
        {
            log::warn!("[catalog] cache put recording {mbid}: {e}");
        }
        Ok(shell)
    }

    // -----------------------------------------------------------------
    // Composer (Phase 1 placeholder; Phase 2 will hydrate work groupings)
    // -----------------------------------------------------------------

    pub async fn get_composer(&self, mbid: &str) -> Result<Composer, SoneError> {
        if mbid.is_empty() {
            return Err(SoneError::Parse("empty composer mbid".into()));
        }
        let key = format!("{COMPOSER_CACHE_PREFIX}{mbid}");
        if let CacheResult::Fresh(bytes) =
            self.cache.get(&key, CacheTier::StaticMeta).await
        {
            if let Ok(cached) = serde_json::from_slice::<Composer>(&bytes) {
                return Ok(cached);
            }
        }

        // D-038 (bug 4) — transient MB failure must NOT be cached.
        let mut composer = match self.mb.fetch_composer(mbid).await {
            Ok(c) => c,
            Err(e) if e.is_transient() => {
                log::warn!(
                    "[catalog] composer {mbid} transient failure; not caching: {e}"
                );
                return Err(e);
            }
            Err(e) => {
                return Err(e);
            }
        };
        if let Err(e) = self.wikipedia.enrich_composer(&mut composer).await {
            log::debug!("[catalog] wiki composer {mbid}: {e}");
        }
        // OpenOpus fills era / dates / portrait when MB left them empty.
        // Pure in-memory lookup (no I/O), so it's safe to call after Wiki.
        if let Err(e) = self.openopus.enrich_composer(&mut composer).await {
            log::debug!("[catalog] openopus composer {mbid}: {e}");
        }
        // Phase 5 (D-020) — composer-level editor note from the snapshot.
        if composer.editor_note.is_none() {
            if let Some(note) = self.editorial.lookup_composer(mbid) {
                composer.editor_note = Some(note.editor_note);
            }
        }
        // Phase 6 (D-022) — Wikidata enrichment + related composers.
        // Best-effort: failures degrade gracefully to no portrait /
        // empty related list. Cached separately so the next call hits
        // the WD cache without re-hitting WDQS.
        self.enrich_composer_with_wikidata(&mut composer).await;
        let bytes = serde_json::to_vec(&composer).unwrap_or_default();
        if let Err(e) = self
            .cache
            .put(&key, &bytes, CacheTier::StaticMeta, &[COMPOSER_CACHE_TAG])
            .await
        {
            log::warn!("[catalog] cache put composer {mbid}: {e}");
        }
        Ok(composer)
    }

    /// Resolve `(work_mbid)` from a `recording_mbid`. Used by the player
    /// "View work" button path: when scrobble has set `recording_mbid`,
    /// we need to find the parent work to navigate. Best-effort: returns
    /// `None` if the recording is not classical / not in MB.
    pub async fn resolve_work_for_recording(
        &self,
        recording_mbid: &str,
    ) -> Result<Option<String>, SoneError> {
        // MB endpoint: /recording/{mbid}?inc=work-rels — the relations
        // array contains the parent work via type=performance + direction=forward.
        let url = format!(
            "https://musicbrainz.org/ws/2/recording/{recording_mbid}?inc=work-rels&fmt=json"
        );
        let body = self.mb.get_json_pub(&url).await?;
        let rels = match body.get("relations").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return Ok(None),
        };
        for rel in rels.iter() {
            let kind = rel.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if kind.eq_ignore_ascii_case("performance") {
                if let Some(id) = rel
                    .get("work")
                    .and_then(|w| w.get("id"))
                    .and_then(|v| v.as_str())
                {
                    return Ok(Some(id.to_string()));
                }
            }
        }
        Ok(None)
    }

    // -----------------------------------------------------------------
    // Phase 3 — movement boundary detection
    // -----------------------------------------------------------------

    /// Phase 3 (B3.2): given a track title playing inside a known Work,
    /// return the matching `MovementContext` so the player can render
    /// "II / IV" + "Attacca →".
    ///
    /// Strategy (delegated to `movement::resolve_*`):
    ///   1. Roman numeral prefix in `track_title`.
    ///   2. Title substring match against `Movement.title`.
    ///   3. Album-position fallback when caller passes `album_position`.
    ///
    /// Pure read on the cached `Work`. Hits the network only if the work
    /// isn't cached yet — typically warm by the time the player asks (the
    /// "View work" button has already triggered `get_work`).
    pub async fn resolve_movement(
        &self,
        work_mbid: &str,
        track_title: &str,
        album_position: Option<u32>,
    ) -> Result<Option<super::movement::MovementContext>, SoneError> {
        let work = self.get_work(work_mbid).await?;
        if let Some(ctx) = super::movement::resolve_by_title(&work, track_title) {
            return Ok(Some(ctx));
        }
        if let Some(pos) = album_position {
            // pos comes 1-based from the Tidal album track-list (1, 2, ...);
            // resolve_by_position takes 0-based.
            let idx0 = pos.saturating_sub(1) as usize;
            if let Some(ctx) = super::movement::resolve_by_position(&work, idx0) {
                return Ok(Some(ctx));
            }
        }
        Ok(None)
    }

    // -----------------------------------------------------------------
    // Phase 2 — browse (composers + works listings)
    // -----------------------------------------------------------------

    /// Phase 2 + 7: top-N composers.
    ///
    /// For `limit ≤ 33` we delegate to OpenOpus (preserves the canonical
    /// curated popular-first ordering used by Hub Featured / Editor's
    /// Choice surfaces).
    ///
    /// For `limit > 33` we serve from the extended snapshot (D-027 /
    /// D-033, ~6k composers harvested from Wikidata classical-genre
    /// filter). The extended provider already orders popular-first
    /// thanks to the OpenOpus merge done at harvest time, so the top
    /// ~30 entries are identical between providers — the only
    /// difference is the long tail beyond OpenOpus' canon.
    ///
    /// Synchronous: snapshots live in-process, no cache or async needed.
    pub fn list_top_composers(&self, limit: usize) -> Vec<ComposerSummary> {
        if limit <= 33 {
            self.openopus.top_composers(limit)
        } else {
            self.composers_extended.top_composers(limit)
        }
    }

    /// Phase 2 + 7: composers in a given era. Always serves from the
    /// extended snapshot (it's a strict superset of OpenOpus by era,
    /// since the harvest defensively preserves every OpenOpus composer
    /// regardless of Wikidata genre claims).
    pub fn list_composers_by_era(&self, era: Era) -> Vec<ComposerSummary> {
        self.composers_extended.composers_by_era(era)
    }

    /// Phase 7 — total composers indexed in the extended snapshot.
    /// Surfaced by the Hub home footer (F7.3) for the "Catalog: X
    /// composers indexed" chip.
    pub fn extended_composers_total(&self) -> usize {
        self.composers_extended.total_count()
    }

    /// Phase 2 + 7: works for a given composer MBID. Strategy:
    ///
    ///   1. Snapshot first — if the composer is in OpenOpus, that gives us
    ///      a curated list of canonical works with `popular` + `genre`. We
    ///      DO NOT have MB work MBIDs from the snapshot, so we must
    ///      cross-reference via MB browse to bind playable identifiers.
    ///   2. MB browse `work?artist={mbid}&inc=aliases+work-rels` returns up
    ///      to 100 works for that artist. We turn each into a
    ///      `WorkSummary` reusing the title parsers. When the MB title
    ///      matches an OpenOpus title (loose substring), inherit
    ///      `popular` from the snapshot. Phase 7 (D-028) drops child
    ///      movements at the provider boundary — they shouldn't surface
    ///      as top-level entries.
    ///   3. Optional `genre` filter is applied at the end.
    ///
    /// Phase 7 (D-029): cache key bumped from v1→v2 to invalidate stale
    /// non-paginated entries; offset becomes part of the key. Phase 8.9
    /// (D-047 / A4): bumped to v3 because `ComposerWorksPage` gained the
    /// `next_offset` field. Cached for 7d (StaticMeta) keyed by
    /// `composer-works:v3:{mbid}:{genre}:{offset}`.
    pub async fn list_works_by_composer(
        &self,
        composer_mbid: &str,
        genre: Option<Genre>,
        offset: u32,
    ) -> Result<ComposerWorksPage, SoneError> {
        if composer_mbid.is_empty() {
            return Err(SoneError::Parse("empty composer mbid".into()));
        }
        let genre_key = genre
            .map(|g| format!("{:?}", g))
            .unwrap_or_else(|| "all".to_string());
        let key = format!(
            "{COMPOSER_WORKS_CACHE_PREFIX}{composer_mbid}:{genre_key}:{offset}"
        );

        match self.cache.get(&key, CacheTier::StaticMeta).await {
            CacheResult::Fresh(bytes) => {
                if let Ok(cached) = serde_json::from_slice::<ComposerWorksPage>(&bytes) {
                    log::debug!("[catalog] composer-works {composer_mbid} cache HIT (offset={offset})");
                    return Ok(cached);
                }
            }
            CacheResult::Stale(bytes) => {
                if let Ok(cached) = serde_json::from_slice::<ComposerWorksPage>(&bytes) {
                    return Ok(cached);
                }
            }
            CacheResult::Miss => {}
        }

        let page = self
            .build_composer_works_fresh(composer_mbid, genre, offset)
            .await?;
        let bytes = serde_json::to_vec(&page).unwrap_or_default();
        if let Err(e) = self
            .cache
            .put(
                &key,
                &bytes,
                CacheTier::StaticMeta,
                &[COMPOSER_WORKS_CACHE_TAG],
            )
            .await
        {
            log::warn!("[catalog] cache put composer-works {composer_mbid} offset={offset}: {e}");
        }
        Ok(page)
    }

    async fn build_composer_works_fresh(
        &self,
        composer_mbid: &str,
        filter_genre: Option<Genre>,
        offset: u32,
    ) -> Result<ComposerWorksPage, SoneError> {
        // (a) MB browse — gives us MBIDs, titles, attribute hints.
        // Phase 7 (D-029): pass offset; (D-028) movement filter applied
        // inside the provider.
        let page = self
            .mb
            .browse_works_by_artist(composer_mbid, COMPOSER_WORKS_LIMIT, offset)
            .await
            .unwrap_or_else(|e| {
                log::warn!("[catalog] mb browse works for {composer_mbid}: {e}");
                super::providers::musicbrainz::MbBrowsedWorksPage {
                    works: Vec::new(),
                    total: 0,
                    offset,
                }
            });
        let mb_works = &page.works;
        let mb_total = page.total;

        // (b) Snapshot lookup — for popular flag + genre fallback.
        let snapshot_works = self.openopus.works_for_composer(composer_mbid);

        // Pre-index OpenOpus titles (lower-case, trimmed) for cheap matching.
        let oo_index: Vec<(String, &super::providers::openopus::OpenOpusWork)> = snapshot_works
            .iter()
            .map(|w| (normalize_title_for_match(&w.title), *w))
            .collect();

        let composer_summary = self.openopus.lookup_composer_summary(composer_mbid);
        let composer_name = composer_summary.as_ref().map(|c| c.name.clone());

        let mut out: Vec<WorkSummary> = Vec::with_capacity(mb_works.len());
        for w in mb_works.iter() {
            let normalized = normalize_title_for_match(&w.title);
            let oo_match = oo_index
                .iter()
                .find(|(t, _)| {
                    t == &normalized || t.contains(&normalized) || normalized.contains(t)
                })
                .map(|(_, oo)| *oo);

            let popular = oo_match.map(|oo| oo.popular).unwrap_or(false);
            let inferred_genre = oo_match
                .and_then(|oo| genre_for_oo_label(oo.genre.as_deref()))
                .or(w.genre);

            if let Some(filter) = filter_genre {
                if Some(filter) != inferred_genre {
                    continue;
                }
            }

            // Phase 9 (B9.1 / D-040) — bucket cached on every
            // `WorkSummary`. Editorial override beats heuristic. P136
            // not threaded here because the MB browse path doesn't
            // carry Wikidata claims; the snapshot covers the canon.
            let editorial_bucket = self.editorial.lookup_bucket(composer_mbid, &w.title);
            let bucket = editorial_bucket.unwrap_or_else(|| {
                buckets::bucket_for(w.work_type, inferred_genre, &[], &w.title)
            });

            out.push(WorkSummary {
                mbid: w.mbid.clone(),
                title: w.title.clone(),
                composer_mbid: Some(composer_mbid.to_string()),
                composer_name: composer_name.clone(),
                catalogue_number: w.catalogue_number.clone(),
                key: w.key.clone(),
                work_type: w.work_type,
                genre: inferred_genre,
                bucket: Some(bucket),
                composition_year: None,
                popular,
            });
        }

        // Sort: popular first, then by catalogue_number (so e.g. Op. 1
        // before Op. 125), then alphabetic.
        out.sort_by(|a, b| {
            b.popular
                .cmp(&a.popular)
                .then_with(|| {
                    sort_key_for_catalogue(a.catalogue_number.as_ref())
                        .cmp(&sort_key_for_catalogue(b.catalogue_number.as_ref()))
                })
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });

        // Phase 7 (D-029) — `has_more` is computed against MB's pre-
        // movement-filter total; that's a slight over-estimate, but
        // honest. The frontend just uses it to decide whether to show
        // "Load more" — false-positives mean an extra empty fetch, not
        // missing data.
        let mb_returned = mb_works.len() as u32;
        let consumed = offset.saturating_add(mb_returned);
        let has_more = consumed < mb_total;
        // D-047 (A4) — MB-pre-filter cursor for the next page. The
        // frontend used to pass `works.length` (post D-028 movement
        // filter) which silently overlapped with already-loaded
        // entries on big catalogues. `next_offset` is the next MB
        // offset to query so paging advances strictly through MB.
        let next_offset = consumed;
        Ok(ComposerWorksPage {
            works: out,
            total: mb_total,
            offset,
            has_more,
            next_offset,
        })
    }

    /// Invalidate every cached classical entry. Reserved for the
    /// "Clear cache" UI in Phase 4. Tag-based so it stays cheap.
    pub async fn invalidate_all(&self) {
        self.cache.invalidate_tag(WORK_CACHE_TAG).await;
        self.cache.invalidate_tag(RECORDING_CACHE_TAG).await;
        self.cache.invalidate_tag(COMPOSER_CACHE_TAG).await;
        self.cache.invalidate_tag(COMPOSER_WORKS_CACHE_TAG).await;
        self.cache.invalidate_tag(TRACK_QUALITY_CACHE_TAG).await;
    }

    /// Phase 4 (B4.3): force-refresh the quality metadata for an
    /// existing work. Drops the cached `Work` entry and the per-track
    /// quality cache, then rebuilds. Used by the "Refresh quality" UI
    /// in the WorkPage when the user wants to discover newly added
    /// HIRES tracks.
    pub async fn refresh_work_recording_qualities(
        &self,
        work_mbid: &str,
    ) -> Result<Work, SoneError> {
        if work_mbid.is_empty() {
            return Err(SoneError::Parse("empty work mbid".into()));
        }
        // Drop the cached work so the next get_work runs the full
        // pipeline including refinement.
        let key = format!("{WORK_CACHE_PREFIX}{work_mbid}");
        self.cache.invalidate_key(&key).await;
        // Drop per-track quality cache scoped to this work — we don't
        // know the ids without rebuilding, so scrub the whole tag. This
        // is fine: it's a manual operation, not a hot path.
        self.cache.invalidate_tag(TRACK_QUALITY_CACHE_TAG).await;
        self.get_work(work_mbid).await
    }

    // -----------------------------------------------------------------
    // Phase 5 — search (D-019)
    // -----------------------------------------------------------------

    /// Phase 5 (B5.1): tokenize a free-text query, build a `SearchPlan`,
    /// and execute it against the in-process catalog. Returns up to
    /// `limit` `SearchHit`s ranked by score.
    ///
    /// Strategy:
    ///   - If the plan has a composer, list their works via
    ///     `list_works_by_composer` (warm-cache path) and rank locally.
    ///   - Otherwise we currently fall back to the snapshot composer
    ///     index — a simple heuristic for V1. MB Lucene fallback can be
    ///     added in Phase 6 once we exercise the V1 surface in real use.
    ///
    /// Phase 8 (B8.1): the implementation is now a thin wrapper around
    /// `search_internal`, the streaming-aware helper that the new
    /// `search_classical_streaming` command also uses. The synchronous
    /// path collects every hit into a `Vec`, sorts by score desc, and
    /// truncates — preserving the exact contract Phase 5 tests rely on.
    pub async fn search_classical(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchResults, SoneError> {
        let limit = limit.clamp(1, 50);
        let mut hits: Vec<SearchHit> = Vec::new();
        // The closure returns `true` to keep going; the synchronous path
        // never short-circuits — we want every hit so the post-sort
        // truncate produces the highest-ranked list.
        let plan = self
            .search_internal(query, usize::MAX, |hit| {
                hits.push(hit);
                true
            })
            .await?;

        // Sort by score descending; ties broken by title length asc
        // (shorter titles tend to be the primary work, longer ones are
        // arrangements / fragments).
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.len().cmp(&b.title.len()))
        });
        hits.truncate(limit);

        Ok(SearchResults { plan, hits })
    }

    /// Internal helper that drives `search_classical`.
    ///
    /// The contract: tokenize+plan first, then enumerate candidate hits
    /// and feed each one to `emit`. The closure decides whether to keep
    /// going (`true`) or stop (`false`). The plan is always returned so
    /// callers can drive the "Detected: ..." chips even when zero hits
    /// pass the score threshold.
    ///
    /// `cap` is the maximum number of hits the helper will emit before
    /// stopping on its own. Pass `usize::MAX` from the synchronous path
    /// (caller does its own truncate after sort); pass the user-visible
    /// cap from the streaming path so we don't waste cycles past it.
    async fn search_internal<F>(
        &self,
        query: &str,
        cap: usize,
        mut emit: F,
    ) -> Result<SearchPlan, SoneError>
    where
        F: FnMut(SearchHit) -> bool,
    {
        // Phase 7 (D-031) — feed the tokenizer the extended snapshot
        // (~6k composers) instead of the OpenOpus subset. The tokenizer
        // logic is unchanged; only the universe grows. This means
        // queries like "Hildegard" or "Pärt" or "Saariaho" tokenize as
        // composers rather than degrading to keywords.
        //
        // The planner still uses `OpenOpusProvider` for its richer
        // metadata (full_name lookup) — composers outside OpenOpus
        // resolve through the extended snapshot's surname token.
        let composer_index = self.composers_extended.top_composers(2000);
        let tokens = search::tokenize(query, &composer_index);
        let plan = search::plan(tokens, &self.openopus);

        // Empty-query short-circuit: nothing to emit, plan is already
        // empty by construction. We still return it so the UI can
        // clear its chips deterministically.
        if cap == 0 || (plan.composer_mbid.is_none() && plan.catalogue.is_none() && plan.keywords.is_empty()) {
            return Ok(plan);
        }

        // Track which work_mbids we've already emitted. Both the
        // composer-list path and the snapshot scan can surface the same
        // work; the dedup keeps the UI from rendering duplicate rows.
        let mut seen_work_mbids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut emitted: usize = 0;

        // -- Composer-list path -------------------------------------------------
        if let Some(ref composer_mbid) = plan.composer_mbid {
            let page = self
                .list_works_by_composer(composer_mbid, None, 0)
                .await
                .ok();
            let works = page.map(|p| p.works).unwrap_or_default();
            for w in works.iter() {
                let score = search::score_work(w, &plan);
                if score <= 0.0 {
                    continue;
                }
                if !seen_work_mbids.insert(w.mbid.clone()) {
                    continue;
                }
                let hit = SearchHit {
                    work_mbid: w.mbid.clone(),
                    title: w.title.clone(),
                    composer_name: w.composer_name.clone().or_else(|| plan.composer_name.clone()),
                    composer_mbid: Some(composer_mbid.clone()),
                    catalogue_display: w
                        .catalogue_number
                        .as_ref()
                        .map(|c| c.display.clone()),
                    score,
                    source: "composer-list".to_string(),
                };
                let keep_going = emit(hit);
                emitted += 1;
                if !keep_going || emitted >= cap {
                    return Ok(plan);
                }
            }
        }

        // -- Snapshot scan path -------------------------------------------------
        // Triggered when we have keywords or a catalogue but no composer
        // hint. Scans the popular works of the top-40 indexed composers,
        // emitting any hit whose score clears the snapshot threshold.
        if plan.composer_mbid.is_none()
            && (plan.catalogue.is_some() || !plan.keywords.is_empty())
        {
            for c in composer_index.iter().take(40) {
                let page = self
                    .list_works_by_composer(&c.mbid, None, 0)
                    .await
                    .ok();
                let works = page.map(|p| p.works).unwrap_or_default();
                for w in works.iter() {
                    let score = search::score_work(w, &plan);
                    if score < 0.4 {
                        continue;
                    }
                    if !seen_work_mbids.insert(w.mbid.clone()) {
                        continue;
                    }
                    let hit = SearchHit {
                        work_mbid: w.mbid.clone(),
                        title: w.title.clone(),
                        composer_name: w
                            .composer_name
                            .clone()
                            .or_else(|| Some(c.name.clone())),
                        composer_mbid: Some(c.mbid.clone()),
                        catalogue_display: w
                            .catalogue_number
                            .as_ref()
                            .map(|c| c.display.clone()),
                        score,
                        source: "snapshot".to_string(),
                    };
                    let keep_going = emit(hit);
                    emitted += 1;
                    if !keep_going || emitted >= cap {
                        return Ok(plan);
                    }
                }
            }
        }

        Ok(plan)
    }

    // -----------------------------------------------------------------
    // Phase 5 — editorial picks (D-020 + D-021)
    // -----------------------------------------------------------------

    /// Hub home grid: editorial picks from the curated snapshot.
    pub fn list_editorial_picks(&self, limit: usize) -> Vec<EditorialPick> {
        self.editorial.list_picks(limit)
    }

    /// Persist a user-set Editor's Choice for a work. Invalidates the
    /// cached `Work` so the next read re-applies the override.
    pub async fn set_user_editors_choice(
        &self,
        work_mbid: &str,
        recording_mbid: &str,
        note: Option<&str>,
    ) -> Result<(), SoneError> {
        if work_mbid.is_empty() || recording_mbid.is_empty() {
            return Err(SoneError::Parse("empty mbid".into()));
        }
        self.stats
            .set_classical_editorial_choice(work_mbid, recording_mbid, note)
            .map_err(|e| SoneError::Parse(format!("editorial set: {e}")))?;
        // Drop the cached Work so the next fetch sees the override.
        let key = format!("{WORK_CACHE_PREFIX}{work_mbid}");
        self.cache.invalidate_key(&key).await;
        Ok(())
    }

    /// Clear a user override. The Work falls back to the embedded
    /// snapshot pick (or none).
    pub async fn clear_user_editors_choice(
        &self,
        work_mbid: &str,
    ) -> Result<(), SoneError> {
        if work_mbid.is_empty() {
            return Err(SoneError::Parse("empty mbid".into()));
        }
        self.stats
            .clear_classical_editorial_choice(work_mbid)
            .map_err(|e| SoneError::Parse(format!("editorial clear: {e}")))?;
        let key = format!("{WORK_CACHE_PREFIX}{work_mbid}");
        self.cache.invalidate_key(&key).await;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Phase 6 — Wikidata composer enrichment + related composers
    // (D-022, deferred from Phase 5)
    // -----------------------------------------------------------------

    /// Enrich a composer in-place using Wikidata SPARQL. Pulls portrait,
    /// birth/death year, and genre QIDs into the composer struct, then
    /// loads the related-composers list (capped at 12). All best-effort —
    /// any error is swallowed and logged.
    ///
    /// Cached at `WIKIDATA_COMPOSER_CACHE_PREFIX` (StaticMeta) keyed by
    /// QID — so two composers sharing a QID (impossible in practice, but
    /// no harm) reuse the same enrichment.
    async fn enrich_composer_with_wikidata(&self, composer: &mut Composer) {
        let qid = match composer.qid.as_ref() {
            Some(q) if !q.is_empty() => q.clone(),
            _ => return,
        };

        // a) Composer-level enrichment (portrait, birth/death year, genres).
        let enrich_key = format!("{WIKIDATA_COMPOSER_CACHE_PREFIX}{qid}");
        let enrichment = match self.cache.get(&enrich_key, CacheTier::StaticMeta).await {
            CacheResult::Fresh(bytes) | CacheResult::Stale(bytes) => {
                serde_json::from_slice::<
                    super::providers::wikidata::WikidataComposerEnrichment,
                >(&bytes)
                .ok()
            }
            CacheResult::Miss => None,
        };
        let enrichment = if let Some(e) = enrichment {
            e
        } else {
            match self.wikidata.enrich_composer(&qid).await {
                Ok(e) => {
                    let bytes = serde_json::to_vec(&e).unwrap_or_default();
                    let _ = self
                        .cache
                        .put(
                            &enrich_key,
                            &bytes,
                            CacheTier::StaticMeta,
                            &[WIKIDATA_COMPOSER_CACHE_TAG],
                        )
                        .await;
                    e
                }
                Err(e) => {
                    log::debug!("[catalog] wikidata enrich {qid}: {e}");
                    super::providers::wikidata::WikidataComposerEnrichment::default()
                }
            }
        };

        // Apply only what is missing on the composer (preserve MB / Wiki).
        if composer.portrait_url.is_none() {
            composer.portrait_url = enrichment.portrait_url.clone();
        }
        if composer.birth.is_none() {
            if let Some(year) = enrichment.birth_year {
                composer.birth = Some(super::types::LifeEvent {
                    year: Some(year),
                    date: None,
                    place: None,
                });
            }
        }
        if composer.death.is_none() {
            if let Some(year) = enrichment.death_year {
                composer.death = Some(super::types::LifeEvent {
                    year: Some(year),
                    date: None,
                    place: None,
                });
            }
        }

        // b) Related composers — separate cache so we can invalidate
        // independently if the SPARQL changes shape.
        let related_key = format!("{WIKIDATA_RELATED_CACHE_PREFIX}{qid}");
        let related_cached: Option<Vec<RelatedComposer>> =
            match self.cache.get(&related_key, CacheTier::StaticMeta).await {
                CacheResult::Fresh(b) | CacheResult::Stale(b) => {
                    serde_json::from_slice::<Vec<RelatedComposer>>(&b).ok()
                }
                CacheResult::Miss => None,
            };
        let related = if let Some(list) = related_cached {
            list
        } else {
            match self.wikidata.list_related_composers(&qid).await {
                Ok(items) => {
                    let mapped: Vec<RelatedComposer> = items
                        .into_iter()
                        .map(|r| RelatedComposer {
                            qid: r.qid,
                            mbid: r.mbid,
                            name: r.name,
                            shared_genres: r.shared_genres,
                            birth_year: r.birth_year,
                            portrait_url: r.portrait_url,
                        })
                        .collect();
                    let bytes = serde_json::to_vec(&mapped).unwrap_or_default();
                    let _ = self
                        .cache
                        .put(
                            &related_key,
                            &bytes,
                            CacheTier::StaticMeta,
                            &[WIKIDATA_RELATED_CACHE_TAG],
                        )
                        .await;
                    mapped
                }
                Err(e) => {
                    log::debug!("[catalog] wikidata related {qid}: {e}");
                    Vec::new()
                }
            }
        };
        composer.related_composers = related;
    }

    /// Public entry-point: list related composers for a Composer MBID.
    /// Resolves the MBID → QID first by reusing the cached Composer
    /// entity, then falls through to Wikidata. Cheap when the composer
    /// is already warm (most flows go through `get_composer` first).
    pub async fn list_related_composers(
        &self,
        composer_mbid: &str,
    ) -> Result<Vec<RelatedComposer>, SoneError> {
        let composer = self.get_composer(composer_mbid).await?;
        Ok(composer.related_composers)
    }

    // -----------------------------------------------------------------
    // Phase 6 — Stats-backed personalisation queries
    // -----------------------------------------------------------------

    /// Tu top works clásicos. The catalog hands the query straight to
    /// stats — Tauri command callers receive `TopClassicalWork` rows.
    pub fn top_classical_works(
        &self,
        window: StatsWindow,
        limit: u32,
    ) -> Result<Vec<TopClassicalWork>, SoneError> {
        self.stats
            .top_classical_works(window, limit)
            .map_err(|e| SoneError::Parse(format!("top works: {e}")))
    }

    /// Tu top composers. Same shape as above — pure SQL aggregation.
    pub fn top_classical_composers(
        &self,
        window: StatsWindow,
        limit: u32,
    ) -> Result<Vec<TopClassicalComposer>, SoneError> {
        self.stats
            .top_classical_composers(window, limit)
            .map_err(|e| SoneError::Parse(format!("top composers: {e}")))
    }

    /// Recently-played sessions grouped by `work_mbid`. The window is
    /// in seconds (typically 7d × 86400); the limit caps the rows.
    pub fn classical_recently_played_works(
        &self,
        window_secs: i64,
        limit: u32,
    ) -> Result<Vec<RecentClassicalSession>, SoneError> {
        self.stats
            .classical_recently_played_works(window_secs, limit)
            .map_err(|e| SoneError::Parse(format!("recent works: {e}")))
    }

    /// Recording comparison rows for a single work.
    pub fn classical_recording_comparison(
        &self,
        work_mbid: &str,
    ) -> Result<Vec<RecordingComparisonRow>, SoneError> {
        if work_mbid.is_empty() {
            return Err(SoneError::Parse("empty work mbid".into()));
        }
        self.stats
            .classical_recording_comparison(work_mbid)
            .map_err(|e| SoneError::Parse(format!("recording comparison: {e}")))
    }

    /// Aggregate classical-only counters for the window.
    pub fn classical_overview(
        &self,
        window: StatsWindow,
    ) -> Result<ClassicalOverview, SoneError> {
        self.stats
            .classical_overview(window)
            .map_err(|e| SoneError::Parse(format!("classical overview: {e}")))
    }

    /// Discovery curve filtered to classical plays only.
    pub fn classical_discovery_curve(
        &self,
        window: StatsWindow,
    ) -> Result<Vec<crate::stats::DiscoveryPoint>, SoneError> {
        self.stats
            .classical_discovery_curve(window)
            .map_err(|e| SoneError::Parse(format!("discovery curve: {e}")))
    }

    // -----------------------------------------------------------------
    // Phase 6 — Favorites CRUD over `classical_favorites`
    // -----------------------------------------------------------------

    /// Save an entity. `kind` ∈ {"work", "recording", "composer",
    /// "performer"}. Idempotent.
    pub fn add_classical_favorite(
        &self,
        kind: &str,
        mbid: &str,
        display_name: &str,
    ) -> Result<(), SoneError> {
        if !is_valid_favorite_kind(kind) {
            return Err(SoneError::Parse(format!("unknown favorite kind: {kind}")));
        }
        if mbid.is_empty() {
            return Err(SoneError::Parse("empty favorite mbid".into()));
        }
        self.stats
            .add_classical_favorite(kind, mbid, display_name)
            .map_err(|e| SoneError::Parse(format!("favorite add: {e}")))
    }

    pub fn remove_classical_favorite(
        &self,
        kind: &str,
        mbid: &str,
    ) -> Result<(), SoneError> {
        if !is_valid_favorite_kind(kind) {
            return Err(SoneError::Parse(format!("unknown favorite kind: {kind}")));
        }
        if mbid.is_empty() {
            return Err(SoneError::Parse("empty favorite mbid".into()));
        }
        self.stats
            .remove_classical_favorite(kind, mbid)
            .map_err(|e| SoneError::Parse(format!("favorite remove: {e}")))
    }

    pub fn is_classical_favorite(
        &self,
        kind: &str,
        mbid: &str,
    ) -> Result<bool, SoneError> {
        if !is_valid_favorite_kind(kind) {
            return Err(SoneError::Parse(format!("unknown favorite kind: {kind}")));
        }
        self.stats
            .is_classical_favorite(kind, mbid)
            .map_err(|e| SoneError::Parse(format!("favorite check: {e}")))
    }

    pub fn list_classical_favorites(
        &self,
        kind: &str,
        limit: u32,
    ) -> Result<Vec<ClassicalFavorite>, SoneError> {
        if !is_valid_favorite_kind(kind) {
            return Err(SoneError::Parse(format!("unknown favorite kind: {kind}")));
        }
        self.stats
            .list_classical_favorites(kind, limit)
            .map_err(|e| SoneError::Parse(format!("favorite list: {e}")))
    }

    // -----------------------------------------------------------------
    // Phase 6 — Browse-by-conductor / orchestra (D-022)
    // -----------------------------------------------------------------

    /// Discography landing for a conductor / orchestra. Pulls a single
    /// MB browse page (cap 100), groups by parent Work when present so
    /// the UI can render "Beethoven 9 (3 recordings)" tiles instead of
    /// a flat list of movements.
    ///
    /// Cached at `ARTIST_DISCOGRAPHY_CACHE_PREFIX` (StaticMeta) keyed by
    /// MBID — the result is stable on the order of months (new
    /// recordings credited to a conductor are rare).
    pub async fn artist_discography(
        &self,
        artist_mbid: &str,
        limit: u32,
    ) -> Result<ArtistDiscography, SoneError> {
        if artist_mbid.is_empty() {
            return Err(SoneError::Parse("empty artist mbid".into()));
        }
        let limit_clamped = limit.clamp(1, 100) as usize;
        let key = format!("{ARTIST_DISCOGRAPHY_CACHE_PREFIX}{artist_mbid}:{limit_clamped}");
        match self.cache.get(&key, CacheTier::StaticMeta).await {
            CacheResult::Fresh(bytes) | CacheResult::Stale(bytes) => {
                if let Ok(cached) = serde_json::from_slice::<ArtistDiscography>(&bytes) {
                    return Ok(cached);
                }
            }
            CacheResult::Miss => {}
        }

        let recs = self
            .mb
            .browse_recordings_by_artist(artist_mbid, limit_clamped)
            .await
            .unwrap_or_else(|e| {
                log::warn!("[catalog] artist discography {artist_mbid}: {e}");
                Vec::new()
            });

        let total = recs.len() as u32;
        let entries: Vec<DiscographyEntry> = recs
            .into_iter()
            .map(|r| DiscographyEntry {
                recording_mbid: r.mbid,
                title: r.title,
                artist_credit: r.artist_credit,
                work_mbid: r.work_mbid,
                release_year: r.release_year,
                length_secs: r.length_secs,
                isrcs: r.isrcs,
            })
            .collect();

        // Group by work_mbid so the UI can render "5 versions of
        // Beethoven 9" cards instead of a flat list. Entries without a
        // work_mbid land in a synthetic group at the end.
        let mut groups_map: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut ungrouped: Vec<usize> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            match e.work_mbid.as_ref() {
                Some(w) if !w.is_empty() => {
                    groups_map.entry(w.clone()).or_default().push(i);
                }
                _ => ungrouped.push(i),
            }
        }
        let mut groups: Vec<DiscographyGroup> = groups_map
            .into_iter()
            .map(|(work_mbid, idxs)| DiscographyGroup {
                work_mbid: Some(work_mbid),
                count: idxs.len() as u32,
                indices: idxs.iter().map(|i| *i as u32).collect(),
            })
            .collect();
        // Sort groups by count desc — the Karajan landing leads with
        // his most-recorded works.
        groups.sort_by_key(|g| std::cmp::Reverse(g.count));
        if !ungrouped.is_empty() {
            groups.push(DiscographyGroup {
                work_mbid: None,
                count: ungrouped.len() as u32,
                indices: ungrouped.iter().map(|i| *i as u32).collect(),
            });
        }

        let result = ArtistDiscography {
            artist_mbid: artist_mbid.to_string(),
            total,
            entries,
            groups,
        };
        let bytes = serde_json::to_vec(&result).unwrap_or_default();
        if let Err(e) = self
            .cache
            .put(
                &key,
                &bytes,
                CacheTier::StaticMeta,
                &[ARTIST_DISCOGRAPHY_CACHE_TAG],
            )
            .await
        {
            log::debug!("[catalog] cache put artist disco {artist_mbid}: {e}");
        }
        Ok(result)
    }

    // -----------------------------------------------------------------
    // Phase 6 — Pre-warm canon (background task)
    // -----------------------------------------------------------------

    /// Iterate the top-N OpenOpus composers and prime their composer
    /// page + works list in cache. Designed to run after auth in a
    /// detached `tokio::spawn` — never holds a strong reference to
    /// AppState beyond the catalog Arc, so the task drops cleanly when
    /// the app shuts down.
    ///
    /// `limit` caps the number of composers (default-ish 30); each
    /// composer takes ~2 MB calls + 1 Wikipedia + 1 Wikidata. With the
    /// 1.1s MB rate limit, 30 composers ≈ 90s baseline.
    pub async fn prewarm_canon(&self, limit: u32) {
        let composers = self.openopus.top_composers(limit.clamp(1, 100) as usize);
        for c in composers.iter() {
            // get_composer pulls MB + Wikipedia + OpenOpus + Wikidata
            // through the existing cache, so subsequent calls are warm.
            if let Err(e) = self.get_composer(&c.mbid).await {
                log::debug!("[prewarm] composer {} failed: {e}", c.mbid);
                continue;
            }
            // list_works_by_composer with no genre filter caches the
            // unfiltered list — the Composer page hits this immediately.
            if let Err(e) = self.list_works_by_composer(&c.mbid, None, 0).await {
                log::debug!("[prewarm] works {} failed: {e}", c.mbid);
            }
        }
        log::info!(
            "[prewarm] classical canon warm-up complete: {} composers",
            composers.len()
        );
    }

    // -----------------------------------------------------------------
    // Phase 9 (B9.3 / B9.4 / D-040 / D-041) — bucketed composer view.
    //
    // The Composer page Works tab is rendered as a list of
    // `BucketSummary` rows — one per non-empty `WorkBucket`, sorted by
    // canonical presentation order, each carrying its top-12 picks
    // and (when the bucket is large) a list of sub-buckets the
    // frontend exposes as filter chips.
    // -----------------------------------------------------------------

    /// Phase 9 (B9.3) — return the full bucketed view for a composer.
    /// Internally:
    ///   1. Browse ALL pages of MB works for the artist (multi-page
    ///      fetcher). Cached at `composer_buckets:v1:{mbid}` for 7d.
    ///   2. Compute `bucket` for each work via the editorial override
    ///      cascade (snapshot first, heuristic second).
    ///   3. Group by bucket; sort buckets by canonical order; each
    ///      bucket sorts works by `(popular desc, catalogue asc, title asc)`.
    ///   4. Top-12 stays as `top_works`; the rest is reachable through
    ///      `list_classical_works_in_bucket`.
    ///   5. Sub-buckets are computed for buckets with `total_count > 12`
    ///      and on a per-bucket basis (Concertos → Piano/Violin/Cello,
    ///      Chamber → Quartets/Trios/Quintets, Keyboard → Sonatas/
    ///      Variations/Études/Character pieces).
    pub async fn list_classical_composer_buckets(
        &self,
        composer_mbid: &str,
    ) -> Result<ComposerBuckets, SoneError> {
        if composer_mbid.is_empty() {
            return Err(SoneError::Parse("empty composer mbid".into()));
        }
        // v3: universal `: ` filter at the MB browse layer drops every
        // `<parent>: <child>` title (operatic arias, suite movements,
        // ballet acts) without requiring an external whitelist. Hob.
        // cat# notation `XVI:50` is preserved (no whitespace).
        let cache_key = format!("classical:composer-buckets:v3:{composer_mbid}");

        // Cache fast-path.
        if let CacheResult::Fresh(bytes) =
            self.cache.get(&cache_key, CacheTier::StaticMeta).await
        {
            if let Ok(cached) = serde_json::from_slice::<ComposerBuckets>(&bytes) {
                return Ok(cached);
            }
        }
        if let CacheResult::Stale(bytes) =
            self.cache.get(&cache_key, CacheTier::StaticMeta).await
        {
            if let Ok(cached) = serde_json::from_slice::<ComposerBuckets>(&bytes) {
                return Ok(cached);
            }
        }

        let buckets = self
            .build_composer_buckets_fresh(composer_mbid)
            .await?;
        let bytes = serde_json::to_vec(&buckets).unwrap_or_default();
        if let Err(e) = self
            .cache
            .put(&cache_key, &bytes, CacheTier::StaticMeta, &[])
            .await
        {
            log::warn!("[catalog] cache put composer-buckets {composer_mbid}: {e}");
        }
        Ok(buckets)
    }

    async fn build_composer_buckets_fresh(
        &self,
        composer_mbid: &str,
    ) -> Result<ComposerBuckets, SoneError> {
        // (1) Multi-page MB browse — collects every parent work
        // attributed to the composer.
        let page = self
            .mb
            .browse_all_works_by_artist(composer_mbid)
            .await
            .unwrap_or_else(|e| {
                log::warn!(
                    "[catalog] composer-buckets: mb browse_all for {composer_mbid} failed: {e}"
                );
                super::providers::musicbrainz::MbBrowsedWorksPage {
                    works: Vec::new(),
                    total: 0,
                    offset: 0,
                }
            });
        let mb_total = page.total;
        let mb_works = page.works;

        // (2) OpenOpus cross-index for popularity + genre fallback.
        let snapshot_works = self.openopus.works_for_composer(composer_mbid);
        let oo_index: Vec<(String, &super::providers::openopus::OpenOpusWork)> = snapshot_works
            .iter()
            .map(|w| (normalize_title_for_match(&w.title), *w))
            .collect();
        let composer_summary = self.openopus.lookup_composer_summary(composer_mbid);
        let composer_name = composer_summary.as_ref().map(|c| c.name.clone());

        // (3) Build the per-work `WorkSummary` enriched with `bucket`.
        // The MB browse layer already drops child-shaped titles via
        // `title_looks_like_movement` (roman prefix, provenance
        // phrases, the universal `: ` rule). OpenOpus is only used
        // here for popularity + genre cross-index, never as a
        // whitelist — its snapshot is too narrow (e.g. only 6 Mozart
        // symphonies of the 41 numbered ones) to be authoritative.
        let mut summaries: Vec<WorkSummary> = Vec::with_capacity(mb_works.len());
        for w in mb_works.iter() {

            let normalized = normalize_title_for_match(&w.title);
            let oo_match = oo_index
                .iter()
                .find(|(t, _)| {
                    t == &normalized || t.contains(&normalized) || normalized.contains(t)
                })
                .map(|(_, oo)| *oo);
            let popular = oo_match.map(|oo| oo.popular).unwrap_or(false);
            let inferred_genre = oo_match
                .and_then(|oo| genre_for_oo_label(oo.genre.as_deref()))
                .or(w.genre);

            let editorial_bucket = self.editorial.lookup_bucket(composer_mbid, &w.title);
            let bucket = editorial_bucket.unwrap_or_else(|| {
                buckets::bucket_for(w.work_type, inferred_genre, &[], &w.title)
            });

            summaries.push(WorkSummary {
                mbid: w.mbid.clone(),
                title: w.title.clone(),
                composer_mbid: Some(composer_mbid.to_string()),
                composer_name: composer_name.clone(),
                catalogue_number: w.catalogue_number.clone(),
                key: w.key.clone(),
                work_type: w.work_type,
                genre: inferred_genre,
                bucket: Some(bucket),
                composition_year: None,
                popular,
            });
        }

        let total_works = summaries.len() as u32;
        let canonical_works_loaded = total_works;

        // (4) Group by bucket, sort each bucket internally.
        let mut grouped: HashMap<WorkBucket, Vec<WorkSummary>> = HashMap::new();
        for s in summaries.into_iter() {
            let b = s.bucket.unwrap_or(WorkBucket::Other);
            grouped.entry(b).or_default().push(s);
        }
        for vec in grouped.values_mut() {
            vec.sort_by(|a, b| {
                b.popular
                    .cmp(&a.popular)
                    .then_with(|| {
                        sort_key_for_catalogue(a.catalogue_number.as_ref())
                            .cmp(&sort_key_for_catalogue(b.catalogue_number.as_ref()))
                    })
                    .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            });
        }

        // (5) Materialise per-bucket summaries in canonical order.
        let mut bucket_summaries: Vec<BucketSummary> = Vec::new();
        let canonical_order = [
            WorkBucket::Stage,
            WorkBucket::ChoralSacred,
            WorkBucket::Vocal,
            WorkBucket::Symphonies,
            WorkBucket::Concertos,
            WorkBucket::Orchestral,
            WorkBucket::Chamber,
            WorkBucket::Keyboard,
            WorkBucket::SoloInstrumental,
            WorkBucket::FilmTheatre,
            WorkBucket::Other,
        ];
        for b in canonical_order.iter() {
            let entries = match grouped.remove(b) {
                Some(v) if !v.is_empty() => v,
                _ => continue,
            };
            let total_count = entries.len() as u32;
            let mut top_works = entries.clone();
            top_works.truncate(12);
            let sub_buckets = if total_count > 12 {
                compute_sub_buckets(*b, &entries)
            } else {
                None
            };
            bucket_summaries.push(BucketSummary {
                bucket: *b,
                label_en: b.label_en().to_string(),
                label_es: b.label_es().to_string(),
                total_count,
                top_works,
                sub_buckets,
            });
        }

        Ok(ComposerBuckets {
            composer_mbid: composer_mbid.to_string(),
            buckets: bucket_summaries,
            total_works,
            mb_total,
            canonical_works_loaded,
        })
    }

    /// Phase 9 (B9.4) — drill-down: list the works inside one bucket
    /// of a composer's catalogue. Operates on the cached
    /// `composer_buckets:v1:{mbid}` payload, so this command never
    /// triggers an MB call once the buckets cache is warm.
    ///
    /// `sub_bucket` (optional): when present, filters by sub-bucket
    /// label produced by `compute_sub_buckets` (e.g. "Piano" inside
    /// `Concertos`). Unknown sub-buckets fall through to the empty
    /// list.
    ///
    /// `sort` (optional): "Catalog" (default — falls back to title
    /// when the work has no catalogue number) | "Date" (composition
    /// year, falls back to title) | "Alphabetical".
    pub async fn list_classical_works_in_bucket(
        &self,
        composer_mbid: &str,
        bucket: WorkBucket,
        sub_bucket: Option<&str>,
        sort: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<WorksPage, SoneError> {
        // Ensure the bucket-summary cache is warm (this also primes
        // the multi-page browse cache the drill-down relies on). If
        // the bucket isn't present in the parent payload, return
        // empty — the drill-down was navigated for a bucket the
        // composer doesn't have.
        let buckets = self.list_classical_composer_buckets(composer_mbid).await?;
        if !buckets.buckets.iter().any(|b| b.bucket == bucket) {
            return Ok(WorksPage {
                works: Vec::new(),
                total: 0,
                offset,
                has_more: false,
            });
        }
        // The bucket summary's `top_works` only carries the top-12;
        // for drill-down we materialise the full bucket separately
        // and cache it under a sibling key so subsequent navigation
        // (sub-bucket changes, sort changes, pagination) doesn't
        // re-walk the multi-page MB browse.
        let full_key =
            format!("classical:bucket-full:v1:{composer_mbid}:{}", bucket_serialised_for_key(bucket));
        let mut full: Vec<WorkSummary> = match self
            .cache
            .get(&full_key, CacheTier::StaticMeta)
            .await
        {
            CacheResult::Fresh(bytes) | CacheResult::Stale(bytes) => {
                serde_json::from_slice::<Vec<WorkSummary>>(&bytes).unwrap_or_default()
            }
            _ => Vec::new(),
        };

        if full.is_empty() {
            // Cache miss — walk the multi-page browse again. The
            // cost is amortised because `browse_all_works_by_artist`
            // is itself fronted by the per-page MB cache; in the
            // typical case this is < 1s warm.
            full = self.gather_full_bucket(composer_mbid, bucket).await?;
            let bytes = serde_json::to_vec(&full).unwrap_or_default();
            if let Err(e) = self
                .cache
                .put(&full_key, &bytes, CacheTier::StaticMeta, &[])
                .await
            {
                log::warn!("[catalog] cache put bucket-full {composer_mbid}/{bucket:?}: {e}");
            }
        }

        // Sub-bucket filter (string-matched against
        // `compute_sub_buckets` labels).
        let mut filtered: Vec<WorkSummary> = if let Some(sb) = sub_bucket {
            full.into_iter()
                .filter(|w| sub_bucket_for_work(bucket, w) == sb)
                .collect()
        } else {
            full
        };

        // Sort.
        match sort.unwrap_or("Catalog") {
            "Date" => {
                filtered.sort_by(|a, b| {
                    a.composition_year
                        .unwrap_or(i32::MAX)
                        .cmp(&b.composition_year.unwrap_or(i32::MAX))
                        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                });
            }
            "Alphabetical" => {
                filtered.sort_by_key(|w| w.title.to_lowercase());
            }
            _ => {
                // "Catalog" default.
                filtered.sort_by(|a, b| {
                    sort_key_for_catalogue(a.catalogue_number.as_ref())
                        .cmp(&sort_key_for_catalogue(b.catalogue_number.as_ref()))
                        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                });
            }
        }

        let total = filtered.len() as u32;
        let start = offset as usize;
        let end = start.saturating_add(limit as usize).min(filtered.len());
        let page = if start < filtered.len() {
            filtered[start..end].to_vec()
        } else {
            Vec::new()
        };
        let has_more = (end as u32) < total;
        Ok(WorksPage {
            works: page,
            total,
            offset,
            has_more,
        })
    }

    /// Helper for `list_classical_works_in_bucket` — re-walk the
    /// full multi-page MB browse and keep only the works whose
    /// computed bucket matches. Cached per `(composer, bucket)`.
    async fn gather_full_bucket(
        &self,
        composer_mbid: &str,
        bucket: WorkBucket,
    ) -> Result<Vec<WorkSummary>, SoneError> {
        let page = self
            .mb
            .browse_all_works_by_artist(composer_mbid)
            .await
            .unwrap_or_else(|e| {
                log::warn!("[catalog] gather_full_bucket browse_all {composer_mbid}: {e}");
                super::providers::musicbrainz::MbBrowsedWorksPage {
                    works: Vec::new(),
                    total: 0,
                    offset: 0,
                }
            });

        let snapshot_works = self.openopus.works_for_composer(composer_mbid);
        let oo_index: Vec<(String, &super::providers::openopus::OpenOpusWork)> = snapshot_works
            .iter()
            .map(|w| (normalize_title_for_match(&w.title), *w))
            .collect();
        let composer_summary = self.openopus.lookup_composer_summary(composer_mbid);
        let composer_name = composer_summary.as_ref().map(|c| c.name.clone());

        let mut out: Vec<WorkSummary> = Vec::new();
        for w in page.works.iter() {
            let normalized = normalize_title_for_match(&w.title);
            let oo_match = oo_index
                .iter()
                .find(|(t, _)| {
                    t == &normalized || t.contains(&normalized) || normalized.contains(t)
                })
                .map(|(_, oo)| *oo);
            let popular = oo_match.map(|oo| oo.popular).unwrap_or(false);
            let inferred_genre = oo_match
                .and_then(|oo| genre_for_oo_label(oo.genre.as_deref()))
                .or(w.genre);
            let editorial_bucket = self.editorial.lookup_bucket(composer_mbid, &w.title);
            let computed = editorial_bucket.unwrap_or_else(|| {
                buckets::bucket_for(w.work_type, inferred_genre, &[], &w.title)
            });
            if computed != bucket {
                continue;
            }
            out.push(WorkSummary {
                mbid: w.mbid.clone(),
                title: w.title.clone(),
                composer_mbid: Some(composer_mbid.to_string()),
                composer_name: composer_name.clone(),
                catalogue_number: w.catalogue_number.clone(),
                key: w.key.clone(),
                work_type: w.work_type,
                genre: inferred_genre,
                bucket: Some(computed),
                composition_year: None,
                popular,
            });
        }
        Ok(out)
    }
}

/// Validate a favorite `kind` against the allowed set. Anything else is
/// rejected at the catalog boundary so the stats DB never accumulates
/// arbitrary kinds.
fn is_valid_favorite_kind(kind: &str) -> bool {
    matches!(kind, "work" | "recording" | "composer" | "performer")
}

/// Phase 6 — flat list + grouped view for the conductor / orchestra
/// landing page. The frontend consumes both: the grouped view drives
/// the "5 versions of Beethoven 9" tiles; the flat entries list backs
/// Phase 7 (D-029) — paginated response for the "All works of a
/// composer" list. Carries `total` so the UI knows whether more pages
/// exist; `has_more` is convenience derived from `offset + len() < total`.
///
/// D-047 (Phase 8.9 / A4) adds `next_offset`: the offset for the
/// next MB browse call. Critically this is **MB-pre-filter** — the
/// frontend used to pass `works.length` (post D-028 movement filter)
/// which silently overlapped with already-loaded entries on big
/// catalogues like Bach (~1100 works). With `next_offset = offset +
/// mb_works.len()`, every "Load more" advances the cursor against
/// MB's own pagination and yields strictly new entries.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposerWorksPage {
    pub works: Vec<WorkSummary>,
    /// MB's reported total (pre-movement-filter; slight over-estimate
    /// vs. the union of all paginated entries).
    pub total: u32,
    pub offset: u32,
    pub has_more: bool,
    /// D-047 — MB-pre-filter cursor for the next page. Frontend
    /// passes this back verbatim to `list_works_by_composer`.
    pub next_offset: u32,
}

// ---------------------------------------------------------------------------
// Phase 9 (B9.3 / B9.4 / D-040 / D-041) — bucketed composer payloads.
// ---------------------------------------------------------------------------

/// Phase 9 (B9.3) — full bucketed view of a composer's catalogue. The
/// frontend `ComposerWorksTab` consumes this directly and renders one
/// `BucketSection` per non-empty entry in `buckets`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposerBuckets {
    pub composer_mbid: String,
    pub buckets: Vec<BucketSummary>,
    /// Total works across all buckets (post-movement-filter, post-bucket
    /// assignment). Used by the page header "N works".
    pub total_works: u32,
    /// MB's reported total before the movement / parent filter ran.
    /// Useful for "showing N of ~M" copy.
    pub mb_total: u32,
    /// Number of works actually loaded into the bucketing pass — equals
    /// `total_works` after a successful multi-page browse, less when MB
    /// returned errors mid-pagination.
    pub canonical_works_loaded: u32,
}

/// Phase 9 (B9.3) — one bucket inside a `ComposerBuckets` response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketSummary {
    pub bucket: WorkBucket,
    /// English label, ready to render. Pre-resolved server-side so
    /// the frontend can display without a lookup.
    pub label_en: String,
    /// Spanish label, paired with `label_en`.
    pub label_es: String,
    /// Number of works in this bucket — may exceed `top_works.len()`.
    /// When > 12, the UI shows a "View all (N)" affordance.
    pub total_count: u32,
    /// First-12 picks ordered `(popular desc, catalogue asc, title asc)`.
    pub top_works: Vec<WorkSummary>,
    /// Sub-bucket breakdown, populated only when `total_count > 12`.
    /// `None` when the bucket renders flat (≤ 12 entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_buckets: Option<Vec<SubBucketSummary>>,
}

/// Phase 9 (B9.3) — a sub-bucket inside a parent bucket. Frontend
/// renders these as filter chips above the bucket grid: clicking a
/// chip filters `top_works` client-side; the drill-down page
/// (`BrowseComposerBucket`) accepts the same label as a query param.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubBucketSummary {
    /// Internal label — also the query-param value. e.g. "Piano",
    /// "Quartets", "Études".
    pub label: String,
    pub count: u32,
}

/// Phase 9 (B9.4) — drill-down response for a single bucket.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksPage {
    pub works: Vec<WorkSummary>,
    pub total: u32,
    pub offset: u32,
    pub has_more: bool,
}

/// the "all recordings" expander.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDiscography {
    pub artist_mbid: String,
    pub total: u32,
    pub entries: Vec<DiscographyEntry>,
    pub groups: Vec<DiscographyGroup>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscographyEntry {
    pub recording_mbid: String,
    pub title: String,
    pub artist_credit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_mbid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_secs: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub isrcs: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscographyGroup {
    /// `None` → the synthetic "no parent work" bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_mbid: Option<String>,
    pub count: u32,
    /// Indices into `ArtistDiscography.entries`. The frontend expands
    /// this to render the rows belonging to the group.
    pub indices: Vec<u32>,
}

/// Match a recording shell against the seed's `(conductor, performer,
/// year)` triple. Used by the editorial pass to find which row of the
/// recordings list represents the curated Editor's Choice.
fn recording_matches_seed(
    rec: &Recording,
    target_conductor: Option<&str>,
    target_performer_lower: &str,
    target_year: Option<i32>,
) -> bool {
    // Normalise the haystack: conductor name + orchestras + artist credits.
    let mut haystack = String::new();
    if let Some(c) = rec.conductor.as_ref() {
        haystack.push_str(&c.name);
        haystack.push(' ');
    }
    for o in rec.orchestras.iter() {
        haystack.push_str(&o.name);
        haystack.push(' ');
    }
    for a in rec.artist_credits.iter() {
        haystack.push_str(a);
        haystack.push(' ');
    }
    if haystack.is_empty() {
        return false;
    }
    let hay_lower = haystack.to_lowercase();

    // Conductor / performer match: at least one of the two must hit.
    let conductor_hit = target_conductor
        .map(|c| hay_lower.contains(c))
        .unwrap_or(false);
    let performer_hit = !target_performer_lower.is_empty()
        && hay_lower.contains(target_performer_lower);

    // The performer field often contains conductor + ensemble together
    // ("Karajan · Berliner Philharmoniker"); split-match on whitespace
    // tokens for the longest distinctive token.
    let performer_token_hit = if !performer_hit {
        target_performer_lower
            .split_whitespace()
            .filter(|t| t.len() >= 5)
            .any(|t| hay_lower.contains(t))
    } else {
        true
    };

    if !conductor_hit && !performer_token_hit {
        return false;
    }

    // Year tolerance ±2 (or unknown on the recording side — accept).
    if let (Some(ty), Some(ry)) = (target_year, rec.recording_year) {
        if (ry - ty).abs() > 2 {
            return false;
        }
    }
    true
}

/// Helper: fetch quality meta for a Tidal track id, using a per-id
/// cache (TTL 4h, SWR 24h) to avoid re-probing the same track on every
/// `get_work` cold pass.
///
/// Lives outside the `impl` so it can be `tokio::spawn`-ed without
/// borrowing &self across the await boundary.
async fn fetch_or_cache_track_quality(
    cache: &Arc<DiskCache>,
    tidal: &Arc<TidalProvider>,
    track_id: u64,
) -> Option<TrackQualityMeta> {
    let key = format!("{TRACK_QUALITY_CACHE_PREFIX}{track_id}");
    match cache.get(&key, CacheTier::Dynamic).await {
        CacheResult::Fresh(bytes) => {
            if let Ok(meta) = serde_json::from_slice::<TrackQualityMeta>(&bytes) {
                return Some(meta);
            }
        }
        CacheResult::Stale(bytes) => {
            // Return stale immediately; spawn a background refresh so
            // the next page load is fresh. We don't have a join handle,
            // and that's intentional — best-effort.
            let stale = serde_json::from_slice::<TrackQualityMeta>(&bytes).ok();
            let cache_clone = Arc::clone(cache);
            let tidal_clone = Arc::clone(tidal);
            tokio::spawn(async move {
                if let Ok(Some(fresh)) =
                    tidal_clone.fetch_track_quality_meta(track_id).await
                {
                    let bytes = serde_json::to_vec(&fresh).unwrap_or_default();
                    let _ = cache_clone
                        .put(
                            &format!("{TRACK_QUALITY_CACHE_PREFIX}{track_id}"),
                            &bytes,
                            CacheTier::Dynamic,
                            &[TRACK_QUALITY_CACHE_TAG],
                        )
                        .await;
                }
            });
            return stale;
        }
        CacheResult::Miss => {}
    }

    // Cold: fetch live, persist, return.
    match tidal.fetch_track_quality_meta(track_id).await {
        Ok(Some(meta)) => {
            let bytes = serde_json::to_vec(&meta).unwrap_or_default();
            if let Err(e) = cache
                .put(&key, &bytes, CacheTier::Dynamic, &[TRACK_QUALITY_CACHE_TAG])
                .await
            {
                log::debug!("[catalog] cache put track-quality {track_id}: {e}");
            }
            Some(meta)
        }
        Ok(None) => None,
        Err(e) => {
            log::debug!("[catalog] track-quality {track_id}: {e}");
            None
        }
    }
}

/// Lower-case, strip punctuation/double-space variants of a work title so
/// MB titles ("Symphony No. 9 in D minor, Op. 125") and OpenOpus titles
/// ("Symphony no. 9 in D minor, op. 125") produce comparable strings.
fn normalize_title_for_match(title: &str) -> String {
    let mut s = title.to_lowercase();
    s = s.replace(['“', '”'], "\"");
    s.retain(|c| c.is_ascii_alphanumeric() || c.is_whitespace());
    let collapsed: String = s.split_whitespace().collect::<Vec<&str>>().join(" ");
    collapsed
}

/// Sort key for catalogue numbers — system letter then numeric portion
/// padded so "9" < "125". Returns an empty string when none.
fn sort_key_for_catalogue(c: Option<&CatalogueNumber>) -> String {
    let c = match c {
        Some(c) => c,
        None => return String::new(),
    };
    let nums: String = c.number.chars().filter(|c| c.is_ascii_digit()).collect();
    format!("{}-{:0>6}", c.system, nums)
}

// Track-type helper used by the Phase-2 list_works_by_composer logic to
// keep `WorkType` in scope during sort decisions. Kept tiny and local to
// avoid widening the public API.
#[allow(dead_code)]
fn _phase2_marker() -> Option<WorkType> {
    None
}

#[async_trait::async_trait]
impl crate::scrobble::WorkMbidResolver for CatalogService {
    async fn resolve_work_for_recording(
        &self,
        recording_mbid: &str,
    ) -> Option<String> {
        match self.resolve_work_for_recording(recording_mbid).await {
            Ok(opt) => opt,
            Err(e) => {
                log::debug!(
                    "[catalog] resolve work for recording {recording_mbid}: {e}"
                );
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 9 (B9.3 / B9.4) — sub-bucket helpers.
//
// Sub-buckets are *only* materialised when the parent bucket exceeds 12
// entries, mirroring D-039's threshold. The labels are deliberately
// human-friendly (rendered verbatim as filter chips) and stable across
// composers — Concertos always offers Piano/Violin/Cello/Other, never a
// composer-specific palette.
// ---------------------------------------------------------------------------

/// Build the sub-bucket palette for a bucket given its full work list.
/// Returns `None` for buckets where sub-bucketing is not meaningful
/// (Stage, Vocal, Symphonies — whose works are all peers).
fn compute_sub_buckets(
    bucket: WorkBucket,
    works: &[WorkSummary],
) -> Option<Vec<SubBucketSummary>> {
    use std::collections::BTreeMap;
    let palette: &[&str] = match bucket {
        WorkBucket::Concertos => &["Piano", "Violin", "Cello", "Other"],
        WorkBucket::Chamber => {
            &["Quartets", "Trios", "Quintets", "Sonatas", "Other"]
        }
        WorkBucket::Keyboard => {
            &["Sonatas", "Variations", "Études", "Character pieces", "Other"]
        }
        WorkBucket::SoloInstrumental => &["Violin", "Cello", "Other"],
        WorkBucket::ChoralSacred => &["Mass", "Requiem", "Cantata", "Passion", "Other"],
        // Stage / Vocal / Symphonies / Orchestral / FilmTheatre / Other:
        // sub-bucketing wouldn't reduce visual load meaningfully.
        _ => {
            return None;
        }
    };

    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    for label in palette.iter() {
        counts.insert(label, 0);
    }
    for w in works.iter() {
        let label = sub_bucket_for_work(bucket, w);
        // Only count labels in the palette; labels outside go to "Other".
        if palette.contains(&label.as_str()) {
            *counts.entry(palette.iter().find(|p| **p == label.as_str()).unwrap()).or_insert(0) += 1;
        } else if let Some(other) = palette.iter().find(|p| **p == "Other") {
            *counts.entry(*other).or_insert(0) += 1;
        }
    }

    // Drop empty palette entries to avoid rendering noise. Keep input
    // order via `palette.iter()` rather than the BTreeMap's sort.
    let summaries: Vec<SubBucketSummary> = palette
        .iter()
        .filter_map(|p| {
            let c = *counts.get(*p).unwrap_or(&0);
            if c == 0 {
                None
            } else {
                Some(SubBucketSummary {
                    label: (*p).to_string(),
                    count: c,
                })
            }
        })
        .collect();
    if summaries.is_empty() {
        None
    } else {
        Some(summaries)
    }
}

/// Classify a single work into one of the sub-bucket palette labels for
/// its parent bucket. The classification is keyword-based on the
/// title; no MB call required. Returns "Other" when the title carries
/// no signal — the sub-bucket palette always includes "Other".
fn sub_bucket_for_work(bucket: WorkBucket, w: &WorkSummary) -> String {
    let lower = w.title.to_lowercase();
    match bucket {
        WorkBucket::Concertos => {
            if lower.contains("piano concerto") || lower.contains("for piano and orchestra") {
                "Piano".to_string()
            } else if lower.contains("violin concerto")
                || lower.contains("for violin and orchestra")
            {
                "Violin".to_string()
            } else if lower.contains("cello concerto")
                || lower.contains("for cello and orchestra")
            {
                "Cello".to_string()
            } else {
                "Other".to_string()
            }
        }
        WorkBucket::Chamber => {
            if lower.contains("string quartet") || lower.contains("quartet") {
                "Quartets".to_string()
            } else if lower.contains("trio") {
                "Trios".to_string()
            } else if lower.contains("quintet") {
                "Quintets".to_string()
            } else if lower.contains("sonata") {
                "Sonatas".to_string()
            } else {
                "Other".to_string()
            }
        }
        WorkBucket::Keyboard => {
            if lower.contains("sonata") {
                "Sonatas".to_string()
            } else if lower.contains("variation") {
                "Variations".to_string()
            } else if lower.contains("étude") || lower.contains("etude") {
                "Études".to_string()
            } else if lower.contains("nocturne")
                || lower.contains("mazurka")
                || lower.contains("polonaise")
                || lower.contains("ballade")
                || lower.contains("impromptu")
                || lower.contains("prelude")
                || lower.contains("fugue")
            {
                "Character pieces".to_string()
            } else {
                "Other".to_string()
            }
        }
        WorkBucket::SoloInstrumental => {
            if lower.contains("violin") || lower.contains("partita") {
                "Violin".to_string()
            } else if lower.contains("cello") || lower.contains("cello suite") {
                "Cello".to_string()
            } else {
                "Other".to_string()
            }
        }
        WorkBucket::ChoralSacred => {
            if lower.starts_with("mass") || lower.contains(" mass ") || lower.contains("missa") {
                "Mass".to_string()
            } else if lower.contains("requiem") {
                "Requiem".to_string()
            } else if lower.contains("cantata") {
                "Cantata".to_string()
            } else if lower.contains("passion") {
                "Passion".to_string()
            } else {
                "Other".to_string()
            }
        }
        _ => "Other".to_string(),
    }
}

/// Stable serialisation of a `WorkBucket` for use in cache keys.
fn bucket_serialised_for_key(b: WorkBucket) -> &'static str {
    match b {
        WorkBucket::Stage => "Stage",
        WorkBucket::ChoralSacred => "ChoralSacred",
        WorkBucket::Vocal => "Vocal",
        WorkBucket::Symphonies => "Symphonies",
        WorkBucket::Concertos => "Concertos",
        WorkBucket::Orchestral => "Orchestral",
        WorkBucket::Chamber => "Chamber",
        WorkBucket::Keyboard => "Keyboard",
        WorkBucket::SoloInstrumental => "SoloInstrumental",
        WorkBucket::FilmTheatre => "FilmTheatre",
        WorkBucket::Other => "Other",
    }
}
