import { useEffect } from "react";

import { useSettingsStore } from "@/stores/settingsStore";

import { resolveFontFamily } from "./fonts";

/** Aplica las preferencias de apariencia GLOBALES que no dependen del
 * personaje activo (a diferencia de `useActiveProfileTheme`, que sigue
 * resolviendo tema/acento por perfil): tamaño y familia de fuente
 * (`--font-scale`/`--font-family-app`, consumidas por `tailwind.config.js`)
 * y densidad de mensajes (`data-density`, consumido por los bloques
 * `[data-density="..."]` de `globals.css`). Se llama junto a
 * `useActiveProfileTheme()` en `App.tsx`. */
export function useAppearanceSettings(): void {
  const appearance = useSettingsStore((s) => s.appearance);
  const fontScale = appearance?.fontScale ?? 100;
  const fontId = appearance?.fontId ?? "inter";
  const density = appearance?.density ?? "normal";

  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--font-scale", String(fontScale / 100));
    root.style.setProperty("--font-family-app", resolveFontFamily(fontId));
    root.dataset.density = density;
  }, [fontScale, fontId, density]);
}
