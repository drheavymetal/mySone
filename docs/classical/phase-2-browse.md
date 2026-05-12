# Phase 2 — Browse experience: Hub landing + Composer pages + Browse axes

**Status**: 🟡 in_progress (started 2026-05-02)
**Owner**: classical-supervisor (specialists falleciendo silenciosamente — ver D-013).
**Tiempo estimado**: ~70h (de §8 doc maestro).
**Decision gate** (§11): cualquier compositor top-30 OpenOpus → su página → cualquier work → cualquier recording → reproduce. Sin errores. → GO.

---

## Contexto crítico — leer antes de empezar

- **Phase 1 entregó** la pieza atómica: una Work page real al final de `classical://work/{mbid}`. El catálogo backend (CatalogService) ya orquesta MB + Wikipedia + Tidal con cascade matching + cache. Phase 2 añade **descubrimiento** — el camino para que el usuario llegue a esa Work page sin tener que reproducir una grabación clásica primero.
- **D-001** (en `DECISIONS.md`): el Hub vive como sub-modo dentro de Explore, accesible vía pill "Classical Hub" en el header de `ExplorePage`. Setting opcional "Promote to sidebar" se difiere a Phase 5.
- **OpenOpus snapshot** todavía no está en el repo. Phase 2 lo trae como `src-tauri/data/openopus.json` (~5 MB) embedded via `include_bytes!`. La lista del top-N se cura del propio snapshot (ranking por `popular = 1`) con override hand-curated cuando aplique.
- **Bit-perfect contract** (igual que en Phase 1): cero cambios en audio routing.
- **Cero regresión** (§10): los nuevos componentes deben ser aditivos. La pill "Classical Hub" se inserta como sección nueva al inicio de `ExplorePage`; el resto de secciones queda intacto.

---

## Objetivo

Shippeable mínimo: el usuario abre `Explore`, ve la pill "Classical Hub" prominente, hace click, llega a `ClassicalHubPage`. Allí ve Listen Now (placeholder editorial Phase 1), Browse Composers (lista top-30), y puede navegar a cualquier `ComposerPage`. Desde la composer page, abre cualquier work → llega a la WorkPage de Phase 1. Browse axes mínimos (composers, periods, genres) navegables.

Phase 2 NO incluye:
- Search clásico (Phase 5).
- Filters de calidad (Phase 4).
- Editor's Choice manual (Phase 5).
- Listening guides (Phase 5).
- Library facets (Phase 6).
- Browse por conductor / orquesta / soloist (Phase 5).
- Pre-warm agresivo (Phase 6).

---

## Sub-tasks granulares

Cada sub-task tiene un ID corto (Bn = backend, Fn = frontend, Cn = curación musicológica). Se ejecutan en orden con la única excepción de C0 que va antes de B1.

### C0. Curación inicial (musicologist role, executed by supervisor)

**Output**: `src-tauri/data/openopus.json` con la lista canon completa + buckets de era/genre.

OpenOpus expone una API pública (`api.openopus.org`) con tres endpoints útiles:

- `/composer/list/pop.json` — top-popular composers según OpenOpus (35-40 entradas).
- `/composer/list/epoch/{epoch}.json` — composers por era (Medieval ... 20th Century).
- `/work/list/composer/{open_opus_id}.json` — works de un composer (incluyendo `popular: "1" | "0"` flag).

**Plan**: pull el listado pop (top-30) vía un script binario standalone (sigue el patrón de `examples/spike_isrc_coverage.rs`) → guardar el JSON exacto que el provider lee en runtime → embed con `include_bytes!`. El snapshot incluye:

- `composers[]`: { open_opus_id, name, complete_name, birth_year, death_year, epoch, mbid }
- `works[open_opus_id][]`: { id, title, subtitle?, genre, popular, searchterms? }

MBIDs no vienen del API OpenOpus directamente — los llevamos via un mapa hand-curated que linka `open_opus_id` → MB artist MBID. Para top-30 esto es 30 entradas; mantenible.

Period buckets (de §5.1 + §16.4) — usados también por el frontend BrowsePeriods:

