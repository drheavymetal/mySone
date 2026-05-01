# SONE — features added 2026-04-30 → 2026-05-01

A reference for the features merged into `master` over this stretch of work.
Each section has a one-paragraph summary, the files that hold the meat of
the implementation, and a **How to use** block with the exact UI gestures
or CLI commands.

---

## 1. Signal Path Transparency Panel

**What it is.** A modal that explains what's happening to the audio at every
stage from TIDAL bytes to your DAC: source codec / bit depth / sample rate
→ decoded format → output format → volume control mode (`hw`, `sw`,
`locked`, `gst`). Surfaces the *bit-perfect contract* — when bit-perfect is
on, software volume is hard-disabled and the panel says `Locked`.

**Files.** `src/components/SignalPathPanel.tsx`, surfacing data computed in
`src-tauri/src/audio/router.rs`.

**How to use.** Click the **QualityBadge** on the PlayerBar (the
`LOSSLESS` / `HI-RES` chip on the right of the now-playing strip). The
modal slides up. Hover any row for the underlying value. Close with `Esc`.

---

## 2. Shell hooks + transport CLI

**What it is.** Two things shipping together:

* **Shell hooks** — when a track changes / plays / pauses / resumes /
  stops, SONE executes `~/.config/sone/hooks/on-{track-change,play,pause,
  resume,stop}` if present, with track metadata in the environment.
  Useful for piping to OBS, status bars, custom scrobblers, smart-home
  triggers.
* **Transport CLI** — `sone status [--json] | play | pause | toggle |
  next | prev | stop | vol [+N|-N|N|mute|get] | help`. The CLI talks to
  the running app over a local socket; you can bind it to media keys or
  scripts.

**Files.** `src-tauri/src/hooks.rs`, `src-tauri/src/cli.rs`,
`src-tauri/src/ipc.rs`.

**How to use.**

```bash
# Drop a script in the hooks dir — it must be executable.
mkdir -p ~/.config/sone/hooks
cat > ~/.config/sone/hooks/on-track-change <<'EOF'
#!/usr/bin/env bash
echo "$(date +%H:%M:%S) $SONE_ARTIST — $SONE_TITLE" >> /tmp/sone-now.log
EOF
chmod +x ~/.config/sone/hooks/on-track-change

# CLI lives next to the binary in dev:
./src-tauri/target/debug/sone status --json
./src-tauri/target/debug/sone vol +5
./src-tauri/target/debug/sone toggle
```

Available env vars in hooks: `SONE_ARTIST`, `SONE_TITLE`, `SONE_ALBUM`,
`SONE_TRACK_ID`, `SONE_DURATION`, `SONE_QUALITY`.

---

## 3. Local listening statistics (page + redesign)

**What it is.** A SQLite database at `~/.config/sone/stats.db` that records
every play (including skips ≥5 s) and a stats page that visualises it.
The redesign on 2026-05-01 turned the plain cards into a hero with a
smoothed-Bézier daily-minutes area chart, gradient stat tiles with icons
and accent halos, podium cards (top track / artist / album), medal rank
badges (gold/silver/bronze) on top lists, and a magenta-to-orange
heatmap with a peak-slot annotation.

**Files.** `src-tauri/src/stats.rs`, `src-tauri/src/commands/stats.rs`,
`src/api/stats.ts`, `src/components/StatsPage.tsx`.

**How to use.** Sidebar → **Stats**. Tabs across the top:

* **Overview** — hero with total time + sparkline + stat tiles + crowned
  cards.
* **Top Tracks / Top Artists / Top Albums** — ranked list, top-3 wear
  medals.
* **Heatmap** — day × hour intensity grid with peak-slot tile.

Window selector pill (Week / Month / Year / All time) is in the header.
Hover any row to reveal the **MB** chip — opens the entity on
MusicBrainz (see §13).

---

## 4. Hardware volume + bit-perfect-safe routing

