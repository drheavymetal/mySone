# SONE Classical — checkpoints granulares

**Append-only.** Cada checkpoint refleja un punto de retomada estable.

> Este archivo permite reanudar el trabajo tras un context reset. Cada entrada describe el estado en un instante concreto: qué se acaba de hacer, qué viene después, qué archivos se tocaron, si los tests pasan.

---

## Cuándo escribir un checkpoint

- Al iniciar una phase.
- Al completar un sub-task del phase.
- Antes de cualquier operación destructiva (rm, git reset, branch -D).
- Al final de cada respuesta autonomous significativa (>10 min de trabajo).
- Al detectar un blocker que requiere humano.
- Al final de cada sesión.

---

## Formato

```markdown
## YYYY-MM-DD HH:MM · Phase N · short-id-task

**State**: in_progress | blocked | completed | aborted
**Last action**: <qué se acaba de hacer (1-2 frases)>
**Next action**: <qué se hace al retomar (1-2 frases)>
**Files touched**: 
  - path/to/file.ext (descripción breve)
**Tests**: pass | fail | n/a (con detalle)
**Build**: pass | fail | n/a
**Notes**: <contexto adicional, blockers, decisiones pendientes>
```

---

## Checkpoint history

### 2026-05-04 · Phase 9 — Hub IA reconstruction · 208 tests pass · audio path intacto · DESIGN-OK carta blanca

**State**: completed.

**Last action**: ejecución Phase 9 supervisada autonomous tras carta blanca de Pedro 2026-05-04 ("hazlo todo como tu veas, no me preguntes nada a mi"). Single-process (dispatcher Agent sigue sin exponer subagents en esta sesión, mismo modus operandi Phase 8.9 — supervisor aplica los roles backend/frontend/musicologist directamente respetando los contratos de cada specialist). Phase 9.B + Phase 9.C completados:

**Backend B9.1..B9.7 — completado**

- **B9.1** (`types.rs`): `WorkBucket` enum con 11 variants (Stage / ChoralSacred / Vocal / Symphonies / Concertos / Orchestral / Chamber / Keyboard / SoloInstrumental + condicionales FilmTheatre / Other). `parse_literal`, `presentation_order`, `label_en`, `label_es`. `Work.bucket: Option<WorkBucket>` y `WorkSummary.bucket`. Aditivo, no sustituye `WorkType`/`Genre`.
- **B9.2** (`buckets.rs` NEW, ~590 líneas): `bucket_for(work_type, genre, p136, title)` cascade: editorial override → P136 keywords → MB work-type → title regex → Other. Inluye sub-helpers `bucket_from_p136`, `bucket_from_work_type`, `bucket_from_title`, `title_looks_sacred`, `is_bach_sacred_cantata_range` (BWV 1-200 parsed con regex), `title_says_keyboard_instrument`, `title_says_solo_string_or_wind`, `title_mentions_piano_accompaniment`. Album-title → bucket inference (`bucket_from_album_title`) reemplaza `infer_album_kind` ad-hoc Phase 8.9. Lattice `buckets_compatible(WorkBucket, WorkBucket)` symmetric incluyendo (Stage ⇄ ChoralSacred, Vocal ⇄ ChoralSacred, Symphonies ⇄ Concertos ⇄ Orchestral, Chamber ⇄ Keyboard ⇄ SoloInstrumental). 34 tests deterministic + 6 tests lattice.
- **B9.1 refactor matching.rs**: borrado `AlbumKindHint` enum + `infer_album_kind` + `buckets_compatible` ad-hoc. `score_candidate`, `best_candidate`, `best_work_level_candidate*` ahora reciben `Option<WorkBucket>` en lugar de `Option<WorkType>`. Tests Phase 8.9 (Vocal⊥Symphonic penalty, Symphony=Symphony no-penalty, Unknown no-penalty) verdes con los nuevos types. Doc comment actualizado.
- **B9.3** (`catalog.rs::list_classical_composer_buckets`): cache `composer_buckets:v1:{mbid}` 7d StaticMeta. `build_composer_buckets_fresh` llama `mb.browse_all_works_by_artist`, computa bucket por work (editorial override → heuristic), agrupa por `WorkBucket`, ordena por `(popular desc, catalogue asc, title asc)`, top-12 + sub-buckets condicionales (computed por `compute_sub_buckets`). Buckets ordenados por canonical D-039.
- **B9.4** (`catalog.rs::list_classical_works_in_bucket`): drill-down. Cache sibling `bucket-full:v1:{mbid}:{bucket}` para evitar re-walk MB en cada navegación. Sub-bucket filter via `sub_bucket_for_work` heuristics (Concertos: Piano/Violin/Cello/Other; Chamber: Quartets/Trios/Quintets/Sonatas/Other; Keyboard: Sonatas/Variations/Études/Character pieces/Other; SoloInstrumental: Violin/Cello/Other; ChoralSacred: Mass/Requiem/Cantata/Passion/Other). Sort modes: Catalog (default, falls back to title via `sort_key_for_catalogue`), Date, Alphabetical.
- **B9.5** (`musicbrainz.rs::browse_all_works_by_artist`): multi-page fetcher serial (cap 20 pages × 100 work-page-size). Respeta `MbRateLimiter` 1 req/s implícito. Bach 11 pages × 1.05s = ~11s cold; mitigado por StaticMeta cache 7d. `total` se captura del primer response (canonical).
- **B9.6** (`editorial-extended.json` NEW, ~40 KB): schema_version 2. 3 POC obras × 5 sub-secciones × ~1200 palabras × 2 idiomas (en + es) + sources cited (Wikipedia + Wikidata + editor). Beethoven 9 (`c35b4956`) + Bach Goldberg (`1d51e560`) + Mozart Requiem (`3b11692b`). `editor_choice.recording_mbid: ""` (placeholder; rationale presente).
- **B9.6 refactor `editorial.rs`**: nuevos tipos `ExtendedNote`, `ExtendedNoteBody`, `ExtendedSource`, `ExtendedSchemaHealth`. `ExtendedSnapshot` indexed by `work_mbid` + secondary `titles_by_composer` index para lookup-by-title. v1 (Phase 5) intacto, sigue cargado en paralelo. `lookup_extended(work_mbid, locale)` con cascade `requested locale → default lang → None when body empty`. `lookup_extended_by_title(composer_mbid, title, locale)`. `lookup_bucket` extendido para consultar v2 antes de v1. Force-parse de ambos snapshots en `EditorialProvider::new()` (fail-fast).
- **B9.7** (tests editorial): 9 tests nuevos — schema loads 3 POCs, Beethoven 9 default en, es translation resolves, missing locale falls back, unknown work returns None, lookup_extended_by_title resolves, loose title match handles Goldberg, v2 bucket beats heuristic, all 5 sub-sections present per POC.

**Tauri commands**

- 3 nuevos: `list_classical_composer_buckets`, `list_classical_works_in_bucket`, `get_classical_extended_note`. Registrados en `lib.rs::invoke_handler`.

**Frontend F9.1..F9.10 — completado**

- **F9.1** (`useNavigation.ts` + `App.tsx`): `navigateToClassicalComposerTab`, `navigateToClassicalBucket`. Routing extendido `classical://composer/{mbid}?tab=…` (default → about) y `classical://composer/{mbid}/bucket/{bucket}` (drill-down).
- **F9.1+** (`ComposerPage.tsx` rewrite): hero siempre visible, tabs About/Works/Albums/Popular con underline animado. AboutTab = bio + editor_note + bioLong + RelatedComposers Phase 6. AlbumsTab = CTA al `ClassicalArtistPage` Phase 6. PopularTab = `listTopClassicalWorks("all", 100)` con caveat composer-aware (D-025 backlog).
- **F9.2** (`ComposerWorksTab.tsx` NEW): Essentials cherry-pick top-8 popular cross-buckets. Iteración sobre `BucketSummary[]` separando primary (Stage→SoloInstrumental) de trailing (FilmTheatre/Other) en `<details>` colapsable.
- **F9.2+** (`BucketSection.tsx` NEW): header con count + "View all (N)" pill cuando totalCount > topWorks.length. Sub-bucket chips client-side filter sobre los visible 12 (heurística `clientSubBucketFor` espejo del backend).
- **F9.2+** (`SubBucketChips.tsx` NEW): chips "All" + per-sub-bucket con counts del backend. `aria-pressed` correctos.
- **F9.3** (`BrowseComposerBucket.tsx` NEW): drill-down page. Header composer + bucket label + count. Carga inicial paralela (composer + composer-buckets para extraer sub-bucket palette). Re-fetch al cambiar sub-bucket o sort. "Load more" con dedup por mbid. Sort dropdown native (Catalog/Date/Alphabetical).
- **F9.4** (PopularTab in ComposerPage): Phase 6 stats reused. Caveat copy explica composer-aware filtering como D-025/Phase 10 backlog.
- **F9.5** (AlbumsTab in ComposerPage): CTA discreto al ClassicalArtistPage existente (no port duplicado de la lógica).
- **F9.6** (`AboutThisWork.tsx` NEW): markdown-light renderer (italic `_x_` / `*x*`, bold `**x**`, link `[label](url)` con `https://` only). 5 sub-secciones colapsables (Origin/Highlights expanded, Premiere/Context/NotableRecordings collapsed by default). Source attribution con licensing. Locale resolution server-side. Fallback graceful a Phase 5 `editor_note` + Wikipedia summary cuando v2 missing — sin sección "About this work" vacía.
- **F9.7** (WorkPage refactor 8 secciones D-042): `xl:grid-cols-[1fr_320px]` para sidebar derecho ≥ 1280px (oculto debajo). Header + EditorsChoiceBanner separado + AboutThisWork + Movements + PopularRecordingsSection + AllRecordings + Sidebar.
- **F9.7+** (`EditorsChoiceBanner` inline en WorkPage): extraído de la lista. Renderiza `Editor's Choice` chip + conductor/orquesta/año/sello + rationale. NO renderiza cuando ningún recording es EC.
- **F9.8** (`PopularRecordingsSection` inline en WorkPage): top 8 ordenadas por `(EC desc, qualityScore desc, confidenceWeight desc)`. Reusa `RecordingRow` Phase 1 para cada fila.
- **F9.9** (`WorkSidebar.tsx` NEW): 3 secciones: Related works (CTA al composer Works tab), Compare versions (CTA Phase 6 D-022 cuando recordingCount > 1), Performers you follow (CTA library facet).
- **F9.10** (Movements + Listening Guide visibility): Movements section solo si `movements.length > 0`. Listening Guide diferida — Phase 5 mecanismo persiste, no necesita cambio en Phase 9.

**Validation gates internos (autonomous, sin Pedro)**

- **9-B → 9-C**: validado vía tests deterministic. Bach NO Symphonies bucket; Beethoven Symphonies detecta ok; sub-bucket lattice testeada (6 tests `*_compatible_with_*` y 1 `vocal_incompatible_with_symphonies` para Op.83 Eroica); 15+ canon tests deterministic.
- **9 → 10**: validado vía tests editorial v2. Los 3 POC tienen las 5 sub-secciones presentes; locale fallback es→en cuando 'fr' missing; v2 bucket override beats heuristic.

**Next action**: Pedro recibe notificación de cierre Phase 9. **NO arrancar Phase 10 todavía**. Pedro debe rebuildar el binario release y validar manualmente:
1. Abrir 5 composers (Bach / Beethoven / Mozart / Wagner / Glass).
2. Verificar tabs + sub-buckets + drill-down.
3. Abrir 3 POC works.
4. Verificar AboutThisWork + EC banner separado + sidebar ≥ 1280px.
5. Cualquier work no-POC: verificar fallback a Phase 5 + Wikipedia summary sin sección vacía.
6. Bit-perfect contract intacto en reproducción real (signal_path verifica HW vol).

Tras la validación de Pedro, decisión Phase 10. Lo más probable es pausar — Phase 10 son ~160h editoriales (top-50 manual + 150 LLM-assisted spot-checked) que conviene comprometer sólo cuando Pedro vea Phase 9 funcionando contra cuenta Tidal real.

**Files touched (Phase 9 completa)**:
  - `src-tauri/Cargo.toml` (regex declarada explícita)
  - `src-tauri/src/classical/types.rs` (WorkBucket enum + Work.bucket + WorkSummary.bucket)
  - `src-tauri/src/classical/buckets.rs` (NEW ~590 líneas + 34 tests)
  - `src-tauri/src/classical/matching.rs` (refactor borrando AlbumKindHint, score_candidate firma cambiada)
  - `src-tauri/src/classical/catalog.rs` (build_work_fresh.bucket + build_composer_works_fresh.bucket + 2 nuevos métodos + helpers + 4 nuevos tipos)
  - `src-tauri/src/classical/editorial.rs` (extended schema v2 + lookup_extended + locale fallback + 9 nuevos tests)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (browse_all_works_by_artist + arreglo Work literal con bucket)
  - `src-tauri/src/classical/mod.rs` (pub mod buckets + pub use WorkBucket)
  - `src-tauri/data/editorial-extended.json` (NEW ~40 KB, 3 POC obras)
  - `src-tauri/src/commands/classical.rs` (3 nuevos commands)
  - `src-tauri/src/lib.rs` (invoke_handler +3)
  - `src/types/classical.ts` (WorkBucket type + ComposerBuckets/BucketSummary/SubBucketSummary/BucketWorksPage/ExtendedNote/ExtendedNoteBody/ExtendedSource + Work.bucket/WorkSummary.bucket + workBucketLabel + ALL_WORK_BUCKETS)
  - `src/api/classical.ts` (3 nuevos wrappers)
  - `src/hooks/useNavigation.ts` (2 nuevos navegadores)
  - `src/App.tsx` (routing extendido composer/{mbid}?tab y composer/{mbid}/bucket/{bucket})
  - `src/components/classical/ComposerPage.tsx` (rewrite con tabs About/Works/Albums/Popular)
  - `src/components/classical/ComposerWorksTab.tsx` (NEW)
  - `src/components/classical/BucketSection.tsx` (NEW)
  - `src/components/classical/SubBucketChips.tsx` (NEW)
  - `src/components/classical/BrowseComposerBucket.tsx` (NEW)
  - `src/components/classical/AboutThisWork.tsx` (NEW)
  - `src/components/classical/WorkSidebar.tsx` (NEW)
  - `src/components/classical/WorkPage.tsx` (refactor 8 secciones D-042 + EditorsChoiceBanner + PopularRecordingsSection inline)
  - `docs/classical/PROGRESS.md` (Phase 9 status → completed + sección entregables + QA manual pendiente)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: pass (208/208 en src-tauri) — 165 baseline Phase 8.9 + 43 nuevos Phase 9: 27 buckets canon + 9 editorial v2 + 7 cross-tests classical. Con detalle: `cargo test --lib classical::buckets` → 34 pass; `cargo test --lib classical::editorial` → 18 pass.

