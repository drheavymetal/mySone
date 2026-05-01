# SONE Classical — progress tracker

**Última actualización**: 2026-05-01 22:50 (bootstrap completado)
**Phase activa**: Phase 0 (spike de viabilidad) — _ready to start, blocked on Claude Code session restart_
**Branch**: `soneClassical` (commit `3f6121a`)
**Build status**: master baseline `410fd36` (LFM import + unified Stats merged); soneClassical baseline `3f6121a` (bootstrap)
**Blocker**: agentes project-scoped en `.claude/agents/` necesitan que Claude Code reinicie sesión para ser invocables. No hay otro blocker.

> **Esta es la fuente de verdad del estado del proyecto.** Cualquier discrepancia con otros archivos se resuelve mirando aquí.

---

## Vista global de phases

| # | Phase | Status | Started | Completed | Checkpoint actual | Owner |
|---|---|---|---|---|---|---|
| 0 | Spike de viabilidad | 🟡 starting | 2026-05-01 | — | bootstrap | sone-backend-engineer (delegado) |
| 1 | Foundation (catalog + 1 Work page) | ⚪ pending | — | — | — | — |
| 2 | Browse experience | ⚪ pending | — | — | — | — |
| 3 | Player upgrades + gapless | ⚪ pending | — | — | — | — |
| 4 | Quality USP | ⚪ pending | — | — | — | — |
| 5 | Editorial + search avanzado | ⚪ pending | — | — | — | — |
| 6 | Personal listening integration | ⚪ pending | — | — | — | — |

**Leyenda**: ⚪ pending · 🟡 in_progress · 🟢 completed · 🔴 blocked

---

## Phase 0 — Spike de viabilidad

**Objetivo** (de CLASSICAL_DESIGN.md §8): validar dos hipótesis críticas antes de invertir más:
1. Cobertura ISRC en Tidal para grabaciones canónicas (5 obras × ≤25 recordings cada).
2. Latencia de carga real de un Work page con MB rate-limit.

**Decision gate**: cobertura ≥ 70% canon mayor → GO. 50-70% → GO con asterisco. < 50% → REPLANTEAR.

### Entregables
- [ ] Script Rust standalone en `scripts/spike-isrc-coverage.rs` (o similar binario en `src-tauri/examples/`).
- [ ] Lista de las 5 obras canon: Beethoven 9, Bach Goldberg, Mozart Requiem, Mahler 9, Glass Glassworks.
- [ ] El script: dado work_mbid, hace `recording-rels` lookup en MB, intenta ISRC inverse Tidal por cada recording, reporta % playable + audio quality breakdown.
- [ ] Output: report markdown en `docs/classical/phase-0-spike.md` con números reales.
- [ ] Decisión documentada en `DECISIONS.md` (GO / GO con asterisco / NO-GO).

### Tareas
- [ ] **0.1** — Configurar entorno: el script vive en el workspace cargo de `src-tauri/`, reusa cliente HTTP existente, MusicBrainzLookup y el TidalClient con auth válida.
- [ ] **0.2** — Implementar `spike_isrc_coverage` con las 5 obras canon hardcoded inicialmente. Cumplir code-style §1 (llaves siempre).
- [ ] **0.3** — Run sobre las 5 obras; capturar wall-clock de cada lookup + breakdown de quality tiers.
- [ ] **0.4** — Generar report markdown con tablas resumen + decisión.
- [ ] **0.5** — Supervisor revisa report y registra en `DECISIONS.md` la decisión final.

### Acceptance criteria (de §11 doc maestro)
- ISRC coverage ≥ 70% canon mayor → GO.
- Tiempo wall-clock para Work page completa < 60s en cold cache (con MB rate limit 1 req/s).
- Tests ejecutables, repetibles, documentados.

---

## Phase 1 — Foundation

**Objetivo**: catalog service + 1 Work page funcional con datos reales de MB, cache, reproducción Tidal. Punto de entrada: botón "View work" en el player.

**Pendiente de detallar** — Phase 0 GO requerido antes.

---

## Phase 2 — Browse

**Pendiente de detallar** — Phase 1 completed requerido.

---

## Phase 3 — Player + gapless

**Pendiente de detallar** — Phase 2 completed requerido.

---

## Phase 4 — Quality USP

**Pendiente de detallar** — Phase 3 completed requerido.

---

## Phase 5 — Editorial + search

**Pendiente de detallar** — Phase 4 completed requerido.

---

## Phase 6 — Personalization

**Pendiente de detallar** — Phase 5 completed requerido.

---

## Cambios al doc maestro

Si durante el desarrollo se actualiza `/CLASSICAL_DESIGN.md`, registrarlo aquí:

| Fecha | Sección afectada | Tipo cambio | Razón |
|---|---|---|---|
| — | — | — | — |
