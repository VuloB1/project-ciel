# ADR-008: ThemeEngine — acento dinámico por personaje

## Estado

Aceptado e implementado.

## Contexto

Último pendiente de la ronda de recomendaciones de interfaz de Grok, y el
único ítem que quedaba del roadmap original de Fase 11 — `src/globals.css`
ya traía un comentario propio marcándolo como diferido ("a switchable/
custom ThemeEngine ... is Fase 11 (Polish), not before").

`profile.accentColor` ya existía en el schema desde Fase 0, pero solo se
usaba en 2 lugares: el fondo del avatar-fallback y el color picker de
Identidad. En todos los demás lugares (botones, switches, sliders,
anillos de foco, links) el acento era un valor **estático**, ya cableado
en cada componente vía clases Tailwind (`bg-accent`, `text-accent`,
`ring-accent`) que resuelven a la variable CSS `--color-accent`.

## Decisión

- **Una sola variable CSS alimentada en runtime**, no un rediseño
  componente por componente: `useActiveProfileAccent` (nuevo hook, llamado
  una vez desde `App.tsx`) escribe `--color-accent`/`-secondary`/
  `-foreground` en `document.documentElement` cada vez que cambia el
  `accentColor` del perfil activo. Como esas tres variables ya eran
  consumidas por Tailwind en todo el árbol de componentes, esto alcanza
  para que botones, switches, sliders, foco y links seguí el color del
  personaje sin tocarlos uno por uno.
- **Tres tokens derivados de un solo hex**, no tres campos guardados: el
  usuario solo elige un color (`accentColor`, ya existente); `accent`
  (base), `accentSecondary` (aclarado ~12% en HSL, para hover) y
  `accentForeground` (blanco o casi-negro, elegido por contraste real vía
  luminancia relativa WCAG) se calculan con funciones puras en
  `src/lib/theme/accentTheme.ts`.
- **`accentForeground` existe porque un color elegido por el usuario puede
  ser claro** (el cyan real de un perfil de esta sesión, `#00eeff`) —
  `Button.tsx`'s variante `primary` y la burbuja de usuario en
  `MessageBubble.tsx` hardcodeaban `text-white` sobre `bg-accent`, lo que
  se vuelve ilegible con un acento claro. Se calcula por contraste real
  (blanco vs. casi-negro, el que dé más contraste), no por un umbral fijo
  de brillo.
- **Ausencia de `accentColor` (o ningún perfil activo) = el violeta de
  siempre** (`DEFAULT_ACCENT_HEX`, consolidado en un solo lugar en vez de
  estar duplicado en `ProfileAvatarBadge.tsx` e `IdentityEditor.tsx`) —
  nadie ve un cambio a menos que haya elegido un color propio.
- **Un solo toque manual, no automatizado**: el ítem activo del sidebar
  pasa de `text-text-primary` a `text-accent` — el único gap real de
  "acento en estado activo" que se identificó revisando el código. No se
  agregaron bordes ni glows nuevos en ningún otro lugar.

## Razones

- Aprovechar que el sistema de diseño ya usaba variables CSS (no colores
  hardcodeados por componente) es lo que hace que esto sea un cambio
  chico con un efecto grande — la alternativa (tocar cada componente) ya
  no haría falta y además sería más frágil a futuro.
- Derivar los 3 tokens de un solo campo evita pedirle al usuario que
  elija 3 colores coordinados, y evita guardar datos derivados que
  podrían desincronizarse del campo base.
- Contraste por luminancia real (no un umbral ad-hoc) es la única forma
  de que esto funcione para *cualquier* color que un usuario elija, no
  solo los que el autor haya probado a mano.

## Consecuencias

- Modo claro, fondos personalizables y "estilos visuales" por personaje
  siguen fuera de alcance — quedan igual de pendientes que antes, esto
  solo resuelve el acento.
- Sin vista previa en vivo mientras se edita el color en Identidad antes
  de guardar — se actualiza cuando el perfil se recarga, igual que
  cualquier otro campo del perfil mostrado en la app.
- Sin motor de contraste general — el swap blanco/casi-negro cubre los 2
  puntos reales de riesgo (texto directamente sobre un fondo sólido de
  acento), no es una auditoría de accesibilidad completa.
