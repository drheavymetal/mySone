# SONE Classical — arquitectura viva

**Estado**: post Phase 2 + auditoría gapless inicial Phase 3 (B3.0).
**Última actualización**: 2026-05-02.

> Este doc es la **síntesis técnica** de la arquitectura tal y como existe en el código. Se distingue del CLASSICAL_DESIGN.md (que es **prescriptivo** — qué debe construirse) en que ARCHITECTURE.md es **descriptivo** — qué existe ahora mismo y cómo encaja.

---

## 1. Visión 30k pies

El Classical Hub es un **módulo aditivo** sobre la arquitectura SONE original. Sus piezas en producción tras Phase 1 + Phase 2:

- **Backend**: nuevo crate-internal module `src-tauri/src/classical/` con un `CatalogService` que orquesta cuatro providers (MusicBrainz, Wikipedia, Tidal, OpenOpus) detrás del trait `ClassicalProvider`. Cache vía `DiskCache::StaticMeta` (TTL 7d, SWR 30d). Tauri commands aditivos en `src-tauri/src/commands/classical.rs`.
- **Frontend**: nuevo directorio `src/components/classical/` con WorkPage, ComposerPage, ClassicalHubPage, BrowseComposers/Periods/Genres/Era, RecordingRow, MovementList, ConfidenceBadge, ClassicalWorkLink. Domain types en `src/types/classical.ts`, wrappers Tauri en `src/api/classical.ts`. Routing aditivo (`classical://hub`, `classical://composer/{mbid}`, `classical://work/{mbid}`, `classical://browse/{axis}`, `classical://era/{era}`). Pill "Classical Hub" en ExplorePage.
- **DB**: migración aditiva — columna `work_mbid` en tabla `plays`, nueva tabla `classical_favorites` con índice, sin DROP/ALTER destructivos.
- **Scrobble**: extensión post-track-start best-effort para resolver `work_mbid` parent vía `WorkMbidResolver` (trait que el `CatalogService` implementa). El scrobble manager mantiene cero acoplamiento compile-time con el módulo classical.

El audio path NO ha sido modificado. El bit-perfect contract está intacto (ver §5).

---

## 2. Capas y módulos backend

### 2.1 Catalog service (`src-tauri/src/classical/catalog.rs`)

Único punto de entrada para Tauri commands de catálogo. Métodos:

- `get_work(mbid)` — fetch + cache + cascade matcher. Devuelve `Work` con movements + recordings (cada uno con `match_confidence`).
- `get_recording(mbid, work_mbid)` — pequeño detalle (Phase 1: stub que retorna dummy; Phase 5 hidrata).
- `get_composer(mbid)` — composer + bio Wikipedia.
- `resolve_work_for_recording(recording_mbid)` — implementación del trait `scrobble::WorkMbidResolver`.
- `list_top_composers(limit)` — Phase 2, OpenOpus snapshot.
- `list_composers_by_era(era)` — Phase 2.
- `list_works_by_composer(composer_mbid, genre)` — Phase 2, MB browse + OpenOpus title-match para `popular` flag.

Internamente: `Arc<DiskCache>` compartido, `Arc<MbRateLimiter>` (1 req/s), providers como `Arc<dyn ClassicalProvider>`.

### 2.2 Provider trait (`src-tauri/src/classical/providers/mod.rs`)

```rust
#[async_trait]
pub trait ClassicalProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn enrich_composer(&self, c: &mut Composer) -> Result<(), SoneError>;
    async fn enrich_work(&self, w: &mut Work) -> Result<(), SoneError>;
    async fn enrich_recording(&self, r: &mut Recording) -> Result<(), SoneError>;
}
```

Implementaciones presentes (Phase 1 + 2):

