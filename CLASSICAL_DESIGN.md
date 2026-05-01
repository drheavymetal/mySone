# Classical Hub — diseño y plan de desarrollo

**Estado**: propuesta de diseño, no shipped.
**Fecha**: 2026-05-01.
**Autor**: investigación conjunta de tres agentes (AMC dissection, fuentes de datos, mapa de código mySone) sintetizada para revisión humana.

Documento autocontenido. Léete §0 (TL;DR) si vas con prisa, §6-§7 si vas a decidir, §8-§10 si vas a implementar.

---

## 0. TL;DR

- **Sí es viable**, pero el reto no es la UI: es **construir el grafo Composer→Work→Recording→Performer** que Apple compró cuando adquirió Primephonic en 2021.
- **Ese grafo es reproducible** desde fuentes abiertas (MusicBrainz + Wikidata + OpenOpus + Wikipedia) sin tocar APIs cerradas. Las piezas críticas ya tienen integración parcial en SONE (commit `1e42641` ya escribe `recording_mbid`, `release_group_mbid`, `artist_mbid` por play; `commands/musicbrainz.rs:213-427` ya hace enriquecimiento con `inc=artist-credits+work-rels+tags`).
- **Tidal es el sustrato de reproducción**, no de catálogo. Cruzamos por **ISRC** (cada `recording_mbid` MB tiene 0..N ISRCs; cada track Tidal tiene 1 ISRC). Cobertura ISRC en Tidal ~80-90% para canon mayor (DG/Decca/EMI/Sony), cae a ~30-50% para grabaciones legendarias pre-1960 (Furtwängler '51, Toscanini NBC, Casals Bach Suites). La UI tiene que mostrar ambos casos sin esconder lo no-reproducible.
- **El USP frente a AMC** que pide el usuario está confirmado por la investigación: AMC **no permite comparar grabaciones por calidad de audio** — no hay filtro Hi-Res, no hay columna de bit-depth/sample-rate en la lista de grabaciones, no hay filtro Atmos. Tres fuentes independientes lo confirman ([What Hi-Fi], [Apple Discussions], [Audiophilia]). Esto + el bit-perfect chain de SONE sobre Linux es una ventaja real, no marketing.
- **Integración recomendada**: nuevo sub-modo dentro de **Explore** (no nueva entrada de sidebar todavía) con su propio shell — *Classical Hub*. Reusa el patrón `explorePage` ya existente (`App.tsx:164-174`, `useNavigation.ts:94-107`) sin tocar la jerarquía actual. Un toggle en settings permite promocionarlo a sidebar para clásicos heavy.
- **Esfuerzo**: el plan completo cubre 6 fases. **Phase 0 (spike) ~1 semana** decide go/no-go; **MVP usable (phases 1-2) ~5-6 semanas**; **paridad funcional con AMC + USP de calidad (phases 3-5) +5-6 semanas**; **personalización profunda (phase 6) +1-2 semanas**. Total realista part-time: **3-5 meses**. Cada fase entrega valor independiente.
- **No perdemos nada**. La integración es aditiva: el flujo actual de Explore queda intacto, los plays clásicos siguen scrobbleando, la galaxy y stats no cambian, los favoritos Tidal no se tocan. Las únicas modificaciones invasivas son (a) extender el `mediaMetadata.tags` rendering para que la badge de Hi-Res aparezca en más sitios y (b) añadir un widget "View work" al player cuando hay `work_mbid` resuelto.

---

## 1. Objetivos y no-objetivos

### Objetivos

1. Reproducir las **funcionalidades core de Apple Music Classical**:
   - Work como entidad de primera clase navegable.
   - Lista de grabaciones por work con conductor / orquesta / año / sello.
   - Composer pages con biografía, retrato, obras agrupadas.
   - Search clásico (compositor + obra + nº catálogo + tonalidad + intérprete + instrumento).
   - Browse por época, género (sinfonía/concierto/cámara/ópera...), instrumento, conductor, orquesta.
   - Curación editorial: Editor's Choice por work, Essentials por compositor, listas comisariadas.
   - Movement-aware player: título de obra persistente, navegación entre movimientos, contexto de tempo/key cuando exista.
   - Library con facets propios (Works, Recordings, Composers, Performers).
   - Multilingüe ES/EN (lo que MB y Wikipedia cubren).
2. Añadir los **diferenciadores que AMC no tiene** (priorizados por leverage del código existente):
   - **Comparación de grabaciones por calidad audio**: columna y filtro Hi-Res / Lossless / MQA / Dolby Atmos / sample-rate / bit-depth.
   - **Bit-perfect chain awareness**: indicador en player si la grabación que suena es bit-perfect contra el DAC actual.
   - **Gapless attacca confiable**: garantía de transición sin gap entre movimientos cuando el sample-rate es continuo.
   - **Living-composer coverage**: aprovechar tags Last.fm + MB para dar protagonismo a Adams, Reich, Pärt, Saariaho, etc., donde AMC se queda corto.
   - **Personal listening integration**: "tus top works", "tu discovery curve clásica", historial cruzado con la stats DB local existente.
3. **Cero regresión** sobre el flujo actual de Explore, Library, Stats, Galaxy, Player y Scrobbling.
4. **Privacy-first**: todo el grafo de works/recordings se cachea local; ninguna llamada a fuente externa lleva información del usuario salvo, opcionalmente, el agente HTTP en MB (que ya está en el cliente HTTP compartido).

### No-objetivos (al menos en V1)

- **No sustituir a AMC offline**: dependemos de que el usuario tenga conexión y subscripción Tidal. No descargamos para offline en V1; la cola Tidal-offline existente (no-clásica) sigue funcionando aparte.
- **No editar MusicBrainz desde mySone**. Si encontramos works ausentes podemos abrir un enlace pre-rellenado a MB para que el usuario edite, pero no escribimos nosotros.
- **No reemplazar el browse de pop/rock** con el nuevo shell. El Classical Hub es ortogonal.
- **No prometer paridad de catálogo con AMC**. AMC tiene 1.2M grabaciones curadas; nosotros tendremos lo que MB tenga (~800k recordings clásicas con `work` rel) intersectado con lo que Tidal tenga playable. Los huecos se mostrarán explícitamente, no se ocultarán.

---

## 2. Por qué AMC es legendario (resumen accionable)

Síntesis de la investigación. Detalles completos en el reporte del agente AMC.

| Concepto AMC | Por qué importa | Coste de reproducir |
|---|---|---|
| Work como entidad navegable | Es la unidad cognitiva real para el oyente clásico (no el álbum) | **Bajo** — MB Work entity ya está |
| Recordings list por Work, ordenable y comparable | Permite elegir versión sin abrir Discogs | **Medio** — MB tiene `recording-rels`; el coste es paginación + cache + UI |
| Composer pages con bio, retrato, obras | Punto de entrada navegacional canónico | **Medio** — Wikipedia + Wikidata + Commons portrait |
| Curated playlists / Editor's Choice / Essentials | El listener no quiere elegir entre 80 grabaciones | **Alto** — Apple paga editores; nosotros empezamos con heurística (popularidad MB + tags Last.fm) y escalamos a curación por usuario o comunidad |
| Listening Guides (time-synced) | Pedagógico, sticky | **Alto** — datos no existen en abierto; solo factibles con esquema community-driven |
| Search con tokenización clásica | "Beethoven 9 Karajan 1962" funciona | **Medio** — parser propio sobre MB Lucene; los catálogos (BWV/K/D/RV/Hob/HWV/Op) son enumerables |
| Movement-level player | Títulos correctos, no "Track 03" | **Bajo** — MB ya da movimientos como sub-works |
| Hi-Res Lossless + Dolby Atmos badges en álbum | Premium feel | **Cero** — Tidal ya expone estos flags; SONE ya los procesa |
| **NO comparable por calidad** | (gap) | **N/A — es nuestro USP** |
| **NO Mac/Linux app** | (gap) | **N/A — es nuestro USP** |
| **NO bit-perfect chain** | (gap) | **N/A — ya implementado** en SONE |
| **NO gapless fiable** | (gap, documentado) | **Medio** — ya somos gapless general; falta verificarlo en attaccas mixtos |

La conclusión técnica: **el 80% de la magia de AMC es la base de datos curada manualmente por Primephonic**. El resto (UI, search inteligente, player movement-aware) son capas relativamente delgadas. Nosotros no podemos clonar la curación editorial de Primephonic, pero podemos construir un grafo equivalente desde MB+Wikidata+OpenOpus, **y batirlos en las dimensiones técnicas (calidad, bit-perfect, gapless, Linux)**.

---

## 3. Fuentes de datos: estrategia en capas

Síntesis del agente de fuentes. Cada fuente queda en su carril.

### 3.1 Fuente canónica por aspecto

| Aspecto | Fuente canónica | Razón |
|---|---|---|
| Lista de works por compositor | **MusicBrainz** | Cobertura más amplia, CC0, ya integrado |
| Catálogo número (BWV/K/D/RV/Op) | **Wikidata P528** | Estructurado; MB lo guarda como atributo de Series-relationship, menos cómodo |
| Tonalidad (key) | **Wikidata P826** | Q-item estructurado; MB la guarda como substring en el título |
| Año de composición | **Wikidata P571** | MB no lo tiene; Wikipedia infobox como fallback |
| Recordings por Work | **MusicBrainz `work?inc=recording-rels`** | **Único** sitio donde está el grafo work↔recording completo y libre |
| Performer credits (conductor/orquesta/solistas + voz/instrumento) | **MusicBrainz recording artist-rels** | El sub-attribute de instrument/voice es lo que da AMC su feel |
| ISRC por recording | **MusicBrainz `recording?inc=isrcs`** | Bridge a Tidal |
| Track playable + audio quality tier | **Tidal API** (siempre) | Único sitio con `mediaMetadata.tags: [LOSSLESS, HIRES_LOSSLESS, DOLBY_ATMOS, MQA]` |
| Edición / sello / año pressing | **Discogs master** | MB tiene release date pero Discogs es más fino para audiophile pressings |
| Cover art | **CoverArtArchive** | Ya integrado |
| Composer biography (long-form) | **Wikipedia REST summary** | CC BY-SA, multilingüe ES/EN, atribución requerida |
| Composer portrait | **Wikidata P18** (Commons) | CC permisivo, alta resolución |
| Era / período | **OpenOpus** `epoch` field | Curado, opinionado, usable directamente; fallback Wikidata P136 |
| Curación "essential works" | **OpenOpus** `popular` + `recommended` | Único sitio con esta señal sin escribirla nosotros |
| Tags / mood / sub-género | **Last.fm** (ya integrado) + MB tags | Comunidad para "minimalist", "barroco temprano", "sacred choral" |
| Similar recordings | **Last.fm `track.getSimilar`** + heurística MB (mismo work + diferente recording) | Ya integrado |

### 3.2 Cadenas de fallback

```
Composer biography:
  Wikipedia[user_locale] → Wikipedia[en] → MB annotation → "Sin biografía"

Catalogue number:
  Wikidata P528 → MB Work-Series with attr.number → Title regex (e.g. /Op\.?\s*(\d+)/) → null

Year of composition:
  Wikidata P571 → Wikipedia infobox parse → MB first-release-year of earliest recording (proxy) → null

Cover art for recording:
  CAA(release_mbid) → CAA(release_group_mbid) → Tidal album art → gradient placeholder (sigue pattern existente)

Playable instance for recording:
  Tidal ISRC lookup → "Not on Tidal" badge (info-only mode)

Performer roles:
  MB artist-rels with attribute → Tidal /credits subendpoint (consumer-music-grade) → null
```

### 3.3 Caching: estrategia de tres niveles

Reusa la infraestructura existente (`cache.rs:17-143` con SWR) y añade:

| Cache | Llave | TTL positivo | TTL negativo | Almacén |
|---|---|---|---|---|
| Per-work full graph | `work_mbid` | 30 días | 24 h | `DiskCache::StaticMeta` |
| Per-recording credits | `recording_mbid` | 30 días | 7 días | `DiskCache::StaticMeta` |
| ISRC → Tidal track | ISRC | 7 días | 24 h | `DiskCache::Dynamic` |
| Wikipedia summary | `lang:title` | 30 días | 7 días | localStorage (frontend) |
| Wikidata SPARQL | hash(query) | 7 días | 24 h | `DiskCache::Dynamic` |
| OpenOpus full DB | n/a (snapshot) | 30 días | n/a | bundle local (~5 MB JSON) |
| CAA images | `release_mbid:size` | 180 días | 30 días | `DiskCache::Image` |
| Last.fm tags | `artist:track` | 30 días | 7 días | localStorage (ya existe) |
| Composer search → Q-ID | nombre normalizado | 30 días | 24 h | localStorage |

**Pre-warming agresivo**: en primer lanzamiento del Hub, background-fetch de los 30 compositores más populares (OpenOpus marca esto) + sus 10 works "recommended" cada uno. Eso son ~300 cache entries en MB pre-pobladas. El usuario no nota latencia al navegar Bach/Beethoven/Mozart.

### 3.4 El cuello de botella: rate limit MB de 1 req/s

El problema concreto: **Beethoven 9ª tiene ~200 recordings en MB**. Si el usuario abre la página y queremos credits completos de todas, son 200 segundos de cold-cache. Inaceptable.

Mitigaciones (en orden de impacto):

1. **Lazy credit enrichment**: la página de obra se renderiza con solo título de orquesta + conductor + año (datos que vienen del `work?inc=recording-rels` inicial, 1 sola call). Los credits completos (solistas con voice type, label, venue) se piden bajo demanda al hacer hover/click sobre la fila. Esto convierte 200 calls upfront en 1 + N-on-demand.
2. **Bundle de canon**: snapshot pre-baked (script de build) con los top-50 compositores y sus top-20 works ya enriched. Tamaño: ~10-20 MB JSON. Se actualiza con cada release de mySone. Cubre ~80% del caso real.
3. **Background sweep**: cuando el usuario abre una obra, en paralelo (slow) traemos credits completos en cola y los cacheamos. Próxima visita está completa.
4. **Mirror MB self-hosted (opcional, advanced users)**: setting que permite apuntar al endpoint a una instancia MB local. La data es CC0, mirror es ~5 GB. No para V1, pero arquitectura debe permitirlo (ya: MB endpoint es config, no constante).

---

## 4. Diferenciadores de mySone vs AMC (USPs)

Validados contra la investigación, ordenados por leverage del código existente.

### 4.1 Comparación de grabaciones por calidad audio (USP principal)

**Confirmado**: AMC no lo tiene ([What Hi-Fi], [Apple Discussions thread 254762608], [Audiophilia]). Filtró sólo binaria de "Hi-Res Lossless" desapareció en una update.

**Cómo lo construimos**:
- Tidal expone `mediaMetadata.tags` con `LOSSLESS | HIRES_LOSSLESS | DOLBY_ATMOS | MQA` por track (ya consumido en `tidal_api.rs:54-113`).
- Sample rate / bit depth: derivable del tag (`HIRES_LOSSLESS` → hasta 24/192). Para mostrar el rate exacto necesitamos consultar el manifest del stream que ya hacemos en el `signal_path` tracker (`signal_path.rs:18-58`). Lo cacheamos por track una vez consultado.
- UI: en cada fila de la lista de recordings de un Work, columna **Q** con badge color-coded:
  - 🟢 `HIRES_LOSSLESS 24/192` (premium)
  - 🟢 `HIRES_LOSSLESS 24/96` (alto)
  - 🔵 `LOSSLESS 16/44.1` (CD)
  - 🟡 `MQA` (legacy, controvertido — explícito)
  - 🟣 `DOLBY_ATMOS` (immersive — bonus)
  - ⚫ Not on Tidal (info-only)
- Sort/filter chips: "Solo Hi-Res", "Solo Atmos", "Sample-rate ≥ 96 kHz", "Sin MQA".
- Header del Work page: badge agregado "Best available: 24/192" si la mejor recording tiene esa calidad.

### 4.2 Bit-perfect chain awareness

SONE ya tiene `feedback_bitperfect_contract.md` (memoria) + el `signal_path` tracker. AMC no tiene historia de hardware audiophile en macOS, mucho menos Linux ([Audiophilia: "Apple delivered half the job"]).

**Implementación**:
- Si `bit_perfect=true` y la recording que vamos a reproducir tiene sample-rate igual al DAC actual (consultar via `hw_volume.rs`), badge verde "Bit-perfect path".
- Si requiere resampling, badge ambar "Will resample to 48k" (con explicación al hover).
- Si `mediaMetadata.tag=DOLBY_ATMOS` y `exclusive_mode=true` con DAC stereo, warning "Atmos requires regular pipeline; bit-perfect off for this track".
- Esto se renderiza en la fila de recording **y** persistente en el player cuando esté esa recording sonando.

### 4.3 Gapless attacca confiable

AMC tiene gaps documentados entre tracks ([Apple Community thread 256011058], [253867403]) — particularmente en álbumes con tracks de formato mixto. Para clásica esto es **catastrófico** porque:
- Beethoven 5ª: el final del 3er mov enlaza directamente con el 4º (attacca). Un gap rompe la pieza.
- Mahler 3ª, Bruckner 8ª, mucha ópera, mucho Bach BWV.

**Implementación**:
- mySone ya hace gapless en general. Validamos en QA que se mantiene cuando:
  - Movimientos del mismo recording tienen mismo sample-rate (siempre esperable).
  - Movimientos del mismo recording tienen mismo audio_quality tier.
- Añadimos test que reproduce los attaccas conocidos (Beethoven 5 III→IV, Mahler 3 V→VI, Bruckner 8 III→IV) y mide gap < 50 ms.
- Si el writer detecta cambio de sample-rate entre movimientos consecutivos del mismo work, log warning y consider pre-warm del nuevo formato.

### 4.4 Living-composer coverage

AMC's *Contemporary Classical* es delgado y curado. SONE puede:
- Dar tratamiento de compositor de primera clase a Adams, Reich, Pärt, Glass, Saariaho, Andriessen, Sciarrino, Lachenmann, Furrer, Tan Dun, Toshio Hosokawa, Caroline Shaw, Anna Thorvaldsdóttir, Du Yun.
- Detectar via Last.fm tags (`minimalist`, `contemporary classical`, `post-minimalism`) los artists que merecen el tratamiento.
- Permitir al usuario "Mark as classical" desde un context menu en cualquier artist page → fuerza al artist a aparecer en el Hub aunque MB no lo categorice así.

### 4.5 Personal listening integration

Stats DB local ya tiene `recording_mbid`. Añadimos vistas:
- **Tus top works (clásica)**: agrupa plays por `work_mbid` (resuelto desde `recording_mbid`), ordena.
- **Tu discovery curve clásica**: filtra el discovery curve actual a plays con `work_mbid` no nulo.
- **Comparison personal**: si has escuchado 3 versiones de la 9ª, side-by-side con tu play count + skip rate por cada.

### 4.6 Open author bylines + community editorial

AMC oculta autores ([Variety]). Nosotros:
- Editorial blurbs vienen de Wikipedia con atribución visible "from Wikipedia, CC BY-SA + link".
- Listening guides (V2) son LRC-style, community-fillable. El fichero por work se guarda en `~/.config/sone/listening-guides/{work_mbid}.lrc`. Sync futuro vía git/IPFS/Obsidian-LiveSync.
- El usuario puede sobrescribir notas locales para sus works favoritos.

### 4.7 Personal-library override

AMC ignora ripped CDs. Nosotros:
- Si futuras versiones soportan FLAC local, el matching local-FLAC ↔ MB-recording será automático via tags + ISRC. Out of scope V1, pero la arquitectura del data model debe permitirlo (los entities `Work` / `Recording` tienen `playable_via: { tidal: track_id?, local_path?, archive_url? }`).

---

## 5. Modelo de datos interno

Diseño que abstrae todas las fuentes en un único modelo de dominio. **Ningún componente UI debe ver schemas crudos de MB / Wikidata / Tidal.** Los providers cumplen `fillFields(domainObject)` y mergean.

### 5.1 Entidades

```typescript
// src/types/classical.ts (nuevo)

interface Composer {
  // Identidad cross-source
  mbid?: string;           // MusicBrainz artist
  qid?: string;            // Wikidata
  openOpusId?: string;
  // Display
  name: string;
  fullName?: string;       // "Ludwig van Beethoven"
  birth?: { year: number; date?: string; place?: string };
  death?: { year?: number; date?: string; place?: string };
  era: Era;                // enum, fuente OpenOpus.epoch
  portraitUrl?: string;    // Commons / Wikipedia
  // Editorial
  bioShort?: string;       // Wikipedia.description (~80 chars)
  bioLong?: string;        // Wikipedia.extract (HTML)
  bioSourceUrl?: string;   // attribution link
  // Navegación
  worksByGenre: Record<Genre, WorkSummary[]>;
  popularWorks: WorkSummary[]; // OpenOpus.popular = 1
  relatedComposerMbids: string[];
}

interface Work {
  mbid: string;            // MB Work, único required
  qid?: string;
  title: string;
  alternativeTitles: string[];   // MB aliases (locale-tagged)
  composerMbid: string;
  catalogueNumber?: {            // BWV 1052, K. 466, etc.
    system: 'BWV' | 'K' | 'D' | 'RV' | 'Hob' | 'HWV' | 'Op' | 'Other';
    number: string;              // "1052", "466", etc.
    display: string;             // "BWV 1052"
  };
  key?: string;                  // "D minor"
  genre?: Genre;
  type: WorkType;                // Symphony / Concerto / Sonata / Opera / ...
  compositionYear?: number;
  premiereYear?: number;
  durationApproxSecs?: number;
  movements?: Movement[];        // Con MB es child-works con part-of
  // Editorial
  description?: string;          // Wikipedia.extract (HTML)
  descriptionSourceUrl?: string;
  // Navegación
  recordingCount: number;        // count(MB recording-rels)
  recordingPreview: RecordingSummary[]; // top-5 by popularity
  editorsChoiceRecordingMbid?: string;  // future, manual
}

interface Movement {
  mbid: string;          // sub-work MBID
  index: number;         // 1, 2, 3...
  title: string;         // "II. Molto vivace"
  durationApproxSecs?: number;
  attaccaTo?: number;    // index del siguiente con attacca
}

interface Recording {
  mbid: string;
  workMbid: string;       // resuelto via MB performance rel
  title?: string;         // a veces difiere del work title
  // Performers
  conductor?: PerformerCredit;
  orchestras: PerformerCredit[];     // multiple posible (orq + choir)
  soloists: PerformerCreditWithRole[]; // con voice/instrument
  ensemble?: PerformerCredit;
  choir?: PerformerCredit;
  // Logistics
  recordingYear?: number;
  recordingDate?: string;     // YYYY-MM-DD si MB lo tiene
  venue?: string;
  label?: string;             // Discogs
  // Bridge to playback
  isrcs: string[];            // MB
  tidalTrackId?: number;       // ISRC lookup result
  audioQuality?: TidalQualityTier; // del tidal track
  audioModes?: string[];       // [STEREO] | [DOLBY_ATMOS]
  // Audio quality refined (post-stream-fetch)
  sampleRateHz?: number;
  bitDepth?: number;
  // Editorial
  isEditorsChoice?: boolean;
  popularityScore?: number;   // MB derived: count of distinct releases
}

interface PerformerCredit {
  mbid?: string;
  name: string;
  type: 'person' | 'group' | 'orchestra' | 'choir' | 'ensemble';
}

interface PerformerCreditWithRole extends PerformerCredit {
  role: string;          // "violin" | "soprano" | "piano" | ...
  instrumentMbid?: string;
}

type Era = 'Medieval' | 'Renaissance' | 'Baroque' | 'Classical' | 'EarlyRomantic'
         | 'Romantic' | 'LateRomantic' | 'TwentiethCentury' | 'PostWar' | 'Contemporary';

type WorkType = 'Symphony' | 'Concerto' | 'Sonata' | 'StringQuartet' | 'Opera'
              | 'Cantata' | 'Mass' | 'Lieder' | 'Suite' | 'Etude' | 'Other';

type Genre = 'Orchestral' | 'Concerto' | 'Chamber' | 'Solo Instrumental'
           | 'Vocal' | 'Choral' | 'Opera' | 'Sacred' | 'Stage' | 'Film' | 'Other';

type TidalQualityTier = 'LOW' | 'HIGH' | 'LOSSLESS' | 'HI_RES' | 'HI_RES_LOSSLESS' | 'DOLBY_ATMOS';
```

### 5.2 Provider pattern (Rust backend)

```rust
// src-tauri/src/classical/providers/mod.rs (nuevo)

#[async_trait]
pub trait ClassicalProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// Mejor-effort: rellena lo que esta fuente sepa, deja el resto.
    async fn enrich_composer(&self, c: &mut Composer) -> Result<(), SoneError>;
    async fn enrich_work(&self, w: &mut Work) -> Result<(), SoneError>;
    async fn enrich_recording(&self, r: &mut Recording) -> Result<(), SoneError>;
}

// Implementaciones:
// - MusicBrainzProvider (extiende el existente MusicBrainzLookup)
// - WikidataProvider (SPARQL client, cached)
// - WikipediaProvider (REST summary, multilingual)
// - OpenOpusProvider (snapshot local)
// - TidalProvider (ISRC → track, quality tier)
// - DiscogsProvider (futuro: edition history)
// - LastfmProvider (tags + similar; ya existe via commands/lastfm.rs)
```

El **Catalog Service** orquesta: dado un `work_mbid`, llama a cada provider en cadena con prioridad y mergea. Si la cache tiene el `Work` completo, salta proveedores. Si solo tiene parcial, completa con providers que faltan.

```rust
// src-tauri/src/classical/catalog.rs

pub struct CatalogService {
    cache: Arc<DiskCache>,
    providers: Vec<Box<dyn ClassicalProvider>>,
}

impl CatalogService {
    pub async fn get_work(&self, mbid: &str) -> Result<Work, SoneError> {
        if let Some(cached) = self.cache.get_work(mbid)? {
            return Ok(cached);
        }
        let mut work = Work::skeleton(mbid);
        for p in &self.providers {
            let _ = p.enrich_work(&mut work).await; // best-effort
        }
        self.cache.set_work(&work)?;
        Ok(work)
    }
    // similar: get_composer, get_recording, list_works_by_composer, ...
}
```

---

## 6. Tres alternativas de integración en SONE

### Alternativa A — Sub-modo dentro de Explore (RECOMENDADA)

**UX**: Explore tab existente sigue tal cual. En su parte superior, debajo del header, una pill prominente: **"🎼 Classical Hub"**. Click → entra al Classical Hub como `explorePage` con `apiPath: classical://hub` (interno, no Tidal). Botón de back vuelve al Explore estándar.

```
┌──────────────────────────────────────────────────────┐
│ Explore                                              │
│ ┌───────────────────────┐                            │
│ │ 🎼 Classical Hub  ›   │  ← entrada al sub-modo    │
│ └───────────────────────┘                            │
│                                                      │
│  [Genres pills]  [Moods pills]  [Decades]            │
│  [New] [Top] [Videos] [HiRes]                        │
│                                                      │
│  TIDAL Editorial Sections...                         │
└──────────────────────────────────────────────────────┘
```

**Pros**:
- Nav existente intacto. Cero riesgo de regresión en Sidebar / routing / Tidal explore.
- El usuario casual descubre por curiosidad.
- Cero código nuevo en Sidebar.tsx.
- Reusa `App.tsx:164-174` (`type: "explorePage"`) extendiendo el switch para detectar `apiPath` con prefijo `classical://`.

**Cons**:
- Un click extra para clásicos heavy.
- "Classical" no aparece en sidebar; visible solo desde Explore.

**Mitigación**: setting "Show Classical in sidebar" (default off) que añade entrada de sidebar promoviendo el Hub a top-level. Implementación trivial.

### Alternativa B — Top-level sidebar

**UX**: Nuevo botón sidebar entre Explore y Stats: 🎼 Classical.

```
🏠 Home
🧭 Explore
🎼 Classical    ← nuevo
📊 Stats
─────
Library...
```

**Pros**:
- Discoverability máxima.
- Coherente con apps como AMC que separan totalmente.

**Cons**:
- Sidebar bloat para usuarios que no escuchan clásica.
- Implica decisión filosófica ("¿es Classical un *modo*, como Explore, o un *contenido*?"). Hoy Explore es *contenido* y Sidebar agrupa modos. Romper esa convención requiere repensar la jerarquía.
- Más cambios en `Sidebar.tsx`, `App.tsx`, `useNavigation.ts`, `types.ts`. Más riesgo.

### Alternativa C — Toggle en Explore (modo conmutado)

**UX**: En el header del Explore page actual, toggle binario "Standard | Classical". Misma página, distinto contenido.

```
┌─ Explore ─────────────────────  [Standard|Classical] ┐
│  (contenido cambia según toggle)                      │
└────────────────────────────────────────────────────────┘
```

**Pros**:
- Un solo "tab", máximo reuso del shell.

**Cons**:
- El usuario olvida que el toggle existe.
- "Browse genres" significa cosa distinta en cada modo, confunde.
- El Classical Hub tiene jerarquía propia (Composers / Works / Recordings / Performers / Periods / Genres) que no encaja en el shell de Explore actual.

### Decisión: **Alternativa A**, con setting opcional para promocionar a sidebar.

Justificación:
1. Cero regresión. Explore y Sidebar siguen igual.
2. Es el patrón ya soportado (`explorePage` route).
3. Permite iterar UI del Hub sin tocar código compartido.
4. El setting de sidebar resuelve el descubrimiento si el usuario lo demanda.
5. La pill "Classical Hub" en Explore es muy visible — efectivamente promociona.

---

## 7. Diseño de la UI del Classical Hub

Mirroring AMC con extensiones SONE. Toda la UI sigue el theme `th-*` existente (`tailwind.config.js`) y los patrones de componentes (`HomeSection`, `MediaCard`, `PageContainer`).

### 7.1 Information architecture

```
Classical Hub (root)
├── Listen Now (default landing)
│   ├── Continue listening (works escuchados parcialmente)
│   ├── For You — sugerencias basadas en tus stats
│   ├── New releases (Tidal "new" + filtrado classical)
│   ├── Editor's Choice (future, manual)
│   └── Curated playlists (heuristic v1)
├── Browse
│   ├── Composers (lista, search, filtros era)
│   ├── Periods (Medieval ... Contemporary)
│   ├── Genres (Symphony, Concerto, Chamber, ...)
│   ├── Conductors
│   ├── Orchestras / Ensembles
│   ├── Soloists
│   ├── Instruments
│   └── Choirs
├── Library
│   ├── Saved Works
│   ├── Saved Recordings
│   ├── Saved Composers
│   └── Saved Performers
└── Search (modo clásico)
```

### 7.2 Pantallas clave

#### Composer page

```
┌─────────────────────────────────────────────────────────────┐
│ ← Back                                                       │
│                                                              │
│  ┌───────┐  Ludwig van Beethoven                             │
│  │Portrait│ 1770 – 1827 · Classical → Romantic              │
│  │ HD    │ German                                            │
│  └───────┘                                                   │
│                                                              │
│  «Beethoven was a German composer and pianist whose…»        │
│  source: Wikipedia (CC BY-SA)                ╲                │
│                                                              │
│  ── Essentials ────────────────────────────────             │
│  [Symphony 9] [Symphony 5] [Piano Sonata 14] [Missa Solemn] │
│                                                              │
│  ── Symphonies ────────────────────────────────  [view all] │
│  [card] [card] [card] [card] [card] [card] [card]           │
│                                                              │
│  ── Piano Concertos ──────────────────────────  [view all]  │
│  ...                                                         │
│                                                              │
│  ── Related Composers ────────────────────────              │
│  [Mozart] [Schubert] [Brahms] [Haydn]                       │
└─────────────────────────────────────────────────────────────┘
```

#### Work page (la pieza central)

```
┌─────────────────────────────────────────────────────────────┐
│ ← Beethoven                                                  │
│                                                              │
│  Symphony No. 9 in D minor "Choral"                          │
│  Op. 125 · 1822-1824 · Choral Symphony                       │
│  4 movements · ~70 min                                       │
│                                                              │
│  «The Ninth Symphony was composed between 1822 and…»          │
│  source: Wikipedia                                           │
│                                                              │
│  ── Movements ────────────────────────                       │
│  I.   Allegro ma non troppo, un poco maestoso ~17m           │
│  II.  Molto vivace                            ~12m           │
│  III. Adagio molto e cantabile                ~16m           │
│  IV.  Presto / Allegro assai (Choral)         ~25m           │
│                                                              │
│  ── 184 Recordings ────────────  [Filter: Hi-Res ✓ Atmos ☐] │
│                                  [Sort: Popularity ▾]        │
│                                                              │
│  ★ Editor's Choice                                           │
│  [card] Karajan · Berliner Philharmoniker · 1962 · DG        │
│         🟢 LOSSLESS 16/44.1 · STEREO · ▶                    │
│                                                              │
│  Popular                                                     │
│  [card] Bernstein · Wiener Philharm. · 1979 · DG             │
│         🟢 HIRES_LOSSLESS 24/96 · STEREO · ▶               │
│  [card] Furtwängler · Bayreuth · 1951 · EMI                  │
│         ⚫ Not on Tidal (info only)                          │
│  [card] Solti · Chicago SO · 1972 · Decca                    │
│         🟢 HIRES_LOSSLESS 24/192 · ATMOS · ▶               │
│  [card] Gardiner · ORR · 1992 · DG                           │
│         🔵 LOSSLESS 16/44.1 · STEREO · ▶                    │
│  ...                                                         │
└─────────────────────────────────────────────────────────────┘
```

Cada fila de recording: cover thumbnail + conductor + orquesta + año + sello + **badge calidad audio** + botón play. Hover muestra solistas + venue + producer.

Filter chips arriba de la lista: `Hi-Res only`, `Atmos`, `Sample-rate ≥ 96k`, `Sin MQA`, `Año desde…`. Sort dropdown: Popularity / Year (newest first) / Year (oldest first) / Audio quality (best first) / Conductor A-Z.

#### Search clásico

```
┌─────────────────────────────────────────────────────────────┐
│ 🔍 Beethoven 9 Karajan 1962                       [×]       │
│                                                              │
│  Detected: composer:Beethoven · work:Symphony 9 ·            │
│            conductor:Karajan · year:1962                     │
│  [✓ Composer]  [✓ Work]  [✓ Conductor]  [✓ Year]            │
│                                                              │
│  ── Best match ──                                            │
│  Beethoven Symphony 9 · Karajan · BPO · 1962 · DG            │
│  🟢 LOSSLESS 16/44.1 · ▶                                   │
│                                                              │
│  ── Other recordings of this work ──                         │
│  [3 more rows]                                               │
│                                                              │
│  ── Other Karajan/Beethoven recordings ──                    │
│  [...]                                                       │
└─────────────────────────────────────────────────────────────┘
```

Parser básico: tokeniza, identifica entidades por:
- Composer apellido conocido (lista de top-200 OpenOpus).
- Catalogue numbers regex (`BWV \d+`, `K\.? *\d+`, `Op\.? *\d+`, etc.).
- Tonalidad regex (`(C|D|E|F|G|A|B)[♭♯]? (minor|major|m|maj)`).
- Años (`\d{4}`).
- Lo que sobre → query texto libre a MB/Tidal.

#### Player extensions

Cuando el track actual tiene `work_mbid` resuelto:
- Sobre el título del track, **work title persistente** ("Beethoven · Symphony 9 · Karajan/BPO 1962").
- Indicador "II / IV" (movimiento actual / total).
- Botón "View work" → abre Work page con scroll a la fila de la recording activa.
- Si bit-perfect activo, badge "Bit-perfect 24/96" en la player bar (verde) o "Resampling to 48k" (ambar).
- Si attacca al siguiente movimiento, indicador "Attacca →" pequeño antes del fade.

### 7.3 Library facets

`saved_works`, `saved_recordings`, `saved_composers`, `saved_performers` van a la stats DB en una tabla nueva `classical_favorites`:

```sql
CREATE TABLE classical_favorites (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,           -- 'work' | 'recording' | 'composer' | 'performer'
  mbid TEXT NOT NULL,
  display_name TEXT NOT NULL,   -- denormalized para evitar lookup
  added_at INTEGER NOT NULL,
  UNIQUE(kind, mbid)
);
```

Library tab en el Hub muestra grid filtrable por kind.

---

## 8. Plan de desarrollo por fases

Cada fase es **independiente** y entrega valor por sí misma. Tras cada fase decidimos seguir, parar, o ajustar.

### Phase 0 — Spike de viabilidad (1 semana, ~30h)

**Objetivo**: validar las dos hipótesis críticas antes de invertir más:
1. **Cobertura ISRC en Tidal** para grabaciones canónicas. Test: 5 obras × 20 recordings cada = 100 recordings. ¿Qué % son playable en Tidal?
2. **Latencia de carga real** de un Work page con MB rate-limit. Test: Beethoven 9, ver wall-clock.

**Tareas**:
- Script Rust standalone que: dado un work_mbid, hace `recording-rels` lookup, intenta ISRC inverse Tidal por cada recording, reporta % playable + audio quality breakdown.
- Run sobre: Beethoven 9, Bach Goldberg, Mozart Requiem, Mahler 9, Glass Glassworks.
- Documentar resultados en `CLASSICAL_DESIGN.md` apendice.
- **Decision gate**: si cobertura < 50% de canon mayor, replantear (¿integrar Qobuz? ¿Apple Music API? ¿IMSLP fallback aggressive?).

**Entregable**: nada en producción, solo report. **Esfuerzo**: 1 dev, 1 semana, 30h.

### Phase 1 — Foundation: Catalog service + 1 Work page (3 semanas, ~90h)

**Objetivo**: shippeables: Work page funcional para *una* ruta de entrada (botón "View work" en el player cuando hay `work_mbid`), con datos reales de MB, cache, y reproducción Tidal.

**Tareas backend (Rust)**:
- Crear módulo `src-tauri/src/classical/` con `mod.rs`, `catalog.rs`, `providers/{musicbrainz,wikipedia,wikidata,tidal,openopus}.rs`.
- Implementar trait `ClassicalProvider` con method `enrich_work`, `enrich_composer`, `enrich_recording`.
- `MusicBrainzProvider`: extiende el existente `MusicBrainzLookup` (`scrobble/musicbrainz.rs:22-153`) con:
  - `fetch_work(mbid)` con `inc=artist-rels+recording-rels+work-rels+aliases+series-rels`.
  - `fetch_recording(mbid)` con `inc=artist-rels+work-rels+isrcs+releases`.
- `WikipediaProvider`: REST summary para composer + work descriptions, multilingual.
- `WikidataProvider`: SPARQL queries para catalogue numbers + composition year + portrait.
- `OpenOpusProvider`: bundle local (snapshot JSON) en `src-tauri/data/openopus.json`.
- `TidalProvider`: ISRC → track lookup, batched. Reusar `tidal_api.rs`.
- `CatalogService` que orquesta + cachea via `DiskCache::StaticMeta`.
- Tauri commands: `get_classical_work(mbid)`, `get_classical_composer(mbid)`, `get_classical_recording(mbid)`.
- DB migration: `classical_favorites` table.

**Tareas frontend (React)**:
- `src/components/classical/` nuevo directorio.
- `WorkPage.tsx`: header de obra, descripción, lista de movimientos, lista de recordings (sin filtros aún, solo sort por popularity).
- `RecordingRow.tsx`: cover + conductor/orq/año + badge calidad + play button.
- `src/api/classical.ts`: wrappers tipados de los Tauri commands.
- `src/types/classical.ts`: types definidos en §5.1.
- Hook `useClassicalWork(mbid)` con stale-while-revalidate.
- Botón "View work" en el player (Player.tsx) cuando `currentTrack.workMbid` existe.

**Entregable**: usuario reproduce un track con MBID, click "View work", ve la página de obra completa con lista de grabaciones playable. **Esfuerzo**: 1 dev, 3 semanas, 90h.

**Decision gate**: ¿la página de obra carga en < 3s con cache cold? ¿La lista muestra ≥ 20 recordings con datos correctos? ¿Funciona reproducir cualquier recording playable?

### Phase 2 — Browse experience: Composer pages, Hub landing, Browse axes (2-3 semanas, ~70h)

**Objetivo**: el Hub en sí. Listen Now landing, Composer pages, Browse por compositor / período / género.

**Tareas backend**:
- `list_classical_composers(filters)` → top-N por OpenOpus popularidad o filtrado por época.
- `list_works_by_composer(mbid, genre)` → MB browse + group by Work type.
- `search_classical(query)` → versión naïve de search clásico (parser básico, full version en Phase 4).

**Tareas frontend**:
- `ClassicalHubPage.tsx`: Listen Now, sub-nav (Browse/Search/Library).
- `ComposerPage.tsx`: hero, bio, works groupados.
- `BrowseComposers.tsx`, `BrowsePeriods.tsx`, `BrowseGenres.tsx`.
- Integración con Explore: añadir pill "Classical Hub" en `ExplorePage.tsx`, registrar nueva ruta `classical://hub` en `App.tsx`.
- Setting "Promote to sidebar" (off por defecto).

**Entregable**: usuario entra a Classical Hub desde Explore, navega Beethoven → Symphony 9 → recordings → reproduce. **Esfuerzo**: 1 dev, 2-3 semanas, 70h.

### Phase 3 — Player upgrades + gapless (1-2 semanas, ~40h)

**Objetivo**: el player se vuelve work-aware.

**Tareas**:
- Player: work title persistente cuando hay `work_mbid` resuelto.
- Indicador "I / IV" de movimiento.
- Test suite de gapless attacca: Beethoven 5 III→IV, Mahler 3 V→VI, Bruckner 8 III→IV. Audio capture + análisis de silencio < 50ms.
- Resolver `work_mbid` desde `recording_mbid` en `on_track_started` (extiende `scrobble/mod.rs:262-355`). Persistir en stats DB para que aparezca en "tus top works".
- "Attacca →" indicator pequeño en player.

**Entregable**: el player muestra contexto de obra/movimiento; gapless validado. **Esfuerzo**: 1 dev, 1-2 semanas, 40h.

### Phase 4 — Quality USP (1-2 semanas, ~40h)

**Objetivo**: la columna y filtro de calidad audio. **El USP central que pidió el usuario.**

**Tareas backend**:
- Sample-rate / bit-depth refinement: para tracks Tidal con tag `HIRES_LOSSLESS`, hacer manifest fetch (lo que hace `signal_path.rs` en stream-time) **antes** de stream para conocer la rate exacta. Cachear por track_id.
- Aggregator que computa "best quality available" por work.

**Tareas frontend**:
- `RecordingRow.tsx` añade columna Quality con badge color-coded.
- Filter chips arriba de la lista de recordings.
- Sort por quality.
- Header del work page con "Best available: 24/192" cuando aplique.
- Player bit-perfect indicator (extends current player UI).
- Search clásico: `quality:hires` chip.

**Entregable**: usuario abre Beethoven 9, filtra "Hi-Res only", ordena por sample rate, encuentra Solti/Chicago en 24/192 + Atmos. **Esfuerzo**: 1 dev, 1-2 semanas, 40h.

### Phase 5 — Editorial layer + search avanzado (2 semanas, ~60h)

**Objetivo**: pulir lo "blando" de AMC.

**Tareas**:
- Search parser robusto: catalogue numbers (BWV/K/D/RV/Hob/HWV/Op), key, instrument, voice type, conductor, soloist. Autocomplete con chips.
- Editor's Choice: heurística v1 = recording con mayor `popularityScore` MB (count distinct releases) + flag manual override por usuario.
- Listening guides scaffolding: leer `~/.config/sone/listening-guides/{work_mbid}.lrc` si existe, mostrar time-synced. UI para editar local.
- Wikipedia integration completa (composer + work) con atribución.
- "Related composers" via Wikidata SPARQL (P136 genre overlap + same era).
- Browse por conductor / orquesta / soloist con sus discografías.

**Entregable**: paridad funcional con AMC core. **Esfuerzo**: 1 dev, 2 semanas, 60h.

### Phase 6 — Personal listening integration (1-2 semanas, ~30h)

**Objetivo**: cosas que AMC no puede hacer porque no tiene tu historial local.

**Tareas**:
- "Tus top works" — agregación stats por `work_mbid`.
- "Tu discovery curve clásica" — filtra el discovery curve actual a plays con `work_mbid`.
- "Recording comparison personal" — si has escuchado N versiones de la misma obra, side-by-side con tu play count + completion rate.
- Library facets en el Hub.
- Pre-warm de canon en background (top-30 compositores, top-20 works each).

**Entregable**: el Hub se siente "tuyo". **Esfuerzo**: 1 dev, 1-2 semanas, 30h.

### Resumen de esfuerzo

| Phase | Duración | Horas | Cumulativo |
|---|---|---|---|
| 0. Spike | 1 sem | 30h | 30h |
| 1. Foundation | 3 sem | 90h | 120h |
| 2. Browse | 2-3 sem | 70h | 190h |
| 3. Player | 1-2 sem | 40h | 230h |
| 4. **Quality USP** | 1-2 sem | 40h | 270h |
| 5. Editorial + search | 2 sem | 60h | 330h |
| 6. Personalization | 1-2 sem | 30h | 360h |

**Full-time**: ~3 meses. **Part-time (~15h/semana)**: ~6 meses.

**Punto natural de pausa**: tras Phase 4. Tienes Hub funcional, paridad parcial AMC, y el USP de calidad en la mano. Phases 5-6 son refinamiento.

---

## 9. Riesgos y mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|---|---|---|---|
| Cobertura ISRC Tidal insuficiente para canon mayor | Media | Alto | Phase 0 gate. Si < 50%, replantear backend (Qobuz, IMSLP fallback) |
| MB rate limit hace Work page lento | Alta | Medio | Lazy enrichment + bundle pre-baked + background sweep |
| Tidal API no oficial cambia | Baja | Crítico | Architecture: separar catalogue layer de playback layer. Si Tidal rompe, podemos cambiar de provider playable sin tocar el catálogo |
| MB Work coverage para living composers patchy | Alta | Medio | Detect via Last.fm + manual "Mark as classical" override |
| Wikipedia CC BY-SA contamination en futuro export | Baja | Bajo | Atribución visible siempre; nunca inline-copy sin source link |
| Discogs ToS si mySone monetiza | Baja | Medio | Discogs solo en fase 5+, opcional. Fácil de quitar si cambia el modelo |
| Performance frontend con 200 recordings en lista | Media | Medio | Virtualization (react-window) en lista de recordings |
| Bundle size de OpenOpus snapshot | Baja | Bajo | ~5 MB es aceptable; si crece, lazy-load |
| Bit-perfect detection no funciona con todos DACs | Media | Bajo | Graceful degradation: no mostrar badge si `hw_volume.rs` no resuelve formato |
| Gapless attacca falla en algunas combinaciones | Media | Alto (UX) | Phase 3 incluye test suite; si falla, documentar limitación y mostrar warning visible |

---

## 10. Cómo no perder funcionalidad existente

Auditoría: cada cambio invasivo identificado, con su mitigación.

| Área SONE existente | Cambio propuesto | Cómo no romper |
|---|---|---|
| `ExplorePage.tsx` | Añadir pill "Classical Hub" | Pill se inserta como sección nueva al inicio; secciones Tidal existentes intactas |
| `Sidebar.tsx` | Setting "Promote to sidebar" | Default off. Cero cambio visible para usuarios que no lo activen |
| `App.tsx:164-174` routing | Detectar `apiPath: "classical://*"` | If/else aditivo; rutas existentes ininterrumpidas |
| `useNavigation.ts:94-107` | `navigateToClassical()` nuevo | Función adicional; no modifica existentes |
| `types.ts:152-200` `AppView` | No cambia | Reusamos `explorePage` con apiPath especial |
| `scrobble/mod.rs::on_track_started` | Resolver `work_mbid` adicional | El resolve es async + best-effort, no bloquea track start |
| `stats.rs` schema | Nueva columna opcional `work_mbid` + tabla `classical_favorites` | Migration aditiva como las anteriores |
| Player UI | Work title persistente cuando hay work_mbid | Renderizado condicional. Sin work_mbid, UI idéntica a hoy |
| `MusicBrainzLookup` | Extender con `fetch_work`, `fetch_recording` | Métodos nuevos, no tocan `lookup_isrc` ni `lookup_by_name` |
| `cache.rs` | Reusa `StaticMeta` tier | Cero cambio en API |
| Galaxy / Stats / Live painting / Share link | No tocados | — |
| TIDAL favorites | No tocados | Saved Works son una capa nueva, no sustituyen TIDAL favorites |

---

## 11. Criterios de éxito y decisión

Tras cada phase, evaluamos:

- **Phase 0**: ISRC coverage ≥ 70% de canon mayor → **GO**. Si 50-70%, GO con asterisco (UX honesta sobre recordings missing). Si < 50%, **ALTO**.
- **Phase 1**: Beethoven 9 page carga en < 3s cold cache, < 200ms warm → **GO**.
- **Phase 2**: Cualquier compositor top-30 OpenOpus → su página → cualquier work → cualquier recording → reproduce. Sin errores. → **GO**.
- **Phase 3**: Test suite gapless attacca pasa en 3/3 cases con gap < 50ms → **GO**.
- **Phase 4**: Filtro "Hi-Res only" en Beethoven 9 muestra solo HIRES_LOSSLESS, sort by quality muestra 24/192 primero → **GO**.
- **Phase 5**: Search "Beethoven 9 Karajan" devuelve la grabación correcta como best match → **GO**.
- **Phase 6**: Tu top work clásico computado correctamente desde stats DB → **GO**.

---

## 12. Apéndice A — Mapeo a archivos existentes

Decisión técnica de cada componente, anclada al código actual.

| Concepto nuevo | Archivo nuevo | Archivos existentes referenciados |
|---|---|---|
| Catalog service | `src-tauri/src/classical/catalog.rs` | extiende `scrobble/musicbrainz.rs:22-153`, usa `cache.rs:17-143` |
| Provider trait | `src-tauri/src/classical/providers/mod.rs` | mismo crate, conventions de `scrobble/mod.rs` |
| MB provider | `src-tauri/src/classical/providers/musicbrainz.rs` | reusa `MusicBrainzLookup` y `commands/musicbrainz.rs:42-427` |
| Wikipedia provider | `src-tauri/src/classical/providers/wikipedia.rs` | nuevo |
| Wikidata SPARQL provider | `src-tauri/src/classical/providers/wikidata.rs` | nuevo |
| OpenOpus provider | `src-tauri/src/classical/providers/openopus.rs` | bundle `src-tauri/data/openopus.json` |
| Tidal ISRC bridge | `src-tauri/src/classical/providers/tidal.rs` | reusa `tidal_api.rs:54-113` |
| Tauri commands | `src-tauri/src/commands/classical.rs` | registra en `lib.rs::run` con resto de handlers |
| Stats schema | extensión en `stats.rs:194-230` | migration aditiva igual que `source` column |
| Hub root page | `src/components/classical/ClassicalHubPage.tsx` | reusa `PageContainer`, `HomeSection.tsx:34-96` |
| Composer page | `src/components/classical/ComposerPage.tsx` | patrón de `ArtistPage.tsx` |
| Work page | `src/components/classical/WorkPage.tsx` | patrón de `AlbumView.tsx` |
| Recording row | `src/components/classical/RecordingRow.tsx` | patrón de `RankedRow` en `StatsPage.tsx:1212-1272` |
| Browse pages | `src/components/classical/Browse{Composers,Periods,Genres,...}.tsx` | reusa `HomeSection` |
| Search clásico | `src/components/classical/ClassicalSearch.tsx` | extiende `SearchView.tsx` |
| Domain types | `src/types/classical.ts` | nuevo |
| API wrappers | `src/api/classical.ts` | patrón de `src/api/stats.ts`, `src/api/lastfm.ts` |
| Pill en Explore | edit `ExplorePage.tsx:45-152` | inserción aditiva |
| Routing | edit `App.tsx:164-174` | detect `classical://` prefix |
| Nav helper | edit `useNavigation.ts:94-107` | nueva función `navigateToClassical()` |

---

## 13. Apéndice B — Bundle pre-baked de canon

Para acelerar primer encuentro con canon, en build time ejecutamos:

```bash
# build-scripts/snapshot-classical-canon.mjs
# Para los top-50 compositores OpenOpus, fetch de cada work top-20:
#   - Work entity completa (MB)
#   - Recordings list con credits básicos (paginated)
#   - Wikipedia summary
# Output: src-tauri/data/classical-canon-snapshot.json (~10-20 MB)
```

En primer launch del Hub, se importa este snapshot al `DiskCache` con TTL 30d. El usuario navega Bach/Beethoven/Mozart instantáneo, sin esperar MB.

Este script se corre manualmente en cada release (no en cada CI build, MB rate limit). Se documenta en `CONTRIBUTING.md` (futuro).

---

## 14. Apéndice C — Privacidad

- Ninguna fuente externa recibe `user_id` ni datos personales del usuario.
- Llamadas a MB / Wikipedia / Wikidata son anónimas con User-Agent `mySone/{version} (https://github.com/lullabyX/sone)`.
- Tidal recibe la auth ya existente (sin cambios).
- Cache local sigue cifrado por el `Crypto` actual (`crypto.rs`).
- Stats DB sigue plain SQLite (no encryption, fs perms — ya documentado).
- No hay telemetría agregada a las nuevas vistas.

---

## 15. Próximos pasos (concretos)

Si apruebas el diseño:

1. **Lanzo Phase 0** (spike de viabilidad): branch `feat/classical-spike`, script Rust standalone, run sobre 5 obras canon, report de cobertura ISRC + latencia.
2. Si Phase 0 GO, **Phase 1** en branch `feat/classical-foundation`, single-shot deliverable: una Work page real al final de la rama.
3. Documentación de cada feature en `FEATURES.md` siguiendo el patrón actual.
4. Stats memory en `~/Obsidian/wiki/sources/` si vamos a fijar decisiones.
5. Después de Phase 1: revisión humana del UX antes de Phase 2, porque el Composer page y el Hub landing son los que más se notan.

---

## 16. Interfaz: alternativas de diseño y decisiones

§7 ya describe la IA y mockups. Aquí entro en las decisiones estéticas y de patrón con alternativas razonadas.

### 16.1 Lenguaje visual — tres caminos

| Camino | Idea | Pros | Cons |
|---|---|---|---|
| **A. AMC-clone** | Mismo grid de tarjetas grandes, mismas pills, misma jerarquía visual | Familiaridad inmediata para usuarios AMC; comparación obvia | Se siente derivativo; pierde la identidad mySone |
| **B. SONE-native** | Hereda el theme actual (`th-*`, gradientes accent, cards rounded-2xl, animaciones suaves), traduce los patrones AMC al lenguaje SONE | Coherencia con Stats, Galaxy, Live painting; identidad propia | Más diseño; el usuario puede no asociarlo con "experiencia clásica" |
| **C. Hybrid (recomendado)** | Layout y jerarquía AMC (porque resuelven bien el problema), tipografía + color + microinteracciones SONE | Mejor de los dos mundos; no reinventa la rueda donde AMC acertó | Requiere disciplina para no acabar siendo A |

**Decisión**: **C (Hybrid)**. Justificación: AMC's IA está validada por años de uso; reinventarla sería arrogancia. Pero el lenguaje visual SONE ya tiene personalidad (gradientes, glow accents, theme tokens) y queremos que el Hub se sienta parte de SONE, no un app injertado.

Concretamente:
- **Layout**: copia el patrón AMC (hero composer, recordings list con cover thumbnail + metadatos en fila).
- **Tarjetas**: usa `rounded-2xl border border-th-border-subtle bg-th-surface/60` ya canónico en `StatsPage.tsx`.
- **Color**: el accent del theme (`var(--th-accent)`) es el highlight; quality badges usan paleta dedicada (verde Hi-Res, azul Lossless, ambar MQA, púrpura Atmos, gris no-Tidal).
- **Microinteracciones**: usa los mismos `transition-colors hover:border-th-accent/40` y `hover:scale-110` que ya hay en el codebase.
- **Tipografía**: mantén el grupo `font-extrabold` para títulos grandes (heredado del Stats hero), `font-bold uppercase tracking-[0.18em]` para etiquetas.

### 16.2 Densidad de información: dos modos

Los listeners clásicos varían de "casual quiero algo bonito" a "audiófilo quiero ver bit depth, sample rate, fecha exacta de grabación, productor, ingeniero". Dos modos seleccionables (toggle en header del Hub):

| Modo | Qué muestra por fila de recording | Para quién |
|---|---|---|
| **Browse** (default) | Cover · Conductor · Orquesta · Año · Quality badge · Play | Casual / explorador |
| **Detailed** | Lo anterior + Sello · Solistas · Venue · Productor · Sample-rate exacto · Bit depth · Atmos badge | Audiófilo / coleccionista |

Densidad la controla el toggle, no el zoom del browser. El estado vive en localStorage (`sone:classical:density`) y persiste entre sesiones.

### 16.3 "Compare mode" — feature único, sin equivalente AMC

Un check-box por fila en la lista de recordings → cuando 2-3 están checked, aparece un sticky bar abajo "Compare 2 recordings" → click expande overlay side-by-side:

```
┌─────────────────────────────────────────────────────────────┐
│  Karajan / BPO / 1962 / DG    │  Bernstein / WP / 1979 / DG │
│  ─────────────────────────────│─────────────────────────────│
│  Quality: LOSSLESS 16/44.1    │  Quality: HIRES 24/96       │
│  Stereo                       │  Stereo                     │
│  Duration: 67:23              │  Duration: 73:51            │
│  Year: 1962                   │  Year: 1979                 │
│  Venue: Jesus-Christus-Kirche │  Wiener Musikverein         │
│  Producer: Otto Gerdes        │  Hans Hirsch                │
│  ─────────────────────────────│─────────────────────────────│
│  Your plays: 12 (9 completed) │  Your plays: 3 (3 completed)│
│  Last played: 2 weeks ago     │  Last played: 6 months ago  │
│  ─────────────────────────────│─────────────────────────────│
│  ▶ Play this                  │  ▶ Play this                │
└─────────────────────────────────────────────────────────────┘
```

Esto es **exactamente** lo que un coleccionista clásico haría manualmente abriendo Discogs en una pestaña y Wikipedia en otra. AMC no tiene nada parecido. Coste: ~2 días en Phase 5 (UI sobre datos ya cacheados).

### 16.4 Timeline view — alternativa de browse

Además de "Browse by composer / period / genre", una vista **timeline horizontal** del compositor: works ordenados por año de composición, scrollable horizontal, con altura proporcional a popularidad. Es el equivalente clásico de las "decadas" de Tidal Explore. Click en un work salta a su page.

```
1770──1780──1790──1800──1810──1820──1830 ›
         │     │     │     │     │
         │     │   ┌─┴─┐   │   ┌─┴─┐
         │     │   │Sym3│   │   │Sym9│  ← altura=popularidad
         │     │   └───┘   │   └───┘
         │   ┌─┴─┐         │ ┌─┴─┐
         │   │PS14│         │ │MS │
         │   └───┘         │ └───┘
       ┌─┴─┐               │
       │Sym1│               │
       └───┘
```

Pros: visualmente memorable, único, contextualiza la obra en la vida del compositor.
Cons: requiere `compositionYear` (Wikidata P571 — coverage mediocre para barroco/medieval).

**Decisión**: ship en Phase 5 como vista opcional (toggle "List | Timeline" en composer page). Si Wikidata no tiene la fecha, fall through a List.

### 16.5 Listening mode — focus state del player

Para óperas y obras largas (Mahler 8, Wagner Ring, Bach Pasiones), un "focus mode" que oculta sidebar + minimiza header y muestra full-screen:

- Cover de la grabación a la izquierda.
- Centro: título de obra grande, movimiento actual destacado, próximos movimientos en gris.
- Inferior: progreso del movimiento + progreso del work completo.
- Esquina: tempo metronome (si MB lo tiene) y key signature.

Activable con `F` (full-screen) o tap sobre la player bar. Ya existe Live Painting Mode (`6f31ec6` commit), patrón similar.

### 16.6 Animaciones / motion

Mantén el estilo SONE: `transition-all duration-150-300`, `cubic-bezier`, leves `scale-95` on click. Evita flashy. El Hub debe sentirse calmado — clásica es contemplativa, no algorithmic-pop-radio.

### 16.7 Responsive / desktop-only

Phase 1-4 son desktop-only (windowed Tauri). Pero en Phase 5 vale invertir 1-2 días en que la vista sea legible si el usuario hace la ventana ≤ 800px de ancho:
- Recordings list colapsa columnas secundarias (label, venue) en accordeon.
- Composer page: portrait + bio se apilan vertical en lugar de side-by-side.
- Browse pages: grid 4-col → 2-col.

Esto **además** prepara el camino al Android app discutido en §18.

### 16.8 Theming clásico opcional

Setting "Classical theme override" (off por default): cuando entra al Hub, swap del accent color a un dorado-cálido (`hsl(40 80% 55%)`) y el surface a un crema oscuro. Recuerda visualmente al "modo concierto". Solo aplica dentro del Hub; salir restaura. Pequeño toque que distingue el sub-modo sin romper el resto de la app.

---

## 17. ¿Spin-off como app separada "SONE Classical"?

Tres opciones, evaluadas seriamente.

### 17.1 Alternativa I — Mantener todo dentro de SONE (plan actual, baseline)

Arquitectura: el Hub vive bajo Explore. Un binario, un release, un settings file. Compartido con el resto de SONE.

**Pros**:
- Cero overhead operacional (un build, un release pipeline, un user state).
- El usuario que escucha pop *y* clásica no tiene que abrir dos apps.
- Reusa al 100% el audio backend, scrobbling, stats DB, auth Tidal, share-link, todo.
- Single source of truth para `recording_mbid` enrichment — los plays clásicos también son plays normales que scrobblean a LFM/LB.
- Discoverabilidad orgánica: usuarios pop descubren el Hub por curiosidad y se enganchan.

**Cons**:
- El binario crece (snapshot OpenOpus + códigos del Hub + UI). Estimado +5-10 MB.
- Tiempo de arranque puede crecer marginalmente si hay pre-warm; mitigable con lazy init.
- Hay usuarios que *solo* escuchan clásica y para ellos el resto de SONE es ruido.
- El equipo de mantenimiento tiene que pensar en clásica en cada cambio de player/UI.

### 17.2 Alternativa II — Binario separado en mismo workspace ("SONE Classical")

Cargo workspace con:
```
mySone/
├── crates/
│   ├── sone-core/        ← audio, scrobble, stats, mb, classical/catalog
│   ├── sone-tidal/       ← Tidal client (extraído)
│   └── sone-classical/   ← lógica específica (Editor's Choice, etc.)
├── apps/
│   ├── sone/             ← Tauri binario actual (todo)
│   └── sone-classical/   ← Tauri binario nuevo (solo Hub + reproducción)
└── frontend/
    ├── shared/           ← componentes UI comunes
    ├── sone/             ← bundle frontend actual
    └── sone-classical/   ← bundle dedicated, sin Explore/Galaxy/Live painting
```

`sone-classical` se construye independiente, con **el mismo Rust backend** (mismo audio engine, mismo scrobbling, mismo bit-perfect contract) pero **frontend mínimo** centrado en el Hub. La auth Tidal se comparte (settings.json en el mismo `~/.config/sone/`) o se separa (`~/.config/sone-classical/`).

**Pros**:
- Usuario que solo quiere clásica tiene un binario más pequeño, arranque más rápido, UI sin distracciones.
- Posicionamiento de marketing más nítido ("alternativa libre a Apple Music Classical para Linux/Windows/macOS").
- Permite identidad visual divergente (theme dorado por defecto, branding distinto) sin tocar SONE principal.
- Posible release independiente en repos como Flathub / AUR como producto separado.
- Reuso de código máximo gracias al workspace.

**Cons**:
- 2× build matrix → 2× CI tiempo, 2× release artefactos.
- 2× Tauri config → 2× icons, deeplinks, single-instance, MPRIS, tray.
- Plays clásicos en `sone-classical` tienen que escribir a la misma stats DB que SONE para que tu "top works" sea uno solo. Implica file locking entre procesos (SQLite WAL lo permite pero hay edge cases).
- La auth Tidal compartida es un cross-app state — si SONE rota tokens, SONE Classical hereda; pero si están abiertos a la vez, race conditions posibles.
- Decisión de "qué va dónde" vuelve a ser una distracción cada vez que añades feature: "¿Live Painting va en SONE o en SONE Classical?"

### 17.3 Alternativa III — Repo completamente separado

Fork ahora, divergencia total. Diseño del Hub from scratch sin la herencia del player de pop.

**Pros**:
- Libertad arquitectónica máxima.
- Equipos posibles independientes.
- No hay tensión entre "feature pop" y "feature clásica".

**Cons**:
- Duplicación masiva de código (audio, scrobbling, MB, Tidal client...).
- Los fixes de seguridad / bugs de audio hay que portar manualmente entre los dos repos.
- Stats divergentes — pierdes el cross-source de §4.5.
- Operacional: doble release, doble issue tracker, doble docs, doble doc de arquitectura...

### 17.4 Recomendación

**Phase 1-4: Alternativa I** (binario único, Hub bajo Explore). Es lo que está plenamente justificado mientras desarrollamos.

**Después de Phase 4, evaluar Alternativa II** si:
- Hay tracción real (telemetría: ≥ 30% de plays vienen del Hub).
- El usuario lo pide explícitamente como producto separado.
- Hay capacidad de mantener dos pipelines.

**Alternativa III nunca** salvo que SONE Classical pivote a otro provider (Qobuz/Apple Music), que justificaría desacoplamiento total.

El paso II es fácil de ejecutar **si** desde Phase 1 estructuramos el código bien:
- Catalog service en su propio módulo (`src-tauri/src/classical/`) sin acoplamiento a Explore o Galaxy.
- Components React bajo `src/components/classical/` autocontenidos.
- Domain types separados (`src/types/classical.ts`).

Si esto se respeta (y el plan actual lo respeta), **migrar a workspace en el futuro es cosmético** — mover archivos a un crate, no rediseñar.

---

## 18. Android app — opciones realistas

Pregunta crítica: ¿qué problema resuelve un Android app? Hay tres respuestas plausibles y cada una lleva a una arquitectura distinta.

### 18.1 Tres premisas posibles

| Premisa | Implica |
|---|---|
| **P1. App standalone clásica para móvil** | Catálogo + reproducción + UI todo en Android. Compite con AMC en su terreno fuerte (iPhone/Android). |
| **P2. Companion / remote control de SONE desktop** | El usuario tiene SONE corriendo en su Linux box / NAS y el Android es para queue control + browse remoto. Reproducción ocurre en el desktop con su DAC. |
| **P3. Hybrid: standalone con sync opcional al desktop** | El Android puede reproducir solo (Tidal directo) o controlar el desktop. Lo mejor de ambos. |

### 18.2 Análisis por premisa

**P1 (standalone)** es la opción más ambiciosa y, honestamente, la peor decisión estratégica. Razones:
- Apple gana en su propio terreno: AMC tiene 1.2M recordings curadas, partnerships con Berlin Phil, Carnegie, etc., editores internos. **No hay forma de igualar la curación editorial sin invertir el equivalente a Primephonic en años de trabajo manual.**
- El USP audiophile (bit-perfect, exclusive ALSA) **no existe en Android**. Android maneja audio via AudioFlinger + AAudio/Oboe, con un mixer del sistema que normalmente upsamplea o downsamplea sin pedir permiso. Hay flags Hi-Res en Snapdragon premium pero el ecosistema no es Linux-grade. Pierdes el principal diferenciador.
- Pierdes el setup de hardware del usuario (DAP HiBy R4, DACs USB, etc. — ver `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/user_hardware.md`).

**P2 (companion)** es mucho más interesante:
- El SONE desktop sigue siendo el motor: bit-perfect, DAC USB, ALSA exclusive.
- El Android es UI para navegar el catálogo + queue control.
- Encaja con la nota de memoria `project_tidal_connect_pending.md` que ya tienes (HiBy R4 + SONE como controlador remoto).
- Uses share-link infrastructure (commits `8b64dab`, `fa4b122`, `dfe5372`) que ya existe — el desktop expone HTTP + control endpoints, el Android los consume.

**P3 (hybrid)** es lo que más se vendería pero tiene 2× el coste de P2.

### 18.3 Stack technology — Android

Cinco caminos técnicos:

| Stack | Reuso de código | Quality | Esfuerzo | Recomendado para |
|---|---|---|---|---|
| **Tauri Mobile (2.0)** | Backend Rust reutilizado parcialmente (no audio) | Mid (WebView UI) | Media-alta | P2 (companion) — si el Android no reproduce localmente |
| **Kotlin Multiplatform + Compose** | Solo lógica abstracta (catalog) en Kotlin | Alta (native UI) | Alta | P3 si lo abordas en serio |
| **Capacitor / PWA wrapper** | 100% del frontend React, fake-native shell | Baja (no offline real, no media session decente) | Baja | Prototipo rápido P2 |
| **Native Android (Kotlin + ExoPlayer)** | Cero código compartido, pero ExoPlayer + Tidal SDK son sólidos | Máxima | Alta | P1 standalone (no recomendado) |
| **Flutter** | Catalog logic en Dart | Alta | Alta | Si quieres iOS también desde la misma base |

### 18.4 Recomendación Android

**Camino I — Companion via PWA + share-link extension (cheap, fast)**

Aprovechar que SONE ya tiene share-link (HTTP server expuesto en LAN cuando estás compartiendo música). Extender ese endpoint para incluir:
- `GET /classical/composer/{mbid}` etc. (los mismos commands del Hub).
- `POST /control/queue/append`, `/control/play`, `/control/seek`.
- Frontend: el mismo bundle React, con una flag `?mode=companion` que oculta lo desktop-only y se conecta vía HTTP al desktop en lugar de Tauri IPC.
- Add-to-home-screen vía PWA manifest.

Coste: ~2 semanas. Reuso al 100% del frontend.
Limitación: solo funciona cuando estás en la misma red que el desktop.

**Camino II — Tauri Mobile companion (Phase 2024+)**

Tauri 2.0 soporta Android. El backend Rust se compila para `aarch64-linux-android`. Cierta cantidad de cosas no funcionan (mpris, idle_inhibit, hw_volume ALSA, GStreamer linux-only). Para companion mode no necesitas audio en el móvil, solo HTTP client al desktop.

Coste: ~1 mes.
Pros: app nativa instalable, no requiere desktop encendido si el catálogo lo cacheas (puedes browseear Hub offline, queue se aplica cuando reconectes).
Cons: WebView Android tiene quirks (especialmente con Chromium versions antiguas).

**Camino III — Native Android Kotlin (full standalone, no recomendado)**

Solo si quieres P1 (standalone). Reescritura completa, ExoPlayer + Tidal SDK Android (existe en repos no-oficiales similares al `tidalapi` Python). 3-6 meses de trabajo full-time. Sin reuso del Rust.

### 18.5 Sintetizando: hoja de ruta Android

**Si decides hacer Android app**:

1. **Phase 7 — PWA companion** (~2 semanas): habilita el frontend en modo companion HTTP-only. El usuario instala como PWA en Android. Demuestra el valor sin commit grande.
2. **Phase 8 — Tauri Mobile companion** (~1 mes): si el companion PWA gana tracción, migra a app nativa Tauri Mobile. Mejor integración con media session de Android (control desde lockscreen, Android Auto).
3. **Phase 9 — Standalone opcional**: si los usuarios piden reproducir directamente en el móvil (no remote-only), añadir camino native con ExoPlayer + reuso del catalog crate via FFI. Solo si Phase 8 demuestra demanda.

**No recomiendo Phase 9 directo**. Salta de cero a "competir con Apple en su terreno" es la receta de un side-project que muere antes de llegar.

### 18.6 La pregunta filosófica

La razón profunda por la que SONE Classical en desktop puede ser legendaria es que **AMC nunca ha tratado al audiófilo desktop**. Ese es un nicho desatendido con hardware específico (DACs USB, monitores activos, sistemas hi-fi conectados al PC).

Ese nicho **no existe en Android**. El audiófilo móvil usa un DAP dedicado (HiBy, FiiO, A&K) con su propio software, o IEMs con Bluetooth donde la calidad audio se cae al nivel de AAC. Llegar a Android compitiendo por el mismo nicho es jugar fuera de casa.

Por eso la lectura honesta es: **Android tiene sentido como companion**. Como standalone clásico, juegas la partida de Apple y la pierdes.

---

## Referencias

**Apple Music Classical (citado por agente AMC)**:
- [What Hi-Fi: Apple Music Classical](https://www.whathifi.com/features/apple-musical-classical-everything-you-need-to-know)
- [Apple Newsroom (Mar 2023)](https://www.apple.com/es/newsroom/2023/03/apple-music-classical-is-here/)
- [Apple Discussions: Hi-Res Lossless filter](https://discussions.apple.com/thread/254762608)
- [Apple Community: gapless playback issues](https://discussions.apple.com/thread/256011058)
- [Audiophilia: not ready for primetime](https://www.audiophilia.com/reviews/2023/3/28/v0c1vr79z3gu8eoe8k4ggxdu3p4y6s)
- [Variety review](https://variety.com/2023/music/reviews/apple-music-classical-platform-app-review-1235573985/)
- [Six Colors first look](https://sixcolors.com/post/2023/03/first-look-apple-classical-is-tuned-for-the-genre-but-hits-a-few-false-notes/)

**Fuentes de datos**:
- [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API)
- [MB Work entity](https://musicbrainz.org/doc/Work)
- [Wikidata P435 (MB Work)](https://www.wikidata.org/wiki/Property:P435)
- [Wikidata SPARQL](https://www.wikidata.org/wiki/Wikidata:SPARQL_query_service)
- [OpenOpus API](https://github.com/openopus-org/openopus_api)
- [Wikipedia REST API](https://en.wikipedia.org/api/rest_v1/)
- [Cover Art Archive](https://musicbrainz.org/doc/Cover_Art_Archive/API)
- [TIDAL Web API Reference](https://tidal-music.github.io/tidal-api-reference/)
- [Discogs Developers](https://www.discogs.com/developers)

**Codebase**: ver §12 para mapeo exacto de archivos y líneas.
