# Phase 6 — Personalization + deferred Phase 5 features

**Status**: ⚪ pending — Phase 5 → 🟢 completed habilita el arranque.
**Owner**: TBD.
**Tiempo estimado**: ~50h (recibe ~30h originales + ~20h de los diferidos por D-022).
**Decision gate** (§11): "Tu top work clásico computado correctamente desde stats DB" + Wikidata-backed related composers funcional.

---

## Contexto crítico — leer antes de empezar

- Phase 6 cierra todo lo que el doc maestro CLASSICAL_DESIGN.md prevé. Tras esto el Hub está completo.
- El bit-perfect contract sigue sagrado: cero cambios en `audio.rs`, `hw_volume.rs`, `signal_path.rs`, `route_volume_change`, writer guard, ni `tidal_api.rs::get_stream_url`.
- Recibe deuda de Phase 5: D-022 difirió Wikidata SPARQL + related composers + browse-by-conductor. **Esa deuda se paga aquí, no se vuelve a diferir.**

---

## Objetivos concretos

### Objetivos originales Phase 6 (CLASSICAL_DESIGN.md §8 Phase 6)

1. **Tus top works clásicos** — agregación stats por `work_mbid` (ya persistido desde Phase 1, columna `plays.work_mbid`).
2. **Tu discovery curve clásica** — filtra el discovery curve actual a plays con `work_mbid` no nulo.
3. **Recording comparison personal** — para una obra con N versiones escuchadas, side-by-side con play count + completion rate.
4. **Library facets en el Hub** — `Saved Works`, `Saved Recordings`, `Saved Composers`, `Saved Performers` (la tabla `classical_favorites` Phase 1 ya soporta estos kinds).
5. **Pre-warm de canon en background** — primer launch del Hub: traer top-30 OpenOpus composers + sus 10 popular works en cola asíncrona.

### Objetivos diferidos desde Phase 5 (D-022)

6. **Wikidata SPARQL provider** — `enrich_composer` con P528 (catalog), P826 (key), P571 (composition year), P18 (portrait HD), P136 (genre overlap → related composers).
7. **Related composers** — sección en ComposerPage debajo de "Symphonies/Concertos/...". Usa P136 + same era heuristic.
8. **Browse por conductor / orquesta / soloist** — drill-down desde RecordingRow (click en conductor name → su discografía cross-MB).

---

## Sub-tasks granulares (V1 propuesta)

### Backend

#### B6.1 — Top classical works (stats query)

Nuevo Tauri command `get_top_classical_works(window: StatsWindow, limit: u32) → Vec<TopWork>`.

- SQL: `SELECT work_mbid, COUNT(*), SUM(listened_secs), MAX(album), MAX(album_artist) FROM plays WHERE started_at >= ?1 AND work_mbid IS NOT NULL GROUP BY work_mbid ORDER BY COUNT(*) DESC LIMIT ?2`.
- Returns `TopWork { work_mbid, plays, listened_secs, sample_title, sample_artist }`.
- Hidrata work title via `CatalogService::get_work` perezosamente en frontend (cache hit típico).

#### B6.2 — Classical discovery curve

Reusa la query `get_discovery_curve` existente con un `WHERE work_mbid IS NOT NULL` extra. Entry point: `get_classical_discovery_curve(window)`.

#### B6.3 — Recording comparison

Para un `work_mbid`, devolver todas las grabaciones de ese work que el usuario ha tocado, con `(recording_mbid, plays, listened_secs, completed_count)`.

`get_classical_recording_comparison(work_mbid) → Vec<RecordingComparisonRow>`.

#### B6.4 — Classical favorites CRUD

Reusa `classical_favorites` Phase 1. Commands:
- `add_classical_favorite(kind, mbid, display_name)`.
- `remove_classical_favorite(kind, mbid)`.
- `list_classical_favorites(kind, limit) → Vec<ClassicalFavorite>`.

#### B6.5 — Pre-warm canon

Background task spawned en `AppState::new` (after auth ready) que itera top-30 composers OpenOpus + sus 10 popular works, calls `get_work(mbid)` con tolerancia a 503 / rate limit. Store progress in app-state for UI feedback ("Warming up... 12/30 composers").

#### B6.6 — Wikidata SPARQL provider

Nuevo `src-tauri/src/classical/providers/wikidata.rs`:
- HTTP client compartido + cache `Dynamic` (4h TTL, 24h SWR).
- Endpoint: `https://query.wikidata.org/sparql?format=json`.
- Properties: P528 (catalog), P826 (key Q-item → resolve via secondary call), P571 (year), P18 (portrait), P136 (genre overlap).
- `enrich_composer` extrae `qid` desde MB (relation `wikidata`), runs SPARQL one-shot con todos los properties, fills `Composer.qid` + `composition_year` proxy.
- `list_related_composers(qid)` → SPARQL: composers compartiendo P106=Q36834 (composer) + P136 ∩ + birth_year ±50.

