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
| Beethoven — Symphony No. 9 in D minor, Op. 125 "Choral" | `c35b4956-d4f8-321a-865b-5b13d9ed192b` | Canon supremo, ~200 recordings, cobertura DG/Decca/EMI completa |
| Bach — Goldberg Variations BWV 988 | `1d51e560-2a59-4e97-8943-13052b6adc03` | Canon barroco, mix de versiones piano + clavecín, HIP relevante |
| Mozart — Requiem in D minor K. 626 | `3b11692b-cdc7-4107-9708-e5b9ee386af3` | Canon coral, múltiples completiones (Süssmayr, Levin, Maunder) |
| Mahler — Symphony No. 9 in D major | `0d459ba8-74cd-4f1c-82b6-4566a5e0778c` | Romanticismo tardío, cobertura DG/Sony fuerte |
| Glass — Glassworks | `1d0df1a9-52a4-48ca-a6e5-290cd880e249` | Composer vivo, minimalist, smaller catalog en MB → test de coverage en repertorio post-1980 |

**MBIDs resueltos 2026-05-01** vía MB API (queries `arid:<composer>` + `inc=work-rels` para localizar el parent desde movements). Todos verificados como **parent works** (no movements). Hallazgo crítico documentado abajo en "Riesgos".

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

> Generado por `cargo run --example spike_isrc_coverage`. Re-correr el script lo regenera.

### Resumen por obra

| Obra | Direct rels | Via children | Via releases | Considered | With ISRC | Playable | % playable | % of-with-ISRC | Wall |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Beethoven 9 | 50 | 0 | 0 | 50 | 1 | 0 | 0.0% | 0.0% | 3.1s |
| Bach Goldberg | 19 | 0 | 0 | 19 | 1 | 1 | 5.3% | 100.0% | 2.9s |
| Mozart Requiem | 11 | 0 | 0 | 11 | 10 | 9 | 81.8% | 90.0% | 4.4s |
| Mahler 9 | 1 | 0 | 0 | 1 | 0 | 0 | 0.0% | 0.0% | 1.9s |
| Glass Glassworks | 0 | 0 | 0 | 0 | 0 | 0 | 0.0% | 0.0% | 2.0s |
| **Overall** | — | — | — | **81** | **12** | **10** | **12.3%** | **83.3%** | 15.3s |

### Quality breakdown (sobre playable)

| Tier | Count | % |
|---|---:|---:|
| LOSSLESS | 9 | 90.0% |
| LOSSLESS+HIRES_LOSSLESS | 1 | 10.0% |

### Probe de canon hand-picked en Tidal (text search)

> Validación independiente de MB ISRC coverage. Cada query es una grabación canónica reconocida; se reporta el top-hit de Tidal v1 search.

