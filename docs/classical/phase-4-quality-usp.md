# Phase 4 — Quality USP

**Status**: ⚪ pending (Phase 3 → 🟢 completed habilita el arranque).
**Owner**: TBD (`sone-backend-engineer` para refinement de sample-rate / bit-depth + aggregator; `sone-frontend-engineer` para UI columna calidad + filter chips + sort + bit-perfect player badge).
**Tiempo estimado**: ~40h (de §8 doc maestro).
**Decision gate** (§11): filtro "Hi-Res only" en Beethoven 9 muestra solo HIRES_LOSSLESS, sort by quality muestra 24/192 primero.

---

## Contexto crítico — leer antes de empezar

- Phase 0..3 dejaron en pie: catalog + browse + player work-aware + gapless deterministic. Phase 4 es **el USP central** del Classical Hub vs AMC: comparación de grabaciones por calidad audio.
- AMC NO permite ordenar/filtrar grabaciones por calidad ([What Hi-Fi], [Apple Discussions thread 254762608], [Audiophilia]). SONE Classical sí lo hará — con datos reales del Tidal manifest.
- El bit-perfect contract sigue siendo el invariante. Ningún cambio en `audio.rs`, `hw_volume.rs`, `signal_path.rs`, ni `route_volume_change`.
- **Phase 4 NO incluye**:
  - Listening guides (Phase 5).
  - Search clásico avanzado (Phase 5).
  - Personalization stats (Phase 6).
  - Compare mode side-by-side (Phase 5+).

---

## Objetivos concretos

1. **Columna `Quality` en RecordingRow** con badge color-coded:
   - 🟢 `HIRES_LOSSLESS 24/192` (premium tier).
   - 🟢 `HIRES_LOSSLESS 24/96` (alto).
   - 🔵 `LOSSLESS 16/44.1` (CD).
   - 🟡 `MQA` (legacy, controvertido — explícito).
   - 🟣 `DOLBY_ATMOS` (immersive — bonus).
   - ⚫ Not on Tidal (info-only).
2. **Filter chips** arriba de la lista de recordings: `Hi-Res only`, `Atmos`, `Sample-rate ≥ 96k`, `Sin MQA`, `Año desde…`.
3. **Sort dropdown**: Popularity / Year (newest first) / Year (oldest first) / Audio quality (best first) / Conductor A-Z.
4. **Header del Work page** con badge agregado: "Best available: 24/192" si la mejor recording llega a esa calidad.
5. **Player bit-perfect indicator** (extends current `QualityBadge` integration): when current track is HIRES_LOSSLESS + DAC negociado iguales → "Bit-perfect 24/96" verde. Esto YA existe parcialmente vía `signal_path` + `QualityBadge`; Phase 4 lo refina y lo expone más prominente.

---

## Sub-tasks granulares

### Backend

#### B4.1 — Sample-rate / bit-depth refinement por track

Tidal `mediaMetadata.tags` da el tier (`HIRES_LOSSLESS`) pero no la rate exacta hasta que se abre el manifest. SONE ya consulta el manifest en stream-time vía `signal_path.rs`. Phase 4 necesita la rate **antes** de stream para mostrar "24/96" en la lista de recordings.

- Nuevo método en `TidalProvider`: `fetch_track_quality_detail(track_id) → Result<{ sample_rate, bit_depth, codec, audio_quality }>`. Reusa el endpoint que `signal_path` consulta (manifest fetch); cachea por `track_id` con TTL Dynamic (24h).
- Llamado batched desde `CatalogService::build_work_fresh` después del cascade matching, paralelo a hasta 6 manifests a la vez. Cap a top-20 recordings por work (los demás muestran solo el tier sin rate hasta hover/play).

**Files**: `src-tauri/src/classical/providers/tidal.rs` (+ test del cache key), `src-tauri/src/classical/types.rs` (extend `Recording` con `sample_rate_hz?: u32`, `bit_depth?: u8`).

#### B4.2 — Aggregator "best quality available" por work

Pure helper en `catalog.rs` o `quality.rs`: dado `Vec<Recording>`, devuelve el tier máximo + rate. Usa una ordenación canónica:
```
DOLBY_ATMOS > HIRES_LOSSLESS 24/192 > HIRES_LOSSLESS 24/96 >
HIRES_LOSSLESS 24/48 > LOSSLESS 16/44.1 > MQA > Not-found
```

Expose via `WorkSummary.best_available_quality?: { tier, sample_rate, bit_depth }` y refleja en `Work.best_available_quality`. Tests unitarios sobre el ranking.

**Files**: `src-tauri/src/classical/quality.rs` (NEW, pure logic + tests), `src-tauri/src/classical/types.rs`.

