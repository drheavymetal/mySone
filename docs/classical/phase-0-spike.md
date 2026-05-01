# Phase 0 — Spike de viabilidad

**Status**: 🟡 starting (2026-05-01)
**Owner**: `sone-backend-engineer` (delegado por `classical-supervisor`)
**Tiempo estimado**: 30h
**Decision gate**: cobertura ISRC ≥ 70% canon mayor → GO; 50-70% → GO con asterisco; < 50% → REPLANTEAR.

---

## Objetivo

Validar las dos hipótesis críticas antes de invertir esfuerzo en Phase 1:

1. **Cobertura ISRC en Tidal** para grabaciones canónicas. Si Tidal no tiene los ISRCs que MusicBrainz reporta, el Hub no puede reproducir lo que muestra. Hipótesis: ≥ 70% para canon mayor (DG/Decca/EMI/Sony), con caída esperada en grabaciones legendarias pre-1960.
2. **Latencia real de un Work page** con MB rate-limit (1 req/s). Hipótesis: < 60s para una obra popular en cold cache, gracias a lazy enrichment.

---

## Obras canon objetivo

Cinco obras representativas de distintos contextos:

| Obra | MBID | Justificación |
|---|---|---|
| Beethoven — Symphony No. 9 in D minor, Op. 125 "Choral" | `9722d000-94d1-3000-93f2-000000000000` (placeholder, resolver en spike) | Canon supremo, ~200 recordings, cobertura DG/Decca/EMI completa |
| Bach — Goldberg Variations BWV 988 | (resolver) | Canon barroco, mix de versiones piano + clavecín, HIP relevante |
| Mozart — Requiem K. 626 | (resolver) | Canon coral, múltiples completiones (Süssmayr, Levin, Maunder) |
| Mahler — Symphony No. 9 | (resolver) | Romanticismo tardío, cobertura DG/Sony fuerte |
| Glass — Glassworks | (resolver) | Composer vivo, minimalist, smaller catalog en MB → test de coverage en repertorio post-1980 |

---

## Plan de ejecución

### Step 0.1 — Resolución de MBIDs reales

Antes de implementar el script, resolver los 5 work_mbids manuales (vía `https://musicbrainz.org/work/?query=...` o equivalente). Documentar los MBIDs reales en este archivo, en la tabla anterior.

### Step 0.2 — Implementación del script

Script standalone en `src-tauri/examples/spike_isrc_coverage.rs` o `scripts/spike_isrc_coverage/` (decidir en sesión). Requirements:

- **Cumple code-style.md §1**: llaves siempre.
- **Reusa infraestructura existente**:
  - `MusicBrainzLookup` (rate limiter compartido) o un cliente equivalente que respete el 1 req/s.
  - `TidalClient` con auth válida (de las settings encriptadas).
- **Hardcodea las 5 obras canon** inicialmente (los MBIDs resueltos en step 0.1).
- **Por cada obra**:
  1. `GET /ws/2/work/{mbid}?inc=recording-rels&fmt=json` → lista de recordings.
  2. Para cada recording (max 25 por obra, paginar si hace falta):
     - `GET /ws/2/recording/{mbid}?inc=isrcs&fmt=json` → ISRCs.
     - Para cada ISRC, intentar Tidal lookup (vía `tidalapi` ISRC search).
     - Marcar la recording como `playable | unavailable | error`.
     - Capturar quality tier si playable: `LOSSLESS | HIRES_LOSSLESS | DOLBY_ATMOS | MQA`.
  3. Reportar wall-clock de procesar la obra completa.
- **Output**: imprimir markdown a stdout con tablas resumen.
- **Cero side-effects en producción** (no escribir a stats DB, no modificar caches reales, no contar como plays).

### Step 0.3 — Run

Ejecutar el script. Se espera que tarde varios minutos por la rate limit de MB (1 req/s × ~250 calls = ~5 min). Capturar el output en bruto.

### Step 0.4 — Análisis y report

Procesar el output a un report estructurado en este mismo archivo, sección "Resultados". El report debe incluir:

- Tabla resumen: por obra, total recordings, % playable, breakdown de quality tiers.
- Tabla de wall-clocks por obra.
- Casos notables: grabaciones legendarias confirmadas missing en Tidal (Furtwängler '51, Toscanini, etc.).
- Análisis de rate-limit budget consumido.
- Recomendación: GO / GO con asterisco / NO-GO.

### Step 0.5 — Decisión

El `classical-supervisor` revisa el report y registra la decisión en `DECISIONS.md` con ID `D-009` (o el siguiente disponible).

Si **GO**, abrir Phase 1 en `PROGRESS.md` y crear `phase-1-foundation.md`.
Si **GO con asterisco**, documentar las mitigaciones necesarias y abrir Phase 1.
Si **NO-GO**, escalar al usuario con análisis y opciones (Qobuz, IMSLP fallback, restructurar el Hub).

---

## Resultados

> _Pendiente de Step 0.4. Esta sección se rellenará tras la ejecución del script._

### Resumen por obra

| Obra | Total recordings MB | Recordings con ISRC | Playable en Tidal | % playable | Wall-clock |
|---|---|---|---|---|---|
| Beethoven 9 | — | — | — | —% | — |
| Bach Goldberg | — | — | — | —% | — |
| Mozart Requiem | — | — | — | —% | — |
| Mahler 9 | — | — | — | —% | — |
| Glass Glassworks | — | — | — | —% | — |
| **Promedio** | — | — | — | —% | — |

### Quality breakdown (sobre playable)

| Tier | Count | % |
|---|---|---|
| HIRES_LOSSLESS | — | —% |
| LOSSLESS | — | —% |
| DOLBY_ATMOS | — | —% |
| MQA | — | —% |
| (sin tag) | — | —% |

### Casos notables

> _Listar grabaciones legendarias confirmed missing en Tidal._

### Decisión

> _GO / GO con asterisco / NO-GO + razón._

---

## Riesgos detectados durante Phase 0

> _Append durante la ejecución._
