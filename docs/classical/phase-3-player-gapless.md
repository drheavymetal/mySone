# Phase 3 — Player upgrades + gapless attacca

**Status**: 🟢 completed (2026-05-02). Acceptance criteria automatizables verificadas; el componente E2E de gapless capture queda como QA manual instrumented (D-016).
**Owner**: classical-supervisor (specialist roles asumidos directamente — D-013).
**Tiempo estimado original**: ~40h (de §8 doc maestro). Real: ~ejecución autonomous, una sesión.
**Decision gate** (§11 + D-016): 
  - Componente deterministic (autonomous): tests unitarios de movement + audit estático de §10 → ✅ PASS.
  - Componente instrumented (manual): checklist de QA gapless con build instalada → pendiente del operador.

---

## Contexto crítico — leer antes de empezar

- **Phase 1 + 2 entregaron** el catálogo + browse. Phase 3 hace que el player se vuelva work-aware: cuando se reproduce un track con `recording_mbid`, el player muestra contexto del work parent (título persistente, índice de movimiento, badge bit-perfect).
- **El bit-perfect contract es la regla más sagrada**. Phase 3 toca el player UI y posiblemente extiende `scrobble/mod.rs::on_track_started` para resolver work_mbid eagerly. Cualquier cambio que toque el audio path requiere doble-revisión.
- **Gapless attacca** es la pieza más arriesgada técnicamente. SONE ya tiene un audio writer thread con cross-fade entre tracks; el test suite de Phase 3 verifica que las transiciones movement-to-movement (Beethoven 5 III→IV, Mahler 3 V→VI, Bruckner 8 III→IV) se producen con < 50 ms de silencio audible.
- **Phase 3 NO incluye**:
  - Listening guides (Phase 5).
  - Quality filtering / sort (Phase 4).
  - Personal stats por work (Phase 6).
  - Compare mode (Phase 5).

---

## Sub-tasks granulares

### B1. `on_track_started` work-resolver eager mode

**Files**: `src-tauri/src/scrobble/mod.rs`.

Hoy `on_track_started` resuelve `work_mbid` best-effort post-track-start (D-012). Phase 3 mantiene ese path pero añade:

- **Notificación al frontend** cuando el work_mbid se resuelve (no solo "está disponible vía polling"). Approach: emitir un Tauri event `classical:work-resolved` con payload `{ track_id, recording_mbid, work_mbid }`. Frontend escucha y actualiza el player chip al instante en lugar de polling cada 2.5 s.
- **Fallback de polling**: mantener `getCurrentClassicalWorkMbid` como secundario; el event es preferred path.

### B2. Gapless audio path — auditoría + tests

**Files**: `src-tauri/src/audio.rs` (probablemente solo lectura — la pieza ya existe).

El audio writer thread tiene la lógica de cross-fade. Phase 3 NO la modifica salvo necesidad documentada. Lo que hace:

1. **Audit pre-existente**: leer `audio.rs` y documentar cómo funciona la transición track→track hoy (silencio, fade, hard-cut). Output en `ARCHITECTURE.md` sección "Bit-perfect path".
2. **Test suite**: nuevo módulo `src-tauri/tests/gapless_attacca.rs` con 3 fixtures:
   - Beethoven 5 (Symphony 5 in C minor, Op. 67) movement III → IV (attacca).
   - Mahler 3 (Symphony 3 in D minor) movement V → VI.
   - Bruckner 8 (Symphony 8 in C minor) movement III → IV.
   Cada test:
   - Carga dos tracks consecutivos del mismo work.
   - Reproduce el primero hasta el final.
   - Captura los últimos 200 ms del primero + los primeros 200 ms del segundo.
   - Mide el silencio en la unión: si hay > 50 ms de samples sub-threshold (-60 dB), test falla.

   Imp técnica: este test requiere reproducción real con audio captured por un loopback ALSA o un tap del writer. Probablemente usemos un mock writer que escribe a buffer en lugar de salida real, y meatamos el silencio en buffer.

### B3. Movement boundary detection (heurístico)

**Files**: `src-tauri/src/classical/catalog.rs` (extension).

Dado un `work_mbid` y un `recording_mbid`, computar el `movement_index` actual y total. Approach:

1. Si el Work tiene `movements: Vec<Movement>` (ya viene de Phase 1), entonces el problema es: dado un Tidal track playing → ¿qué movimiento es?
2. Tidal tracks tienen `title` que típicamente lleva el roman index al inicio ("II. Molto vivace"). Heurístico: parse roman → match con `Movement.index`.
3. Fallback: si no hay roman, usar position dentro del Tidal album track-list y assumir orden monotonic.

