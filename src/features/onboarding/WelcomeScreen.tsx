import { Check } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/design-system/Button";
import { cn } from "@/lib/theme/cn";
import { useSettingsStore } from "@/stores/settingsStore";

import { AiProvidersSettings } from "@/features/settings/AiProvidersSettings";
import { AppearanceSettingsSection } from "@/features/settings/AppearanceSettings";
import { SttSettings } from "@/features/settings/SttSettings";
import { TtsSettings } from "@/features/settings/TtsSettings";

import { CreateCharacterStep } from "./CreateCharacterStep";
import { LanguageStep } from "./LanguageStep";
import { UserNameStep } from "./UserNameStep";

const STEPS = ["intro", "language", "appearance", "aiProvider", "sttVoice", "ttsVoice", "userName", "character", "finish"] as const;
type StepKey = (typeof STEPS)[number];

interface StepHelpers {
  onAdvance: () => void;
  /** Deja que el paso activo se "enganche" al botón "Continuar" del pie:
   * si registra una función, "Continuar" la espera (y se queda en el paso
   * si tira error) antes de avanzar, en vez de avanzar directo. Pensado
   * para `CreateCharacterStep` — antes tenía su propio botón "Crear
   * personaje" aparte, que quedaba pisado por "Continuar" (pedido del
   * usuario: "le he dado a continuar ignorando el botón ese" — el
   * personaje nunca se guardaba). `null` desregistra (vuelve a avanzar
   * directo), que es lo que hace cada paso al desmontarse. */
  registerSubmit: (fn: (() => Promise<void>) | null) => void;
}

/** `appearance`/`aiProvider`/`sttVoice`/`ttsVoice` reusan tal cual los
 * componentes ya existentes de Configuración (autocontenidos, autoguardan)
 * — este wizard nunca duplica esos formularios, solo les agrega un
 * encabezado corto propio del paso. */
function renderStep(step: StepKey, helpers: StepHelpers): ReactNode {
  switch (step) {
    case "language":
      return <LanguageStep onAdvance={helpers.onAdvance} />;
    case "appearance":
      return <AppearanceSettingsSection />;
    case "aiProvider":
      return <AiProvidersSettings />;
    case "sttVoice":
      return <SttSettings />;
    case "ttsVoice":
      return <TtsSettings />;
    case "userName":
      return <UserNameStep registerSubmit={helpers.registerSubmit} />;
    case "character":
      return <CreateCharacterStep registerSubmit={helpers.registerSubmit} />;
    default:
      return null;
  }
}