**What it is.** When the active output exposes hardware mixer controls
(`Master`, `PCM`, `Speaker`), the volume slider drives them directly via
ALSA — no software attenuation, no resampling. When the device has no
HW mixer (USB DAC like the HiBy R4) and bit-perfect is on, the slider is
**locked** and the Signal Path Panel shows `Locked`. The router enforces
this contract at the writer level, not just the UI: the SoftwareVolume
filter is forbidden to enter the pipeline whenever `bit_perfect=true`.

**Files.** `src-tauri/src/audio/router.rs`,
`src-tauri/src/audio/alsa_writer.rs`, `src/components/PlayerBar.tsx`.

**How to use.** Settings → Output → pick your device. Toggle
**Bit-perfect**. The lock state shows live in the QualityBadge → Signal
Path Panel.

---

## 5. Share link — browser-playable HTTP audio stream

**What it is.** A one-button "Share" action on a track or queue that
spins up a local HTTP server and gives you a URL. Anyone on the LAN can
paste that URL in a browser and hear what you're playing. The landing
page is full-featured: now-playing card, queue, transport buttons,
search bar with add-to-queue, and a quality badge that updates live via
SSE.

The server transcodes to MP3 (browser-friendly raw AAC was hitting tap
pre-roll edge cases). URL pre-fetch + a quality cache keep the next
track ready before the current one ends.

**Files.** `src-tauri/src/share/`, `src/components/ShareLink.tsx`,
landing page in `src-tauri/src/share/landing.rs` (raw HTML/JS).

**How to use.**

1. Click the **Share** icon on the now-playing bar.
2. Confirm the broadcast — a URL like `http://192.168.1.42:7777/` is
   copied to your clipboard.
3. Send it. Recipient opens it in any browser; they get the player UI
   and audio together.

To stop, click **Share** again.

---

## 6. Synced lyrics panel

**What it is.** A side panel that displays time-synced lyrics, scrolling
the active line as the song plays. Falls back gracefully to plain
unsynced lyrics when the source has none.

**Files.** `src/components/LyricsPanel.tsx`,
`src-tauri/src/commands/metadata.rs::get_track_lyrics`.

**How to use.** Player drawer → **Lyrics** tab (or `L`). The current
line highlights and auto-centers. Click any line to seek the player to
that timestamp.

---

## 7. Queue chat — natural-language queue building

**What it is.** Type a sentence (`"chill instrumental jazz for coding,
~45 min"`) and SONE asks an LLM to build a queue, runs the suggestions
through TIDAL search, and inserts the resolved tracks into your queue.

**Files.** `src/components/QueueChatPanel.tsx`,
`src-tauri/src/llm/` (OpenAI-compatible client).

**How to use.** Sidebar → **Queue chat** (chat-bubble icon). Type a
prompt. Review the proposed list, edit if needed, click **Add to
queue**. Provider/model is configured in Settings → AI.

---

## 8. Live painting mode

**What it is.** A fullscreen ambient mode where the album art breathes,
floats, and reflects in a soft halo behind the playback. Designed to
turn your laptop into a "ambient screen" for parties or focus.

**Files.** `src/components/LivePaintingMode.tsx`.

**How to use.** Press **F11** while on now-playing, or click the
"Live painting" button in the player drawer. Move the mouse to dim the
overlay; idle for a few seconds and the controls fade back out. `Esc`
exits.

---

## 9. 3D Library Galaxy → Album Constellations

**What it is.** Your favourite library as a navigable 3D star field.
Each *track* is a star, clustered around its **album** (cover acts as
the "sun" of the cluster). Albums of the same artist are joined by
**constellation lines**. The galaxy spirals outward by **decade**, with
each decade arm rendered as a soft nebula tinted by the dominant artist
hues of that era.