| Provider | File | Tareas |
|---|---|---|
| `MusicBrainzProvider` | `providers/musicbrainz.rs` | fetch_work + recordings browse (1 call con isrcs + artist-credits + releases inline), fetch_recording_detail, fetch_composer, browse_works_by_artist; parsers (BWV/K/D/RV/Hob/HWV/Op/key/work-type/era) |
| `WikipediaProvider` | `providers/wikipedia.rs` | REST summary multilingual |
| `TidalProvider` | `providers/tidal.rs` | lookup_by_isrc + canonical search wrapper + query builder |
| `OpenOpusProvider` | `providers/openopus.rs` | Phase 2 — parse en `OnceLock` del snapshot embedded `data/openopus.json` (227 KB). Top composers, by-era, works |

Pendiente (Phase 5+): `WikidataProvider` (SPARQL), `LastfmProvider` adaptado, `DiscogsProvider`.

### 2.3 Cache strategy

Reusa `DiskCache::StaticMeta` (TTL 7d, SWR 30d) para todo el catálogo classical. Keys:

- `classical:work:v1:{mbid}`
- `classical:recording:v1:{mbid}`
- `classical:composer:v1:{mbid}`
- `classical:composer-works:v1:{mbid}:{genre|nogenre}`

§3.3 del doc maestro lista 30d/24h ideal; el `DiskCache` actual no expone TTL custom por key. El delta es cosmético — los entries no expiran en el horizonte de una sesión.

### 2.4 Stats DB schema additions

Migración aditiva sobre `stats.rs`:

- Columna nueva `work_mbid TEXT NULL` en tabla `plays`. Backfill via `on_track_started` (best-effort, post-track-start).
- Tabla nueva `classical_favorites (id, kind, mbid, display_name, added_at, UNIQUE(kind,mbid))` con índice por `kind`. Phase 6 la consumirá.

---

## 3. Capas frontend

### 3.1 Domain types (`src/types/classical.ts`)

Mirror exacto del backend con serde camelCase. Definidos:
- `Era` (con `Unknown` como sentinel — NO browseable)
- `WorkType`, `Genre`
- `MatchConfidence` (IsrcBound | TextSearchInferred | NotFound)
- `PerformerCredit`, `PerformerCreditWithRole`
- `CatalogueNumber`, `LifeEvent`
- `Composer`, `Movement`, `Recording`, `Work`
- `ComposerSummary`, `WorkSummary` (browse projections)

Helpers UI: `BROWSEABLE_ERAS`, `eraLabel`, `eraYearSpan`, `genreLabel`, `workTypeLabel`.

### 3.2 API wrappers (`src/api/classical.ts`)

Wrappers tipados sobre `invoke()` para los 8 commands actuales (5 Phase 1 + 3 Phase 2). Phase 3 añadirá `getClassicalMovementForTrack` y un listener de event subscription en `ClassicalWorkLink`.

### 3.3 Componentes presentes

```
src/components/classical/
├── ClassicalHubPage.tsx        (Phase 2)
├── ComposerPage.tsx            (Phase 2)
├── WorkPage.tsx                (Phase 1)
├── RecordingRow.tsx            (Phase 1)
├── MovementList.tsx            (Phase 1)
├── ConfidenceBadge.tsx         (Phase 1)
├── ClassicalWorkLink.tsx       (Phase 1 — polling, refactor en Phase 3)
├── BrowseComposers.tsx         (Phase 2)
├── BrowsePeriods.tsx           (Phase 2)
├── BrowseGenres.tsx            (Phase 2 — informacional, drill-down en Phase 5)
├── BrowseEra.tsx               (Phase 2)
├── EraBadge.tsx                (Phase 2)
├── ComposerCard.tsx            (Phase 2)
└── WorkSummaryCard.tsx         (Phase 2)
```

---

## 4. Flujos clave

### 4.1 Open Work page (cold cache)

```
WorkPage(mbid) mount
  → getClassicalWork(mbid)
    → CatalogService::get_work
      → cache miss
      → MB call 1: work?inc=artist-rels+composer-rels  (~1.1s)
      → MB call 2: recording browse with isrcs+artist-credits+releases inline  (~1.1s)
      → matching::Matcher cascade
          for each recording:
            if recording.isrc → TidalProvider::lookup_by_isrc  (~50ms)
            else → TidalProvider::canonical_search → score → bind  (~50ms)
      → Wikipedia REST summary  (~500ms)
      → cache.set
      → return Work
  → WorkPage renders movements + recordings
```

