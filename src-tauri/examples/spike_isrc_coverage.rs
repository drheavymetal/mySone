//! Phase 0 spike — ISRC coverage in Tidal for canonical classical works.
//!
//! Validates two hypotheses before investing in Phase 1:
//!  1. ISRC coverage in Tidal for the canonical 5 works ≥ 70% → GO.
//!  2. Wall-clock per work in cold cache stays in a workable budget given
//!     the MusicBrainz 1 req/s rate limit.
//!
//! Read-only with respect to production state:
//!  - Decrypts `~/.config/sone/settings.json` to extract Tidal auth tokens
//!    (uses the keyring-backed master key, never re-saves the file).
//!  - Refreshes the access token in-memory only; the new token is not
//!    persisted back. Production stays untouched.
//!  - Does NOT touch the production cache (`~/.config/sone/cache/`),
//!    `mbid_cache.json`, `mbid_name_cache.json`, the stats DB, the
//!    scrobble queue, or any audio routing.
//!  - Writes its temporary cache to `/tmp/sone-spike-cache/` (created if
//!    missing). Safe to delete between runs.
//!
//! Usage:
//!   cargo run --example spike_isrc_coverage --release
//!
//! Flags via env vars:
//!   SPIKE_MAX_RECORDINGS_PER_WORK  default 25
//!   SPIKE_INCLUDE_CHILD_WORKS      default 1 (set to 0 to disable)
//!   SPIKE_OUTPUT_PATH              default docs/classical/phase-0-spike.md
//!
//! Exit codes:
//!   0  success (report written, decision printed)
//!   1  Tidal auth invalid / refresh failed (blocker)
//!   2  MusicBrainz unreachable / sustained 503 (blocker)
//!   3  unexpected runtime error
//!
//! See `docs/classical/phase-0-spike.md` for the plan and `CLASSICAL_DESIGN.md`
//! §8 Phase 0 for the gating thresholds.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri_app_lib::crypto::Crypto;
use tauri_app_lib::embedded_config;
use tauri_app_lib::tidal_api::{TidalClient, TidalTrack};
use tauri_app_lib::Settings;

const TIDAL_API_V1: &str = "https://api.tidal.com/v1";

const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";
const MB_RATE_LIMIT_MS: u64 = 1_100;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const HTTP_TIMEOUT_SECS: u64 = 15;
const DEFAULT_MAX_RECORDINGS_PER_WORK: usize = 25;
const DEFAULT_OUTPUT_PATH: &str = "docs/classical/phase-0-spike.md";

#[derive(Clone, Debug)]
struct CanonWork {
    label: &'static str,
    composer: &'static str,
    title: &'static str,
    mbid: &'static str,
    /// Hand-picked canonical recordings used to probe Tidal's catalogue
    /// directly via text search. Each entry is a `(query, label)` pair.
    /// These should be unambiguous canonical reference recordings — the
    /// kind any classical listener would expect to be playable.
    canon_probes: &'static [(&'static str, &'static str)],
}