Bloom post-processing is tuned conservative (strength 0.32, threshold
0.62) so the structure stays readable. A search bar in the top-left
flies the camera to the matched album. Click any star to play that
track first, with the rest of the album queued in chronological order.

**Files.** `src/components/LibraryGalaxy.tsx`.

**How to use.** Sidebar → **Galaxy** icon. Drag to orbit; scroll to
zoom; press **F** in the search bar to focus and start typing
(album/artist/track). Click a star to play.

---

## 10. SSE state push + quality cache + URL pre-fetch + quality badge

**What it is.** Plumbing improvements that landed together:

* **SSE state push** — the share-link landing page subscribes to a
  Server-Sent Events stream so transport state, queue, and quality
  update without polling.
* **Quality cache** — TIDAL's per-track quality manifest is cached so
  the badge is correct on the first frame, not after the first audio
  packet.
* **URL pre-fetch** — the next track's stream URL is resolved while the
  current one is still playing, avoiding the tap pre-roll gap.
* **Quality badge** — `LOSSLESS`, `HI-RES LOSSLESS`, `MAX`, etc. shown
  on the player and the share-link landing page, glow-tinted by tier.

**Files.** `src-tauri/src/share/sse.rs`, `src-tauri/src/cache.rs`,
`src/components/QualityBadge.tsx`.

**How to use.** No UI to enable — automatic. Watch the badge update as
you skip tracks; on the share landing page, multiple browsers stay in
sync without refresh.

---

## 11. ListenBrainz history import

**What it is.** Backfill the local stats DB with your full ListenBrainz
play history. The importer pages through `GET /1/user/{user_name}/listens`
newest-first, converts each scrobble to a local play record, and uses
the dedup-aware `bulk_import_plays` so re-running the import doesn't
produce duplicates.

A `source` column was added to the `plays` table to distinguish locally
recorded plays from imported ones (`source='local'` vs.
`source='listenbrainz'`).

**Files.** `src-tauri/src/scrobble/listenbrainz.rs::fetch_listens`,
`src-tauri/src/scrobble/mod.rs::import_listenbrainz_history`,
`src-tauri/src/commands/scrobble.rs::import_listenbrainz_history`,
`src-tauri/src/stats.rs::bulk_import_plays`,
`src/components/ScrobbleModal.tsx::LbHistoryImport`.

**How to use.**

1. Open Settings → Scrobbling (sidebar → Scrobble icon).
2. Make sure ListenBrainz is connected (your token is already saved).
3. In the ListenBrainz card, click **Start** under "Import history".
4. Watch the live counter (`Page N · imported X · skipped Y dupes`).
   The walk stops automatically when it runs out of history or hits
   three consecutive duplicate-heavy pages.
5. Stats page reflects the imported plays immediately — open Stats →
   Top Tracks and select **All time**.

The importer is safe to re-run any time; only new listens are added.
For private LB profiles, flip your profile to public on
listenbrainz.org first (we send the token but the API still requires
public listens).

---

## 12. Album covers + artist photos in Stats

**What it is.** The Top X lists and podium cards in Stats now show real
album covers and artist photos pulled from TIDAL. A name-derived
gradient with initials is used as a placeholder while the cover loads,
and remains as a fallback if no match is found. Lookups go through a
localStorage cache (positive 30 d, negative 7 d) with concurrency
limited to 4 simultaneous requests.

**Files.** `src/api/coverLookup.ts`,
`src/components/StatsPage.tsx::CoverArt`.

**How to use.** Automatic — covers stream in as you scroll. Re-visiting
the page is instant (cache). To clear the cache (rare):

```js
// In the dev console:
localStorage.removeItem("sone:stats-cover-cache:v1")
```

---

## 13. MusicBrainz cross-link

**What it is.** Hover any row in Top Tracks / Top Artists / Top Albums
and a small **MB** chip appears in the right column. Click it and the
matching MusicBrainz search page opens in your default browser
(artist / release-group / recording, parameterised by name + artist).
Useful for credits, MBID grabbing, or just rabbit-holing.

