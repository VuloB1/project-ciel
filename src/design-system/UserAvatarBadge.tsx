import { User } from "lucide-react";
import type { CSSProperties } from "react";

import { useUserAvatar } from "@/lib/useUserAvatar";

/** The user's own avatar badge — same sizing as `ProfileAvatarBadge`
 * (named preset, or a raw pixel number, always circular in that case), but
 * for the one global (not per-character) picture, falling back to a
 * generic person icon instead of an initial (there's no "name" to pull a
 * letter from for the user). */
export function UserAvatarBadge({ size }: { size: "sm" | "md" | "lg" | number }) {
  const avatarUrl = useUserAvatar();
  const dimensions =
    typeof size === "number"
      ? "rounded-full"
      : size === "lg"
        ? "h-16 w-16 rounded-xl"
        : size === "md"
          ? "h-8 w-8 rounded-md"
          : "h-5 w-5 rounded-full";
  const iconSize = typeof size === "number" ? "h-5 w-5" : size === "lg" ? "h-7 w-7" : size === "md" ? "h-4 w-4" : "h-2.5 w-2.5";
  const style: CSSProperties | undefined = typeof size === "number" ? { width: size, height: size } : undefined;

  return (
    <div
      className={`flex shrink-0 items-center justify-center overflow-hidden bg-surface-secondary text-text-secondary ${dimensions}`}
      style={style}
    >
      {avatarUrl ? <img src={avatarUrl} alt="" className="h-full w-full object-cover" /> : <User className={iconSize} />}
    </div>
  );
}
