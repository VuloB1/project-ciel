# ADR-005: Búsqueda semántica local para memoria

## Estado

Aceptado e implementado (Fases posteriores a Fase 11, en tres etapas:
proveedor de embeddings + descarga del modelo; almacenamiento y cálculo
en segundo plano; búsqueda híbrida). Ver `docs/DEVELOPMENT_STATUS.md`
para el detalle completo de cada etapa.

## Contexto

`docs/ADR/004-memory-architecture.md` ya dejó anotado "FTS5 primero,
embeddings como mejora posterior" como alcance aprobado del producto
(sección 36 del brief), pero sin comprometerse a un runtime, un modelo,
ni una forma de almacenamiento concretos — decisiones con costo real de
revertir, exactamente el tipo de cosa que el proceso de ADR de este
proyecto existe para fijar antes de implementar.

El pedido concreto del usuario fue aplicar esta mejora **de forma
preventiva**: hoy una pregunta parafraseada sin las palabras exactas de
una memoria guardada ("¿de qué querés hablar cuando salimos de viaje?"
sin decir "viaje" literal) no encuentra memorias relevantes que un
embedding sí encontraría por significado, aunque todavía no sea un
problema real observado — se construye antes de que la memoria crezca
lo suficiente para que duela.

## Decisión

- **Motor: `candle`** (framework de ML en Rust puro), no `ort`/ONNX
  Runtime. `candle-transformers::models::bert::BertModel` cubre
  exactamente lo necesario (`BertModel::load` + `forward`), y a
  diferencia de `ort` no exige distribuir un `onnxruntime.dll/.so`
  separado junto al ejecutable.
- **Modelo: `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2`**
  — arquitectura BERT plana, 384 dimensiones, multilingüe (necesario:
  las conversaciones son en español), pooling mean-tokens, ~449MB.
  Descarga única desde Hugging Face a `<app_data_dir>/models/embedding/`,
  mismo mecanismo que ya usa `aivt-provider-stt` para whisper.cpp
  (streaming, archivo `.part` + rename atómico, eventos de progreso).
- **Almacenamiento: tabla satélite `memory_embeddings`**, no una columna
  en `memories` — `BLOB` de `f32` little-endian crudo. Ausencia de fila
  significa "todavía no embebida".
- **Similitud: fuerza bruta** (producto punto sobre vectores ya
  L2-normalizados), sin índice ANN (`sqlite-vec` ni base de datos
  vectorial externa).
- **Cálculo en segundo plano, nunca bloqueante** — crear/editar/extraer
  una memoria dispara un `tokio::spawn` que la embebe; un modelo no
  descargado o una inferencia fallida no interrumpen nada.
- **Búsqueda híbrida, no un reemplazo de FTS5** — `MemoryManager::search`
  combina los resultados de FTS5 (BM25) y de similitud semántica vía
  Reciprocal Rank Fusion (constante k=60, FTS con peso 1.2 contra 1.0 de
  la parte semántica), degradándose automáticamente a solo-FTS si el
  modelo no está descargado o la consulta no se puede embeber.
- **Sin toggle de configuración nuevo** — la descarga del modelo ES el
  opt-in (Configuración > Memoria), misma UX que ya tiene STT.

## Razones

- `candle` resultó, en la práctica, **más simple** de empaquetar que
  whisper.cpp (que ya vive en este proyecto): no exige cmake ni un SDK
  de GPU para compilar, solo Rust puro con un backend CPU por defecto.
- El modelo elegido no tiene una alternativa realmente más chica dentro
  de la misma familia multilingüe: cualquier modelo derivado de
  `Multilingual-MiniLM-L12-H384` comparte el mismo vocabulario de 250k
  tokens (~366MB solo en esa matriz), que es el piso real de tamaño, no
  la profundidad del modelo. Es del mismo orden que Whisper "Small"
  (466MB), que la app ya distribuye sin problema.
- Fuerza bruta sin ANN: a la escala de este proyecto (memoria de un solo
  usuario, cientos a bajos miles de filas en toda la vida de la app,
  nunca millones) un índice aproximado no aporta nada salvo una
  dependencia nativa más que empaquetar.
- Cálculo en segundo plano y degradación silenciosa: misma filosofía que
  ya regía la recuperación de memoria antes de este trabajo ("es una
  mejora, no una dependencia dura") — una respuesta de chat, o guardar
  una memoria, nunca deben esperar a (ni fallar por) el cálculo de un
  embedding.
- RRF en vez de mezclar los scores de BM25 y similitud coseno
  directamente: son escalas incomparables (BM25 no está acotado,
  coseno sí) — RRF solo usa la *posición* dentro de cada lista, evitando
  tener que normalizarlas entre sí. El peso mayor para FTS evita que una
  coincidencia exacta de término/nombre quede enterrada bajo un
  resultado "parecido pero no exacto".

## Consecuencias

- La calidad de la búsqueda de memoria ahora depende de si el usuario
  descargó el modelo — sin descargarlo, el comportamiento es idéntico al
  de antes de este trabajo (FTS5 puro), sin ninguna regresión.
- Un futuro cambio de modelo necesitaría re-embeber las memorias
  existentes. El esquema ya lo soporta (`memory_embeddings.model_version`
  guarda qué modelo generó cada vector), pero no existe todavía un
  disparador automático que detecte una versión desactualizada y
  re-encole el cálculo — quedaría como trabajo futuro si/cuando se
  cambie el modelo, no bloqueante hoy.
- Twitch, Discord y Llamada no participan de esta mejora — hoy ninguno
  de esos canales llama a `MemoryManager::search` en absoluto (una
  decisión de alcance previa a este trabajo, ver
  `docs/ADR/004-memory-architecture.md`), así que la búsqueda híbrida
  solo beneficia a Chat y a la pestaña Memoria por ahora.
