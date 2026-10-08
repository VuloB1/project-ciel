# ADR-011: Dictado por voz en el chat (grabar, transcribir al detener)

## Estado

Aceptado e implementado (2026-10). Probado en vivo por el usuario con su voz;
los gestos y el flujo se verificaron además con eventos de ratón simulados.

## Contexto

El usuario pidió un botón de micrófono junto al de enviar en el chat: mantenerlo
pulsado para dictar, deslizar hacia arriba para fijar la grabación, un botón de
stop mientras se escucha, ver el texto mientras se habla, y un ajuste para
enviar solo lo dictado. La app ya tenía transcripción local (whisper.cpp) y
captura de micrófono para Llamada, con selección de dispositivo.

## Decisión

- **Reutilizar la captura y Whisper de Llamada** (`CallService::dictation_start/
  stop/cancel`), no una API del navegador. El audio se acumula en memoria
  (tope de 5 minutos) y se transcribe localmente: no sale del equipo.
- **Grabar y transcribir al detener** para el texto final (pasada con todo el
  audio, la más precisa), más **texto provisional en vivo**: mientras se graba, cada
  ~1,5 s se transcribe lo acumulado y se publica `AppEvent::DictationPartial`
  (se reemplaza entero, no se concatena). La siguiente pasada empieza cuando
  termina la anterior, así que nunca se acumulan en cola. Los provisionales solo
  miran los últimos 25 s para que no se vuelvan cada vez más lentos.
- **Filtros contra alucinaciones de Whisper:** el audio cuyo pico queda por
  debajo de un umbral se trata como silencio y no se transcribe, y se quitan las
  etiquetas entre corchetes (`[Música]`) que Whisper inventa sobre ruido.
- **Falla temprano:** si hay una llamada en curso o no hay un modelo descargado,
  `dictation_start` lo dice antes de que el usuario hable.
- **Gestos** (`src/features/chat/useDictation.ts`): mantener pulsado graba y al
  soltar se detiene; deslizar ≥56 px hacia arriba lo fija; un toque corto también
  lo fija; Escape cancela. Al fijar, el micrófono pasa a un botón de stop.
- **Ajuste** `dictation_settings.autoSend` (Configuración > Transcripción de
  voz): enviar solo el texto al detener, o dejarlo en el cuadro para revisarlo.

## Razones

- Una sola implementación de captura y transcripción para Llamada y Chat: el
  mismo micrófono elegido, el mismo modelo descargado, el mismo sesgo hacia el
  nombre del personaje.
- Whisper no es un reconocedor en streaming: repetir la transcripción sobre el
  audio acumulado da un texto en vivo aceptable sin añadir otro motor, a costa
  de CPU mientras se habla.

## Alternativas descartadas

- **`SpeechRecognition` del navegador (WebView2):** depende de un servicio en la
  nube del fabricante, no funciona sin conexión y rompe la promesa local-first.
- **Transcripción en streaming real (otro modelo/motor):** más complejidad y otra
  dependencia para algo que el texto provisional ya resuelve.

## Consecuencias

- El texto en vivo consume CPU mientras se habla; en equipos justos puede notarse.
  El intervalo y la ventana son constantes fáciles de ajustar
  (`PARTIAL_INTERVAL`, `PARTIAL_WINDOW_SAMPLES`).
- Dictados de más de 25 s: lo provisional muestra solo el final, pero el texto
  final incluye todo.
- Un micrófono muy bajo podría quedar por debajo del umbral de silencio y no
  transcribirse (umbral conservador, no calibrado por dispositivo).