#### B4.3 — Tauri commands

- Nuevo: `refresh_work_recording_qualities(work_mbid)` — opcional, force-refresh el batch de manifests si el usuario abre una página y quiere ver las calidades exactas que faltaron en cold cache.

### Frontend

#### F4.1 — RecordingRow con columna Quality

- Refactor `RecordingRow.tsx`: nueva sub-componente `QualityChip.tsx` (color-coded por tier + rate). Render dentro de la fila junto a conductor / orchestra / year / label.
- Hover sobre el chip → tooltip con la query de match + el `match_score` (vínculo a confidence, ya existente).

**Files**: `src/components/classical/QualityChip.tsx` (NEW), `src/components/classical/RecordingRow.tsx` (extension).

#### F4.2 — Filter + Sort UI

- Nuevo `RecordingFilters.tsx` con chips: Hi-Res only / Atmos / ≥ 96k / sin MQA / Year ≥ N.
- Nuevo `RecordingSort.tsx` dropdown.
- `WorkPage.tsx` mantiene un local state con `{ filters, sort }` aplicado memoized sobre `work.recordings`. NO refetch.

**Files**: `src/components/classical/RecordingFilters.tsx` (NEW), `src/components/classical/RecordingSort.tsx` (NEW), `src/components/classical/WorkPage.tsx` (extension).

#### F4.3 — "Best available" badge en WorkPage header

- Banner pequeño bajo el título del work: `Best available: 24/192 HIRES_LOSSLESS`. Click → activa filtro Hi-Res en la lista (UX hint).

**Files**: `src/components/classical/WorkPage.tsx`.

#### F4.4 — Player bit-perfect indicator refinement

- El `QualityBadge` ya cubre el caso. Phase 4 solo asegura que cuando `signal_path.bit_perfect && exclusive_mode` el badge muestra "BIT-PERFECT" en lugar de solo "HI-RES LOSSLESS". El cambio es 4 líneas de UI dentro de `QualityBadge.tsx`. Cero impacto en routing.

**Files**: `src/components/QualityBadge.tsx` (extension cosmética).

---

## Acceptance criteria (de CLASSICAL_DESIGN.md §11 — Phase 4 gate)

- [ ] Filter "Hi-Res only" en Beethoven 9 → muestra solo HIRES_LOSSLESS rows.
- [ ] Sort by quality (best first) → 24/192 al top, 24/96 luego, 16/44.1 al fondo.
- [ ] Header del work page muestra "Best available" cuando >= 1 recording HIRES_LOSSLESS.
- [ ] Player bit-perfect badge: cuando track HIRES_LOSSLESS + signal_path.bit_perfect=true → label "BIT-PERFECT" verde.
- [ ] Cero regresión §10:
  - audio.rs / hw_volume.rs / signal_path.rs sin cambios.
  - QualityBadge.tsx — extensión cosmética solamente.
  - RecordingRow / WorkPage / api/classical / types/classical extendidos aditivamente.
- [ ] `cargo check`, `cargo clippy --lib`, `cargo build --release`, `cargo test --lib classical::`, `tsc --noEmit`, `npm run build` clean.
- [ ] Tests unitarios: ranking de quality (`classical::quality`), aggregator "best of N", cache key del manifest fetch.

---

## Riesgos específicos de Phase 4

| Riesgo | Mitigación |
|---|---|
| Manifest fetch para 60 tracks rompe rate limit Tidal | Cap a top-20 por work + paralelismo limitado a 6 + cache 24h. Override force-refresh manual. |
| Tidal manifest endpoint cambia | El módulo `tidal_api.rs` ya lo usa para `signal_path`; reutilizamos su path para no duplicar el riesgo. |
| MQA detection ambigua (track con tag MQA + HIRES_LOSSLESS simultáneo) | Política: si MQA está presente, mostrar warning chip naranja independiente del tier máximo. |
| Filter chips multiplican re-renders en la lista | `useMemo` sobre `applyFilters(work.recordings, filters, sort)`; rows usan `memo`. |

---

## Próximos pasos al finalizar Phase 4

- Si gate ✅ → Phase 5 (Editorial layer + search avanzado).
- Phase 4 cierra el USP central. Tras esto, mySone Classical tiene paridad funcional + ventaja audiophile sobre AMC.

---

## Checklist supervisor (al recibir entregables)

- [ ] §1 estilo (llaves) verificado.
- [ ] §10 regresión: audio path intacto.
- [ ] PROGRESS.md, CHECKPOINTS.md, DECISIONS.md updated.
- [ ] Tests unitarios cubren los rankings y la aggregator.
- [ ] El "Best available" del work page reacciona a refresh.
