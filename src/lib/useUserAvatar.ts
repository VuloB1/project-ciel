import { getUserAvatarBase64 } from "@/lib/ipc/settings";
import { useCachedImage } from "@/lib/useCachedImage";
import { useSettingsStore } from "@/stores/settingsStore";

/** Avatar global del usuario como `blob:` URL compartido (ver `imageCache.ts`).
 * `userAvatarVersion` (contador que sube tras una subida) entra en la clave. */
export function useUserAvatar(): string | null {
  const version = useSettingsStore((s) => s.userAvatarVersion);
  return useCachedImage(`user:avatar:${version}`, getUserAvatarBase64);
}
