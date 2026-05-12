# Phase 10 — Editorial scaling (USP "About this work" como eje diferencial)

**Status**: 📝 plan listo, pendiente ejecutar tras Phase 9.
**Estimación**: ~160-170h (10.1: 50-60h + 10.2: 90h + 10.3: 20h).
**Decisiones**: D-046 (hybrid scaling) + D-044 (USP schema) + D-045 (snapshot v2).
**Bloqueado por**: Phase 9.C cerrada (necesita WorkPage rediseñada + provider editorial extended + 3 POC validados por Pedro).

---

## Por qué Phase 10

Pedro (2026-05-04):

> "ademas estaba bien poner informacion sobre la propia obra, como punto distintivo"

Apple Music Classical e Idagio tienen editorial limitado (Editor's Choice + breve nota; ~80 palabras). mySone propone 1200 palabras estructuradas en 5 sub-secciones (D-044) — un competitive edge documentable.

Phase 9.C entrega el **infrastructure** (snapshot v2 + provider + UI `AboutThisWork`) + **3 POC**. Phase 10 entrega el **contenido al volumen necesario** para que el USP sea real, no demo.

---

## Etapa 10.1 — Top 50 manual (50-60h, ~6 semanas)

### Scope

50 obras canon escritas a mano con plantilla 5 sub-secciones (origin / premiere / highlights / context / notable_recordings). 1200 palabras × obra. Sources cited en cada entry.

### Lista canónica propuesta

#### Beethoven (5 obras)
- Symphony No. 9, Op. 125 ("Choral")
- Symphony No. 5, Op. 67
- Symphony No. 7, Op. 92
- Symphony No. 3, Op. 55 ("Eroica")
- Piano Sonata No. 14, Op. 27 No. 2 ("Moonlight")

#### Mozart (5 obras)
- Requiem, K. 626
- Don Giovanni, K. 527
- Piano Concerto No. 21, K. 467
- Symphony No. 41, K. 551 ("Jupiter")
- Eine kleine Nachtmusik, K. 525

#### Bach (5 obras)
- Mass in B Minor, BWV 232
- Goldberg Variations, BWV 988
- St Matthew Passion, BWV 244
- Brandenburg Concertos, BWV 1046-1051 (entry agrupada)
- Cellosuites, BWV 1007-1012 (entry agrupada)

#### Mahler (3 obras)
- Symphony No. 2 ("Resurrection")
- Symphony No. 5
- Das Lied von der Erde

#### Brahms (3 obras)
- Symphony No. 1, Op. 68
- Symphony No. 4, Op. 98
- Ein deutsches Requiem, Op. 45

#### Schubert (3 obras)
- Winterreise, D. 911
- Piano Sonata D. 960 (B♭ major)
- String Quartet "Death and the Maiden", D. 810

#### Chopin (3 obras)
- Nocturnes (entry agrupada Op. 9, 15, 27, 32, 37, 48, 55, 62, 72)
- Ballades (entry agrupada Op. 23, 38, 47, 52)
- Piano Concerto No. 1, Op. 11

#### Debussy (3 obras)
- La Mer
- Préludes (Books I & II)
- Pelléas et Mélisande

#### Stravinsky (3 obras)
- Le Sacre du printemps
- Petrushka (ballet completo)
- Pulcinella

#### Shostakovich (3 obras)
- Symphony No. 5, Op. 47
- Symphony No. 8, Op. 65
- String Quartet No. 8, Op. 110

#### Tchaikovsky (3 obras)
- Symphony No. 6 ("Pathétique"), Op. 74
- Piano Concerto No. 1, Op. 23
- Swan Lake (ballet)

#### Otros canónicos (12 obras)
- Vivaldi: Le Quattro Stagioni, RV 269/315/293/297
- Handel: Messiah, HWV 56
- Haydn: The Creation, Hob. XXI:2
- Wagner: Der Ring des Nibelungen (entry agrupada)
- Verdi: La traviata
- Puccini: La bohème
- Dvořák: Symphony No. 9 "From the New World", Op. 95
- Sibelius: Symphony No. 5, Op. 82
- Rachmaninov: Piano Concerto No. 2, Op. 18
- Ravel: Boléro / Daphnis et Chloé (suite agrupada)
- Bartók: Concerto for Orchestra, Sz. 116
- Britten: War Requiem, Op. 66

### Plantilla per entry

```markdown
## {composer}: {work_title}, {catalog}

### Origin & commission
{600-300 palabras: quién encargó, cuándo, por qué, dedicatoria, manuscrito}

### Premiere & reception
{200-300 palabras: fecha, lugar, intérpretes, recepción crítica inicial}

### Musical highlights
{200-300 palabras: key changes, motifs, instrumentación, structural notes accesibles}

### Historical context
{200-300 palabras: lugar en obra del compositor, época, influencias, legacy}

### Notable recordings
{100-200 palabras: brief essay sobre 3-5 grabaciones de referencia con conductor + año + sello}

### Sources
- Wikipedia: <url>
- Wikidata: Q<id>
- Editor: mySone team
```

### Proceso

1. Musicologist propone draft (~1h/obra).
2. Supervisor + Pedro review.
3. Translation ES (~30 min/obra).
4. Embed en `editorial-extended.json`.

### Estimación

50 obras × 1.5h drafting + 0.5h review + 0.5h translation = 2.5h × 50 = 125h. **Realistic con paralelización musicologist + Pedro: 50-60h en 6 semanas.**

---

## Etapa 10.2 — Top 200 LLM-assisted (~90h)

### Scope

150 obras canónicas adicionales (top-200 minus top-50 manual) generadas via pipeline LLM-assisted con spot-check 20% obligatorio.

### Pipeline

```
Para cada work_mbid en top-200 \ top-50:
  1. Fetch Wikipedia full article (REST API /page/segments).
  2. Fetch Wikidata claims (P50, P88, P1191, P710, P179, P138).
  3. Prompt LLM (Claude Opus o equivalente) con plantilla estricta:
     "Eres un musicólogo. Resume las siguientes fuentes en 5 secciones:
      origin, premiere, highlights, context, notable_recordings.
      800-1200 palabras totales. Cita atribuciones con [wikipedia], [wikidata].
      NO inventes fechas, intérpretes, ni eventos. Si una fuente no cubre
      una sección, escribe `null`. notable_recordings ES OPCIONAL — si las
      fuentes no listan grabaciones específicas, devuelve null."
  4. Output JSON parseable por editorial-extended schema v2.
  5. Validation: (a) 20% random spot-check humano, (b) flag cualquier
     entry con notable_recordings poblado para revisión obligatoria.
  6. Embed en editorial-extended.json con disclaimer.
```

### Disclaimer UI obligatorio

Sección "About this work" muestra footer cuando `extended.sources` contiene `{"kind": "llm-assisted", "model": "..."}`:

> **Note**: This editorial draws from Wikipedia and Wikidata, summarized with AI assistance. Spot-checked by our team but may contain errors. [Suggest correction →]

### Estimación

- Build script Python (fetch + prompt + parse): 6h.
- Prompts iteration + golden test cases: 10h.
- Run sobre 150 obras (background, ~30 min/obra LLM time): 75h LLM time + 5h human dispatch.
- Validación humana 20% spot-check (30 obras × 30 min): 15h.
- Validación notable_recordings (todas las que lo tengan, ~50% × 150 = 75 obras × 15 min): 19h.
- Tests integration: 5h.
- **Total**: ~50h human + 75h LLM = ~50h human-attended. Round up a 90h por iteration.

### GO/NO-GO criterion

Spot-check 20% obligatorio (30 obras). Umbral: **0 alucinaciones detectadas en fechas, intérpretes, eventos**. Si > 0, NO-GO ampliar Etapa 10.2 → revertir a manual scaling Etapa 10.1.

---

## Etapa 10.3 — Long tail Wikipedia-only (~20h)

### Scope

Obras 200-2000 (~1500 obras restantes del canon ampliado). `editor_note` breve auto-generado (Wikipedia first paragraph + cleanup). Sin sección "About this work" extended; fallback al behavior Phase 5.

### Pipeline

```
Para cada work_mbid en top-2000 \ top-200:
  1. Fetch Wikipedia summary (REST API /page/summary).
  2. Limpieza: strip de markup residual, primer parágrafo solo.
  3. Embed en editorial.json schema v1 (Phase 5 compat).
  4. Sin sección extended.
```

### Estimación

- Build script: 4h.
- Run + cleanup: 6h.
- Validation rápida (sample 5%): 5h.
- Tests: 5h.
- **Total**: ~20h.

---

## Etapa 10.4 — Crowdsourcing (V2+, NO V1)

Diferido. Requiere sync infrastructure + moderation + UI para submit. No bloquea V1.

V2: usuario puede escribir extended notes locales para sus works favoritos en `~/.config/sone/listening-guides/{work_mbid}.note.md`. Sync futuro vía Obsidian-LiveSync.

---

## Acceptance criteria

### Etapa 10.1
- 50 works canon: extended note ≥ 1200 palabras con 5 secciones.
- Source attribution visible.
- Validación humana cruzada (musicologist + supervisor).

### Etapa 10.2
- 150 works LLM-assisted: disclaimer visible.
- Spot-check 20% pasado (0 alucinaciones).
- `editorial-extended.json` size < 5 MB.

### Etapa 10.3
- 1500+ works long tail: editorial_note breve generado.
- `editorial.json` v1 ampliado, backward compat preservada.

### Validation gate Phase 10 → close
- Sources audit: 0 alucinaciones detectadas en 20% spot-check de Etapa 2 (umbral GO/NO-GO).
- Pedro confirma sample de 10 obras random que cumplen el estándar editorial pedido.

---

## Phase 10.5 — Browse axes adicionales (opcional, ~20h)

(Gap MEDIO musicólogo #6 + #7.)

- **Browse by Instrument** (Piano music / Violin music / Cello music / Guitar music / Organ music). Datasource: agrupar `works.work_type` + heurística título. `bucket_for` ya tenemos; instrument axis es ortogonal — un work `Bucket=SoloInstrumental` con título "for guitar" → axis "Guitar music".
- **Browse by Orchestra** como axis separado de Conductor (Phase 6 tenía conductor). Datasource: `recording.orchestras[].mbid` agrupado.
- **Browse by Choir** (mismo patrón).

**Estimación**: ~12h backend + ~8h frontend = 20h. Fit Phase 10.5 (low risk, additive).

---

## Files que se tocan (summary)

Backend:
- `src-tauri/data/editorial-extended.json` — populated en 10.1 + 10.2.
- `src-tauri/data/editorial.json` v1 — ampliado en 10.3.
- `docs/classical/scripts/editorial_pipeline.py` (NEW) — Etapas 10.2 + 10.3 build pipeline.

Frontend:
- `src/components/classical/AboutThisWork.tsx` — disclaimer LLM-assisted condicional (Phase 9 ya implementa el render base).

Docs:
- `docs/classical/editorial-style-guide.md` (NEW) — plantilla + tone + sources policy.

---

## Riesgos

1. **LLM hallucinations Etapa 10.2**: detectables principalmente en `notable_recordings`. Mitigation: spot-check obligatorio + flag sobre cada entry con grabaciones listadas.
2. **Editorial labour Etapa 10.1**: 50 obras × 1.5h drafting es compromiso real. Mitigation: arrancar con 20 obras (no 50), evaluar tracción.
3. **Cap tamaño**: 200 obras × 1200 palabras × 5 bytes ≈ 1.2 MB. + ES translation = 2.4 MB. + Etapa 10.3 (1500 × ~150 palabras × 5 bytes) ≈ 1.1 MB. Total ~3.5 MB. Cap §G4 Phase 7 fue ≤ 5 MB. Margen.
4. **Drift de fuentes**: Wikipedia se actualiza. Snapshot embebido fija a un punto en tiempo. Re-build periodico (V2: anualmente) recoge drifts.

---

## Estado actual

📝 **Plan completo. Bloqueado por Phase 9.C cerrada (necesita los 3 POC validados + AboutThisWork renderer + provider extended).** Cuando arranque: delegar a `classical-musicologist` (lead) + `classical-supervisor` (curator/reviewer) + `sone-backend-engineer` (pipeline scripts).
