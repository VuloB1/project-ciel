import { AppWindow, Power, RefreshCw } from "lucide-react";
import { useEffect, useLayoutEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { MenuButtonList } from "@/design-system/MenuButtonList";
import { hideCurrentWindow, quitApp, reloadMainWindow, resizeMenuWindow, showMainWindow } from "@/lib/ipc/window";
import { useActiveProfileTheme } from "@/lib/theme/useActiveProfileTheme";
import { useAppearanceSettings } from "@/lib/theme/useAppearanceSettings";
import { useProfileStore } from "@/stores/profileStore";
import { useSettingsStore } from "@/stores/settingsStore";

/** Ventana propia sin bordes para el clic derecho en el ícono de la bandeja
 * — reemplaza el menú nativo del SO (que no puede seguir los colores,
 * redondez ni densidad del tema de la app) con el mismo aspecto que
 * `AppContextMenu.tsx`. Rust (`lib.rs`'s `TrayIconBuilder`) la posiciona
 * junto al ícono y la muestra/enfoca en cada clic derecho, y la esconde
 * sola al perder el foco — acá solo hace falta cargar perfil/apariencia
 * para pintar el tema correcto y ofrecer las acciones. Es una ventana
 * Tauri separada (no un overlay dentro de `main`) porque la bandeja vive
 * fuera del webview de la ventana principal. */
export function TrayMenuWindow() {
  const { t } = useTranslation();
  useActiveProfileTheme();
  useAppearanceSettings();

  const loadSettings = useSettingsStore((s) => s.load);
  const loadProfiles = useProfileStore((s) => s.load);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void loadSettings();
    void loadProfiles();
  }, [loadSettings, loadProfiles]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") void hideCurrentWindow();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Alto de la ventana atado al contenido real en vez de un valor fijo en
  // `tauri.conf.json` — un `ResizeObserver` (no solo un efecto al montar)
  // porque el alto también puede cambiar después del primer render: más
  // ítems en este mismo menú más adelante, o un cambio de densidad/escala
  // de fuente en Apariencia que agranda cada botón.
  useLayoutEffect(() => {
    const node = containerRef.current;
    if (!node) return;
    const observer = new ResizeObserver(() => {
      void resizeMenuWindow(node.offsetHeight);
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={containerRef}
      className="flex w-screen flex-col overflow-hidden rounded-md border border-border bg-surface-secondary p-1 shadow-md"
      onContextMenu={(event) => event.preventDefault()}
    >
      <MenuButtonList
        items={[
          { key: "show", icon: AppWindow, label: t("trayMenu.show"), onSelect: () => void showMainWindow() },
          { key: "reload", icon: RefreshCw, label: t("trayMenu.reload"), onSelect: () => void reloadMainWindow() },
          { key: "quit", icon: Power, label: t("trayMenu.quit"), onSelect: () => void quitApp() },
        ]}
      />
    </div>
  );
}