**Files.** `src/components/StatsPage.tsx::MbLink`.

**How to use.** Stats → any Top tab → hover a row → click **MB ↗**.

---

## 14. MusicBrainz enrichment per play (recording, release-group, artist MBIDs)

**What it is.** Every track that starts playing now triggers a parallel
MusicBrainz lookup that resolves three identifiers:

* `recording_mbid` — the specific track recording
* `release_group_mbid` — the album as a *concept*, so reissues collapse
* `artist_mbid` — canonical artist, immune to casing/punctuation drift

When ISRC is present we still do the ISRC-by-id lookup (more reliable
for the recording), and we run a name search in parallel to pick up
the other two MBIDs. Results are cached on disk in
`mbid_name_cache.json`, keyed by `lower(title)|lower(artist)`, so
subsequent plays of the same track skip the network entirely.

The MBIDs are stored on every new row of the local stats `plays` table
(via the `recording_mbid`, `release_group_mbid`, `artist_mbid` columns
added in this migration).

**Files.** `src-tauri/src/scrobble/musicbrainz.rs::lookup_by_name`,
`src-tauri/src/scrobble/mod.rs::on_track_started`,
`src-tauri/src/stats.rs` (schema + queries).

**How to use.** Automatic — nothing to enable. Hidden side effects:

* Stats Top X dedupes by MBID when present, so "The Beatles" and
  "Beatles" stop showing as two artists; a remastered album collapses
  onto the same row as the original; same recording on different
  releases stops splitting plays.
* Scrobbles to ListenBrainz now ship the `recording_mbid` automatically
  (already did; the new lookup means more tracks resolve it).

To force a re-resolve, delete `~/.config/sone/mbid_name_cache.json`.

---

## 15. Cover Art Archive fallback for album covers

**What it is.** When TIDAL has no cover for an album (rare releases,
bootlegs, regional editions), the cover lookup falls back to MusicBrainz
→ Cover Art Archive. The backend resolves the release-group MBID,
HEAD-probes `coverartarchive.org/release-group/{mbid}/front-500`, and
returns the URL only when the image actually exists. The full https URL
is stored in the localStorage cache; `getTidalImageUrl` passes it
through unchanged.

**Files.**
`src-tauri/src/commands/musicbrainz.rs::lookup_album_cover_caa`,
`src/api/coverLookup.ts::caaAlbumCover`.

**How to use.** Automatic — Stats and any UI that uses `getAlbumCover`
benefits. To force a re-resolve, clear localStorage:

```js
localStorage.removeItem("sone:stats-cover-cache:v1")
```

---

## 16. MusicBrainz panel in the Credits tab

**What it is.** The drawer's **Credits** tab now appends a *MusicBrainz*
section beneath the existing TIDAL credits and artist bio. Renders:

* **Disambiguation** + first-release year (helps tell apart Live /
  Demo / Studio recordings of the same title).
* **Tags** — community-voted genre/mood/era tags, sorted by votes,
  capped at 8.
* **Extra credits** — writers, composers, lyricists, instrument
  performers that TIDAL doesn't expose.
* **External links** — Wikipedia, Discogs, AllMusic, Bandcamp,
  YouTube, Spotify, official homepage… one chip per relation, opens
  in the system browser.
* **View** chip — direct link to the recording's MusicBrainz page.

The section hides itself entirely when MB has nothing to add.

**Files.** `src/components/NowPlayingDrawer.tsx::MusicBrainzSection`,
`src-tauri/src/commands/musicbrainz.rs::get_mb_track_details`.

**How to use.** Drawer → **Credits** tab. Scroll past the TIDAL
credits/bio. Click any external link or chip to open in your browser.

---

## 17. Last.fm "Similar tracks" tab + tag enrichment

**What it is.** Two pieces shipping together that lean on the Last.fm
public API (no account required — uses the embedded API key only):

