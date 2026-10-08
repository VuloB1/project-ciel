import type { CSSProperties } from "react";

import { computeAccentTheme, DEFAULT_ACCENT_HEX } from "@/lib/theme/accentTheme";
import { useProfileAvatar } from "@/lib/useProfileAvatar";
import type { Profile } from "@/types/profile";

/** Small avatar badge that fetches its own image — reused anywhere a
 * profile needs to show its picture (falls back to a colored initial while
 * loading or when there's no avatar set). `size` takes a named preset, or a
 * raw pixel number for the rare spot that needs something in between (e.g.
 * chat bubbles) — always circular in that case. */
export function ProfileAvatarBadge({ profile, size }: { profile: Profile | null; size: "sm" | "md" | "lg" | number }) {
  const avatarUrl = useProfileAvatar(profile?.id, profile?.avatarPath);
  const dimensions =
    typeof size === "number"
      ? "rounded-full text-sm"
      : size === "lg"
        ? "h-16 w-16 rounded-xl text-2xl"
        : size === "md"
          ? "h-8 w-8 rounded-md text-sm"
          : "h-5 w-5 rounded-full text-[10px]";
  const backgroundColor = profile?.accentColor ?? DEFAULT_ACCENT_HEX;
  // La inicial se lee sobre el fondo: oscura sobre un acento claro (como el de
  // fábrica, #96fff3), blanca sobre uno oscuro.
  const style: CSSProperties = { backgroundColor, color: `rgb(${computeAccentTheme(backgroundColor).accentForeground.replace(/ /g, " ")})` };
  if (typeof size === "number") {
    style.width = size;
    style.height = size;
  }

  return (
    <div className={`flex shrink-0 items-center justify-center overflow-hidden font-semibold ${dimensions}`} style={style}>
      {avatarUrl ? (
        <img src={avatarUrl} alt="" className="h-full w-full object-cover" />
      ) : (
        (profile?.name ?? "A").slice(0, 1).toUpperCase()
      )}
    </div>
  );
}
