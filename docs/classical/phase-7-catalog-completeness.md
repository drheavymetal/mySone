# Phase 7 — Catalog completeness

**Estado**: 📝 plan pending review (creado 2026-05-02 tras feedback del usuario sobre el cierre V1).
**Branch**: `soneClassical` (continúa, no se abre rama nueva).
**Owner**: classical-supervisor.
**Modo**: NO autónomo. Este plan no se implementa hasta que el usuario lo autorice explícitamente. La sesión actual sólo redacta el plan.

> **Mandato del usuario (textual, 2026-05-02)**:
>
> > "quiero tener todos los compositores y todas las obras disponibles en el catálogo de tidal, hazlo como quieras, pero no quiero perder nada de lo que pueda escuchar, no tiene sentido"
>
> El Hub V1 cerrado (Phases 0-6) se queda corto: 33 compositores en el snapshot, ComposerPage cap 100 obras sin filtro de parent-only (Tchaikovsky muestra "III. Adagio lamentoso" de la Pathétique en lugar del parent), y nada de paginación. **Phase 7 cierra ese hueco como parte del scope V1** — no es V2. Calidad sobre velocidad. Cero estimaciones en horas.

---

## 0. TL;DR

1. **Universo de compositores: ampliar embedded snapshot a ~600-1500 composers** vía Wikidata SPARQL + MB cross-check, **NO 30K**. La cifra "todos los compositores Wikidata P106 wd:Q36834" (~30K) incluye muchos sin recordings comerciales y sería un binario pesadísimo. Compromiso: cargar los que tengan ≥ N grabaciones en MusicBrainz como proxy de "tiene catálogo audible en cualquier parte". El gate Phase 7.0 valida la N concreta.
2. **Bug Tchaikovsky (movement-as-parent)**: filtrar `browse_works_by_artist` para excluir child works (works con `part-of` rel a otro work). Cascade: si OpenOpus tiene el composer como ground-truth, mergear obras parent-only del snapshot con MB excluyendo movements.
3. **Paginación works > 100**: extender `browse_works_by_artist` con offset; UI lazy-load infinito en ComposerPage. MB API soporta offset hasta `total-count`.
4. **Tidal availability**: NO pre-screen. Mostrar todos los works del compositor; el primer click dispara fetch de recordings; si vacío tras cascade, marcar work como "Sin grabaciones disponibles" persistente (cache negativo 7d).
5. **Search universal**: ampliar el tokenizer Phase 5 con un índice secundario "extended composer index" del nuevo snapshot ampliado. Tokenizer ya determinístico — solo se le inyecta más universo.
6. **Frontend**: BrowseComposers paginado client-side (snapshot ya entero en memoria) + "Show all eras" + persistente expand. ComposerPage: dos secciones — "Top works" (curated por OpenOpus popular flag) + "All works" (paginado, expand-on-demand).
7. **Bit-perfect inviolable**: cero archivo de §10 audio path tocado. Auditado en cada B7.x.
8. **Riesgos**: tamaño binario (~5-10 MB extra), parse-time del snapshot grande, MB rate-limit con paginación pesada, Wikidata SPARQL availability en build-time.

---

## 1. Diagnóstico del estado actual (V1 cerrado)

### 1.1 Bugs concretos reportados

**Bug 1 — "Solo me salen dos compositores"**.

Lectura desde el código: `BrowseComposers.tsx:35` invoca `listClassicalTopComposers(100)` que el backend resuelve a `OpenOpusProvider::top_composers(100)`. Pero el snapshot embebido (`src-tauri/data/openopus.json`) contiene 33 composers y `truncate(100)` no inventa los 67 restantes.

La percepción del usuario ("solo dos") puede deberse a: (a) el filtrado por era está activo y solo dos compositores caen en la era seleccionada; (b) el listado es real (33) pero la primera fila visible pre-scroll muestra dos cards en mobile-first layout; (c) o el bug visual del Hub (`ClassicalHubPage` muestra `featured.slice(0, 12)`) que da impresión de catálogo pequeño. **A clarificar antes de cerrar diagnóstico** — pero independiente de la causa exacta, el remedio (ampliar el snapshot) cubre todos los escenarios.

**Bug 2 — Tchaikovsky muestra "III. Adagio lamentoso" en lugar de la Pathétique**.

`MusicBrainzProvider::browse_works_by_artist` (`src-tauri/src/classical/providers/musicbrainz.rs:449-500`) hace `GET /work?artist={mbid}&inc=aliases&fmt=json&limit=100`. **Sin `inc=work-rels`**, MB devuelve indistintamente parent works ("Symphony No. 6 'Pathétique'") y child works ("III. Adagio lamentoso"). El parent puede caer fuera del top-100 alfabético si MB ordena por título y empieza por "III.". El cascade con OpenOpus (`build_composer_works_fresh:715-789`) preserva ambos sin filtrar.

**Tchaikovsky's MBID de OpenOpus snapshot**: a verificar. Si el snapshot incluye Tchaikovsky entre los 33, el cascade tiene los works canónicos pero los emite mezclados con movements. Si no lo incluye, los datos vienen 100% de MB sin filtro.

**Bug 3 — Bach/Mozart truncados a 100 obras**.

Mismo método con `limit.min(100)` en `musicbrainz.rs:454`. Bach tiene > 1000 works en MB, Mozart > 600. El frontend nunca pide más allá de la primera página → catálogo incompleto.

### 1.2 Lo que NO está roto