| Bucket | Span | OpenOpus epoch tags |
|---|---|---|
| Medieval | -800 | "Medieval" |
| Renaissance | 1400-1599 | "Renaissance" |
| Baroque | 1600-1749 | "Baroque" |
| Classical | 1750-1799 | "Classical" |
| Early Romantic | 1800-1849 | "Early Romantic" |
| Romantic | 1850-1899 | "Romantic" |
| 20th Century | 1900-1929 | "Early 20th C." |
| Post-war | 1930-1959 | "Post-War" |
| Contemporary | 1960-now | "Contemporary" |

Genre taxonomy (§5.1 + browse axes) — taxonomía estándar usada por el frontend BrowseGenres:

`Orchestral · Concerto · Chamber · Solo Instrumental · Vocal · Choral · Opera · Sacred · Stage · Film · Other`

OpenOpus usa los siguientes labels en `work.genre`: "Chamber", "Keyboard", "Orchestral", "Stage", "Vocal", "Other". El provider mapea OpenOpus→nuestro Genre así:

- "Keyboard" → `SoloInstrumental`
- "Orchestral" → `Orchestral`
- "Chamber" → `Chamber`
- "Stage" → `Opera` (sí: Stage en OpenOpus es opera/ballet)
- "Vocal" → `Vocal`
- "Other" → `Other`

### B1. OpenOpus snapshot + provider

**Files**:
- `src-tauri/data/openopus.json` (~5 MB embed; snapshot del API filtrado a top-30 composers + works)
- `src-tauri/src/classical/providers/openopus.rs` (provider stateless)

**API**:

```rust
pub struct OpenOpusProvider {
    snapshot: &'static OpenOpusSnapshot,
}

impl OpenOpusProvider {
    pub fn new() -> Self;
    pub fn top_composers(&self, limit: usize) -> Vec<ComposerSummary>;
    pub fn composers_by_era(&self, era: Era) -> Vec<ComposerSummary>;
    pub fn works_for_composer(&self, mbid: &str) -> Vec<OpenOpusWork>;
    pub fn lookup_composer_by_mbid(&self, mbid: &str) -> Option<ComposerSummary>;
}
```

`ComposerSummary` y `OpenOpusWork` son types nuevos en `types.rs` con shape minimal para grids/lists.

`enrich_composer` impl: si encuentra un MBID → composer match en el snapshot, rellena `era`, `birth.year`, `death.year`, `full_name` cuando MB no los tiene.

**Tests**: 4 fixtures — Bach, Beethoven, Mozart, Glass — verificando lookup por MBID, era classification, works grouping. (1 entry hand-built en el test no carga el snapshot, va contra una tabla mini.)

### B2. CatalogService browse extensions

**Files**: `src-tauri/src/classical/catalog.rs` (extensión).

Nuevos métodos:

```rust
pub fn list_top_composers(&self, limit: usize) -> Vec<ComposerSummary>;
pub fn list_composers_by_era(&self, era: Era) -> Vec<ComposerSummary>;
pub async fn list_works_by_composer(
    &self,
    composer_mbid: &str,
    genre: Option<Genre>,
) -> Result<Vec<WorkSummary>, SoneError>;
```

`list_top_composers` y `list_composers_by_era` son síncronos (lectura de snapshot). `list_works_by_composer` consulta MB browse `work?artist={mbid}&inc=...&limit=100`, parsea, deriva `WorkSummary`, agrupa por work-type/genre cuando el caller lo pide. Cache StaticMeta tier (igual que get_work). Cache key: `classical:composer-works:v1:{mbid}:{genre|all}`.

Critical: la respuesta puede traer ≥ 100 works (Mozart > 600). Strategy:
- Limit MB browse a 100 (paginación queda diferida a Phase 5; los top-30 composers tienen sus works más populares al inicio del ranking MB con `inc=ratings` aproximado).
- Filtra a popular si OpenOpus tiene `popular=1` para esa work.
- WorkSummary preserva `recording_count_estimate: u32` (no precisa, viene de un secondary fetch lazy on Phase 5).

### B3. Tauri commands para browse

