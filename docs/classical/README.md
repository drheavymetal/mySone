# SONE Classical — documentación del proyecto

**Branch**: `soneClassical` · **Started**: 2026-05-01 · **Status**: en desarrollo activo

Este directorio contiene **toda la documentación operativa** del proyecto SONE Classical. El diseño maestro vive en `/CLASSICAL_DESIGN.md` (raíz del repo); aquí está lo que cambia con el desarrollo: estado, decisiones, checkpoints, agentes activos.

---

## Índice

| Archivo | Para qué sirve | Cuándo se actualiza |
|---|---|---|
| [`README.md`](README.md) | Este índice | Cuando se añade un doc |
| [`PROGRESS.md`](PROGRESS.md) | Estado por phase: pending / in_progress / completed con checkpoint actual | Tras cada cambio significativo de estado |
| [`DECISIONS.md`](DECISIONS.md) | Log append-only de decisiones arquitectónicas con justificación | Cada vez que se toma una decisión que se desvía del doc original o que el doc no cubría |
| [`CHECKPOINTS.md`](CHECKPOINTS.md) | Checkpoints granulares — sirven para retomar tras context reset | Tras cada acción significativa, mínimo al final de cada sesión |
| [`AGENTS.md`](AGENTS.md) | Lista de agentes activos, su rol, su file path | Cuando se añade/modifica un agente |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Síntesis viva de la arquitectura — actualizada cuando emerge del código | A medida que se implementan los providers / catalog service |
| [`phase-0-spike.md`](phase-0-spike.md) | Plan, ejecución y resultados del spike de viabilidad | Al inicio y fin de Phase 0 |
| `phase-1-foundation.md` | Foundation: catalog service + Work page (un fichero por phase) | Al inicio de Phase 1 |
| `phase-2-browse.md` | Browse experience: composer pages, browse axes, search inicial | Al inicio de Phase 2 |
| `phase-3-player.md` | Player upgrades + gapless test suite | Al inicio de Phase 3 |
| `phase-4-quality.md` | USP de calidad audio: filtros, sort, compare mode | Al inicio de Phase 4 |
| `phase-5-editorial.md` | Editorial layer + search avanzado | Al inicio de Phase 5 |
| `phase-6-personalization.md` | Stats integration personal | Al inicio de Phase 6 |

---

## Cómo navegar este directorio según tu rol

### Si eres el usuario humano
- **Ver progreso global**: `PROGRESS.md`.
- **Entender por qué se tomó una decisión**: `DECISIONS.md`.
- **Saber dónde pararse y retomar**: `CHECKPOINTS.md`.

### Si eres Claude principal (orquestador de memoria)
- **Cargar al iniciar sesión**: `PROGRESS.md` → `CHECKPOINTS.md` → `DECISIONS.md`.
- **Tras context reset**: seguir `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/reference_classical_resume_protocol.md`.

### Si eres el `classical-supervisor`
- **Antes de delegar**: leer la phase activa en `PROGRESS.md` y verificar criterios de §11 del doc maestro.
- **Tras recibir trabajo**: actualizar `CHECKPOINTS.md` con el resultado.
- **Cuando aprendes algo del doc maestro**: registrar en `DECISIONS.md` con cita al CLASSICAL_DESIGN.md afectado.

### Si eres el `classical-musicologist`
- **Para validar consistencia editorial**: revisar entradas tipo `EDITORIAL` en `DECISIONS.md`.

### Si eres `sone-backend-engineer` o `sone-frontend-engineer`
- **Antes de empezar**: leer phase actual en `PROGRESS.md` + `phase-N-*.md` + `ARCHITECTURE.md`.
- **Antes de tocar archivos no triviales**: verificar `DECISIONS.md` por restricciones previas.

---

## Reglas de oro

1. **Estos archivos viven en git**. Cualquier cambio se commitea con su trabajo asociado.
2. **Los fichero de phase son append-only durante esa phase**, read-only después salvo errata.
3. **`DECISIONS.md` es estrictamente append-only** — nunca borrar ni editar entradas previas (si una decisión se revierte, se añade entrada nueva con `SUPERSEDES: D-NNN`).
4. **`CHECKPOINTS.md` puede tener entradas obsoletas archivadas** (mover a `CHECKPOINTS-archive-YYYY-MM.md` cuando crece >500 líneas).
5. **`PROGRESS.md` es la fuente de verdad del estado**. Si los archivos contradicen `PROGRESS.md`, gana `PROGRESS.md`.

---

## El doc maestro

Recuerda: las decisiones de diseño NO viven aquí. Viven en `/CLASSICAL_DESIGN.md` raíz del repo. Este directorio es **operativo** (qué se ha hecho, cuándo, por qué); el doc maestro es **prescriptivo** (qué debe construirse).

Si la realidad demuestra que el doc maestro está mal, se actualiza el doc maestro **y** se registra el cambio en `DECISIONS.md` aquí.
