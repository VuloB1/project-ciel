import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";

import { isRgbTripletLight } from "@/lib/theme/accentTheme";
import { cn } from "@/lib/theme/cn";
import { LIGHT_NEUTRAL_RAMP, THEME_PRESET_IDS, THEME_PRESETS, type ThemePresetId, type ThemeTokens } from "@/lib/theme/presets";
import { applyRoundness, useResolvedColorMode } from "@/lib/theme/useActiveProfileTheme";
import { useSettingsStore } from "@/stores/settingsStore";

/** Vista previa en miniatura de un tema — mini "tarjeta de app" con su
 * propia superficie, dos líneas de contenido simuladas y una píldora de
 * acento, para poder comparar los 15 temas de un vistazo (pedido del
 * usuario, inspirado en la galería de temas de OpChat) en vez de solo un
 * círculo de dos colores.
 *
 * Las esquinas de la tarjeta Y de la píldora de acento usan el radio REAL
 * que ese tema tendría aplicado (`applyRoundness` — el propio del preset
 * mientras el usuario no haya tocado el override global, o el override si
 * ya lo tocó) — así "más redondo o más cuadrado" se ve directamente en la
 * vista previa, no solo se explica (Terminal/Noir/Cyberpunk/Industrial, con
 * radio 0, muestran una píldora totalmente cuadrada en vez de la cápsula
 * redondeada de un tema como Pastel, salvo que el override global ya esté
 * activo). El fondo/texto/borde siguen el mismo criterio de modo claro/
 * oscuro que `useActiveProfileTheme` (rampa clara compartida salvo que el
 * preset ya sea claro por sí mismo), para que la preview coincida con lo
 * que realmente se va a ver al elegir ese tema. */
function ThemeCard({
  id,
  tokens,
  selected,
  resolvedMode,
  roundness,
  label,
  onClick,
}: {
  id: ThemePresetId;
  tokens: ThemeTokens;
  selected: boolean;
  resolvedMode: "dark" | "light";
  roundness: number | null | undefined;
  label: string;
  onClick: () => void;
}) {
  const useLightRamp = resolvedMode === "light" && !isRgbTripletLight(tokens.background);
  const surface = useLightRamp ? LIGHT_NEUTRAL_RAMP.surface : tokens.surface;
  const border = useLightRamp ? LIGHT_NEUTRAL_RAMP.border : tokens.border;
  const textPrimary = useLightRamp ? LIGHT_NEUTRAL_RAMP.textPrimary : tokens.textPrimary;
  const { radiusSm, radiusMd } = applyRoundness(tokens, roundness);

  return (
    <button
      key={id}
      type="button"
      onClick={onClick}
      title={label}
      aria-label={label}
      aria-pressed={selected}
      className={cn(
        "relative flex h-24 flex-col justify-between border-2 p-3 text-left transition-transform hover:scale-[1.02]",
        selected ? "border-accent" : "border-transparent",
      )}
      style={{
        background: `rgb(${surface})`,
        borderRadius: radiusMd,
      }}
    >
      <div className="flex flex-col gap-1.5">
        <span className="block h-1.5 w-3/4 rounded-full" style={{ background: `rgb(${border})` }} />
        <span className="block h-1.5 w-1/2 rounded-full opacity-60" style={{ background: `rgb(${border})` }} />
      </div>
      <div className="flex items-center justify-between gap-2">
        <span className="truncate text-[11px] font-medium" style={{ color: `rgb(${textPrimary})` }}>
          {label}
        </span>
        <span
          className="h-2 w-8 shrink-0"
          style={{ background: `rgb(${tokens.accent})`, borderRadius: radiusSm }}
        />
      </div>
      {selected ? (
        <span className="absolute right-1.5 top-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-accent text-accent-foreground">
          <Check className="h-2.5 w-2.5" strokeWidth={3} />
        </span>
      ) : null}
    </button>
  );
}

/** Grid de vistas previas para elegir el tema visual completo del perfil
 * activo (ADR-009), en la pestaña Identidad > Apariencia — 15 presets
 * fijos, clickear el activo lo deselecciona (vuelve a los defaults de
 * ADR-008). El acento personalizado (override independiente del tema, ver
 * `useActiveProfileTheme`) se maneja aparte, con un switch — no es un
 * preset más de este grid. */
export function ThemePresetPicker({ theme, onThemeChange }: { theme: string | null; onThemeChange: (id: ThemePresetId | null) => void }) {
  const { t } = useTranslation();
  const appearance = useSettingsStore((s) => s.appearance);
  const resolvedMode = useResolvedColorMode(appearance?.colorMode);
  const roundness = appearance?.roundness;

  return (
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
      {THEME_PRESET_IDS.map((id) => {
        const selected = theme === id;
        return (
          <ThemeCard
            key={id}
            id={id}
            tokens={THEME_PRESETS[id]}
            selected={selected}
            resolvedMode={resolvedMode}
            roundness={roundness}
            label={t(`settings.themePresets.${id}`)}
            onClick={() => onThemeChange(selected ? null : id)}
          />
        );
      })}
    </div>
  );
}
