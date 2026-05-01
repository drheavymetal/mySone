# SONE Classical — checkpoints granulares

**Append-only.** Cada checkpoint refleja un punto de retomada estable.

> Este archivo permite reanudar el trabajo tras un context reset. Cada entrada describe el estado en un instante concreto: qué se acaba de hacer, qué viene después, qué archivos se tocaron, si los tests pasan.

---

## Cuándo escribir un checkpoint

- Al iniciar una phase.
- Al completar un sub-task del phase.
- Antes de cualquier operación destructiva (rm, git reset, branch -D).
- Al final de cada respuesta autonomous significativa (>10 min de trabajo).
- Al detectar un blocker que requiere humano.
- Al final de cada sesión.

---

## Formato

```markdown
## YYYY-MM-DD HH:MM · Phase N · short-id-task

**State**: in_progress | blocked | completed | aborted
**Last action**: <qué se acaba de hacer (1-2 frases)>
**Next action**: <qué se hace al retomar (1-2 frases)>
**Files touched**: 
  - path/to/file.ext (descripción breve)
**Tests**: pass | fail | n/a (con detalle)
**Build**: pass | fail | n/a
**Notes**: <contexto adicional, blockers, decisiones pendientes>
```

---

## Checkpoint history

### 2026-05-01 22:35 · meta · bootstrap

**State**: completed

**Last action**: bootstrap del sistema de gestión del proyecto autonomous. Memorias guardadas (code-style, project status, resume protocol, índice MEMORY.md actualizado). Branch `soneClassical` creada desde master (410fd36). LFM import committeado a master. `.gitignore` con carve-outs para `docs/classical/` y `.claude/agents/`.

**Next action**: bootstrap de docs/classical/ files restantes (AGENTS.md, ARCHITECTURE.md, phase-0-spike.md scaffold) → actualizar agentes con braces style + resume protocol → commit inicial soneClassical → handoff a `classical-supervisor` para iniciar Phase 0.

