import { useTranslation } from "react-i18next";

import { cn } from "@/lib/theme/cn";
import { useCallStore } from "@/stores/callStore";

/** Barra con el volumen actual del micrófono. Lee el nivel directamente del
 * store (el evento llega unas 15 veces por segundo) para que solo se vuelva a
 * dibujar esta barra y no toda la página. */
export function MicMeter({ muted = false, className }: { muted?: boolean; className?: string }) {
  const { t } = useTranslation();
  const level = useCallStore((s) => s.micLevel);
  const shown = muted ? 0 : level;

  return (
    <div
      role="meter"
      aria-label={t("call.micLevel")}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(shown * 100)}
      className={cn("h-2 w-full overflow-hidden rounded-full bg-surface-secondary", className)}
    >
      <div
        className={cn("h-full rounded-full transition-[width] duration-100 ease-out", shown > 0.9 ? "bg-warning" : "bg-accent")}
        style={{ width: `${shown * 100}%` }}
      />
    </div>
  );
}