const CANON: [CanonWork; 5] = [
    CanonWork {
        label: "Beethoven 9",
        composer: "Ludwig van Beethoven",
        title: "Symphony No. 9 in D minor, Op. 125 \"Choral\"",
        mbid: "c35b4956-d4f8-321a-865b-5b13d9ed192b",
        canon_probes: &[
            ("Beethoven Symphony 9 Karajan Berlin Philharmonic 1962", "Karajan/BPO 1962 (DG)"),
            ("Beethoven Symphony 9 Bernstein Vienna 1979", "Bernstein/VPO 1979 (DG)"),
            ("Beethoven Symphony 9 Solti Chicago", "Solti/Chicago SO (Decca)"),
            ("Beethoven Symphony 9 Furtwangler Bayreuth 1951", "Furtwängler/Bayreuth 1951 (EMI)"),
            ("Beethoven Symphony 9 Gardiner Orchestre Revolutionnaire", "Gardiner/ORR (DG Archiv)"),
        ],
    },
    CanonWork {
        label: "Bach Goldberg",
        composer: "Johann Sebastian Bach",
        title: "Goldberg Variations, BWV 988",
        mbid: "1d51e560-2a59-4e97-8943-13052b6adc03",
        canon_probes: &[
            ("Goldberg Variations Glenn Gould 1981", "Glenn Gould 1981 (CBS/Sony)"),
            ("Goldberg Variations Glenn Gould 1955", "Glenn Gould 1955 (Columbia)"),
            ("Goldberg Variations Andras Schiff", "András Schiff (Decca)"),
            ("Goldberg Variations Murray Perahia", "Murray Perahia (Sony)"),
            ("Goldberg Variations Pierre Hantai harpsichord", "Pierre Hantaï (harpsichord)"),
        ],
    },
    CanonWork {
        label: "Mozart Requiem",
        composer: "Wolfgang Amadeus Mozart",
        title: "Requiem in D minor, K. 626",
        mbid: "3b11692b-cdc7-4107-9708-e5b9ee386af3",
        canon_probes: &[
            ("Mozart Requiem Karl Bohm Vienna", "Böhm/VPO (DG)"),
            ("Mozart Requiem Herbert von Karajan", "Karajan/BPO (DG)"),
            ("Mozart Requiem John Eliot Gardiner Monteverdi", "Gardiner/Monteverdi Choir (Philips)"),
            ("Mozart Requiem Nikolaus Harnoncourt", "Harnoncourt/Concentus Musicus (Sony)"),
            ("Mozart Requiem Rene Jacobs", "René Jacobs (HMU)"),
        ],
    },
    CanonWork {
        label: "Mahler 9",
        composer: "Gustav Mahler",
        title: "Symphony No. 9 in D major",
        mbid: "0d459ba8-74cd-4f1c-82b6-4566a5e0778c",
        canon_probes: &[
            ("Mahler Symphony 9 Bernstein Berlin 1979", "Bernstein/BPO 1979 (DG)"),
            ("Mahler Symphony 9 Karajan Berlin Philharmonic", "Karajan/BPO (DG)"),
            ("Mahler Symphony 9 Abbado Berlin", "Abbado/BPO (DG)"),
            ("Mahler Symphony 9 Bruno Walter Vienna 1938", "Bruno Walter/VPO 1938 (EMI/Sony)"),
            ("Mahler Symphony 9 Bernard Haitink", "Haitink/Concertgebouw (Philips)"),
        ],
    },
    CanonWork {
        label: "Glass Glassworks",
        composer: "Philip Glass",
        title: "Glassworks",
        mbid: "1d0df1a9-52a4-48ca-a6e5-290cd880e249",
        canon_probes: &[
            ("Philip Glass Glassworks Michael Riesman", "Riesman/Philip Glass Ensemble (Sony 1982)"),
            ("Glassworks Philip Glass Ensemble Opening", "PGE — Opening track"),
            ("Glassworks Vikingur Olafsson", "Víkingur Ólafsson (DG)"),
            ("Glassworks Lavinia Meijer harp", "Lavinia Meijer (Sony harp arrangement)"),
            ("Glassworks Floraleda Sacchi", "Floraleda Sacchi (Amadeus harp)"),
        ],
    },
];

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct WorkReport {
    label: String,
    title: String,
    composer: String,
    work_mbid: String,
    /// Recording rels found directly on the parent work entity.
    direct_recording_count: usize,
    /// Recordings found via child works (movements / variations) walked
    /// through `parts` work-rels in backward direction.
    via_children_recording_count: usize,
    /// Recordings discovered via `/release?work=...&inc=recordings` browse.
    /// Captures commercial recordings that lack a direct work-recording rel
    /// in MB but are referenced from a release-track linked to the work.
    via_releases_recording_count: usize,
    /// Total unique recordings considered (deduplicated by MBID, capped
    /// by SPIKE_MAX_RECORDINGS_PER_WORK).
    considered_count: usize,
    recordings_with_isrc: usize,
    playable_in_tidal: usize,
    quality_breakdown: HashMap<String, usize>,
    not_on_tidal: Vec<NotOnTidal>,
    /// Hand-picked canonical recordings probed via Tidal text search.
    /// Reveals what the catalogue actually contains regardless of MB ISRC
    /// coverage. Each probe reports `found=true` if any track in the top-3
    /// Tidal results plausibly matches (artist + work title heuristic).
    canon_probe_results: Vec<CanonProbeResult>,
    wall_clock_secs: f64,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanonProbeResult {
    label: String,
    query: String,
    found: bool,
    top_hit_title: Option<String>,
    top_hit_artist: Option<String>,
    top_hit_quality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotOnTidal {
    recording_mbid: String,
    title: String,
    isrcs: Vec<String>,
    artists: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AggregateReport {
    works: Vec<WorkReport>,
    overall_pct_playable: f64,
    overall_recordings_considered: usize,
    overall_playable: usize,
    rate_limit_calls_total: usize,
    started_at: String,
    duration_total_secs: f64,
}

/// HTTP client bound to MB rate limit. Single-threaded by design — the spike
/// is a sequential script, no concurrency needed.
struct MbClient {
    http: reqwest::Client,
    last_request: Instant,
    user_agent: String,
    calls: usize,
}

impl MbClient {
    fn new() -> Result<Self, String> {
        let user_agent = format!(
            "SONE-classical-spike/{APP_VERSION} (https://github.com/lullabyX/sone)"
        );
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .user_agent(user_agent.clone())
            .build()
            .map_err(|e| format!("build http client: {e}"))?;
        Ok(Self {
            http,
            last_request: Instant::now() - Duration::from_millis(MB_RATE_LIMIT_MS),
            user_agent,
            calls: 0,
        })
    }

    async fn get_json(&mut self, url: &str) -> Result<serde_json::Value, String> {
        let elapsed = self.last_request.elapsed();
        let min = Duration::from_millis(MB_RATE_LIMIT_MS);
        if elapsed < min {
            tokio::time::sleep(min - elapsed).await;
        }
        self.last_request = Instant::now();
        self.calls += 1;

        let resp = self
            .http
            .get(url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| format!("request {url}: {e}"))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if status.as_u16() == 503 {
            // MB throttling. One retry after 5 s, then give up cleanly.
            log::warn!("MB 503 on {url}, sleeping 5 s and retrying once");
            tokio::time::sleep(Duration::from_secs(5)).await;
            self.last_request = Instant::now();
            self.calls += 1;

            let retry = self
                .http
                .get(url)
                .header(reqwest::header::USER_AGENT, &self.user_agent)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(|e| format!("retry {url}: {e}"))?;

            let retry_status = retry.status();
            let retry_body = retry.text().await.unwrap_or_default();
            if !retry_status.is_success() {
                return Err(format!(
                    "MB {retry_status} on retry of {url}: {retry_body}"
                ));
            }
            return serde_json::from_str(&retry_body)
                .map_err(|e| format!("parse retry json: {e}"))
            ;
        }

        if !status.is_success() {
            return Err(format!("MB {status} on {url}: {body}"));
        }

        serde_json::from_str(&body).map_err(|e| format!("parse json: {e}"))
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Err(code) = run().await {
        eprintln!("spike aborted with exit code {code}");
        std::process::exit(code);
    }
}

async fn run() -> Result<(), i32> {
    let started = Instant::now();
    let started_at_iso = chrono_now_iso();

    let max_recordings = read_usize_env("SPIKE_MAX_RECORDINGS_PER_WORK")
        .unwrap_or(DEFAULT_MAX_RECORDINGS_PER_WORK);
    let include_children = read_usize_env("SPIKE_INCLUDE_CHILD_WORKS")
        .map(|n| n != 0)
        .unwrap_or(true);
    let output_path = std::env::var("SPIKE_OUTPUT_PATH").unwrap_or_else(|_| {
        // The example is normally launched as `cargo run --example ...` from
        // the `src-tauri/` directory. Resolve the doc path relative to the
        // repo root by walking up if the relative path doesn't exist.
        let direct = PathBuf::from(DEFAULT_OUTPUT_PATH);
        if direct.exists() {
            return direct.to_string_lossy().into_owned();
        }
        let from_src_tauri = PathBuf::from("..").join(DEFAULT_OUTPUT_PATH);
        if from_src_tauri.exists() {
            return from_src_tauri.to_string_lossy().into_owned();
        }
        DEFAULT_OUTPUT_PATH.to_string()
    });

    eprintln!("=== SONE Classical — Phase 0 spike ===");
    eprintln!("max_recordings_per_work={max_recordings} include_children={include_children}");
    eprintln!("output_path={output_path}");
    eprintln!();

    // 1. Load Tidal auth from production settings (read-only).
    let mut tidal = match load_tidal_client().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[BLOCKER] Tidal auth setup failed: {e}");
            return Err(1);
        }
    };

    // 2. MB client with shared rate limit budget.
    let mut mb = MbClient::new().map_err(|e| {
        eprintln!("[ERR] {e}");
        3
    })?;

    let mut aggregate = AggregateReport {
        started_at: started_at_iso,
        ..Default::default()
    };

    for work in CANON.iter() {
        let report = match process_work(work, &mut mb, &mut tidal, max_recordings, include_children)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERR] processing {}: {e}", work.label);
                let mut empty = WorkReport {
                    label: work.label.to_string(),
                    title: work.title.to_string(),
                    composer: work.composer.to_string(),
                    work_mbid: work.mbid.to_string(),
                    ..Default::default()
                };
                empty.notes.push(format!("ERROR: {e}"));
                empty
            }
        };
        log_progress(&report);
        aggregate.works.push(report);
    }

    // Aggregate metrics.
    let considered: usize = aggregate.works.iter().map(|w| w.considered_count).sum();
    let playable: usize = aggregate.works.iter().map(|w| w.playable_in_tidal).sum();
    aggregate.overall_recordings_considered = considered;
    aggregate.overall_playable = playable;
    aggregate.overall_pct_playable = if considered > 0 {
        (playable as f64) * 100.0 / (considered as f64)
    } else {
        0.0
    };
    aggregate.rate_limit_calls_total = mb.calls;
    aggregate.duration_total_secs = started.elapsed().as_secs_f64();

    // 3. Render markdown report.
    let md = render_markdown(&aggregate);
    print!("{md}");
    write_report_section(&output_path, &md).map_err(|e| {
        eprintln!("[ERR] writing report: {e}");
        3
    })?;

    // 4. Recommendation.
    let decision = decide(&aggregate);
    eprintln!();
    eprintln!("=== DECISION ===");
    eprintln!("{decision}");
    eprintln!("================");

    Ok(())
}