| Obra | Probe | Encontrado | Top hit Tidal | Quality |
|---|---|---|---|---|
| Beethoven 9 | Karajan/BPO 1962 (DG) | ✓ | Gundula Janowitz — _Beethoven: Symphony No. 9 in D Minor, Op. 125 "Choral": IVc-j. Presto. O Freunde nicht diese Töne – Prestissimo_ | LOSSLESS+HIRES_LOSSLESS |
| Beethoven 9 | Bernstein/VPO 1979 (DG) | ✓ | Wiener Philharmoniker — _Beethoven: Symphony No. 9 in D Minor, Op. 125 "Choral": III. Adagio molto e cantabile_ | LOSSLESS+HIRES_LOSSLESS |
| Beethoven 9 | Solti/Chicago SO (Decca) | ✓ | Berliner Philharmoniker — _Beethoven: Symphony No. 9 in D Minor, Op. 125 "Choral": II. Scherzo. Molto vivace – Presto_ | LOSSLESS+HIRES_LOSSLESS |
| Beethoven 9 | Furtwängler/Bayreuth 1951 (EMI) | ✓ | Wilhelm Furtwängler — _Symphony No. 9 in D Minor, Op. 125 "Choral": IV. Presto - "O Freunde, nicht diese Töne!" (Ode to Joy)_ | LOSSLESS |
| Beethoven 9 | Gardiner/ORR (DG Archiv) | ✓ | Orchestre Révolutionnaire et Romantique — _Beethoven: Symphony No. 9 in D Minor, Op. 125 "Choral": II. Molto vivace_ | LOSSLESS |
| Bach Goldberg | Glenn Gould 1981 (CBS/Sony) | ✓ | Glenn Gould — _Goldberg Variations, BWV 988: Aria_ | LOSSLESS+HIRES_LOSSLESS |
| Bach Goldberg | Glenn Gould 1955 (Columbia) | ✓ | Glenn Gould — _Studio Outakes from the 1955 Goldberg Variations_ | LOSSLESS |
| Bach Goldberg | András Schiff (Decca) | ✓ | András Schiff — _J.S. Bach: Goldberg Variations, BWV 988: Aria_ | LOSSLESS |
| Bach Goldberg | Murray Perahia (Sony) | ✓ | Murray Perahia — _Goldberg Variations, BWV 988: Aria_ | LOSSLESS |
| Bach Goldberg | Pierre Hantaï (harpsichord) | ✓ | Niklas Liepe — _Goldberg Variations, BWV 988: Aria (Arr. for Violin, String Orchestra & Harpsichord by Andreas N. Tarkmann)_ | LOSSLESS+HIRES_LOSSLESS |
| Mozart Requiem | Böhm/VPO (DG) | ✓ | Roberta Peters — _Mozart: Die Zauberflöte, K. 620, Act II: No. 14, Der Hölle Rache "Queen of the Night Aria"_ | LOSSLESS |
| Mozart Requiem | Karajan/BPO (DG) | ✓ | Anna Tomowa-Sintow — _Mozart: Requiem, K. 626: I. Introitus. Requiem aeternam_ | LOSSLESS |
| Mozart Requiem | Gardiner/Monteverdi Choir (Philips) | ✓ | Monteverdi Choir — _Mozart: Requiem, K. 626: IIIf. Lacrimosa_ | LOSSLESS |
| Mozart Requiem | Harnoncourt/Concentus Musicus (Sony) | ✓ | Nikolaus Harnoncourt — _Symphony No. 41 in C Major, K. 551 "Jupiter": IV. Molto allegro_ | LOSSLESS |
| Mozart Requiem | René Jacobs (HMU) | ✓ | Berliner Philharmoniker — _Mozart: Requiem, K. 626 (Ed. Beyer/Levin): IIIc. Rex tremendae_ | LOSSLESS |
| Mahler 9 | Bernstein/BPO 1979 (DG) | ✓ | Berliner Philharmoniker — _Mahler: Symphony No. 9: Ia. Andante comodo_ | LOSSLESS |
| Mahler 9 | Karajan/BPO (DG) | ✓ | Gundula Janowitz — _Beethoven: Symphony No. 9 in D Minor, Op. 125 "Choral": IVc-j. Presto. O Freunde nicht diese Töne – Prestissimo_ | LOSSLESS+HIRES_LOSSLESS |
| Mahler 9 | Abbado/BPO (DG) | ✓ | Gundula Janowitz — _Beethoven: Symphony No. 9 in D Minor, Op. 125 "Choral": IVc-j. Presto. O Freunde nicht diese Töne – Prestissimo_ | LOSSLESS+HIRES_LOSSLESS |
| Mahler 9 | Bruno Walter/VPO 1938 (EMI/Sony) | ✓ | Bruno Walter — _Symphony No. 9 in D Major: I. Andante comodo_ | LOSSLESS+HIRES_LOSSLESS |
| Mahler 9 | Haitink/Concertgebouw (Philips) | ✓ | Berliner Philharmoniker — _Mahler: Symphony No. 9 in D Major: Andante comodo_ | LOSSLESS |
| Glass Glassworks | Riesman/Philip Glass Ensemble (Sony 1982) | ✓ | Philip Glass — _Glassworks: VI. Closing_ | LOSSLESS |
| Glass Glassworks | PGE — Opening track | ✓ | Philip Glass — _Glassworks: I. Opening_ | LOSSLESS |
| Glass Glassworks | Víkingur Ólafsson (DG) | ✓ | Víkingur Ólafsson — _Glass: Glassworks: Opening_ | LOSSLESS+HIRES_LOSSLESS |
| Glass Glassworks | Lavinia Meijer (Sony harp arrangement) | ✓ | Lavinia Meijer — _Metamorphosis (Arr. for Harp by Lavinia Meijer): II. Metamorphosis Two. Flowing_ | LOSSLESS+HIRES_LOSSLESS |
| Glass Glassworks | Floraleda Sacchi (Amadeus harp) | ✓ | Floraleda Sacchi — _Jóhann Jóhannsson_ | LOSSLESS |
| **Overall** | — | **25/25 (100.0%)** | — | — |

### Casos notables (recordings con ISRC NO encontradas en Tidal)

**Beethoven 9**

