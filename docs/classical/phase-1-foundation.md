# Phase 1 — Foundation: Catalog service + 1 Work page

**Status**: ⚪ pending (Phase 0 → GO con asterisco habilita el arranque)
**Owner**: TBD (delegará a `sone-backend-engineer` + `sone-frontend-engineer`)
**Tiempo estimado**: ~110h (el original era 90h; +20h por la enmienda D-010)
**Decision gate**: Beethoven 9 page carga en < 3s warm-cache, < 30s cold-cache; lista muestra ≥ 20 recordings con datos correctos; cualquier recording playable se reproduce sin tocar audio routing.

---

## Contexto crítico — leer antes de empezar

- **D-010** (en `DECISIONS.md`): la arquitectura de catálogo no puede depender exclusivamente de ISRC→Tidal. Phase 0 demostró que MB tiene ISRC para sólo 14.8% del canon de muestra; Tidal sí tiene el catálogo completo (100% de canon-probes). Por eso Phase 1 debe implementar **el cascade de matching**: ISRC primero, Tidal text search después, con UI que distingue confianza.
- **Bit-perfect contract** (`feedback_bitperfect_contract.md` + `CLASSICAL_DESIGN.md` §0): la reproducción de un recording desde el Hub debe ser idéntica a la actual. Cero cambios en `route_volume_change`, en el writer, ni en el signal_path. El catalog service queda OUT del audio path.
- **Cero regresión** (`CLASSICAL_DESIGN.md` §10): ninguna área existente (Explore, Sidebar, Player, Stats, Galaxy, Live painting, Share link, Scrobbling, MusicBrainzLookup, Cache) puede romperse. Cada cambio que toque esos archivos pasa por checklist explícito.

---

## Objetivo

Shippeable mínimo: el usuario reproduce un track con `recording_mbid` resuelto, ve un botón "View work" en el player, hace click, llega a una `WorkPage` que:

1. Muestra título, compositor, fecha de composición, descripción breve.
2. Lista los movements (si MB los tiene como child works).
3. Lista ≥ 20 recordings de la obra con: cover, conductor, orquesta, año, sello, badge calidad audio, botón play.
4. Distingue visualmente:
   - 🟢 ISRC-bound (alta confianza)
   - 🟡 Inferred-by-text (media; tooltip muestra el query)
   - ⚫ Not on Tidal (info-only)
5. Click en play reproduce el track Tidal vía pipeline existente (sin tocar el audio path).

Phase 1 NO incluye:
- Hub landing.
- Browse pages.
- Search clásico.
- Composer pages standalone (sólo placeholder linkable).
- Editor's Choice manual.
- Filters de calidad (eso es Phase 4).
- Player upgrades (Phase 3).

---

## Tareas

### Backend (Rust) — ~70h

#### B1. Módulo `src-tauri/src/classical/` (nuevo)

```
src-tauri/src/classical/
  mod.rs              -- export públicos del módulo
  catalog.rs          -- CatalogService: orquesta providers + cache
  providers/
    mod.rs            -- trait ClassicalProvider + tipos comunes
    musicbrainz.rs    -- impl: fetch_work, fetch_recording_set
    wikipedia.rs      -- impl: composer + work descriptions
    wikidata.rs       -- impl: catalogue numbers + composition year
    tidal.rs          -- impl: ISRC lookup + text search cascade
    openopus.rs       -- impl: era + popular works (snapshot)
  matching.rs         -- el cascade ISRC → text-search con scoring
  types.rs            -- domain types alineados con CLASSICAL_DESIGN.md §5.1
```

Reglas:
- `pub mod classical` en `lib.rs`.
- Cada provider implementa `enrich_*` con best-effort (no falla el flow entero si una fuente falla).
- Tests unitarios con fixtures JSON para cada provider.

#### B2. Cascade matching (nuevo, dictado por D-010)

`matching.rs` expone:

```rust
pub enum MatchConfidence {
    IsrcBound,           // determinista
    TextSearchInferred,  // top-1 con score >= threshold
    NotFound,
}

pub struct MatchResult {
    pub tidal_track_id: Option<u64>,
    pub confidence: MatchConfidence,
    pub query_used: Option<String>,  // para inferred — para mostrar al usuario
    pub score: Option<f64>,
}
```

