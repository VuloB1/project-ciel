# ADR-018: Auditoría de accesibilidad de los temas

## Estado

Aceptado e implementado (2026-10). Verificado con una prueba automática que recorre los 15
temas; **pendiente de una pasada visual en la app** (sobre todo el modo Claro y los bordes
de los campos, que cambian de aspecto).

## Contexto

Los 15 temas se diseñaron a ojo (ADR-009) y el modo Claro (una rampa de neutros compartida)
y los acentos propios por perfil se añadieron encima. Nadie había medido el contraste. Se
midió con WCAG 2.1 AA (4,5:1 para texto normal, 3:1 para el borde de un control) sobre lo
que realmente se pinta, y salió esto:

- **Modo Claro:** el acento como texto (enlaces, `text-accent`) daba entre 1,1:1 y 3,4:1 en
  los 14 temas oscuros, y con el acento de fábrica (#96FFF3) 1,3:1.
- **Colores de estado** (éxito, aviso, error) eran los mismos en todos los temas y estaban
  hechos para fondo oscuro: sobre fondo claro (modo Claro y el tema Paper) quedaban en
  1,7–2,2:1 el verde y el ámbar, y 2,8–3,4:1 el rojo. El botón de peligro (blanco sobre
  rojo) daba 3,4:1.
- **Texto secundario** por debajo de 4,5:1 en Cyberpunk, Minimal, Luxury, Noir y Paper
  (hasta 2,9:1 en Paper).
- **Acentos propios** muy claros, oscuros o de tono medio no tenían ninguna garantía.
- **Bordes de campos, selectores e interruptores:** entre 1,2:1 y 1,8:1, es decir, casi
  invisibles para quien ve poco contraste.
- Lo que sí cumplía: texto principal (≥11:1 en todos), texto sobre el acento y el acento
  como texto en los 14 temas oscuros.

## Decisión

Se arregla en la paleta final, no tema por tema a mano, y se deja una prueba que lo vigila.

1. **Texto secundario:** se ajustaron los 5 presets que no llegaban (acercándolo al texto
   principal lo mínimo; en Paper, más oscuro). Autumn: acento un punto más claro para que
   el texto atenuado de sus botones también se lea.
2. **Acento legible (`withReadableAccent`):** si el acento o el acento secundario no llegan
   a 4,5:1 sobre el fondo y las dos superficies —también sobre su propio fondo translúcido
   `bg-accent/10`—, se aclara u oscurece conservando el matiz, solo lo necesario. Sirve para
   el modo Claro y para cualquier color propio. Un acento que ya cumple no se toca. El texto
   de los botones (blanco o casi negro) se conserva si se lee, también atenuado al 85 %;
   si no, se cambia al que más contraste dé.
3. **Colores de estado por modo (`STATUS_COLORS`):** un juego para fondos oscuros (el rojo se
   aclaró a `255 132 124`) y otro para claros (verde `17 106 51`, ámbar `141 75 0`, rojo
   `179 28 28`), calculados contra las superficies de todos los temas de su modo y contra su
   fondo translúcido. Nuevo token `error-foreground` (casi negro en oscuro, blanco en claro)
   para el texto del botón de peligro, el de colgar y el cierre de la barra de título.
4. **Bordes de controles (`--color-control-border`, clase `border-control`):** el borde del
   tema acercado al texto secundario hasta 3:1 sobre las superficies. Lo usan Input, Select,
   Textarea y Switch. Los separadores de tarjetas siguen con el borde suave del tema: WCAG
   exime a lo decorativo.
5. Las insignias y el chip del avatar pasan de fondo `/15` a `/10`, y el texto atenuado de
   las burbujas de `/70` a `/85`, porque con la opacidad anterior no había contraste posible
   en varios temas.

`resolveThemeTokens` (función pura) calcula todo esto y el hook solo escribe las variables
CSS, así la prueba mide exactamente lo que ve el usuario.

## La prueba

`src/lib/theme/__tests__/themeContrast.test.ts`: 15 temas × {oscuro, claro}, el tema por
defecto, y 10 acentos propios (blanco, negro, azul puro, amarillo, rojo, verde, gris...)
sobre el tema por defecto y sobre Forest, en los dos modos. Exige 4,5:1 en texto principal y
secundario, acento y acento secundario (solos y sobre su fondo translúcido), colores de
estado (solos y sobre su fondo translúcido), texto de botones y texto atenuado, y 3:1 en el
borde de controles. Otra prueba exige que cada tema ya venga limpio en su propio modo (el
ajuste en tiempo de ejecución no lo modifica). Añadir un tema nuevo con mal contraste hace
fallar la prueba.

## Alcance y límites

- Es contraste de color. No se midieron tamaño táctil, orden de foco, lectores de pantalla
  ni movimiento reducido.
- Los iconos y los anillos de foco usan el acento (≥4,5:1 ahora, más que el 3:1 exigido).
- Un acento propio muy claro u oscuro **se muestra ajustado** en los textos y botones
  (no el avatar, que usa el color elegido tal cual). Es el compromiso: el color elegido
  sigue mandando mientras sea legible.
- En modo oscuro el rojo de error es más claro y los campos tienen el borde más marcado.

## Alternativas descartadas

- **Repintar los 15 temas a mano en modo Claro:** ya se descartó al introducir la rampa
  (ADR-009); el ajuste automático mantiene un solo mecanismo.
- **Borde de 3:1 en todas partes:** volvería ruidosa cualquier tarjeta; solo se exige donde
  hace falta para ubicar un control.
- **Elegir siempre el texto de botón por contraste WCAG puro:** ya se evitó en el pasado
  (cambiaba el violeta por defecto); aquí solo se cambia cuando el elegido no se lee.
