# ADR-019: Modos de privacidad por proveedor de IA

## Estado

Aceptado e implementado (2026-10). Probado con pruebas automáticas (filtro, orquestador,
ajustes, interfaz); **pendiente de probar en la app con un proveedor en la nube real**.

## Contexto

La app puede hablar con un modelo en tu propio equipo (LM Studio, Ollama) o con servicios en
la nube (OpenAI, Anthropic, Gemini, Groq…), y con el cambio automático (ADR de proveedores
múltiples) una petición puede acabar en uno distinto del esperado. Hasta ahora todo
proveedor recibía lo mismo: la conversación, TODOS los recuerdos relevantes (privados y
públicos), el estado de ánimo y las imágenes adjuntas. Para un modelo local es lo
correcto; para un servicio en la nube, no necesariamente lo que el usuario quiere.

## Decisión

Cada proveedor tiene un **modo de privacidad**, elegido por el usuario (Configuración >
Proveedores de IA > Editar):

| Modo | Recibe |
|---|---|
| **Completo** (por defecto) | conversación, recuerdos privados y públicos, estado de ánimo, imágenes |
| **Solo recuerdos públicos** | conversación, estado de ánimo, imágenes, y de los recuerdos solo los marcados como públicos (ADR-013) |
| **Mínimo** | solo la conversación: sin recuerdos, sin estado de ánimo, sin imágenes |

La conversación siempre llega: sin ella no se puede responder. El modo por defecto es
Completo para no cambiar el comportamiento de quien ya tenía proveedores; los datos
guardados antes de esta función cargan como Completo.

### Cómo se aplica

- El orquestador ya **no pega** los recuerdos ni el estado de ánimo en el prompt: los manda
  aparte en `ChatRequest::context` (`ContextBlock::Memories { all, public_only }` y
  `ContextBlock::Mood`). Los recuerdos se redactan en las dos variantes porque el
  encabezado y el idioma los pone el orquestador.
- Cada proveedor del pool se construye envuelto en un `PrivacyGuardProvider` con su modo,
  que anexa al mensaje de sistema solo lo permitido y quita las imágenes si no se permiten.
  Como el envoltorio viaja con el proveedor, **la regla se aplica también cuando el cambio
  automático pasa a otro**: una reserva en la nube en modo mínimo no recibe lo que sí recibía
  el modelo local.
- **Falla cerrado:** un proveedor sin envoltorio simplemente ignora el contexto. Por eso se
  envuelven todos, también en modo Completo.
- **Fusionar memorias** manda todos los recuerdos guardados (`contains_stored_memories`):
  un proveedor que no está en modo Completo rechaza la petición con un error claro
  (`ProviderError::PrivacyBlocked`), que no hace saltar al siguiente proveedor — es una
  acción del usuario y tiene que decidir él.
- **Visión:** en modo Mínimo el interruptor de visión se apaga y queda bloqueado (también en
  el backend), para que el chat no ofrezca adjuntar una imagen que el modelo no vería.
- **Local o nube:** cada proveedor muestra una insignia. Es "Local" un servidor genérico
  cuya URL apunta a esta máquina o a una red privada (`localhost`, `127.*`, `10.*`,
  `192.168.*`, `172.16–31.*`, `*.local`, `::1`); todo lo demás, incluido un servidor
  genérico remoto, es "Nube". Los proveedores con endpoint fijo (OpenAI, Anthropic…) siempre
  son nube.

## Alcance y límites

- **Qué NO cubre:** lo que escribes y lo que el personaje dice en la conversación llega
  siempre; el prompt de identidad (nombre, personalidad) también. Los mensajes de Twitch y
  Discord que entran como conversación tampoco se filtran. La búsqueda web manda tu consulta
  a Brave, no al proveedor, y sus resultados vuelven al modelo como parte de la
  conversación.
- **Extracción de memorias** (a partir de una conversación) usa el proveedor en uso con el
  contenido de esa conversación, igual que el chat; no está bloqueada en ningún modo.
- Es una garantía de la app frente al proveedor, no frente al servicio: lo que SÍ se manda
  a un proveedor en la nube queda sujeto a las políticas de ese servicio.

## Alternativas descartadas

- **Un único ajuste global:** no sirve con cambio automático entre un modelo local y uno en
  la nube; lo razonable es dar más a lo local y menos a lo remoto.
- **Interruptores sueltos por dato** (recuerdos sí/no, ánimo sí/no, imágenes sí/no):
  más combinaciones que explicar; tres modos cubren los casos reales.
- **Marcar secciones del prompt con delimitadores y recortarlas después:** si un proveedor
  no pasa por el filtro, filtra lo privado hacia fuera sin avisar. El contexto aparte falla
  cerrado.
- **Marcar el valor por defecto de los proveedores en la nube como restringido:**
  cambiaría el comportamiento de quien ya los usa; la insignia "Nube" y la nota del
  formulario ya dejan clara la decisión.

## Consecuencias

- El orden del prompt cambia ligeramente: el estado de ánimo y los recuerdos van ahora al
  final del mensaje de sistema, después de la fecha, las herramientas y las instrucciones del
  canal.
- Se observó de paso que los adaptadores de Anthropic y Gemini solo reenviaban el PRIMER
  mensaje de sistema, así que el recordatorio de fecha/herramientas que el orquestador añade
  después del historial no les llegaba. Corregido: ahora se unen todos los mensajes de sistema,
  en orden, en el único campo de sistema de cada API (`joined_system_prompt`). El recordatorio
  llega, aunque queda arriba y no después del historial, porque esas APIs no admiten sistema
  intercalado.
- El panel de uso por proveedor (peticiones, errores, última vez usado) sigue pendiente.
