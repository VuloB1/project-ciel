import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { Badge, type BadgeVariant } from "@/design-system/Badge";
import { useMoodStore } from "@/stores/moodStore";

type Tier = "low" | "mid" | "high";

function tier(value: number): Tier {
  if (value <= 33) return "low";
  if (value <= 66) return "mid";
  return "high";
}

// Mismos 9 tiers que `aivt_orchestrator::MoodPromptBuilder` — el color
// depende solo de valence (bajo=warning, neutral=neutral, alto=success),
// la etiqueta refleja además la energía.
const VARIANT_BY_VALENCE: Record<Tier, BadgeVariant> = { low: "warning", mid: "neutral", high: "success" };
const CAPITALIZED: Record<Tier, string> = { low: "Low", mid: "Mid", high: "High" };

/** Pequeño indicador de "cómo está" el personaje ahora mismo — distinto
 * de su personalidad (fija). Ver `identity.personality` para lo estático. */
export function MoodBadge({ profileId }: { profileId: string }) {
  const { t } = useTranslation();
  const mood = useMoodStore((s) => s.moodByProfile[profileId]);
  const load = useMoodStore((s) => s.load);

  useEffect(() => {
    void load(profileId);
  }, [profileId, load]);

  if (!mood) return null;

  const valenceTier = tier(mood.valence);
  const energyTier = tier(mood.energy);
  const key = valenceTier === "mid" && energyTier === "mid" ? "neutral" : `${valenceTier}${CAPITALIZED[energyTier]}`;

  return <Badge variant={VARIANT_BY_VALENCE[valenceTier]}>{t(`mood.states.${key}`)}</Badge>;
}
