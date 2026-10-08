import { useEffect, useState, useSyncExternalStore } from "react";

import { getImageVersion, loadCachedImage, peekCachedImage, subscribeImageVersion } from "@/lib/imageCache";

/** Hook común de los avatares y adjuntos: devuelve el `blob:` URL de la imagen
 * identificada por `key` (o `null` si `key` es null / no hay imagen). Ver
 * `imageCache.ts` para por qué se comparte entre componentes. */
export function useCachedImage(
  key: string | null,
  load: () => Promise<string | null>,
  mimeType?: string,
): string | null {
  const version = useSyncExternalStore(subscribeImageVersion, getImageVersion);
  const [, forceRender] = useState(0);

  useEffect(() => {
    if (!key || peekCachedImage(key) !== undefined) return;
    let cancelled = false;
    loadCachedImage(key, load, mimeType)
      .catch(() => null)
      .then(() => {
        if (!cancelled) forceRender((n) => n + 1);
      });
    return () => {
      cancelled = true;
    };
    // `load` y `mimeType` van determinados por `key`; `version` fuerza la relectura tras invalidar.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version]);

  if (!key) return null;
  return peekCachedImage(key) ?? null;
}
