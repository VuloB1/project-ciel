import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { Card } from "@/design-system/Card";
import { Slider } from "@/design-system/Slider";
import { errorMessage } from "@/lib/errorMessage";
import type { AppearanceSettings, ColorMode, MessageDensity } from "@/lib/ipc/settings";
import { cn } from "@/lib/theme/cn";
import { FONT_OPTIONS } from "@/lib/theme/fonts";
import { useSettingsStore } from "@/stores/settingsStore";
import { useToastStore } from "@/stores/toastStore";

const COLOR_MODES: ColorMode[] = ["dark", "light", "system"];
const DENSITIES: MessageDensity[] = ["compact", "normal", "spacious"];

/** Pangrama estándar de tipografía en español — mismo propósito que la
 * frase de muestra de la referencia (OpChat): mostrar de un vistazo cómo
 * se ve el tamaño/fuente elegidos sin depender del contenido real del
 * chat. */
const SAMPLE_SENTENCE = "El veloz murciélago hindú comía feliz cardillo y kiwi.";

/** Preferencias de apariencia GLOBALES de la app (modo de color, redondez,
 * densidad, tamaño y familia de fuente) — a diferencia del tema visual y el
 * acento (por personaje, ahora en Identidad > Apariencia), esto es "cómo
 * quiero usar yo el programa", no "cómo se ve este personaje", así que vive
 * en Configuración junto al resto de los ajustes globales de la app
 * (`settingsStore`, no `profileStore`). Autoguarda en cada cambio, mismo
 * patrón que el resto de Configuración. */
export function AppearanceSettingsSection() {
  const { t } = useTranslation();
  const appearance = useSettingsStore((s) => s.appearance);
  const setAppearance = useSettingsStore((s) => s.setAppearance);
  const pushToast = useToastStore((s) => s.push);

  async function patch(next: Partial<AppearanceSettings>) {
    if (!appearance) return;
    try {
      await setAppearance({ ...appearance, ...next });
    } catch (err) {
      pushToast({ variant: "error", title: t("settings.appearance.saveError"), description: errorMessage(err) });
    }
  }

  if (!appearance) return null;

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card className="flex flex-col gap-4 p-4">
        <div className="flex flex-col gap-2">
          <p className="text-sm font-medium text-text-primary">{t("settings.appearance.modeTitle")}</p>
          <div className="flex gap-2">
            {COLOR_MODES.map((mode) => (
              <Button
                key={mode}
                size="sm"
                variant={appearance.colorMode === mode ? "primary" : "secondary"}
                onClick={() => void patch({ colorMode: mode })}
              >
                {t(`settings.appearance.mode.${mode}`)}
              </Button>
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-2 border-t border-border pt-4">
          <div className="flex items-center justify-between gap-2">
            <p className="text-sm font-medium text-text-primary">{t("settings.appearance.roundnessTitle")}</p>
            {appearance.roundness != null ? (
              <Button size="sm" variant="ghost" onClick={() => void patch({ roundness: null })}>
                {t("settings.appearance.roundnessReset")}
              </Button>
            ) : null}
          </div>
          <p className="text-xs text-text-secondary">
            {appearance.roundness == null ? t("settings.appearance.roundnessAuto") : t("settings.appearance.roundnessCustom")}
          </p>
          <div className="flex items-center gap-3">
            <span className="shrink-0 text-xs text-text-secondary">{t("settings.appearance.roundnessAngular")}</span>
            <Slider
              value={appearance.roundness ?? 100}
              onValueChange={(value) => void patch({ roundness: value })}
              min={0}
              max={200}
              step={10}
              label={t("settings.appearance.roundnessTitle")}
            />
            <span className="shrink-0 text-xs text-text-secondary">{t("settings.appearance.roundnessRounded")}</span>
          </div>
        </div>

        <div className="flex flex-col gap-2 border-t border-border pt-4">
          <p className="text-sm font-medium text-text-primary">{t("settings.appearance.densityTitle")}</p>
          <div className="flex gap-2">
            {DENSITIES.map((density) => (
              <Button
                key={density}
                size="sm"
                variant={appearance.density === density ? "primary" : "secondary"}
                onClick={() => void patch({ density })}
              >
                {t(`settings.appearance.density.${density}`)}
              </Button>
            ))}
          </div>
        </div>

        <div className="flex flex-col gap-2 border-t border-border pt-4">
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-text-primary">{t("settings.appearance.fontSizeTitle")}</p>
            <span className="text-xs tabular-nums text-text-secondary">{appearance.fontScale}%</span>
          </div>
          <Slider
            value={appearance.fontScale}
            onValueChange={(value) => void patch({ fontScale: value })}
            min={80}
            max={140}
            step={5}
            label={t("settings.appearance.fontSizeTitle")}
          />
          <p className="rounded-md border border-border bg-surface-secondary px-3 py-2 text-sm text-text-primary">{SAMPLE_SENTENCE}</p>
        </div>

        <div className="flex flex-col gap-2 border-t border-border pt-4">
          <p className="text-sm font-medium text-text-primary">{t("settings.appearance.fontTitle")}</p>
          <div className="flex max-h-64 flex-col divide-y divide-border overflow-y-auto rounded-md border border-border">
            {FONT_OPTIONS.map((font) => {
              const selected = appearance.fontId === font.id;
              return (
                <button
                  key={font.id}
                  type="button"
                  onClick={() => void patch({ fontId: font.id })}
                  aria-pressed={selected}
                  className={cn(
                    "px-3 py-2 text-left text-sm transition-colors",
                    selected ? "bg-surface-secondary text-accent" : "text-text-primary hover:bg-surface-secondary",
                  )}
                  style={{ fontFamily: font.cssFamily.includes(" ") ? `"${font.cssFamily}"` : font.cssFamily }}
                >
                  {font.label}
                </button>
              );
            })}
          </div>
        </div>
      </Card>
    </div>
  );
}
