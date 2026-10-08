# ADR-014: Copia de seguridad completa de la app

## Estado

Aceptado e implementado (2026-10). Probado con pruebas automáticas y con una copia
de los datos reales del usuario (en carpeta temporal). **Pendiente de probar la
restauración de punta a punta en la app instalada** (reinicio incluido).

## Contexto

Todo lo del usuario vive en la carpeta de datos de la app: la base SQLite, los
adjuntos de chat y los avatares. Solo existía la exportación de un personaje
(`.aivtprofile`, ADR-012), que no cubre memorias, ajustes, personas ni el resto de
personajes. Perder el equipo o corromper la base significaba perderlo todo.

## Decisión

### Qué lleva una copia

Un ZIP (`.aivtbackup`) con `manifest.json`, `aivt.db`, `attachments/` y `avatars/`.

- **La base se copia con `VACUUM INTO`**, no copiando el archivo: así la instantánea es
  consistente aunque la app esté en uso y aunque haya datos todavía en el WAL.
- **Se comprueba `PRAGMA integrity_check` sobre la instantánea antes de empaquetarla.**
  Una copia de una base dañada da una falsa sensación de seguridad; es mejor fallar
  en ese momento. Después se relee el ZIP escrito (CRC de manifiesto y base) y solo
  entonces reemplaza al `.partial`, así un corte a mitad nunca deja una copia a medias
  con el nombre final.
- **No lleva secretos** (API keys, tokens de Twitch/Discord): viven en el almacén
  seguro del sistema (`SecretStore`), y un archivo que los contuviera sería peligroso
  de compartir o perder. La interfaz lo avisa; al restaurar en otro equipo hay que
  volver a escribirlos.
- **No lleva** modelos descargados ni logs (se regeneran o se vuelven a bajar).
- Las rutas guardadas en la base son relativas a la carpeta de datos, así que una copia
  se puede restaurar en otro equipo o usuario.

### Restaurar: en dos pasos, aplicando al arrancar

La base no se puede reemplazar mientras la app la tiene abierta.

1. **Preparar** (`stage_restore`): valida la copia y la extrae a `restore_pending/`
   sin tocar nada de lo que está en uso.
2. **Aplicar** (`apply_pending_restore`): al **reiniciar**, antes de abrir la base,
   intercambia los archivos.

Validaciones antes de preparar: es un ZIP; trae `manifest.json` de Project Ciel con un
formato conocido (un `.aivtprofile` u otro ZIP no se acepta); su versión de esquema no
es **más nueva** que la de esta app; y la base extraída pasa `integrity_check` abierta
en solo lectura. Si algo falla, no queda nada a medias.

Seguridad de extracción: se acepta solo `aivt.db`, `attachments/**` y `avatars/**`
(el resto se ignora); se rechazan rutas con `..`, absolutas, `\` o `:` (flujos
alternos de Windows); hay topes de tamaño total (32 GiB) y de número de entradas.

### Nunca se destruye lo actual

Al aplicar, la base y las carpetas que había se **mueven** a `pre-restore/<fecha>/`,
no se borran. Si algo falla durante el intercambio se deshace y todo queda como
estaba. Una preparación incompleta (sin el marcador `READY`) nunca se aplica. Si la
restauración falla al arrancar, la app sigue con los datos actuales y lo registra.

### Interfaz

Configuración > Copia de seguridad. "Crear" abre el diálogo de guardar. "Restaurar"
elige un archivo, lo valida y muestra una confirmación (fecha de la copia, tamaño,
y que lo actual se conserva aparte); al confirmar reinicia la app. Cancelar descarta
la preparación.

## Razones

- Copiar el `.db` en caliente puede dar una copia inconsistente (WAL); `VACUUM INTO`
  es la forma soportada de obtener una instantánea limpia, y de paso la compacta.
- Aplicar al arrancar evita cualquier carrera con conexiones abiertas y hace el
  intercambio un simple renombrado de archivos.
- Mover lo anterior en vez de borrarlo convierte una restauración equivocada en algo
  reversible a mano.

## Alternativas descartadas

- **Incluir los secretos cifrados con una contraseña:** más superficie de riesgo y una
  contraseña más que olvidar; volver a pegar las keys es poco costoso.
- **Reemplazar la base en caliente cerrando el pool:** los servicios en marcha (Twitch,
  Discord, Llamada) tienen clones del pool; cerrarlo a medias es una fuente de errores.
- **Copia incremental o automática programada:** no se ha pedido; el formato deja la
  puerta abierta (el manifiesto lleva versión).

## Consecuencias

- El intercambio de archivos al restaurar **reintenta** el renombrado unos segundos: en Windows un
  archivo recién cerrado (antivirus, indexador) puede seguir bloqueado un instante. Se detectó como
  un fallo intermitente (1 de cada 5 ejecuciones) en las pruebas.

- Una copia pesa lo que pesen la base y los adjuntos (los audios guardados de chat son
  lo que más crece); en los datos reales del usuario: ~53 MB.
- `pre-restore/` se acumula con cada restauración: no se limpia solo.
- Una copia hecha por una versión futura con un esquema más nuevo se rechaza con un
  mensaje claro en vez de arriesgarse a abrirla.
- Prueba manual con datos reales: `AIVT_REAL_DATA_DIR=<datos> cargo test -p
  aivt-service-backup real_data -- --ignored --nocapture` (trabaja sobre una copia).