Flow:
1. Si la recording tiene ISRC → `lookup_tidal_by_isrc` (ya implementado en spike, mover a producción). Si match → `IsrcBound`.
2. Si no → construir query canónico: `{composer_last_name} {work_title_normalized} {primary_artist} {year_or_decade}`. Llamar `tidal.search(query, 5)`. Scorear cada hit por:
   - artist substring match (peso 0.4)
   - work title substring (peso 0.3)
   - año ±2 (peso 0.2)
   - duración ±10% (peso 0.1)
3. Si top score ≥ 0.6 → `TextSearchInferred`. Si no → `NotFound`.

Tests obligatorios con fixtures:
- ISRC happy path.
- Texto sin año (Glass Glassworks).
- Texto con año y resultado correcto (Beethoven 9 Karajan 1962).
- Texto donde Tidal devuelve un movement primero (Mahler 9 → Andante comodo) → debe agregar al recording, no al movement.
- NotFound (Furtwängler Bayreuth 1951 — el spike confirmó que está, pero un test de fixture cubre el path negativo).

#### B3. CatalogService

```rust
pub struct CatalogService {
    cache: Arc<DiskCache>,       // reusa el existente
    providers: Vec<Box<dyn ClassicalProvider>>,
    matching: matching::Matcher,
    mb_rate: Arc<MbRateLimiter>, // shared con MusicBrainzLookup existente
}

impl CatalogService {
    pub async fn get_work(&self, mbid: &str) -> Result<Work, SoneError>;
    pub async fn get_recording(&self, mbid: &str) -> Result<Recording, SoneError>;
    pub async fn get_composer(&self, mbid: &str) -> Result<Composer, SoneError>;
}
```

- Cache key: `classical:work:v1:{mbid}` (versionado para futuras migraciones).
- TTL: 30d positivo, 24h negativo (alineado con CLASSICAL_DESIGN.md §3.3).
- Lazy enrichment: la primera call devuelve work + recordings shell (sin credits completos por recording). Hover/click expande on-demand.

#### B4. Tauri commands `src-tauri/src/commands/classical.rs`

- `get_classical_work(mbid: String) -> Work`
- `get_classical_recording(mbid: String) -> Recording`
- `get_classical_composer(mbid: String) -> Composer` — placeholder en Phase 1, devuelve sólo nombre + bio.

Registrados en `lib.rs::run` junto al resto de handlers. Aditivo, no rompe nada.

#### B5. DB migration

```sql
-- migration N+1 (additive)
CREATE TABLE IF NOT EXISTS classical_favorites (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  mbid TEXT NOT NULL,
  display_name TEXT NOT NULL,
  added_at INTEGER NOT NULL,
  UNIQUE(kind, mbid)
);
```

Ya está scaffold en `CLASSICAL_DESIGN.md` §7.3. Migration aditiva, igual que las anteriores.

#### B6. Pre-warm de canon (lite)

En primer launch del Hub, fetch background de top-3 compositores OpenOpus (snapshot estará en `src-tauri/data/openopus.json` — ya snippeado en CLASSICAL_DESIGN.md §3.4). En Phase 1 sólo top-3 (no top-30) para limitar el rate budget. Phase 6 expande.

### Frontend (TypeScript / React) — ~40h

#### F1. Types `src/types/classical.ts`

Mirror exacto del backend `types.rs`. Ver `CLASSICAL_DESIGN.md` §5.1.

#### F2. API wrappers `src/api/classical.ts`

Patrón de `src/api/stats.ts`: una función por command, tipada, con error handling consistent.

#### F3. Componentes `src/components/classical/`

- `WorkPage.tsx` — la pieza central. Layout según `CLASSICAL_DESIGN.md` §7.2.
- `RecordingRow.tsx` — fila con cover + conductor/orq/año + badge confidence + botón play.
- `MovementList.tsx` — lista plana de movements.
- `ConfidenceBadge.tsx` — los 3 estados (🟢 🟡 ⚫). Tooltip explica.

Reglas frontend:
- **Llaves siempre** (también en JSX inline conditionals — `{cond && (...)}` es OK, `{cond && <X/>}` también, pero `if (cond) return <X/>` debe llevar llaves).
- Theme `th-*` (no hex literales).
- A11y: cada botón tiene aria-label, focus-ring visible, roles correctos.
- Virtualization (`react-window`) si la lista pasa de 50 recordings — ver §9 del doc.

#### F4. Player extension

En `Player.tsx` (o donde viva el render del player bar), si `currentTrack.workMbid` existe, render botón "View work" pequeño junto al título. Click → `navigateToClassical(work_mbid)`.

