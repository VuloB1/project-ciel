# ADR-009: Theme Presets — 15 temas completos seleccionables por personaje

## Estado

Aceptado e implementado.

## Contexto

El ThemeEngine (ADR-008) dejó que cada perfil tiñera el acento de la app
(`accent`/`accentSecondary`/`accentForeground`) vía 3 variables CSS
derivadas de un `accentColor` hex libre. El usuario pidió después ir más
lejos: personalizar mucho más el aspecto de la app, con "temas o diseños
diferentes" que le dieran personalidad propia a cada personaje. Terminamos
diseñando y aprobando 15 direcciones visuales completas como mockups
estáticos (`.dc.html`): Cozy, Cyberpunk, Minimal, Pastel, Luxury,
Vaporwave, Forest, Industrial, Ocean, Sakura, Terminal, Noir, Autumn,
Royal y Paper — y el pedido final fue meterlas todas a la app real como
opciones que cada personaje pueda elegir e ir cambiando.

Explorando el código se confirmó que `globals.css`/`tailwind.config.js`
ya exponían no solo el acento sino **toda la paleta**
(`background`/`surface`/`surface-secondary`/`border`/`text-primary`/
`text-secondary`) y **los 3 radios** (`rounded-sm/md/lg` →
`var(--radius-sm/md/lg)`) como variables CSS — el mismo mecanismo de
ADR-008 alcanzaba para un tema completo, no solo el acento, sin tocar
componente por componente.

## Decisión

- **Nuevo campo `theme` (nullable) en `profiles`**, mismo patrón que
  `accentColor`: migración aditiva (`ALTER TABLE profiles ADD COLUMN
  theme TEXT`), threading idéntico por Rust/TS/IPC.
- **12 variables CSS en vez de 3**: `useActiveProfileTheme` reemplaza a
  `useActiveProfileAccent` y escribe las 9 de paleta + 3 de radios en
  `document.documentElement` en cada cambio de perfil activo — siempre
  las 12, nunca un subconjunto, para que cambiar de un perfil temático a
  uno sin tema no deje valores pegados del anterior.
- **15 presets fijos, hand-authored, no derivados**: `src/lib/theme/
  presets.ts` define `THEME_PRESETS`, un `Record<ThemePresetId,
  ThemeTokens>` con los 9 colores + 3 radios de cada tema, extraídos
  directamente de los mockups `.dc.html` ya aprobados. A diferencia del
  acento (3 tokens derivados de 1 hex vía funciones puras), acá no hay
  fórmula razonable para derivar "toda una paleta + radios" de una sola
  entrada del usuario — de ahí el catálogo fijo en vez de un generador.
- **`theme: null` = "Personalizado", el comportamiento de ADR-008 sin
  cambios**: paleta/radios por defecto (mismos literales de `:root`) +
  el acento derivado de `accentColor` vía `computeAccentTheme`. Ningún
  perfil existente cambia de aspecto a menos que el usuario elija un
  preset explícitamente.
- **Alcance deliberadamente acotado a paleta+radios** — no se replican de
  los mockups: fuentes custom (Luxury/Royal), texturas/efectos de fondo
  (scanlines, grid), flourishes SVG decorativos, ni los tratamientos
  bespoke del ítem de nav activo por tema (todos los temas siguen usando
  el mismo `text-accent` de ADR-008). `success`/`warning`/`error` quedan
  constantes en los 15 — son colores funcionales, no de marca.
- **La UI de Identidad pasa de un color picker a un grid de 16 swatches**
  (`ThemePresetPicker`, nuevo componente): 15 presets + "Personalizado",
  que revela el `<input type="color">` existente solo cuando está
  seleccionado — nadie con un `accentColor` propio pierde esa opción.

## Razones

- Reusar el mecanismo de ADR-008 (escribir CSS vars en runtime) en vez de
  inventar uno nuevo evita duplicar infraestructura — la única diferencia
  real es *cuántas* variables se escriben.
- Un catálogo fijo de paletas hand-authored, no una fórmula, porque no
  existe una forma razonable de generar "una identidad visual completa"
  (fondo, superficies, bordes, texto, acento, radios, coherentes entre
  sí) a partir de una sola entrada de usuario — a diferencia del acento,
  donde HSL/YIQ sí alcanzaban para derivar 3 tokens coherentes de un hex.
- Cortar el alcance en paleta+radios (no fuentes/texturas/flourishes/
  tratamientos bespoke por tema) es lo que hace viable ofrecer 15
  direcciones reales en vez de una — replicar cada detalle visual de los
  15 mockups 1:1 multiplicaría el costo por 15 para un beneficio marginal
  frente a la paleta, que ya carga la mayor parte de la "personalidad" de
  cada tema.
- Mantener `theme: null` como default explícito (no forzar un preset a
  todo perfil existente) preserva la garantía de compatibilidad que ya
  estableció ADR-008: nadie ve un cambio de aspecto sin pedirlo.

## Consecuencias

- Los 15 presets en la app real no son pixel-idénticos a sus mockups —
  les falta tipografía custom, texturas, flourishes y los tratamientos
  bespoke del nav activo. Si el usuario quiere cerrar esa brecha más
  adelante, es una ampliación de alcance nueva, no un bug de esta
  implementación.
- Un puñado de clases `rounded-full`/`rounded-xl` hardcodeadas fuera de
  la escala `sm/md/lg` (avatares circulares, algún badge) no siguen el
  radio del tema — son casos ya redondos/píldora donde tiene sentido que
  no cambien.
- Agregar un preset 16 a futuro es editar `presets.ts` y los i18n, sin
  tocar el schema (`theme` sigue siendo un `string` suelto, no un enum
  validado en el borde) — pero también significa que un `theme` guardado
  puede quedar "huérfano" si un preset se elimina; `useActiveProfileTheme`
  lo cubre cayendo a "Personalizado" en vez de romper.