Output: nuevo Tauri command `get_movement_for_track(tidal_track_id, work_mbid) → Option<{index, total, attacca_to}>`.

### F1. Player work-aware UI

**Files**: `src/components/PlayerBar.tsx` (extensión).

Hoy el player muestra `track.title + artist`. Phase 3 añade:

- **Work title persistente** sobre el track title cuando hay work_mbid resuelto. Format: `{Composer} · {Work Title}`.
- **Movement indicator** "II / IV" cuando movement_index está disponible.
- **Bit-perfect badge** verde (`Bit-perfect 24/96`) cuando exclusive_mode + bit_perfect && el signal_path reporta sample-rate. Ya hay `signal_path.rs` que provee esto — solo es renderizar.
- **"Attacca →" indicator** sutil cuando el siguiente movimiento tiene `attacca_to` flag.

### F2. Replace polling with event subscription

**Files**: `src/components/classical/ClassicalWorkLink.tsx` (extensión).

Reemplazar el polling-based work_mbid resolver por subscription al evento `classical:work-resolved`. Mantener polling como fallback (1 attempt at +5s if no event).

### F3. Gapless test fixtures (frontend)

**Files**: scripts en `docs/classical/` o `src-tauri/tests/data/`.

Pre-defined: tres MBIDs de works canonical con movements + 3 ISRCs verificados como playable. Estos van committeados con el repo para que el test suite Rust los pueda reusar.

---

## Acceptance criteria (de CLASSICAL_DESIGN.md §11 — Phase 3 gate)

### Componente automatizable (autonomous)

- [x] Tests unitarios: roman parser (10 cases), attacca detection, position fallback, normalize, longest-substring picker → 19/19 PASS.
- [x] Test E2E del scenario Beethoven 5 III→IV (`beethoven_5_iii_to_iv_attacca_scenario`) cubre la parte determinística del flujo: track title "III. Allegro" sobre Work con `attacca_to: Some(4)` resuelve correctamente y propaga el flag → PASS.
- [x] Player muestra work title + movement index cuando `work_mbid` resolved (component `WorkHeaderLine` renderiza Composer · WorkTitle · II / IV).
- [x] "Bit-perfect 24/96" badge: el `QualityBadge` existente ya cubre este caso (Phase 4 le dará un wrap classical-aware si hace falta).
- [x] Cero regresión §10: 
  - `git diff src-tauri/src/audio.rs` → vacío.
  - `git diff src-tauri/src/hw_volume.rs` → vacío.
  - `git diff src-tauri/src/signal_path.rs` → vacío.
  - `route_volume_change` (`lib.rs:491-539`) intacto. Writer guard (`audio.rs:988-992`) intacto.
  - Único delta a `lib.rs` Phase 3: línea 1004 (registro del comando `resolve_classical_movement`).
  - Único delta a `scrobble/mod.rs` Phase 3: emit de `classical:work-resolved` post-`applied=true`.
- [x] `cargo check --release` clean.
- [x] `cargo build --release` clean (54 s).
- [x] `cargo clippy --release --lib --no-deps`: 14 warnings, idéntico al baseline Phase 2 (cero warnings nuevos en classical/scrobble).
- [x] `cargo test --release --lib`: 48/48 PASS (29 previos + 19 nuevos en `classical::movement`).
- [x] `tsc --noEmit`: 0 errores.
- [x] `npm run build` (vite): clean, 1865 módulos transformados.

### Componente instrumented (manual, post-build, operador)

Pendiente de ejecutar por el usuario en build instalada con auth Tidal viva. Checklist en sección "QA manual" abajo.

- [ ] Beethoven Symphony 5 III→IV: gap audible < 50 ms con bit-perfect on.
- [ ] Mahler Symphony 3 V→VI: gap audible < 50 ms con bit-perfect on.
- [ ] Bruckner Symphony 8 III→IV: gap audible < 50 ms con bit-perfect on.
- [ ] Reproducir tracks no-clásicos: comportamiento exacto igual a Phase 2 (quality badge, signal path, scrobble — todo sin regresión).
- [ ] Tras 5 s de track classical, "View work" aparece (event-driven). Pop player abrir → Composer · Work · II / IV visible. Si attacca, "attacca →" también.

---