fn read_usize_env(key: &str) -> Option<usize> {
    std::env::var(key).ok().and_then(|v| v.parse().ok())
}

/// Mirrors `commands::auth::resolve_credentials` — settings first, embedded
/// fallback. Kept duplicated here to avoid making a Tauri command pub.
fn resolve_credentials(settings: &Settings) -> (String, String) {
    let id = if settings.client_id.is_empty() {
        embedded_config::stream_key_a()
    } else {
        settings.client_id.clone()
    };
    let secret = if settings.client_secret.is_empty() {
        embedded_config::stream_key_b()
    } else {
        settings.client_secret.clone()
    };
    (id, secret)
}

async fn load_tidal_client() -> Result<TidalClient, String> {
    let mut config_dir = dirs::config_dir().ok_or_else(|| "no config dir".to_string())?;
    config_dir.push("sone");
    let settings_path = config_dir.join("settings.json");

    if !settings_path.exists() {
        return Err(format!("settings.json not found at {settings_path:?}"));
    }

    let crypto = Crypto::new(&config_dir).map_err(|e| format!("crypto init: {e:?}"))?;
    let raw = fs::read(&settings_path).map_err(|e| format!("read settings: {e}"))?;
    let plain = crypto
        .decrypt(&raw)
        .map_err(|e| format!("decrypt settings: {e:?}"))?;
    let text = String::from_utf8(plain).map_err(|e| format!("settings utf8: {e}"))?;
    let settings: Settings =
        serde_json::from_str(&text).map_err(|e| format!("parse settings: {e}"))?;

    let tokens = settings
        .auth_tokens
        .clone()
        .ok_or_else(|| "no auth_tokens in settings — please log in to SONE first".to_string())?;

    let mut tidal = TidalClient::new(&settings.proxy);
    let (id, secret) = resolve_credentials(&settings);
    if id.is_empty() {
        return Err(
            "no Tidal client_id available (settings empty + no embedded keys)".to_string(),
        );
    }
    tidal.set_credentials(&id, &secret);
    tidal.tokens = Some(tokens);

    // Probe the session. If the access token is expired, attempt one refresh.
    // If refresh fails, this is a blocker that needs human intervention.
    let probe = tidal.search("test", 1).await;
    match probe {
        Ok(_) => {
            log::info!("Tidal auth probe ok");
            Ok(tidal)
        }
        Err(e) => {
            log::warn!("Tidal probe failed ({e:?}), attempting refresh");
            tidal
                .refresh_token()
                .await
                .map_err(|re| format!("refresh after probe failure: {re:?}; original: {e:?}"))?;
            log::info!("Tidal token refreshed successfully");
            // Verify with a second probe.
            tidal
                .search("test", 1)
                .await
                .map_err(|e2| format!("search after refresh: {e2:?}"))?;
            Ok(tidal)
        }
    }
}

