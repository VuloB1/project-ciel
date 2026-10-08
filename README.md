# Project Ciel

> Nombre interno del código y de los crates: `aivt-*` (el producto se renombró a
> Project Ciel; el identificador de la app es `com.projectciel.app`).

Aplicación de escritorio (Windows / Linux / macOS, con arquitectura preparada
para Android/iOS más adelante) para crear y utilizar IAs de compañía con
personalidad propia — chat, llamada de voz, Twitch, Discord y avatar, todo
representando al **mismo personaje** seleccionado. Ver
[docs/architecture.md](docs/architecture.md) para el porqué de cada decisión.

Estado actual: las fases 0 a 9 y 11 están completas (chat, memoria, voz,
llamada, avatar, Twitch, Discord y pulido), junto con varios proveedores de IA y
de voz con cambio automático, dictado por voz y exportación/importación completa
de perfiles. La voz por Discord (fase 10) se intentó y se abandonó. Ver
[docs/DEVELOPMENT_STATUS.md](docs/DEVELOPMENT_STATUS.md) para el detalle de qué
funciona hoy, qué falta por verificar con claves reales y qué está pendiente (por
ejemplo, la personalización del instalador).

## Stack

- Frontend: React + TypeScript (strict) + Vite + Zustand + TailwindCSS + Radix UI.
- Backend/core: Rust vía Tauri 2, workspace de crates por dominio.
- Persistencia: SQLite (`sqlx`, sin ORM completo) con migraciones versionadas.

## Requisitos

- [Node.js](https://nodejs.org/) 20+ y npm.
- [Rust](https://rustup.rs/) (toolchain estable) — necesario para compilar el
  backend de Tauri. Instálalo con `rustup` antes del primer `npm run tauri dev`.
- [CMake](https://cmake.org/download/) y [LLVM](https://github.com/llvm/llvm-project/releases)
  (que provee `libclang`) — necesarios para compilar `whisper-rs`
  (`aivt-provider-stt`, transcripción de voz embebida vía whisper.cpp).
  Instálalos (en Windows, `winget install Kitware.CMake` y
  `winget install LLVM.LLVM`) y asegúrate de que `cmake` esté en el PATH
  antes de compilar. Solo se necesitan para compilar la app — el ejecutable
  final no depende de ellos.
- [Vulkan SDK](https://vulkan.lunarg.com/sdk/home) — necesario para compilar
  el feature `vulkan` de `whisper-rs` (aceleración por GPU opcional de la
  transcripción, activable en Configuración > Transcripción de voz; CPU
  sigue siendo el default). En Windows, `winget install KhronosGroup.VulkanSDK`
  y asegúrate de que la variable de entorno `VULKAN_SDK` apunte a la carpeta
  instalada (ej. `C:\VulkanSDK\<versión>`).
  **Nota Windows**: si la compilación falla con muchos errores
  `FileTracker: could not create the new file tracking log file`, es el
  límite de longitud de ruta de Windows (260 caracteres) chocando con las
  rutas profundas que genera la compilación de los shaders de Vulkan — no
  es un error de código. Se soluciona compilando en una ruta corta:
  `$env:CARGO_TARGET_DIR = "C:\cgt-aivt"` (PowerShell) antes de `cargo
  build`/`npm run tauri dev`. No se guardó como config del repo porque es
  una ruta absoluta de Windows y rompería builds en Linux/macOS.
- En Linux, además: dependencias nativas de Tauri (webkit2gtk, etc. — ver la
  [guía de prerequisitos de Tauri](https://v2.tauri.app/start/prerequisites/))
  y un backend de Secret Service en ejecución para que `SecretStore`
  funcione (ver [docs/security.md](docs/security.md)).

## Desarrollo

```bash
npm install
npm run tauri dev
```

Solo frontend (sin backend Tauri — útil para iterar UI rápido, pero
`invoke()`/eventos fallarán de forma controlada):

```bash
npm run dev
```

## Comandos útiles

| Comando | Qué hace |
|---|---|
| `npm run tauri dev` | App completa (frontend + backend Rust) en modo desarrollo. |
| `npm run dev` | Solo el servidor Vite, sin el backend Tauri. |
| `npm run build` | Type-check (`tsc -b`) + build de producción del frontend. |
| `npm run test` | Suite de tests del frontend (Vitest). |
| `npm run lint` | Type-check sin build. |
| `cargo test --workspace` | Suite de tests del backend (todos los crates). Requiere Rust instalado. |
| `cargo build --workspace` | Compila todo el backend sin empaquetar la app. |

## Estructura del proyecto

Ver [docs/architecture.md](docs/architecture.md) §4 para la estructura de
carpetas completa y el razonamiento detrás de ella (workspace de Cargo con
un crate por módulo de dominio, frontend organizado por feature).

## Documentación

- [docs/architecture.md](docs/architecture.md) — arquitectura, stack, riesgos, fases.
- [docs/database.md](docs/database.md) — esquema de base de datos y convenciones de migraciones.
- [docs/security.md](docs/security.md) — manejo de secretos, redacción de logs, privacidad.
- [docs/ADR/](docs/ADR/) — decisiones arquitectónicas registradas (proveedores y cambio automático: ADR-010; dictado por voz: ADR-011; formato de perfiles: ADR-012; alcance de la memoria entre canales: ADR-013; copia de seguridad: ADR-014; centro de diagnóstico: ADR-015).
- [docs/DEVELOPMENT_STATUS.md](docs/DEVELOPMENT_STATUS.md) — qué está completado, en progreso y pendiente.
