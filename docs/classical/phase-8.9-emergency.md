# Phase 8.9 — Emergency bug fixes (post-Pedro 2026-05-04 review)

**Status**: 📝 plan listo, pendiente ejecutar.
**Estimación**: 5-7h.
**Decisiones nuevas**: D-041 (supersedes D-037) + D-047 + D-048.
**Bloquea**: Phase 9 (necesita los bugs A2 + A3 cerrados antes de extender Phase 9.B y 9.C que tocan los mismos files).

---

## Por qué existe esta sub-fase

Pedro arrancó la build dev tras los fixes de la sesión 2026-05-03 (B8.7+B8.8+F8.5+F8.6 bug 1-4 cerrados) y reportó cinco síntomas serios. Análisis del classical-supervisor:

1. **Falsos positivos catastróficos** — Beethoven Op. 83 (lieder vocal) → fallback work-level matchea Eroica I. Allegro (Symphony No. 3 en query "Beethoven 3 Gesänge von Goethe"). Click play reproduce Eroica.
2. **Solo UNA grabación por work** cuando Tidal tiene decenas de grabaciones canónicas.
3. **"Cargar más" no funciona** en ComposerPage — botón aparenta no responder.
4. **Movements colados** en composer page ("II. Andante…" al mismo nivel que works parent).
5. (cuarto reportado por Pedro pero ya cubierto por planning anterior — no requiere fix nuevo).

Los bugs son aislables y la solución no requiere re-arquitectura. Phase 8.9 cierra el "sangrado" antes de Phase 9 (que cambiará la IA de la Hub).

Etiqueta **8.9** porque Phase 8 sigue formalmente abierta (B8.1 streaming + B8.2..B8.6 pendientes); meterlos como sub-bundle dentro de Phase 8 mantiene la trazabilidad.

---

## Sub-tasks

### A1 — Fallback work-level devuelve top-N (no top-1)

**File**: `src-tauri/src/classical/catalog.rs::try_work_level_fallback` (líneas 310-373).
**Decisión**: D-041.

#### Diagnóstico

```rust
// HOY (catalog.rs:310-373):
let outcome = matching::best_work_level_candidate(&results.tracks, ...);  // singular
// sintetiza UN solo Recording
let synthetic_mbid = format!("synthetic:tidal:{}", work.mbid);
let mut synth = Recording::shell(&synthetic_mbid, &work.mbid);
// ...
work.recordings.push(synth);
```

D-037 lo decidió consciente:
> "Generar N (3-5) recordings sintéticas a partir del top-N de la query: rechazado V1 — el ranking por debajo del top-1 con scores 0.50-0.59 es ruidoso. Ampliar a N en V1.1 si telemetría lo justifica."

**La realidad demuestra que el rechazo era prematuro**: para repertorio común con query bien construida los top-8 son consistentemente parents/children del mismo work (e.g. Beethoven Op. 83 → 8 cantantes distintos cantando el mismo lied). Pedro acaba de proporcionar la telemetría.

#### Fix propuesto

1. Refactor `try_work_level_fallback` → `try_work_level_fallback_multiple`:
   - Score TODOS los candidatos del top-8 con `score_candidate(None, None, None)`.
   - Cap inferior: `WORK_LEVEL_THRESHOLD = 0.62` (subido en A3).
   - Devolver `Vec<MatchOutcome>` ordenado por score desc, hasta `MAX_WORK_LEVEL_SYNTH = 12`.
   - **Movement penalty −0.25** sigue aplicando per candidate.

2. Síntesis: por cada outcome, `Recording::shell(format!("synthetic:tidal:{}:{}", work_mbid, idx), &work_mbid)`. El índice estabiliza el MBID frente a re-fetches y permite distinguir versiones en logs / dedup.

3. `MatchConfidence::TidalDirectInferred` se mantiene en TODAS (badge UI ya implementado en F8.5).

4. `recording_count = recordings.len()` sigue auto-actualizándose post-fallback.