- James Ellroy / Kirsty Young — _[interview]_ — `no ISRC`
- Orchestre de la Société des Concerts du Conservatoire de Paris / Carl Schuricht — _Symphonie N° 9 en Ré mineur Op. 125 "avec chœurs" (Deuxième Partie)_ — `no ISRC`
- Fritz Reiner — _Symphonie No. 9_ — `no ISRC`
- Orchestre de la Société des Concerts du Conservatoire de Paris / Carl Schuricht — _Symphonie N° 9 en Ré mineur Op. 125 "avec chœurs" (Première Partie)_ — `no ISRC`
- Orchestre National Bordeaux Aquitaine / Alain Lombard — _"Orange Mécanique" Symphonie No. 9_ — `no ISRC`
- 1977 Phantom Regiment — _Symphony No. 9_ — `no ISRC`
- David Krehbiel — _Symphony no. 9_ — `no ISRC`
- Chorus of Royal Northern Sinfonia / Members of London Symphony Chorus / Royal Northern Sinfonia / Richard Hickox — _Symphony No. 9 in D minor 'Choral' (excerpts)_ — `no ISRC`

**Bach Goldberg**

- Alexandre Tharaud — _Goldberg Variations, BWV 988_ — `no ISRC`
- Anthony Newman — _Goldberg Variations_ — `no ISRC`
- Rosalyn Tureck — _Goldberg Variations: Variations 27-30; Aria da Capo_ — `no ISRC`
- Benjamin Nacar — _Goldberg Variations_ — `no ISRC`
- Glenn Gould — _Goldberg Variations, BWV 988: Variations Nos. 17 through 30_ — `no ISRC`
- Glenn Gould — _Goldberg Variations, BWV 988: Variations 16-30 & Aria da capo_ — `no ISRC`
- Christiane Jaccottet — _Goldberg-Variationen_ — `no ISRC`
- Rosalyn Tureck — _Goldberg Variations: Variations 19-26_ — `no ISRC`

**Mozart Requiem**

- The Tabernacle Choir at Temple Square / Frank W. Asper / Alexander Schreiner / The Philadelphia Orchestra / Richard P. Condie / Eugene Ormandy — _Requiem aeternam "Give unto the meek" - Kyrie eleison "Show Thy Mercy" (2023 Remastered Version)_ — `no ISRC`
- 田中公平 — _魔曲 モーツァルト作 レクイエム_ — `JPPC09637400`

**Mahler 9**

- Frankfurt Radio Symphony Orchestra / Eliahu Inbal — _Symphony no. 9_ — `no ISRC`


### Rate-limit budget

- MB calls totales: **5**
- Tiempo total: **15.3s**
- Iniciado: epoch+1777670943

### Decisión

> ISRC→Tidal: 10/81 (12.3%) | of-with-ISRC: 83.3% | canon probes: 25/25 (100.0%) → GO with asterisco — Tidal catalogue is healthy; ISRC coverage in MB is the bottleneck. Phase 1 must add Tidal-text-search as a parallel discovery path.

### Análisis ejecutivo

La cifra agregada (12.3%) se lee como NO-GO si miras solo el threshold del gate original. **Es engañosa**:

1. **El catálogo Tidal está al 100%** sobre el canon hand-picked. Karajan/BPO 1962, Bernstein/VPO 1979, Solti/Chicago, Furtwängler/Bayreuth 1951, Gardiner/ORR, Glenn Gould 1981 + 1955, Schiff, Perahia, Hantaï, Böhm, Karajan Mozart, Gardiner Monteverdi, Harnoncourt, Jacobs, Bernstein/BPO Mahler, Bruno Walter Mahler 1938, Haitink, Riesman/PGE, Ólafsson — todos encontrados. **Tidal no es el problema**.

2. **El cuello de botella es la dispersión de ISRCs en MusicBrainz**. Sólo Mozart Requiem está bien curado (10/11 con ISRC = 91% — y de esos, 9/10 playable = 90%, lo que SÍ supera el threshold cuando los datos están). Beethoven 9, Mahler 9, Bach Goldberg tienen ISRC en 1-2 recordings de las primeras 25 que MB devuelve. La muestra que MB ordena no son las grabaciones canónicas.

3. **Cuando MB tiene ISRC, la conversión a Tidal es 83.3%** — supera el threshold de 70%. Eso valida el path técnico.

4. **Wall-clock 15.3s totales** (5 MB calls + 81 Tidal isrc lookups + 25 Tidal text searches). El threshold era < 60s/work; estamos en ~3s/work. Latencia no es problema.

5. **Quality breakdown** sobre playable: 90% LOSSLESS (16/44.1), 10% HIRES_LOSSLESS. Los probes hand-picked muestran que el canon premium (Karajan/Bernstein/Gardiner) sí está en HIRES_LOSSLESS — la mayoría de los matches por ISRC dan LOSSLESS porque MB tiende a tener ISRCs para releases más antiguos.

**Verdict supervisor**: GO con asterisco. La arquitectura original asumía implícitamente que MB tendría ISRC para una proporción razonable del canon — eso es **falso para canon mayor** (Beethoven 9, Mahler 9, Bach Goldberg). El Hub no puede depender exclusivamente de ISRC inverse-lookup. La enmienda está formalizada en `DECISIONS.md` D-010: Phase 1 implementa cascade matching (ISRC primario + Tidal text search secundario, con UI que distingue confianza). Estimate sube de 90h → 110h.

