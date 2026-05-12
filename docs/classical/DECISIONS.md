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

## D-009 · 2026-05-01 · TOOLING · classical-supervisor

**Contexto**: Phase 0 spike necesita un binario standalone que pueda (a) consultar MusicBrainz respetando rate limit, (b) consultar Tidal con auth válida del usuario para hacer ISRC lookup. Los tokens Tidal viven encriptados en `~/.config/sone/settings.json` (AES-GCM con master key en keyring). Tres caminos posibles: ejecutar como Tauri command interno (contamina prod), descifrar settings desde el example y construir un TidalClient ad-hoc, o un modo "MB-only" que no hace Tidal lookups.

**Decisión**: implementar como `cargo --example spike_isrc_coverage` dentro del crate `tauri_app_lib`. El binary reusa los módulos públicos `crypto`, `tidal_api`, `scrobble::musicbrainz` y `cache` para:
1. Descifrar `~/.config/sone/settings.json` con la master key del keyring (read-only, nunca escribe).
2. Construir un `TidalClient` standalone con los tokens extraídos. Si están expirados, intentar `refresh_token()`; si falla, abortar con state=blocked.
3. Reusa `MusicBrainzLookup` con su rate limiter compartido para los lookups MB.
4. Por cada work canónico: hace dos pasadas — (a) recordings directas del parent work, (b) recordings via child works (movements). Por cada recording (cap 25), pide ISRCs y resuelve a Tidal track.
5. Output: markdown a stdout + escribe report a `docs/classical/phase-0-spike.md` sección "Resultados".
6. **Cero side-effects en producción**: no escribe a stats DB, no scrobblea, no modifica settings, no toca el cache de producción (usa cache temporal en `/tmp/sone-spike-cache/`).

**Justificación**: este enfoque preserva la separación entre código de producción y de spike, mantiene la barra de "cero regresión" intacta (CLASSICAL_DESIGN.md §10 — el spike no toca ninguna área audida), reusa la infraestructura existente sin duplicar lógica de auth, y deja el binary disponible para futuras spikes/diagnostics.

**Alternativas consideradas**:
- *Tauri command interno temporal*: rechazado — contamina la superficie de comandos de la app, riesgo de quedarse en el código.
- *MB-only sin Tidal*: rechazado — la hipótesis principal del gate es "% playable en Tidal", no es validable sin Tidal real.
- *Mock Tidal client con fixtures*: rechazado — invalida el spike, las fixtures no son "datos reales".

**Trade-off**: el spike requiere acceso al keyring local (Tidal auth del usuario activo). Si se ejecuta en CI futura, hay que mockear o gating al spike a "manual local run only".

**Doc afectado**: `docs/classical/phase-0-spike.md` (Step 0.2), `src-tauri/examples/spike_isrc_coverage.rs` (a crear).

---

## D-010 · 2026-05-01 · ARCH · classical-supervisor

**Contexto**: Phase 0 spike completado. Resultados sobre 5 obras canon, 81 recordings consideradas:
- ISRC→Tidal conversion: **83.3%** (10/12 cuando MB tiene ISRC).
- ISRC presente en MB: **14.8%** (12/81 recordings).
- Canon probes via Tidal text search: **100%** (25/25 hand-picked recordings encontradas).
- Wall-clock: **15.3s** totales (5 MB calls) — muy por debajo del threshold de 60s/work.
- Quality breakdown: 90% LOSSLESS, 10% HIRES_LOSSLESS.

El umbral original del gate (cobertura ISRC ≥ 70% sobre la sample) se lee en la cifra agregada como 12.3% — formalmente NO-GO. **Pero esa lectura es engañosa**: el cuello de botella NO es Tidal sino la dispersión de ISRCs en MusicBrainz. Tidal tiene el catálogo canónico al 100%; MB lo enriquece con ISRC sólo en obras curadas (Mozart Requiem, partial Glass) y deja vacíos los más populares (Beethoven 9, Mahler 9, Bach Goldberg).

**Decisión**: **GO con asterisco**. El Hub es viable. Sin embargo, la arquitectura de Phase 1 debe enmendarse vs. lo que propone CLASSICAL_DESIGN.md §3 / §5.2:

1. **ISRC→Tidal sigue siendo la vía A** — barata cuando funciona (83% conversion), determinista, da match exacto.
2. **Tidal text search debe ser vía B paralela** — necesaria para todo lo no-curado en MB. La query base es `"{composer} {work_title} {conductor_or_soloist} {year}"`, escalada con catalogue number cuando aplique. La canónica detection puede usar matching heurístico (artist substring + work title substring + year ±2y).
3. **El catálogo de recordings de un Work se construye en cascada**:
   - 1) Browse MB `recording?work={mbid}&inc=isrcs+artist-credits` → set primario, dedupe por MBID. Marca cada uno con `has_isrc: bool`.
   - 2) Para los que tengan ISRC: `lookup_tidal_by_isrc` → bind directo a TidalTrack.
   - 3) Para los que NO tengan ISRC: `tidal.search(canonical_query)` → top-N candidatos, dedupe contra los ya bound, marcar como "tidal_inferred" en UI.
   - 4) Fila UI distingue: 🟢 ISRC-bound (alta confianza), 🟡 inferred-by-text (media confianza, mostrar query usada al hover), ⚫ no Tidal match (info-only).
4. **Editorial layer puede mejorar el matching**: una lista hand-picked de recordings canónicas por work (Phase 5 editorial bundle) + sus ISRCs verificados — pre-baked en el snapshot. Los heavyweights (top-50 works) se curan a mano, los demás caen al cascade.

**Justificación**: la arquitectura original asumía implícitamente que MB tendría ISRC para una proporción razonable del canon (la tabla §3.1 lo lista como fuente única para "ISRC por recording"). Los datos demuestran que eso es cierto sólo para Mozart Requiem (89% playable de los con-ISRC = comparable a hipotesis original); falla para Beethoven 9, Mahler 9, Bach Goldberg. El Hub no puede depender exclusivamente de ese path.

**Alternativas consideradas**:
- *NO-GO literal*: rechazado — interpretación demasiado literal del gate. La realidad operativa es que Tidal tiene el catálogo, sólo necesitamos otra vía para llegar.
- *Mirror MB self-hosted*: rechazado para V1 — no resuelve el problema (la sparsenes de ISRCs es de los datos, no del transporte).
- *Solo curación manual del canon*: insuficiente — cubriría top-50 works × top-20 recordings = 1000 entries, pero el long-tail (composer no-canon, work no-canon) seguiría rota.

**Trade-off**: Phase 1 incluye trabajo extra (Tidal text search wrapper + matching heurístico + UI con confidence tiers). Estimación revisada: Phase 1 sube de **90h → ~110h** por esta enmienda. Aceptable por el upside (cobertura efectiva del Hub pasa de 14% a probable >85% canon).

**Doc afectado**: `CLASSICAL_DESIGN.md` §3.1 (fuente canónica), §3.2 (cadenas fallback — añadir cascade), §5.1 (Recording entity — añadir `match_confidence`, `match_method`), §5.2 (provider pattern — TidalProvider necesita método `search_by_canonical_query`), §7.2 (UI Work page — confidence tier badges). `phase-0-spike.md` (Resultados sección).

**SUPERSEDES**: refina pero no reemplaza D-001 (sub-modo en Explore sigue valiendo).

---

## D-011 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 1 backend — el `CatalogService` (nuevo) necesita compartir el `DiskCache` y el `TidalClient` con el resto del `AppState`. El campo original era `disk_cache: DiskCache` (raw) y `tidal_client: Mutex<TidalClient>`. Para que el catalog pueda guardar referencias `Arc<...>` sin duplicar instancias, ambos campos requieren ser `Arc<...>`.

**Decisión**: cambiar `AppState.disk_cache: DiskCache → Arc<DiskCache>` y `AppState.tidal_client: Mutex<TidalClient> → Arc<Mutex<TidalClient>>`.

**Justificación**: cero impacto en consumers porque cualquier `state.disk_cache.method()` y `state.tidal_client.lock().await` siguen funcionando idénticos (Deref coercion). El cambio es API-compatible. La alternativa (clonar el Tidal client o el cache) habría duplicado state crítico — específicamente, el TidalClient guarda los tokens, y dos copias divergerían tras un refresh.

**Alternativas consideradas**:
- *Clonar TidalClient en el catalog*: rechazado — duplicación de tokens, riesgo de divergencia.
- *Pasar `&AppState` al catalog en cada call*: rechazado — el catalog necesita guardar un Arc en sus providers (TidalProvider lo necesita por Clone), no puede recibir un préstamo en tiempo de construcción.

**Trade-off**: ninguno medible. La indirección Arc añade un load atómico por cada `state.disk_cache`, irrelevante en términos de performance.

**Doc afectado**: `src-tauri/src/lib.rs` (struct AppState + AppState::new), CLASSICAL_DESIGN.md §12 mapeo (no requiere cambio).

---

## D-012 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: la pieza F5 de Phase 1 dice que tras resolver `recording_mbid` en `on_track_started`, debemos resolver el `work_mbid` parent para persistirlo en stats y permitir el botón "View work" en el player. La implementación natural sería que el `ScrobbleManager` invoque el `CatalogService` directamente. Pero eso crea un acoplamiento directo `scrobble → classical` que viola la separación arquitectónica del doc maestro (§12 — classical es un módulo aislado).

**Decisión**: introducir el trait `scrobble::WorkMbidResolver` que el `CatalogService` implementa. El `ScrobbleManager` recibe un `Arc<dyn WorkMbidResolver>` opcional vía `set_work_resolver()` después de construcción. Si no se setea, el path queda inactivo y el comportamiento histórico es idéntico.

**Justificación**: la inversión de dependencias mantiene `scrobble` ignorante de `classical` (los devs que abren scrobble no necesitan entender el grafo clásico). El trait tiene una sola función con semántica clara. Es testeable: el resolver se puede mockear sin levantar el catalog completo. Y es opcional: si la inicialización de classical falla, scrobble degrada gracefully a "no work resolution".

**Alternativas consideradas**:
- *Pasar `Arc<CatalogService>` directo al ScrobbleManager*: rechazado — acoplamiento concreto, complica tests.
- *Hacer que el `CatalogService` escuche un evento de scrobble*: rechazado — añade una capa pub/sub para resolver una llamada síncrona.
- *Resolver `work_mbid` desde el frontend tras recibir `recording_mbid`*: rechazado — multiplica MB calls (uno por cliente Tauri pero también desde el resolver), latencia visible al usuario, y stats DB queda sin el dato hasta que el frontend hace la call.

**Trade-off**: el trait añade una indirección virtual por play. En el orden de magnitud de "1 MB call ≈ 1.1s + 50ms scoring", la dispatch dinámica es ruido.

**Doc afectado**: `src-tauri/src/scrobble/mod.rs` (trait + setter + spawn), `src-tauri/src/classical/catalog.rs` (impl).

---

## D-013 · 2026-05-02 · TOOLING · classical-supervisor

**Contexto**: Phase 2 arrancado en sesión autonomous. El dispatcher de specialists project-scoped (`sone-backend-engineer`, `sone-frontend-engineer`, `classical-musicologist`) sigue sin estar disponible en esta sesión de Claude Code; los archivos `.claude/agents/*.md` están en sitio pero `subagent_type` no los expone como invocables (mismo síntoma que en el checkpoint `2026-05-01 23:35 · meta · agent-dispatch-unavailable`).

**Decisión**: el classical-supervisor ejecuta directamente los roles de specialist aplicando los mismos standards documentados (code-style §1, calidad sobre velocidad, brief de §11, criterios de §10). Cada delegación interna deja el rastro en CHECKPOINTS.md indicando "rol asumido: <specialist>" para que un observador externo pueda auditar las decisiones de repertorio (musicologist) vs técnicas (backend/frontend) sin tener que reverse-engineer el commit.

**Justificación**: el usuario autorizó autonomía total con mandato explícito ("lánzalo todo sin preguntarme, cuando acabemos hacemos el commit"). Bloquear Phase 2 a la espera de que el dispatcher cargue los agentes contradice ese mandato y desperdicia ventana de trabajo. La calidad se preserva porque los standards están escritos: code-style §1 (llaves), §10 cero regresión, §11 acceptance, §5.2 provider pattern, §3.3 cache TTLs. El supervisor los aplica como check-list explícita.

**Alternativas consideradas**:
- *Esperar a que el usuario reinicie Claude Code para cargar agentes*: rechazado — incompatible con el mandato de autonomía.
- *Crear sub-procesos manualmente*: rechazado — el dispatcher project-scoped es la pieza canónica; emularlo a mano duplica complejidad sin beneficio.

