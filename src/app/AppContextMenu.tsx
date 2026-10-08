import { ClipboardCopy, ClipboardPaste, Copy, PartyPopper, Scissors, SearchCode, Settings as SettingsIcon, TextSelect } from "lucide-react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import { MenuButtonList } from "@/design-system/MenuButtonList";
import type { IconComponent } from "@/design-system/PlatformIcons";
import { openDevtools } from "@/lib/ipc/settings";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSettingsStore } from "@/stores/settingsStore";

interface MenuItem {
  key: string;
  icon: IconComponent;
  label: string;
  disabled?: boolean;
  run: () => void | Promise<void>;
}

interface MenuState {
  x: number;
  y: number;
  editable: boolean;
  hasSelection: boolean;
  /** Texto crudo de `message.content` cuando el click cae dentro de una
   * burbuja de chat (`data-copy-text` en `MessageBubble.tsx`) — permite
   * copiar el mensaje completo aunque no haya nada seleccionado. */
  messageText: string | null;
  /** Shift+click derecho agrega "Inspeccionar" al final del mismo menú
   * propio, en vez de reemplazarlo por el menú nativo del webview. */
  shiftKey: boolean;
}

const MENU_WIDTH = 208;
const ITEM_HEIGHT = 34;
const VIEWPORT_MARGIN = 8;

function hasActiveSelection(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
    return target.selectionStart !== target.selectionEnd;
  }
  return (window.getSelection()?.toString().length ?? 0) > 0;
}

/** Reemplaza el menú contextual nativo del webview (con entradas de
 * navegador tipo "Atrás"/"Ver código fuente"/"Inspeccionar") por uno propio,
 * acotado a lo que sirve dentro de la app. Un único listener en `window` en
 * vez de envolver el árbol con un trigger posicional: los diálogos de
 * Configuración y el Command Palette se montan en un Portal fuera del árbol
 * principal (`document.body`), así que un trigger anidado en algún punto de
 * `App.tsx` nunca los cubriría — esto sí, sin importar dónde caiga el click. */