5. Caso edge: si tras synth todas tienen score < 0.62, no se sintetiza nada y `tidal_unavailable` queda true → banner. Comportamiento actual preservado.

#### Tests nuevos (matching.rs)

- 8 candidates Beethoven Op. 83 mock → al menos 5 con `TidalDirectInferred`, ranking por score desc, queries idénticas.
- 8 candidates con 4 movements + 4 parents → solo los 4 parents synth (movement penalty filtra los movements).
- 0 candidates después de threshold → `Vec::new()`, fallback no-op.
- Todas las recordings syntheticas tienen MBID prefijo `synthetic:tidal:{work_mbid}:` con índice numérico.

#### Riesgo y mitigación

Sort/filter de `WorkPage` (Phase 4 — `applyRecordingFilters`, `applyRecordingSort`) verificado: no asume MBIDs únicos no-prefijados. El sort se hace por `quality_score` y `recording_year`. **OK.**

---

### A2 — Query construction usa catalog number obligatorio

**Files**: `src-tauri/src/classical/providers/tidal.rs::build_canonical_query` (líneas 308-355) + callsites.
**Decisión**: D-041.

#### Diagnóstico

`tidal.rs:331-355` `build_canonical_query(composer, title, primary_artist, year)` no acepta catalog number. `catalog.rs:311` invoca con `(composer_name, &work.title, None, None)`. El catalog number (Op. 83, BWV 244) reside en `work.catalogue_number` desde Phase 1 pero NO se anexa.

Peor: `strip_catalogue_suffix` ACTIVAMENTE elimina el catalog number del título original. Para "3 Gesänge von Goethe, Op. 83":
- `find(',')` corta a "3 Gesänge von Goethe".
- Output query: "Beethoven 3 Gesänge von Goethe".

Tidal full-text engine ve "3" como token frecuente y la heurística devuelve "Symphony No. **3**".

#### Fix propuesto

```rust
pub fn build_canonical_query(
    composer_name: Option<&str>,
    work_title: &str,
    catalogue: Option<&CatalogueNumber>,  // ← nuevo
    primary_artist: Option<&str>,
    year: Option<i32>,
) -> String {
    // ... existing logic ...
    if let Some(cat) = catalogue {
        out.push(' ');
        out.push_str(&cat.display);  // "Op. 83", "BWV 244", "K. 466"
    }
    // ... year + artist as before ...
}
```

Mantener `strip_catalogue_suffix` para cuando la query queda demasiado larga, pero AHORA con override: si `catalogue.is_some()`, no strippear el "in D minor" tail del título — el catalog number es el discriminador, la key contribuye también.

Update callsites:
- `catalog.rs::try_work_level_fallback`: pasar `work.catalogue_number.as_ref()`.
- `matching` per-recording cascade Phase 1: mismo cambio aditivo.

#### Tests nuevos

- `build_canonical_query(Some("Beethoven"), "3 Gesänge von Goethe", Some(&CatalogueNumber{system:"Op", display:"Op. 83", ..}), None, None)` → contains "Op. 83".
- `build_canonical_query(Some("Bach"), "Matthäus-Passion", Some(&BWV_244), None, None)` → contains "BWV 244".
- Backward compat: `(Some("Glass"), "Glassworks", None, None, None)` → "Glass Glassworks".

---

### A3 — Threshold 0.62 + scoring genre-aware

**File**: `src-tauri/src/classical/matching.rs`.
**Decisión**: D-041.

#### Diagnóstico

Match Eroica I. Allegro contra "3 Gesänge" scoreó 0.775 en log. Movement penalty −0.25 aplicó si el título Tidal tenía roman numeral; pero independientemente del 0.775 exacto, el matcher actual **no consulta el género/work-type** para penalizar mismatches semánticos. `work.work_type = WorkType::Lieder` (vocal) vs Tidal track album_title "Symphonies Nos. 1-9" debería ser penalización fuerte. Hoy es invisible.

#### Fix propuesto