**Files**: `src-tauri/src/commands/classical.rs` (extensión).

```rust
#[tauri::command]
pub async fn list_classical_top_composers(state, limit: u32) -> Result<Vec<ComposerSummary>, SoneError>;

#[tauri::command]
pub async fn list_classical_composers_by_era(state, era: String) -> Result<Vec<ComposerSummary>, SoneError>;

#[tauri::command]
pub async fn list_classical_works_by_composer(
    state, composer_mbid: String, genre: Option<String>
) -> Result<Vec<WorkSummary>, SoneError>;
```

Registrar en `lib.rs::run` invoke_handler.

### B4. Tipos derivados (ComposerSummary, WorkSummary)

**Files**: `src-tauri/src/classical/types.rs` (extensión).

```rust
pub struct ComposerSummary {
    pub mbid: String,
    pub open_opus_id: Option<String>,
    pub name: String,
    pub full_name: Option<String>,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    pub era: Era,
    pub portrait_url: Option<String>,
    pub popular: bool,
}

pub struct WorkSummary {
    pub mbid: String,
    pub title: String,
    pub composer_mbid: Option<String>,
    pub composer_name: Option<String>,
    pub catalogue_number: Option<CatalogueNumber>,
    pub key: Option<String>,
    pub work_type: Option<WorkType>,
    pub genre: Option<Genre>,
    pub composition_year: Option<i32>,
    pub popular: bool,
}
```

Frontend mirror: `src/types/classical.ts`.

### F1. Frontend types + API wrappers

**Files**:
- `src/types/classical.ts` (extensión: `ComposerSummary`, `WorkSummary`).
- `src/api/classical.ts` (extensión: `listClassicalTopComposers`, `listClassicalComposersByEra`, `listClassicalWorksByComposer`).

### F2. ExplorePage pill "Classical Hub"

**Files**: `src/components/ExplorePage.tsx` (edit minimal).

Inserción de un PROMINENT pill al inicio del listado de secciones — pre-loading. Click navega via `navigateToClassicalHub()`. Visualmente: gradient + icon + sutil glow.

### F3. Routing extensions

**Files**:
- `src/App.tsx` (extensión: detect `classical://hub`, `classical://composer/{mbid}`, `classical://browse/{axis}` además de `classical://work/{mbid}`).
- `src/hooks/useNavigation.ts` (extensión: `navigateToClassicalHub`, `navigateToClassicalComposer(mbid, name?)`, `navigateToClassicalBrowse(axis)`).

Patrón aditivo igual a Phase 1: switch interno en el branch `case "explorePage":`.

### F4. ClassicalHubPage

**File**: `src/components/classical/ClassicalHubPage.tsx`.

Layout:
- Hero: "Classical Hub" + subtitle "Discover composers, works, and recordings".
- Sub-nav: tabs "Listen Now" (default) | "Browse" | "Search [coming soon Phase 5]" | "Library [coming soon Phase 6]"  → controlled state.
- Listen Now (placeholder Phase 2):
  - "Featured composers" — top-12 OpenOpus → cards con portrait + dates + era.
  - "Listening suggestions" — placeholder ("Coming soon — based on your stats").
  - "Editor's Choice works" — placeholder Phase 5.
- Browse (cuando tab=browse):
  - Quick browse-axes section: 3 cards horizontales:
    - "Browse Composers" → BrowseComposers.
    - "Browse Periods" → BrowsePeriods.
    - "Browse Genres" → BrowseGenres.

### F5. ComposerPage

**File**: `src/components/classical/ComposerPage.tsx`.

Layout (de CLASSICAL_DESIGN.md §7.2):
- Back link.
- Header: portrait + nombre + dates + era badge + bio short.
- Bio long (Wikipedia, con atribución).
- "Essentials" — top-N popular works del composer (chips).
- Sections agrupadas por work-type/genre (Symphonies, Concertos, Piano Sonatas, ...).
- Cada section: scroll horizontal de WorkSummaryCard.
- Click en card → WorkPage Phase 1.