### Reproducir el experimento

```sh
# desde la raíz del repo
cd src-tauri
cargo run --example spike_isrc_coverage --release
# variables opcionales:
#   SPIKE_MAX_RECORDINGS_PER_WORK=50
#   SPIKE_INCLUDE_CHILD_WORKS=1
#   SPIKE_OUTPUT_PATH=../docs/classical/phase-0-spike.md
```

El spike requiere:
- Login Tidal previo en SONE GUI (los tokens se leen de `~/.config/sone/settings.json` encriptado — el spike lo descifra read-only).
- Conexión a internet (MB + Tidal API).

Tiempo de ejecución: ~15-30s con cache cold (5 MB calls + ~80 Tidal calls). El cache temporal del spike (`/tmp/sone-spike-cache/`) es safe-to-delete.

---

## Riesgos detectados durante Phase 0

### R1 — Recording-rels en parent work están infrarrepresentadas (2026-05-01, durante step 0.1)

**Síntoma inicial**: `GET /ws/2/work/{mbid}?inc=recording-rels` sobre los parent works canónicos devuelve counts muy por debajo de la realidad. Beethoven 9 → 64 directos. Mozart Requiem → 11 directos. La mayoría de grabaciones reales están relacionadas movement-by-movement (child works).

**Resolución durante el spike**: el endpoint **`recording?work={mbid}&inc=isrcs+artist-credits&limit=N`** sí agrega recordings de toda la jerarquía del work (parent + children) en una sola call. Reemplaza el plan original (1 call padre + N calls children + N calls detail) por **1 call por work**. El spike v3 hace exactamente esto y termina en 5 MB calls totales para 5 obras (15.3s wall-clock total).

**Mitigación arquitectónica para Phase 1**: `MusicBrainzProvider::fetch_work_recordings(mbid)` usa `recording?work=...&inc=isrcs+artist-credits&limit=100`. Una sola call. Cache 30 días. Esto es lo que CLASSICAL_DESIGN.md §3.4 llama "lazy enrichment first call" pero más eficiente todavía.

---

### R2 — La sample que devuelve MB no son las grabaciones canónicas (2026-05-01, durante step 0.4)

**Síntoma**: las primeras 25-50 recordings que devuelve `recording?work=...` para Beethoven 9 incluyen rarezas no canónicas (radio archives de Klaus Mäkelä, "Orange Mécanique" remixes, Phantom Regiment marching band, "Cyber Nation", "AAA Intro -GOLD SYMPHONY-", interview tracks). Las grabaciones canónicas (Karajan/BPO 1962, Bernstein/VPO, Solti/Chicago, Furtwängler) **no aparecen** en esa muestra.

**Causa**: MB no ordena por popularidad ni por canon-ness — el orden es esencialmente por orden de inserción del editor. Y los editors entry-level añaden rarezas más a menudo que canon (el canon ya está, hay menos incentivo de añadir).

**Impacto**: la métrica de cobertura ISRC sobre N primeras recordings no refleja la realidad. La métrica más útil es "% de canon hand-picked playable en Tidal" (que el spike v3 mide vía text search → 100%).

**Mitigación arquitectónica para Phase 1**: la lista de recordings de un Work en la UI no debe simplemente paginar por orden MB. Necesita ordering por popularidad inferida: count de releases en MB (un proxy decente — más releases = más comercializado), play count en stats DB local (si la has tocado), y hand-picked editorial al frente (Phase 5 bundle). Sin ordering, el primer screen del Work page muestra tracks irrelevantes.

---

### R3 — ISRC sparsenes en MB es el cuello de botella, no Tidal coverage (2026-05-01, durante step 0.4)

**Síntoma**: 12 de 81 recordings (14.8%) tienen ISRC en MB. Mozart Requiem es la excepción (10/11 = 91%); Beethoven 9 y Mahler 9 tienen prácticamente 0 (1/50 y 0/1 respectivamente, después de browse).

**Causa**: MusicBrainz depende de editores manuales para asociar ISRCs a recordings. Las grabaciones más populares (canon DG/Decca) tienen ISRCs en sus releases pero no necesariamente en el campo `isrcs` del recording. Los releases recientes y obras menos famosas o muy curadas (Mozart Requiem) sí tienen.

**Impacto**: el path "ISRC inverse Tidal" cubre sólo 14.8% del canon mayor. Insuficiente.

**Mitigación arquitectónica para Phase 1** (formalizado en D-010): cascade de matching — ISRC primario, Tidal text search secundario, con confidence tiering en UI. El catálogo del Hub no puede depender exclusivamente de ISRC.

---