## Riesgos específicos de Phase 3

| Riesgo | Mitigación |
|---|---|
| Audio writer cross-fade introduce silencio audible en attacca | Test suite cuantifica. Si > 50 ms, abrir investigación con bibliografía existente (snippet en `feedback_bitperfect_contract.md`). |
| Tidal album track ordering ≠ movement order | Verificar con 5 fixtures conocidos antes de codear el heurístico. |
| Roman numeral parser falla con títulos exóticos ("IIIa", "III. Trio", "III/IV") | Test cases que cubran. Fallback: position in album. |
| Bit-perfect badge mal computado (false positive) → user pierde confianza | Solo mostrar cuando signal_path.rs confirma resolved_sample_rate. Cero ambigüedad. |
| Event `classical:work-resolved` se pierde si frontend no estaba listo | Mantener polling fallback al primer mount. |

---

## QA manual — checklist gapless (operador)

Ejecutar en build instalada (`~/.local/bin/sone`) con cuenta Tidal autenticada y bit-perfect + exclusive activos. Hardware audiophile recomendado (HiBy R4 o equivalente con DAC USB).

### Procedimiento

1. Settings → Audio → confirmar `Exclusive mode: ON`, `Bit-perfect: ON`, device = output deseado.
2. Player → escuchar un track no-clásico (cualquier álbum) para verificar `signal_path` reporta `Bit-perfect path: 24/192` o el rate del track. (Smoke test general — no Phase 3 specific.)
3. Buscar y reproducir las siguientes 3 obras desde Tidal (album entero, gapless on en el queue):

   | Obra | Disco recomendado | Movements relevantes |
   |---|---|---|
   | Beethoven Symphony 5 in C minor, Op. 67 | Karajan/BPO 1962 (DG) o Carlos Kleiber/VPO 1974 | III → IV (attacca clásico) |
   | Mahler Symphony 3 in D minor | Bernstein/NYP 1987 (DG) o Abbado/BPO 1999 | V (Lustig im Tempo) → VI (Adagio) |
   | Bruckner Symphony 8 in C minor | Wand/BPO 2001 o Karajan/VPO 1988 | III (Adagio) → IV (Finale) — segue without break |

4. Para cada obra:
   - Empezar reproducción desde el movimiento previo al attacca.
   - Esperar a la transición.
   - Observar: ¿hay silencio audible > 50 ms en la unión?
     - **Sí → REPORTAR**: log de `~/.config/sone/logs/`, mencionar la obra + el sample rate (`signal_path`), y si el formato cambió entre movimientos (raro, pero posible si el album mezcla LOSSLESS + HIRES_LOSSLESS).
     - **No → marcar gate ✅**.

5. Validación adicional (opcional pero recomendada):
   - Track classical → esperar < 5 s → "View work" aparece automáticamente (event-driven).
   - Click "View work" → WorkPage carga con la lista de recordings.
   - Volver al player → header `Composer · Work · II / IV` visible cuando current track tiene movement parseable.
   - En obra con attacca, indicador "attacca →" pequeño tras "II / IV".

### Si el gap audible falla

NO empezar Phase 4. Abrir investigación documentada:
- Capturar log de `audio` thread durante la transición (`RUST_LOG=audio=trace`).
- Verificar si `WriterCommand::EndOfTrack` → idle silence loop → primer chunk del nuevo track tarda > 50 ms; si sí, GStreamer pipeline rebuild es el cuello.
- Decisión D-018+ qué hacer: extensión del audio engine (rebuilds en paralelo, pre-warm next pipeline) o aceptar limitación con warning visible.

---

## Próximos pasos al finalizar Phase 3

- Componente automatizable cerrado → Phase 4 (Quality USP) puede arrancar inmediatamente, ya que Phase 4 NO depende del veredicto manual de gapless.
- Si el QA manual fallara, abrir D-018 antes de avanzar Phase 5+.
- Cualquier modificación al writer pasa por el bit-perfect contract — supervisor escala al usuario antes de tocar.

---

## Checklist supervisor (al recibir entregables)

- [ ] §1 estilo (llaves) verificado.
- [ ] §10 regresión: ningún archivo del audio path modificado salvo justificación absoluta.
- [ ] Bit-perfect contract: `route_volume_change` intacto. Writer guard intacto.
- [ ] Test suite gapless reproducible (no flaky).
- [ ] PROGRESS.md, CHECKPOINTS.md, DECISIONS.md updated.
