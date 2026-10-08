# ADR-017: Mejoras de audio de la Llamada

## Estado

Aceptado e implementado (2026-10). Probado con pruebas automáticas de la lógica (nivel,
guarda de eco, ajustes); **pendiente de probar con un micrófono y altavoces reales**.

## Contexto

Tres carencias de la Llamada: no había forma de saber si el micrófono elegido se oía
(solo se descubría hablando y viendo que no pasaba nada); con altavoces el personaje se
escuchaba a sí mismo por el micrófono, se transcribía y se interrumpía o se contestaba
sola; y las voces solo podían salir por la salida predeterminada de Windows.

## Decisión

### Medidor de nivel del micrófono

- El callback de captura publica `AppEvent::CallMicLevel { level }` unas 15 veces por
  segundo (`LevelMeter` junta el pico de varios bloques; no se publica por bloque).
- El nivel es el pico en dBFS pasado a escala 0–1 entre -60 y 0 dB: es como se percibe el
  volumen, y una escala lineal dejaría la barra casi quieta con voz normal.
- Se ve en la página de la Llamada (a cero si estás silenciado) y en Configuración de la
  llamada, donde al abrir el diálogo se abre el micrófono elegido **solo para medir**
  (`call_mic_test_start/stop`): no transcribe ni guarda nada, se suelta al cerrar o al
  empezar una llamada, y se reabre al cambiar de micrófono.
- El evento viaja por el bus general. `speak_and_wait` ya no da la voz por terminada si el
  bus "se queda atrás" unos eventos (antes lo hacía): sigue esperando el aviso de fin.

### Guarda contra el eco ("Uso altavoces")

- `CallPolicy::echo_guard`, apagado por defecto (con auriculares conviene poder
  interrumpir hablando, que es lo que había).
- Encendido: mientras suena la voz del personaje (eventos `VoicePlaybackStarted` →
  `Finished/Error`) y 400 ms después, al detector de voz se le pasa **silencio** en lugar
  del micrófono. Se manda silencio y no se omite el audio para que el segmentador siga
  contando el tiempo igual. Si se pierde el aviso de fin, se suelta el micrófono antes que
  dejarlo ignorado para siempre.
- **No es cancelación de eco real.** Una real resta del micrófono el audio que sale por los
  altavoces y deja pasar lo que dices a la vez; exige un procesador de audio nativo (WebRTC
  AEC / Speex), una dependencia C++ pesada de compilar en Windows. Esta guarda funciona por
  turnos: con ella no se puede interrumpir al personaje hablando mientras habla.

### Salida de audio de las voces

- Ajuste global `audio_output_device` (`None` = predeterminada), con selector en los ajustes
  de la llamada. `AudioPlayer` reabre su flujo en el dispositivo elegido; si ya no existe o
  falla cae a la predeterminada. Se aplica al arrancar y al cambiar (lo que suena se corta).
- Afecta a las voces que reproduce la app (nube y servidores HTTP). **La voz del sistema
  (Windows) no se puede redirigir**: usa siempre la salida predeterminada; la interfaz lo
  avisa.

## Alternativas descartadas

- **AEC real (WebRTC/Speex):** ver arriba; se deja para cuando se pida y se pueda compilar.
- **Subir el umbral de detección mientras habla en vez de silenciar:** deja pasar el eco
  fuerte (volumen alto, sala con reverberación) y es más difícil de explicar al usuario.
- **Un evento por bloque de audio:** decenas por segundo por el puente de Tauri sin ganar nada.

## Consecuencias

- Durante una llamada o el diálogo abierto hay ~15 eventos/s en el bus; el canal tiene
  capacidad 256 y los consumidores que se atrasan se recuperan solos.
- El selector de salida es global (Chat, Twitch y Llamada), aunque viva en los ajustes de
  la Llamada.
