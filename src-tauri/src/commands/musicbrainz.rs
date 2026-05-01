//! MusicBrainz / Cover Art Archive enrichment commands.
//!
//! Two endpoints exposed to the frontend:
//!  - `lookup_album_cover_caa(album, artist)`: search MB for the album's
//!    release-group and return a Cover Art Archive front-cover URL when
//!    one exists. Used as a fallback when TIDAL has no cover for an
//!    obscure release.
//!  - `get_mb_recording_details(title, artist)`: resolve the recording
//!    by name and pull credits, tags, and external links so the UI can
//!    render a "track details" panel without forcing a roundtrip per
//!    field.

use std::collections::HashSet;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::scrobble::musicbrainz::MbResolved;
use crate::AppState;
use crate::SoneError;

const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn user_agent() -> String {
    format!("SONE/{APP_VERSION} (https://github.com/lullabyX/sone)")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaaCover {
    /// Direct URL to the front cover JPEG hosted by Cover Art Archive.
    pub url: String,
    /// Release-group MBID the cover belongs to, useful for caching.
    pub release_group_mbid: String,
}

/// Find a Cover Art Archive front cover for an album. Returns `None`
/// when MusicBrainz has no matching release-group, or when CAA has no
/// art for it.
#[tauri::command(rename_all = "camelCase")]
pub async fn lookup_album_cover_caa(
    state: State<'_, AppState>,
    album: String,
    artist: String,
) -> Result<Option<CaaCover>, SoneError> {
    log::debug!("[caa] lookup album={album:?} artist={artist:?}");
    let mb = state.scrobble_manager.mb_lookup();

    // Search for the album by name+artist via MB. We piggyback on the
    // existing rate-limited lookup_by_name for the artist resolution
    // and do the release-group search inline (different MB endpoint).
    let release_group_mbid = match search_release_group(&album, &artist).await {
        Ok(Some(id)) => id,
        Ok(None) => return Ok(None),
        Err(e) => {
            log::debug!("[caa] release-group search failed: {e}");
            return Ok(None);
        }
    };

    // Verify CAA actually has art for this release-group. The HEAD
    // request is what the JSON API recommends — saves bandwidth and
    // tells us "no art" via 404.
    let url = format!(
        "https://coverartarchive.org/release-group/{release_group_mbid}/front-500"
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| SoneError::Network(format!("caa client: {e}")))?;
    let resp = client.head(&url).send().await;
    let exists = match resp {
        Ok(r) => {
            let s = r.status().as_u16();
            // CAA serves the image via a 307 → S3. Either is "exists".
            s == 200 || s == 301 || s == 302 || s == 307
        }
        Err(_) => false,
    };
    if !exists {
        return Ok(None);
    }

    // Persist a cache hit on the MBID-by-name path so future plays
    // skip the search.
    mb.set_resolved_for_album(&album, &artist, MbResolved {
        recording_mbid: None,
        release_group_mbid: Some(release_group_mbid.clone()),
        artist_mbid: None,
    })
    .await;

    Ok(Some(CaaCover {
        url,
        release_group_mbid,
    }))
}

async fn search_release_group(album: &str, artist: &str) -> Result<Option<String>, String> {
    let query = format!(
        "release-group:\"{}\" AND artist:\"{}\"",
        album.replace('"', "\\\""),
        artist.replace('"', "\\\""),
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client: {e}"))?;
    let resp = client
        .get(format!("{MB_API_BASE}/release-group/"))
        .query(&[
            ("query", query.as_str()),
            ("fmt", "json"),
            ("limit", "5"),
        ])
        .header(reqwest::header::USER_AGENT, user_agent())
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let groups = body
        .get("release-groups")
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default();

    let album_lower = album.to_lowercase();
    // Prefer "Album" type, then any title match.
    let mut album_match: Option<String> = None;
    let mut any_match: Option<String> = None;
    for g in &groups {
        let title_match = g
            .get("title")
            .and_then(|t| t.as_str())
            .map(|t| t.to_lowercase() == album_lower)
            .unwrap_or(false);
        if !title_match {
            continue;
        }
        let id = g
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from);
        let primary = g
            .get("primary-type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if primary.eq_ignore_ascii_case("album") && album_match.is_none() {
            album_match = id.clone();
        }
        if any_match.is_none() {
            any_match = id;
        }
    }
    Ok(album_match.or(any_match))
}

// --------------------------------------------------------------------
// Track details panel
// --------------------------------------------------------------------

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MbTrackDetails {
    pub recording_mbid: Option<String>,
    pub artist_mbid: Option<String>,
    pub release_group_mbid: Option<String>,
    /// `disambiguation` field — useful when there are multiple recordings
    /// with the same title (e.g. live, demo).
    pub disambiguation: Option<String>,
    /// Year of first release for this recording.
    pub first_release_year: Option<i32>,
    /// Per-instrument or per-role artist credits — what TIDAL hides.
    pub credits: Vec<TrackCredit>,
    /// Community tags (genre-ish), sorted by votes desc.
    pub tags: Vec<TrackTag>,
    /// External resources: Wikipedia, Discogs, AllMusic, official site, etc.
    pub urls: Vec<TrackUrl>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCredit {
    pub artist_name: String,
    pub artist_mbid: Option<String>,
    /// Role string: "vocals", "lead vocals", "guitar", "writer", …
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct TrackTag {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackUrl {
    pub kind: String, // "wikipedia", "discogs", "allmusic", "homepage", …
    pub url: String,
}

/// Full enrichment for the player's track details panel. Resolves the
/// MBID by name first, then a single GET-by-id with all the includes
/// the UI cares about.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_mb_track_details(
    state: State<'_, AppState>,
    title: String,
    artist: String,
) -> Result<MbTrackDetails, SoneError> {
    log::debug!("[mb-details] {title:?} / {artist:?}");
    let mb = state.scrobble_manager.mb_lookup();
    let resolved = mb.lookup_by_name(&title, &artist).await;
    let Some(resolved) = resolved else {
        return Ok(MbTrackDetails::default());
    };
    let Some(ref mbid) = resolved.recording_mbid else {
        return Ok(MbTrackDetails {
            recording_mbid: None,
            artist_mbid: resolved.artist_mbid,
            release_group_mbid: resolved.release_group_mbid,
            ..Default::default()
        });
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| SoneError::Network(format!("client: {e}")))?;
    let url = format!("{MB_API_BASE}/recording/{mbid}");
    let resp = client
        .get(&url)
        .query(&[
            ("inc", "artist-credits+url-rels+work-rels+tags+releases"),
            ("fmt", "json"),
        ])
        .header(reqwest::header::USER_AGENT, user_agent())
        .send()
        .await;
    let body: serde_json::Value = match resp {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(v) => v,
            Err(e) => {
                log::debug!("[mb-details] parse: {e}");
                return Ok(MbTrackDetails {
                    recording_mbid: Some(mbid.clone()),
                    artist_mbid: resolved.artist_mbid,
                    release_group_mbid: resolved.release_group_mbid,
                    ..Default::default()
                });
            }
        },
        _ => {
            return Ok(MbTrackDetails {
                recording_mbid: Some(mbid.clone()),
                artist_mbid: resolved.artist_mbid,
                release_group_mbid: resolved.release_group_mbid,
                ..Default::default()
            });
        }
    };

    // Disambiguation
    let disambiguation = body
        .get("disambiguation")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    // First release year
    let first_release_year = body
        .get("first-release-date")
        .and_then(|v| v.as_str())
        .and_then(|s| s.split('-').next())
        .and_then(|y| y.parse::<i32>().ok());

    // Credits — primary artist-credit + work writers via work-rels.
    let mut credits: Vec<TrackCredit> = Vec::new();
    let mut seen_credit_keys: HashSet<String> = HashSet::new();
    if let Some(arr) = body.get("artist-credit").and_then(|v| v.as_array()) {
        for c in arr {
            let artist = c.get("artist");
            let name = artist
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from);
            let mbid_a = artist
                .and_then(|a| a.get("id"))
                .and_then(|n| n.as_str())
                .map(String::from);
            if let Some(name) = name {
                let key = format!("{}|primary", name.to_lowercase());
                if seen_credit_keys.insert(key) {
                    credits.push(TrackCredit {
                        artist_name: name,
                        artist_mbid: mbid_a,
                        role: "primary".into(),
                    });
                }
            }
        }
    }
    if let Some(rels) = body.get("relations").and_then(|v| v.as_array()) {
        for rel in rels {
            let rel_type = rel
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            // Walk into work credits (writer, composer, lyricist).
            if let Some(work) = rel.get("work") {
                if let Some(work_rels) = work.get("relations").and_then(|r| r.as_array()) {
                    for wr in work_rels {
                        let role = wr
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        let artist = wr.get("artist");
                        let name = artist
                            .and_then(|a| a.get("name"))
                            .and_then(|n| n.as_str())
                            .map(String::from);
                        let mbid_a = artist
                            .and_then(|a| a.get("id"))
                            .and_then(|n| n.as_str())
                            .map(String::from);
                        if let Some(name) = name {
                            let key = format!("{}|{}", name.to_lowercase(), role);
                            if seen_credit_keys.insert(key) {
                                credits.push(TrackCredit {
                                    artist_name: name,
                                    artist_mbid: mbid_a,
                                    role,
                                });
                            }
                        }
                    }
                }
            }
            // Direct artist relations on the recording (e.g. instruments).
            if let Some(artist) = rel.get("artist") {
                let name = artist
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(String::from);
                let mbid_a = artist
                    .get("id")
                    .and_then(|n| n.as_str())
                    .map(String::from);
                if let Some(name) = name {
                    let role = if rel_type.is_empty() {
                        "performer".to_string()
                    } else {
                        rel_type.clone()
                    };
                    let key = format!("{}|{}", name.to_lowercase(), role);
                    if seen_credit_keys.insert(key) {
                        credits.push(TrackCredit {
                            artist_name: name,
                            artist_mbid: mbid_a,
                            role,
                        });
                    }
                }
            }
        }
    }

    // Tags
    let mut tags: Vec<TrackTag> = body
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(|n| n.as_str())?.to_string();
                    let count = t.get("count").and_then(|c| c.as_i64()).unwrap_or(0);
                    Some(TrackTag { name, count })
                })
                .collect()
        })
        .unwrap_or_default();
    tags.sort_by(|a, b| b.count.cmp(&a.count));
    tags.truncate(8);

    // URL relations on the recording.
    let urls: Vec<TrackUrl> = body
        .get("relations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let t = r.get("type").and_then(|x| x.as_str())?;
                    let url = r
                        .get("url")
                        .and_then(|u| u.get("resource"))
                        .and_then(|x| x.as_str())?
                        .to_string();
                    Some(TrackUrl {
                        kind: t.to_string(),
                        url,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(MbTrackDetails {
        recording_mbid: Some(mbid.clone()),
        artist_mbid: resolved.artist_mbid,
        release_group_mbid: resolved.release_group_mbid,
        disambiguation,
        first_release_year,
        credits,
        tags,
        urls,
    })
}
