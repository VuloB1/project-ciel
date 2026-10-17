# Lista de comprobaciones (build de pruebas)

Marca cada punto. Si algo falla, anota **qué hiciste, qué esperabas y qué pasó**, y el modelo/voz que usabas.
Todo esto está implementado y probado con pruebas automáticas, pero **nada se ha visto en la app real**.

## Antes de empezar

- [ ] **Haz una copia de seguridad.** Esta build añade una migración de base de datos (0021). Una versión
      más vieja (como la 0.1.0 publicada) puede negarse a arrancar sobre una base ya migrada. Usa
      Configuración > copia de seguridad, o copia la carpeta de datos de la app.
- [ ] Ten abierto tu servidor de modelos (LM Studio) con el modelo que usas normalmente.
- [ ] Usa un personaje de pruebas, no el principal.

## 1. Cómo conversa (Chat)

Manda estos mensajes en un chat nuevo y mira el largo y el tono. Antes, un modelo típico contestaba varios párrafos.

- [ ] `holaa`, `jajaja sii`, `tengo sueño` → una frase corta, sin ofrecer ayuda.
- [ ] `hoy fue un día horrible` → reacciona y acompaña, sin lista de consejos.
- [ ] `no quiero consejos, solo quería contártelo` → no da consejos.
- [ ] `explícame cómo funciona la memoria de un ordenador` → aquí SÍ debe explayarse y puede estructurar.
- [ ] Escribe varios mensajes secos seguidos (`ok`, `jaja`, `sí`) y luego uno mediano → la respuesta no debe ser mucho más larga que tus mensajes.
- [ ] No termina cada respuesta con "¿en qué más puedo ayudarte?" ni con una pregunta en cada turno.
- [ ] Cambia el idioma de la app (por ejemplo a inglés) y repite 2 mensajes: debe responder en ese idioma.

## 2. Personalidad

- [ ] En Identidad > Vista previa: con los sliders en 40–60 las secciones neutras desaparecen del texto.
- [ ] El texto habla en segunda persona ("Tu trato es…", "You are…"), sin "ella/she/ela".
- [ ] Prueba un personaje con pronombres masculinos o no binarios: nada del prompt debe contradecirlo.
- [ ] Sube un slider a 90 (humor, por ejemplo) y comprueba que el personaje lo refleja al hablar.

## 3. Resumen de sesión y temas pendientes

- [ ] Tras ~20 mensajes en un mismo chat, deja pasar unos segundos y sigue hablando.
- [ ] Menciona algo pendiente al principio (por ejemplo "mañana tengo un examen"), alarga la charla unos 25 mensajes y pregunta "¿de qué hablábamos al principio?".
- [ ] Mira que no invente cosas en lo que recuerda. Si lo hace, anota con qué modelo.
- [ ] Confirma que la respuesta del chat no se retrasa cuando se crea el resumen.
- [ ] (No hay pantalla para ver el resumen; solo se nota en lo que dice.)

## 4. Memoria

- [ ] Dile un dato falso sobre ti, deja que Ciel responda, y luego pregunta algo vago ("sí, ese"). No debe recuperar recuerdos relacionados con lo que ELLA inventó.
- [ ] En una conversación muy larga con mensajes largos, las respuestas siguen siendo coherentes (el historial ahora se corta por tamaño).

## 5. Llamada de voz

- [ ] La primera frase empieza a sonar antes de que el modelo termine la respuesta completa.
- [ ] Entre frases no hay cortes raros. Con una voz en la nube puede haber una pausa corta: anota cuánto dura.
- [ ] Interrúmpela a mitad de una respuesta y pregúntale "¿qué me estabas diciendo?": debe recordar solo lo que alcanzó a decir.
- [ ] Respuestas habladas cortas, sin "asterisco", listas ni símbolos leídos en voz alta.
- [ ] Modo frase de activación: con ruido o charla de fondo sin su nombre, no debe interrumpirse ni quedarse en un estado raro. Al decir su nombre, responde.
- [ ] Con "Uso altavoces" activado no se puede interrumpir hablando (es una limitación conocida).
- [ ] Al colgar y volver a llamar, no queda nada colgado.

## 6. Voz y audio guardado

- [ ] Cambia entre la voz del sistema y una voz remota y reproduce un mensaje con cada una: nunca deben sonar a la vez.
- [ ] Si usas Groq: activa guardar audio, genera una respuesta y reprodúcela otra vez desde el botón (antes el archivo guardado tenía formato equivocado).
- [ ] Reproduce el audio guardado de un mensaje, y enseguida el de otro: el primero se corta.
- [ ] Con un clip guardado sonando, pulsa Stop y también el botón de silenciar global: ambos deben pararlo.

## 7. Que nada se haya roto

- [ ] Una conversación existente de antes se abre bien y se puede seguir.
- [ ] Twitch y Discord (si los usas) siguen respondiendo.
- [ ] Copia de seguridad y restauración siguen funcionando.
- [ ] Centro de diagnóstico sin errores nuevos.

## 8. Conversation Lab (opcional, mide con números)

Con LM Studio abierto:

```bash
LAB_MODEL=nombre-del-modelo cargo test -p aivt-orchestrator lab -- --ignored --nocapture
```

- [ ] Guarda la salida. Cuenta cuántas de las 30 situaciones salen limpias y pásamela para compararla con cambios futuros.

## Qué anotar si algo falla

Versión de la app, modelo y proveedor, voz usada, pasos exactos, y si pasa siempre o a veces.
