# Phase 8 — Polish, cleanup y search streaming

**Status**: 🟡 in_progress
**Started**: 2026-05-02
**Owner**: classical-supervisor (autonomous, discreción full per mandato del usuario)

---

## 0. TL;DR

Phase 7 cerró el catálogo amplio (6082 composers, 138/138 tests, audio path intacto). Phase 8 es **polish + cleanup + search streaming** sin nueva funcionalidad mayor. Es la última pasada antes de commit.

**Drivers**:
- Mandato del usuario textual: *"si busco compositores de forma vaga o albumes o piezas u obras, lo que quiero es que salgan un maximo de X pero que vayan saliendo segun las vayas encontrando con un loading al final"* → search incremental con cap visible y spinner-tail.
- Carta blanca para cualquier mejora que el equipo detecte como senior dev/diseñador audit.

**Reglas**:
- Audio path §10 — cero diff. Verificar `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty al cierre.
- Tests ≥138 manteniendo verde, target ≥145 con coverage del search streaming nuevo.
- Llaves siempre. Calidad sobre velocidad.
- Cualquier scope grande que aparezca → checkpoint state=blocked y reportar.

---

## 1. Sub-tasks (priorizadas)

### B8.1 — Search streaming backend (PRIORIDAD del usuario)

Refactorizar `search_classical` de "compute-all-then-return" a emit-as-found.

**Estado actual** (Phase 5/7):
- `CatalogService::search_classical(&query, limit) -> Result<SearchResults>` síncrono.
- Calcula plan → enumera works del composer (si hay match) → fallback snapshot scan top-40 composers cuando no hay composer match → sort → truncate → return.
- Para queries vagas como "concerto" sin composer match, recorre 40 composers × N works cada uno antes de devolver nada.

**Refactor propuesto**:
- Nuevo command `search_classical_streaming(query, query_id, limit)` que emite eventos Tauri:
  - `classical:search-plan` — `{ queryId, plan }` inmediato tras tokenize/plan.
  - `classical:search-hit` — `{ queryId, hit }` por cada match emitido (deduplicated por workMbid).
  - `classical:search-done` — `{ queryId, totalEmitted, truncated }` al final.
- El backend respeta `query_id`; si una query nueva entra mientras la anterior corre, no podemos cancelar mid-flight (no mantenemos el handle), pero el frontend ignora hits con `queryId` viejo.
- El handler del comando hace `tokio::spawn` para no bloquear el caller; retorna `()` inmediatamente.
- `search_classical` síncrono se preserva (no breaking change para tests existentes).
- Cap default 50, configurable via param.

**Por qué events y no `tauri::ipc::Channel`**:
- El codebase no usa Channel en ningún sitio. `app_handle.emit(...)` es el patrón establecido (scrobble bulk-import, miniplayer events, tray events, scrobble auth-error, classical work-resolved).
- Consistencia > novedad. Decisión D-035.

**Tests**:
- Unit: la helper interna `enumerate_search_hits<F: FnMut(SearchHit)>(plan, ..., emit_fn)` se testa en isolación. Mock emit_fn into Vec, run vs golden cases.
- Edge: query vacía, query "concerto" sin composer, query con composer top-1, cap respected.
- Mantener todos los tests Phase 5 (24) intactos: search síncrono sigue funcionando.

### F8.1 — Search streaming frontend

Refactor `ClassicalSearch.tsx`:
- En lugar de `await searchClassical()` y mostrar todo o nada, suscribirse a `classical:search-hit` y appendear inmediato.
- `query_id` local (incrementa por cada nuevo runSearch). Hits con queryId viejo se descartan.
- Mostrar spinner pequeño al final de la lista mientras `done` no llega.
- Plan chips renderean en cuanto llega `search-plan`.
- Resultado: el usuario tipea "concerto" y empieza a ver results al instante en lugar de esperar el barrido completo.

### B8.2 — Audit ligero estados loading/empty/error

Pasada de senior dev sobre los componentes nuevos Phase 7:
- `BrowseComposers.tsx` — empty state cuando filter no coincide; error state si fetch falla.
- `ComposerPage.tsx` — empty state en "Full catalog" si works=0 tras paginar; error state en `loadMoreWorks`.
- `WorkPage.tsx` — banner Tidal-unavailable copy + accessible label.
- `ClassicalHubPage.tsx` — footer chip si extendedTotal=0 (loading vs error).
- `ClassicalSearch.tsx` — propio del search streaming.

Mensajes de error: actionable, no genéricos. "Failed to load works for {composer}. Check connection and try again." en lugar de "Failed to load".

### B8.3 — Botón "Re-check Tidal" feedback

WorkPage tiene `handleRecheckTidal` que invoca `recheckClassicalWorkTidal`. Hoy: click → 5-15s wait → re-render. No hay loading state visible en el botón.

Fix: `recheckLoading` state, disabled + spinner inline durante la llamada.

### B8.4 — Documentación de operador

Un mini README dentro de `docs/classical/` apuntando a:
- "Cómo regenerar el snapshot extended" (link al Python script).
- "Cómo recargar editorial.json" (manual edit + rebuild).
- "Cómo poblar listening guides" (formato LRC + path).
- "Cómo verificar bit-perfect en classical playback" (instrumented manual de Phase 3).

Útil para el operador (el usuario) y para futuros maintainers. Discreto, no marketing.

### B8.5 — D-034 (composer-resolution stats) — evaluación

Mandato: "si hay budget tras Phase 7, cerrar; si pasa de la mitad del esfuerzo de Phase 7, déjalo como deuda V1+".

Phase 7 fue ~6h efort estimado. Half = ~3h. B8.5 estimado ~4h (migration + resolver extension + scrobble extension + backfill task + tests + frontend StatsPage refresh). **Veredicto inicial: deuda V1+ explícita confirmada**, no se ejecuta en Phase 8. Re-confirmamos D-034-status como deferred y cerramos el bucle.

### F8.2 — Microinteracciones / polish visual

Pasada visual menor:
- `BrowseComposers` Load more button: hover state + disabled state when no more.
- `ComposerPage` Load more works: idem.
- `ClassicalHubPage` footer chip: hover affordance clarifica que es CTA.
- `FavoriteToggle`: animación heart fill smooth (no abrupta).
- Verificar que loading skeletons usan tokens `th-surface/40` consistente.

### B8.6 — Regression smoke check

Antes del cierre Phase 8:
- `cargo check --release` clean.
- `cargo build --release` clean.
- `cargo clippy --release --lib --no-deps`: 14 warnings idéntico baseline.
- `cargo test --release --lib`: ≥145 PASS (138 baseline + ~7 nuevos search streaming).
- `tsc --noEmit`: 0 errores.
- `npm run build`: clean.
- `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` → empty.
- Routing intacto: `route_volume_change` (lib.rs) sin diffs no relacionados.
- Writer guard (audio.rs:988-992) intacto.

---

## 2. Sub-tasks NO incluidas (deuda explícita V1+)

- **B7.6/F7.4** (composer-resolution stats) — re-evaluado en B8.5, queda deferred. Esfuerzo > budget.
- **MB Lucene fallback en search** — diseño Phase 5/6 lo mencionaba como "Phase 6+". El streaming no lo necesita; resuelve el UX issue ("ver hits según vienen") sin necesidad de ampliar al MB endpoint. Si en uso real el catálogo in-process se queda corto, es una iteración futura (V1.1+).
- **Cancelable search server-side** — el frontend ignora hits con queryId viejo, suficiente. Cancellable nativo requiere mantener `JoinHandle` en AppState + Mutex; complejidad no proporcional al valor.
- **Mobile** — D-003 (no V1).
- **Snapshot offline para Wikipedia/Wikidata** — V1.1+.

---

## 3. Decisiones nuevas previstas

- **D-035** — Search streaming via Tauri events `app_handle.emit(...)` por consistencia con el patrón establecido (no Channel).
- **D-036** — Query-id-based dedup en frontend (cliente-side cancellation suficiente).
- D-037+ — surgirán durante implementación.

---

## 4. Acceptance criteria (§11 doc maestro adapted)

### Search streaming
- ⏳ Search "concerto" muestra primer hit < 200ms, hits siguen apareciendo, spinner al final hasta `done`.
- ⏳ Search "Beethoven 9 Karajan 1962" empieza a mostrar resultados al instante (composer match path).
- ⏳ Cambio de query mid-search: hits viejos descartados, nuevos hits aparecen con queryId nuevo.
- ⏳ Cap=50 respetado: no más de 50 hits visibles aunque el backend encuentre más (truncated=true en done).

### Audit
- ⏳ Cero pantallas con "Failed to load" sin contexto.
- ⏳ Cero botones que hacen network call sin feedback (loading state).
- ⏳ Empty states presentes y útiles donde correspondan.

### Regression
- ⏳ `git diff src-tauri/src/{audio,hw_volume,signal_path,tidal_api}.rs` empty.
- ⏳ `route_volume_change` (lib.rs:491-539) intacto.
- ⏳ Writer guard (audio.rs:988-992) intacto.
- ⏳ Tests ≥145/145.
- ⏳ Build clean (cargo + tsc + vite).

---

## 5. Plan de ejecución

1. Plan + checkpoint inicial (este doc + PROGRESS.md update + checkpoint phase-8-init). [DONE]
2. B8.1 backend search streaming → checkpoint B8.1-completed.
3. F8.1 frontend search streaming → checkpoint F8.1-completed.
4. B8.2 audit pass → checkpoint B8.2-completed.
5. B8.3 rechek-tidal feedback → checkpoint B8.3-completed.
6. F8.2 microinteracciones → checkpoint F8.2-completed.
7. B8.4 docs operador → checkpoint B8.4-completed.
8. B8.5 D-034 re-evaluation → registro DECISIONS.md (deferred reaffirmed) → checkpoint B8.5-completed.
9. B8.6 regression smoke + Phase 8 final checkpoint.
10. PROGRESS.md → Phase 8 🟢 completed.

Las sub-tasks F8.x se intercalan donde corresponda (F8.1 inmediatamente tras B8.1, F8.2 puede ir paralelo a B8.4).
