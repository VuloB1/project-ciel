import type { IconComponent } from "@/design-system/PlatformIcons";
import { cn } from "@/lib/theme/cn";

export interface MenuButtonItem {
  key: string;
  icon: IconComponent;
  label: string;
  disabled?: boolean;
  onSelect: () => void;
}

/** Fila de botones de un menú tipo contexto — el mismo look en
 * `AppContextMenu.tsx` (overlay dentro de la ventana principal) y
 * `TrayMenuWindow.tsx` (ventana propia junto al ícono de la bandeja), que
 * necesitan el mismo aspecto pero viven en documentos separados y no pueden
 * compartir el contenedor posicionado (`fixed`/portal vs. ventana propia). */
export function MenuButtonList({ items }: { items: MenuButtonItem[] }) {
  return (
    <>
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <button
            key={item.key}
            type="button"
            disabled={item.disabled}
            onClick={item.onSelect}
            className={cn(
              "flex w-full items-center gap-2.5 rounded-sm px-2.5 py-1.5 text-left text-sm text-text-primary outline-none transition-colors",
              "hover:bg-surface disabled:pointer-events-none disabled:opacity-50",
            )}
          >
            <Icon className="h-4 w-4 shrink-0" />
            <span className="truncate">{item.label}</span>
          </button>
        );
      })}
    </>
  );
}
