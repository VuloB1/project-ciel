import { Card } from "@/design-system/Card";
import { LANGUAGE_LABELS, SUPPORTED_LANGUAGES } from "@/lib/i18n";
import { cn } from "@/lib/theme/cn";
import { useSettingsStore } from "@/stores/settingsStore";

interface LanguageStepProps {
  onAdvance: () => void;
}

/** Elegir un idioma guarda y avanza en el mismo gesto — no hace falta un
 * botón "Continuar" aparte para esto, mismo espíritu que un selector de
 * idioma de cualquier wizard de primer uso. */
export function LanguageStep({ onAdvance }: LanguageStepProps) {
  const language = useSettingsStore((s) => s.language);
  const setLanguage = useSettingsStore((s) => s.setLanguage);

  async function handleSelect(lang: string) {
    if (lang !== language) {
      await setLanguage(lang);
    }
    onAdvance();
  }

  return (
    <Card className="grid grid-cols-2 gap-2 p-4 sm:grid-cols-3">
      {SUPPORTED_LANGUAGES.map((lang) => (
        <button
          key={lang}
          type="button"
          onClick={() => void handleSelect(lang)}
          className={cn(
            "rounded-md border px-4 py-3 text-sm font-medium transition-colors",
            lang === language
              ? "border-accent bg-surface-secondary text-accent"
              : "border-border text-text-primary hover:bg-surface-secondary",
          )}
        >
          {LANGUAGE_LABELS[lang]}
        </button>
      ))}
    </Card>
  );
}
