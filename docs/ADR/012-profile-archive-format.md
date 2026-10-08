# ADR-012: Formato `.aivtprofile` — un ZIP con perfil, imágenes y chats

## Estado

Aceptado e implementado (2026-09). Pendiente: ícono propio para el tipo de
archivo y el doble clic para importar, que solo funcionan con la app instalada.

## Contexto

"Exportar" un personaje generaba un JSON plano con los campos de texto: al
borrarlo e importarlo de nuevo se perdían la foto, las imágenes del avatar
reactivo y todo el historial de chat. Para quien usa la app de verdad, era un
respaldo engañoso.

## Decisión

- `.aivtprofile` es un **ZIP** (`profile.json` con versión 2, las imágenes del
  perfil, y opcionalmente `chats.json` + `attachments/<hash>.<ext>`). Los chats
  van marcados por un checkbox en el diálogo de exportar, **activado por
  defecto** para no perder nada por accidente, desactivable para compartir el
  personaje sin las conversaciones propias.
- Los adjuntos se deduplican por hash, igual que en disco
  (`ChatService::save_attachment`).
- **Compatibilidad hacia atrás:** al importar se intenta abrir como ZIP y, si no
  lo es, se lee como el JSON plano de antes. Ningún archivo viejo queda ilegible.
- Importar siempre crea un perfil nuevo (ids nuevos), nunca pisa uno existente.
- Doble clic en un `.aivtprofile`: con la app cerrada o abierta se publica
  `AppEvent::ProfileImportRequested` y el frontend pide **confirmación** antes de
  importar; nunca importa sin preguntar.

## Razones

- Un ZIP resuelve de una vez "varios archivos" y "compresión" sin inventar un
  formato propio.
- Deduplicar por hash evita que el archivo crezca si un mismo adjunto aparece en
  varios mensajes.

## Consecuencias

- Un `.aivtprofile` con chats puede ser grande y contiene las conversaciones del
  usuario: el checkbox existe por eso (ver `docs/security.md`).
- La asociación de archivo y su ícono dependen del instalador (Tauri
  `fileAssociations` no tiene un campo de ícono por extensión en esta versión):
  un ícono propio exige una plantilla NSIS personalizada y se comprueba solo con
  una build instalada.
- **Seguridad de la importación:** un `.aivtprofile` es una entrada no confiable. Los nombres de adjunto (`hash`, `extension`) se validan estrictamente antes de formar cualquier ruta, las rutas de las entradas del ZIP nunca se usan para escribir, y hay topes de tamaño por entrada (64 MB) y totales (1 GB). Una primera versión de la importación no validaba los nombres de adjunto y permitía escribir fuera de la carpeta de adjuntos; se corrigió el 2026-10-02.
