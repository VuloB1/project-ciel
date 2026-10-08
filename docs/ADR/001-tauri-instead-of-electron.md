# ADR-001: Tauri 2 en vez de Electron

## Estado

Aceptado.

## Contexto

AIVT Companion necesita una app de escritorio multiplataforma (Windows,
Linux, macOS ahora; Android/iOS potencialmente después) que sea ligera,
capaz de correr modelos/servicios locales sin ataduras a un proveedor, y
con una superficie de ataque pequeña dado que maneja credenciales
sensibles (API keys, tokens de Twitch/Discord).

## Decisión

Usar **Tauri 2** con un frontend React/TypeScript y un backend Rust, en vez
de Electron (Chromium + Node.js embebidos).

## Razones

- **Tamaño y consumo de recursos**: Tauri usa el motor de renderizado nativo
  del SO (WebView2 en Windows, WebKit en macOS/Linux) en vez de empaquetar
  Chromium completo — binarios e instalación significativamente más
  pequeños, menor uso de RAM en reposo. Esto importa directamente para el
  requisito de "aplicación ligera" (sección 2 del brief original).
- **Backend en Rust, no Node.js**: nos da errores tipados, un modelo de
  concurrencia más predecible para el pipeline de audio en tiempo real
  (Fase 6), y acceso directo a crates maduras para audio (`cpal`, más
  adelante `songbird` para Discord Voice) sin depender de bindings nativos
  de Node.
- **Seguridad por defecto**: el sistema de capabilities/ACL de Tauri v2
  obliga a declarar explícitamente qué puede hacer el frontend (qué
  comandos, qué plugins) en vez de exponer todo el runtime de Node al
  contexto de renderizado, como hace Electron por defecto.
- **Multiplataforma real hacia mobile**: Tauri 2 compila a Android/iOS desde
  la misma base de código de frontend, lo cual encaja con la sección 115
  del brief (arquitectura preparada para mobile, sin implementarlo todavía).

## Consecuencias

- El equipo necesita un toolchain de Rust instalado además de Node — mayor
  fricción de setup inicial que un proyecto Electron puro.
- Lógica de integración con SO (autostart, keyring, ventanas) pasa por
  plugins oficiales de Tauri (`tauri-plugin-autostart`,
  `tauri-plugin-single-instance`) en vez de APIs de Node — API de superficie
  distinta a la que muchos desarrolladores web ya conocen.
- Todo el estado sensible (DB, secretos, providers) vive en el proceso Rust,
  nunca en el WebView — refuerza la regla de "el frontend no contiene
  lógica sensible" (sección 4 del brief).
