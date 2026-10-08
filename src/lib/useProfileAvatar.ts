import { getProfileAvatarBase64 } from "@/lib/ipc/profiles";
import { useCachedImage } from "@/lib/useCachedImage";

/** Avatar de un perfil como `blob:` URL compartido entre todos los
 * componentes (ver `imageCache.ts`). `avatarPath` entra en la clave de la
 * caché: cambia al subir otra imagen con otra extensión; si la ruta queda
 * igual, quien sube llama a `invalidateCachedImages`. */
export function useProfileAvatar(profileId: string | null | undefined, avatarPath: string | null | undefined): string | null {
  const key = profileId && avatarPath ? `profile:${profileId}:avatar:${avatarPath}` : null;
  return useCachedImage(key, () => getProfileAvatarBase64(profileId as string));
}