* **Similar tab in the drawer** — for the currently playing track,
  fetches Last.fm's collaborative-filter graph (`track.getSimilar`)
  and lists up to 25 similar tracks ranked by match score (0–100%).
  Each row shows match %, artist, and global playcount; the row is
  click-to-play, with hover-revealed *queue* and *play* buttons.
  Click resolution: SONE searches TIDAL on demand for the picked LFM
  track; if it's not on TIDAL the user gets a toast instead of a
  silent failure. Results cached 7 days in localStorage.

* **Last.fm tags in the Credits tab** — alongside the existing
  MusicBrainz tags, a separate "Last.fm tags" cluster renders below
  the MB tags. Different flavour: more mood / era / "vibe"
  (`dreamy`, `00s indie`, `running music`) where MB skews to canonical
  genre. Capped at 8, ordered by community use count, cached 30 days.

**Files.** `src-tauri/src/commands/lastfm.rs`, `src/api/lastfm.ts`,
`src/components/NowPlayingDrawer.tsx::SimilarTab` and the extended
`MusicBrainzSection` (now renamed "More info" since it covers both
sources).

**How to use.**

* Drawer → **Similar** tab (radio-tower icon). Click any row to play,
  hover to reveal queue / play buttons explicitly.
* Drawer → **Credits** tab → scroll to "More info" → tags appear in
  two clusters labelled *MusicBrainz tags* and *Last.fm tags*.

To clear cached LFM responses:

```js
localStorage.removeItem("sone:lastfm-cache:v1")
```

---

## 18. Last.fm history import + unified Stats (no source toggle)

**What it is.** The Stats page used to have a Local / ListenBrainz /
Last.fm pill that switched the view between the local DB and live
calls to the remote APIs. That's gone — replaced by a single unified
view computed entirely from the local SQLite DB. Both ListenBrainz
and Last.fm scrobbles are *imported* into that DB instead of being
queried live.

The piece that closes the loop is a **Last.fm history importer**,
mirror of the existing ListenBrainz one. Connect Last.fm in
Settings → Scrobbling and the connected card reveals an *Import
Last.fm history* panel. Hit Start and the backend pages
`user.getRecentTracks` (200 scrobbles per page) using the embedded
API key — no session token needed because we only need read-only
access to a public profile. Each page is converted into local
`PlayRecord`s with `source: "lastfm"` and pushed through the same
`bulk_import_plays` path the LB importer uses, so the dedup key
`(started_at, lower(title), lower(artist))` quietly skips any row
that already exists. That means: re-running is safe, and a track
SONE played locally (and scrobbled to LFM) won't double-count when
you import — the timestamps line up.

The walk stops on an empty page, three consecutive ≥95% duplicate
pages, the API's reported `totalPages`, or a 250-page hard cap.
Progress streams to the frontend on `import-lastfm-progress` so the
panel shows `Page N / total · imported X · skipped Y dupes` live.

Because everything lives in the same DB, every existing aggregate
(Overview totals, Top tracks/artists/albums, heatmap, the new hour
clock + discovery curve) automatically includes imported history
without any source-aware branching. The header badge now reads
**Unified · Local + imports** instead of toggling.

**Files.** `src-tauri/src/scrobble/lastfm.rs::fetch_recent_tracks`
(API call), `src-tauri/src/scrobble/mod.rs::import_lastfm_history`
(walker + `bulk_import_plays`), `src-tauri/src/commands/scrobble.rs`
(Tauri command), `src/api/stats.ts::importLastfmHistory`,
`src/components/ScrobbleModal.tsx::LfmHistoryImport`.

**How to use.** Sidebar → Scrobble → connect Last.fm → in the
connected card, *Import Last.fm history* → **Start**. Watch the page
counter tick. When it finishes, every Stats tab includes those
scrobbles next to your SONE-recorded plays.

---

