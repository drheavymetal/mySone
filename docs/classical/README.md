# SONE Classical — README operativo

**Estado**: V1 completo (todas las phases 0-6 cerradas, 2026-05-02).
**Branch**: `soneClassical`.
**Audiencia de este doc**: cualquier dev / operador que abre el repo y quiere entender qué se construyó, dónde vive, cómo probarlo, cómo extenderlo.

> El doc maestro de **diseño** vive en `/CLASSICAL_DESIGN.md` (prescriptivo: qué se quería). Este doc es **descriptivo**: qué existe ahora.

---

## TL;DR

**SONE Classical** es un Hub de música clásica integrado dentro de mySone (fork audiophile de lullabyX/sone). Reemplaza la experiencia "Apple Music Classical" sobre Linux + Tidal + grafo abierto (MusicBrainz / Wikipedia / OpenOpus / Wikidata) — bit-perfect, sin telemetría, con curación auditable.

USPs entregados:
- **Comparación de calidad** entre recordings de la misma obra (filter/sort por sample-rate, Hi-Res, Atmos, MQA penalty).
- **Bit-perfect awareness** en el player (`route_volume_change` + writer guard intactos).
- **Movement-aware player** (work title persistente, "II / IV", "Attacca →").
- **Curación editorial** auditable: 48 work seeds + 15 composer notes embebidos, override manual.
- **Search clásico** con tokenizer (catálogo BWV/K/Op + key + year + composer).
- **Wikidata-powered related composers** (genre overlap + birth proximity).
- **Browse-by-conductor** con grouping por parent work.
- **Personal stats** classical-only (top works, top composers, recording comparison, discovery curve).

Todo aditivo sobre la app existente: cero modificaciones al audio engine, al routing de volumen, ni al scrobbling core.

---

## Cómo se accede

Desde la app:
- **Explore** → click en pill **"Classical Hub"** (D-001).
- O desde el player: badge **"View work"** cuando un track clásico tiene `work_mbid` resuelto.
- O desde **Stats** → tab **Classical**.

URL scheme interno (`apiPath` del routing aditivo en `App.tsx::renderView`):

| URL | Página |
|---|---|
| `classical://hub` | ClassicalHubPage (Listen Now / Browse / Search / Library tabs) |
| `classical://work/{mbid}` | WorkPage |
| `classical://composer/{mbid}` | ComposerPage |
| `classical://browse/composers` | BrowseComposers |
| `classical://browse/periods` | BrowsePeriods |
| `classical://browse/genres` | BrowseGenres |
| `classical://era/{Era}` | BrowseEra |
| `classical://search?q=...` | ClassicalSearch |
| `classical://library{,/{facet}}` | ClassicalLibrary (work/recording/composer/performer) |
| `classical://artist/{mbid}` | ClassicalArtistPage (browse-by-conductor) |
| `classical://compare/{workMbid}` | ClassicalRecordingComparison |

---

## Arquitectura

Detalle vivo: `docs/classical/ARCHITECTURE.md`. Resumen:

- **Backend** (`src-tauri/src/classical/`): un `CatalogService` (`catalog.rs`) que orquesta cinco providers (`musicbrainz.rs`, `wikipedia.rs`, `tidal.rs`, `openopus.rs`, `wikidata.rs`) detrás del trait `ClassicalProvider`. Cache via `DiskCache` con tiers (Static, Dynamic). Tauri commands en `commands/classical.rs`.
- **Frontend** (`src/components/classical/`): 18 componentes, todos dependientes solo de `src/api/classical.ts` (wrappers Tauri tipados). Routing aditivo en `App.tsx`. Tipos en `src/types/classical.ts`.
- **Stats DB**: 3 migraciones aditivas — columna `plays.work_mbid`, tabla `classical_favorites` (Phase 1), tabla `classical_editorial` (Phase 5). Schema viejo intacto.

Diagrama de capas:

```
┌─────────────────────────────────────────────────────┐
│ React UI (src/components/classical/)                │
│ ├── ClassicalHubPage  (Listen Now / Browse / ...)   │
│ ├── ComposerPage      (hero + bio + works + related)│
│ ├── WorkPage          (movements + recordings)      │
│ ├── ClassicalSearch                                 │
│ ├── ClassicalLibrary  (favorites grid)              │
│ ├── ClassicalArtistPage (browse-by-conductor)       │
│ └── ClassicalRecordingComparison                    │
└──────────────────────┬──────────────────────────────┘
                       │ Tauri invoke (~28 commands)
┌──────────────────────▼──────────────────────────────┐
│ commands/classical.rs                               │
│ State<AppState> → state.classical.*                 │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│ classical::CatalogService (catalog.rs)              │
│ ├── DiskCache (StaticMeta 7d/30d, Dynamic 4h/24h)   │
│ ├── StatsDb (Arc — read-only for the catalog)       │
│ └── 5 providers + EditorialProvider snapshot        │
└────┬─────┬──────────┬──────────┬─────────┬──────────┘
     │     │          │          │         │
     ▼     ▼          ▼          ▼         ▼
   MB   Wikipedia   Tidal    OpenOpus   Wikidata
 (1r/s) (REST)    (ISRC+    (snapshot   (SPARQL,
                  search)   embedded)   1.5s/query)
```

