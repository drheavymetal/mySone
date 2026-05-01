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
