import { getAttachmentBase64 } from "@/lib/ipc/chat";
import { useCachedImage } from "@/lib/useCachedImage";

/** Imagen de un adjunto del chat como `blob:` URL, una sola petición por id
 * (ver `imageCache.ts`). Los adjuntos son inmutables, así que no se invalidan. */
export function useAttachmentImage(attachmentId: string | null | undefined, mimeType: string | null | undefined): string | null {
  const key = attachmentId && mimeType ? `attachment:${attachmentId}` : null;
  return useCachedImage(key, () => getAttachmentBase64(attachmentId as string), mimeType ?? undefined);
}
