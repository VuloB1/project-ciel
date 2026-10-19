import { useEffect, useMemo, useState } from "react";

import { useSettingsStore } from "@/stores/settingsStore";

import { regionOf, resolveFestivity, type FestivityArt, type FestivityId } from "./festivities";

/** The drawings weigh ~170 KB, so they only load once a festivity is actually showing. */
const artCache = new Map<FestivityId, FestivityArt>();

async function loadArt(id: FestivityId): Promise<FestivityArt> {
  const cached = artCache.get(id);
  if (cached) return cached;
  const { ART } = await import("./art.generated");
  const art = ART[id];
  artCache.set(id, art);
  return art;
}

export interface ActiveFestivity {
  id: FestivityId;
  art: FestivityArt;
  /** Whether the particles should move (the user's switch; the system's "reduce motion" is handled in CSS). */
  motion: boolean;
}

/** The festivity to decorate the app with right now, or `null` (setting off, no festivity today, art still loading). */
export function useFestivity(): ActiveFestivity | null {
  const mode = useSettingsStore((s) => s.appearance?.seasonal);
  const motion = useSettingsStore((s) => s.appearance?.seasonalMotion ?? true);
  // Re-check the date every hour so the app changes by itself when midnight passes.
  const [today, setToday] = useState(() => new Date().toDateString());
  useEffect(() => {
    const timer = setInterval(() => setToday(new Date().toDateString()), 3_600_000);
    return () => clearInterval(timer);
  }, []);

  const id = useMemo(
    () => resolveFestivity(mode, new Date(), regionOf(typeof navigator === "undefined" ? undefined : navigator.language)),
    // `today` is what makes the memo re-run when the date changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [mode, today],
  );

  const [loaded, setLoaded] = useState<{ id: FestivityId; art: FestivityArt } | null>(null);
  useEffect(() => {
    if (!id) return;
    let cancelled = false;
    void loadArt(id)
      .then((art) => {
        if (!cancelled) setLoaded({ id, art });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [id]);

  return id && loaded?.id === id ? { id, art: loaded.art, motion } : null;
}