export function AppContextMenu() {
  const { t } = useTranslation();
  const openSettings = useNavigationStore((s) => s.openSettings);
  const [menu, setMenu] = useState<MenuState | null>(null);

  useEffect(() => {
    function handleContextMenu(event: MouseEvent) {
      event.preventDefault();
      const target = event.target as HTMLElement | null;
      const editable = target?.closest("input, textarea, [contenteditable='true'], [contenteditable='']") != null;
      const messageText = target?.closest<HTMLElement>("[data-copy-text]")?.dataset.copyText ?? null;
      setMenu({
        x: event.clientX,
        y: event.clientY,
        editable,
        hasSelection: hasActiveSelection(event.target),
        messageText,
        shiftKey: event.shiftKey,
      });
    }
    function dismiss() {
      setMenu(null);
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setMenu(null);
    }
    window.addEventListener("contextmenu", handleContextMenu);
    window.addEventListener("click", dismiss);
    window.addEventListener("blur", dismiss);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("contextmenu", handleContextMenu);
      window.removeEventListener("click", dismiss);
      window.removeEventListener("blur", dismiss);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  if (!menu) return null;

  function runAndClose(run: MenuItem["run"]) {
    void run();
    setMenu(null);
  }

  // `execCommand("paste")` está bloqueado en Chromium por seguridad desde
  // hace años (lectura de portapapeles vía script) — se reemplaza por la
  // Clipboard API async + `insertText` (que sí sigue soportado) para
  // insertar en la posición del cursor sin perder la selección previa.
  async function pasteFromClipboard() {
    const text = await navigator.clipboard.readText().catch(() => "");
    if (text) document.execCommand("insertText", false, text);
  }

  const baseItems: MenuItem[] = menu.editable
    ? [
        { key: "cut", icon: Scissors, label: t("contextMenu.cut"), disabled: !menu.hasSelection, run: () => void document.execCommand("cut") },
        { key: "copy", icon: Copy, label: t("contextMenu.copy"), disabled: !menu.hasSelection, run: () => void document.execCommand("copy") },
        { key: "paste", icon: ClipboardPaste, label: t("contextMenu.paste"), run: pasteFromClipboard },
        { key: "selectAll", icon: TextSelect, label: t("contextMenu.selectAll"), run: () => void document.execCommand("selectAll") },
      ]
    : (() => {
        const messageText = menu.messageText;
        return [
          ...(menu.hasSelection
            ? [{ key: "copy", icon: Copy, label: t("contextMenu.copy"), run: () => void document.execCommand("copy") }]
            : []),
          ...(messageText
            ? [{ key: "copyMessage", icon: ClipboardCopy, label: t("contextMenu.copyMessage"), run: () => navigator.clipboard.writeText(messageText) }]
            : []),
          { key: "settings", icon: SettingsIcon, label: t("contextMenu.settings"), run: () => openSettings() },
        ];
      })();

  // Shift+click derecho no cambia qué menú se muestra — solo le agrega
  // "Inspeccionar" al final, vía de escape para depurar sin perder el resto
  // de las opciones curadas ni caer al menú nativo del webview.
  const items: MenuItem[] = menu.shiftKey
    ? [
        ...baseItems,
        { key: "inspect", icon: SearchCode, label: t("contextMenu.inspect"), run: () => void openDevtools() },
        // TEMPORAL — pedido puntual del usuario para poder abrir la pantalla
        // de bienvenida y probarla sin tocar la consola. No persiste nada:
        // solo fuerza `onboardingCompleted: false` en memoria, así que un
        // reinicio de la app vuelve a mostrar el layout normal si ya la
        // había completado antes. Sacar este item cuando ya no haga falta.
        {
          key: "previewWelcome",
          icon: PartyPopper,
          label: "Ver pantalla de bienvenida (test)",
          run: () => useSettingsStore.setState({ onboardingCompleted: false }),
        },
      ]
    : baseItems;

  // El menú nunca se calcula fuera de la ventana: se recorta contra el
  // borde derecho/inferior en vez de dejar que el navegador lo desborde.
  const menuHeight = items.length * ITEM_HEIGHT + 8;
  const left = Math.min(menu.x, window.innerWidth - MENU_WIDTH - VIEWPORT_MARGIN);
  const top = Math.min(menu.y, window.innerHeight - menuHeight - VIEWPORT_MARGIN);

  // Portal a `document.body` en vez de renderizar inline donde está montado
  // `AppContextMenu` (la raíz de `App.tsx`, junto al resto de la app) — igual
  // que hacen `Dialog`/`DropdownMenu`/`Popover`/`Select`/`Tooltip` en este
  // mismo proyecto. Sin esto, un right-click con Configuración (o cualquier
  // otro diálogo modal) abierto quedaba VISUALMENTE TAPADO por el overlay
  // oscuro del diálogo (mismo z-50, pero el diálogo se monta después en el
  // DOM y gana el empate) y, encima, Radix marca como `aria-hidden` todo lo
  // que esté fuera de su propio Portal mientras el modal está abierto — un
  // nodo portado recién al abrir el menú se crea DESPUÉS de ese barrido y
  // no queda afectado.
  return createPortal(
    <div
      // `pointerEvents: "auto"` inline (no solo la clase Tailwind): Radix's
      // `DismissableLayer` pone `document.body.style.pointerEvents = "none"`
      // mientras cualquier diálogo modal (Configuración, Command Palette...)
      // está abierto, y solo reactiva `auto` en SUS PROPIOS nodos
      // (Overlay/Content) vía estilo en línea — sin esto el menú se veía
      // bien (ya portado a `document.body`) pero heredaba el `none` del
      // body y ningún botón respondía al click. Tiene que ser un estilo en
      // línea (mayor especificidad que cualquier clase) para ganarle
      // siempre a ese `none`, sin importar el orden de las hojas de estilo.
      className="animate-pop-in fixed z-50 overflow-hidden rounded-md border border-border bg-surface-secondary p-1 shadow-md"
      style={{ left, top, width: MENU_WIDTH, pointerEvents: "auto" }}
      onContextMenu={(event) => event.preventDefault()}
      // El menú vive en un Portal fuera del diálogo de Configuración, así que
      // para Radix un clic acá cuenta como "clic afuera" y cierra el diálogo
      // (`DismissableLayer` escucha `pointerdown` en `document`). Cortar la
      // propagación acá evita que le llegue. Y `preventDefault` en `mousedown`
      // evita que el botón robe el foco del campo: sin foco en el input,
      // "Pegar"/"Cortar"/"Copiar" no tendrían sobre qué actuar.
      onPointerDown={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.preventDefault()}
    >
      <MenuButtonList items={items.map((item) => ({ ...item, onSelect: () => runAndClose(item.run) }))} />
    </div>,
    document.body,
  );
}
