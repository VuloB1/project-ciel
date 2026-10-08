import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/lib/i18n";
import "@/lib/theme/fonts";
import "./globals.css";

import { ErrorBoundary } from "./app/ErrorBoundary";
import { TrayMenuWindow } from "./app/TrayMenuWindow";

// `globals.css`'s `body { background-color: rgb(var(--color-background)) }`
// es opaco a propósito para la ventana principal — acá anularía la
// transparencia de la ventana `tray-menu` (config en `tauri.conf.json`) y
// se vería un rectángulo sólido en vez de la tarjeta flotante con esquinas
// redondeadas. Estilo en línea (no una regla CSS con más especificidad):
// gana siempre sin importar el orden en que Vite inyecta la hoja de
// estilos, mismo motivo que el fix de `pointerEvents` en
// `AppContextMenu.tsx`.
document.documentElement.style.background = "transparent";
document.body.style.background = "transparent";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("Root element not found");
}

createRoot(rootElement).render(
  <StrictMode>
    <ErrorBoundary>
      <TrayMenuWindow />
    </ErrorBoundary>
  </StrictMode>,
);