**Files touched**:
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/feedback_code_style.md` (nuevo)
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/project_classical_status.md` (nuevo)
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/reference_classical_resume_protocol.md` (nuevo)
  - `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/MEMORY.md` (índice actualizado)
  - `.gitignore` (carve-outs para classical docs y agents)
  - `docs/code-style.md` (nuevo)
  - `docs/classical/README.md` (nuevo)
  - `docs/classical/PROGRESS.md` (nuevo)
  - `docs/classical/DECISIONS.md` (nuevo, con D-001..D-008)
  - `docs/classical/CHECKPOINTS.md` (este archivo, nuevo)

**Tests**: n/a (bootstrap, no production code)
**Build**: n/a

**Notes**: Master en 410fd36 (LFM import). Stash `wip-classical-design-and-agents` aplicado a soneClassical y resuelto. Doc maestro en `/CLASSICAL_DESIGN.md` está versionado en la rama. Los 4 agentes están en `.claude/agents/` pero faltan actualizar con braces style + resume protocol.

---

### 2026-05-01 22:50 · meta · infra-completo + handoff bloqueado por runtime

**State**: blocked (necesita restart de Claude Code)

**Last action**: completado el bootstrap de infraestructura — los 4 agentes (`.claude/agents/*.md`) actualizados con resume protocol + braces rule + obligación de checkpoint write-back. `docs/classical/` completo (README, PROGRESS, DECISIONS con D-001..D-008, CHECKPOINTS, AGENTS, ARCHITECTURE skeleton, phase-0-spike.md plan). Branch `soneClassical` con commit inicial `3f6121a` (14 archivos, 2878 inserciones).

Intento de invocar `classical-supervisor` desde la sesión actual de Claude Code para arrancar Phase 0. **Falló**: "Agent type 'classical-supervisor' not found". Los agentes project-scoped en `.claude/agents/` se cargan en el arranque de Claude Code, no en caliente. La sesión actual fue iniciada antes de que existieran los archivos de agente, por eso el dispatcher no los reconoce.

**Next action** (al retomar en sesión nueva):

1. **Verificar contexto** — Claude principal de la nueva sesión carga automáticamente las memorias `project_classical_status.md` + `reference_classical_resume_protocol.md`. Si no, las lee manualmente.
2. **Verificar branch** — `git branch --show-current` debe ser `soneClassical`.
3. **Confirmar agentes disponibles** — al estar la nueva sesión iniciada con los archivos `.claude/agents/*.md` en sitio, los 4 agentes deben aparecer como `subagent_type` invocables.
4. **Invocar `classical-supervisor`** con el prompt de kickoff de Phase 0 (preservado abajo en sección "Prompt de retomada para classical-supervisor").
5. El supervisor toma el control y ejecuta Phase 0 al completo (Step 0.1 a 0.5).

**Files touched** (este checkpoint):
  - `.claude/agents/classical-supervisor.md` (resume protocol + braces rule añadidos)
  - `.claude/agents/classical-musicologist.md` (resume protocol + persistencia editorial añadidos)
  - `.claude/agents/sone-backend-engineer.md` (resume protocol + braces rule añadidos)
  - `.claude/agents/sone-frontend-engineer.md` (resume protocol + braces rule añadidos)
  - `docs/classical/README.md` (nuevo)
  - `docs/classical/AGENTS.md` (nuevo)
  - `docs/classical/ARCHITECTURE.md` (skeleton)
  - `docs/classical/phase-0-spike.md` (plan completo de Phase 0)

**Tests**: n/a
**Build**: n/a

**Notes**: el sistema está 100% listo para arrancar Phase 0 en cuanto la nueva sesión cargue los agentes. Cero blockers de diseño o decisión — solo el restart de Claude Code es necesario. El usuario fue informado.

---

## Prompt de retomada para classical-supervisor (usar al primer turno de la nueva sesión)

> **Para el classical-supervisor en la próxima sesión** (copiar literal al invocar via Agent tool):

```
El usuario ha autorizado el desarrollo autonomous completo de SONE Classical. Tu trabajo: tomar el control y dirigir Phase 0 hasta entregar una decisión GO/NO-GO con datos reales.

# Contexto que debes cargar PRIMERO (en orden estricto)

1. /home/drheavymetal/myProjects/mySone/CLASSICAL_DESIGN.md (todo, especialmente §0, §3, §4, §8 Phase 0, §11)
2. /home/drheavymetal/myProjects/mySone/docs/classical/PROGRESS.md
3. /home/drheavymetal/myProjects/mySone/docs/classical/CHECKPOINTS.md (último checkpoint = bootstrap completado)
4. /home/drheavymetal/myProjects/mySone/docs/classical/DECISIONS.md (D-001..D-008)
5. /home/drheavymetal/myProjects/mySone/docs/classical/phase-0-spike.md (plan detallado del spike)
6. /home/drheavymetal/myProjects/mySone/docs/code-style.md

# Mandato del usuario (textual, 2026-05-01)

- Todas las phases (0..6) se completan en V1, sin diferir nada.
- Calidad sobre velocidad; mantenibilidad como métrica primaria.
- Llaves siempre, incluso one-liners (TS/JS y Rust).
- Bit-perfect + exclusive audio MUST inviolables.
- Mobile diferido (no V1).
- Delegas a classical-musicologist para repertorio, sone-backend-engineer para Rust, sone-frontend-engineer para UI.
- Puedes crear más agentes si lo consideras necesario para una phase concreta.

# Tu tarea inmediata: ejecutar Phase 0 spike completo

Sigue exactamente el plan de docs/classical/phase-0-spike.md (steps 0.1 a 0.5):

- Step 0.1: resolver MBIDs reales de las 5 obras canon vía MB API.
- Step 0.2: delegar al sone-backend-engineer la implementación del script standalone en src-tauri/examples/spike_isrc_coverage.rs (no toca producción, sin side-effects).
- Step 0.3: supervisar el run.
- Step 0.4: generar report con tablas y análisis en phase-0-spike.md.
- Step 0.5: registrar decisión en DECISIONS.md como D-009. Si GO o GO-con-asterisco, abrir Phase 1 en PROGRESS.md y crear phase-1-foundation.md scaffold.

Reglas innegociables: cero regresión, bit-perfect intacto, llaves siempre, cada acción significativa → checkpoint en CHECKPOINTS.md, cada decisión → entrada en DECISIONS.md.

Si encuentras blockers que requieren humano (auth Tidal expirada, MB 503 sostenido), para, escribe checkpoint con state=blocked, y reporta. No improvises decisiones humanas.

Tu output final: resumen ejecutivo (status, cobertura %, decisión, próximos pasos, files modificados, checkpoint escrito).
```
