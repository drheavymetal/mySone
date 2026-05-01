# Code style — mySone / sone-classical

Esta es la guía oficial de estilo de código para todo el desarrollo en mySone, especialmente la rama `soneClassical`. **Es de obligado cumplimiento** y será verificada por los agentes especialistas (`classical-supervisor`, `sone-backend-engineer`, `sone-frontend-engineer`) en cada review.

---

## 1. Llaves siempre

**Regla dura, sin excepciones.** Toda construcción que admita llaves las usa, incluso si el cuerpo es de una sola línea.

### TypeScript / JavaScript

```ts
// ✗ NO
if (x) return;
if (y) doThing();
for (const item of list) console.log(item);
arr.forEach(x => x.bar);
const f = (x) => x * 2;  // ← lambdas de UNA expresión están permitidas

// ✓ SÍ
if (x) {
  return;
}
if (y) {
  doThing();
}
for (const item of list) {
  console.log(item);
}
arr.forEach((x) => {
  x.bar;
});
const f = (x) => x * 2;  // ← lambdas de UNA expresión SIN bloque están permitidas
```

**Excepción única**: arrow functions de una sola expresión sin bloque (`(x) => x * 2`) son válidas porque el `=>` ya es un delimitador de cuerpo.

### Rust

```rust
// ✗ NO
if let Some(v) = x { return v; }
if x.is_empty() { return; }

// ✓ SÍ
if let Some(v) = x {
    return v;
}
if x.is_empty() {
    return;
}
```

**Excepción única**: closures de una sola expresión (`|x| x * 2`) son válidas.

### Por qué

Mantenibilidad. Añadir una segunda línea a un `if x do_thing()` es una fuente clásica de bugs. Llaves siempre eliminan esa clase entera de errores. Diff noise también baja: añadir una línea no obliga a re-formatear.

---

## 2. Calidad sobre velocidad

Cada bloque de código debe pasar el **test del dev nuevo**: un developer que abre el repo por primera vez puede entenderlo sin contexto. Si la respuesta a "¿qué hace esto?" requiere consultar Slack, otro archivo, o la cabeza del autor, el código está mal.

Concretamente:

- **Naming explícito.** `fetchWorkRecordings(workMbid)` > `fetchData(id)`.
- **Funciones cortas.** Si una función no cabe mentalmente en una pantalla (~40 líneas), parte.
- **Separation of concerns.** Lógica de red, transformación de datos, y presentación viven en capas separadas.
- **Errores con contexto.** `Err(format!("musicbrainz fetch_work({mbid}) failed: {e}"))` > `Err(e)`.
- **Sin código muerto.** Si una función ya no se usa, se borra. No comentada, no `#[allow(dead_code)]`.

---

## 3. Sin atajos

Evita los siguientes pares de "atajos" salvo que el código requiera explícitamente la expresividad:

```ts
// ✗ Evitar
const result = data && data.items && data.items[0];

// ✓ Preferible
const result = data?.items?.[0];

// ✗ Evitar  
arr.length === 0

// ✓ Preferible
arr.length === 0   // ambos OK; consistencia en archivo

// ✗ Casi siempre evitar
let x: any = ...;

// ✓ Tipa explícitamente
let x: SpecificType = ...;
```

```rust
// ✗ Evitar — silencia errores
let _ = result;

// ✓ Maneja explícitamente
match result {
    Ok(v) => use(v),
    Err(e) => log::warn!("...: {e}"),
}
```

---

## 4. Comentarios — solo el WHY, nunca el WHAT

```rust
// ✗ NO — comenta lo obvio
// Inserta el track en la base de datos
self.db.insert(&track)?;

// ✓ SÍ — comenta el WHY no obvio
// MB devuelve recordings agrupados por release-group; aplanamos
// porque la UI los presenta como filas planas con badge de release.
let flat: Vec<Recording> = groups.into_iter().flat_map(|g| g.recordings).collect();
```

Una buena heurística: si borrar el comentario no perjudica al lector futuro, no lo escribas.

---

## 5. Mantenibilidad como métrica

- **Tests para toda lógica nueva.** Mock providers cuando aplica, fixtures, golden files.
- **Migraciones aditivas.** Nunca DROP, RENAME, ni reorderar columnas en stats DB.
- **Versioning de cache keys.** Cambio de schema → nueva versión (`work-cache:v2`).
- **Build limpio**: `cargo check` y `npm run build` son green; `cargo clippy` sin warnings nuevos en archivos tocados.

---

## 6. Documentación

- **README por módulo** cuando el módulo tiene >5 archivos o >3 conceptos no obvios.
- **Doc comments** en APIs públicas (`pub fn`, `export function`).
- **Decision logs** en `docs/classical/DECISIONS.md` para trade-offs arquitectónicos.

---

## 7. Lenguaje

- **Identifiers**: inglés (consistente con la upstream lullabyX/sone).
- **Comentarios**: inglés salvo cuando el contexto cultural lo justifique.
- **Documentación interna** (`docs/classical/`): español o inglés, lo que el autor prefiera; consistencia dentro del fichero.
- **Mensajes de commit**: inglés.
- **Conversación con el usuario**: español.

---

## 8. Enforcement

- Cada agente especialista verifica este estilo en review.
- `classical-supervisor` rechaza cualquier cambio que viole §1 (llaves) sin excepción.
- Pre-commit hook **futuro**: linter custom que detecte one-line ifs sin llaves (P5+).

---

**Why this exists:** El usuario lo pidió explícitamente al iniciar sone-classical (2026-05-01). Es la primera regla del proyecto.
