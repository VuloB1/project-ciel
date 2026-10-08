import path from "node:path";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri expects a fixed dev server port and relative asset paths.
// See: https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      // Segunda entrada para `tray-menu` (ver `tauri.conf.json`'s `windows`)
      // — sin esto el build de producción solo empaqueta `index.html` y la
      // ventanita del clic derecho en la bandeja quedaría rota fuera de
      // `npm run dev` (el dev server de Vite sirve cualquier `.html` del
      // proyecto sin necesitar esto).
      input: {
        main: path.resolve(import.meta.dirname, "index.html"),
        trayMenu: path.resolve(import.meta.dirname, "tray-menu.html"),
      },
    },
  },
});
