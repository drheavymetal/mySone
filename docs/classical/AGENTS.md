# SONE Classical — agentes activos

Los agentes que dirigen el desarrollo del Classical Hub. Definidos en `.claude/agents/` y disponibles automáticamente en cualquier sesión Claude Code en este repo.

---

## Roster

| Agente | File | Rol | Modelo | Cuándo invocarlo |
|---|---|---|---|---|
| `classical-supervisor` | `.claude/agents/classical-supervisor.md` | Guardián del CLASSICAL_DESIGN.md, autoridad final, delega y verifica conformidad | opus | **Siempre primero** para cualquier trabajo del Hub |
| `classical-musicologist` | `.claude/agents/classical-musicologist.md` | Experto cultural (repertorio, catálogos, performers, sellos, Editor's Choice) | opus | Decisiones de contenido, terminología, repertorio, editorial |
| `sone-backend-engineer` | `.claude/agents/sone-backend-engineer.md` | Senior Rust/Tauri con conocimiento profundo de SONE (bit-perfect, MB rate limit, cache, schema) | sonnet | Implementación backend |
| `sone-frontend-engineer` | `.claude/agents/sone-frontend-engineer.md` | Senior React + diseñador (theme `th-*`, patrones de componente, animaciones) | sonnet | Implementación frontend / UI / diseño visual |

---

## Flujo de control canónico

```
        Usuario o Claude principal pide algo del Hub
                          │
                          ▼
              ┌─────────────────────────┐
              │  classical-supervisor   │
              │  (lee CLASSICAL_DESIGN  │
              │   identifica phase,     │
              │   verifica criteria)    │
              └────────────┬────────────┘
                           │ delega con contrato:
                           │   - sección del doc
                           │   - acceptance criteria
                           │   - lista NO-tocar (§10)
                           │   - tiempo estimado (§8)
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
   musicologist        backend            frontend
   (repertorio)        (Rust/Tauri)        (React)
        │                  │                  │
        └──────────────────┴──────────────────┘
                           │ devuelven trabajo
                           ▼
              supervisor verifica:
                ✓ §11 acceptance
                ✓ §10 cero regresión
                ✓ code-style.md (braces siempre)
                ✓ tests verdes
                ✓ docs actualizados (PROGRESS, CHECKPOINTS, DECISIONS)
                           │
                           ▼
                aprueba o rechaza con cita
                escribe checkpoint en CHECKPOINTS.md
```

---

## Reglas de colaboración

### Quién decide qué

| Tipo de decisión | Decide |
|---|---|
| ¿Está justificada esta nueva tarea? ¿En qué phase encaja? | `classical-supervisor` |
| ¿Qué grabación es la canónica de Beethoven 9? ¿Qué obras destacar de Mahler? | `classical-musicologist` |
| ¿Cómo estructuro el provider trait? ¿Qué TTL para este cache? | `sone-backend-engineer` (dentro de los marcos del doc) |
| ¿Cómo se ve esta tarjeta? ¿Qué hover state? | `sone-frontend-engineer` (dentro de §16 del doc) |
| ¿Cambia la arquitectura? | `classical-supervisor` con escalada al usuario si aplica |
| ¿Cambia el bit-perfect contract? | **Escala al usuario siempre** — nadie lo decide unilateralmente |

### Cómo invocan

Cualquier agente puede invocar a otro vía el tool `Agent` con el `subagent_type` correspondiente. La regla:
- **El supervisor** es el orquestador habitual; los demás se llaman entre sí solo cuando el supervisor lo autoriza.
- Si el `sone-frontend-engineer` necesita decidir terminología musicológica, **delega al musicologist** sin preguntar al supervisor.
- Si el `sone-backend-engineer` encuentra un trade-off arquitectónico no previsto, **escala al supervisor**, no decide solo.

### Identidad y autoría en commits

Los commits se firman como cualquier commit Claude:
```
Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
```
No se diferencia qué agente concreto hizo qué; el supervisor es responsable del PR completo.

---

## Cómo añadir agentes adicionales

Si una phase necesita especialización extra (por ejemplo `audio-qa-engineer` para Phase 3 gapless tests, o `sparql-specialist` para queries Wikidata complejas), el supervisor puede crear sub-agentes adicionales:

1. Crear `.claude/agents/<nombre>.md` con frontmatter `name`, `description`, `tools`, `model`.
2. Documentar en este archivo (`AGENTS.md`) — añadir fila a la tabla de roster.
3. Registrar en `DECISIONS.md` con categoría `PROCESS`.
4. El supervisor sigue siendo el único punto de delegación inicial.

**Importante**: NO crees agentes para escenarios genéricos (debugging, refactoring) — esos los maneja Claude principal o los engineers existentes. Crea agentes solo cuando hay especialización persistente que se reutilizará en múltiples sesiones.

---

## Estado actual del roster

**Activos**: 4 (los del roster).
**Pendientes de creación**: ninguno por ahora. El supervisor evaluará en cada phase si necesita auxiliares.
