# ADR-013: Alcance de la memoria (`scope`) y memoria compartida entre canales

## Estado

Aceptado e implementado (2026-10). Probado con pruebas automáticas; **pendiente de
probar en vivo** (ver "Pendiente / sin verificar" en `docs/DEVELOPMENT_STATUS.md`).

## Contexto

Solo el Chat leía la memoria y el estado de ánimo del personaje. La Llamada,
Twitch y Discord respondían sin ningún recuerdo, así que el mismo personaje
"no sabía quién eras" según el canal por el que le hablaras. Compartirla sin más
tenía un riesgo claro: las memorias se extraen de charlas **privadas** con el dueño,
y Twitch/Discord son canales **abiertos a terceros**. Un recuerdo privado
("cuéntame lo que te dije de mi familia") podría aparecer en una respuesta que lee
todo el chat.

## Decisión

### Un campo `scope` por memoria

- `memories.scope` (migración `0019`): `'private'` (por defecto) o `'public'`.
  - **Privada**: solo los canales del dueño la usan — Chat y Llamada.
  - **Pública**: además la usan los canales abiertos — Twitch y Discord.
- Todo lo que ya existía queda `'private'`, y toda memoria nueva (extraída de una
  charla, creada a mano o fusionada) nace privada. **Hacer pública una memoria es
  siempre una decisión explícita del usuario**: falla cerrado.
- Un valor desconocido se rechaza al escribir (`MemoriesRepository::set_scope`) y se
  lee como privado en el frontend (`memoryScopeSchema.catch("private")`).

### Qué lee cada canal

| Canal | Memorias | Estado de ánimo |
|---|---|---|
| Chat | todas (`search`) | sí |
| Llamada | todas (`search`) | sí |
| Twitch | solo públicas (`search_public`) | no |
| Discord | solo públicas (`search_public`) | no |

- La Llamada es una charla en directo con el dueño, igual de privada que el Chat, por
  eso ve todo. Busca con una ventana corta de los últimos turnos (`CallHistory::
  retrieval_query_with`), sin la marca "[Mensaje por voz...]", cuyas palabras
  harían coincidir cualquier memoria que las mencione.
- Twitch y Discord **no usan el estado de ánimo**: sale de charlas privadas con el
  dueño y no hay motivo para que condicione lo que se dice en público.
- Un fallo de búsqueda o de ánimo nunca bloquea la respuesta: degrada a "sin esa
  sección" (mismo criterio que el Chat, ADR-006).

### El filtro va en la consulta, no después

`MemoryManager::search_public` pasa `public_only` hasta el SQL
(`search_scoped`, `search_by_embedding_scoped`). Filtrar después de limitar dejaría
que memorias privadas mejor posicionadas ocuparan el tope del ranking y la pública
nunca apareciera; hay una prueba que lo cubre.

### Fusión de memorias

Una memoria fusionada es pública **solo si todas las originales lo eran**. Mezclar un
dato privado en una pública lo expondría, así que cualquier duda la deja privada.

### Interfaz

En la pestaña Memoria cada tarjeta tiene un botón candado/globo. Pasar a pública pide
**confirmación** (explica que Twitch y Discord podrán usarla y que cualquiera que
escriba allí podría verla reflejada en una respuesta); volver a privada es inmediato.
Las públicas llevan una etiqueta "Pública".

## Razones

- Un campo de dos valores es lo mínimo que resuelve el riesgo; algo más fino (por
  canal, por persona) añadiría una interfaz que nadie ha pedido.
- Que el valor por defecto sea privado hace imposible exponer algo por omisión: la
  migración no cambia el comportamiento actual de ningún canal que ya funcionaba.
- Pasar `public_only` al SQL evita una clase de errores (privadas que desplazan a
  públicas) en vez de repararla con lógica adicional después.

## Alternativas descartadas

- **Compartir toda la memoria con todos los canales:** filtra charlas privadas.
- **No compartir nada:** deja a la Llamada sin recuerdos, que era la queja original.
- **Un alcance por canal** (p. ej. "visible en Twitch pero no en Discord"): más
  interfaz y más casos sin una necesidad real todavía.
- **Que la extracción decida el alcance con el modelo:** un modelo débil decidiendo
  qué es privado es justo el fallo que no se puede permitir.

## Consecuencias

- Mientras el usuario no marque ninguna memoria como pública, Twitch y Discord
  siguen respondiendo exactamente igual que antes (sin recuerdos).
- Las memorias creadas a mano también nacen privadas; si quiere que Twitch conozca
  un dato, lo marca después con el botón.
- La Llamada ahora hace una búsqueda de memoria por turno: añade una consulta local
  (y un embedding de la consulta si el modelo está descargado) antes de generar.
  Se ha mantenido pequeño (5 recuerdos).
- Una memoria extraída de una conversación exportada desde la Llamada al Chat sigue
  siendo privada.
- El ánimo sigue sin escribirse desde la Llamada: solo lo actualiza la extracción
  del Chat (ADR-006). La Llamada lo lee pero no lo mueve.
