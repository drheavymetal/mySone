# SONE Classical — arquitectura viva

**Estado**: skeleton. Se rellena a medida que se implementa.

> Este doc es la **síntesis técnica** de la arquitectura tal y como existe en el código. Se distingue del CLASSICAL_DESIGN.md (que es **prescriptivo** — qué debe construirse) en que ARCHITECTURE.md es **descriptivo** — qué existe ahora mismo y cómo encaja.

Mientras Phase 0 esté en marcha, este archivo solo enumera lo que se planea construir, con punteros al doc maestro. Tras Phase 1 se rellena con detalle real.

---

## Índice esperado (tras Phase 1)

```
1. Visión 30k pies
2. Capas y módulos
   2.1 Catalog service
   2.2 Provider trait y proveedores
   2.3 Cache strategy
   2.4 Stats DB schema additions
3. Capas frontend
   3.1 Domain types (src/types/classical.ts)
   3.2 API wrappers (src/api/classical.ts)
   3.3 Componentes
4. Flujos clave
   4.1 Open Work page (cold cache)
   4.2 Open Work page (warm cache)
   4.3 ISRC bridge MB → Tidal
   4.4 Editor's Choice resolution
5. Bit-perfect path en el Hub
   (CRÍTICO — verifica que nada del Hub introduce SW volume)
6. Decisiones diferidas
```

---

## 1. Visión 30k pies

Pendiente de Phase 1. Ver CLASSICAL_DESIGN.md §0 (TL;DR) y §5 (modelo de datos interno).

## 2. Capas y módulos

Pendiente. Estructura prevista (de §12 doc maestro):

```
src-tauri/src/classical/
├── mod.rs                   ← punto de entrada del módulo
├── catalog.rs               ← CatalogService orquestador
├── domain/                  ← types: Composer, Work, Recording, Movement
│   ├── mod.rs
│   ├── composer.rs
│   ├── work.rs
│   ├── recording.rs
│   └── performer.rs
└── providers/
    ├── mod.rs               ← trait ClassicalProvider
    ├── musicbrainz.rs       ← extiende MusicBrainzLookup existente
    ├── wikipedia.rs
    ├── wikidata.rs          ← cliente SPARQL
    ├── openopus.rs          ← snapshot bundled
    └── tidal.rs             ← ISRC bridge
```

```
src-tauri/src/commands/classical.rs   ← Tauri commands
src-tauri/data/openopus.json          ← snapshot pre-baked
```

```
src/types/classical.ts        ← domain types frontend
src/api/classical.ts          ← invoke wrappers tipados
src/components/classical/
├── ClassicalHubPage.tsx      ← root del Hub
├── ComposerPage.tsx
├── WorkPage.tsx
├── RecordingRow.tsx
├── QualityBadge.tsx
├── BrowseComposers.tsx
├── BrowsePeriods.tsx
├── BrowseGenres.tsx
├── BrowseConductors.tsx
├── BrowseOrchestras.tsx
├── BrowseSoloists.tsx
├── BrowseInstruments.tsx
├── ClassicalSearch.tsx
├── ClassicalLibrary.tsx
└── HourClock... (already exists in StatsPage)
```

## 3-6. (Pendientes)

Se rellenan a medida que el código existe.

---

## Bit-perfect path — invariante crítica

**Nunca, bajo ninguna circunstancia, el Classical Hub introduce un cambio que rompa el contrato bit-perfect.**

Mecánica del contrato (de `~/.claude/projects/-home-drheavymetal-myProjects-mySone/memory/feedback_bitperfect_contract.md`):

1. Capa router (`lib.rs::route_volume_change`): cuando `bit_perfect=true`, solo permite ruta `Hw` o `Locked`. La ruta `Sw` está prohibida.
2. Writer guard (`audio.rs` alsa-writer thread): rechaza changes de volumen SW cuando `bit_perfect=true`. Defensa de profundidad.

**Implicaciones para el Hub**:
- El Hub no toca volumen. Todo lo que hace el Hub es **catálogo** (lookups, browse, comparison) — el playback sigue exactamente el mismo path que hoy.
- Cambios en el player UI relacionados con classical (work title persistente, indicador de movimiento, badge bit-perfect) son **read-only** al estado de routing.
- Phase 3 incluye test del contrato post-cambios.

Si en cualquier momento un agente propone tocar `lib.rs::route_volume_change` o el alsa-writer thread como parte del Classical Hub, **escala inmediatamente al usuario**. No decide ni el supervisor solo.