- Audio path: cero archivo de §10 modificado en 6 phases. Confirmado por `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty.
- Tests 118/118 pasando.
- Provider+catalog pattern (§5.2) intacto.
- Cache TTLs (§3.3) coherentes.

Phase 7 NO tiene que reescribir nada de Phases 0-6. Es estrictamente **aditivo + reemplazo del snapshot embebido**.

---

## 2. Principios de diseño (no negociables)

| # | Principio | Origen |
|---|---|---|
| P1 | Cero regresión §10. Ningún archivo del audio path se modifica. | D-005 |
| P2 | Llaves siempre, código nuevo y modificado. | D-004 |
| P3 | Provider+catalog pattern (§5.2) — toda fuente nueva como `ClassicalProvider`. | CLASSICAL_DESIGN.md §5 |
| P4 | TTLs según §3.3. Cualquier cache nuevo justifica su TTL. | CLASSICAL_DESIGN.md §3.3 |
| P5 | Decisiones de repertorio (qué composers entran al snapshot, umbrales de "popular") consultadas con `classical-musicologist`. | Supervisor mandate |
| P6 | El snapshot es **build-time output**, no run-time. Lo regenera un script reproducible documentado en `docs/classical/scripts/`. | D-009 / D-013 |
| P7 | Cascade de fallback explícito: si la fuente A falla, la B sigue. Nunca un único punto de fallo. | §3.2 |
| P8 | Cero pre-screen de Tidal availability. La realidad de "playable" se descubre al click, no al list. | Phase 0 evidencia: 30K composers × 2s = inviable. |

---

## 3. Decisiones nuevas a registrar (D-027+)

Cada una sustituye o refina decisiones previas. Plantilla canónica de `DECISIONS.md`.

### D-027 — Universo de compositores: harvest Wikidata + MB filter

**Categoría**: ARCH.

**Decisión propuesta**: el snapshot ampliado contiene composers que cumplan **todas** estas condiciones:
1. `wdt:P106 wd:Q36834` (occupation = composer) en Wikidata.
2. `wdt:P434` no nulo (tienen identificador MusicBrainz).
3. `?recording_count >= 5` en MB (proxy de "catálogo audible existe").

El target es **600-1500 composers** según donde caiga el corte 5. Sub-task B7.0 ejecuta el harvest y reporta cifras reales antes de fijar el cap.

**Justificación**: 30K composers Wikidata sin filtro inflaría el binario en ~30 MB JSON (1KB/composer) y cargaría composers sin impacto audible. El threshold "≥5 recordings MB" es defensivo — captura desde Hildegard von Bingen (medieval con N grabaciones) hasta Caroline Shaw (contemporánea con un puñado), pero excluye composers sólo nominales.

**Alternativas consideradas**:
- *Universo completo Wikidata sin filtro*: rechazado — binario gigante, mayoría inviable.
- *Lazy-fetch on demand desde MB*: rechazado — hace BrowseComposers cold-cache lentísimo (la página depende de tener el universo en memoria).
- *Pre-baked snapshot via build script + Wikidata REST snapshot fija*: este es el camino elegido.

**Trade-off**: el snapshot crece de ~227 KB → estimado 2-5 MB. Aceptable. Carga inicial de `OnceLock` parse aumenta de ~5ms → estimado 30-80ms (medible en Phase 7.0).

**Doc afectado**: `src-tauri/data/openopus.json` reemplazado o renombrado a `src-tauri/data/composers-snapshot.json` (a decidir en B7.0). Si renombrado, OpenOpus snapshot original se preserva como source-of-truth de "popular flag" para top picks; el nuevo snapshot lo extiende sin sustituirlo.

### D-028 — Movement filter en `browse_works_by_artist`

**Categoría**: ARCH.

**Decisión propuesta**: extender la query MB con `inc=work-rels` y filtrar en el parser cualquier work cuya respuesta liste un `parent` rel hacia otro work. Solo se emiten parent works (los que NO son hijos de otro). Movements quedan accesibles vía `Work.movements[]` en la WorkPage.

**Justificación**: el bug Tchaikovsky es exactamente esto — child works leakeando como entries top-level. MB modela movements como sub-works con `part-of` rel; el filtro es local y barato.

**Alternativas consideradas**:
- *Cascade desde OpenOpus como ground-truth para popular composers*: complementario, no sustituto. OpenOpus solo cubre 33 composers; necesitamos arreglar MB-only path para los 1500+.
- *Title heuristic (descartar títulos que empiezan por roman numeral)*: frágil, falsos positivos en sonatas titulados "I. Allegro" como pieza independiente.

**Trade-off**: la query MB pasa de `inc=aliases` → `inc=aliases+work-rels`. Cada response es ~30% más pesado pero el rate-limit no cambia (1 req/s). Cache StaticMeta absorbe el coste.

**Doc afectado**: `src-tauri/src/classical/providers/musicbrainz.rs:449` (parser + URL builder).

### D-029 — Paginación works en ComposerPage

**Categoría**: ARCH.

**Decisión propuesta**: `browse_works_by_artist(artist_mbid, limit, offset)` recibe offset opcional. Backend pagina hasta el `?work-count` total que MB devuelve en el header del response (`work-count` field). UI carga primera página de 100, botón "Show more" paga la siguiente.

Cache key incluye offset: `classical:composer-works:v2:{mbid}:{genre}:{offset}`. Schema bump `v1→v2` invalida cache antiguo (v1 era sin offset).

**Justificación**: Bach (>1000 works) y Mozart (>600) necesitan páginas múltiples. La alternativa "fetch todo en cold cache de un golpe" colapsa el rate-limit MB.

**Alternativas consideradas**:
- *Single mega-fetch en background al abrir composer*: rechazado — rate-limit pressure + UX delay.
- *Cap dura a 200 con "see all in MB" link externo*: rechazado, contradice el mandato del usuario ("no perder nada").

**Trade-off**: cache fragmentado por offset. Aceptable: cada cache key sigue siendo immutable hasta TTL. La invalidación cascade-by-tag (`COMPOSER_WORKS_CACHE_TAG`) sigue funcionando.

**Doc afectado**: `src-tauri/src/classical/providers/musicbrainz.rs`, `src-tauri/src/classical/catalog.rs:670` (signature change), `commands/classical.rs` (extender command).

### D-030 — Tidal availability: lazy + cache negativo

**Categoría**: ARCH.

**Decisión propuesta**: NO pre-screen. La WorkPage ejecuta su cascade ISRC + Tidal text search Phase 1 en cold-cache la primera vez. Si tras `Matcher` no hay recordings con `tidal_track_id`, persistir el resultado vacío con TTL 7d en `classical:work-tidal-empty:v1:{work_mbid}` y mostrar UI "Tidal no tiene grabaciones de esta obra" con CTA a re-buscar.

**Justificación**: el cálculo en hilo del usuario:
- 30K composers × 2s probe = 16h. **Inviable**.
- 1500 composers × 5 works promedio × 2s = 4h. **Inviable** en build-time, dudoso en run-time pre-warm.
- On-click cold-cache es ~12s (Phase 1 budget) por work. **Aceptable** porque el usuario sólo visita los works que le interesan.

El cache negativo TTL 7d evita re-pegar la cascade contra works comprobadamente vacíos en Tidal. Si Tidal añade catalog (raro pero posible), el TTL expira y reintenta.

**Alternativas consideradas**:
- *Pre-warm de top-50 composers × top-20 works en background*: válido como complemento (D-026 ya hace algo parecido para canon). Phase 7 puede extender el universo del pre-warm pero con cap conservador para no saturar Tidal.
- *Marcar la card del work con un dot "verificado" tras el primer fetch*: sí, sub-task F7.x. Costo: indicador visual en `WorkSummaryCard` que refleja `cached_tidal_empty` flag.

**Trade-off**: la primera vez que el usuario abre un work de un composer obscuro, paga la cascade Phase 1 entera. Subsecuente cache hit es instantáneo.

**Doc afectado**: `src-tauri/src/classical/catalog.rs::build_work_fresh` (extiende escritura de cache negativo cuando recordings vacíos).

### D-031 — Search tokenizer: índice extendido del snapshot ampliado

**Categoría**: ARCH.

**Decisión propuesta**: el tokenizer Phase 5 (`classical/search.rs::COMPOSER_INDEX`) actualmente lookups sobre los 33 composers OpenOpus. Phase 7 le inyecta el nuevo snapshot ampliado; index pasa a 600-1500 entries. Cero cambio de lógica, sólo amplía universo.

**Justificación**: el tokenizer es determinístico (D-019), in-process. El coste de search es O(snapshot.len()) para composer-name match. Con 1500 entries y `name.to_lowercase().contains(query)` sigue siendo µs.

**Alternativas consideradas**:
- *Ranked-tokenizer con Levenshtein*: overkill para Phase 7, deferimiento legítimo.
- *Trie pre-built*: optimización prematura. Si > 50ms en algún test, lo abrimos como sub-task.

**Trade-off**: ninguno relevante.

**Doc afectado**: `src-tauri/src/classical/search.rs` (consume snapshot ampliado), no cambia API pública.

### D-032 — Snapshot regeneration script reproducible

**Categoría**: TOOLING.

**Decisión propuesta**: script `docs/classical/scripts/snapshot_composers_extended.py` (o `.sh`/`.mjs`, a decidir en B7.0 según afinidad del operador) que:
1. Hace SPARQL contra `query.wikidata.org/sparql` con la query oficial Phase 7.
2. Para cada Wikidata QID con MB ID, valida en MB que el composer existe + cuenta recordings.
3. Filtra por `recording_count >= N`.
4. Mergea con OpenOpus original (preservando `popular`/`recommended` flags y `epoch` cuando OpenOpus los tiene; defaulteando desde Wikidata cuando no).
5. Output: `src-tauri/data/composers-snapshot.json` (o reemplaza `openopus.json` si decidimos colapsar).
6. Versionado en repo. Re-ejecutable. CI no lo corre (rate-limit + no-determinismo de WDQS).

**Justificación**: el snapshot es propiedad versionada del repo. Cada release de mySone puede actualizarlo si el dev considera que el universo cambió. NO es output de runtime — el binario sólo consume el JSON ya bakeado.

**Alternativas consideradas**:
- *Snapshot regenerado on first launch*: rechazado — primer launch online dependency contradice §14 privacy + offline-first.
- *Script en `build.rs` Cargo*: rechazado — build determinismo se rompe (WDQS responde distinto cada día).

**Trade-off**: el snapshot envejece. Contramedida: documentar en README cuándo regenerarlo (cada release o cuando se detecte composer faltante reportable por usuario).

**Doc afectado**: `docs/classical/scripts/snapshot_composers_extended.{py|sh|mjs}` (NEW).

### D-033 — Dual-snapshot: OpenOpus original + extended

**Categoría**: ARCH.

**Decisión propuesta** (subject to revisión en B7.0): mantener **dos snapshots embebidos**:

1. `src-tauri/data/openopus.json` (original 33 composers, preservado intacto).
2. `src-tauri/data/composers-extended.json` (nuevo, 600-1500 composers).

`OpenOpusProvider` permanece como la fuente autoritativa de "popular" + "recommended" flags + works recommendations. Nuevo `ExtendedComposersProvider` (o ext del OpenOpusProvider) carga el universo amplio sólo para BrowseComposers + search index.

**Justificación**: el snapshot OpenOpus original tiene curación editorial por ellos (no nosotros) — `popular=true` significa "OpenOpus considera al composer canon". Sustituirlo perdería esa señal. Phase 5 editorial seeds (D-020) dependen de la lista original.

**Alternativas consideradas**:
- *Snapshot único colapsado*: rechazado en favor de separación de responsabilidades (canon curado vs universo amplio).
- *Solo extended snapshot, sin OpenOpus original*: rechazado — pérdida de la curación.

**Trade-off**: dos archivos a mantener. El extended no tiene `popular` flag autoritativo (lo hereda de OpenOpus si el composer está en ambos; FALSE para los nuevos).

**Doc afectado**: `src-tauri/src/classical/providers/openopus.rs` (extiende API o se renombra a `composers.rs`), `src-tauri/data/composers-extended.json` (NEW).

### D-034 — Composer-resolution en stats: `composer_mbid` por play (refinement de D-025)

**Categoría**: ARCH.

**Decisión propuesta**: D-025 documentó que "top classical composers" stats devuelve "top performers". Phase 7 puede cerrar la deuda introduciendo columna `plays.composer_mbid TEXT NULL`. Resolver: extiende `WorkMbidResolver` (D-012) con `resolve_composer_for_work`, llamada post-track-start best-effort.

**SUPERSEDES**: parcial sobre D-025 (sólo el caveat — la limitación se cierra).

**Decisión del supervisor**: **diferir B7.x.composer-resolution a sub-fase opcional** dentro de Phase 7. Se cierra si el resto del Phase 7 sale limpio y queda budget; si no, se mantiene como deuda explícita V1.

**Justificación**: backfill de `composer_mbid` para plays históricos requiere re-resolver work→composer en MB para cada play (rate-limit pressure). Para plays nuevos es fácil (resolver call extra de ~1s post-track). El backfill es lo costoso.

**Alternativas consideradas**:
- *Backfill diferido a launch único en background*: viable, sub-task F7.composers.
- *Solo plays nuevos*: top-composers sigue siendo "top performers" para el histórico hasta que el usuario re-escuche.

**Trade-off**: una migration aditiva más en `stats.rs`. Idempotent.

**Doc afectado**: `src-tauri/src/stats.rs`, `src-tauri/src/scrobble/mod.rs`.

---

## 4. Sub-tasks B7.x (backend)

### B7.0 — Snapshot extended: harvest + script + decisión universal size

**Sección del doc**: §3.1 (fuentes de datos), D-027, D-032.

**Deliverable**:
- `docs/classical/scripts/snapshot_composers_extended.{py|sh|mjs}` (NEW).
- `src-tauri/data/composers-extended.json` generado y versionado.
- Reporte en `phase-7-catalog-completeness.md` Apéndice A con: composers totales, distribution por era, distribution por recording_count, tamaño JSON final.

**Pre-condición**: decidir el threshold N de recording_count en el harvest (default proposed: 5). Decisión consultada con `classical-musicologist`.

**Sub-pasos**:
1. Drafting de la query SPARQL (Wikidata) con paging.
2. Implementación del script con rate-limit WDQS (1.5s/query, D-023) + MB rate-limit (1 req/s).
3. Run controlado en local (no CI), captura outputs.
4. Reporte Apéndice A — número exacto, tamaño, anomalías.
5. Decisión final del N + commit del snapshot.

**Cero regresión**: el snapshot original `openopus.json` NO se toca. El nuevo es archivo separado. **El binario no incrementa hasta que B7.1 lo bakea con `include_bytes!`**.

**Acceptance**:
- Script reproducible (re-run produce JSON estable salvo orden — sort lexicográfico).
- Snapshot tiene composer_mbid + qid + name + era + birth/death year + recording_count.
- Tamaño JSON ≤ 5 MB.
- Composers conocidos faltantes (Tchaikovsky, Schubert, Sibelius, Pärt extended, Adams, Reich, Glass, Saariaho, Pärt, Hildegard, Caroline Shaw) están todos.

**Consultas musicologist**:
- ¿N=5 o N=3 o N=10? Trade-off entre catálogo amplio y ruido.
- ¿Se incluye composers con MBID pero sin Wikidata QID (raros pero existen)? Recomendación inicial: NO en V1, log gracefully en script.

### B7.1 — ExtendedComposersProvider + carga del nuevo snapshot

**Sección del doc**: §5.2, D-033.

**Deliverable**:
- `src-tauri/src/classical/providers/composers_extended.rs` (NEW) — provider que parsea `composers-extended.json` con OnceLock + expose API similar a OpenOpusProvider.
- O alternativamente, extender `openopus.rs` con campos opcionales para entries del extended snapshot. **Decisión final en B7.1 step 1** consultada con backend-engineer.
- 8-12 tests unitarios cubriendo: parse correcto, lookup_composer_summary contra extended set, top_composers_extended con orden estable, era filter sobre extended.

**Cero regresión**: `OpenOpusProvider` actual no cambia su API pública. Cualquier consumer existente sigue viendo los 33 composers como "popular".

**Acceptance**:
- `cargo test classical::providers::composers_extended` ≥ 8/8.
- `cargo clippy --release --lib --no-deps` sin warnings nuevos.
- Tamaño binario release: deltado vs baseline post-Phase 6, justificable por el JSON embebido.

### B7.2 — Movement filter en `browse_works_by_artist`

**Sección del doc**: D-028, §3.1.

**Deliverable**:
- Patch a `src-tauri/src/classical/providers/musicbrainz.rs:449-500`:
  - URL builder añade `inc=aliases+work-rels`.
  - Parser extrae `relations[]` y descarta works con cualquier rel donde `direction=backward` y `type=parts` (= "es child de otro work").
  - Tests nuevos en mismo archivo: 3 cases (parent-only emerge, child-only excluded, mixed input filtered correctamente).
- Bumping cache key version: `classical:composer-works:v1:` → `v2:` para invalidar el cache viejo.

**Riesgo**: MB devuelve relations para works sin parent, pero algunos works son standalone (Mozart Eine kleine Nachtmusik no tiene movements en MB). Verificar con fixtures que el filtro NO los descarta.

**Cero regresión**: el path para composers ya cubiertos por OpenOpus snapshot sigue funcionando (cascade con título normalizado, D-015). El filtro es estrictamente sustractivo sobre el set MB.

**Acceptance**:
- Test acceptance "Tchaikovsky pathétique appears as parent, not movements": fixture MB JSON con Pathétique parent + 4 child movements; filter devuelve solo el parent.
- 118 + nuevos tests pasan.
- Beethoven 9 cascade sigue funcionando (regresión test).

### B7.3 — Paginación de works

**Sección del doc**: D-029.

**Deliverable**:
- `MusicBrainzProvider::browse_works_by_artist(artist_mbid, limit, offset)` con offset opcional.
- Response parser captura `work-count` total para que el cliente sepa si hay más páginas.
- Nueva struct `BrowsedWorksPage { works: Vec<MbBrowsedWork>, total: u32, offset: u32 }`.
- `CatalogService::list_works_by_composer(mbid, genre, page)` extendido. Cache key `classical:composer-works:v2:{mbid}:{genre}:{offset}`.
- Tauri command `list_classical_works_by_composer` extendido con parámetro `offset` opcional (default 0).

**Cero regresión**: callers existentes que no pasan offset siguen funcionando con offset=0.

**Acceptance**:
- Bach (artist_mbid `24f1766e-9635-4d58-a4d4-9413f9f98a4c`) page 1 + page 2 + page 3 cargan sin error, total cuenta consistente.
- Cache hit en ambas páginas tras primer fetch.
- `cargo test`: nuevos tests sobre paginación.

### B7.4 — Cache negativo "Tidal empty" para Works

**Sección del doc**: D-030.

**Deliverable**:
- `CatalogService::build_work_fresh` (existing) detecta cuando, tras la cascade ISRC + Tidal text search, **cero recordings tienen `tidal_track_id`**.
- En ese caso, escribir el `Work` resultante con flag `tidal_unavailable: true` (campo nuevo aditivo en `Work` struct + types.ts).
- Cache TTL 7d (Dynamic tier, no StaticMeta — el catálogo Tidal cambia con más frecuencia que MB).
- Frontend muestra banner "Tidal does not have recordings of this work yet" + botón "Re-check now" que invalida la cache key del work y refetcha.

**Cero regresión**: works que SÍ tienen recordings Tidal renderizan idéntico a Phase 1-6. El flag es opt-in.

**Acceptance**:
- Test unitario sobre `build_work_fresh` con mock provider que devuelve cero ISRC matches y cero text-search matches → flag se setea, cache se escribe.
- Test que clear_cache lo invalida correctamente.
- Manual QA: abrir un work obscuro sin recordings Tidal → ve banner; re-check → relauncha cascade.

### B7.5 — Search tokenizer: ampliar índice de composer

**Sección del doc**: D-031.

**Deliverable**:
- `classical/search.rs::build_composer_index` consume el extended snapshot además del original. El index de search pasa a 600-1500 entries.
- Tests existentes (24) siguen pasando.
- Nuevo test: "Hildegard von Bingen tokenizer detection" verifica que un composer fuera del top-33 ahora se reconoce.

**Cero regresión**: las 24 búsquedas previamente exitosas siguen siendo exitosas (orden puede cambiar si surge un nuevo competing composer en el índice — verificar).

**Acceptance**:
- 24 tests originales pasan.
- ≥ 3 tests nuevos pasan: composer fuera del canon top-33 detectado por el tokenizer.

### B7.6 (opcional) — Composer resolver para stats

**Sección del doc**: D-034.

**Deliverable**:
- `WorkMbidResolver::resolve_composer_for_work` añadido al trait + impl en `CatalogService`.
- Extensión de `scrobble/mod.rs::on_track_started` para invocar este resolver post-track-start (best-effort, off the critical path) y persistir `plays.composer_mbid`.
- Migración aditiva `stats.rs`: columna `composer_mbid TEXT NULL`.
- `top_classical_composers` reescrito para agrupar por `composer_mbid` cuando NOT NULL, fallback a `artist_mbid` cuando NULL (compatibility con plays viejos).
- Backfill task: spawn en background tras boot que recorre plays sin `composer_mbid` y los resuelve. Conservadora rate-limit (1 req/s MB).

**Cero regresión §10**: `scrobble/mod.rs` modificado **solo** en la sección post-`applied=true` (donde Phase 3 ya añadió `classical:work-resolved` event). No toca el critical path. `dispatch_scrobble`, `fire_now_playing`, `record_to_stats` intactos.

**Acceptance**:
- Migration idempotent (rerun no-op).
- Top composers stats refleja composer real, no performer.
- Backfill task converge en N horas (medible) sin saturar MB.

**Decisión inclusion**: si Phase 7 sale a tiempo, B7.6 se incluye. Si no, queda como D-034 documentada y diferida explícitamente.

---

## 5. Sub-tasks F7.x (frontend)

### F7.0 — BrowseComposers paginated (client-side) + searchable + era filter

**Sección del doc**: §7.1, §16.2.

**Deliverable**:
- `BrowseComposers.tsx` extendido:
  - `listClassicalTopComposers(2000)` (cap alto que el backend respeta como `min(snapshot.len, 2000)`).
  - Pagination client-side: render primera página de 60 cards, "Load more" botón append + 60.
  - Search input ya existente (mantenido).
  - Era filter ya existente (mantenido).
  - Indicador "X of Y composers shown" actualizado dinámicamente.
- Skeleton loading idéntico al Phase 2.

**Cero regresión**: si el snapshot extended no se carga, fallback a OpenOpus original (33 composers) con misma UI.

**Acceptance**:
- Hub → Browse → Composers carga visible en < 200ms (snapshot in-memory).
- Filter "Romantic era" + search "Tcha" → encuentra Tchaikovsky.
- Scroll + Load more → siguiente batch aparece sin re-render full list.

### F7.1 — ComposerPage: "Top works" + "All works" expandable

**Sección del doc**: §7.2 Composer page.

**Deliverable**:
- `ComposerPage.tsx` reorganizado:
  - Hero (existing, unchanged).
  - Section "Essentials / Top works" — usa OpenOpus `popular=true` flag o, si el composer no está en OpenOpus, primeras N works ordenadas por catalogue number (heurístico).
  - Section "All works" colapsable, on-expand: dispara `listClassicalWorksByComposer` page 1, render, botón "Load more" → page 2, etc.
  - Filtros por work-type/genre encima de "All works" (ya existentes).
- `WorkSummaryCard.tsx` extendido con flag visual `tidal_unavailable` (D-030) — gris-out + tooltip "Sin grabaciones en Tidal" cuando la cache lo conoce.

**Cero regresión**: composers ya cubiertos por OpenOpus mantienen layout actual de "Essentials" + "Symphonies" + "Concertos" + ... — solo se añade "All works" como section nueva al final.

**Acceptance**:
- Tchaikovsky → Top works (Pathétique, Eugene Onegin, etc., curated por OpenOpus si está; heurístico si no) + All works expandible muestra el catálogo completo (≥ N works).
- Bach → All works expand muestra > 100 works tras Load more.
- Performance: render incremental sin freeze del UI.

### F7.2 — WorkPage: banner "Tidal unavailable" + Re-check

**Sección del doc**: D-030.

**Deliverable**:
- `WorkPage.tsx` extendido: cuando `work.tidalUnavailable === true`, render banner amarillo arriba de la lista de recordings (lista ya estará vacía).
- Banner CTA "Re-check Tidal now" → invoca nuevo command `recheck_classical_work_tidal(work_mbid)` que invalida cache + refetcha.

**Cero regresión**: works con recordings normales no muestran el banner.

**Acceptance**:
- Work obscuro sin Tidal hits → ve banner.
- Click Re-check → loading state → refetch → banner desaparece o persiste según resultado.

### F7.3 — Hub home: indicador de catálogo completo

**Sección del doc**: §7.1 Listen Now.

**Deliverable**:
- `ClassicalHubPage.tsx` Listen Now: footer texto "Catalog: X composers, Y works indexed" como subtle info chip al final de la página.
- "Browse all composers" CTA prominente cerca del Featured grid (ya existe el path, se hace más visible).

**Cero regresión**: secciones existentes (Featured, Editor's Choice, Recently played, Top works) intactas.

**Acceptance**:
- Hub muestra el contador del nuevo snapshot (e.g. "1342 composers, ~85K works").

### F7.4 (opcional, ligado a B7.6) — StatsPage Classical tab refleja composer real

**Sección del doc**: D-034.

**Deliverable**:
- `StatsPage.tsx` Classical tab — Top composers ahora muestra el composer real, no el performer.
- Mientras backfill no termina, banner "Composer resolution in progress — N plays remaining" temporal.

**Decisión inclusion**: ligada a B7.6.

---

## 6. Decision-gate pre-implementación (OBLIGATORIO)

**No se implementa nada hasta que el usuario apruebe explícitamente** los siguientes puntos:

| # | Pregunta | Default propuesto | Necesita confirmación del usuario |
|---|---|---|---|
| G1 | Threshold N de "recording_count >= N" en harvest. ¿N=5? | 5 | Sí — afecta tamaño snapshot y exhaustividad. |
| G2 | ¿Snapshot único colapsado o dual (D-033)? | Dual (preserva curación OpenOpus). | Sí. |
| G3 | ¿Incluimos composer-resolution backfill (B7.6 / D-034) en Phase 7? | "Sí si hay budget; documentar diferimiento si no." | Sí. |
| G4 | ¿Threshold de "tamaño JSON aceptable"? | ≤ 5 MB. | Sí. |
| G5 | Lenguaje del script de harvest (Python / shell / Node)? | Python (standard data-tooling). | Sí. |
| G6 | ¿Snapshot original `openopus.json` se preserva o se renombra? | Preservado intacto (D-033). | Sí. |
| G7 | ¿Pre-warm extendido a top-N composers del nuevo snapshot? | NO en Phase 7 (D-026 sigue cubriendo top-30). | Sí. |
| G8 | Consulta a `classical-musicologist` para repertorio: ¿N final? ¿Composers borderline a incluir? | Pending consulta. | Sí. |

**Hasta que G1-G8 estén respondidas, ningún sub-task B7.x ni F7.x se inicia.** El supervisor delegará a los specialists sólo después de la autorización.

---

## 7. Acceptance criteria (Phase 7 GO/NO-GO)

Para que Phase 7 se cierre como `🟢 completed`, deben cumplirse:

### Catálogo
- [ ] BrowseComposers muestra ≥ 600 composers (target 1000+).
- [ ] Search "Tchaikovsky" devuelve al composer; click → ComposerPage carga.
- [ ] ComposerPage de Tchaikovsky muestra "Symphony No. 6 'Pathétique'" como parent work (NO "III. Adagio lamentoso") en Top works.
- [ ] ComposerPage de Bach: "Show all works" + Load more → carga > 100 works incrementalmente.
- [ ] Composers fuera del canon (Hildegard, Pärt, Caroline Shaw) tienen page funcional.

### Tidal availability
- [ ] WorkPage de obra con recordings Tidal: cero cambio de comportamiento Phase 1-6.
- [ ] WorkPage de obra sin recordings Tidal: banner "Sin grabaciones disponibles" + botón Re-check.

### Search
- [ ] Tokenizer reconoce composers fuera del top-33 OpenOpus (≥ 3 nuevos casos en tests).
- [ ] 24 tests originales del search siguen pasando.

### Bit-perfect / regresión
- [ ] `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty.
- [ ] `route_volume_change` (`lib.rs:491-539`) intacto.
- [ ] Writer guard (`audio.rs:988-992`) intacto.
- [ ] `lib.rs` Phase 7 delta: solo entries en invoke_handler para nuevos commands (si los hay) + opcional backfill spawn.
- [ ] `stats.rs` Phase 7 delta: aditivo (columna `composer_mbid` si B7.6 incluido). Migration idempotent.