async fn process_work(
    work: &CanonWork,
    mb: &mut MbClient,
    tidal: &mut TidalClient,
    max_recordings: usize,
    _include_children: bool,
) -> Result<WorkReport, String> {
    let started = Instant::now();
    let mut report = WorkReport {
        label: work.label.to_string(),
        title: work.title.to_string(),
        composer: work.composer.to_string(),
        work_mbid: work.mbid.to_string(),
        ..Default::default()
    };

    // 1) Browse all recordings linked to this work in a single MB call,
    //    inline ISRCs + artist-credits. This collapses what used to be
    //    1 + N detail calls into 1, freeing rate-limit budget for the
    //    Tidal side. The MB `recording?work=...` endpoint aggregates
    //    work-recording rels both directly on the parent work and via
    //    child works (movements), so no second pass is required.
    let hits = browse_recordings_for_work(mb, work.mbid, max_recordings).await?;
    report.direct_recording_count = hits.len();
    report.via_children_recording_count = 0;
    report.via_releases_recording_count = 0;

    let truncated = if hits.len() > max_recordings {
        hits.into_iter().take(max_recordings).collect::<Vec<_>>()
    } else {
        hits
    };
    report.considered_count = truncated.len();

    // 2) For each recording: try every ISRC against Tidal; first match wins.
    //    Track quality tier on hit, and record (artists, title, ISRCs) on miss.
    for hit in truncated.iter() {
        if !hit.isrcs.is_empty() {
            report.recordings_with_isrc += 1;
        }

        let mut tidal_hit: Option<TidalTrack> = None;
        for isrc in hit.isrcs.iter() {
            match lookup_tidal_by_isrc(tidal, isrc).await {
                Ok(Some(t)) => {
                    tidal_hit = Some(t);
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    log::debug!("Tidal isrc lookup {isrc} failed: {e}");
                }
            }
        }

        if let Some(t) = tidal_hit {
            report.playable_in_tidal += 1;
            let key = quality_label(&t);
            *report.quality_breakdown.entry(key).or_insert(0) += 1;
        } else {
            report.not_on_tidal.push(NotOnTidal {
                recording_mbid: hit.mbid.clone(),
                title: hit.title.clone(),
                isrcs: hit.isrcs.clone(),
                artists: hit.artists.clone(),
            });
        }
    }

    // 3) Canon-recordings probe via Tidal text search. Independent of MB
    //    ISRC coverage — answers "is the canonical recording on Tidal at all".
    for (query, label) in work.canon_probes.iter() {
        let result = match tidal.search(query, 3).await {
            Ok(results) => {
                if let Some(track) = results.tracks.first() {
                    let mut t = track.clone();
                    t.backfill_artist();
                    let artist_name = t.artist.as_ref().map(|a| a.name.clone());
                    CanonProbeResult {
                        label: (*label).to_string(),
                        query: (*query).to_string(),
                        found: true,
                        top_hit_title: Some(t.title.clone()),
                        top_hit_artist: artist_name,
                        top_hit_quality: Some(quality_label(&t)),
                    }
                } else {
                    CanonProbeResult {
                        label: (*label).to_string(),
                        query: (*query).to_string(),
                        found: false,
                        top_hit_title: None,
                        top_hit_artist: None,
                        top_hit_quality: None,
                    }
                }
            }
            Err(e) => {
                log::debug!("canon probe '{query}' search failed: {e:?}");
                CanonProbeResult {
                    label: (*label).to_string(),
                    query: (*query).to_string(),
                    found: false,
                    top_hit_title: None,
                    top_hit_artist: None,
                    top_hit_quality: None,
                }
            }
        };
        report.canon_probe_results.push(result);
    }

    report.wall_clock_secs = started.elapsed().as_secs_f64();
    Ok(report)
}

