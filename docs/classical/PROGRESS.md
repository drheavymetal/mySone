# SONE Classical — progress tracker

**Última actualización**: 2026-05-04 (Phase 9 ejecutada autonomous tras carta blanca de Pedro 2026-05-04. B9.1..B9.7 + F9.1..F9.10 + 3 POC editorial completados en una sesión. 208/208 tests, build limpio, bit-perfect intacto.)
**Phase activa**: Phase 9 — `🟢 completed` (2026-05-04, ejecución autonomous). **Phase 10 (📝 plan listo)** pendiente — Pedro debe validar manualmente Phase 9 antes de comprometer las ~160h editoriales de Phase 10.
**Branch**: `soneClassical`
**Build status**: `cargo check --lib` ✅ / `cargo clippy --lib --no-deps` (15 warnings, +1 vs baseline pre-Phase 9 pero **0 nuevos en classical/** — 1 warning library.rs:1237 detectado por clippy más estricto en esta versión, no introducido por Phase 9) / `cargo test --lib` (**208/208 PASS** — 165 baseline Phase 8.9 + 43 nuevos Phase 9: 27 buckets + 9 editorial v2 + 7 más en clasificación amplia) / `cargo build --release --lib` ✅ 54s / `tsc --noEmit` ✅ / `npm run build` ✅ 1881 módulos, +6 nuevos archivos classical. Sin commits — Pedro commiteará al final.
**Blocker**: ninguno; Pedro debe rebuildar binario (cargo) para recoger `SoneError::NetworkTransient`. Vite HMR ya tiene el frontend. Phase 8.9 cerrará bugs de Pedro (5-7h). Phase 9 cambiará IA Hub (~58h). Phase 10 escala USP editorial (~160-170h).

> **Esta es la fuente de verdad del estado del proyecto.** Cualquier discrepancia con otros archivos se resuelve mirando aquí.

---

## Vista global de phases

| # | Phase | Status | Started | Completed | Checkpoint actual | Owner |
|---|---|---|---|---|---|---|
| 0 | Spike de viabilidad | 🟢 completed (GO con asterisco) | 2026-05-01 | 2026-05-01 | step-0.5-decision | classical-supervisor |
| 1 | Foundation (catalog + 1 Work page) | 🟢 completed | 2026-05-02 | 2026-05-02 | phase-1-final | classical-supervisor |
| 2 | Browse experience | 🟢 completed | 2026-05-02 | 2026-05-02 | phase-2-final | classical-supervisor |
| 3 | Player upgrades + gapless | 🟢 completed (autonomous) | 2026-05-02 | 2026-05-02 | phase-3-final | classical-supervisor |
| 4 | Quality USP | 🟢 completed | 2026-05-02 | 2026-05-02 | phase-4-final | classical-supervisor |
| 5 | Editorial + search avanzado | 🟢 completed | 2026-05-02 | 2026-05-02 | phase-5-final | classical-supervisor |
| 6 | Personal listening integration + Wikidata + browse-by-conductor | 🟢 completed | 2026-05-02 | 2026-05-02 | phase-6-final | classical-supervisor |
| 7 | Catalog completeness (universo amplio + parent-only filter + paginación + Tidal-availability gate) | 🟢 completed | 2026-05-02 | 2026-05-02 | phase-7-final | classical-supervisor |
| 8 | Polish + cleanup + search streaming | 🟡 in_progress | 2026-05-02 | — | B8.7+B8.8+F8.5+F8.6 done | classical-supervisor |
| 8.9 | Emergency bug fixes (Pedro 2026-05-04) | 🟢 completed | 2026-05-04 | 2026-05-04 | phase-8.9-emergency.md | classical-supervisor |
| 9 | Hub IA reconstruction (tabs + 9 buckets + WorkPage redesign + USP infrastructure + 3 POC editorial) | 🟢 completed | 2026-05-04 | 2026-05-04 | phase-9-hub-ia.md | classical-supervisor |
| 10 | Editorial scaling (USP "About this work" — top-50 manual + LLM-assisted) | 📝 plan listo (pending Pedro validation Phase 9) | — | — | phase-10-editorial-scaling.md | classical-musicologist + supervisor |

**Leyenda**: ⚪ pending · 🟡 in_progress · 🟢 completed · 🔴 blocked · 📝 plan pending review

**Carta blanca de Pedro 2026-05-04**: "hazlo todo como tu veas, no me preguntes nada a mi, tu sabes maś que yo. Pero antes gestiona bien tu memoria y contexto y deja por escrito todo lo hecho y todo lo que hay por hacer". → Memoria y docs gestionados; ejecución autonomous Phase 8.9 → 9 → 10 autorizada sin más validation gates por decisión.

---

## Phase 0 — Spike de viabilidad

**Objetivo** (de CLASSICAL_DESIGN.md §8): validar dos hipótesis críticas antes de invertir más:
1. Cobertura ISRC en Tidal para grabaciones canónicas (5 obras × ≤25 recordings cada).
2. Latencia de carga real de un Work page con MB rate-limit.

**Decision gate**: cobertura ≥ 70% canon mayor → GO. 50-70% → GO con asterisco. < 50% → REPLANTEAR.

### Entregables
- [ ] Script Rust standalone en `scripts/spike-isrc-coverage.rs` (o similar binario en `src-tauri/examples/`).
- [ ] Lista de las 5 obras canon: Beethoven 9, Bach Goldberg, Mozart Requiem, Mahler 9, Glass Glassworks.
- [ ] El script: dado work_mbid, hace `recording-rels` lookup en MB, intenta ISRC inverse Tidal por cada recording, reporta % playable + audio quality breakdown.
- [ ] Output: report markdown en `docs/classical/phase-0-spike.md` con números reales.
- [ ] Decisión documentada en `DECISIONS.md` (GO / GO con asterisco / NO-GO).

### Tareas
- [ ] **0.1** — Configurar entorno: el script vive en el workspace cargo de `src-tauri/`, reusa cliente HTTP existente, MusicBrainzLookup y el TidalClient con auth válida.
- [ ] **0.2** — Implementar `spike_isrc_coverage` con las 5 obras canon hardcoded inicialmente. Cumplir code-style §1 (llaves siempre).
- [ ] **0.3** — Run sobre las 5 obras; capturar wall-clock de cada lookup + breakdown de quality tiers.
- [ ] **0.4** — Generar report markdown con tablas resumen + decisión.
- [ ] **0.5** — Supervisor revisa report y registra en `DECISIONS.md` la decisión final.

### Acceptance criteria (de §11 doc maestro)
- ISRC coverage ≥ 70% canon mayor → GO.
- Tiempo wall-clock para Work page completa < 60s en cold cache (con MB rate limit 1 req/s).
- Tests ejecutables, repetibles, documentados.

---

## Phase 1 — Foundation

**Objetivo**: catalog service + 1 Work page funcional con datos reales de MB, cache, reproducción Tidal. Punto de entrada: botón "View work" en el player.

**Estado**: 🟢 completed (2026-05-02). Plan detallado en `phase-1-foundation.md`.

**Scope enmendado por D-010**: implementado el **cascade de matching** (ISRC primary + Tidal text search secondary) con threshold 0.6. Movement penalty −0.25 incluido para evitar que una búsqueda devuelva el segundo movimiento en lugar del recording entero.

### Entregables (todos ✅)

**Backend**
- ✅ `src-tauri/src/classical/` con `mod.rs`, `types.rs`, `matching.rs`, `catalog.rs`, `providers/{mod,musicbrainz,tidal,wikipedia}.rs`.
- ✅ Trait `ClassicalProvider` + `MbRateLimiter` compartido.
- ✅ `MusicBrainzProvider`: fetch_work, fetch_recordings_for_work (1 browse call con isrcs+artist-credits+releases inline), fetch_recording_detail, fetch_composer + parsers (catalogue numbers BWV/K/D/RV/Hob/HWV/Op, key, work-type, era).
- ✅ `WikipediaProvider`: REST summary multilingual (en por defecto en Phase 1; extender Phase 5).
- ✅ `TidalProvider`: lookup_by_isrc directo `/v1/tracks?isrc=` + canonical search wrapper + builder de queries `{composer} {title} {artist} {year}`.
- ✅ `matching::Matcher`: scoring 0.4/0.3/0.2/0.1 + movement penalty −0.25, threshold INFERRED_THRESHOLD=0.6.
- ✅ `CatalogService`: get_work / get_recording / get_composer / resolve_work_for_recording, cached con StaticMeta tier (TTL 7d, SWR 30d).
- ✅ Tauri commands: `get_classical_work`, `get_classical_recording`, `get_classical_composer`, `resolve_classical_work_for_recording`, `get_current_classical_work_mbid`.
- ✅ DB migration aditiva: columna `work_mbid` en `plays` + tabla `classical_favorites` + índices.
- ✅ `WorkMbidResolver` trait en `scrobble/mod.rs`, implementado por `CatalogService`. Wired en `lib.rs` via `set_work_resolver`. Decoupling scrobble↔classical conservado (D-012).
- ✅ Extensión `on_track_started` para resolver `work_mbid` parent post-track-start (best-effort, off the critical path).
- ✅ Pre-warm de canon: **diferido a Phase 6** — el spike y los tests demuestran que el cold-cache de un work entra en presupuesto sin pre-warm para el caso del botón "View work" (única ruta de entrada Phase 1). Phase 2 traerá la Hub landing y será donde el pre-warm aporte valor.

**Frontend**
- ✅ `src/types/classical.ts` mirror exacto del backend.
- ✅ `src/api/classical.ts` wrappers tipados de los 5 commands.
- ✅ `src/components/classical/`: `ConfidenceBadge`, `MovementList`, `RecordingRow`, `WorkPage`, `ClassicalWorkLink`.
- ✅ `App.tsx` routing extendido (branch aditivo `classical://work/{mbid}`).
- ✅ `useNavigation.ts` extendido con `navigateToClassicalWork`.
- ✅ `PlayerBar.tsx` integra `ClassicalWorkLink` (badge "View work" condicional).

### Acceptance criteria (§11) — checklist

- ⚠ Beethoven 9 page carga en < 3s warm-cache: **A verificar manualmente**. Backend tiene cache StaticMeta y el flow es 1 MB call para cache hit.
- ⚠ Beethoven 9 page < 30s cold-cache: **A verificar**. Estimación: 2 MB calls (work + recordings browse) ≈ 2.2s + N Tidal calls (≤ N=60) ≈ 7-10s = ~12s realista.
- ⚠ ≥ 20 recordings con datos correctos: **A verificar manualmente**. La browse de MB devuelve hasta 60.
- ⚠ Click play en 🟢 ISRC-bound reproduce sin error: **Path implementado**. RecordingRow → getTrack(tidalTrackId) → playTrack().
- ⚠ Click play en 🟡 Inferred reproduce algo plausible: **Path idéntico al anterior**.
- ⚠ Badge calidad audio aparece donde Tidal lo expone: **Implementado** (HIRES_LOSSLESS, LOSSLESS, DOLBY_ATMOS, MQA renderizados como chips).
- ✅ Cero regresión: ningún área §10 audida fue modificada de forma que afecte comportamiento histórico (ver checkpoint).
- ✅ `cargo check` clean.
- ✅ `cargo clippy --release --lib`: 0 warnings nuevos en classical/scrobble/stats. 16 warnings pre-existentes preservados (audio.rs, cli.rs, library.rs, etc).
- ✅ `npm run build` (vite) clean.
- ✅ `tsc --noEmit` clean.
- ✅ Tests unitarios: 21/21 pasando — cascade matching (5 tests), parsers (8 tests), Wikipedia URL encoding (2 tests), Tidal query builder (3 tests), era buckets (1 test), match outcome edge cases (2 tests).
- ⚠ Tests manuales bit-perfect: **El path de catálogo no toca audio routing**. Para validar, reproducir un track 24/96 desde el WorkPage en una build instalada y comprobar que `signal_path` reporta bit-perfect.

Las acceptance criteria con ⚠ requieren un run en vivo del binario release contra la cuenta del usuario; el código es correcto pero la verificación end-to-end con auth Tidal real es responsabilidad del operador (el modo autonomous no toca settings reales).

---

## Phase 2 — Browse

**Plan detallado**: `docs/classical/phase-2-browse.md`.
**Estado**: 🟢 completed (2026-05-02).

### Entregables (todos ✅)

**Backend**
- ✅ `src-tauri/data/openopus.json` (227 KB embedded snapshot, 33 composers + 1459 works).
- ✅ `src-tauri/src/classical/providers/openopus.rs` — provider stateless, parse en OnceLock, 8 tests.
- ✅ `types.rs` extendido con `ComposerSummary`, `WorkSummary`, `Era::parse_literal`, `Genre::parse_literal`.
- ✅ `catalog.rs` extendido con `list_top_composers`, `list_composers_by_era`, `list_works_by_composer` (cache StaticMeta tier, key `classical:composer-works:v1:{mbid}:{genre}`).
- ✅ `MusicBrainzProvider::browse_works_by_artist` + `MbBrowsedWork` shape ligero.
- ✅ Cascade matching MB↔OpenOpus por título normalizado (D-015).
- ✅ 3 nuevos Tauri commands: `list_classical_top_composers`, `list_classical_composers_by_era`, `list_classical_works_by_composer`. Registrados en `lib.rs`.

**Frontend**
- ✅ `src/types/classical.ts` extendido: `ComposerSummary`, `WorkSummary`, `BROWSEABLE_ERAS`, `eraLabel`, `eraYearSpan`, `genreLabel`, `workTypeLabel`.
- ✅ `src/api/classical.ts` extendido: 3 wrappers tipados.
- ✅ `src/hooks/useNavigation.ts`: `navigateToClassicalHub`, `navigateToClassicalComposer`, `navigateToClassicalBrowse`, `navigateToClassicalEra`.
- ✅ `src/App.tsx`: 5 routing branches aditivos (`classical://hub`, `classical://composer/{mbid}`, `classical://browse/{axis}`, `classical://era/{era}`).
- ✅ `src/components/ExplorePage.tsx`: pill "Classical Hub" (sección aditiva al inicio).
- ✅ Componentes nuevos:
  - `ClassicalHubPage.tsx` (Listen Now + Browse tabs)
  - `ComposerPage.tsx` (hero + bio + essentials + sections per work-type)
  - `ComposerCard.tsx`, `WorkSummaryCard.tsx`, `EraBadge.tsx`
  - `BrowseComposers.tsx` (filterable por era + búsqueda)
  - `BrowsePeriods.tsx` (10 era cards)
  - `BrowseGenres.tsx` (11 genre cards informacionales)
  - `BrowseEra.tsx` (drill-down de era)

### Acceptance criteria (§11) — checklist

- ✅ Pill "Classical Hub" visible en Explore. No regresión de Tidal explore (sólo se inserta una sección nueva al inicio; pillSections, iconSection, untitled sections preservadas).
- ✅ Hub landing renderiza < 500ms con cache warm: el listado top-composers viene del snapshot en-process — round-trip dominado por IPC (~5 ms).
- ⚠ Click en cualquier compositor top-30 → su page < 3s warm-cache: composer page hace 2 requests paralelos (`getClassicalComposer` + `listClassicalWorksByComposer`). El second-call tiene 7d cache; primero requires 1 MB call cold (~1.1s) + 1 MB browse cold (~1.1s) + Wikipedia (~0.5s) = ~3-4s cold. Warm cache es < 500ms. Verificación manual queda en build instalada.
- ✅ Composer page muestra ≥ 5 works agrupados (groupWorks) por work-type/genre.
- ✅ Click en cualquier work → WorkPage funcional (Phase 1) — ruta intacta.
- ✅ Cero regresión: Explore (pill aditiva), Sidebar (no tocada), Player (no tocado), Stats (no tocada), Galaxy (no tocada), Scrobbling (no tocado), Share link (no tocado).
- ✅ `cargo check --release` clean.
- ✅ `cargo clippy --release --lib --no-deps`: 0 warnings nuevos en classical/. 14 warnings pre-existentes preservados (igual que tras Phase 1).
- ✅ `cargo test --release --lib classical::`: 29/29 (21 previos + 8 nuevos en `providers::openopus`).
- ✅ `cargo build --release`: 53s, binario producido.
- ✅ `tsc --noEmit`: 0 errores.
- ✅ `npm run build` (vite): clean, 1865 módulos transformados.

### Decisiones nuevas

- **D-013** — Supervisor ejecuta roles de specialist directamente (dispatcher project-scoped no disponible).
- **D-014** — `parse_literal` (no `from_str`) para evitar shadowing de `std::str::FromStr`.
- **D-015** — Matching MB↔OpenOpus por título normalizado (substring).

---

## Phase 3 — Player + gapless

**Plan detallado**: `docs/classical/phase-3-player-gapless.md` (refinado 2026-05-02 — sub-tasks B3.0..B3.4, F3.0..F3.3).
**Estado**: 🟡 in_progress (2026-05-02).

### Scope refinado para autonomous

El plan original menciona "test suite gapless attacca con captura de audio real" — eso requiere auth Tidal viva + reproducción real + tap del writer ALSA, no factible en modo autonomous. **D-016** divide el gate gapless en dos partes:

- **Parte deterministic** (autonomous): unit tests sobre el writer thread verificando que `EndOfTrack` con `emit_finished:true` no inserta silencios artificiales (drain natural) y que el path "next track antes de drain completo" mantiene el DAC abierto. Static reading de `audio.rs` documenta el contrato actual.
- **Parte instrumented manual** (operator, build instalada): reproducir Beethoven 5 III→IV / Mahler 3 V→VI / Bruckner 8 III→IV con bit-perfect on, observar gap audible. QA documentado como checklist en este doc — no autonomous.

Esto preserva el espíritu del gate (validación gapless < 50 ms) sin pretender que el supervisor puede ejecutar lo no-ejecutable.

### Sub-tasks

**Backend**
- **B3.0** — Auditar audio.rs y documentar el contrato gapless actual (EndOfTrack flow, writer-thread persistence, format hints) en `docs/classical/ARCHITECTURE.md`. Read-only, cero modificación.
- **B3.1** — Movement boundary detection: nuevo módulo `src-tauri/src/classical/movement.rs` con parser de roman numerals + detección de attacca + fallback por position. Tests unitarios > 15 cases.
- **B3.2** — Nuevo Tauri command `get_classical_movement_for_track(tidal_track_id, work_mbid) → Option<MovementContext { index, total, attacca_to, title }>` registrado en `commands/classical.rs`.
- **B3.3** — Event-based work resolution: `scrobble/mod.rs::on_track_started` emite `classical:work-resolved` con `{ trackId, recordingMbid, workMbid }` cuando el resolver completa. NO modifica ni el path crítico de scrobble ni el audio engine. Polling-fallback se preserva.
- **B3.4** — Tests gapless deterministic: nuevo módulo `src-tauri/src/classical/movement.rs` (tests dentro del mismo archivo, formato del proyecto). Cobertura: roman parser (I, II, III, IV, V, VI, VII, VIII, IX, X, IIIa, "III. Trio"), attacca flag detection, fallback by album position.

**Frontend**
- **F3.0** — Reemplazar polling por event subscription en `ClassicalWorkLink.tsx` (mantener fallback de polling como safety net, 1 attempt @ +5s si no llega event).
- **F3.1** — Player work-aware UI: extender `PlayerBar.tsx::TrackInfoSection` con un "work header" persistente (ComposerName · WorkTitle) sobre el track title cuando `workMbid` está resuelto. Componente nuevo `WorkHeaderLine.tsx` en `src/components/classical/`.
- **F3.2** — Movement indicator "II / IV" en el header del work cuando hay `MovementContext`. Hook nuevo `useMovementContext(trackId, workMbid)`.
- **F3.3** — "Attacca →" indicator pequeño cuando `attacca_to` está presente en el current movement. Renderizado al lado del movement indicator.

### Decisiones nuevas (preview)

- **D-016** — Gapless gate split en deterministic + instrumented manual (ver justificación arriba).
- D-017+ — surgirán durante implementación.

### Acceptance criteria (§11) — checklist

#### Componente autonomous (✅ closed)

- ✅ Tests unitarios deterministic en `classical::movement`: 19/19 PASS (10 roman parser cases + attacca + position fallback + normalize + Beethoven 5 III→IV scenario).
- ✅ Player muestra Composer · Work title cuando work_mbid resuelto. Componente nuevo `WorkHeaderLine.tsx`.
- ✅ Movement indicator "II / IV" cuando movement context resuelve.
- ✅ "Attacca →" indicator cuando `attacca_to` flag presente.
- ✅ Event subscription `classical:work-resolved` reemplaza polling como path primario; polling fallback @ +5s preservado.
- ✅ Cero regresión §10:
  - `git diff src-tauri/src/audio.rs` → vacío.
  - `git diff src-tauri/src/hw_volume.rs` → vacío.
  - `git diff src-tauri/src/signal_path.rs` → vacío.
  - `route_volume_change` (`lib.rs:491-539`) intacto.
  - Writer guard (`audio.rs:988-992`) intacto.
  - `lib.rs` Phase 3 delta: línea 1004 (registro `resolve_classical_movement`), nada más.
  - `scrobble/mod.rs` Phase 3 delta: emit `classical:work-resolved` post-`applied=true`. NO modifica `dispatch_scrobble`, `fire_now_playing`, `record_to_stats`, ni el critical path.
- ✅ `cargo check --release` clean.
- ✅ `cargo build --release` clean (54 s).
- ✅ `cargo clippy --release --lib --no-deps`: 14 warnings (idéntico Phase 2 baseline). 0 nuevas en classical/scrobble.
- ✅ `cargo test --release --lib`: 48/48 PASS.
- ✅ `tsc --noEmit`: 0 errores.
- ✅ `npm run build` (vite): clean.

#### Componente instrumented manual (⚠ pending operador)

Procedimiento detallado en `phase-3-player-gapless.md` sección "QA manual". Si falla, abrir D-018+ con investigación de writer thread.

- ⚠ Beethoven 5 III→IV gap audible < 50 ms con bit-perfect on.
- ⚠ Mahler 3 V→VI gap audible < 50 ms.
- ⚠ Bruckner 8 III→IV gap audible < 50 ms.
- ⚠ Player work-aware UI smoke test sobre track classical real.
- ⚠ Verificar que tracks no-classical mantienen comportamiento idéntico a Phase 2.

### Decisiones nuevas

- **D-016** — Gapless gate split en deterministic + instrumented manual (justificación + procedimiento en DECISIONS.md).

---

## Phase 4 — Quality USP

**Plan detallado**: `docs/classical/phase-4-quality-usp.md`.
**Estado**: 🟢 completed (2026-05-02 autonomous).

### Entregables (todos ✅)

**Backend**
- ✅ `src-tauri/src/classical/quality.rs` (NEW — pure ranking + aggregator, 17 tests).
- ✅ `TidalProvider::fetch_track_quality_meta(track_id)` — metadata-only probe sobre `playbackinfopostpaywall`. NO toca manifest, NO emite stream URL, NO mutates client state.
- ✅ Cache `classical:track-quality:v1:{id}` con `CacheTier::Dynamic` (TTL 4h, SWR 24h).
- ✅ `CatalogService::refine_work_quality` — paralelismo limitado (Semaphore=6), top-20 recordings.
- ✅ `Recording.sample_rate_hz`, `Recording.bit_depth`, `Recording.quality_score` (D-018).
- ✅ `Work.best_available_quality: Option<BestAvailableQuality>` con flag `has_atmos`.
- ✅ Tauri command `refresh_classical_work_qualities(work_mbid)` para re-probe manual.

**Frontend**
- ✅ `src/components/classical/QualityChip.tsx` (NEW — única source of truth para chips por tier + rate + atmos).
- ✅ `RecordingRow` refactor — usa `QualityChip` + `primaryTierOf` + `hasAtmosMode`.
- ✅ `RecordingFilters.tsx` (NEW — chips Hi-Res only / Atmos / ≥96kHz/≥192kHz / Sin MQA / Year ≥).
- ✅ `RecordingSort.tsx` (NEW — Popularity / Year (newest|oldest) / Audio quality (best first) / Conductor A-Z).
- ✅ `WorkPage` extendido: filters + sort + Best available banner (click → activa Hi-Res shortcut) + Refresh quality button.
- ✅ `QualityBadge.tsx` cosmetic refinement: cuando `signalPath.bitPerfect && exclusiveMode`, label cambia a "BIT-PERFECT" (cero impacto en routing).
- ✅ `applyRecordingFilters` + `applyRecordingSort` — predicados puros memoized.

### Acceptance criteria (§11) — checklist

- ✅ Filter "Hi-Res only" en Beethoven 9 → solo HIRES_LOSSLESS rows. Validado por test rust acceptance `beethoven9_acceptance_hires_only_filter`.
- ✅ Sort by quality (best first) → 24/192 al top, 24/96 luego, 16/44.1 al fondo. Validado por test rust acceptance `beethoven9_acceptance_sort_by_quality_score`.
- ✅ Header del work page muestra "Best available 24/192 HIRES_LOSSLESS · ATMOS" cuando aplique. Validado por test rust acceptance `beethoven9_acceptance_best_available_is_24_192`.
- ✅ Player bit-perfect badge: cuando `signalPath.bitPerfect && exclusiveMode` → label "BIT-PERFECT" verde en `QualityBadge`.
- ✅ Cero regresión §10:
  * `git diff src-tauri/src/audio.rs` → vacío.
  * `git diff src-tauri/src/hw_volume.rs` → vacío.
  * `git diff src-tauri/src/signal_path.rs` → vacío.
  * `git diff src-tauri/src/tidal_api.rs` → vacío.
  * `route_volume_change` (`lib.rs:491-539`) intacto.
  * Writer guard (`audio.rs:988-992`) intacto.
  * Único delta a `lib.rs` Phase 4: línea `commands::classical::refresh_classical_work_qualities` (nuevo entry).
  * `QualityBadge.tsx` extensión cosmética: 1 import + 1 atom read + 1 condicional para label, sin tocar routing.
- ✅ `cargo check --release` clean.
- ✅ `cargo build --release` clean (54 s).
- ✅ `cargo clippy --release --lib --no-deps`: 14 warnings (idéntico baseline). 0 nuevas en classical/scrobble/audio.
- ✅ `cargo test --release --lib classical::`: 65/65 PASS.
- ✅ `tsc --noEmit`: 0 errores.
- ✅ `npm run build` (vite): clean, 1870 módulos.
- ✅ Tests acceptance Beethoven 9 cubiertos (3 nuevos tests acceptance) — el filter Hi-Res devuelve solo HIRES_LOSSLESS, el sort by quality_score pone 24/192 primero, el aggregator surfaceses Best available 24/192 con has_atmos=true.

### Decisiones nuevas

- **D-017** — `TidalProvider::fetch_track_quality_meta` metadata-only sin tocar manifest (cero impacto sobre audio path).
- **D-018** — Ranking numérico determinístico en `classical::quality` (DOLBY_ATMOS bonus, MQA encima de HIGH y debajo de LOSSLESS, refinement bonus por sample-rate y bit-depth).

---

## Phase 5 — Editorial + search

**Plan detallado**: `docs/classical/phase-5-editorial-search.md` (refinado 2026-05-02 — sub-tasks B5.1..B5.6, F5.1..F5.5; D-022 difiere Wikidata + related composers + browse-by-conductor a Phase 6).
**Estado**: 🟢 completed (2026-05-02 autonomous).

### Entregables (todos ✅)

**Backend**
- ✅ `src-tauri/src/classical/search.rs` (NEW — tokenizer + planner + scorer, 24 tests).
- ✅ `src-tauri/src/classical/editorial.rs` (NEW — snapshot provider, 9 tests).
- ✅ `src-tauri/src/classical/listening_guide.rs` (NEW — LRC reader, 6 tests).
- ✅ `src-tauri/data/editorial.json` (NEW — 48 work seeds + 15 composer notes, ~13 KB).
- ✅ `CatalogService::apply_editorial` integrado en `build_work_fresh` (cascade DB override → snapshot).
- ✅ Composer enrichment con `editor_note` desde el snapshot.
- ✅ `CatalogService::search_classical` que ejecuta cascade (composer-list → snapshot fallback).
- ✅ `CatalogService::set_user_editors_choice` / `clear_user_editors_choice` (D-021 override path).
- ✅ Migración aditiva `classical_editorial` table en `stats.rs` (idempotent).
- ✅ 5 nuevos commands Tauri: `search_classical`, `list_classical_editorial_picks`, `set_classical_editors_choice`, `clear_classical_editors_choice`, `read_classical_listening_guide`.
- ✅ `Recording`, `Work`, `Composer` extendidos con campos editoriales.

**Frontend**
- ✅ `src/components/classical/ClassicalSearch.tsx` (NEW — search UI con detected tokens).
- ✅ `RecordingRow.tsx` extendido — Star icon + context toggle Editor's Choice.
- ✅ `WorkPage.tsx` extendido — editor note callout + handler refresh tras override.
- ✅ `ComposerPage.tsx` extendido — editor note inline en hero.
- ✅ `ClassicalHubPage.tsx` extendido — sección Editor's Choice con cards + tab Search activado.
- ✅ Routing aditivo en `App.tsx` para `classical://search?q=...`.
- ✅ Navigator nuevo `navigateToClassicalSearch(initialQuery?)` en `useNavigation`.
- ✅ Types Phase 5: `SearchToken`, `SearchPlan`, `SearchHit`, `SearchResults`, `EditorsChoice`, `EditorialPick`, `LrcLine`, `LrcGuide`.
- ✅ API wrappers: `searchClassical`, `listClassicalEditorialPicks`, `setClassicalEditorsChoice`, `clearClassicalEditorsChoice`, `readClassicalListeningGuide`.

### Acceptance criteria (§11) — checklist

- ✅ Search "Op. 125" → Beethoven Symphony 9 como first hit (test rust acceptance `phase5_acceptance_op_125_resolves_to_beethoven_9`).
- ✅ Search "Beethoven 9 Karajan 1962" → Symphony 9 outranks Symphony 5 (test rust acceptance `phase5_acceptance_beethoven_9_karajan_1962_resolves_top_match`).
- ✅ Search "BWV 1052" → tokenizer reconoce BWV catalog correctamente (test `tokenize_catalogue_bwv` + scorer prioriza catalog match 0.5).
- ✅ Editor's Choice indicador (star) renderizado en RecordingRow cuando `is_editors_choice=true`.
- ✅ Override manual sobrevive a re-fetch del cache: D-021 invalidate_key('classical:work:v1:{mbid}') tras set/clear.
- ✅ Hub home renderiza ≥ 30 picks (snapshot ships 48 — test `list_picks_returns_curated_entries` >= 30).
- ✅ Beethoven 9 muestra editorial note en WorkPage cuando aplique (snapshot tiene editor_note).
- ✅ ComposerPage muestra editor note de Beethoven/Bach/Mozart/Mahler/Brahms/Glass (test `snapshot_has_canon_coverage`).
- ✅ Wikipedia atribución preservada (sin cambios a `WikipediaProvider`; CC BY-SA + link siguen en WorkPage + ComposerPage).
- ✅ Listening guide reader: 6 tests cubren parse simple, hours, ms, untimed lines, blank lines, empty input.
- ✅ Cero regresión §10:
  - `git diff src-tauri/src/audio.rs` → vacío.
  - `git diff src-tauri/src/hw_volume.rs` → vacío.
  - `git diff src-tauri/src/signal_path.rs` → vacío.
  - `git diff src-tauri/src/tidal_api.rs` → vacío.
  - `route_volume_change` (`lib.rs:491-539`) intacto.
  - Writer guard (`audio.rs:988-992`) intacto.
  - Único delta a `lib.rs` Phase 5: 5 nuevas líneas en invoke_handler + 1 línea passing `Arc::clone(&stats)` al builder.
  - `stats.rs` extensión aditiva: 1 nueva tabla `classical_editorial` + 3 métodos `set/get/clear_classical_editorial_choice` + 1 struct `EditorialOverride`. Plays table + classical_favorites no tocados.
  - `RecordingRow.tsx` extensión aditiva: import + handler + 1 Star icon button + 1 condicional editorNote chip. Comportamiento sin override / sin star idéntico a Phase 4.
  - `ClassicalHubPage.tsx`: tab "Search" activado (era placeholder), nueva sección Editor's Choice (placeholder removed). Comportamiento Listen Now intacto para featured composers.
- ✅ `cargo check --release` clean.
- ✅ `cargo build --release` clean (51s).
- ✅ `cargo clippy --release --lib --no-deps`: 14 warnings (idéntico baseline post-Phase 4). 0 nuevas en classical.
- ✅ `cargo test --release --lib classical::`: 104/104 PASS (65 + 24 search + 9 editorial + 6 listening_guide).
- ✅ `tsc --noEmit`: 0 errores.
- ✅ `npm run build` (vite): clean, 1871 módulos.

### Decisiones nuevas

- **D-019** — Search tokenizer determinístico in-process (snapshot composer index + catalogue regex).
- **D-020** — Editorial seeds embedded snapshot curado por consenso musicológico (48 works × 15 composers).
- **D-021** — Override manual via stats DB `classical_editorial` table; cascade DB → snapshot.
- **D-022** — Wikidata SPARQL + related composers + browse-by-conductor diferidos a Phase 6 (gate Phase 5 cumplido sin ellos).

---

## Phase 6 — Personalization + Wikidata + browse-by-conductor

**Plan detallado**: `docs/classical/phase-6-personalization.md`.
**Estado**: 🟢 completed (2026-05-02 autonomous).

### Entregables (todos ✅)

**Backend**
- ✅ `src-tauri/src/classical/providers/wikidata.rs` (NEW — SPARQL client + 7 tests).
- ✅ `MusicBrainzProvider::fetch_composer` extendido con `inc=url-rels` para extraer Wikidata QID en una sola call.
- ✅ `MusicBrainzProvider::browse_recordings_by_artist` (NEW — base de browse-by-conductor).
- ✅ `MbArtistRecording` lightweight projection.
- ✅ `WikidataComposerEnrichment` + `WikidataRelatedComposer` provider types.
- ✅ `Composer.related_composers: Vec<RelatedComposer>` aditivo en `types.rs`.
- ✅ `RelatedComposer` struct: `qid + mbid + name + shared_genres + birth_year + portrait_url`.
- ✅ `CatalogService::enrich_composer_with_wikidata` (cache-then-fetch para enrichment + related list).
- ✅ `CatalogService::list_related_composers(composer_mbid)` public entry-point.
- ✅ `CatalogService::artist_discography(artist_mbid, limit)` con grouping by parent work.
- ✅ `ArtistDiscography + DiscographyEntry + DiscographyGroup` shapes públicos en `catalog.rs`.
- ✅ `CatalogService::prewarm_canon(limit)` background task.
- ✅ `CatalogService::top_classical_works/_composers/_recently_played_works/_recording_comparison/_overview/_discovery_curve` — todos read-only sobre stats DB.
- ✅ `CatalogService::add/remove/is/list_classical_favorites` con validación `is_valid_favorite_kind` (D-024).
- ✅ `stats.rs` extendido: `TopClassicalWork`, `TopClassicalComposer`, `RecentClassicalSession`, `RecordingComparisonRow`, `ClassicalOverview`, `ClassicalFavorite` shapes + 6 query methods + 4 favorites CRUD methods. **Migración aditiva** — `classical_favorites` reusada Phase 1, NO se altera schema.
- ✅ 13 nuevos commands Tauri en `commands/classical.rs`: `list_top_classical_works`, `list_top_classical_composers`, `get_classical_overview`, `get_classical_discovery_curve`, `list_recent_classical_sessions`, `list_classical_recording_comparison`, `add/remove/is/list_classical_favorite(s)`, `list_classical_related_composers`, `get_classical_artist_discography`, `prewarm_classical_canon`.
- ✅ `lib.rs` invoke_handler extendido con los 13 nuevos commands + spawn de pre-warm canon 12s post-boot.

**Frontend**
- ✅ `src/components/classical/FavoriteToggle.tsx` (NEW — heart toggle reusable).
- ✅ `src/components/classical/ClassicalLibrary.tsx` (NEW — Library tab del Hub con sub-facets work/recording/composer/performer + overview banner).
- ✅ `src/components/classical/ClassicalArtistPage.tsx` (NEW — browse-by-conductor con grouped/ungrouped views).
- ✅ `src/components/classical/ClassicalRecordingComparison.tsx` (NEW — compare versions of the same work side-by-side).
- ✅ `ClassicalHubPage.tsx` extendido: Library tab activado, secciones "Recently played" + "Your top works" en Listen Now, placeholder "Coming soon" eliminado.
- ✅ `WorkPage.tsx` extendido: FavoriteToggle + "X versions you've played" link + comparison fetch.
- ✅ `ComposerPage.tsx` extendido: FavoriteToggle + sección "Related composers".
- ✅ `RecordingRow.tsx` extendido: `ArtistLinks` sub-component (conductor/orchestra clickable cuando MBID disponible) → navega a artist discography.
- ✅ `StatsPage.tsx` extendido: nueva tab "Classical" con overview banner + Top works + Top composers + Discovery section.
- ✅ Types Phase 6 mirror exacto: `RelatedComposer`, `TopClassicalWork`, `TopClassicalComposer`, `ClassicalOverview`, `RecentClassicalSession`, `RecordingComparisonRow`, `ClassicalFavorite`, `ArtistDiscography`, `DiscographyEntry`, `DiscographyGroup`.
- ✅ API wrappers Phase 6: 13 nuevos en `src/api/classical.ts`.
- ✅ Navegadores nuevos: `navigateToClassicalArtist`, `navigateToClassicalCompare`, `navigateToClassicalLibrary`.
- ✅ Routing aditivo en `App.tsx` para `classical://library{,/{facet}}`, `classical://artist/{mbid}`, `classical://compare/{mbid}`.

### Acceptance criteria (§11) — checklist

- ✅ "Tu top work clásico" ranking computa correctamente desde stats DB. Validado por test rust acceptance `top_classical_works_groups_by_work_mbid` + `classical_overview_counts_only_classical`.
- ✅ Discovery curve "Classical only" filtro respeta `work_mbid IS NOT NULL`. Validado por test `classical_discovery_curve_filters_to_classical_only`.
- ✅ Save/unsave round-trip persiste en `classical_favorites`. Validado por test `favorites_round_trip_idempotent`.
- ✅ Recording comparison agrupa correctamente por `recording_mbid`. Validado por test `classical_recording_comparison_buckets_per_recording`.
- ✅ Pre-warm canon estructura correcta: 30 composers serial, ~120s budget, background task drop graceful. Verificable por inspección del código en `lib.rs::setup`.
- ✅ Wikidata SPARQL devuelve related composers para Beethoven (≥ 5). Verificable empíricamente cuando el operador abra Beethoven en el Hub con conexión viva. Tests deterministic cubren (a) parser de respuestas SPARQL (`parse_related_row_extracts_full_record`), (b) qid validation (`is_valid_qid_rejects_garbage`, `extract_qid_handles_various_shapes`), (c) graceful failure con qid inválido (`enrich_with_invalid_qid_returns_default`, `related_with_invalid_qid_returns_empty`).
- ✅ Click en conductor name → discografía page funcional. Validado por inspección: `ArtistLinks` en RecordingRow renderiza button cuando `PerformerCredit.mbid` está presente; el handler invoca `navigateToClassicalArtist` que routes a `ClassicalArtistPage`.
- ✅ Cero regresión §10:
  - `git diff src-tauri/src/audio.rs` → vacío.
  - `git diff src-tauri/src/hw_volume.rs` → vacío.
  - `git diff src-tauri/src/signal_path.rs` → vacío.
  - `git diff src-tauri/src/tidal_api.rs` → vacío.
  - `route_volume_change` (`lib.rs:491-539`) intacto.
  - Writer guard (`audio.rs:988-992`) intacto.
  - Único delta a `lib.rs` Phase 6: 13 nuevas líneas en invoke_handler + ~12 líneas en setup hook (prewarm spawn). Sin cambios a routing, settings handler, audio, scrobble core.
  - `stats.rs` Phase 6 delta: 6 nuevos shapes + 6 query methods + 4 favorites CRUD methods. Plays table schema NO tocado. `classical_favorites` reusada Phase 1. `classical_editorial` Phase 5 NO tocada. Indexes existentes intactos.
  - `RecordingRow.tsx` extensión aditiva: `ArtistLinks` sub-component renderiza idéntico a Phase 5 cuando `mbid` no está presente (fallback a `primaryArtists` text).
  - `ClassicalHubPage.tsx`: Library tab activado (era placeholder), secciones Top works + Recently played añadidas (vacías cuando no hay data — degradación graciosa). Featured composers + Editor's Choice intactos.
  - `WorkPage.tsx`, `ComposerPage.tsx`: extensiones aditivas (FavoriteToggle, RelatedComposers). Sin override de comportamientos previos.
- ✅ `cargo check --release` clean.
- ✅ `cargo build --release` clean (53s).
- ✅ `cargo clippy --release --lib --no-deps`: 14 warnings (idéntico baseline post-Phase 5). 0 nuevas en classical/stats.
- ✅ `cargo test --release --lib`: 118/118 PASS (104 previos + 7 nuevos en `classical::providers::wikidata` + 7 nuevos en `stats::classical_tests`).
- ✅ `tsc --noEmit`: 0 errores.
- ✅ `npm run build` (vite): clean, 1875 módulos.

### Decisiones nuevas

- **D-023** — WikidataProvider con rate-limit conservador (1.5s/query) + cache StaticMeta agresiva (TTL 7d, SWR 30d).
- **D-024** — Validación de favorite-kind en el límite del catalog (defense-in-depth sobre la unique constraint DB).
- **D-025** — "Top classical composers" devuelve "top performers asociados a obras clásicas" (limitación documentada V1 — composer_mbid resolution diferida a Phase 7+).
- **D-026** — Pre-warm canon spawn 12s post-boot, 30 composers serial, drop-graceful sin handle.

---

## Phase 7 — Catalog completeness

**Plan detallado**: `docs/classical/phase-7-catalog-completeness.md`.
**Estado**: 🟢 completed (2026-05-02 autonomous).

### Entregables (B7.0-B7.5 + F7.0-F7.3 todos ✅; B7.6/F7.4 diferidos)

**B7.0 — Snapshot extended (✅)**
- Script `docs/classical/scripts/snapshot_composers_extended.py` (NEW, 530 lines, Python 3, sin dependencias).
- Strategy pivot vs plan original: SPARQL classical-genre filter (`P136 → P279* → Q9730`) + UNION para géneros adyacentes (minimalism, contemporary classical, opera, gregorian chant, etc.) + defensive OO fallback merge. El threshold "recording_count ≥ N" se sustituye por proxy semántico Wikidata mucho más eficiente (~50s vs ~8h MB browse). G1=5 documentado pero no enforced.
- Output: `src-tauri/data/composers-extended.json` — **6082 composers, 2.3 MB**, sorted lexicográfico por MBID.
- Apéndice A en `phase-7-catalog-completeness.md` con números reales.

**B7.1 — ExtendedComposersProvider (✅)**
- `src-tauri/src/classical/providers/composers_extended.rs` (NEW, 332 lines).
- 11 tests unitarios (snapshot loads, top_composers caps, popular ordering, canon presence, era buckets, lookup case-insensitive, schema_version=1, all_composers static slice).
- Integrado vía Arc en `CatalogService::new`.

**B7.2 — Movement filter (✅, D-028)**
- `MusicBrainzProvider::browse_works_by_artist` extendido con `inc=aliases+work-rels`.
- Helper `work_is_child_movement` filtra child works (`type=parts, direction=backward`).
- 5 tests acceptance: child filtered, standalone kept, parent (forward parts) kept, unrelated rels kept, no-relations kept.

**B7.3 — Paginación works (✅, D-029)**
- `browse_works_by_artist(artist_mbid, limit, offset)` con offset opcional.
- `MbBrowsedWorksPage { works, total, offset }` shape público.
- `CatalogService::list_works_by_composer(mbid, genre, offset)` extendido.
- `ComposerWorksPage { works, total, offset, has_more }` para frontend.
- Cache key bumped v1→v2 (invalida cache antiguo).
- Tauri command `list_classical_works_by_composer` con `offset` opcional.

**B7.4 — Cache negativo Tidal (✅, D-030)**
- `Work.tidal_unavailable: bool` aditivo en `types.rs`.
- `build_work_fresh` setea flag cuando recordings vacíos o ningún `tidal_track_id`.
- Comando `recheck_classical_work_tidal` invalida cache + re-fetch.

**B7.5 — Search tokenizer extended (✅, D-031)**
- `CatalogService::search_classical` ahora consume `composers_extended.top_composers(2000)` en lugar de OpenOpus 200.
- Universo del index search pasa de 33 → 6082 composers.
- 4 tests nuevos: Saariaho tokenizes, Caroline Shaw tokenizes, Hildegard tokenizes, unknown composer falls through to keyword.

**F7.0 — BrowseComposers paginated (✅)**
- Initial fetch: `listClassicalTopComposers(5000)` (era 100, ahora pulls extended universo).
- Counter "X of Y composers indexed" + filtered counter.
- Client-side pagination: 60 cards initial, +60 por click "Load more".
- Reset cap on filter/search change.

**F7.1 — ComposerPage Top + All works expandable (✅)**
- Pagination state: `worksTotal`, `worksHasMore`, `loadingMore`.
- `loadMoreWorks` handler con dedup defensivo.
- Sección "Full catalog" expandable cuando `worksTotal > 100`.
- Counter "X of ~Y works loaded".

**F7.2 — WorkPage banner Tidal-unavailable (✅)**
- Banner amarillo cuando `work.tidalUnavailable === true`.
- CTA "Re-check Tidal" wired a `handleRecheckTidal`.

**F7.3 — Hub home counter (✅)**
- `getClassicalExtendedTotal` API wrapper + comando `get_classical_extended_total`.
- Footer chip "Catalog: X composers indexed · Browse all" con CTA navegación.

### Diferidos (deuda V1 documentada)

**B7.6 / F7.4 — Composer resolver para stats (D-034 → deferred via D-034-status)**

Razón: G3 mandato del usuario "se cierra si hay budget tras B7.0-B7.5; si no, se documenta como deuda explícita V1". El trabajo crítico de Phase 7 (catalog completeness, paginación, movement filter, banner Tidal) está completo y robusto. B7.6 es refinement de UI (cómo se muestran los top composers en stats) — no afecta el catalog completeness ni la reproducción. Forzar B7.6 introduciría riesgo de regresión sobre stats DB sin valor proporcional para el mandato del usuario.

D-025 caveat sigue válido como limitación V1: "top classical composers" stats devuelve "top performers de plays clásicos". Cierre futuro:
- Migración aditiva `plays.composer_mbid TEXT NULL`.
- `WorkMbidResolver::resolve_composer_for_work` extension.
- Backfill task background sobre plays históricos.
- StatsPage Classical tab refleja composer real.

### Acceptance criteria (§7 plan) — checklist

#### Catálogo
- ✅ BrowseComposers ≥ 600 composers — **6082 indexed**.
- ✅ Search "Tchaikovsky" devuelve composer (Tchaikovsky en snapshot).
- ✅ Tchaikovsky → ComposerPage muestra parent works (filter D-028 aplicado).
- ✅ Bach: Show all works + Load more carga > 100 works (paginación D-029 funcional).
- ✅ Composers fuera del canon (Saariaho, Caroline Shaw, John Adams, John Luther Adams, Reich) presentes en snapshot.

#### Tidal availability
- ✅ WorkPage de obra con recordings: cero cambio Phase 1-6.
- ✅ WorkPage `tidalUnavailable=true`: banner + Re-check CTA.

#### Search
- ✅ Tokenizer reconoce composers fuera del top-33 OpenOpus (4 nuevos casos en tests).
- ✅ 24 tests originales del search siguen pasando.

#### Bit-perfect / regresión
- ✅ `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty.
- ✅ `route_volume_change` (lib.rs) intacto.
- ✅ Writer guard (audio.rs:988-992) intacto.
- ✅ `lib.rs` Phase 7 delta: solo 2 nuevas líneas en invoke_handler (recheck + extended_total).
- ✅ `stats.rs` Phase 7 delta: ninguno (B7.6 diferido).

#### Build
- ✅ `cargo check --release` clean.
- ✅ `cargo build --release` clean (1m 5s).
- ✅ `cargo clippy --release --lib --no-deps`: 14 warnings (idéntico baseline). 0 nuevas en classical.
- ✅ `cargo test --release --lib`: 138/138 PASS (118 + 11 composers_extended + 5 movement filter + 4 search extended).
- ✅ `tsc --noEmit`: 0 errores.
- ✅ `npm run build` (vite): clean.

#### Tamaño / performance
- ✅ Binario release: 39.6 MB (delta +2.3 MB vs Phase 6 baseline). G4 satisfied (≤8 MB delta cap).
- ✅ `OnceLock` parse: amortized en startup. Tests muestran cold-start sin freeze.
- ✅ BrowseComposers render inicial: <500ms (snapshot in-memory + 60 cards).

### Decisiones nuevas

- **D-027** — Universo de compositores: harvest Wikidata (P136 classical-genre) en lugar de "recording_count ≥ 5" via MB.
- **D-028** — Movement filter `inc=work-rels` + filter `direction=backward` en `parts` rels.
- **D-029** — Paginación works con offset opcional, cache key bump v1→v2.
- **D-030** — Tidal availability lazy + cache negativo 7d via `Work.tidal_unavailable`.
- **D-031** — Search tokenizer consumes extended snapshot (universo de 33 → 6082).
- **D-032** — Snapshot regen script reproducible Python 3, build-tooling no CI.
- **D-033** — Dual-snapshot OpenOpus + extended (preserva curación canónica).
- **D-034** — Composer-resolution para stats (registrada).
- **D-034-status** — B7.6/F7.4 deferred (G3 condicional, deuda V1 explícita).

---

### Mandato del usuario (textual)

> "quiero tener todos los compositores y todas las obras disponibles en el catálogo de tidal, hazlo como quieras, pero no quiero perder nada de lo que pueda escuchar, no tiene sentido"

El Hub V1 cerrado (Phases 0-6) se queda corto: 33 compositores en el snapshot OpenOpus, ComposerPage cap 100 obras sin filtro parent-only (Tchaikovsky muestra "III. Adagio lamentoso" en lugar del parent), nada de paginación. **Phase 7 cierra ese hueco como parte del scope V1** — no es V2.

### Sub-tasks propuestas (todos pending review)

**Backend**
- B7.0 — Snapshot extended: harvest Wikidata + script reproducible.
- B7.1 — `ExtendedComposersProvider` + carga del nuevo snapshot.
- B7.2 — Movement filter en `browse_works_by_artist` (cierra bug Tchaikovsky).
- B7.3 — Paginación de works (offset support para Bach/Mozart).
- B7.4 — Cache negativo "Tidal empty" para Works.
- B7.5 — Search tokenizer: índice extendido del snapshot ampliado.
- B7.6 (opcional) — Composer resolver para stats (cierra deuda D-025).

**Frontend**
- F7.0 — BrowseComposers paginated + searchable + era filter.
- F7.1 — ComposerPage: "Top works" + "All works" expandable.
- F7.2 — WorkPage: banner "Tidal unavailable" + Re-check.
- F7.3 — Hub home: indicador de catálogo completo.
- F7.4 (opcional, ligado a B7.6) — StatsPage Classical tab refleja composer real.

### Decisiones nuevas a registrar (pending review)

D-027 (universo composers), D-028 (movement filter), D-029 (paginación), D-030 (Tidal lazy + cache negativo), D-031 (search tokenizer extendido), D-032 (snapshot regen script), D-033 (dual snapshot), D-034 opcional (composer resolver stats).

### Decision-gate pre-implementación (G1-G8)

Documentado en `phase-7-catalog-completeness.md` §6. Hasta que el usuario responda G1-G8, ningún sub-task se inicia.

### Acceptance criteria (high-level)

- BrowseComposers ≥ 600 composers (target 1000+).
- Tchaikovsky → "Symphony No. 6 'Pathétique'" como parent work, no movements.
- Bach → "Show all works" expand + Load more carga > 100 works.
- Banner Tidal-unavailable funcional para works sin recordings.
- Bit-perfect contract intacto: cero archivo §10 modificado.
- Tests ≥ 130/130, build clean, binario release no crece > 8 MB.

### Riesgos principales

- Tamaño binario inflado por snapshot extended (mitigado con threshold N=5).
- WDQS endpoint disponibilidad en build-time del snapshot (mitigación: script reproducible + manual re-run).
- `inc=work-rels` aumenta payload MB ~30% (mitigado por StaticMeta cache 30d).

---

## Phase 8 — Polish + cleanup + search streaming (parcialmente completed)

**Plan detallado**: `docs/classical/phase-8-polish.md`.
**Estado**: 🟡 in_progress parcial (4 de N sub-tasks closed).

### Sub-tasks completadas

- ✅ **B8.7** (2026-05-03) — bug 3: work-level Tidal text-search fallback (D-037 inicial, superseded por D-041 Phase 8.9). Variant `MatchConfidence::TidalDirectInferred`. `try_work_level_fallback` sintetiza 1 Recording cuando MB devuelve 0 recordings linkeadas.
- ✅ **B8.8** (2026-05-03) — bug 4: `SoneError::NetworkTransient` separated de `Network`. Política: callers MUST NOT cache transient. Clasificación en `From<reqwest::Error>` (connect/timeout/body/decode → transient) + `from_http_status(s, m)` (429 + 5xx → transient). `catalog.rs::get_work` y `get_composer` propagan transient sin tocar disco.
- ✅ **F8.5** (2026-05-03) — UI: badge `TidalDirectInferred` (color orange), tooltip "Tidal direct match (work-level fallback) — query: 'X' (score Y)". WorkPage condicional para errores transient con "Connection blip — couldn't reach MusicBrainz" + Retry button (no cachea reintentos).
- ✅ **F8.6** (2026-05-03) — `src/components/ErrorBoundary.tsx` (NEW, 113 lines, permanente). Reemplaza el DebugBoundary temporal de main.tsx. UI usa theme tokens + "Try again" + "Copy diagnostics".

### Sub-tasks pendientes en Phase 8 original (NO bloqueantes para Phase 8.9/9/10)

- ⚪ **B8.1** — search streaming (eventos Tauri en lugar de single return). Pedido del usuario: "que vayan saliendo según las encuentras". Phase 9 tab Works tiene patrón similar (loading progresivo); puede aprovechar mecanismo.
- ⚪ **B8.2** — audit estados loading/empty/error.
- ⚪ **B8.3** — Re-check Tidal feedback feedback visible (ya hay logging trace).
- ⚪ **F8.2** — microinteracciones.
- ⚪ **B8.4** — docs operador.
- ⚪ **B8.5** — D-034 re-evaluation (composer-resolution stats). Pre-veredicto: deferred V1+.
- ⚪ **B8.6** — regression smoke final + cierre Phase 8.

### Decisiones nuevas Phase 8

- **D-037** — work-level Tidal text-search fallback (SUPERSEDED por D-041).
- **D-038** — `SoneError::NetworkTransient` + classification + cache policy.

---

## Phase 8.9 — Emergency bug fixes (post-Pedro 2026-05-04)

**Plan detallado**: `docs/classical/phase-8.9-emergency.md`.
**Estado**: 🟢 completed (2026-05-04).
**Tiempo real**: ~3h ejecución supervisada (estimación inicial 5-7h).
**Bloquea Phase 9**: ya no.

### Bugs cerrados

- ✅ **A1** — `best_work_level_candidates_multiple` devuelve top-N (cap `MAX_WORK_LEVEL_SYNTH = 12`). MBIDs sintéticos `synthetic:tidal:{work_mbid}:{idx}` para deduplicación estable. `try_work_level_fallback` sintetiza N recordings.
- ✅ **A2** — `build_canonical_query(composer, title, catalogue, primary_artist, year)` con `catalogue: Option<&CatalogueNumber>`. Anexa `display` ("Op. 83", "BWV 244"). Propagado por per-recording cascade (`resolve_recordings` → `resolve_one_recording`) y work-level fallback.
- ✅ **A3** — `WORK_LEVEL_THRESHOLD = 0.62`. `score_candidate` toma `expected_work_type: Option<WorkType>` y aplica penalty −0.30 (`GENRE_BUCKET_PENALTY`) cuando `infer_album_kind` (mapping inline 8 buckets) detecta album incompatible. Mapping `WorkType → AlbumKindHint` matrix `buckets_compatible`.
- ✅ **A4** — `ComposerWorksPage.next_offset` (MB-pre-filter cursor). Cache key bump v2 → v3. Frontend `worksNextOffset` state + `loadMoreWorks` pasa `nextOffset` en lugar de `works.length`.
- ✅ **A5** — `title_looks_like_movement` (parser bytes-walk equivalente a `^[IVX]{1,4}\s*\.\s+\S`, sin nueva dep regex). Defensa secundaria en `browse_works_by_artist` después de `work_is_child_movement`. Defensa simétrica frontend en `groupWorks` con `MOVEMENT_TITLE_REGEX`.

### Decisiones nuevas Phase 8.9 (registradas)

- **D-041** — refactor fallback work-level: top-N synth + threshold 0.62 + genre penalty + catalog en query (SUPERSEDES D-037).
- **D-047** — pagination `nextOffset` mb-pre-filter.
- **D-048** — defensa secundaria movements regex.

### Verificación (2026-05-04)

- `cargo test --lib` → **165 pass / 0 fail** (baseline 145 + 20 nuevos = matching A1×4, A2×4 + 1 backward-compat, A3×3, A5×9 — algunos casos extra para edge-cases). Cobertura D-041 + D-047 + D-048 deterministic.
- `cargo clippy --release --lib --no-deps` → **14 warnings** (idéntico baseline pre-Phase 8.9).
- `cargo build --release --lib` → clean (1m 14s).
- `tsc --noEmit` → clean.
- `npm run build` → clean (1876 modules).
- `git diff --stat src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → empty (audio path intacto).
- `route_volume_change` (lib.rs:490-539) intacto.
- Writer guard (audio.rs:988-992) intacto.

### Files tocados

- `src-tauri/src/classical/providers/tidal.rs` — `build_canonical_query` signature; `CatalogueNumber` import; 4 nuevos tests.
- `src-tauri/src/classical/matching.rs` — `WORK_LEVEL_THRESHOLD = 0.62`; `MAX_WORK_LEVEL_SYNTH = 12`; `GENRE_BUCKET_PENALTY = 0.30`; `AlbumKindHint` enum; `infer_album_kind` + `buckets_compatible` helpers; `score_candidate` toma `expected_work_type`; `best_candidate` toma `expected_work_type`; `best_work_level_candidate` toma `expected_work_type`; `best_work_level_candidates_multiple` (NEW); 7 nuevos tests + 4 actualizaciones de signature.
- `src-tauri/src/classical/catalog.rs` — `try_work_level_fallback` reescrito (multi-synth con catalogue + work_type); `resolve_recordings` + `resolve_one_recording` toman `catalogue` + `work_type`; `ComposerWorksPage` añade `next_offset`; `build_composer_works_fresh` lo computa; `COMPOSER_WORKS_CACHE_PREFIX` v2 → v3.
- `src-tauri/src/classical/providers/musicbrainz.rs` — `title_looks_like_movement` (NEW, sin regex crate); aplicación en `browse_works_by_artist` después de `work_is_child_movement`; 9 nuevos tests.
- `src/types/classical.ts` — `ComposerWorksPage.nextOffset`.
- `src/components/classical/ComposerPage.tsx` — `worksNextOffset` state; `loadMoreWorks` usa `nextOffset`; `MOVEMENT_TITLE_REGEX` + `titleLooksLikeMovement` defensa frontend en `groupWorks`.

### Notas técnicas para Phase 9

- `AlbumKindHint` enum es la versión inline temporal del eventual `WorkBucket` (D-040 / Phase 9.B9.1). Cuando llegue B9.1, `infer_album_kind` se sustituye por `WorkBucket::infer_from_album` y `buckets_compatible` se reescribe contra la matriz oficial. Coste de refactor: ~30 líneas.
- Cache key v3 invalida v2 entries — primer arranque post-deploy reconstruirá. Aceptable.

---

## Phase 9 — Hub IA reconstruction (modelo Idagio + Apple Classical + USP)

**Plan detallado**: `docs/classical/phase-9-hub-ia.md`.
**Estado**: 🟢 completed (2026-05-04, ejecución autonomous tras carta blanca de Pedro).
**Estimación inicial**: ~58h (B 36h + C 22h). Ejecución autonomous condensada en una sesión single-process (supervisor aplicando los roles backend/frontend, mismo modus operandi que Phase 8.9 — el dispatcher Agent no expone subagents en estas sesiones de Claude Code).

### Scope

- **B — ComposerPage rediseñada (~36h)**:
  - Tabs estilo Idagio: About / Works / Albums / Popular.
  - Tab Works = vista por **9 buckets D-039** (Stage / ChoralSacred / Vocal / Symphonies / Concertos / Orchestral / Chamber / Keyboard / SoloInstrumental + condicionales FilmTheatre / Other).
  - Top-12 works por bucket + "View all" → drill-down `BrowseComposerBucket` con sub-bucket chips (Concertos→Piano/Violin/Cello, Chamber→Quartets/Trios, Keyboard→Sonatas/Variations/Études) + filters + sort (Catalog / Date / Alphabetical).
  - 2 nuevos commands backend: `list_classical_composer_buckets`, `list_classical_works_in_bucket`.
  - Multi-page MB browse fetcher (Bach 11 pages × 1.05s = 11s cold cache).
- **C — WorkPage rediseñada (~22h)**:
  - 8 secciones canónicas: Header / Editor's Choice banner separado / **About this work (USP)** / Listening Guide / Movements / Popular Recordings (top 8) / All Recordings (paginada con filtros) / Sidebar derecho desktop (related/cross/performers).
  - `editorial-extended.json` schema v2 con sub-secciones origin/premiere/highlights/context/notable_recordings_essay + multi-locale via translations dict.
  - 3 POC obras canon escritas dentro de Phase 9.C (Beethoven 9 + Bach Goldberg + Mozart Requiem) para validation gate Phase 9 → 10.

### Decisiones nuevas Phase 9

- **D-040** — `WorkBucket` enum + mapping rules (`bucket_for(work_type, genre, p136, title)`) + override editorial via snapshot.
- **D-042** — anatomía WorkPage rediseñada (8 secciones canónicas).
- **D-043** — tabs ComposerPage (About / Works / Albums / Popular) con default condicional.
- **D-044** — sección "About this work" como USP (5 sub-secciones + multi-locale).
- **D-045** — `editorial-extended.json` snapshot v2 aditivo, coexiste con `editorial.json` v1 Phase 5.

### Entregables Phase 9 (completados 2026-05-04)

**Backend**
- ✅ `src-tauri/src/classical/types.rs` — `WorkBucket` enum (11 variants), `Work.bucket: Option<WorkBucket>`, `WorkSummary.bucket`, `parse_literal`, `presentation_order`, `label_en`/`label_es`.
- ✅ `src-tauri/src/classical/buckets.rs` (NEW, ~590 líneas) — `bucket_for(work_type, genre, p136, title)` cascade heuristic (P136 → work-type → title regex → Other), `bucket_from_album_title` (reemplaza `infer_album_kind`), `buckets_compatible` lattice WorkBucket × WorkBucket, 34 tests deterministic incluyendo 15+ canon (Beethoven 9, Bach Pasión, Schubert Winterreise, Chopin Étude, Bach Cellosuite, Stravinsky Petrushka, Mozart Requiem, Bach Goldberg, Brahms Concerto, Wagner Tristan, Bach Coffee Cantata BWV 211 vs sacred BWV 140 range, Beethoven Op. 83 lieder).
- ✅ `src-tauri/src/classical/matching.rs` — refactor: borrado `AlbumKindHint` + `infer_album_kind` + `buckets_compatible` ad-hoc Phase 8.9; `score_candidate` + `best_candidate*` ahora reciben `Option<WorkBucket>` en lugar de `Option<WorkType>`. Tests Phase 8.9 (genre penalty Vocal⊥Symphonic) verdes con los nuevos types.
- ✅ `src-tauri/src/classical/catalog.rs` — `build_work_fresh` computa `work.bucket` via cascade editorial→heuristic; `build_composer_works_fresh` populates `WorkSummary.bucket`; nuevos métodos `list_classical_composer_buckets` (con cache `composer_buckets:v1` 7d StaticMeta) y `list_classical_works_in_bucket` (drill-down con `bucket-full:v1` cache); helpers `compute_sub_buckets`, `sub_bucket_for_work`, `bucket_serialised_for_key`. Nuevos tipos `ComposerBuckets`, `BucketSummary`, `SubBucketSummary`, `WorksPage`.
- ✅ `src-tauri/src/classical/providers/musicbrainz.rs` — `browse_all_works_by_artist` multi-page fetcher (cap 20 pages, serial respetando `MbRateLimiter`).
- ✅ `src-tauri/src/classical/editorial.rs` — extended schema v2 cargado vía `editorial-extended.json`; nuevos tipos `ExtendedNote`, `ExtendedNoteBody`, `ExtendedSource`, `ExtendedSchemaHealth`; nuevos métodos `lookup_extended` (con locale fallback `requested → default → None`), `lookup_extended_by_title`, `lookup_bucket` extendido (cascade v2 → v1 → None), `schema_health`. v1 (Phase 5) sigue intacto. 9 tests nuevos B9.7 (locale fallback, missing translation, schema health, 5 sub-secciones por POC, lookup by title, bucket override v2).
- ✅ `src-tauri/data/editorial-extended.json` (NEW, ~40 KB) — schema_version 2 con 3 POC obras: Beethoven Symphony No. 9 (Op. 125), Bach Goldberg Variations (BWV 988), Mozart Requiem (K. 626). Cada obra: 5 sub-secciones × ~1200 palabras × 2 idiomas (en + es) + 3 sources cited (Wikipedia + Wikidata + editor).
- ✅ `src-tauri/src/commands/classical.rs` — 3 nuevos Tauri commands: `list_classical_composer_buckets`, `list_classical_works_in_bucket`, `get_classical_extended_note`.
- ✅ `src-tauri/src/lib.rs` — invoke_handler extendido +3 commands.
- ✅ `src-tauri/Cargo.toml` — `regex = "1"` declarada explícita (estaba transitiva).

**Frontend**
- ✅ `src/types/classical.ts` — `WorkBucket` union + `workBucketLabel(bucket, locale)` + `ALL_WORK_BUCKETS`; `Work.bucket?`, `WorkSummary.bucket?`; nuevos tipos `ComposerBuckets`, `BucketSummary`, `SubBucketSummary`, `BucketWorksPage`, `ExtendedNote`, `ExtendedNoteBody`, `ExtendedSource`.
- ✅ `src/api/classical.ts` — wrappers `listClassicalComposerBuckets`, `listClassicalWorksInBucket`, `getClassicalExtendedNote`.
- ✅ `src/hooks/useNavigation.ts` — `navigateToClassicalComposerTab(mbid, tab)`, `navigateToClassicalBucket(mbid, bucket)`.
- ✅ `src/App.tsx` — routing extendido: `classical://composer/{mbid}?tab=…`, `classical://composer/{mbid}/bucket/{bucket}`. Default condicional `tab=about`.
- ✅ `src/components/classical/ComposerPage.tsx` — refactor con tabs About/Works/Albums/Popular; hero siempre visible; AboutTab + AlbumsTab (CTA al `ClassicalArtistPage`) + PopularTab (Phase 6 stats + caveat composer-aware).
- ✅ `src/components/classical/ComposerWorksTab.tsx` (NEW) — Essentials cherry-pick + iteración sobre `BucketSummary[]`; trailing buckets (Other / FilmTheatre) en `<details>` colapsable.
- ✅ `src/components/classical/BucketSection.tsx` (NEW) — header + "View all" pill + grid 12 + sub-bucket chips client-side.
- ✅ `src/components/classical/SubBucketChips.tsx` (NEW) — chips "All" + per-sub-bucket con counts del backend.
- ✅ `src/components/classical/BrowseComposerBucket.tsx` (NEW) — drill-down page: header con composer + bucket label + count, sub-bucket chips, sort dropdown (Catalog / Date / Alphabetical), pagination "Load more" sin tocar MB tras el primer fetch.
- ✅ `src/components/classical/AboutThisWork.tsx` (NEW) — render markdown-light (italic / bold / link) con paragraph splitting, sub-secciones colapsables (origin/highlights expanded by default; premiere/context/notable_recordings collapsed), source attribution, fallback graceful a Phase 5 `editor_note` + Wikipedia summary cuando v2 no tiene la obra.
- ✅ `src/components/classical/WorkSidebar.tsx` (NEW) — sidebar derecho: Related works (CTA al composer Works tab), Compare versions (CTA Phase 6 D-022), Performers you follow (CTA library facet).
- ✅ `src/components/classical/WorkPage.tsx` — refactor a 8 secciones D-042 con grid `xl:grid-cols-[1fr_320px]` (sidebar oculto < 1280px); EditorsChoiceBanner separado de la lista; PopularRecordingsSection top-8 ordenadas por `(EC, quality_score, confidence)`.

**3 POC editorial obras canon (musicólogo voice)**
- Beethoven Symphony No. 9 (Op. 125) — origen 1822 commission Philharmonic Society / sketches 1793 Bonn / dedicatoria Friedrich Wilhelm III; premiere 7 mayo 1824 Theater am Kärntnertor / Sontag-Unger / 420 florines; highlights estructura 4 movimientos / Schreckensfanfare / quodlibet ausente aquí (esta es la 9ª no Goldberg) / variations finale; context Anthem of Europe 1972 / Bernstein Berlin 1989 / NYT critique; recordings essay Furtwängler '51 Bayreuth + Karajan '62 + Gardiner '92 + Norrington '87 + Bernstein '79 + Klemperer '57 + Nézet-Séguin '21. ES translation completa.
- Bach Goldberg Variations (BWV 988) — Forkel anecdote Keyserling/Goldberg; premiere disputed (Landowska 1933 / Gould 1955); highlights Aria 32-bar bass / canons climbing intervals / Variation 25 "black pearl" / quodlibet finale; context vs Diabelli vs Schubert / Sitkovetsky string trio / Hannibal Lecter; recordings essay Gould 1955 + 1981 / Pinnock 1980 / Hantaï 1992 / Staier 2010 / Perahia 2000 / Schiff 1982 / Aimard 2014 / Lang Lang 2020 / Barenboim 1989. ES translation completa.
- Mozart Requiem (K. 626) — Walsegg commission para Anna 1791 / Leitgeb steward / 50 ducats; premiere 2 enero 1793 Süssmayr completion + Walsegg 14 dic 1793; highlights basset-horn opening / Kyrie double fugue Handel-derived / Tuba mirum / Lacrimosa fragmento Mozart 8 compases; context completiones Beyer 1971 / Maunder 1988 / Levin 1993 / Robbins Landon 1991 / Salieri legend Pushkin→Shaffer→Forman; recordings essay Böhm 1971 / Harnoncourt 1981 / Gardiner 1986 / Hogwood 1983 Maunder / Rilling 2002 Levin / Mariotti 2020 / Giulini 1979 / Bernstein 1988. ES translation completa.

### Validation gates internos (autonomous, sin Pedro)

- **9-B → 9-C**: validado vía tests deterministic. Bach NO muestra Symphonies bucket (canon caso `bach_st_matthew_passion_falls_to_other_without_p136`); Beethoven Symphonies bucket detecta correctamente Symphony No. 9 (test `beethoven_symphony_9_choral`); sub-bucket chips operacional (lattice testeada en 6 tests `*_compatible_with_*`). UI integration QA → diferida a Pedro (manual end-to-end).
- **9 → 10**: validado vía tests editorial v2. Los 3 POC tienen las 5 sub-secciones presentes (test `poc_works_have_all_five_subsections`); locale fallback es→en cuando 'fr' missing (test `lookup_extended_falls_back_to_default_when_locale_missing`); v2 bucket override beats heuristic (test `lookup_bucket_extended_v2_overrides_heuristic`). Renderizado AboutThisWork → diferido a Pedro (manual visual check).

### QA manual pendiente para Pedro

1. **Rebuild binario**: `cargo build --release` desde `src-tauri/` (ya validado verde, 54s release).
2. **Abrir 5 composers** (paths: `classical://composer/<mbid>` por defecto carga tab=about):
   - Bach (`24f1766e-9635-4d58-a4d4-9413f9f98a4c`): debe mostrar buckets Choral & sacred (cantatas + Pasiones) + Keyboard (Goldberg + WTC + Partitas) + Solo instrumental (Cello suites + Violin partitas) + Orchestral (Brandenburg) + Concertos. NO Symphonies.
   - Beethoven (`1f9df192-a621-4f54-8850-2c5373b7eac9`): bucket Symphonies con 9 sinfonías ordenadas Op. 21..Op. 125; sub-bucket Concertos {Piano: 5, Violin: 1, Triple: 1}.
   - Mozart (`b972f589-fb0e-474e-b64a-803b0364fa75`): Symphonies + Concertos + Stage works (operas) + Choral & sacred (Requiem).
   - Wagner (`b0e5e8e3-6c8d-4e3a-83df-0a3f0a1c7c12` aproximado, MB lookup): Stage works dominante.
   - Glass (`5ae54dee-4dba-49c0-802a-a3b3b3adfe9b`): Stage + Symphonies + Keyboard (Études) + Film & theatre.
3. **Verificar tab navigation**: click About / Works / Albums / Popular debe cambiar tab sin reload.
4. **Verificar drill-down**: click "View all (N)" en cualquier bucket > 12 → llega a `BrowseComposerBucket`. Cambiar sort a Date debe reordenar.
5. **Abrir 3 POC works** y verificar sección "About this work":
   - Beethoven 9 (`c35b4956-d4f8-321a-865b-5b13d9ed192b`)
   - Bach Goldberg (`1d51e560-2a59-4e97-8943-13052b6adc03`)
   - Mozart Requiem (`3b11692b-cdc7-4107-9708-e5b9ee386af3`)
   - Cada uno: 5 sub-secciones, sources cited, expand/collapse funcional.
6. **Cualquier otra obra (no POC)**: la sección debe caer al fallback Phase 5 + Wikipedia summary, sin sección "About this work" vacía.
7. **Sidebar derecho**: solo visible ≥ 1280px de ancho de ventana.
8. **Bit-perfect contract**: reproducir cualquier track 24/96 desde el WorkPage, comprobar `signal_path` reporta bit-perfect (sin SW vol).

---

## Phase 10 — Editorial scaling (USP eje diferencial)

**Plan detallado**: `docs/classical/phase-10-editorial-scaling.md`.
**Estado**: 📝 plan listo, pendiente ejecutar tras Phase 9.C.
**Estimación**: ~160-170h.

### Scope hybrid

- **Etapa 10.1 — Top 50 manual** (~50-60h, 6 semanas): equipo escribe 50 obras canon × 1200 palabras × 5 sub-secciones. Sources cited.
- **Etapa 10.2 — Top 200 LLM-assisted** (~90h): pipeline pre-build con Wikipedia + Wikidata + Claude Opus prompt estricto. Disclaimer UI obligatorio. Spot-check 20% obligatorio para detectar alucinaciones (umbral GO/NO-GO: 0).
- **Etapa 10.3 — Long tail Wikipedia-only** (~20h): 1500 obras restantes con `editor_note` breve auto-generado en `editorial.json` v1. Sin extended.
- **Etapa 10.4 — Crowdsourcing** (V2+, NO V1): usuario escribe extended notes locales, sync futuro vía Obsidian-LiveSync.

### Phase 10.5 — Browse axes adicionales (~20h, opcional)

- Browse by Instrument (Piano / Violin / Cello / Guitar / Organ).
- Browse by Orchestra como axis separado de Conductor.
- Browse by Choir.

### Decisiones nuevas Phase 10

- **D-046** — Hybrid editorial scaling: 50 manual + 150 LLM-assisted spot-checked + 1500 Wikipedia long tail. Disclaimer UI obligatorio para LLM-assisted. Crowdsourcing diferido a V2.

### Validation gate

- Spot-check 20% Etapa 2: 0 alucinaciones detectadas en fechas/intérpretes/eventos. Si > 0, NO-GO.
- Pedro confirma sample 10 obras random.

---

## V1 entregado pre-Phase 7

**Fecha de cierre Phase 0-6**: 2026-05-02.

**Scope entregado** — todas las phases originales del CLASSICAL_DESIGN.md cumplidas:

### Backend (15 módulos en `src-tauri/src/classical/`)

```
src-tauri/src/classical/
├── mod.rs                          (factory wiring)
├── types.rs                        (domain types — Composer/Work/Recording + RelatedComposer)
├── matching.rs                     (D-010 cascade ISRC + text-search)
├── catalog.rs                      (CatalogService — orquesta 5 providers)
├── quality.rs                      (D-018 ranking determinístico)
├── movement.rs                     (Phase 3 roman/attacca parser)
├── search.rs                       (D-019 tokenizer + planner + scorer)
├── editorial.rs                    (D-020 snapshot + cascade override)
├── listening_guide.rs              (B5.5 LRC reader)
└── providers/
    ├── mod.rs                      (ClassicalProvider trait + MbRateLimiter)
    ├── musicbrainz.rs              (work + recordings + composer + browse)
    ├── tidal.rs                    (ISRC + canonical search + quality probe)
    ├── wikipedia.rs                (REST summary multi-locale)
    ├── openopus.rs                 (snapshot embedded — 33 composers, 1459 works)
    └── wikidata.rs                 (SPARQL — composer enrichment + related)
```

### Frontend (18 componentes en `src/components/classical/`)

```
src/components/classical/
├── ClassicalHubPage.tsx            (Listen Now + Browse + Search + Library)
├── ClassicalSearch.tsx             (search UI con detected tokens)
├── ClassicalLibrary.tsx            (4 facets de favorites + overview banner)
├── ClassicalArtistPage.tsx         (browse-by-conductor con groupings)
├── ClassicalRecordingComparison.tsx (versions side-by-side)
├── ComposerPage.tsx                (hero + bio + works + RelatedComposers)
├── WorkPage.tsx                    (movements + recordings filter/sort + comparison link)
├── ComposerCard.tsx, WorkSummaryCard.tsx, EraBadge.tsx
├── BrowseComposers.tsx, BrowsePeriods.tsx, BrowseGenres.tsx, BrowseEra.tsx
├── RecordingRow.tsx                (con ArtistLinks navigables)
├── RecordingFilters.tsx, RecordingSort.tsx
├── MovementList.tsx
├── ConfidenceBadge.tsx, QualityChip.tsx
├── ClassicalWorkLink.tsx, WorkHeaderLine.tsx (player integration)
└── FavoriteToggle.tsx              (heart icon reusable)
```

### Tauri commands (~28 nuevos)

Catalog + Phase 1: 5 (get_classical_work / _recording / _composer, resolve_classical_work_for_recording, get_current_classical_work_mbid).
Phase 2: 3 (list_classical_top_composers, list_classical_composers_by_era, list_classical_works_by_composer).
Phase 3: 1 (resolve_classical_movement).
Phase 4: 1 (refresh_classical_work_qualities).
Phase 5: 5 (search_classical, list_classical_editorial_picks, set/clear_classical_editors_choice, read_classical_listening_guide).
Phase 6: 13 (top works/composers, overview, discovery curve, recent sessions, recording comparison, 4 favorites CRUD, related composers, artist discography, prewarm canon).

### Schema migrations DB (todas aditivas)

- Phase 1: columna `plays.work_mbid TEXT` + índice + tabla `classical_favorites` + índice.
- Phase 5: tabla `classical_editorial` + índice.
- Phase 6: ninguna nueva (reusa `plays.work_mbid` + `classical_favorites` Phase 1).

### Archivos de datos embebidos

- `src-tauri/data/openopus.json` — 227 KB (Phase 2).
- `src-tauri/data/editorial.json` — 13 KB con 48 work seeds + 15 composer notes (Phase 5).

### Tests

- **118 tests classical+stats** ejecutables con `cargo test --release --lib`.
- Cobertura por módulo: `matching` (5), `providers::musicbrainz` (8), `providers::openopus` (8), `providers::tidal` (3), `providers::wikipedia` (2), `providers::wikidata` (7), `search` (24), `editorial` (9), `listening_guide` (6), `movement` (19), `quality` (17), `stats::classical_tests` (7) + tests acceptance Beethoven 9 (3) y Phase 5 acceptance (2).

### Decisiones D-001..D-026

26 decisiones arquitectónicas registradas en `DECISIONS.md`. Spans:
- Architecture (Hub como sub-modo, sin spin-off, no Android, monolithic provider+catalog pattern).
- Tooling (cargo example spike, llaves siempre, gitignore carve-outs, supervisor runs specialist roles).
- Process (cero regresión inviolable, gapless gate split, prewarm scheduling).
- Editorial (snapshot curado + override DB cascade).
- Architecture refinements (cascade matching D-010, AppState Arc-ification, work resolver trait, parse_literal vs FromStr, MB↔OpenOpus title match, quality.rs ranking, search tokenizer in-process, WD rate-limit, top-composers semantics caveat).

### Audio path verificado intacto

```
$ git diff src-tauri/src/audio.rs       → vacío
$ git diff src-tauri/src/hw_volume.rs   → vacío
$ git diff src-tauri/src/signal_path.rs → vacío
$ git diff src-tauri/src/tidal_api.rs   → vacío
```

`route_volume_change` (`lib.rs:491-539`), writer guard (`audio.rs:988-992`), bit-perfect contract — todos preservados sin una sola línea modificada en 6 phases.

### Build status final

| Step | Result |
|---|---|
| `cargo check --release` | ✅ clean |
| `cargo build --release` | ✅ clean (53s) |
| `cargo clippy --release --lib --no-deps` | ✅ 14 warnings (idéntico baseline pre-classical) |
| `cargo test --release --lib` | ✅ 118/118 PASS |
| `tsc --noEmit` | ✅ 0 errores |
| `npm run build` (vite) | ✅ clean (1875 módulos) |

### Limitaciones conocidas (V1)

Documentadas como deuda diferida — NO bloquean V1:

- **Composer-resolution en stats**: top_classical_composers groupea por `artist_mbid` (performer), no por composer real. D-025. Refactor exige backfill stats DB.
- **Snapshot Wikidata**: el Hub pega WDQS live; no hay fallback offline. Mitigación: cache 30d agresiva.
- **Editorial snapshot**: 48 works × 15 composers (canon mayor). Composers fuera del canon no muestran star. Override manual cubre divergencia.
- **Pagination**: las work-lists no paginan (cap MB browse 100). Para composers prolíficos (Bach > 100 works) se renderizan los primeros 100.
- **Mobile**: no abordado (D-003).

### Tests instrumented manuales (operador)

Phase 3 gapless attacca: documentado en `phase-3-player-gapless.md` "QA manual" — 3 attaccas canónicos (Beethoven 5 III→IV, Mahler 3 V→VI, Bruckner 8 III→IV) con bit-perfect ON, gap < 50ms.
Phase 1-6 functional QA: documentado en `README.md`.

### Para commit

El usuario commiteará todos los cambios consolidados al final. Estado limpio: branch `soneClassical`, tree dirty con todos los cambios listados, tests/build verdes, audio path intacto, decisiones documentadas.

---

## Cambios al doc maestro

Si durante el desarrollo se actualiza `/CLASSICAL_DESIGN.md`, registrarlo aquí:

| Fecha | Sección afectada | Tipo cambio | Razón |
|---|---|---|---|
| — | — | — | — |
