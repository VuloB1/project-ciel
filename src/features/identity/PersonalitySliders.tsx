import { useTranslation } from "react-i18next";

import { Slider } from "@/design-system/Slider";
import { Textarea } from "@/design-system/Textarea";
import { PERSONALITY_TRAIT_KEYS, type ProfilePersonality } from "@/types/profile";

export interface PersonalitySlidersProps {
  value: ProfilePersonality;
  onChange: (next: ProfilePersonality) => void;
}

/** The free description + 19 sliders from sección 11 del brief. Values
 * never go to the model as numbers — see `aivt_orchestrator::PersonalityPromptBuilder`. */
export function PersonalitySliders({ value, onChange }: PersonalitySlidersProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-5">
      <div>
        <label className="mb-1.5 block text-sm font-medium text-text-primary" htmlFor="personality-free-description">
          {t("identity.personality.freeDescription")}
        </label>
        <Textarea
          autoGrow
          id="personality-free-description"
          rows={3}
          value={value.freeDescription}
          onChange={(event) => onChange({ ...value, freeDescription: event.target.value })}
          placeholder={t("identity.personality.freeDescriptionPlaceholder")}
        />
      </div>

      <div className="grid grid-cols-1 gap-x-8 gap-y-4 sm:grid-cols-2">
        {PERSONALITY_TRAIT_KEYS.map((key) => (
          <div key={key} className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between text-sm">
              <span className="text-text-primary">{t(`identity.personality.traits.${key}`)}</span>
              <span className="tabular-nums text-text-secondary">{value[key]}</span>
            </div>
            <Slider
              label={t(`identity.personality.traits.${key}`)}
              value={value[key]}
              onValueChange={(next) => onChange({ ...value, [key]: next })}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