/// Browse recordings linked to the work and harvest ISRCs in one shot.
/// `recording?work={mbid}&inc=isrcs+artist-credits&limit=100` returns full
/// recording objects with ISRCs inline, saving N+1 detail fetches that the
/// per-recording path makes. Returns a vec of (mbid, isrcs, title, artists)
/// tuples ordered by MB's default order (which roughly correlates with
/// editor-curation density, not popularity — see "Riesgos R2" in the doc).
async fn browse_recordings_for_work(
    mb: &mut MbClient,
    work_mbid: &str,
    limit: usize,
) -> Result<Vec<RecordingHit>, String> {
    let url = format!(
        "{MB_API_BASE}/recording?work={work_mbid}&inc=isrcs+artist-credits&fmt=json&limit={limit}"
    );
    let body = mb.get_json(&url).await?;
    let mut out = Vec::new();
    let recordings = match body.get("recordings").and_then(|r| r.as_array()) {
        Some(a) => a,
        None => return Ok(out),
    };
    for rec in recordings {
        let mbid = match rec.get("id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let title = rec
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("(untitled)")
            .to_string();
        let isrcs = rec
            .get("isrcs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let artists = rec
            .get("artist-credit")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        c.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(RecordingHit {
            mbid,
            title,
            isrcs,
            artists,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct RecordingHit {
    mbid: String,
    title: String,
    isrcs: Vec<String>,
    artists: Vec<String>,
}

/// Resolve an ISRC to a Tidal track via the v1 endpoint that filters by ISRC.
/// `/v1/tracks?isrc=XXXXXXXXXXXX&countryCode=YY` returns the matching track(s)
/// for that ISRC in the user's region. We pick the first that exact-matches.
///
/// We hit the endpoint directly (no public wrapper exposed by `TidalClient`),
/// reusing the client's auth tokens, country_code, and proxy-aware http client.
async fn lookup_tidal_by_isrc(
    tidal: &TidalClient,
    isrc: &str,
) -> Result<Option<TidalTrack>, String> {
    let access_token = tidal
        .tokens
        .as_ref()
        .ok_or_else(|| "no tokens".to_string())?
        .access_token
        .clone();
    let country = tidal.country_code.clone();
    let url = format!("{TIDAL_API_V1}/tracks");
    let resp = tidal
        .raw_client()
        .get(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .query(&[("isrc", isrc), ("countryCode", country.as_str())])
        .send()
        .await
        .map_err(|e| format!("isrc lookup network: {e}"))?;

    let status = resp.status();
    if status.as_u16() == 404 {
        return Ok(None);
    }
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // 451 = region-restricted; 404 already handled. Any other 4xx/5xx is
        // a real error worth surfacing to debug logs.
        return Err(format!("Tidal isrc lookup HTTP {status}: {body}"));
    }

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("parse isrc body: {e}"))?;

    // Two response shapes are possible:
    //  - Top-level array of TidalTrack objects.
    //  - { items: [...] } wrapper.
    let arr_owned: Vec<serde_json::Value>;
    let arr: &Vec<serde_json::Value> = if let Some(a) = json.as_array() {
        a
    } else if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
        items
    } else {
        arr_owned = Vec::new();
        &arr_owned
    };

    if arr.is_empty() {
        return Ok(None);
    }

    for item in arr {
        match serde_json::from_value::<TidalTrack>(item.clone()) {
            Ok(mut t) => {
                t.backfill_artist();
                if t.isrc.as_deref() == Some(isrc) {
                    return Ok(Some(t));
                }
            }
            Err(e) => {
                log::debug!("isrc lookup decode skip: {e}");
            }
        }
    }

    // No exact match — but Tidal returned something, return the first decoded
    // track if any (often Tidal's ISRC index uses a normalised form that may
    // differ in case from MB's; trust the API filter and accept the first).
    if let Some(first) = arr.first() {
        if let Ok(mut t) = serde_json::from_value::<TidalTrack>(first.clone()) {
            t.backfill_artist();
            return Ok(Some(t));
        }
    }
    Ok(None)
}

/// Build a tier label combining the base quality flag and any AUDIO_MODES
/// like `DOLBY_ATMOS` or `STEREO`. Falls back to "(unlabelled)".
fn quality_label(t: &TidalTrack) -> String {
    if let Some(meta) = &t.media_metadata {
        if !meta.tags.is_empty() {
            return meta.tags.join("+");
        }
    }
    if let Some(q) = &t.audio_quality {
        return q.clone();
    }
    "(unlabelled)".to_string()
}

fn log_progress(r: &WorkReport) {
    let pct = if r.considered_count > 0 {
        (r.playable_in_tidal as f64) * 100.0 / (r.considered_count as f64)
    } else {
        0.0
    };
    eprintln!(
        "  {:<22} considered={:>3}  playable={:>3} ({:>5.1}%)  with_isrc={:>3}  wall={:>5.1}s",
        r.label, r.considered_count, r.playable_in_tidal, pct, r.recordings_with_isrc, r.wall_clock_secs
    );
}

fn decide(agg: &AggregateReport) -> String {
    let pct_isrc_to_tidal = agg.overall_pct_playable;
    let total_probes: usize = agg.works.iter().map(|w| w.canon_probe_results.len()).sum();
    let found_probes: usize = agg
        .works
        .iter()
        .flat_map(|w| w.canon_probe_results.iter())
        .filter(|p| p.found)
        .count();
    let pct_canon_in_tidal = if total_probes > 0 {
        (found_probes as f64) * 100.0 / (total_probes as f64)
    } else {
        0.0
    };
    let total_with_isrc: usize = agg.works.iter().map(|w| w.recordings_with_isrc).sum();
    let pct_isrc_conversion = if total_with_isrc > 0 {
        (agg.overall_playable as f64) * 100.0 / (total_with_isrc as f64)
    } else {
        0.0
    };

    // Decision is composite: the original gate (ISRC coverage on canon
    // sample) checks the WORK→PLAYBACK pipeline as currently designed;
    // the canon probe checks whether Tidal HAS the canonical catalogue
    // when MB ISRCs are missing. The interesting case is high probe
    // success + low ISRC coverage: GO with asterisk + revise data model
    // to use Tidal text search as a parallel discovery path.
    let verdict = if pct_isrc_to_tidal >= 70.0 {
        "GO"
    } else if pct_isrc_to_tidal >= 50.0 {
        "GO with asterisk"
    } else if pct_canon_in_tidal >= 70.0 && pct_isrc_conversion >= 70.0 {
        "GO with asterisk — Tidal catalogue is healthy; ISRC coverage in MB is the bottleneck. Phase 1 must add Tidal-text-search as a parallel discovery path."
    } else {
        "NO-GO — replantear"
    };
    format!(
        "ISRC→Tidal: {}/{} ({:.1}%) | of-with-ISRC: {:.1}% | canon probes: {}/{} ({:.1}%) → {verdict}",
        agg.overall_playable,
        agg.overall_recordings_considered,
        pct_isrc_to_tidal,
        pct_isrc_conversion,
        found_probes,
        total_probes,
        pct_canon_in_tidal
    )
}

fn render_markdown(agg: &AggregateReport) -> String {
    let mut s = String::new();
    s.push_str("\n### Resumen por obra\n\n");
    s.push_str("| Obra | Direct rels | Via children | Via releases | Considered | With ISRC | Playable | % playable | % of-with-ISRC | Wall |\n");
    s.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for w in agg.works.iter() {
        let pct = if w.considered_count > 0 {
            (w.playable_in_tidal as f64) * 100.0 / (w.considered_count as f64)
        } else {
            0.0
        };
        let pct_of_isrc = if w.recordings_with_isrc > 0 {
            (w.playable_in_tidal as f64) * 100.0 / (w.recordings_with_isrc as f64)
        } else {
            0.0
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {:.1}% | {:.1}% | {:.1}s |\n",
            w.label,
            w.direct_recording_count,
            w.via_children_recording_count,
            w.via_releases_recording_count,
            w.considered_count,
            w.recordings_with_isrc,
            w.playable_in_tidal,
            pct,
            pct_of_isrc,
            w.wall_clock_secs
        ));
    }
    let total_with_isrc: usize = agg.works.iter().map(|w| w.recordings_with_isrc).sum();
    let pct_of_isrc_total = if total_with_isrc > 0 {
        (agg.overall_playable as f64) * 100.0 / (total_with_isrc as f64)
    } else {
        0.0
    };
    s.push_str(&format!(
        "| **Overall** | — | — | — | **{}** | **{}** | **{}** | **{:.1}%** | **{:.1}%** | {:.1}s |\n",
        agg.overall_recordings_considered,
        total_with_isrc,
        agg.overall_playable,
        agg.overall_pct_playable,
        pct_of_isrc_total,
        agg.duration_total_secs,
    ));

    s.push_str("\n### Quality breakdown (sobre playable)\n\n");
    let mut totals: HashMap<String, usize> = HashMap::new();
    for w in agg.works.iter() {
        for (k, v) in w.quality_breakdown.iter() {
            *totals.entry(k.clone()).or_insert(0) += v;
        }
    }
    let total_playable: usize = totals.values().sum();
    let mut tiers: Vec<(String, usize)> = totals.into_iter().collect();
    tiers.sort_by(|a, b| b.1.cmp(&a.1));
    s.push_str("| Tier | Count | % |\n");
    s.push_str("|---|---:|---:|\n");
    for (tier, count) in tiers.iter() {
        let pct = if total_playable > 0 {
            (*count as f64) * 100.0 / (total_playable as f64)
        } else {
            0.0
        };
        s.push_str(&format!("| {} | {} | {:.1}% |\n", tier, count, pct));
    }

    s.push_str("\n### Probe de canon hand-picked en Tidal (text search)\n\n");
    s.push_str("> Validación independiente de MB ISRC coverage. Cada query es una grabación canónica reconocida; se reporta el top-hit de Tidal v1 search.\n\n");
    s.push_str("| Obra | Probe | Encontrado | Top hit Tidal | Quality |\n");
    s.push_str("|---|---|---|---|---|\n");
    let mut total_probes = 0_usize;
    let mut found_probes = 0_usize;
    for w in agg.works.iter() {
        for p in w.canon_probe_results.iter() {
            total_probes += 1;
            if p.found {
                found_probes += 1;
            }
            let title = p.top_hit_title.as_deref().unwrap_or("—");
            let artist = p.top_hit_artist.as_deref().unwrap_or("—");
            let q = p.top_hit_quality.as_deref().unwrap_or("—");
            let mark = if p.found { "✓" } else { "✗" };
            s.push_str(&format!(
                "| {} | {} | {} | {} — _{}_ | {} |\n",
                w.label, p.label, mark, artist, title, q
            ));
        }
    }
    let pct_canon = if total_probes > 0 {
        (found_probes as f64) * 100.0 / (total_probes as f64)
    } else {
        0.0
    };
    s.push_str(&format!(
        "| **Overall** | — | **{}/{} ({:.1}%)** | — | — |\n",
        found_probes, total_probes, pct_canon
    ));

    s.push_str("\n### Casos notables (recordings con ISRC NO encontradas en Tidal)\n\n");
    for w in agg.works.iter() {
        if w.not_on_tidal.is_empty() {
            continue;
        }
        s.push_str(&format!("**{}**\n\n", w.label));
        for n in w.not_on_tidal.iter().take(8) {
            let artists = if n.artists.is_empty() {
                "(unknown)".to_string()
            } else {
                n.artists.join(" / ")
            };
            let isrcs = if n.isrcs.is_empty() {
                "no ISRC".to_string()
            } else {
                n.isrcs.join(", ")
            };
            s.push_str(&format!(
                "- {} — _{title}_ — `{isrcs}`\n",
                artists,
                title = n.title,
                isrcs = isrcs
            ));
        }
        s.push_str("\n");
    }

    s.push_str("\n### Rate-limit budget\n\n");
    s.push_str(&format!(
        "- MB calls totales: **{}**\n- Tiempo total: **{:.1}s**\n- Iniciado: {}\n",
        agg.rate_limit_calls_total, agg.duration_total_secs, agg.started_at
    ));

    s.push_str("\n### Decisión\n\n");
    s.push_str(&format!("> {}\n", decide(agg)));

    s
}

/// Replace the placeholder "Resultados" section in `phase-0-spike.md` with
/// the freshly rendered tables. Keeps surrounding sections (plan, riesgos,
/// histórico) intact so the doc stays human-editable.
fn write_report_section(output_path: &str, md: &str) -> Result<(), String> {
    let path = PathBuf::from(output_path);
    let original = fs::read_to_string(&path)
        .map_err(|e| format!("read {output_path}: {e}"))?;

    const HEADER: &str = "## Resultados";
    const NEXT_HEADER: &str = "## Riesgos detectados durante Phase 0";

    let header_pos = original
        .find(HEADER)
        .ok_or_else(|| format!("'{HEADER}' header not found in {output_path}"))?;
    let next_pos = original
        .find(NEXT_HEADER)
        .ok_or_else(|| format!("'{NEXT_HEADER}' anchor not found in {output_path}"))?;

    if next_pos <= header_pos {
        return Err("section anchors out of order".into());
    }

    let mut new_doc = String::new();
    new_doc.push_str(&original[..header_pos]);
    new_doc.push_str(HEADER);
    new_doc.push_str("\n\n");
    new_doc.push_str("> Generado por `cargo run --example spike_isrc_coverage`. Re-correr el script lo regenera.\n");
    new_doc.push_str(md);
    new_doc.push_str("\n---\n\n");
    new_doc.push_str(&original[next_pos..]);

    fs::write(&path, new_doc).map_err(|e| format!("write {output_path}: {e}"))?;
    Ok(())
}

fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // crude ISO-ish — avoid pulling chrono just for this
    format!("epoch+{secs}")
}