## 19. New Stats charts: hour clock + discovery curve

**What it is.** Two new visualisations alongside the heatmap, both
computed from the same unified local DB:

* **Hour clock** (Patterns tab, next to the heatmap). A radial 24-h
  bar chart. Each spoke is one hour of day, length = total minutes
  listened in that hour across the window. Midnight at the top, noon
  at the bottom. The colour ramp follows a sundial: deep blues in the
  pre-dawn hours, golden tones around noon, rose-magenta in the
  evening, back to blue at midnight. The bright accent dot on the rim
  marks your peak hour, the centre prints the total. Distinct from
  the heatmap (which is dow×hour) — this collapses across days to
  answer *am I a morning, afternoon, evening or late-night listener?*
  in one glance.
* **Discovery curve** (Overview tab). A cumulative line of
  *first-time encounters* — for each day in the window, count the
  artists and tracks whose very first play in your entire local
  history happened that day, then accumulate. Solid line = artists,
  dashed = tracks, soft bars under = the per-day count of new
  artists. A steep curve means you're exploring; a flat tail means
  you're in your comfort zone. The dataset is densified to one row
  per day so silent days flatten the curve visibly instead of being
  hidden by the chart's interpolation.

Both queries reuse the existing `recording_mbid` /
`artist_mbid` / `release_group_mbid` enrichment to identify
artists/tracks robustly across name drift.

The "Heatmap" tab is renamed **Patterns** to reflect that it now
holds both temporal-pattern charts.

**Files.** `src-tauri/src/stats.rs` (`hour_minutes`,
`discovery_curve`), `src-tauri/src/commands/stats.rs`
(`get_hour_minutes`, `get_discovery_curve`),
`src/api/stats.ts` (typed wrappers),
`src/components/StatsPage.tsx` (`HourClockCard`, `DiscoveryCard`,
`PatternsTab`).

**How to use.** Stats → Overview to see the discovery curve, Stats →
Patterns to see the heatmap + hour clock side by side. Both react to
the Week / Month / Year / All window selector.

---

## 20. Heatmap colour ramp: red → yellow → green

**What it is.** The day × hour heatmap got a stoplight-scale ramp:
silent slots are near-empty, low-activity slots tint **red**, mid
activity yellow/lime, peak activity bright **green**. Hue is a clean
linear interpolation 0° → 120° with constant saturation/lightness so
the gradient reads as a single smooth ramp rather than the previous
multi-stop blue/magenta/orange rainbow.

The legend strip in the heatmap header reflects the new ramp.

**Files.** `src/components/StatsPage.tsx::heatColor`.

**How to use.** Stats → Heatmap. No knob, no pref — just looks
right now.

---

## Appendix — files & paths

| Thing                          | Path                                       |
|--------------------------------|--------------------------------------------|
| Encrypted settings             | `~/.config/sone/settings.json`             |
| Stats DB (plain SQLite)        | `~/.config/sone/stats.db`                  |
| Hooks dir                      | `~/.config/sone/hooks/`                    |
| Scrobble queue (offline cache) | `~/.config/sone/scrobble_queue.bin`        |
| MusicBrainz ISRC cache         | `~/.config/sone/mbid_cache.json`           |
| MusicBrainz name cache         | `~/.config/sone/mbid_name_cache.json`      |
| Dev binary                     | `src-tauri/target/debug/sone`              |
| Dev log                        | `/tmp/sone-dev.log`                        |

Validation baselines (last known good on 2026-05-01):

```
cargo check                                     → Finished, 0 errors
cargo clippy -- -D warnings                     → 13 errors (pre-existing)
cargo fmt --check                               → 74 diffs (pre-existing)
npx tsc --noEmit                                → exit 0
npx eslint src/                                 → 230 problems / 4 errors
                                                  (pre-existing, set-state-
                                                  in-effect warnings)
npm run build                                   → built in <3 s
```