#### B6.7 — Browse por conductor / orquesta / soloist

- Nueva tabla derivada en stats? No: query directa contra MB.
- `get_artist_recordings_by_role(artist_mbid, role) → Vec<RecordingSummary>` con cache StaticMeta.
- Frontend wire: click en conductor name → navega a `classical://artist/{mbid}?role=conductor`.

### Frontend

#### F6.1 — Hub Library tab

Activar tab "Library" en `ClassicalHubPage.tsx` con sub-tabs `Works | Recordings | Composers | Performers`. Cada uno renderiza grid filtrable.

#### F6.2 — Top works section

Nueva sección en Hub Listen Now: "Your top classical works". Usa `getTopClassicalWorks(window)`.

#### F6.3 — Discovery curve filter

En StatsPage, toggle "Classical only" para el discovery curve chart.

#### F6.4 — Recording comparison view

Nueva ruta `classical://compare/{work_mbid}`. Renderiza side-by-side de grabaciones que el user ha tocado.

#### F6.5 — Save buttons

Heart icon en WorkPage / ComposerPage / RecordingRow para add/remove a favorites.

#### F6.6 — Related composers section

`ComposerPage.tsx`: sección "Related composers" debajo de la última work-type group. Renderiza ComposerCard chips (5-10).

#### F6.7 — Click conductor → discografía

Conductor / orchestra / soloist names en RecordingRow hover-clickable. Click → `classical://artist/{mbid}?role=conductor` page con su lista cross-work.

---

## Acceptance criteria (de §11 doc maestro Phase 6)

- [ ] "Tu top work clásico" ranking computa correctamente desde stats DB (test rust acceptance con fixture rows).
- [ ] Discovery curve "Classical only" filter respeta `work_mbid IS NOT NULL`.
- [ ] Save/unsave round-trip persiste en `classical_favorites`.
- [ ] Pre-warm canon termina en < 5 minutos en cold cache para top-30 composers (con MB rate-limit 1.1 s/req → 30 × ~3s ≈ 90s baseline; quotas razonable).
- [ ] Wikidata SPARQL devuelve related composers para Beethoven (≥ 5 — Mozart, Schubert, Brahms, Haydn, Schumann).
- [ ] Click en conductor name → discografía page funcional.
- [ ] Cero regresión §10:
  - audio.rs / hw_volume.rs / signal_path.rs / tidal_api.rs sin cambios.
  - Schema migration aditiva (no DROP/ALTER de tablas existentes).
- [ ] `cargo check`, `cargo clippy --release --lib --no-deps`, `cargo build --release`, `cargo test --release --lib classical::`, `tsc --noEmit`, `npm run build` clean.
- [ ] Tests unitarios: top-works query + favorites CRUD + Wikidata SPARQL parse + related composers heuristic.

---

## Riesgos específicos de Phase 6

| Riesgo | Mitigación |
|---|---|
| Wikidata SPARQL endpoint slow / down | Cache 4h + degradación graciosa (`null` → render "Related composers unavailable"). |
| Pre-warm canon corre fuera de control en cold cache | Task cancellable; throttle 1 req/2s en lugar de 1.1 s para no saturar MB. |
| Recording comparison desemboca en N+1 lookups | Pre-fetch cached Work entity para todos los recording_mbid distintos en una sola pasada. |
| Library tab balloon de favorites | Pagination + virtualization en grid. |
| Browse-by-conductor requires new MB endpoint | Use `/recording?artist={mbid}&inc=...` con role attribute filter post-fetch. |
| Top works UI diverge si user no tiene work_mbid plays | Fallback empty state con CTA "Play a classical track to populate this". |

---

## Próximos pasos al finalizar Phase 6

- Tras Phase 6 → mySone Classical V1 está **funcionalmente completo** según CLASSICAL_DESIGN.md.
- Phase 7+ (no V1): Mobile companion, community editorial sync, FLAC local matching, mirror MB self-hosted.
- Evaluación post-Phase 6: si telemetría muestra ≥ 30% plays vienen del Hub (D-002), considerar Alternativa II (binario separado SONE Classical).

---

## Checklist supervisor (al recibir entregables internos)

- [ ] §1 estilo (llaves) verificado en cada archivo nuevo.
- [ ] §10 regresión: audio path intacto.
- [ ] PROGRESS.md, CHECKPOINTS.md, DECISIONS.md updated.
- [ ] Tests unitarios cubren stats queries + favorites CRUD + Wikidata SPARQL + related composers.
- [ ] Pre-warm canon NO bloquea startup (background task asíncrono).
- [ ] Wikidata atribución (CC0 Q-items, sí; pero el endpoint en sí debe respetar el SPARQL service usage policy: identifying user-agent + reasonable rate).