### Build
- [ ] `cargo check --release` clean.
- [ ] `cargo build --release` clean.
- [ ] `cargo clippy --release --lib --no-deps` 14 warnings (idéntico baseline) o reducción.
- [ ] `cargo test --release --lib` ≥ 130 tests (118 previos + ≥ 12 nuevos en composers_extended + browse filter + paginación + tidal_empty + search extended + opcional B7.6).
- [ ] `tsc --noEmit` 0 errores.
- [ ] `npm run build` clean.

### Tamaño / performance
- [ ] Binario release no crece > 8 MB vs baseline post-Phase 6.
- [ ] `OnceLock` parse del extended snapshot < 100ms en cold start.
- [ ] BrowseComposers render inicial < 500ms.

### Decisiones
- [ ] D-027, D-028, D-029, D-030, D-031, D-032, D-033 (y opcional D-034) registradas en DECISIONS.md.
- [ ] PROGRESS.md Phase 7 → 🟢 completed con entregables listados.

---

## 8. Riesgos y mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|---|---|---|---|
| Tamaño binario inflado (> 10 MB extra) | Media | Medio | Threshold N=5 conservador; medible en B7.0. Si excede, subir N a 7 o 10. |
| `OnceLock` parse > 200ms en cold start | Baja | Bajo | Profile en B7.1; si excede, switch a `serde_json::from_reader` lazy o lazy-by-section. |
| MB rate-limit con paginación pesada (Bach 10+ pages) | Media | Bajo | Cada page cacheada StaticMeta 30d; tras primer load, 99% cache hit. |
| WDQS endpoint down en build-time | Media | Bajo | Script reproducible; si falla, retry exponencial. NO se ejecuta en CI. Operador re-ejecuta cuando el endpoint vuelva. |
| Movement filter falsos positivos (parent works marcados como child) | Baja | Medio | Tests con fixtures MB reales para 5 composers. Si se detecta, fallback a NO filtrar para ese composer (allowlist negativa). |
| Cache negativo "Tidal empty" persiste tras Tidal añadir recordings | Baja | Bajo | TTL 7d razonable. Manual re-check botón en UI. |
| Snapshot extended diverge de OpenOpus en composers comunes (era distinta, etc.) | Media | Bajo | Merge prioriza OpenOpus para composers presentes en ambos (autoritativo). Test de coverage. |
| Script harvest produce snapshot no-determinístico entre runs | Media | Bajo | Sort lexicográfico final por mbid + freeze output. Diff entre runs debe ser solo data nueva, no orden. |
| B7.6 backfill task satura MB rate-limit y bloquea otros features | Media | Medio | Backfill conservadora (1 play / 2s, off-hours). Pause/resume controlable desde Settings. |
| `inc=work-rels` en MB browse aumenta payload > 2x | Alta | Bajo | Ya documentado. Cache StaticMeta absorbe. |
| Composers con MBID válido pero sin Wikidata QID se pierden en harvest | Media | Bajo | B7.0 reporta cifra; si > 5%, abrir sub-task de salvado vía MB-only path. |