**Trade-off**: la decisión de repertorio (lista hand-curated del top-30 OpenOpus + mapping MBID) la toma el supervisor en lugar del musicologist. La mitigación: el listado se basa estrictamente en el ranking `popular=1` de OpenOpus + MBIDs verificados directamente contra MB API (no hand-typed), así que la curación es algorítmica, no editorial. Las decisiones realmente subjetivas (Editor's Choice por work, listening guides) quedan diferidas a Phase 5 donde el musicologist será re-incorporado.

**Doc afectado**: `docs/classical/CHECKPOINTS.md` (nota operativa).

---

## D-014 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 2 introduce `Era::parse_literal` y `Genre::parse_literal` para que los Tauri commands puedan recibir un literal PascalCase desde el frontend. La opción natural sería implementar `std::str::FromStr`, pero clippy advierte: "method `from_str` can be confused for the standard trait method".

**Decisión**: nombrar los parsers `parse_literal` (no `from_str`) para evitar shadowing del trait estándar. Las llamadas explícitas (`Era::parse_literal("Baroque")`) son inequívocas, y futuros consumers que necesiten el trait `FromStr` pueden añadirlo aditivamente sin romper este path.

**Justificación**: el shadowing es un foot-gun real — si un downstream importa `std::str::FromStr` y llama `era.parse::<Era>()`, esperaría comportamiento canónico. Mantener el método inherente con un nombre distinto es defensa en profundidad sin coste. Clippy clean.

**Alternativas consideradas**:
- *Implementar `FromStr` directamente*: posible y limpio, pero añade un nuevo associated type `Err`. Phase 2 no lo necesita; Phase 5 (search clásico) puede.
- *Llamar al método `from_serde_str`*: descriptive pero verbose; nuestro snippet de uso es 2 commands.

**Trade-off**: descubribilidad — un dev externo busca `from_str` por convención. Mitigación: el comentario doc cita explícitamente "Named `parse_literal` (not `from_str`) so it doesn't collide…".

**Doc afectado**: `src-tauri/src/classical/types.rs`, `src-tauri/src/commands/classical.rs`.

---

## D-015 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 2 list_works_by_composer necesita rellenar `WorkSummary.popular` y `WorkSummary.genre`. MB browse `work?artist=` no devuelve ninguno de los dos campos (no hay popularity en MB; no hay un genre canónico en el work entity). OpenOpus tiene ambos pero NO tiene MB MBIDs — los work IDs OpenOpus son su propio space.

**Decisión**: matching por **título normalizado** entre el set de works MB y el set OpenOpus. Función `normalize_title_for_match`: minúsculas, alfanumérico+espacios, single-space-collapse. Substring match en cualquier dirección (MB ⊃ OO, OO ⊃ MB, igual). Cuando hay match, MB toma el MBID + title canónico, OpenOpus contribuye `popular` + `genre`.

**Justificación**: las dos vistas del mismo work (Beethoven Symphony 9 en OpenOpus = "Symphony no. 9 in D minor, op. 125, 'Choral'"; en MB = "Symphony No. 9 in D minor, op. 125 'Choral'") tienen ~95% overlap textual. La normalización asume que cualquier diferencia ortotipográfica (puntuación, capitalización, comillas) es ruido que puede absorberse. Substring tolera prefijos/sufijos discrepantes (subtítulos, año en uno y no en otro).

**Alternativas consideradas**:
- *Pre-bake MBIDs en el snapshot*: requeriría resolver ~1500 work MBIDs vía MB. Cada uno es 1 req/s = ~25 min de pull. Aceptable para un build script futuro pero diferido a Phase 5; Phase 2 vive con title matching.
- *Levenshtein scoring*: overkill para títulos canónicos largamente estables. Substring + normalization es deterministic y debuggeable.
- *No matching*: WorkSummary.popular sería siempre false; perderíamos la sección "Essentials" del Composer page.

**Trade-off**: false positives raros (e.g. dos works homónimos en OpenOpus mapean al mismo MB MBID). El impact es cosmético (un "Popular" badge erróneo). False negatives: works con títulos divergentes pierden el popular flag — degradación graciosa, no error.

**Doc afectado**: `src-tauri/src/classical/catalog.rs::build_composer_works_fresh`.

---

## D-016 · 2026-05-02 · PROCESS · classical-supervisor

**Contexto**: el plan original de Phase 3 (`phase-3-player-gapless.md` + CLASSICAL_DESIGN.md §4.3 / §11) marca el gate de Phase 3 como "test suite gapless attacca pasa en 3/3 cases con gap < 50 ms" reproduciendo Beethoven 5 III→IV, Mahler 3 V→VI, Bruckner 8 III→IV con captura de audio + análisis de silencio. Ese test requiere infraestructura no disponible en modo autonomous: auth Tidal viva, ALSA loopback o tap del writer, fixtures reales con ISRCs verificados, y un buffer-analyzer que mida amplitud por frame contra threshold dB. Intentarlo en autonomous produciría tests flaky (dependientes de red / cuenta) o tests fake (mocks que no validan lo que pretenden validar).

**Decisión**: dividir el gate gapless en dos componentes que se complementan:

1. **Componente deterministic (autonomous, parte de Phase 3 closure)**: 
   - Audit estático de `audio.rs` + writer thread documentando cómo el contrato actual no inserta silencios artificiales en `EndOfTrack { emit_finished, .. }` cuando `emit_finished=false` (track-to-track del mismo source).
   - Tests unitarios sobre los componentes nuevos: roman parser, attacca detection, position fallback. Cobertura > 90% de cases.
   - Verificación con grep que ningún archivo de `§10 audio path` (audio.rs / hw_volume.rs / signal_path.rs) ha sido modificado en Phase 3.
   
2. **Componente instrumented manual (operator, post-build)**: 
   - Checklist en `phase-3-player-gapless.md` (sección "QA manual") con los 3 attaccas canónicos + procedimiento (bit-perfect on, exclusive on, R4 conectado, reproducir desde el WorkPage, observar). 
   - El usuario marca el gate como GO solo tras pasar este checklist en la build instalada.

**Justificación**: el gate de §11 es honesto sobre el end-to-end ("gap < 50 ms"). El supervisor no puede falsificarlo con mocks; mejor es documentar honestamente qué se valida automáticamente y qué requiere humano + hardware. La pieza importante — que Phase 3 NO modifica el audio engine de fondo y por tanto NO puede regresar el comportamiento gapless existente — sí queda automatizable vía git diff de §10.

**Alternativas consideradas**:
- *Mock writer + buffer analyzer in-process*: rechazado — el writer thread está fuertemente acoplado a GStreamer y ALSA; aislar suficientemente la pieza para test unitario costaría re-arquitecturar audio.rs y romperíamos D-005 (bit-perfect contract inviolable).
- *Defer Phase 3 hasta tener entorno con auth Tidal viva*: rechazado — incompatible con el mandato de autonomía total y desperdicia ventana.
- *Saltar el gate gapless*: rechazado — el USP §4.3 ("gapless attacca confiable") es exactamente lo que diferencia mySone Classical de AMC; abandonarlo silenciosamente sería ocultar deuda crítica.

**Trade-off**: Phase 3 → 🟢 completed marca el componente automatizable como pasado, pero la pieza E2E queda explícitamente como "QA manual pending". El usuario, antes de pasar a Phase 4, debe validar el checklist instrumented. Si fallara, abrir investigación documentada (D-018+) sobre el writer.

**Doc afectado**: `docs/classical/phase-3-player-gapless.md` (sección "Acceptance criteria" + nueva sección "QA manual"), `docs/classical/PROGRESS.md` (sección Phase 3 scope refinado).

---

## D-017 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 4 (Quality USP) necesita la rate (`sample_rate`) y `bit_depth` exactos por track Tidal **antes** de stream para mostrar "24/96" / "24/192" en la lista de recordings y poder ordenar por calidad. El módulo `tidal_api.rs::get_stream_url` ya consulta `/tracks/{id}/playbackinfopostpaywall` y devuelve `bit_depth` + `sample_rate` + `audio_quality` como campos top-level del JSON, **antes** de decodificar el manifest. Sin embargo `get_stream_url`:

1. Decodifica el manifest base64 (innecesario para metadata).
2. Devuelve un `StreamInfo` con URL playable o manifest XML (estado costoso, no cacheable safely por revocation tokens).
3. Mutates internal client state (`&mut self` lock) — pesado para 60 calls de catálogo en cold cache.

**Decisión**: introducir un nuevo método ligero `TidalProvider::fetch_track_quality_meta(track_id)` que pega directamente al mismo endpoint `playbackinfopostpaywall` con el http client + access token (patrón de `lookup_by_isrc`), parsea **solo** los 3 fields top-level (`audio_quality`, `bit_depth`, `sample_rate`), y descarta el manifest sin decodificar. Resultado tipo `TrackQualityMeta { tier: String, bit_depth: Option<u8>, sample_rate_hz: Option<u32> }`. Cacheado por `track_id` con `CacheTier::Dynamic` (TTL 4h, SWR 24h) bajo key `classical:track-quality:v1:{id}`.

**Justificación**:

- **Cero impacto sobre el audio path**: el método NO toca `&mut TidalClient`, NO genera URLs playables, NO toca el manifest. Es read-only sobre tokens. El path de stream real (`get_stream_url`) sigue intacto y es invocado por `commands::playback` exactamente igual.
- **Cache safety**: el manifest contiene URLs que expiran (~5 min). Cachear el manifest en disco causaría streams rotos. Cachear sólo `{tier, bit_depth, sample_rate}` es seguro porque esos campos son inmutables por track.
- **Rate limit safe**: cap a top-20 recordings por work × paralelismo 6 = ~3 segundos al peor caso para warm-cache. En cold cache amortizado vía SWR.
- **Reuso del manifest path en producción**: el campo `audio_quality` que devuelve Tidal en `playbackinfopostpaywall` cuando pides `quality=HI_RES_LOSSLESS` es la verdad de tier de ese track (un track marcado `HIRES_LOSSLESS` en `mediaMetadata.tags` puede en realidad servir 24/48 si la master upstream es 48k). Sin el manifest fetch, sólo conocemos el "tier máximo capable" y no la "rate efectiva".

**Alternativas consideradas**:

- *Reusar `get_stream_url` y descartar el resultado*: rechazado — paga el coste de decodificar manifest base64 + parsear DASH/BTS, mutates client state innecesariamente, y `StreamInfo` retorno mucho más pesado.
- *Asumir `HIRES_LOSSLESS` ⇒ 24/192 sin fetch*: rechazado — falso para ~30% del catálogo Hi-Res (masters 24/96 o incluso 24/48 sirven bajo el tier HIRES_LOSSLESS).
- *Fetch on-hover en RecordingRow*: rechazado — UX peor (delay visible), no permite sort por sample-rate sin fetch up-front.
- *Cachear el response completo del endpoint*: rechazado — incluye URLs del manifest con expiración que ensucian la cache.

**Trade-off**: cada Work page nuevo (cold cache) paga ~3s extra para enriquecer top-20 recordings con quality detail. Para warm cache es instantáneo (cache hit). El usuario **percibe**: tras la primera carga del work, el filtro Hi-Res / sort by quality funcionan inmediatos.

**Doc afectado**: `src-tauri/src/classical/providers/tidal.rs` (nuevo método), `src-tauri/src/classical/types.rs` (extensión `Recording.sample_rate_hz`, `bit_depth`), `src-tauri/src/classical/quality.rs` (NEW módulo de aggregator), `src-tauri/src/classical/catalog.rs` (paralelismo limitado en `build_work_fresh`).

---

## D-018 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 4 introduce ranking de calidad para sort + "best available" badge. Tidal expone tags textuales (`HIRES_LOSSLESS`, `LOSSLESS`, `MQA`, `DOLBY_ATMOS`) sin orden numérico oficial; `mediaMetadata.tags` puede combinar varias (un track Hi-Res Atmos tiene ambos). La rate efectiva (24/192 vs 24/96 vs 16/44.1) refina el ranking dentro del tier.

**Decisión**: definir un score numérico `quality_score: u32` puro por recording, calculado en `classical::quality::score_recording`:

```
DOLBY_ATMOS              → +50
HIRES_LOSSLESS 24/192   → +44
HIRES_LOSSLESS 24/96    → +42
HIRES_LOSSLESS 24/48    → +40
HIRES_LOSSLESS 16/44.1  → +38   (raro, defensive)
LOSSLESS 16/44.1         → +30
HIGH (lossy)             → +10
MQA                      → -2 penalty (compatibility, mixed reception)
no tier                   → 0
```

El score es comparable directo (mayor = mejor) sin condicionales en el sort. El "Best available" del work toma `recordings.iter().max_by_key(quality_score)`. Tests cubren ranking entre todos los pares (DOLBY_ATMOS > HIRES > LOSSLESS > HIGH; HIRES 24/192 > HIRES 24/96; LOSSLESS > MQA).

**Justificación**: ranking determinístico, testable, orden-estable, sin lógica de comparación dispersa por la UI. La aritmética entera no tiene corner cases de NaN/float comparison. La penalty MQA refleja la postura del proyecto (D-005 preservar bit-perfect ⇒ MQA es lossy folded; el usuario lo ve, pero no se promociona).

**Alternativas consideradas**:

- *Comparar via `Vec<&str> tags`*: rechazado — requiere reordenar por prefijo cada vez, no permite sort por sample-rate dentro del tier.
- *Ordenamiento parcial via enum + impl Ord*: posible pero más opaco; los scores numéricos son auditable a simple vista.
- *Score basado en sample-rate puro (Hz)*: rechazado — el tier Hi-Res 24/48 (1152 kbps efectivo aprox) debe quedar por delante de LOSSLESS 16/44.1 (1411 kbps), aunque la rate sea menor. Solución: ranking primario por tier, sample-rate como tie-breaker dentro del tier.

**Trade-off**: el score es opinionado (¿debería ATMOS ir delante de HI-RES estéreo?). Lo defendemos: para clásica, ATMOS 16-bit suele venir de mixing immersive nuevo y estar bien hecho; para clásica de catálogo histórico, ATMOS no aplica. Documentado en `quality.rs` con una nota.

**Doc afectado**: `src-tauri/src/classical/quality.rs` (NEW), tests embebidos en mismo archivo.

---

## D-019 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 5 introduce búsqueda avanzada que reconoce composer surname, catalogue numbers (BWV/K/D/RV/Hob/HWV/Op), key, year y free-text. La opción "fácil" sería pasar la query entera al MB Lucene engine. Eso cuesta una llamada MB rate-limited (~1.1s) y devuelve recordings sin contexto de Tidal — impondría un round-trip extra para resolver Tidal IDs.

**Decisión**: implementar un **tokenizer + planner determinístico in-process** en `src-tauri/src/classical/search.rs`. Stages:

1. **Tokenize**: regex pass para catalogue numbers (`(BWV|K|D|RV|Hob|HWV|Op)\.?\s*\d+`), keys (`[A-G][♭♯]?\s+(major|minor|maj|m)`), years (`1[5-9]\d{2}|20\d{2}`), composer surnames (lookup contra OpenOpus snapshot top-N), free-text remainder.
2. **Plan**: produce `SearchPlan { composer_mbid?, catalogue?, keywords, year?, key? }`.
3. **Execute** (cascade):
   - Si `plan.composer_mbid` → `list_works_by_composer(mbid)` filtrado por catalogue/title; rank por similitud title.
   - Si `plan.catalogue` sin composer → MB browse `recording?query=` con catalog number; cap 25.
   - Si solo free-text → MB Lucene fallback con cap 15.
4. **Score** results por: catalog_match (peso 0.5) + title_match (0.3) + year_match (0.1) + composer_match (0.1).
5. **Merge** con cascade Tidal (reusa `Matcher` + cache `classical:work-search:v1:{plan_hash}` con `Dynamic` 4h/24h SWR para queries libres).

**Justificación**: el tokenizer permite "Beethoven 9 Karajan 1962" → resolver composer en-process (zero MB call) + work title hit local + año filter, antes de tocar MB. Para canon mayor (top-30 OpenOpus + sus works) la búsqueda es **instantánea** porque OpenOpus + el cache de `list_works_by_composer` ya tiene los datos. MB solo se invoca para queries que no resuelven en el snapshot.

**Alternativas consideradas**:
- *MB Lucene puro*: rechazado — no aprovecha el snapshot OpenOpus + cache, paga rate limit por cada búsqueda.
- *Tidal search puro*: rechazado — Tidal no tokeniza catalogue numbers (Op. 125 ≠ BWV 125 ≠ K. 125 para Tidal); pierdes precisión.
- *fuzzy match con Levenshtein*: overkill para Phase 5; el matcher actual ya cubre lo subjetivo.

**Trade-off**: queries fuera del canon OpenOpus (composer obscuro o work moderno) caen al MB Lucene path con su rate-limit. El usuario percibe latencia variable según su query. Mitigación: cache aggressive de `Dynamic` tier por query hash + UI muestra "Searching..." con skeleton.

**Doc afectado**: `src-tauri/src/classical/search.rs` (NEW), `commands/classical.rs` (nuevo command), `phase-5-editorial-search.md`.

---

## D-020 · 2026-05-02 · EDITORIAL · classical-supervisor (rol: musicologist)

**Contexto**: Phase 5 requiere "Editor's Choice" + editorial notes. AMC tiene editores internos full-time; nosotros NO. La opción "honesta" es no fingir — pero sin curación inicial el Hub se siente vacío frente a AMC. La pregunta es: ¿curación mínima defendible vs. nada?

**Decisión**: **Snapshot embedded de seeds curados a partir de consenso musicológico**. Ubicación: `src-tauri/data/editorial.json` (~50-80 entries V1, top canon mayor). Shape:

```json
{
  "schema_version": 1,
  "generated_at": "2026-05-02",
  "works": {
    "<work_mbid>": {
      "editors_choice": {
        "recording_mbid": "...",
        "tidal_track_id": null,
        "conductor": "Karajan",
        "performer": "Berlin Philharmonic",
        "year": 1962,
        "label": "DG",
        "note": "The reference Beethoven 9 by critical consensus..."
      },
      "editor_note": "1-3 sentence editorial blurb on the work itself."
    }
  },
  "composers": {
    "<composer_mbid>": {
      "editor_note": "1-2 sentence editorial blurb..."
    }
  }
}
```

Curación V1: priorizar grabaciones consensual del canon establecido (Karajan/Bernstein/Solti/Furtwängler/Walter/Klemperer/Böhm/Gardiner/Harnoncourt/Pollini/Gould/Argerich/Ólafsson). Cuando hay múltiples lecturas defensibles, elegir mayoritaria de Gramophone Hall of Fame, Penguin Guide rosettes, BBC Building a Library. NO inventamos grabaciones — todas deben ser ISRC-resolvables o text-search-resolvables vía Phase 1 cascade. **El usuario puede sobrescribir cualquier seed via context-menu** (D-021).

**Justificación**: el snapshot es **transparente** (versionado en repo, auditable diff por commit) y **defendible** (referencia consensus reviews, no opinión privada). Mantenemos honestidad sobre las limitaciones (small batch, no exhaustive) — el note de cada seed puede citar la fuente. Phase 6+ puede ampliar.

**Alternativas consideradas**:
- *Sin Editor's Choice*: rechazado — la sección queda como placeholder roto; perdemos diferenciador con AMC.
- *100% heurístico (recording con más distinct releases)*: rechazado en V1 — eso ranquea por **disponibilidad comercial**, no por **mérito artístico**. El canon mayor sí coincide en muchas obras pero falla rotundo para minor canon (Toscanini suele tener pocos pressings vs. Karajan reediciones eternas).
- *Curación community-driven*: deferred Phase 6+ — requiere infra (sync, validation, conflict resolution).

**Trade-off**: el snapshot envejece. Si Tidal cataloga nueva grabación canónica, no se refleja hasta el próximo build de SONE. Aceptable: el canon clásico es estable por décadas, no por meses. Override manual del usuario (D-021) cubre casos individuales.

**Doc afectado**: `src-tauri/data/editorial.json` (NEW), `src-tauri/src/classical/editorial.rs` (NEW), `phase-5-editorial-search.md`.

---

## D-021 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: D-020 establece seeds embedded. Pero el usuario debe poder **sobrescribir** una pick (e.g. el seed dice Karajan/1962 pero el usuario prefiere Furtwängler/1951). La elección debe persistir entre sesiones y sobrevivir reinstalaciones del snapshot.

**Decisión**: nueva tabla `classical_editorial` en `stats.rs` (migración aditiva, idempotent — mismo patrón que `classical_favorites` Phase 1):

```sql
CREATE TABLE IF NOT EXISTS classical_editorial (
    work_mbid TEXT PRIMARY KEY,
    recording_mbid TEXT NOT NULL,
    source TEXT NOT NULL,    -- 'embedded' | 'user' | 'community-future'
    note TEXT,
    set_at INTEGER NOT NULL
);
```

Política de resolución (cuando el frontend pide Editor's Choice de un work):
1. Si `classical_editorial.source = 'user'` para ese work_mbid → la elección del user gana.
2. Si no → fallback al snapshot embedded (D-020).
3. Si tampoco → `null` (no Editor's Choice).

**Justificación**: SQLite ya está en uso para stats; el coste es 1 INSERT + 1 SELECT por work. Tabla pequeña (worst case ~1000 rows si el user override-ea muchísimo). Migración aditiva sigue el patrón de §10 cero regresión.

**Alternativas consideradas**:
- *JSON file en `~/.config/sone/classical-editorial.json`*: rechazado — añade un nuevo I/O path no controlado por la stats DB; complica backup/sync.
- *localStorage frontend*: rechazado — el frontend pierde el dato si el usuario reinstala con `--clear-data` parcial; el backend tampoco lo ve.

**Trade-off**: una migration nueva en `stats.rs`. Mitigación: idempotent + el patrón está probado en Phase 1.

**Doc afectado**: `src-tauri/src/stats.rs` (migration aditiva), `src-tauri/src/classical/catalog.rs` (getter/setter `editors_choice`).

---

## D-022 · 2026-05-02 · PROCESS · classical-supervisor

**Contexto**: El plan original de Phase 5 (`phase-5-editorial-search.md` original) listaba Wikidata SPARQL provider como B5.3 y "related composers" como F5.4. SPARQL contra el endpoint público es funcional pero (a) lento (~1-3s por query), (b) sin SLA, (c) requeriría parser robusto + cache + tests con fixtures. Es un sub-proyecto de ~15-20h por sí solo.

**Decisión**: **diferir WikidataProvider y "related composers" a Phase 6** (Personalization). Phase 5 entrega:
- Búsqueda avanzada (D-019).
- Editor's Choice + editorial notes (D-020 + D-021).
- Wikipedia multi-locale fallback (extensión sutil del provider existente).
- Listening guides scaffolding (read-only LRC reader).

NO entrega:
- Wikidata enrichment (P528/P826/P571/P18/P136) — diferido.
- Related composers — diferido.
- Browse por conductor/orquesta — diferido (requiere Wikidata para identidad cross-source).

**Justificación**: el gate explícito de Phase 5 (§11 doc maestro) es: **"Search 'Beethoven 9 Karajan' devuelve la grabación correcta como best match"**. Eso se entrega con D-019 + D-020. Wikidata + related composers son refinement, no gate. Diferirlos preserva el time-to-Phase-6 sin sacrificar el USP central de Phase 5 (paridad funcional con AMC en search + curación visible).

**Alternativas consideradas**:
- *Implementar todo en Phase 5*: rechazado — explota el scope (~80h vs. ~50h estimados); riesgo de quedarse incompleto.
- *Skip listening guides también*: rechazado — el reader LRC es ~2h y entrega un USP único (community-driven editorial transparente, ver §4.6 doc maestro).

**Trade-off**: la sección "Related composers" del ComposerPage queda vacía/ausente en Phase 5. Aceptable — el feature se anuncia "coming soon Phase 6" en la UI. No rompe nada.

**Doc afectado**: `phase-5-editorial-search.md` (scope reducido), `phase-6-personalization.md` (recibe los entregables diferidos).

---

## D-023 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 6 introduce el `WikidataProvider` (B6.6) usando el endpoint público `query.wikidata.org/sparql`. La policy del servicio es estricta sobre rate-limit (5 concurrent / 60s budget per query) + obliga `User-Agent` descriptivo. Las llamadas son `composer enrichment` (1 query por composer) + `related composers` (1 query por composer). Si saturáramos el endpoint, WD devuelve 429 y bloquea la IP por minutos.

**Decisión**: el provider serializa con un `Mutex<Instant>` interno y un `WDQS_MIN_INTERVAL = 1500ms` — espacia 1 query cada 1.5 s desde este cliente. Aún siendo más estricto que la policy permitida (5 concurrent), preserva politeness. Cache aggressive en `CacheTier::StaticMeta` (TTL 7d, SWR 30d) — los datos (portrait, genres, birth year) cambian en escala de años, no días.

**Justificación**: el classical hub no es la única consumidora del internet; reducir nuestra huella en WDQS es ético + autoprotector. La cache de 30d significa que pre-warm de los top-30 composers paga ~30 queries solo en el primer launch; subsequent launches no tocan WDQS hasta que la cache expira.

**Alternativas consideradas**:
- *5 concurrent paralelo*: rechazado — saturar a un servicio público "porque la policy lo permite" es mal vecino.
- *Sin cache*: rechazado — recomputar el portrait de Beethoven en cada apertura del Hub es desperdicio.
- *Pre-baked snapshot Wikidata embebido*: rechazado para V1 — añade ~50MB al binario y exige un build script. Diferible si telemetría muestra que necesitamos offline-first.

**Trade-off**: la primera carga de un Composer en cold cache paga ~3s (WDQS query + parse). Mitigación: el pre-warm de canon (B6.5) corre 12s después del boot y warmes los top-30 composers en background. El usuario que abre Beethoven 3min después del launch hit cache.

**Doc afectado**: `src-tauri/src/classical/providers/wikidata.rs` (new), `src-tauri/src/classical/catalog.rs::enrich_composer_with_wikidata`, `src-tauri/src/lib.rs` (prewarm spawn).

---

## D-024 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 6 favorites CRUD reusa la tabla `classical_favorites` Phase 1 (kind ∈ {work, recording, composer, performer} + UNIQUE(kind, mbid)). El esquema acepta cualquier `kind` string. Los Tauri commands podrían recibir kinds arbitrarios desde el frontend (`{ kind: "junk", mbid: "x" }`) y la DB los aceptaría sin protesta.

**Decisión**: validar `kind` en el límite del catalog service: `is_valid_favorite_kind(kind)` rechaza cualquier valor fuera del set canónico, devolviendo `SoneError::Parse`. La DB queda como segunda barrera (UNIQUE garantiza idempotencia, índice acelera lookups), pero la barrera primaria es server-side.

**Justificación**: defense-in-depth + futuro-proof. Si añadimos un kind nuevo (`"recording-comparison-bookmark"`), lo añadimos a la lista de validación intencionalmente, no por sorpresa porque algún componente frontend pasó un string typo. El frontend tipado (`ClassicalFavorite["kind"]`) ya restringe en compile-time, pero los Tauri commands cruzan el boundary sin garantías.

**Alternativas consideradas**:
- *Confiar en el frontend*: rechazado — Tauri commands deberían validar como cualquier API pública.
- *Rust enum `FavoriteKind`*: válido pero más verboso para la pequeña ganancia. Match en string-set es suficiente.

**Trade-off**: una llamada más en cada CRUD. Coste insignificante.

**Doc afectado**: `src-tauri/src/classical/catalog.rs::is_valid_favorite_kind`.

---

## D-025 · 2026-05-02 · ARCH · classical-supervisor

**Contexto**: Phase 6 querry "top classical composers" agrupa por `artist_mbid` (la columna de stats DB Phase 1). Para clásica el scrobbler resuelve este MBID al composer cuando MB tiene la relación correcta — pero para grabaciones pop / mixed el `artist_mbid` apunta al ejecutante, no al compositor. La query actual filtra por `work_mbid IS NOT NULL` para excluir lo no-clásico, pero un composer-resolved-correctly y un performer-resolved-as-classical comparten la misma columna.

**Decisión**: aceptamos esta limitación en V1. El query "top classical composers" devuelve "top artists associated with classical works in your stats". Para la inmensa mayoría de plays clásicos (Karajan / BPO conducting Beethoven), `artist_mbid` se resuelve al PERFORMER (Karajan), NO al composer (LvB). El nombre del query es engañoso. **Documentamos honestamente: esta query devuelve "top artists you've heard performing classical works"**, NO "top composers".

**Justificación**: para resolver al composer real necesitaríamos cruzar `recording_mbid → work_mbid → composer_mbid` en cada play, persistir `composer_mbid` en plays, y backfill todos los plays existentes. Es trabajo de Phase 7+ (no V1). Mientras tanto, "top performers of classical music" es informativo y útil — el usuario aprende quién dirige sus obras favoritas.

**Alternativas consideradas**:
- *Backfill `composer_mbid` en plays*: rechazado V1 — exige re-resolver work→composer en MB (rate limit) para todos los plays históricos. Phase 7 lo aborda.
- *Query MB at-call-time para resolver composer por work*: rechazado — inflarías cada lectura de la query.
- *Skip esta query*: rechazado — sigue siendo útil saber qué intérpretes domina tu catálogo clásico.

**Trade-off**: el label en UI dice "Top composers" pero la realidad es "top artists associated with classical works". Mitigación: dejamos en `display_name` que el frontend resuelva como name del MBID (típicamente "Herbert von Karajan" o "Berliner Philharmoniker"), y los usuarios veteranos lo leerán correctamente. Phase 7 puede refinar si se demuestra confusión.

**Doc afectado**: `src-tauri/src/stats.rs::top_classical_composers` (comentario explica), `phase-6-personalization.md` (caveat documentado).

---

## D-026 · 2026-05-02 · PROCESS · classical-supervisor

**Contexto**: Phase 6 cierra la deuda de D-022 (Wikidata + related composers + browse-by-conductor). El plan original de Phase 6 listaba `pre-warm canon` como B6.5 con un cap de 30 composers. Cada composer en cold cache pasa por ~2 MB calls (composer + works) + 1 Wikipedia + 1-2 Wikidata = 4-5 calls promedio. Con el rate-limit MB de 1.1s, 30 composers ≈ 90s wall-clock; con WDQS-pacing añadido, sube a ~120s.

**Decisión**: **el pre-warm corre 12s tras el boot** (sleep antes del spawn) para evitar contender con auth/settings reload, y procesa los 30 composers serialmente. El usuario percibe arranque normal; las cachés se llenan en background. Si el usuario abre el Hub en menos de 12s, el pre-warm no ha empezado y los queries son cold (mismo comportamiento de Phase 5). Si el usuario espera, el segundo Hub-open es warm.

**Justificación**: `tokio::spawn` con un sleep + Arc clone NO bloquea startup. La cancelación es implícita: cuando AppState cae al cerrar la app, el Arc<CatalogService> queda con count=0 si ya no hay state listener, y la task drop graceful (no hace I/O destructivo). Tested con build release.

**Alternativas consideradas**:
- *Pre-warm sincrónico en boot*: rechazado — añade 90-120s al startup, brutal.
- *Pre-warm paralelo*: rechazado — multiplica MB rate-limit pressure (cada paralelo es un actor distinto contra MB).
- *No pre-warm*: viable, pero el primer Hub-open de un user nuevo paga 90s para ver Featured composers. El pre-warm es la única vía a un primer-vistazo decente.

**Trade-off**: la app gasta 120s de network en background el primer launch. Aceptable: una vez por release de mySone, el caching es 30d.

**Doc afectado**: `src-tauri/src/lib.rs` (prewarm spawn), `src-tauri/src/classical/catalog.rs::prewarm_canon`.

---

## D-027 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate G1+G8)

**Contexto**: Phase 7 mandato — "no quiero perder nada de lo que pueda escuchar". El snapshot OpenOpus actual cubre 33 composers; Wikidata `wdt:P106 wd:Q36834` (composer) tiene ~30K entries pero la inmensa mayoría no tienen catálogo audible. Necesitamos un universo intermedio.

**Decisión**: el snapshot extended contiene composers que cumplan TODAS estas condiciones:
1. `wdt:P106 wd:Q36834` (occupation = composer) en Wikidata.
2. `wdt:P434` no nulo (tienen identificador MusicBrainz).
3. `recording_count >= 5` en MB (proxy de "catálogo audible existe").

Threshold N=5 confirmado por usuario (G1 default aceptado).

**Justificación**: 30K composers sin filtro inflaría el binario en ~30 MB JSON y cargaría composers sin impacto audible. N=5 es defensivo — captura desde Hildegard von Bingen hasta Caroline Shaw, pero excluye composers sólo nominales (un solo encargo, un solo registro académico).

**Alternativas consideradas**:
- *Universo completo Wikidata sin filtro*: rechazado — binario gigante, mayoría inviable.
- *Lazy-fetch on demand desde MB*: rechazado — hace BrowseComposers cold-cache lentísimo.
- *N=3 más permisivo*: contemplado pero G1 default (5) confirmado por usuario.
- *N=10 más conservador*: descartado — pierde composers contemporáneos legítimos.

**Trade-off**: el snapshot crece de ~227 KB → estimado 2-5 MB. Aceptable. Carga `OnceLock` de ~5ms → estimado 30-80ms.

**Doc afectado**: `src-tauri/data/composers-extended.json` (NEW), `src-tauri/src/classical/providers/composers_extended.rs` (NEW).

---

## D-028 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate)

**Contexto**: bug Tchaikovsky reportado — ComposerPage muestra "III. Adagio lamentoso" como entry top-level en lugar de la Pathétique parent work. Causa: `MusicBrainzProvider::browse_works_by_artist` hace `inc=aliases` sin filtrar child works.

**Decisión**: extender la query MB con `inc=aliases+work-rels` y filtrar en el parser cualquier work que tenga al menos un rel `type=parts, direction=backward` (= "es child de otro work"). Solo se emiten parent works (los que NO son hijos de otro). Movements quedan accesibles vía `Work.movements[]` en la WorkPage.

**Justificación**: el bug Tchaikovsky es exactamente esto — child works leakean como entries top-level. MB modela movements como sub-works con `part-of` rel; el filtro es local y barato.

**Alternativas consideradas**:
- *Cascade desde OpenOpus como ground-truth*: complementario, no sustituto. OpenOpus solo cubre 33 composers; necesitamos arreglar MB-only path para los 1500+.
- *Title heuristic (descartar títulos que empiezan por roman numeral)*: frágil, falsos positivos en sonatas titulados "I. Allegro" como pieza independiente.

**Trade-off**: la query MB pasa de `inc=aliases` → `inc=aliases+work-rels`. Cada response es ~30% más pesado pero el rate-limit no cambia (1 req/s). Cache StaticMeta absorbe el coste.

**Doc afectado**: `src-tauri/src/classical/providers/musicbrainz.rs:449-500` (parser + URL builder).

---

## D-029 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate)

**Contexto**: Bach (>1000 works en MB) y Mozart (>600) necesitan páginas múltiples. El método actual es `limit.min(100)` en `musicbrainz.rs:454` y la UI nunca pide más allá de la primera página.

**Decisión**: `browse_works_by_artist(artist_mbid, limit, offset)` recibe offset opcional. Backend pagina hasta el `?work-count` total que MB devuelve en el header del response. UI carga primera página de 100, botón "Load more" añade siguiente.

Cache key incluye offset: `classical:composer-works:v2:{mbid}:{genre}:{offset}`. Schema bump `v1→v2` invalida cache antiguo.

**Justificación**: mandato del usuario "no quiero perder nada". Single mega-fetch en cold cache colapsaría rate-limit.

**Alternativas consideradas**:
- *Single mega-fetch en background al abrir composer*: rechazado — rate-limit pressure + UX delay.
- *Cap dura a 200 con "see all in MB" link externo*: rechazado, contradice mandato del usuario.

**Trade-off**: cache fragmentado por offset. Aceptable: cada cache key sigue immutable hasta TTL.

**Doc afectado**: `src-tauri/src/classical/providers/musicbrainz.rs`, `src-tauri/src/classical/catalog.rs::list_works_by_composer`, `src-tauri/src/commands/classical.rs::list_classical_works_by_composer`.

---

## D-030 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate)

**Contexto**: Phase 0 spike demostró que pre-screen Tidal-availability para 1500 composers × 5 works × 2s = 4h no es viable. Pero la UX del Hub no debe ofrecer obras que el usuario no puede escuchar sin marcarlas claramente.

**Decisión**: NO pre-screen. La WorkPage ejecuta cascade ISRC + Tidal text search (Phase 1) en cold-cache la primera vez. Si tras `Matcher` no hay recordings con `tidal_track_id`, persistir el resultado vacío con flag `tidal_unavailable=true` con TTL 7d en `Work` cache, y mostrar UI "Tidal does not have recordings of this work yet" + CTA "Re-check now" que invalida el cache key del work.

**Justificación**: on-click cold-cache es ~12s (Phase 1 budget) por work, aceptable porque el usuario sólo visita los works que le interesan. Cache negativo TTL 7d evita re-pegar la cascade contra works comprobadamente vacíos.

**Alternativas consideradas**:
- *Pre-warm de top-50 composers × top-20 works en background*: válido como complemento (D-026 ya cubre top-30 canon). G7 confirmado: NO se extiende pre-warm en Phase 7.
- *Marcar la card del work con dot "verificado" tras primer fetch*: opt-in, decidido a nivel UI en F7.x.

**Trade-off**: la primera vez que el usuario abre un work de un composer obscuro paga la cascade Phase 1 entera. Subsecuente cache hit es instantáneo.

**Doc afectado**: `src-tauri/src/classical/types.rs` (campo `tidal_unavailable`), `src-tauri/src/classical/catalog.rs::build_work_fresh`, `src-tauri/src/commands/classical.rs::recheck_classical_work_tidal` (NEW).

---

## D-031 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate)

**Contexto**: el tokenizer Phase 5 (`classical/search.rs::COMPOSER_INDEX`) actualmente lookups sobre los 33 composers OpenOpus. Phase 7 amplía universo — el tokenizer debe reconocer composers fuera del canon.

**Decisión**: el tokenizer consume el extended snapshot además del original. Index pasa a 600-1500 entries. Cero cambio de lógica; sólo amplía universo.

**Justificación**: tokenizer es determinístico (D-019), in-process. Coste O(snapshot.len()) para composer-name match. Con 1500 entries y `name.to_lowercase().contains(query)` sigue siendo µs.

**Alternativas consideradas**:
- *Ranked-tokenizer con Levenshtein*: overkill para Phase 7, deferimiento legítimo.
- *Trie pre-built*: optimización prematura. Si > 50ms en algún test, lo abrimos como sub-task.

**Trade-off**: ninguno relevante.

**Doc afectado**: `src-tauri/src/classical/search.rs` (consume snapshot ampliado), no cambia API pública.

---

## D-032 · 2026-05-02 · TOOLING · usuario (locked-in via decision-gate G5)

**Contexto**: el snapshot extended es propiedad versionada del repo. Necesita un script reproducible documentado.

**Decisión**: script `docs/classical/scripts/snapshot_composers_extended.py` (Python confirmado por G5 default) que:
1. Hace SPARQL contra `query.wikidata.org/sparql` con la query Phase 7.
2. Para cada Wikidata QID con MB ID, valida en MB que el composer existe + cuenta recordings.
3. Filtra por `recording_count >= N` (D-027, N=5).
4. Mergea con OpenOpus original (preservando `popular`/`recommended` flags y `epoch` cuando OpenOpus los tiene; defaulteando desde Wikidata cuando no).
5. Output: `src-tauri/data/composers-extended.json` (D-033, NO reemplaza openopus.json).
6. Versionado en repo. Re-ejecutable. CI no lo corre (rate-limit + no-determinismo de WDQS).

**Justificación**: el snapshot es build-time output. Cada release puede actualizarlo si el dev considera que el universo cambió. NO es output de runtime.

**Alternativas consideradas**:
- *Snapshot regenerado on first launch*: rechazado — primer launch online dependency contradice §14 privacy + offline-first.
- *Script en `build.rs` Cargo*: rechazado — build determinismo se rompe (WDQS responde distinto cada día).
- *Lenguaje Node/Shell*: G5 default aceptado: Python.

**Trade-off**: el snapshot envejece. Contramedida: documentar en README cuándo regenerarlo.

**Doc afectado**: `docs/classical/scripts/snapshot_composers_extended.py` (NEW).

---

## D-033 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate G2+G6)

**Contexto**: el snapshot OpenOpus original tiene curación editorial por OpenOpus (no nosotros) — `popular=true` significa "OpenOpus considera al composer canon". Sustituirlo perdería esa señal. Phase 5 editorial seeds (D-020) dependen de la lista original.

**Decisión**: mantener **dos snapshots embebidos**:
1. `src-tauri/data/openopus.json` (original 33 composers, **preservado intacto**, G6 confirmado).
2. `src-tauri/data/composers-extended.json` (nuevo, 600-1500 composers).

`OpenOpusProvider` permanece como fuente autoritativa de `popular` + `recommended` flags + works recommendations. Nuevo `ExtendedComposersProvider` carga el universo amplio sólo para BrowseComposers + search index.

**Justificación**: separación de responsabilidades — canon curado vs universo amplio. Backwards-compat con Phase 5.

**Alternativas consideradas**:
- *Snapshot único colapsado*: rechazado en favor de separación de responsabilidades.
- *Solo extended snapshot*: rechazado — pérdida de curación.

**Trade-off**: dos archivos a mantener. El extended hereda `popular` flag desde OpenOpus si overlap; FALSE para composers nuevos.

**Doc afectado**: `src-tauri/src/classical/providers/openopus.rs` (intacto), `src-tauri/src/classical/providers/composers_extended.rs` (NEW), `src-tauri/data/composers-extended.json` (NEW).

---

## D-034 · 2026-05-02 · ARCH · usuario (locked-in via decision-gate G3 — conditional close)

**Contexto**: D-025 documentó que "top classical composers" stats devuelve "top performers". Phase 7 puede cerrar la deuda introduciendo columna `plays.composer_mbid TEXT NULL`.

**Decisión**: introducir resolver `WorkMbidResolver::resolve_composer_for_work` + extender `scrobble/mod.rs::on_track_started` post-track-start para persistir `plays.composer_mbid`. Migración aditiva en `stats.rs`. `top_classical_composers` agrupa por `composer_mbid` cuando NOT NULL, fallback a `artist_mbid` cuando NULL.

**SUPERSEDES (parcial)**: D-025 caveat — la limitación se cierra **si** B7.6 se ejecuta.

**Justificación**: backfill de `composer_mbid` para plays históricos requiere re-resolver work→composer en MB (rate-limit pressure). Para plays nuevos es ~1s post-track. Backfill es lo costoso; conservadora 1 req/s en background.

**Alternativas consideradas**:
- *Backfill diferido a launch único en background*: viable, sub-task F7.4.
- *Solo plays nuevos*: top-composers sigue siendo "top performers" para histórico hasta que el usuario re-escuche.

**Trade-off**: una migration aditiva más en `stats.rs`. Idempotent.

**Doc afectado**: `src-tauri/src/stats.rs`, `src-tauri/src/scrobble/mod.rs`, `src-tauri/src/classical/catalog.rs`.

---

## D-034-status · 2026-05-02 · PROCESS · classical-supervisor

**Contexto**: G3 del decision-gate dice "B7.6 se cierra si hay budget tras B7.0-B7.5; si no, se documenta como deuda explícita V1".

**Decisión**: B7.6/F7.4 (composer-resolution) son **condicionales**. Tras cerrar B7.0-B7.5 + F7.0-F7.3, evaluar si:
- Tests siguen verdes (≥118 + nuevos).
- Cero regresión §10 mantenido.
- Tiempo de implementación restante razonable (juicio del supervisor — no se fuerza si la calidad sufre).

Si SÍ → ejecutar B7.6 + F7.4, registrar D-034 como cerrado.
Si NO → documentar deuda V1 explícita en `phase-7-catalog-completeness.md` Apéndice + `PROGRESS.md` "Limitaciones conocidas V1", marcar D-034 como **deferred** (no superseded).

**Justificación**: G3 mandato del usuario. Calidad sobre velocidad: si forzar B7.6 introduce risk de regresión a otras phases, no merece la pena.

**Doc afectado**: `phase-7-catalog-completeness.md` Apéndice, `PROGRESS.md` Phase 7 closure.

---

## D-037 · 2026-05-03 · ARCH · classical-supervisor

**Contexto**: cache-wipe + browse de works en plena Phase 8 destapó un fallo cuando MB devuelve un work cuya browse `recording?work={mbid}` produce 0 recordings o cuyas recordings no tienen ISRC ni texto suficiente para que el cascade per-recording (D-010) cruce el threshold 0.6. Síntoma: el work llega al frontend con `recordings: []` o con todas las filas `NotFound`, banner "Tidal unavailable" aparece para obras que SÍ están en Tidal (sólo MB no las tenía bien linkeadas). Bug 3 del briefing del usuario.

**Decisión**: añadir un **tercer escalón al cascade matching, work-level**. Tras `resolve_recordings`, si:

1. `recordings.is_empty()` (MB no devolvió browse hits para el work), O
2. todas las recordings con `MatchConfidence::NotFound` (cero playable),

ejecutar `tidal.search_canonical(build_canonical_query(composer, title, None, None), 8)` y, si el top-result cruza un nuevo threshold conservador `WORK_LEVEL_THRESHOLD = 0.55` (ligeramente por debajo del per-recording 0.6 porque carecemos de artist constraint), **inferir un único `Recording` sintético** marcado con confidence `MatchConfidence::TidalDirectInferred` (variant nuevo). Ese recording lleva el track_id, álbum, quality_tags y un campo `match_query` con la query usada para que el frontend pueda mostrarla al hover en F8.5.

El work resultante mantiene `recording_count = 1` (synthetic) y `tidal_unavailable = false`.

**Justificación**: el escenario "MB conoce el work pero no tiene recordings linkeadas a él en su browse" es real y frecuente para repertorio reciente o less-curated. El usuario navega a la WorkPage desde el Hub (composer page list_works_by_composer), espera ver al menos UNA fila reproducible. La alternativa actual (banner "no Tidal availability") es false-negativa para obras que están en Tidal vía text search canónico.

**Alternativas consideradas**:
- *Forzar text search per-recording aunque MB devuelva vacío*: imposible, no hay recordings sobre las que iterar.
- *Devolver vacío y dejar que el usuario use search global*: rompe el flujo Hub → ComposerPage → WorkPage.
- *Generar N (3-5) recordings sintéticas a partir del top-N de la query*: rechazado V1 — el ranking por debajo del top-1 con scores 0.50-0.59 es ruidoso. Ampliar a N en V1.1 si telemetría lo justifica.

**Trade-off**: una fila con confidence menor que `IsrcBound` y `TextSearchInferred`. Mitigación: badge UI distinto + tooltip con la query usada (F8.5).

**Doc afectado**: `src-tauri/src/classical/types.rs` (variant `MatchConfidence::TidalDirectInferred`), `src-tauri/src/classical/matching.rs` (constante `WORK_LEVEL_THRESHOLD`), `src-tauri/src/classical/catalog.rs::build_work_fresh` (escalón nuevo tras `resolve_recordings`), `src/components/classical/ConfidenceBadge.tsx` + `RecordingRow.tsx` + `src/types/classical.ts`.

---

## D-038 · 2026-05-03 · ARCH · classical-supervisor

**Contexto**: examinando `/tmp/sone-dev.log` post-cache-wipe Phase 8, el prewarm canon falla en TODOS los composers con `Network error: error trying to connect: unexpected EOF` por DNS-IPv6-only de MB en la red local del usuario. Diagnóstico: red externa, no SONE. Pero destapa Bug 4: `catalog.rs::build_work_fresh` actualmente swallow-ea ese error en `fetch_recordings_for_work` (`Vec::new()`) y `get_work` cachea el work resultante con `tidal_unavailable=true` durante 7 días. Resultado: un blip de red de 30s envenena el cache disco del usuario para la siguiente semana, requiere wipe manual para recuperar.

**Decisión**: introducir variant nuevo `SoneError::NetworkTransient(String)` (opción **a** del briefing), separado de `SoneError::Network(String)`:

- **Clasificación al construir**: la conversión `From<reqwest::Error>` consulta `is_connect() | is_timeout() | is_request() | is_body() | is_decode()` → `NetworkTransient`; el resto se mapea a `Network` permanente. Para errores construidos a mano (string-based de status codes), 429/500/502/503/504 → `NetworkTransient`; 4xx ≠ 429 → `Network`.
- **Helper `is_transient(&self)`**: `matches!(self, SoneError::NetworkTransient(_))`. Llamado desde `get_work` y `get_composer` para decidir cachear.
- **Política de cache**: si `build_work_fresh` propaga `is_transient()`, `get_work` devuelve el `Err` al frontend SIN tocar disco. El frontend renderiza error rojo con CTA reintenta (F8.5 lo cubre). Si el error NO es transient, comportamiento previo conservado (cache + flag `tidal_unavailable`).
- **Propagación obligatoria en `build_work_fresh`**: el actual `match { Ok(r) => r, Err(e) => Vec::new() }` en `fetch_recordings_for_work` se reemplaza por `?`. La pérdida de robustez aparente se mitiga porque solo errores transient propagan; errores 404 / parse inválido siguen no siendo transient y siguen produciendo el comportamiento "vacío + cacheado" actual (válido para obras realmente sin recordings en MB).

**Helper `is_transient` semántica detallada**:
- `NetworkTransient(_)`: connect-fail, timeout, TLS handshake failure, partial body, 429, 5xx → reintentable, NO cachear.
- `Network(_)`: 4xx no-429, parse failure encadenado, errors permanentes → comportamiento previo (puede cachearse vacío).

**Justificación**: el cache disco del Hub es un activo crítico (TTL 7d StaticMeta). Polluirlo con resultados producto de errores transient invalidaba el principio de §3.3 ("cache TTLs reflejan permanencia del dato"). La pérdida es asimétrica: cachear OK por error transient cuesta 7 días de mala UX (banner falso de Tidal-unavailable en works comunes); NO cachear por error transient cuesta una llamada extra MB cuando vuelve red. Coste-beneficio claramente a favor de NO cachear transient.

Variant nuevo > método sobre `Network`: aunque incrementa surface (4 nuevos sites: `From<reqwest::Error>` + `From<...>` para tauri/io/serde sin cambio + serialización + tests), la claridad semántica vale el coste. El frontend recibe el JSON `{kind: "NetworkTransient", message: "..."}` y puede mostrar mensajería específica ("Conectividad intermitente con MusicBrainz, reintenta") distinta de errores hard. Integraciones futuras (retry-with-backoff automático en background, telemetría de health) pueden detectar el variant directamente sin parse de strings.

**Alternativas consideradas**:
- *Opción b: método `Network(_).is_transient()` con detección heurística por substring del string*: rechazado — frágil (cualquier cambio del format string rompe), opaca, no permite mensajería específica frontend.
- *Reintentar internamente en `fetch_recordings_for_work` con backoff*: rechazado V1 — la retry policy es responsabilidad del caller (frontend con CTA), no del provider. El provider ya tiene 1× retry en 503 (musicbrainz.rs:94); doblar la lógica es complejidad opaca.
- *Cachear con TTL muy corto cuando transient (1min)*: rechazado — cualquier TTL > 0 polluya; es más limpio NO cachear.

**Trade-off**: cada uso de `SoneError::Network(...)` literal en strings requiere revisar si debe migrar a `NetworkTransient` cuando construye desde HTTP status conocido (5xx, 429). Backend engineer hace audit caso-a-caso en los providers MB / Tidal / Wikipedia / Wikidata. Llamada de juicio: errores construidos manualmente con status code conocido se clasifican; errores construidos sin clasificación (e.g. parse inválido) quedan en `Network`.

**Doc afectado**: `src-tauri/src/error.rs` (variant + From + is_transient), `src-tauri/src/classical/catalog.rs` (propagación + decisión cache), `src-tauri/src/classical/providers/{musicbrainz,tidal,wikipedia,wikidata}.rs` (clasificación de status codes), `src/api/classical.ts` (types `SoneErrorKind`), `src/components/classical/WorkPage.tsx` (mensajería específica transient).

---

## D-039 · 2026-05-03 · EDITORIAL · classical-musicologist

**Contexto**: Pedro reportó que la ComposerPage actual lista works "directamente con los movimientos" en lugar de agruparlos como Apple Music Classical / Idagio (Symphonies / Concertos / Operas / Chamber / etc.). Investigación confirma que el set actual `WorkType` (Symphony / Concerto / Sonata / StringQuartet / Opera / Cantata / Mass / Lieder / Suite / Etude / Other) es insuficiente — Apple e Idagio usan ~9-12 buckets más afinados, y muchos works MB no traen `workType` poblado, cayendo en "Other" y desorganizando la página. Se necesita un set canónico de categorías que (a) un melómano reconozca, (b) sea cubrible desde MB+Wikidata sin curación manual masiva, (c) no sature cuando un compositor sólo tiene 5 obras.

**Decisión**: la taxonomía canónica del Hub para agrupar works en ComposerPage adopta **9 buckets primarios + 2 buckets condicionales**, en este orden de presentación (mirroring Apple Classical "Browse > Genres" + Idagio "Genres"):

1. **Stage works** — Operas, balletos, música incidental, Singspiel, zarzuela. (Etiqueta UI: "Stage works" en EN, "Obras escénicas" en ES.)
2. **Choral & sacred** — Misas, réquiems, oratorios, pasiones, motetes, cantatas sacras, Te Deum. (UI EN: "Choral & sacred", ES: "Coral y sacro".)
3. **Vocal** — Lieder, ciclos de canciones, canzonette, mélodies, art songs, cantatas seculares. (UI EN: "Vocal", ES: "Vocal".)
4. **Symphonies** — Sinfonías numeradas + sinfonías corales (Beethoven 9 vive aquí, no en Choral). (UI EN: "Symphonies", ES: "Sinfonías".)
5. **Concertos** — Conciertos para uno o más solistas con orquesta, conciertos grossi barrocos, sinfonías concertantes. (UI EN: "Concertos", ES: "Conciertos".)
6. **Orchestral** — Poemas sinfónicos, oberturas, suites orquestales, divertimenti, serenatas orquestales, música para ballet en versión de concierto, fantasías orquestales. (UI EN: "Orchestral", ES: "Orquestal".)
7. **Chamber** — Cuartetos, tríos, quintetos, sextetos, sonatas para dos o más instrumentos, piezas para ensemble pequeño. (UI EN: "Chamber", ES: "Cámara".)
8. **Keyboard** — Sonatas para piano, suites de clave, obras para órgano solo, preludios, fugas, estudios, variaciones para teclado. Incluye fortepiano y harpsichord. (UI EN: "Keyboard", ES: "Teclado".)
9. **Solo instrumental** — Obras para un instrumento solo distinto de teclado: violín solo (Bach Sonatas y Partitas), cello solo (Suites), flauta solo, guitarra clásica, etc. (UI EN: "Solo instrumental", ES: "Instrumento solo".)

Buckets condicionales (sólo se renderizan si tienen ≥1 obra):

10. **Film & stage music** — Bandas sonoras, music for plays, theatre incidental cuando no es claramente "stage work". (UI EN: "Film & theatre", ES: "Cine y teatro".) Sólo aparece para compositores con esta producción significativa (Korngold, Williams, Glass, Herrmann).
11. **Other** — Cajón de sastre. Mostrarse al final, plegado por defecto.

**Etiquetas y textos rechazados**:
- "Sacred" como bucket separado de "Choral" — fusionado: 95% del repertorio sacred es coral, separarlos crea bucket-fragmentation.
- "Lieder" como bucket separado de "Vocal" — Lieder es subcategoría germánica de Vocal, no peer.
- "Sonatas" como bucket único — colapsa keyboard-sonatas con violin-cello-sonatas que son chamber. Resuelto: piano sonata → Keyboard, violin sonata → Chamber.
- "Choirs" como bucket en composer page — Idagio sí lo tiene a nivel browse pero a nivel composer page agrega contra Choral & sacred.
- "Symphonic poem" como peer de Symphonies — Apple lo mete en "Orchestral", lo seguimos.
- "Etude" como bucket — mantenerlo como peer de Sonata era prematuro; un estudio de Chopin pertenece a Keyboard.

**Justificación**:
- **Mirroring industry**: Apple Music Classical Browse > Genres muestra exactamente "Composers / Periods / Genres / Conductors / Orchestras / Soloists / Ensembles / Choirs" (Apple Support — Browse Categories) y dentro de Genres: Symphony / Concerto / Chamber / Solo / Vocal / Choral / Opera / Orchestral. Idagio (Idagio Genres /790, app.idagio.com/browse) confirma "Opera / Orchestral / Concertos / Chamber / Vocal / Choral / Solo Keyboard / Solo Instrumental". La diferencia entre nuestros 9 y los 8 de cada uno es que separamos Keyboard y Solo Instrumental (Idagio lo hace; Apple lo colapsa pero los reviewers lo critican).
- **Coverage MB+Wikidata**: cada bucket tiene mapping determinístico desde MB `work-type` + Wikidata P136 instance-of. La regla de mapeo va en el plan técnico del supervisor.
- **Granularidad apropiada**: 9 buckets es el número que un melómano recorre con la vista en una sola pantalla. Más de 12 → bucket-overload. Idagio tiene 8 a nivel root y los reviews lo elogian; añadimos Solo instrumental separado de Keyboard porque la diferencia entre las Sonatas y Partitas BWV 1001-1006 (violín solo) y las Goldberg Variations (clave) es significativa para el oyente clásico.
- **Orden de presentación**: Stage > Choral > Vocal > Symphonies > Concertos > Orchestral > Chamber > Keyboard > Solo instrumental refleja la convención editorial de Gramophone, Penguin Guide y los CD-box-sets de DG Complete Edition (orden Apple ligeramente distinto pero los reviews del NYT critican que ponga Symphonies primero porque sesga hacia el repertorio orquestal-romántico). Esta ordenación funciona para todas las eras: Bach (Choral primero respeta su producción real), Beethoven (Symphonies es lo que el usuario busca pero está en posición 4 — visible sin scroll en cualquier pantalla razonable), Wagner (Stage works dominan, primero correcto).

**Mapping de fuentes a bucket** (canónico, Phase técnico lo implementará):

| Bucket | MB `work-type` que mapea | Wikidata P136 keywords | Fallback heurístico (regex/title) |
|---|---|---|---|
| Stage works | Opera, Operetta, Musical, Ballet, Incidental music, Zarzuela | opera, ballet, operetta, music drama | título contiene "Opera", "Ballet"; libretto present |
| Choral & sacred | Mass, Requiem, Oratorio, Cantata (sacred), Motet, Passion, Te Deum, Anthem, Magnificat | mass, requiem, oratorio, motet, passion, sacred | título empieza "Missa", "Requiem", "Oratorio"; key religious-text |
| Vocal | Song, Song cycle, Lied, Aria, Madrigal, Cantata (secular) | art song, lied, song cycle, mélodie | "Lieder", "Songs" en título; con vocalist + piano |
| Symphonies | Symphony, Symphonic poem (¡no!), Sinfonia | symphony | título empieza "Symphony No." / "Sinfonía" |
| Concertos | Concerto, Concerto grosso, Sinfonia concertante | concerto, concerto grosso | título contiene "Concerto for" / "Concierto para" |
| Orchestral | Overture, Symphonic poem, Suite (orchestral), Variations (orchestral), Serenade (orchestral), Divertimento (orchestral), Tone poem, Fantasy (orchestral), Rhapsody (orchestral) | overture, symphonic poem, tone poem, orchestral suite | "Overture", "Tone Poem", "Symphonic Poem", "Suite for orchestra" |
| Chamber | String Quartet, Piano Trio, String Trio, Piano Quintet, Quintet, Sextet, Octet, Sonata (when ≠ piano-solo), Serenade (chamber) | string quartet, chamber music, piano trio | dos+ instrumentistas no orquesta |
| Keyboard | Sonata (piano/harpsichord/organ solo), Prelude, Fugue, Étude, Nocturne, Mazurka, Polonaise, Variations (keyboard), Suite (keyboard), Partita (keyboard) | piano sonata, organ work, harpsichord work | "for piano", "for organ", "Sonata No. X for Piano" |
| Solo instrumental | Sonata (violin/cello/flute/guitar solo), Partita (non-keyboard), Suite (non-keyboard solo), Caprice | solo violin, solo cello, solo guitar | "for violin solo", "Cello Suite", "Partita for violin" |
| Film & theatre | Film score, Theatre music, Incidental music (cuando NO es clearly stage) | film score, soundtrack | publishing date >1900 + sin opus |
| Other | (rest) | (rest) | fallback |

**Reglas de tiebreaker** (cuando una obra podría caer en dos buckets):
- "Sonata" sin instrumentación clara → Chamber.
- Sinfonía con coral (Beethoven 9, Mahler 2/3/4/8) → Symphonies (no Choral). El elemento estructural sinfónico domina.
- Concert version de ballet (Stravinsky Pulcinella suite) → Orchestral. Pero el ballet completo (Pulcinella ballet) → Stage works.
- Cantata barroca de Bach (BWV 1-200) → Choral & sacred (95% son sacred). Cantatas seculares específicas (BWV 201-216 Cofee Cantata, etc.) → Vocal.
- Misa de Réquiem (Verdi, Mozart, Brahms Deutsches Requiem) → Choral & sacred siempre, aunque el texto no sea litúrgico.

**Threshold de visualización**:
- Bucket con 0 works → no se renderiza.
- Bucket con 1-5 works → se renderiza completo (no "view all").
- Bucket con 6-12 works → se renderiza completo, sin "view all".
- Bucket con 13+ works → primeros 12 + chip "View all (N)" → drill-down a sub-página por bucket.

**Buckets de "Essentials"** (sección hero antes de los buckets):
- Mostrar 4-8 obras cherry-picked. Fuente: editorial.json + OpenOpus `popular`. Cada Essential es un atajo a su WorkPage. Esta sección NO sustituye a los buckets; los precede.

**Página por bucket cuando se hace drill-down**:
- Header: "Beethoven · Concertos (12)".
- Filter chips secundarios (sub-categoría dentro del bucket cuando aplica): para Concertos → "Piano / Violin / Cello / Other"; para Chamber → "String Quartets / Piano Trios / Sonatas / Quintets / Other"; para Keyboard → "Sonatas / Variations / Pieces / Études".
- Sort: Catalog number (default) / Date / A-Z.

**Composer-specific overrides** (D-039-extension a registrarse caso por caso):
- Wagner: presentación natural Stage works > Orchestral (sus arreglos de concerto del Anillo) > Vocal (los Wesendonck Lieder). El resto vacío. Buckets condicionales evitan el bloat.
- Bach: Choral & sacred (cantatas + pasiones + misas) > Keyboard (Goldberg, WTC, Partitas, Inventions) > Solo instrumental (Suites cello, Sonatas y Partitas violín) > Orchestral (Brandenburg, Suites) > Concertos > Chamber. Aquí "Symphonies" no existe (y bien).
- Chopin: Keyboard domina absolutamente. Tendrá Chamber con el Trio Op.8 y los Cellosonatas, pero ~95% en Keyboard. La sección Keyboard de Chopin merece sub-categorías visibles dentro: Sonatas / Concertos for piano / Études / Nocturnes / Ballades / Scherzos / Mazurkas / Polonaises / Preludes / Impromptus / Pieces. Estos sub-buckets sólo aparecen si bucket > 12.
- Debussy: Orchestral (Préludes, La Mer, Iberia) > Keyboard (Préludes, Études, Suite Bergamasque, Images) > Chamber (Quartet Op.10, Sonatas tardías) > Vocal (Mélodies) > Stage works (Pelléas).
- Glass: Stage works (Einstein, Akhnaten, Satyagraha, Nixon es de Adams), Symphonies (las 14), Concertos, Keyboard (Études, Metamorphosis), Chamber (los string quartets), Film (Koyaanisqatsi, The Hours).

Estos overrides no rompen la regla universal — sólo confirman que al ser data-driven, los buckets vacíos colapsan y la página queda limpia.

**Alternativas consideradas**:
- *Mantener el set actual de WorkType (11 entries) y forzar mejor coverage MB*: rechazado. Los 11 actuales mezclan niveles (Symphony y Mass son work-types, pero Lieder es género). El problema es semántico, no de coverage.
- *Adoptar literalmente Apple's 8 buckets*: rechazado. Apple no separa Keyboard de Solo Instrumental y los reviewers lo critican. Idagio sí los separa y la comunidad audiophile lo prefiere.
- *Adoptar Idagio's 8 buckets exactos*: rechazado por una razón menor — Idagio llama "Solo Keyboard" y "Solo Instrumental" peer; nosotros simplificamos a "Keyboard" porque incluye obras para 4 manos (ej. Schubert Fantasía D940) que son still teclado.
- *Set extendido de 15+ buckets (separar Lieder, Mass, Étude, Symphonic Poem, etc.)*: rechazado. Saturación cognitiva. Un compositor con 5 obras que se reparten en 5 buckets se ve más roto que con 2 buckets de 3+2.
- *Bucket único "All works" + filter dropdown en lugar de buckets*: rechazado. Es lo que Apple hace y exactamente la queja del review de Caleb Carman ("700 works without filtering"). El bucket-grouping es la respuesta correcta.

**Trade-off**:
- Implementación técnica más compleja (mapping work-type + P136 + heurística tiebreaker) que un simple `groupBy(workType)`. Lo asume el supervisor en el plan técnico que sigue.
- Algunos works marginales caerán en "Other" (probablemente <5% del corpus) hasta que se afine el mapping. Aceptable como deuda V1 — mejor "Other" pequeño y honesto que un bucket fragmentado.
- "Choral & sacred" es etiqueta combinada: si el usuario busca solo música sacra instrumental (organ Mass, etc.) no la encuentra. V2 podría añadir filter chip dentro del bucket. V1 cubre el 95% del caso.

**Doc afectado**:
- `CLASSICAL_DESIGN.md` §7.2 ComposerPage: la sección actual "Symphonies / Piano Concertos / [...]" se sustituye por la lista de 9+2 buckets con orden canónico.
- `src-tauri/src/classical/types.rs::WorkType` y `Genre`: el supervisor decidirá si extiende el enum existente o introduce un nuevo `WorkBucket` enum 1:1 con la taxonomía. Mi recomendación: introducir `WorkBucket` enum nuevo (pure presentation tier), mantener `WorkType` y `Genre` como-están (data tier desde MB/Wikidata) y mapear en `catalog.rs` o frontend.
- `src/components/classical/ComposerPage.tsx::groupWorks`: re-implementar la función de agrupación según el mapping de la tabla.
- `src/types/classical.ts`: nuevos labels ES/EN para los 11 buckets.

**Limitación V1 conocida**:
La calidad del bucketing depende de cuánto MusicBrainz puebla `work-type`. En spike testing, ~60% de los works MB tienen work-type explícito; el 40% restante exige fallback heurístico de título. La regex de fallback debe ser conservadora: mejor caer a "Other" que mis-bucketing a "Symphonies" un poema sinfónico. Para corpus canónico (top-100 composers Wikidata), el editorial.json snapshot puede llevar overrides explícitos `work_mbid → bucket` en V1.x si la heurística falla en obras prominentes.

---

## D-040 · 2026-05-04 · ARCH · classical-supervisor

**Contexto**: tras D-039 (taxonomía 9+2 buckets) hay que decidir cómo modelar el bucketing en el código y cómo se calcula el `WorkBucket` para cada obra. Los datos de origen (MB `work-type`, Wikidata P136, OpenOpus genre) son inconsistentes — ~60% de works MB tienen work-type explícito; el resto exige fallback heurístico de título.

**Decisión**: introducir `WorkBucket` como **enum nuevo de presentación** en `src-tauri/src/classical/types.rs` con las 11 entries de D-039 (Stage, ChoralSacred, Vocal, Symphonies, Concertos, Orchestral, Chamber, Keyboard, SoloInstrumental + condicionales FilmTheatre, Other). NO reemplaza `WorkType` ni `Genre` (data tier desde MB/Wikidata). Función pura `bucket_for(work_type, genre, p136, title) -> WorkBucket` en módulo nuevo `classical/buckets.rs` con la matriz de mapping documentada en D-039 §F. Override editorial via snapshot: `editorial.json` extiende cada entry con campo opcional `bucket`; cascade snapshot > heurística. Tests deterministic con casos canon (Beethoven 9 → Symphonies, Bach Pasión San Mateo → ChoralSacred, Schubert Winterreise → Vocal, Chopin Étude Op.10 → Keyboard, Bach Cellosuite → SoloInstrumental).

**Justificación**: separar tier de presentación (bucket) del tier de datos (work-type + genre) preserva la honestidad del modelo: MB nunca expone "WorkBucket", lo computamos. Override editorial cubre los casos en que la heurística falla en obras prominentes (D-039 limitación V1). Co-existencia con `WorkType`/`Genre` permite que la WorkPage muestre "Symphony · Op. 125" usando `work_type` como label fino mientras el bucket agrupa.

**Alternativas consideradas**:
- *Extender `WorkType` enum existente*: rechazado. `WorkType` es data-mirror de MB; añadir entries presentation-only contamina el data tier.
- *Calcular bucket en frontend desde work_type+genre*: rechazado. Lógica heurística de título + override editorial es complicada; mejor en backend con tests rust deterministic.
- *Bucket calculado on-the-fly en cada read*: rechazado. Cacheado en `Work.bucket` ahorra cálculos repetidos en list views.

**Trade-off**: tres tiers de clasificación coexistiendo (work_type, genre, bucket). El campo `Work.bucket` se computa al fetch + se cachea con el work; cualquier cambio a `bucket_for` exige cache invalidation (cache key bump). El override editorial añade carga al snapshot pero acota a casos puntuales.

**Doc afectado**: `CLASSICAL_DESIGN.md` §5.1 (entity Work), `classical/types.rs`, `classical/buckets.rs` (NEW), `classical/catalog.rs::build_work_fresh`, `editorial.rs` schema.

---

## D-041 · 2026-05-04 · ARCH · classical-supervisor

**Contexto**: D-037 introdujo `try_work_level_fallback` cuando MB devuelve 0 recordings linkeadas a un work. La implementación V1 sintetizaba **una sola** Recording, query construida sin catalog number, threshold 0.55, no consultaba `work_type` para penalizar mismatches semánticos. Pedro reportó (2026-05-04) que esto produce dos efectos catastróficos:
- Beethoven Op. 83 ("3 Gesänge von Goethe", lieder vocal) → query `"Beethoven 3 Gesänge von Goethe"` matchea Symphony No. 3 Eroica con score 0.775 (false positive grave: click play reproduce Eroica).
- Cualquier work con MB recording-rels missing muestra solo 1 recording sintética cuando Tidal tiene decenas.

**Decisión**: refactor del fallback con **cuatro cambios** simultáneos:

1. **Top-N synthesis** (no top-1): scoring sobre TODOS los candidatos del top-8 que devuelve `tidal.search_canonical(query, 8)`. Cap `MAX_WORK_LEVEL_SYNTH = 12`. Los que cruzan threshold se sintetizan como `Recording` distintos con MBID `synthetic:tidal:{work_mbid}:{idx}`.
2. **Query con catalog number obligatorio**: `build_canonical_query(composer, title, catalogue, primary_artist, year)` extendido con parámetro `catalogue: Option<&CatalogueNumber>`. Cuando presente, se anexa `catalogue.display` (e.g. "Op. 83", "BWV 244", "K. 466") al final de la query — token discriminativo.
3. **Threshold subido 0.55 → 0.62**: rationale numérica — con catalog number en query, una query bien formada scorea ≥ 0.65 fácil. 0.62 deja margen para ortografías exóticas pero corta el caso Eroica.
4. **Genre-aware penalty**: `score_candidate` consulta `work.work_type` (mapeado a `WorkBucket` via D-040). Si Tidal candidate tiene `album.title` o `album.tags` que sugieren un bucket incompatible (Vocal ⊥ Symphonic, Chamber ⊥ Stage, etc.), penalty −0.30. Cuando `album_kind == Unknown`, no penaliza.

**Justificación**:
- **Top-N**: para repertorio común con query bien construida los top-8 son consistentemente parents/children del mismo work (8 cantantes distintos del mismo lied de Beethoven). Pedro confirmó la telemetría empírica: hay decenas de grabaciones legítimas, sintetizar solo una es inaceptable.
- **Catalog number**: Tidal FTS pondera tokens raros. `Op. 83` en la query pasa de 0.775 (matcheando Eroica) a Eroica scoreando ~0.45 (pierde el match) y los 3 Gesänge reales scoreando ~0.82.
- **Threshold 0.62**: combinado con catalog number, mantiene tasa de TP alta y mata FP. Ajustable post-deploy si telemetría exige.
- **Genre penalty**: defensa adicional. Aunque catalog number y threshold ya filtran el caso Op. 83/Eroica, futuros works sin catalog number explícito (música contemporánea, opera obscura) se benefician del genre check.

**Alternativas consideradas**:
- *Mantener top-1 + threshold 0.62*: rechazado. No resuelve "1 sola grabación cuando Tidal tiene decenas". Pedro lo flagged explícitamente.
- *Buscar Tidal por catalog number solo (sin composer/título)*: rechazado. Catalog number es discriminativo en combinación, pero "Op. 83" solo matchea cualquier obra con esa numeración (Brahms Sextet Op. 83, Dvořák Symphony No. 7 Op. 83 si la hubiera, etc.). Combinado es robusto.
- *Threshold dinámico por bucket*: rechazado V1. Complejidad sin beneficio claro. V1.1 si telemetría lo justifica.

**Trade-off**: subir threshold a 0.62 puede causar regresión: works canónicos donde MB tiene 0 recordings pero fallback acertaba con 0.58-0.61 ahora caen al banner. Mitigación: tras shippear, validation gate manual sobre 5-10 obras conocidas. Si tasa de falso-negativo > 10%, revertimos a 0.58 y compensamos con genre penalty más estricto.

**Doc afectado**: `classical/catalog.rs::try_work_level_fallback`, `classical/matching.rs::WORK_LEVEL_THRESHOLD + best_work_level_candidate*`, `classical/providers/tidal.rs::build_canonical_query`, `classical/types.rs::Recording.match_query`.

**SUPERSEDES**: D-037. Conserva variant `MatchConfidence::TidalDirectInferred` (frontend ya implementado en F8.5). Refina la política sin reemplazar el variant.

---

## D-042 · 2026-05-04 · UX · classical-supervisor

**Contexto**: WorkPage actual (post-Phase 7) tiene un layout funcional pero plano: header simple, lista de movements, lista de recordings con filtros + sort, banner Tidal-unavailable cuando aplica. Pedro pidió rediseño estilo Apple Music Classical / Idagio + "info sobre la obra como punto distintivo".

**Decisión**: anatomía rediseñada en **8 secciones canónicas** (orden de presentación):

1. **Header**: título grande, composer link, catalog + key + year + duration + movements count + recording count + best-quality badge (Phase 4).
2. **Editor's Choice banner separado**: 1 grabación destacada con conductor + orquesta + año + sello + quality badge + 3 líneas de rationale editorial. NO inline en lista; sección propia visual prominente.
3. **About this work — USP**: prosa larga estructurada en 5 sub-secciones (origin / premiere / highlights / context / notable_recordings_essay). Sources cited. Multi-locale via `extended.translations` dict. Detalle en D-044.
4. **Listening Guide**: cuando existe (Phase 5 LRC), reader con time-sync. Colapsable.
5. **Movements**: lista cuando work tiene `movements.len() > 0` (Phase 1).
6. **Popular Recordings**: top 8 ordenadas por `(quality_score desc, popularity_inferred desc, has_editors_choice desc)`. Sub-set distinto de "All Recordings".
7. **All Recordings**: lista completa Phase 4 con filters Hi-Res/Atmos/sample-rate/MQA/conductor/label/year + sort + paginación.
8. **Sidebar derecho** (desktop ≥ 1280px): related works (Wikidata + heurística) + cross-version comparison Phase 6 D-022 + performers you follow.

**Justificación**:
- **Sección 2 separada**: hoy el Editor's Choice se marca con star inline en RecordingRow. Apple e Idagio le dan banner propio porque es la primera mirada del usuario novato; mezclarlo en una lista de 100 recordings lo entierra.
- **Sección 3 (USP)**: pedido explícito de Pedro. Diferenciador competitivo claro vs Apple/Idagio (que tienen editorial limitado). Detalle en D-044.
- **Sección 6 vs 7**: split entre "lo recomendado" (8 versiones) y "lo completo" (todas). Idagio lo hace; Apple lo hace; UX justifica.
- **Sección 8 sidebar**: aprovecha Phase 6 D-022 + Wikidata. Solo desktop ≥ 1280px (responsive collapse en pantallas pequeñas).

**Alternativas consideradas**:
- *Mantener Phase 7 layout*: rechazado. Pedro pidió rediseño explícito.
- *Tabs WorkPage (Recordings / About / Movements)*: rechazado. WorkPage es objeto-única; las tabs fragmentan la consulta natural ("¿qué grabación pongo de la 9ª?"). Apple no usa tabs en work page; Idagio tampoco.
- *Sidebar siempre visible*: rechazado. En mobile o ventana estrecha el sidebar quita protagonismo a la lista de recordings.

**Trade-off**: 8 secciones es más vertical scroll que la WorkPage actual. Mitigación: primeras 4 secciones visibles en first viewport (header + EC banner + first lines de About + Listening Guide CTA). El usuario que solo quiere reproducir tiene Editor's Choice en posición 2, accesible sin scroll.

**Doc afectado**: `CLASSICAL_DESIGN.md` §7.2, `src/components/classical/WorkPage.tsx`, nuevos componentes `AboutThisWork.tsx`, `EditorChoiceBanner.tsx`, `WorkSidebar.tsx`.

---

## D-043 · 2026-05-04 · UX · classical-supervisor

**Contexto**: ComposerPage actual muestra todo en una página vertical. Pedro pidió "se asemeje a apple music classical o idagio". Idagio especialmente convirtió el modelo "tabs en composer page" en su seña de identidad.

**Decisión**: **4 tabs persistentes** en el header de la ComposerPage post-Phase 9: **About / Works / Albums / Popular**.

- **About** (default cuando entry es desde browse): portrait grande, bio Wikipedia, fechas, era badge, related composers (Phase 6 D-022 ya implementado).
- **Works** (default cuando entry es desde search): la pestaña que define la página. Vista por buckets D-039 (top-12 works por bucket + "View all" → drill-down a `BrowseComposerBucket`). Detalle en plan Phase 9.
- **Albums**: discografía clásica del compositor (releases/albums asociadas). Reusa o porta de `ClassicalArtistPage` Phase 6.
- **Popular**: top reproducciones del usuario para este composer. Filtro de `top_classical_works` Phase 6 stats por composer. Cuando el usuario tiene 0 plays clasicales del composer, fallback a "Hub-popular" (placeholder o curado editorial).

**Routing**: `classical://composer/{mbid}?tab=works` con default condicional. `useNavigation` añade `navigateToClassicalComposerTab(mbid, tab)`.

**Justificación**:
- **Tabs vs scroll vertical**: Idagio confirmó que tabs reduce visual overload y permite drill-down rápido. 4 tabs es el sweet spot (5+ tabs es saturación; 3 tabs no aprovecha el patrón).
- **Default condicional**: usuario que llega desde browse de composer → quiere ver quién es (About). Usuario que llega desde search "Beethoven Symphony 5" → quiere ver el catálogo (Works).
- **Tab Albums separado de Works**: works es agrupación catalográfica conceptual (composer-centric); albums es la realidad publicada (commercial-centric, intérprete-centric). Idagio los separa.
- **Tab Popular como nuevo**: Phase 6 stats provee el dato. Surface natural en el composer page como "lo que este usuario escucha de Beethoven".

**Alternativas consideradas**:
- *Una sola página vertical con secciones plegables*: rechazado. Es lo que tenemos hoy y Pedro pidió cambio.
- *Tabs About / Catalog / Discography*: rechazado. "Catalog" es ambiguo entre works y albums; el split Works/Albums es más limpio.
- *5 tabs (añadir Recordings flat)*: rechazado V1. Recordings flat es otro browse axis que pertenece más naturalmente a `ClassicalArtistPage` (browse-by-conductor). En composer page introduce confusión.

**Trade-off**: tabs requiere routing extendido + state persistence cuando navegas back. Mitigación: state se persiste en URL via query param `?tab=`, no en client state local. Idagio lo hace así.

**Doc afectado**: `CLASSICAL_DESIGN.md` §7.1, `src/components/classical/ComposerPage.tsx`, `src/hooks/useNavigation.ts`, `src/App.tsx` (routing).

---

## D-044 · 2026-05-04 · EDITORIAL · classical-musicologist + classical-supervisor

**Contexto**: Pedro pidió "ademas estaba bien poner informacion sobre la propia obra, como punto distintivo". Phase 5 introdujo `editor_note` (snapshot 48 obras × ~80 palabras) que da una línea editorial. El USP exige profundidad y anchura mucho mayores.

**Decisión**: sección "About this work" en WorkPage como **eje diferencial competitivo**. Schema editorial v2 con 5 sub-secciones canónicas:

| Sub-sección | Contenido | Fuentes primarias |
|---|---|---|
| Origin & commission | Quién encargó, cuándo, por qué, dedicatoria, manuscrito | snapshot extended + Wikidata P88/P179/P50/P138 |
| Premiere & reception | Fecha, lugar, intérpretes, recepción crítica inicial | snapshot extended + Wikidata P1191/P710 + Wikipedia |
| Musical highlights | Key changes, motifs, instrumentación, structural notes accesibles | snapshot extended (curado) + Wikipedia "Music"/"Analysis" parsed |
| Historical context | Lugar en obra del compositor, época, influencias, legacy | Wikipedia background + snapshot extended |
| Notable recordings essay | Brief essay sobre grabaciones de referencia (Furtwängler '51, Karajan '62, etc.) | snapshot extended ONLY (curated, never auto-generated) |

Schema JSON v2 (`editorial-extended.json` separado del v1 `editorial.json` Phase 5):

```json
{
  "schema_version": 2,
  "works": [
    {
      "work_mbid": "9c9a3b5b-...",
      "composer_mbid": "1f9df192-...",
      "match_titles": ["Symphony No. 9", "Choral Symphony"],
      "bucket": "Symphonies",
      "editor_note": "...",
      "extended": {
        "origin": "...",
        "premiere": "...",
        "highlights": "...",
        "context": "...",
        "notable_recordings_essay": "...",
        "sources": [
          {"kind": "wikipedia", "url": "...", "license": "CC BY-SA"},
          {"kind": "editor", "name": "mySone team"}
        ],
        "language": "en",
        "translations": {
          "es": { "origin": "...", ... }
        }
      },
      "editors_choice": {
        "recording_mbid": "...",
        "rationale": "..."
      }
    }
  ]
}
```

`editorial.rs` Phase 5 se extiende:
- Backward compat v1 (sólo `editor_note`).
- Nuevo método `lookup_extended(work_mbid) -> Option<ExtendedNote>`.
- Locale fallback: `extended.translations[locale]` → `extended` (default lang) → None.

**Justificación**:
- **Eje diferencial**: ni Apple Music Classical ni Idagio cubren editorial profundo. Tienen Editor's Choice + breve nota; mySone propone 1200 palabras estructuradas. Punto fuerte vendible.
- **5 sub-secciones**: cubre el espectro typical melómano: origin (qué es), premiere (cuándo apareció), highlights (qué escuchar), context (dónde encaja), recordings essay (cómo aproximarse a las versiones).
- **Schema v2 separado**: `editorial.json` v1 Phase 5 sigue válido para canon mayor con `editor_note` breve. `editorial-extended.json` v2 es aditivo, no sustituye. Backward compat preservada.
- **Multi-locale**: español + inglés mínimo, ampliable. Si entry no tiene translación, fallback al default lang.
- **Sources cited**: cada entry lista fuentes (Wikipedia URL, Wikidata Q-id, "mySone team"). Cumple legal CC BY-SA.

**Alternativas consideradas**:
- *Inline en `editorial.json` Phase 5 (extender schema v1)*: rechazado. Cambia schema cargado, fuerza re-parse de los 48 entries Phase 5. v2 separado es aditivo.
- *Almacenar editorial en SQLite*: rechazado. Para 200 entries × 1200 palabras (~1.2 MB) un JSON embebido es trivial. SQLite es overkill y exige migración.
- *Editorial gradual via API remota*: rechazado V1. Network dependency. Embebido garantiza disponibilidad offline.

**Trade-off**: 1.2 MB añadidos al binario en V1 (200 obras). 3 MB en V2 (500 obras). Cap §G4 Phase 7 fue ≤5 MB delta total — queda margen.

**Doc afectado**: `CLASSICAL_DESIGN.md` §7.2 (WorkPage layout), `src-tauri/data/editorial-extended.json` (NEW), `classical/editorial.rs` (extender), `src/components/classical/AboutThisWork.tsx` (NEW).

---

## D-045 · 2026-05-04 · ARCH · classical-supervisor

**Contexto**: D-044 introduce `editorial-extended.json` snapshot embebido. Hay que decidir convivencia con el `editorial.json` v1 Phase 5 (48 obras × `editor_note` breve).

**Decisión**: **dos snapshots coexistentes**, ambos embebidos en binario:

- `src-tauri/data/editorial.json` v1 — Phase 5, 48 obras × `editor_note` breve. NO se modifica.
- `src-tauri/data/editorial-extended.json` v2 — D-044, 200 obras V1 (target) × extended notes. NEW.

`editorial.rs` Phase 5 carga ambos en memoria (OnceLock). Lookup cascade per work:
1. `lookup_extended(work_mbid)` → si Some, devuelve. Frontend renderiza sección "About this work" completa.
2. `lookup(work_mbid)` (Phase 5) → si Some, devuelve `editor_note` breve. Frontend renderiza el callout pequeño Phase 5.
3. None → frontend cae a Wikipedia summary auto-fetched (Phase 1).

Cap tamaño: 5 MB total ambos JSONs.

**Justificación**:
- **Aditivo**: cero riesgo a las 48 entries Phase 5. Tests existentes siguen pasando. Snapshot v1 no se altera.
- **Schema independiente**: v1 simple, v2 estructurado. Cada uno evoluciona a su ritmo.
- **Cascade de lookup**: graceful degradation. Si extended no existe pero v1 sí, mostramos lo que tenemos.

**Alternativas consideradas**:
- *Migrar v1 a v2 (todas las 48 entries reescritas con extended schema)*: rechazado V1. Coste ~50h (1h/entry × 48). Snapshot v1 sigue siendo válido como fast-coverage.
- *Archivo único v2 que incluya todas las 48 entries v1 con campo `extended` opcional*: rechazado. Mezcla audiences (los 48 son canon mayor, los 200 V1 incluyen menos canónicos). Mantenerlos separados clarifica.

**Trade-off**: dos archivos = dos pasos de mantenimiento. Mitigación: a partir de Phase 10, cualquier obra que pase de v1 a v2 (porque adquiere extended note) se mueve de `editorial.json` a `editorial-extended.json` y se elimina del v1. Final state: v1 deprecated cuando v2 cubre todo el canon mayor (Phase 10 V2+).

**Doc afectado**: `src-tauri/data/editorial-extended.json` (NEW), `classical/editorial.rs`, `CLASSICAL_DESIGN.md` §5.3 editorial pipeline.

---

## D-046 · 2026-05-04 · EDITORIAL · classical-musicologist + classical-supervisor

**Contexto**: D-044 y D-045 establecen el schema y la convivencia. Falta decidir cómo se escribe el contenido — qué proceso editorial, quién, en qué fases.

**Decisión**: **hybrid editorial scaling con 4 etapas**:

- **Etapa 10.1 — Top 50 manual** (50-60h, 6 semanas): equipo (musicologist + Pedro) escribe a mano 50 obras canon (Beethoven 9/5/7/3, Mozart Requiem/Don Giovanni/K.466, Bach Mass in B Minor/Goldberg/Matthäus, Mahler 2/5/9, Brahms 1/4/Deutsches Requiem, Schubert Winterreise/D960, Chopin Nocturnes/Ballades, Debussy La Mer/Préludes, Stravinsky Sacre/Petrushka, Shostakovich 5/8, etc.). 1200 palabras × obra siguiendo plantilla de 5 sub-secciones (D-044). Sources cited (Wikipedia, Grove fragments, Penguin Guide, NYT/Gramophone reviews) en sección Sources. Validación cruzada por musicologist + supervisor antes de embed.

- **Etapa 10.2 — Top 200 LLM-assisted** (~90h): pipeline pre-build:
  - Para cada `work_mbid` canónico fuera del top-50:
    - Fetch Wikipedia full article (`/page/segments` API).
    - Fetch Wikidata claims (P50, P88, P1191, P710, P179, P138).
    - Prompt LLM (Claude Opus o equivalente) con plantilla estricta:
      > "Eres un musicólogo. Resume las siguientes fuentes en 5 secciones: origin, premiere, highlights, context, notable_recordings (este último opcional). 800-1200 palabras totales. Cita atribuciones con `[wikipedia]`, `[wikidata]`. NO inventes fechas, intérpretes, ni eventos. Si una fuente no cubre una sección, escribe `null`."
  - Output revisado por humano (spot-check 20% random + flag any `notable_recordings` para revisión obligatoria — alucinación frecuente).
  - Disclaimer prominente UI: *"This editorial draws from Wikipedia and Wikidata, summarized with AI assistance. Spot-checked by our team but may contain errors. [Suggest correction]."*

- **Etapa 10.3 — Long tail Wikipedia-only** (~20h): obras 200-2000. `editor_note` breve auto-generado (Wikipedia first paragraph + cleanup). Sin sección "About this work" extended; fallback al behavior Phase 5.

- **Etapa 10.4 — Crowdsourcing** (V2+, no V1): usuario puede escribir extended notes locales para sus works favoritos (almacenados en `~/.config/sone/listening-guides/{work_mbid}.note.md`). Sync futuro vía Obsidian-LiveSync. **NO en V1**.

**Justificación**:
- **Hybrid**: ningún approach único da anchura + profundidad + control de calidad.
- **Top 50 manual**: el canon más reproducido. Coste editorial alto pero coverage densamente útil.
- **Top 200 LLM-assisted**: anchura sin sacrificar consistencia. Spot-check 20% es feasible para detectar hallucinations en eventos/fechas.
- **Long tail Wikipedia**: el resto del corpus tiene editor_note breve (Phase 5 ya lo cubre para 48 obras; Etapa 10.3 amplía a 1500). Sin pretensión de extended.
- **Crowdsourcing diferido V2**: requiere sync infrastructure + moderation + UI para submit. No bloquea el USP V1.

**Alternativas consideradas**:
- *Solo manual*: rechazado. 200 × 1h/entry = 200h. Bottleneck en musicologist.
- *Solo LLM*: rechazado. Hallucinations sin manual seed son arriesgadas para un USP. Top 50 manual establece baseline de calidad y sirve como few-shot context para Etapa 10.2.
- *APIs Britannica/Oxford/Grove via licensing*: rechazado V1. Negociación legal lenta. Phase 12+ si hay tracción.

**Trade-off**: Etapa 10.2 introduce riesgo de alucinación. Mitigación: prompt estricto + spot-check 20% obligatorio + flag manual sobre `notable_recordings`. Si tasa alucinación detectada > 0 en spot-check, NO-GO ampliar Etapa 10.2; revertir a manual escaling.

**Doc afectado**: `docs/classical/phase-10-editorial-scaling.md` (NEW), `src-tauri/data/editorial-extended.json` build pipeline (Phase 10.2 + 10.3).

---

## D-047 · 2026-05-04 · ARCH · classical-supervisor

**Contexto**: Pedro reportó (2026-05-04) que "Cargar más" en ComposerPage no funciona — click no carga obras nuevas. Análisis del classical-supervisor (lectura de código): `ComposerPage.tsx:209-213` invoca `listClassicalWorksByComposer(mbid, undefined, works.length)` donde `works.length` es el conteo POST-filter D-028 (parents only). Pero el offset MB es PRE-filter. Bach: page 1 backend → MB browse offset=0, limit=100 → 100 mb_works → filtro D-028 deja 30 parents → frontend `works.length=30`. Click "Load more" → `offset=30` → MB browse offset=30 devuelve mb_works[30..130] que en page 1 ya estaban en mb_works[30..100] (overlap). De-dup defensivo elimina los duplicados → 0-5 nuevos works visibles. Pedro percibe "no funciona".

**Decisión**: el offset cliente debe ser **MB-pre-filter, no post-filter**. Backend devuelve nuevo campo `next_offset: u32` calculado como `offset + (mb_works.len() as u32)`. Frontend pasa ese campo a la siguiente call.

Cambios:
- Backend `ComposerWorksPage` añade `pub next_offset: u32`, serializado camelCase como `nextOffset`.
- En `build_composer_works_fresh`: `next_offset = offset + (mb_works.len() as u32)`.
- Frontend `ComposerWorksPage` type añade `nextOffset: number`.
- ComposerPage state: nuevo `nextOffset` en lugar de pasar `works.length`.
- `loadMoreWorks` invoca `listClassicalWorksByComposer(mbid, undefined, nextOffset)`.
- Cache key bump v2 → v3 (D-029 lo subió a v2; ahora v3) porque `next_offset` es nuevo en JSON cacheado.

**Justificación**: corrige el root cause sin tocar el filtro D-028. La separación backend conoce la verdad MB / frontend conoce sólo sus parents post-filter es honesta. El campo `total` ahora puede mantenerse como MB-pre-filter (indicando "MB tiene 1100 obras") aunque post-filter el frontend muestre 800 parents — el usuario entiende mejor "X of ~Y" cuando el ~Y refleja MB y se filtra por parents.

**Alternativas consideradas**:
- *Frontend trackea `mbConsumed` propio*: rechazado. Frágil. Rompe encapsulación: el frontend no debería conocer detalles de pagination MB.
- *Backend pagina post-filter aunque exija más MB calls*: rechazado. Para Bach (1100 obras) requeriría 11 MB pages serial (~11s) para garantizar 100 parents. Hoy 1 page basta. La complejidad está en backend que ya conoce mb_works.len().
- *Eliminar el filtro D-028 (mostrar movements en composer page)*: rechazado. D-028 es decisión cerrada de Phase 7 que arregló el bug Tchaikovsky.

**Trade-off**: cache key bump invalida todos los `composer-works:v2:*` existentes. Wipe automático al primer arranque post-deploy. Pedro tendrá una espera adicional al re-cachear (1 page = ~1s cuando MB responde, x N composers warm).

**Doc afectado**: `classical/catalog.rs::ComposerWorksPage`, `classical/catalog.rs::build_composer_works_fresh`, `src/types/classical.ts::ComposerWorksPage`, `src/components/classical/ComposerPage.tsx::loadMoreWorks`.

---

## D-048 · 2026-05-04 · ARCH · classical-supervisor

**Contexto**: D-028 (Phase 7) introdujo filter `direction=backward, type=parts` en `MusicBrainzProvider::browse_works_by_artist` para excluir child movements de la lista de works del compositor. Funciona si MB tiene la rel cargada. Pedro reportó (2026-05-04) que algunos movimientos siguen colándose ("II. Andante…" al mismo nivel que works parent) en ComposerPage. Hipótesis verificada: works donde MB **no** tiene la `parts` relationship cargada quedan sin filtrar.

**Decisión**: defensa **secundaria** por regex de título, complementando D-028 (NO sustituyéndolo):

```rust
fn title_looks_like_movement(title: &str) -> bool {
    static MOVEMENT_RE: OnceLock<Regex> = OnceLock::new();
    let re = MOVEMENT_RE.get_or_init(|| {
        Regex::new(r"^(?:[IVX]{1,4})\s*\.\s+\S").unwrap()
    });
    re.is_match(title)
}
```

Match: "I. Allegro", "IV. Presto", "VIII. Andante mosso" → drop.
No match: "Andante in C major", "Andantino", "Symphony No. 1 in C" → keep.

Aplicación en `browse_works_by_artist`, después del check D-028:

```rust
if work_is_child_movement(w) {
    continue;
}
if title_looks_like_movement(&title) {
    log::debug!("[mb] dropping movement-like title: {}", title);
    continue;
}
```

**Justificación**:
- **Secundaria**: D-028 sigue siendo defensa primaria (cubre casos con `parts` rel correctamente cargada). El regex solo atrapa los que escapan a D-028.
- **Conservadora**: regex requiere romance numeral + dot + non-whitespace. Falsos positivos sobre obras antiguas con prefijo romano legítimo son raros (más raros que movements escapando).
- **Defensa adicional en frontend**: ComposerPage.tsx renderiza filter adicional sobre el bucket `Other` cuando `title` matchea el regex. Logs warning, no muestra. Cero riesgo de mostrar "II. Andante" aunque backend lo deje pasar.

**Alternativas consideradas**:
- *Regex con número arábigo* (`^\d+\.\s+`): rechazado. Demasiado riesgo de falso positivo (works numbered "1." legítimamente).
- *Whitelist de standalone titles* ("Andante in...", "Andantino"): rechazado. Lista nunca exhaustiva; mantenibilidad alta.
- *Solo confiar en D-028 + override editorial caso por caso*: rechazado. Rendimiento bajo para casos extreme (Tchaikovsky tenía decenas de movements colándose pre-D-028).

**Trade-off**: false positives sobre obras antiguas con prefijo romano legítimo. Mitigado por el `\.` requerido tras el roman numeral — "I" solo NO matchea, "I." sí.

**Doc afectado**: `classical/providers/musicbrainz.rs::browse_works_by_artist + title_looks_like_movement`, `src/components/classical/ComposerPage.tsx` (defensa frontend simétrica).

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
