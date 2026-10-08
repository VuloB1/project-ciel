# ADR-020: Panel de uso por proveedor de IA

## Estado

Aceptado e implementado (2026-10). Probado con pruebas automáticas; **pendiente de verlo en la
app con peticiones reales**.

## Contexto

Con varios proveedores y cambio automático (y ahora modos de privacidad, ADR-019) no había
forma de saber cuál estaba respondiendo de verdad, cuánto, ni cuántas veces fallaba. Para un
proveedor de pago, tampoco una idea de cuánto se consume.

## Decisión

Configuración > Proveedores de IA muestra, bajo la lista, un panel **Uso** con un periodo
(hoy, 7 días, 30 días, todo) y, por proveedor con actividad: peticiones, tokens estimados
enviados y recibidos, fallos, último uso y último error. Botón "Reiniciar" con confirmación.

### Cómo se mide

- `UsageRecordingProvider` envuelve a cada proveedor del pool, **por dentro** del filtro de
  privacidad: cuenta lo que de verdad se manda al proveedor, ya filtrado. Un cambio
  automático queda reflejado solo: cada proveedor cuenta sus propios intentos, incluidos los
  que fallaron antes de pasar al siguiente.
- Cada petición se anota **una vez**, al terminar el flujo, al fallar o al abandonarlo (el
  usuario interrumpe a media respuesta). La escritura es en una tarea aparte: contar nunca
  retrasa la respuesta.
- Una petición abandonada sin recibir nada después de 30 s cuenta como fallo ("no respondió a
  tiempo"): el cambio automático corta por tiempo y el proveedor no llega a devolver un error
  propio.
- **Los tokens son una estimación** (caracteres / 4): los proveedores no los devuelven siempre
  y los adaptadores no los leen. La interfaz lo dice. No es lo que factura el proveedor.

### Qué se guarda

Tabla `llm_usage` (migración `0020`), una fila por proveedor y día: número de peticiones, de
errores, caracteres enviados y recibidos, última vez y último error (cortado a 200
caracteres). **Nunca el texto de los mensajes.** Se borra al eliminar el proveedor y al
reiniciar el panel. Entra en la copia de seguridad como el resto de la base (ADR-014).

## Alternativas descartadas

- **Leer el consumo real de las respuestas** (campo `usage` de cada API): cada adaptador lo
  entrega distinto y a veces no lo entrega con streaming; queda para cuando se necesite el
  dato exacto.
- **Guardar cada petición** (un registro por llamada): más datos sensibles y más tamaño sin
  aportar al panel; el agregado diario basta.
- **Contar en el frontend:** se perdería lo que ocurre con la ventana cerrada (Twitch,
  Discord, proactivo) y entre reinicios.

## Consecuencias

- El gasto en dinero no se calcula: depende del precio de cada modelo, que la app no conoce.
- Lo que no pasa por `build_llm_provider` (la prueba de conexión del formulario, por
  ejemplo) no se cuenta.
- Un proveedor eliminado y vuelto a crear empieza de cero (tiene un id nuevo).