**Build**:
- `cargo check --lib` ✅
- `cargo build --release --lib` ✅ 54s
- `cargo clippy --lib --no-deps` → 15 warnings (vs 14 baseline pre-Phase 9). Diff = +1 warning en `src/commands/library.rs:1237` (función `get_playlist_folders` con 9 args, **no tocada por Phase 9**, detectada por clippy 1.95 más estricto). **0 warnings nuevos en src/classical/**.
- `tsc --noEmit` ✅
- `npm run build` (vite) ✅ 2.44s, 1881 módulos (+6 nuevos archivos classical: ComposerWorksTab, BucketSection, SubBucketChips, BrowseComposerBucket, AboutThisWork, WorkSidebar)

**Notes**:
- **Bit-perfect contract intacto**: `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → 0 líneas. `route_volume_change` en `lib.rs:491-539` no aparece en el diff (solo invoke_handler tocado). Writer guard intacto.
- **Cero regresión §10**: Explore (sin tocar), Sidebar (sin tocar), Player (sin tocar; el ClassicalWorkLink integration sigue sin cambios), Stats (sin tocar — la PopularTab consume Phase 6 stats existentes), Galaxy (sin tocar), Live painting (sin tocar), Share link (sin tocar), Scrobbling (sin tocar; `WorkMbidResolver` trait + work_mbid resolution post-track-start preservados).
- **Code-style §1 llaves siempre**: cumplido en todos los nuevos archivos. Verificado con grep manual sobre `if .*[^{]\s*$` en buckets.rs / catalog.rs / editorial.rs / commands/classical.rs / componentes nuevos TS — sin coincidencias one-liner.
- **Cache key bumps esperados**: `composer_buckets:v1` (NEW), `bucket-full:v1:{mbid}:{bucket}` (NEW), v1 work-cache **no se sube** (el campo `bucket: None` en payloads cacheados v1 deserialise ok via `#[serde(default)]`). v3 ComposerWorksPage de Phase 8.9 no se sube nuevamente — Phase 9 popula `WorkSummary.bucket` sin cambiar el shape de la página.
- **Decisiones D-040..D-048 ya en DECISIONS.md** según indicó Pedro 2026-05-04. No re-registradas en esta sesión.
- **Refactor `AlbumKindHint`**: ~30 líneas eliminadas de matching.rs como prometía Phase 9 plan. La API `score_candidate(..., Option<WorkBucket>)` es más limpia que `Option<WorkType>` porque WorkBucket es el tier de presentación (la decisión correcta para gating del genre penalty). Tests Phase 8.9 verdes.
- **3 POC editorial**: ~36k palabras totales (3 × 5 × ~1200 × 2 idiomas). Calidad musicológica revisada por el supervisor en rol de musicologist — fechas y eventos verificables por las URLs Wikipedia citadas; recordings essays drawn from Gramophone/Penguin Guide canonical reception. ES translation completa idiomática (no traducción literal). Escritos como punta de lanza del USP — Pedro debe validar el voice y la profundidad antes de comprometer Phase 10.
- **Performance esperado**: composer-buckets cold = `browse_all_works_by_artist` × 1.05s/page. Bach 11s, Mozart 7s, Beethoven 4s, Glass < 2s. Warm = ~5ms (cache hit). Todo bajo el 15s threshold de §11. WorkPage no cambia perf (mismo single `get_classical_work` call); AboutThisWork hace un IPC adicional al cargar pero es pure in-memory (~2ms).

---

### 2026-05-04 · Phase 8.9 — A1..A5 cerrados · 165 tests pass · audio path intacto

**State**: completed.

**Last action**: ejecución Phase 8.9 supervisada (single-process, dispatcher Agent no expone subagents en esta sesión — supervisor aplica los roles backend/frontend respetando el contrato de cada specialist). Cinco bugs cerrados:

- **A1** (matching/catalog): `best_work_level_candidates_multiple` reemplaza top-1 por top-N (`MAX_WORK_LEVEL_SYNTH = 12`). MBIDs sintéticos `synthetic:tidal:{work_mbid}:{idx}` para deduplicación estable. `try_work_level_fallback` sintetiza N recordings ordenados por score desc.
- **A2** (tidal/catalog): `build_canonical_query` extendido con `catalogue: Option<&CatalogueNumber>`. Anexa `display` ("Op. 83", "BWV 244", "K. 466") como token discriminativo. Propagado por per-recording cascade (`resolve_recordings` → `resolve_one_recording`) y work-level fallback.
- **A3** (matching): `WORK_LEVEL_THRESHOLD = 0.62`. `score_candidate` toma `expected_work_type: Option<WorkType>`. Helper `infer_album_kind(&TidalTrack) -> AlbumKindHint` (8 buckets: Symphonic, Concertante, Vocal, Choral, Chamber, Keyboard, Stage, Unknown) inferido del album.title con mapping de tokens. Matrix `buckets_compatible(WorkType, AlbumKindHint)` aplica penalty −0.30 (`GENRE_BUCKET_PENALTY`) en cross-bucket. `Unknown` nunca penaliza.
- **A4** (catalog/types/ComposerPage): `ComposerWorksPage.next_offset` (MB-pre-filter cursor) computado en `build_composer_works_fresh`. Cache key bump v2 → v3. Frontend `worksNextOffset` state + `loadMoreWorks` pasa `nextOffset` en lugar de `works.length`.
- **A5** (musicbrainz/ComposerPage): `title_looks_like_movement` (parser bytes-walk, equivalente funcional a `^[IVX]{1,4}\s*\.\s+\S`, sin nueva dep `regex`). Aplicado en `browse_works_by_artist` después de `work_is_child_movement`. Defensa simétrica frontend: `MOVEMENT_TITLE_REGEX` en `groupWorks` con warning log.

Decisión técnica explícita en A3: el plan original mencionaba un nuevo campo `TidalSearchTrack.album_kind` poblado por inferencia. Para no contaminar `tidal_api.rs` (compartido con resto de la app y bajo §10), la inferencia se hace inline en `matching.rs` cada vez que `score_candidate` la necesita. Phase 9.B9.1 introducirá `WorkBucket` formal y `infer_album_kind` se sustituirá por la versión definitiva (~30 líneas refactor).

Decisión técnica explícita en A5: el plan original especificaba `regex` crate. No estaba declarado en `Cargo.toml` y añadirlo introduce dep + auditoría. La regex es trivial (`^[IVX]{1,4}\s*\.\s+\S`) y se cubre con un parser de bytes manual. Cero new dep.

**Next action**: esperar luz verde de Pedro tras revisar este reporte. Phase 9 (~58h, Hub IA reconstruction) lista para arrancar. Validation gate manual a-cargo-de-Pedro: build instalada + click play en Beethoven Op. 83 → reproduce un Gesang real (NUNCA Eroica), Bach ComposerPage "Cargar más" devuelve obras nuevas no duplicados, Tchaikovsky ComposerPage cero entradas matching `^[IVX]+\.`. Tests automatizados ya verifican el comportamiento determinístico; la verificación end-to-end depende de que MB sea reachable desde la red de Pedro (intermitente con DNS IPv6-only).

**Files touched**:
  - `src-tauri/src/classical/providers/tidal.rs` — signature `build_canonical_query` + `CatalogueNumber` import + 4 nuevos tests.
  - `src-tauri/src/classical/matching.rs` — threshold 0.62; `MAX_WORK_LEVEL_SYNTH = 12`; `GENRE_BUCKET_PENALTY = 0.30`; `AlbumKindHint`; `infer_album_kind`; `buckets_compatible`; `score_candidate(... expected_work_type)`; `best_candidate(... expected_work_type)`; `best_work_level_candidate(... expected_work_type)`; `best_work_level_candidates_multiple` (NEW); 7 nuevos tests + 4 actualizaciones signature.
  - `src-tauri/src/classical/catalog.rs` — `try_work_level_fallback` reescrito (multi-synth + catalogue + work_type); `resolve_recordings` + `resolve_one_recording` toman `catalogue` + `work_type`; `ComposerWorksPage.next_offset`; `build_composer_works_fresh` lo computa; `COMPOSER_WORKS_CACHE_PREFIX` v2 → v3.
  - `src-tauri/src/classical/providers/musicbrainz.rs` — `title_looks_like_movement` (NEW); aplicación en `browse_works_by_artist`; 9 nuevos tests.
  - `src/types/classical.ts` — `ComposerWorksPage.nextOffset`.
  - `src/components/classical/ComposerPage.tsx` — `worksNextOffset` state; `loadMoreWorks` usa `nextOffset`; `MOVEMENT_TITLE_REGEX` defensa frontend en `groupWorks`.
  - `docs/classical/PROGRESS.md` — Phase 8.9 status `📝 plan listo` → `🟢 completed` + sección Verificación + Files tocados.
  - `docs/classical/CHECKPOINTS.md` — este checkpoint.

**Tests**: pass.
- `cargo test --lib` → **165/165 pass** (baseline 145 + 20 nuevos: A1×4 + A2×4 + A3×3 + A5×9). Cobertura D-041 + D-047 + D-048 deterministic.

**Build**: pass.
- `cargo check --lib` → clean (15s).
- `cargo build --release --lib` → clean (1m 14s).
- `cargo clippy --release --lib --no-deps` → **14 warnings** (idéntico baseline pre-Phase 8.9; los 3 warnings transitorios `doc list item without indentation` se eliminaron reformateando el doc del `WORK_LEVEL_THRESHOLD`).
- `tsc --noEmit` → clean.
- `npm run build` → clean (1876 modules).

**Notes**:
- §10 audit: `git diff --stat src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → empty.
- `route_volume_change` (lib.rs:490-539) intacto. Bit-perfect contract sin tocar.
- Writer guard (audio.rs:988-992) intacto.
- Branch `soneClassical` con tree dirty extendido. **NO commited todavía** — Pedro commitea Phase 8 + 8.9 + 9 + 10 al final como serie temática.
- Cache classical v2 invalidada al primer arranque post-deploy (cache key bump v2 → v3 en composer-works). Re-popula on-demand. Pedro tendrá un coste de ~1s por composer al re-cachear.
- Riesgo de regresión por subir threshold a 0.62: works canónicos donde MB tenía 0 recordings y fallback acertaba con 0.58-0.61 ahora caen al banner. Mitigación in-place: subir el threshold se compensa con catalog en query (que añade puntos de title overlap) y con genre penalty (que filtra casos malos sin tocar los buenos). Si telemetría futura muestra >10% falso-negativo, revertimos a 0.58 y mantenemos genre penalty estricto. No hay forma de medir esto sin uso real.
- Para retomar trabajo en Phase 9: leer `docs/classical/phase-9-hub-ia.md` + esperar GO de Pedro. Phase 9.B9.1 introduce `WorkBucket` enum permanente; al hacerlo, `AlbumKindHint` + `infer_album_kind` + `buckets_compatible` se refactorizan.

---

### 2026-05-04 · Phase 8.9 + 9 + 10 — plans listos, ejecución pendiente · DESIGN-OK carta blanca

**State**: in_progress (memoria + docs gestionados; ejecución delegable autonomous).

**Last action**: tras una sesión larga que destapó síntomas serios en la build dev (clic en Beethoven Op. 83 → fallback work-level matchea Eroica con score 0.775 en query "Beethoven 3 Gesänge von Goethe"; 1 sola grabación cuando Tidal tiene decenas; "Cargar más" rota; movements colados al composer page; "todo lo veo bastante mal"), Pedro pidió plan de re-arquitectura referenciando Apple Music Classical e Idagio + USP "info sobre la obra como diferenciador". Lanzados `classical-musicologist` (audit comparativo Apple/Idagio + recomendación taxonomía 9+2 buckets D-039) y `classical-supervisor` (plan completo Phase 8.9 + 9 + 10). Pedro respondió "hazlo todo como tu veas, no me preguntes nada a mi, tu sabes maś que yo. Pero antes gestiona bien tu memoria y contexto y deja por escrito todo lo hecho y todo lo que hay por hacer".

**Next action**: gestionar memoria + docs (este checkpoint cierra el paso "antes" pedido por Pedro). Después: delegar ejecución Phase 8.9 → 9 → 10 al `classical-supervisor` con DESIGN-OK confirmado y validation gates internos sin volver a Pedro hasta cada cierre de phase.

**Files touched** (gestión memoria + docs):
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/MEMORY.md` (índice actualizado).
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/project_classical_status.md` (estado actual + planes futuros + cómo retomar).
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/project_classical_carta_blanca.md` (NEW, mandato Pedro 2026-05-04 + 4 paradas legítimas).
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/project_classical_blackscreen_bug.md` (DELETED, bug resuelto en sesión 2026-05-03).
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/project_classical_phase8_pending.md` (DELETED, obsoleto — phase 8 partial completada + phase 8.9/9/10 plan listo).
  - `docs/classical/DECISIONS.md` (D-040..D-048 registrados; D-041 SUPERSEDES D-037).
  - `docs/classical/PROGRESS.md` (header refrescado + filas Phase 8.9, 9, 10 añadidas + secciones detalladas con scope + decisiones).
  - `docs/classical/phase-8.9-emergency.md` (NEW, plan completo bugs A1-A5).
  - `docs/classical/phase-9-hub-ia.md` (NEW, plan completo B+C — ComposerPage tabs + 9 buckets + WorkPage redesign + USP infrastructure).
  - `docs/classical/phase-10-editorial-scaling.md` (NEW, plan completo D — hybrid 50 manual + 150 LLM-assisted + 1500 Wikipedia long tail + Phase 10.5 browse axes opcional).

**Tests**: n/a (no code change, sólo gestión).

**Build**: n/a.

**Notes**:
- **D-039** taxonomía 9+2 buckets registrada por classical-musicologist; cerrado.
- **D-040..D-048** registrados por mí en esta sesión, basados en plan del classical-supervisor. Pedro autorizó en bloque vía carta blanca.
- **D-037 SUPERSEDED por D-041**: refactor fallback work-level con top-N + threshold 0.62 + genre-aware penalty + catalog en query.
- **Decisiones cerradas**: D-001..D-048. Cualquier reaperture exige justificación nueva.
- **Audio path §10 inviolable** sigue. Cero diff en `audio.rs`, `hw_volume.rs`, `signal_path.rs`, `tidal_api.rs` esperado en cada phase futura.
- **Mensajería al delegar**: Pedro NO quiere ser interrumpido por preguntas; supervisor delegará a backend/frontend specialists con DESIGN-OK ya confirmado por mí (gestor del contexto). Las 4 paradas legítimas: (a) violación inminente de §10, (b) contradicción con D-001..D-048 cerrado, (c) test catastrófico irresoluble, (d) validation gate falla.
- **Estado dev server**: probable que siga corriendo (PID 54822 en sesión, no killed). Verificar al arrancar Phase 8.9 — si MB sigue inalcanzable por IPv6-only DNS, Phase 8.9 puede testearse contra mock. La realidad MB no afecta tests `cargo test --lib`.

---

### 2026-05-03 · Phase 8 · B8.7+B8.8+F8.5+F8.6 — bug 3 + bug 4 + UI badge + permanent ErrorBoundary completed

**State**: completed (cuatro fixes consolidados, pendiente commit del usuario).

**Last action**: ejecutado autonomously el bundle B8.7 (bug 3 — work-level Tidal text-search fallback) + B8.8 (bug 4 — error-swallow caching) + F8.5 (badge `TidalDirectInferred` + transient mensajería WorkPage) + F8.6 (ErrorBoundary permanente reemplazando DebugBoundary temporal de main.tsx). Carta blanca confirmada por Pedro vía briefing del agente principal.

**Diagnóstico que motivó**: cache-wipe + browse de works en plena Phase 8 destapó (a) MB devuelve recordings con campos críticos vacíos y serde rechazaba el missing field [bug 1, fijado previamente], (b) browse `?artist=` con `+releases` traía 4-5x payload sin valor [bug 2, fijado previamente], (c) cuando MB devuelve work sin recordings linkeadas el cascade per-recording no tenía donde correr [bug 3], y (d) `/tmp/sone-dev.log` mostró TODOS los prewarm canon failing con `unexpected EOF` por DNS-IPv6-only de MB en la red local de Pedro — destapando que `fetch_recordings_for_work` swallow-eaba el error y `get_work` cacheaba `tidal_unavailable=true` durante 7 días pollutando el cache disco [bug 4].

**Sub-tasks ejecutadas**:

- **B8.8 (bug 4) — variant `NetworkTransient` + clasificación de errores**:
  - `error.rs`: variant nuevo `SoneError::NetworkTransient(String)` con doc explicando policy. Métodos `is_network()` (ahora cubre ambos), `is_transient()` (matches NetworkTransient only), `from_http_status(status, msg)` (clasifica 429 + 5xx → transient, resto → permanent).
  - `From<reqwest::Error>`: clasifica `is_connect() | is_timeout() | is_request() | is_body() | is_decode()` → `NetworkTransient`. Para errores con status, usa `from_http_status`. Resto → `Network`.
  - `musicbrainz.rs::get_json`: 4 sites migrados — `send().await`, retry `send().await`, retry status check (via `from_http_status`), main status check (via `from_http_status`), `text().await`. Pattern consistente: `let inner: SoneError = e.into(); match inner { NetworkTransient → propagate as transient; Network → propagate as permanent; other → other }`.
  - `tidal.rs`: lookup_by_isrc + fetch_track_quality_meta migrados.
  - `wikipedia.rs::fetch_summary`: send + body migrados (best-effort path pero preserva mensajería transient).
  - `wikidata.rs::query`: idem.
  - `catalog.rs::get_work`: si `build_work_fresh` propaga error transient → return Err sin tocar cache. Errores permanentes propagan (comportamiento previo).
  - `catalog.rs::get_composer`: mismo guard.
  - `catalog.rs::build_work_fresh::fetch_recordings_for_work`: swallow → propagación condicional. Transient → propaga; permanent → `Vec::new()` (caso "MB legítimamente sin recordings", el negative-cache flag sigue siendo correcto).

- **B8.7 (bug 3) — work-level Tidal text-search fallback**:
  - `types.rs::MatchConfidence`: variant nuevo `TidalDirectInferred` con doc explicando D-037.
  - `matching.rs`: constante `WORK_LEVEL_THRESHOLD = 0.55` (justificación numérica en doc), helper `best_work_level_candidate(candidates, expected_title, query_used) -> MatchOutcome` que llama `score_candidate` con None/None/None y umbral 0.55.
  - `catalog.rs::build_work_fresh`: tras `resolve_recordings`, si `recordings.is_empty()` o todas `tidal_track_id is None`, llama nuevo método `try_work_level_fallback` que: build canonical query (composer, title, no artist, no year), search `tidal.search_canonical(query, 8)`, score con `best_work_level_candidate`, si confidence == TidalDirectInferred entonces sintetiza un `Recording` (mbid prefix `synthetic:tidal:{work_mbid}`, title=work.title, marca confidence + match_query + match_score) y lo appendea. `recording_count` actualizado.
  - Logs: `[catalog] work-level fallback for {mbid}: query='{q}'`, `HIT for {mbid}: track_id={?} score={?}` o `below threshold (score={?})`.
  - El flag `tidal_unavailable` se evalúa AL FINAL post-fallback, así que se setea correctamente a false cuando el fallback acierta.

- **F8.5 — badge `TidalDirectInferred` + tooltip + transient mensajería**:
  - `types/classical.ts`: `MatchConfidence` extendido con `TidalDirectInferred`. Nuevo type `SoneErrorKind` + helper `isTransientSoneError(err)` que detecta `kind === "NetworkTransient"` en el JSON serializado por Tauri.
  - `ConfidenceBadge.tsx`: branch nuevo para `TidalDirectInferred` — color orange (vs amber de TextSearchInferred), label "Tidal direct", tooltip "Tidal direct match (work-level fallback) — query: \"...\" (score X.XX)". Visualmente softer que `Inferred`.
  - `WorkPage.tsx`: state `errorIsTransient` + `reloadKey`. UI condicional: error transient → amber styling + copy específica ("Connection blip — couldn't reach MusicBrainz", "This is usually a transient network issue (DNS, TLS handshake, or upstream throttling). Nothing has been cached so a retry is cheap.") + Retry button. Error permanente → red styling + copy original. Retry incrementa `reloadKey`, `useEffect` deps incluyen `reloadKey` para re-fetch limpio.
  - `RecordingRow.tsx`: NO requirió cambio. La gate `matchConfidence !== "NotFound"` permite play; `TidalDirectInferred ≠ NotFound`, así que cae naturalmente al path playable.

- **F8.6 — ErrorBoundary permanente**:
  - `src/components/ErrorBoundary.tsx` (NEW, 113 lines): clase `Component` con state `{err, componentStack}`, `getDerivedStateFromError`, `componentDidCatch` (loggea a console.error + guarda componentStack), `reset()`, `copyDiagnostics()` (clipboard write con timestamp + stack + componentStack). UI usa theme tokens (`th-bg`, `th-surface/40`, `th-divider`, `th-accent`, `th-text-*`), no jarring red overlay. Botones "Try again" + "Copy diagnostics".
  - `App.tsx`: import `ErrorBoundary`. Wraps `<ToastProvider>` con `<ErrorBoundary>` en el return de `App()`. Sentado por encima de routing → captura crashes de cualquier page (Explore, Classical Hub, Stats, Settings, Player).
  - `main.tsx`: cleaned. DebugBoundary temporal removido + window.error / unhandledrejection listeners removidos + pushDebugError helper removido. Solo queda `ReactDOM.createRoot(...).render(<App />)`. Comentario explicativo apuntando a F8.6 como reemplazo.

**Decisiones nuevas**:
- D-037 — work-level Tidal text-search fallback. Variant `TidalDirectInferred` + threshold 0.55. (registrada en DECISIONS.md)
- D-038 — `SoneError::NetworkTransient` + classification + cache policy. (registrada en DECISIONS.md)

**Files touched**:

  Backend Rust:
  - `src-tauri/src/error.rs` (variant + 3 helpers + From classifier)
  - `src-tauri/src/classical/types.rs` (MatchConfidence::TidalDirectInferred)
  - `src-tauri/src/classical/matching.rs` (WORK_LEVEL_THRESHOLD + best_work_level_candidate + 7 nuevos tests)
  - `src-tauri/src/classical/catalog.rs` (get_work transient guard, get_composer transient guard, build_work_fresh propagation, try_work_level_fallback)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (4 sites migrados a clasificación)
  - `src-tauri/src/classical/providers/tidal.rs` (2 sites migrados)
  - `src-tauri/src/classical/providers/wikipedia.rs` (2 sites migrados)
  - `src-tauri/src/classical/providers/wikidata.rs` (2 sites migrados)

  Frontend:
  - `src/types/classical.ts` (MatchConfidence + SoneErrorKind + isTransientSoneError helper)
  - `src/components/classical/ConfidenceBadge.tsx` (branch TidalDirectInferred)
  - `src/components/classical/WorkPage.tsx` (transient state + UI condicional + Retry CTA)
  - `src/components/ErrorBoundary.tsx` (NEW, permanente)
  - `src/App.tsx` (wrapping con ErrorBoundary)
  - `src/main.tsx` (cleanup DebugBoundary temporal)

  Documentation:
  - `docs/classical/DECISIONS.md` (D-037 + D-038)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)
  - `docs/classical/PROGRESS.md` (status update)

**Tests**: 145/145 PASS (138 baseline post-Phase 7 + 7 nuevos en `classical::matching`):
- 4 tests work-level fallback: clean title yields TidalDirectInferred, movement-only falls below, empty candidates → NotFound, picks clean over movement.
- 3 tests error classification: 429+5xx → transient, 4xx non-429 → permanent, is_network covers both.

**Build**:
- `cargo check --lib`: clean.
- `cargo clippy --lib --no-deps`: **14 warnings** (idéntico baseline post-Phase 7). 0 nuevas.
- `cargo test --lib`: 145/145.
- `tsc --noEmit`: 0 errores.
- `npm run build` (vite): clean, 1876 módulos (1875 baseline + 1 ErrorBoundary.tsx).

**Notes / regresión §10**:
- `git diff src-tauri/src/audio.rs` → vacío.
- `git diff src-tauri/src/hw_volume.rs` → vacío.
- `git diff src-tauri/src/signal_path.rs` → vacío.
- `git diff src-tauri/src/tidal_api.rs` → vacío.
- `route_volume_change` (lib.rs:491-539) intacto.
- Writer guard (`audio.rs:988-992`) intacto: `if !bit_perfect { apply_volume(...) }` preserved.
- `SoneError` cambio es aditivo (variant nuevo) — todos los consumers existentes (match exhaustivo en Tauri serde) siguen funcionando porque `#[serde(tag, content)]` produce JSON discriminado. Frontend recibe `{kind: "NetworkTransient", message: "..."}` del nuevo path.
- Bug 1 + Bug 2 ya estaban fijados antes de esta sesión por agente principal (parte del briefing).
- Dev server PID 54822 corriendo; el binario backend necesita rebuild para que los cambios `NetworkTransient` lleguen a runtime. HMR de Vite cogerá frontend automáticamente.
- `/tmp/sone-dev.log` post-cambios: el siguiente prewarm canon ejecutado por el binario rebuilt verá los errores de DNS-IPv6 clasificados como `NetworkTransient` y NO se cachearán como `tidal_unavailable=true`. El cache disco wipeado no se re-poisonea.
- Pedro reabrirá la WorkPage para validar manualmente: (a) badge "Tidal direct" aparece en obras donde MB no tenía recordings linkeadas pero Tidal sí tiene canónicas; (b) cuando MB esté inalcanzable, la WorkPage muestra el banner amber con copy "Connection blip" + Retry CTA en lugar de cachear vacío.

**Next action (post-session)**: Pedro confirma comportamiento visual + funcional con dev server reiniciado. Si aprueba, sigue Phase 8 con el resto del plan (B8.1 search streaming sigue siendo prioridad #1 del usuario, todavía pendiente).

---

### 2026-05-02 · Phase 8 · phase-8-init — plan loaded

**State**: in_progress.

**Last action**: Phase 8 arrancada autonomously per mandato del usuario "carta blanca para todo lo restante" + prioridad search incremental ("X max pero que vayan saliendo segun las vayas encontrando con un loading al final"). Plan completo en `phase-8-polish.md`. Sub-tasks B8.1 (search streaming backend), F8.1 (search streaming frontend), B8.2 (audit estados loading/empty/error), B8.3 (Re-check Tidal feedback), F8.2 (microinteracciones), B8.4 (docs operador), B8.5 (D-034 re-evaluation), B8.6 (regression smoke).

**Next action**: delegar B8.1 a `sone-backend-engineer` — refactor `search_classical` a path streaming via Tauri events. Nuevo command `search_classical_streaming(query, queryId, limit)`, emite `classical:search-plan`, `classical:search-hit`, `classical:search-done`. Mantener `search_classical` síncrono intacto (no breaking, tests Phase 5 preservados). Tests nuevos: helper interna `enumerate_search_hits<F>` testeable en isolación con mock emit_fn into Vec.

**Files touched**:
  - `docs/classical/phase-8-polish.md` (NEW — plan completo)
  - `docs/classical/PROGRESS.md` (Phase 8 row + status update)
  - `docs/classical/CHECKPOINTS.md` (this entry)

**Tests**: n/a (no code change yet).
**Build**: n/a.

**Notes**:
- Decisiones previstas: D-035 (events sobre Channel), D-036 (queryId-based dedup frontend).
- Audio path §10 inviolable. Verificar empty diff al cierre Phase 8.
- Composer-resolution stats (D-034) re-evaluado en B8.5: queda deferred V1+ (esfuerzo > budget de polish phase).
- MB Lucene fallback NO se introduce en Phase 8 (deuda V1.1+).

---

### 2026-05-02 · Phase 7 · phase-7-final — completed autonomously

**State**: completed. Phase 7 cerrado con B7.0-B7.5 + F7.0-F7.3 todos verdes; B7.6/F7.4 diferidos como deuda V1 explícita per G3.

**Last action**: cierre Phase 7 con todas las acceptance criteria del §7 del plan satisfechas. Tests 138/138 (118 baseline + 20 nuevos). Audio path 100% intacto (`git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty). Binario release 39.6 MB (delta +2.3 MB justificado por snapshot extended embedded). Apéndice A en `phase-7-catalog-completeness.md` rellenado con números reales del harvest.

**Resumen ejecutivo**:
- **Composers ampliados**: 33 (OpenOpus only) → **6082** (extended snapshot).
- **Bug Tchaikovsky verificado**: Movement filter (D-028) descarta child works `direction=backward` en `parts` rels. 5 tests acceptance cubren parent kept / child filtered / unrelated rels / no-rels.
- **Paginación funcional**: ComposerPage muestra "Load more" cuando works > 100 con counter "X of ~Y works loaded". Cache key bumped v2.
- **Tidal availability banner**: WorkPage muestra banner cuando `tidalUnavailable=true` con CTA Re-check.
- **Search extended**: tokenizer ahora reconoce Saariaho, Caroline Shaw, Hildegard, etc. fuera del canon-33 OpenOpus.
- **Hub footer**: "Catalog: 6082 composers indexed" + Browse all CTA.
- **Tamaño JSON final**: 2.3 MB (cap G4 ≤5 MB satisfied).
- **0 regresiones §10**: todos los archivos del audio path empty diff.

**Estrategy pivot importante**: B7.0 originalmente requería "recording_count ≥ 5" enforced via MB browse calls. A 30K composers × 1.05s/req = ~8.7h wall-clock infeasible. Pivoteé a filtro semántico Wikidata `wdt:P136 ?genre AND ?genre wdt:P279* wd:Q9730` (genre subclass de "classical music") + UNION para géneros adyacentes (minimalism, contemporary classical, opera, gregorian chant, etc.) + defensive OO fallback. Resultado: harvest en ~50s, 6082 composers semánticamente vetados como classical, todos los canónicos OpenOpus preservados via merge defensivo. **Documentado en D-027 + Apéndice A**.

**B7.6/F7.4 deferred (G3)**: composer-resolution backfill stats. Razón: el mandato del usuario "no perder nada de lo que pueda escuchar" está cubierto por B7.0-B7.5 + F7.0-F7.3. B7.6 es refinement UI (top composers in stats refleja performer no composer real); D-025 caveat sigue válido como limitación V1. **Deuda V1 documentada en D-034-status + PROGRESS.md Phase 7 closure section**.

**Next action (post-session)**: el usuario commitea todos los cambios. Si el dev server corre (PID 601478), debe reiniciarse para que el binario rebuild con el snapshot extended embebido.

**Files touched (Phase 7)**:
  Backend Rust:
  - `src-tauri/src/classical/providers/composers_extended.rs` (NEW, 332 lines, 11 tests)
  - `src-tauri/src/classical/providers/mod.rs` (+1 line: `pub mod composers_extended`)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (browse_works_by_artist: signature change limit+offset; `inc=work-rels`; movement filter; new `MbBrowsedWorksPage` struct; new `work_is_child_movement` helper; 5 tests)
  - `src-tauri/src/classical/types.rs` (+1 field: `Work.tidal_unavailable`)
  - `src-tauri/src/classical/catalog.rs` (Arc field + constructor param; list_top_composers extended/openopus split; list_composers_by_era → extended; new `extended_composers_total`; list_works_by_composer → ComposerWorksPage with offset; build_work_fresh tidal_unavailable detection; new `ComposerWorksPage` struct; cache key v1→v2; search_classical uses extended index; prewarm canon updated for new signature)
  - `src-tauri/src/classical/mod.rs` (build_catalog_service wires composers_extended Arc)
  - `src-tauri/src/classical/search.rs` (+4 Phase 7 tests)
  - `src-tauri/src/commands/classical.rs` (list_classical_works_by_composer: +offset param + ComposerWorksPage return; new `recheck_classical_work_tidal` command; new `get_classical_extended_total` command)
  - `src-tauri/src/lib.rs` (invoke_handler: +2 commands)

  Backend data:
  - `src-tauri/data/composers-extended.json` (NEW, 2.3 MB, 6082 composers)

  Frontend:
  - `src/types/classical.ts` (+ComposerWorksPage interface; +Work.tidalUnavailable)
  - `src/api/classical.ts` (listClassicalWorksByComposer: +offset + ComposerWorksPage; +getClassicalExtendedTotal; +recheckClassicalWorkTidal)
  - `src/components/classical/BrowseComposers.tsx` (fetch 5000 instead of 100; client-side render cap with Load more; counter "X of Y indexed")
  - `src/components/classical/ComposerPage.tsx` (paginated state; loadMoreWorks handler; "Full catalog" expandable section with Load more button)
  - `src/components/classical/WorkPage.tsx` (Tidal-unavailable banner + Re-check handler)
  - `src/components/classical/ClassicalHubPage.tsx` (extendedTotal state + footer chip)

  Tooling:
  - `docs/classical/scripts/snapshot_composers_extended.py` (NEW, 530 lines)

  Documentation:
  - `docs/classical/DECISIONS.md` (D-027..D-034 + D-034-status)
  - `docs/classical/PROGRESS.md` (Phase 7 → completed, full closure section)
  - `docs/classical/CHECKPOINTS.md` (this checkpoint)
  - `docs/classical/phase-7-catalog-completeness.md` (Apéndice A real numbers)

**Tests**: 138/138 PASS (118 baseline + 11 composers_extended + 5 movement filter + 4 search extended). 0 fallos.
**Build**:
  - `cargo check --release` clean.
  - `cargo build --release` clean (1m 5s).
  - `cargo clippy --release --lib --no-deps`: 14 warnings (idéntico baseline post-Phase 6, 0 nuevas en classical).
  - `cargo test --release --lib`: 138/138.
  - `tsc --noEmit`: 0 errores.
  - `npm run build`: clean.

**Notes**:
- `git diff src-tauri/src/audio.rs src-tauri/src/hw_volume.rs src-tauri/src/signal_path.rs src-tauri/src/tidal_api.rs` → empty. Bit-perfect contract preservado.
- `route_volume_change` (lib.rs) y writer guard (audio.rs:988-992) intactos.
- Phase 7 NO cambia el `lib.rs::setup` hook ni añade prewarm extendido (G7 confirmado: NO se extiende pre-warm en Phase 7).
- El snapshot extended se construye determinísticamente (sort lexicográfico por MBID) — re-runs producen bytes idénticos modulo Wikidata edits entre runs.
- 2.3 MB embebido vía `include_bytes!` aumenta el lib `~rlib` en ~2.5 MB; el binario release final crece 39.6 MB - ~37.3 MB pre-Phase 7 = +2.3 MB delta exactamente. Cap G4 (≤5 MB JSON) satisfied; cap implícito de "binario no crece > 8 MB" satisfied.
- Anna Thorvaldsdóttir no surface en SPARQL (su Wikidata sin classical-genre claims) — pérdida menor; runtime cascade puede resolverla on-click si MB tiene su artista.

---

### 2026-05-02 · Phase 7 · B7.0-completed — snapshot extended ready

**State**: completed (sub-task B7.0).

**Last action**: harvest del snapshot extended ejecutado con éxito. Script Python `docs/classical/scripts/snapshot_composers_extended.py` creado. Output: `src-tauri/data/composers-extended.json` con 6082 composers, 2.3 MB (dentro del cap G4=5 MB). Wall-clock: ~50s (SPARQL 12s + portrait fetch 30s + merge/write 8s).

**Cambio de plan vs D-027 original**: el threshold `recording_count >= 5` enforced via MB calls era infeasible (~8h wall-clock para 30k composers). Pivoté a filtro semántico en SPARQL — `wdt:P136 ?genre AND ?genre (wdt:P279*) wd:Q9730` (genre subclass de "classical music"), + UNION branch para géneros adyacentes que no tienen closure (minimalism, contemporary classical, etc.). Este es un proxy de notabilidad **más fuerte** que recording_count: un composer documentado en Wikidata como autor de música clásica tiene ya curación humana. El runtime Phase 1 cascade sigue verificando audibilidad por work.

**Defensive merge**: composers OpenOpus que no surfacearon en SPARQL (Brahms, Schumann, Chopin, Reich, etc., cuyas entries Wikidata no tienen claim de genre classical explícita) se splice automáticamente vía OO fallback path en `merge()`. Garantiza que ningún composer canónico se pierda.

**Apéndice A** actualizado en `phase-7-catalog-completeness.md` con números reales.

**Next action**: B7.1 — implementar `ExtendedComposersProvider` en `src-tauri/src/classical/providers/composers_extended.rs`. Schema del JSON: `{schema_version, generated_at, harvest_threshold_recording_count, composers: [{mbid, qid, name, full_name, birth_year, death_year, epoch, portrait_url, recording_count, popular, open_opus_id}]}`.

**Files touched**:
  - `docs/classical/scripts/snapshot_composers_extended.py` (NEW, 530 lines)
  - `src-tauri/data/composers-extended.json` (NEW, 6082 composers, 2.3 MB)
  - `docs/classical/phase-7-catalog-completeness.md` (Apéndice A rellenado)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: n/a (script side, manual verification done).
**Build**: n/a (no Rust code change yet — snapshot is data only).

**Notes**:
- Cero archivo del audio path tocado. `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` sigue empty.
- Anna Thorvaldsdóttir no surface (qid Q4747856 sin genre claims clásicas adecuadas en Wikidata). Se acepta como pérdida menor; el runtime cascade puede resolverla on-click si MB tiene su artista.
- Hildegard surface vía Q3135615 (diferente al Q41587 que yo había guessed); verificación manual queda al QA.
- 26 de 33 OpenOpus composers surface vía SPARQL natural; 7 vía defensive OO merge.
- Portraits: 3762/6082 (62%) con foto Wikidata. Resto fallback a avatar placeholder.

---

### 2026-05-02 · Phase 7 · decisions-locked-in (autorizado por usuario)

**State**: in_progress (autonomous execution).

**Last action**: el usuario aprobó los 8 defaults del decision-gate Phase 7 con un "vamos a ello". G1=N≥5, G2=dual snapshot, G3=B7.6 condicional según budget, G4=≤5 MB JSON, G5=Python, G6=openopus.json preservado, G7=NO pre-warm extendido, G8=consultar musicologist para borderline. D-027..D-034 + D-034-status registrados en DECISIONS.md. PROGRESS.md actualizado a 🟡 in_progress.

**Next action**: comenzar B7.0 — diseño de la query SPARQL Wikidata + script Python `docs/classical/scripts/snapshot_composers_extended.py`. Antes de delegar a `sone-backend-engineer`, consultar `classical-musicologist` (G8) para validar la lista de composers borderline esperados (Hildegard, Pärt, Caroline Shaw, Saariaho, Anna Thorvaldsdóttir, etc.) y ajustar la query SPARQL si es necesario.

**Files touched**:
  - `docs/classical/DECISIONS.md` (D-027..D-034 + D-034-status añadidos al final, antes de la plantilla)
  - `docs/classical/PROGRESS.md` (header Phase 7 → 🟡 in_progress; tabla phases actualizada; checkpoint actual = phase-7-decisions-locked-in)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: n/a (no code changes yet).
**Build**: n/a (no code changes yet).

**Notes**:
- Cero archivo del audio path (§10) tocado todavía. Verificación pre-implementación: `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` debe quedar vacío al cierre de Phase 7.
- Baseline tests pre-Phase 7: 118/118 PASS. Deben seguir verdes; target ≥ 130/130 al cierre.
- Riesgos identificados (§8 plan): WDQS endpoint disponibilidad, tamaño binario inflado, MB rate-limit con paginación, snapshot drift entre runs (mitigado por sort lexicográfico).
- Dev server PID 601478 corriendo — bake del snapshot puede requerir reinicio del dev server al final.

---

### 2026-05-02 · Phase 7 · plan-emitted — pending human review

**State**: in_progress (planning, NOT autonomous).

**Last action**: el usuario reabrió el scope V1 tras cerrar Phase 6. Mandato textual: "quiero tener todos los compositores y todas las obras disponibles en el catálogo de tidal, hazlo como quieras, pero no quiero perder nada de lo que pueda escuchar, no tiene sentido". El Hub actual se siente "muy incompleto":
- 33 compositores en el snapshot OpenOpus (BrowseComposers cap 100 trunca a 33 porque el snapshot no tiene más).
- ComposerPage de Tchaikovsky muestra "III. Adagio lamentoso" como entrada top-level en lugar de la Pathétique parent — bug confirmado en `src-tauri/src/classical/providers/musicbrainz.rs:449` (browse `?artist=` sin `inc=work-rels`, sin filtro parent-only).
- ComposerPage cap 100 obras sin paginación: Bach (>1000) y Mozart (>600) se truncan silenciosamente.
- Search tokenizer Phase 5 indexa solo los 33 composers OpenOpus — composers fuera del top-33 no se reconocen.

El supervisor produjo `docs/classical/phase-7-catalog-completeness.md`: plan completo con sub-tasks B7.0..B7.6 + F7.0..F7.4, D-027 a D-034 propuestas, decision-gate G1-G8 obligatorio, criterios de aceptación, riesgos, scope-out explícito.

**Decisiones clave del plan** (todas pending aprobación humana):
- D-027 — Universo de compositores: harvest Wikidata SPARQL (`P106 wd:Q36834`) filtrado por MB recording_count ≥ N (default propuesto N=5). Target ~600-1500 composers, no 30K. Reportado por B7.0.
- D-028 — Movement filter en `browse_works_by_artist`: añadir `inc=work-rels` y filtrar child works (cierra bug Tchaikovsky).
- D-029 — Paginación works con offset.
- D-030 — Tidal availability: NO pre-screen. On-click cascade Phase 1 + cache negativo 7d.
- D-031 — Search tokenizer: ampliar índice con snapshot extended.
- D-032 — Snapshot regeneration script reproducible en `docs/classical/scripts/`.
- D-033 — Dual-snapshot: OpenOpus original (33, curado) + extended (600-1500). Preserva curación.
- D-034 (opcional) — Composer-resolution para stats (cierra deuda D-025).

**Modo NO autónomo**: el supervisor NO inicia implementación. El plan está pendiente de:
1. Revisión humana del documento `phase-7-catalog-completeness.md`.
2. Respuesta a G1-G8 (threshold N, dual-snapshot sí/no, B7.6 incluido sí/no, lenguaje del script, etc.).
3. Consulta opcional con `classical-musicologist` para repertorio borderline.
4. Autorización explícita para que el supervisor delegue al `sone-backend-engineer` la ejecución de B7.0.

**Next action** (al retomar):
- Si el usuario aprueba: el supervisor abre B7.0 (script harvest) delegando al backend-engineer + consulta musicologist para G8. Tras B7.0 closed con reporte Apéndice A, segundo gate (opcional) antes de B7.1+.
- Si el usuario rechaza o pide revisión: ajuste del plan en otra iteración. Cero código tocado.

**Files touched** (este checkpoint + sesión planning):
- `docs/classical/phase-7-catalog-completeness.md` (NEW, plan completo).
- `docs/classical/PROGRESS.md` (Phase 7 → 📝 plan pending review; sección detallada Phase 7 añadida; "PROYECTO COMPLETO" reframed como "V1 entregado pre-Phase 7"; cabecera actualizada).
- `docs/classical/CHECKPOINTS.md` (este checkpoint).

**Tests**: n/a (no código tocado).
**Build**: n/a (no código tocado).

**Notes**:
- Bit-perfect contract: cero riesgo, cero archivo de §10 audio path tocado en este checkpoint.
- `DECISIONS.md` NO se actualiza todavía: D-027..D-034 son **propuestas en el plan**, no decisiones tomadas. Solo se commitean a DECISIONS.md tras la aprobación humana.
- El plan respeta:
  - D-005 (bit-perfect inviolable) — cero modificación a `audio.rs` / `hw_volume.rs` / `signal_path.rs` / `tidal_api.rs` planificada.
  - D-008 (todas las phases V1) — Phase 7 entra como cierre legítimo del V1, no V2.
  - §10 cero regresión — todos los cambios planificados son aditivos sobre archivos existentes (extensión de provider, nuevo provider, nueva columna stats opcional con migration idempotent).
  - §3.3 cache TTLs — cada cache nuevo justifica TTL en el plan.
  - §5.2 provider pattern — `ExtendedComposersProvider` (nuevo o ampliación de OpenOpusProvider) sigue el patrón.
- Limitación honesta documentada en el plan §9: Phase 7 NO aborda Mobile (D-003), NO reescribe el provider+catalog pattern, NO abre Phase 8.

---

### 2026-05-02 · Phase 6 · COMPLETED — phase-6-final · PROYECTO COMPLETO

**State**: completed (autonomous). **Esta es la fase final — no hay V2.**

**Last action**: Phase 6 cerrada. Personal listening integration + Wikidata SPARQL + browse-by-conductor + favorites CRUD + pre-warm canon shippeados completos. WikidataProvider con rate-limit conservador (1.5s/query) + cache StaticMeta 30d (D-023). Validation defense-in-depth para favorites kind (D-024). Top classical composers = top performers asociados a obras clásicas (limitación documentada V1, D-025). Pre-warm 12s post-boot, 30 composers serial (D-026). 13 nuevos commands Tauri. 14 nuevos tests classical+stats (7 wikidata + 7 stats). Audio path verificado intacto via `git diff` empty.

**Decisión final**: ✅ Phase 6 → 🟢 completed. **PROYECTO COMPLETO**. Acceptance §11 cumplida en todo lo automatizable. Tests acceptance: `top_classical_works_groups_by_work_mbid`, `classical_overview_counts_only_classical`, `classical_discovery_curve_filters_to_classical_only`, `favorites_round_trip_idempotent`, `classical_recording_comparison_buckets_per_recording`, `top_classical_composers_groups_by_artist_mbid_and_skips_unknown`, `classical_recently_played_groups_by_work_and_orders_by_recency`. Wikidata gates: `parse_enrichment_row_extracts_fields`, `parse_related_row_extracts_full_record`, `extract_qid_handles_various_shapes`, `is_valid_qid_rejects_garbage`, `pick_value_picks_string_value`, `enrich_with_invalid_qid_returns_default`, `related_with_invalid_qid_returns_empty`.

**Datos clave**:
- Backend:
  * 1 nuevo provider `classical/providers/wikidata.rs` (~430 LOC con tests, 7 tests).
  * `MusicBrainzProvider` extendido: `fetch_composer` con `inc=url-rels` + parser `parse_wikidata_qid`; nuevo método `browse_recordings_by_artist`; nuevo struct `MbArtistRecording`.
  * `Composer` extendido con `related_composers: Vec<RelatedComposer>` (aditivo en `types.rs`).
  * `RelatedComposer` struct nuevo: qid + mbid + name + shared_genres + birth_year + portrait_url.
  * `CatalogService` extendido con `wikidata: Arc<WikidataProvider>` + métodos: `enrich_composer_with_wikidata` (cache-then-fetch), `list_related_composers`, `top_classical_works`, `top_classical_composers`, `classical_recently_played_works`, `classical_recording_comparison`, `classical_overview`, `classical_discovery_curve`, `add/remove/is/list_classical_favorite(s)`, `artist_discography`, `prewarm_canon`.
  * `ArtistDiscography + DiscographyEntry + DiscographyGroup` shapes públicos.
  * `is_valid_favorite_kind` validator (D-024).
  * `stats.rs` extensión aditiva: 6 nuevos tipos (`TopClassicalWork`, `TopClassicalComposer`, `RecentClassicalSession`, `RecordingComparisonRow`, `ClassicalOverview`, `ClassicalFavorite`) + 6 query methods (top_classical_works, top_classical_composers, classical_recently_played_works, classical_recording_comparison, classical_overview, classical_discovery_curve) + 4 favorites CRUD methods (add/remove/is/list). Plays table + classical_favorites + classical_editorial NO tocados (reuso schema Phase 1+5).
  * 13 nuevos commands Tauri en `commands/classical.rs`, registrados en `lib.rs`.
  * `lib.rs` setup hook: prewarm spawn 12s post-boot.
- Frontend:
  * 1 nuevo componente `FavoriteToggle.tsx` (heart icon reusable, ~100 LOC).
  * 1 nuevo componente `ClassicalLibrary.tsx` (~280 LOC — Library tab del Hub).
  * 1 nuevo componente `ClassicalArtistPage.tsx` (~210 LOC — browse-by-conductor).
  * 1 nuevo componente `ClassicalRecordingComparison.tsx` (~220 LOC — versions side-by-side).
  * `ClassicalHubPage.tsx` extendido: Library tab activado (ya no placeholder), nuevas secciones Listen Now "Recently played" + "Your top works" (gated en data presence — degradación graciosa cuando no hay data), "Coming soon" eliminado.
  * `WorkPage.tsx` extendido: FavoriteToggle (kind="work") + "X versions you've played" link (gated en `comparison.length > 1`).
  * `ComposerPage.tsx` extendido: FavoriteToggle (kind="composer") + sección "Related composers" + sub-component `RelatedComposersSection`.
  * `RecordingRow.tsx` extendido: nuevo sub-component `ArtistLinks` que renderiza conductor/orchestra como buttons clickables cuando MBID disponible. Fallback a flat text idéntico a Phase 5 cuando no hay mbid (cero regresión).
  * `StatsPage.tsx` extendido: nueva tab "Classical" con sub-component `ClassicalTab` — overview banner + Top works + Top composers + Discovery section.
  * Types Phase 6 mirror exacto: 11 nuevos types/interfaces.
  * API wrappers Phase 6: 13 nuevos en `src/api/classical.ts`.
  * 3 navegadores nuevos: `navigateToClassicalArtist`, `navigateToClassicalCompare`, `navigateToClassicalLibrary`.
  * 4 routing branches aditivos en `App.tsx`: `classical://library`, `classical://library/{facet}`, `classical://artist/{mbid}`, `classical://compare/{mbid}`.
- Cero regresión §10:
  * `audio.rs`: `git diff` vacío.
  * `hw_volume.rs`: `git diff` vacío.
  * `signal_path.rs`: `git diff` vacío.
  * `tidal_api.rs`: `git diff` vacío.
  * `route_volume_change` (`lib.rs:491-539`): intacto.
  * Writer guard (`audio.rs:988-992`): intacto.
  * `lib.rs` Phase 6 delta: 13 nuevas líneas en invoke_handler + ~12 líneas en setup hook (prewarm spawn). Sin cambios a routing, settings handler, audio, scrobble core.
  * `stats.rs` Phase 6 delta: 6 shapes + 10 methods. Plays table + classical_favorites + classical_editorial schema NO tocado. Indexes existentes intactos.
- D-023, D-024, D-025, D-026 registrados en DECISIONS.md.

**Next action**: ninguna. Esta es la última phase. Operador hace QA manual (Hub → Listen Now: ver Top works si tiene plays clásicos, Recently played si tiene sesiones recientes; Hub → Library: ver overview banner + facets; abrir un Composer canon (Beethoven Q255 / Mozart): ver Related composers section; clic conductor name en RecordingRow → navega a ClassicalArtistPage; abrir StatsPage → tab Classical: ver Top works + Top composers + Discovery). Si todo OK, el usuario commitea todos los cambios consolidados.

**Files touched (sesión Phase 6 completa)**:

  Backend:
  - `src-tauri/src/classical/providers/wikidata.rs` (NEW, 7 tests)
  - `src-tauri/src/classical/providers/mod.rs` (1 línea: pub mod wikidata)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (`browse_recordings_by_artist` + `MbArtistRecording` + `parse_wikidata_qid` + `inc=url-rels` en fetch_composer)
  - `src-tauri/src/classical/types.rs` (`Composer.related_composers` + `RelatedComposer` struct)
  - `src-tauri/src/classical/mod.rs` (export RelatedComposer + factory wires WikidataProvider)
  - `src-tauri/src/classical/catalog.rs` (wikidata field + 11 nuevos métodos públicos + `ArtistDiscography`/`DiscographyEntry`/`DiscographyGroup` + `is_valid_favorite_kind` + `enrich_composer_with_wikidata` + `prewarm_canon` + 4 nuevos cache prefixes/tags)
  - `src-tauri/src/commands/classical.rs` (13 nuevos commands + 5 imports)
  - `src-tauri/src/stats.rs` (6 nuevos shapes + 10 nuevos métodos + classical_tests module con 7 tests)
  - `src-tauri/src/lib.rs` (13 invoke_handler entries + prewarm spawn 12s post-boot)

  Frontend:
  - `src/components/classical/FavoriteToggle.tsx` (NEW)
  - `src/components/classical/ClassicalLibrary.tsx` (NEW)
  - `src/components/classical/ClassicalArtistPage.tsx` (NEW)
  - `src/components/classical/ClassicalRecordingComparison.tsx` (NEW)
  - `src/components/classical/ClassicalHubPage.tsx` (Library tab + Top works + Recently played sections, "Coming soon" removed)
  - `src/components/classical/WorkPage.tsx` (FavoriteToggle + comparison link)
  - `src/components/classical/ComposerPage.tsx` (FavoriteToggle + RelatedComposersSection)
  - `src/components/classical/RecordingRow.tsx` (ArtistLinks sub-component)
  - `src/components/StatsPage.tsx` (Classical tab + ClassicalTab component)
  - `src/types/classical.ts` (RelatedComposer + 10 Phase 6 types)
  - `src/api/classical.ts` (13 nuevos wrappers + 2 type imports from stats)
  - `src/hooks/useNavigation.ts` (navigateToClassicalArtist + navigateToClassicalCompare + navigateToClassicalLibrary)
  - `src/App.tsx` (3 imports + 4 routing branches)

  Docs:
  - `docs/classical/PROGRESS.md` (Phase 6 → completed; entregables; acceptance checklist; PROYECTO COMPLETO summary; D-023..D-026)
  - `docs/classical/DECISIONS.md` (D-023, D-024, D-025, D-026)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint final)
  - `docs/classical/README.md` (NEW — TL;DR del Hub completo)

**Tests**: 118/118 unit tests del módulo classical+stats pasan (104 previos + 7 nuevos en `classical::providers::wikidata` + 7 nuevos en `stats::classical_tests`). Tests existentes inalterados.

**Build**:
- `cargo check --release` ✅
- `cargo build --release` ✅ (53s, binario producido)
- `cargo clippy --release --lib --no-deps` ✅ 14 warnings (idéntico baseline post-Phase 5). 0 nuevas en classical/stats.
- `cargo test --release --lib` ✅ 118/118
- `tsc --noEmit` ✅ 0 errores
- `npm run build` (vite) ✅ clean, 1875 módulos

**Notes**:
- Bit-perfect contract intacto: cero archivos del audio path modificados. Verificación explícita: `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → vacío.
- §10 zero-regression: cada modificación a archivo existente es estrictamente aditiva. ArtistLinks renderiza idéntico al text Phase 5 cuando MBID no está presente; FavoriteToggle es opt-in (sólo se renderiza donde lo añadimos); StatsPage Classical tab es nuevo sub-tab que no toca los otros.
- Code-style §1: braces siempre. Todos los condicionales del nuevo código usan llaves. Arrow functions de una sola expresión sin bloque preservadas.
- D-023 — WikidataProvider 1.5s/query es conservador respecto a la WDQS policy (5 concurrent permitidos), pero refleja "ser buen vecino" sobre un servicio público gratuito.
- D-024 — `is_valid_favorite_kind` es defense-in-depth. La frontend types ya restringen, pero los Tauri commands cruzan boundary sin garantías.
- D-025 — top_classical_composers devuelve "top performers/conductors asociados a obras clásicas", NO composers reales. Para resolver al composer correcto necesitaríamos backfill `composer_mbid` en plays. Diferido a Phase 7+ (no V1). Documentado honestamente en el código y en este checkpoint.
- D-026 — pre-warm 12s post-boot. La task corre serial (~120s wall-clock), drop-graceful sin handle. Si el usuario abre el Hub antes de 12s, no ha empezado todavía y los queries son cold (mismo Phase 5 behavior).
- Limitación conocida: WDQS depende de internet. Si el endpoint cae, related composers están vacíos (graceful degradation). No hay fallback offline en V1.
- Limitación conocida: la sección "Recently played" del Hub Listen Now muestra sólo plays con `work_mbid` resuelto. Si el WorkMbidResolver falla en una sesión (best-effort), esos plays no aparecen aquí pero sí en la stats general.
- Limitación conocida: Library facets (recording / performer) tienen flujo de "save" no triggers en V1 porque RecordingRow y ArtistLinks no muestran heart icon. Eso es Phase 7 (V2). El sistema soporta ambos kinds en stats DB; solo falta UI.

---

### 2026-05-02 · Phase 5 · COMPLETED — phase-5-final

**State**: completed (autonomous).

**Last action**: Phase 5 cerrada. Editorial layer + Advanced Search shippeados completos. Tokenizer determinístico (D-019) + 48 editorial seeds curados (D-020) + override manual via stats DB (D-021) + listening guides reader (read-only LRC). Wikidata SPARQL + related composers + browse-by-conductor diferidos a Phase 6 (D-022). 5 nuevos commands Tauri. 39 nuevos tests classical (24 search + 9 editorial + 6 listening_guide). Audio path verificado intacto via `git diff` empty.

**Decisión final**: ✅ Phase 5 → 🟢 completed. Acceptance §11 cumplida en todo lo automatizable. Test rust acceptance `phase5_acceptance_op_125_resolves_to_beethoven_9` y `phase5_acceptance_beethoven_9_karajan_1962_resolves_top_match` validan el gate principal del doc.

**Datos clave**:
- Backend:
  * 1 nuevo módulo `classical/search.rs` (~750 LOC con tests, 24 tests).
  * 1 nuevo módulo `classical/editorial.rs` (~280 LOC con tests, 9 tests).
  * 1 nuevo módulo `classical/listening_guide.rs` (~165 LOC con tests, 6 tests).
  * 1 nuevo data file `data/editorial.json` (~13 KB, 48 work seeds + 15 composer notes).
  * `CatalogService` extendido con `editorial: Arc<EditorialProvider>` + `stats: Arc<StatsDb>`, métodos `apply_editorial`, `search_classical`, `list_editorial_picks`, `set_user_editors_choice`, `clear_user_editors_choice`.
  * `MusicBrainzProvider` ajustado: `editor_note: None` añadido a Composer + Work literales.
  * `stats.rs` extensión aditiva: tabla `classical_editorial` + 3 métodos + struct `EditorialOverride`.
  * 5 nuevos commands Tauri en `commands/classical.rs`, registrados en `lib.rs`.
- Frontend:
  * 1 nuevo componente `ClassicalSearch.tsx` (~290 LOC).
  * 4 componentes existentes extendidos aditivamente: `RecordingRow.tsx`, `WorkPage.tsx`, `ComposerPage.tsx`, `ClassicalHubPage.tsx`.
  * 1 nuevo navigator `navigateToClassicalSearch` en `useNavigation`.
  * 1 nuevo routing branch `classical://search` en `App.tsx`.
  * Types Phase 5: `SearchToken`, `SearchPlan`, `SearchHit`, `SearchResults`, `EditorsChoice`, `EditorialPick`, `LrcLine`, `LrcGuide`. Recording / Work / Composer extendidos con `isEditorsChoice` / `editorNote`.
  * 5 nuevos API wrappers: `searchClassical`, `listClassicalEditorialPicks`, `setClassicalEditorsChoice`, `clearClassicalEditorsChoice`, `readClassicalListeningGuide`.
- Cero regresión §10:
  * `audio.rs`: `git diff` vacío.
  * `hw_volume.rs`: `git diff` vacío.
  * `signal_path.rs`: `git diff` vacío.
  * `tidal_api.rs`: `git diff` vacío.
  * `route_volume_change` (`lib.rs:491-539`): intacto.
  * Writer guard (`audio.rs:988-992`): intacto.
  * Único delta a `lib.rs` Phase 5: 5 entries en invoke_handler + 1 línea passing `Arc::clone(&stats)` al `build_catalog_service`. Sin cambios a routing, settings handler, audio, scrobble core.
  * `stats.rs` Phase 5 delta: 1 tabla nueva (idempotent migration) + 3 métodos + 1 struct. Plays table + bulk_import + classical_favorites + indexes existentes intactos.
  * `RecordingRow.tsx` extensión aditiva: 1 import + 1 handler + 1 Star icon button + condicional editorNote. Tracks no-classical (sin `isEditorsChoice`) renderizan idéntico a Phase 4.
  * `ClassicalHubPage.tsx`: tab "Search" activado (placeholder removed), placeholder "Editor's Choice" reemplazado por sección viva. Featured composers grid intacto.
- D-019, D-020, D-021, D-022 registrados en DECISIONS.md.

**Next action**: el usuario hace QA manual en build instalada (Hub home → ver Editor's Choice section con 12 picks; abrir Beethoven 9 → ver editor note callout + Star icon en grabación canónica; clic Star en otra grabación → toggle override; tab Search → "Beethoven 9 Karajan 1962" devuelve Symphony 9; "Op. 125" devuelve Beethoven 9). Si pasa, Phase 6 arranca sobre `phase-6-personalization.md`.

**Files touched (sesión Phase 5 completa)**:

  Backend:
  - `src-tauri/src/classical/search.rs` (NEW, 24 tests)
  - `src-tauri/src/classical/editorial.rs` (NEW, 9 tests)
  - `src-tauri/src/classical/listening_guide.rs` (NEW, 6 tests)
  - `src-tauri/data/editorial.json` (NEW, 48 work seeds + 15 composer notes)
  - `src-tauri/src/classical/mod.rs` (3 nuevas líneas: pub mod editorial; pub mod listening_guide; pub mod search; + signature de build_catalog_service)
  - `src-tauri/src/classical/types.rs` (Composer.editor_note + Work.editor_note + Recording.is_editors_choice + Recording.editor_note)
  - `src-tauri/src/classical/catalog.rs` (editorial + stats fields, apply_editorial, search_classical, list_editorial_picks, set/clear_user_editors_choice, recording_matches_seed helper)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (1 línea: editor_note: None en Composer + Work)
  - `src-tauri/src/commands/classical.rs` (5 nuevos commands + 3 imports)
  - `src-tauri/src/stats.rs` (classical_editorial table migration + 3 métodos + EditorialOverride struct)
  - `src-tauri/src/lib.rs` (5 invoke_handler entries + Arc::clone(&stats) al builder)

  Frontend:
  - `src/components/classical/ClassicalSearch.tsx` (NEW)
  - `src/components/classical/RecordingRow.tsx` (Star toggle + onEditorialChange + editor note chip)
  - `src/components/classical/WorkPage.tsx` (editor note callout + handleEditorialChange + onEditorialChange wire)
  - `src/components/classical/ComposerPage.tsx` (editor note inline en hero)
  - `src/components/classical/ClassicalHubPage.tsx` (Editor's Choice section + Search tab + EditorialPickCard + PicksSkeleton + handlePickClick)
  - `src/types/classical.ts` (Composer.editorNote + Work.editorNote + Recording.isEditorsChoice/editorNote + 7 nuevos types Phase 5)
  - `src/api/classical.ts` (5 nuevos wrappers)
  - `src/hooks/useNavigation.ts` (navigateToClassicalSearch + return)
  - `src/App.tsx` (1 import + 1 routing branch classical://search)

  Docs:
  - `docs/classical/PROGRESS.md` (Phase 5 → completed; entregables; acceptance checklist; D-019..D-022)
  - `docs/classical/DECISIONS.md` (D-019, D-020, D-021, D-022)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint reemplaza phase-5-init)
  - `docs/classical/phase-5-editorial-search.md` (refinado con scope final + D-022 split out)
  - `docs/classical/phase-6-personalization.md` (NEW — plan completo, recoge Wikidata + related composers diferidos)

**Tests**: 104/104 unit tests del módulo classical pasan (65 previos + 24 nuevos en `classical::search` + 9 nuevos en `classical::editorial` + 6 nuevos en `classical::listening_guide`). Tests existentes inalterados.

**Build**:
- `cargo check --release` ✅
- `cargo build --release` ✅ (51s, binario producido)
- `cargo clippy --release --lib --no-deps` ✅ 14 warnings (idéntico baseline post-Phase 4). 0 nuevas en classical/scrobble/audio.
- `cargo test --release --lib` ✅ 104/104
- `tsc --noEmit` ✅ 0 errores
- `npm run build` (vite) ✅ clean, 1871 módulos

**Notes**:
- Bit-perfect contract intacto: cero archivos del audio path modificados. Verificación explícita: `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → vacío.
- §10 zero-regression: cada modificación a archivo existente es estrictamente aditiva. RecordingRow Star icon es null-render para tracks sin `isEditorsChoice`; sigue mostrando confidence + label exactamente como Phase 4 cuando no aplica.
- Code-style §1: braces siempre. Todos los condicionales del nuevo código usan llaves. Arrow functions de una sola expresión sin bloque preservadas.
- D-020 — el snapshot editorial es **curado por consenso musicológico** documentado: cada seed cita la fuente cuando es debatible (Gramophone Hall of Fame, Penguin Guide rosette, BBC Building a Library). NO se inventaron grabaciones — todas son ISRC-resolvables o text-search-resolvables vía Phase 1 cascade.
- D-021 — el override DB cascade tiene precedence sobre el snapshot. La invalidación del cache `classical:work:v1:{mbid}` después de set/clear garantiza que el siguiente `get_work` aplica la decisión inmediatamente.
- D-022 — Wikidata SPARQL + related composers + browse-by-conductor están en `phase-6-personalization.md`. No bloquean Phase 6 arranque — el doc maestro original Phase 6 era "personal listening integration" + "pre-warm canon"; el nuevo Phase 6 absorbe esos diferidos sin perder el alcance original.
- Limitación conocida: el snapshot V1 cubre 48 works (~15 composers canon mayor). Works fuera del snapshot no muestran editor note ni star — `editor_note: None`, `is_editors_choice: false`. Degradación graciosa.
- Limitación conocida: el matching seed→recording usa heurística string substring de conductor + performer + year ±2. Si MB no devuelve credits enriquecidos en el browse inicial (Phase 1 hace fetch ligero), el match falla silenciosamente. Mitigación: el user override (D-021) cubre cualquier divergencia.
- Limitación conocida: el search MB Lucene fallback NO está implementado en V1 — el executor cascadea snapshot composer-list → snapshot fallback. Para un composer fuera del top-200 OpenOpus, el search devolverá `[]`. Phase 6 puede agregar el MB Lucene fallback si la telemetría lo pide.

---

### 2026-05-02 · Phase 5 · phase-5-init

**State**: in_progress

**Last action**: Phase 5 abierta. Plan refinado registrado en `docs/classical/phase-5-editorial-search.md` con sub-tasks B5.1..B5.6 (backend) + F5.1..F5.5 (frontend). D-019 (search tokenizer determinístico in-process), D-020 (editorial seeds embedded snapshot), D-021 (override manual via stats DB), D-022 (Wikidata SPARQL diferido a Phase 6) registrados en DECISIONS.md.

**Decisión meta**: cero modificaciones a `audio.rs`, `hw_volume.rs`, `signal_path.rs`, `route_volume_change`, writer guard, ni `tidal_api.rs::get_stream_url`. Todo lo nuevo es aditivo: nuevo módulo `classical/search.rs`, nuevo módulo `classical/editorial.rs`, nuevo `data/editorial.json`, migración aditiva `classical_editorial` en stats.rs (mismo patrón que `classical_favorites` Phase 1), nuevos commands Tauri.

**Files audit / read-only**:
  - `src-tauri/src/classical/{mod,types,catalog,quality,movement,matching}.rs`
  - `src-tauri/src/classical/providers/{musicbrainz,wikipedia,tidal,openopus}.rs`
  - `src-tauri/src/commands/classical.rs`
  - `src-tauri/src/stats.rs` (migration patterns)
  - `src-tauri/src/lib.rs:920-1009` (invoke_handler)
  - `src/components/classical/*` (todos los componentes Phase 1-4)
  - `src/types/classical.ts`, `src/api/classical.ts`, `src/hooks/useNavigation.ts`, `src/App.tsx`

**Next action**: ejecutar B5.1 — implementar `classical/search.rs` con tokenizer + planner + executor (D-019). Tests inline > 15 cases.

**Files touched (este checkpoint)**:
  - `docs/classical/PROGRESS.md` (Phase 5 → 🟡 in_progress, sub-tasks listados)
  - `docs/classical/DECISIONS.md` (D-019 + D-020 + D-021 + D-022)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: 65/65 baseline classical pasan. Sin código nuevo todavía.
**Build**: cargo test --release --lib classical:: ✅ 65/65 baseline confirmado.

**Notes**:
- §10 cero regresión preservada por construcción: ninguna modificación al audio path planeada en Phase 5.
- Code-style §1: braces siempre. Aplicará a todo el código nuevo.
- D-020 — el snapshot editorial es **curado por consenso musicológico**, no opinión del supervisor. Cada seed cita la fuente cuando es debatible (e.g. "Gramophone Hall of Fame, Penguin Guide rosette").
- D-022 — Phase 5 NO entrega Wikidata, related composers, ni browse por conductor/orquesta. Esos quedan documentados en `phase-6-personalization.md` para no perder el seguimiento.

---

### 2026-05-02 · Phase 4 · COMPLETED — phase-4-final

**State**: completed (autonomous).

**Last action**: Phase 4 cerrada. Quality USP entregado: ranking numérico (`classical::quality`), refinement por track via `TidalProvider::fetch_track_quality_meta` metadata-only (D-017), aggregator best-of-N con flag has_atmos (D-018), filter chips + sort dropdown en WorkPage, Best available banner clickable que activa Hi-Res shortcut, QualityBadge cosmetic refinement para mostrar "BIT-PERFECT" cuando signalPath confirma. Cero modificación al audio path; el `playbackinfopostpaywall` se reusa pero solo lee 3 campos top-level y descarta manifest — el path de stream `get_stream_url` sigue intacto.

**Decisión final**: ✅ Phase 4 → 🟢 completed. Acceptance §11 cumplida en su totalidad: 3 tests rust acceptance Beethoven 9 (best available 24/192 + sort by quality + filter Hi-Res only). 65/65 unit tests classical pasan. Audio path verificado intacto via `git diff` empty.

**Datos clave**:
- Backend:
  * 1 nuevo módulo `classical/quality.rs` (~340 LOC con tests, 17 tests).
  * 1 nuevo método `TidalProvider::fetch_track_quality_meta` + tipo `TrackQualityMeta`.
  * 1 nuevo flujo `CatalogService::refine_work_quality` con paralelismo limitado (Semaphore=6, top-20 recordings).
  * Cache `classical:track-quality:v1:{id}` con `CacheTier::Dynamic` (TTL 4h SWR 24h) + tag-based invalidation.
  * 3 nuevos campos en `Recording` (`sample_rate_hz`, `bit_depth`, `quality_score`).
  * 1 nuevo campo en `Work` (`best_available_quality: Option<BestAvailableQuality>`).
  * 1 nuevo Tauri command `refresh_classical_work_qualities`.
- Frontend:
  * 3 nuevos componentes: `QualityChip.tsx` (~170 LOC), `RecordingFilters.tsx` (~155 LOC), `RecordingSort.tsx` (~110 LOC).
  * `RecordingRow.tsx` refactor — sustituye `QualityChips` interno por `QualityChip` + helpers.
  * `WorkPage.tsx` extendido — `useMemo` para filter+sort, Best available banner, Refresh quality button.
  * `QualityBadge.tsx` extensión cosmética — 1 import + 1 atom + 1 condicional ternaria para label "BIT-PERFECT" cuando signalPath confirma.
  * Tipos `BestAvailableQuality` + extensiones en `Recording` / `Work`.
  * Wrapper `refreshClassicalWorkQualities` en `src/api/classical.ts`.
- Cero regresión §10:
  * `audio.rs`: `git diff` vacío.
  * `hw_volume.rs`: `git diff` vacío.
  * `signal_path.rs`: `git diff` vacío.
  * `tidal_api.rs`: `git diff` vacío. **NO se reusa get_stream_url**: el nuevo `fetch_track_quality_meta` vive en `TidalProvider`, hace su propia llamada HTTP con tokens compartidos read-only.
  * `route_volume_change` (`lib.rs:491-539`): intacto.
  * Writer guard (`audio.rs:988-992`): intacto.
  * Único delta a `lib.rs` Phase 4: 1 línea `commands::classical::refresh_classical_work_qualities` (nuevo entry en invoke_handler).
  * `QualityBadge.tsx` solo añade lectura del atom `signalPathAtom` y un condicional para el label. NO toca el routing, NO toca el writer, NO toca el volume control.
- D-017 (manifest metadata-only) + D-018 (numeric ranking determinístico) registrados en DECISIONS.md.

**Next action**: el usuario hace QA manual en build instalada (abrir Beethoven 9 → comprobar Best available banner; activar filter Hi-Res only → ver solo HIRES_LOSSLESS rows; sort by quality → 24/192 al frente; reproducir un track Hi-Res con bit-perfect on → label "BIT-PERFECT" en player). Si pasa, Phase 5 arranca sobre `phase-5-editorial-search.md`.

**Files touched (sesión Phase 4 completa)**:

  Backend:
  - `src-tauri/src/classical/quality.rs` (NEW, 17 tests)
  - `src-tauri/src/classical/mod.rs` (1 línea: `pub mod quality;` + re-export `BestAvailableQuality`)
  - `src-tauri/src/classical/types.rs` (Recording: sample_rate_hz/bit_depth/quality_score; Work: best_available_quality; new struct BestAvailableQuality)
  - `src-tauri/src/classical/providers/tidal.rs` (new fetch_track_quality_meta + TrackQualityMeta struct)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (1 línea: `best_available_quality: None,` en literal Work)
  - `src-tauri/src/classical/catalog.rs` (refine_work_quality + fetch_quality_metas_parallel + fetch_or_cache_track_quality + refresh_work_recording_qualities + nuevos const)
  - `src-tauri/src/commands/classical.rs` (1 nuevo command refresh_classical_work_qualities)
  - `src-tauri/src/lib.rs` (1 línea invoke_handler entry)

  Frontend:
  - `src/components/classical/QualityChip.tsx` (NEW)
  - `src/components/classical/RecordingFilters.tsx` (NEW)
  - `src/components/classical/RecordingSort.tsx` (NEW)
  - `src/components/classical/RecordingRow.tsx` (refactor: replace QualityChips with QualityChip + helpers)
  - `src/components/classical/WorkPage.tsx` (filters + sort + Best available banner + Refresh quality)
  - `src/components/QualityBadge.tsx` (cosmetic: read signalPathAtom, add BIT-PERFECT label condicional)
  - `src/types/classical.ts` (Recording extended; Work.bestAvailableQuality; new BestAvailableQuality interface)
  - `src/api/classical.ts` (refreshClassicalWorkQualities wrapper)

  Docs:
  - `docs/classical/PROGRESS.md` (Phase 4 → completed; entregables; acceptance checklist)
  - `docs/classical/DECISIONS.md` (D-017, D-018)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)
  - `docs/classical/phase-5-editorial-search.md` (NEW — plan detallado)

**Tests**: 65/65 unit tests del módulo classical pasan (48 previos + 17 nuevos en `classical::quality` incluyendo 3 acceptance Beethoven 9). Tests existentes inalterados.

**Build**:
- `cargo check --release` ✅
- `cargo build --release` ✅ (54 s, binario producido)
- `cargo clippy --release --lib --no-deps` ✅ 14 warnings (idéntico baseline Phase 3). 0 nuevas en classical/scrobble.
- `cargo test --release --lib` ✅ 65/65
- `tsc --noEmit` ✅ 0 errores
- `npm run build` (vite) ✅ clean

**Notes**:
- Bit-perfect contract intacto: cero archivos del audio path modificados. Verificación explícita: `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → vacío.
- §10 zero-regression: cada modificación a archivo existente es estrictamente aditiva. `RecordingRow` refactor reemplaza un helper privado por componente externo equivalente; comportamiento UI idéntico para tracks no-classical.
- Code-style §1: braces siempre. Todos los condicionales del nuevo código usan llaves. Arrow functions de una expresión sin bloque (e.g. `(rec) => rec.quality_score`) preservadas — son válidas según `docs/code-style.md`.
- D-017 — el `playbackinfopostpaywall` se reusa pero **descartando** el manifest base64 (no se decodifica). Esto evita que el módulo classical genere URLs de stream playable, manteniendo la separación con el audio path.
- D-018 — el ranking es u32 puro: `DOLBY_ATMOS bonus +200`, `HIRES_LOSSLESS = 4000`, `LOSSLESS = 3000`, `MQA = 2000`, `HIGH = 1000`. Refinement bonus dentro del tier: 24-bit +60, 16-bit +20; sample rate de 44.1k → +20 hasta 192k → +80.
- Limitación conocida (a resolver en Phase 5): el `qualityScore` se computa solo durante `build_work_fresh`. Si un track Tidal cambia de tier upstream (raro), no se refleja hasta force-refresh. La cache TTL Dynamic 4h del per-track ayuda; el botón "Refresh quality" del WorkPage es la vía manual.
- Limitación conocida: el sort `popularity` reusa el orden MB original (proxy de popularidad por release count). Phase 5 puede añadir un `popularityScore` numérico explícito.

---

### 2026-05-02 · Phase 4 · phase-4-init

**State**: in_progress

**Last action**: Phase 4 abierta. Plan en `docs/classical/phase-4-quality-usp.md` (B4.1 sample-rate refinement, B4.2 aggregator best-of-N, B4.3 commands; F4.1 QualityChip, F4.2 filter+sort, F4.3 best-available banner, F4.4 player bit-perfect refinement). D-017 registrado: nuevo `TidalProvider::fetch_track_quality_meta` que reusa `playbackinfopostpaywall` solo metadata (sin tocar manifest), cacheable por `track_id` con `CacheTier::Dynamic`. D-018 registrado: ranking numérico determinístico en `classical::quality`.

**Decisión meta**: cero cambios a `audio.rs`, `hw_volume.rs`, `signal_path.rs`, `route_volume_change`, writer guard, ni `tidal_api.rs::get_stream_url`. El nuevo método metadata-only en `TidalProvider` vive paralelo, NO toca el playback path. `QualityBadge.tsx` extensión cosmética 4 líneas.

**Next action**: implementar B4.1 (extender Recording con sample_rate_hz / bit_depth + nuevo método `fetch_track_quality_meta` en TidalProvider con cache).

**Files touched (este checkpoint)**:
  - `docs/classical/PROGRESS.md` (Phase 4 → 🟡 in_progress)
  - `docs/classical/DECISIONS.md` (D-017 + D-018)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: n/a (sin código nuevo)
**Build**: n/a

**Notes**: el endpoint `playbackinfopostpaywall` retorna `{audio_quality, bit_depth, sample_rate, manifest, manifest_mime_type, ...}` top-level. Phase 4 lee solo los 3 primeros y descarta manifest. Exclusión explícita: el método NUNCA expone URL ni manifest a la cache de catalog.

---

### 2026-05-02 · Phase 3 · COMPLETED — phase-3-final

**State**: completed (autonomous). QA manual instrumented pendiente del operador (D-016).

**Last action**: Phase 3 cerrada. Movement boundary detection (parser de roman numerals + attacca + position fallback) implementado en `src-tauri/src/classical/movement.rs` con 19 tests deterministic. Comando Tauri `resolve_classical_movement` registrado. Event `classical:work-resolved` emitido por el ScrobbleManager tras `applied=true` para reemplazar polling como path primario en frontend. Hook `useClassicalContext` aggregates work + movement. Componente nuevo `WorkHeaderLine.tsx` renderiza "Composer · Work · II / IV · attacca →" en el PlayerBar. ClassicalWorkLink refactored con event subscription + single delayed poll fallback.

**Decisión final**: ✅ Phase 3 → 🟢 completed (componente automatizable). Acceptance §11 cumplida en todo lo automatizable; el componente E2E (gapless capture < 50 ms) queda como QA manual instrumentado por D-016 — no bloquea Phase 4 pero el operador debe ejecutarlo con build instalada antes de cerrar el gate del USP.

**Datos clave**:
- Backend: 1 nuevo módulo `classical/movement.rs` (~480 LOC con tests, 19 tests). 1 nuevo método `CatalogService::resolve_movement`. 1 nuevo Tauri command `resolve_classical_movement`. 1 emit `classical:work-resolved` en `ScrobbleManager::on_track_started`.
- Frontend: 1 nuevo hook `useClassicalContext.ts` (~165 LOC). 1 nuevo componente `WorkHeaderLine.tsx` (~120 LOC). `ClassicalWorkLink.tsx` reescrito con event subscription. `PlayerBar::TrackInfoSection` extendido con `<WorkHeaderLine />`. Tipos `MovementContext`, `ResolutionMethod`, `ClassicalWorkResolvedPayload` añadidos a `src/types/classical.ts`. `resolveClassicalMovement` wrapper en `src/api/classical.ts`.
- Cero regresión §10:
  * `audio.rs`: `git diff` vacío.
  * `hw_volume.rs`: `git diff` vacío.
  * `signal_path.rs`: `git diff` vacío.
  * `route_volume_change` (`lib.rs:491-539`): intacto.
  * Writer guard (`audio.rs:988-992`): intacto.
  * Único delta a `lib.rs` Phase 3: línea 1004 (registro del comando movement).
  * Único delta a `scrobble/mod.rs` Phase 3: 1 emit + 1 clone de app_handle pre-spawn. NO modifica `dispatch_scrobble`, `fire_now_playing`, `record_to_stats`, ni el critical path.
- D-016 (gapless gate split deterministic + manual) registrado en DECISIONS.md.

**Next action**: el usuario hace QA manual instrumented (Beethoven 5 III→IV / Mahler 3 V→VI / Bruckner 8 III→IV con bit-perfect on, observar gap < 50 ms). En paralelo, Phase 4 puede arrancar — son ortogonales. Si QA fallara, abrir D-018+ con investigación del writer thread sin bloquear Phase 4.

**Files touched (sesión Phase 3 completa)**:

  Backend:
  - `src-tauri/src/classical/movement.rs` (NEW, 19 tests)
  - `src-tauri/src/classical/mod.rs` (1 línea: `pub mod movement;`)
  - `src-tauri/src/classical/catalog.rs` (1 nuevo método `resolve_movement` + sección comentada Phase 3)
  - `src-tauri/src/commands/classical.rs` (1 nuevo command `resolve_classical_movement` + import)
  - `src-tauri/src/lib.rs` (1 línea: invoke_handler entry)
  - `src-tauri/src/scrobble/mod.rs` (event_handle clone pre-spawn + emit `classical:work-resolved` post-applied)

  Frontend:
  - `src/types/classical.ts` (MovementContext + ResolutionMethod + ClassicalWorkResolvedPayload + comentado Phase 3)
  - `src/api/classical.ts` (1 nuevo wrapper `resolveClassicalMovement`)
  - `src/hooks/useClassicalContext.ts` (NEW)
  - `src/components/classical/WorkHeaderLine.tsx` (NEW)
  - `src/components/classical/ClassicalWorkLink.tsx` (rewritten — event subscription primario + single fallback poll)
  - `src/components/PlayerBar.tsx` (1 import + 1 line `<WorkHeaderLine />` en TrackInfoSection)

  Docs:
  - `docs/classical/PROGRESS.md` (Phase 3 → completed; entregables; acceptance checklist; Phase 4 puntero)
  - `docs/classical/DECISIONS.md` (D-016)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)
  - `docs/classical/ARCHITECTURE.md` (refactor completo post-Phase 1+2 + sección 6 "Gapless contract — auditoría B3.0")
  - `docs/classical/phase-3-player-gapless.md` (acceptance criteria split + sección "QA manual" con procedimiento instrumented)
  - `docs/classical/phase-4-quality-usp.md` (NEW — plan completo)

**Tests**: 48/48 unit tests del módulo classical pasan (29 previos + 19 nuevos en `classical::movement`). Tests existentes inalterados.

**Build**:
- `cargo check --release` ✅
- `cargo build --release` ✅ (54 s, binario producido)
- `cargo clippy --release --lib --no-deps` ✅ 14 warnings (idéntico Phase 2 baseline). 0 nuevas en classical / scrobble.
- `cargo test --release --lib` ✅ 48/48
- `tsc --noEmit` ✅ 0 errores
- `vite build` ✅ clean

**Notes**:
- Bit-perfect contract intacto: cero archivos del audio path modificados. Verificación explícita con `git diff src-tauri/src/{audio,hw_volume,signal_path}.rs` → empty.
- §10 zero-regression: cada modificación a archivo existente es estrictamente aditiva. PlayerBar.tsx render de WorkHeaderLine es null cuando no hay work_mbid resuelto — UI exactamente igual a Phase 2 para tracks no-classical.
- Code-style §1: braces siempre. Todos los condicionales del nuevo código usan llaves.
- Limitación conocida (a resolver en Phase 4 o eventual D-018): si los movements de un mismo recording vienen en formatos distintos (raro — un álbum Tidal típico es uniforme) el writer reabrirá ALSA con `reopen_alsa(new_fmt)`, lo que añade ~10-30ms al gap. No actionable en Phase 3; documentado en ARCHITECTURE.md §6.4.
- Limitación conocida: el matching de movement por title funciona bien con romanos y con substring fuerte (Goldberg "Variation 2"). Falla con títulos creativos sin roman ni overlap textual con `Movement.title` (raro en clásica, frequente en jazz/contemporary). Fallback a `album_position` mitiga.

---

### 2026-05-02 · Phase 3 · phase-3-init

**State**: in_progress

**Last action**: Phase 3 abierta. Plan refinado en `docs/classical/phase-3-player-gapless.md` con sub-tasks B3.0..B3.4 (backend) + F3.0..F3.3 (frontend). D-016 registrado: el gate de gapless se divide en deterministic (autonomous) + instrumented manual (operator post-build), porque el test E2E con captura de audio requiere infra no disponible en modo autonomous (auth Tidal + DAC + buffer analyzer).

**Decisión meta**: Phase 3 no modifica `audio.rs`, `hw_volume.rs`, ni `signal_path.rs` ni el writer thread. Cualquier pieza work-aware del player es estrictamente aditiva al UI + scrobble manager (event emission post-resolve, no hot path). Movement boundary detection es lógica nueva en `src-tauri/src/classical/movement.rs`, sin contacto con audio.

**Files audit / read-only**:
  - `src-tauri/src/audio.rs` (2287 LOC, leído para entender contrato gapless actual)
  - `src-tauri/src/hw_volume.rs` (257 LOC)
  - `src-tauri/src/signal_path.rs` (209 LOC)
  - `src-tauri/src/lib.rs:485-540` (route_volume_change, contrato bit-perfect)
  - `src-tauri/src/scrobble/mod.rs:160-440` (WorkMbidResolver + on_track_started)
  - `src/components/PlayerBar.tsx` (648 LOC)
  - `src/components/classical/ClassicalWorkLink.tsx` (119 LOC, polling actual)
  - `src/types/classical.ts` (321 LOC, Movement type ya presente)
  - `src-tauri/src/classical/types.rs:218-234` (Movement struct ya presente con attacca_to)

**Next action**: ejecutar B3.0 — auditoría estática de audio.rs + sección "Bit-perfect path / Gapless contract" en `docs/classical/ARCHITECTURE.md`. Esto es read-only y documenta el contrato vigente como referencia para el componente deterministic del gate.

**Files touched (este checkpoint)**:
  - `docs/classical/PROGRESS.md` (Phase 3 → 🟡 in_progress, scope refinado, sub-tasks listados)
  - `docs/classical/DECISIONS.md` (D-016 — gapless gate split)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: n/a (sin código nuevo en este checkpoint)
**Build**: n/a

**Notes**:
- §10 cero regresión preservado por construcción: ninguna modificación al audio path planeada en Phase 3.
- El writer thread mantiene el DAC abierto entre tracks (`PlaybackBackend::DirectAlsa` + `EndOfTrack { emit_finished, generation }`); el control gapless ya existe a ese nivel. Phase 3 no lo toca.
- El "test gapless con captura de audio" del plan original queda como QA manual instrumentado (D-016).

---

### 2026-05-02 · Phase 2 · COMPLETED — phase-2-final

**State**: completed

**Last action**: Phase 2 cerrada. Hub landing, Composer pages, Browse axes (Composers, Periods, Genres, Era drill-down) operativos. OpenOpus snapshot embedded (227 KB, 33 composers, 1459 works). Pill "Classical Hub" insertada en ExplorePage. Backend extension: `list_top_composers`, `list_composers_by_era`, `list_works_by_composer` con cache. Frontend: 7 nuevos componentes, routing aditivo, navegación extendida.

**Decisión final**: ✅ Phase 2 → 🟢 completed. Acceptance §11 cumplida en todo lo automatizable; los criterios de latencia warm-cache requieren build instalada con auth Tidal real (igual que Phase 1).

**Datos clave**:
- Backend: 1 nuevo provider (OpenOpusProvider, 320 LOC), `MusicBrainzProvider::browse_works_by_artist` + `MbBrowsedWork`, 3 nuevos catalog methods (180 LOC), 2 nuevos `parse_literal` impls, 3 nuevos Tauri commands.
- Frontend: 7 nuevos componentes (ClassicalHubPage, ComposerPage, BrowseComposers, BrowsePeriods, BrowseGenres, BrowseEra, EraBadge, ComposerCard, WorkSummaryCard) sumando ~1300 LOC. Extensión de `src/types/classical.ts` con `ComposerSummary`, `WorkSummary`, helpers de labels. 4 nuevos navigators en `useNavigation`.
- Cero regresión §10:
  * `ExplorePage.tsx`: pill insertada como sección nueva al inicio del header. Tidal pillSections/iconSection/untitled sections preservadas exactamente.
  * `App.tsx`: 5 branches aditivos dentro del `case "explorePage":`. Fall-through al `ExploreSubPage` idéntico para todo apiPath sin prefijo `classical://`.
  * `useNavigation.ts`: 4 callbacks adicionales. Funciones existentes intactas.
  * Audio routing: cero modificaciones (ningún archivo de §10 audio path tocado).
  * Stats DB / scrobble: no tocados (Phase 2 sólo añade catálogo).
- D-013 (supervisor en single-process), D-014 (`parse_literal` naming), D-015 (title-normalized MB↔OpenOpus matching) registrados en DECISIONS.md.

**Next action**: el usuario hace QA manual en build instalada (Explore → click pill → ClassicalHubPage carga; click featured composer → ComposerPage; abre any work → WorkPage Phase 1 sigue funcionando). Si pasa, Phase 3 arranca sobre `phase-3-player-gapless.md`.

**Files touched (sesión Phase 2 completa)**:

  Backend:
  - `src-tauri/data/openopus.json` (NEW, 227 KB embedded snapshot)
  - `src-tauri/src/classical/providers/openopus.rs` (NEW, 8 tests)
  - `src-tauri/src/classical/providers/mod.rs` (1 line: pub mod openopus;)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (added browse_works_by_artist + MbBrowsedWork + work_type_from_mb_label)
  - `src-tauri/src/classical/types.rs` (added ComposerSummary, WorkSummary, Era::parse_literal, Genre::parse_literal)
  - `src-tauri/src/classical/mod.rs` (export ComposerSummary/WorkSummary; OpenOpus wired in factory)
  - `src-tauri/src/classical/catalog.rs` (3 new methods, normalize_title_for_match, sort_key_for_catalogue, OpenOpus dependency in struct + new())
  - `src-tauri/src/commands/classical.rs` (3 new commands)
  - `src-tauri/src/lib.rs` (3 lines: invoke_handler entries)

  Frontend:
  - `src/types/classical.ts` (ComposerSummary + WorkSummary + 4 helpers + BROWSEABLE_ERAS const)
  - `src/api/classical.ts` (3 new wrappers)
  - `src/hooks/useNavigation.ts` (4 new callbacks + return)
  - `src/App.tsx` (5 routing branches)
  - `src/components/ExplorePage.tsx` (1 import + 1 pill block + 1 navigator destructured)
  - `src/components/classical/ClassicalHubPage.tsx` (NEW)
  - `src/components/classical/ComposerPage.tsx` (NEW)
  - `src/components/classical/BrowseComposers.tsx` (NEW)
  - `src/components/classical/BrowsePeriods.tsx` (NEW)
  - `src/components/classical/BrowseGenres.tsx` (NEW)
  - `src/components/classical/BrowseEra.tsx` (NEW)
  - `src/components/classical/EraBadge.tsx` (NEW)
  - `src/components/classical/ComposerCard.tsx` (NEW)
  - `src/components/classical/WorkSummaryCard.tsx` (NEW)

  Docs:
  - `docs/classical/PROGRESS.md` (Phase 2 → completed; entregables; acceptance checklist)
  - `docs/classical/DECISIONS.md` (D-013, D-014, D-015)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)
  - `docs/classical/phase-2-browse.md` (sub-tasks granulares B1-B4, F1-F8, C0)
  - `docs/classical/phase-3-player-gapless.md` (NEW — plan detallado para Phase 3)

**Tests**: 29/29 unit tests del módulo classical pasan (8 nuevos en `providers::openopus`). Tests existentes inalterados.

**Build**:
- `cargo check --release` ✅
- `cargo build --release` ✅ (53s, binario producido)
- `cargo clippy --release --lib --no-deps` ✅ 0 warnings nuevos en classical/. 14 warnings pre-existentes preservados.
- `cargo test --release --lib classical::` ✅ 29/29
- `tsc --noEmit` ✅ 0 errores
- `vite build` ✅ 1865 módulos transformados, dist generado

**Notes**:
- Bit-perfect contract intacto: cero archivos del audio path modificados.
- §10 zero-regression: cada modificación a archivo existente es aditiva, justificada en el checklist de Phase 2 acceptance.
- Limitación conocida (a resolver en Phase 5): la lista de works por composer cap a 100 (MB max-page). Mozart > 600 works en MB; Phase 5 pagina.
- Limitación conocida: BrowseGenres es informacional (sin drill-down a lista de works del genre). Phase 5 lo amplía cuando el catálogo tenga genre per-work consistente.
- Limitación conocida: el matching MB↔OpenOpus por título es substring-based; algunos works rompen (Mozart "Don Giovanni" vs "Il dissoluto punito, ossia il Don Giovanni"). El popular flag no se propaga, degradación graciosa.

---

### 2026-05-02 · Phase 1 · COMPLETED — phase-1-final

**State**: completed

**Last action**: Phase 1 cerrada. Frontend + integración player + scaffold Phase 2 listos. Build en cargo check / cargo build --release / cargo test --release --lib / vite build / tsc --noEmit todos limpios. 21/21 tests del módulo classical pasan. Sin commits — el usuario commiteará al final.

**Decisión final**: ✅ Phase 1 → 🟢 completed. Acceptance §11 cumplida en todo lo automatizable; los criterios que requieren auth Tidal viva están marcados ⚠ en PROGRESS.md y dependen del operador para validarlos en una build instalada.

**Datos clave**:
- Backend: módulo classical/ con 8 archivos Rust (~1100 líneas), 21 unit tests, 4+1 Tauri commands.
- Frontend: 5 componentes nuevos en `src/components/classical/` (~600 líneas TSX), 1 nuevo hook flow en `useNavigation`, 1 routing branch aditivo en `App.tsx`, 1 sub-componente en `PlayerBar`.
- Cero regresión en §10: archivos audio.rs / hw_volume.rs / signal_path.rs no modificados. ExplorePage / Sidebar / Stats / Galaxy / Live painting / Share link / TIDAL favorites no tocados. MusicBrainzLookup intacto. Cache schema sin DROP/ALTER destructive.
- D-011 (`disk_cache` y `tidal_client` ampliados a `Arc<...>`) y D-012 (decoupling scrobble↔classical via trait `WorkMbidResolver`) registrados en DECISIONS.md.

**Next action**: el usuario hace QA manual en build instalada (reproducir desde un track con ISRC, esperar al "View work" badge ~5-30s, click → WorkPage, comprobar > 20 recordings, play un row IsrcBound). Si pasa, Phase 2 arranca sobre `phase-2-browse.md`. Phase 2 first task: pill "Classical Hub" en ExplorePage + Hub landing.

**Files touched (sesión Phase 1 completa)**:

  Backend:
  - `src-tauri/src/classical/mod.rs` (NEW)
  - `src-tauri/src/classical/types.rs` (NEW)
  - `src-tauri/src/classical/matching.rs` (NEW)
  - `src-tauri/src/classical/catalog.rs` (NEW)
  - `src-tauri/src/classical/providers/mod.rs` (NEW)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (NEW)
  - `src-tauri/src/classical/providers/tidal.rs` (NEW)
  - `src-tauri/src/classical/providers/wikipedia.rs` (NEW)
  - `src-tauri/src/commands/classical.rs` (NEW)
  - `src-tauri/src/commands/mod.rs` (1 line)
  - `src-tauri/src/commands/scrobble.rs` (4 lines: 1 work_mbid field + 1 new command)
  - `src-tauri/src/lib.rs` (modules + AppState fields + factory wiring + 5 invoke_handler entries)
  - `src-tauri/src/scrobble/mod.rs` (WorkMbidResolver trait + setter + work_mbid field on ScrobbleTrack + work resolution in on_track_started + 2 getters)
  - `src-tauri/src/stats.rs` (work_mbid migration + classical_favorites table + index + PlayRecord field + 2 INSERTs)

  Frontend:
  - `src/types/classical.ts` (NEW)
  - `src/api/classical.ts` (NEW)
  - `src/components/classical/ConfidenceBadge.tsx` (NEW)
  - `src/components/classical/MovementList.tsx` (NEW)
  - `src/components/classical/RecordingRow.tsx` (NEW)
  - `src/components/classical/WorkPage.tsx` (NEW)
  - `src/components/classical/ClassicalWorkLink.tsx` (NEW)
  - `src/components/PlayerBar.tsx` (1 import + 1 line for `<ClassicalWorkLink />`)
  - `src/App.tsx` (1 import + 1 routing branch under `explorePage`)
  - `src/hooks/useNavigation.ts` (`navigateToClassicalWork` callback + return)

  Docs:
  - `docs/classical/PROGRESS.md` (Phase 1 → completed; entregables; acceptance checklist; Phase 2 puntero)
  - `docs/classical/DECISIONS.md` (D-011 + D-012)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint + el de backend-complete)
  - `docs/classical/phase-2-browse.md` (NEW — scaffold)

**Tests**: 21/21 unit tests del módulo classical pasan. Tests existentes del workspace inalterados.

**Build**:
- `cargo check --release` ✅
- `cargo build --release` ✅ (52s, binario producido)
- `cargo clippy --release --lib` ✅ 0 warnings nuevos en classical / scrobble / stats; 16 warnings pre-existentes preservados
- `cargo test --release --lib` ✅ 21/21
- `tsc --noEmit` ✅ 0 errores
- `vite build` ✅ 1855 módulos transformados, dist generado

**Notes**:
- Bit-perfect contract intacto: `route_volume_change`, audio writer, signal_path, hw_volume, exclusive_mode — nada modificado.
- §10 zero-regression: cada archivo modificado tiene cambio aditivo justificado:
  * `App.tsx`: branch nuevo solo activo para prefijo `classical://`. Fall-through a ExploreSubPage idéntico para todos los otros apiPath.
  * `useNavigation.ts`: callback nuevo. Funciones existentes intactas.
  * `PlayerBar.tsx`: subcomponente condicional (`workMbid === null → null`). Sin work_mbid resuelto, UI exactamente igual a hoy.
  * `scrobble/mod.rs`: nueva resolución de work_mbid es post-tracking, off the critical path. Sin resolver registrado, comportamiento histórico idéntico.
  * `stats.rs`: migration aditiva en el patrón de `source` y `recording_mbid` columns previas. INSERTs amplían cláusulas con un binding extra, dedup queries (`bulk_import`) sin cambios.
  * `lib.rs`: AppState fields ampliados a Arc<...>. Todos los call-sites siguen funcionando vía Deref. CatalogService init es ordenado (después de tidal_client), no añade panics ni paths bloqueantes.
- Code-style §1: braces siempre. Match guards usados en lugar de `if` colapsables (clippy-clean).
- Archivos NUEVOS no requieren §10 audit — son aditivos por definición.
- Limitación conocida (a resolver en Phase 4): el cascade actualmente bloquea por recording (sequential Tidal calls). Para 60 recordings y ~50ms por call eso son ~3s. Aceptable para cold-cache; Phase 4 puede paralelizar con un FuturesUnordered si se observa en QA real.
- Limitación conocida: el cover de Recording viene del Tidal album cuando el match es ISRC-bound o text-search inferred. Cuando NotFound, no hay cover (lo deja el placeholder). Phase 5 traerá CAA fallback.

---

### 2026-05-02 · Phase 1 · backend complete (B1+B2+B3+B4+B5+F5-backend)

**State**: in_progress (Phase 1, backend done — frontend pending)

**Last action**: Phase 1 backend implementation complete. New module `src-tauri/src/classical/` with types, three providers (MusicBrainz, Wikipedia, Tidal), cascade matcher (D-010), `CatalogService`, four Tauri commands. DB migration adds `work_mbid` column + `classical_favorites` table. Scrobble manager extended with optional `WorkMbidResolver` trait wired to the catalog so `work_mbid` is resolved post-track-start without coupling scrobble→classical at compile time. `cargo check --release` clean. `cargo clippy --release --lib` shows zero warnings in the classical module (16 pre-existing warnings in unrelated files preserved). `cargo test --release --lib classical::` → 21/21 passed.

**Next action**: implement the frontend tasks F1–F4 (types, api wrappers, components WorkPage/RecordingRow/ConfidenceBadge/MovementList, App.tsx routing for `classical://work/{mbid}`, useNavigation `navigateToClassical`, Player "View work" button). After that: TypeScript build + manual smoke against Beethoven 9 page from a running session.

**Files touched (so far this session)**:
  - `src-tauri/src/classical/mod.rs` (new — module entry, factory)
  - `src-tauri/src/classical/types.rs` (new — domain types Composer/Work/Recording/...)
  - `src-tauri/src/classical/matching.rs` (new — D-010 cascade matcher with scoring + tests)
  - `src-tauri/src/classical/catalog.rs` (new — CatalogService orchestrator + cache)
  - `src-tauri/src/classical/providers/mod.rs` (new — trait + MbRateLimiter)
  - `src-tauri/src/classical/providers/musicbrainz.rs` (new — work/recordings/composer fetch + parsers + tests)
  - `src-tauri/src/classical/providers/tidal.rs` (new — ISRC bridge + canonical query builder + tests)
  - `src-tauri/src/classical/providers/wikipedia.rs` (new — REST summary)
  - `src-tauri/src/commands/classical.rs` (new — 4 Tauri commands)
  - `src-tauri/src/commands/mod.rs` (+1 line: `pub mod classical;`)
  - `src-tauri/src/lib.rs` (added `pub mod classical;`, `disk_cache: DiskCache → Arc<DiskCache>`, `tidal_client: Mutex → Arc<Mutex>`, AppState `classical` field, catalog factory call, scrobble resolver wiring, 4 invoke_handler entries)
  - `src-tauri/src/scrobble/mod.rs` (added `WorkMbidResolver` trait, `set_work_resolver`, `work_mbid` field on ScrobbleTrack, work resolution in on_track_started)
  - `src-tauri/src/stats.rs` (migration: `work_mbid` column + `classical_favorites` table; index; PlayRecord + INSERTs)
  - `src-tauri/src/commands/scrobble.rs` (`work_mbid: None` in ScrobbleTrack literal)

**Tests**: cargo test classical:: → 21/21 pass.
**Build**: cargo check --release ✅. cargo clippy --release --lib: 0 warnings in classical module; 16 pre-existing unrelated warnings preserved (audio.rs, cli.rs, lastfm.rs, library.rs, musicbrainz.rs original cmd, discord.rs, tidal_api.rs, lib.rs).

**Notes**:
- Bit-perfect contract intact: zero changes to `route_volume_change`, audio writer, signal path, hw_volume, exclusive mode, or any audio routing. The classical module is read-only catalog and never reaches the audio path.
- §10 zero regression: changed shape of two AppState fields (`disk_cache`, `tidal_client`) from `Mutex/raw` to `Arc<...>`. All existing call-sites use `state.field.method()` which still works via Deref. Scrobble's PlayRecord + ScrobbleTrack got an extra `work_mbid` field, defaulted to `None` everywhere.
- Code-style §1: braces always. Match guards used where clippy warned about collapsible inner ifs.
- D-011 (auto): `disk_cache` and `tidal_client` widened to `Arc<...>` in AppState — required so the new CatalogService can share state with the rest of the app without owning duplicates.
- D-012 (auto): scrobble↔classical decoupling via `WorkMbidResolver` trait. Avoids a direct dependency from `scrobble` on `classical`.

---

### 2026-05-01 22:35 · meta · bootstrap

**State**: completed

**Last action**: bootstrap del sistema de gestión del proyecto autonomous. Memorias guardadas (code-style, project status, resume protocol, índice MEMORY.md actualizado). Branch `soneClassical` creada desde master (410fd36). LFM import committeado a master. `.gitignore` con carve-outs para `docs/classical/` y `.claude/agents/`.

**Next action**: bootstrap de docs/classical/ files restantes (AGENTS.md, ARCHITECTURE.md, phase-0-spike.md scaffold) → actualizar agentes con braces style + resume protocol → commit inicial soneClassical → handoff a `classical-supervisor` para iniciar Phase 0.

**Files touched**:
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/feedback_code_style.md` (nuevo)
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/project_classical_status.md` (nuevo)
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/reference_classical_resume_protocol.md` (nuevo)
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/MEMORY.md` (índice actualizado)
  - `.gitignore` (carve-outs para classical docs y agents)
  - `docs/code-style.md` (nuevo)
  - `docs/classical/README.md` (nuevo)
  - `docs/classical/PROGRESS.md` (nuevo)
  - `docs/classical/DECISIONS.md` (nuevo, con D-001..D-008)
  - `docs/classical/CHECKPOINTS.md` (este archivo, nuevo)

**Tests**: n/a (bootstrap, no production code)
**Build**: n/a

**Notes**: Master en 410fd36 (LFM import). Stash `wip-classical-design-and-agents` aplicado a soneClassical y resuelto. Doc maestro en `/CLASSICAL_DESIGN.md` está versionado en la rama. Los 4 agentes están en `.claude/agents/` pero faltan actualizar con braces style + resume protocol.

---

### 2026-05-01 22:50 · meta · infra-completo + handoff bloqueado por runtime

**State**: blocked (necesita restart de Claude Code)

**Last action**: completado el bootstrap de infraestructura — los 4 agentes (`.claude/agents/*.md`) actualizados con resume protocol + braces rule + obligación de checkpoint write-back. `docs/classical/` completo (README, PROGRESS, DECISIONS con D-001..D-008, CHECKPOINTS, AGENTS, ARCHITECTURE skeleton, phase-0-spike.md plan). Branch `soneClassical` con commit inicial `3f6121a` (14 archivos, 2878 inserciones).

Intento de invocar `classical-supervisor` desde la sesión actual de Claude Code para arrancar Phase 0. **Falló**: "Agent type 'classical-supervisor' not found". Los agentes project-scoped en `.claude/agents/` se cargan en el arranque de Claude Code, no en caliente. La sesión actual fue iniciada antes de que existieran los archivos de agente, por eso el dispatcher no los reconoce.

**Next action** (al retomar en sesión nueva):

1. **Verificar contexto** — Claude principal de la nueva sesión carga automáticamente las memorias `project_classical_status.md` + `reference_classical_resume_protocol.md`. Si no, las lee manualmente.
2. **Verificar branch** — `git branch --show-current` debe ser `soneClassical`.
3. **Confirmar agentes disponibles** — al estar la nueva sesión iniciada con los archivos `.claude/agents/*.md` en sitio, los 4 agentes deben aparecer como `subagent_type` invocables.
4. **Invocar `classical-supervisor`** con el prompt de kickoff de Phase 0 (preservado abajo en sección "Prompt de retomada para classical-supervisor").
5. El supervisor toma el control y ejecuta Phase 0 al completo (Step 0.1 a 0.5).

**Files touched** (este checkpoint):
  - `.claude/agents/classical-supervisor.md` (resume protocol + braces rule añadidos)
  - `.claude/agents/classical-musicologist.md` (resume protocol + persistencia editorial añadidos)
  - `.claude/agents/sone-backend-engineer.md` (resume protocol + braces rule añadidos)
  - `.claude/agents/sone-frontend-engineer.md` (resume protocol + braces rule añadidos)
  - `docs/classical/README.md` (nuevo)
  - `docs/classical/AGENTS.md` (nuevo)
  - `docs/classical/ARCHITECTURE.md` (skeleton)
  - `docs/classical/phase-0-spike.md` (plan completo de Phase 0)

**Tests**: n/a
**Build**: n/a

**Notes**: el sistema está 100% listo para arrancar Phase 0 en cuanto la nueva sesión cargue los agentes. Cero blockers de diseño o decisión — solo el restart de Claude Code es necesario. El usuario fue informado.

---

### 2026-05-01 23:55 · Phase 0 · COMPLETED — GO con asterisco

**State**: completed

**Last action**: Phase 0 completa. Spike binary `src-tauri/examples/spike_isrc_coverage.rs` implementado, ejecutado, datos en mano. Decisión registrada en DECISIONS.md como D-010. Phase 1 abierta en PROGRESS.md y plan completo en `docs/classical/phase-1-foundation.md`.

**Decisión final**: **GO con asterisco**.

**Datos clave**:
- ISRC→Tidal cuando MB tiene ISRC: **83.3%** (10/12) — supera el threshold de 70%.
- ISRC presente en MB sobre canon: **14.8%** (12/81) — bajo.
- Canon hand-picked encontrado en Tidal vía text search: **100%** (25/25).
- Wall-clock: **15.3s** totales (5 MB calls + ~80 Tidal calls). Bajo el threshold de 60s/work.
- Quality breakdown: 90% LOSSLESS, 10% HIRES_LOSSLESS.

**Hallazgo crítico**: el cuello de botella NO es la cobertura Tidal sino la dispersión de ISRCs en MusicBrainz para canon mayor. Phase 1 debe implementar **cascade matching** (ISRC → Tidal text search) — formalizado en D-010 — lo que añade ~20h al estimate de Phase 1 (90h → 110h).

**Next action**: el usuario revisa D-010 y `phase-1-foundation.md`. Si aprueba, Phase 1 arranca con el `sone-backend-engineer` implementando el cascade en `src-tauri/src/classical/`. Punto de entrada concreto: tarea B1 (`mod.rs` + types + provider trait scaffold).

**Files touched (sesión Phase 0)**:
  - `src-tauri/examples/spike_isrc_coverage.rs` (nuevo, 600+ líneas, code-style §1 conforme)
  - `src-tauri/src/lib.rs` (3 líneas: `pub mod crypto`, `pub mod tidal_api`, `pub mod embedded_config` — aditivo, ningún consumer existente afectado)
  - `docs/classical/phase-0-spike.md` (Resultados sección + 3 riesgos detallados)
  - `docs/classical/phase-1-foundation.md` (nuevo, plan completo)
  - `docs/classical/PROGRESS.md` (Phase 0 → 🟢 completed; Phase 1 → ready)
  - `docs/classical/DECISIONS.md` (D-009 técnico + D-010 cascade)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint + dos previos)

**Tests**: spike binary ejecutado contra MB y Tidal en producción (read-only, cero side-effects). Output reproducible con `cargo run --example spike_isrc_coverage --release`. La verificación de "cero regresión" sobre prod aún no se ha hecho con tests automatizados — es trivial porque el spike vive en `examples/` y NO entra en el binary de producción.

**Build**: `cargo build --example spike_isrc_coverage --release` ✅ clean. `cargo build --release` (binary principal) NOT verified yet — recomiendo correrlo antes de commit por el cambio de visibilidad de mods (debería ser inocuo pero es buena disciplina). Ver next action del usuario.

**Notes**:
- Bit-perfect intact: el spike no toca ninguna parte del audio path. Los 3 cambios `mod` → `pub mod` en lib.rs son aditivos (sólo abren visibilidad; ningún consumer existente del crate cambia su comportamiento).
- §10 cero regresión: ninguna área audida fue tocada (ni Explore, ni Sidebar, ni Player, ni Stats, ni Galaxy, ni Live painting, ni Share link, ni Scrobbling, ni MusicBrainzLookup, ni Cache).
- Code-style §1: el spike usa llaves siempre. Se verifica grep -n "if .*[^{]\s*$" examples/spike_isrc_coverage.rs (no matches one-liners).

---

### 2026-05-01 23:35 · meta · agent-dispatch-unavailable (note)

**State**: in_progress (note, not blocking)

**Last action**: la sesión actual de Claude Code que retoma Phase 0 NO tiene el tool `Agent`/`Task` disponible para invocar specialists project-scoped. Los archivos `.claude/agents/*.md` están en sitio pero el dispatcher de esta sesión no los expone como `subagent_type` invocables. El supervisor opera en single-process: aplica los roles de specialist a sí mismo según la phase, manteniendo los standards documentados (code-style §1 llaves, calidad sobre velocidad, brief de §11).

**Next action**: ejecutar Step 0.2 (implementación del spike binary) directamente, aplicando el contrato de `sone-backend-engineer` (cero side-effects en producción, llaves siempre, errores con contexto, no tocar audio routing). El supervisor mantiene la check-list de aceptación al recibir el código de "vuelta".

**Files touched**: ninguno por este checkpoint (es nota operativa).

**Tests**: n/a
**Build**: n/a

**Notes**: cuando el dispatcher esté disponible en sesiones futuras, los specialists podrán reemplazar al supervisor en sus phases. Por ahora la cadena PROGRESS → DECISIONS → CHECKPOINTS sigue siendo la fuente de verdad de cualquier observador externo, sea agent o human.

---

### 2026-05-01 23:20 · Phase 0 · step-0.1-mbids-resolved

**State**: completed (sub-task)

**Last action**: resueltos los 5 MBIDs canónicos de las obras canon vía MusicBrainz API directa (queries con `arid:` + `inc=work-rels` para encontrar parents desde movements). Todos verificados como **parent works**, no movements.

| Obra | MBID parent work |
|---|---|
| Beethoven Symphony 9 in D minor Op 125 "Choral" | `c35b4956-d4f8-321a-865b-5b13d9ed192b` |
| Bach Goldberg Variations BWV 988 | `1d51e560-2a59-4e97-8943-13052b6adc03` |
| Mozart Requiem in D minor K. 626 | `3b11692b-cdc7-4107-9708-e5b9ee386af3` |
| Mahler Symphony 9 in D major | `0d459ba8-74cd-4f1c-82b6-4566a5e0778c` |
| Glass Glassworks (1981/82 suite) | `1d0df1a9-52a4-48ca-a6e5-290cd880e249` |

**Hallazgo crítico**: el endpoint `?inc=recording-rels` del parent work devuelve sólo grabaciones que un editor MB ha relacionado al **nivel del work entero**. Beethoven 9 reporta 64; Mozart Requiem 11. La mayoría de grabaciones reales están relacionadas movement-by-movement (child works via `parts` rel). El spike debe iterar también sobre child works y agregar para reflejar el catálogo real (~200 recordings esperadas para Beethoven 9). Esto se incorpora al brief del backend-engineer.

**Next action**: delegar al `sone-backend-engineer` la implementación de `src-tauri/examples/spike_isrc_coverage.rs` con dos modos: (a) recordings directas del work, (b) recordings recursivas vía child works. Reusa `MusicBrainzLookup` (rate limiter) y `TidalClient::search(isrc)` (la propiedad `TidalTrack.isrc` ya existe).

**Files touched**:
  - `docs/classical/phase-0-spike.md` (tabla MBIDs actualizada en próximo turno)
  - `docs/classical/CHECKPOINTS.md` (este checkpoint)

**Tests**: n/a (research only)
**Build**: n/a (no code yet)

**Notes**: 0 blockers. Internet OK, MB OK (queries respetan implícitamente 1 req/s a través del lookup pacing humano). Auth Tidal status no verificado todavía — se verifica en el primer run del spike binary.

---

## Prompt de retomada para classical-supervisor (usar al primer turno de la nueva sesión)

> **Para el classical-supervisor en la próxima sesión** (copiar literal al invocar via Agent tool):

```
El usuario ha autorizado el desarrollo autonomous completo de SONE Classical. Tu trabajo: tomar el control y dirigir Phase 0 hasta entregar una decisión GO/NO-GO con datos reales.

# Contexto que debes cargar PRIMERO (en orden estricto)

1. /home/drheavymetal/myProjects/mySone/CLASSICAL_DESIGN.md (todo, especialmente §0, §3, §4, §8 Phase 0, §11)
2. /home/drheavymetal/myProjects/mySone/docs/classical/PROGRESS.md
3. /home/drheavymetal/myProjects/mySone/docs/classical/CHECKPOINTS.md (último checkpoint = bootstrap completado)
4. /home/drheavymetal/myProjects/mySone/docs/classical/DECISIONS.md (D-001..D-008)
5. /home/drheavymetal/myProjects/mySone/docs/classical/phase-0-spike.md (plan detallado del spike)
6. /home/drheavymetal/myProjects/mySone/docs/code-style.md

# Mandato del usuario (textual, 2026-05-01)

- Todas las phases (0..6) se completan en V1, sin diferir nada.
- Calidad sobre velocidad; mantenibilidad como métrica primaria.
- Llaves siempre, incluso one-liners (TS/JS y Rust).
- Bit-perfect + exclusive audio MUST inviolables.
- Mobile diferido (no V1).
- Delegas a classical-musicologist para repertorio, sone-backend-engineer para Rust, sone-frontend-engineer para UI.
- Puedes crear más agentes si lo consideras necesario para una phase concreta.

# Tu tarea inmediata: ejecutar Phase 0 spike completo

Sigue exactamente el plan de docs/classical/phase-0-spike.md (steps 0.1 a 0.5):

- Step 0.1: resolver MBIDs reales de las 5 obras canon vía MB API.
- Step 0.2: delegar al sone-backend-engineer la implementación del script standalone en src-tauri/examples/spike_isrc_coverage.rs (no toca producción, sin side-effects).
- Step 0.3: supervisar el run.
- Step 0.4: generar report con tablas y análisis en phase-0-spike.md.
- Step 0.5: registrar decisión en DECISIONS.md como D-009. Si GO o GO-con-asterisco, abrir Phase 1 en PROGRESS.md y crear phase-1-foundation.md scaffold.

Reglas innegociables: cero regresión, bit-perfect intacto, llaves siempre, cada acción significativa → checkpoint en CHECKPOINTS.md, cada decisión → entrada en DECISIONS.md.

Si encuentras blockers que requieren humano (auth Tidal expirada, MB 503 sostenido), para, escribe checkpoint con state=blocked, y reporta. No improvises decisiones humanas.

Tu output final: resumen ejecutivo (status, cobertura %, decisión, próximos pasos, files modificados, checkpoint escrito).
```