Bit-perfect contract: el módulo classical NUNCA toca `audio.rs`, `hw_volume.rs`, `signal_path.rs`, ni `tidal_api.rs::get_stream_url`. La playback fluye por la pipeline existente sin modificación.

---

## Cómo el operador lo prueba

Tras `cargo build --release` + `npm run build`, ejecutar la app con auth Tidal viva.

### Smoke tests Phase 1-2 (catálogo)

1. Explore → click pill "Classical Hub" → Listen Now con composers featured.
2. Click en cualquier composer → ComposerPage carga bio + works.
3. Click en cualquier work → WorkPage carga movements + recordings.
4. Click en play en una row con badge 🟢 ISRC-bound → reproduce sin error.

### Smoke tests Phase 3 (player)

5. Mientras suena un track clásico: el PlayerBar muestra "Composer · Work title" arriba del track title (cuando `work_mbid` resuelve).
6. Movement indicator "II / IV" cuando el work tiene movements parseados.
7. "Attacca →" indicator en movements con flag editorial (raros en V1, requiere editorial seed).

### Smoke tests Phase 4 (quality USP)

8. WorkPage → header muestra "Best available 24/192 HIRES_LOSSLESS · ATMOS" cuando aplique.
9. Click en "Best available" badge → activa filter Hi-Res only.
10. Filter chips (Hi-Res only / Atmos / ≥96kHz / ≥192kHz / Sin MQA / Year ≥) funcionan.
11. Sort dropdown (Popularity / Year / Audio quality / Conductor) funciona.
12. "Refresh quality" button re-prueba top-20 recordings.
13. PlayerBar cuando bit-perfect+exclusive: badge "BIT-PERFECT" (verde).

### Smoke tests Phase 5 (editorial + search)

14. Hub Listen Now → sección "Editor's Choice" con 12 picks curados.
15. Click en un pick → navega a Search con composer + catalogue prerellenado.
16. Search "Op. 125" → Beethoven Symphony 9 first hit.
17. Search "Beethoven 9 Karajan 1962" → Symphony 9 outranks Symphony 5.
18. Search "BWV 1052" → tokenizer reconoce BWV catalog.
19. Star icon en RecordingRow toggles user override de Editor's Choice.
20. Editorial note callout en WorkPage cuando hay seed.

### Smoke tests Phase 6 (personal + Wikidata + browse-by-conductor)

21. Tras escuchar varios tracks clásicos: Hub Listen Now → sección "Recently played" + "Your top works".
22. Hub → tab Library → 4 facets (Works/Recordings/Composers/Performers) + overview banner.
23. Heart icon en WorkPage / ComposerPage → toggle save/unsave (verificable en Library tab).
24. ComposerPage → sección "Related composers" (~5-10 entries vía Wikidata) en composers canon (Beethoven, Mozart, Bach, Brahms).
25. Click en conductor name dentro de RecordingRow → navega a ClassicalArtistPage con discografía agrupada por work.
26. WorkPage cuando ≥ 2 versiones jugadas: link "X versions you've played" → navega a ClassicalRecordingComparison.
27. Stats → tab Classical → overview banner + Top works + Top composers + Discovery section.

### Verificar bit-perfect intacto

Reproducir un track clásico 24/96+ con bit-perfect ON + exclusive ALSA. Verificar:
- `signal_path` reporta `bitPerfect=true` y `exclusiveMode=true`.
- Volumen del slider está locked (HW only o disabled).
- Player badge muestra "BIT-PERFECT".

### Verificar attacca gapless (instrumented manual)

Documentado en `phase-3-player-gapless.md` "QA manual" — reproducir Beethoven 5 III→IV / Mahler 3 V→VI / Bruckner 8 III→IV con bit-perfect ON, observar gap < 50 ms.

---

## Cómo extenderlo

### Añadir un nuevo provider

1. Crear `src-tauri/src/classical/providers/<source>.rs`.
2. Implementar el trait `ClassicalProvider` (`enrich_composer / _work / _recording` best-effort).
3. Si requiere rate-limit propio, instanciar un `Mutex<Instant>` interno (ver `WikidataProvider` para el patrón).
4. Registrar el módulo en `providers/mod.rs` y construirlo en `mod.rs::build_catalog_service`.
5. Inyectar en `CatalogService` y usar.

