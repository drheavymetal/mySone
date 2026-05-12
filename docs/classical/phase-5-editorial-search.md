# Phase 5 — Editorial layer + advanced classical search

**Status**: 🟡 in_progress — arrancada 2026-05-02 (Phase 4 → 🟢 completed habilita el arranque).
**Owner**: classical-supervisor (rol único; specialists project-scoped no invocables — ver D-013).
**Tiempo estimado**: ~50h (recortado desde el 60h original; D-022 difiere Wikidata + related composers a Phase 6).
**Decision gate** (§11): "Beethoven 9 Karajan" devuelve la grabación correcta como best match.

---

## Contexto crítico — leer antes de empezar

- Phase 0..4 entregaron: catalog + browse + player work-aware + USP de calidad. Phase 5 cierra la **paridad funcional** con AMC en lo que se refiere a curación + descubrimiento.
- AMC tiene Editor's Choice + Listening Guides + search clásico tokenizado. Phase 5 reproduce las tres piezas pero con autoría visible (CLASSICAL_DESIGN.md §4.6).
- El bit-perfect contract sigue sagrado. Cero cambio en `audio.rs`, `hw_volume.rs`, `signal_path.rs`, `route_volume_change`, writer guard, `tidal_api.rs::get_stream_url`.
- **Phase 5 NO incluye** (diferido a Phase 6 por D-022):
  - Wikidata SPARQL provider (P528/P826/P571/P18/P136).
  - "Related composers" via genre overlap.
  - Browse por conductor / orquesta / soloist.
  - Personal listening integration ("tus top works") — siempre fue Phase 6.
  - Compare mode side-by-side — diferido a post-Phase 5 si el usuario lo pide.
  - Pre-warm de canon en background — Phase 6.

---

## Objetivos concretos (Phase 5 V1)

1. **Search parser tokenizado** (D-019) — usuario teclea "Beethoven 9 Karajan 1962" y el backend reconoce composer + work + conductor + year, devuelve la grabación canónica como first hit.
2. **Editor's Choice** (D-020) — snapshot embedded de seeds curados por consenso musicológico (50-80 seeds V1, canon mayor). Star indicator visible en RecordingRow.
3. **Editorial notes** — 1-3 sentences blurb por work canónico + 1-2 por composer canónico, leídas del snapshot.
4. **Override manual** (D-021) — context-menu "Mark as Editor's Choice" persistido en `classical_editorial` table.
5. **Listening guides scaffolding** — leer `~/.config/sone/listening-guides/{work_mbid}.lrc` time-synced si existe; UI mínima de visualización.
6. **Wikipedia coverage extendida** — work + composer en lenguaje de UI con fallback EN; atribución visible CC BY-SA. (Sutil — extiende provider existente.)
7. **Hub home enriquecido** — sección "Editor's Choice" con 6-12 picks visibles directos al WorkPage scrolled to that recording.

---

## Sub-tasks granulares

### Backend

#### B5.1 — Search parser (D-019)

Nuevo módulo `src-tauri/src/classical/search.rs` con:

- `tokenize(query) → Vec<Token>` reconociendo:
  - Composer surname (lookup en OpenOpus snapshot top-N + MB exacto).
  - Catalogue numbers (`(BWV|K|D|RV|Hob|HWV|Op)\.?\s*\d+`).
  - Tonalidad (`(C|D|E|F|G|A|B)[♭♯b#]?\s+(minor|major|m|maj)`).
  - Year (`1[5-9]\d{2}|20\d{2}`).
  - Texto libre (resto).
- `plan(tokens) → SearchPlan { composer_mbid?, catalogue?, keywords, year?, key? }`.
- `search_classical(query) → Vec<SearchHit>` que cascade-executes:
  1. Si plan.composer → `list_works_by_composer(plan.composer_mbid)` y matchea title + catalog.
  2. Si plan.catalogue sin composer → MB Lucene fallback con cap 25.
  3. En cualquier caso, top-N hits van por cascade Tidal (reusa Phase 1 matcher) — but solo top-5 para no explotar latencia.
- Score combinada: catalog_match (0.5) + title_match (0.3) + year_match (0.1) + composer_match (0.1).
- Cache `classical:search:v1:{plan_hash}` con `Dynamic` tier (4h TTL, 24h SWR).
- **Tests > 15 cases**: tokenizer (BWV 1052 / Op. 125 / K. 466 / "Symphony No. 5" / "Beethoven 9 Karajan 1962" / "Bach"), planner (compositor solo, catalog solo, year solo, mixto), execute (composer + work, catalog only, free text).

#### B5.2 — Editorial seeds snapshot (D-020)

Nuevo `src-tauri/data/editorial.json` con shape definida en D-020.