Carga: `useEffect` invoca `getClassicalComposer(mbid)` y `listClassicalWorksByComposer(mbid)` en paralelo. Maneja loading skeleton + error state.

### F6. WorkSummaryCard, ComposerCard, EraBadge

**Files**:
- `src/components/classical/ComposerCard.tsx` — portrait/initial + nombre + dates + era badge.
- `src/components/classical/WorkSummaryCard.tsx` — título + cat number + key + year.
- `src/components/classical/EraBadge.tsx` — chip color-coded por era.

Tema `th-*`, hover scale, accesible. Tamaños consistentes con `MediaCard`.

### F7. BrowseComposers, BrowsePeriods, BrowseGenres

**Files**:
- `src/components/classical/BrowseComposers.tsx` — grid filtrable por era; controles era pills + búsqueda local (filter inline).
- `src/components/classical/BrowsePeriods.tsx` — grid de era cards (9 buckets).
- `src/components/classical/BrowseGenres.tsx` — grid de genre cards.

Cada uno: layout simple, click → drill-down (era → BrowseComposers filtered, genre → "Coming soon Phase 5: works of genre X" placeholder o mostrar composers cuyas works mayoritariamente caen ahí).

### F8. ClassicalHubLanding sub-componentes (HubFeatured, HubBrowseGrid)

Sub-componentes que orquestan F4. Mantienen layout limpio.

---

## Acceptance criteria (de CLASSICAL_DESIGN.md §11 — Phase 2 gate)

- [ ] Pill "Classical Hub" visible en Explore. No regresión de Tidal explore.
- [ ] Hub landing renderiza < 500ms con cache warm (lista de OpenOpus snapshot — síncrono).
- [ ] Click en cualquier compositor top-30 → su page < 3s con cache warm.
- [ ] Composer page muestra ≥ 5 works agrupados.
- [ ] Click en cualquier work → WorkPage funcional (Phase 1) con recordings reales.
- [ ] Cero regresión: Explore, Sidebar, Player default, Stats, Galaxy, Scrobbling, Share link.
- [ ] `cargo check`, `cargo clippy --lib`, `npm run build`, `tsc --noEmit` clean.
- [ ] Tests unitarios cubren: OpenOpus snapshot lookup (3 cases), composer works grouping (2 cases), era era_to_string round-trip (1 case).

---

## Riesgos específicos de Phase 2

| Riesgo | Mitigación |
|---|---|
| OpenOpus snapshot desactualizado | Acepto el coste — la curación es low-frequency. Re-snapshot manual cada release. Documentado en script. |
| OpenOpus API down al construir snapshot | Snapshot vive en repo committed; recompila desde JSON local sin internet. |
| MB browse `work?artist=` devuelve too many works (Mozart > 1000) | Cap a 100 (MB max-page) en el primer fetch. Phase 5 paginará. |
| Composer page lenta porque MB browse + getComposer son secuenciales | Ejecutarlos en paralelo en el frontend (Promise.all). |
| Living composers tienen MB coverage patchy | OpenOpus cubre el listado canon. Phase 5 traerá editor MB para gaps. |
| OpenOpus → MBID matching incorrecto | Mapa hand-curated en el snapshot (top-30) es deterministic. Tests cubren. |

---

## Próximos pasos al finalizar Phase 2

- Si gate ✅ → Phase 3 (Player upgrades + gapless).
- Si gate fallado en latencia: revisar el lazy loading + considerar snapshot pre-bake del top-3 hidrato completo.

---

## Checklist supervisor (al recibir entregables)

- [ ] §1 estilo (llaves) verificado en cada archivo nuevo.
- [ ] §10 regresión: cada modificación a ExplorePage / App.tsx / useNavigation justificada y mínima.
- [ ] §5.2 provider pattern: OpenOpus es nuevo `ClassicalProvider` impl, no command directo.
- [ ] §3.3 cache TTLs: nueva entry para `composer_works:{mbid}` y `top_composers:v1` documentada.
- [ ] D-001 cumplido: pill prominent, no top-level sidebar entry todavía.
- [ ] PROGRESS.md, CHECKPOINTS.md, DECISIONS.md updated.