Total cold: 2 MB calls (rate-limited a 1s c/u) + ~60 Tidal lookups en paralelo + 1 Wikipedia. Realista 3–10s contra warm < 300ms.

### 4.2 Bridge ISRC → Tidal

Phase 1 implementa el cascade D-010:
1. Recording con `isrcs[]` no vacío → `TidalProvider::lookup_by_isrc(isrc)` → bind directo. Confidence: `IsrcBound`.
2. Recording sin ISRC → `build_canonical_query(composer, work, conductor, year)` → `TidalProvider::canonical_search(q)` → top-N candidatos → `Matcher::score()` → si > 0.6 bind. Confidence: `TextSearchInferred` con score visible. Si < 0.6 → `NotFound`.

Movement penalty −0.25 evita binding a un movimiento individual cuando se busca el work entero.

### 4.3 Work resolution post-track-start

```
on_track_started(track):
  ScrobbleManager:
    1. Replace current_track (lock + unlock fast)
    2. Concurrent: dispatch_scrobble(prev) + fire_now_playing(new)
    3. Spawn background task:
       a. Parallel ISRC + name lookups → recording_mbid + release_group_mbid + artist_mbid
       b. Apply MBIDs to live track if same_track guard passes
       c. If has resolver and recording_mbid resolved:
            work_mbid = resolver.resolve_work_for_recording(recording_mbid)
            apply work_mbid if same_track guard still passes
       d. (Phase 3 add) emit "classical:work-resolved" event
```

Cero impacto en el critical path: scrobble fire ocurre antes del lookup, y el work resolution es absolutamente best-effort.

---

## 5. Bit-perfect path — invariante crítica

**El Classical Hub no modifica, en ningún punto, el audio path o el contrato bit-perfect.**

### 5.1 Mecánica del contrato

Citada de `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/feedback_bitperfect_contract.md`:

1. **Capa router** (`src-tauri/src/lib.rs::route_volume_change`, líneas 491-539):
   - Cuando `bit_perfect=true`, solo permite `VolumeRoute::Hw` (mixer) o `VolumeRoute::Locked` (no-op).
   - `VolumeRoute::Sw` está prohibido.
   - Cuando no hay HW control y bit_perfect on → `Locked` (frontend bloquea slider, audio_player.set_volume(1.0) hard).

2. **Writer guard** (`src-tauri/src/audio.rs` writer thread, líneas 988-992):
   - Defense in depth: el writer rechaza aplicar volumen SW cuando `bit_perfect=true`. Aunque un futuro bug bypassara el router, el writer no escala samples.
   ```rust
   if !bit_perfect {
       let vol = f32::from_bits(combined_vol.load(Ordering::Relaxed));
       apply_volume(&mut chunk.data, &current_fmt, vol);
   }
   ```

### 5.2 Implicaciones para el Hub

- El Hub es **catálogo + UI** — nunca llama a `audio_player`, nunca toca `combined_vol`, nunca emite WriterCommands.
- WorkPage / ComposerPage / Browse pages reproducen tracks vía el `playTrack()` existente del frontend (`usePlaybackActions`), exactamente el mismo path que cualquier ruta no-classical.
- Cambios cosméticos en el player (Phase 3: work title persistente, movement indicator, "Attacca →") son **read-only** al estado de routing — leen de atoms y emit listeners, nunca escriben.

Si un agente propone tocar `lib.rs::route_volume_change` o el alsa-writer thread como parte de cualquier feature classical, **escalar inmediatamente al usuario**. El supervisor solo no decide.

---

## 6. Gapless contract — auditoría Phase 3 B3.0

### 6.1 Modos de pipeline

`PlaybackBackend` tiene dos variantes (`audio.rs:75-95`):