### Añadir una nueva query stats classical-aware

1. Añadir el shape (`SomeQuery`) y el método `pub fn some_query(...)` en `stats.rs`. Filtrar por `work_mbid IS NOT NULL` para que sólo plays clásicos cuenten.
2. Añadir un test en `stats::classical_tests` con fixtures.
3. Exponer un wrapper en `CatalogService` que mapee errores de SQLite a `SoneError`.
4. Crear un Tauri command en `commands/classical.rs` que invoque el wrapper.
5. Registrar en `lib.rs::invoke_handler`.
6. Añadir el wrapper en `src/api/classical.ts` + el type mirror en `src/types/classical.ts`.

### Añadir un editorial seed

Editar `src-tauri/data/editorial.json`. Schema:

```json
{
  "schema_version": 1,
  "works": {
    "{work_mbid}": {
      "editors_choice": {
        "recording_mbid": "...",
        "tidal_track_id": null,
        "conductor": "...",
        "performer": "...",
        "year": 1962,
        "label": "...",
        "note": "..."
      },
      "editor_note": "..."
    }
  },
  "composers": {
    "{composer_mbid}": { "editor_note": "..." }
  }
}
```

Cada seed debe ser **defendible**: cita la fuente cuando es debatible (Gramophone Hall of Fame, Penguin Guide rosette, BBC Building a Library). Nunca inventes grabaciones.

### Añadir un nuevo kind de favorite

1. Validar en `is_valid_favorite_kind` (catalog.rs).
2. Mirror en `ClassicalFavorite["kind"]` types (frontend).
3. Renderizar UI en `ClassicalLibrary.tsx` (sub-tab) y wire heart icon donde aplique.

### Añadir un command Tauri nuevo

1. Function async en `commands/classical.rs` con `#[tauri::command]` + `state: State<'_, AppState>`.
2. Registrar en `lib.rs::invoke_handler`.
3. Wrapper tipado en `src/api/classical.ts`.

---

## Limitaciones conocidas (V1)

| Limitación | Razón | Phase 7+ candidato |
|---|---|---|
| top_classical_composers groupea por `artist_mbid` (performer), no composer real | Refactor exige backfill `composer_mbid` en plays históricos | Sí |
| Wikidata online-only | Sin snapshot offline embebido | Maybe |
| Editorial cubre 48 works + 15 composers (canon mayor) | Curación inicial conservadora | Community-driven |
| Pagination MB capped at 100 works | MB browse limit | Sí |
| RecordingRow / ArtistLinks no tienen heart icon | UI no trigger Library facets recording/performer todavía | Sí |
| Listening guides reader read-only | Schema soporta más; UI básica | Sí |
| Mobile / Android | Out of scope V1 (D-003) | No |

---

## Documentación viva

| Doc | Función |
|---|---|
| `/CLASSICAL_DESIGN.md` | Doc maestro de diseño (prescriptivo). Léelo antes de tocar el módulo. |
| `docs/classical/PROGRESS.md` | Estado por phase + summary global del proyecto completo. |
| `docs/classical/CHECKPOINTS.md` | Granular append-only — punto de retomada tras context reset. |
| `docs/classical/DECISIONS.md` | D-001..D-026 con contexto + alternativas + trade-offs. |
| `docs/classical/ARCHITECTURE.md` | Síntesis técnica viva del código que existe. |
| `docs/classical/phase-N-*.md` | Plan detallado de cada phase (sub-tasks, scope refinements). |
| `docs/code-style.md` | Estilo obligatorio (llaves siempre, calidad sobre velocidad). |

---

## Cómo retomar el trabajo

1. Lee `/CLASSICAL_DESIGN.md` (refrésalo).
2. Lee `docs/classical/PROGRESS.md` (estado actual).
3. Lee `docs/classical/CHECKPOINTS.md` (último checkpoint = punto de retomada).
4. Lee `docs/classical/DECISIONS.md` (no re-decidas).
5. Lee `docs/code-style.md`.

---

## Builds + tests one-liner

```bash
# Backend
cd src-tauri
cargo check --release
cargo build --release
cargo clippy --release --lib --no-deps
cargo test --release --lib

# Frontend
cd ..
npx tsc --noEmit
npm run build
```

Estado esperado: todo verde, 14 warnings clippy idénticos al baseline pre-classical, 118/118 tests classical+stats pasan.

---

## Para commit consolidado

El proyecto deja:
- Branch `soneClassical` con tree dirty.
- Audio path verificable intacto (`git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → vacío).
- Builds + tests green.
- 26 decisiones documentadas.
- 6 phases cerradas (todas las del CLASSICAL_DESIGN.md).

Ready to commit como un solo commit / cherry-pick / rebase a master según preferencia del operador.