Contenido V1 (curated): 50-80 entries cubriendo:
- Top-30 OpenOpus composers × top-2 works each ≈ 60 work entries con Editor's Choice.
- Top-15 composers × editor_note breve.

Cada seed cita brevemente la fuente cuando es debatible: Gramophone Hall of Fame, Penguin Guide rosette, BBC Building a Library.

Nuevo `src-tauri/src/classical/editorial.rs` provider read-only:
- Parse en `OnceLock` del snapshot embedded (mismo patrón que OpenOpus).
- `lookup_work_editorial(work_mbid) → Option<EditorialEntry>`.
- `lookup_composer_editorial(composer_mbid) → Option<ComposerEditorial>`.
- `list_editorial_picks(limit) → Vec<EditorialPick>` para Hub home.

Integración en `CatalogService`:
- `get_work` añade `work.editor_note` + marca `recording.is_editors_choice = true` en la fila que matchee el seed.
- `get_composer` añade `composer.editor_note`.
- `list_editorial_picks` para Hub.

#### B5.3 — Editorial override persistence (D-021)

Migración aditiva en `stats.rs::migrate`:

```sql
CREATE TABLE IF NOT EXISTS classical_editorial (
    work_mbid TEXT PRIMARY KEY,
    recording_mbid TEXT NOT NULL,
    source TEXT NOT NULL,    -- 'embedded' | 'user'
    note TEXT,
    set_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_classical_editorial_source ON classical_editorial(source);
```

Idempotent. Mismo patrón que `classical_favorites` Phase 1.

API en `stats.rs`:
- `set_editorial_choice(work_mbid, recording_mbid, source, note?)`.
- `get_editorial_choice(work_mbid) → Option<EditorialChoice>`.
- `clear_editorial_choice(work_mbid)`.

API en `CatalogService`:
- `editors_choice_for(work_mbid)` que cascade: user override (DB) → embedded (snapshot) → None.
- `set_user_editors_choice(work_mbid, recording_mbid)` que escribe a DB con `source='user'`.

#### B5.4 — Wikipedia multi-locale fallback

Extender `WikipediaProvider`:
- Aceptar `lang_chain: &[&str]` (e.g. `["es", "en"]`) en lugar del literal `"en"` actual.
- Para Phase 5, hardcoded `["en"]` (no breaking change). Setting de UI lang queda Phase 6.
- Si la primera lang devuelve 404 / extract vacío, intenta la siguiente.

NO agregamos nuevos commands en este sub-task; el cambio es interno + atribución existente sigue funcionando.

#### B5.5 — Listening guides reader

Nuevo Tauri command `read_classical_listening_guide(work_mbid) → Option<LrcGuide>`.

- Path: `~/.config/sone/listening-guides/{mbid}.lrc`.
- Parser LRC: `[mm:ss.cs] line` → array de `{ ts_ms, text }`.
- Read-only del filesystem; si no existe, devuelve `None`.

#### B5.6 — Tauri commands consolidados

Registrados en `commands/classical.rs` + `lib.rs::run`:

- `search_classical(query, limit) → Vec<SearchHit>`.
- `set_classical_editors_choice(work_mbid, recording_mbid)`.
- `clear_classical_editors_choice(work_mbid)` (revierte al snapshot).
- `get_classical_editorial_picks(limit) → Vec<EditorialPick>`.
- `read_classical_listening_guide(work_mbid) → Option<LrcGuide>`.

### Frontend

#### F5.1 — Classical search UI

Nuevo `src/components/classical/ClassicalSearch.tsx` con:
- Input grande con autocompletado.
- Detalle de tokens reconocidos visible bajo el input (e.g. "Detected: composer:Beethoven · work:Symphony 9 · year:1962"). Pure UI; backend devuelve los tokens en el response.
- Resultados agrupados: "Best match" / "More recordings of this work" / "Other".
- Click en cualquier hit → `navigateToClassicalWork(mbid)`.

Activado el tab "Search" en `ClassicalHubPage.tsx` (era placeholder "Phase 5").

#### F5.2 — Editor's Choice indicator

- Star icon en `RecordingRow.tsx` cuando `recording.isEditorsChoice === true`. Tooltip: "Editor's Choice — {note}".
- Context-menu "Mark as Editor's Choice" + "Clear override" en cada fila (oculto si ya es la elección actual).
- Botón en WorkPage header: "Editor's note: ..." quando work.editorNote existe.

#### F5.3 — Listening guide UI

Componente `ListeningGuide.tsx` que toma un array `LrcGuide` + el `playbackPositionMs` actual (atom) y resalta la línea activa. Render solo si el archivo existe; si no, sección oculta.

#### F5.4 — Hub home Editor's Choice section

