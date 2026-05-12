# Phase 9 — Hub IA reconstruction (modelo Idagio + Apple Classical + USP "About this work")

**Status**: 📝 plan listo, pendiente ejecutar tras Phase 8.9.
**Estimación**: ~58h (B 36h + C 22h, sin seeds editoriales que van en Phase 10).
**Decisiones nuevas**: D-040 + D-042 + D-043 + D-044 + D-045.
**Bloqueado por**: Phase 8.9 cerrada (los fixes de bug 3 fallback tocan los mismos files).

---

## Por qué existe Phase 9

Pedro (2026-05-04), tras revisar la build dev:

> "no se, lo veo todo bastante mal. el catalogo por compositor lo has puesto directamente con los movimientos, debería de salir operas / conciertos / overturas / etc... revisa como lo hace apple music classical o idagio. Hablalo con tu equipo y darme un solucion que se asemeje a estas plataformas, eso es lo que quiero. No lo hagas con prisa, hazlo bien"

Y después:

> "ademas estaba bien poner informacion sobre la propia obra, como punto distintivo"

Audit completo del classical-musicologist confirma 12 gaps mySone vs Apple/Idagio (registrado en D-039). Los principales:

- Movimientos colándose como peer de works parent (cubierto por Phase 8.9 A5).
- Buckets de works en ComposerPage insuficientes (set actual `WorkType` mezcla niveles, ~40% cae en "Other").
- Falta de tabs claros About / Works / Albums / Popular (Idagio's identity feature).
- Sin sub-buckets dentro de bucket grande (Chopin Keyboard 60+ obras → debería sub-dividirse).
- Editor's Choice no tiene banner separado (Apple lo destaca).
- WorkPage sin sección editorial profunda — Apple no la tiene fuerte, Idagio tampoco; este es el USP de mySone (D-044).

---

## B — Re-arquitectura ComposerPage (~36h)

### B.1 Modelo de datos

#### B9.1 — `WorkBucket` enum + mapping rules (D-040)

**File**: `src-tauri/src/classical/types.rs` + nuevo `src-tauri/src/classical/buckets.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum WorkBucket {
    Stage,
    ChoralSacred,
    Vocal,
    Symphonies,
    Concertos,
    Orchestral,
    Chamber,
    Keyboard,
    SoloInstrumental,
    FilmTheatre,  // condicional
    Other,        // último, condicional
}
```

`Work.bucket: WorkBucket` aditivo en `types.rs`. Calculado backend en `build_work_fresh` y `build_composer_works_fresh`. NO se quita `work_type` ni `genre`.

**Estimación**: 4h (enum + tests roundtrip serde).

#### B9.2 — `bucket_for(work_type, genre, p136, title)` heurística + override editorial

**File**: `src-tauri/src/classical/buckets.rs` (NEW).

Algoritmo (cascade):
1. Snapshot override (`editorial.json` o `editorial-extended.json` `bucket` field) → bucket directo.
2. Wikidata P136 keywords → bucket directo.
3. `work_type` mapping:
   - `Symphony` → Symphonies (caveat: sinfonías corales como Beethoven 9, Mahler 2 → siguen Symphonies).
   - `Concerto` → Concertos.
   - `Opera`, `Ballet` → Stage.
   - `Cantata` → si título empieza por `BWV [1-200]` o contiene "Sacred"/"sacra" → ChoralSacred; else Vocal.
   - `Mass` → ChoralSacred.
   - `Lieder` → Vocal.
   - `Sonata` → si título contiene "for piano|harpsichord|organ" → Keyboard; si "for violin|cello|flute|guitar" → SoloInstrumental (solo si NO menciona piano accompanist); si menciona piano + otro instrumento → Chamber. Default ambiguous → Chamber.
   - `StringQuartet` → Chamber.
   - `Suite` → si "for orchestra"/"orchestral" → Orchestral; "for cello"/"for violin" → SoloInstrumental; "for keyboard"/"for harpsichord"/"for piano" → Keyboard; default → Orchestral.
   - `Etude` → Keyboard salvo título indique "for guitar"/"for violin" → SoloInstrumental.
   - `Other` → fallback heurística título.
4. Heurística título (regex):
   - `r"^(Missa|Requiem|Te Deum|Stabat Mater|Magnificat)\b"` → ChoralSacred.
   - `r"\bOverture\b"` → Orchestral.
   - `r"\bSymphonic Poem\b|\bTone Poem\b"` → Orchestral.
   - `r"\bConcerto\b"` → Concertos.
   - `r"\bSerenade\b"`: orquesta (>3 instrumentos) → Orchestral; chamber → Chamber.
   - `r"\b(Variations|Préludes?|Ballades?|Mazurkas?|Polonaises?|Nocturnes?)\b"` → Keyboard si compositor canónico de teclado.
5. Default → `Other`.

Tests deterministic (canon):
- Beethoven 9 → Symphonies.
- Bach Pasión San Mateo (BWV 244) → ChoralSacred.
- Schubert Winterreise → Vocal.
- Chopin Étude Op.10 No.1 → Keyboard.
- Bach Cellosuite BWV 1007 → SoloInstrumental.
- Stravinsky Petrushka (suite) → Orchestral.
- Stravinsky Petrushka (ballet completo) → Stage.

**Estimación**: 3h (heurística + 15 tests canon).

#### B9.3 — `list_classical_composer_buckets` command + cache (D-041)

**File**: `src-tauri/src/commands/classical.rs` + `catalog.rs`.

Nuevo command:
```rust
#[tauri::command]
pub async fn list_classical_composer_buckets(
    state: State<'_, AppState>,
    composer_mbid: String,
) -> Result<ComposerBuckets, SoneError>;
```

Shape:
```rust
pub struct ComposerBuckets {
    pub composer_mbid: String,
    pub buckets: Vec<BucketSummary>,
    pub total_works: u32,
    pub mb_total: u32,
    pub canonical_works_loaded: u32,
}

pub struct BucketSummary {
    pub bucket: WorkBucket,
    pub label_en: String,
    pub label_es: String,
    pub total_count: u32,
    pub top_works: Vec<WorkSummary>,  // hasta 12
    pub sub_buckets: Option<Vec<SubBucketSummary>>,
}
```

Internamente:
1. Llama `list_works_by_composer(mbid, None, 0)` (cache 7d, normalmente warm).
2. Si `mb_total > 100`, **carga TODAS las páginas en serial** (1 req/s MB) y cachea el merged result.
3. Aplica `bucket_for(...)` a cada work, agrupa.
4. Ordena buckets por orden canónico D-039.
5. Cada bucket: top_works = primeras 12 ordenadas por `(popular desc, catalog asc, title asc)`.
6. Sub-buckets condicionales (sólo si `total_count > 12`): tabla per bucket (Concertos→Piano/Violin/Cello, Chamber→Quartets/Trios/Quintets, Keyboard→Sonatas/Variations/Études/Character pieces).
7. Cachea `composer_buckets:v1:{mbid}` con TTL 7d StaticMeta.

**Cost cold-load**:
- Bach 11 MB pages × 1.05s = 11s.
- Mozart 7 pages = 7s.
- Beethoven 4 pages = 4s.
- Otros < 2s.
- Pre-warm canon (D-026) ya cubre top 30 composers tras 12s post-boot.

**Mitigación**: emite evento Tauri `classical:composer-buckets-loading` con `{ composer_mbid, page_done, page_total }` para spinner progresivo (siguiendo patrón B8.1 search streaming).

**Estimación**: 4h.

#### B9.4 — `list_classical_works_in_bucket` command (D-041)

**File**: `src-tauri/src/commands/classical.rs`.

```rust
#[tauri::command]
pub async fn list_classical_works_in_bucket(
    state: State<'_, AppState>,
    composer_mbid: String,
    bucket: WorkBucket,
    sub_bucket: Option<String>,
    sort: Option<String>,  // "Catalog" | "Date" | "Alphabetical"
    offset: u32,
    limit: u32,
) -> Result<WorksPage, SoneError>;
```

Filtra el set ya cargado en cache `composer_buckets:v1:{mbid}`. NO requiere MB calls extra — opera sobre cache.

**Estimación**: 2h.

#### B9.5 — Multi-page MB browse fetcher

**File**: `src-tauri/src/classical/providers/musicbrainz.rs`.

Helper `browse_all_works_by_artist(artist_mbid)` que itera paginating MB hasta agotar.

**Estimación**: 3h.

### B.2 UI ComposerPage rediseñada (D-042 + D-043)

#### F9.1 — Tabs ComposerPage + routing

**File**: `src/components/classical/ComposerPage.tsx` + `src/hooks/useNavigation.ts` + `src/App.tsx`.

Tabs: **About** / **Works** / **Albums** / **Popular**.

Routing: `classical://composer/{mbid}?tab=works` (default `?tab=about` cuando entry desde browse; `?tab=works` cuando entry desde search).

`useNavigation`:
```ts
navigateToClassicalComposerTab(mbid: string, tab: "about" | "works" | "albums" | "popular");
```

**Estimación**: 3h.

#### F9.2 — `ComposerWorksTab` + `BucketSection` + sub-buckets

**Files**: NEW `src/components/classical/ComposerWorksTab.tsx`, `BucketSection.tsx`, `SubBucketChips.tsx`.

Estructura:
```
┌─ Tab: Works ────────────────────────────────────────────┐
│  ── Essentials ───────────────  (cherry-picked, 4-8)    │
│  [card] [card] [card] [card]                            │
│                                                          │
│  ── Symphonies (9) ─────────────────  [View all (9)]    │
│  [card] [card] ... 12 cards                             │
│                                                          │
│  ── Concertos (7) ──────────────────  [View all (7)]    │
│  Filter: [All] [Piano (5)] [Violin (1)] [Triple (1)]   │
│  [card] [card] ... up to 12                             │
│                                                          │
│  ── Chamber (16) ───────────────────  [View all (16)]   │
│  ...etc...                                               │
└──────────────────────────────────────────────────────────┘
```

Bucket headers: `<h2>{label} <span>{count}</span></h2>` + "View all" cuando count > 12.
Sub-bucket chips inline: si bucket tiene sub_buckets calculados, chips arriba del grid filtran client-side los 12 mostrados.
Buckets vacíos: NO se renderizan.
`Other` y `FilmTheatre`: al final, plegados (`<details>`) cuando count > 0.

**Estimación**: 5h.

#### F9.3 — `BrowseComposerBucket` page (drill-down)

**File**: NEW `src/components/classical/BrowseComposerBucket.tsx`.

Ruta: `classical://composer/{mbid}/bucket/{bucket}` (e.g. `/bucket/Concertos`).

Header:
```
┌─ Beethoven · Concertos (7) ──────────────────────────────┐
│ ← Back                                                   │
│  Filter: [Piano (5)] [Violin (1)] [Triple (1)]          │
│  Sort:   [Catalog ▾]  Catalog | Date | Title            │
│                                                          │
│  [WorkSummaryCardExpanded] · Concerto No. 1 · Op. 15   │
│      C major · 1795 · ~38 min · 32 recordings           │
│  ...                                                    │
└──────────────────────────────────────────────────────────┘
```

`WorkSummaryCardExpanded`: variante con catalog + key + year + recording count.

Filter chips secundarios:
- "With Editor's Choice" (filtro `has_editors_choice`).
- Period of composition (decade chips cuando spread > 30 años).

**Estimación**: 4h.

#### F9.4 — Tab "Popular" reusing Phase 6 stats

**File**: `src/components/classical/ComposerPage.tsx::PopularTab`.

Filtra `top_classical_works` Phase 6 stats por composer. Cuando 0 plays → fallback a "Hub-popular" (placeholder o curado editorial).

**Estimación**: 2h.

#### F9.5 — Tab "Albums" portado/integrado

**File**: `src/components/classical/ComposerPage.tsx::AlbumsTab`.

Reusa lógica de `ClassicalArtistPage.tsx` Phase 6 (que tiene browse-by-conductor con groupings) adaptada para artist=composer.

**Estimación**: 2h.

### B.6 Defensa anti-movements en frontend

Como segunda defensa: el render filtra works cuyo `bucket = Other` Y `title` matchea regex movement (D-048). Logs warning, no muestra.

---

## C — Re-arquitectura WorkPage (~22h, sin seeds editoriales)

### C.1 Anatomía rediseñada (D-042)

8 secciones canónicas:
1. Header (título, composer link, catalog + key + year + duration + movements + recording count + best-quality badge).
2. **Editor's Choice banner separado** (no inline en lista).
3. **About this work** (USP — sección NUEVA, prosa larga).
4. Listening Guide (LRC, Phase 5).
5. Movements (Phase 1).
6. Popular Recordings (top 8 por quality+popularity).
7. All Recordings (con filters/sort, paginada).
8. Sidebar derecho desktop ≥ 1280px (related/cross/performers).

### C.2 Sub-tasks

#### B9.6 — `editorial-extended.json` schema + provider (D-044 + D-045)

**Files**: NEW `src-tauri/data/editorial-extended.json`, extender `src-tauri/src/classical/editorial.rs`.

Schema v2:
```json
{
  "schema_version": 2,
  "works": [
    {
      "work_mbid": "9c9a3b5b-...",
      "composer_mbid": "1f9df192-...",
      "match_titles": ["Symphony No. 9", "Choral Symphony"],
      "bucket": "Symphonies",
      "editor_note": "...",
      "extended": {
        "origin": "...",
        "premiere": "...",
        "highlights": "...",
        "context": "...",
        "notable_recordings_essay": "...",
        "sources": [
          {"kind": "wikipedia", "url": "...", "license": "CC BY-SA"},
          {"kind": "editor", "name": "mySone team"}
        ],
        "language": "en",
        "translations": {
          "es": { "origin": "...", ... }
        }
      },
      "editors_choice": {
        "recording_mbid": "...",
        "rationale": "..."
      }
    }
  ]
}
```

`editorial.rs` extiende:
- Backward compat v1 (`editor_note` solo).
- Nuevo `lookup_extended(work_mbid) -> Option<ExtendedNote>`.
- Locale fallback: `extended.translations[locale]` → `extended` (default lang) → None.

Tamaño: 200 works × 1200 palabras × 5 bytes ≈ 1.2 MB.

**Estimación**: 3h.

#### B9.7 — Multi-locale + lookup extended

**File**: `src-tauri/src/classical/editorial.rs`.

Tests: lookup with fallback locale, missing translation, schema_version detection.

**Estimación**: 2h.

#### F9.6 — `AboutThisWork` component

**File**: NEW `src/components/classical/AboutThisWork.tsx`.

Renderiza markdown-light (negritas, links, párrafos). Cada sección colapsable individualmente; "Origin" y "Highlights" expanded por default. Source attribution visible al final ("Editor's notes by mySone team. Wikipedia material under CC BY-SA. Wikidata under CC0.").

Cuando `extended` no existe: fallback al `editor_note` Phase 5 + Wikipedia summary actual (no regresión).

**Estimación**: 4h.

#### F9.7 — WorkPage layout reorder + Editor's Choice banner separado

**File**: `src/components/classical/WorkPage.tsx`.

Refactor del orden de secciones según D-042. Editor's Choice extraído de la lista a banner propio prominente.

**Estimación**: 3h.

#### F9.8 — Popular Recordings sub-section

**File**: `src/components/classical/WorkPage.tsx::PopularRecordings`.

Top 8 ordenadas por `(quality_score desc, popularity desc, has_editors_choice desc)`.

**Estimación**: 2h.

#### F9.9 — Sidebar desktop ≥ 1280px

**File**: NEW `src/components/classical/WorkSidebar.tsx`.

Related works (Wikidata + heurística), cross-version comparison (Phase 6 D-022), performers you follow.

Responsive: collapse en ventanas estrechas.

**Estimación**: 4h.

#### F9.10 — Movements + Listening Guide visibility refinement

**File**: `src/components/classical/WorkPage.tsx`.

Solo mostrar Movements cuando `movements.len() > 0`. Listening Guide cuando guide existe (Phase 5 mecanismo ya implementado).

**Estimación**: 1h.

#### Editorial seeds POC (3 obras canon dentro de Phase 9.C)

Para validar el USP antes de invertir en Phase 10 entera, escribir **3 obras canon** con extended editorial (Beethoven 9, Bach Goldberg, Mozart Requiem). Sirve de proof-of-concept para Pedro validar que la sección "About this work" es lo que pidió.

**Estimación**: 6h (2h × 3 obras = ~30 min escritura + 1h fact-check + 30 min translation ES).

#### Tests + QA

**Estimación**: 3h.

---

## Acceptance criteria

### Phase 9.B (ComposerPage)

- ComposerPage Bach renderiza tabs About/Works/Albums/Popular sin crash.
- Tab Works muestra ≥ 6 buckets visibles para Bach; el bucket Symphonies NO aparece (Bach no escribió ninguna).
- Beethoven: bucket Symphonies muestra exactamente 9 works, ordenadas Op. 21..Op. 125.
- Click "View all" en cualquier bucket → drill-down funcional, sub-bucket chips operan, sort funcional.
- Movements NO aparecen como entradas top-level en ningún bucket (defensa A5 + frontend).
- Cargar Bach buckets cold cache: < 15s (11 MB pages × 1.05s + parse).
- Cero regresión §10.

### Phase 9.C (WorkPage)

- WorkPage Beethoven 9 muestra Editor's Choice banner separado.
- About this work renderiza para los 3 obras POC (Beethoven 9, Bach Goldberg, Mozart Requiem) con ≥ 4 secciones.
- Sin extended editorial → fallback a `editor_note` + Wikipedia summary, sin sección About vacía.
- Click recording sintetizada `TidalDirectInferred` con badge naranja → reproduce sin crash, tooltip muestra query usada.
- Filters/sort Phase 4 siguen funcionando idénticos.
- Cero regresión §10.

### Validation gate

- **9-B → 9-C**: Pedro abre 5 composers (Bach, Beethoven, Mozart, Wagner, Glass) y confirma que la presentación buckets es lo que pidió. Si NO está conforme, parar antes de WorkPage redesign.
- **9 → 10**: Pedro valida WorkPage rediseñada con los 3 POC. Si la presentación no satisface el USP, pivot del scope D antes de invertir 160h.

---

## Files que se tocan (summary)

Backend:
- `src-tauri/src/classical/types.rs` — `WorkBucket` enum, `Work.bucket` field.
- `src-tauri/src/classical/buckets.rs` (NEW) — `bucket_for` heurística + tests.
- `src-tauri/src/classical/catalog.rs` — `build_work_fresh` set bucket, `ComposerBuckets` shape, `build_composer_buckets`, `list_works_in_bucket`, multi-page browse.
- `src-tauri/src/classical/editorial.rs` — `lookup_extended` + locale fallback + schema v2.
- `src-tauri/src/classical/providers/musicbrainz.rs` — `browse_all_works_by_artist`.
- `src-tauri/data/editorial-extended.json` (NEW).
- `src-tauri/src/commands/classical.rs` — 2 nuevos commands.
- `src-tauri/src/lib.rs` — invoke_handler.

Frontend:
- `src/types/classical.ts` — `WorkBucket`, `ComposerBuckets`, `ExtendedNote`.
- `src/api/classical.ts` — 2 nuevos wrappers.
- `src/components/classical/ComposerPage.tsx` — tabs, refactor.
- `src/components/classical/ComposerWorksTab.tsx` (NEW).
- `src/components/classical/BucketSection.tsx` (NEW).
- `src/components/classical/SubBucketChips.tsx` (NEW).
- `src/components/classical/BrowseComposerBucket.tsx` (NEW).
- `src/components/classical/WorkPage.tsx` — refactor 8 secciones.
- `src/components/classical/AboutThisWork.tsx` (NEW).
- `src/components/classical/WorkSidebar.tsx` (NEW).
- `src/hooks/useNavigation.ts` — `navigateToClassicalComposerTab`, `navigateToClassicalBucket`.
- `src/App.tsx` — routing extendido.

---

## Estado actual

📝 **Plan completo. Pendiente delegar a `sone-backend-engineer` (B9.1-B9.5 + B9.6-B9.7) + `sone-frontend-engineer` (F9.1-F9.10) + `classical-musicologist` (3 POC editorial)** vía classical-supervisor con DESIGN-OK confirmado por carta blanca de Pedro (2026-05-04).

Bloqueado por Phase 8.9 (los fixes A1-A5 cierran bugs reportados por Pedro y tocan los mismos files que Phase 9.B/C).
