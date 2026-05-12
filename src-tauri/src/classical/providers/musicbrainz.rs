//! MusicBrainz provider for the Classical Hub.
//!
//! Three responsibilities, in increasing scope:
//!
//!  1. `fetch_work` — given a work MBID, return a fully-fleshed `Work`
//!     entity: title, composer, catalogue number, key, movements,
//!     description placeholder. ONE MB call.
//!
//!  2. `fetch_recordings_for_work` — given a work MBID, return all
//!     recordings linked to it (directly or via child works). The MB
//!     `/recording?work={mbid}&inc=isrcs+artist-credits` browse
//!     endpoint walks both directly-linked and child-linked recordings,
//!     so a single page call captures the catalogue. We paginate up to
//!     `max_recordings` (default 60 in Phase 1; the UI virtualises any
//!     overflow).
//!
//!  3. `fetch_composer` — given an artist MBID, return name, dates,
//!     country. Wikipedia summary is filled by the Wikipedia provider
//!     (separate trait impl).
//!
//! All MB calls go through the shared `MbRateLimiter`. The provider does
//! NOT cache — that's the catalog service's job. Errors carry context.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::super::types::{
    CatalogueNumber, Composer, Era, Genre, LifeEvent, Movement, PerformerCredit,
    PerformerCreditWithRole, Recording, Work, WorkType,
};
use super::{ClassicalProvider, MbRateLimiter};
use crate::SoneError;

const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// Cap initial recording set so cold cache for Beethoven 9 (~200 in MB)
/// doesn't blow past the per-page rate budget. Phase 1 acceptance asks
/// for ≥20 recordings; 60 gives breathing room without N MB detail
/// fetches.
pub const DEFAULT_MAX_RECORDINGS: usize = 60;

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub struct MusicBrainzProvider {
    http: reqwest::Client,
    rate: Arc<MbRateLimiter>,
    user_agent: String,
}

impl MusicBrainzProvider {
    pub fn new(http: reqwest::Client, rate: Arc<MbRateLimiter>) -> Self {
        Self {
            http,
            rate,
            user_agent: format!(
                "SONE-classical/{APP_VERSION} (https://github.com/lullabyX/sone)"
            ),
        }
    }

    /// Replace the HTTP client (called when proxy settings change).
    pub fn set_http_client(&mut self, http: reqwest::Client) {
        self.http = http;
    }

    /// Public alias for `get_json` so the catalog service can issue
    /// one-off lookups (e.g. recording → work resolution) without
    /// duplicating the rate-limited HTTP plumbing.
    pub async fn get_json_pub(&self, url: &str) -> Result<Value, SoneError> {
        self.get_json(url).await
    }

    /// Single GET → JSON with shared MB rate limit + 1× retry on 503.
    async fn get_json(&self, url: &str) -> Result<Value, SoneError> {
        self.rate.acquire().await;

        // D-038 — classify reqwest errors via the From impl so connect
        // failures / TLS EOF / timeouts surface as `NetworkTransient`.
        let resp = self
            .http
            .get(url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                let inner: SoneError = e.into();
                match inner {
                    SoneError::NetworkTransient(s) => {
                        SoneError::NetworkTransient(format!("mb request {url}: {s}"))
                    }
                    SoneError::Network(s) => {
                        SoneError::Network(format!("mb request {url}: {s}"))
                    }
                    other => other,
                }
            })?;

