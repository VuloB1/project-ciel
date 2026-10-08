import { getAvatarStateImage, type AvatarVisualState } from "@/lib/ipc/avatar";
import { getProfileAvatarBase64 } from "@/lib/ipc/profiles";
import { useCachedImage } from "@/lib/useCachedImage";
import type { Profile } from "@/types/profile";

const STATE_PATH_KEY: Record<AvatarVisualState, keyof Pick<Profile, "avatarIdlePath" | "avatarThinkingPath" | "avatarTalkingPath">> = {
  idle: "avatarIdlePath",
  thinking: "avatarThinkingPath",
  talking: "avatarTalkingPath",
};

function useStateImage(profile: Profile | null, state: AvatarVisualState): string | null {
  const ownPath = profile ? profile[STATE_PATH_KEY[state]] : null;
  // Sin imagen propia cae a la foto normal del perfil; la clave refleja de dónde sale.
  const key = profile ? (ownPath ? `profile:${profile.id}:${state}:${ownPath}` : `profile:${profile.id}:${state}:fallback:${profile.avatarPath ?? ""}`) : null;
  return useCachedImage(key, async () => {
    const base64 = ownPath ? await getAvatarStateImage(profile!.id, state) : null;
    return base64 ?? (await getProfileAvatarBase64(profile!.id));
  });
}

/** Las tres imágenes del avatar reactivo (idle/thinking/talking), precargadas
 * y compartidas vía `imageCache.ts`. Antes el efecto dependía del objeto
 * `profile` entero, así que cualquier edición del perfil (cada autoguardado
 * de Identidad) volvía a pedir las tres por IPC. Ahora solo cambian si cambian
 * sus rutas. Para un estado sin imagen propia se usa la foto normal. */
export function useAvatarStateImages(profile: Profile | null): Record<AvatarVisualState, string | null> {
  return {
    idle: useStateImage(profile, "idle"),
    thinking: useStateImage(profile, "thinking"),
    talking: useStateImage(profile, "talking"),
  };
}
