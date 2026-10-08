/** Caché compartida de imágenes que llegan del backend como base64 (avatares,
 * adjuntos). Antes cada componente que mostraba una imagen la pedía por IPC y
 * armaba su propio `data:` URI: con un avatar de 1,6 MB repetido en cada
 * burbuja del chat eran decenas de transferencias y decodificaciones iguales,
 * y la ventana se trababa. Ahora se pide una sola vez por clave, las peticiones
 * simultáneas comparten la misma promesa, y todos reciben el mismo `blob:` URL
 * (el navegador decodifica la imagen una sola vez). */

const resolved = new Map<string, string | null>();
const inflight = new Map<string, Promise<string | null>>();

function sniffMime(bytes: Uint8Array): string {
  if (bytes[0] === 0x89 && bytes[1] === 0x50) return "image/png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return "image/jpeg";
  if (bytes[0] === 0x47 && bytes[1] === 0x49) return "image/gif";
  if (bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[8] === 0x57) return "image/webp";
  return "image/png";
}

function base64ToBlobUrl(base64: string, mimeType?: string): string {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return URL.createObjectURL(new Blob([bytes], { type: mimeType ?? sniffMime(bytes) }));
}

/** Sincrónico: lo ya cargado (`undefined` = aún no se pidió). Permite pintar
 * la imagen en el primer render sin parpadeo cuando ya estaba en caché. */
export function peekCachedImage(key: string): string | null | undefined {
  return resolved.get(key);
}

/** Devuelve el `blob:` URL de la imagen, pidiéndola con `load` solo si nadie
 * la pidió antes con la misma `key`. Un fallo no se guarda, para reintentar. */
export function loadCachedImage(key: string, load: () => Promise<string | null>, mimeType?: string): Promise<string | null> {
  if (resolved.has(key)) return Promise.resolve(resolved.get(key) ?? null);
  const pending = inflight.get(key);
  if (pending) return pending;

  const promise = load()
    .then((base64) => {
      const url = base64 ? base64ToBlobUrl(base64, mimeType) : null;
      resolved.set(key, url);
      return url;
    })
    .finally(() => {
      inflight.delete(key);
    });
  inflight.set(key, promise);
  return promise;
}

let version = 0;
const listeners = new Set<() => void>();

/** Contador que sube cada vez que se invalida algo: los hooks lo leen con
 * `useSyncExternalStore` para volver a pedir la imagen tras una subida, aunque
 * la ruta del archivo no haya cambiado (re-subir `avatar.png` sobre `avatar.png`). */
export function subscribeImageVersion(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
export function getImageVersion(): number {
  return version;
}

/** Olvida las imágenes cuya clave empieza con `prefix` y avisa a los hooks.
 * No libera los `blob:` URL: algún componente aún puede estar mostrándolos
 * hasta que recargue; son pocos y pequeños frente al costo de un parpadeo. */
export function invalidateCachedImages(prefix: string): void {
  for (const key of [...resolved.keys()]) {
    if (key.startsWith(prefix)) resolved.delete(key);
  }
  version += 1;
  listeners.forEach((listener) => listener());
}

/** Solo para pruebas. */
export function clearImageCache(): void {
  for (const url of resolved.values()) if (url) URL.revokeObjectURL(url);
  resolved.clear();
  inflight.clear();
  version = 0;
}