        let status = resp.status();
        if status.as_u16() == 503 {
            log::warn!("[mb] 503 on {url}, retrying once after 5s");
            tokio::time::sleep(Duration::from_secs(5)).await;
            self.rate.acquire().await;
            let retry = self
                .http
                .get(url)
                .header(reqwest::header::USER_AGENT, &self.user_agent)
                .header(reqwest::header::ACCEPT, "application/json")
                .timeout(HTTP_TIMEOUT)
                .send()
                .await
                .map_err(|e| {
                    let inner: SoneError = e.into();
                    match inner {
                        SoneError::NetworkTransient(s) => {
                            SoneError::NetworkTransient(format!("mb retry {url}: {s}"))
                        }
                        SoneError::Network(s) => {
                            SoneError::Network(format!("mb retry {url}: {s}"))
                        }
                        other => other,
                    }
                })?;
            let retry_status = retry.status();
            let retry_body = retry.text().await.unwrap_or_default();
            if !retry_status.is_success() {
                return Err(SoneError::from_http_status(
                    retry_status.as_u16(),
                    format!("mb {retry_status} on retry of {url}: {retry_body}"),
                ));
            }
            return serde_json::from_str(&retry_body)
                .map_err(|e| SoneError::Parse(format!("mb retry json {url}: {e}")));
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SoneError::from_http_status(
                status.as_u16(),
                format!("mb {status} on {url}: {body}"),
            ));
        }

        let body = resp.text().await.map_err(|e| {
            let inner: SoneError = e.into();
            match inner {
                SoneError::NetworkTransient(s) => {
                    SoneError::NetworkTransient(format!("mb body {url}: {s}"))
                }
                SoneError::Network(s) => SoneError::Network(format!("mb body {url}: {s}")),
                other => other,
            }
        })?;
        serde_json::from_str(&body)
            .map_err(|e| SoneError::Parse(format!("mb json {url}: {e}")))
    }

    // ---------------- Work ----------------

    /// Fetch a work and turn the MB JSON into our `Work` shell — no
    /// recordings yet (those come from `fetch_recordings_for_work`).
    pub async fn fetch_work(&self, mbid: &str) -> Result<Work, SoneError> {
        let url = format!(
            "{MB_API_BASE}/work/{mbid}?inc=artist-rels+work-rels+aliases&fmt=json"
        );
        let body = self.get_json(&url).await?;

        let title = body
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                SoneError::Parse(format!("mb work {mbid}: missing title"))
            })?;

        let alternative_titles = body
            .get("aliases")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Composer is in `relations` with `type` == "composer" (or
        // "composer in" for arrangers). We pick the first `composer`.
        let (composer_mbid, composer_name) = body
            .get("relations")
            .and_then(|v| v.as_array())
            .and_then(|rels| {
                rels.iter().find_map(|rel| {
                    let kind = rel.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if !kind.eq_ignore_ascii_case("composer") {
                        return None;
                    }
                    let artist = rel.get("artist")?;
                    let id =
                        artist.get("id").and_then(|s| s.as_str()).map(String::from);
                    let name = artist
                        .get("name")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    Some((id, name))
                })
            })
            .unwrap_or((None, None));

        // Movements: child works linked via 'parts' relation (backward).
        // MB exposes them in the `relations` array with type="parts" and
        // direction="backward" — meaning "this work is parts of X" is
        // forward, and "X has these parts" is backward (child-of).
        let mut movements: Vec<Movement> = body
            .get("relations")
            .and_then(|v| v.as_array())
            .map(|rels| {
                rels.iter()
                    .filter(|rel| {
                        let kind =
                            rel.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        let dir = rel
                            .get("direction")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        kind.eq_ignore_ascii_case("parts") && dir == "backward"
                    })
                    .filter_map(|rel| {
                        let work = rel.get("work")?;
                        let mbid = work
                            .get("id")
                            .and_then(|s| s.as_str())
                            .map(String::from)?;
                        let title = work
                            .get("title")
                            .and_then(|s| s.as_str())
                            .map(String::from)
                            .unwrap_or_default();
                        let order = rel
                            .get("ordering-key")
                            .and_then(|n| n.as_u64())
                            .map(|n| n as u32);
                        Some((order.unwrap_or(0), mbid, title))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(idx, (order, mbid, title))| Movement {
                mbid,
                index: if order > 0 { order } else { (idx + 1) as u32 },
                title,
                duration_approx_secs: None,
                attacca_to: None,
            })
            .collect();
        // Stable order by `index` (MB ordering keys).
        movements.sort_by_key(|m| m.index);

        let catalogue_number = parse_catalogue_number(&title);
        let key = parse_key_from_title(&title);
        let work_type = parse_work_type_from_title(&title);

        Ok(Work {
            mbid: mbid.to_string(),
            qid: None,
            title,
            composer_mbid,
            composer_name,
            alternative_titles,
            catalogue_number,
            key,
            genre: None,
            work_type,
            // Phase 9 (D-040) — bucket is computed by
            // `catalog::build_work_fresh` after the MB skeleton is
            // hydrated; the provider leaves it `None` so the
            // catalog stays the single source of truth for bucket
            // assignment (snapshot override + heuristic cascade).
            bucket: None,
            composition_year: None,
            premiere_year: None,
            duration_approx_secs: None,
            movements,
            description: None,
            description_source_url: None,
            recordings: Vec::new(),
            recording_count: 0,
            best_available_quality: None,
            editor_note: None,
            tidal_unavailable: false,
        })
    }

    /// Fetch recordings linked to a work (parent + children) in a single
    /// browse call. Returns recordings in MB's natural order — usually
    /// editor-curation density, not popularity. Phase 4 will sort by
    /// popularity heuristic; Phase 1 accepts MB's order.
    pub async fn fetch_recordings_for_work(
        &self,
        work_mbid: &str,
        max: usize,
    ) -> Result<Vec<Recording>, SoneError> {
        let limit = max.min(100); // MB caps per-page at 100
        let url = format!(
            "{MB_API_BASE}/recording?work={work_mbid}&inc=isrcs+artist-credits&fmt=json&limit={limit}"
        );
        let body = self.get_json(&url).await?;

        let arr = match body.get("recordings").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return Ok(Vec::new()),
        };

        let mut out: Vec<Recording> = Vec::with_capacity(arr.len());
        for rec in arr.iter() {
            let mbid = match rec.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let title = rec
                .get("title")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            let isrcs = rec
                .get("isrcs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let artist_credits = rec
                .get("artist-credit")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| {
                            c.get("name")
                                .or_else(|| c.get("artist").and_then(|a| a.get("name")))
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string())
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // Earliest release date as proxy for recording_year + label.
            let (recording_year, recording_date, label) = rec
                .get("releases")
                .and_then(|v| v.as_array())
                .map(|releases| earliest_release_meta(releases))
                .unwrap_or((None, None, None));

            let duration_secs = rec
                .get("length")
                .and_then(|v| v.as_u64())
                .map(|ms| (ms / 1000) as u32);

            let mut shell = Recording::shell(&mbid, work_mbid);
            shell.title = title;
            shell.isrcs = isrcs;
            shell.artist_credits = artist_credits;
            shell.recording_year = recording_year;
            shell.recording_date = recording_date;
            shell.label = label;
            shell.duration_secs = duration_secs;
            out.push(shell);

            if out.len() >= max {
                break;
            }
        }

        Ok(out)
    }

    /// Fetch full recording detail with artist-rels — used by the
    /// `enrich_recording` path to resolve conductor/orchestra/soloists.
    /// Optional in Phase 1: lazy-on-hover (not invoked from list view).
    pub async fn fetch_recording_detail(
        &self,
        mbid: &str,
    ) -> Result<RecordingDetail, SoneError> {
        let url = format!(
            "{MB_API_BASE}/recording/{mbid}?inc=artist-rels+isrcs+releases&fmt=json"
        );
        let body = self.get_json(&url).await?;

        let mut conductor: Option<PerformerCredit> = None;
        let mut orchestras: Vec<PerformerCredit> = Vec::new();
        let mut soloists: Vec<PerformerCreditWithRole> = Vec::new();
        let mut choir: Option<PerformerCredit> = None;

        if let Some(rels) = body.get("relations").and_then(|v| v.as_array()) {
            for rel in rels.iter() {
                let kind =
                    rel.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let artist = match rel.get("artist") {
                    Some(a) => a,
                    None => continue,
                };
                let name = match artist.get("name").and_then(|n| n.as_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let mbid = artist
                    .get("id")
                    .and_then(|i| i.as_str())
                    .map(String::from);
                let entity_kind = artist
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Person");

                match kind {
                    "conductor" if conductor.is_none() => {
                        conductor = Some(PerformerCredit {
                            mbid,
                            name,
                            kind: lower(entity_kind),
                        });
                    }
                    "orchestra" | "performing orchestra" => {
                        orchestras.push(PerformerCredit {
                            mbid,
                            name,
                            kind: lower(entity_kind),
                        });
                    }
                    "chorus master" | "choir" | "choir vocals"
                        if choir.is_none() =>
                    {
                        choir = Some(PerformerCredit {
                            mbid,
                            name,
                            kind: lower(entity_kind),
                        });
                    }
                    "vocal" | "instrument" | "performer" => {
                        let attrs: Vec<String> = rel
                            .get("attributes")
                            .and_then(|a| a.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|x| {
                                        x.as_str().map(|s| s.to_string())
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        let role = attrs.first().cloned().unwrap_or_else(|| {
                            kind.to_string()
                        });
                        soloists.push(PerformerCreditWithRole {
                            mbid,
                            name,
                            kind: lower(entity_kind),
                            role,
                            instrument_mbid: None,
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(RecordingDetail {
            conductor,
            orchestras,
            soloists,
            choir,
        })
    }

    /// Phase 2 + 7: list works composed by a given artist MBID.
    ///
    /// **Phase 7 (D-028)** — adds `inc=work-rels` and filters out child
    /// works (works with a `parts` rel pointing backward to a parent
    /// work). This closes the Tchaikovsky-Pathétique bug where MB's
    /// alphabetical ordering surfaces "III. Adagio lamentoso" as a
    /// top-level entry instead of the parent symphony.
    ///
    /// **Phase 7 (D-029)** — adds `offset` for pagination. Bach (>1000
    /// works) and Mozart (>600) need multiple pages; the response also
    /// carries `work-count` so the UI knows whether more pages exist.
    ///
    /// The cap stays at 100 per request (MB's per-page max). For larger
    /// totals, the caller invokes successive offsets.
    pub async fn browse_works_by_artist(
        &self,
        artist_mbid: &str,
        limit: usize,
        offset: u32,
    ) -> Result<MbBrowsedWorksPage, SoneError> {
        let limit = limit.min(100);
        let url = format!(
            "{MB_API_BASE}/work?artist={artist_mbid}\
             &inc=aliases+work-rels&fmt=json&limit={limit}&offset={offset}"
        );
        let body = self.get_json(&url).await?;

        let total = body
            .get("work-count")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);

        let arr = match body.get("works").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => {
                return Ok(MbBrowsedWorksPage {
                    works: Vec::new(),
                    total,
                    offset,
                });
            }
        };

        let mut out: Vec<MbBrowsedWork> = Vec::with_capacity(arr.len());
        for w in arr.iter() {
            let mbid = match w.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let title = match w.get("title").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            // D-028 — discard child works (movements) by inspecting the
            // `relations[]` array. A work is a child when it has a relation
            // with `type=parts` and `direction=backward` — meaning "this
            // work is part of another, larger work". The parent work in
            // the same response is what we want; the children we drop.
            if work_is_child_movement(w) {
                continue;
            }

            // D-048 (Phase 8.9 / A5) — secondary defensive filter:
            // some MB works expose no `parts` rel (because no editor
            // has linked the parent, or the work itself is a freshly-
            // ingested movement) and would slip past the structural
            // check above. We drop titles that obviously read as
            // movement labels (`I. Allegro`, `IV. Presto`,
            // `VIII. Andante mosso`). Standalone titles like
            // "Andante in C major" or "Andantino" are intentionally
            // kept because they're real top-level works, not movements.
            if title_looks_like_movement(&title) {
                log::debug!("[mb] dropping movement-like title: {}", title);
                continue;
            }

            // MB exposes `type` on works (e.g. "Symphony", "Sonata") for
            // classical workshops. Otherwise we fall through to the title
            // parser, which already handles the most common labels.
            let work_type = w
                .get("type")
                .and_then(|v| v.as_str())
                .and_then(work_type_from_mb_label)
                .or_else(|| parse_work_type_from_title(&title));
            let catalogue_number = parse_catalogue_number(&title);
            let key = parse_key_from_title(&title);
            // MB does not classify works into our Genre buckets directly;
            // we leave it empty here and let the snapshot fill it.
            let genre: Option<Genre> = None;

            out.push(MbBrowsedWork {
                mbid,
                title,
                catalogue_number,
                key,
                work_type,
                genre,
            });
        }

        Ok(MbBrowsedWorksPage {
            works: out,
            total,
            offset,
        })
    }

    /// Phase 9 (B9.5 / D-041) — multi-page MB browse fetcher.
    ///
    /// Iterates `browse_works_by_artist` with offset stepping until
    /// either MB's reported `work-count` is exhausted or the cap of
    /// `MAX_BROWSE_PAGES` pages is reached, whichever comes first.
    /// Each page is requested serially because `MbRateLimiter` already
    /// enforces 1 req/s globally — parallelising here would just queue
    /// behind the limiter and waste sockets.
    ///
    /// Cost (cold cache):
    ///   * Bach (~1100 works) → 11 pages × ~1.05s ≈ 11s.
    ///   * Mozart (~620 works) → 7 pages ≈ 7s.
    ///   * Beethoven (~330 works) → 4 pages ≈ 4s.
    ///   * Rest (< 200 works) → 1-2 pages ≈ 1-2s.
    ///
    /// `MAX_BROWSE_PAGES = 20` caps the absolute worst case at 20s for
    /// composers with very long catalogues. The `total` reported in the
    /// returned `MbBrowsedWorksPage` is the value MB sent on the first
    /// page (so callers can render "showing N of M" honestly).
    pub async fn browse_all_works_by_artist(
        &self,
        artist_mbid: &str,
    ) -> Result<MbBrowsedWorksPage, SoneError> {
        const PAGE_SIZE: usize = 100;
        const MAX_BROWSE_PAGES: u32 = 20;

        let mut offset: u32 = 0;
        let mut total_reported: u32 = 0;
        let mut all_works: Vec<MbBrowsedWork> = Vec::new();
        let mut pages_done: u32 = 0;

        loop {
            let page = self
                .browse_works_by_artist(artist_mbid, PAGE_SIZE, offset)
                .await?;
            // Capture total from the first page (MB sometimes drifts
            // between pages; first response is canonical).
            if pages_done == 0 {
                total_reported = page.total;
            }
            let returned = page.works.len() as u32;
            all_works.extend(page.works);
            pages_done += 1;
            offset = offset.saturating_add(PAGE_SIZE as u32);

            if returned == 0 {
                break;
            }
            if total_reported > 0 && offset >= total_reported {
                break;
            }
            if pages_done >= MAX_BROWSE_PAGES {
                log::warn!(
                    "[mb] browse_all hit cap of {MAX_BROWSE_PAGES} pages for {artist_mbid} \
                     (total={total_reported}, collected={})",
                    all_works.len()
                );
                break;
            }
        }

        Ok(MbBrowsedWorksPage {
            works: all_works,
            total: total_reported,
            offset: 0,
        })
    }

    /// Phase 6 (B6.6): list recordings credited to an artist (any role).
    /// Returns a lightweight projection — the caller filters by role
    /// (conductor / orchestra / soloist) downstream because MB doesn't
    /// expose role-filtered browse endpoints. We pull `artist-credits`
    /// plus `work-rels` so we can both label the row and link to the
    /// parent Work.
    ///
    /// The intended caller is the "Browse by conductor" landing page
    /// (e.g. all recordings credited to Karajan). Capped at `limit` (≤
    /// 100, MB max) — the UI paginates if the user wants more.
    pub async fn browse_recordings_by_artist(
        &self,
        artist_mbid: &str,
        limit: usize,
    ) -> Result<Vec<MbArtistRecording>, SoneError> {
        let limit = limit.min(100);
        let url = format!(
            "{MB_API_BASE}/recording?artist={artist_mbid}&inc=artist-credits+work-rels&fmt=json&limit={limit}"
        );
        let body = self.get_json(&url).await?;

        let arr = match body.get("recordings").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return Ok(Vec::new()),
        };

        let mut out: Vec<MbArtistRecording> = Vec::with_capacity(arr.len());
        for r in arr.iter() {
            let mbid = match r.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let title = r
                .get("title")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_default();
            if title.is_empty() {
                continue;
            }

            // Extract a flattened artist-credit string the UI can show as
            // "Karajan / BPO" without us having to render an array.
            let mut artist_credit = String::new();
            if let Some(arr) = r.get("artist-credit").and_then(|v| v.as_array()) {
                for ac in arr.iter() {
                    if let Some(name) = ac.get("name").and_then(|v| v.as_str()) {
                        if !artist_credit.is_empty() {
                            artist_credit.push_str(" / ");
                        }
                        artist_credit.push_str(name);
                    } else if let Some(artist) = ac.get("artist") {
                        if let Some(n) = artist.get("name").and_then(|v| v.as_str()) {
                            if !artist_credit.is_empty() {
                                artist_credit.push_str(" / ");
                            }
                            artist_credit.push_str(n);
                        }
                    }
                }
            }

            // The first parent-work (if any) lets the UI link to a Work
            // page from the artist's discography landing.
            let work_mbid = r
                .get("relations")
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    arr.iter().find_map(|rel| {
                        let kind = rel.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if !kind.eq_ignore_ascii_case("performance") {
                            return None;
                        }
                        rel.get("work")
                            .and_then(|w| w.get("id"))
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    })
                });

            // Year is best-effort from the first release date.
            let release_year = r
                .get("releases")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|rel| rel.get("date").and_then(|v| v.as_str()))
                .and_then(|date| date.get(..4))
                .and_then(|y| y.parse::<i32>().ok());

            let length_secs = r
                .get("length")
                .and_then(|v| v.as_u64())
                .map(|ms| (ms / 1000) as u32);

            let isrcs: Vec<String> = r
                .get("isrcs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            out.push(MbArtistRecording {
                mbid,
                title,
                artist_credit,
                work_mbid,
                release_year,
                length_secs,
                isrcs,
            });
        }
        Ok(out)
    }

    /// Fetch a composer (MB artist) for the Hub composer page. Includes
    /// `url-rels` so we can extract a Wikidata QID in the same call —
    /// zero extra rate-limit cost vs. a follow-up artist?inc=url-rels.
    pub async fn fetch_composer(&self, mbid: &str) -> Result<Composer, SoneError> {
        let url = format!("{MB_API_BASE}/artist/{mbid}?inc=url-rels&fmt=json");
        let body = self.get_json(&url).await?;

        let name = body
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| {
                SoneError::Parse(format!("mb artist {mbid}: missing name"))
            })?;

        let full_name = body
            .get("sort-name")
            .and_then(|v| v.as_str())
            .map(String::from);

        let birth = parse_life_event(body.get("life-span"), "begin");
        let death = parse_life_event(body.get("life-span"), "end");

        let era = match (
            birth.as_ref().and_then(|b| b.year),
            death.as_ref().and_then(|d| d.year),
        ) {
            (Some(b), _) => year_to_era(b),
            _ => Era::Unknown,
        };

        // Extract Wikidata QID from the `url-rels` block, if present.
        // MB stores it as a relation of type "wikidata" pointing to a
        // Wikidata URL we strip down to the bare Q-id.
        let qid = parse_wikidata_qid(body.get("relations"));

        Ok(Composer {
            mbid: mbid.to_string(),
            qid,
            open_opus_id: None,
            name,
            full_name,
            birth,
            death,
            era,
            portrait_url: None,
            bio_short: None,
            bio_long: None,
            bio_source_url: None,
            editor_note: None,
            related_composers: Vec::new(),
        })
    }
}

/// Pull a Wikidata QID (e.g. `Q255`) out of the MB url-rels array on an
/// artist payload. Returns the first match — there is at most one per
/// artist by MB convention.
fn parse_wikidata_qid(rels: Option<&Value>) -> Option<String> {
    let arr = rels.and_then(|v| v.as_array())?;
    for rel in arr.iter() {
        let kind = rel.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !kind.eq_ignore_ascii_case("wikidata") {
            continue;
        }
        let url = rel
            .get("url")
            .and_then(|u| u.get("resource"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if let Some(qid) = url.rsplit('/').next() {
            if qid.starts_with('Q') && qid[1..].chars().all(|c| c.is_ascii_digit()) {
                return Some(qid.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct RecordingDetail {
    pub conductor: Option<PerformerCredit>,
    pub orchestras: Vec<PerformerCredit>,
    pub soloists: Vec<PerformerCreditWithRole>,
    pub choir: Option<PerformerCredit>,
}

/// Lightweight work descriptor returned by `browse_works_by_artist`. The
/// catalog upstream merges this with the OpenOpus snapshot to produce
/// `WorkSummary` entities for the Composer page.
#[derive(Debug, Clone)]
pub struct MbBrowsedWork {
    pub mbid: String,
    pub title: String,
    pub catalogue_number: Option<CatalogueNumber>,
    pub key: Option<String>,
    pub work_type: Option<WorkType>,
    pub genre: Option<Genre>,
}

/// Phase 7 (D-029) — paged response of `browse_works_by_artist`. Carries
/// the requested offset + total count so the caller (CatalogService /
/// frontend) can render "Load more" affordances.
#[derive(Debug, Clone)]
pub struct MbBrowsedWorksPage {
    pub works: Vec<MbBrowsedWork>,
    /// Total number of parent works available for this artist (post-
    /// movement-filter, so this may differ from MB's `work-count`).
    /// Used by the frontend to decide whether to show "Load more".
    pub total: u32,
    pub offset: u32,
}

/// Phase 6 — lightweight projection of a recording from the
/// `browse_recordings_by_artist` browse call. The conductor /
/// orchestra page uses this to render a flat list across MB's "all
/// recordings credited to this artist" view.
#[derive(Debug, Clone)]
pub struct MbArtistRecording {
    pub mbid: String,
    pub title: String,
    /// Flat artist-credit string ("Karajan / BPO"). We deliberately
    /// keep this as a single string at the provider boundary; the
    /// catalog can split into structured fields if/when needed.
    pub artist_credit: String,
    /// Parent Work MBID via the first `performance` rel, if any. Lets
    /// the UI deep-link a row → Work page.
    pub work_mbid: Option<String>,
    pub release_year: Option<i32>,
    pub length_secs: Option<u32>,
    pub isrcs: Vec<String>,
}

/// Map MB's `type` field on a work entity → our `WorkType`. MB's type
/// vocabulary is small but inconsistent in Title Case; we normalise.
fn work_type_from_mb_label(label: &str) -> Option<WorkType> {
    match label.to_lowercase().as_str() {
        "symphony" => Some(WorkType::Symphony),
        "concerto" => Some(WorkType::Concerto),
        "sonata" => Some(WorkType::Sonata),
        "string quartet" | "quartet" => Some(WorkType::StringQuartet),
        "opera" => Some(WorkType::Opera),
        "cantata" => Some(WorkType::Cantata),
        "mass" | "requiem" => Some(WorkType::Mass),
        "lied" | "lieder" | "song" | "song cycle" => Some(WorkType::Lieder),
        "suite" | "variations" => Some(WorkType::Suite),
        "étude" | "etude" => Some(WorkType::Etude),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Trait impl
// ---------------------------------------------------------------------------

#[async_trait]
impl ClassicalProvider for MusicBrainzProvider {
    fn name(&self) -> &'static str {
        "musicbrainz"
    }

    async fn enrich_work(&self, w: &mut Work) -> Result<(), SoneError> {
        if !w.title.is_empty() {
            return Ok(());
        }
        let fetched = self.fetch_work(&w.mbid).await?;
        *w = fetched;
        Ok(())
    }

    async fn enrich_composer(&self, c: &mut Composer) -> Result<(), SoneError> {
        if !c.name.is_empty() {
            return Ok(());
        }
        let fetched = self.fetch_composer(&c.mbid).await?;
        *c = fetched;
        Ok(())
    }

    async fn enrich_recording(&self, r: &mut Recording) -> Result<(), SoneError> {
        // Only resolve conductor/orchestra/soloists if we don't have them.
        if r.conductor.is_some() || !r.orchestras.is_empty() {
            return Ok(());
        }
        let detail = self.fetch_recording_detail(&r.mbid).await?;
        r.conductor = detail.conductor;
        r.orchestras = detail.orchestras;
        r.soloists = detail.soloists;
        r.choir = detail.choir;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers (pure, easy to unit test)
// ---------------------------------------------------------------------------

fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// Phase 7 (D-028) — true iff the MB work entity is a child movement of
/// another work (e.g. "III. Adagio lamentoso" of Tchaikovsky's Pathétique).
///
/// MB models this via the `parts` work-work relationship. A child has a
/// rel with `type = "parts"` and `direction = "backward"` — the rel
/// reads as "this work is part of <parent>". The parent has the same
/// rel `type` but with `direction = "forward"`, which we keep.
///
/// Returns `false` for standalone works (no relations or only forward
/// `parts` rels — i.e. the work has its own movements but isn't itself
/// a child).
fn work_is_child_movement(w: &Value) -> bool {
    let relations = match w.get("relations").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return false,
    };
    for rel in relations.iter() {
        let rel_type = rel.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let direction = rel.get("direction").and_then(|v| v.as_str()).unwrap_or("");
        if rel_type == "parts" && direction == "backward" {
            // This work is a child of another. Drop it.
            return true;
        }
    }
    false
}

/// Heuristic title-only filter for child / sub-work entries that the
/// structural `work_is_child_movement` check fails to catch (because
/// MB browse-by-artist with `inc=work-rels` only returns artist↔work
/// relations, not the work↔work `parts` relations we'd need).
///
/// Recognises four families:
///
///   1. **Roman-numeral prefix** — `^[IVX]{1,4}\s*\.\s+\S`.
///      Examples: "I. Allegro", "IV. Presto", "VIII. Andante mosso".
///   2. **Taken-from prefix** — provenance phrase such as
///      " aus der ", " aus dem ", " from the opera ", " from \"".
///      Example: `"Mohrentanz" aus der Zauberflöte`.
///   3. **Colon-suffix marker** — `<parent>: <suffix>` where
///      `<suffix>` starts with `No. N`, `Nr. N`, `N° N`, a roman
///      numeral, an arabic + dot, or a tempo / form word (Allegro,
///      Aria, Variation, Prélude, Overture, etc.).
///   4. **Universal colon-space rule** — any title containing `: `
///      (colon followed by space) is treated as a child. MB's
///      convention for child works of every kind (operatic arias,
///      suite movements, ballet acts, song-cycle entries) is to
///      prefix the parent work title before a colon. The Hob.
///      catalogue notation `XVI:50` does NOT have a space and is
///      unaffected. This rule supersedes case (3) but the named
///      patterns are still useful for tests / clarity.
///
/// The cost is rare false positives — a tiny set of legitimate
/// top-level works whose MB titles include `: ` (subtitled
/// collections like `The Well-Tempered Clavier: Book 1`) get
/// dropped. We accept this trade because the alternative is letting
/// hundreds of arias and movements leak into composer catalogues.
///
/// Returns `false` for legitimate top-level works without these
/// markers: "Andante in C major", "Symphony No. 1 in C",
/// "Variations on a Theme by Haydn, Op. 56a", "Don Giovanni, K. 527".
pub(crate) fn title_looks_like_movement(title: &str) -> bool {
    if title.is_empty() {
        return false;
    }

    // (1) Roman-numeral dot-prefix.
    if has_roman_dot_prefix(title.as_bytes()) {
        return true;
    }

    let lower = title.to_ascii_lowercase();

    // (2) Provenance phrases — "X aus der Y", "X from the opera Y", etc.
    const TAKEN_FROM: &[&str] = &[
        " aus der ",
        " aus dem ",
        " aus den ",
        " from the opera ",
        " from the symphony ",
        " from the ballet ",
        " from \"",
        " from '",
    ];
    for needle in TAKEN_FROM {
        if lower.contains(needle) {
            return true;
        }
    }

    // (3) Colon-suffix marker (kept for clarity, though (4) below
    // already covers it; useful so the named markers still count
    // even on titles that lack the trailing space).
    if let Some(idx) = title.find(':') {
        let suffix = title[idx + 1..].trim_start();
        if !suffix.is_empty() && suffix_is_movement_marker(suffix) {
            return true;
        }
    }

    // (4) Universal `: ` rule — colon followed by a space is the MB
    // parent/child separator. Hob. cat# uses `XVI:50` (no space) so
    // it is intentionally untouched.
    if title.contains(": ") {
        return true;
    }

    false
}

/// Bytes-walk parser for the `^[IVX]{1,4}\s*\.\s+\S` pattern.
fn has_roman_dot_prefix(bytes: &[u8]) -> bool {
    let mut idx = 0;
    while idx < bytes.len() && idx < 4 && matches!(bytes[idx], b'I' | b'V' | b'X') {
        idx += 1;
    }
    if idx == 0 {
        return false;
    }

    while idx < bytes.len() && bytes[idx] == b' ' {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b'.' {
        return false;
    }
    idx += 1;

    let mut saw_ws = false;
    while idx < bytes.len() && bytes[idx] == b' ' {
        idx += 1;
        saw_ws = true;
    }
    if !saw_ws {
        return false;
    }
    if idx >= bytes.len() || bytes[idx] == b' ' {
        return false;
    }
    true
}

/// Suffix (after the parent's `:`) reads as a movement marker.
fn suffix_is_movement_marker(suffix: &str) -> bool {
    let bytes = suffix.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    // Roman-numeral dot-prefix in the suffix: "I. Allegro".
    if has_roman_dot_prefix(bytes) {
        return true;
    }

    // Arabic number then dot: "1. Prélude", "14. Variation".
    if bytes[0].is_ascii_digit() {
        let mut p = 0;
        while p < bytes.len() && bytes[p].is_ascii_digit() {
            p += 1;
        }
        if p < bytes.len() && bytes[p] == b'.' {
            return true;
        }
    }

    let lower = suffix.to_ascii_lowercase();

    // "No. 3", "Nr. 3", "N° 3" — number-prefix tokens.
    const NUM_PREFIXES: &[&str] = &["no.", "no ", "nr.", "nr ", "n°", "no:", "nº"];
    for p in NUM_PREFIXES {
        if let Some(rest) = lower.strip_prefix(p) {
            let rest = rest.trim_start();
            if rest.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
                return true;
            }
        }
    }

    // Tempo / form / dance markers as the leading word of the suffix.
    const MARKERS: &[&str] = &[
        "allegro",
        "allegretto",
        "adagio",
        "adagietto",
        "andante",
        "andantino",
        "presto",
        "prestissimo",
        "largo",
        "larghetto",
        "lento",
        "vivace",
        "vivacissimo",
        "moderato",
        "grave",
        "maestoso",
        "menuetto",
        "minuet",
        "scherzo",
        "scherzando",
        "rondo",
        "finale",
        "aria",
        "arioso",
        "recitativo",
        "recitative",
        "cavatina",
        "cabaletta",
        "coro",
        "chorus",
        "duetto",
        "duet",
        "trio",
        "quartetto",
        "ensemble",
        "variation",
        "variazione",
        "variations",
        "march",
        "marcia",
        "prélude",
        "preludio",
        "prelude",
        "fugue",
        "fuga",
        "toccata",
        "cadenza",
        "coda",
        "ouverture",
        "overture",
        "sinfonia",
        "ritornello",
        "intermezzo",
        "interlude",
        "act",
        "scene",
        "tableau",
        "movement",
    ];
    for m in MARKERS {
        if lower.starts_with(m) {
            let next = lower.as_bytes().get(m.len());
            match next {
                None => return true,
                Some(b) if !b.is_ascii_alphabetic() => return true,
                _ => {}
            }
        }
    }

    false
}

/// Pick the earliest release entry's date and label. MB returns release
/// dates as ISO `YYYY-MM-DD` (sometimes `YYYY` or `YYYY-MM`).
fn earliest_release_meta(
    releases: &[Value],
) -> (Option<i32>, Option<String>, Option<String>) {
    let mut best_date: Option<String> = None;
    let mut best_year: Option<i32> = None;
    let mut best_label: Option<String> = None;

    for r in releases.iter() {
        let date = r.get("date").and_then(|d| d.as_str()).map(String::from);
        let year = date.as_deref().and_then(|s| s.get(0..4)).and_then(|s| s.parse::<i32>().ok());
        let label = r
            .get("label-info")
            .and_then(|l| l.as_array())
            .and_then(|arr| arr.first())
            .and_then(|li| li.get("label"))
            .and_then(|l| l.get("name"))
            .and_then(|n| n.as_str())
            .map(String::from);

        match (year, best_year) {
            (Some(y), None) => {
                best_year = Some(y);
                best_date = date.clone();
                best_label = label;
            }
            (Some(y), Some(by)) if y < by => {
                best_year = Some(y);
                best_date = date.clone();
                best_label = label;
            }
            _ => {
                if best_label.is_none() {
                    best_label = label;
                }
            }
        }
    }
    (best_year, best_date, best_label)
}

fn parse_life_event(life_span: Option<&Value>, field: &str) -> Option<LifeEvent> {
    let span = life_span?;
    let date = span.get(field).and_then(|d| d.as_str())?;
    if date.is_empty() {
        return None;
    }
    let year = date.get(0..4).and_then(|s| s.parse::<i32>().ok());
    Some(LifeEvent {
        year,
        date: Some(date.to_string()),
        place: None,
    })
}

fn year_to_era(birth_year: i32) -> Era {
    match birth_year {
        ..=1399 => Era::Medieval,
        1400..=1599 => Era::Renaissance,
        1600..=1749 => Era::Baroque,
        1750..=1799 => Era::Classical,
        1800..=1849 => Era::EarlyRomantic,
        1850..=1899 => Era::Romantic,
        1900..=1929 => Era::TwentiethCentury,
        1930..=1959 => Era::PostWar,
        1960..=2025 => Era::Contemporary,
        _ => Era::Unknown,
    }
}

/// Recognise BWV 1052, K. 466, K.626, Op. 125, RV 580, D. 911, Hob.
/// XVI:50, HWV 56, etc., embedded inside a work title. Falls back to
/// None when the catalogue notation isn't present.
pub fn parse_catalogue_number(title: &str) -> Option<CatalogueNumber> {
    // Order matters: more specific systems before generic Op.
    const SYSTEMS: &[(&str, &str)] = &[
        ("BWV", r"\bBWV\s*\.?\s*([0-9]+[a-zA-Z]?)"),
        ("K", r"\bK\s*\.?\s*([0-9]+[a-zA-Z]?)"),
        ("D", r"\bD\s*\.?\s*([0-9]+[a-zA-Z]?)"),
        ("RV", r"\bRV\s*\.?\s*([0-9]+[a-zA-Z]?)"),
        ("Hob", r"\bHob\s*\.?\s*([IVXLCDM]+:[0-9]+[a-zA-Z]?)"),
        ("HWV", r"\bHWV\s*\.?\s*([0-9]+[a-zA-Z]?)"),
        ("Op", r"\bOp\s*\.?\s*([0-9]+[a-zA-Z]?)"),
    ];

    for (system, pattern) in SYSTEMS {
        if let Some(num) = first_capture(pattern, title) {
            let display = format!("{} {}", canonical_system_label(system), num);
            return Some(CatalogueNumber {
                system: (*system).to_string(),
                number: num,
                display,
            });
        }
    }
    None
}

fn canonical_system_label(system: &str) -> &str {
    match system {
        "BWV" => "BWV",
        "K" => "K.",
        "D" => "D.",
        "RV" => "RV",
        "Hob" => "Hob.",
        "HWV" => "HWV",
        "Op" => "Op.",
        _ => system,
    }
}

/// Tiny ad-hoc regex driver. We avoid pulling in `regex` for a handful
/// of well-known patterns — the search is linear and the patterns are
/// trivial. Each pattern uses two anchors:
///   `\b<keyword>\s*\.?\s*<capture>` where `<capture>` is `[0-9]+...`.
/// Returns the captured group as a `String`.
fn first_capture(pattern: &str, haystack: &str) -> Option<String> {
    // The patterns we feed always look like `\b<KW>\s*\.?\s*(<inner>)`.
    // Parse out the keyword and the inner group manually.
    let kw_start = pattern.find("\\b")? + 2;
    let kw_end = pattern[kw_start..].find('\\')? + kw_start;
    let keyword = &pattern[kw_start..kw_end];
    let inner_start = pattern.find('(')? + 1;
    let inner_end = pattern.rfind(')')?;
    let inner = &pattern[inner_start..inner_end];

    // Find the keyword (case-insensitive, word-boundary).
    let bytes = haystack.as_bytes();
    let kw_lower = keyword.to_ascii_lowercase();
    let hay_lower = haystack.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(idx) = hay_lower[search_from..].find(&kw_lower) {
        let abs = search_from + idx;
        // word boundary on the left
        let left_ok = abs == 0
            || !bytes[abs - 1].is_ascii_alphanumeric()
                && bytes[abs - 1] != b'_';
        // word boundary on the right (after keyword)
        let after = abs + kw_lower.len();
        let right_ok = after >= bytes.len()
            || !bytes[after].is_ascii_alphanumeric()
                && bytes[after] != b'_';
        if left_ok && right_ok {
            // skip optional whitespace + dot + whitespace
            let mut p = after;
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            if p < bytes.len() && bytes[p] == b'.' {
                p += 1;
            }
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            // capture using inner pattern
            if let Some(cap) = capture_inner(inner, &haystack[p..]) {
                return Some(cap);
            }
        }
        search_from = abs + kw_lower.len();
    }
    None
}

/// Mini engine for the inner patterns we actually use:
///   `[0-9]+[a-zA-Z]?` → digits, optional trailing letter.
///   `[IVXLCDM]+:[0-9]+[a-zA-Z]?` → roman, colon, digits, opt letter.
fn capture_inner(pattern: &str, input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    if pattern == "[0-9]+[a-zA-Z]?" {
        let mut end = 0;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == 0 {
            return None;
        }
        if end < bytes.len() && bytes[end].is_ascii_alphabetic() {
            end += 1;
        }
        return Some(input[..end].to_string());
    }
    if pattern == "[IVXLCDM]+:[0-9]+[a-zA-Z]?" {
        let mut end = 0;
        while end < bytes.len() && b"IVXLCDM".contains(&bytes[end]) {
            end += 1;
        }
        if end == 0 {
            return None;
        }
        if end >= bytes.len() || bytes[end] != b':' {
            return None;
        }
        end += 1;
        let nums_start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == nums_start {
            return None;
        }
        if end < bytes.len() && bytes[end].is_ascii_alphabetic() {
            end += 1;
        }
        return Some(input[..end].to_string());
    }
    None
}

/// Pick out a key like "D minor", "C-sharp major", "Eb major" embedded
/// in a work title. Conservative — returns None if uncertain.
pub fn parse_key_from_title(title: &str) -> Option<String> {
    // Look for "<note> <accidental>? (minor|major)".
    let lower = title.to_lowercase();
    for note in &["a", "b", "c", "d", "e", "f", "g"] {
        for mode in &["minor", "major"] {
            for accidental in &["", " sharp", " flat", "#", "b", "♯", "♭"] {
                let needle = format!(" {note}{accidental} {mode}");
                if let Some(idx) = lower.find(&needle) {
                    // Re-extract from the original to preserve case.
                    let start = idx + 1; // skip the leading space
                    let end = idx + needle.len();
                    return Some(title[start..end].to_string());
                }
            }
        }
    }
    None
}

/// Heuristic mapping of a title to a `WorkType`. Phase 1 covers the big
/// canonical buckets; Phase 5 will refine via Wikidata P31.
pub fn parse_work_type_from_title(title: &str) -> Option<WorkType> {
    let lower = title.to_lowercase();
    if lower.contains("symphony") {
        return Some(WorkType::Symphony);
    }
    if lower.contains("concerto") {
        return Some(WorkType::Concerto);
    }
    if lower.contains("sonata") {
        return Some(WorkType::Sonata);
    }
    if lower.contains("string quartet") || lower.contains("quartet") {
        return Some(WorkType::StringQuartet);
    }
    if lower.contains("opera") {
        return Some(WorkType::Opera);
    }
    if lower.contains("cantata") {
        return Some(WorkType::Cantata);
    }
    if lower.contains("mass") || lower.contains("requiem") {
        return Some(WorkType::Mass);
    }
    if lower.contains("lieder") || lower.contains("lied ") {
        return Some(WorkType::Lieder);
    }
    if lower.contains("suite") || lower.contains("variations") {
        return Some(WorkType::Suite);
    }
    if lower.contains("etude") || lower.contains("étude") {
        return Some(WorkType::Etude);
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_op_125() {
        let cn = parse_catalogue_number("Symphony No. 9 in D minor, Op. 125 \"Choral\"")
            .expect("op 125 should parse");
        assert_eq!(cn.system, "Op");
        assert_eq!(cn.number, "125");
        assert_eq!(cn.display, "Op. 125");
    }

    #[test]
    fn parses_bwv() {
        let cn = parse_catalogue_number("Goldberg Variations, BWV 988").unwrap();
        assert_eq!(cn.system, "BWV");
        assert_eq!(cn.number, "988");
        assert_eq!(cn.display, "BWV 988");
    }

    #[test]
    fn parses_k() {
        let cn = parse_catalogue_number("Requiem in D minor, K. 626").unwrap();
        assert_eq!(cn.system, "K");
        assert_eq!(cn.number, "626");
    }

    #[test]
    fn parses_hob() {
        let cn = parse_catalogue_number("Piano Sonata Hob. XVI:50").unwrap();
        assert_eq!(cn.system, "Hob");
        assert_eq!(cn.number, "XVI:50");
    }

    #[test]
    fn no_catalogue_when_absent() {
        assert!(parse_catalogue_number("Glassworks").is_none());
    }

    #[test]
    fn parses_d_minor_key() {
        assert_eq!(
            parse_key_from_title("Symphony No. 9 in D minor"),
            Some("D minor".to_string())
        );
    }

    #[test]
    fn parses_d_major_key() {
        assert_eq!(
            parse_key_from_title("Symphony No. 9 in D major"),
            Some("D major".to_string())
        );
    }

    #[test]
    fn parses_work_type_symphony() {
        assert_eq!(
            parse_work_type_from_title("Symphony No. 9"),
            Some(WorkType::Symphony)
        );
    }

    #[test]
    fn era_buckets_basic() {
        assert!(matches!(year_to_era(1685), Era::Baroque));
        assert!(matches!(year_to_era(1770), Era::Classical));
        assert!(matches!(year_to_era(1860), Era::Romantic));
        assert!(matches!(year_to_era(1937), Era::PostWar));
        assert!(matches!(year_to_era(1985), Era::Contemporary));
    }

    // ---------------------------------------------------------------
    // Phase 7 (D-028) — child-movement filter tests.
    // ---------------------------------------------------------------

    #[test]
    fn child_movement_with_backward_parts_rel_is_filtered() {
        // Fixture: "III. Adagio lamentoso" — child of Tchaikovsky's
        // Symphony No. 6. MB serialises the rel as direction=backward.
        let w = serde_json::json!({
            "id": "child-mvt-mbid",
            "title": "Symphony no. 6 in B minor, op. 74 \"Pathétique\": III. Adagio lamentoso",
            "relations": [
                {
                    "type": "parts",
                    "direction": "backward",
                    "work": { "id": "parent-symphony-mbid", "title": "Pathétique" }
                }
            ]
        });
        assert!(work_is_child_movement(&w));
    }

    #[test]
    fn standalone_work_with_no_relations_is_kept() {
        // Mozart's Eine kleine Nachtmusik — standalone, no work-rels.
        let w = serde_json::json!({
            "id": "kleine-nachtmusik-mbid",
            "title": "Serenade no. 13 in G major, K. 525 \"Eine kleine Nachtmusik\"",
            "relations": []
        });
        assert!(!work_is_child_movement(&w));
    }

    #[test]
    fn parent_work_with_forward_parts_rel_is_kept() {
        // Beethoven's 9th — parent of 4 movements. The rels are forward
        // (parent reads "this work has parts <child>"). We keep it.
        let w = serde_json::json!({
            "id": "beethoven-9-mbid",
            "title": "Symphony no. 9 in D minor, op. 125 \"Choral\"",
            "relations": [
                { "type": "parts", "direction": "forward", "work": {} },
                { "type": "parts", "direction": "forward", "work": {} },
                { "type": "parts", "direction": "forward", "work": {} },
                { "type": "parts", "direction": "forward", "work": {} }
            ]
        });
        assert!(!work_is_child_movement(&w));
    }

    #[test]
    fn work_with_unrelated_relations_is_kept() {
        // A work with non-`parts` rels (e.g. "based on" another work)
        // is not a movement. Keep.
        let w = serde_json::json!({
            "id": "work-with-tribute-rel",
            "title": "Tribute to ...",
            "relations": [
                { "type": "based on", "direction": "backward", "work": {} }
            ]
        });
        assert!(!work_is_child_movement(&w));
    }

    #[test]
    fn work_with_no_relations_field_is_kept() {
        // MB sometimes omits the field entirely (e.g. when inc=work-rels
        // wasn't requested). We must not crash and must keep the work.
        let w = serde_json::json!({
            "id": "no-rels-mbid",
            "title": "Some Symphony"
        });
        assert!(!work_is_child_movement(&w));
    }

    // ---------------------------------------------------------------
    // Phase 8.9 (D-048 / A5) — secondary movement-title heuristic.
    // ---------------------------------------------------------------

    #[test]
    fn movement_title_roman_one_dot_space() {
        assert!(title_looks_like_movement("I. Allegro"));
    }

    #[test]
    fn movement_title_roman_four_dot_space() {
        assert!(title_looks_like_movement("IV. Presto"));
    }

    #[test]
    fn movement_title_roman_eight_with_descriptor() {
        assert!(title_looks_like_movement("VIII. Andante mosso"));
    }

    #[test]
    fn standalone_andante_is_not_movement() {
        // Real top-level work — must be kept.
        assert!(!title_looks_like_movement("Andante in C major"));
    }

    #[test]
    fn andantino_without_dot_is_not_movement() {
        assert!(!title_looks_like_movement("Andantino"));
    }

    #[test]
    fn symphony_title_is_not_movement() {
        // The numeral here is arabic, not roman, and there's no
        // dot-space pattern. Keep.
        assert!(!title_looks_like_movement("Symphony No. 1 in C"));
    }

    #[test]
    fn movement_title_long_descriptor_kept_as_movement() {
        assert!(title_looks_like_movement(
            "II. Andante con moto, quasi Allegretto"
        ));
    }

    #[test]
    fn empty_title_is_not_movement() {
        assert!(!title_looks_like_movement(""));
    }

    #[test]
    fn lone_roman_no_dot_is_not_movement() {
        // "I" alone or "IV" alone is just a numeral string — no dot,
        // no descriptor, do not treat as movement.
        assert!(!title_looks_like_movement("I"));
        assert!(!title_looks_like_movement("IV"));
    }

    // -----------------------------------------------------------------
    // Provenance-phrase heuristic ("X aus der Y", "X from the Y").
    // -----------------------------------------------------------------

    #[test]
    fn aus_der_zauberfloete_is_movement() {
        assert!(title_looks_like_movement(
            "\"Mohrentanz\" aus der Zauberflöte"
        ));
    }

    #[test]
    fn aus_dem_singspiel_is_movement() {
        assert!(title_looks_like_movement(
            "Arie aus dem Singspiel \"Bastien und Bastienne\""
        ));
    }

    #[test]
    fn from_the_opera_is_movement() {
        assert!(title_looks_like_movement(
            "Casta diva from the opera \"Norma\""
        ));
    }

    // -----------------------------------------------------------------
    // Colon-suffix heuristic (`<parent>: <movement-marker>`).
    // -----------------------------------------------------------------

    #[test]
    fn colon_no_n_is_movement() {
        assert!(title_looks_like_movement(
            "4 Contredanses for Orchestra, K. 271c/267: No. 3 in A major"
        ));
    }

    #[test]
    fn colon_aria_first_words_is_movement() {
        // Caught by the universal `: ` rule even though the aria text
        // ("Madamina") doesn't match any named marker word.
        assert!(title_looks_like_movement(
            "Don Giovanni, K. 527: Madamina, il catalogo è questo"
        ));
    }

    #[test]
    fn colon_variation_n_is_movement() {
        assert!(title_looks_like_movement(
            "Goldberg Variations, BWV 988: Variation 14"
        ));
    }

    #[test]
    fn colon_roman_is_movement() {
        assert!(title_looks_like_movement(
            "Symphony No. 9 in D minor, Op. 125: III. Adagio molto e cantabile"
        ));
    }

    #[test]
    fn colon_arabic_dot_is_movement() {
        assert!(title_looks_like_movement(
            "Suite No. 1 in G major, BWV 1007: 1. Prélude"
        ));
    }

    #[test]
    fn colon_overture_is_movement() {
        assert!(title_looks_like_movement(
            "Le nozze di Figaro, K. 492: Overture"
        ));
    }

    // Negative checks for legitimate top-level titles that should pass.

    #[test]
    fn variations_on_a_theme_is_not_movement() {
        // No colon, no provenance phrase, no roman prefix.
        assert!(!title_looks_like_movement(
            "Variations on a Theme by Haydn, Op. 56a"
        ));
    }

    #[test]
    fn well_tempered_clavier_book_is_dropped() {
        // The universal `: ` rule treats this as a child. MB usually
        // exposes the WTC books as separate top-level works without a
        // colon (`The Well-Tempered Clavier I, BWV 846-869`), so the
        // colon-form is the duplicate / sub-entry we drop.
        assert!(title_looks_like_movement(
            "The Well-Tempered Clavier: Book 1"
        ));
    }

    #[test]
    fn hob_catalogue_colon_is_kept() {
        // Haydn's catalogue notation `Hob. XVI:50` uses a colon with
        // no whitespace — the universal `: ` rule must NOT fire here.
        assert!(!title_looks_like_movement(
            "Piano Sonata in C major, Hob. XVI:50"
        ));
    }

    #[test]
    fn nickname_after_comma_is_not_movement() {
        // "Pathétique" after a comma is a nickname, not a movement.
        assert!(!title_looks_like_movement(
            "Symphony No. 6 in B minor, Op. 74, \"Pathétique\""
        ));
    }
}
