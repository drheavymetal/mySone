# SONE Classical — decision log

**Append-only.** Nunca borrar ni editar entradas previas. Si una decisión se revierte o supera, añadir entrada nueva con `SUPERSEDES: D-NNN`.

Cada entrada lleva:
- ID único (`D-NNN`).
- Fecha (`YYYY-MM-DD`).
- Categoría (`ARCH | EDITORIAL | TOOLING | PROCESS | UX`).
- Owner (qué agente o human la tomó).
- Contexto, Decisión, Alternativas consideradas, Trade-offs.

---

## D-001 · 2026-05-01 · ARCH · usuario

**Contexto**: arrancamos sone-classical desde cero. Necesidad de elegir integración: nueva sidebar entry vs sub-modo de Explore vs toggle.

**Decisión**: Classical Hub vive como sub-modo dentro de Explore, accesible desde una pill prominente en el header del Explore actual. Setting opcional "Promote to sidebar" (default off) para usuarios que escuchan mucho clásica.

**Justificación**: cero regresión sobre el routing existente; reusa el patrón `explorePage` ya soportado (`App.tsx:164-174`); no bloat de sidebar; descubribilidad orgánica.

**Alternativas consideradas**:
- Sidebar top-level entry: discoverabilidad máxima pero rompe la convención (Sidebar = modos, Explore = contenido).
- Toggle Standard/Classical en Explore: confunde; el Hub tiene jerarquía propia que no cabe en el shell de Explore.

**Trade-off**: un click extra para usuarios clásicos heavy (mitigado por el setting de promoción).

**Doc afectado**: CLASSICAL_DESIGN.md §6 (alternativas) → §7 (IA).

---

## D-002 · 2026-05-01 · ARCH · usuario

**Contexto**: ¿spin-off como app separada "SONE Classical"?

**Decisión**: NO ahora. Mantener un binario único (Alternativa I del doc §17). Reevaluar tras Phase 4 si hay tracción real (≥30% plays desde Hub).

**Justificación**: cero overhead operacional, máximo reuso de código (audio backend, scrobbling, stats DB, auth Tidal), discoverability orgánica para usuarios pop que descubren el Hub.

**Alternativas consideradas**:
- Workspace con dos binarios: deferred a post-Phase 4.
- Repo separado: rechazado salvo pivote completo a otro provider.

**Trade-off**: el binario crece +5-10MB por OpenOpus snapshot + código del Hub.

**Doc afectado**: CLASSICAL_DESIGN.md §17.

---

## D-003 · 2026-05-01 · ARCH · usuario

**Contexto**: ¿Android app?

**Decisión**: deferred. No se aborda en V1. Cuando se aborde, será como **companion** (PWA → Tauri Mobile) que controla el desktop, no como standalone que compita con AMC en su terreno.

**Justificación**: el USP audiophile (bit-perfect, exclusive ALSA) no existe en Android. El nicho audiophile-móvil usa DAPs dedicados (HiBy R4 etc.), no apps Android. Standalone clasical en Android es entrar al territorio de Apple sin ventaja.

**Doc afectado**: CLASSICAL_DESIGN.md §18.

---

## D-004 · 2026-05-01 · TOOLING · usuario

**Contexto**: estilo de código para todo el proyecto.

**Decisión**: llaves siempre, incluso en one-liners (TS/JS y Rust). Calidad sobre velocidad. Mantenibilidad como métrica principal. Tests para toda lógica nueva. Comentarios solo el WHY no obvio.

**Doc afectado**: nuevo `docs/code-style.md` (autoritativo).

**Memoria persistida**: `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/feedback_code_style.md`.

---

## D-005 · 2026-05-01 · PROCESS · usuario

**Contexto**: garantía de no perder bit-perfect / exclusive audio bajo ningún concepto.

**Decisión**: el bit-perfect contract (`feedback_bitperfect_contract.md`) es MUST inviolable. Cualquier cambio que toque audio routing pasa por verificación explícita del supervisor + backend engineer + revisión humana antes de merge. Tests del contrato deben mantenerse green.

**Mecanismo de enforcement**: el `classical-supervisor` lo cita explícitamente como regla innegociable; el `sone-backend-engineer` lo verifica en cada Tauri command que toque audio o routing.

**Doc afectado**: CLASSICAL_DESIGN.md §0 TL;DR y §10 auditoría regresión.

---

## D-006 · 2026-05-01 · PROCESS · usuario

**Contexto**: el desarrollo será autonomous (agentes ejecutan, Claude principal coordina memoria/contexto). Necesidad de resumibilidad tras context resets.

**Decisión**: sistema de archivos de estado en `docs/classical/`:
- `PROGRESS.md` (estado por phase)
- `DECISIONS.md` (este log)
- `CHECKPOINTS.md` (granular, append-only)
- `AGENTS.md` (lista de agentes activos)

Más memorias persistentes en `~/.claude/projects/.../memory/`:
- `project_classical_status.md`
- `reference_classical_resume_protocol.md`

**Mecanismo**: tras cada acción significativa, actualizar checkpoints. Al iniciar sesión nueva, Claude principal sigue el protocolo en `reference_classical_resume_protocol.md`.

---

## D-007 · 2026-05-01 · TOOLING · claude-principal

**Contexto**: `.gitignore` original ignoraba `docs/` y `.claude/` con patrones agresivos (`*claude*` matchea cualquier substring). Necesidad de trackear docs operativos del proyecto y agentes project-scoped.

**Decisión**: carve-outs específicos en `.gitignore`:
- `/docs/*` ignorado, pero `!/docs/classical/` y `!/docs/code-style.md` tracked.
- `/.claude/*` ignorado, pero `!/.claude/agents/` tracked.
- `**/CLAUDE.md`, `**/.claude-session`, `**/claude-history` siguen ignorados (personal Claude state).

**Doc afectado**: `/.gitignore`.

**Trade-off**: superficie de trackeo más amplia, pero sigue protegiendo state personal de Claude.

---

## D-008 · 2026-05-01 · ARCH · usuario

**Contexto**: alcance del proyecto autonomous.

**Decisión**: TODAS las phases (0-6) deben completarse en V1. No hay V2. Cada phase probada perfectamente. Mobile diferido (no abordado en V1).

**Doc afectado**: CLASSICAL_DESIGN.md §8 (todas las phases marcadas como obligatorias V1).

---

## Plantilla para nuevas entradas

```markdown
## D-NNN · YYYY-MM-DD · CATEGORY · owner

**Contexto**: <1-3 frases situando el problema>

**Decisión**: <qué se decidió>

**Justificación**: <por qué>

**Alternativas consideradas**: <2-3 opciones rechazadas con razón breve>

**Trade-off**: <coste real de la decisión>

**Doc afectado**: <archivo:sección>

**SUPERSEDES**: D-NNN  ← solo si reemplaza una decisión previa
```