`navigateToClassical()` es función nueva en `useNavigation.ts:94-107`. Hace `setView({type: "explorePage", apiPath: "classical://work/{mbid}"})`. El switch en `App.tsx:164-174` se extiende para detectar el prefix `classical://` y renderizar `WorkPage` en lugar del Tidal explore content.

Cero cambio en código existente — sólo branch nuevo. La regresión es imposible si el branch `classical://` nunca se hit por usuarios sin `workMbid`.

#### F5. Resolver de `workMbid` en track started

Extiende `scrobble/mod.rs::on_track_started` (ver `CLASSICAL_DESIGN.md` §10): tras resolver `recording_mbid`, hacer un MB call adicional (cached) para resolver `work_mbid` desde recording-rels. Async, best-effort, no bloquea track start.

Persistir `work_mbid` en stats DB para que Phase 6 pueda agrupar por obra.

---

## Acceptance criteria (de CLASSICAL_DESIGN.md §11 — Phase 1 gate)

- [ ] Beethoven 9 page carga en < 3s con cache warm.
- [ ] Beethoven 9 page carga en < 30s con cache cold (relajado vs §11 que dice < 3s; el cold realista incluye el cascade de matching).
- [ ] La lista muestra ≥ 20 recordings con datos correctos (conductor + orquesta + año + label correctos en al menos 80% de los rows).
- [ ] Click play en cualquier recording 🟢 reproduce el track sin error.
- [ ] Click play en cualquier recording 🟡 reproduce algo plausible (test manual con QA list).
- [ ] El badge de calidad audio aparece donde Tidal lo expone.
- [ ] Cero regresión en: Explore, Sidebar, Player default state, Stats, Galaxy, Scrobbling, Share link, MusicBrainzLookup ISRC path, Cache schema actual.
- [ ] `cargo check` + `cargo clippy` (sobre archivos tocados) sin warnings.
- [ ] `npm run build` clean.
- [ ] Tests unitarios cubriendo: cascade matching (5 cases), provider fallback chain (3 cases), cache schema migration (smoke test).
- [ ] Tests manuales firmados: el bit-perfect chain sigue intacto al reproducir desde el Hub (verificación con un track Hi-Res 24/96 y un DAC compatible).

---

## Riesgos específicos de Phase 1

| Riesgo | Mitigación |
|---|---|
| Tidal text search devuelve movements en lugar del recording entero | Scoring penaliza títulos con "I.", "II.", numerales en romano. Si el top hit es un movement, agregar todos los movements del album como un "virtual recording" agrupado. |
| Score threshold 0.6 deja muchos casos en NotFound | Ajustar tras QA con 30+ works reales. Si > 30% de canon cae en NotFound, bajar a 0.5 con flag de inseguridad más visible. |
| MB rate limit hace cold-cache > 30s para Beethoven 9 | Lazy enrichment: render shell con metadata mínima inmediatamente, hidratar credits async. La acceptance pide < 30s para "datos correctos", no para cobertura full. |
| Cache crece sin control | TTL 30d + max-entries por tier (futuro: GC en idle). Phase 1 deja la knob lista. |
| Scrobbling ve `work_mbid` desconocido y falla | El campo es opcional; el path actual ignora desconocidos. Verificar con test que la column nueva no causa CRASH si se queda en NULL. |

---

## Próximos pasos al finalizar Phase 1

- Si gate ✅ → Phase 2 (Browse experience).
- Si gate fallado en cobertura: revisar threshold de matching, considerar editorial bundle más temprano (mover trabajo de Phase 5 a Phase 1.5).
- Si gate fallado en latencia: optimizar queries MB (parallel browse + cached prefix), o pre-bake snapshot en build.

---

## Checklist supervisor (al recibir entregables)

- [ ] §1 estilo (llaves) verificado en cada archivo nuevo.
- [ ] §10 regresión: cada archivo modificado existente está justificado y tiene test/QA correspondiente.
- [ ] §5.2 provider pattern: cada nueva fuente es un `ClassicalProvider`, no un command directo.
- [ ] §3.3 cache TTLs: cada nuevo cache tiene TTL definido en doc.
- [ ] D-010 cascade: ISRC primario + text-search secundario implementados con confidence tiering.
- [ ] FEATURES.md y/o CLASSICAL_DESIGN.md actualizados.
- [ ] PROGRESS.md, CHECKPOINTS.md, DECISIONS.md updated con cada milestone significativo.