Reemplaza el placeholder "Coming soon · Editor's Choice" del Hub home (`ClassicalHubPage.tsx`) por una sección live:
- Carga `getClassicalEditorialPicks(12)` on mount.
- Cada pick es card horizontal: cover + work title + conductor + "★ Editor's Choice" + click → navigate al work.

#### F5.5 — Editor note rendering

- `WorkPage.tsx`: si `work.editorNote` existe, render como callout sobre el listado de recordings.
- `ComposerPage.tsx`: si `composer.editorNote` existe, render dentro del hero (debajo del bioShort).

---

## Acceptance criteria (de CLASSICAL_DESIGN.md §11 — Phase 5 gate)

- [ ] Search "Beethoven 9 Karajan" → grabación correcta como best match (rank 1).
- [ ] Search "BWV 1052" → Concerto for Keyboard No. 1 BWV 1052 con sus recordings.
- [ ] Search "Op. 125" → Beethoven Symphony No. 9 como first hit.
- [ ] Editor's Choice indicador visible y persistente; override manual sobrevive a re-fetch del cache (porque el resolver checa DB antes que snapshot).
- [ ] Hub home renderiza ≥ 10 Editor's Choice picks.
- [ ] Un work del canon mayor (ejemplo: Beethoven 9, Bach Goldberg) muestra editorial note en el WorkPage header.
- [ ] Listening guide rendering con línea activa sincronizada al `playbackPositionMs` cuando el archivo `.lrc` existe.
- [ ] Wikipedia atribución visible siempre que se renderice texto de Wikipedia.
- [ ] Cero regresión §10:
  - audio.rs / hw_volume.rs / signal_path.rs / tidal_api.rs sin cambios.
  - Schema migration aditiva (classical_editorial nueva, no DROP/ALTER).
  - ExplorePage / Sidebar / Player / Stats / Galaxy / Live painting / Share link / Scrobbling / TIDAL favorites no tocados (excepto si se añade Editor's Choice star, que es aditivo).
- [ ] `cargo check`, `cargo clippy --release --lib --no-deps`, `cargo build --release`, `cargo test --release --lib classical::`, `tsc --noEmit`, `npm run build` clean.
- [ ] Tests unitarios:
  - `classical::search` tokenizer ≥ 15 cases.
  - `classical::editorial` snapshot parse + lookup.
  - `classical::search` end-to-end con OpenOpus snapshot fixtures (no MB).
  - Acceptance test: "Op. 125" → Beethoven 9 work_mbid resuelto via composer + catalog match.

---

## Riesgos específicos de Phase 5

| Riesgo | Mitigación |
|---|---|
| Tokenizer ambiguo (Mozart Don Giovanni) | Greedy parse + lista de works canónicos por composer; si no resuelve, degrada a búsqueda libre con composer prefijado. |
| Editorial seeds envejecen (Tidal cataloga nueva canónica) | Override manual D-021 + snapshot regenerable; commit history del snapshot es trazable. |
| Snapshot editorial.json grande | 50-80 entries × ~500 bytes c/u = ~30 KB. Acceptable en bundle. |
| MQA / no-Tidal complica search ranking | Search reusa `qualityScore` de Phase 4; recordings sin Tidal aparecen al fondo (qualityScore=0). |
| Listening guides en directorio con permisos | Read-only del filesystem; si no existe, render oculto. |
| Search performance (60 recordings × tokenize per-call) | Tokenize antes del fan-out; cache de planes por query hash (Dynamic 4h). |
| Editor's Choice star colisiona con UI existente | Star solo visible cuando `is_editors_choice=true`. Tracks no-classical no tienen `is_editors_choice`. UI default unchanged. |

---

## Próximos pasos al finalizar Phase 5

- Si gate ✅ → Phase 6 (Personal listening integration + Wikidata + related composers + browse conductor/orquesta).
- Phase 5 cierra la **paridad cualitativa** con AMC. Tras esto, la única ventaja AMC sería curación profesional editorial de ~1.2M recordings — terreno donde mySone Classical compite con autoría visible + listening guides community-driven + override transparent + bit-perfect.

---

## Checklist supervisor (al recibir entregables internos)

- [ ] §1 estilo (llaves) verificado en cada archivo nuevo.
- [ ] §10 regresión: audio path intacto.
- [ ] PROGRESS.md, CHECKPOINTS.md, DECISIONS.md updated.
- [ ] Tests unitarios cubren tokenizer + editorial parse + lrc reader + search end-to-end.
- [ ] Editorial seeds: cada uno tiene fuente o nota defendible. NO grabaciones inventadas.
- [ ] Wikipedia atribución verificable en runtime (CC BY-SA + link).