1. Extender `Recording`/`Track` candidate scoring con un signal de género derivado:
   - `tidal.rs::TidalSearchTrack`: nuevo campo `album_kind: Option<TidalAlbumKind>` poblado por inferencia ligera del `album.title` y `album.tags`.
   - `TidalAlbumKind = Symphonic | Concertante | Vocal | Choral | Chamber | Keyboard | Stage | Unknown`.

2. `score_candidate` consulta `work.work_type` (mapeado a `WorkBucket` via D-040) y compara con `candidate.album_kind`:
   - Buckets incompatibles (Vocal ⊥ Symphonic, Chamber ⊥ Stage, etc.): penalty `−0.30`.
   - `album_kind == Unknown`: no penaliza.

3. Subir `WORK_LEVEL_THRESHOLD = 0.55 → 0.62`. Mantener `INFERRED_THRESHOLD = 0.6` (Phase 1 D-010) sin cambio.

#### Tests nuevos

- `score_candidate(work_type=Lieder, album_kind=Symphonic, ...)` → score baja −0.30, queda < 0.62 incluso con title overlap alto.
- `score_candidate(work_type=Symphony, album_kind=Symphonic, ...)` → no penalty.
- `score_candidate(work_type=Sonata, album_kind=Unknown, ...)` → no penalty.

#### Riesgo

Subir threshold causa regresión: works canónicos donde MB tiene 0 recordings pero fallback acertaba con 0.58-0.61 ahora caen al banner. Mitigación: tras shippear, validation gate manual sobre 5-10 obras conocidas. Si tasa de falso-negativo > 10%, revertimos a 0.58.

---

### A4 — "Cargar más" pagination fix

**Files**: `src-tauri/src/classical/catalog.rs::ComposerWorksPage` + `src/components/classical/ComposerPage.tsx::loadMoreWorks`.
**Decisión**: D-047.

#### Diagnóstico (confirmado por classical-supervisor leyendo código)

`ComposerPage.tsx:209-213`:
```ts
const page = await listClassicalWorksByComposer(mbid, undefined, works.length);
```

`works.length` es POST-filter D-028 (parents). Pero el offset MB es PRE-filter. Bach:
- Page 1 backend: MB browse offset=0, limit=100 → 100 mb_works → filtro D-028 deja 30 parents → frontend `works.length = 30`.
- Click "Load more" → frontend pide `offset=30` → MB browse offset=30, limit=100 → devuelve los mb_works[30..130] que en page 1 ya estaban en mb_works[30..100] (overlap). Post-filter ~10 nuevos parents.
- De-dup defensivo elimina los duplicados → 0-5 nuevos works visibles. Pedro percibe "no funciona".

#### Fix propuesto

Backend:
- `ComposerWorksPage` añade `pub next_offset: u32`.
- En `build_composer_works_fresh`: `next_offset = offset + (mb_works.len() as u32)`.

Frontend:
- `ComposerWorksPage` type añade `nextOffset: number`.
- ComposerPage state: nuevo `nextOffset`.
- `loadMoreWorks` pasa `nextOffset` en lugar de `works.length`.
- Cache key bump v2 → v3 (D-029 lo subió a v2).

#### Tests nuevos

- Test acceptance Bach mock: 1100 work-count, 100 mb_works first page, post-filter 30. `nextOffset` retorna 100. Segunda call con `offset=100` retorna 100 nuevos mb_works → 30 nuevos parents. De-dup count = 0.

---

### A5 — Movements colados pese a D-028

**File**: `src-tauri/src/classical/providers/musicbrainz.rs::browse_works_by_artist`.
**Decisión**: D-048.

#### Diagnóstico

D-028 filtra `direction=backward, type=parts`. Funciona si MB tiene la rel cargada. Casos donde falla:
- MB no cargó la rel (works menos curados, repertorio raro).
- Movements modelados como type="parts" pero el child apunta forward al parent (raro pero posible).
- Movements que NO son sub-works de un parent en MB (standalone "Andante" como obra independiente; no es movement, es pieza pequeña). El filtro **no debe** atrapar estos.

#### Fix propuesto