export function WelcomeScreen() {
  const { t } = useTranslation();
  const completeOnboarding = useSettingsStore((s) => s.completeOnboarding);
  const [stepIndex, setStepIndex] = useState(0);
  // Qué lado de la pantalla "entra" el paso siguiente — Continuar desliza
  // desde la derecha, Atrás desde la izquierda, como pasar páginas.
  const [direction, setDirection] = useState<"forward" | "backward">("forward");
  // `goNext`/`goBack` always clamp `stepIndex` inside `[0, STEPS.length)`,
  // so this index is never actually out of range — the fallback only
  // satisfies `noUncheckedIndexedAccess`.
  const step: StepKey = STEPS[stepIndex] ?? "intro";
  // Lo que el paso activo registró vía `registerSubmit` — ver el doc
  // comment de `StepHelpers`. Un ref (no state) porque escribirlo nunca
  // debe re-renderizar este componente por sí solo.
  const submitRef = useRef<(() => Promise<void>) | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const goNext = () => {
    setDirection("forward");
    setStepIndex((i) => Math.min(i + 1, STEPS.length - 1));
  };
  const goBack = () => {
    setDirection("backward");
    setStepIndex((i) => Math.max(i - 1, 0));
  };
  const finish = () => void completeOnboarding();

  /** El botón "Continuar" del pie: si el paso activo registró algo (hoy,
   * solo `CreateCharacterStep`), lo espera y solo avanza si no tiró error
   * — el propio paso ya muestra su toast de error, acá alcanza con quedarse
   * quieto para que el usuario pueda corregir y reintentar. */
  async function handleContinue() {
    if (submitRef.current) {
      setIsSubmitting(true);
      try {
        await submitRef.current();
      } catch {
        return;
      } finally {
        setIsSubmitting(false);
      }
    }
    goNext();
  }

  return (
    // Mismo "cascarón de ventana" que `SettingsDialog` (bg-surface, borde,
    // rounded-lg, shadow-md, ancho xl) en vez de ocupar toda la pantalla
    // borde a borde — pedido explícito del usuario de que la bienvenida se
    // vea como la ventana de Configuración. Centrada sobre el fondo de la
    // app (heredado del contenedor raíz en App.tsx).
    <div className="flex flex-1 items-center justify-center overflow-hidden p-6">
      <div className="flex w-full max-w-4xl flex-col rounded-lg border border-border bg-surface p-5 shadow-md">
        {/* Grid de 3 columnas iguales en vez de `justify-between`: así los
            puntos quedan centrados en el ANCHO TOTAL de la ventana pase lo
            que pase en la columna derecha (con o sin "Omitir todo"), en vez
            de centrarse solo respecto al espacio libre entre ambos
            elementos. */}
        <div className="grid grid-cols-[1fr_auto_1fr] items-center">
          <div />
          <div className="flex justify-center gap-1.5" role="img" aria-label={t("onboarding.stepIndicator", { current: stepIndex + 1, total: STEPS.length })}>
            {STEPS.map((s, i) => (
              <span
                key={s}
                className={cn(
                  "h-1.5 w-6 rounded-full transition-colors",
                  i === stepIndex ? "bg-accent" : i < stepIndex ? "bg-accent/40" : "bg-surface-secondary",
                )}
              />
            ))}
          </div>
          <div className="flex justify-end">
            {step !== "finish" ? (
              <Button variant="ghost" size="sm" onClick={finish}>
                {t("onboarding.skipAll")}
              </Button>
            ) : null}
          </div>
        </div>

        {/* Panel de contenido con su propio borde/alto fijo (h-[65vh]), igual
            que el panel de `SettingsDialog` — le da a cada paso el mismo
            "recuadro de ventana" en vez de que el contenido flote suelto.
            `m-auto` en vez de `justify-content: center`: con `margin: auto`
            en un hijo flex, el navegador lo centra cuando sobra espacio pero
            los márgenes se reducen a 0 cuando el contenido es más alto que
            el panel — nunca deja contenido en scroll negativo inalcanzable
            (lo que cortaba el título de Apariencia). Mismo contenedor para
            todos los pasos: se centran solos cuando entran (Bienvenida,
            Idioma, Personaje...) y arrancan arriba sin recortarse cuando no
            (Apariencia). */}
        <div className="mt-4 flex h-[65vh] flex-col overflow-y-auto rounded-md border border-border px-6 py-8">
          <div
            key={step}
            className={cn("m-auto w-full max-w-xl", direction === "forward" ? "animate-slide-in-right" : "animate-slide-in-left")}
          >
            {step === "intro" ? (
              <div className="flex flex-col items-center text-center">
                <img src="/app-icon.png" alt="" className="mb-6 h-36 w-36 rounded-3xl shadow-lg" />
                <h1 className="text-2xl font-semibold text-text-primary">{t("onboarding.intro.title")}</h1>
                <p className="mt-2 text-sm text-text-secondary">{t("onboarding.intro.description")}</p>
              </div>
            ) : step === "finish" ? (
              <div className="flex flex-col items-center text-center">
                <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-success/10 text-success">
                  <Check className="h-6 w-6" />
                </div>
                <h1 className="text-2xl font-semibold text-text-primary">{t("onboarding.finish.title")}</h1>
                <p className="mt-2 text-sm text-text-secondary">{t("onboarding.finish.description")}</p>
                <Button className="mt-6" onClick={finish}>
                  {t("onboarding.finishCta")}
                </Button>
              </div>
            ) : (
              <>
                <div className="mb-4">
                  <h2 className="text-lg font-semibold text-text-primary">{t(`onboarding.${step}.title`)}</h2>
                  <p className="mt-1 text-sm text-text-secondary">{t(`onboarding.${step}.description`)}</p>
                </div>
                {renderStep(step, { onAdvance: goNext, registerSubmit: (fn) => { submitRef.current = fn; } })}
              </>
            )}
          </div>
        </div>

        {/* Atrás/Continuar juntos y centrados dentro del mismo ancho que el
            contenido (max-w-xl) en vez de pegados a los bordes de toda la
            ventana. El slot de Continuar SIEMPRE se renderiza (con
            `invisible` en el último paso, no `null`) para que el par mida lo
            mismo en todo momento — si no, al desaparecer, "Atrás" queda solo
            y `justify-center` lo recentra de golpe en medio de la fila. */}
        <div className="mt-5 flex justify-center">
          <div className="mx-auto flex w-full max-w-xl justify-center gap-[340px]">
            <Button variant="secondary" onClick={goBack} disabled={stepIndex === 0}>
              {t("onboarding.back")}
            </Button>
            <div className={cn(step === "finish" && "invisible")}>
              <Button onClick={() => void handleContinue()} disabled={isSubmitting} tabIndex={step === "finish" ? -1 : undefined}>
                {isSubmitting ? t("identity.saving") : stepIndex === 0 ? t("onboarding.start") : t("onboarding.continue")}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
