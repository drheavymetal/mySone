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

## Appendix — files & paths

| Thing                          | Path                                       |
|--------------------------------|--------------------------------------------|
| Encrypted settings             | `~/.config/sone/settings.json`             |
| Stats DB (plain SQLite)        | `~/.config/sone/stats.db`                  |
| Hooks dir                      | `~/.config/sone/hooks/`                    |
| Scrobble queue (offline cache) | `~/.config/sone/scrobble_queue.bin`        |
| MusicBrainz lookup cache       | `~/.config/sone/mb_lookup.bin`             |
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