Filtro defensivo SECUNDARIO por title regex en el provider, complementando D-028 (NO sustituyéndolo):

```rust
fn title_looks_like_movement(title: &str) -> bool {
    static MOVEMENT_RE: OnceLock<Regex> = OnceLock::new();
    let re = MOVEMENT_RE.get_or_init(|| {
        Regex::new(r"^(?:[IVX]{1,4})\s*\.\s+\S").unwrap()
    });
    re.is_match(title)
}
```

En `browse_works_by_artist`, después del check D-028:
```rust
if work_is_child_movement(w) {
    continue;
}
if title_looks_like_movement(&title) {
    log::debug!("[mb] dropping movement-like title: {}", title);
    continue;
}
```

Defensa simétrica frontend en `ComposerPage.tsx::groupWorks`.

#### Tests nuevos

- "I. Allegro" → drop.
- "IV. Presto" → drop.
- "VIII. Andante mosso" → drop.
- "Andante in C major" → keep (standalone).
- "Andantino" → keep (sin dot).
- "Symphony No. 1 in C" → keep.

---

## Acceptance criteria (validation gate Phase 8.9 → 9)

1. **Beethoven Op. 83** (`347d1f6b-...`): click play → reproduce uno de los 3 Gesänge, NUNCA Eroica. Verificable manualmente con build instalada.
2. **Beethoven Op. 83**: muestra ≥ 5 recordings en WorkPage (top-N synth funcionando).
3. **Bach** ComposerPage: click "Cargar más" devuelve obras nuevas (no duplicados de page 1). Verificable navegando + observando counter "X of ~Y".
4. **Tchaikovsky** ComposerPage: cero entradas con título matching `^[IVX]+\.`. Verificable visualmente.
5. **Tests**: `cargo test --lib` ≥ 145 + 10 nuevos = 155 pasando (A1: 4, A2: 3, A3: 3, A4: 1, A5: 6 = 17 esperados, aceptable margen).
6. Cero regresión §10:
   - `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty.
   - `route_volume_change` (lib.rs) intacto.
   - Writer guard (audio.rs:988-992) intacto.
7. Build clean: `cargo check --release` + `cargo clippy --release --lib --no-deps` (14 warnings baseline preservados) + `npm run build` + `tsc --noEmit`.

Si falla 1-4 → diagnóstico y fix antes de Phase 9.

---

## Files que se tocan

Backend:
- `src-tauri/src/classical/catalog.rs` — `try_work_level_fallback*`, `ComposerWorksPage`, `build_composer_works_fresh`.
- `src-tauri/src/classical/matching.rs` — `WORK_LEVEL_THRESHOLD`, `score_candidate`, `best_work_level_candidate*`.
- `src-tauri/src/classical/types.rs` — `Recording`, `MatchConfidence` (sin variant nuevo, conserva `TidalDirectInferred`).
- `src-tauri/src/classical/providers/tidal.rs` — `build_canonical_query` (signature change), `TidalSearchTrack` (nuevo `album_kind`), inferencia album_kind.
- `src-tauri/src/classical/providers/musicbrainz.rs` — `browse_works_by_artist`, `title_looks_like_movement`.

Frontend:
- `src/types/classical.ts` — `ComposerWorksPage` añade `nextOffset`.
- `src/components/classical/ComposerPage.tsx` — `loadMoreWorks` con `nextOffset`, defensa frontend movement regex.

Tests: nuevos en `matching.rs`, `tidal.rs::tests`, `musicbrainz.rs::tests`.

---

## Phase fit

Phase 8.9 emergency fix bundle. **Bloquea Phase 9** (Phase 9 toca `try_work_level_fallback*` también; coherente cerrar A1-A3 antes de Phase 9.B/C).

Cero §10 audio path violation. Cero variant nuevo en enum exposed (todo aditivo en estructuras existentes).

---

## Estado actual

📝 **Plan completo. Pendiente delegar a `sone-backend-engineer` + `sone-frontend-engineer`** vía classical-supervisor con DESIGN-OK confirmado por carta blanca de Pedro (2026-05-04).