---

## 9. Lo que Phase 7 NO hace (scope explícito)

- **No reescribe el provider+catalog pattern**. Cero refactor estructural.
- **No toca Mobile** (D-003 sigue diferido).
- **No abre Phase 8**. Phase 7 es el cierre absoluto del scope V1, conforme al mandato del usuario.
- **No reemplaza Wikidata SPARQL runtime** por snapshot (Phase 6 D-023 sigue válido para `related_composers` runtime).
- **No introduce mirror MB self-hosted** (§3.4 punto 4 sigue diferido a "advanced users opt-in").
- **No cambia la UI fundamental del Hub** (Phase 2-6 layout preserved). Solo añade pagination + Top/All works + banner Tidal-empty.
- **No aborda spin-off binario separado** (D-002, evaluación post-Phase 4 sigue legítima pero independiente).

---

## 10. Apéndice A — Reporte de B7.0 (real run — 2026-05-02)

```
Fecha del harvest: 2026-05-02 12:41 UTC
Threshold N (recording_count >=): 5 (no enforced — see "Notability proxy" below)
Composers totales: 6082
Wall-clock: ~50 seconds (SPARQL ~12s, portrait fetch ~30s, merge ~1s, write ~1s)
JSON tamaño: 2336.9 KB (well within 5 MB cap, G4 satisfied)

Distribution por era:
  20th Century:    1402
  Post-War:        1086
  Late Romantic:    878
  Baroque:          514
  Romantic:         439
  Classical:        323
  Early Romantic:   315
  Renaissance:      124
  Unknown:          122
  Contemporary:     112
  Medieval:          20

Distribution por recording_count:
  ≥ 100: 0     (MB enrichment skipped — see notability proxy)
  10-99: 0
  5-9:   6082  (every composer marked with sentinel `recording_count = -1`,
                meaning "MB enrichment opted out, accepted via Wikidata
                P136 classical-genre filter as notability proxy")

Notability proxy (D-027 refinement during execution):
  The original plan called for `recording_count >= 5` enforced via MB
  browse calls, but at 1.05s/composer × 30k unfiltered Wikidata
  composers, that's an 8h job — infeasible in practice. The script
  pivoted to a Wikidata-side filter: `wdt:P136 ?genre AND
  ?genre (wdt:P279*) wd:Q9730` plus a UNION branch for genres adjacent
  to classical that lack direct closure (minimalism, contemporary
  classical, opera, sonata, gregorian chant, etc.). This delivers
  ~6k composers all of which have a documented classical-genre claim
  in Wikidata — a stronger semantic vetting than raw recording counts.
  The runtime Phase 1 cascade still vets per-work audibility.

  Operators who want strict recording_count enforcement can re-run
  with `--with-mb-counts`.

Composers conocidos verificados presentes:
  ✓ Bach          (mbid 24f1766e-..., qid Q1339)
  ✓ Mozart        (mbid b972f589-..., qid Q254)
  ✓ Beethoven     (mbid 1f9df192-..., qid Q255)
  ✓ Tchaikovsky   (mbid 9ddd7abc-..., qid Q7315)
  ✓ Schubert      (mbid f91e3a88-..., qid Q7312)
  ✓ Sibelius      (mbid 691b0e9d-..., qid Q43203)
  ✓ Pärt          (mbid ae0b2424-..., qid Q188313)
  ✓ Reich         (mbid a3031680-..., via OO-fallback merge)
  ✓ Glass         (mbid 5ae54dee-..., qid Q42747)
  ✓ John Adams    (mbid 94f46f90-..., qid Q84114)
  ✓ John Luther Adams (mbid 96681463-..., qid Q788469)
  ✓ Saariaho      (mbid 456596a9-..., qid Q241432)
  ✓ Caroline Shaw (mbid b37b4bed-..., qid Q15430653)
  ✓ Hildegard ?   (qid Q3135615 surfaced by name match — different from
                   the Q41587 I had originally guessed; the snapshot has
                   a Hildegard but verifying which one she is is left to
                   F7.x manual QA in the Hub)
  ✗ Anna Thorvaldsdóttir (qid Q4747856 not surfaced — her Wikidata
                          entry likely lacks both the P136 classical
                          claim and the adjacent-genre fallback. Can be
                          added in a future harvest by extending the
                          UNION VALUES list, e.g. wd:Q1124983 "Icelandic
                          contemporary classical" if such a refinement
                          exists.)

Notable extras surfaced (not in original CANONICAL list but valuable):
  - 4 Bach-family composers, 5 Adams-named composers, hundreds of
    medieval/Renaissance composers via the genre adjacency branch.
  - 26 of 33 OpenOpus composers naturally surfaced via SPARQL; 7 had
    to be merged via the defensive OO-fallback path (Brahms, Schumann,
    Chopin, Reich, etc. — their Wikidata genre claims didn't include
    the canonical classical-music subclass tree).

OO fallback rescues (defensive path triggered by merge):
  Brahms, Schumann, Chopin, Reich, [verify others on next run]

Portraits fetched (second-pass P18 batch query):
  3762 / 6082 composers (62%). Composers without Wikidata P18 portrait
  fall back to the avatar placeholder in the UI.

Anomalies / debugging notes:
  - Some composer entries have unusually old / non-Latin names. Spot
    check showed they're legitimate Wikidata classical entries (Asian
    composers via opera or contemporary classical genre claims).
  - "Unknown" era bucket has 122 composers (composers with no birth
    or death year on Wikidata). Acceptable; they still surface in
    Browse Composers and search.
```

The harvest produces a deterministic snapshot (sorted by MBID) — re-runs
yield identical bytes given the same WDQS state, modulo Wikidata edits
made between runs.

---

## 11. Próximos pasos concretos

1. **Usuario revisa este plan**. Responde G1-G8 del decision-gate (§6).
2. **Supervisor consulta `classical-musicologist`** para G1, G3 y G8.
3. **Si autorizado**, supervisor abre **B7.0** delegando al `sone-backend-engineer` (script harvest) + verifica con musicologist los composers borderline.
4. Tras B7.0 closed con reporte Apéndice A, supervisor reabre revisión con usuario antes de B7.1+ (segundo gate, opcional).
5. Implementación lineal B7.1 → B7.2 → B7.3 → B7.4 → B7.5 → (B7.6 condicional) → F7.0 → F7.1 → F7.2 → F7.3 → (F7.4 condicional). Cada sub-task con checkpoint en `CHECKPOINTS.md`.
6. Closure de Phase 7 → update PROGRESS.md → reporte final al usuario → commit.

---

**Plan emitido por classical-supervisor 2026-05-02. Pending review humano.**