- **`Normal`**: GStreamer pipeline con `autoaudiosink` → mixer del sistema (PulseAudio/PipeWire). El mixer gestiona transiciones internamente.
- **`DirectAlsa`**: GStreamer pipeline con `appsink` → writer thread separado → `alsa::PCM`. El writer permanece vivo entre tracks (línea 75: "ALSA writer sender + thread handle live as separate state variables so they persist across PlayUrl calls (track changes keep DAC open)").

### 6.2 Transición track → track en `DirectAlsa` (path bit-perfect)

Secuencia (`audio.rs:921-1001`):

1. **Track antiguo termina**: bus watcher recibe `MessageView::Eos` y envía `WriterCommand::EndOfTrack { emit_finished: true, generation }` al writer (línea 1370-1376).

2. **Writer drena el ring ALSA**: 
   ```
   drain_writer_rx + write_silence(silence_buf)
   ```
   El `silence_buf` se escribe **una sola vez** para asegurar que ALSA termina de drenar el último chunk válido sin underrun. NO es un fade artificial; es la cantidad mínima de silencio para que `pcm.delay()` esté coherente.

3. **Writer entra en idle loop** (línea 942):
   ```
   loop {
       write_silence(pcm, silence_buf)
       try_recv() ⇒
         Ok(Data(chunk)) → break to main loop with chunk
         Ok(Shutdown)    → break 'main
         Empty           → continue idle
   }
   ```
   Esto mantiene el DAC clock vivo entre tracks. **El gap percibible depende de cuánto tarda el siguiente PlayUrl en llegar y producir el primer chunk en `appsink`**, NO de un silencio artificial añadido por el writer.

4. **Llega `WriterCommand::Data(chunk)` del nuevo track**:
   - Si `chunk.format == current_fmt` → flush stale silence (`pcm.drop().ok(); pcm.prepare().ok();`) → `write_bytes(chunk)` inmediatamente. Esto es el **path gapless real** — la transición es <50ms si el siguiente chunk llega en menos de la duración del silence_buf.
   - Si `chunk.format != current_fmt` → `reopen_alsa(new_fmt)` → write. Aquí sí hay un retraso medible (típicamente 10-30ms en HDA, más en USB-DAC). Para movements del **mismo recording** esto NO ocurre: misma sample rate, mismo bit depth, mismo formato.

5. **Generation guard** (líneas 922, 951): cualquier chunk con `generation < writer_gen` se descarta. Esto evita que un nuevo PlayUrl con bump de generation reciba mezcla de samples del track previo.

### 6.3 Por qué Phase 3 NO necesita modificar el writer

- El attacca de **Beethoven 5 III→IV** dentro del mismo recording = el frontend hace `playTrack(next_track)` que dispara `PlayUrl`. El writer mantiene el DAC abierto (idle loop), recibe el primer chunk del IV, formato idéntico (es el mismo album/release), `pcm.drop().ok();pcm.prepare().ok()` → `write_bytes()`. **Latencia sólo limitada por GStreamer pipeline rebuild + Tidal stream initial latency**.
- Para que esto fallara, GStreamer tendría que tardar > 50ms en producir el primer chunk del nuevo track — improbable cuando el Tidal stream URL ya está pre-resuelto.

**Conclusión auditoría B3.0**: el contrato gapless en `DirectAlsa` ya entrega gaps < 50ms en el caso típico (mismo formato entre movements). Phase 3 verifica esto con QA manual sin tocar el writer.

### 6.4 Riesgo conocido — formato distinto entre movements

Improbable en la práctica clásica (un álbum Tidal entrega los 4 movements en mismo formato), pero el writer sí inserta latencia adicional al `reopen_alsa` si los formatos difieren. No es atribuible al Hub. Documentado como riesgo de §9 doc maestro y mitigable con QA manual; no actionable en Phase 3.

---

## 7. Decisiones diferidas

- **D-016** — gapless gate split en deterministic + manual instrumented. La parte E2E con captura de audio queda fuera del scope autonomous.
- **Pre-warm canon** — Phase 6.
- **WikidataProvider** — Phase 5.
- **Editor's Choice manual** — Phase 5.
- **Listening guides** — Phase 5+.
- **Pagination MB > 100 works** — Phase 5.
